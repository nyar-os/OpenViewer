//! In-process image sniffing, metadata, decoding and thumbnails.
//!
//! All decoding runs inside this process through the `image` crate. Nothing in
//! this module shells out to a converter, so the capability report can honestly
//! describe what the build itself can do.

use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
};

use image::{ColorType, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder, ImageFormat, ImageReader};
use serde::Serialize;

use crate::error::{Error, Result};

/// Pixel ceiling for decode and thumbnail work (100 megapixels).
pub const MAX_PIXELS: u64 = 100_000_000;
/// Largest thumbnail edge accepted by the public API.
pub const MAX_THUMBNAIL_EDGE: u32 = 8192;

/// Formats this build decodes, in process.
pub const SUPPORTED_FORMATS: &[&str] = &["bmp", "gif", "ico", "jpeg", "png", "pnm", "tga", "tiff", "webp"];

/// Metadata read without decoding pixels.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    /// Absolute path as supplied by the caller.
    pub path: String,
    /// Container format name.
    pub format: String,
    /// Decoded width in pixels.
    pub width: u32,
    /// Decoded height in pixels.
    pub height: u32,
    /// `width * height`.
    pub pixel_count: u64,
    /// File size in bytes.
    pub file_size: u64,
    /// Native colour type reported by the decoder.
    pub color_type: String,
    /// Bits per pixel reported by the decoder.
    pub bits_per_pixel: u16,
    /// Whether the native colour type carries alpha.
    pub has_alpha: bool,
    /// Frame count when the container makes it cheap to count, otherwise `null`.
    pub frames: Option<u32>,
    /// Whether the container declares animation.
    pub animated: bool,
    /// Whether the image is under [`MAX_PIXELS`].
    pub within_pixel_limit: bool,
}

/// Fully decoded RGBA8 pixels.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Row-major RGBA8 bytes, `width * height * 4` long.
    pub rgba: Vec<u8>,
}

/// PNG-encoded thumbnail plus source dimensions.
#[derive(Debug, Clone)]
pub struct Thumbnail {
    /// Source width.
    pub source_width: u32,
    /// Source height.
    pub source_height: u32,
    /// Thumbnail width.
    pub width: u32,
    /// Thumbnail height.
    pub height: u32,
    /// PNG bytes.
    pub png: Vec<u8>,
}

/// Preview payload, structurally identical to [`Thumbnail`].
pub type PreviewImage = Thumbnail;

/// Result of writing a thumbnail to disk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailWrite {
    /// Written path.
    pub path: String,
    /// Thumbnail width.
    pub width: u32,
    /// Thumbnail height.
    pub height: u32,
    /// Source width.
    pub source_width: u32,
    /// Source height.
    pub source_height: u32,
    /// PNG byte count.
    pub bytes: u64,
}

/// Read metadata for `path` without decoding pixels.
pub fn info(path: impl AsRef<Path>) -> Result<ImageInfo> {
    let path = path.as_ref();
    let file_size = fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::not_found(format!("image not found: {}", path.display()))
        }
        else {
            Error::io(error)
        }
    })?;

    let format = sniff_format(path)?;
    let reader = ImageReader::with_format(BufReader::new(File::open(path).map_err(Error::io)?), format);
    let decoder = reader.into_decoder().map_err(|error| Error::decode(error.to_string()))?;
    let (width, height) = decoder.dimensions();
    let color_type = decoder.color_type();
    drop(decoder);

    let pixel_count = u64::from(width).saturating_mul(u64::from(height));
    let (frames, animated) = animation_facts(path, format)?;

    Ok(ImageInfo {
        path: path.display().to_string(),
        format: format_name(format).to_string(),
        width,
        height,
        pixel_count,
        file_size: file_size.len(),
        color_type: color_name(color_type).to_string(),
        bits_per_pixel: color_type.bits_per_pixel(),
        has_alpha: color_type.has_alpha(),
        frames,
        animated,
        within_pixel_limit: pixel_count <= MAX_PIXELS,
    })
}

/// Decode `path` to RGBA8, enforcing [`MAX_PIXELS`].
pub fn decode(path: impl AsRef<Path>) -> Result<DecodedImage> {
    let path = path.as_ref();
    let (_format, width, height) = dimensions(path)?;
    enforce_pixel_limit(width, height)?;

    let reader = ImageReader::open(path).map_err(Error::io)?;
    let decoded = reader.decode().map_err(|error| Error::decode(error.to_string()))?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(DecodedImage { width, height, rgba: rgba.into_raw() })
}

/// Render a PNG thumbnail that fits inside `max_edge` without upscaling.
pub fn thumbnail(path: impl AsRef<Path>, max_edge: u32) -> Result<Thumbnail> {
    validate_max_edge(max_edge)?;
    let path = path.as_ref();
    let (_format, width, height) = dimensions(path)?;
    enforce_pixel_limit(width, height)?;

    let reader = ImageReader::open(path).map_err(Error::io)?;
    let decoded = reader.decode().map_err(|error| Error::decode(error.to_string()))?;
    let (width, height) = decoded.dimensions();

    let scaled = if width <= max_edge && height <= max_edge { decoded } else { decoded.thumbnail(max_edge, max_edge) };
    let rgba = scaled.to_rgba8();
    let (thumb_width, thumb_height) = rgba.dimensions();

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(rgba.as_raw(), thumb_width, thumb_height, ExtendedColorType::Rgba8)
        .map_err(|error| Error::decode(format!("failed to encode thumbnail PNG: {error}")))?;

    Ok(Thumbnail { source_width: width, source_height: height, width: thumb_width, height: thumb_height, png })
}

/// Render the same thumbnail payload used by the desktop preview pane.
pub fn preview(path: impl AsRef<Path>, max_edge: u32) -> Result<PreviewImage> {
    thumbnail(path, max_edge)
}

/// Render a thumbnail and write it as a PNG file.
pub fn write_thumbnail(path: impl AsRef<Path>, out: impl AsRef<Path>, max_edge: u32) -> Result<ThumbnailWrite> {
    let path = path.as_ref();
    let out = out.as_ref();
    if same_file(path, out)? {
        return Err(Error::invalid("thumbnail output must not overwrite the source image"));
    }
    let thumbnail = thumbnail(path, max_edge)?;
    fs::write(out, &thumbnail.png).map_err(Error::io)?;
    Ok(ThumbnailWrite {
        path: out.display().to_string(),
        width: thumbnail.width,
        height: thumbnail.height,
        source_width: thumbnail.source_width,
        source_height: thumbnail.source_height,
        bytes: thumbnail.png.len() as u64,
    })
}

fn validate_max_edge(max_edge: u32) -> Result<()> {
    if max_edge == 0 {
        return Err(Error::invalid("maxEdge must be greater than zero"));
    }
    if max_edge > MAX_THUMBNAIL_EDGE {
        return Err(Error::invalid(format!("maxEdge must not exceed {MAX_THUMBNAIL_EDGE}")));
    }
    Ok(())
}

fn dimensions(path: &Path) -> Result<(ImageFormat, u32, u32)> {
    let format = sniff_format(path)?;
    let reader = ImageReader::with_format(BufReader::new(File::open(path).map_err(Error::io)?), format);
    let (width, height) = reader.into_dimensions().map_err(|error| Error::decode(error.to_string()))?;
    Ok((format, width, height))
}

fn enforce_pixel_limit(width: u32, height: u32) -> Result<()> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > MAX_PIXELS {
        return Err(Error::pixel_limit(width, height, MAX_PIXELS));
    }
    Ok(())
}

fn sniff_format(path: &Path) -> Result<ImageFormat> {
    let mut head = [0u8; 32];
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::not_found(format!("image not found: {}", path.display()))
        }
        else {
            Error::io(error)
        }
    })?;
    let read = file.read(&mut head).map_err(Error::io)?;

    if let Ok(format) = image::guess_format(&head[..read]) {
        return Ok(format);
    }

    if let Some(extension) = path.extension().and_then(|value| value.to_str())
        && let Some(format) = ImageFormat::from_extension(extension)
        && SUPPORTED_FORMATS.contains(&format_name(format))
    {
        return Ok(format);
    }

    Err(Error::unsupported_format(format!("{} (supported: {})", path.display(), SUPPORTED_FORMATS.join(", "))))
}

fn animation_facts(path: &Path, format: ImageFormat) -> Result<(Option<u32>, bool)> {
    match format {
        ImageFormat::Gif => {
            let frames = gif_frame_count(path)?;
            Ok((Some(frames), frames > 1))
        }
        ImageFormat::WebP => {
            let reader = BufReader::new(File::open(path).map_err(Error::io)?);
            let decoder = image::codecs::webp::WebPDecoder::new(reader).map_err(|error| Error::decode(error.to_string()))?;
            Ok((None, decoder.has_animation()))
        }
        _ => Ok((None, false)),
    }
}

fn gif_frame_count(path: &Path) -> Result<u32> {
    let file = BufReader::new(File::open(path).map_err(Error::io)?);
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options.read_info(file).map_err(|error| Error::decode(format!("failed to read GIF: {error}")))?;
    let mut frames = 0u32;
    loop {
        let frame = decoder.read_next_frame().map_err(|error| Error::decode(format!("failed to read GIF frame: {error}")))?;
        if frame.is_none() {
            break;
        }
        frames = frames.saturating_add(1);
    }
    Ok(frames)
}

fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Bmp => "bmp",
        ImageFormat::Gif => "gif",
        ImageFormat::Ico => "ico",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Png => "png",
        ImageFormat::Pnm => "pnm",
        ImageFormat::Tga => "tga",
        ImageFormat::Tiff => "tiff",
        ImageFormat::WebP => "webp",
        _ => "unknown",
    }
}

fn color_name(color: ColorType) -> &'static str {
    match color {
        ColorType::L8 => "luma8",
        ColorType::La8 => "luma-alpha8",
        ColorType::Rgb8 => "rgb8",
        ColorType::Rgba8 => "rgba8",
        ColorType::L16 => "luma16",
        ColorType::La16 => "luma-alpha16",
        ColorType::Rgb16 => "rgb16",
        ColorType::Rgba16 => "rgba16",
        ColorType::Rgb32F => "rgb32f",
        ColorType::Rgba32F => "rgba32f",
        _ => "unknown",
    }
}

fn same_file(source: &Path, output: &Path) -> Result<bool> {
    let source = fs::canonicalize(source).map_err(Error::io)?;
    let output = if output.exists() {
        fs::canonicalize(output).map_err(Error::io)?
    }
    else {
        let parent = match output.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        fs::canonicalize(parent).map_err(Error::io)?.join(output.file_name().unwrap_or_default())
    };
    Ok(source == output)
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use image::{ExtendedColorType, ImageEncoder};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("open-viewer-image-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_png(path: &Path, width: u32, height: u32) {
        let pixels: Vec<u8> = (0..width * height * 4).map(|index| (index % 251) as u8).collect();
        let file = File::create(path).unwrap();
        image::codecs::png::PngEncoder::new(file).write_image(&pixels, width, height, ExtendedColorType::Rgba8).unwrap();
    }

    #[test]
    fn metadata_reads_dimensions_without_decoding() {
        let dir = temp_dir();
        let path = dir.join("sample.png");
        write_png(&path, 12, 7);
        let info = info(&path).unwrap();
        assert_eq!(info.format, "png");
        assert_eq!(info.width, 12);
        assert_eq!(info.height, 7);
        assert_eq!(info.pixel_count, 84);
        assert_eq!(info.color_type, "rgba8");
        assert_eq!(info.frames, None);
        assert!(!info.animated);
        assert!(info.within_pixel_limit);
    }

    #[test]
    fn decode_returns_rgba_pixels() {
        let dir = temp_dir();
        let path = dir.join("sample.png");
        write_png(&path, 3, 2);
        let decoded = decode(&path).unwrap();
        assert_eq!(decoded.width, 3);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.rgba.len(), 3 * 2 * 4);
    }

    #[test]
    fn thumbnail_does_not_upscale_small_images() {
        let dir = temp_dir();
        let path = dir.join("sample.png");
        write_png(&path, 8, 6);
        let thumb = thumbnail(&path, 16).unwrap();
        assert_eq!(thumb.width, 8);
        assert_eq!(thumb.height, 6);
        assert!(!thumb.png.is_empty());
    }

    #[test]
    fn thumbnail_fits_and_preserves_aspect_ratio() {
        let dir = temp_dir();
        let path = dir.join("sample.png");
        write_png(&path, 40, 20);
        let thumb = thumbnail(&path, 10).unwrap();
        assert_eq!(thumb.width, 10);
        assert_eq!(thumb.height, 5);
    }

    #[test]
    fn zero_edge_is_rejected() {
        let dir = temp_dir();
        let path = dir.join("sample.png");
        write_png(&path, 4, 4);
        let error = thumbnail(&path, 0).unwrap_err();
        assert_eq!(error.code(), crate::codes::INVALID_ARGUMENT);
    }

    #[test]
    fn unsupported_content_reports_a_stable_code() {
        let dir = temp_dir();
        let path = dir.join("notes.txt");
        fs::write(&path, "not an image").unwrap();
        let error = info(&path).unwrap_err();
        assert_eq!(error.code(), crate::codes::UNSUPPORTED_FORMAT);
    }

    #[test]
    fn write_thumbnail_produces_a_png_file() {
        let dir = temp_dir();
        let source = dir.join("sample.png");
        let output = dir.join("thumb.png");
        write_png(&source, 40, 20);
        let write = write_thumbnail(&source, &output, 10).unwrap();
        assert_eq!(write.width, 10);
        assert_eq!(write.height, 5);
        assert!(output.is_file());
        assert!(write.bytes > 0);
    }

    #[test]
    fn writing_over_the_source_is_refused() {
        let dir = temp_dir();
        let source = dir.join("sample.png");
        write_png(&source, 4, 4);
        let error = write_thumbnail(&source, &source, 4).unwrap_err();
        assert_eq!(error.code(), crate::codes::INVALID_ARGUMENT);
    }
}

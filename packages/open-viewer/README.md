# OpenViewer

Inspect local images from the command line or a desktop window. OpenViewer reads
format, dimensions, colour type and frame metadata, decodes pixels, and renders
PNG previews and thumbnails inside the process. It is a viewer, not an editor:
there is no batch re-encoding, drawing or destructive conversion.

## Install

```bash
npm install -g @wae/open-viewer
```

The native addon ships for Windows x64. Other hosts have no binary and are not
claimed as supported.

## Commands

| Command | Purpose |
|---|---|
| `open-viewer info <path>` | Print format, dimensions, colour type and file size |
| `open-viewer thumbnail <path> --out <path>` | Render a PNG thumbnail (`--dry-run` previews, `--max` sets the edge limit) |
| `open-viewer capabilities` | List capability states for this build |
| `open-viewer audit` | Query the append-only audit log |
| `open-viewer install` | Plan an install or upgrade (`--apply` applies) |
| `open-viewer uninstall` | Plan a removal (`--apply` applies) |
| `open-viewer gui` | Open the desktop viewer window |
| `open-viewer version` | Print the package version |

`info`, `thumbnail` and `capabilities` accept `--json`; every command accepts
`--actor`, `--actor-kind` and `--actor-source` for audit attribution. Exit codes
are stable: `0` success, `1` runtime failure, `2` usage error, `3` capability
unavailable.

## Supported formats

`bmp`, `gif`, `ico`, `jpeg`, `png`, `pnm`, `tga`, `tiff`, `webp`. Detection
sniffs file bytes and falls back to the extension. Decoding is refused above
100,000,000 pixels, and thumbnails never upscale.

Animation playback, EXIF fields, ICC colour management, camera RAW and
HEIC/HEIF are not implemented in this build. They are reported as
`not_implemented` or `dependency_blocked` with a stable code rather than
silently degraded.

## Capability states

Every capability reports `ready`, or one of `unsupported_platform`,
`not_implemented` or `dependency_blocked`. The same refusal applies to the CLI,
the desktop window and agent skills, because the native module is the only
implementation path.

## Decoding in process

Image parsing, scaling and PNG encoding use libraries linked into the addon.
OpenViewer does not start external binaries or shell pipelines for image work.

## Development

```bash
pnpm install
pnpm run build:napi
pnpm run build:ts
pnpm run build:ui
pnpm run check:skills
```

`build:napi` compiles the Rust addon into the platform package, `build:ui`
builds the static window interface into `dist/gui`. Run `pnpm run gui` for the
desktop window.

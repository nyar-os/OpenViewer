//! Stable error codes and the structured capability error shape.

use std::{fmt, io};

use crate::capability::CapabilityState;

/// Stable error and capability codes shared with the CLI surface.
pub mod codes {
    /// A caller supplied an invalid argument.
    pub const INVALID_ARGUMENT: &str = "invalid_argument";
    /// A filesystem read or write failed.
    pub const IO_ERROR: &str = "io_error";
    /// The requested path does not exist.
    pub const NOT_FOUND: &str = "not_found";
    /// The file is not a format this build decodes.
    pub const UNSUPPORTED_FORMAT: &str = "unsupported_format";
    /// The bytes are a supported container but failed to decode.
    pub const DECODE_ERROR: &str = "decode_error";
    /// The decoded pixel count exceeds the configured ceiling.
    pub const PIXEL_LIMIT: &str = "pixel_limit";
    /// The capability exists in the product contract but is not implemented.
    pub const NOT_IMPLEMENTED: &str = "not_implemented";
    /// A required in-process library is missing or cannot be shipped.
    pub const DEPENDENCY_BLOCKED: &str = "dependency_blocked";
    /// The current target has no backend for this capability.
    pub const UNSUPPORTED_PLATFORM: &str = "unsupported_platform";
    /// A capability that is normally available reported `ready`.
    pub const READY: &str = "ready";
    /// A second launch was rejected by the declared window policy.
    pub const WINDOW_SECOND_LAUNCH_REJECTED: &str = "window_second_launch_rejected";
    /// The declared window policy carried a value the host does not understand.
    pub const WINDOW_CONFIG_INVALID: &str = "window_config_invalid";
    /// A non-capability message without a more specific code.
    pub const MESSAGE: &str = "message";
}

/// Domain error carrying a stable code and an optional capability state.
#[derive(Debug, Clone)]
pub struct Error {
    code: &'static str,
    state: Option<CapabilityState>,
    message: String,
}

impl Error {
    /// Build a plain message error.
    pub fn message(message: impl Into<String>) -> Self {
        Self { code: codes::MESSAGE, state: None, message: message.into() }
    }

    /// Wrap an I/O error.
    pub fn io(error: io::Error) -> Self {
        Self { code: codes::IO_ERROR, state: None, message: error.to_string() }
    }

    /// A caller supplied an invalid argument.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self { code: codes::INVALID_ARGUMENT, state: None, message: message.into() }
    }

    /// A request refused by a declared policy, with a stable script-facing code.
    pub fn policy(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, state: None, message: message.into() }
    }

    /// A path does not exist.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self { code: codes::NOT_FOUND, state: None, message: message.into() }
    }

    /// The container is not in the supported format set.
    pub fn unsupported_format(format: impl fmt::Display) -> Self {
        Self { code: codes::UNSUPPORTED_FORMAT, state: None, message: format!("unsupported image format: {format}") }
    }

    /// Decoding failed after the container was recognised.
    pub fn decode(message: impl Into<String>) -> Self {
        Self { code: codes::DECODE_ERROR, state: None, message: message.into() }
    }

    /// The decoded pixel count is above the configured ceiling.
    pub fn pixel_limit(width: u32, height: u32, max: u64) -> Self {
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        Self {
            code: codes::PIXEL_LIMIT,
            state: None,
            message: format!("image is {width}x{height} ({pixels} pixels), above the {max} pixel ceiling"),
        }
    }

    /// A structured capability refusal.
    pub fn capability(state: CapabilityState, detail: impl Into<String>) -> Self {
        Self { code: state.code(), state: Some(state), message: detail.into() }
    }

    /// Stable error code.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Capability state when the error is a refusal rather than a failure.
    pub fn capability_state(&self) -> Option<&CapabilityState> {
        self.state.as_ref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Core result type.
pub type Result<T> = std::result::Result<T, Error>;

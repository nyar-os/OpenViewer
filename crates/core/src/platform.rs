//! Runtime platform identity and capability availability.

use serde::Serialize;

/// Platform identity reported by the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    /// Windows.
    Windows,
    /// macOS.
    Macos,
    /// Linux.
    Linux,
    /// Any other compile target.
    Other,
}

impl Platform {
    /// Platform of the running build.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        }
        else if cfg!(target_os = "macos") {
            Self::Macos
        }
        else if cfg!(target_os = "linux") {
            Self::Linux
        }
        else {
            Self::Other
        }
    }

    /// Stable lowercase name used in reports and install path planning.
    pub fn name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::Other => "other",
        }
    }

    /// Whether a desktop WebView host exists for this target.
    pub fn has_webview(self) -> bool {
        matches!(self, Self::Windows | Self::Macos | Self::Linux)
    }
}

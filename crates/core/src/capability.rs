//! Capability report with the shared three-state refusal vocabulary.

use serde::Serialize;

use crate::{error::codes, platform::Platform};

/// Schema id of the capability report payload.
pub const CAPABILITY_SCHEMA: &str = "open-viewer.capabilities/v0";

const APP_ID: &str = "open-viewer";

/// State of one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CapabilityState {
    /// The capability is implemented for this build and target.
    Ready,
    /// The target lacks a backend for this capability.
    UnsupportedPlatform {
        /// Platform that produced the report.
        platform: Platform,
        /// Stable state code.
        code: &'static str,
    },
    /// The product promises the capability but this build does not implement it.
    NotImplemented {
        /// Stable state code.
        code: &'static str,
        /// What is missing.
        detail: String,
    },
    /// A required in-process dependency is unavailable or not shippable.
    DependencyBlocked {
        /// Stable state code.
        code: &'static str,
        /// What dependency is missing.
        detail: String,
    },
}

impl CapabilityState {
    /// A structured `not_implemented` state.
    pub fn not_implemented(detail: impl Into<String>) -> Self {
        Self::NotImplemented { code: codes::NOT_IMPLEMENTED, detail: detail.into() }
    }

    /// A structured `dependency_blocked` state.
    pub fn dependency_blocked(detail: impl Into<String>) -> Self {
        Self::DependencyBlocked { code: codes::DEPENDENCY_BLOCKED, detail: detail.into() }
    }

    /// A structured `unsupported_platform` state.
    pub fn unsupported_platform(platform: Platform) -> Self {
        Self::UnsupportedPlatform { platform, code: codes::UNSUPPORTED_PLATFORM }
    }

    /// Stable state code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Ready => codes::READY,
            Self::UnsupportedPlatform { code, .. } | Self::NotImplemented { code, .. } | Self::DependencyBlocked { code, .. } => code,
        }
    }

    /// Whether the capability can be used on this build.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// One capability entry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityEntry {
    /// Stable capability id.
    pub id: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// Current state.
    pub state: CapabilityState,
    /// Formats covered by the capability, when applicable.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<&'static str>,
}

/// Full capability report.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReport {
    /// Report schema id.
    pub schema: &'static str,
    /// Application id.
    pub app: &'static str,
    /// Application version.
    pub version: &'static str,
    /// Runtime platform.
    pub platform: Platform,
    /// Capability entries.
    pub entries: Vec<CapabilityEntry>,
}

fn entry(id: &'static str, title: &'static str, state: CapabilityState) -> CapabilityEntry {
    CapabilityEntry { id, title, state, formats: Vec::new() }
}

/// Build the capability report for the running target.
pub fn capability_report() -> CapabilityReport {
    let platform = Platform::current();
    let entries = vec![
        CapabilityEntry {
            id: "image-decode",
            title: "Decode image pixels",
            state: CapabilityState::Ready,
            formats: crate::image_data::SUPPORTED_FORMATS.to_vec(),
        },
        entry("image-metadata", "Read image metadata", CapabilityState::Ready),
        entry("image-thumbnail", "Create a PNG thumbnail", CapabilityState::Ready),
        entry("image-preview", "Encode a preview data URL", CapabilityState::Ready),
        entry(
            "image-animation-playback",
            "Animated playback",
            CapabilityState::not_implemented("frame metadata is readable, but the viewer does not play animation timelines"),
        ),
        entry("image-exif", "EXIF metadata", CapabilityState::not_implemented("no in-process EXIF reader is wired into this build")),
        entry(
            "image-color-management",
            "ICC colour management",
            CapabilityState::not_implemented("embedded ICC profiles are not applied; pixels are reported in the file's native space"),
        ),
        entry(
            "image-raw",
            "Camera RAW decode",
            CapabilityState::dependency_blocked("no in-process Rust RAW decoder with an acceptable license is available in this build"),
        ),
        entry(
            "image-heic",
            "HEIC/HEIF decode",
            CapabilityState::dependency_blocked("no in-process HEIC decoder with an acceptable license is available in this build"),
        ),
        entry(
            "gui-host",
            "Desktop viewer window",
            if platform.has_webview() { CapabilityState::Ready } else { CapabilityState::unsupported_platform(platform) },
        ),
        entry("install-plan", "Plan host installation", CapabilityState::Ready),
        entry(
            "install-apply",
            "Apply host installation",
            CapabilityState::not_implemented("the plan is real and audited; per-user config and launcher writes are pending"),
        ),
        entry(
            "uninstall-apply",
            "Apply host removal",
            CapabilityState::not_implemented("the plan is real and audited; per-user removal is pending"),
        ),
    ];

    CapabilityReport { schema: CAPABILITY_SCHEMA, app: APP_ID, version: env!("CARGO_PKG_VERSION"), platform, entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_covers_all_three_refusal_states() {
        let report = capability_report();
        let states: Vec<&'static str> = report.entries.iter().map(|entry| entry.state.code()).collect();
        assert!(states.contains(&codes::NOT_IMPLEMENTED));
        assert!(states.contains(&codes::DEPENDENCY_BLOCKED));
        if !Platform::current().has_webview() {
            assert!(states.contains(&codes::UNSUPPORTED_PLATFORM));
        }
        assert!(report.entries.iter().any(|entry| entry.id == "image-decode" && entry.state.is_ready()));
    }

    #[test]
    fn report_serialises_ready_state_without_fake_details() {
        let value = serde_json::to_value(capability_report()).unwrap();
        let decode = value["entries"].as_array().unwrap().iter().find(|entry| entry["id"] == "image-decode").unwrap();
        assert_eq!(decode["state"]["state"], "ready");
        assert_eq!(decode["state"].as_object().unwrap().len(), 1);
        assert_eq!(value["schema"], CAPABILITY_SCHEMA);
    }
}

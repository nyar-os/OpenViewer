//! OpenViewer domain core.
//!
//! The core owns the viewer's only implementation of image sniffing, decoding,
//! metadata and thumbnail creation. Every expensive operation runs in process
//! with a library dependency; the crate never spawns an external process.
//!
//! Capability refusals use the three stable states from the shared open-app
//! contract: `unsupported_platform`, `not_implemented` and `dependency_blocked`.

#![deny(missing_docs)]

mod audit;
mod capability;
mod error;
mod image_data;
mod install;
mod platform;

pub use audit::{AuditMode, Operation};
pub use capability::{CAPABILITY_SCHEMA, CapabilityEntry, CapabilityReport, CapabilityState, capability_report};
pub use error::{Error, Result, codes};
pub use image_data::{
    DecodedImage, ImageInfo, MAX_PIXELS, MAX_THUMBNAIL_EDGE, PreviewImage, SUPPORTED_FORMATS, Thumbnail, ThumbnailWrite, decode, info, preview,
    thumbnail, write_thumbnail,
};
pub use install::{
    APP_ID, APP_TITLE, ActionKind, InstallOptions, InstallPlan, PlannedAction, UninstallOptions, UninstallPlan, apply_install, apply_uninstall,
    plan_install, plan_install_with, plan_uninstall, plan_uninstall_with,
};
pub use platform::Platform;

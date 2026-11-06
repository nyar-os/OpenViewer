//! Audit-facing operation metadata owned by the core.
//!
//! The N-API layer owns the actual `wae.audit/v0` writer. This module keeps the
//! capability names and read/write classification next to the domain so the CLI,
//! GUI and N-API cannot invent a divergent audit vocabulary.

/// Audit mode mirror of the shared audit contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditMode {
    /// Reads only, no host mutation.
    DryRun,
    /// Mutates host state.
    Execute,
}

/// Domain operation that can be audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Report the application version.
    Version,
    /// Report capability states.
    Capabilities,
    /// Read image metadata.
    Info,
    /// Decode image pixels.
    Decode,
    /// Encode a preview payload.
    Preview,
    /// Write a thumbnail file.
    Thumbnail,
    /// Open the desktop window.
    GuiOpen,
    /// Pump the window event loop.
    GuiPump,
    /// Close the desktop window.
    GuiClose,
    /// Query window state.
    GuiStatus,
    /// Plan an installation.
    InstallPlan,
    /// Apply an installation.
    InstallApply,
    /// Plan an uninstallation.
    UninstallPlan,
    /// Apply an uninstallation.
    UninstallApply,
}

impl Operation {
    /// Stable capability name written into audit records.
    pub fn capability(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Capabilities => "capabilities",
            Self::Info => "image-info",
            Self::Decode => "image-decode",
            Self::Preview => "image-preview",
            Self::Thumbnail => "image-thumbnail",
            Self::GuiOpen => "gui-open",
            Self::GuiPump => "gui-pump",
            Self::GuiClose => "gui-close",
            Self::GuiStatus => "gui-status",
            Self::InstallPlan => "install-plan",
            Self::InstallApply => "install-apply",
            Self::UninstallPlan => "uninstall-plan",
            Self::UninstallApply => "uninstall-apply",
        }
    }

    /// Audit mode for the operation.
    pub fn mode(self) -> AuditMode {
        match self {
            Self::Version
            | Self::Capabilities
            | Self::Info
            | Self::Decode
            | Self::Preview
            | Self::GuiPump
            | Self::GuiStatus
            | Self::InstallPlan
            | Self::UninstallPlan => AuditMode::DryRun,
            Self::Thumbnail | Self::GuiOpen | Self::GuiClose | Self::InstallApply | Self::UninstallApply => AuditMode::Execute,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_are_execute_and_reads_are_dry_run() {
        assert_eq!(Operation::Thumbnail.mode(), AuditMode::Execute);
        assert_eq!(Operation::Info.mode(), AuditMode::DryRun);
        assert_eq!(Operation::InstallApply.capability(), "install-apply");
    }
}

//! Profile check status and the closed, content-free reasons behind it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileStatus {
    Missing,
    UnsupportedVersion,
    Unavailable,
    AvailableAuthUnknown,
    Unauthenticated,
    Ready,
    Busy,
    Failed,
}

/// Why a profile or run is not usable. Closed, content-free; ids are stable snake_case.
/// Only the profile, version, gate, asset and Apple groups reach the API; run reasons are log-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    ExecutableMissing,
    ProfileHomeMissing,
    ProfileHomeNotDirectory,
    ProfileUnknown,
    KindMismatch,
    VersionUnsupported,
    VersionUnreadable,
    ProbeTimedOut,
    ProbeFailed,
    SpawnFailed,
    HostBusy,
    HostShuttingDown,
    LeaseEnded,
    CancelledBeforeStart,
    RunDirUnavailable,
    AssetsMissing,
    AssetsInvalid,
    PromptPinRejected,
    PacketTooLarge,
    OsUnsupported,
    DeviceIneligible,
    AppleIntelligenceDisabled,
    ModelNotReady,
    ModelUnavailableUnknown,
    LocaleUnsupported,
    NativeSessionBusy,
    NativeErrorUnmapped,
    HelperMissing,
    HelperUntrusted,
    HelperFailed,
    RecordTooLarge,
    OutputTooLarge,
    EscapedDescendants,
    ExitNonzero,
    ResultMissing,
    InitMismatch,
}
impl Reason {
    pub const ALL: [Self; 36] = [
        Self::ExecutableMissing,
        Self::ProfileHomeMissing,
        Self::ProfileHomeNotDirectory,
        Self::ProfileUnknown,
        Self::KindMismatch,
        Self::VersionUnsupported,
        Self::VersionUnreadable,
        Self::ProbeTimedOut,
        Self::ProbeFailed,
        Self::SpawnFailed,
        Self::HostBusy,
        Self::HostShuttingDown,
        Self::LeaseEnded,
        Self::CancelledBeforeStart,
        Self::RunDirUnavailable,
        Self::AssetsMissing,
        Self::AssetsInvalid,
        Self::PromptPinRejected,
        Self::PacketTooLarge,
        Self::OsUnsupported,
        Self::DeviceIneligible,
        Self::AppleIntelligenceDisabled,
        Self::ModelNotReady,
        Self::ModelUnavailableUnknown,
        Self::LocaleUnsupported,
        Self::NativeSessionBusy,
        Self::NativeErrorUnmapped,
        Self::HelperMissing,
        Self::HelperUntrusted,
        Self::HelperFailed,
        Self::RecordTooLarge,
        Self::OutputTooLarge,
        Self::EscapedDescendants,
        Self::ExitNonzero,
        Self::ResultMissing,
        Self::InitMismatch,
    ];
    pub fn parse(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == v)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExecutableMissing => "executable_missing",
            Self::ProfileHomeMissing => "profile_home_missing",
            Self::ProfileHomeNotDirectory => "profile_home_not_directory",
            Self::ProfileUnknown => "profile_unknown",
            Self::KindMismatch => "kind_mismatch",
            Self::VersionUnsupported => "version_unsupported",
            Self::VersionUnreadable => "version_unreadable",
            Self::ProbeTimedOut => "probe_timed_out",
            Self::ProbeFailed => "probe_failed",
            Self::SpawnFailed => "spawn_failed",
            Self::HostBusy => "host_busy",
            Self::HostShuttingDown => "host_shutting_down",
            Self::LeaseEnded => "lease_ended",
            Self::CancelledBeforeStart => "cancelled_before_start",
            Self::RunDirUnavailable => "run_dir_unavailable",
            Self::AssetsMissing => "assets_missing",
            Self::AssetsInvalid => "assets_invalid",
            Self::PromptPinRejected => "prompt_pin_rejected",
            Self::PacketTooLarge => "packet_too_large",
            Self::OsUnsupported => "os_unsupported",
            Self::DeviceIneligible => "device_ineligible",
            Self::AppleIntelligenceDisabled => "apple_intelligence_disabled",
            Self::ModelNotReady => "model_not_ready",
            Self::ModelUnavailableUnknown => "model_unavailable_unknown",
            Self::LocaleUnsupported => "locale_unsupported",
            Self::NativeSessionBusy => "native_session_busy",
            Self::NativeErrorUnmapped => "native_error_unmapped",
            Self::HelperMissing => "helper_missing",
            Self::HelperUntrusted => "helper_untrusted",
            Self::HelperFailed => "helper_failed",
            Self::RecordTooLarge => "record_too_large",
            Self::OutputTooLarge => "output_too_large",
            Self::EscapedDescendants => "escaped_descendants",
            Self::ExitNonzero => "exit_nonzero",
            Self::ResultMissing => "result_missing",
            Self::InitMismatch => "init_mismatch",
        }
    }
    /// Reasons a status check can report to the API (the rest are log-only run reasons).
    pub fn is_api_visible(self) -> bool {
        !matches!(
            self,
            Self::RecordTooLarge
                | Self::OutputTooLarge
                | Self::EscapedDescendants
                | Self::ExitNonzero
                | Self::ResultMissing
                | Self::InitMismatch
                | Self::NativeErrorUnmapped
                | Self::CancelledBeforeStart
                | Self::LeaseEnded
                | Self::HostShuttingDown
                | Self::NativeSessionBusy
        )
    }
}

/// A status plus the closed reason behind it (`None` when ready or not yet attributed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileCheck {
    pub status: ProfileStatus,
    pub reason: Option<Reason>,
}
impl From<ProfileStatus> for ProfileCheck {
    fn from(status: ProfileStatus) -> Self {
        Self {
            status,
            reason: None,
        }
    }
}

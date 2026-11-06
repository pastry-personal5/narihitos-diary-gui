//! Existing fault-injection build only. None of these hooks enables Trash support.
use diary_application::source_trash::DeleteError;

#[cfg(feature = "fault-injection")]
pub(super) fn hit(point: &str, ordinal: Option<u8>) -> Result<(), DeleteError> {
    let key = match ordinal {
        Some(n) => format!("trash:{point}:{n}"),
        None => format!("trash:{point}"),
    };
    let value = std::env::var("DIARY_FAULT").unwrap_or_default();
    if value == format!("{key}:kill") {
        // Intentional process death, no Rust unwind or destructor settlement.
        // SAFETY: this test-only feature requests immediate process termination.
        unsafe { libc::_exit(86) }
    }
    if value == format!("{key}:io") {
        return Err(DeleteError::Storage);
    }
    Ok(())
}
#[cfg(not(feature = "fault-injection"))]
pub(super) fn hit(_point: &str, _ordinal: Option<u8>) -> Result<(), DeleteError> {
    Ok(())
}

pub(super) fn native_failure(ordinal: u8) -> bool {
    #[cfg(feature = "fault-injection")]
    {
        std::env::var("DIARY_FAULT").ok().as_deref()
            == Some(format!("trash:native_failure:{ordinal}").as_str())
    }
    #[cfg(not(feature = "fault-injection"))]
    {
        let _ = ordinal;
        false
    }
}

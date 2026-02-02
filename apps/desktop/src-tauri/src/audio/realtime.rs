//! Real-time Thread Priority
//!
//! Sets audio thread priority to real-time on macOS for glitch-free audio processing.
//! On other platforms, this is a no-op (with warnings logged).

/// Result type for thread priority operations
pub type PriorityResult = Result<(), String>;

/// Set the current thread to real-time priority
///
/// On macOS, this uses the Mach thread policy API to set time constraints.
/// On other platforms, this logs a warning and returns Ok.
pub fn set_realtime_priority() -> PriorityResult {
    #[cfg(target_os = "macos")]
    {
        set_realtime_priority_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        log::warn!("Real-time thread priority not implemented for this platform");
        Ok(())
    }
}

/// macOS-specific real-time thread priority implementation
#[cfg(target_os = "macos")]
fn set_realtime_priority_macos() -> PriorityResult {
    // Mach kernel constants
    const THREAD_TIME_CONSTRAINT_POLICY: u32 = 2;
    const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;

    #[repr(C)]
    struct ThreadTimeConstraintPolicy {
        period: u32,
        computation: u32,
        constraint: u32,
        preemptible: i32,
    }

    extern "C" {
        fn mach_thread_self() -> u32;
        fn thread_policy_set(
            thread: u32,
            flavor: u32,
            policy_info: *const ThreadTimeConstraintPolicy,
            count: u32,
        ) -> i32;
    }

    // Audio timing parameters
    // Assuming 48kHz sample rate, 512 sample buffer = ~10.67ms period
    // We want computation to complete well within this period
    let period: u32 = 512_000; // ~10ms in Mach absolute time units (approximate)
    let computation: u32 = 256_000; // Allow ~5ms for computation
    let constraint: u32 = 512_000; // Must complete within period

    let policy = ThreadTimeConstraintPolicy {
        period,
        computation,
        constraint,
        preemptible: 0, // Non-preemptible for audio
    };

    let result = unsafe {
        let thread = mach_thread_self();
        thread_policy_set(
            thread,
            THREAD_TIME_CONSTRAINT_POLICY,
            &policy as *const ThreadTimeConstraintPolicy,
            THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };

    if result == 0 {
        log::info!("Set real-time thread priority successfully");
        Ok(())
    } else {
        let msg = format!(
            "Failed to set real-time thread priority: error code {}",
            result
        );
        log::warn!("{}", msg);
        Err(msg)
    }
}

/// Helper to set audio thread priority
/// On macOS, sets real-time priority. On other platforms, logs a warning.
pub fn configure_audio_thread() {
    let _ = set_realtime_priority();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configure_audio_thread() {
        // Should not panic
        configure_audio_thread();
    }
}

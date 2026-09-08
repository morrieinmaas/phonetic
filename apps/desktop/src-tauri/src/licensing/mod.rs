//! Licence and trial state.
//!
//! ponytail: a timestamp in the app data directory, no cryptographic DRM. A
//! determined user can delete the file and restart the trial. That is a fair
//! trade for a professional tool at this price — a licence check that
//! inconveniences paying customers costs more than the piracy it prevents.
//! Upgrade path if piracy ever becomes measurable: server-side activation with
//! a machine fingerprint, which the Lemon Squeezy API already supports.

use serde::{Deserialize, Serialize};

pub const TRIAL_DAYS: u32 = 14;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseRecord {
    pub key: Option<String>,
    /// Days since the Unix epoch on first launch.
    pub first_run_day: i64,
    pub activated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum LicenseState {
    Trial { days_remaining: u32 },
    Licensed { key_suffix: String },
    Expired,
}

/// Decide what the user is entitled to. Pure, so it is directly testable.
pub fn evaluate(record: &LicenseRecord, today: i64) -> LicenseState {
    if record.activated {
        let suffix = record
            .key
            .as_deref()
            .and_then(|k| k.rsplit('-').next())
            .unwrap_or("")
            .to_string();
        return LicenseState::Licensed { key_suffix: suffix };
    }

    // A clock set backwards yields a negative elapsed count; clamp rather than
    // granting an unlimited trial.
    let elapsed = (today - record.first_run_day).max(0) as u32;
    if elapsed >= TRIAL_DAYS {
        return LicenseState::Expired;
    }

    LicenseState::Trial { days_remaining: TRIAL_DAYS - elapsed }
}

/// Days since the Unix epoch, from the system clock.
pub fn today() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

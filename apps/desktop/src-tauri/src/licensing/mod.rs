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

    LicenseState::Trial {
        days_remaining: TRIAL_DAYS - elapsed,
    }
}

/// Days since the Unix epoch, from the system clock.
pub fn today() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

use std::path::Path;

const RECORD_FILE: &str = "license.json";
const ACTIVATE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses/activate";

/// Read the stored record, or start a fresh trial. Never fails: a missing or
/// corrupt file must not lock a paying customer out of their own software.
pub fn load_record(dir: &Path) -> LicenseRecord {
    let fresh = || LicenseRecord {
        key: None,
        first_run_day: today(),
        activated: false,
    };

    let Ok(text) = std::fs::read_to_string(dir.join(RECORD_FILE)) else {
        return fresh();
    };

    serde_json::from_str(&text).unwrap_or_else(|_| fresh())
}

pub fn save_record(dir: &Path, record: &LicenseRecord) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let text =
        serde_json::to_string_pretty(record).map_err(|e| format!("serialise licence: {e}"))?;
    std::fs::write(dir.join(RECORD_FILE), text).map_err(|e| format!("write licence record: {e}"))
}

/// Activate a licence key with Lemon Squeezy. Returns whether it was accepted.
pub fn activate(key: &str) -> Result<bool, String> {
    let response = ureq::post(ACTIVATE_URL)
        .set("Accept", "application/json")
        .send_form(&[("license_key", key), ("instance_name", "phonetic-desktop")]);

    match response {
        Ok(res) => {
            let body: serde_json::Value = res
                .into_json()
                .map_err(|e| format!("read activation response: {e}"))?;
            Ok(body["activated"].as_bool().unwrap_or(false))
        }
        // A 400 means the key was rejected, which is an answer, not a failure.
        Err(ureq::Error::Status(400, _)) => Ok(false),
        Err(e) => Err(format!(
            "could not reach the licence server: {e}. Check your internet connection."
        )),
    }
}

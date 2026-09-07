//! Rig-error diagnosis.
//!
//! Every analyzer shows a bad trace and leaves the operator to work out why.
//! This module names the cause. All inputs are already computed elsewhere; the
//! rules are pure functions of a snapshot so each one is directly testable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    /// Stable identifier, safe to match on in the UI and in tests.
    pub code: String,
    pub severity: Severity,
    /// Written for a system tech standing at front of house, not for a developer.
    pub message: String,
    /// Identifier of a one-click fix the UI can offer, if there is one.
    pub action: Option<String>,
}

/// Everything the rules need, sampled once per second by the audio engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RigSnapshot {
    /// Linear peak over the last window, 0.0 to 1.0.
    pub reference_peak: f32,
    pub measurement_peak: f32,
    pub reference_rms: f32,
    pub measurement_rms: f32,
    pub coherence: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub frequencies: Vec<f32>,
    /// What the delay finder currently reports.
    pub measured_delay_samples: i32,
    /// What is actually applied to the transfer function.
    pub applied_delay_samples: usize,
    /// Recent delay measurements, oldest first. Drift shows up here.
    pub delay_history: Vec<i32>,
    pub sample_rate: f32,
    pub fft_size: usize,
}

const CLIPPING_PEAK: f32 = 0.999;
const SILENCE_RMS: f32 = 1e-5;
const QUIET_RMS: f32 = 0.001; // about -60 dBFS
const SWAPPED_GAIN_DB: f32 = 12.0;
const LOW_COHERENCE: f32 = 0.5;
const HF_SPLIT_HZ: f32 = 2000.0;
/// Below this, a delay is small enough not to matter.
const DELAY_MATTERS_SAMPLES: i32 = 48;

type Rule = fn(&RigSnapshot) -> Option<Diagnosis>;

/// Every rule, evaluated independently. Adding a diagnosis means adding one
/// function and one entry here.
const RULES: [Rule; 7] = [
    clipping,
    no_measurement_signal,
    measurement_too_quiet,
    channels_swapped,
    delay_not_applied,
    sample_rate_drift,
    low_coherence,
];

/// Run every rule, most severe first.
pub fn diagnose(snapshot: &RigSnapshot) -> Vec<Diagnosis> {
    let mut found: Vec<Diagnosis> = RULES.iter().filter_map(|rule| rule(snapshot)).collect();
    found.sort_by_key(|d| std::cmp::Reverse(d.severity));
    found
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

/// Mean of `values` restricted to frequencies satisfying `predicate`.
fn mean_where(values: &[f32], freqs: &[f32], predicate: impl Fn(f32) -> bool) -> f32 {
    let selected: Vec<f32> = freqs
        .iter()
        .zip(values)
        .filter(|(f, _)| predicate(**f))
        .map(|(_, v)| *v)
        .collect();
    mean(&selected)
}

fn clipping(s: &RigSnapshot) -> Option<Diagnosis> {
    let channel = match (
        s.measurement_peak >= CLIPPING_PEAK,
        s.reference_peak >= CLIPPING_PEAK,
    ) {
        (true, true) => "both channels",
        (true, false) => "the measurement channel",
        (false, true) => "the reference channel",
        (false, false) => return None,
    };

    Some(Diagnosis {
        code: "input_clipping".into(),
        severity: Severity::Error,
        message: format!("Input clipping on {channel}. Reduce preamp gain — the measurement is invalid until you do."),
        action: None,
    })
}

fn no_measurement_signal(s: &RigSnapshot) -> Option<Diagnosis> {
    if s.measurement_rms > SILENCE_RMS {
        return None;
    }

    Some(Diagnosis {
        code: "no_measurement_signal".into(),
        severity: Severity::Error,
        message: "No signal on the measurement channel. Check phantom power, the cable, and that the right input is selected.".into(),
        action: None,
    })
}

fn measurement_too_quiet(s: &RigSnapshot) -> Option<Diagnosis> {
    if s.measurement_rms <= SILENCE_RMS || s.measurement_rms >= QUIET_RMS {
        return None;
    }

    let dbfs = 20.0 * s.measurement_rms.max(1e-12).log10();
    Some(Diagnosis {
        code: "measurement_too_quiet".into(),
        severity: Severity::Warning,
        message: format!("Measurement level is {dbfs:.0} dBFS — too low for a reliable result. Raise preamp gain."),
        action: None,
    })
}

fn channels_swapped(s: &RigSnapshot) -> Option<Diagnosis> {
    if mean(&s.magnitude_db) < SWAPPED_GAIN_DB {
        return None;
    }

    Some(Diagnosis {
        code: "channels_swapped".into(),
        severity: Severity::Warning,
        message: "The measurement channel is far hotter than the reference across the whole band. Reference and measurement are probably swapped.".into(),
        action: Some("swap_channels".into()),
    })
}

fn delay_not_applied(s: &RigSnapshot) -> Option<Diagnosis> {
    let outstanding = (s.measured_delay_samples - s.applied_delay_samples as i32).abs();
    if outstanding < DELAY_MATTERS_SAMPLES {
        return None;
    }

    let lf = mean_where(&s.coherence, &s.frequencies, |f| f < HF_SPLIT_HZ);
    let hf = mean_where(&s.coherence, &s.frequencies, |f| f >= HF_SPLIT_HZ);
    // The signature of an uncompensated delay: fine low, collapsed high.
    if hf >= LOW_COHERENCE || lf <= hf + 0.2 {
        return None;
    }

    let ms = s.measured_delay_samples as f32 * 1000.0 / s.sample_rate;
    Some(Diagnosis {
        code: "delay_not_applied".into(),
        severity: Severity::Warning,
        message: format!("Coherence is low above {:.0} kHz — the measurement is delayed {ms:.1} ms and nothing is compensating for it.", HF_SPLIT_HZ / 1000.0),
        action: Some("apply_delay".into()),
    })
}

fn sample_rate_drift(s: &RigSnapshot) -> Option<Diagnosis> {
    if s.delay_history.len() < 4 {
        return None;
    }

    let steps: Vec<i32> = s.delay_history.windows(2).map(|w| w[1] - w[0]).collect();
    let all_one_way = steps.iter().all(|d| *d > 0) || steps.iter().all(|d| *d < 0);
    let total = (s.delay_history[s.delay_history.len() - 1] - s.delay_history[0]).abs();

    // Jitter wanders both ways; drift walks. Require both to avoid false alarms.
    if !all_one_way || total < DELAY_MATTERS_SAMPLES {
        return None;
    }

    Some(Diagnosis {
        code: "sample_rate_drift".into(),
        severity: Severity::Warning,
        message: format!("Measured delay has drifted {total} samples in one direction. Two clocks are running at different rates — check your aggregate device or clock source."),
        action: None,
    })
}

fn low_coherence(s: &RigSnapshot) -> Option<Diagnosis> {
    if mean(&s.coherence) >= LOW_COHERENCE {
        return None;
    }
    // If the delay rule already explains it, do not say it twice.
    if delay_not_applied(s).is_some() {
        return None;
    }

    Some(Diagnosis {
        code: "low_coherence".into(),
        severity: Severity::Warning,
        message: "Coherence is low across the band. Raise the signal above the ambient noise, or average for longer.".into(),
        action: None,
    })
}

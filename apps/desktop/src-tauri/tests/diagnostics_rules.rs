use phonetic_lib::dsp::{diagnose, RigSnapshot, Severity};

/// A snapshot of a rig that is working correctly. Each test perturbs one field.
fn healthy() -> RigSnapshot {
    let frequencies: Vec<f32> = (0..200).map(|i| 20.0 * 1.05f32.powi(i)).collect();
    RigSnapshot {
        reference_peak: 0.5,
        measurement_peak: 0.4,
        reference_rms: 0.2,
        measurement_rms: 0.18,
        coherence: vec![0.98; frequencies.len()],
        magnitude_db: vec![0.5; frequencies.len()],
        frequencies,
        measured_delay_samples: 480,
        applied_delay_samples: 480,
        delay_history: vec![480, 480, 481, 480, 480],
        sample_rate: 48_000.0,
        fft_size: 4096,
    }
}

#[test]
fn a_healthy_rig_produces_no_diagnoses() {
    assert!(
        diagnose(&healthy()).is_empty(),
        "healthy rig should be quiet"
    );
}

#[test]
fn clipping_is_detected_and_is_an_error() {
    let mut s = healthy();
    s.measurement_peak = 0.9999;
    let found = diagnose(&s);
    let d = found
        .iter()
        .find(|d| d.code == "input_clipping")
        .expect("should flag clipping");
    assert_eq!(d.severity, Severity::Error);
}

#[test]
fn a_signal_far_too_quiet_is_flagged() {
    let mut s = healthy();
    s.measurement_rms = 0.0005; // about -66 dBFS
    s.measurement_peak = 0.001;
    let found = diagnose(&s);
    assert!(found.iter().any(|d| d.code == "measurement_too_quiet"));
}

#[test]
fn a_silent_measurement_channel_is_distinguished_from_a_quiet_one() {
    let mut s = healthy();
    s.measurement_rms = 0.0;
    s.measurement_peak = 0.0;
    let found = diagnose(&s);
    let d = found
        .iter()
        .find(|d| d.code == "no_measurement_signal")
        .expect("should flag silence");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message.to_lowercase().contains("phantom"),
        "should suggest phantom power, got: {}",
        d.message
    );
}

#[test]
fn swapped_reference_and_measurement_is_detected() {
    let mut s = healthy();
    // The "measurement" is 20 dB hotter than the "reference" right across the band:
    // almost certainly the cables are the wrong way round.
    s.magnitude_db = vec![20.0; s.frequencies.len()];
    let found = diagnose(&s);
    assert!(found.iter().any(|d| d.code == "channels_swapped"));
}

#[test]
fn an_unapplied_delay_is_detected_with_a_one_click_fix() {
    let mut s = healthy();
    s.applied_delay_samples = 0;
    s.measured_delay_samples = 470;
    // Coherence collapses at HF, which is the signature.
    for (i, f) in s.frequencies.iter().enumerate() {
        s.coherence[i] = if *f > 2000.0 { 0.3 } else { 0.95 };
    }
    let found = diagnose(&s);
    let d = found
        .iter()
        .find(|d| d.code == "delay_not_applied")
        .expect("should flag delay");
    assert_eq!(d.action.as_deref(), Some("apply_delay"));
    assert!(
        d.message.contains("9.8"),
        "should quote the delay in ms, got: {}",
        d.message
    );
}

#[test]
fn sample_rate_drift_is_detected_from_a_creeping_delay() {
    let mut s = healthy();
    // A delay that walks in one direction means two clocks running at different rates.
    s.delay_history = vec![480, 495, 511, 526, 542, 558];
    s.measured_delay_samples = 558;
    let found = diagnose(&s);
    let d = found
        .iter()
        .find(|d| d.code == "sample_rate_drift")
        .expect("should flag drift");
    assert!(
        d.message.to_lowercase().contains("aggregate")
            || d.message.to_lowercase().contains("clock")
    );
}

#[test]
fn stable_delay_jitter_is_not_mistaken_for_drift() {
    let mut s = healthy();
    s.delay_history = vec![480, 482, 479, 481, 480, 478];
    assert!(
        !diagnose(&s).iter().any(|d| d.code == "sample_rate_drift"),
        "normal jitter must not be reported as drift"
    );
}

#[test]
fn broadband_low_coherence_without_delay_is_flagged_as_noise() {
    let mut s = healthy();
    s.coherence = vec![0.2; s.frequencies.len()];
    let found = diagnose(&s);
    assert!(found.iter().any(|d| d.code == "low_coherence"));
}

#[test]
fn diagnoses_are_ranked_most_severe_first() {
    let mut s = healthy();
    s.measurement_peak = 0.9999; // Error
    s.coherence = vec![0.2; s.frequencies.len()]; // Warning
    let found = diagnose(&s);
    assert!(found.len() >= 2);
    assert_eq!(found[0].severity, Severity::Error, "errors must sort first");
}

use phonetic_lib::dsp::{bank_response, BiquadFilter, FilterType};

const FS: f32 = 48_000.0;

fn peaking(freq: f32, gain_db: f32, q: f32) -> BiquadFilter {
    BiquadFilter {
        filter_type: FilterType::Peaking,
        frequency_hz: freq,
        gain_db,
        q,
        enabled: true,
    }
}

fn db(h: realfft::num_complex::Complex<f32>) -> f32 {
    20.0 * h.norm().log10()
}

fn phase(h: realfft::num_complex::Complex<f32>) -> f32 {
    h.arg().to_degrees()
}

/// A peaking filter must deliver exactly its specified gain at its centre frequency.
#[test]
fn a_peaking_filter_hits_its_gain_at_centre() {
    for gain in [-12.0_f32, -6.0, -3.0, 3.0, 6.0, 12.0] {
        let f = peaking(1000.0, gain, 1.0);
        let got = db(f.response_at(1000.0, FS));
        assert!(
            (got - gain).abs() < 0.05,
            "asked for {gain:+.1} dB at 1 kHz, got {got:+.3} dB"
        );
    }
}

/// Far from centre, a peaking filter must be transparent.
#[test]
fn a_peaking_filter_is_flat_far_from_centre() {
    let f = peaking(1000.0, 12.0, 4.0);
    for probe in [50.0_f32, 100.0, 15_000.0] {
        let got = db(f.response_at(probe, FS));
        assert!(
            got.abs() < 1.0,
            "should be flat at {probe} Hz, got {got:+.3} dB"
        );
    }
}

/// A Q at or below the physical floor must clamp to a finite, usable
/// response rather than blowing up. `alpha = sin(w0) / (2*q)` divides by
/// `q`, so `q <= 0` (or NaN) must not reach that division unclamped.
#[test]
fn invalid_q_clamps_to_a_finite_response() {
    for bad_q in [0.0_f32, -5.0, f32::NAN] {
        let f = peaking(1000.0, 12.0, bad_q);
        let h = f.response_at(1000.0, FS);
        assert!(
            h.re.is_finite() && h.im.is_finite(),
            "q={bad_q} produced a non-finite response: {h:?}"
        );
        let got = db(h);
        assert!(
            got.is_finite(),
            "q={bad_q} produced a non-finite magnitude: {got}"
        );
    }
}

/// Higher Q means a narrower bell.
#[test]
fn higher_q_narrows_the_bell() {
    let wide = peaking(1000.0, 12.0, 0.5);
    let narrow = peaking(1000.0, 12.0, 8.0);
    let probe = 1414.0; // half an octave up
    assert!(
        db(wide.response_at(probe, FS)) > db(narrow.response_at(probe, FS)),
        "a wider filter should still be lifting half an octave away"
    );
}

/// A high-pass is -3 dB at its corner.
#[test]
fn a_highpass_is_three_db_down_at_its_corner() {
    let f = BiquadFilter {
        filter_type: FilterType::HighPass,
        frequency_hz: 100.0,
        gain_db: 0.0,
        q: std::f32::consts::FRAC_1_SQRT_2, // Butterworth
        enabled: true,
    };
    let got = db(f.response_at(100.0, FS));
    assert!(
        (got - -3.01).abs() < 0.15,
        "expected -3.01 dB at the corner, got {got:.3}"
    );
}

/// Shelves reach their gain well past the corner.
#[test]
fn a_low_shelf_reaches_its_gain_at_dc() {
    let f = BiquadFilter {
        filter_type: FilterType::LowShelf,
        frequency_hz: 200.0,
        gain_db: 6.0,
        q: 0.707,
        enabled: true,
    };
    let got = db(f.response_at(20.0, FS));
    assert!(
        (got - 6.0).abs() < 0.5,
        "expected +6 dB well below the corner, got {got:.3}"
    );
}

/// Shelves reach their gain well past the corner (high shelf: above it).
#[test]
fn a_high_shelf_reaches_its_gain_above_corner() {
    let f = BiquadFilter {
        filter_type: FilterType::HighShelf,
        frequency_hz: 2000.0,
        gain_db: 6.0,
        q: 0.707,
        enabled: true,
    };
    let got = db(f.response_at(20_000.0, FS));
    assert!(
        (got - 6.0).abs() < 0.5,
        "expected +6 dB well above the corner, got {got:.3}"
    );
}

/// A low-pass is -3.01 dB at its corner (Butterworth Q).
#[test]
fn a_lowpass_is_three_db_down_at_its_corner() {
    let f = BiquadFilter {
        filter_type: FilterType::LowPass,
        frequency_hz: 1000.0,
        gain_db: 0.0,
        q: std::f32::consts::FRAC_1_SQRT_2, // Butterworth
        enabled: true,
    };
    let got = db(f.response_at(1000.0, FS));
    assert!(
        (got - -3.01).abs() < 0.15,
        "expected -3.01 dB at the corner, got {got:.3}"
    );
}

/// A low-pass attenuates strongly well above its corner, and is transparent
/// well below it.
#[test]
fn a_lowpass_shapes_the_spectrum_around_its_corner() {
    let f = BiquadFilter {
        filter_type: FilterType::LowPass,
        frequency_hz: 1000.0,
        gain_db: 0.0,
        q: std::f32::consts::FRAC_1_SQRT_2,
        enabled: true,
    };

    let below = db(f.response_at(50.0, FS));
    assert!(
        below.abs() < 0.5,
        "should be transparent well below the corner, got {below:+.3} dB"
    );

    let above = db(f.response_at(10_000.0, FS));
    assert!(
        above < -30.0,
        "should attenuate strongly well above the corner, got {above:+.3} dB"
    );
}

/// Filters compose: two +3 dB bells at the same spot make +6 dB.
#[test]
fn filters_in_a_bank_multiply() {
    let freqs = vec![1000.0_f32];
    let bank = vec![peaking(1000.0, 3.0, 1.0), peaking(1000.0, 3.0, 1.0)];
    let got = db(bank_response(&bank, &freqs, FS)[0]);
    assert!(
        (got - 6.0).abs() < 0.1,
        "two +3 dB filters should give +6 dB, got {got:.3}"
    );
}

#[test]
fn a_disabled_filter_contributes_nothing() {
    let freqs = vec![1000.0_f32];
    let mut disabled = peaking(1000.0, 12.0, 1.0);
    disabled.enabled = false;
    let got = db(bank_response(&[disabled], &freqs, FS)[0]);
    assert!(
        got.abs() < 0.01,
        "a disabled filter must be transparent, got {got:.3} dB"
    );
}

#[test]
fn an_empty_bank_is_transparent() {
    let freqs = vec![100.0_f32, 1000.0, 10_000.0];
    for h in bank_response(&[], &freqs, FS) {
        assert!((h.norm() - 1.0).abs() < 1e-6);
    }
}

use phonetic_lib::dsp::{predict_with_eq, TransferFunctionData};

fn flat_trace() -> TransferFunctionData {
    let frequencies: Vec<f32> = (0..200).map(|i| 20.0 * 1.05f32.powi(i)).collect();
    let n = frequencies.len();
    TransferFunctionData {
        magnitude_db: vec![0.0; n],
        phase_deg: vec![0.0; n],
        coherence: vec![1.0; n],
        group_delay_ms: vec![0.0; n],
        frequencies,
    }
}

#[test]
fn predicting_with_no_filters_returns_the_trace_unchanged() {
    let trace = flat_trace();
    let out = predict_with_eq(&trace, &[], FS);
    for (a, b) in trace.magnitude_db.iter().zip(&out.magnitude_db) {
        assert!((a - b).abs() < 1e-4);
    }
}

#[test]
fn a_cut_shows_up_in_the_prediction() {
    let trace = flat_trace();
    let filters = vec![peaking(1000.0, -6.0, 2.0)];
    let out = predict_with_eq(&trace, &filters, FS);

    // Find the point closest to 1 kHz.
    let idx = out
        .frequencies
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1 - 1000.0)
                .abs()
                .partial_cmp(&(b.1 - 1000.0).abs())
                .unwrap()
        })
        .map(|(i, _)| i)
        .unwrap();

    assert!(
        (out.magnitude_db[idx] - -6.0).abs() < 0.5,
        "expected about -6 dB at 1 kHz, got {:.3}",
        out.magnitude_db[idx]
    );
}

#[test]
fn prediction_preserves_coherence_untouched() {
    let trace = flat_trace();
    let out = predict_with_eq(&trace, &[peaking(1000.0, 6.0, 1.0)], FS);
    assert_eq!(
        out.coherence, trace.coherence,
        "EQ does not change measurement confidence"
    );
}

/// A peaking boost is a minimum-phase bell: phase leads (positive) below
/// centre and lags (negative) above centre -- the characteristic S-curve.
/// These probe frequencies and gain/Q reproduce the reviewer's own
/// worked example (800 Hz -> +14.7931 deg) that a sign error in
/// `z1 = Complex::new(w.cos(), -w.sin())` would silently invert.
#[test]
fn a_peaking_boost_has_the_characteristic_phase_s_curve() {
    let f = peaking(1000.0, 6.0, 1.0);

    let below = phase(f.response_at(800.0, FS));
    assert!(
        below > 0.0,
        "below centre, phase should lead (be positive), got {below:+.4} deg"
    );
    assert!(
        below.abs() < 90.0,
        "phase should stay within a sane bound, got {below:+.4} deg"
    );

    let above = phase(f.response_at(1100.0, FS));
    assert!(
        above < 0.0,
        "above centre, phase should lag (be negative), got {above:+.4} deg"
    );
    assert!(
        above.abs() < 90.0,
        "phase should stay within a sane bound, got {above:+.4} deg"
    );
}

/// At the exact centre frequency, a peaking filter's phase is ~0 deg.
#[test]
fn a_peaking_filter_has_zero_phase_at_centre() {
    let f = peaking(1000.0, 6.0, 1.0);
    let got = phase(f.response_at(1000.0, FS));
    assert!(
        got.abs() < 0.1,
        "expected ~0 deg at the centre frequency, got {got:+.4} deg"
    );
}

/// A Butterworth high-pass leads by ~90 deg at its corner; a Butterworth
/// low-pass lags by ~90 deg at its corner. This is a 2nd-order (biquad,
/// 12 dB/oct) filter, so the corner phase is +/-90 deg -- not the +/-45 deg
/// of a 1st-order (6 dB/oct) RC-style filter. Confirmed independently from
/// the analog Butterworth prototype H(s) = wc^2 / (s^2 + sqrt(2) wc s + wc^2):
/// at s = j*wc, H = 1 / (j*sqrt(2)), i.e. -90 deg (and the dual +90 deg for
/// the high-pass), consistent with the existing -3.01 dB corner magnitude.
#[test]
fn butterworth_corner_phase_has_the_expected_sign_and_magnitude() {
    let hp = BiquadFilter {
        filter_type: FilterType::HighPass,
        frequency_hz: 1000.0,
        gain_db: 0.0,
        q: std::f32::consts::FRAC_1_SQRT_2,
        enabled: true,
    };
    let hp_phase = phase(hp.response_at(1000.0, FS));
    assert!(
        (hp_phase - 90.0).abs() < 1.0,
        "expected ~+90 deg at the high-pass corner, got {hp_phase:+.4} deg"
    );

    let lp = BiquadFilter {
        filter_type: FilterType::LowPass,
        frequency_hz: 1000.0,
        gain_db: 0.0,
        q: std::f32::consts::FRAC_1_SQRT_2,
        enabled: true,
    };
    let lp_phase = phase(lp.response_at(1000.0, FS));
    assert!(
        (lp_phase - -90.0).abs() < 1.0,
        "expected ~-90 deg at the low-pass corner, got {lp_phase:+.4} deg"
    );
}

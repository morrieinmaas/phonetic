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

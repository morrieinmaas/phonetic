//! Accuracy tests: every expected value here is derived analytically, not
//! captured from a previous run. If the DSP drifts, these fail with a number
//! you can read off against theory.
//!
//! The existing suites check that outputs are finite and in range. These check
//! that they are *correct*.

use phonetic_lib::dsp::{
    a_weighting, DelayFinder, ImpulseResponseProcessor, TransferFunctionProcessor,
};
use std::f32::consts::PI;

const FS: f32 = 48_000.0;
const FFT: usize = 4096;
const FRAMES: usize = 60;

/// Deterministic white noise in [-1, 1). xorshift32 so runs are reproducible.
fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// Run measurement + reference through the processor and return the result.
/// Note the argument order: `process(measurement, reference)`.
fn measure(meas: &[f32], reference: &[f32]) -> phonetic_lib::dsp::TransferFunctionData {
    let mut tf = TransferFunctionProcessor::new(FFT, FS);
    tf.process(meas, reference);
    assert!(
        tf.average_count() > 10,
        "not enough averages to be meaningful"
    );
    tf.get_data()
}

/// Bin indices covering a frequency range, skipping DC and Nyquist edges.
fn band(lo_hz: f32, hi_hz: f32) -> std::ops::Range<usize> {
    let bin = |f: f32| (f / (FS / FFT as f32)).round() as usize;
    bin(lo_hz)..bin(hi_hz)
}

// ---------------------------------------------------------------------------
// Transfer function
// ---------------------------------------------------------------------------

#[test]
fn identity_path_reads_zero_db_and_unity_coherence() {
    let x = noise(FFT * FRAMES, 1);
    let d = measure(&x, &x);

    for i in band(50.0, 20_000.0) {
        assert!(
            d.magnitude_db[i].abs() < 0.01,
            "bin {} ({:.0} Hz): expected 0 dB, got {:.3} dB",
            i,
            d.frequencies[i],
            d.magnitude_db[i]
        );
        assert!(
            d.coherence[i] > 0.999,
            "bin {} ({:.0} Hz): expected coherence 1.0, got {:.4}",
            i,
            d.frequencies[i],
            d.coherence[i]
        );
    }
}

#[test]
fn known_gain_is_recovered_to_within_a_hundredth_of_a_db() {
    let x = noise(FFT * FRAMES, 2);
    // -6.0206 dB exactly.
    let y: Vec<f32> = x.iter().map(|s| s * 0.5).collect();
    let d = measure(&y, &x);

    for i in band(50.0, 20_000.0) {
        assert!(
            (d.magnitude_db[i] - (-6.0206)).abs() < 0.01,
            "bin {} ({:.0} Hz): expected -6.02 dB, got {:.3} dB",
            i,
            d.frequencies[i],
            d.magnitude_db[i]
        );
    }
}

#[test]
fn inverted_polarity_reads_180_degrees() {
    let x = noise(FFT * FRAMES, 3);
    let y: Vec<f32> = x.iter().map(|s| -s).collect();
    let d = measure(&y, &x);

    for i in band(100.0, 15_000.0) {
        let wrapped = d.phase_deg[i].rem_euclid(360.0);
        assert!(
            (wrapped - 180.0).abs() < 1.0,
            "bin {} ({:.0} Hz): expected 180 deg, got {:.2}",
            i,
            d.frequencies[i],
            wrapped
        );
    }
}

/// The load-bearing test. A one-pole lowpass has a closed-form response:
///     H(w) = (1 - a) / (1 - a * e^-jw)
/// If the analyzer recovers both magnitude and phase of a filter we know
/// exactly, the dual-FFT engine is correct. If this passes, a trace next to
/// Smaart's should agree.
#[test]
fn one_pole_lowpass_matches_closed_form_response() {
    let a = 0.9_f32;
    let x = noise(FFT * FRAMES, 4);

    let mut prev = 0.0;
    let y: Vec<f32> = x
        .iter()
        .map(|s| {
            prev = a * prev + (1.0 - a) * s;
            prev
        })
        .collect();

    let d = measure(&y, &x);

    let (mut worst_mag, mut worst_phase) = (0.0_f32, 0.0_f32);
    for i in band(100.0, 15_000.0) {
        let w = 2.0 * PI * d.frequencies[i] / FS;
        // H = (1-a) / (1 - a*e^-jw), expanded into real/imaginary parts.
        let (re, im) = (1.0 - a * w.cos(), a * w.sin());
        let expect_mag = 20.0 * ((1.0 - a) / (re * re + im * im).sqrt()).log10();
        let expect_phase = -im.atan2(re).to_degrees();

        worst_mag = worst_mag.max((d.magnitude_db[i] - expect_mag).abs());
        worst_phase = worst_phase.max((d.phase_deg[i] - expect_phase).abs());
    }

    assert!(
        worst_mag < 0.5,
        "worst magnitude error {worst_mag:.3} dB (limit 0.5)"
    );
    assert!(
        worst_phase < 2.0,
        "worst phase error {worst_phase:.3} deg (limit 2.0)"
    );
}

/// Coherence is not decorative — it is what tells an operator whether to trust
/// the trace. For signal plus uncorrelated noise the expected value is exactly
/// SNR / (1 + SNR).
#[test]
fn coherence_matches_theoretical_value_for_known_snr() {
    for (snr_db, expected) in [(20.0, 100.0 / 101.0), (10.0, 10.0 / 11.0), (0.0, 0.5)] {
        let snr: f32 = 10f32.powf(snr_db / 10.0);
        let x = noise(FFT * FRAMES, 5);
        let n = noise(FFT * FRAMES, 6);
        let scale = (1.0 / snr).sqrt();
        let y: Vec<f32> = x.iter().zip(&n).map(|(s, e)| s + e * scale).collect();

        let d = measure(&y, &x);
        let r = band(200.0, 15_000.0);
        let mean: f32 = d.coherence[r.clone()].iter().sum::<f32>() / r.len() as f32;

        assert!(
            (mean - expected).abs() < 0.03,
            "{snr_db} dB SNR: expected coherence {expected:.4}, got {mean:.4}"
        );
    }
}

/// A pure delay must show up as a flat group delay equal to that delay.
#[test]
fn pure_delay_reads_as_flat_group_delay() {
    let delay_samples = 48; // exactly 1.0 ms at 48 kHz
    let x = noise(FFT * FRAMES, 7);
    let mut y = vec![0.0; x.len()];
    y[delay_samples..].copy_from_slice(&x[..x.len() - delay_samples]);

    let d = measure(&y, &x);

    let r = band(200.0, 10_000.0);
    let mean: f32 = d.group_delay_ms[r.clone()].iter().sum::<f32>() / r.len() as f32;
    assert!(
        (mean - 1.0).abs() < 0.05,
        "expected 1.00 ms group delay, got {mean:.3} ms"
    );
}

/// A realistic acoustic delay (10 ms ≈ 3.4 m) must not degrade the measurement once
/// the delay finder's result is applied to the reference.
#[test]
fn compensated_acoustic_delay_preserves_coherence_and_flattens_phase() {
    let delay_samples = 480; // 10 ms — a mic 3.4 m from the box
    let x = noise(FFT * FRAMES, 8);
    let mut y = vec![0.0; x.len()];
    y[delay_samples..].copy_from_slice(&x[..x.len() - delay_samples]);

    // The delay finder locates it exactly.
    let mut finder = DelayFinder::new(FFT, FS, 50.0);
    let found = finder.find_delay(&x[..FFT * 2], &y[..FFT * 2]);
    assert_eq!(
        found.delay_samples, delay_samples as i32,
        "delay finder should be exact"
    );

    // Applying it restores the measurement.
    let mut tf = TransferFunctionProcessor::new(FFT, FS);
    tf.set_delay_samples(found.delay_samples as usize);
    tf.process(&y, &x);
    let d = tf.get_data();

    let r = band(2_000.0, 15_000.0);
    let mean_coh: f32 = d.coherence[r.clone()].iter().sum::<f32>() / r.len() as f32;
    assert!(
        mean_coh > 0.99,
        "compensated coherence should be ~1.0, got {mean_coh:.4}"
    );

    let mean_mag: f32 = d.magnitude_db[r.clone()].iter().sum::<f32>() / r.len() as f32;
    assert!(
        mean_mag.abs() < 0.5,
        "compensated magnitude should be ~0 dB, got {mean_mag:.3} dB"
    );
}

/// Setting a delay must discard averages accumulated at the old alignment.
#[test]
fn changing_delay_resets_averaging() {
    let x = noise(FFT * 8, 11);
    let mut tf = TransferFunctionProcessor::new(FFT, FS);
    tf.process(&x, &x);
    assert!(tf.average_count() > 0);

    tf.set_delay_samples(64);
    assert_eq!(tf.average_count(), 0, "changing delay must reset averaging");
    assert_eq!(tf.delay_samples(), 64);
}

/// Averaging must saturate, not grow without bound, or the display stops responding
/// to changes in the system under test.
#[test]
fn averaging_saturates_at_the_configured_limit() {
    let x = noise(FFT * 200, 12);
    let mut tf = TransferFunctionProcessor::new(FFT, FS);
    tf.set_max_averages(8);
    tf.process(&x, &x);
    assert_eq!(
        tf.average_count(),
        8,
        "average count must cap at max_averages"
    );
}

/// After saturation the estimate must track a change in the system rather than stay
/// anchored to history. Measure at 0 dB, switch the path to -6 dB, and confirm the
/// display converges on the new value.
#[test]
fn saturated_averaging_still_tracks_a_changed_system() {
    let mut tf = TransferFunctionProcessor::new(FFT, FS);
    tf.set_max_averages(8);

    let x1 = noise(FFT * 100, 13);
    tf.process(&x1, &x1); // unity gain, long enough to saturate

    let x2 = noise(FFT * 100, 14);
    let y2: Vec<f32> = x2.iter().map(|s| s * 0.5).collect(); // now -6.02 dB
    tf.process(&y2, &x2);

    let d = tf.get_data();
    let r = band(500.0, 10_000.0);
    let mean: f32 = d.magnitude_db[r.clone()].iter().sum::<f32>() / r.len() as f32;
    assert!(
        (mean - (-6.0206)).abs() < 0.5,
        "should have converged on -6.02 dB, got {mean:.3} dB"
    );
}

// ---------------------------------------------------------------------------
// Weighting curves and RT60
// ---------------------------------------------------------------------------

/// IEC 61672-1 Table 3 nominal A-weighting values.
#[test]
fn a_weighting_matches_iec_61672_table() {
    let table = [
        (63.0, -26.2),
        (125.0, -16.1),
        (250.0, -8.6),
        (500.0, -3.2),
        (1000.0, 0.0),
        (2000.0, 1.2),
        (4000.0, 1.0),
        (8000.0, -1.1),
    ];

    for (f, expected) in table {
        let got = a_weighting(f);
        assert!(
            (got - expected).abs() < 0.2,
            "{f} Hz: IEC 61672 says {expected:+.1} dB, got {got:+.2} dB"
        );
    }
}

/// Feed a decay whose T60 we chose, and check the estimator returns it.
/// Reference is a unit impulse, so deconvolution passes the measurement
/// through unchanged and the RT60 path is what is under test.
#[test]
fn rt60_recovers_a_synthesised_decay() {
    const N: usize = 16_384; // 341 ms at 48 kHz
    let t60_ms = 200.0_f32;

    let mut reference = vec![0.0; N];
    reference[0] = 1.0;

    let raw = noise(N, 9);
    let measured: Vec<f32> = raw
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let t_ms = i as f32 * 1000.0 / FS;
            s * 10f32.powf(-3.0 * t_ms / t60_ms)
        })
        .collect();

    let mut ir = ImpulseResponseProcessor::new(N, FS);
    ir.start_capture();
    ir.add_samples(&reference, &measured);
    assert!(ir.is_capture_complete());

    let data = ir.compute_impulse_response().expect("IR should compute");
    let rt60 = data.rt60_ms.expect("RT60 should be estimated");

    let error_pct = (rt60 - t60_ms).abs() / t60_ms * 100.0;
    assert!(
        error_pct < 10.0,
        "synthesised T60 was {t60_ms} ms, estimator said {rt60:.1} ms ({error_pct:.1}% off)"
    );
}

/// Displayed phase must be wrapped to [-180, 180], as every reference analyzer shows
/// it. A 10 ms delay accumulates thousands of degrees if left unwrapped.
#[test]
fn displayed_phase_is_wrapped_to_plus_minus_180() {
    let delay_samples = 480;
    let x = noise(FFT * FRAMES, 15);
    let mut y = vec![0.0; x.len()];
    y[delay_samples..].copy_from_slice(&x[..x.len() - delay_samples]);

    let d = measure(&y, &x);

    for i in band(100.0, 15_000.0) {
        assert!(
            d.phase_deg[i] >= -180.5 && d.phase_deg[i] <= 180.5,
            "bin {} ({:.0} Hz): phase {:.1} deg is outside [-180, 180]",
            i,
            d.frequencies[i],
            d.phase_deg[i]
        );
    }
}

/// A quiet reference must give the same recovered impulse response as a loud
/// one. A *flat-spectrum* reference (a unit impulse) cannot expose this: its
/// |X(f)| is a frequency-independent constant, so any epsilon just rescales
/// the whole recovered spectrum by one scalar, and a scale-invariant metric
/// like RT60 (built on an EDC normalised to its own peak) never sees it. So
/// this reference is spectrally *shaped* instead — white noise through the
/// same one-pole lowpass as `one_pole_lowpass_matches_closed_form_response` —
/// which makes an absolute epsilon suppress its weak high-frequency bins
/// disproportionately, smearing the recovered impulse response. The "system"
/// is a pure sample delay (applied as a circular shift, matching what a
/// deconvolution over one FFT block actually assumes), so the ideal recovered
/// response is a clean spike at a known index and any epsilon-driven leakage
/// shows up as blur around it.
#[test]
fn impulse_response_is_independent_of_reference_level() {
    const N: usize = 16_384;
    let delay_samples = 1000;
    let a = 0.97_f32;

    let raw = noise(N, 20);
    let mut prev = 0.0;
    let shaped: Vec<f32> = raw
        .iter()
        .map(|s| {
            prev = a * prev + (1.0 - a) * s;
            prev
        })
        .collect();

    let mut normalised_irs = Vec::new();
    let mut peak_indices = Vec::new();

    // Same system, reference 60 dB apart in level.
    for gain in [1.0_f32, 0.001] {
        let reference: Vec<f32> = shaped.iter().map(|s| s * gain).collect();
        let mut measured = reference.clone();
        measured.rotate_right(delay_samples);

        let mut ir = ImpulseResponseProcessor::new(N, FS);
        ir.start_capture();
        ir.add_samples(&reference, &measured);
        let data = ir.compute_impulse_response().expect("IR should compute");

        let (peak_idx, peak_val) = data
            .samples
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(i, &v)| (i, v))
            .expect("IR should be non-empty");
        peak_indices.push(peak_idx);
        normalised_irs.push(
            data.samples
                .iter()
                .map(|&s| s / peak_val)
                .collect::<Vec<f32>>(),
        );
    }

    assert_eq!(
        peak_indices[0], peak_indices[1],
        "recovered delay moved between reference levels: {} vs {} samples",
        peak_indices[0], peak_indices[1]
    );

    // Compare the normalised shapes in a window around the peak, where
    // epsilon-driven smearing of the spike shows up.
    let window = peak_indices[0].saturating_sub(100)..(peak_indices[0] + 300).min(N);
    let worst: f32 = window
        .clone()
        .map(|i| (normalised_irs[0][i] - normalised_irs[1][i]).abs())
        .fold(0.0, f32::max);

    assert!(
        worst < 0.05,
        "normalised impulse responses differ by {worst:.3} around the peak (samples {}..{}) between reference levels",
        window.start,
        window.end
    );
}

// ---------------------------------------------------------------------------
// Multi-time-window
// ---------------------------------------------------------------------------

/// The whole point of multi-time-window: a closed-form filter recovered accurately
/// at 25 Hz, where a single 4096-point FFT has only two bins below 25 Hz and cannot.
#[test]
fn multi_window_resolves_low_frequencies_a_single_fft_cannot() {
    let a = 0.9_f32;
    let x = noise(65_536 * 8, 20);

    let mut prev = 0.0;
    let y: Vec<f32> = x
        .iter()
        .map(|s| {
            prev = a * prev + (1.0 - a) * s;
            prev
        })
        .collect();

    let mut mw = phonetic_lib::dsp::MultiWindowProcessor::new(FS);
    mw.process(&y, &x);
    let d = mw.get_data();

    let mut worst = 0.0_f32;
    for (i, &f) in d.frequencies.iter().enumerate() {
        if !(25.0..=15_000.0).contains(&f) {
            continue;
        }
        let w = 2.0 * PI * f / FS;
        let (re, im) = (1.0 - a * w.cos(), a * w.sin());
        let expect = 20.0 * ((1.0 - a) / (re * re + im * im).sqrt()).log10();
        worst = worst.max((d.magnitude_db[i] - expect).abs());
    }

    assert!(
        worst < 1.0,
        "worst magnitude error {worst:.3} dB across 25 Hz-15 kHz"
    );
}

/// `new()` only ever gets whatever default sample rate the caller had on hand;
/// the real device rate arrives later via `set_sample_rate`, exactly as the
/// engine does at device-start. If that call is dropped, or doesn't actually
/// correct the axis, every bin-to-hertz mapping used to stitch the log axis
/// stays wrong. 44.1 kHz is the realistic non-48k case (44.1/48/96/192 kHz
/// interfaces are all common in the field).
#[test]
fn multi_window_tracks_a_non_48k_sample_rate_after_set_sample_rate() {
    let fs = 44_100.0_f32;
    let a = 0.9_f32;
    let x = noise(65_536 * 8, 24);

    let mut prev = 0.0;
    let y: Vec<f32> = x
        .iter()
        .map(|s| {
            prev = a * prev + (1.0 - a) * s;
            prev
        })
        .collect();

    // Mirrors engine startup: constructed at a default that is not the real
    // device rate, then corrected before any processing happens.
    let mut mw = phonetic_lib::dsp::MultiWindowProcessor::new(FS);
    mw.set_sample_rate(fs);
    mw.process(&y, &x);
    let d = mw.get_data();

    // The axis itself must still cover 20 Hz-20 kHz regardless of sample rate.
    assert!(d.frequencies[0] >= 19.0 && d.frequencies[0] <= 21.0);
    assert!((d.frequencies.last().unwrap() - 20_000.0).abs() < 1.0);

    let mut worst = 0.0_f32;
    for (i, &f) in d.frequencies.iter().enumerate() {
        if !(25.0..=15_000.0).contains(&f) {
            continue;
        }
        let w = 2.0 * PI * f / fs;
        let (re, im) = (1.0 - a * w.cos(), a * w.sin());
        let expect = 20.0 * ((1.0 - a) / (re * re + im * im).sqrt()).log10();
        worst = worst.max((d.magnitude_db[i] - expect).abs());
    }

    // A stale 48 kHz label on this 44.1 kHz signal mislabels every bin by a
    // factor of 48000/44100, which the one-pole filter's transition region
    // turns into ~0.7 dB of magnitude error — well outside this tolerance.
    assert!(
        worst < 0.2,
        "worst magnitude error {worst:.3} dB across 25 Hz-15 kHz at {fs} Hz sample rate"
    );
}

/// The edges of the log axis (20 Hz and 20 kHz) are the two ends of the
/// displayed chart. They must show the true group delay, not a hard zero from
/// a loop that skipped the first and last points.
#[test]
fn multi_window_group_delay_is_correct_at_the_axis_edges() {
    // Small enough that the phase change between adjacent log-axis points near
    // 20 kHz stays well under 180 degrees, so the wrap-correction picks the
    // right branch instead of aliasing.
    let delay_samples = 40; // 0.833 ms at 48 kHz
    let x = noise(65_536 * 8, 26);
    let mut y = vec![0.0; x.len()];
    y[delay_samples..].copy_from_slice(&x[..x.len() - delay_samples]);

    let mut mw = phonetic_lib::dsp::MultiWindowProcessor::new(FS);
    mw.process(&y, &x);
    let d = mw.get_data();

    let expected_ms = delay_samples as f32 * 1000.0 / FS;
    let first = d.group_delay_ms[0];
    let last = *d.group_delay_ms.last().unwrap();

    assert!(
        (first - expected_ms).abs() < 0.3,
        "group delay at {:.1} Hz (first axis point) should be ~{expected_ms:.3} ms, got {first:.3} ms",
        d.frequencies[0]
    );
    assert!(
        (last - expected_ms).abs() < 0.3,
        "group delay at {:.1} Hz (last axis point) should be ~{expected_ms:.3} ms, got {last:.3} ms",
        d.frequencies[d.frequencies.len() - 1]
    );
}

#[test]
fn multi_window_output_is_on_the_log_axis() {
    let x = noise(65_536 * 2, 21);
    let mut mw = phonetic_lib::dsp::MultiWindowProcessor::new(FS);
    mw.process(&x, &x);
    let d = mw.get_data();

    assert!(d.frequencies[0] >= 19.0 && d.frequencies[0] <= 21.0);
    assert_eq!(d.frequencies.len(), d.magnitude_db.len());
    assert_eq!(d.frequencies.len(), d.coherence.len());
    assert_eq!(d.frequencies.len(), d.phase_deg.len());
    assert_eq!(d.frequencies.len(), d.group_delay_ms.len());
}

#[test]
fn smoothing_setting_is_applied_to_multi_window_output() {
    use phonetic_lib::dsp::{MultiWindowProcessor, SmoothingFraction};

    let x = noise(65_536 * 2, 22);
    let n = noise(65_536 * 2, 23);
    let y: Vec<f32> = x.iter().zip(&n).map(|(s, e)| s + e * 0.3).collect();

    let mut raw = MultiWindowProcessor::new(FS);
    raw.process(&y, &x);

    let mut smoothed = MultiWindowProcessor::new(FS);
    smoothed.set_smoothing(SmoothingFraction::Octave6);
    smoothed.process(&y, &x);

    // Bin-to-bin variation must fall once smoothing is on.
    let roughness = |v: &[f32]| -> f32 {
        v.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / (v.len() - 1) as f32
    };

    let r_raw = roughness(&raw.get_data().magnitude_db);
    let r_smooth = roughness(&smoothed.get_data().magnitude_db);
    assert!(
        r_smooth < r_raw * 0.7,
        "smoothing should reduce roughness: raw {r_raw:.4}, smoothed {r_smooth:.4}"
    );
}

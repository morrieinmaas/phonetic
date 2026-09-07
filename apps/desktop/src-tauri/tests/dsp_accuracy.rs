//! Accuracy tests: every expected value here is derived analytically, not
//! captured from a previous run. If the DSP drifts, these fail with a number
//! you can read off against theory.
//!
//! The existing suites check that outputs are finite and in range. These check
//! that they are *correct*.

use phonetic_lib::dsp::{a_weighting, DelayFinder, ImpulseResponseProcessor, TransferFunctionProcessor};
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
    assert!(tf.average_count() > 10, "not enough averages to be meaningful");
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

    assert!(worst_mag < 0.5, "worst magnitude error {worst_mag:.3} dB (limit 0.5)");
    assert!(worst_phase < 2.0, "worst phase error {worst_phase:.3} deg (limit 2.0)");
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

/// Documents a real limitation: the delay finder result is never fed back into
/// the transfer function, so a realistic acoustic delay (10 ms = ~3.4 m) is
/// left uncompensated and coherence collapses at high frequency. Fix the
/// pipeline and this assertion should be inverted to demand good coherence.
#[test]
fn uncompensated_acoustic_delay_collapses_hf_coherence() {
    let delay_samples = 480; // 10 ms — a mic 3.4 m from the box
    let x = noise(FFT * FRAMES, 8);
    let mut y = vec![0.0; x.len()];
    y[delay_samples..].copy_from_slice(&x[..x.len() - delay_samples]);

    let d = measure(&y, &x);
    let r = band(2_000.0, 15_000.0);
    let mean: f32 = d.coherence[r.clone()].iter().sum::<f32>() / r.len() as f32;

    // The delay finder sees it perfectly...
    let mut finder = DelayFinder::new(FFT, FS, 50.0);
    let found = finder.find_delay(&x[..FFT * 2], &y[..FFT * 2]);
    assert_eq!(found.delay_samples, delay_samples as i32, "delay finder should be exact");

    // ...but nothing applies it, so the transfer function is still degraded.
    assert!(
        mean < 0.999,
        "coherence {mean:.4} — if this now reads ~1.0, delay compensation landed; \
         invert this assertion"
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

//! Analytical accuracy tests for the multi-time-window transfer function.
//!
//! These compare measured output against closed-form expectations (e.g. a
//! one-pole filter's known frequency response) rather than just checking that
//! numbers are finite.

use std::f32::consts::PI;

const FS: f32 = 48_000.0;

/// Deterministic pseudo-random noise (xorshift64), so tests are reproducible
/// without depending on external randomness or adding a dev-dependency on `rand`.
fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut state = seed.wrapping_mul(2_685_821_657_736_338_717).wrapping_add(1);
    (0..n)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state as f64 / u64::MAX as f64) as f32 * 2.0 - 1.0
        })
        .collect()
}

/// The whole point of multi-time-window: a closed-form filter recovered accurately
/// at 25 Hz, where a single 4096-point FFT has only 2 bins below 25 Hz and cannot.
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

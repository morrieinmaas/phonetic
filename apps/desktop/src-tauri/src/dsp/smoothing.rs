//! Fractional-octave smoothing.
//!
//! Averaging is done on the complex transfer function, never on dB values.
//! Averaging dB throws away phase and biases magnitude at every null.

use realfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmoothingFraction {
    None,
    Octave1,
    Octave3,
    Octave6,
    Octave12,
    Octave24,
    Octave48,
}

impl SmoothingFraction {
    /// The N in "1/N octave". `None` for no smoothing.
    pub fn denominator(&self) -> Option<f32> {
        match self {
            SmoothingFraction::None => None,
            SmoothingFraction::Octave1 => Some(1.0),
            SmoothingFraction::Octave3 => Some(3.0),
            SmoothingFraction::Octave6 => Some(6.0),
            SmoothingFraction::Octave12 => Some(12.0),
            SmoothingFraction::Octave24 => Some(24.0),
            SmoothingFraction::Octave48 => Some(48.0),
        }
    }
}

/// Average `h` over a 1/N-octave band centred on each frequency.
pub fn smooth_complex(
    h: &[Complex<f32>],
    freqs: &[f32],
    fraction: SmoothingFraction,
) -> Vec<Complex<f32>> {
    let Some(n) = fraction.denominator() else {
        return h.to_vec();
    };

    // Half-bandwidth as a frequency ratio: 2^(1/2N).
    let half = 2f32.powf(1.0 / (2.0 * n));

    freqs
        .iter()
        .enumerate()
        .map(|(i, &centre)| {
            let (lo, hi) = (centre / half, centre * half);
            let mut sum = Complex::new(0.0, 0.0);
            let mut count = 0usize;

            for (j, &f) in freqs.iter().enumerate() {
                if f >= lo && f <= hi {
                    sum += h[j];
                    count += 1;
                }
            }

            if count == 0 {
                h[i]
            } else {
                sum / count as f32
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use realfft::num_complex::Complex;

    fn axis() -> Vec<f32> {
        crate::dsp::log_frequency_axis(20.0, 20_000.0, 48)
    }

    #[test]
    fn no_smoothing_is_the_identity() {
        let freqs = axis();
        let h: Vec<Complex<f32>> = freqs
            .iter()
            .map(|f| Complex::new(f / 1000.0, 0.0))
            .collect();
        let out = smooth_complex(&h, &freqs, SmoothingFraction::None);
        assert_eq!(out, h);
    }

    #[test]
    fn smoothing_preserves_a_flat_response() {
        let freqs = axis();
        let h: Vec<Complex<f32>> = freqs.iter().map(|_| Complex::new(1.0, 0.0)).collect();
        let out = smooth_complex(&h, &freqs, SmoothingFraction::Octave3);
        for (i, v) in out.iter().enumerate() {
            assert!(
                (v.norm() - 1.0).abs() < 1e-4,
                "flat input must stay flat, bin {i} became {:.5}",
                v.norm()
            );
        }
    }

    #[test]
    fn smoothing_attenuates_a_narrow_notch() {
        let freqs = axis();
        let notch_at = freqs.len() / 2;
        let mut h: Vec<Complex<f32>> = freqs.iter().map(|_| Complex::new(1.0, 0.0)).collect();
        h[notch_at] = Complex::new(0.01, 0.0); // -40 dB, one point wide

        let out = smooth_complex(&h, &freqs, SmoothingFraction::Octave3);
        let depth_db = 20.0 * out[notch_at].norm().log10();
        assert!(
            depth_db > -6.0,
            "a one-point notch should smooth away, still {depth_db:.1} dB deep"
        );
    }

    #[test]
    fn wider_smoothing_removes_more() {
        let freqs = axis();
        let notch_at = freqs.len() / 2;
        let mut h: Vec<Complex<f32>> = freqs.iter().map(|_| Complex::new(1.0, 0.0)).collect();
        h[notch_at] = Complex::new(0.01, 0.0);

        let narrow = smooth_complex(&h, &freqs, SmoothingFraction::Octave24);
        let wide = smooth_complex(&h, &freqs, SmoothingFraction::Octave1);
        assert!(
            wide[notch_at].norm() > narrow[notch_at].norm(),
            "1/1 octave should smooth more than 1/24"
        );
    }
}

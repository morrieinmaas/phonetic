//! Virtual EQ.
//!
//! Filter responses are evaluated analytically from RBJ Audio EQ Cookbook
//! biquad coefficients. No audio is processed: the prediction is exact and
//! instant, and every part of it is a pure function.

use crate::dsp::TransferFunctionData;
use realfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    Peaking,
    LowShelf,
    HighShelf,
    HighPass,
    LowPass,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BiquadFilter {
    pub filter_type: FilterType,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

/// Q values at or below this are physically meaningless and would divide by
/// ~zero. Clamped rather than rejected so a malformed filter degrades to a
/// very wide one instead of failing a whole measurement prediction.
///
/// `f32::max` follows IEEE 754 `maxNum` semantics: when either operand is
/// NaN, the *other* (non-NaN) operand is returned. So `self.q.max(MIN_Q)`
/// is NaN-safe by construction -- a NaN `q` clamps to `MIN_Q`, never to NaN.
const MIN_Q: f32 = 0.01;

/// Direct-form biquad coefficients, normalised by a0.
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadFilter {
    /// RBJ cookbook coefficients for this filter at `sample_rate`.
    fn coefficients(&self, sample_rate: f32) -> Coefficients {
        let a = 10f32.powf(self.gain_db / 40.0);
        let w0 = 2.0 * PI * self.frequency_hz / sample_rate;
        let (sin_w0, cos_w0) = (w0.sin(), w0.cos());
        let alpha = sin_w0 / (2.0 * self.q.max(MIN_Q));

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::Peaking => (
                1.0 + alpha * a,
                -2.0 * cos_w0,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos_w0,
                1.0 - alpha / a,
            ),
            FilterType::LowShelf => {
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
                    (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
                    (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
                )
            }
            FilterType::HighShelf => {
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
                    (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
                    (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
                )
            }
            FilterType::HighPass => (
                (1.0 + cos_w0) / 2.0,
                -(1.0 + cos_w0),
                (1.0 + cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::LowPass => (
                (1.0 - cos_w0) / 2.0,
                1.0 - cos_w0,
                (1.0 - cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
        };

        Coefficients {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Complex response at `freq_hz`. A disabled filter is transparent.
    pub fn response_at(&self, freq_hz: f32, sample_rate: f32) -> Complex<f32> {
        if !self.enabled {
            return Complex::new(1.0, 0.0);
        }

        let c = self.coefficients(sample_rate);
        let w = 2.0 * PI * freq_hz / sample_rate;
        // H(e^jw) = (b0 + b1 z^-1 + b2 z^-2) / (1 + a1 z^-1 + a2 z^-2), z^-1 = e^-jw
        let z1 = Complex::new(w.cos(), -w.sin());
        let z2 = z1 * z1;

        let numerator = Complex::new(c.b0, 0.0) + z1 * c.b1 + z2 * c.b2;
        let denominator = Complex::new(1.0, 0.0) + z1 * c.a1 + z2 * c.a2;

        if denominator.norm() < 1e-20 {
            return Complex::new(1.0, 0.0);
        }
        numerator / denominator
    }
}

/// Combined response of a filter bank. Filters in series multiply.
pub fn bank_response(
    filters: &[BiquadFilter],
    freqs: &[f32],
    sample_rate: f32,
) -> Vec<Complex<f32>> {
    freqs
        .iter()
        .map(|&f| {
            filters.iter().fold(Complex::new(1.0, 0.0), |acc, filter| {
                acc * filter.response_at(f, sample_rate)
            })
        })
        .collect()
}

/// Apply a filter bank to a measured trace and return the predicted result.
/// Coherence is carried through untouched: EQ changes the response, not the
/// confidence with which the response was measured.
pub fn predict_with_eq(
    trace: &TransferFunctionData,
    filters: &[BiquadFilter],
    sample_rate: f32,
) -> TransferFunctionData {
    let eq = bank_response(filters, &trace.frequencies, sample_rate);

    let mut magnitude_db = Vec::with_capacity(eq.len());
    let mut phase_deg = Vec::with_capacity(eq.len());

    for (i, h_eq) in eq.iter().enumerate() {
        let gain_db = 20.0 * h_eq.norm().max(1e-10).log10();
        magnitude_db.push(trace.magnitude_db[i] + gain_db);

        let mut phase = trace.phase_deg[i] + h_eq.arg().to_degrees();
        while phase > 180.0 {
            phase -= 360.0;
        }
        while phase < -180.0 {
            phase += 360.0;
        }
        phase_deg.push(phase);
    }

    TransferFunctionData {
        magnitude_db,
        phase_deg,
        coherence: trace.coherence.clone(),
        group_delay_ms: trace.group_delay_ms.clone(),
        frequencies: trace.frequencies.clone(),
    }
}

//! LEQ (equivalent continuous sound level) integrator.
//!
//! LEQ is the level of the steady signal carrying the same energy as the
//! measured one: `Leq = 10*log10( (1/T)*integral p^2(t) dt / p0^2 )`. Over
//! dBFS the reference `p0` is full scale (1.0), so this reduces to a
//! running mean of the squared samples, converted to dB once at read time.
//!
//! `push` runs on the real-time audio thread: it must not allocate, must
//! not lock, and must not call anything that could. A running sum of
//! squares plus a sample count is all it needs -- see commit d2ac837 for
//! why nothing on this path may block the callback.

use std::f64::consts::PI;

/// Frequency weighting applied before energy averaging.
///
/// `Z` (zero/unweighted) passes samples through unchanged. `A` and `C` are
/// time-domain filters -- cascaded biquads designed per IEC 61672-1 at the
/// integrator's sample rate. This is deliberately NOT
/// `calibration::a_weighting`/`c_weighting`: those are per-frequency
/// corrections for spectra and cannot be applied to a time series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyWeighting {
    Z,
    A,
    C,
}

/// Reported when no samples have been pushed yet, or the accumulated
/// energy is (numerically) silent. Keeps the UI from ever rendering
/// "NaN dB" before the first block arrives.
const SILENCE_FLOOR_DB: f32 = -120.0;

/// IEC 61672-1 pole frequencies (Hz) shared by the A- and C-weighting
/// analog prototypes (the classic four corner frequencies of the standard
/// A/C curves).
const F1_HZ: f64 = 20.598_997;
const F2_HZ: f64 = 107.652_65;
const F3_HZ: f64 = 737.862_23;
const F4_HZ: f64 = 12_194.217;

/// Gain corrections so each weighting curve is exactly 0 dB at 1 kHz, per
/// IEC 61672-1 (matches the classic Couvreur `adsgn`/`cdsgn` derivation).
const A1000_DB: f64 = 1.9997;
const C1000_DB: f64 = 0.0619;

/// Direct-form-II-transposed biquad. Coefficients are normalised by a0
/// (b0, b1, b2, a1, a2), the same convention as
/// `dsp::virtual_eq::BiquadFilter`'s internal `Coefficients` -- this is a
/// second, stateful instance because that one only predicts frequency
/// response and never filters a time series.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Bilinear-transforms (no prewarp, matching the reference A/C designs)
    /// an analog 2nd-order section `(n2*s^2 + n1*s + n0) / (d2*s^2 + d1*s +
    /// d0)` into a digital biquad at `sample_rate`.
    fn from_analog_section(
        n2: f64,
        n1: f64,
        n0: f64,
        d2: f64,
        d1: f64,
        d0: f64,
        sample_rate: f32,
    ) -> Self {
        let k = 2.0 * sample_rate as f64;
        let k2 = k * k;

        let num0 = n2 * k2 + n1 * k + n0;
        let num1 = -2.0 * n2 * k2 + 2.0 * n0;
        let num2 = n2 * k2 - n1 * k + n0;

        let den0 = d2 * k2 + d1 * k + d0;
        let den1 = -2.0 * d2 * k2 + 2.0 * d0;
        let den2 = d2 * k2 - d1 * k + d0;

        Self {
            b0: (num0 / den0) as f32,
            b1: (num1 / den0) as f32,
            b2: (num2 / den0) as f32,
            a1: (den1 / den0) as f32,
            a2: (den2 / den0) as f32,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    #[inline]
    fn reset_state(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// A-weighting analog prototype (IEC 61672-1) factors into three real
/// second-order sections: `(s+w1)^2`, `(s+w4)^2` and `(s+w2)(s+w3)` in the
/// denominator, with the `gain * s^4` numerator split as `s^2 * (gain*s^2)
/// * 1` across them.
fn a_weighting_biquads(sample_rate: f32) -> [Biquad; 3] {
    let w1 = 2.0 * PI * F1_HZ;
    let w2 = 2.0 * PI * F2_HZ;
    let w3 = 2.0 * PI * F3_HZ;
    let w4 = 2.0 * PI * F4_HZ;
    let gain = w4 * w4 * 10f64.powf(A1000_DB / 20.0);

    [
        Biquad::from_analog_section(1.0, 0.0, 0.0, 1.0, 2.0 * w1, w1 * w1, sample_rate),
        Biquad::from_analog_section(gain, 0.0, 0.0, 1.0, 2.0 * w4, w4 * w4, sample_rate),
        Biquad::from_analog_section(0.0, 0.0, 1.0, 1.0, w2 + w3, w2 * w3, sample_rate),
    ]
}

/// C-weighting analog prototype (IEC 61672-1) factors into two real
/// second-order sections: `(s+w1)^2` and `(s+w4)^2` in the denominator,
/// with the `gain * s^2` numerator on the latter.
fn c_weighting_biquads(sample_rate: f32) -> [Biquad; 2] {
    let w1 = 2.0 * PI * F1_HZ;
    let w4 = 2.0 * PI * F4_HZ;
    let gain = w4 * w4 * 10f64.powf(C1000_DB / 20.0);

    [
        Biquad::from_analog_section(0.0, 0.0, 1.0, 1.0, 2.0 * w1, w1 * w1, sample_rate),
        Biquad::from_analog_section(gain, 0.0, 0.0, 1.0, 2.0 * w4, w4 * w4, sample_rate),
    ]
}

/// The weighting cascade, sized inline (no heap) since A needs three
/// stages, C needs two, and Z needs none.
#[derive(Debug, Clone)]
enum WeightingFilter {
    None,
    A([Biquad; 3]),
    C([Biquad; 2]),
}

impl WeightingFilter {
    fn new(weighting: FrequencyWeighting, sample_rate: f32) -> Self {
        match weighting {
            FrequencyWeighting::Z => Self::None,
            FrequencyWeighting::A => Self::A(a_weighting_biquads(sample_rate)),
            FrequencyWeighting::C => Self::C(c_weighting_biquads(sample_rate)),
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        match self {
            Self::None => x,
            Self::A(stages) => stages.iter_mut().fold(x, |acc, stage| stage.process(acc)),
            Self::C(stages) => stages.iter_mut().fold(x, |acc, stage| stage.process(acc)),
        }
    }

    fn reset_state(&mut self) {
        match self {
            Self::None => {}
            Self::A(stages) => stages.iter_mut().for_each(Biquad::reset_state),
            Self::C(stages) => stages.iter_mut().for_each(Biquad::reset_state),
        }
    }
}

/// Energy-averaging LEQ integrator. Accumulates a running sum of squares
/// and a sample count; the dB conversion happens only at read time.
pub struct LeqIntegrator {
    filter: WeightingFilter,
    sum_squares: f64,
    count: u64,
}

impl LeqIntegrator {
    pub fn new(sample_rate: f32, weighting: FrequencyWeighting) -> Self {
        Self {
            filter: WeightingFilter::new(weighting, sample_rate),
            sum_squares: 0.0,
            count: 0,
        }
    }

    /// Audio thread: no allocation, no lock. Filters each sample through
    /// the weighting cascade in place (no scratch buffer needed -- the
    /// biquad state already carries across samples and calls) and folds it
    /// into the running sum of squares.
    pub fn push(&mut self, samples: &[f32]) {
        for &x in samples {
            let weighted = self.filter.process(x);
            self.sum_squares += (weighted as f64) * (weighted as f64);
        }
        self.count += samples.len() as u64;
    }

    pub fn leq_db(&self) -> f32 {
        if self.count == 0 {
            return SILENCE_FLOOR_DB;
        }
        let mean_square = self.sum_squares / self.count as f64;
        if mean_square <= 0.0 {
            return SILENCE_FLOOR_DB;
        }
        (10.0 * mean_square.log10()) as f32
    }

    pub fn sample_count(&self) -> u64 {
        self.count
    }

    pub fn reset(&mut self) {
        self.sum_squares = 0.0;
        self.count = 0;
        self.filter.reset_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn sine(freq: f32, amplitude: f32, seconds: f32) -> Vec<f32> {
        let n = (FS * seconds) as usize;
        (0..n)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / FS).sin())
            .collect()
    }

    /// A sine of amplitude a has RMS a/sqrt(2), so its level is
    /// 20*log10(a) - 3.01 dBFS. Full scale therefore reads -3.01, not 0.
    #[test]
    fn leq_of_a_sine_is_its_rms_level() {
        let mut leq = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        leq.push(&sine(1000.0, 1.0, 1.0));
        let got = leq.leq_db();
        assert!(
            (got - -3.0103).abs() < 0.05,
            "full-scale sine should read -3.01 dBFS, got {got:.3}"
        );
    }

    /// Halving amplitude drops the level by 6.02 dB. Any implementation that
    /// averages dB instead of power gets this wrong.
    #[test]
    fn halving_amplitude_drops_six_db() {
        let mut loud = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        loud.push(&sine(1000.0, 1.0, 1.0));
        let mut quiet = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        quiet.push(&sine(1000.0, 0.5, 1.0));

        let delta = loud.leq_db() - quiet.leq_db();
        assert!(
            (delta - 6.0206).abs() < 0.05,
            "expected 6.02 dB, got {delta:.3}"
        );
    }

    /// The defining property of LEQ: a signal at full level for half the time
    /// has the same energy as one 3.01 dB down for the whole time. This is the
    /// test that fails if the integrator averages levels rather than energy.
    #[test]
    fn half_duration_at_full_level_equals_full_duration_three_db_down() {
        let mut intermittent = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        intermittent.push(&sine(1000.0, 1.0, 1.0));
        intermittent.push(&vec![0.0f32; FS as usize]);

        let mut steady = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        // 3.01 dB down in level is a factor of 1/sqrt(2) in amplitude.
        steady.push(&sine(1000.0, 1.0 / 2f32.sqrt(), 2.0));

        let delta = intermittent.leq_db() - steady.leq_db();
        assert!(
            delta.abs() < 0.05,
            "energy equivalence broken by {delta:.3} dB"
        );
    }

    /// Order must not matter — LEQ is an energy average, not a recency-weighted one.
    #[test]
    fn leq_is_order_independent() {
        let loud = sine(1000.0, 1.0, 0.5);
        let quiet = sine(1000.0, 0.1, 0.5);

        let mut a = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        a.push(&loud);
        a.push(&quiet);

        let mut b = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        b.push(&quiet);
        b.push(&loud);

        assert!((a.leq_db() - b.leq_db()).abs() < 1e-3);
    }

    /// A-weighting attenuates low frequencies heavily and leaves 1 kHz alone.
    /// IEC 61672-1: A(1 kHz) = 0 dB by definition, A(100 Hz) is about -19.1 dB.
    #[test]
    fn a_weighting_leaves_one_khz_and_cuts_one_hundred_hz() {
        let mut flat_1k = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        flat_1k.push(&sine(1000.0, 0.5, 2.0));
        let mut weighted_1k = LeqIntegrator::new(FS, FrequencyWeighting::A);
        weighted_1k.push(&sine(1000.0, 0.5, 2.0));
        assert!(
            (flat_1k.leq_db() - weighted_1k.leq_db()).abs() < 0.5,
            "A-weighting should be 0 dB at 1 kHz"
        );

        let mut flat_100 = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        flat_100.push(&sine(100.0, 0.5, 2.0));
        let mut weighted_100 = LeqIntegrator::new(FS, FrequencyWeighting::A);
        weighted_100.push(&sine(100.0, 0.5, 2.0));
        let attenuation = flat_100.leq_db() - weighted_100.leq_db();
        assert!(
            (attenuation - 19.1).abs() < 1.5,
            "A-weighting at 100 Hz should be about -19.1 dB, measured {attenuation:.2}"
        );
    }

    /// An empty integrator reports a floor rather than NaN, so the UI never
    /// renders "NaN dB" before the first block arrives.
    #[test]
    fn an_empty_integrator_is_finite() {
        let leq = LeqIntegrator::new(FS, FrequencyWeighting::Z);
        assert!(leq.leq_db().is_finite(), "got {}", leq.leq_db());
        assert_eq!(leq.sample_count(), 0);
    }
}

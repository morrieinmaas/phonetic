//! Multi-time-window transfer function.
//!
//! A single FFT length cannot serve the whole audio band: 4096 points at 48 kHz
//! gives 11.7 Hz resolution, which is adequate at 10 kHz and useless at 60 Hz.
//! Running several lengths in parallel and stitching them per octave gives roughly
//! constant resolution in *octaves* rather than in hertz, which is how hearing and
//! loudspeaker behaviour actually work.

use crate::dsp::{
    smooth_complex, SmoothingFraction, TransferFunctionData, TransferFunctionProcessor,
};
use realfft::num_complex::Complex;

/// Output resolution of the stitched axis. 48 per octave is finer than any display
/// needs and leaves headroom for smoothing to average over.
pub const POINTS_PER_OCTAVE: usize = 48;

/// Geometrically spaced frequency axis from `f_min` to `f_max` inclusive.
pub fn log_frequency_axis(f_min: f32, f_max: f32, points_per_octave: usize) -> Vec<f32> {
    let octaves = (f_max / f_min).log2();
    let count = (octaves * points_per_octave as f32).round() as usize;
    let ratio = (f_max / f_min).powf(1.0 / count as f32);
    (0..=count).map(|i| f_min * ratio.powi(i as i32)).collect()
}

/// FFT length paired with the upper frequency bound it serves, longest first.
/// Chosen so every band holds at least four cycles of its lowest frequency at
/// 48 kHz, which is the floor below which a transfer function stops meaning
/// anything.
pub const RESOLUTION_BANDS: [(usize, f32); 6] = [
    (16384, 100.0),
    (8192, 200.0),
    (4096, 400.0),
    (2048, 800.0),
    (1024, 1600.0),
    (512, f32::INFINITY),
];

/// FFT length serving `freq_hz`.
pub fn band_for_frequency(freq_hz: f32) -> usize {
    RESOLUTION_BANDS
        .iter()
        .find(|(_, upper)| freq_hz < *upper)
        .map(|(size, _)| *size)
        .unwrap_or(512)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_axis_spans_the_range_with_even_octave_spacing() {
        let axis = log_frequency_axis(20.0, 20_000.0, 48);

        assert!((axis[0] - 20.0).abs() < 0.01, "should start at f_min");
        assert!(
            (axis[axis.len() - 1] - 20_000.0).abs() < 1.0,
            "should end at f_max, got {}",
            axis[axis.len() - 1]
        );

        // Just under 10 octaves from 20 Hz to 20 kHz, 48 points each.
        assert!(
            axis.len() > 470 && axis.len() < 490,
            "got {} points",
            axis.len()
        );

        // Every step is the same ratio.
        let ratio = axis[1] / axis[0];
        for w in axis.windows(2) {
            assert!(
                (w[1] / w[0] - ratio).abs() < 1e-4,
                "spacing should be geometric: {} -> {}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn low_frequencies_get_the_longest_fft() {
        assert_eq!(
            band_for_frequency(30.0),
            16384,
            "30 Hz needs the longest window"
        );
        assert_eq!(band_for_frequency(60.0), 16384);
        assert_eq!(
            band_for_frequency(10_000.0),
            512,
            "10 kHz needs the shortest"
        );
    }

    #[test]
    fn band_assignment_is_monotonic() {
        let mut previous = usize::MAX;
        let mut f = 20.0_f32;
        while f < 20_000.0 {
            let size = band_for_frequency(f);
            assert!(
                size <= previous,
                "FFT size must not grow with frequency at {f} Hz"
            );
            previous = size;
            f *= 1.05;
        }
    }

    /// Delay compensation and averaging depth must reach every internal
    /// per-band processor, not just the first one. Without this, switching to
    /// multi-window mode silently drops the operator's delay compensation.
    #[test]
    fn set_delay_samples_and_max_averages_reach_every_sub_processor() {
        let mut mw = MultiWindowProcessor::new(48_000.0);
        mw.set_delay_samples(480);
        mw.set_max_averages(4);

        for p in &mw.processors {
            assert_eq!(
                p.delay_samples(),
                480,
                "delay must reach every band's processor"
            );
            assert_eq!(
                p.max_averages(),
                4,
                "averaging depth must reach every band's processor"
            );
        }
    }

    /// The point of the exercise: enough cycles inside every window to resolve the
    /// frequency. Below about 4 cycles a measurement is meaningless.
    #[test]
    fn every_band_holds_at_least_four_cycles_at_its_lowest_frequency() {
        let sample_rate = 48_000.0_f32;
        let mut lower = 20.0_f32;
        for (fft_size, upper) in RESOLUTION_BANDS {
            let window_seconds = fft_size as f32 / sample_rate;
            let cycles = lower * window_seconds;
            assert!(
                cycles >= 4.0,
                "{fft_size}-point window holds only {cycles:.1} cycles at {lower} Hz"
            );
            lower = upper;
        }
    }
}

/// Lowest and highest frequency the stitched output covers.
const F_MIN: f32 = 20.0;
const F_MAX: f32 = 20_000.0;

/// Runs one `TransferFunctionProcessor` per resolution band and stitches their
/// outputs onto a shared log-spaced axis.
pub struct MultiWindowProcessor {
    processors: Vec<TransferFunctionProcessor>,
    frequencies: Vec<f32>,
    /// Index into `processors` for each output frequency, precomputed.
    source: Vec<usize>,
    sample_rate: f32,
    smoothing: SmoothingFraction,
}

impl MultiWindowProcessor {
    pub fn new(sample_rate: f32) -> Self {
        let processors = RESOLUTION_BANDS
            .iter()
            .map(|(size, _)| TransferFunctionProcessor::new(*size, sample_rate))
            .collect();

        let frequencies = log_frequency_axis(F_MIN, F_MAX, POINTS_PER_OCTAVE);

        let source = frequencies
            .iter()
            .map(|&f| {
                let size = band_for_frequency(f);
                RESOLUTION_BANDS
                    .iter()
                    .position(|(s, _)| *s == size)
                    .unwrap_or(0)
            })
            .collect();

        Self {
            processors,
            frequencies,
            source,
            sample_rate,
            smoothing: SmoothingFraction::None,
        }
    }

    pub fn set_smoothing(&mut self, fraction: SmoothingFraction) {
        self.smoothing = fraction;
    }

    pub fn smoothing(&self) -> SmoothingFraction {
        self.smoothing
    }

    /// Feed the same samples to every resolution. Each keeps its own accumulator
    /// and produces a frame whenever it has enough samples.
    pub fn process(&mut self, measurement: &[f32], reference: &[f32]) {
        for p in &mut self.processors {
            p.process(measurement, reference);
        }
    }

    pub fn reset_averaging(&mut self) {
        for p in &mut self.processors {
            p.reset_averaging();
        }
    }

    /// Delay the reference signal by `delay` samples before computing every
    /// band's transfer function, compensating for acoustic propagation time.
    /// Forwarded to every internal per-band processor so switching resolution
    /// mode never drops the operator's delay compensation.
    pub fn set_delay_samples(&mut self, delay: usize) {
        for p in &mut self.processors {
            p.set_delay_samples(delay);
        }
    }

    /// Number of frames retained by each band's running average. Forwarded to
    /// every internal per-band processor.
    pub fn set_max_averages(&mut self, n: usize) {
        for p in &mut self.processors {
            p.set_max_averages(n);
        }
    }

    /// Linear interpolation of a processor's complex H at an arbitrary frequency.
    /// Interpolating H rather than dB and degrees keeps phase continuous through
    /// wrap points.
    fn sample_at(
        p: &TransferFunctionProcessor,
        freq: f32,
        sample_rate: f32,
    ) -> (Complex<f32>, f32) {
        let bin_width = sample_rate / p.fft_size() as f32;
        let exact = freq / bin_width;
        let lo = exact.floor() as usize;
        let last = p.h().len() - 1;

        if lo >= last {
            return (p.h()[last], p.get_coherence()[last]);
        }

        let frac = exact - lo as f32;
        let h = p.h()[lo] * (1.0 - frac) + p.h()[lo + 1] * frac;
        let coh = p.get_coherence()[lo] * (1.0 - frac) + p.get_coherence()[lo + 1] * frac;
        (h, coh)
    }

    pub fn get_data(&self) -> TransferFunctionData {
        let n = self.frequencies.len();
        let mut magnitude_db = vec![-120.0; n];
        let mut phase_deg = vec![0.0; n];
        let mut coherence = vec![0.0; n];

        let mut stitched = vec![Complex::new(0.0, 0.0); n];
        for i in 0..n {
            let p = &self.processors[self.source[i]];
            let (h, coh) = Self::sample_at(p, self.frequencies[i], self.sample_rate);
            stitched[i] = h;
            coherence[i] = coh.clamp(0.0, 1.0);
        }

        let stitched = smooth_complex(&stitched, &self.frequencies, self.smoothing);

        for i in 0..n {
            let mag = stitched[i].norm();
            magnitude_db[i] = if mag > 1e-10 {
                20.0 * mag.log10()
            } else {
                -120.0
            };
            phase_deg[i] = stitched[i].arg().to_degrees();
        }

        let group_delay_ms = group_delay_from_log_axis(&self.frequencies, &phase_deg);

        TransferFunctionData {
            magnitude_db,
            phase_deg,
            coherence,
            group_delay_ms,
            frequencies: self.frequencies.clone(),
        }
    }
}

/// Group delay on a non-uniform axis. Phase must be unwrapped locally before
/// differentiating, since the displayed phase is wrapped.
fn group_delay_from_log_axis(freqs: &[f32], phase_deg: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; freqs.len()];
    for i in 1..freqs.len().saturating_sub(1) {
        let mut dphi = phase_deg[i + 1] - phase_deg[i - 1];
        // Undo a single wrap across the interval.
        while dphi > 180.0 {
            dphi -= 360.0;
        }
        while dphi < -180.0 {
            dphi += 360.0;
        }
        let df = freqs[i + 1] - freqs[i - 1];
        out[i] = if df > 0.0 {
            -dphi / (360.0 * df) * 1000.0
        } else {
            0.0
        };
    }
    out
}

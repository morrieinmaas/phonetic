//! Multi-time-window transfer function.
//!
//! A single FFT length cannot serve the whole audio band: 4096 points at 48 kHz
//! gives 11.7 Hz resolution, which is adequate at 10 kHz and useless at 60 Hz.
//! Running several lengths in parallel and stitching them per octave gives roughly
//! constant resolution in *octaves* rather than in hertz, which is how hearing and
//! loudspeaker behaviour actually work.

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

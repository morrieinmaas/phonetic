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
}

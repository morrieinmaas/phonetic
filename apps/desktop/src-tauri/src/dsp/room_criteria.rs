//! Room criteria ratings: NC (ANSI S12.2), NR and RC.
//!
//! All three families work the same way — compare measured octave-band SPL
//! against a family of curves and report the lowest curve no band exceeds — so
//! there is one evaluation engine here and three tables.

/// Octave band centres used by NC, NR and RC.
pub const OCTAVE_CENTRES_HZ: [f32; 8] =
    [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];

/// Level reported for a band containing no FFT bins.
const EMPTY_BAND_DB: f32 = -200.0;

fn band_edges(centre_hz: f32) -> (f32, f32) {
    let half = 2f32.sqrt();
    (centre_hz / half, centre_hz * half)
}

/// Sum narrowband SPL into octave bands.
///
/// This is a power sum over the bins in each band, not an average: ten bins at
/// 50 dB make a 60 dB band.
pub fn octave_band_levels(spl_db: &[f32], frequencies: &[f32]) -> [f32; 8] {
    let mut out = [EMPTY_BAND_DB; 8];

    for (band, &centre) in OCTAVE_CENTRES_HZ.iter().enumerate() {
        let (lo, hi) = band_edges(centre);
        let power: f64 = frequencies
            .iter()
            .zip(spl_db.iter())
            .filter(|(&f, _)| f >= lo && f < hi)
            .map(|(_, &db)| 10f64.powf(db as f64 / 10.0))
            .sum();

        if power > 0.0 {
            out[band] = (10.0 * power.log10()) as f32;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_frequencies(n: usize, spacing_hz: f32) -> Vec<f32> {
        (0..n).map(|i| i as f32 * spacing_hz).collect()
    }

    /// The eight octave bands used by every room criteria family.
    #[test]
    fn octave_centres_are_the_standard_eight() {
        assert_eq!(
            OCTAVE_CENTRES_HZ,
            [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0]
        );
    }

    /// Ten bins at 50 dB each inside one band sum to 60 dB, not 50 dB.
    /// This is the test that fails if someone averages instead of power-summing.
    #[test]
    #[allow(clippy::needless_range_loop)]
    fn band_level_is_a_power_sum_not_an_average() {
        // 1 Hz spacing puts bins 708..=1414 inside the 1 kHz octave. Put ten
        // equal-level bins in there and silence everywhere else.
        let frequencies = linear_frequencies(2048, 1.0);
        let mut spl = vec![-200.0f32; frequencies.len()];
        for bin in 1000..1010 {
            spl[bin] = 50.0;
        }

        let bands = octave_band_levels(&spl, &frequencies);
        let one_khz = bands[4];
        assert!(
            (one_khz - 60.0).abs() < 0.1,
            "ten 50 dB bins should sum to 60 dB, got {one_khz:.2}"
        );
    }

    /// Energy must land in the band it belongs to and nowhere else.
    #[test]
    fn energy_lands_in_the_right_band() {
        let frequencies = linear_frequencies(20_000, 1.0);
        let mut spl = vec![-200.0f32; frequencies.len()];
        spl[1000] = 80.0; // 1000 Hz

        let bands = octave_band_levels(&spl, &frequencies);
        assert!((bands[4] - 80.0).abs() < 0.1, "1 kHz band: {:.2}", bands[4]);
        for (i, &level) in bands.iter().enumerate() {
            if i != 4 {
                assert!(level < 0.0, "band {i} should be empty, got {level:.2}");
            }
        }
    }

    /// A band with no bins reports a floor rather than -inf or NaN, so the
    /// rating logic downstream never has to special-case it.
    #[test]
    fn an_empty_band_reports_a_finite_floor() {
        let frequencies = linear_frequencies(100, 1.0); // nothing above 100 Hz
        let spl = vec![40.0f32; frequencies.len()];

        let bands = octave_band_levels(&spl, &frequencies);
        for &level in &bands[3..] {
            assert!(level.is_finite(), "empty band produced {level}");
        }
    }
}

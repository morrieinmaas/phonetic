//! Room criteria ratings: NC (ANSI S12.2) and NR (ISO 1996).
//!
//! Both families work the same way — compare measured octave-band SPL against
//! a family of curves and report the lowest curve no band exceeds (the
//! "tangency" method) — so there is one evaluation engine here and two
//! tables.
//!
//! RC (ASHRAE Room Criterion) is deliberately not implemented alongside them:
//! an RC rating is the arithmetic mean of the 500/1000/2000 Hz bands plus a
//! separate spectral-quality letter (N/LF/MF/HF). It is not a tangency
//! rating and does not fit the `rate()` API below — forcing it through would
//! silently produce a wrong acoustic rating.

use serde::{Deserialize, Serialize};

/// Octave band centres used by NC and NR.
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

/// A room-criteria curve family. RC is not a variant here — see the module
/// doc comment for why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveFamily {
    Nc,
    Nr,
}

/// One curve in a room-criteria family: a rating number and its ceiling SPL
/// at each of the eight octave bands.
#[derive(Debug, Clone, Copy)]
pub struct Curve {
    pub rating: f32,
    pub levels: [f32; 8],
}

// NC (Noise Criterion) curves — octave bands 63/125/250/500/1000/2000/4000/8000 Hz.
//
// Citation 1: The Engineering ToolBox, "Noise Criterion (NC) Levels: Definitions,
//   Standards & Calculator" — https://www.engineeringtoolbox.com/nc-noise-criterion-d_725.html
// Citation 2: Airmaster Emirates, HVAC Sound Attenuator Catalogue (2011 ed.), p.6 —
//   https://airmasteremirates.com/downloads/Sound%20Attenuator(Catalogue).pdf
//
// DISAGREEMENT FOUND: a third source (Cirrus Research / noisemeters.com technical
// note TN31, "Calculation of NR & NC Curves...", section 4.2, which cites its NC
// table explicitly to Beranek, L.L., "Revised criteria for noise in buildings,"
// Noise Control 3, 19-27 (1957) — the *original* pre-ANSI curve set) gives 1 dB
// HIGHER values at the 125 Hz and/or 250 Hz bands for NC-20, NC-25, NC-35 and
// NC-40 (e.g. NC-25 at 125 Hz: 45 dB there vs 44 dB below). Since the brief calls
// for the current ANSI S12.2 curve rather than the superseded 1957 Beranek
// original, and two independently published, mutually agreeing sources (above)
// use the values below, those are the ones used here.
const NC_CURVES: [Curve; 12] = [
    Curve {
        rating: 15.0,
        levels: [47.0, 36.0, 29.0, 22.0, 17.0, 14.0, 12.0, 11.0],
    },
    Curve {
        rating: 20.0,
        levels: [51.0, 40.0, 33.0, 26.0, 22.0, 19.0, 17.0, 16.0],
    },
    Curve {
        rating: 25.0,
        levels: [54.0, 44.0, 37.0, 31.0, 27.0, 24.0, 22.0, 21.0],
    },
    Curve {
        rating: 30.0,
        levels: [57.0, 48.0, 41.0, 35.0, 31.0, 29.0, 28.0, 27.0],
    },
    Curve {
        rating: 35.0,
        levels: [60.0, 52.0, 45.0, 40.0, 36.0, 34.0, 33.0, 32.0],
    },
    Curve {
        rating: 40.0,
        levels: [64.0, 56.0, 50.0, 45.0, 41.0, 39.0, 38.0, 37.0],
    },
    Curve {
        rating: 45.0,
        levels: [67.0, 60.0, 54.0, 49.0, 46.0, 44.0, 43.0, 42.0],
    },
    Curve {
        rating: 50.0,
        levels: [71.0, 64.0, 58.0, 54.0, 51.0, 49.0, 48.0, 47.0],
    },
    Curve {
        rating: 55.0,
        levels: [74.0, 67.0, 62.0, 58.0, 56.0, 54.0, 53.0, 52.0],
    },
    Curve {
        rating: 60.0,
        levels: [77.0, 71.0, 67.0, 63.0, 61.0, 59.0, 58.0, 57.0],
    },
    Curve {
        rating: 65.0,
        levels: [80.0, 75.0, 71.0, 68.0, 66.0, 64.0, 63.0, 62.0],
    },
    Curve {
        rating: 70.0,
        levels: [83.0, 79.0, 75.0, 72.0, 71.0, 70.0, 69.0, 68.0],
    },
];

// NR (Noise Rating) curves — octave bands 63/125/250/500/1000/2000/4000/8000 Hz.
// (The standard also defines a 31.5 Hz column; it is dropped here because it
// falls outside OCTAVE_CENTRES_HZ.)
//
// Citation 1: Cirrus Research plc / noisemeters.com, technical note TN31,
//   "Calculation of NR & NC Curves in the optimus sound level meter and
//   NoiseTools software," section 4.1 NR Table, cited there as "From ISO/R
//   1996:1971" —
//   https://support.noisemeters.com/product/optimus/TN31_Calculcation_of_NR_and_NC_Curves_in_the_optimus_sound_level_meter_and_NoiseTools_software.pdf
// Citation 2: The Engineering ToolBox, "NR - the Noise Rating Curve" —
//   https://www.engineeringtoolbox.com/nr-noise-rating-d_60.html
//
// Cross-check: the two agree exactly, band for band, at every curve checked
// (NR0 through NR70, after rounding TN31's unrounded values to the nearest
// whole dB). A third source (Castle Group Ltd, "Noise Rating (NR) and Noise
// Criteria (NC)") publishes an a/b formula table whose 500 Hz constant
// (b = 0.930) is inconsistent with both sources above and with its own
// table's values at every other band; b = 0.974 there reproduces the other
// two sources exactly, so 0.930 looks like a transcription error and that
// source was not used.
const NR_CURVES: [Curve; 14] = [
    Curve {
        rating: 0.0,
        levels: [36.0, 22.0, 12.0, 5.0, 0.0, -4.0, -6.0, -8.0],
    },
    Curve {
        rating: 10.0,
        levels: [43.0, 31.0, 21.0, 15.0, 10.0, 7.0, 4.0, 2.0],
    },
    Curve {
        rating: 20.0,
        levels: [51.0, 39.0, 31.0, 24.0, 20.0, 17.0, 14.0, 13.0],
    },
    Curve {
        rating: 30.0,
        levels: [59.0, 48.0, 40.0, 34.0, 30.0, 27.0, 25.0, 23.0],
    },
    Curve {
        rating: 40.0,
        levels: [67.0, 57.0, 49.0, 44.0, 40.0, 37.0, 35.0, 33.0],
    },
    Curve {
        rating: 50.0,
        levels: [75.0, 66.0, 59.0, 54.0, 50.0, 47.0, 45.0, 44.0],
    },
    Curve {
        rating: 60.0,
        levels: [83.0, 74.0, 68.0, 63.0, 60.0, 57.0, 55.0, 54.0],
    },
    Curve {
        rating: 70.0,
        levels: [91.0, 83.0, 77.0, 73.0, 70.0, 68.0, 66.0, 64.0],
    },
    Curve {
        rating: 80.0,
        levels: [99.0, 92.0, 86.0, 83.0, 80.0, 78.0, 76.0, 74.0],
    },
    Curve {
        rating: 90.0,
        levels: [107.0, 100.0, 96.0, 93.0, 90.0, 88.0, 86.0, 85.0],
    },
    Curve {
        rating: 100.0,
        levels: [115.0, 109.0, 105.0, 102.0, 100.0, 98.0, 96.0, 95.0],
    },
    Curve {
        rating: 110.0,
        levels: [122.0, 118.0, 114.0, 112.0, 110.0, 108.0, 107.0, 105.0],
    },
    Curve {
        rating: 120.0,
        levels: [130.0, 126.0, 124.0, 122.0, 120.0, 118.0, 117.0, 116.0],
    },
    Curve {
        rating: 130.0,
        levels: [138.0, 135.0, 133.0, 131.0, 130.0, 128.0, 127.0, 126.0],
    },
];

impl CurveFamily {
    /// Curves in ascending rating order. Nestedness (see the
    /// `curves_are_nested` test) depends on this ordering, so keep the
    /// tables sorted.
    pub fn curves(self) -> &'static [Curve] {
        match self {
            CurveFamily::Nc => &NC_CURVES,
            CurveFamily::Nr => &NR_CURVES,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomCriteriaResult {
    pub family: CurveFamily,
    pub rating: f32,
    pub exceeded_band_hz: f32,
    pub band_levels_db: [f32; 8],
    pub curve_db: [f32; 8],
}

/// Index of the band with the largest `measured - limit` margin against a
/// curve — the band an acoustician would actually go and fix.
fn band_with_largest_margin(bands_db: &[f32; 8], curve_levels: &[f32; 8]) -> usize {
    bands_db
        .iter()
        .zip(curve_levels.iter())
        .enumerate()
        .map(|(i, (&measured, &limit))| (i, measured - limit))
        .fold((0usize, f32::NEG_INFINITY), |acc, (i, margin)| {
            if margin > acc.1 {
                (i, margin)
            } else {
                acc
            }
        })
        .0
}

/// Lowest curve in the family that no band exceeds — the tangency method.
/// Louder than every curve: report the highest curve and the band that put
/// it there, rather than extrapolating a rating the standard does not define.
pub fn rate(bands_db: &[f32; 8], family: CurveFamily) -> RoomCriteriaResult {
    let curves = family.curves();

    let contained = curves.iter().find(|curve| {
        bands_db
            .iter()
            .zip(curve.levels.iter())
            .all(|(&measured, &limit)| measured <= limit)
    });

    let curve = contained.unwrap_or(&curves[curves.len() - 1]);
    let band = band_with_largest_margin(bands_db, &curve.levels);

    RoomCriteriaResult {
        family,
        rating: curve.rating,
        exceeded_band_hz: OCTAVE_CENTRES_HZ[band],
        band_levels_db: *bands_db,
        curve_db: curve.levels,
    }
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

    /// Every curve in every family falls with frequency. A transposed or
    /// mis-pasted table almost always breaks this.
    #[test]
    fn every_curve_falls_with_frequency() {
        for family in [CurveFamily::Nc, CurveFamily::Nr] {
            for curve in family.curves() {
                for w in curve.levels.windows(2) {
                    assert!(
                        w[1] <= w[0],
                        "{family:?} curve {} rises from {} to {}",
                        curve.rating,
                        w[0],
                        w[1]
                    );
                }
            }
        }
    }

    /// Curves are nested: a higher-numbered curve is at or above a lower one in
    /// every band. If this fails the rating algorithm is not well defined.
    #[test]
    fn curves_are_nested() {
        for family in [CurveFamily::Nc, CurveFamily::Nr] {
            let curves = family.curves();
            for pair in curves.windows(2) {
                for band in 0..8 {
                    assert!(
                        pair[1].levels[band] >= pair[0].levels[band],
                        "{family:?}: curve {} dips below curve {} at band {band}",
                        pair[1].rating,
                        pair[0].rating
                    );
                }
            }
        }
    }

    /// A room exactly on a curve rates as that curve, not the one above.
    #[test]
    fn sitting_exactly_on_a_curve_rates_as_that_curve() {
        let family = CurveFamily::Nc;
        let curve = family.curves()[2];
        let result = rate(&curve.levels, family);
        assert!(
            (result.rating - curve.rating).abs() < 0.01,
            "on curve {} the rating came out {}",
            curve.rating,
            result.rating
        );
    }

    /// One band over the curve raises the whole rating — the defining property
    /// of a tangency rating, and the one people get wrong by averaging.
    #[test]
    fn a_single_exceeded_band_raises_the_rating() {
        let family = CurveFamily::Nc;
        let curve = family.curves()[2];

        let mut bands = curve.levels;
        bands[0] += 12.0; // one noisy low band, everything else unchanged

        let result = rate(&bands, family);
        assert!(
            result.rating > curve.rating,
            "rating stayed at {} despite a band 12 dB over",
            result.rating
        );
        assert!(
            (result.exceeded_band_hz - OCTAVE_CENTRES_HZ[0]).abs() < 0.01,
            "blamed {} Hz instead of the band that was actually over",
            result.exceeded_band_hz
        );
    }

    /// A quiet room does not rate below the family's lowest curve; it rates at
    /// it. Reporting an extrapolated number would be inventing precision the
    /// standard does not define.
    #[test]
    fn a_very_quiet_room_clamps_to_the_lowest_curve() {
        let family = CurveFamily::Nc;
        let lowest = family.curves()[0].rating;
        let result = rate(&[0.0f32; 8], family);
        assert!(
            (result.rating - lowest).abs() < 0.01,
            "silence rated {} against a lowest curve of {lowest}",
            result.rating
        );
    }

    /// Rating rises monotonically with level: adding noise never improves a room.
    #[test]
    fn rating_rises_with_level() {
        let family = CurveFamily::Nc;
        let mut previous = f32::NEG_INFINITY;
        for offset in 0..12 {
            let bands = [20.0 + offset as f32 * 5.0; 8];
            let rating = rate(&bands, family).rating;
            assert!(
                rating >= previous,
                "rating fell from {previous} to {rating} as the room got louder"
            );
            previous = rating;
        }
    }
}

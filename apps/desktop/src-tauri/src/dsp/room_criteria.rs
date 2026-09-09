//! Room criteria ratings: NC (ANSI S12.2), NR (ISO 1996), and RC (ASHRAE
//! Room Criterion, Mark II).
//!
//! NC and NR are tangency ratings — compare measured octave-band SPL against
//! a family of curves and report the lowest curve no band exceeds — so
//! there is one evaluation engine (`rate`) here and two tables.
//!
//! RC Mark II is not a tangency rating and does not fit that engine: it
//! reports the arithmetic mean of the 500/1000/2000 Hz bands as a numeric
//! level, plus a separate spectral-quality letter (N/LF/MF/HF) describing how
//! the measured spectrum deviates from a −5 dB/octave reference line through
//! that level at 1000 Hz. `rate_rc` is its own function — see the citation
//! comment above it for the classification procedure and its sources.

use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Octave band centres used by NC and NR.
pub const OCTAVE_CENTRES_HZ: [f32; 8] = [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];

/// Octave band centres used by RC — 16 Hz to 4 kHz, the range RC Mark II is
/// defined over. This is deliberately not merged into `OCTAVE_CENTRES_HZ`:
/// NC and NR are defined on their own eight bands and their curve tables are
/// keyed to that array, so widening it would ripple through working, tested
/// code for no benefit.
pub const RC_OCTAVE_CENTRES_HZ: [f32; 9] = [
    16.0, 31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0,
];

/// Level reported for a band containing no FFT bins.
const EMPTY_BAND_DB: f32 = -200.0;

fn band_edges(centre_hz: f32) -> (f32, f32) {
    let half = 2f32.sqrt();
    (centre_hz / half, centre_hz * half)
}

/// Power-sum narrowband SPL into octave bands centred on `centres`, and
/// report how many FFT bins backed each band.
///
/// This is a power sum over the bins in each band, not an average: ten bins
/// at 50 dB make a 60 dB band. A band with zero bins reports `EMPTY_BAND_DB`
/// rather than -inf or NaN, so rating logic downstream never has to
/// special-case it — but the bin count is returned alongside so a caller can
/// still tell a real silence from an unresolved band.
fn band_levels(spl_db: &[f32], frequencies: &[f32], centres: &[f32]) -> (Vec<f32>, Vec<usize>) {
    centres
        .iter()
        .map(|&centre| {
            let (lo, hi) = band_edges(centre);
            let bin_powers: Vec<f64> = frequencies
                .iter()
                .zip(spl_db.iter())
                .filter(|(&f, _)| f >= lo && f < hi)
                .map(|(_, &db)| 10f64.powf(db as f64 / 10.0))
                .collect();

            let level = if bin_powers.is_empty() {
                EMPTY_BAND_DB
            } else {
                (10.0 * bin_powers.iter().sum::<f64>().log10()) as f32
            };

            (level, bin_powers.len())
        })
        .unzip()
}

/// Sum narrowband SPL into the eight NC/NR octave bands.
///
/// Thin wrapper over `band_levels` — see that function for the summing rule.
pub fn octave_band_levels(spl_db: &[f32], frequencies: &[f32]) -> [f32; 8] {
    let (levels, _bins_per_band) = band_levels(spl_db, frequencies, &OCTAVE_CENTRES_HZ);
    levels.try_into().expect("OCTAVE_CENTRES_HZ has 8 elements")
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

// RC Mark II (ASHRAE Room Criterion, refined by Blazier, 1997) — octave
// bands 16 Hz to 4 kHz, grouped into three spectral regions.
//
// Citation 1: ASHRAE Handbook 2023, HVAC Applications, Chapter 49, "Noise
//   and Vibration Control" — https://handbook.ashrae.org/Handbooks/A23/IP/A23_Ch49/a23_ch49_ip.aspx
//   "The number is the arithmetic average rounded to the nearest integer of
//   sound pressure levels in the 500, 1000, and 2000 Hz octave bands...
//   The RC reference curve is chosen to be that which has the same value at
//   1000 Hz as the calculated average value... Calculate the energy-averaged
//   spectral deviations from the RC reference curve in each of three
//   frequency groups: low (LF; 16 to 63 Hz), medium (MF; 125 to 500 Hz), and
//   high (HF; 1000 to 4000 Hz)... QAI is the range in dB between the highest
//   and lowest values of the spectral deviation factors... If QAI <= 5 dB,
//   the spectrum is assigned a neutral (N) rating. If QAI exceeds 5 dB, the
//   sound quality descriptor of the RC rating is the letter designation of
//   the frequency region of the deviation factor having the highest
//   positive value."
// Citation 2: Arup, "RC-II Rating" (Strutt help documentation) —
//   https://strutt.arup.com/help/Ratings/RCII.htm
//   Gives the same three regions (LF: 16/31.5/63 Hz, MF: 125/250/500 Hz, HF:
//   1000/2000/4000 Hz) and the same per-region formula, e.g.
//   LF = 10*log10((10^(0.1*delta16) + 10^(0.1*delta31.5) + 10^(0.1*delta63))/3),
//   with QAI defined as "the range from the highest to the lowest of HF, MF
//   and LF" and the same QAI < 5 -> Neutral threshold.
//
// The two sources agree exactly on the region boundaries, the energy-average
// formula, and the 5 dB threshold — no disagreement to report here (unlike
// the NC/NR tables above).
//
// One judgment call the sources leave open: what "highest positive value"
// means when none of the three factors is actually positive. This
// implementation assigns the descriptor to whichever factor is largest
// regardless of sign, which is the only reading that always produces an
// answer and agrees with the sources whenever at least one factor is
// positive (the case both citations' worked examples cover).

/// Indices into `RC_OCTAVE_CENTRES_HZ` for each RC Mark II spectral region.
const RC_LF_BANDS: Range<usize> = 0..3; // 16, 31.5, 63 Hz
const RC_MF_BANDS: Range<usize> = 3..6; // 125, 250, 500 Hz
const RC_HF_BANDS: Range<usize> = 6..9; // 1000, 2000, 4000 Hz

/// Indices of the 500/1000/2000 Hz bands the RC level is averaged from.
const RC_LEVEL_BANDS: Range<usize> = 5..8;

/// QAI at or below this is Neutral; above it, the spectrum takes the letter
/// of whichever region deviates most.
const RC_QAI_NEUTRAL_THRESHOLD_DB: f32 = 5.0;

/// Minimum FFT bins a band needs before its level counts as resolved.
///
/// A single bin is not a sum — it is that one bin's raw narrowband value
/// standing in for an entire octave, as likely to land on a spectral null or
/// a stray tone as to represent the band's true energy. Two is the smallest
/// count at which `band_levels`'s power sum is doing any averaging at all,
/// so that is the floor used here. (At the default 2048-point FFT and 48 kHz
/// the 16 Hz band has zero bins and the 31.5 Hz band has exactly one — both
/// below this floor, which is exactly the case this flag exists to catch.)
const MIN_BINS_PER_RC_BAND: usize = 2;

/// RC reference line: −5 dB/octave through `level_at_1k_db` at 1000 Hz.
fn rc_reference_db(level_at_1k_db: f32, centre_hz: f32) -> f32 {
    const SLOPE_DB_PER_OCTAVE: f32 = -5.0;
    let octaves_from_1k = (centre_hz / 1000.0).log2();
    level_at_1k_db + SLOPE_DB_PER_OCTAVE * octaves_from_1k
}

/// Energy-averaged spectral deviation factor for one RC Mark II region: the
/// power average, in dB, of (measured − reference) across the region's
/// bands.
fn spectral_deviation_factor(
    band_levels_db: &[f32],
    reference_db: &[f32],
    region: Range<usize>,
) -> f32 {
    let count = region.len() as f64;
    let power_sum: f64 = region
        .map(|i| 10f64.powf((band_levels_db[i] - reference_db[i]) as f64 / 10.0))
        .sum();
    (10.0 * (power_sum / count).log10()) as f32
}

/// RC Mark II spectral quality descriptor — how the room sounds, not how
/// loud it is. See the citation comment above `rate_rc` for the
/// classification procedure and its sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RcQuality {
    Neutral,
    LowFrequency,
    MidFrequency,
    HighFrequency,
}

/// Result of an RC Mark II rating: a numeric level plus a spectral quality
/// letter, and how well-resolved the measurement backing it was.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcResult {
    pub level: f32,
    pub quality: RcQuality,
    pub band_levels_db: Vec<f32>,
    pub bins_per_band: Vec<usize>,
    pub reference_db: Vec<f32>,
    pub under_resolved: bool,
    /// The minimum bin count a band needs to be trustworthy, carried over the
    /// wire so the display marks the same bands thin that `under_resolved` was
    /// computed from. Duplicating this constant in TypeScript would let the two
    /// sides disagree silently the moment either changed.
    pub min_bins_per_band: usize,
}

/// Rate a spectrum on RC Mark II: a numeric level (the 500/1000/2000 Hz
/// average) plus a spectral quality letter. Not a tangency rating — see the
/// module doc comment and the citation comment above for why this has its
/// own function instead of going through `rate()`.
pub fn rate_rc(spl_db: &[f32], frequencies: &[f32]) -> RcResult {
    let (band_levels_db, bins_per_band) = band_levels(spl_db, frequencies, &RC_OCTAVE_CENTRES_HZ);

    let level = {
        let sum: f32 = RC_LEVEL_BANDS.map(|i| band_levels_db[i]).sum();
        (sum / RC_LEVEL_BANDS.len() as f32).round()
    };

    let reference_db: Vec<f32> = RC_OCTAVE_CENTRES_HZ
        .iter()
        .map(|&f| rc_reference_db(level, f))
        .collect();

    let lf = spectral_deviation_factor(&band_levels_db, &reference_db, RC_LF_BANDS);
    let mf = spectral_deviation_factor(&band_levels_db, &reference_db, RC_MF_BANDS);
    let hf = spectral_deviation_factor(&band_levels_db, &reference_db, RC_HF_BANDS);

    let factors = [lf, mf, hf];
    let qai = factors.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        - factors.iter().copied().fold(f32::INFINITY, f32::min);

    let quality = if qai <= RC_QAI_NEUTRAL_THRESHOLD_DB {
        RcQuality::Neutral
    } else {
        [
            (RcQuality::LowFrequency, lf),
            (RcQuality::MidFrequency, mf),
            (RcQuality::HighFrequency, hf),
        ]
        .into_iter()
        .fold(
            (RcQuality::Neutral, f32::NEG_INFINITY),
            |best, candidate| {
                if candidate.1 > best.1 {
                    candidate
                } else {
                    best
                }
            },
        )
        .0
    };

    let under_resolved = bins_per_band.iter().any(|&n| n < MIN_BINS_PER_RC_BAND);

    RcResult {
        level,
        quality,
        band_levels_db,
        bins_per_band,
        reference_db,
        under_resolved,
        min_bins_per_band: MIN_BINS_PER_RC_BAND,
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

    // -- RC Mark II -----------------------------------------------------

    /// A synthetic spectrum with exactly one narrowband bin at each RC
    /// octave centre, at the given per-band level. There is no other energy
    /// on the frequency axis at all, so `rate_rc`'s power sum for each band
    /// reduces to that single value.
    fn rc_single_bin_spectrum(levels_by_band: [f32; 9]) -> (Vec<f32>, Vec<f32>) {
        (levels_by_band.to_vec(), RC_OCTAVE_CENTRES_HZ.to_vec())
    }

    /// Band levels that sit exactly on the RC-35 reference line.
    fn on_curve_rc35_bands() -> [f32; 9] {
        let mut bands = [0.0f32; 9];
        for (i, &f) in RC_OCTAVE_CENTRES_HZ.iter().enumerate() {
            bands[i] = rc_reference_db(35.0, f);
        }
        bands
    }

    /// A spectrum sitting exactly on an RC-35 reference line rates level
    /// ≈35 and quality Neutral.
    #[test]
    fn on_curve_spectrum_rates_neutral_at_its_own_level() {
        let (spl, frequencies) = rc_single_bin_spectrum(on_curve_rc35_bands());

        let result = rate_rc(&spl, &frequencies);

        assert!(
            (result.level - 35.0).abs() < 0.5,
            "expected level ~35, got {}",
            result.level
        );
        assert_eq!(
            result.quality,
            RcQuality::Neutral,
            "on-curve spectrum should rate Neutral, got {:?}",
            result.quality
        );
    }

    /// Low-frequency energy well above the reference yields LF, not MF or
    /// HF — proves the regions are not aliased.
    #[test]
    fn low_frequency_excess_rates_low_frequency_only() {
        let mut bands = on_curve_rc35_bands();
        bands[0] += 30.0; // 16 Hz
        bands[1] += 30.0; // 31.5 Hz
        bands[2] += 30.0; // 63 Hz
        let (spl, frequencies) = rc_single_bin_spectrum(bands);

        let result = rate_rc(&spl, &frequencies);

        assert_eq!(
            result.quality,
            RcQuality::LowFrequency,
            "a 30 dB LF-region excess should rate LowFrequency, got {:?}",
            result.quality
        );
    }

    /// Mid-frequency energy well above the reference yields MF, not LF or
    /// HF. The 500 Hz band is left untouched since it also feeds the RC
    /// level.
    #[test]
    fn mid_frequency_excess_rates_mid_frequency_only() {
        let mut bands = on_curve_rc35_bands();
        bands[3] += 30.0; // 125 Hz
        bands[4] += 30.0; // 250 Hz
        let (spl, frequencies) = rc_single_bin_spectrum(bands);

        let result = rate_rc(&spl, &frequencies);

        assert_eq!(
            result.quality,
            RcQuality::MidFrequency,
            "a 30 dB MF-region excess should rate MidFrequency, got {:?}",
            result.quality
        );
    }

    /// High-frequency energy well above the reference yields HF, not LF or
    /// MF. The 1000/2000 Hz bands are left untouched since they also feed
    /// the RC level.
    #[test]
    fn high_frequency_excess_rates_high_frequency_only() {
        let mut bands = on_curve_rc35_bands();
        bands[8] += 30.0; // 4000 Hz
        let (spl, frequencies) = rc_single_bin_spectrum(bands);

        let result = rate_rc(&spl, &frequencies);

        assert_eq!(
            result.quality,
            RcQuality::HighFrequency,
            "a 30 dB HF-region excess should rate HighFrequency, got {:?}",
            result.quality
        );
    }

    /// The level depends only on the 500/1000/2000 Hz bands: changing the
    /// 63 Hz band alone must not move it, even though it moves the quality
    /// letter.
    #[test]
    fn level_ignores_a_63_hz_change_but_the_quality_letter_may_move() {
        let base = on_curve_rc35_bands();
        let (spl, frequencies) = rc_single_bin_spectrum(base);
        let base_result = rate_rc(&spl, &frequencies);

        let mut perturbed = base;
        perturbed[2] += 25.0; // 63 Hz only
        let (spl, frequencies) = rc_single_bin_spectrum(perturbed);
        let perturbed_result = rate_rc(&spl, &frequencies);

        assert_eq!(
            base_result.level, perturbed_result.level,
            "moving only the 63 Hz band changed the level from {} to {}",
            base_result.level, perturbed_result.level
        );
        assert_eq!(
            perturbed_result.quality,
            RcQuality::LowFrequency,
            "a 25 dB excess at 63 Hz alone should still move the quality letter to LF, got {:?}",
            perturbed_result.quality
        );
    }

    /// A frequency axis too coarse to populate the low bands — the default
    /// 2048-point FFT at 48 kHz, bin spacing 48000/2048 = 23.4375 Hz — sets
    /// `under_resolved` and reports the true (zero or one) bin counts for
    /// the 16 Hz and 31.5 Hz bands.
    #[test]
    fn coarse_fft_axis_flags_under_resolved_and_reports_true_bin_counts() {
        let fft_size = 2048usize;
        let sample_rate = 48_000.0f32;
        let bin_count = fft_size / 2 + 1;
        let frequencies: Vec<f32> = (0..bin_count)
            .map(|i| i as f32 * sample_rate / fft_size as f32)
            .collect();
        let spl = vec![40.0f32; frequencies.len()];

        let result = rate_rc(&spl, &frequencies);

        assert_eq!(
            result.bins_per_band[0], 0,
            "16 Hz band should contain zero bin centres at this resolution, got {}",
            result.bins_per_band[0]
        );
        assert_eq!(
            result.bins_per_band[1], 1,
            "31.5 Hz band should contain exactly one bin centre at this resolution, got {}",
            result.bins_per_band[1]
        );
        assert!(
            result.under_resolved,
            "a zero-bin 16 Hz band should set under_resolved"
        );
    }
}

//! Noise-limit presets and exposure dose.
//!
//! Phonetic ships *presets*, not a legal database. It must never present a
//! limit as authoritative: every shipped preset carries a non-empty
//! `source` citation and an `as_of` date, and if a number cannot be cited it
//! does not ship (see `every_builtin_limit_is_sourced_and_dated` below).
//!
//! The two built-in presets are the EU Physical Agents (Noise) Directive
//! 2003/10/EC lower and upper exposure action values (Article 3(1)(c) and
//! 3(1)(b): LEX,8h = 80 dB(A) and 85 dB(A), each over an 8-hour reference
//! period per the Article 2 definition of LEX,8h). Both were verified
//! against two independent renderings of the directive text -- EUR-Lex
//! (<https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A02003L0010-20190726>)
//! and legislation.gov.uk's consolidated copy
//! (<https://www.legislation.gov.uk/eudr/2003/10/article/3/data.htm?view=plain>)
//! -- which agree exactly. The directive's *third* value, the 87 dB(A)
//! exposure limit value, is deliberately not shipped: it is equally
//! well-sourced, but the brief names only the two action values as presets,
//! and a preset nobody asked for is exactly the kind of unrequested
//! addition this project avoids.
//!
//! `lapeak_limit_db` is left `None` on both built-ins even though the
//! directive also fixes peak values (112 Pa / 140 Pa, i.e. 135 / 137 dB(C)
//! via 20*log10(p/2e-5 Pa) -- confirmed C-weighted by the same two sources).
//! `LeqMetrics::lapeak_db` in this codebase is the A-weighted peak
//! (`LeqMeter`'s `a_filter` feeds it, see `dsp::leq`), not C-weighted. A/C
//! weighting diverge most at the low frequencies impulsive peaks often
//! carry, so comparing this app's LAFpeak against the directive's LCpeak
//! would silently under-report peak exceedance -- worse than shipping no
//! peak limit at all.
//!
//! The single source of truth for the built-in data is
//! `resources/noise_limits.json`, embedded at compile time so `builtin()`
//! and `load()` share one parser and can never drift apart.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::LeqMetrics;

/// One noise-exposure preset. Crosses the Rust/TS boundary in a later task
/// -- field names stay snake_case, never add
/// `#[serde(rename_all = "camelCase")]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseLimit {
    pub id: String,
    pub name: String,
    /// Required, non-empty: instrument, article/section, date. Never a vague
    /// "EU directive".
    pub source: String,
    /// Required, ISO date: when the cited value was verified correct.
    pub as_of: String,
    pub laeq_limit_db: f32,
    pub reference_hours: f64,
    pub lapeak_limit_db: Option<f32>,
}

/// A collection of presets, loadable from and savable to a plain JSON array
/// on disk (not wrapped in an object -- an empty file is simply `[]`).
#[derive(Debug, Clone, Default)]
pub struct LimitSet {
    pub limits: Vec<NoiseLimit>,
}

/// The built-in preset data, embedded at compile time so `builtin()` can
/// never see a different dataset than a maintainer editing the JSON file.
const BUILTIN_LIMITS_JSON: &str = include_str!("../../resources/noise_limits.json");

impl LimitSet {
    /// Loads a limit set from a plain JSON array on disk. A malformed file
    /// is always an error -- an operator who edited their limits and made a
    /// typo must find out, never fall back to the built-ins and measure
    /// against numbers they did not choose. An empty array is legitimately
    /// empty and loads successfully with zero limits.
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|err| format!("cannot read limits file at {}: {err}", path.display()))?;
        Self::parse(&contents)
            .map_err(|err| format!("malformed limits file at {}: {err}", path.display()))
    }

    /// Writes the set to disk as a plain JSON array, preserving every field
    /// -- including `source` and `as_of` -- so an edited dataset never
    /// silently loses its citations.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.limits)
            .map_err(|err| format!("cannot serialize limits: {err}"))?;
        std::fs::write(path, json)
            .map_err(|err| format!("cannot write limits file at {}: {err}", path.display()))
    }

    /// The shipped presets -- see the module doc comment for their sources.
    pub fn builtin() -> Self {
        Self::parse(BUILTIN_LIMITS_JSON).expect("resources/noise_limits.json must parse")
    }

    fn parse(contents: &str) -> Result<Self, String> {
        let limits: Vec<NoiseLimit> =
            serde_json::from_str(contents).map_err(|err| err.to_string())?;
        Ok(Self { limits })
    }
}

/// The result of comparing a measurement against a limit's reference period.
/// Crosses the Rust/TS boundary in a later task -- snake_case, never
/// `rename_all = "camelCase"`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DoseResult {
    /// Exposure dose accumulated so far, as a percentage of the limit's
    /// full-reference-period energy budget. 100% means the limit's energy
    /// has been used up.
    pub percent: f64,
    /// What `percent` would be if the current LAeq held for the entire
    /// reference period, i.e. the dose rate projected forward rather than
    /// scaled by elapsed time so far.
    pub projected_percent_at_reference: f64,
    /// Whether the measured A-weighted peak exceeded the limit's peak
    /// ceiling. Always `false` when the limit has no peak ceiling
    /// (`lapeak_limit_db` is `None`) -- absence of a limit is not evidence
    /// of compliance, just absence of a claim.
    pub peak_exceeded: bool,
}

/// Exposure energy against `limit`'s energy over its reference period:
/// `percent = 100 * (T_elapsed / T_ref) * 10^((LAeq - L_limit) / 10)`.
///
/// This is an energy (not level) relationship, so 3 dB over the limit
/// doubles the dose and exactly-at-the-limit-for-the-full-reference-period
/// is exactly 100%.
pub fn dose(metrics: &LeqMetrics, limit: &NoiseLimit) -> DoseResult {
    let reference_seconds = limit.reference_hours * 3600.0;
    let energy_ratio = 10f64.powf((metrics.laeq_db as f64 - limit.laeq_limit_db as f64) / 10.0);

    let percent = if reference_seconds > 0.0 {
        100.0 * (metrics.elapsed_seconds / reference_seconds) * energy_ratio
    } else {
        0.0
    };

    let projected_percent_at_reference = if metrics.elapsed_seconds > 0.0 {
        100.0 * energy_ratio
    } else {
        0.0
    };

    let peak_exceeded = limit
        .lapeak_limit_db
        .is_some_and(|ceiling| metrics.lapeak_db > ceiling);

    DoseResult {
        percent,
        projected_percent_at_reference,
        peak_exceeded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics_with(laeq: f32, elapsed_seconds: f64) -> LeqMetrics {
        LeqMetrics {
            leq_db: laeq,
            laeq_db: laeq,
            lceq_db: laeq,
            lafmax_db: laeq,
            lapeak_db: laeq,
            elapsed_seconds,
        }
    }

    /// Every shipped preset is sourced and dated. A preset without both is a
    /// number presented as authority, which this plan forbids.
    #[test]
    fn every_builtin_limit_is_sourced_and_dated() {
        let set = LimitSet::builtin();
        assert!(!set.limits.is_empty(), "no presets at all");
        for limit in &set.limits {
            assert!(
                !limit.source.trim().is_empty(),
                "{} has no source",
                limit.name
            );
            assert!(
                limit.as_of.len() >= 4 && limit.as_of.chars().next().unwrap().is_ascii_digit(),
                "{} has no usable as_of date: {:?}",
                limit.name,
                limit.as_of
            );
        }
    }

    /// Exactly at the limit for exactly the reference period is 100% dose.
    #[test]
    fn at_the_limit_for_the_reference_period_is_full_dose() {
        let limit = NoiseLimit {
            id: "test".into(),
            name: "Test".into(),
            source: "test".into(),
            as_of: "2026-01-01".into(),
            laeq_limit_db: 85.0,
            reference_hours: 8.0,
            lapeak_limit_db: None,
        };
        let metrics = metrics_with(85.0, 8.0 * 3600.0);
        let d = dose(&metrics, &limit);
        assert!((d.percent - 100.0).abs() < 0.1, "got {:.2}%", d.percent);
    }

    /// 3 dB over the limit doubles the dose — the energy relationship that
    /// makes an exchange-rate error visible.
    #[test]
    fn three_db_over_doubles_the_dose() {
        let limit = NoiseLimit {
            id: "test".into(),
            name: "Test".into(),
            source: "test".into(),
            as_of: "2026-01-01".into(),
            laeq_limit_db: 85.0,
            reference_hours: 8.0,
            lapeak_limit_db: None,
        };
        let at = dose(&metrics_with(85.0, 3600.0), &limit).percent;
        let over = dose(&metrics_with(88.0, 3600.0), &limit).percent;
        assert!((over / at - 2.0).abs() < 0.02, "ratio {:.4}", over / at);
    }

    /// A round trip through disk preserves every field, including the source
    /// and date, so an edited dataset does not silently lose its citations.
    #[test]
    fn a_saved_limit_set_round_trips_with_its_citations() {
        let mut path = std::env::temp_dir();
        path.push(format!("phonetic-limits-{}.json", std::process::id()));

        let original = LimitSet::builtin();
        original.save(&path).unwrap();
        let loaded = LimitSet::load(&path).unwrap();

        assert_eq!(loaded.limits.len(), original.limits.len());
        for (a, b) in original.limits.iter().zip(loaded.limits.iter()) {
            assert_eq!(a.source, b.source);
            assert_eq!(a.as_of, b.as_of);
            assert!((a.laeq_limit_db - b.laeq_limit_db).abs() < 1e-6);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// A malformed file is an error naming the problem, not a silent fallback
    /// to built-ins — an operator who edited their limits and made a typo must
    /// find out, not measure against numbers they did not choose.
    #[test]
    fn a_malformed_limit_file_is_an_error_not_a_silent_fallback() {
        let mut path = std::env::temp_dir();
        path.push(format!("phonetic-limits-bad-{}.json", std::process::id()));
        std::fs::write(&path, "{ not json").unwrap();

        let result = LimitSet::load(&path);
        assert!(
            result.is_err(),
            "malformed file loaded as {:?}",
            result.map(|s| s.limits.len())
        );
        let _ = std::fs::remove_file(&path);
    }

    /// An empty JSON array is legitimately empty, not malformed: an operator
    /// who deletes every preset should get zero limits, not an error.
    #[test]
    fn an_empty_array_loads_as_zero_limits_not_an_error() {
        let mut path = std::env::temp_dir();
        path.push(format!("phonetic-limits-empty-{}.json", std::process::id()));
        std::fs::write(&path, "[]").unwrap();

        let loaded = LimitSet::load(&path).unwrap();
        assert!(loaded.limits.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}

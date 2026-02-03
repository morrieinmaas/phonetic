//! Reference curves for audio measurement calibration
//!
//! This module provides standard reference curves used in professional audio measurement:
//! - ISO 226:2003 equal-loudness contours (phon curves)
//! - X-Curve (ISO 2969) for cinema calibration
//! - Common house curves for live sound
//! - Flat reference line

use serde::{Deserialize, Serialize};

/// Reference curve types available for overlay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceCurveType {
    /// Flat response (0 dB across all frequencies)
    Flat,
    /// ISO 226:2003 equal-loudness contour at 40 phon
    Iso226_40Phon,
    /// ISO 226:2003 equal-loudness contour at 60 phon
    Iso226_60Phon,
    /// ISO 226:2003 equal-loudness contour at 80 phon
    Iso226_80Phon,
    /// X-Curve (ISO 2969) for cinema calibration
    XCurve,
    /// House curve: gentle low-frequency boost, high-frequency rolloff
    HouseCurve,
    /// A-weighting curve (inverse of A-weighting filter)
    AWeighting,
    /// C-weighting curve (inverse of C-weighting filter)
    CWeighting,
}

impl ReferenceCurveType {
    /// Get all available curve types
    pub fn all() -> Vec<Self> {
        vec![
            Self::Flat,
            Self::Iso226_40Phon,
            Self::Iso226_60Phon,
            Self::Iso226_80Phon,
            Self::XCurve,
            Self::HouseCurve,
            Self::AWeighting,
            Self::CWeighting,
        ]
    }

    /// Get display name for the curve type
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Flat => "Flat",
            Self::Iso226_40Phon => "ISO 226 (40 phon)",
            Self::Iso226_60Phon => "ISO 226 (60 phon)",
            Self::Iso226_80Phon => "ISO 226 (80 phon)",
            Self::XCurve => "X-Curve (Cinema)",
            Self::HouseCurve => "House Curve",
            Self::AWeighting => "A-Weighting",
            Self::CWeighting => "C-Weighting",
        }
    }
}

/// Reference curve data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceCurvePoint {
    pub frequency: f64,
    pub amplitude_db: f64,
}

/// Complete reference curve with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceCurve {
    pub curve_type: ReferenceCurveType,
    pub name: String,
    pub description: String,
    pub points: Vec<ReferenceCurvePoint>,
}

impl ReferenceCurve {
    /// Generate a reference curve at specified frequencies
    pub fn generate(curve_type: ReferenceCurveType, frequencies: &[f64]) -> Self {
        let points: Vec<ReferenceCurvePoint> = frequencies
            .iter()
            .map(|&freq| ReferenceCurvePoint {
                frequency: freq,
                amplitude_db: get_curve_value(curve_type, freq),
            })
            .collect();

        Self {
            curve_type,
            name: curve_type.display_name().to_string(),
            description: get_curve_description(curve_type),
            points,
        }
    }

    /// Interpolate curve value at a specific frequency
    #[allow(dead_code)]
    pub fn interpolate(&self, frequency: f64) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }

        // Find surrounding points
        let mut lower_idx = 0;
        let mut upper_idx = self.points.len() - 1;

        for (i, point) in self.points.iter().enumerate() {
            if point.frequency <= frequency {
                lower_idx = i;
            }
            if point.frequency >= frequency && i < upper_idx {
                upper_idx = i;
                break;
            }
        }

        if lower_idx == upper_idx {
            return self.points[lower_idx].amplitude_db;
        }

        // Linear interpolation in log-frequency domain
        let f1 = self.points[lower_idx].frequency;
        let f2 = self.points[upper_idx].frequency;
        let a1 = self.points[lower_idx].amplitude_db;
        let a2 = self.points[upper_idx].amplitude_db;

        let log_f = frequency.ln();
        let log_f1 = f1.ln();
        let log_f2 = f2.ln();

        let t = (log_f - log_f1) / (log_f2 - log_f1);
        a1 + t * (a2 - a1)
    }
}

/// Get curve value at a specific frequency
fn get_curve_value(curve_type: ReferenceCurveType, frequency: f64) -> f64 {
    match curve_type {
        ReferenceCurveType::Flat => 0.0,
        ReferenceCurveType::Iso226_40Phon => iso226_equal_loudness(frequency, 40.0),
        ReferenceCurveType::Iso226_60Phon => iso226_equal_loudness(frequency, 60.0),
        ReferenceCurveType::Iso226_80Phon => iso226_equal_loudness(frequency, 80.0),
        ReferenceCurveType::XCurve => x_curve(frequency),
        ReferenceCurveType::HouseCurve => house_curve(frequency),
        ReferenceCurveType::AWeighting => a_weighting_curve(frequency),
        ReferenceCurveType::CWeighting => c_weighting_curve(frequency),
    }
}

/// Get curve description
fn get_curve_description(curve_type: ReferenceCurveType) -> String {
    match curve_type {
        ReferenceCurveType::Flat => "Flat frequency response reference".to_string(),
        ReferenceCurveType::Iso226_40Phon => {
            "ISO 226:2003 equal-loudness contour at 40 phon (quiet listening)".to_string()
        }
        ReferenceCurveType::Iso226_60Phon => {
            "ISO 226:2003 equal-loudness contour at 60 phon (moderate listening)".to_string()
        }
        ReferenceCurveType::Iso226_80Phon => {
            "ISO 226:2003 equal-loudness contour at 80 phon (loud listening)".to_string()
        }
        ReferenceCurveType::XCurve => {
            "X-Curve (ISO 2969) for cinema sound system calibration".to_string()
        }
        ReferenceCurveType::HouseCurve => {
            "Typical house curve for live sound: bass boost, HF rolloff".to_string()
        }
        ReferenceCurveType::AWeighting => "A-weighting frequency response curve".to_string(),
        ReferenceCurveType::CWeighting => "C-weighting frequency response curve".to_string(),
    }
}

/// ISO 226:2003 equal-loudness contour approximation
/// Returns the SPL offset relative to 1kHz for perceived equal loudness
fn iso226_equal_loudness(frequency: f64, phon: f64) -> f64 {
    // Simplified ISO 226 approximation using frequency-dependent loudness sensitivity
    // Reference: ISO 226:2003 Equal-loudness contours
    let f = frequency.clamp(20.0, 20000.0);

    // Base curve shape (normalized to 0 dB at 1kHz)
    let base = match f {
        f if f < 100.0 => {
            // Low frequency rise
            let t = (100.0 - f) / 80.0;
            12.0 * t * t
        }
        f if f < 500.0 => {
            // Low-mid transition
            let t = (500.0 - f) / 400.0;
            2.0 * t * t
        }
        f if f < 2000.0 => {
            // Flat region around 1kHz
            0.0
        }
        f if f < 4000.0 => {
            // Ear canal resonance dip (more sensitive)
            let t = (f - 2000.0) / 2000.0;
            -3.0 * (std::f64::consts::PI * t).sin()
        }
        f if f < 10000.0 => {
            // High frequency rise
            let t = (f - 4000.0) / 6000.0;
            4.0 * t
        }
        _ => {
            // Very high frequency rolloff
            let t = (f - 10000.0) / 10000.0;
            4.0 + 8.0 * t
        }
    };

    // Apply phon-dependent scaling (louder = flatter curve)
    let phon_factor = 1.0 - (phon - 40.0) / 80.0;
    base * phon_factor.max(0.3)
}

/// X-Curve (ISO 2969) for cinema calibration
/// Standard cinema room equalization target
fn x_curve(frequency: f64) -> f64 {
    let f = frequency.clamp(20.0, 20000.0);

    match f {
        // Flat from 63Hz to 2kHz
        f if f < 63.0 => {
            // Low frequency rolloff below 63Hz
            let octaves = (63.0 / f).log2();
            -3.0 * octaves
        }
        f if f <= 2000.0 => 0.0,
        // -3dB/octave rolloff above 2kHz
        _ => {
            let octaves = (f / 2000.0).log2();
            -3.0 * octaves
        }
    }
}

/// House curve for live sound
/// Gentle low frequency boost, high frequency rolloff
fn house_curve(frequency: f64) -> f64 {
    let f = frequency.clamp(20.0, 20000.0);

    // Bass boost below 200Hz (up to +6dB at 50Hz)
    let bass = if f < 200.0 {
        let octaves = (200.0 / f).log2();
        3.0 * octaves.min(2.0)
    } else {
        0.0
    };

    // High frequency rolloff above 8kHz (gentle -2dB/octave)
    let treble = if f > 8000.0 {
        let octaves = (f / 8000.0).log2();
        -2.0 * octaves
    } else {
        0.0
    };

    bass + treble
}

/// A-weighting frequency response curve
/// Based on IEC 61672-1
fn a_weighting_curve(frequency: f64) -> f64 {
    let f = frequency.clamp(10.0, 20000.0);
    let f2 = f * f;

    // A-weighting formula from IEC 61672-1
    let ra = (12194.0_f64.powi(2) * f2.powi(2))
        / ((f2 + 20.6_f64.powi(2))
            * ((f2 + 107.7_f64.powi(2)) * (f2 + 737.9_f64.powi(2))).sqrt()
            * (f2 + 12194.0_f64.powi(2)));

    // Convert to dB, normalized to 0 dB at 1kHz (+2.0 for normalization)
    20.0 * ra.log10() + 2.0
}

/// C-weighting frequency response curve
/// Based on IEC 61672-1
fn c_weighting_curve(frequency: f64) -> f64 {
    let f = frequency.clamp(10.0, 20000.0);
    let f2 = f * f;

    // C-weighting formula from IEC 61672-1
    let rc = (12194.0_f64.powi(2) * f2) / ((f2 + 20.6_f64.powi(2)) * (f2 + 12194.0_f64.powi(2)));

    // Convert to dB, normalized to 0 dB at 1kHz (+0.06 for normalization)
    20.0 * rc.log10() + 0.06
}

/// Generate standard audio frequencies for curve display (20Hz to 20kHz, log-spaced)
pub fn generate_standard_frequencies(num_points: usize) -> Vec<f64> {
    let log_min = 20.0_f64.log10();
    let log_max = 20000.0_f64.log10();
    let step = (log_max - log_min) / (num_points - 1) as f64;

    (0..num_points)
        .map(|i| 10.0_f64.powf(log_min + i as f64 * step))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_curve() {
        let frequencies = generate_standard_frequencies(100);
        let curve = ReferenceCurve::generate(ReferenceCurveType::Flat, &frequencies);

        for point in &curve.points {
            assert!(
                (point.amplitude_db - 0.0).abs() < 0.001,
                "Flat curve should be 0 dB at all frequencies"
            );
        }
    }

    #[test]
    fn test_x_curve_rolloff() {
        // X-curve should be flat at 1kHz
        let at_1k = x_curve(1000.0);
        assert!(
            at_1k.abs() < 0.1,
            "X-curve should be ~0 dB at 1kHz, got {}",
            at_1k
        );

        // Should roll off above 2kHz
        let at_4k = x_curve(4000.0);
        assert!(
            at_4k < -2.5,
            "X-curve should roll off above 2kHz, got {} at 4kHz",
            at_4k
        );

        // Should roll off below 63Hz
        let at_31 = x_curve(31.5);
        assert!(
            at_31 < -2.5,
            "X-curve should roll off below 63Hz, got {} at 31.5Hz",
            at_31
        );
    }

    #[test]
    fn test_house_curve_bass_boost() {
        // House curve should have bass boost
        let at_50 = house_curve(50.0);
        let at_1k = house_curve(1000.0);

        assert!(
            at_50 > at_1k + 3.0,
            "House curve should have bass boost at 50Hz"
        );
    }

    #[test]
    fn test_house_curve_treble_rolloff() {
        // House curve should have treble rolloff
        let at_1k = house_curve(1000.0);
        let at_16k = house_curve(16000.0);

        assert!(at_16k < at_1k - 1.0, "House curve should roll off at 16kHz");
    }

    #[test]
    fn test_a_weighting_at_1k() {
        let at_1k = a_weighting_curve(1000.0);
        assert!(
            at_1k.abs() < 1.0,
            "A-weighting should be ~0 dB at 1kHz, got {}",
            at_1k
        );
    }

    #[test]
    fn test_a_weighting_low_frequency() {
        // A-weighting has significant attenuation at low frequencies
        let at_100 = a_weighting_curve(100.0);
        let at_1k = a_weighting_curve(1000.0);

        assert!(
            at_100 < at_1k - 15.0,
            "A-weighting should attenuate ~19dB at 100Hz vs 1kHz"
        );
    }

    #[test]
    fn test_c_weighting_at_1k() {
        let at_1k = c_weighting_curve(1000.0);
        assert!(
            at_1k.abs() < 1.0,
            "C-weighting should be ~0 dB at 1kHz, got {}",
            at_1k
        );
    }

    #[test]
    fn test_c_weighting_flatter_than_a() {
        // C-weighting is flatter than A-weighting at low frequencies
        let a_100 = a_weighting_curve(100.0);
        let c_100 = c_weighting_curve(100.0);

        assert!(
            c_100 > a_100 + 10.0,
            "C-weighting should be much less attenuated at 100Hz than A-weighting"
        );
    }

    #[test]
    fn test_iso226_40phon() {
        // At 1kHz, all phon curves should be near 0
        let at_1k = iso226_equal_loudness(1000.0, 40.0);
        assert!(
            at_1k.abs() < 1.0,
            "ISO 226 should be ~0 dB at 1kHz, got {}",
            at_1k
        );
    }

    #[test]
    fn test_iso226_low_frequency_boost() {
        // Low frequencies need more SPL for equal loudness
        let at_50 = iso226_equal_loudness(50.0, 40.0);
        let at_1k = iso226_equal_loudness(1000.0, 40.0);

        // At 50Hz relative to 1kHz, equal loudness requires ~4-5dB more SPL at 40 phon
        assert!(
            at_50 > at_1k + 4.0,
            "ISO 226 should show low frequency boost needed for equal loudness: at_50={}, at_1k={}",
            at_50,
            at_1k
        );
    }

    #[test]
    fn test_iso226_phon_scaling() {
        // Higher phon levels should have flatter curves
        let at_50_40phon = iso226_equal_loudness(50.0, 40.0);
        let at_50_80phon = iso226_equal_loudness(50.0, 80.0);

        assert!(
            at_50_40phon > at_50_80phon,
            "Higher phon curves should be flatter at low frequencies"
        );
    }

    #[test]
    fn test_generate_standard_frequencies() {
        let freqs = generate_standard_frequencies(100);

        assert_eq!(freqs.len(), 100);
        assert!((freqs[0] - 20.0).abs() < 0.01, "Should start at 20Hz");
        assert!((freqs[99] - 20000.0).abs() < 1.0, "Should end at 20kHz");

        // Should be monotonically increasing
        for i in 1..freqs.len() {
            assert!(
                freqs[i] > freqs[i - 1],
                "Frequencies should be monotonically increasing"
            );
        }
    }

    #[test]
    fn test_curve_interpolation() {
        let frequencies = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let curve = ReferenceCurve::generate(ReferenceCurveType::XCurve, &frequencies);

        // Test interpolation at a frequency between points
        let at_500 = curve.interpolate(500.0);
        let at_1k = curve.interpolate(1000.0);

        // Both should be near 0 for X-curve (within flat region)
        assert!(
            at_500.abs() < 0.5,
            "Interpolated value at 500Hz should be ~0"
        );
        assert!(
            (at_1k - curve.points[2].amplitude_db).abs() < 0.001,
            "Interpolation at exact point should match"
        );
    }

    #[test]
    fn test_all_curve_types() {
        let all = ReferenceCurveType::all();
        assert_eq!(all.len(), 8, "Should have 8 curve types");

        // Generate each curve type
        let freqs = generate_standard_frequencies(50);
        for curve_type in all {
            let curve = ReferenceCurve::generate(curve_type, &freqs);
            assert_eq!(curve.points.len(), 50);
            assert!(!curve.name.is_empty());
            assert!(!curve.description.is_empty());
        }
    }

    #[test]
    fn test_curve_display_names() {
        assert_eq!(ReferenceCurveType::Flat.display_name(), "Flat");
        assert_eq!(
            ReferenceCurveType::XCurve.display_name(),
            "X-Curve (Cinema)"
        );
        assert_eq!(
            ReferenceCurveType::Iso226_60Phon.display_name(),
            "ISO 226 (60 phon)"
        );
    }
}

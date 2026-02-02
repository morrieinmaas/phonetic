//! SPL Calibration and Microphone Sensitivity
//!
//! Provides calibration functionality for accurate SPL measurements.
//! Supports importing calibration files (.cal, .txt) for measurement microphones.

// Allow unused methods - these are library APIs for future frontend integration
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Microphone calibration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophoneCalibration {
    /// Calibration name/description
    pub name: String,
    /// Microphone sensitivity in dB re 1V/Pa (typically -30 to -50 dB)
    pub sensitivity_db: f32,
    /// Reference SPL for calibration (typically 94 dB or 114 dB)
    pub reference_spl_db: f32,
    /// Frequency-dependent correction curve (frequency Hz -> correction dB)
    /// If empty, flat response is assumed
    pub frequency_corrections: BTreeMap<u32, f32>,
}

impl Default for MicrophoneCalibration {
    fn default() -> Self {
        Self {
            name: "Uncalibrated".to_string(),
            sensitivity_db: -40.0,  // Typical measurement mic sensitivity
            reference_spl_db: 94.0, // 1 Pa = 94 dB SPL
            frequency_corrections: BTreeMap::new(),
        }
    }
}

impl MicrophoneCalibration {
    /// Create a new calibration with given sensitivity
    pub fn new(name: &str, sensitivity_db: f32, reference_spl_db: f32) -> Self {
        Self {
            name: name.to_string(),
            sensitivity_db,
            reference_spl_db,
            frequency_corrections: BTreeMap::new(),
        }
    }

    /// Add a frequency correction point
    pub fn add_correction(&mut self, frequency_hz: u32, correction_db: f32) {
        self.frequency_corrections
            .insert(frequency_hz, correction_db);
    }

    /// Get interpolated correction at a given frequency
    pub fn get_correction(&self, frequency_hz: f32) -> f32 {
        if self.frequency_corrections.is_empty() {
            return 0.0;
        }

        let freq_u32 = frequency_hz as u32;

        // Find surrounding points
        let lower = self.frequency_corrections.range(..=freq_u32).next_back();
        let upper = self.frequency_corrections.range(freq_u32..).next();

        match (lower, upper) {
            (Some((&f1, &c1)), Some((&f2, &c2))) if f1 != f2 => {
                // Linear interpolation
                let t = (frequency_hz - f1 as f32) / (f2 as f32 - f1 as f32);
                c1 + t * (c2 - c1)
            }
            (Some((_, &c)), None) | (None, Some((_, &c))) | (Some((_, &c)), Some(_)) => c,
            (None, None) => 0.0,
        }
    }

    /// Parse a .cal calibration file (REW/UMIK format)
    ///
    /// Format: each line is "frequency correction" or "frequency,correction"
    /// Lines starting with ; or # are comments
    pub fn from_cal_file(content: &str, name: &str) -> Result<Self, String> {
        let mut cal = Self::new(name, -40.0, 94.0);

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                // Check for sensitivity in comments
                if line.contains("Sens Factor") || line.contains("Sensitivity") {
                    if let Some(sens) = extract_sensitivity(line) {
                        cal.sensitivity_db = sens;
                    }
                }
                continue;
            }

            // Parse frequency and correction
            let parts: Vec<&str> = line
                .split([' ', '\t', ','])
                .filter(|s| !s.is_empty())
                .collect();

            if parts.len() >= 2 {
                if let (Ok(freq), Ok(correction)) =
                    (parts[0].parse::<f32>(), parts[1].parse::<f32>())
                {
                    cal.add_correction(freq as u32, correction);
                }
            }
        }

        if cal.frequency_corrections.is_empty() {
            return Err("No valid calibration data found".to_string());
        }

        Ok(cal)
    }
}

/// Extract sensitivity from a comment line
fn extract_sensitivity(line: &str) -> Option<f32> {
    // Look for patterns like "Sens Factor =-40.0dB" or "Sensitivity: -38.5 dB"
    let line = line.to_lowercase();

    for pattern in &["sens factor", "sensitivity"] {
        if let Some(idx) = line.find(pattern) {
            let after = &line[idx + pattern.len()..];
            // Find numeric value
            let value_str: String = after
                .chars()
                .skip_while(|c| !c.is_ascii_digit() && *c != '-' && *c != '.')
                .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.')
                .collect();

            if let Ok(val) = value_str.parse::<f32>() {
                return Some(val);
            }
        }
    }

    None
}

/// SPL Calculator for converting dBFS to dB SPL
#[derive(Debug, Clone)]
pub struct SplCalculator {
    calibration: MicrophoneCalibration,
    /// Preamp/interface gain in dB
    preamp_gain_db: f32,
    /// A-weighting enabled
    a_weighting: bool,
}

impl SplCalculator {
    /// Create a new SPL calculator with given calibration
    pub fn new(calibration: MicrophoneCalibration) -> Self {
        Self {
            calibration,
            preamp_gain_db: 0.0,
            a_weighting: false,
        }
    }

    /// Set preamp/interface gain
    pub fn set_preamp_gain(&mut self, gain_db: f32) {
        self.preamp_gain_db = gain_db;
    }

    /// Enable/disable A-weighting
    pub fn set_a_weighting(&mut self, enabled: bool) {
        self.a_weighting = enabled;
    }

    /// Convert dBFS to dB SPL
    ///
    /// SPL = dBFS - sensitivity_db - preamp_gain + reference_spl
    pub fn dbfs_to_spl(&self, dbfs: f32) -> f32 {
        dbfs - self.calibration.sensitivity_db - self.preamp_gain_db
            + self.calibration.reference_spl_db
    }

    /// Convert dBFS to dB SPL with frequency correction
    pub fn dbfs_to_spl_at_frequency(&self, dbfs: f32, frequency_hz: f32) -> f32 {
        let base_spl = self.dbfs_to_spl(dbfs);
        let correction = self.calibration.get_correction(frequency_hz);
        let a_weight = if self.a_weighting {
            a_weighting(frequency_hz)
        } else {
            0.0
        };
        base_spl + correction + a_weight
    }

    /// Apply calibration to a spectrum (dBFS values)
    pub fn calibrate_spectrum(&self, spectrum_dbfs: &[f32], frequencies: &[f32]) -> Vec<f32> {
        spectrum_dbfs
            .iter()
            .zip(frequencies.iter())
            .map(|(&dbfs, &freq)| self.dbfs_to_spl_at_frequency(dbfs, freq))
            .collect()
    }

    /// Get current calibration
    pub fn calibration(&self) -> &MicrophoneCalibration {
        &self.calibration
    }

    /// Update calibration
    pub fn set_calibration(&mut self, calibration: MicrophoneCalibration) {
        self.calibration = calibration;
    }

    /// Perform calibration with reference tone
    ///
    /// Given a measured dBFS level and known SPL, calculate sensitivity
    pub fn calibrate_with_reference(&mut self, measured_dbfs: f32, reference_spl_db: f32) {
        // sensitivity = measured_dbfs - reference_spl + reference_offset
        self.calibration.sensitivity_db =
            measured_dbfs - reference_spl_db + self.calibration.reference_spl_db;
    }
}

impl Default for SplCalculator {
    fn default() -> Self {
        Self::new(MicrophoneCalibration::default())
    }
}

/// A-weighting filter magnitude at given frequency
///
/// Based on IEC 61672-1:2013
pub fn a_weighting(freq: f32) -> f32 {
    if freq <= 0.0 {
        return -f32::INFINITY;
    }

    let f2 = freq * freq;
    let f4 = f2 * f2;

    // A-weighting constants
    const C1: f32 = 12194.217_f32;
    const C2: f32 = 20.598_997_f32;
    const C3: f32 = 107.652_65_f32;
    const C4: f32 = 737.862_24_f32;

    let c1_sq = C1 * C1;
    let c2_sq = C2 * C2;
    let c3_sq = C3 * C3;
    let c4_sq = C4 * C4;

    let num = c1_sq * f4;
    let den = (f2 + c2_sq) * (f2 + c1_sq) * ((f2 + c3_sq) * (f2 + c4_sq)).sqrt();

    let ra = num / den;

    // Convert to dB and apply reference offset
    // A(1000) = 0 dB
    20.0 * ra.log10() + 2.0
}

/// C-weighting filter magnitude at given frequency
pub fn c_weighting(freq: f32) -> f32 {
    if freq <= 0.0 {
        return -f32::INFINITY;
    }

    let f2 = freq * freq;

    const C1: f32 = 12194.217_f32;
    const C2: f32 = 20.598_997_f32;

    let c1_sq = C1 * C1;
    let c2_sq = C2 * C2;

    let num = c1_sq * f2;
    let den = (f2 + c2_sq) * (f2 + c1_sq);

    let rc = num / den;

    20.0 * rc.log10() + 0.062
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_calibration() {
        let cal = MicrophoneCalibration::default();
        assert_eq!(cal.sensitivity_db, -40.0);
        assert_eq!(cal.reference_spl_db, 94.0);
    }

    #[test]
    fn test_dbfs_to_spl() {
        let cal = MicrophoneCalibration::new("Test", -40.0, 94.0);
        let calc = SplCalculator::new(cal);

        // 0 dBFS with -40 dB sensitivity should give 94 + 40 = 134 dB SPL
        assert!((calc.dbfs_to_spl(0.0) - 134.0).abs() < 0.1);

        // -40 dBFS should give 94 dB SPL
        assert!((calc.dbfs_to_spl(-40.0) - 94.0).abs() < 0.1);
    }

    #[test]
    fn test_frequency_correction_interpolation() {
        let mut cal = MicrophoneCalibration::default();
        cal.add_correction(100, -2.0);
        cal.add_correction(1000, 0.0);
        cal.add_correction(10000, 3.0);

        // Exact frequency
        assert!((cal.get_correction(1000.0) - 0.0).abs() < 0.01);

        // Interpolated
        let correction_500 = cal.get_correction(500.0);
        assert!(correction_500 > -2.0 && correction_500 < 0.0);
    }

    #[test]
    fn test_a_weighting_1khz() {
        // A-weighting at 1kHz should be 0 dB
        let weight = a_weighting(1000.0);
        assert!(
            weight.abs() < 0.5,
            "A-weighting at 1kHz should be ~0 dB, got {}",
            weight
        );
    }

    #[test]
    fn test_a_weighting_low_frequency() {
        // A-weighting at 100Hz should be around -19 dB
        let weight = a_weighting(100.0);
        assert!(
            (weight - (-19.1)).abs() < 1.0,
            "A-weighting at 100Hz should be ~-19 dB, got {}",
            weight
        );
    }

    #[test]
    fn test_parse_cal_file() {
        let cal_content = r#"
; Calibration file for test mic
; Sens Factor =-38.5dB
20 -3.5
100 -1.2
1000 0.0
10000 2.1
20000 -5.0
"#;

        let cal = MicrophoneCalibration::from_cal_file(cal_content, "Test Mic").unwrap();

        assert!((cal.sensitivity_db - (-38.5)).abs() < 0.1);
        assert_eq!(cal.frequency_corrections.len(), 5);
        assert!((cal.get_correction(1000.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_calibrate_with_reference() {
        let cal = MicrophoneCalibration::new("Test", -40.0, 94.0);
        let mut calc = SplCalculator::new(cal);

        // If we measure -20 dBFS at 94 dB SPL
        calc.calibrate_with_reference(-20.0, 94.0);

        // Sensitivity should now be -20 dB
        assert!((calc.calibration.sensitivity_db - (-20.0)).abs() < 0.1);
    }
}

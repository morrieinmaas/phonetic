//! Delay Finder using cross-correlation
//!
//! Finds the propagation delay between a reference signal (generator output)
//! and a measured signal (microphone input) using normalized cross-correlation.
//!
//! This is critical for accurate transfer function measurements in live sound
//! applications where there's propagation delay through PA systems and rooms.

// Allow unused methods - these are library APIs for future frontend integration
#![allow(dead_code)]

use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

/// Result of delay finding operation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DelayResult {
    /// Detected delay in samples
    pub delay_samples: i32,
    /// Detected delay in milliseconds
    pub delay_ms: f32,
    /// Correlation coefficient at peak (0.0 to 1.0)
    /// Higher values indicate more confidence in the result
    pub correlation: f32,
    /// Whether the delay was reliably detected
    /// (correlation above threshold)
    pub is_valid: bool,
}

/// Cross-correlation based delay finder
pub struct DelayFinder {
    fft_size: usize,
    sample_rate: f32,
    fft_planner: RealFftPlanner<f32>,
    forward_fft: Arc<dyn RealToComplex<f32>>,
    inverse_fft: Arc<dyn realfft::ComplexToReal<f32>>,
    /// Maximum delay to search (in samples)
    max_delay_samples: usize,
    /// Minimum correlation threshold for valid detection
    correlation_threshold: f32,
    /// Scratch buffers
    scratch_ref: Vec<f32>,
    scratch_meas: Vec<f32>,
    spectrum_ref: Vec<realfft::num_complex::Complex<f32>>,
    spectrum_meas: Vec<realfft::num_complex::Complex<f32>>,
    correlation_result: Vec<f32>,
}

impl DelayFinder {
    /// Create a new delay finder
    ///
    /// # Arguments
    /// * `fft_size` - FFT size for correlation (should be at least 2x max expected delay)
    /// * `sample_rate` - Sample rate in Hz
    /// * `max_delay_ms` - Maximum delay to search in milliseconds
    pub fn new(fft_size: usize, sample_rate: f32, max_delay_ms: f32) -> Self {
        let mut fft_planner = RealFftPlanner::new();
        let forward_fft = fft_planner.plan_fft_forward(fft_size);
        let inverse_fft = fft_planner.plan_fft_inverse(fft_size);

        let max_delay_samples = (max_delay_ms * sample_rate / 1000.0) as usize;
        let spectrum_size = fft_size / 2 + 1;

        Self {
            fft_size,
            sample_rate,
            fft_planner,
            forward_fft,
            inverse_fft,
            max_delay_samples: max_delay_samples.min(fft_size / 2),
            correlation_threshold: 0.3, // 30% correlation minimum
            scratch_ref: vec![0.0; fft_size],
            scratch_meas: vec![0.0; fft_size],
            spectrum_ref: vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_size],
            spectrum_meas: vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_size],
            correlation_result: vec![0.0; fft_size],
        }
    }

    /// Set the correlation threshold for valid detection
    pub fn set_correlation_threshold(&mut self, threshold: f32) {
        self.correlation_threshold = threshold.clamp(0.1, 0.9);
    }

    /// Set maximum delay to search (in milliseconds)
    pub fn set_max_delay_ms(&mut self, max_delay_ms: f32) {
        self.max_delay_samples =
            ((max_delay_ms * self.sample_rate / 1000.0) as usize).min(self.fft_size / 2);
    }

    /// Find delay between reference and measured signals
    ///
    /// Uses normalized cross-correlation computed via FFT:
    /// R_xy(τ) = IFFT(conj(FFT(x)) * FFT(y)) / (||x|| * ||y||)
    ///
    /// The peak of the cross-correlation indicates the delay.
    ///
    /// # Arguments
    /// * `reference` - Reference signal (generator output)
    /// * `measured` - Measured signal (microphone input)
    ///
    /// # Returns
    /// DelayResult with detected delay and confidence
    pub fn find_delay(&mut self, reference: &[f32], measured: &[f32]) -> DelayResult {
        let len = reference.len().min(measured.len()).min(self.fft_size);

        // Copy and zero-pad inputs
        self.scratch_ref.fill(0.0);
        self.scratch_meas.fill(0.0);
        self.scratch_ref[..len].copy_from_slice(&reference[..len]);
        self.scratch_meas[..len].copy_from_slice(&measured[..len]);

        // Compute RMS for normalization
        let rms_ref = compute_rms(&self.scratch_ref[..len]);
        let rms_meas = compute_rms(&self.scratch_meas[..len]);

        // Check for silence
        if rms_ref < 1e-6 || rms_meas < 1e-6 {
            return DelayResult {
                delay_samples: 0,
                delay_ms: 0.0,
                correlation: 0.0,
                is_valid: false,
            };
        }

        // Forward FFT of both signals
        self.forward_fft
            .process(&mut self.scratch_ref, &mut self.spectrum_ref)
            .expect("FFT failed");
        self.forward_fft
            .process(&mut self.scratch_meas, &mut self.spectrum_meas)
            .expect("FFT failed");

        // Cross-correlation in frequency domain: conj(X) * Y
        for i in 0..self.spectrum_ref.len() {
            let conj_ref = self.spectrum_ref[i].conj();
            self.spectrum_ref[i] = conj_ref * self.spectrum_meas[i];
        }

        // Inverse FFT to get correlation
        self.inverse_fft
            .process(&mut self.spectrum_ref, &mut self.correlation_result)
            .expect("IFFT failed");

        // Normalize by FFT size and signal energies
        let norm_factor = 1.0 / (self.fft_size as f32 * rms_ref * rms_meas * len as f32);
        for sample in &mut self.correlation_result {
            *sample *= norm_factor;
        }

        // Find peak in the valid delay range
        // Positive delays: indices 0 to max_delay_samples
        // Negative delays: indices fft_size - max_delay_samples to fft_size - 1
        let (peak_idx, peak_val) = self.find_peak();

        // Convert index to signed delay
        let delay_samples = if peak_idx <= self.fft_size / 2 {
            peak_idx as i32
        } else {
            peak_idx as i32 - self.fft_size as i32
        };

        let delay_ms = delay_samples as f32 * 1000.0 / self.sample_rate;
        let is_valid = peak_val >= self.correlation_threshold;

        DelayResult {
            delay_samples,
            delay_ms,
            correlation: peak_val,
            is_valid,
        }
    }

    /// Find peak in correlation result within allowed delay range
    fn find_peak(&self) -> (usize, f32) {
        let mut best_idx = 0;
        let mut best_val = f32::NEG_INFINITY;

        // Search positive delays (0 to max_delay_samples)
        for i in 0..=self.max_delay_samples {
            if self.correlation_result[i] > best_val {
                best_val = self.correlation_result[i];
                best_idx = i;
            }
        }

        // Search negative delays (wrap-around at end of array)
        let neg_start = self.fft_size.saturating_sub(self.max_delay_samples);
        for i in neg_start..self.fft_size {
            if self.correlation_result[i] > best_val {
                best_val = self.correlation_result[i];
                best_idx = i;
            }
        }

        (best_idx, best_val.max(0.0))
    }

    /// Get current sample rate
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Update sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }
}

/// Compute RMS of a signal
fn compute_rms(signal: &[f32]) -> f32 {
    if signal.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = signal.iter().map(|&x| x * x).sum();
    (sum_sq / signal.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_delay() {
        let mut finder = DelayFinder::new(4096, 48000.0, 100.0);

        // Generate test signal (sine wave)
        let freq = 1000.0;
        let signal: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin())
            .collect();

        let result = finder.find_delay(&signal, &signal);

        assert!(result.is_valid);
        assert_eq!(result.delay_samples, 0);
        assert!(
            result.correlation > 0.9,
            "Expected high correlation for identical signals"
        );
    }

    #[test]
    fn test_positive_delay() {
        let mut finder = DelayFinder::new(4096, 48000.0, 100.0);

        // Generate reference signal - use broadband chirp to avoid periodicity issues
        // Frequency sweep from 500Hz to 5000Hz
        let reference: Vec<f32> = (0..2048)
            .map(|i| {
                let t = i as f32 / 48000.0;
                let freq = 500.0 + (5000.0 - 500.0) * t * 20.0; // Quick chirp
                (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect();

        // Delay by 100 samples (~2.08ms at 48kHz) - not a multiple of any signal period
        let delay = 100;
        let mut measured = vec![0.0; 2048];
        measured[delay..].copy_from_slice(&reference[..2048 - delay]);

        let result = finder.find_delay(&reference, &measured);

        assert!(result.is_valid);
        assert!(
            (result.delay_samples - delay as i32).abs() <= 5,
            "Expected delay ~{}, got {}",
            delay,
            result.delay_samples
        );
    }

    #[test]
    fn test_silence_detection() {
        let mut finder = DelayFinder::new(4096, 48000.0, 100.0);

        let silence = vec![0.0; 2048];
        let signal: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect();

        let result = finder.find_delay(&silence, &signal);
        assert!(!result.is_valid);

        let result2 = finder.find_delay(&signal, &silence);
        assert!(!result2.is_valid);
    }

    #[test]
    fn test_uncorrelated_signals() {
        let mut finder = DelayFinder::new(4096, 48000.0, 100.0);

        // Two different frequency signals
        let signal1: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect();
        let signal2: Vec<f32> = (0..2048)
            .map(|i| (2.0 * std::f32::consts::PI * 3500.0 * i as f32 / 48000.0).sin())
            .collect();

        let result = finder.find_delay(&signal1, &signal2);

        // Should have low correlation
        assert!(
            result.correlation < 0.5,
            "Expected low correlation for uncorrelated signals, got {}",
            result.correlation
        );
    }

    #[test]
    fn test_delay_ms_calculation() {
        let mut finder = DelayFinder::new(4096, 48000.0, 100.0);

        // Use broadband signal to avoid periodicity issues
        // 96 samples at 48kHz = 2ms
        let reference: Vec<f32> = (0..2048)
            .map(|i| {
                let t = i as f32 / 48000.0;
                let freq = 500.0 + (5000.0 - 500.0) * t * 20.0;
                (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect();

        let delay_samples = 96; // 2ms at 48kHz
        let mut measured = vec![0.0; 2048];
        measured[delay_samples..].copy_from_slice(&reference[..2048 - delay_samples]);

        let result = finder.find_delay(&reference, &measured);

        assert!(result.is_valid);
        // Allow some tolerance
        assert!(
            (result.delay_ms - 2.0).abs() < 0.2,
            "Expected ~2.0ms delay, got {}ms",
            result.delay_ms
        );
    }
}

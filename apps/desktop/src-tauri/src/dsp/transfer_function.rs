use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use std::sync::Arc;

use super::windows::apply_hann_window;

/// Transfer function measurement data returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferFunctionData {
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
    pub coherence: Vec<f32>,
    pub group_delay_ms: Vec<f32>,
    pub frequencies: Vec<f32>,
}

/// Processor for computing transfer function H(f) = Y(f)/X(f)
/// where X is the reference (output) and Y is the measurement (input)
pub struct TransferFunctionProcessor {
    fft_size: usize,
    sample_rate: f32,
    #[allow(dead_code)]
    planner: RealFftPlanner<f32>,
    fft: Arc<dyn RealToComplex<f32>>,

    // Buffers for FFT processing
    input_buffer: Vec<f32>,
    ref_buffer: Vec<f32>,
    scratch: Vec<Complex<f32>>,
    input_spectrum: Vec<Complex<f32>>,
    ref_spectrum: Vec<Complex<f32>>,

    // Accumulator buffers for input samples
    input_accumulator: Vec<f32>,
    ref_accumulator: Vec<f32>,
    hop_size: usize,

    // Averaging state for noise reduction (Welch's method)
    // Sxx = auto-spectrum of reference
    // Syy = auto-spectrum of input
    // Sxy = cross-spectrum
    sxx_sum: Vec<f32>,
    syy_sum: Vec<f32>,
    sxy_sum: Vec<Complex<f32>>,
    average_count: usize,

    // Output data
    magnitude_db: Vec<f32>,
    phase_deg: Vec<f32>,
    coherence: Vec<f32>,
    group_delay_ms: Vec<f32>,
    frequencies: Vec<f32>,
}

impl TransferFunctionProcessor {
    pub fn new(fft_size: usize, sample_rate: f32) -> Self {
        let mut planner = RealFftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        let scratch_len = fft.get_scratch_len();
        let spectrum_len = fft_size / 2 + 1;
        let hop_size = fft_size / 2; // 50% overlap

        // Pre-compute frequency bins
        let frequencies: Vec<f32> = (0..spectrum_len)
            .map(|i| i as f32 * sample_rate / fft_size as f32)
            .collect();

        Self {
            fft_size,
            sample_rate,
            planner,
            fft,
            input_buffer: vec![0.0; fft_size],
            ref_buffer: vec![0.0; fft_size],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            input_spectrum: vec![Complex::new(0.0, 0.0); spectrum_len],
            ref_spectrum: vec![Complex::new(0.0, 0.0); spectrum_len],
            input_accumulator: Vec::with_capacity(fft_size * 2),
            ref_accumulator: Vec::with_capacity(fft_size * 2),
            hop_size,
            sxx_sum: vec![0.0; spectrum_len],
            syy_sum: vec![0.0; spectrum_len],
            sxy_sum: vec![Complex::new(0.0, 0.0); spectrum_len],
            average_count: 0,
            magnitude_db: vec![-120.0; spectrum_len],
            phase_deg: vec![0.0; spectrum_len],
            coherence: vec![0.0; spectrum_len],
            group_delay_ms: vec![0.0; spectrum_len],
            frequencies,
        }
    }

    /// Process input (measurement) and reference (output) sample buffers
    pub fn process(&mut self, input_samples: &[f32], ref_samples: &[f32]) {
        // Accumulate samples
        self.input_accumulator.extend_from_slice(input_samples);
        self.ref_accumulator.extend_from_slice(ref_samples);

        // Process when we have enough samples
        while self.input_accumulator.len() >= self.fft_size
            && self.ref_accumulator.len() >= self.fft_size
        {
            // Copy samples to buffers
            self.input_buffer
                .copy_from_slice(&self.input_accumulator[..self.fft_size]);
            self.ref_buffer
                .copy_from_slice(&self.ref_accumulator[..self.fft_size]);

            // Apply window function
            apply_hann_window(&mut self.input_buffer);
            apply_hann_window(&mut self.ref_buffer);

            // Perform FFT on both signals
            if self
                .fft
                .process_with_scratch(
                    &mut self.input_buffer,
                    &mut self.input_spectrum,
                    &mut self.scratch,
                )
                .is_err()
            {
                return;
            }

            // Need to re-copy ref buffer since FFT modifies input in place
            self.ref_buffer
                .copy_from_slice(&self.ref_accumulator[..self.fft_size]);
            apply_hann_window(&mut self.ref_buffer);

            if self
                .fft
                .process_with_scratch(
                    &mut self.ref_buffer,
                    &mut self.ref_spectrum,
                    &mut self.scratch,
                )
                .is_err()
            {
                return;
            }

            // Update averaging sums
            // Sxx = |X|^2 (auto-spectrum of reference)
            // Syy = |Y|^2 (auto-spectrum of input)
            // Sxy = conj(X) * Y (cross-spectrum)
            for i in 0..self.sxx_sum.len() {
                let x = self.ref_spectrum[i];
                let y = self.input_spectrum[i];

                self.sxx_sum[i] += x.norm_sqr();
                self.syy_sum[i] += y.norm_sqr();
                self.sxy_sum[i] += x.conj() * y;
            }
            self.average_count += 1;

            // Compute transfer function from averaged spectra
            self.compute_transfer_function();

            // Remove processed samples (hop by hop_size for overlap)
            self.input_accumulator.drain(..self.hop_size);
            self.ref_accumulator.drain(..self.hop_size);
        }
    }

    /// Compute H(f), phase, coherence, and group delay from accumulated spectra
    fn compute_transfer_function(&mut self) {
        if self.average_count == 0 {
            return;
        }

        let n = self.average_count as f32;
        let mut prev_unwrapped_phase = 0.0f32;

        for i in 0..self.sxx_sum.len() {
            let sxx = self.sxx_sum[i] / n;
            let syy = self.syy_sum[i] / n;
            let sxy = self.sxy_sum[i] / n;

            // Transfer function H = Sxy / Sxx
            let h = if sxx > 1e-10 {
                sxy / sxx
            } else {
                Complex::new(0.0, 0.0)
            };

            // Magnitude in dB
            let mag = h.norm();
            self.magnitude_db[i] = if mag > 1e-10 {
                20.0 * mag.log10()
            } else {
                -120.0
            };

            // Phase in degrees (with unwrapping)
            let phase_rad = h.arg();
            let mut unwrapped_phase = phase_rad;

            if i > 0 {
                // Unwrap phase to be continuous
                let diff = unwrapped_phase - prev_unwrapped_phase;
                if diff > PI {
                    unwrapped_phase -= 2.0 * PI * ((diff + PI) / (2.0 * PI)).floor();
                } else if diff < -PI {
                    unwrapped_phase += 2.0 * PI * ((-diff + PI) / (2.0 * PI)).floor();
                }
            }
            prev_unwrapped_phase = unwrapped_phase;
            self.phase_deg[i] = unwrapped_phase * 180.0 / PI;

            // Coherence γ² = |Sxy|² / (Sxx * Syy)
            let coherence_denom = sxx * syy;
            self.coherence[i] = if coherence_denom > 1e-10 {
                (sxy.norm_sqr() / coherence_denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }

        // Compute group delay from phase derivative
        // τg = -dφ/dω = -dφ/df * (1/2π)
        self.compute_group_delay();
    }

    /// Compute group delay from phase using numerical differentiation
    fn compute_group_delay(&mut self) {
        let freq_resolution = self.sample_rate / self.fft_size as f32;

        for i in 0..self.group_delay_ms.len() {
            if i == 0 || i == self.group_delay_ms.len() - 1 {
                // Edge cases: use one-sided difference
                let phase_diff = if i == 0 {
                    self.phase_deg[1] - self.phase_deg[0]
                } else {
                    self.phase_deg[i] - self.phase_deg[i - 1]
                };
                // Convert phase difference (degrees) to group delay (ms)
                // τg = -dφ/dω = -(dφ_deg / df) / 360
                self.group_delay_ms[i] = -phase_diff / (360.0 * freq_resolution) * 1000.0;
            } else {
                // Central difference
                let phase_diff = self.phase_deg[i + 1] - self.phase_deg[i - 1];
                self.group_delay_ms[i] = -phase_diff / (2.0 * 360.0 * freq_resolution) * 1000.0;
            }
        }
    }

    /// Get computed transfer function data
    pub fn get_data(&self) -> TransferFunctionData {
        TransferFunctionData {
            magnitude_db: self.magnitude_db.clone(),
            phase_deg: self.phase_deg.clone(),
            coherence: self.coherence.clone(),
            group_delay_ms: self.group_delay_ms.clone(),
            frequencies: self.frequencies.clone(),
        }
    }

    /// Get magnitude response in dB
    #[allow(dead_code)]
    pub fn get_magnitude_db(&self) -> &[f32] {
        &self.magnitude_db
    }

    /// Get phase response in degrees
    #[allow(dead_code)]
    pub fn get_phase_deg(&self) -> &[f32] {
        &self.phase_deg
    }

    /// Get coherence (0-1)
    #[allow(dead_code)]
    pub fn get_coherence(&self) -> &[f32] {
        &self.coherence
    }

    /// Get group delay in milliseconds
    #[allow(dead_code)]
    pub fn get_group_delay_ms(&self) -> &[f32] {
        &self.group_delay_ms
    }

    /// Reset averaging and clear accumulated data
    pub fn reset_averaging(&mut self) {
        self.sxx_sum.iter_mut().for_each(|v| *v = 0.0);
        self.syy_sum.iter_mut().for_each(|v| *v = 0.0);
        self.sxy_sum
            .iter_mut()
            .for_each(|v| *v = Complex::new(0.0, 0.0));
        self.average_count = 0;
        self.magnitude_db.iter_mut().for_each(|v| *v = -120.0);
        self.phase_deg.iter_mut().for_each(|v| *v = 0.0);
        self.coherence.iter_mut().for_each(|v| *v = 0.0);
        self.group_delay_ms.iter_mut().for_each(|v| *v = 0.0);
        self.input_accumulator.clear();
        self.ref_accumulator.clear();
    }

    /// Change FFT size (resets processor)
    pub fn set_fft_size(&mut self, fft_size: usize) {
        if fft_size != self.fft_size && fft_size.is_power_of_two() {
            *self = Self::new(fft_size, self.sample_rate);
        }
    }

    /// Update sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() > 0.1 {
            self.sample_rate = sample_rate;
            // Recalculate frequencies
            let spectrum_len = self.fft_size / 2 + 1;
            self.frequencies = (0..spectrum_len)
                .map(|i| i as f32 * sample_rate / self.fft_size as f32)
                .collect();
        }
    }

    /// Get current FFT size
    #[allow(dead_code)]
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Get number of frequency bins
    #[allow(dead_code)]
    pub fn bin_count(&self) -> usize {
        self.fft_size / 2 + 1
    }

    /// Get number of averages accumulated
    #[allow(dead_code)]
    pub fn average_count(&self) -> usize {
        self.average_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48000.0;
    const FFT_SIZE: usize = 2048;

    fn create_sine_wave(
        freq: f32,
        sample_rate: f32,
        num_samples: usize,
        amplitude: f32,
    ) -> Vec<f32> {
        (0..num_samples)
            .map(|i| amplitude * (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn test_processor_creation() {
        let processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);
        assert_eq!(processor.fft_size(), FFT_SIZE);
        assert_eq!(processor.bin_count(), FFT_SIZE / 2 + 1);
        assert_eq!(processor.average_count(), 0);
    }

    #[test]
    fn test_unity_transfer_function() {
        // When input == reference, transfer function should be ~0dB, ~0°
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let signal = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);

        // Process same signal as both input and reference
        processor.process(&signal, &signal);

        let data = processor.get_data();

        // Find the bin for 1kHz
        let bin_1k = (1000.0 / SAMPLE_RATE * FFT_SIZE as f32).round() as usize;

        // Magnitude should be near 0dB for unity gain
        assert!(
            data.magnitude_db[bin_1k].abs() < 3.0,
            "Expected ~0dB, got {} dB",
            data.magnitude_db[bin_1k]
        );

        // Coherence should be high (close to 1) for identical signals
        assert!(
            data.coherence[bin_1k] > 0.9,
            "Expected coherence > 0.9, got {}",
            data.coherence[bin_1k]
        );
    }

    #[test]
    fn test_attenuated_signal() {
        // Input at half amplitude should give -6dB
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let reference = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 1.0);
        let input = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);

        processor.process(&input, &reference);

        let data = processor.get_data();
        let bin_1k = (1000.0 / SAMPLE_RATE * FFT_SIZE as f32).round() as usize;

        // -6dB = 20 * log10(0.5)
        let expected_db = -6.02;
        assert!(
            (data.magnitude_db[bin_1k] - expected_db).abs() < 2.0,
            "Expected ~{} dB, got {} dB",
            expected_db,
            data.magnitude_db[bin_1k]
        );
    }

    #[test]
    fn test_coherence_uncorrelated() {
        // Uncorrelated signals should have low coherence
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let reference = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);
        let input = create_sine_wave(2000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5); // Different frequency

        processor.process(&input, &reference);

        let data = processor.get_data();
        let bin_1k = (1000.0 / SAMPLE_RATE * FFT_SIZE as f32).round() as usize;

        // Coherence at 1kHz should be low since input is at 2kHz
        assert!(
            data.coherence[bin_1k] < 0.5,
            "Expected low coherence, got {}",
            data.coherence[bin_1k]
        );
    }

    #[test]
    fn test_delayed_signal_phase() {
        // A delayed signal should show linear phase vs frequency
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let delay_samples = 10;
        let reference = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);
        let mut input = vec![0.0; delay_samples];
        input.extend(&reference[..reference.len() - delay_samples]);

        processor.process(&input, &reference);

        // Phase should be non-zero due to delay
        let data = processor.get_data();
        let bin_1k = (1000.0 / SAMPLE_RATE * FFT_SIZE as f32).round() as usize;

        // Just verify we get some phase value (exact value depends on delay)
        // The test is that it doesn't crash and produces valid data
        assert!(data.phase_deg[bin_1k].is_finite());
    }

    #[test]
    fn test_reset_averaging() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let signal = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);
        processor.process(&signal, &signal);

        assert!(processor.average_count() > 0);

        processor.reset_averaging();

        assert_eq!(processor.average_count(), 0);
        let data = processor.get_data();
        assert!(data.magnitude_db.iter().all(|&v| v == -120.0));
    }

    #[test]
    fn test_fft_size_change() {
        let mut processor = TransferFunctionProcessor::new(1024, SAMPLE_RATE);
        assert_eq!(processor.fft_size(), 1024);

        processor.set_fft_size(2048);
        assert_eq!(processor.fft_size(), 2048);
        assert_eq!(processor.bin_count(), 1025);
    }

    #[test]
    fn test_non_power_of_two_rejected() {
        let mut processor = TransferFunctionProcessor::new(1024, SAMPLE_RATE);
        processor.set_fft_size(1000); // Not power of 2
        assert_eq!(processor.fft_size(), 1024); // Should remain unchanged
    }

    #[test]
    fn test_group_delay_finite() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let signal = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);
        processor.process(&signal, &signal);

        let data = processor.get_data();

        // All group delay values should be finite
        for &gd in &data.group_delay_ms {
            assert!(gd.is_finite(), "Group delay contains non-finite value");
        }
    }

    #[test]
    fn test_frequency_bins_correct() {
        let processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);
        let data = processor.get_data();

        assert_eq!(data.frequencies.len(), FFT_SIZE / 2 + 1);
        assert!((data.frequencies[0] - 0.0).abs() < 0.1);

        // Nyquist frequency
        let expected_nyquist = SAMPLE_RATE / 2.0;
        let actual_nyquist = data.frequencies[data.frequencies.len() - 1];
        assert!(
            (actual_nyquist - expected_nyquist).abs() < 1.0,
            "Expected Nyquist ~{}, got {}",
            expected_nyquist,
            actual_nyquist
        );
    }

    #[test]
    fn test_multiple_processing_calls() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);
        let signal = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 2, 0.5);

        // Process multiple times for averaging
        for _ in 0..10 {
            processor.process(&signal, &signal);
        }

        assert!(processor.average_count() > 1, "Should have accumulated averages");

        let data = processor.get_data();
        let bin_1k = (1000.0 / SAMPLE_RATE * FFT_SIZE as f32).round() as usize;
        assert!(
            data.coherence[bin_1k] > 0.95,
            "Coherence should improve with averaging"
        );
    }

    #[test]
    fn test_sample_rate_change() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, 48000.0);
        processor.set_sample_rate(96000.0);

        let data = processor.get_data();
        let expected_nyquist = 48000.0; // 96000 / 2
        let actual_nyquist = data.frequencies[data.frequencies.len() - 1];
        assert!(
            (actual_nyquist - expected_nyquist).abs() < 1.0,
            "Nyquist should update with sample rate"
        );
    }

    #[test]
    fn test_silence_input() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);
        let reference = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);
        let silence = vec![0.0; FFT_SIZE * 4];

        processor.process(&silence, &reference);

        let data = processor.get_data();
        // Magnitude should be very low
        assert!(
            data.magnitude_db.iter().all(|&v| v < -60.0),
            "Silent input should produce low magnitude"
        );
    }

    #[test]
    fn test_inverted_polarity() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        let reference = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);
        let inverted: Vec<f32> = reference.iter().map(|&x| -x).collect();

        processor.process(&inverted, &reference);

        let data = processor.get_data();
        let bin_1k = (1000.0 / SAMPLE_RATE * FFT_SIZE as f32).round() as usize;

        // Phase should be near 180 degrees (or -180)
        assert!(
            (data.phase_deg[bin_1k].abs() - 180.0).abs() < 10.0,
            "Inverted signal should have ~180° phase, got {}",
            data.phase_deg[bin_1k]
        );
    }

    #[test]
    fn test_broadband_signal() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);

        // Multi-tone signal covering frequency range
        let signal: Vec<f32> = (0..(FFT_SIZE * 4))
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE;
                0.2 * (2.0 * PI * 100.0 * t).sin()
                    + 0.2 * (2.0 * PI * 500.0 * t).sin()
                    + 0.2 * (2.0 * PI * 1000.0 * t).sin()
                    + 0.2 * (2.0 * PI * 5000.0 * t).sin()
                    + 0.2 * (2.0 * PI * 10000.0 * t).sin()
            })
            .collect();

        processor.process(&signal, &signal);

        let data = processor.get_data();
        // Check that data is valid and has expected length
        assert_eq!(data.magnitude_db.len(), FFT_SIZE / 2 + 1);
        assert_eq!(data.coherence.len(), FFT_SIZE / 2 + 1);

        // All values should be finite
        assert!(data.magnitude_db.iter().all(|&v| v.is_finite()));
        assert!(data.coherence.iter().all(|&v| v.is_finite()));
    }

    #[test]
    fn test_small_fft_size() {
        let processor = TransferFunctionProcessor::new(256, SAMPLE_RATE);
        assert_eq!(processor.fft_size(), 256);
        assert_eq!(processor.bin_count(), 129);
    }

    #[test]
    fn test_large_fft_size() {
        let processor = TransferFunctionProcessor::new(16384, SAMPLE_RATE);
        assert_eq!(processor.fft_size(), 16384);
        assert_eq!(processor.bin_count(), 8193);
    }

    #[test]
    fn test_magnitude_db_range() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);
        let signal = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);

        processor.process(&signal, &signal);

        let data = processor.get_data();
        for &mag in &data.magnitude_db {
            assert!(mag.is_finite(), "Magnitude should be finite");
            assert!(mag <= 20.0, "Magnitude should not exceed reasonable max");
        }
    }

    #[test]
    fn test_coherence_range() {
        let mut processor = TransferFunctionProcessor::new(FFT_SIZE, SAMPLE_RATE);
        let signal = create_sine_wave(1000.0, SAMPLE_RATE, FFT_SIZE * 4, 0.5);

        processor.process(&signal, &signal);

        let data = processor.get_data();
        for &coh in &data.coherence {
            assert!(coh >= 0.0 && coh <= 1.0, "Coherence must be in [0, 1]");
        }
    }
}

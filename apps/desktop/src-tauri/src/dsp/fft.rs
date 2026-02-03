use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

use super::windows::apply_hann_window;

#[allow(dead_code)]
pub struct FftProcessor {
    fft_size: usize,
    planner: RealFftPlanner<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    input_buffer: Vec<f32>,
    scratch: Vec<realfft::num_complex::Complex<f32>>,
    spectrum: Vec<realfft::num_complex::Complex<f32>>,
    magnitude_db: Vec<f32>,
    sample_accumulator: Vec<f32>,
    hop_size: usize,
}

impl FftProcessor {
    pub fn new(fft_size: usize) -> Self {
        let mut planner = RealFftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        let scratch_len = fft.get_scratch_len();
        let spectrum_len = fft_size / 2 + 1;

        Self {
            fft_size,
            planner,
            fft,
            input_buffer: vec![0.0; fft_size],
            scratch: vec![realfft::num_complex::Complex::new(0.0, 0.0); scratch_len],
            spectrum: vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_len],
            magnitude_db: vec![-120.0; spectrum_len],
            sample_accumulator: Vec::with_capacity(fft_size * 2),
            hop_size: fft_size / 2, // 50% overlap
        }
    }

    pub fn process(&mut self, samples: &[f32]) {
        // Accumulate samples
        self.sample_accumulator.extend_from_slice(samples);

        // Process when we have enough samples
        while self.sample_accumulator.len() >= self.fft_size {
            // Copy samples to input buffer
            self.input_buffer
                .copy_from_slice(&self.sample_accumulator[..self.fft_size]);

            // Apply window function
            apply_hann_window(&mut self.input_buffer);

            // Perform FFT
            if let Err(e) = self.fft.process_with_scratch(
                &mut self.input_buffer,
                &mut self.spectrum,
                &mut self.scratch,
            ) {
                log::error!("FFT error: {}", e);
                return;
            }

            // Calculate magnitude in dB
            let scale = 2.0 / self.fft_size as f32;
            for (i, bin) in self.spectrum.iter().enumerate() {
                let magnitude = (bin.re * bin.re + bin.im * bin.im).sqrt() * scale;
                // Convert to dB with floor at -120 dB
                let db = if magnitude > 1e-6 {
                    20.0 * magnitude.log10()
                } else {
                    -120.0
                };
                // Apply smoothing (simple exponential moving average)
                self.magnitude_db[i] = self.magnitude_db[i] * 0.7 + db * 0.3;
            }

            // Remove processed samples (hop by hop_size for overlap)
            self.sample_accumulator.drain(..self.hop_size);
        }
    }

    pub fn get_magnitude_db(&self) -> Vec<f32> {
        self.magnitude_db.clone()
    }

    #[allow(dead_code)]
    pub fn set_fft_size(&mut self, fft_size: usize) {
        if fft_size != self.fft_size && fft_size.is_power_of_two() {
            *self = Self::new(fft_size);
        }
    }

    #[allow(dead_code)]
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    #[allow(dead_code)]
    pub fn bin_count(&self) -> usize {
        self.fft_size / 2 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_fft_processor_creation() {
        let processor = FftProcessor::new(1024);
        assert_eq!(processor.fft_size(), 1024);
        assert_eq!(processor.bin_count(), 513); // N/2 + 1
    }

    #[test]
    fn test_fft_processor_various_sizes() {
        for &size in &[512, 1024, 2048, 4096, 8192] {
            let processor = FftProcessor::new(size);
            assert_eq!(processor.fft_size(), size);
            assert_eq!(processor.bin_count(), size / 2 + 1);
        }
    }

    #[test]
    fn test_fft_set_size() {
        let mut processor = FftProcessor::new(1024);
        assert_eq!(processor.fft_size(), 1024);

        processor.set_fft_size(2048);
        assert_eq!(processor.fft_size(), 2048);
        assert_eq!(processor.bin_count(), 1025);
    }

    #[test]
    fn test_fft_rejects_non_power_of_two() {
        let mut processor = FftProcessor::new(1024);
        processor.set_fft_size(1000); // Not power of 2
        assert_eq!(processor.fft_size(), 1024); // Should remain unchanged
    }

    #[test]
    fn test_fft_sine_wave_detection() {
        let mut processor = FftProcessor::new(1024);
        let sample_rate = 48000.0;
        let frequency = 1000.0;

        // Generate a 1kHz sine wave
        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate).sin())
            .collect();

        processor.process(&samples);

        let magnitude = processor.get_magnitude_db();
        assert!(!magnitude.is_empty());

        // Find the bin with highest magnitude
        let (max_bin, _max_value) = magnitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        // Expected bin for 1kHz at 48kHz sample rate with 1024 FFT size
        let expected_bin = (frequency / sample_rate * 1024.0).round() as usize;

        // Allow some tolerance (within 2 bins)
        assert!(
            (max_bin as i32 - expected_bin as i32).abs() <= 2,
            "Expected peak near bin {}, got bin {}",
            expected_bin,
            max_bin
        );
    }

    #[test]
    fn test_fft_silence_returns_low_db() {
        let mut processor = FftProcessor::new(1024);
        let silence: Vec<f32> = vec![0.0; 2048];

        processor.process(&silence);

        let magnitude = processor.get_magnitude_db();
        for &db in &magnitude {
            assert!(db <= -100.0, "Silence should produce very low dB values");
        }
    }

    #[test]
    fn test_fft_accumulator() {
        let mut processor = FftProcessor::new(1024);

        // Process smaller chunks
        let chunk: Vec<f32> = vec![0.1; 256];
        for _ in 0..8 {
            processor.process(&chunk);
        }

        let magnitude = processor.get_magnitude_db();
        assert!(!magnitude.is_empty());
    }

    #[test]
    fn test_fft_multiple_frequencies() {
        let mut processor = FftProcessor::new(2048);
        let sample_rate = 48000.0;
        let freq1 = 440.0; // A4
        let freq2 = 880.0; // A5

        // Generate two sine waves
        let samples: Vec<f32> = (0..4096)
            .map(|i| {
                let t = i as f32 / sample_rate;
                0.5 * (2.0 * PI * freq1 * t).sin() + 0.5 * (2.0 * PI * freq2 * t).sin()
            })
            .collect();

        processor.process(&samples);
        let magnitude = processor.get_magnitude_db();

        // Both frequencies should produce peaks
        let bin1 = (freq1 / sample_rate * 2048.0).round() as usize;
        let bin2 = (freq2 / sample_rate * 2048.0).round() as usize;

        // Check that both bins have significant energy (above -60dB)
        assert!(
            magnitude[bin1] > -60.0,
            "Bin {} should have significant energy",
            bin1
        );
        assert!(
            magnitude[bin2] > -60.0,
            "Bin {} should have significant energy",
            bin2
        );
    }

    #[test]
    fn test_fft_dc_offset() {
        let mut processor = FftProcessor::new(1024);

        // Signal with DC offset
        let samples: Vec<f32> = vec![0.5; 2048];

        processor.process(&samples);
        let magnitude = processor.get_magnitude_db();

        // DC component (bin 0) should be the highest
        let max_bin = magnitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        assert_eq!(max_bin, 0, "DC offset should produce peak at bin 0");
    }

    #[test]
    fn test_fft_high_frequency() {
        let mut processor = FftProcessor::new(1024);
        let sample_rate = 48000.0;
        let frequency = 20000.0; // Near Nyquist

        let samples: Vec<f32> = (0..2048)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate).sin())
            .collect();

        processor.process(&samples);
        let magnitude = processor.get_magnitude_db();

        // Should have peak near the expected bin
        let expected_bin = (frequency / sample_rate * 1024.0).round() as usize;
        let (max_bin, _) = magnitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        assert!(
            (max_bin as i32 - expected_bin as i32).abs() <= 3,
            "High frequency peak at bin {}, expected {}",
            max_bin,
            expected_bin
        );
    }

    #[test]
    fn test_fft_low_amplitude() {
        let mut processor = FftProcessor::new(1024);
        let sample_rate = 48000.0;
        let frequency = 1000.0;
        let amplitude = 0.001; // Very low amplitude

        let samples: Vec<f32> = (0..2048)
            .map(|i| amplitude * (2.0 * PI * frequency * i as f32 / sample_rate).sin())
            .collect();

        processor.process(&samples);
        let magnitude = processor.get_magnitude_db();

        // Should still detect the frequency even at low amplitude
        let expected_bin = (frequency / sample_rate * 1024.0).round() as usize;
        let (max_bin, _) = magnitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        assert!(
            (max_bin as i32 - expected_bin as i32).abs() <= 2,
            "Low amplitude signal peak at bin {}, expected {}",
            max_bin,
            expected_bin
        );
    }

    #[test]
    fn test_fft_impulse_response() {
        let mut processor = FftProcessor::new(1024);

        // Single impulse
        let mut samples = vec![0.0; 2048];
        samples[0] = 1.0;

        processor.process(&samples);
        let magnitude = processor.get_magnitude_db();

        // Impulse should have flat spectrum
        let avg: f32 = magnitude.iter().sum::<f32>() / magnitude.len() as f32;
        let variance: f32 =
            magnitude.iter().map(|&x| (x - avg).powi(2)).sum::<f32>() / magnitude.len() as f32;

        // Variance should be relatively low for flat spectrum
        assert!(
            variance < 100.0,
            "Impulse spectrum should be relatively flat"
        );
    }

    #[test]
    fn test_fft_set_size_same() {
        let mut processor = FftProcessor::new(1024);
        processor.set_fft_size(1024); // Same size
        assert_eq!(processor.fft_size(), 1024);
    }

    #[test]
    fn test_fft_smallest_valid_size() {
        let processor = FftProcessor::new(64);
        assert_eq!(processor.fft_size(), 64);
        assert_eq!(processor.bin_count(), 33);
    }

    #[test]
    fn test_fft_largest_practical_size() {
        let processor = FftProcessor::new(16384);
        assert_eq!(processor.fft_size(), 16384);
        assert_eq!(processor.bin_count(), 8193);
    }
}

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    Sine,
    WhiteNoise,
    PinkNoise,
    Sweep,
}

pub struct Generator {
    signal_type: SignalType,
    sample_rate: f64,
    frequency: f64,
    amplitude: f64,
    phase: f64,
    // Pink noise state (Voss-McCartney algorithm)
    pink_rows: [f32; 16],
    pink_running_sum: f32,
    pink_index: usize,
    // Sweep state
    sweep_start_freq: f64,
    sweep_end_freq: f64,
    sweep_duration: f64,
    sweep_position: f64,
    // RNG - using SmallRng which is Send + Sync
    rng: SmallRng,
}

impl Generator {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            signal_type: SignalType::Sine,
            sample_rate,
            frequency: 1000.0,
            amplitude: 0.5,
            phase: 0.0,
            pink_rows: [0.0; 16],
            pink_running_sum: 0.0,
            pink_index: 0,
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
            sweep_duration: 5.0,
            sweep_position: 0.0,
            rng: SmallRng::from_entropy(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
    }

    pub fn set_signal_type(&mut self, signal_type: SignalType) {
        self.signal_type = signal_type;
        self.phase = 0.0;
        self.sweep_position = 0.0;
    }

    pub fn set_frequency(&mut self, frequency: f64) {
        self.frequency = frequency.clamp(20.0, 20000.0);
    }

    pub fn set_amplitude(&mut self, amplitude: f64) {
        self.amplitude = amplitude.clamp(0.0, 1.0);
    }

    pub fn signal_type_name(&self) -> String {
        match self.signal_type {
            SignalType::Sine => "sine".to_string(),
            SignalType::WhiteNoise => "white_noise".to_string(),
            SignalType::PinkNoise => "pink_noise".to_string(),
            SignalType::Sweep => "sweep".to_string(),
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let sample = match self.signal_type {
            SignalType::Sine => self.generate_sine(),
            SignalType::WhiteNoise => self.generate_white_noise(),
            SignalType::PinkNoise => self.generate_pink_noise(),
            SignalType::Sweep => self.generate_sweep(),
        };

        (sample * self.amplitude) as f32
    }

    fn generate_sine(&mut self) -> f64 {
        let sample = (2.0 * PI * self.phase).sin();
        self.phase += self.frequency / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        sample
    }

    fn generate_white_noise(&mut self) -> f64 {
        self.rng.gen_range(-1.0..1.0)
    }

    fn generate_pink_noise(&mut self) -> f64 {
        // Voss-McCartney algorithm for pink noise
        let white: f32 = self.rng.gen_range(-1.0..1.0);

        self.pink_index = (self.pink_index + 1) % 16;

        // Update running sum
        self.pink_running_sum -= self.pink_rows[self.pink_index];
        self.pink_rows[self.pink_index] = white;
        self.pink_running_sum += white;

        // Mix with white noise for smoothness
        let pink = (self.pink_running_sum / 16.0 + white) / 2.0;

        pink as f64
    }

    fn generate_sweep(&mut self) -> f64 {
        // Logarithmic sweep
        let t = self.sweep_position / self.sample_rate;
        let duration = self.sweep_duration;

        if t >= duration {
            self.sweep_position = 0.0;
            return 0.0;
        }

        let k = (self.sweep_end_freq / self.sweep_start_freq).ln() / duration;
        let phase_integral = self.sweep_start_freq * ((k * t).exp() - 1.0) / k;

        self.sweep_position += 1.0;

        (2.0 * PI * phase_integral).sin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 48000.0;

    #[test]
    fn test_generator_creation() {
        let gen = Generator::new(SAMPLE_RATE);
        assert_eq!(gen.signal_type_name(), "sine");
    }

    #[test]
    fn test_sine_wave_range() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_amplitude(1.0);

        for _ in 0..1000 {
            let sample = gen.next_sample();
            assert!(
                (-1.0..=1.0).contains(&sample),
                "Sine sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_sine_wave_frequency() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_frequency(1000.0);
        gen.set_amplitude(1.0);

        // Generate one full cycle (48 samples at 48kHz for 1kHz)
        let samples_per_cycle = (SAMPLE_RATE / 1000.0) as usize;
        let samples: Vec<f32> = (0..samples_per_cycle * 2)
            .map(|_| gen.next_sample())
            .collect();

        // Find zero crossings
        let mut crossings = 0;
        for i in 1..samples.len() {
            if (samples[i - 1] < 0.0) != (samples[i] < 0.0) {
                crossings += 1;
            }
        }

        // Two cycles should have 4 zero crossings
        assert!(
            (3..=5).contains(&crossings),
            "Expected ~4 zero crossings, got {}",
            crossings
        );
    }

    #[test]
    fn test_white_noise_range() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::WhiteNoise);
        gen.set_amplitude(1.0);

        for _ in 0..1000 {
            let sample = gen.next_sample();
            assert!(
                (-1.0..=1.0).contains(&sample),
                "White noise sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_white_noise_distribution() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::WhiteNoise);
        gen.set_amplitude(1.0);

        // White noise should have roughly zero mean
        let samples: Vec<f32> = (0..10000).map(|_| gen.next_sample()).collect();
        let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;

        assert!(
            mean.abs() < 0.1,
            "White noise mean {} should be near zero",
            mean
        );
    }

    #[test]
    fn test_pink_noise_range() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::PinkNoise);
        gen.set_amplitude(1.0);

        for _ in 0..1000 {
            let sample = gen.next_sample();
            assert!(
                (-2.0..=2.0).contains(&sample), // Pink noise can have wider range
                "Pink noise sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_sweep_range() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sweep);
        gen.set_amplitude(1.0);

        for _ in 0..1000 {
            let sample = gen.next_sample();
            assert!(
                (-1.0..=1.0).contains(&sample),
                "Sweep sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_amplitude_scaling() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_frequency(1000.0);

        // Test half amplitude
        gen.set_amplitude(0.5);
        for _ in 0..1000 {
            let sample = gen.next_sample();
            assert!(
                (-0.5..=0.5).contains(&sample),
                "Sample {} exceeds amplitude 0.5",
                sample
            );
        }
    }

    #[test]
    fn test_frequency_clamping() {
        let mut gen = Generator::new(SAMPLE_RATE);

        gen.set_frequency(10.0); // Below min
        gen.set_signal_type(SignalType::Sine);
        // Should still work, just clamped to 20Hz

        gen.set_frequency(25000.0); // Above max
                                    // Should still work, just clamped to 20kHz
    }

    #[test]
    fn test_amplitude_clamping() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);

        gen.set_amplitude(2.0); // Above max
        for _ in 0..100 {
            let sample = gen.next_sample();
            assert!(sample.abs() <= 1.0, "Amplitude should be clamped to 1.0");
        }
    }

    #[test]
    fn test_signal_type_switching() {
        let mut gen = Generator::new(SAMPLE_RATE);

        gen.set_signal_type(SignalType::Sine);
        assert_eq!(gen.signal_type_name(), "sine");

        gen.set_signal_type(SignalType::WhiteNoise);
        assert_eq!(gen.signal_type_name(), "white_noise");

        gen.set_signal_type(SignalType::PinkNoise);
        assert_eq!(gen.signal_type_name(), "pink_noise");

        gen.set_signal_type(SignalType::Sweep);
        assert_eq!(gen.signal_type_name(), "sweep");
    }

    #[test]
    fn test_sine_zero_amplitude() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_amplitude(0.0);

        for _ in 0..100 {
            let sample = gen.next_sample();
            assert!(
                sample.abs() < 0.001,
                "Zero amplitude should produce silence"
            );
        }
    }

    #[test]
    fn test_sine_nyquist_frequency() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_frequency(SAMPLE_RATE / 2.0); // Nyquist
        gen.set_amplitude(1.0);

        // Should still produce valid samples
        for _ in 0..100 {
            let sample = gen.next_sample();
            assert!(
                sample.is_finite(),
                "Should produce finite samples at Nyquist"
            );
        }
    }

    #[test]
    fn test_sine_low_frequency() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_frequency(20.0);
        gen.set_amplitude(1.0);

        let samples: Vec<f32> = (0..48000).map(|_| gen.next_sample()).collect();
        let max = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min = samples.iter().cloned().fold(f32::INFINITY, f32::min);

        // Should span full amplitude range
        assert!(max > 0.9, "Low freq sine should reach positive peak");
        assert!(min < -0.9, "Low freq sine should reach negative peak");
    }

    #[test]
    fn test_multiple_sample_rates() {
        for &rate in &[44100.0, 48000.0, 96000.0, 192000.0] {
            let mut gen = Generator::new(rate);
            gen.set_signal_type(SignalType::Sine);
            gen.set_frequency(1000.0);
            gen.set_amplitude(1.0);

            for _ in 0..100 {
                let sample = gen.next_sample();
                assert!(
                    (-1.0..=1.0).contains(&sample),
                    "Sample rate {} should produce valid samples",
                    rate
                );
            }
        }
    }

    #[test]
    fn test_sweep_progress() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sweep);
        gen.set_amplitude(1.0);

        // Generate enough samples for a full sweep
        let num_samples = (SAMPLE_RATE * 5.0) as usize;
        let mut all_finite = true;
        for _ in 0..num_samples {
            let sample = gen.next_sample();
            if !sample.is_finite() {
                all_finite = false;
                break;
            }
        }

        assert!(all_finite, "All sweep samples should be finite");
    }

    #[test]
    fn test_pink_noise_variance() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::PinkNoise);
        gen.set_amplitude(1.0);

        let samples: Vec<f32> = (0..10000).map(|_| gen.next_sample()).collect();
        let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
        let variance: f32 =
            samples.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / samples.len() as f32;

        // Pink noise should have some variance (not all zeros)
        assert!(variance > 0.01, "Pink noise should have variance");
    }

    #[test]
    fn test_continuous_output() {
        let mut gen = Generator::new(SAMPLE_RATE);
        gen.set_signal_type(SignalType::Sine);
        gen.set_frequency(440.0);
        gen.set_amplitude(1.0);

        // Generate a large number of samples without issues
        let mut prev_sample = gen.next_sample();
        for _ in 0..100000 {
            let sample = gen.next_sample();
            // Samples should not jump by more than expected for a sine wave
            let max_delta = 2.0 * std::f32::consts::PI * 440.0 / SAMPLE_RATE as f32 * 1.5;
            assert!(
                (sample - prev_sample).abs() < max_delta * 2.0,
                "Sample delta too large: {} -> {}",
                prev_sample,
                sample
            );
            prev_sample = sample;
        }
    }
}

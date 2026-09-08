use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, StreamConfig};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use super::deinterleave::ChannelBuffers;
use super::generator::{Generator, SignalType};
use super::realtime::configure_audio_thread;
use crate::dsp::{
    FftProcessor, MultiWindowProcessor, RigSnapshot, SmoothingFraction, TransferFunctionData,
    TransferFunctionProcessor,
};

#[cfg(target_os = "macos")]
use super::coreaudio_backend::{
    self, CoreAudioConfig, CoreAudioInputStream, CoreAudioOutputStream,
};

const FFT_SIZE: usize = 2048;
const DEFAULT_SAMPLE_RATE: f32 = 48000.0;

/// Real system tuning uses several mics at once. Eight covers the practical
/// ceiling of interfaces used for live sound measurement.
pub const MAX_INPUT_CHANNELS: usize = 8;

/// Audio backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioBackend {
    /// Cross-platform audio via CPAL (default)
    #[default]
    Cpal = 0,
    /// Direct CoreAudio HAL access (macOS only, lower latency)
    #[cfg(target_os = "macos")]
    CoreAudio = 1,
}

impl From<u8> for AudioBackend {
    fn from(value: u8) -> Self {
        match value {
            #[cfg(target_os = "macos")]
            1 => Self::CoreAudio,
            _ => Self::Cpal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioState {
    pub is_running: bool,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub generator_active: bool,
    pub generator_type: Option<String>,
    pub backend: String,
    pub backend_available: Vec<String>,
    /// Whether exclusive (hog) mode is enabled (macOS CoreAudio only)
    pub exclusive_mode: bool,
}

/// Peak and RMS over the most recent callback, plus a short delay history for
/// drift detection.
#[derive(Default)]
pub struct LevelStats {
    pub reference_peak: f32,
    pub measurement_peak: f32,
    pub reference_rms: f32,
    pub measurement_rms: f32,
    pub delay_history: Vec<i32>,
}

impl LevelStats {
    /// Retain at most this many delay samples; enough to see a trend, short
    /// enough to react when the operator fixes the clock.
    const DELAY_HISTORY: usize = 16;

    pub fn push_delay(&mut self, delay: i32) {
        self.delay_history.push(delay);
        if self.delay_history.len() > Self::DELAY_HISTORY {
            self.delay_history.remove(0);
        }
    }

    /// Compute peak and RMS in place over the given slices. Called from the
    /// audio callback, so this must not allocate.
    pub fn update(&mut self, measurement: &[f32], reference: &[f32]) {
        let peak = |v: &[f32]| v.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let rms = |v: &[f32]| {
            if v.is_empty() {
                0.0
            } else {
                (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt()
            }
        };
        self.measurement_peak = peak(measurement);
        self.reference_peak = peak(reference);
        self.measurement_rms = rms(measurement);
        self.reference_rms = rms(reference);
    }
}

/// Thread-safe audio engine that can be used with Tauri state.
/// Audio streams run in a dedicated thread since cpal::Stream is not Send+Sync.
pub struct AudioEngine {
    is_running: Arc<AtomicBool>,
    sample_rate: Arc<AtomicU32>,
    buffer_size: Arc<AtomicU32>,
    input_device_name: Arc<Mutex<Option<String>>>,
    output_device_name: Arc<Mutex<Option<String>>>,
    fft_processor: Arc<Mutex<FftProcessor>>,
    // One transfer function processor per input channel, so several mic
    // positions can be measured (and later spatially averaged) at once.
    // Always MAX_INPUT_CHANNELS long; only the first `input_channel_count`
    // are fed by the audio callback.
    transfer_function_processors: Vec<Arc<Mutex<TransferFunctionProcessor>>>,
    multi_window_processor: Arc<Mutex<MultiWindowProcessor>>,
    // Whether the multi-time-window processor (vs. the single-FFT processor) is active
    multi_window_enabled: Arc<AtomicBool>,
    // Number of input channels to open on the next stream start, clamped to
    // 1..=MAX_INPUT_CHANNELS.
    input_channel_count: Arc<AtomicUsize>,
    // Which input channel feeds level metering and rig diagnostics.
    measurement_channel: Arc<AtomicUsize>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    // Buffer to store reference (generator) samples for transfer function
    reference_buffer: Arc<Mutex<Vec<f32>>>,
    // Audio backend selection (0 = CPAL, 1 = CoreAudio on macOS)
    backend: Arc<AtomicU8>,
    // Exclusive mode flag (macOS CoreAudio only)
    exclusive_mode: Arc<AtomicBool>,
    /// Rolling level and delay statistics, updated from the input callback.
    level_stats: Arc<Mutex<LevelStats>>,
}

// SAFETY: AudioEngine is Send + Sync because:
// - All atomic fields (is_running, sample_rate, buffer_size, generator_active, stop_signal)
//   are inherently thread-safe via std::sync::atomic
// - All Mutex-wrapped fields (input_device_name, output_device_name, fft_processor,
//   transfer_function_processors, multi_window_processor, generator, thread_handle,
//   reference_buffer) use parking_lot::Mutex which is Send + Sync
// - Arc provides thread-safe reference counting
// The thread_handle stores a JoinHandle which is Send, and we only mutate it under a Mutex lock.
// cpal::Stream is not Send+Sync, but we don't store streams - they live in the audio thread only.
unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    pub fn new() -> Self {
        let fft_processor = Arc::new(Mutex::new(FftProcessor::new(FFT_SIZE)));
        let transfer_function_processors = (0..MAX_INPUT_CHANNELS)
            .map(|_| {
                Arc::new(Mutex::new(TransferFunctionProcessor::new(
                    FFT_SIZE,
                    DEFAULT_SAMPLE_RATE,
                )))
            })
            .collect();
        let multi_window_processor =
            Arc::new(Mutex::new(MultiWindowProcessor::new(DEFAULT_SAMPLE_RATE)));
        let generator = Arc::new(Mutex::new(Generator::new(DEFAULT_SAMPLE_RATE as f64)));

        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            sample_rate: Arc::new(AtomicU32::new(DEFAULT_SAMPLE_RATE as u32)),
            buffer_size: Arc::new(AtomicU32::new(512)),
            input_device_name: Arc::new(Mutex::new(None)),
            output_device_name: Arc::new(Mutex::new(None)),
            fft_processor,
            transfer_function_processors,
            multi_window_processor,
            multi_window_enabled: Arc::new(AtomicBool::new(false)),
            input_channel_count: Arc::new(AtomicUsize::new(1)),
            measurement_channel: Arc::new(AtomicUsize::new(0)),
            generator,
            generator_active: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
            reference_buffer: Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE * 2))),
            backend: Arc::new(AtomicU8::new(AudioBackend::default() as u8)),
            exclusive_mode: Arc::new(AtomicBool::new(false)),
            level_stats: Arc::new(Mutex::new(LevelStats::default())),
        }
    }

    pub fn get_state(&self) -> AudioState {
        // Lock generator first to avoid race between checking active and getting type
        let gen = self.generator.lock();
        let is_active = self.generator_active.load(Ordering::Acquire);
        let gen_type = if is_active {
            Some(gen.signal_type_name())
        } else {
            None
        };
        drop(gen); // Release lock before acquiring others

        let current_backend = AudioBackend::from(self.backend.load(Ordering::Relaxed));
        let backend_name = match current_backend {
            AudioBackend::Cpal => "cpal",
            #[cfg(target_os = "macos")]
            AudioBackend::CoreAudio => "core_audio",
        };

        let mut available = vec!["cpal".to_string()];
        #[cfg(target_os = "macos")]
        available.push("core_audio".to_string());

        AudioState {
            is_running: self.is_running.load(Ordering::Acquire),
            sample_rate: self.sample_rate.load(Ordering::Relaxed),
            buffer_size: self.buffer_size.load(Ordering::Relaxed),
            input_device: self.input_device_name.lock().clone(),
            output_device: self.output_device_name.lock().clone(),
            generator_active: is_active,
            generator_type: gen_type,
            backend: backend_name.to_string(),
            backend_available: available,
            exclusive_mode: self.exclusive_mode.load(Ordering::Relaxed),
        }
    }

    /// Set the audio backend (takes effect on next stream start)
    pub fn set_backend(&self, backend: AudioBackend) -> Result<(), String> {
        #[cfg(not(target_os = "macos"))]
        if matches!(backend, AudioBackend::CoreAudio) {
            return Err("CoreAudio is only available on macOS".to_string());
        }

        self.backend.store(backend as u8, Ordering::Relaxed);
        log::info!("Audio backend set to {:?}", backend);
        Ok(())
    }

    /// Get current backend
    pub fn get_backend(&self) -> AudioBackend {
        AudioBackend::from(self.backend.load(Ordering::Relaxed))
    }

    /// Set exclusive mode (takes effect on next stream start, CoreAudio only)
    pub fn set_exclusive_mode(&self, enabled: bool) {
        self.exclusive_mode.store(enabled, Ordering::Relaxed);
        log::info!("Exclusive mode set to {}", enabled);
    }

    /// Get exclusive mode setting
    pub fn get_exclusive_mode(&self) -> bool {
        self.exclusive_mode.load(Ordering::Relaxed)
    }

    pub fn start_stream(
        &self,
        input_device_id: Option<&str>,
        output_device_id: Option<&str>,
    ) -> Result<(), String> {
        // Stop any existing stream first
        self.stop_stream();

        let backend = self.get_backend();

        #[cfg(target_os = "macos")]
        if backend == AudioBackend::CoreAudio {
            return self.start_coreaudio_stream(input_device_id, output_device_id);
        }

        self.start_cpal_stream(input_device_id, output_device_id)
    }

    /// Start audio stream using CPAL backend
    fn start_cpal_stream(
        &self,
        input_device_id: Option<&str>,
        output_device_id: Option<&str>,
    ) -> Result<(), String> {
        let host = cpal::default_host();

        // Find input device
        let input_device = match input_device_id {
            Some(id) => find_device_by_name(&host, id, true)?,
            None => host
                .default_input_device()
                .ok_or("No default input device")?,
        };

        // Find output device
        let output_device = match output_device_id {
            Some(id) => find_device_by_name(&host, id, false)?,
            None => host
                .default_output_device()
                .ok_or("No default output device")?,
        };

        let input_name = input_device.name().unwrap_or_default();
        let output_name = output_device.name().unwrap_or_default();

        // Get supported config
        let input_config = input_device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        let output_config = output_device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {}", e))?;

        let sample_rate = input_config.sample_rate().0;
        let buffer_size = 512u32;

        // Update state
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.buffer_size.store(buffer_size, Ordering::Relaxed);
        *self.input_device_name.lock() = Some(input_name.clone());
        *self.output_device_name.lock() = Some(output_name.clone());

        // Update generator and transfer function processor sample rates.
        // Every processor must be told: the multi-window one derives its whole
        // frequency axis from this, so a stale rate silently mislabels every
        // reading on any device that is not 48 kHz.
        self.generator.lock().set_sample_rate(sample_rate as f64);
        for processor in &self.transfer_function_processors {
            processor.lock().set_sample_rate(sample_rate as f32);
        }
        self.multi_window_processor
            .lock()
            .set_sample_rate(sample_rate as f32);

        // Reset stop signal and clear reference buffer
        self.stop_signal.store(false, Ordering::Relaxed);
        self.reference_buffer.lock().clear();

        // Clone Arcs for the audio thread
        let fft_processor = Arc::clone(&self.fft_processor);
        let transfer_function_processors = self.transfer_function_processors.clone();
        let multi_window_processor = Arc::clone(&self.multi_window_processor);
        let multi_window_enabled = Arc::clone(&self.multi_window_enabled);
        let channel_count = self
            .input_channel_count
            .load(Ordering::Relaxed)
            .clamp(1, MAX_INPUT_CHANNELS);
        let measurement_channel = Arc::clone(&self.measurement_channel);
        let generator = Arc::clone(&self.generator);
        let generator_active = Arc::clone(&self.generator_active);
        let is_running = Arc::clone(&self.is_running);
        let stop_signal = Arc::clone(&self.stop_signal);
        let reference_buffer = Arc::clone(&self.reference_buffer);
        let level_stats = Arc::clone(&self.level_stats);

        let output_channels = output_config.channels();

        // Spawn audio thread - streams must live in the same thread they're created
        let handle = thread::spawn(move || {
            run_cpal_streams(
                input_device,
                output_device,
                sample_rate,
                buffer_size,
                channel_count,
                output_channels,
                fft_processor,
                transfer_function_processors,
                multi_window_processor,
                multi_window_enabled,
                measurement_channel,
                generator,
                generator_active,
                is_running,
                stop_signal,
                reference_buffer,
                level_stats,
            );
        });

        *self.thread_handle.lock() = Some(handle);
        self.is_running.store(true, Ordering::Relaxed);

        log::info!(
            "Audio stream started (CPAL): {} -> {}, {} Hz",
            input_name,
            output_name,
            sample_rate
        );

        Ok(())
    }

    /// Start audio stream using CoreAudio backend (macOS only)
    #[cfg(target_os = "macos")]
    fn start_coreaudio_stream(
        &self,
        input_device_id: Option<&str>,
        output_device_id: Option<&str>,
    ) -> Result<(), String> {
        let devices = coreaudio_backend::enumerate_devices();

        // Find input device
        let input_device = match input_device_id {
            Some(id) => {
                let name = id.strip_prefix("input:").unwrap_or(id);
                devices
                    .iter()
                    .find(|d| d.is_input && d.name == name)
                    .ok_or_else(|| format!("Input device not found: {}", id))?
            }
            None => devices
                .iter()
                .find(|d| {
                    d.is_input
                        && coreaudio_backend::get_default_input_device()
                            .map(|def| def == d.id)
                            .unwrap_or(false)
                })
                .or_else(|| devices.iter().find(|d| d.is_input))
                .ok_or("No input device available")?,
        };

        // Find output device
        let output_device = match output_device_id {
            Some(id) => {
                let name = id.strip_prefix("output:").unwrap_or(id);
                devices
                    .iter()
                    .find(|d| d.is_output && d.name == name)
                    .ok_or_else(|| format!("Output device not found: {}", id))?
            }
            None => devices
                .iter()
                .find(|d| {
                    d.is_output
                        && coreaudio_backend::get_default_output_device()
                            .map(|def| def == d.id)
                            .unwrap_or(false)
                })
                .or_else(|| devices.iter().find(|d| d.is_output))
                .ok_or("No output device available")?,
        };

        let input_name = input_device.name.clone();
        let output_name = output_device.name.clone();
        let input_id = input_device.id;
        let output_id = output_device.id;

        let sample_rate = 48000u32;
        let buffer_size = 256u32; // Lower buffer for CoreAudio

        // Update state
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.buffer_size.store(buffer_size, Ordering::Relaxed);
        *self.input_device_name.lock() = Some(input_name.clone());
        *self.output_device_name.lock() = Some(output_name.clone());

        // Update generator and transfer function processor sample rates.
        // Every processor must be told: the multi-window one derives its whole
        // frequency axis from this, so a stale rate silently mislabels every
        // reading on any device that is not 48 kHz.
        self.generator.lock().set_sample_rate(sample_rate as f64);
        for processor in &self.transfer_function_processors {
            processor.lock().set_sample_rate(sample_rate as f32);
        }
        self.multi_window_processor
            .lock()
            .set_sample_rate(sample_rate as f32);

        // Reset stop signal and clear reference buffer
        self.stop_signal.store(false, Ordering::Relaxed);
        self.reference_buffer.lock().clear();

        // Clone Arcs for the audio thread
        let fft_processor = Arc::clone(&self.fft_processor);
        let transfer_function_processors = self.transfer_function_processors.clone();
        let multi_window_processor = Arc::clone(&self.multi_window_processor);
        let multi_window_enabled = Arc::clone(&self.multi_window_enabled);
        let channel_count = self
            .input_channel_count
            .load(Ordering::Relaxed)
            .clamp(1, MAX_INPUT_CHANNELS);
        let measurement_channel = Arc::clone(&self.measurement_channel);
        let generator = Arc::clone(&self.generator);
        let generator_active = Arc::clone(&self.generator_active);
        let is_running = Arc::clone(&self.is_running);
        let stop_signal = Arc::clone(&self.stop_signal);
        let reference_buffer = Arc::clone(&self.reference_buffer);
        let level_stats = Arc::clone(&self.level_stats);
        let exclusive_mode = self.exclusive_mode.load(Ordering::Relaxed);

        // Spawn audio thread
        let handle = thread::spawn(move || {
            run_coreaudio_streams(
                input_id,
                output_id,
                sample_rate,
                buffer_size,
                channel_count,
                exclusive_mode,
                fft_processor,
                transfer_function_processors,
                multi_window_processor,
                multi_window_enabled,
                measurement_channel,
                generator,
                generator_active,
                is_running,
                stop_signal,
                reference_buffer,
                level_stats,
            );
        });

        *self.thread_handle.lock() = Some(handle);
        self.is_running.store(true, Ordering::Relaxed);

        log::info!(
            "Audio stream started (CoreAudio): {} -> {}, {} Hz, {} samples buffer",
            input_name,
            output_name,
            sample_rate,
            buffer_size
        );

        Ok(())
    }

    pub fn stop_stream(&self) {
        // Signal the audio thread to stop
        self.stop_signal.store(true, Ordering::Relaxed);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.lock().take() {
            let _ = handle.join();
        }

        self.is_running.store(false, Ordering::Relaxed);
        *self.input_device_name.lock() = None;
        *self.output_device_name.lock() = None;
        log::info!("Audio stream stopped");
    }

    pub fn start_generator(&self, signal_type: SignalType, frequency: f64, amplitude: f64) {
        // Configure generator while holding lock, then set active flag
        // Using Release ordering ensures the configuration is visible to the audio thread
        {
            let mut gen = self.generator.lock();
            gen.set_signal_type(signal_type);
            gen.set_frequency(frequency);
            gen.set_amplitude(amplitude);
        }
        self.generator_active.store(true, Ordering::Release);
        log::info!("Generator started: {:?} at {} Hz", signal_type, frequency);
    }

    pub fn stop_generator(&self) {
        self.generator_active.store(false, Ordering::Release);
        log::info!("Generator stopped");
    }

    pub fn get_spectrum_data(&self) -> Vec<f32> {
        self.fft_processor.lock().get_magnitude_db()
    }

    pub fn get_transfer_function_data(&self) -> TransferFunctionData {
        if self.multi_window_enabled.load(Ordering::Relaxed) {
            // Take a cheap stitch under the lock, release it, then do the
            // expensive smoothing and conversion. Doing that work while holding
            // the lock made the real-time audio callback wait on a UI read.
            let snapshot = self.multi_window_processor.lock().snapshot();
            snapshot.into_data()
        } else {
            self.transfer_function_processors[0].lock().get_data()
        }
    }

    /// Transfer function data for one input channel. In multi-window mode
    /// there is a single shared processor (widening that one is future work),
    /// so every index returns the same data there.
    pub fn transfer_function_data_for_channel(&self, index: usize) -> TransferFunctionData {
        if self.multi_window_enabled.load(Ordering::Relaxed) {
            return self.multi_window_processor.lock().get_data();
        }
        let clamped = index.min(self.transfer_function_processors.len() - 1);
        self.transfer_function_processors[clamped].lock().get_data()
    }

    /// Configure how many input channels the next stream should open.
    /// Clamped to 1..=MAX_INPUT_CHANNELS; takes effect on the next
    /// `start_stream`.
    pub fn set_input_channel_count(&self, count: usize) {
        let clamped = count.clamp(1, MAX_INPUT_CHANNELS);
        self.input_channel_count.store(clamped, Ordering::Relaxed);
        log::info!("Input channel count set to {}", clamped);
    }

    /// Number of input channels the next stream will open.
    pub fn input_channel_count(&self) -> usize {
        self.input_channel_count.load(Ordering::Relaxed)
    }

    /// Select which input channel feeds level metering and rig diagnostics.
    /// Out-of-range indices for the stream's actual channel count fall back
    /// safely at read time; `ChannelBuffers::channel` never panics.
    pub fn set_measurement_channel(&self, index: usize) {
        let clamped = index.min(MAX_INPUT_CHANNELS - 1);
        self.measurement_channel.store(clamped, Ordering::Relaxed);
        log::info!("Measurement channel set to {}", clamped);
    }

    /// Currently selected measurement channel.
    pub fn measurement_channel(&self) -> usize {
        self.measurement_channel.load(Ordering::Relaxed)
    }

    pub fn reset_averaging(&self) {
        for processor in &self.transfer_function_processors {
            processor.lock().reset_averaging();
        }
        self.multi_window_processor.lock().reset_averaging();
        log::info!("Transfer function averaging reset");
    }

    pub fn set_fft_size(&self, size: usize) -> Result<(), String> {
        if !size.is_power_of_two() {
            return Err("FFT size must be a power of 2".to_string());
        }
        if !(512..=16384).contains(&size) {
            return Err("FFT size must be between 512 and 16384".to_string());
        }

        let sample_rate = self.sample_rate.load(Ordering::Relaxed) as f32;
        self.fft_processor.lock().set_fft_size(size);
        for processor in &self.transfer_function_processors {
            let mut processor = processor.lock();
            processor.set_fft_size(size);
            // Update sample rate in case it changed
            processor.set_sample_rate(sample_rate);
        }

        log::info!("FFT size set to {}", size);
        Ok(())
    }

    /// Delay the transfer function reference to compensate for propagation time.
    pub fn set_transfer_function_delay(&self, delay_samples: usize) {
        for processor in &self.transfer_function_processors {
            processor.lock().set_delay_samples(delay_samples);
        }
        self.multi_window_processor
            .lock()
            .set_delay_samples(delay_samples);
    }

    /// Current transfer function reference delay in samples.
    pub fn transfer_function_delay(&self) -> usize {
        self.transfer_function_processors[0].lock().delay_samples()
    }

    /// Frames retained by the transfer function running average.
    pub fn set_max_averages(&self, n: usize) {
        for processor in &self.transfer_function_processors {
            processor.lock().set_max_averages(n);
        }
        self.multi_window_processor.lock().set_max_averages(n);
    }

    /// Switch between the single-FFT processor and the multi-time-window one.
    /// A fixed FFT cannot serve the whole band: 4096 points at 48 kHz gives
    /// 11.7 Hz resolution, fine at 10 kHz and useless at 60 Hz.
    pub fn set_resolution_mode(&self, multi_window: bool) {
        self.multi_window_enabled
            .store(multi_window, Ordering::Relaxed);
        log::info!("Resolution mode: multi_window={}", multi_window);
    }

    /// Whether the multi-time-window processor is currently active.
    pub fn is_multi_window(&self) -> bool {
        self.multi_window_enabled.load(Ordering::Relaxed)
    }

    /// Fractional-octave smoothing applied to the multi-window output.
    pub fn set_smoothing(&self, fraction: SmoothingFraction) {
        self.multi_window_processor.lock().set_smoothing(fraction);
    }

    /// Current smoothing setting.
    pub fn smoothing(&self) -> SmoothingFraction {
        self.multi_window_processor.lock().smoothing()
    }

    /// Current sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// Current FFT size in samples.
    pub fn fft_size(&self) -> usize {
        self.transfer_function_processors[0].lock().fft_size()
    }

    /// Record a delay measurement so drift detection has history.
    pub fn push_measured_delay(&self, delay_samples: i32) {
        self.level_stats.lock().push_delay(delay_samples);
    }

    /// Sample everything the diagnostics rules need.
    pub fn rig_snapshot(&self) -> RigSnapshot {
        let stats = self.level_stats.lock();
        let data = self.get_transfer_function_data();
        RigSnapshot {
            reference_peak: stats.reference_peak,
            measurement_peak: stats.measurement_peak,
            reference_rms: stats.reference_rms,
            measurement_rms: stats.measurement_rms,
            coherence: data.coherence,
            magnitude_db: data.magnitude_db,
            frequencies: data.frequencies,
            measured_delay_samples: stats.delay_history.last().copied().unwrap_or(0),
            applied_delay_samples: self.transfer_function_delay(),
            delay_history: stats.delay_history.clone(),
            sample_rate: self.sample_rate() as f32,
            fft_size: self.fft_size(),
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Find a device by its ID (format: "input:name" or "output:name") or by name
fn find_device_by_name(host: &cpal::Host, id: &str, is_input: bool) -> Result<Device, String> {
    // Strip the prefix if present to get the device name
    let name = if is_input {
        id.strip_prefix("input:").unwrap_or(id)
    } else {
        id.strip_prefix("output:").unwrap_or(id)
    };

    let devices = if is_input {
        host.input_devices()
    } else {
        host.output_devices()
    };

    devices
        .map_err(|e| format!("Failed to enumerate devices: {}", e))?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        .ok_or_else(|| format!("Device not found: {}", id))
}

/// Run audio streams using CPAL backend
#[allow(clippy::too_many_arguments)]
fn run_cpal_streams(
    input_device: Device,
    output_device: Device,
    sample_rate: u32,
    buffer_size: u32,
    channel_count: usize,
    output_channels: u16,
    fft_processor: Arc<Mutex<FftProcessor>>,
    transfer_function_processors: Vec<Arc<Mutex<TransferFunctionProcessor>>>,
    multi_window_processor: Arc<Mutex<MultiWindowProcessor>>,
    multi_window_enabled: Arc<AtomicBool>,
    measurement_channel: Arc<AtomicUsize>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    reference_buffer: Arc<Mutex<Vec<f32>>>,
    level_stats: Arc<Mutex<LevelStats>>,
) {
    // Set real-time priority for this audio thread
    configure_audio_thread();

    let input_config = StreamConfig {
        channels: channel_count as u16,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    let output_config = StreamConfig {
        channels: output_channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    // Clone references for input callback
    let tf_processors = transfer_function_processors.clone();
    let mw_processor = Arc::clone(&multi_window_processor);
    let mw_enabled = Arc::clone(&multi_window_enabled);
    let meas_channel = Arc::clone(&measurement_channel);
    let ref_buffer_for_input = Arc::clone(&reference_buffer);
    let stats = Arc::clone(&level_stats);

    // Allocated once, up front, sized for the maximum supported channel count
    // so it never needs to grow. Deinterleaving inside the callback below must
    // not allocate: that risks a lock in the allocator and an audible dropout.
    let mut channel_buffers =
        ChannelBuffers::new(channel_count, buffer_size as usize * MAX_INPUT_CHANNELS);

    // Build input stream
    let input_stream = match input_device.build_input_stream(
        &input_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            channel_buffers.fill(data);
            let frame_len = channel_buffers.channel(0).len();

            let meas_index = meas_channel
                .load(Ordering::Relaxed)
                .min(channel_buffers.channel_count().saturating_sub(1));
            let measurement_data = channel_buffers.channel(meas_index);

            // Spectrum analysis and input levels are input-only: they must run
            // whether or not the generator is producing a reference, or the
            // analyser goes blank on a live source and clipping goes undetected.
            fft_processor.lock().process(measurement_data);

            // The transfer function needs a reference. Take it without holding
            // any other lock.
            let reference: Option<Vec<f32>> = {
                let mut ref_buf = ref_buffer_for_input.lock();
                if ref_buf.len() >= frame_len {
                    Some(ref_buf.drain(..frame_len).collect())
                } else {
                    None
                }
            };

            let Some(ref_data) = reference else {
                stats.lock().update(measurement_data, &[]);
                return;
            };

            stats.lock().update(measurement_data, &ref_data);

            // Process transfer function
            // Route to whichever resolution mode is active. Both processors keep
            // their own state, so switching modes never loses the other's averages.
            if mw_enabled.load(Ordering::Relaxed) {
                mw_processor.lock().process(measurement_data, &ref_data);
            } else {
                for (index, processor) in tf_processors
                    .iter()
                    .enumerate()
                    .take(channel_buffers.channel_count())
                {
                    processor
                        .lock()
                        .process(channel_buffers.channel(index), &ref_data);
                }
            }
        },
        |err| log::error!("Input stream error: {}", err),
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to build input stream: {}", e);
            is_running.store(false, Ordering::Relaxed);
            return;
        }
    };

    // Build output stream
    let out_channels = output_channels as usize;
    let max_ref_buffer_size = buffer_size as usize * 4; // Keep a few buffers worth

    let output_stream = match output_device.build_output_stream(
        &output_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            // Use Acquire ordering to see any generator configuration changes
            if generator_active.load(Ordering::Acquire) {
                let mut gen = generator.lock();
                let mut ref_samples = Vec::with_capacity(data.len() / out_channels);

                // Fill all channels with the same signal
                for frame in data.chunks_mut(out_channels) {
                    let sample = gen.next_sample();
                    ref_samples.push(sample);
                    for s in frame.iter_mut() {
                        *s = sample;
                    }
                }

                // Store reference samples for transfer function
                let mut ref_buf = reference_buffer.lock();
                ref_buf.extend_from_slice(&ref_samples);
                // Prevent unbounded growth
                let buf_len = ref_buf.len();
                if buf_len > max_ref_buffer_size {
                    ref_buf.drain(..buf_len - max_ref_buffer_size);
                }
            } else {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            }
        },
        |err| log::error!("Output stream error: {}", err),
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to build output stream: {}", e);
            is_running.store(false, Ordering::Relaxed);
            return;
        }
    };

    // Start streams
    if let Err(e) = input_stream.play() {
        log::error!("Failed to start input stream: {}", e);
        is_running.store(false, Ordering::Relaxed);
        return;
    }

    if let Err(e) = output_stream.play() {
        log::error!("Failed to start output stream: {}", e);
        is_running.store(false, Ordering::Relaxed);
        return;
    }

    // Keep thread alive until stop signal
    while !stop_signal.load(Ordering::Relaxed) {
        thread::sleep(std::time::Duration::from_millis(50));
    }

    // Streams are dropped here when the thread exits
    is_running.store(false, Ordering::Relaxed);
}

/// Run audio streams using CoreAudio backend (macOS only)
#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
fn run_coreaudio_streams(
    input_device_id: coreaudio::sys::AudioDeviceID,
    output_device_id: coreaudio::sys::AudioDeviceID,
    sample_rate: u32,
    buffer_size: u32,
    channel_count: usize,
    exclusive_mode: bool,
    fft_processor: Arc<Mutex<FftProcessor>>,
    transfer_function_processors: Vec<Arc<Mutex<TransferFunctionProcessor>>>,
    multi_window_processor: Arc<Mutex<MultiWindowProcessor>>,
    multi_window_enabled: Arc<AtomicBool>,
    measurement_channel: Arc<AtomicUsize>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    reference_buffer: Arc<Mutex<Vec<f32>>>,
    level_stats: Arc<Mutex<LevelStats>>,
) {
    // Set real-time priority for this audio thread
    configure_audio_thread();

    let input_config = CoreAudioConfig {
        sample_rate: sample_rate as f64,
        buffer_size,
        channels: channel_count as u32,
        exclusive_mode,
    };
    let output_config = CoreAudioConfig {
        sample_rate: sample_rate as f64,
        buffer_size,
        channels: 1,
        exclusive_mode,
    };

    // Clone references for input callback
    let tf_processors = transfer_function_processors.clone();
    let mw_processor = Arc::clone(&multi_window_processor);
    let mw_enabled = Arc::clone(&multi_window_enabled);
    let meas_channel = Arc::clone(&measurement_channel);
    let ref_buffer_for_input = Arc::clone(&reference_buffer);
    let fft_proc_clone = Arc::clone(&fft_processor);
    let stats = Arc::clone(&level_stats);

    // Allocated once, up front, sized for the maximum supported channel count
    // so it never needs to grow. Deinterleaving inside the callback below must
    // not allocate: that risks a lock in the allocator and an audible dropout.
    let mut channel_buffers =
        ChannelBuffers::new(channel_count, buffer_size as usize * MAX_INPUT_CHANNELS);

    // Create input stream
    let mut input_stream = match CoreAudioInputStream::new(
        Some(input_device_id),
        input_config,
        move |data: &[f32]| {
            channel_buffers.fill(data);
            let frame_len = channel_buffers.channel(0).len();

            let meas_index = meas_channel
                .load(Ordering::Relaxed)
                .min(channel_buffers.channel_count().saturating_sub(1));
            let measurement_data = channel_buffers.channel(meas_index);

            // Spectrum analysis and input levels are input-only: they must run
            // whether or not the generator is producing a reference, or the
            // analyser goes blank on a live source and clipping goes undetected.
            fft_proc_clone.lock().process(measurement_data);

            // The transfer function needs a reference. Take it without holding
            // any other lock.
            let reference: Option<Vec<f32>> = {
                let mut ref_buf = ref_buffer_for_input.lock();
                if ref_buf.len() >= frame_len {
                    Some(ref_buf.drain(..frame_len).collect())
                } else {
                    None
                }
            };

            let Some(ref_data) = reference else {
                stats.lock().update(measurement_data, &[]);
                return;
            };

            stats.lock().update(measurement_data, &ref_data);

            // Route to whichever resolution mode is active. Both processors keep
            // their own state, so switching modes never loses the other's averages.
            if mw_enabled.load(Ordering::Relaxed) {
                mw_processor.lock().process(measurement_data, &ref_data);
            } else {
                for (index, processor) in tf_processors
                    .iter()
                    .enumerate()
                    .take(channel_buffers.channel_count())
                {
                    processor
                        .lock()
                        .process(channel_buffers.channel(index), &ref_data);
                }
            }
        },
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to create CoreAudio input stream: {}", e);
            is_running.store(false, Ordering::Relaxed);
            return;
        }
    };

    // Clone references for output callback
    let max_ref_buffer_size = buffer_size as usize * 4;

    // Create output stream
    let mut output_stream = match CoreAudioOutputStream::new(
        Some(output_device_id),
        output_config,
        move |data: &mut [f32]| {
            if generator_active.load(Ordering::Acquire) {
                let mut gen = generator.lock();
                let mut ref_samples = Vec::with_capacity(data.len());

                for sample in data.iter_mut() {
                    let s = gen.next_sample();
                    *sample = s;
                    ref_samples.push(s);
                }

                // Store reference samples
                let mut ref_buf = reference_buffer.lock();
                ref_buf.extend_from_slice(&ref_samples);
                let buf_len = ref_buf.len();
                if buf_len > max_ref_buffer_size {
                    ref_buf.drain(..buf_len - max_ref_buffer_size);
                }
            } else {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            }
        },
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to create CoreAudio output stream: {}", e);
            is_running.store(false, Ordering::Relaxed);
            return;
        }
    };

    // Start streams
    if let Err(e) = input_stream.start() {
        log::error!("Failed to start CoreAudio input stream: {}", e);
        is_running.store(false, Ordering::Relaxed);
        return;
    }

    if let Err(e) = output_stream.start() {
        log::error!("Failed to start CoreAudio output stream: {}", e);
        let _ = input_stream.stop();
        is_running.store(false, Ordering::Relaxed);
        return;
    }

    log::info!("CoreAudio streams running");

    // Keep thread alive until stop signal
    while !stop_signal.load(Ordering::Relaxed) {
        thread::sleep(std::time::Duration::from_millis(50));
    }

    // Stop streams
    let _ = input_stream.stop();
    let _ = output_stream.stop();

    is_running.store(false, Ordering::Relaxed);
    log::info!("CoreAudio streams stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulates exactly what the audio callback does: deinterleave a
    /// multi-channel buffer, then route each channel's samples to its own
    /// `TransferFunctionProcessor`. Proves channel 1 (half the gain of
    /// channel 0) ends up in `transfer_function_processors[1]`, not `[0]`.
    #[test]
    fn each_channel_processor_tracks_its_own_signal_independently() {
        let engine = AudioEngine::new();

        let frames = FFT_SIZE * 4;
        let reference: Vec<f32> = (0..frames)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / DEFAULT_SAMPLE_RATE).sin() * 0.5
            })
            .collect();

        // Interleave [ch0, ch1, ch0, ch1, ...]: channel 0 at unity gain,
        // channel 1 at half gain (-6 dB).
        let mut interleaved = Vec::with_capacity(frames * 2);
        for &sample in &reference {
            interleaved.push(sample);
            interleaved.push(sample * 0.5);
        }

        let mut buffers = ChannelBuffers::new(2, frames);
        buffers.fill(&interleaved);

        for (index, processor) in engine
            .transfer_function_processors
            .iter()
            .enumerate()
            .take(2)
        {
            processor.lock().process(buffers.channel(index), &reference);
        }

        let channel_0 = engine.transfer_function_data_for_channel(0);
        let channel_1 = engine.transfer_function_data_for_channel(1);

        let bin_1k = (1000.0 / DEFAULT_SAMPLE_RATE * FFT_SIZE as f32).round() as usize;
        assert!(
            channel_0.magnitude_db[bin_1k].abs() < 3.0,
            "channel 0 should read ~0 dB (unity gain), got {}",
            channel_0.magnitude_db[bin_1k]
        );
        assert!(
            (channel_1.magnitude_db[bin_1k] - (-6.02)).abs() < 2.0,
            "channel 1 should read ~-6 dB (half gain), got {}",
            channel_1.magnitude_db[bin_1k]
        );

        // get_transfer_function_data() must keep channel 0 as its source so
        // every existing caller and test keeps working.
        let default_data = engine.get_transfer_function_data();
        assert_eq!(default_data.magnitude_db, channel_0.magnitude_db);
    }

    #[test]
    fn measurement_channel_is_clamped_to_the_supported_range() {
        let engine = AudioEngine::new();
        engine.set_measurement_channel(3);
        assert_eq!(engine.measurement_channel(), 3);
        engine.set_measurement_channel(99);
        assert_eq!(engine.measurement_channel(), MAX_INPUT_CHANNELS - 1);
    }
}

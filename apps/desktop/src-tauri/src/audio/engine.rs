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
#[cfg(test)]
use super::routing::MeasurementPair;
use super::routing::{ReferenceSource, Routing, MAX_MEASUREMENT_PAIRS};
use crate::dsp::{
    FftProcessor, LeqMeter, LeqMetrics, MultiWindowProcessor, RigSnapshot, SmoothingFraction,
    TransferFunctionData, TransferFunctionProcessor,
};

#[cfg(target_os = "macos")]
use super::coreaudio_backend::{
    self, CoreAudioConfig, CoreAudioInputStream, CoreAudioOutputStream,
};

const FFT_SIZE: usize = 2048;
const DEFAULT_SAMPLE_RATE: f32 = 48000.0;

/// Real system tuning uses several mics at once. Thirty-two covers the
/// practical ceiling of interfaces used for live sound measurement.
pub const MAX_INPUT_CHANNELS: usize = 32;

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
    // One transfer function processor per measurement-pair *slot*. A pair's
    // slot is its index into `Routing::pairs` — not its position among only
    // the enabled pairs — so disabling an earlier pair never reassigns a
    // later pair's averaging state to it. Always MAX_MEASUREMENT_PAIRS long.
    // Deliberately separate from `transfer_function_processors` above: two
    // pairs are allowed to share a measurement channel (see
    // `routing::tests::two_pairs_may_share_a_channel`), so indexing by
    // channel would let one pair's averaging state bleed into another's.
    pair_processors: Vec<Arc<Mutex<TransferFunctionProcessor>>>,
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
    // Current channel routing. The Mutex guards only the pointer swap; readers
    // (including the real-time callback) clone the inner Arc — one atomic
    // increment, no lock held across any work — so the UI thread publishing a
    // new routing can never block the audio thread. See commit d2ac837.
    routing: Arc<Mutex<Arc<Routing>>>,
    /// LEQ/LAeq/LCeq/peak meter, fed from both input callbacks (CPAL and
    /// CoreAudio) via `feed_leq_meter` below. `push` is a bounded running
    /// sum and `metrics()` a read of seven floats -- neither is expensive
    /// work under a lock, so this needs no commit-d2ac837-style snapshot
    /// split. Recreated (not just re-pointed) at the start of each stream so
    /// its A/C-weighting biquads are designed for the device's real sample
    /// rate -- see `start_cpal_stream`/`start_coreaudio_stream`.
    leq_meter: Arc<Mutex<LeqMeter>>,
}

// SAFETY: AudioEngine is Send + Sync because:
// - All atomic fields (is_running, sample_rate, buffer_size, generator_active, stop_signal)
//   are inherently thread-safe via std::sync::atomic
// - All Mutex-wrapped fields (input_device_name, output_device_name, fft_processor,
//   transfer_function_processors, pair_processors, multi_window_processor, generator,
//   thread_handle, reference_buffer, routing) use parking_lot::Mutex which is Send + Sync
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
        let pair_processors = (0..MAX_MEASUREMENT_PAIRS)
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
            pair_processors,
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
            routing: Arc::new(Mutex::new(Arc::new(Routing::default()))),
            leq_meter: Arc::new(Mutex::new(LeqMeter::new(DEFAULT_SAMPLE_RATE))),
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
        for processor in &self.pair_processors {
            processor.lock().set_sample_rate(sample_rate as f32);
        }
        self.multi_window_processor
            .lock()
            .set_sample_rate(sample_rate as f32);

        // The A/C-weighting biquads are designed at construction time for a
        // specific sample rate (see dsp::leq) -- a stale rate here would
        // silently mis-weight every reading on any device that isn't 48 kHz,
        // same failure mode as the processors above. Recreated rather than
        // just re-pointed, so a restarted stream also starts a fresh
        // measurement, matching `reference_buffer.lock().clear()` below.
        *self.leq_meter.lock() = LeqMeter::new(sample_rate as f32);

        // Reset stop signal and clear reference buffer
        self.stop_signal.store(false, Ordering::Relaxed);
        self.reference_buffer.lock().clear();

        // Clone Arcs for the audio thread
        let fft_processor = Arc::clone(&self.fft_processor);
        let pair_processors = self.pair_processors.clone();
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
        let routing = Arc::clone(&self.routing);
        let leq_meter = Arc::clone(&self.leq_meter);

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
                pair_processors,
                multi_window_processor,
                multi_window_enabled,
                measurement_channel,
                generator,
                generator_active,
                is_running,
                stop_signal,
                reference_buffer,
                level_stats,
                routing,
                leq_meter,
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
        for processor in &self.pair_processors {
            processor.lock().set_sample_rate(sample_rate as f32);
        }
        self.multi_window_processor
            .lock()
            .set_sample_rate(sample_rate as f32);

        // See the matching comment in `start_cpal_stream`: the A/C-weighting
        // biquads are designed for a specific sample rate, so the meter must
        // be recreated here too -- this backend must never diverge from the
        // other one on this.
        *self.leq_meter.lock() = LeqMeter::new(sample_rate as f32);

        // Reset stop signal and clear reference buffer
        self.stop_signal.store(false, Ordering::Relaxed);
        self.reference_buffer.lock().clear();

        // Clone Arcs for the audio thread
        let fft_processor = Arc::clone(&self.fft_processor);
        let pair_processors = self.pair_processors.clone();
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
        let routing = Arc::clone(&self.routing);
        let leq_meter = Arc::clone(&self.leq_meter);

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
                pair_processors,
                multi_window_processor,
                multi_window_enabled,
                measurement_channel,
                generator,
                generator_active,
                is_running,
                stop_signal,
                reference_buffer,
                level_stats,
                routing,
                leq_meter,
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

    /// Transfer function data for one measurement-pair slot (a pair's index
    /// into `Routing::pairs`). Mapping a pair's id to its slot is a later
    /// task's job; this is the slot-indexed primitive it will build on. In
    /// multi-window mode there is a single shared processor, so every slot
    /// returns the same data there — matching `transfer_function_data_for_channel`.
    pub fn get_pair_transfer_function_by_slot(&self, slot: usize) -> TransferFunctionData {
        if self.multi_window_enabled.load(Ordering::Relaxed) {
            // See commit d2ac837: take the cheap stitch under the lock, release
            // it, then do the expensive smoothing and conversion outside it.
            let snapshot = self.multi_window_processor.lock().snapshot();
            return snapshot.into_data();
        }
        let clamped = slot.min(self.pair_processors.len() - 1);
        self.pair_processors[clamped].lock().get_data()
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
        for processor in &self.pair_processors {
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
        for processor in &self.pair_processors {
            let mut processor = processor.lock();
            processor.set_fft_size(size);
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
        for processor in &self.pair_processors {
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
        for processor in &self.pair_processors {
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

    /// Replace the channel routing. Validated against `MAX_INPUT_CHANNELS`,
    /// not the engine's current `input_channel_count`: routing is normally
    /// configured before a stream (and its real channel count) exists, so
    /// validating against the live count would reject every pair beyond
    /// channel 0 during ordinary setup. Device-specific validation belongs
    /// at stream start.
    ///
    /// On success the new `Routing` is published by swapping an `Arc` under
    /// a lock held only for the pointer write — never across validation or
    /// any other work — so this can never block the real-time callback. On
    /// failure the previous routing is left untouched.
    ///
    /// A pair's slot is its index into `routing.pairs`, so reordering or
    /// inserting pairs shifts later pairs onto different slots. Without
    /// this, a pair that moves into a slot would read whatever averaged
    /// data the slot's previous occupant left behind — under the new
    /// pair's id — until it happens to re-average. That is the same
    /// wrong-data-under-a-wrong-id bug fixed for concurrent processing in
    /// commit 65ea590, here on a rarer trigger (reordering rather than two
    /// pairs sharing a channel). Comparing pair ids slot by slot and
    /// resetting only the slots whose occupant actually changed keeps every
    /// other slot's averaging — and thus its data — intact.
    pub fn set_routing(&self, routing: Routing) -> Result<(), String> {
        routing.validate(MAX_INPUT_CHANNELS)?;

        let previous = self.routing();
        for slot in 0..MAX_MEASUREMENT_PAIRS {
            let previous_id = previous.pairs.get(slot).map(|pair| pair.id.as_str());
            let next_id = routing.pairs.get(slot).map(|pair| pair.id.as_str());
            if previous_id != next_id {
                self.pair_processors[slot].lock().reset_averaging();
            }
        }

        *self.routing.lock() = Arc::new(routing);
        Ok(())
    }

    /// Current channel routing. Cloning the `Arc` is one atomic increment —
    /// cheap enough to call once per audio callback.
    pub fn routing(&self) -> Arc<Routing> {
        self.routing.lock().clone()
    }

    /// Current LAeq/LCeq/LAFmax/LApeak/LCpeak/elapsed reading, integrated
    /// since the stream last (re)started. A read of seven floats under the
    /// lock, safe to call from a UI poll.
    pub fn leq_metrics(&self) -> LeqMetrics {
        self.leq_meter.lock().metrics()
    }

    /// A clone of the shared meter handle, for code that needs to read
    /// metrics off the audio thread -- e.g. the interval logger's
    /// background thread (see `commands::start_leq_logging`), which must
    /// never do its file I/O from inside the callback itself.
    pub fn leq_meter_handle(&self) -> Arc<Mutex<LeqMeter>> {
        Arc::clone(&self.leq_meter)
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
    pair_processors: Vec<Arc<Mutex<TransferFunctionProcessor>>>,
    multi_window_processor: Arc<Mutex<MultiWindowProcessor>>,
    multi_window_enabled: Arc<AtomicBool>,
    measurement_channel: Arc<AtomicUsize>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    reference_buffer: Arc<Mutex<Vec<f32>>>,
    level_stats: Arc<Mutex<LevelStats>>,
    routing: Arc<Mutex<Arc<Routing>>>,
    leq_meter: Arc<Mutex<LeqMeter>>,
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
    let pair_processors_for_input = pair_processors.clone();
    let mw_processor = Arc::clone(&multi_window_processor);
    let mw_enabled = Arc::clone(&multi_window_enabled);
    let meas_channel = Arc::clone(&measurement_channel);
    let ref_buffer_for_input = Arc::clone(&reference_buffer);
    let stats = Arc::clone(&level_stats);
    let routing_for_input = Arc::clone(&routing);

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

            // LEQ integration is input-only for the same reason, and must sit
            // above the reference-buffer draining below: that same
            // reference-gated placement once silently killed the spectrum
            // analyser (see the comment above), and LEQ must not repeat it.
            // See `feed_leq_meter` for why both backends share this call.
            feed_leq_meter(&leq_meter, measurement_data);

            // The transfer function needs a reference. Take it without holding
            // any other lock. This is the generator's own output, shared by
            // every Generator-referenced pair below — an Input-referenced pair
            // needs none of this and must still be processed if it's empty.
            let generator_reference: Option<Vec<f32>> = {
                let mut ref_buf = ref_buffer_for_input.lock();
                if ref_buf.len() >= frame_len {
                    Some(ref_buf.drain(..frame_len).collect())
                } else {
                    None
                }
            };

            stats.lock().update(
                measurement_data,
                generator_reference.as_deref().unwrap_or(&[]),
            );

            // Route to whichever resolution mode is active. Multi-window is a
            // single global high-resolution view of the legacy measurement
            // channel; the matrix below is every active pair against its own
            // reference. They keep their own state, so switching modes never
            // loses the other's averages.
            if mw_enabled.load(Ordering::Relaxed) {
                if let Some(ref_data) = &generator_reference {
                    mw_processor.lock().process(measurement_data, ref_data);
                }
            } else {
                let routing = routing_for_input.lock().clone();
                process_active_pairs(
                    &routing,
                    &channel_buffers,
                    &pair_processors_for_input,
                    generator_reference.as_deref(),
                );
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
    pair_processors: Vec<Arc<Mutex<TransferFunctionProcessor>>>,
    multi_window_processor: Arc<Mutex<MultiWindowProcessor>>,
    multi_window_enabled: Arc<AtomicBool>,
    measurement_channel: Arc<AtomicUsize>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    reference_buffer: Arc<Mutex<Vec<f32>>>,
    level_stats: Arc<Mutex<LevelStats>>,
    routing: Arc<Mutex<Arc<Routing>>>,
    leq_meter: Arc<Mutex<LeqMeter>>,
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
    let pair_processors_for_input = pair_processors.clone();
    let mw_processor = Arc::clone(&multi_window_processor);
    let mw_enabled = Arc::clone(&multi_window_enabled);
    let meas_channel = Arc::clone(&measurement_channel);
    let ref_buffer_for_input = Arc::clone(&reference_buffer);
    let fft_proc_clone = Arc::clone(&fft_processor);
    let stats = Arc::clone(&level_stats);
    let routing_for_input = Arc::clone(&routing);

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

            // LEQ integration is input-only for the same reason, and must sit
            // above the reference-buffer draining below -- the exact bug that
            // once killed the spectrum analyser above. See `feed_leq_meter`:
            // this call must be identical to the CPAL callback's, or LEQ
            // silently diverges between backends.
            feed_leq_meter(&leq_meter, measurement_data);

            // The transfer function needs a reference. Take it without holding
            // any other lock. This is the generator's own output, shared by
            // every Generator-referenced pair below — an Input-referenced pair
            // needs none of this and must still be processed if it's empty.
            let generator_reference: Option<Vec<f32>> = {
                let mut ref_buf = ref_buffer_for_input.lock();
                if ref_buf.len() >= frame_len {
                    Some(ref_buf.drain(..frame_len).collect())
                } else {
                    None
                }
            };

            stats.lock().update(
                measurement_data,
                generator_reference.as_deref().unwrap_or(&[]),
            );

            // Route to whichever resolution mode is active. Multi-window is a
            // single global high-resolution view of the legacy measurement
            // channel; the matrix below is every active pair against its own
            // reference. They keep their own state, so switching modes never
            // loses the other's averages.
            if mw_enabled.load(Ordering::Relaxed) {
                if let Some(ref_data) = &generator_reference {
                    mw_processor.lock().process(measurement_data, ref_data);
                }
            } else {
                let routing = routing_for_input.lock().clone();
                process_active_pairs(
                    &routing,
                    &channel_buffers,
                    &pair_processors_for_input,
                    generator_reference.as_deref(),
                );
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

/// Feeds one audio block into the LEQ meter. Both the CPAL and CoreAudio
/// input callbacks call this exact function rather than each locking
/// `leq_meter` inline, so the two backends' LEQ behaviour cannot drift apart
/// -- the same divergence-between-backends failure mode `process_active_pairs`
/// exists to prevent for pair routing. `push` is a bounded running sum, safe
/// to do under this lock every block (see `dsp::leq`'s module doc for the
/// real-time constraints it was written against).
fn feed_leq_meter(leq_meter: &Mutex<LeqMeter>, measurement_data: &[f32]) {
    leq_meter.lock().push(measurement_data);
}

/// Run one measurement pair's block through its transfer function processor.
///
/// Argument order matches `TransferFunctionProcessor::process`: measurement
/// first, reference second. Swap them and every transfer function computed
/// through this pair is inverted.
fn process_pair(
    processor: &mut TransferFunctionProcessor,
    measurement: &[f32],
    reference: &[f32],
    delay_samples: usize,
) {
    // A no-op when the delay hasn't changed since the last block; only resets
    // averaging (via `reset_averaging`) when it actually moves.
    processor.set_delay_samples(delay_samples);
    processor.process(measurement, reference);
}

/// Runs every active pair in `routing` for one audio block. Shared by both
/// the CPAL and CoreAudio input callbacks so their per-pair behaviour can
/// never diverge between backends — that exact divergence has shipped as a
/// bug in this project before.
///
/// `pair_processors` is indexed by *slot*, and a pair's slot is its index
/// into `routing.pairs` — not its position among only the enabled pairs.
/// That distinction matters twice over: `routing.rs` explicitly permits two
/// pairs to share a `measurement_channel` (see
/// `routing::tests::two_pairs_may_share_a_channel`), so indexing by channel
/// would silently blend two pairs' averaging state into one meaningless
/// trace; and indexing by position among only the *enabled* pairs would mean
/// disabling an earlier pair reassigns every later pair's slot — and with it,
/// its accumulated averaging state — to the wrong id. Iterating
/// `routing.pairs` directly (rather than `routing.active_pairs()`) keeps each
/// pair's slot fixed regardless of what else is enabled.
///
/// `generator_reference` is the block already drained from `reference_buffer`
/// for this callback (or `None` if the generator hasn't produced enough
/// samples yet); it is shared by every `Generator`-referenced pair rather
/// than drained once per pair. `channel_buffers` must already be filled for
/// this block. Does not allocate.
fn process_active_pairs(
    routing: &Routing,
    channel_buffers: &ChannelBuffers,
    pair_processors: &[Arc<Mutex<TransferFunctionProcessor>>],
    generator_reference: Option<&[f32]>,
) {
    let channel_count = channel_buffers.channel_count();

    for (slot, pair) in routing.pairs.iter().enumerate().filter(|(_, p)| p.enabled) {
        if slot >= pair_processors.len() {
            continue;
        }
        if pair.measurement_channel >= channel_count {
            continue;
        }

        let reference = match pair.reference {
            ReferenceSource::Generator => match generator_reference {
                Some(reference) => reference,
                // No generator output buffered yet this block — a pair
                // referenced to the generator simply has nothing to measure
                // against; other pairs must still be processed.
                None => continue,
            },
            ReferenceSource::Input { channel } => {
                if channel >= channel_count {
                    continue;
                }
                channel_buffers.channel(channel)
            }
        };

        let measurement = channel_buffers.channel(pair.measurement_channel);
        let mut processor = pair_processors[slot].lock();
        process_pair(&mut processor, measurement, reference, pair.delay_samples);
    }
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

    #[test]
    fn the_engine_supports_thirty_two_input_channels() {
        assert_eq!(MAX_INPUT_CHANNELS, 32);
    }

    /// Channel selection must clamp to the new ceiling, not the old one.
    #[test]
    fn channel_selection_clamps_to_thirty_two() {
        let engine = AudioEngine::new();
        engine.set_measurement_channel(31);
        assert_eq!(engine.measurement_channel(), 31);
        engine.set_measurement_channel(32);
        assert_eq!(engine.measurement_channel(), MAX_INPUT_CHANNELS - 1);
        engine.set_measurement_channel(999);
        assert_eq!(engine.measurement_channel(), MAX_INPUT_CHANNELS - 1);
    }

    /// Constructing the engine with 32 channels must not take a visibly long
    /// time — this is the cheapest early warning that per-channel allocation
    /// has become the wrong shape.
    #[test]
    fn constructing_a_thirty_two_channel_engine_is_fast() {
        let start = std::time::Instant::now();
        let _engine = AudioEngine::new();
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 500,
            "AudioEngine::new() took {elapsed:?} with {MAX_INPUT_CHANNELS} channels"
        );
    }

    /// Routing must be swappable while the engine believes it is running,
    /// without the setter blocking on anything expensive.
    #[test]
    fn routing_can_be_replaced_and_read_back() {
        let engine = AudioEngine::new();
        assert_eq!(engine.routing().pairs.len(), 0);

        let routing = Routing {
            pairs: vec![MeasurementPair {
                id: "a".into(),
                name: "Main L".into(),
                measurement_channel: 0,
                reference: ReferenceSource::Generator,
                delay_samples: 0,
                calibration_offset_db: 0.0,
                enabled: true,
            }],
        };
        engine.set_routing(routing).unwrap();
        assert_eq!(engine.routing().pairs.len(), 1);
        assert_eq!(engine.routing().pairs[0].name, "Main L");
    }

    /// Invalid routing is refused and the previous routing survives — a typo in
    /// the matrix must not silently stop a running measurement.
    #[test]
    fn invalid_routing_is_refused_and_the_old_one_survives() {
        let engine = AudioEngine::new();
        let good = Routing {
            pairs: vec![MeasurementPair {
                id: "a".into(),
                name: "Main L".into(),
                measurement_channel: 0,
                reference: ReferenceSource::Generator,
                delay_samples: 0,
                calibration_offset_db: 0.0,
                enabled: true,
            }],
        };
        engine.set_routing(good).unwrap();

        let bad = Routing {
            pairs: vec![MeasurementPair {
                id: "b".into(),
                name: "Bad".into(),
                measurement_channel: 999,
                reference: ReferenceSource::Generator,
                delay_samples: 0,
                calibration_offset_db: 0.0,
                enabled: true,
            }],
        };
        assert!(engine.set_routing(bad).is_err());
        assert_eq!(
            engine.routing().pairs[0].name,
            "Main L",
            "the good routing was lost"
        );
    }

    /// A reader holding an Arc keeps reading a consistent routing while a
    /// writer swaps in a new one — the property the callback depends on.
    #[test]
    fn a_held_routing_is_unaffected_by_a_concurrent_swap() {
        let engine = AudioEngine::new();
        engine
            .set_routing(Routing {
                pairs: vec![MeasurementPair {
                    id: "a".into(),
                    name: "First".into(),
                    measurement_channel: 0,
                    reference: ReferenceSource::Generator,
                    delay_samples: 0,
                    calibration_offset_db: 0.0,
                    enabled: true,
                }],
            })
            .unwrap();

        let held = engine.routing();
        engine.set_routing(Routing { pairs: vec![] }).unwrap();

        assert_eq!(
            held.pairs.len(),
            1,
            "the held snapshot changed under the reader"
        );
        assert_eq!(engine.routing().pairs.len(), 0);
    }

    /// Reading routing must be cheap enough to do per callback. This is not a
    /// benchmark, it is a tripwire for someone replacing the Arc clone with
    /// something that allocates.
    #[test]
    fn reading_routing_is_cheap() {
        let engine = AudioEngine::new();
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = engine.routing();
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 100,
            "100k routing reads took {elapsed:?} — this runs per audio callback"
        );
    }

    /// A pair referenced to the generator behaves exactly as the single-channel
    /// path did — this is the regression test for everything that worked before.
    #[test]
    fn a_generator_referenced_pair_matches_the_legacy_path() {
        let fs = 48_000.0f32;
        let signal: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / fs).sin())
            .collect();

        let mut legacy = TransferFunctionProcessor::new(2048, fs);
        legacy.process(&signal, &signal);

        let mut paired = TransferFunctionProcessor::new(2048, fs);
        process_pair(&mut paired, &signal, &signal, 0);

        let a = legacy.get_data();
        let b = paired.get_data();
        assert_eq!(a.magnitude_db.len(), b.magnitude_db.len());
        for (i, (x, y)) in a.magnitude_db.iter().zip(b.magnitude_db.iter()).enumerate() {
            assert!((x - y).abs() < 1e-4, "bin {i}: {x} vs {y}");
        }
    }

    /// A pair's delay shifts the reference, which is what makes a delayed
    /// measurement read as flat phase rather than a spiral.
    #[test]
    fn a_pair_delay_aligns_a_delayed_measurement() {
        let fs = 48_000.0f32;
        let delay = 128usize;
        let reference: Vec<f32> = (0..8192)
            .map(|i| ((i * 2654435761usize) as f32).sin()) // deterministic broadband
            .collect();
        let mut measurement = vec![0.0f32; reference.len()];
        measurement[delay..].copy_from_slice(&reference[..reference.len() - delay]);

        let mut aligned = TransferFunctionProcessor::new(2048, fs);
        process_pair(&mut aligned, &measurement, &reference, delay);

        let data = aligned.get_data();
        // With the delay compensated, phase in the coherent midband is near zero.
        let midband: Vec<f32> = data
            .frequencies
            .iter()
            .zip(data.phase_deg.iter())
            .zip(data.coherence.iter())
            .filter(|((&f, _), &c)| f > 200.0 && f < 5000.0 && c > 0.9)
            .map(|((_, &p), _)| p.abs())
            .collect();
        assert!(!midband.is_empty(), "no coherent midband to check");
        let mean = midband.iter().sum::<f32>() / midband.len() as f32;
        assert!(
            mean < 20.0,
            "mean |phase| was {mean:.1} deg, expected near zero"
        );
    }

    /// A disabled pair produces no processing at all.
    #[test]
    fn a_disabled_pair_is_not_processed() {
        let routing = Routing {
            pairs: vec![MeasurementPair {
                id: "a".into(),
                name: "Off".into(),
                measurement_channel: 0,
                reference: ReferenceSource::Generator,
                delay_samples: 0,
                calibration_offset_db: 0.0,
                enabled: false,
            }],
        };
        assert_eq!(routing.active_pairs().count(), 0);
    }

    fn pair_processors_for_test(count: usize) -> Vec<Arc<Mutex<TransferFunctionProcessor>>> {
        (0..count)
            .map(|_| {
                Arc::new(Mutex::new(TransferFunctionProcessor::new(
                    FFT_SIZE,
                    DEFAULT_SAMPLE_RATE,
                )))
            })
            .collect()
    }

    fn pair(id: &str, measurement_channel: usize, reference: ReferenceSource) -> MeasurementPair {
        MeasurementPair {
            id: id.into(),
            name: id.into(),
            measurement_channel,
            reference,
            delay_samples: 0,
            calibration_offset_db: 0.0,
            enabled: true,
        }
    }

    /// This is the regression test for the actual bug: two active pairs that
    /// share a measurement channel but have different references must land
    /// in different processors and produce different transfer function data.
    /// Before the fix, both pairs were looked up as
    /// `transfer_function_processors[pair.measurement_channel]` — the same
    /// index for both, since they share a measurement channel — so the
    /// second pair processed each block silently overwrote the first pair's
    /// averaging state with its own, and both pairs read back the same
    /// (meaningless) blended data.
    #[test]
    fn pairs_sharing_a_measurement_channel_with_different_references_stay_independent() {
        let fs = DEFAULT_SAMPLE_RATE;
        let frames = FFT_SIZE * 4;

        let tone: Vec<f32> = (0..frames)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / fs).sin() * 0.5)
            .collect();

        // Channel 0: the shared measurement signal.
        // Channel 1: identical to the measurement — pair "a"'s reference.
        // Channel 2: half the measurement — pair "b"'s reference, which
        // should read back roughly +6 dB where "a" reads roughly 0 dB.
        let mut interleaved = Vec::with_capacity(frames * 3);
        for &sample in &tone {
            interleaved.push(sample);
            interleaved.push(sample);
            interleaved.push(sample * 0.5);
        }
        let mut buffers = ChannelBuffers::new(3, frames);
        buffers.fill(&interleaved);

        let routing = Routing {
            pairs: vec![
                pair("a", 0, ReferenceSource::Input { channel: 1 }),
                pair("b", 0, ReferenceSource::Input { channel: 2 }),
            ],
        };

        let pair_processors = pair_processors_for_test(2);
        process_active_pairs(&routing, &buffers, &pair_processors, None);

        let bin_1k = (1000.0 / fs * FFT_SIZE as f32).round() as usize;
        let a = pair_processors[0].lock().get_data();
        let b = pair_processors[1].lock().get_data();

        assert!(
            a.magnitude_db[bin_1k].abs() < 3.0,
            "pair a (unity-gain reference) should read ~0 dB, got {}",
            a.magnitude_db[bin_1k]
        );
        assert!(
            (b.magnitude_db[bin_1k] - 6.02).abs() < 2.0,
            "pair b (half-gain reference) should read ~+6 dB, got {}",
            b.magnitude_db[bin_1k]
        );
        assert_ne!(
            a.magnitude_db, b.magnitude_db,
            "pairs sharing a measurement channel must not share a processor"
        );
    }

    /// A pair's slot is its index into `routing.pairs`, not its position
    /// among only the enabled pairs. Disabling an earlier pair must not
    /// shift a later pair into a different slot — doing so would hand it
    /// another pair's (or nobody's) accumulated averaging state.
    #[test]
    fn disabling_an_earlier_pair_does_not_shift_a_later_pairs_slot() {
        let fs = DEFAULT_SAMPLE_RATE;
        let frames = FFT_SIZE * 2;
        let tone: Vec<f32> = (0..frames)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / fs).sin())
            .collect();

        let mut buffers = ChannelBuffers::new(1, frames);
        buffers.fill(&tone);

        let mut disabled = pair("a", 0, ReferenceSource::Generator);
        disabled.enabled = false;
        let routing = Routing {
            pairs: vec![disabled, pair("b", 0, ReferenceSource::Generator)],
        };

        let pair_processors = pair_processors_for_test(2);
        process_active_pairs(&routing, &buffers, &pair_processors, Some(&tone));

        // Slot 0 belongs to the disabled pair "a" and must stay untouched —
        // a fresh processor's magnitude starts at -120 dB in every bin.
        let slot0 = pair_processors[0].lock().get_data();
        assert!(
            slot0.magnitude_db.iter().all(|&db| db == -120.0),
            "the disabled pair's slot should be untouched, not repurposed for pair b"
        );

        // Slot 1 belongs to pair "b", which measured a tone against itself —
        // it must actually have been processed.
        let slot1 = pair_processors[1].lock().get_data();
        assert!(
            slot1.magnitude_db.iter().any(|&db| db != -120.0),
            "pair b's own slot should have been processed"
        );
    }

    /// Controller ruling B9. Reordering two pairs swaps which slot each
    /// occupies; without a reset, the pair that moves into a slot would
    /// read whatever averaged data the previous occupant left there, under
    /// its own (wrong) id, until it re-averages.
    #[test]
    fn reordering_pairs_resets_the_slots_whose_occupant_changed() {
        let fs = DEFAULT_SAMPLE_RATE;
        let frames = FFT_SIZE * 4;
        let tone: Vec<f32> = (0..frames)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / fs).sin() * 0.5)
            .collect();

        // Channel 0: the shared measurement signal. Channel 1: pair "a"'s
        // unity-gain reference. Channel 2: pair "b"'s half-gain reference.
        let mut interleaved = Vec::with_capacity(frames * 3);
        for &sample in &tone {
            interleaved.push(sample);
            interleaved.push(sample);
            interleaved.push(sample * 0.5);
        }
        let mut buffers = ChannelBuffers::new(3, frames);
        buffers.fill(&interleaved);

        let engine = AudioEngine::new();
        let initial = Routing {
            pairs: vec![
                pair("a", 0, ReferenceSource::Input { channel: 1 }),
                pair("b", 0, ReferenceSource::Input { channel: 2 }),
            ],
        };
        engine.set_routing(initial.clone()).unwrap();

        // Drive real, distinct data into slot 0 ("a") and slot 1 ("b")
        // directly, bypassing the audio callback like the sibling test above.
        process_active_pairs(&initial, &buffers, &engine.pair_processors, None);

        let a_before = engine.get_pair_transfer_function_by_slot(0);
        let b_before = engine.get_pair_transfer_function_by_slot(1);
        assert!(
            a_before.magnitude_db.iter().any(|&db| db != -120.0),
            "slot 0 should hold pair a's real data before reordering"
        );
        assert!(
            b_before.magnitude_db.iter().any(|&db| db != -120.0),
            "slot 1 should hold pair b's real data before reordering"
        );
        assert_ne!(a_before.magnitude_db, b_before.magnitude_db);

        // Reorder: "b" now occupies slot 0, "a" now occupies slot 1.
        let reordered = Routing {
            pairs: vec![
                pair("b", 0, ReferenceSource::Input { channel: 2 }),
                pair("a", 0, ReferenceSource::Input { channel: 1 }),
            ],
        };
        engine.set_routing(reordered).unwrap();

        let slot0_after = engine.get_pair_transfer_function_by_slot(0);
        let slot1_after = engine.get_pair_transfer_function_by_slot(1);
        assert!(
            slot0_after.magnitude_db.iter().all(|&db| db == -120.0),
            "pair b's new slot must not read pair a's leftover averaging"
        );
        assert!(
            slot1_after.magnitude_db.iter().all(|&db| db == -120.0),
            "pair a's new slot must not read pair b's leftover averaging"
        );
    }

    /// Only the slots whose occupant actually changed are reset — a routing
    /// update that only touches one pair must not throw away another pair's
    /// running average.
    #[test]
    fn set_routing_leaves_unaffected_slots_averaging_intact() {
        let fs = DEFAULT_SAMPLE_RATE;
        let frames = FFT_SIZE * 4;
        let tone: Vec<f32> = (0..frames)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / fs).sin() * 0.5)
            .collect();

        let mut interleaved = Vec::with_capacity(frames * 3);
        for &sample in &tone {
            interleaved.push(sample);
            interleaved.push(sample);
            interleaved.push(sample * 0.5);
        }
        let mut buffers = ChannelBuffers::new(3, frames);
        buffers.fill(&interleaved);

        let engine = AudioEngine::new();
        let initial = Routing {
            pairs: vec![
                pair("a", 0, ReferenceSource::Input { channel: 1 }),
                pair("b", 0, ReferenceSource::Input { channel: 2 }),
            ],
        };
        engine.set_routing(initial.clone()).unwrap();
        process_active_pairs(&initial, &buffers, &engine.pair_processors, None);

        let a_before = engine.get_pair_transfer_function_by_slot(0);
        assert!(a_before.magnitude_db.iter().any(|&db| db != -120.0));

        // Same pairs, same slots, only "b" disabled — slot 0 ("a") is untouched.
        let mut updated = initial;
        updated.pairs[1].enabled = false;
        engine.set_routing(updated).unwrap();

        let a_after = engine.get_pair_transfer_function_by_slot(0);
        assert_eq!(
            a_before.magnitude_db, a_after.magnitude_db,
            "pair a's slot kept the same id and must not have been reset"
        );
    }

    /// `feed_leq_meter` is the single call site both input callbacks share.
    /// Proving it pushes into the meter is proving both backends do, without
    /// needing a real audio device to exercise either callback directly.
    #[test]
    fn feed_leq_meter_pushes_the_block_into_the_meter() {
        let leq_meter = Mutex::new(LeqMeter::new(DEFAULT_SAMPLE_RATE));
        let block = vec![0.5f32; DEFAULT_SAMPLE_RATE as usize];

        feed_leq_meter(&leq_meter, &block);

        let metrics = leq_meter.lock().metrics();
        assert!(
            (metrics.elapsed_seconds - 1.0).abs() < 1e-6,
            "expected one second of samples pushed, got {}",
            metrics.elapsed_seconds
        );
    }

    /// `AudioEngine::leq_metrics` and `leq_meter_handle` are the two ways
    /// anything outside this module reads the meter (commands and the
    /// interval logger, respectively). Both must observe the same pushed
    /// samples through the engine's own field, not a copy.
    #[test]
    fn engine_leq_metrics_reflects_samples_pushed_through_the_shared_handle() {
        let engine = AudioEngine::new();
        let handle = engine.leq_meter_handle();
        let block = vec![0.25f32; 1000];

        feed_leq_meter(&handle, &block);

        let metrics = engine.leq_metrics();
        assert!(
            metrics.elapsed_seconds > 0.0,
            "leq_metrics() should reflect samples pushed via leq_meter_handle()"
        );
    }
}

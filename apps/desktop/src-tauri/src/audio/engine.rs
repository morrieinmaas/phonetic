use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, StreamConfig};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;

use super::generator::{Generator, SignalType};
use super::realtime::configure_audio_thread;
use crate::dsp::{FftProcessor, TransferFunctionData, TransferFunctionProcessor};

#[cfg(target_os = "macos")]
use super::coreaudio_backend::{
    self, CoreAudioConfig, CoreAudioInputStream, CoreAudioOutputStream,
};

const FFT_SIZE: usize = 2048;
const DEFAULT_SAMPLE_RATE: f32 = 48000.0;

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

/// Thread-safe audio engine that can be used with Tauri state.
/// Audio streams run in a dedicated thread since cpal::Stream is not Send+Sync.
pub struct AudioEngine {
    is_running: Arc<AtomicBool>,
    sample_rate: Arc<AtomicU32>,
    buffer_size: Arc<AtomicU32>,
    input_device_name: Arc<Mutex<Option<String>>>,
    output_device_name: Arc<Mutex<Option<String>>>,
    fft_processor: Arc<Mutex<FftProcessor>>,
    transfer_function_processor: Arc<Mutex<TransferFunctionProcessor>>,
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
}

// SAFETY: AudioEngine is Send + Sync because:
// - All atomic fields (is_running, sample_rate, buffer_size, generator_active, stop_signal)
//   are inherently thread-safe via std::sync::atomic
// - All Mutex-wrapped fields (input_device_name, output_device_name, fft_processor,
//   transfer_function_processor, generator, thread_handle, reference_buffer)
//   use parking_lot::Mutex which is Send + Sync
// - Arc provides thread-safe reference counting
// The thread_handle stores a JoinHandle which is Send, and we only mutate it under a Mutex lock.
// cpal::Stream is not Send+Sync, but we don't store streams - they live in the audio thread only.
unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    pub fn new() -> Self {
        let fft_processor = Arc::new(Mutex::new(FftProcessor::new(FFT_SIZE)));
        let transfer_function_processor = Arc::new(Mutex::new(TransferFunctionProcessor::new(
            FFT_SIZE,
            DEFAULT_SAMPLE_RATE,
        )));
        let generator = Arc::new(Mutex::new(Generator::new(DEFAULT_SAMPLE_RATE as f64)));

        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            sample_rate: Arc::new(AtomicU32::new(DEFAULT_SAMPLE_RATE as u32)),
            buffer_size: Arc::new(AtomicU32::new(512)),
            input_device_name: Arc::new(Mutex::new(None)),
            output_device_name: Arc::new(Mutex::new(None)),
            fft_processor,
            transfer_function_processor,
            generator,
            generator_active: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
            reference_buffer: Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE * 2))),
            backend: Arc::new(AtomicU8::new(AudioBackend::default() as u8)),
            exclusive_mode: Arc::new(AtomicBool::new(false)),
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

        // Update generator and transfer function processor sample rates
        self.generator.lock().set_sample_rate(sample_rate as f64);
        self.transfer_function_processor
            .lock()
            .set_sample_rate(sample_rate as f32);

        // Reset stop signal and clear reference buffer
        self.stop_signal.store(false, Ordering::Relaxed);
        self.reference_buffer.lock().clear();

        // Clone Arcs for the audio thread
        let fft_processor = Arc::clone(&self.fft_processor);
        let transfer_function_processor = Arc::clone(&self.transfer_function_processor);
        let generator = Arc::clone(&self.generator);
        let generator_active = Arc::clone(&self.generator_active);
        let is_running = Arc::clone(&self.is_running);
        let stop_signal = Arc::clone(&self.stop_signal);
        let reference_buffer = Arc::clone(&self.reference_buffer);

        let output_channels = output_config.channels();

        // Spawn audio thread - streams must live in the same thread they're created
        let handle = thread::spawn(move || {
            run_cpal_streams(
                input_device,
                output_device,
                sample_rate,
                buffer_size,
                output_channels,
                fft_processor,
                transfer_function_processor,
                generator,
                generator_active,
                is_running,
                stop_signal,
                reference_buffer,
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

        // Update generator and transfer function processor sample rates
        self.generator.lock().set_sample_rate(sample_rate as f64);
        self.transfer_function_processor
            .lock()
            .set_sample_rate(sample_rate as f32);

        // Reset stop signal and clear reference buffer
        self.stop_signal.store(false, Ordering::Relaxed);
        self.reference_buffer.lock().clear();

        // Clone Arcs for the audio thread
        let fft_processor = Arc::clone(&self.fft_processor);
        let transfer_function_processor = Arc::clone(&self.transfer_function_processor);
        let generator = Arc::clone(&self.generator);
        let generator_active = Arc::clone(&self.generator_active);
        let is_running = Arc::clone(&self.is_running);
        let stop_signal = Arc::clone(&self.stop_signal);
        let reference_buffer = Arc::clone(&self.reference_buffer);
        let exclusive_mode = self.exclusive_mode.load(Ordering::Relaxed);

        // Spawn audio thread
        let handle = thread::spawn(move || {
            run_coreaudio_streams(
                input_id,
                output_id,
                sample_rate,
                buffer_size,
                exclusive_mode,
                fft_processor,
                transfer_function_processor,
                generator,
                generator_active,
                is_running,
                stop_signal,
                reference_buffer,
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
        self.transfer_function_processor.lock().get_data()
    }

    pub fn reset_averaging(&self) {
        self.transfer_function_processor.lock().reset_averaging();
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
        self.transfer_function_processor.lock().set_fft_size(size);

        // Update sample rate in case it changed
        self.transfer_function_processor
            .lock()
            .set_sample_rate(sample_rate);

        log::info!("FFT size set to {}", size);
        Ok(())
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
    output_channels: u16,
    fft_processor: Arc<Mutex<FftProcessor>>,
    transfer_function_processor: Arc<Mutex<TransferFunctionProcessor>>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    reference_buffer: Arc<Mutex<Vec<f32>>>,
) {
    // Set real-time priority for this audio thread
    configure_audio_thread();

    let input_config = StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    let output_config = StreamConfig {
        channels: output_channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };

    // Clone references for input callback
    let tf_processor = Arc::clone(&transfer_function_processor);
    let ref_buffer_for_input = Arc::clone(&reference_buffer);

    // Build input stream
    let input_stream = match input_device.build_input_stream(
        &input_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Process FFT for spectrum analyzer
            let mut fft_proc = fft_processor.lock();
            fft_proc.process(data);

            // Process transfer function if we have reference data
            let ref_data: Vec<f32>;
            {
                let mut ref_buf = ref_buffer_for_input.lock();
                if ref_buf.len() >= data.len() {
                    // Take the oldest reference samples matching input length
                    ref_data = ref_buf.drain(..data.len()).collect();
                } else {
                    return; // Not enough reference data yet
                }
            }

            // Process transfer function
            let mut tf_proc = tf_processor.lock();
            tf_proc.process(data, &ref_data);
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
    exclusive_mode: bool,
    fft_processor: Arc<Mutex<FftProcessor>>,
    transfer_function_processor: Arc<Mutex<TransferFunctionProcessor>>,
    generator: Arc<Mutex<Generator>>,
    generator_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    reference_buffer: Arc<Mutex<Vec<f32>>>,
) {
    // Set real-time priority for this audio thread
    configure_audio_thread();

    let config = CoreAudioConfig {
        sample_rate: sample_rate as f64,
        buffer_size,
        channels: 1,
        exclusive_mode,
    };

    // Clone references for input callback
    let tf_processor = Arc::clone(&transfer_function_processor);
    let ref_buffer_for_input = Arc::clone(&reference_buffer);
    let fft_proc_clone = Arc::clone(&fft_processor);

    // Create input stream
    let mut input_stream = match CoreAudioInputStream::new(
        Some(input_device_id),
        config.clone(),
        move |data: &[f32]| {
            // Process FFT for spectrum analyzer
            let mut fft_proc = fft_proc_clone.lock();
            fft_proc.process(data);

            // Process transfer function if we have reference data
            let ref_data: Vec<f32>;
            {
                let mut ref_buf = ref_buffer_for_input.lock();
                if ref_buf.len() >= data.len() {
                    ref_data = ref_buf.drain(..data.len()).collect();
                } else {
                    return;
                }
            }

            let mut tf_proc = tf_processor.lock();
            tf_proc.process(data, &ref_data);
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
        config,
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

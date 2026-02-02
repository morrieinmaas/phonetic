use tauri::State;

use crate::audio::{enumerate_devices, AudioDeviceList, AudioEngine, AudioState, SignalType};

/// Frequency range for audio generation (human hearing range)
const MIN_FREQUENCY: f64 = 20.0;
const MAX_FREQUENCY: f64 = 20000.0;

/// Amplitude range (0.0 to 1.0)
const MIN_AMPLITUDE: f64 = 0.0;
const MAX_AMPLITUDE: f64 = 1.0;

#[tauri::command]
pub fn list_audio_devices() -> AudioDeviceList {
    enumerate_devices()
}

#[tauri::command]
pub fn get_audio_state(engine: State<'_, AudioEngine>) -> AudioState {
    engine.get_state()
}

#[tauri::command]
pub fn start_audio_stream(
    engine: State<'_, AudioEngine>,
    input_device_id: Option<String>,
    output_device_id: Option<String>,
) -> Result<(), String> {
    // Validate device IDs if provided
    if let Some(ref id) = input_device_id {
        if id.is_empty() {
            return Err("Input device ID cannot be empty".to_string());
        }
    }
    if let Some(ref id) = output_device_id {
        if id.is_empty() {
            return Err("Output device ID cannot be empty".to_string());
        }
    }

    engine.start_stream(input_device_id.as_deref(), output_device_id.as_deref())
}

#[tauri::command]
pub fn stop_audio_stream(engine: State<'_, AudioEngine>) {
    engine.stop_stream()
}

#[tauri::command]
pub fn start_generator(
    engine: State<'_, AudioEngine>,
    signal_type: SignalType,
    frequency: f64,
    amplitude: f64,
) -> Result<(), String> {
    // Validate frequency
    if !frequency.is_finite() {
        return Err("Frequency must be a finite number".to_string());
    }
    if !(MIN_FREQUENCY..=MAX_FREQUENCY).contains(&frequency) {
        return Err(format!(
            "Frequency must be between {} and {} Hz",
            MIN_FREQUENCY, MAX_FREQUENCY
        ));
    }

    // Validate amplitude
    if !amplitude.is_finite() {
        return Err("Amplitude must be a finite number".to_string());
    }
    if !(MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&amplitude) {
        return Err(format!(
            "Amplitude must be between {} and {}",
            MIN_AMPLITUDE, MAX_AMPLITUDE
        ));
    }

    engine.start_generator(signal_type, frequency, amplitude);
    Ok(())
}

#[tauri::command]
pub fn stop_generator(engine: State<'_, AudioEngine>) {
    engine.stop_generator()
}

#[tauri::command]
pub fn get_spectrum_data(engine: State<'_, AudioEngine>) -> Vec<f32> {
    engine.get_spectrum_data()
}

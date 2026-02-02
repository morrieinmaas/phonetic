use tauri::State;

use crate::audio::{
    enumerate_devices, AudioBackend, AudioDeviceList, AudioEngine, AudioState, SignalType,
};
use crate::dsp::{
    export_impulse_response_csv, export_impulse_response_json, export_impulse_response_txt,
    export_transfer_function_csv, export_transfer_function_json, export_transfer_function_txt,
    write_to_file, DelayFinder, DelayResult, ExportFormat, ImpulseResponseData,
    ImpulseResponseProcessor, MeasurementMetadata, MicrophoneCalibration, SplCalculator,
    TransferFunctionData,
};
use parking_lot::Mutex;

/// Frequency range for audio generation (human hearing range)
const MIN_FREQUENCY: f64 = 20.0;
const MAX_FREQUENCY: f64 = 20000.0;

/// Amplitude range (0.0 to 1.0)
const MIN_AMPLITUDE: f64 = 0.0;
const MAX_AMPLITUDE: f64 = 1.0;

const FFT_SIZE: usize = 2048;
const DEFAULT_SAMPLE_RATE: f32 = 48000.0;

/// DSP state for delay finder, impulse response, and calibration
pub struct DspState {
    pub delay_finder: Mutex<DelayFinder>,
    pub impulse_response: Mutex<ImpulseResponseProcessor>,
    pub spl_calculator: Mutex<SplCalculator>,
}

impl DspState {
    pub fn new() -> Self {
        Self {
            delay_finder: Mutex::new(DelayFinder::new(FFT_SIZE * 2, DEFAULT_SAMPLE_RATE, 500.0)),
            impulse_response: Mutex::new(ImpulseResponseProcessor::new(
                FFT_SIZE * 4,
                DEFAULT_SAMPLE_RATE,
            )),
            spl_calculator: Mutex::new(SplCalculator::default()),
        }
    }
}

impl Default for DspState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub fn list_audio_devices() -> AudioDeviceList {
    enumerate_devices()
}

#[tauri::command]
pub fn get_audio_state(engine: State<'_, AudioEngine>) -> AudioState {
    engine.get_state()
}

#[tauri::command]
pub fn set_audio_backend(engine: State<'_, AudioEngine>, backend: AudioBackend) -> Result<(), String> {
    engine.set_backend(backend)
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

#[tauri::command]
pub fn get_transfer_function_data(engine: State<'_, AudioEngine>) -> TransferFunctionData {
    engine.get_transfer_function_data()
}

#[tauri::command]
pub fn reset_averaging(engine: State<'_, AudioEngine>) {
    engine.reset_averaging()
}

#[tauri::command]
pub fn set_fft_size(engine: State<'_, AudioEngine>, size: usize) -> Result<(), String> {
    // Validate FFT size
    if !size.is_power_of_two() {
        return Err("FFT size must be a power of 2".to_string());
    }
    if !(512..=16384).contains(&size) {
        return Err("FFT size must be between 512 and 16384".to_string());
    }

    engine.set_fft_size(size)
}

// ============================================================================
// Delay Finder Commands
// ============================================================================

#[tauri::command]
pub fn find_delay(
    dsp: State<'_, DspState>,
    reference: Vec<f32>,
    measured: Vec<f32>,
) -> DelayResult {
    dsp.delay_finder.lock().find_delay(&reference, &measured)
}

#[tauri::command]
pub fn set_delay_finder_max_delay(dsp: State<'_, DspState>, max_delay_ms: f32) {
    dsp.delay_finder.lock().set_max_delay_ms(max_delay_ms);
}

#[tauri::command]
pub fn set_delay_finder_threshold(dsp: State<'_, DspState>, threshold: f32) {
    dsp.delay_finder.lock().set_correlation_threshold(threshold);
}

// ============================================================================
// Impulse Response Commands
// ============================================================================

#[tauri::command]
pub fn start_impulse_capture(dsp: State<'_, DspState>) {
    dsp.impulse_response.lock().start_capture();
}

#[tauri::command]
pub fn stop_impulse_capture(dsp: State<'_, DspState>) -> bool {
    dsp.impulse_response.lock().stop_capture()
}

#[tauri::command]
pub fn add_impulse_samples(dsp: State<'_, DspState>, reference: Vec<f32>, measured: Vec<f32>) {
    dsp.impulse_response
        .lock()
        .add_samples(&reference, &measured);
}

#[tauri::command]
pub fn get_impulse_capture_progress(dsp: State<'_, DspState>) -> f32 {
    dsp.impulse_response.lock().capture_progress()
}

#[tauri::command]
pub fn is_impulse_capture_complete(dsp: State<'_, DspState>) -> bool {
    dsp.impulse_response.lock().is_capture_complete()
}

#[tauri::command]
pub fn compute_impulse_response(dsp: State<'_, DspState>) -> Option<ImpulseResponseData> {
    dsp.impulse_response.lock().compute_impulse_response()
}

#[tauri::command]
pub fn clear_impulse_capture(dsp: State<'_, DspState>) {
    dsp.impulse_response.lock().clear();
}

// ============================================================================
// SPL Calibration Commands
// ============================================================================

#[tauri::command]
pub fn load_calibration_file(
    dsp: State<'_, DspState>,
    content: String,
    name: String,
) -> Result<(), String> {
    let calibration = MicrophoneCalibration::from_cal_file(&content, &name)?;
    dsp.spl_calculator.lock().set_calibration(calibration);
    Ok(())
}

#[tauri::command]
pub fn set_calibration_sensitivity(dsp: State<'_, DspState>, sensitivity_db: f32) {
    let mut calc = dsp.spl_calculator.lock();
    let mut cal = calc.calibration().clone();
    cal.sensitivity_db = sensitivity_db;
    calc.set_calibration(cal);
}

#[tauri::command]
pub fn set_preamp_gain(dsp: State<'_, DspState>, gain_db: f32) {
    dsp.spl_calculator.lock().set_preamp_gain(gain_db);
}

#[tauri::command]
pub fn set_a_weighting(dsp: State<'_, DspState>, enabled: bool) {
    dsp.spl_calculator.lock().set_a_weighting(enabled);
}

#[tauri::command]
pub fn dbfs_to_spl(dsp: State<'_, DspState>, dbfs: f32) -> f32 {
    dsp.spl_calculator.lock().dbfs_to_spl(dbfs)
}

#[tauri::command]
pub fn calibrate_with_reference(
    dsp: State<'_, DspState>,
    measured_dbfs: f32,
    reference_spl_db: f32,
) {
    dsp.spl_calculator
        .lock()
        .calibrate_with_reference(measured_dbfs, reference_spl_db);
}

#[tauri::command]
pub fn get_calibration_info(dsp: State<'_, DspState>) -> MicrophoneCalibration {
    dsp.spl_calculator.lock().calibration().clone()
}

// ============================================================================
// Export Commands
// ============================================================================

#[tauri::command]
pub fn export_transfer_function(
    engine: State<'_, AudioEngine>,
    format: ExportFormat,
    file_path: String,
    name: Option<String>,
    notes: Option<String>,
) -> Result<(), String> {
    let data = engine.get_transfer_function_data();
    let state = engine.get_state();

    let metadata = MeasurementMetadata {
        name: name.unwrap_or_else(|| "Transfer Function".to_string()),
        sample_rate: state.sample_rate as f32,
        fft_size: 2048, // TODO: get from engine
        notes,
        ..Default::default()
    };

    let content = match format {
        ExportFormat::Csv => export_transfer_function_csv(&data, &metadata),
        ExportFormat::Json => export_transfer_function_json(&data, &metadata)
            .map_err(|e| format!("JSON serialization error: {}", e))?,
        ExportFormat::Txt => export_transfer_function_txt(&data, &metadata),
    };

    write_to_file(&file_path, &content).map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub fn export_impulse_response(
    dsp: State<'_, DspState>,
    format: ExportFormat,
    file_path: String,
    name: Option<String>,
    notes: Option<String>,
) -> Result<(), String> {
    let data = dsp
        .impulse_response
        .lock()
        .compute_impulse_response()
        .ok_or("No impulse response data available")?;

    let metadata = MeasurementMetadata {
        name: name.unwrap_or_else(|| "Impulse Response".to_string()),
        sample_rate: data.sample_rate,
        notes,
        ..Default::default()
    };

    let content = match format {
        ExportFormat::Csv => export_impulse_response_csv(&data, &metadata),
        ExportFormat::Json => export_impulse_response_json(&data, &metadata)
            .map_err(|e| format!("JSON serialization error: {}", e))?,
        ExportFormat::Txt => export_impulse_response_txt(&data, &metadata),
    };

    write_to_file(&file_path, &content).map_err(|e| format!("Failed to write file: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_validation() {
        // Valid frequencies should not error
        assert!((MIN_FREQUENCY..=MAX_FREQUENCY).contains(&20.0));
        assert!((MIN_FREQUENCY..=MAX_FREQUENCY).contains(&1000.0));
        assert!((MIN_FREQUENCY..=MAX_FREQUENCY).contains(&20000.0));

        // Invalid frequencies
        assert!(!(MIN_FREQUENCY..=MAX_FREQUENCY).contains(&19.9));
        assert!(!(MIN_FREQUENCY..=MAX_FREQUENCY).contains(&20001.0));
    }

    #[test]
    fn test_amplitude_validation() {
        // Valid amplitudes
        assert!((MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&0.0));
        assert!((MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&0.5));
        assert!((MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&1.0));

        // Invalid amplitudes
        assert!(!(MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&-0.1));
        assert!(!(MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&1.1));
    }

    #[test]
    fn test_fft_size_validation() {
        // Valid FFT sizes (powers of 2 in range)
        assert!(512usize.is_power_of_two() && (512..=16384).contains(&512));
        assert!(1024usize.is_power_of_two() && (512..=16384).contains(&1024));
        assert!(2048usize.is_power_of_two() && (512..=16384).contains(&2048));
        assert!(4096usize.is_power_of_two() && (512..=16384).contains(&4096));
        assert!(8192usize.is_power_of_two() && (512..=16384).contains(&8192));
        assert!(16384usize.is_power_of_two() && (512..=16384).contains(&16384));

        // Invalid: not power of 2
        assert!(!1000usize.is_power_of_two());

        // Invalid: out of range
        assert!(!(512..=16384).contains(&256));
        assert!(!(512..=16384).contains(&32768));
    }
}

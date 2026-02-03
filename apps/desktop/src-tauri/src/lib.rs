mod audio;
mod commands;
pub mod dsp;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            log::info!("Phonetic starting up...");

            // Initialize audio engine
            let audio_engine = audio::AudioEngine::new();
            app.manage(audio_engine);

            // Initialize DSP state for delay finder, impulse response, and calibration
            let dsp_state = commands::DspState::new();
            app.manage(dsp_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Audio device and stream commands
            commands::list_audio_devices,
            commands::get_audio_state,
            commands::set_audio_backend,
            commands::set_exclusive_mode,
            commands::get_exclusive_mode,
            commands::start_audio_stream,
            commands::stop_audio_stream,
            commands::start_generator,
            commands::stop_generator,
            commands::get_spectrum_data,
            commands::get_transfer_function_data,
            commands::reset_averaging,
            commands::set_fft_size,
            // Delay finder commands
            commands::find_delay,
            commands::set_delay_finder_max_delay,
            commands::set_delay_finder_threshold,
            // Impulse response commands
            commands::start_impulse_capture,
            commands::stop_impulse_capture,
            commands::add_impulse_samples,
            commands::get_impulse_capture_progress,
            commands::is_impulse_capture_complete,
            commands::compute_impulse_response,
            commands::clear_impulse_capture,
            // SPL calibration commands
            commands::load_calibration_file,
            commands::set_calibration_sensitivity,
            commands::set_preamp_gain,
            commands::set_a_weighting,
            commands::dbfs_to_spl,
            commands::calibrate_with_reference,
            commands::get_calibration_info,
            // Export commands
            commands::export_transfer_function,
            commands::export_impulse_response,
            // Reference curve commands
            commands::get_available_reference_curves,
            commands::get_reference_curve,
            commands::get_reference_curve_at_frequencies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

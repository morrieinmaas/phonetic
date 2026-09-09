pub mod audio;
pub mod cli;
mod commands;
pub mod dsp;
pub mod licensing;
pub mod session;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    println!("Phonetic starting up...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            log::info!("Phonetic setup starting...");
            println!("Phonetic setup starting...");

            // Initialize audio engine
            let audio_engine = audio::AudioEngine::new();
            app.manage(audio_engine);

            // Initialize DSP state for delay finder, impulse response, and calibration
            let dsp_state = commands::DspState::new();
            app.manage(dsp_state);

            log::info!("Phonetic setup complete");
            println!("Phonetic setup complete");
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
            commands::set_transfer_function_delay,
            commands::get_transfer_function_delay,
            commands::set_max_averages,
            commands::set_resolution_mode,
            commands::is_multi_window,
            commands::set_smoothing,
            commands::get_diagnostics,
            commands::set_input_channel_count,
            commands::set_measurement_channel,
            commands::get_routing,
            commands::set_routing,
            commands::get_pair_transfer_function,
            commands::average_traces,
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
            // Room criteria commands
            commands::get_room_criteria,
            commands::get_rc_rating,
            // Export commands
            commands::export_transfer_function,
            commands::export_impulse_response,
            // Reference curve commands
            commands::get_available_reference_curves,
            commands::get_reference_curve,
            commands::get_reference_curve_at_frequencies,
            // Offline WAV file analysis
            commands::measure_wav_file,
            // Virtual EQ commands
            commands::predict_eq,
            // Licensing
            commands::get_license_state,
            commands::activate_license,
            // Session save/load commands
            commands::save_session,
            commands::load_session,
            // LEQ commands
            commands::get_leq_metrics,
            commands::list_noise_limits,
            commands::compute_leq_dose,
            commands::start_leq_logging,
            commands::stop_leq_logging,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod audio;
mod commands;
mod dsp;

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_audio_devices,
            commands::get_audio_state,
            commands::start_audio_stream,
            commands::stop_audio_stream,
            commands::start_generator,
            commands::stop_generator,
            commands::get_spectrum_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

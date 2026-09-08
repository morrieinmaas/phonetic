use phonetic_lib::session::{
    load_session_from_path, save_session_to_path, Session, SessionSettings, SESSION_VERSION,
};

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn example() -> Session {
    Session {
        version: SESSION_VERSION,
        created_at: "2026-09-07T12:00:00Z".into(),
        name: "Main PA — stalls".into(),
        settings: SessionSettings {
            sample_rate: 48_000.0,
            fft_size: 4096,
            multi_window: true,
            smoothing: "octave6".into(),
            delay_samples: 480,
            max_averages: 16,
        },
        traces: vec![serde_json::json!({ "id": "t1", "name": "Left", "visible": true })],
    }
}

#[test]
fn session_roundtrips_through_a_file() {
    let path = tmp("phonetic_session.json");
    save_session_to_path(&path, &example()).expect("save should succeed");
    let loaded = load_session_from_path(&path).expect("load should succeed");

    assert_eq!(loaded.version, SESSION_VERSION);
    assert_eq!(loaded.name, "Main PA — stalls");
    assert_eq!(loaded.settings.delay_samples, 480);
    assert_eq!(loaded.settings.smoothing, "octave6");
    assert_eq!(loaded.traces.len(), 1);

    std::fs::remove_file(&path).ok();
}

/// The point of JSON over a binary blob: a person can read and edit it.
#[test]
fn session_file_is_human_readable() {
    let path = tmp("phonetic_readable.json");
    save_session_to_path(&path, &example()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();

    assert!(
        text.contains("\"name\": \"Main PA — stalls\""),
        "should be pretty-printed"
    );
    assert!(text.contains('\n'), "should span multiple lines");
    assert!(text.lines().count() > 5);

    std::fs::remove_file(&path).ok();
}

#[test]
fn a_newer_session_version_is_refused_with_a_clear_message() {
    let path = tmp("phonetic_future.json");
    let mut future = example();
    future.version = SESSION_VERSION + 1;
    save_session_to_path(&path, &future).unwrap();

    let err = load_session_from_path(&path).expect_err("should refuse a future version");
    assert!(
        err.contains("newer version"),
        "message should explain the version problem, got: {err}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn malformed_json_is_an_error_not_a_panic() {
    let path = tmp("phonetic_broken.json");
    std::fs::write(&path, "{ this is not json").unwrap();
    assert!(load_session_from_path(&path).is_err());
    std::fs::remove_file(&path).ok();
}

#[test]
fn a_missing_file_is_an_error() {
    assert!(load_session_from_path("/nope/missing.json").is_err());
}

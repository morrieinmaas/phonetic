use phonetic_lib::audio::{read_wav, write_wav};

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn stereo_roundtrip_preserves_samples_and_rate() {
    let path = tmp("phonetic_roundtrip.wav");
    let left: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) * 2.0 - 1.0).collect();
    let right: Vec<f32> = left.iter().map(|s| -s).collect();

    write_wav(&path, &[left.clone(), right.clone()], 48_000.0).expect("write should succeed");
    let read = read_wav(&path).expect("read should succeed");

    assert_eq!(read.sample_rate, 48_000.0);
    assert_eq!(read.channels.len(), 2);
    assert_eq!(read.frames(), 1000);

    for (i, (a, b)) in left.iter().zip(&read.channels[0]).enumerate() {
        assert!((a - b).abs() < 1e-6, "left sample {i}: wrote {a}, read {b}");
    }
    for (i, (a, b)) in right.iter().zip(&read.channels[1]).enumerate() {
        assert!(
            (a - b).abs() < 1e-6,
            "right sample {i}: wrote {a}, read {b}"
        );
    }

    std::fs::remove_file(&path).ok();
}

#[test]
fn mono_roundtrip_works() {
    let path = tmp("phonetic_mono.wav");
    let mono: Vec<f32> = (0..500).map(|i| (i as f32 * 0.01).sin()).collect();

    write_wav(&path, std::slice::from_ref(&mono), 44_100.0).unwrap();
    let read = read_wav(&path).unwrap();

    assert_eq!(read.channels.len(), 1);
    assert_eq!(read.sample_rate, 44_100.0);
    assert_eq!(read.frames(), 500);

    std::fs::remove_file(&path).ok();
}

#[test]
fn missing_file_reports_an_error_rather_than_panicking() {
    let err = read_wav("/nonexistent/definitely-not-here.wav");
    assert!(err.is_err(), "a missing file must return Err, not panic");
}

#[test]
fn channel_length_mismatch_is_rejected() {
    let path = tmp("phonetic_mismatch.wav");
    let err = write_wav(&path, &[vec![0.0; 10], vec![0.0; 5]], 48_000.0);
    assert!(err.is_err(), "ragged channels must be rejected");
}

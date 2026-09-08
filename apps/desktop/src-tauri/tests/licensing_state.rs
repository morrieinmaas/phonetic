use phonetic_lib::licensing::{evaluate, LicenseRecord, LicenseState, TRIAL_DAYS};

fn unlicensed(first_run_day: i64) -> LicenseRecord {
    LicenseRecord {
        key: None,
        first_run_day,
        activated: false,
    }
}

#[test]
fn a_fresh_install_gets_the_full_trial() {
    match evaluate(&unlicensed(1000), 1000) {
        LicenseState::Trial { days_remaining } => assert_eq!(days_remaining, TRIAL_DAYS),
        other => panic!("expected a full trial, got {other:?}"),
    }
}

#[test]
fn the_trial_counts_down() {
    match evaluate(&unlicensed(1000), 1005) {
        LicenseState::Trial { days_remaining } => assert_eq!(days_remaining, TRIAL_DAYS - 5),
        other => panic!("expected 9 days left, got {other:?}"),
    }
}

#[test]
fn the_trial_expires_on_the_last_day() {
    assert!(matches!(
        evaluate(&unlicensed(1000), 1000 + TRIAL_DAYS as i64),
        LicenseState::Expired
    ));
}

#[test]
fn an_activated_licence_never_expires() {
    let record = LicenseRecord {
        key: Some("ABCD-EFGH-IJKL-MNOP".into()),
        first_run_day: 1000,
        activated: true,
    };
    match evaluate(&record, 9999) {
        LicenseState::Licensed { key_suffix } => assert_eq!(key_suffix, "MNOP"),
        other => panic!("expected Licensed, got {other:?}"),
    }
}

#[test]
fn a_key_present_but_not_activated_is_still_a_trial() {
    let record = LicenseRecord {
        key: Some("ABCD-EFGH-IJKL-MNOP".into()),
        first_run_day: 1000,
        activated: false,
    };
    assert!(matches!(
        evaluate(&record, 1001),
        LicenseState::Trial { .. }
    ));
}

/// A clock set backwards must not extend the trial indefinitely.
#[test]
fn a_clock_set_backwards_does_not_extend_the_trial() {
    assert!(matches!(
        evaluate(&unlicensed(1000), 900),
        LicenseState::Trial {
            days_remaining: TRIAL_DAYS
        }
    ));
}

use phonetic_lib::licensing::{load_record, save_record};

#[test]
fn a_record_round_trips_through_the_data_directory() {
    let dir = std::env::temp_dir().join("phonetic_license_test");
    std::fs::create_dir_all(&dir).unwrap();

    let record = LicenseRecord {
        key: Some("AAAA-BBBB-CCCC-DDDD".into()),
        first_run_day: 20_000,
        activated: true,
    };
    save_record(&dir, &record).expect("save should succeed");

    let loaded = load_record(&dir);
    assert_eq!(loaded.key.as_deref(), Some("AAAA-BBBB-CCCC-DDDD"));
    assert_eq!(loaded.first_run_day, 20_000);
    assert!(loaded.activated);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_first_run_creates_a_trial_record_dated_today() {
    let dir = std::env::temp_dir().join("phonetic_license_first_run");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let record = load_record(&dir);
    assert!(!record.activated, "a first run is not licensed");
    assert_eq!(record.first_run_day, phonetic_lib::licensing::today());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_corrupt_record_falls_back_to_a_fresh_trial_rather_than_locking_the_user_out() {
    let dir = std::env::temp_dir().join("phonetic_license_corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("license.json"), "not json at all").unwrap();

    let record = load_record(&dir);
    assert!(!record.activated);

    std::fs::remove_dir_all(&dir).ok();
}

use phonetic_lib::licensing::{evaluate, LicenseRecord, LicenseState, TRIAL_DAYS};

fn unlicensed(first_run_day: i64) -> LicenseRecord {
    LicenseRecord { key: None, first_run_day, activated: false }
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
    assert!(matches!(evaluate(&record, 1001), LicenseState::Trial { .. }));
}

/// A clock set backwards must not extend the trial indefinitely.
#[test]
fn a_clock_set_backwards_does_not_extend_the_trial() {
    assert!(matches!(
        evaluate(&unlicensed(1000), 900),
        LicenseState::Trial { days_remaining: TRIAL_DAYS }
    ));
}

//! External runtimes remain opt-in locally. The two isolated brokers are mandatory in CI.
#![allow(dead_code)]

use std::ffi::OsString;

pub fn checked_value(var: &str, value: Option<OsString>, mandatory: bool) -> Option<String> {
    let value = value
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty());
    assert!(
        !mandatory || value.is_some(),
        "{var} must be nonblank Unicode in CI"
    );
    value
}

fn ci() -> bool {
    std::env::var_os("GITHUB_ACTIONS").as_deref() == Some(std::ffi::OsStr::new("true"))
}

pub fn require_env(var: &str) -> Option<String> {
    let mandatory = ci() && matches!(var, "PULSE_NATS_SERVER_BIN" | "ARCHIPELAGO_NATS_SERVER_BIN");
    let value = checked_value(var, std::env::var_os(var), mandatory);
    if value.is_none() {
        eprintln!("skipping: {var} not set");
    }
    value
}

/// Called only by the selected configured fixture entry paths, never all scratch databases.
pub fn required_database_in_ci() {
    checked_value(
        "CATALYST_TEST_DATABASE_URL",
        std::env::var_os("CATALYST_TEST_DATABASE_URL"),
        ci(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerequisite_values_fail_closed_without_global_environment_mutation() {
        for var in [
            "PULSE_NATS_SERVER_BIN",
            "ARCHIPELAGO_NATS_SERVER_BIN",
            "CATALYST_TEST_DATABASE_URL",
        ] {
            for value in [None, Some(OsString::from("")), Some(OsString::from(" \t"))] {
                let panic = std::panic::catch_unwind(|| checked_value(var, value.clone(), true));
                assert!(panic.is_err(), "{var}: missing/blank must fail");
                assert!(checked_value(var, value, false).is_none());
            }
            assert_eq!(
                checked_value(var, Some(OsString::from("fixture")), true).as_deref(),
                Some("fixture")
            );
        }
        for var in ["LIVEKIT_URL", "WORLDS_TEST_URL"] {
            assert!(checked_value(var, None, false).is_none());
        }
    }

    #[test]
    fn non_unicode_prerequisites_fail_closed() {
        #[cfg(unix)]
        let invalid = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xff])
        };
        #[cfg(windows)]
        let invalid = {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xd800])
        };
        for var in [
            "PULSE_NATS_SERVER_BIN",
            "ARCHIPELAGO_NATS_SERVER_BIN",
            "CATALYST_TEST_DATABASE_URL",
        ] {
            assert!(
                std::panic::catch_unwind(|| checked_value(var, Some(invalid.clone()), true))
                    .is_err()
            );
            assert!(checked_value(var, Some(invalid.clone()), false).is_none());
        }
    }
}

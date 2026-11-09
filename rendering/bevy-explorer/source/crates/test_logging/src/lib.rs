//! Logging for test binaries: installed once per process and captured by the test harness, so
//! a failing test prints its logs and a passing one stays quiet. `RUST_LOG` sets the filter.
//! Only ever a dev-dependency.

#![forbid(unsafe_code)]

use std::sync::Once;

pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use bevy::log::tracing_subscriber::{self, EnvFilter};

            // a binary whose fixture already installed a logger keeps that one
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .with_test_writer()
                .try_init();
        }
        #[cfg(target_arch = "wasm32")]
        {
            bevy::app::App::new().add_plugins(bevy::log::LogPlugin::default());
        }
    });
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[test]
    fn repeated_initialization_preserves_errors_on_test_failure() {
        if std::env::var_os("DCL_TEST_LOG_CHILD").is_some() {
            super::init();
            super::init();
            bevy::log::error!("test_logging expected error");
            panic!("test_logging expected failure");
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::repeated_initialization_preserves_errors_on_test_failure",
            ])
            .env("DCL_TEST_LOG_CHILD", "1")
            .env("RUST_LOG", "info")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test_logging expected error"), "{stdout}");
        assert!(stdout.contains("test_logging expected failure"), "{stdout}");
    }
}

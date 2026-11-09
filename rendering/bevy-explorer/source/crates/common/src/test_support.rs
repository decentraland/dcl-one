use std::sync::Once;

#[cfg(not(target_arch = "wasm32"))]
use bevy::log::tracing_subscriber::{self, EnvFilter};

pub fn init_logging() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .with_test_writer()
                .init();
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
            super::init_logging();
            super::init_logging();
            bevy::log::error!("test-support expected error");
            panic!("test-support expected failure");
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "test_support::tests::repeated_initialization_preserves_errors_on_test_failure",
            ])
            .env("DCL_TEST_LOG_CHILD", "1")
            .env("RUST_LOG", "info")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test-support expected error"), "{stdout}");
        assert!(stdout.contains("test-support expected failure"), "{stdout}");
        assert!(!stdout.contains("already set"), "{stdout}");
    }
}

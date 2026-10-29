//! Finding, starting and closing Chrome.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::cli::Settings;
use crate::failure::Failure;
use crate::out;
use crate::platform;
use crate::profile::PROFILE_DIRECTORY;

pub fn find(configured: Option<&str>) -> Result<PathBuf, Failure> {
    if let Some(path) = configured.filter(|path| !path.is_empty()) {
        return if Path::new(path).is_file() {
            Ok(PathBuf::from(path))
        } else {
            Err(Failure::cannot_run(format!(
                "no Chrome at '{}' (CHROME_PATH)",
                out::safe(path)
            )))
        };
    }

    let mut candidates = Vec::new();
    if cfg!(windows) {
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
            if let Some(dir) = std::env::var_os(variable).filter(|dir| !dir.is_empty()) {
                candidates.push(
                    Path::new(&dir)
                        .join("Google")
                        .join("Chrome")
                        .join("Application")
                        .join("chrome.exe"),
                );
            }
        }
    } else if cfg!(target_os = "macos") {
        let inside = "Google Chrome.app/Contents/MacOS/Google Chrome";
        candidates.push(Path::new("/Applications").join(inside));
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            candidates.push(home.join("Applications").join(inside));
        }
    } else {
        let path = std::env::var_os("PATH").unwrap_or_default();
        for name in [
            "google-chrome-stable",
            "google-chrome",
            "chromium",
            "chromium-browser",
        ] {
            for dir in std::env::split_paths(&path) {
                candidates.push(dir.join(name));
            }
        }
    }

    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
        .ok_or_else(|| {
            let looked: Vec<String> = candidates
                .iter()
                .take(3)
                .map(|c| c.display().to_string())
                .collect();
            Failure::cannot_run(format!(
                "Google Chrome is not installed (looked in {}); install it, or point CHROME_PATH at it",
                looked.join(", ")
            ))
        })
}

/// The command line Chrome is started with, `url` last.
pub fn arguments(settings: &Settings, url: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        // The profile: one per laptop, kept across runs and builds. It holds the stored guest
        // (the warm-up creates it).
        &format!("--user-data-dir={}", settings.profile.display()),
        // the folder `profile::scrub` cleans
        &format!("--profile-directory={PROFILE_DIRECTORY}"),
        // No first-run page, default-browser prompt, search-engine choice, crash-restore bubble
        // or keychain prompt (macOS) in an unattended run.
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-search-engine-choice-screen",
        "--disable-session-crashed-bubble",
        "--hide-crash-restore-bubble",
        "--password-store=basic",
        "--use-mock-keychain",
        "--disable-hang-monitor",
        // A hidden or throttled tab pauses the engine (deploy/web/engine/boot.js), and Windows
        // reports a covered window as hidden unless told not to.
        "--disable-renderer-backgrounding",
        "--disable-backgrounding-occluded-windows",
        "--disable-background-timer-throttling",
        // Nothing else for Chrome to do during a measured run.
        "--disable-background-mode",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-sync",
        "--disable-default-apps",
        // Features off, in the one switch Chrome honours (a second --disable-features replaces
        // the first; so does one passed with --chrome-arg): Windows' covered-window occlusion
        // tracking, which would report a window another window overlaps as hidden, and the
        // Local Network Access checks behind that permission prompt.
        //
        // The Local Network Access switch is kept off for the whole browser, on purpose. Nobody
        // answers a prompt in an unattended run; the page under test is on loopback and calls
        // nothing private, so it should never be asked, and what the switch removes is the guard
        // for a third-party page the run opens (a scene's openExternalUrl, a sign-in popup),
        // which could then reach localhost and the LAN without asking. The narrower setting is
        // Chrome's policy LocalNetworkAccessAllowedForUrls for this origin, which is a machine
        // policy (registry, managed preferences) and not a command-line switch: writing it would
        // change the laptop's system settings, which the runner does not, and no narrower switch
        // exists to try. The loss is small because the server answers such a page nothing: it
        // needs the run's token, a loopback Host and, for `end`, a JSON content type.
        "--disable-features=CalculateNativeWinOcclusion,LocalNetworkAccessChecks",
        // No audio gesture gate: nobody clicks.
        "--autoplay-policy=no-user-gesture-required",
        // The GPU the desktop client renders on: wgpu picks the high-performance adapter
        // natively, while Chrome's WebGPU picks the low-power one on a laptop with two GPUs, where
        // the same build runs many times slower (METRICS.md has the numbers). No-op with one GPU.
        "--force-high-performance-gpu",
        // Chrome's own log noise (updater pipes, GPU info) stays out of the job log.
        "--log-level=3",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    match settings.window.as_deref() {
        None => args.push("--start-maximized".into()),
        Some(window) => {
            // "1280x720"
            args.push(format!("--window-size={}", window.replace('x', ",")));
            args.push("--window-position=0,0".into());
        }
    }

    args.extend(settings.chrome_args.iter().cloned());
    args.push(url.to_string());
    args
}

/// Chrome's output goes nowhere (`--log-level=3` leaves little of it anyway): it must not share the
/// runner's pipes, or a Chrome that outlives the runner would keep the step's output open and the
/// Actions runner would wait on it.
pub fn launch(chrome: &Path, settings: &Settings, url: &str) -> Result<Child, Failure> {
    let mut command = Command::new(chrome);
    command
        .args(arguments(settings, url))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    platform::own_process_group(&mut command);
    command
        .spawn()
        .map_err(|error| Failure::cannot_run(format!("Chrome did not start ({error})")))
}

/// How long `close` waits: for Chrome to exit after the polite request, then for it to be gone
/// after the kill.
#[derive(Debug, Clone, Copy)]
pub struct Patience {
    pub polite: Duration,
    pub after_kill: Duration,
}

impl Patience {
    /// After a run the page ended, or the timeout cut off.
    pub const RUN: Self = Self {
        polite: Duration::from_secs(20),
        after_kill: Duration::from_secs(10),
    };

    /// After an interrupt. A cancelled Actions job stops the runner's process a few seconds after
    /// it asks it to stop, and a Chrome still open then is left running on the profile.
    pub const INTERRUPTED: Self = Self {
        polite: Duration::from_secs(3),
        after_kill: Duration::from_secs(2),
    };
}

/// A polite close first, so Chrome flushes its profile; then the whole tree.
pub async fn close(chrome: &mut Child, patience: Patience) {
    if matches!(chrome.try_wait(), Ok(Some(_))) {
        return;
    }
    let Some(pid) = chrome.id() else {
        return;
    };

    let asked = platform::request_close(pid);
    if asked && exits_within(chrome, patience.polite).await {
        out::log("Chrome closed");
        return;
    }

    out::log(if asked {
        format!(
            "Chrome did not close in {} s; killing it",
            patience.polite.as_secs()
        )
    } else {
        "Chrome has no window to close; killing it".to_string()
    });
    platform::kill_tree(pid);
    let _ = chrome.start_kill();
    exits_within(chrome, patience.after_kill).await;
}

async fn exits_within(chrome: &mut Child, limit: Duration) -> bool {
    tokio::time::timeout(limit, chrome.wait()).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            out_dir: ".".into(),
            port: 18742,
            profile: "/profiles/one".into(),
            chrome_path: None,
            realm: "supportteam.dcl.eth".into(),
            query: None,
            window: None,
            chrome_args: vec![],
            no_browser: false,
            keep_service_worker: false,
            timeout: Duration::from_secs(1),
            warmup_timeout: Duration::from_secs(1),
        }
    }

    #[test]
    fn chrome_runs_on_the_profile_on_the_fast_gpu_without_prompts() {
        let args = arguments(&settings(), "http://localhost:18742/");

        assert_eq!(
            args[0],
            format!("--user-data-dir={}", Path::new("/profiles/one").display())
        );
        assert_eq!(args.last().unwrap(), "http://localhost:18742/");
        for wanted in [
            "--profile-directory=Default",
            "--force-high-performance-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-search-engine-choice-screen",
            "--hide-crash-restore-bubble",
            "--disable-renderer-backgrounding",
            "--disable-backgrounding-occluded-windows",
            "--autoplay-policy=no-user-gesture-required",
            "--start-maximized",
        ] {
            assert!(args.iter().any(|arg| arg == wanted), "{wanted}");
        }
        // The one switch Chrome honours: a second --disable-features would replace the first.
        let features: Vec<&String> = args
            .iter()
            .filter(|a| a.starts_with("--disable-features="))
            .collect();
        assert_eq!(features.len(), 1);
        assert!(features[0].contains("CalculateNativeWinOcclusion"));
        assert!(features[0].contains("LocalNetworkAccessChecks"));
        // Nothing that weakens the page's sandbox.
        for forbidden in [
            "--no-sandbox",
            "--disable-web-security",
            "--ignore-certificate-errors",
            "--remote-debugging-port",
        ] {
            assert!(
                !args.iter().any(|arg| arg.starts_with(forbidden)),
                "{forbidden}"
            );
        }
        assert!(!args
            .iter()
            .any(|arg| arg == "--headless" || arg.starts_with("--headless=")));
    }

    #[test]
    fn a_window_size_replaces_maximized_and_extra_flags_come_before_the_address() {
        let mut s = settings();
        s.window = Some("1280x720".into());
        s.chrome_args = vec!["--one".into(), "--two=2".into()];
        let args = arguments(&s, "http://localhost:1/");

        assert!(args.iter().any(|arg| arg == "--window-size=1280,720"));
        assert!(args.iter().any(|arg| arg == "--window-position=0,0"));
        assert!(!args.iter().any(|arg| arg == "--start-maximized"));
        assert_eq!(
            &args[args.len() - 3..],
            ["--one", "--two=2", "http://localhost:1/"]
        );
    }

    #[test]
    fn a_chrome_that_is_not_where_it_was_said_to_be_is_a_message() {
        let missing = find(Some("/no/such/chrome"));
        assert!(
            matches!(missing, Err(Failure::CannotRun(message)) if message.contains("no Chrome at"))
        );
    }
}

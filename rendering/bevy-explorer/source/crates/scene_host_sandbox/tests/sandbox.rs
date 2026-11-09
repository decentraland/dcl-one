//! A process cannot sandbox itself and keep running tests, so each test runs this binary again with
//! `SCENE_HOST_SANDBOX_TEST_CHILD` set. The child applies the sandbox, runs the probe and prints
//! one JSON line; the test parses it.

fn main() {
    #[cfg(target_os = "linux")]
    linux::main();
}

#[cfg(target_os = "linux")]
mod linux {
    use scene_host_sandbox::{apply, probe, Landlock, Mode, Report, Status};
    use serde::{Deserialize, Serialize};
    use std::net::TcpListener;
    use std::process::Command;

    const CHILD: &str = "SCENE_HOST_SANDBOX_TEST_CHILD";

    const TESTS: [(&str, fn()); 4] = [
        (
            "a_sandboxed_process_cannot_start_programs_or_touch_files",
            a_sandboxed_process_cannot_start_programs_or_touch_files,
        ),
        (
            "a_sandboxed_process_can_still_run_threads_jit_and_tcp",
            a_sandboxed_process_can_still_run_threads_jit_and_tcp,
        ),
        (
            "require_mode_fails_when_applied_with_several_threads",
            require_mode_fails_when_applied_with_several_threads,
        ),
        ("off_mode_changes_nothing", off_mode_changes_nothing),
    ];

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ChildOutput {
        status: Option<Status>,
        error: Option<String>,
        report: Option<Report>,
        kernel: KernelState,
    }

    /// What the kernel says about the process, independent of what `apply` reports.
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KernelState {
        no_new_privs: bool,
        seccomp_mode: u32,
    }

    fn kernel_state() -> KernelState {
        let status = std::fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
        let field = |name: &str| -> u32 {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_else(|| panic!("no {name} in /proc/self/status"))
        };
        KernelState {
            no_new_privs: field("NoNewPrivs:") == 1,
            seccomp_mode: field("Seccomp:"),
        }
    }

    fn child(mode: &str) {
        let loopback = TcpListener::bind("127.0.0.1:0").expect("bind a loopback listener");
        let result = match mode {
            "require" => apply(Mode::Require),
            "off" => apply(Mode::Off),
            "require-with-a-thread" => {
                let (release, wait) = std::sync::mpsc::channel::<()>();
                let thread = std::thread::spawn(move || wait.recv());
                let result = apply(Mode::Require);
                drop(release);
                let _ = thread.join();
                result
            }
            other => panic!("unknown child mode {other}"),
        };
        let output = ChildOutput {
            report: result.is_ok().then(|| probe(&loopback)),
            error: result.as_ref().err().map(|e| format!("{e:#}")),
            status: result.ok(),
            kernel: kernel_state(),
        };
        println!("{}", serde_json::to_string(&output).expect("serialize"));
    }

    fn run_child(mode: &str) -> ChildOutput {
        let output = Command::new(std::env::current_exe().expect("current_exe"))
            .env(CHILD, mode)
            .output()
            .expect("run the child");
        assert!(
            output.status.success(),
            "child `{mode}` failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let parsed: ChildOutput = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "child `{mode}` printed no report ({e}): {}",
                String::from_utf8_lossy(&output.stdout)
            )
        });
        eprintln!(
            "child `{mode}`: {}",
            serde_json::to_string(&parsed).unwrap()
        );
        parsed
    }

    fn sandboxed() -> Report {
        let output = run_child("require");
        let status = output.status.unwrap_or_else(|| {
            panic!(
                "apply(Require) failed: {}",
                output.error.unwrap_or_default()
            )
        });
        assert!(status.seccomp, "{status:?}");
        assert!(status.no_new_privs, "{status:?}");
        assert!(
            matches!(
                status.landlock,
                Landlock::FullyEnforced | Landlock::PartiallyEnforced
            ),
            "{status:?}"
        );
        assert_eq!(
            output.kernel,
            KernelState {
                no_new_privs: true,
                seccomp_mode: 2
            }
        );
        output.report.expect("a report")
    }

    fn a_sandboxed_process_cannot_start_programs_or_touch_files() {
        let Report {
            cannot_spawn_program,
            cannot_replace_program,
            cannot_fork,
            cannot_create_temp_file,
            cannot_open_home,
            cannot_read_init_environ,
            cannot_open_unix_socket,
            cannot_signal_parent,
            cannot_ptrace_parent,
            can_run_thread: _,
            can_run_jit_code: _,
            can_connect_loopback_tcp: _,
            can_resolve_localhost: _,
            can_write_dev_null: _,
            can_read_own_proc: _,
            can_signal_self: _,
        } = sandboxed();
        assert!(cannot_spawn_program);
        assert!(cannot_replace_program);
        assert!(cannot_fork);
        assert!(cannot_create_temp_file);
        assert!(cannot_open_home);
        assert!(cannot_read_init_environ);
        assert!(cannot_open_unix_socket);
        assert!(cannot_signal_parent);
        assert!(cannot_ptrace_parent);
    }

    fn a_sandboxed_process_can_still_run_threads_jit_and_tcp() {
        let Report {
            cannot_spawn_program: _,
            cannot_replace_program: _,
            cannot_fork: _,
            cannot_create_temp_file: _,
            cannot_open_home: _,
            cannot_read_init_environ: _,
            cannot_open_unix_socket: _,
            cannot_signal_parent: _,
            cannot_ptrace_parent: _,
            can_run_thread,
            can_run_jit_code,
            can_connect_loopback_tcp,
            can_resolve_localhost,
            can_write_dev_null,
            can_read_own_proc,
            can_signal_self,
        } = sandboxed();
        assert!(can_run_thread);
        assert!(can_run_jit_code);
        assert!(can_connect_loopback_tcp);
        assert!(can_resolve_localhost);
        assert!(can_write_dev_null);
        assert!(can_read_own_proc);
        assert!(can_signal_self);
    }

    fn require_mode_fails_when_applied_with_several_threads() {
        let output = run_child("require-with-a-thread");
        assert_eq!(output.status, None);
        assert!(output.error.is_some());
        assert!(output.report.is_none());
        assert_eq!(output.kernel, kernel_state(), "nothing was applied");
    }

    fn off_mode_changes_nothing() {
        let output = run_child("off");
        assert_eq!(
            output.status,
            Some(Status {
                landlock: Landlock::NotEnforced,
                seccomp: false,
                no_new_privs: false
            })
        );
        assert_eq!(output.kernel, kernel_state());
        // home, /proc/1 and ptrace also depend on the host's own permissions, so they are not
        // asserted here
        let Report {
            cannot_spawn_program,
            cannot_replace_program,
            cannot_fork,
            cannot_create_temp_file,
            cannot_open_home: _,
            cannot_read_init_environ: _,
            cannot_open_unix_socket,
            cannot_signal_parent,
            cannot_ptrace_parent: _,
            can_run_thread,
            can_run_jit_code,
            can_connect_loopback_tcp,
            can_resolve_localhost,
            can_write_dev_null,
            can_read_own_proc,
            can_signal_self,
        } = output.report.expect("a report");
        assert!(!cannot_spawn_program);
        assert!(!cannot_replace_program);
        assert!(!cannot_fork);
        assert!(!cannot_create_temp_file);
        assert!(!cannot_open_unix_socket);
        assert!(!cannot_signal_parent);
        assert!(can_run_thread);
        assert!(can_run_jit_code);
        assert!(can_connect_loopback_tcp);
        assert!(can_resolve_localhost);
        assert!(can_write_dev_null);
        assert!(can_read_own_proc);
        assert!(can_signal_self);
    }

    /// The libtest arguments `cargo test` passes on: name filters, `--exact`, `--skip`, `--list`
    /// and `--ignored`.
    pub fn main() {
        if let Ok(mode) = std::env::var(CHILD) {
            child(&mode);
            return;
        }

        let mut filters = Vec::new();
        let mut skips = Vec::new();
        let (mut exact, mut list, mut ignored) = (false, false, false);
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--exact" => exact = true,
                "--list" => list = true,
                "--ignored" => ignored = true,
                "--skip" => skips.extend(args.next()),
                "--test-threads" | "--format" | "--color" | "--logfile" | "--shuffle-seed"
                | "-Z" => {
                    args.next();
                }
                _ if arg.starts_with('-') => {}
                _ => filters.push(arg),
            }
        }
        let matches = |name: &str, pattern: &String| {
            if exact {
                name == pattern
            } else {
                name.contains(pattern.as_str())
            }
        };
        let selected: Vec<_> = TESTS
            .into_iter()
            .filter(|_| !ignored)
            .filter(|(name, _)| filters.is_empty() || filters.iter().any(|f| matches(name, f)))
            .filter(|(name, _)| !skips.iter().any(|s| matches(name, s)))
            .collect();

        if list {
            for (name, _) in &selected {
                println!("{name}: test");
            }
            return;
        }

        println!("\nrunning {} tests", selected.len());
        let mut failed = Vec::new();
        for (name, test) in &selected {
            let passed = std::panic::catch_unwind(*test).is_ok();
            println!("test {name} ... {}", if passed { "ok" } else { "FAILED" });
            if !passed {
                failed.push(*name);
            }
        }
        println!(
            "\ntest result: {}. {} passed; {} failed",
            if failed.is_empty() { "ok" } else { "FAILED" },
            selected.len() - failed.len(),
            failed.len()
        );
        if !failed.is_empty() {
            std::process::exit(101);
        }
    }
}

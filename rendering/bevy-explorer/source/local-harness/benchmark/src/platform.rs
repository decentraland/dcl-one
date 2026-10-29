//! What differs between the operating systems: asking a process to close, stopping it with its
//! children, and listing the running processes' command lines. Everything else is one code path.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::failure::Failure;

/// A process and the command line it was started with.
pub struct Process {
    pub pid: u32,
    pub command: String,
}

/// Asks the process to close its window, so Chrome flushes its profile. False when it has no
/// window to ask.
#[cfg(windows)]
pub fn request_close(pid: u32) -> bool {
    taskkill(&[&pid.to_string()])
}

/// Stops the process and everything it started.
#[cfg(windows)]
pub fn kill_tree(pid: u32) {
    taskkill(&[&pid.to_string(), "/T", "/F"]);
}

#[cfg(windows)]
fn taskkill(arguments: &[&str]) -> bool {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    Command::new(
        std::path::Path::new(&root)
            .join("System32")
            .join("taskkill.exe"),
    )
    .arg("/PID")
    .args(arguments)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .is_ok_and(|status| status.success())
}

/// Every process that has a `--user-data-dir` in its command line. Windows keeps command lines in
/// WMI; Windows PowerShell is on every machine. The script goes in encoded, so no quoting is
/// involved.
#[cfg(windows)]
pub fn chrome_processes() -> Result<Vec<Process>, Failure> {
    use base64::Engine;

    const SCRIPT: &str = "\
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-CimInstance Win32_Process |
  Where-Object { $_.CommandLine -and $_.CommandLine.Contains('--user-data-dir=') } |
  ForEach-Object { '{0}|{1}' -f $_.ProcessId, $_.CommandLine }
";
    let utf16: Vec<u8> = SCRIPT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let mut command = Command::new(
        std::path::Path::new(&root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe"),
    );
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        &base64::engine::general_purpose::STANDARD.encode(utf16),
    ]);

    Ok(capture(command)?
        .lines()
        .filter_map(|line| {
            let (pid, command) = line.trim().split_once('|')?;
            Some(Process {
                pid: pid.parse().ok()?,
                command: command.trim().to_string(),
            })
        })
        .collect())
}

/// Chrome is started in a process group of its own, so the whole tree can be stopped at once.
#[cfg(unix)]
pub fn own_process_group(command: &mut tokio::process::Command) {
    command.process_group(0);
}

#[cfg(windows)]
pub fn own_process_group(_command: &mut tokio::process::Command) {}

#[cfg(unix)]
pub fn request_close(pid: u32) -> bool {
    signal(pid as i32, libc::SIGTERM)
}

/// The process group Chrome was started in, then the process itself (a stray from an earlier run
/// is not a group leader of ours).
#[cfg(unix)]
pub fn kill_tree(pid: u32) {
    // 0 and 1 name the caller's whole group and init: never a Chrome.
    let Ok(pid) = i32::try_from(pid).map(i32::abs).map_err(drop) else {
        return;
    };
    if pid <= 1 {
        return;
    }
    signal(-pid, libc::SIGKILL);
    signal(pid, libc::SIGKILL);
}

#[cfg(unix)]
fn signal(target: i32, signal: i32) -> bool {
    // SAFETY: kill(2) takes two integers and touches no memory.
    unsafe { libc::kill(target, signal) == 0 }
}

#[cfg(unix)]
pub fn chrome_processes() -> Result<Vec<Process>, Failure> {
    let mut command = Command::new("/bin/ps");
    command.args(["-A", "-ww", "-o", "pid=", "-o", "command="]);

    Ok(capture(command)?
        .lines()
        .filter_map(|line| {
            let (pid, command) = line.trim().split_once(char::is_whitespace)?;
            Some(Process {
                pid: pid.parse().ok()?,
                command: command.trim().to_string(),
            })
        })
        .collect())
}

/// The standard output of a short command that lists processes, or the reason it cannot be had.
fn capture(mut command: Command) -> Result<String, Failure> {
    const LIMIT: Duration = Duration::from_secs(30);
    let cannot =
        |why: String| Failure::cannot_run(format!("cannot list the running processes ({why})"));

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // PowerShell's progress records go there
        .spawn()
        .map_err(|error| cannot(error.to_string()))?;

    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let output = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
    });

    let deadline = Instant::now() + LIMIT;
    let status = loop {
        match child
            .try_wait()
            .map_err(|error| cannot(error.to_string()))?
        {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                return Err(cannot("it took too long".into()));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let _ = errors.join();
    let bytes = output
        .join()
        .map_err(|_| cannot("its output could not be read".into()))?
        .map_err(|error| cannot(error.to_string()))?;

    if !status.success() {
        return Err(cannot(format!("it exited with {status}")));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

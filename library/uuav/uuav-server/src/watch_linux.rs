//! Orphan prevention: exit as soon as the parent (the host) process is gone.

use anyhow::{Context as _, ensure};
use std::process;
use std::thread;
use std::time::Duration;

/// Asks the kernel to SIGKILL this process when the parent thread that
/// spawned it exits (`PR_SET_PDEATHSIG`), then closes the race the flag
/// leaves open — a parent that died between the spawn and this call, or
/// a spawning thread that exits before the process — with a watcher that
/// polls the parent pid and exits when it changes.
pub fn exit_when_parent_dies(parent_pid: u32) -> anyhow::Result<()> {
    ensure!(
        unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } == 0,
        "prctl(PR_SET_PDEATHSIG) failed"
    );
    let parent = libc::pid_t::try_from(parent_pid).context("parent pid")?;
    // already reparented: the parent died before the flag took effect
    ensure!(
        unsafe { libc::getppid() } == parent,
        "parent process is not observable (already exited?)"
    );

    thread::Builder::new()
        .name("uuav-parent-watch".into())
        .spawn(move || {
            loop {
                if unsafe { libc::getppid() } != parent {
                    process::exit(0);
                }
                thread::sleep(Duration::from_millis(200));
            }
        })
        .context("spawn parent watch thread")?;

    Ok(())
}

//! Writing a file this process is about to execute.
//!
//! Linux refuses to exec a file that is open for writing anywhere (ETXTBSY),
//! and a descriptor one thread has open for writing is also open in every
//! child another thread forks meanwhile, until that child execs: fork copies
//! the whole descriptor table, and close-on-exec only acts at the exec. `start`
//! spawns node, esbuild and the sidecars from other threads while it unpacks
//! the embedded servers, so a plain write can leave the server it just unpacked
//! unrunnable for as long as some unrelated child sits between fork and exec.
//! Renaming or linking does not help: what is busy is the inode, under any name.
//!
//! So the file is written on a thread with a descriptor table of its own: no
//! fork elsewhere in the process copies the descriptor, and it is closed before
//! the thread ends. [`private_descriptor_table`] is the same shelter for any
//! descriptor whose closing must take effect when it is closed, such as a lock.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

/// Creates `path` holding `bytes`, executable when asked, without its write
/// descriptor ever being in the table other threads fork from.
pub fn write_new(path: &Path, bytes: &[u8], executable: bool) -> io::Result<()> {
    with_new_file(path, |file| {
        file.write_all(bytes)?;
        if executable {
            make_executable(file)?;
        }
        Ok(())
    })
}

/// Creates `path` and hands it to `fill` on a thread whose descriptor table is
/// its own; the file is closed by the time this returns.
fn with_new_file<T: Send>(
    path: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<T> + Send,
) -> io::Result<T> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("exec-file-writer".into())
            .spawn_scoped(scope, || {
                private_descriptor_table();
                let mut file = File::create(path)?;
                fill(&mut file)
            })?
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

/// Gives the calling thread a descriptor table of its own holding only stdio:
/// what it opens from then on is never copied into a child another thread
/// forks, and it holds nothing else the process has open.
#[cfg(target_os = "linux")]
pub(crate) fn private_descriptor_table() {
    // close_range(3, ~0, CLOSE_RANGE_UNSHARE), Linux 5.9.
    // SAFETY: plain syscalls on the calling thread's own descriptor table.
    let unshared = unsafe {
        libc::syscall(
            libc::SYS_close_range,
            3 as libc::c_long,
            libc::c_uint::MAX as libc::c_long,
            libc::CLOSE_RANGE_UNSHARE as libc::c_long,
        )
    } == 0;
    if unshared {
        return;
    }
    // Older kernels: a private copy of the whole table, its copies of the
    // process's other descriptors closed here. If even the copy is refused,
    // the caller goes ahead on the shared table.
    // SAFETY: as above.
    if unsafe { libc::unshare(libc::CLONE_FILES) } != 0 {
        return;
    }
    let copies: Vec<libc::c_int> = std::fs::read_dir("/proc/thread-self/fd")
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > 2)
        .collect();
    for fd in copies {
        // SAFETY: this thread's own copies; the listing's descriptor is
        // already closed, so closing it again only fails.
        unsafe { libc::close(fd) };
    }
}

/// Elsewhere exec does not refuse a file that is open for writing, and a
/// descriptor's closing is left to its closing.
#[cfg(not(target_os = "linux"))]
pub(crate) fn private_descriptor_table() {}

#[cfg(unix)]
fn make_executable(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_file: &File) -> io::Result<()> {
    Ok(())
}

/// A forked child that has not exec'd yet, parked on a pipe: it holds a copy
/// of the descriptor table it was forked with until it is let go (dropped).
#[cfg(all(test, target_os = "linux"))]
pub(crate) struct Parked {
    pid: libc::pid_t,
    release: libc::c_int,
}

#[cfg(all(test, target_os = "linux"))]
impl Parked {
    pub(crate) fn fork() -> Self {
        let mut pipe = [0; 2];
        // SAFETY: a fresh array for the two descriptors.
        assert_eq!(
            unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        // SAFETY: the child calls only async-signal-safe functions.
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // Its own copy of the write end goes, so the parent closing its
            // copy (or dying) is an end of file here.
            unsafe {
                libc::close(pipe[1]);
                let mut byte = 0u8;
                libc::read(pipe[0], (&raw mut byte).cast(), 1);
                libc::_exit(0);
            }
        }
        assert!(pid > 0, "fork: {}", io::Error::last_os_error());
        // SAFETY: the parent's read end, unused here.
        unsafe { libc::close(pipe[0]) };
        Parked {
            pid,
            release: pipe[1],
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
impl Drop for Parked {
    fn drop(&mut self) {
        // SAFETY: the parent's write end and the child it forked.
        unsafe {
            libc::close(self.release);
            libc::waitpid(self.pid, std::ptr::null_mut(), 0);
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// The race `start` lost now and then, made certain: another thread forks
    /// while the file is open for writing, and its child sits between fork and
    /// exec until the file has been written, closed and executed.
    #[test]
    fn a_fork_on_another_thread_mid_write_leaves_the_file_runnable() {
        let dir =
            std::env::temp_dir().join(format!("dcl-scene-cli-exec-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("runs.sh");
        let (open_tx, open_rx) = mpsc::channel::<()>();
        let (forked_tx, forked_rx) = mpsc::channel::<()>();
        let writer = std::thread::spawn({
            let script = script.clone();
            move || {
                with_new_file(&script, move |file| {
                    file.write_all(b"#!/bin/sh\nexit 0\n")?;
                    make_executable(file)?;
                    open_tx.send(()).unwrap();
                    forked_rx.recv().unwrap();
                    Ok(())
                })
            }
        });
        open_rx.recv().unwrap();
        let parked = Parked::fork();
        forked_tx.send(()).unwrap();
        writer.join().unwrap().unwrap();

        let ran = std::process::Command::new(&script).status();
        drop(parked);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            ran.as_ref().is_ok_and(|status| status.success()),
            "the file was written and closed, yet running it gave {ran:?}"
        );
    }
}

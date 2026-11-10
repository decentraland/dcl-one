//! The sandbox the scene host applies to itself on Linux, before it reads a scene.
//!
//! - Landlock limits the filesystem to reading the system libraries, the TLS and resolver
//!   configuration, the time zone data, this process's own `/proc` entry, the CPU count and
//!   quota files, `/proc/sys/vm/overcommit_memory` and `/dev/urandom`; `/dev/null` is the only
//!   writable path. Where the kernel supports it,
//!   signals and abstract Unix sockets are also scoped to the sandbox.
//! - seccomp denies starting programs, reaching other processes (ptrace, memory access, pidfds,
//!   signals, signal ownership, limits, scheduling and memory policy of another pid), changing
//!   file metadata, terminal input injection, new sockets other than IPv4, IPv6 and route netlink,
//!   and kernel interfaces a scene runtime never needs. Threads, futexes and executable memory for
//!   the JIT stay allowed.
//!
//! On other targets [`apply`] reports [`Landlock::Unsupported`], or fails for [`Mode::Require`],
//! and [`probe`] runs no check.

use serde::{Deserialize, Serialize};
use std::net::TcpListener;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod probe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Touch nothing.
    Off,
    /// Apply what the kernel offers and report it.
    On,
    /// Fail unless seccomp applied and Landlock is at least partially enforced.
    Require,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub landlock: Landlock,
    pub seccomp: bool,
    pub no_new_privs: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Landlock {
    FullyEnforced,
    /// The kernel enforces an older Landlock ABI than the one requested.
    PartiallyEnforced,
    /// Not applied: the kernel has no Landlock, or the mode was [`Mode::Off`].
    NotEnforced,
    /// The platform has no Landlock.
    Unsupported,
}

/// One named result per check. Each field is true when the check holds: the action was denied for
/// `cannot_*`, and it worked for `can_*`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub cannot_spawn_program: bool,
    pub cannot_replace_program: bool,
    pub cannot_fork: bool,
    pub cannot_create_temp_file: bool,
    pub cannot_open_home: bool,
    pub cannot_read_init_environ: bool,
    pub cannot_open_unix_socket: bool,
    pub cannot_signal_parent: bool,
    pub cannot_ptrace_parent: bool,
    pub can_run_thread: bool,
    pub can_run_jit_code: bool,
    pub can_connect_loopback_tcp: bool,
    pub can_resolve_localhost: bool,
    pub can_write_dev_null: bool,
    pub can_read_own_proc: bool,
    pub can_signal_self: bool,
}

impl Report {
    pub fn all_hold(&self) -> bool {
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
            can_run_thread,
            can_run_jit_code,
            can_connect_loopback_tcp,
            can_resolve_localhost,
            can_write_dev_null,
            can_read_own_proc,
            can_signal_self,
        } = *self;
        [
            cannot_spawn_program,
            cannot_replace_program,
            cannot_fork,
            cannot_create_temp_file,
            cannot_open_home,
            cannot_read_init_environ,
            cannot_open_unix_socket,
            cannot_signal_parent,
            cannot_ptrace_parent,
            can_run_thread,
            can_run_jit_code,
            can_connect_loopback_tcp,
            can_resolve_localhost,
            can_write_dev_null,
            can_read_own_proc,
            can_signal_self,
        ]
        .into_iter()
        .all(|holds| holds)
    }
}

/// Call once, on the main thread, while the process has one thread: Landlock covers only the
/// calling thread and the threads it creates afterwards.
pub fn apply(mode: Mode) -> anyhow::Result<Status> {
    #[cfg(target_os = "linux")]
    {
        linux::apply(mode)
    }
    #[cfg(not(target_os = "linux"))]
    {
        if mode == Mode::Require {
            anyhow::bail!("the scene host sandbox is only available on Linux");
        }
        Ok(Status {
            landlock: Landlock::Unsupported,
            seccomp: false,
            no_new_privs: false,
        })
    }
}

/// Tries each action the sandbox denies and each it must still allow. `loopback` is a listener the
/// caller opened before [`apply`]; the probe connects to it once and accepts that connection.
pub fn probe(loopback: &TcpListener) -> Report {
    #[cfg(target_os = "linux")]
    {
        probe::run(loopback)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = loopback;
        Report::default()
    }
}

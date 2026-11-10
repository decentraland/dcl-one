//! The sandbox the scene host applies to itself on Linux, before it reads a scene.
//!
//! - Landlock limits the filesystem to reading the system libraries, the TLS and resolver
//!   configuration, the time zone data, this process's own `/proc` entry, the CPU count and
//!   quota files, `/proc/sys/vm/overcommit_memory` and `/dev/urandom`; `/dev/null` is the only
//!   writable path. Where the kernel supports it,
//!   signals and abstract Unix sockets are also scoped to the sandbox.
//! - seccomp denies starting programs, reaching other processes (ptrace, memory access, pidfds,
//!   signals, signal ownership, limits, scheduling and memory policy of another pid, System V
//!   IPC), changing file metadata, terminal input injection, new sockets other than TCP and UDP
//!   over IPv4 and IPv6 and route netlink,
//!   and kernel interfaces a scene runtime never needs. Threads, futexes and executable memory for
//!   the JIT stay allowed.
//!   seccomp is a deny list: the runtime's own syscalls change with glibc, V8 and tokio, so the
//!   filter names what must not happen and allows the rest. Syscalls newer than the last one it was
//!   reviewed against answer ENOSYS until someone reviews them.
//!
//! On other targets [`apply`] reports [`Landlock::Unsupported`], or fails for [`Mode::Require`],
//! and [`Probe::run`] runs no check.

use serde::{Deserialize, Serialize};

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
    pub cannot_execute_memfd: bool,
    pub cannot_fork: bool,
    pub cannot_use_clone3: bool,
    pub cannot_create_temp_file: bool,
    pub cannot_change_file_metadata: bool,
    pub cannot_open_home: bool,
    pub cannot_read_other_process: bool,
    pub cannot_open_unix_socket: bool,
    pub cannot_use_sysv_ipc: bool,
    pub cannot_open_other_inet_protocol: bool,
    pub cannot_open_non_route_netlink: bool,
    pub cannot_signal_other_process: bool,
    pub cannot_signal_process_group: bool,
    pub cannot_set_signal_owner: bool,
    pub cannot_inject_terminal_input: bool,
    pub cannot_ptrace_other_process: bool,
    pub cannot_reschedule_other_process: bool,
    pub cannot_read_other_limits: bool,
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
            cannot_execute_memfd,
            cannot_fork,
            cannot_use_clone3,
            cannot_create_temp_file,
            cannot_change_file_metadata,
            cannot_open_home,
            cannot_read_other_process,
            cannot_open_unix_socket,
            cannot_use_sysv_ipc,
            cannot_open_other_inet_protocol,
            cannot_open_non_route_netlink,
            cannot_signal_other_process,
            cannot_signal_process_group,
            cannot_set_signal_owner,
            cannot_inject_terminal_input,
            cannot_ptrace_other_process,
            cannot_reschedule_other_process,
            cannot_read_other_limits,
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
            cannot_execute_memfd,
            cannot_fork,
            cannot_use_clone3,
            cannot_create_temp_file,
            cannot_change_file_metadata,
            cannot_open_home,
            cannot_read_other_process,
            cannot_open_unix_socket,
            cannot_use_sysv_ipc,
            cannot_open_other_inet_protocol,
            cannot_open_non_route_netlink,
            cannot_signal_other_process,
            cannot_signal_process_group,
            cannot_set_signal_owner,
            cannot_inject_terminal_input,
            cannot_ptrace_other_process,
            cannot_reschedule_other_process,
            cannot_read_other_limits,
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

/// What the checks need from outside the sandbox: a loopback listener to connect to, and a second
/// process of the same user, a child that exits when the probe is dropped.
pub struct Probe {
    #[cfg(target_os = "linux")]
    targets: probe::Targets,
}

impl Probe {
    /// Call before [`apply`], which would refuse both.
    pub fn prepare() -> std::io::Result<Self> {
        Ok(Probe {
            #[cfg(target_os = "linux")]
            targets: probe::Targets::prepare()?,
        })
    }

    /// Tries each action the sandbox denies and each it must still allow.
    pub fn run(&self) -> Report {
        #[cfg(target_os = "linux")]
        {
            probe::run(&self.targets)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Report::default()
        }
    }
}

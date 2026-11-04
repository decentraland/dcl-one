//! The Linux sandbox the helper applies to itself. FFmpeg demuxes
//! attacker-controlled media, so before the IPC channels are adopted and
//! any untrusted byte arrives, the helper locks itself down with the two
//! unprivileged mechanisms the kernel offers:
//!
//! - **Landlock** (5.13+) restricts the filesystem: read-only access to
//!   the system library and configuration trees plus the helper's own
//!   directory, read-write only to the DRM render nodes VAAPI opens and to
//!   `/dev/null`. Nothing under the user's home is reachable, and no
//!   persistent write is possible anywhere. The `file:` protocol
//!   (`--allow-file-read`, local playback for development) widens *reads*
//!   to the whole tree; writes stay denied.
//! - **seccomp** denies process creation and introspection: `execve`,
//!   `execveat`, `fork`, `vfork`, `clone` without `CLONE_THREAD`, `ptrace`
//!   and `process_vm_*` fail with `EPERM`; `clone3` answers `ENOSYS` so
//!   glibc's thread creation falls back to the inspectable `clone`. New
//!   `AF_UNIX` sockets are denied too: both IPC channels are inherited
//!   socketpairs, so the only Unix-domain endpoints a compromised helper
//!   could still reach are the session bus and the display server, which
//!   Landlock does not mediate. `AF_NETLINK` stays open because glibc's
//!   `getaddrinfo` enumerates interfaces over it; name resolution itself
//!   goes to the stub resolver over UDP, with `nss-resolve` and `nscd`
//!   reporting unavailable when their sockets are refused.
//!
//! This is weaker than the macOS Seatbelt profile: device and Internet
//! access are not enumerated, and on a kernel without Landlock
//! only the seccomp half applies (reported through the helper's log line
//! so the host can tell). Fail closed where the mechanism exists: a
//! kernel that has Landlock but refuses the ruleset ends the helper.

use anyhow::{Context as _, Result, anyhow, bail};
use landlock::{
    ABI, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
    path_beneath_rules,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};
use std::collections::BTreeMap;
use std::path::Path;

/// What the lockdown achieved, for the helper's startup log line.
pub struct Applied {
    pub landlock: &'static str,
}

/// Applies the sandbox to the current process. `plugin_dir` is the
/// helper's own directory (the FFmpeg libraries resolve from it);
/// `allow_media_file_read` opens broad file reads for the `file:` protocol.
pub fn apply(plugin_dir: &Path, allow_media_file_read: bool) -> Result<Applied> {
    let landlock = apply_landlock(plugin_dir, allow_media_file_read)?;
    apply_seccomp()?;
    Ok(Applied { landlock })
}

fn apply_landlock(plugin_dir: &Path, allow_media_file_read: bool) -> Result<&'static str> {
    // V3 adds truncate to the handled set; a kernel below it degrades to
    // what it has (best effort), a kernel without Landlock enforces nothing
    let abi = ABI::V3;
    let read_only = [
        "/usr", "/lib", "/lib64", "/lib32", "/bin", "/sbin", "/etc", "/opt", "/proc", "/sys",
        "/dev", "/run", "/var/lib",
    ];
    let read_write = ["/dev/dri", "/dev/null"];

    let mut ruleset = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .context("landlock: handle filesystem access")?
        .create()
        .context("landlock: create ruleset")?;

    if allow_media_file_read {
        ruleset = ruleset
            .add_rules(path_beneath_rules(["/"], AccessFs::from_read(abi)))
            .context("landlock: allow media file reads")?;
    } else {
        ruleset = ruleset
            .add_rules(path_beneath_rules(read_only, AccessFs::from_read(abi)))
            .context("landlock: system read rules")?
            .add_rules(path_beneath_rules([plugin_dir], AccessFs::from_read(abi)))
            .context("landlock: plugin directory rule")?;
    }
    ruleset = ruleset
        .add_rules(path_beneath_rules(read_write, AccessFs::from_all(abi)))
        .context("landlock: device rules")?;

    let status = ruleset.restrict_self().context("landlock: restrict_self")?;
    Ok(match status.ruleset {
        RulesetStatus::FullyEnforced => "landlock fully enforced",
        RulesetStatus::PartiallyEnforced => "landlock partially enforced (older kernel)",
        RulesetStatus::NotEnforced => "landlock unavailable on this kernel",
    })
}

fn apply_seccomp() -> Result<()> {
    let arch = std::env::consts::ARCH
        .try_into()
        .map_err(|e| anyhow!("seccomp: unsupported architecture: {e}"))?;

    // CLONE_THREAD unset = a new process, not a thread
    let non_thread_clone = SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::MaskedEq(libc::CLONE_THREAD as u64),
            0,
        )
        .map_err(|e| anyhow!("seccomp: clone condition: {e}"))?,
    ])
    .map_err(|e| anyhow!("seccomp: clone rule: {e}"))?;

    // AF_UNIX in the domain argument: a new Unix-domain endpoint
    let unix_domain = SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Eq,
            libc::AF_UNIX as u64,
        )
        .map_err(|e| anyhow!("seccomp: socket domain condition: {e}"))?,
    ])
    .map_err(|e| anyhow!("seccomp: socket rule: {e}"))?;

    let mut denied: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for syscall in [
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
    ] {
        denied.insert(syscall, Vec::new());
    }
    #[cfg(target_arch = "x86_64")]
    {
        denied.insert(libc::SYS_fork, Vec::new());
        denied.insert(libc::SYS_vfork, Vec::new());
    }
    denied.insert(libc::SYS_clone, vec![non_thread_clone]);
    denied.insert(libc::SYS_socket, vec![unix_domain.clone()]);
    denied.insert(libc::SYS_socketpair, vec![unix_domain]);

    let eperm: BpfProgram = SeccompFilter::new(
        denied,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )
    .map_err(|e| anyhow!("seccomp: build EPERM filter: {e}"))?
    .try_into()
    .map_err(|e| anyhow!("seccomp: compile EPERM filter: {e}"))?;

    // clone3 takes its flags through a struct the filter cannot read:
    // refuse it as "not implemented" so glibc retries with clone
    let mut unsupported: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    unsupported.insert(libc::SYS_clone3, Vec::new());
    let enosys: BpfProgram = SeccompFilter::new(
        unsupported,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::ENOSYS as u32),
        arch,
    )
    .map_err(|e| anyhow!("seccomp: build ENOSYS filter: {e}"))?
    .try_into()
    .map_err(|e| anyhow!("seccomp: compile ENOSYS filter: {e}"))?;

    for (name, program) in [("EPERM", &eperm), ("ENOSYS", &enosys)] {
        if let Err(e) = seccompiler::apply_filter(program) {
            bail!("seccomp: apply {name} filter: {e}");
        }
    }
    Ok(())
}

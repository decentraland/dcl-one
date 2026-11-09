use crate::{Landlock, Mode, Status};
use anyhow::{anyhow, bail, Context as _, Result};
use landlock::{
    path_beneath_rules, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
    Scope, ABI,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// Read-only, and only where they exist. A symlink grants its target.
const READ_ONLY: &[&str] = &[
    // libraries the resolver and OpenSSL load after startup
    "/lib",
    "/lib64",
    "/usr/lib",
    "/usr/lib64",
    "/usr/local/lib",
    "/etc/ld.so.cache",
    // OpenSSL configuration and CA certificates, per distribution
    "/etc/ssl",
    "/etc/pki",
    "/etc/ca-certificates",
    "/usr/share/ca-certificates",
    "/usr/local/share/ca-certificates",
    // name resolution
    "/etc/resolv.conf",
    "/etc/hosts",
    "/etc/nsswitch.conf",
    "/etc/gai.conf",
    "/etc/host.conf",
    "/run/systemd/resolve",
    // the local time zone scenes see in Date
    "/etc/localtime",
    "/usr/share/zoneinfo",
    // CPU count and quota, V8's memory strategy, randomness
    "/sys/devices/system/cpu",
    "/sys/fs/cgroup",
    "/proc/sys/vm/overcommit_memory",
    "/dev/urandom",
];

const READ_WRITE: &[&str] = &["/dev/null"];

pub fn apply(mode: Mode) -> Result<Status> {
    if mode == Mode::Off {
        return Ok(Status {
            landlock: Landlock::NotEnforced,
            seccomp: false,
            no_new_privs: false,
        });
    }

    let threads = std::fs::read_dir("/proc/self/task")
        .context("count the threads in /proc/self/task")?
        .count();
    if threads != 1 {
        bail!("the sandbox must be applied while the process has one thread; it has {threads}");
    }

    let no_core = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: plain syscalls with valid arguments.
    unsafe {
        check(
            libc::setrlimit(libc::RLIMIT_CORE, &no_core),
            "setrlimit(RLIMIT_CORE)",
        )?;
        check(
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0),
            "prctl(PR_SET_DUMPABLE)",
        )?;
        check(
            libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0),
            "prctl(PR_SET_NO_NEW_PRIVS)",
        )?;
    }

    let status = Status {
        landlock: apply_landlock()?,
        seccomp: apply_seccomp()?,
        no_new_privs: true,
    };
    if mode == Mode::Require
        && !(status.seccomp
            && matches!(
                status.landlock,
                Landlock::FullyEnforced | Landlock::PartiallyEnforced
            ))
    {
        bail!("the kernel does not offer the required sandbox: {status:?}");
    }
    Ok(status)
}

fn check(ret: libc::c_int, call: &str) -> Result<()> {
    if ret != 0 {
        return Err(io::Error::last_os_error()).context(call.to_owned());
    }
    Ok(())
}

fn apply_landlock() -> Result<Landlock> {
    // The newest ABI this crate knows; an older kernel enforces the subset it has.
    let abi = ABI::V9;
    let read = AccessFs::ReadFile | AccessFs::ReadDir;
    let read_write = read | AccessFs::WriteFile | AccessFs::Truncate;
    let own_proc = Path::new("/proc")
        .join(std::fs::read_link("/proc/self").context("landlock: resolve /proc/self")?);

    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .context("landlock: handle filesystem access")?
        .scope(Scope::from_all(abi))
        .context("landlock: scope signals and abstract sockets")?
        .create()
        .context("landlock: create the ruleset")?
        .add_rules(path_beneath_rules(READ_ONLY, read))
        .context("landlock: read-only paths")?
        .add_rules(path_beneath_rules([own_proc], read))
        .context("landlock: this process's /proc entry")?
        .add_rules(path_beneath_rules(READ_WRITE, read_write))
        .context("landlock: read-write paths")?
        .restrict_self()
        .context("landlock: restrict this process")?;
    Ok(match status.ruleset {
        RulesetStatus::FullyEnforced => Landlock::FullyEnforced,
        RulesetStatus::PartiallyEnforced => Landlock::PartiallyEnforced,
        RulesetStatus::NotEnforced => Landlock::NotEnforced,
    })
}

/// Returns false only when the kernel has no seccomp at all.
fn apply_seccomp() -> Result<bool> {
    // SAFETY: plain syscall with valid arguments.
    if unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) } < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EINVAL) {
            return Ok(false);
        }
        return Err(error).context("seccomp: prctl(PR_GET_SECCOMP)");
    }

    let arch = std::env::consts::ARCH
        .try_into()
        .map_err(|e| anyhow!("seccomp: unsupported architecture: {e}"))?;
    let programs = [
        ("EPERM", filter(denied_syscalls()?, libc::EPERM, arch)?),
        // clone3 passes its flags in memory the filter cannot read; ENOSYS makes glibc retry
        // with clone, whose flags it can
        (
            "ENOSYS",
            filter(
                BTreeMap::from([(libc::SYS_clone3, vec![])]),
                libc::ENOSYS,
                arch,
            )?,
        ),
        #[cfg(target_arch = "x86_64")]
        ("x32", x32_filter()),
    ];

    for (name, program) in programs {
        seccompiler::apply_filter_all_threads(&program)
            .map_err(|e| anyhow!("seccomp: apply the {name} filter: {e}"))?;
    }
    Ok(true)
}

fn filter(
    rules: BTreeMap<i64, Vec<SeccompRule>>,
    errno: libc::c_int,
    arch: seccompiler::TargetArch,
) -> Result<BpfProgram> {
    SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(errno as u32),
        arch,
    )
    .map_err(|e| anyhow!("seccomp: build the filter: {e}"))?
    .try_into()
    .map_err(|e| anyhow!("seccomp: compile the filter: {e}"))
}

fn denied_syscalls() -> Result<BTreeMap<i64, Vec<SeccompRule>>> {
    let pid = u64::from(std::process::id());
    let mut denied = BTreeMap::new();

    for syscall in [
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_getfd,
        libc::SYS_pidfd_send_signal,
        libc::SYS_kcmp,
        libc::SYS_process_madvise,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_chroot,
        libc::SYS_pivot_root,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_userfaultfd,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_open_by_handle_at,
        libc::SYS_kexec_load,
        libc::SYS_kexec_file_load,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
        libc::SYS_swapon,
        libc::SYS_swapoff,
        libc::SYS_reboot,
        libc::SYS_acct,
        libc::SYS_personality,
        #[cfg(target_arch = "x86_64")]
        libc::SYS_fork,
        #[cfg(target_arch = "x86_64")]
        libc::SYS_vfork,
    ] {
        denied.insert(syscall, vec![]);
    }

    // a new process, not a thread
    denied.insert(
        libc::SYS_clone,
        vec![rule([(
            0,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::MaskedEq(libc::CLONE_THREAD as u64),
            0,
        )])?],
    );

    // the first argument names a process; only this one may be signalled
    for syscall in [
        libc::SYS_kill,
        libc::SYS_tkill,
        libc::SYS_tgkill,
        libc::SYS_rt_sigqueueinfo,
        libc::SYS_rt_tgsigqueueinfo,
    ] {
        denied.insert(
            syscall,
            vec![rule([(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Ne, pid)])?],
        );
    }
    // pid 0 is this process
    denied.insert(
        libc::SYS_prlimit64,
        vec![rule([
            (0, SeccompCmpArgLen::Dword, SeccompCmpOp::Ne, 0),
            (0, SeccompCmpArgLen::Dword, SeccompCmpOp::Ne, pid),
        ])?],
    );

    // IPv4 and IPv6 for fetch and WebSocket; route netlink for the resolver's interface listing
    let socket_rules = vec![
        rule([
            (
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::AF_INET as u64,
            ),
            (
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::AF_INET6 as u64,
            ),
            (
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::AF_NETLINK as u64,
            ),
        ])?,
        rule([
            (
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Eq,
                libc::AF_NETLINK as u64,
            ),
            (
                2,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::NETLINK_ROUTE as u64,
            ),
        ])?,
    ];
    denied.insert(libc::SYS_socket, socket_rules.clone());
    denied.insert(libc::SYS_socketpair, socket_rules);

    Ok(denied)
}

fn rule<const N: usize>(
    conditions: [(u8, SeccompCmpArgLen, SeccompCmpOp, u64); N],
) -> Result<SeccompRule> {
    let conditions = conditions
        .into_iter()
        .map(|(arg, len, op, value)| {
            SeccompCondition::new(arg, len, op, value)
                .map_err(|e| anyhow!("seccomp: condition on argument {arg}: {e}"))
        })
        .collect::<Result<Vec<_>>>()?;
    SeccompRule::new(conditions).map_err(|e| anyhow!("seccomp: rule: {e}"))
}

/// The x32 ABI shares the x86_64 audit architecture with its own syscall numbers, which the
/// numbered rules above would not match.
#[cfg(target_arch = "x86_64")]
fn x32_filter() -> BpfProgram {
    use seccompiler::sock_filter;
    const X32_SYSCALL_BIT: u32 = 0x4000_0000;
    let stmt = |code: u32, k: u32| sock_filter {
        code: code as u16,
        jt: 0,
        jf: 0,
        k,
    };
    vec![
        // seccomp_data.nr
        stmt(libc::BPF_LD | libc::BPF_W | libc::BPF_ABS, 0),
        sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as u16,
            jt: 0,
            jf: 1,
            k: X32_SYSCALL_BIT,
        },
        stmt(
            libc::BPF_RET | libc::BPF_K,
            libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
        ),
        stmt(libc::BPF_RET | libc::BPF_K, libc::SECCOMP_RET_ALLOW),
    ]
}

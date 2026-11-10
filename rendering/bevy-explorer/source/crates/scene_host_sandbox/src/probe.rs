use crate::Report;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::process::{Command, Stdio};

const FIOSETOWN: libc::c_ulong = 0x8901;

pub struct Targets {
    loopback: TcpListener,
    other: OtherProcess,
}

impl Targets {
    pub fn prepare() -> io::Result<Self> {
        Ok(Targets {
            loopback: TcpListener::bind("127.0.0.1:0")?,
            other: OtherProcess::start()?,
        })
    }
}

/// A child of the same user that waits on a pipe and exits when this side closes it. A child,
/// so Yama's default ptrace scope does not refuse it before the sandbox can.
struct OtherProcess {
    pid: libc::pid_t,
    release: Option<OwnedFd>,
}

impl OtherProcess {
    fn start() -> io::Result<Self> {
        let mut fds = [0; 2];
        // SAFETY: pipe2 fills both descriptors on success.
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: both descriptors are new and owned here.
        let (wait, release) =
            unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        // SAFETY: the child calls only close, read and _exit, which are async-signal-safe.
        match unsafe { libc::fork() } {
            -1 => Err(io::Error::last_os_error()),
            0 => unsafe {
                libc::close(release.as_raw_fd());
                let mut byte = 0u8;
                loop {
                    let read = libc::read(wait.as_raw_fd(), (&raw mut byte).cast(), 1);
                    if read < 0 && *libc::__errno_location() == libc::EINTR {
                        continue;
                    }
                    libc::_exit(0);
                }
            },
            pid => Ok(OtherProcess {
                pid,
                release: Some(release),
            }),
        }
    }
}

impl Drop for OtherProcess {
    fn drop(&mut self) {
        drop(self.release.take());
        // SAFETY: waiting on our own child, which exits once the pipe is closed.
        unsafe { libc::waitpid(self.pid, std::ptr::null_mut(), 0) };
    }
}

pub fn run(targets: &Targets) -> Report {
    let other = targets.other.pid;
    // fork first: the process has one thread until can_run_thread
    let cannot_fork = cannot_fork();
    Report {
        cannot_spawn_program: cannot_spawn_program(),
        cannot_replace_program: cannot_replace_program(),
        cannot_execute_memfd: cannot_execute_memfd(),
        cannot_fork,
        cannot_use_clone3: cannot_use_clone3(),
        cannot_create_temp_file: cannot_create_temp_file(),
        cannot_change_file_metadata: cannot_change_file_metadata(),
        cannot_open_home: cannot_open_home(),
        cannot_read_other_process: refused(fs::read(format!("/proc/{other}/status"))),
        cannot_open_unix_socket: refused(UnixDatagram::unbound()) && refused(UnixStream::pair()),
        cannot_open_non_route_netlink: cannot_open_non_route_netlink(),
        // SAFETY: signal 0 only checks permission.
        cannot_signal_other_process: last_call_denied(unsafe { libc::kill(other, 0) }.into()),
        cannot_signal_process_group: cannot_signal_process_group(),
        cannot_set_signal_owner: cannot_set_signal_owner(&targets.loopback, other),
        cannot_inject_terminal_input: cannot_inject_terminal_input(),
        cannot_ptrace_other_process: cannot_ptrace(other),
        cannot_reschedule_other_process: cannot_reschedule(other),
        cannot_read_other_limits: cannot_read_limits(other),
        can_run_thread: std::thread::spawn(|| 42).join().is_ok_and(|v| v == 42),
        can_run_jit_code: can_run_jit_code(),
        can_connect_loopback_tcp: can_connect_loopback_tcp(&targets.loopback),
        can_resolve_localhost: ("localhost", 0)
            .to_socket_addrs()
            .is_ok_and(|mut addrs| addrs.any(|addr| addr.ip().is_loopback())),
        can_write_dev_null: OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .and_then(|mut null| null.write_all(b"probe"))
            .is_ok(),
        can_read_own_proc: fs::read_to_string("/proc/self/status").is_ok_and(|s| !s.is_empty()),
        can_signal_self: can_signal_self(),
    }
}

/// EPERM and EACCES: what seccomp and Landlock answer.
fn denied(error: &io::Error) -> bool {
    error.kind() == ErrorKind::PermissionDenied
}

fn refused<T>(result: io::Result<T>) -> bool {
    result.as_ref().is_err_and(denied)
}

fn last_call_denied(ret: libc::c_long) -> bool {
    ret == -1 && denied(&io::Error::last_os_error())
}

fn cannot_spawn_program() -> bool {
    match Command::new("/bin/sh")
        .args(["-c", "exit 0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            let _ = child.wait();
            false
        }
        Err(error) => denied(&error),
    }
}

fn cannot_replace_program() -> bool {
    // argv points into the never-mapped first page, so an execve the sandbox allows fails with
    // EFAULT instead of replacing this process
    let unreadable_argv = std::ptr::dangling::<*const libc::c_char>();
    // SAFETY: the kernel validates every pointer; no memory of this process is touched.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_execve,
            c"/bin/sh".as_ptr(),
            unreadable_argv,
            std::ptr::null::<*const libc::c_char>(),
        )
    };
    last_call_denied(ret)
}

fn cannot_fork() -> bool {
    // SAFETY: the child only calls _exit.
    let libc_fork = fork_denied(|| unsafe { libc::fork() }.into());
    #[cfg(target_arch = "x86_64")]
    // SAFETY: as above.
    let fork_syscall = fork_denied(|| unsafe { libc::syscall(libc::SYS_fork) });
    #[cfg(not(target_arch = "x86_64"))]
    let fork_syscall = true;
    libc_fork && fork_syscall
}

fn fork_denied(fork: impl FnOnce() -> libc::c_long) -> bool {
    match fork() {
        // SAFETY: async-signal-safe exit of the child.
        0 => unsafe { libc::_exit(0) },
        -1 => denied(&io::Error::last_os_error()),
        child => {
            // SAFETY: waiting on our own child.
            unsafe { libc::waitpid(child as libc::pid_t, std::ptr::null_mut(), 0) };
            false
        }
    }
}

fn cannot_create_temp_file() -> bool {
    let path = std::env::temp_dir().join(format!("scene-host-sandbox-{}", std::process::id()));
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(_) => {
            let _ = fs::remove_file(&path);
            false
        }
        Err(error) => denied(&error),
    }
}

fn cannot_open_home() -> bool {
    std::env::var_os("HOME").is_some_and(|home| refused(fs::read_dir(home)))
}

fn cannot_signal_process_group() -> bool {
    // SAFETY: signal 0 only checks permission.
    unsafe {
        last_call_denied(libc::kill(0, 0).into()) && last_call_denied(libc::kill(-1, 0).into())
    }
}

fn cannot_ptrace(other: libc::pid_t) -> bool {
    // SAFETY: on success the other process is interrupted, waited for and detached again.
    unsafe {
        if libc::ptrace(libc::PTRACE_SEIZE, other, 0, 0) == 0 {
            libc::ptrace(libc::PTRACE_INTERRUPT, other, 0, 0);
            libc::waitpid(other, std::ptr::null_mut(), libc::__WALL);
            libc::ptrace(libc::PTRACE_DETACH, other, 0, 0);
            return false;
        }
    }
    denied(&io::Error::last_os_error())
}

/// Each call sets what the other process already has.
fn cannot_reschedule(other: libc::pid_t) -> bool {
    // SAFETY: getters fill locals; setters pass back the values just read.
    unsafe {
        let nice = libc::getpriority(libc::PRIO_PROCESS, other as libc::id_t);
        let priority = last_call_denied(
            libc::setpriority(libc::PRIO_PROCESS, other as libc::id_t, nice).into(),
        );
        let mut cpus: libc::cpu_set_t = std::mem::zeroed();
        let size = std::mem::size_of::<libc::cpu_set_t>();
        let affinity = libc::sched_getaffinity(other, size, &mut cpus) == 0
            && last_call_denied(libc::sched_setaffinity(other, size, &cpus).into());
        let mut head: *mut libc::c_void = std::ptr::null_mut();
        let mut len: libc::size_t = 0;
        let robust_list = last_call_denied(libc::syscall(
            libc::SYS_get_robust_list,
            other,
            &mut head,
            &mut len,
        ));
        priority && affinity && robust_list
    }
}

fn cannot_read_limits(other: libc::pid_t) -> bool {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: reads into a local; sets nothing.
    let ret = unsafe { libc::prlimit(other, libc::RLIMIT_NOFILE, std::ptr::null(), &mut limit) };
    last_call_denied(ret.into())
}

fn cannot_set_signal_owner(socket: &TcpListener, other: libc::pid_t) -> bool {
    let fd = socket.as_raw_fd();
    // SAFETY: plain calls on a socket this process owns; an owner set here is cleared again.
    unsafe {
        let fcntl = libc::fcntl(fd, libc::F_SETOWN, other);
        let fcntl_denied = last_call_denied(fcntl.into());
        let ioctl = libc::ioctl(fd, FIOSETOWN as _, &other);
        let ioctl_denied = last_call_denied(ioctl.into());
        if fcntl == 0 || ioctl == 0 {
            libc::fcntl(fd, libc::F_SETOWN, 0);
        }
        fcntl_denied && ioctl_denied
    }
}

/// The sandbox refuses before the terminal check, which /dev/null fails without it.
fn cannot_inject_terminal_input() -> bool {
    let Ok(null) = File::open("/dev/null") else {
        return false;
    };
    let byte = b'x';
    // SAFETY: the argument is a valid byte.
    let ret = unsafe { libc::ioctl(null.as_raw_fd(), libc::TIOCSTI, &byte) };
    last_call_denied(ret.into())
}

fn cannot_open_non_route_netlink() -> bool {
    // SAFETY: a plain syscall; a socket it returns is closed.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if fd >= 0 {
        // SAFETY: the descriptor was just returned.
        unsafe { libc::close(fd) };
        return false;
    }
    denied(&io::Error::last_os_error())
}

/// clone3 is answered with ENOSYS, so glibc falls back to clone, whose flags the filter reads;
/// without the sandbox an empty argument fails with EINVAL.
fn cannot_use_clone3() -> bool {
    // SAFETY: a zero size is rejected before the pointer is read.
    let ret = unsafe { libc::syscall(libc::SYS_clone3, std::ptr::null::<u8>(), 0usize) };
    ret == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::ENOSYS)
}

fn memfd() -> io::Result<File> {
    // SAFETY: a plain syscall with a valid name.
    let fd = unsafe { libc::memfd_create(c"scene-host-sandbox-probe".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the descriptor was just returned and is owned here.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Landlock does not mediate memory-backed files, so only seccomp stops this.
fn cannot_execute_memfd() -> bool {
    let Ok(mut file) = memfd() else {
        return false;
    };
    if file.write_all(b"not a program").is_err() {
        return false;
    }
    // argv points into the never-mapped first page and the content is no program, so an
    // execveat the sandbox allows fails instead of replacing this process
    let unreadable_argv = std::ptr::dangling::<*const libc::c_char>();
    // SAFETY: the kernel validates every pointer; no memory of this process is touched.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_execveat,
            file.as_raw_fd(),
            c"".as_ptr(),
            unreadable_argv,
            std::ptr::null::<*const libc::c_char>(),
            libc::AT_EMPTY_PATH,
        )
    };
    last_call_denied(ret)
}

/// On a file this process created, so each call works without the sandbox: mode, owner, times,
/// extended attributes and a truncate by path.
fn cannot_change_file_metadata() -> bool {
    let Ok(file) = memfd() else {
        return false;
    };
    let fd = file.as_raw_fd();
    let path = std::ffi::CString::new(format!("/proc/self/fd/{fd}")).expect("no NUL");
    // SAFETY: plain syscalls on a file this process owns, with valid arguments.
    unsafe {
        let mode = last_call_denied(libc::fchmod(fd, 0o600).into());
        let owner = last_call_denied(libc::fchown(fd, u32::MAX, u32::MAX).into());
        let times = last_call_denied(libc::futimens(fd, std::ptr::null()).into());
        let xattr = last_call_denied(
            libc::fsetxattr(fd, c"user.probe".as_ptr(), b"1".as_ptr().cast(), 1, 0).into(),
        );
        let truncate = last_call_denied(libc::truncate(path.as_ptr(), 0).into());
        mode && owner && times && xattr && truncate
    }
}

fn can_signal_self() -> bool {
    // SAFETY: signal 0 only checks permission.
    unsafe {
        let pid = libc::getpid();
        let tid = libc::syscall(libc::SYS_gettid);
        libc::kill(pid, 0) == 0 && libc::syscall(libc::SYS_tgkill, pid, tid, 0) == 0
    }
}

fn can_connect_loopback_tcp(loopback: &TcpListener) -> bool {
    let Ok(addr) = loopback.local_addr() else {
        return false;
    };
    let Ok(stream) = TcpStream::connect(addr) else {
        return false;
    };
    let Ok((_, peer)) = loopback.accept() else {
        return false;
    };
    stream.local_addr().is_ok_and(|local| local == peer)
}

/// `return 42` in machine code.
#[cfg(target_arch = "x86_64")]
const RETURN_42: Option<&[u8]> = Some(&[0xb8, 0x2a, 0x00, 0x00, 0x00, 0xc3]);
#[cfg(target_arch = "aarch64")]
const RETURN_42: Option<&[u8]> = Some(&[0x40, 0x05, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]);
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const RETURN_42: Option<&[u8]> = None;

fn can_run_jit_code() -> bool {
    let Some(code) = RETURN_42 else {
        return false;
    };
    // SAFETY: a private anonymous page this function maps, fills, executes and unmaps.
    unsafe {
        let len = libc::sysconf(libc::_SC_PAGESIZE) as usize;
        let page = libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if page == libc::MAP_FAILED {
            return false;
        }
        std::ptr::copy_nonoverlapping(code.as_ptr(), page.cast::<u8>(), code.len());
        // the instruction cache does not see data writes on its own
        #[cfg(target_arch = "aarch64")]
        std::arch::asm!("dc cvau, {0}", "dsb ish", "ic ivau, {0}", "dsb ish", "isb", in(reg) page);
        let ran = libc::mprotect(page, len, libc::PROT_READ | libc::PROT_EXEC) == 0 && {
            let function = std::mem::transmute::<*mut libc::c_void, extern "C" fn() -> u32>(page);
            function() == 42
        };
        libc::munmap(page, len);
        ran
    }
}

use crate::Report;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::process::{Command, Stdio};

pub fn run(loopback: &TcpListener) -> Report {
    // fork first: the process has one thread until can_run_thread
    let cannot_fork = cannot_fork();
    Report {
        cannot_spawn_program: cannot_spawn_program(),
        cannot_replace_program: cannot_replace_program(),
        cannot_fork,
        cannot_create_temp_file: cannot_create_temp_file(),
        cannot_open_home: cannot_open_home(),
        cannot_read_init_environ: refused(fs::read("/proc/1/environ")),
        cannot_open_unix_socket: refused(UnixDatagram::unbound()) && refused(UnixStream::pair()),
        cannot_signal_parent: cannot_signal_parent(),
        cannot_ptrace_parent: cannot_ptrace_parent(),
        can_run_thread: std::thread::spawn(|| 42).join().is_ok_and(|v| v == 42),
        can_run_jit_code: can_run_jit_code(),
        can_connect_loopback_tcp: can_connect_loopback_tcp(loopback),
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

fn cannot_signal_parent() -> bool {
    // SAFETY: signal 0 only checks permission.
    last_call_denied(unsafe { libc::kill(libc::getppid(), 0) }.into())
}

fn cannot_ptrace_parent() -> bool {
    // SAFETY: on success the parent is interrupted, waited for and detached again.
    unsafe {
        let parent = libc::getppid();
        if libc::ptrace(libc::PTRACE_SEIZE, parent, 0, 0) == 0 {
            libc::ptrace(libc::PTRACE_INTERRUPT, parent, 0, 0);
            libc::waitpid(parent, std::ptr::null_mut(), libc::__WALL);
            libc::ptrace(libc::PTRACE_DETACH, parent, 0, 0);
            return false;
        }
    }
    denied(&io::Error::last_os_error())
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

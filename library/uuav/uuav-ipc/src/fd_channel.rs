//! Out-of-band Linux channel for shared-memory frame slots.
//!
//! The memfd of every slot plane travels as an `SCM_RIGHTS` file
//! descriptor over a second `AF_UNIX` `SOCK_SEQPACKET` socketpair (the
//! control channel's framing cannot carry descriptors), tagged with the
//! (player, generation, slot, plane) cell it belongs to and the mapping
//! length.
//!
//! The pair is private to the two processes — nothing can look it up —
//! but, as on macOS, the receiver still authenticates every message: with
//! `SO_PASSCRED` the kernel attaches the sender's real pid, which must be
//! the helper this session spawned. Anything else is closed unread.

use anyhow::{Context as _, Result, bail, ensure};
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

use crate::channel_unix::set_option;

/// Identifies which shared-memory cell a transferred descriptor backs;
/// pairs with the control-channel `TextureSet` announcement.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceTag {
    pub player: u64,
    pub generation: u32,
    pub slot: u8,
    pub plane: u8,
}

/// 'UUAV'. Shape marker on surface messages; sender-controlled, so it is
/// validation, never authentication.
const SURFACE_MAGIC: u32 = 0x5555_4156;

#[repr(C)]
#[derive(Clone, Copy)]
struct Header {
    magic: u32,
    generation: u32,
    player: u64,
    /// Bytes of the mapping behind the descriptor.
    len: u64,
    slot: u8,
    plane: u8,
    _pad: [u8; 6],
}

const HEADER_LEN: usize = size_of::<Header>();

/// Bytes of one descriptor in an `SCM_RIGHTS` payload.
const FD_LEN: usize = size_of::<RawFd>();

/// Room for one `SCM_RIGHTS` fd and one `SCM_CREDENTIALS` block.
const CMSG_SPACE_LEN: usize = 128;

/// Client side: owns the receiving end, created before the helper spawn.
pub struct Receiver {
    fd: OwnedFd,
    /// The only pid [`Self::recv`] accepts messages from. `None` (before
    /// [`Self::expect_sender`] arms it) accepts nothing: unarmed reception
    /// fails closed.
    sender_pid: Option<u32>,
}

/// The helper's pre-created end, alive in the parent only between
/// [`Receiver::pair`] and the spawn.
pub struct ChildHandoff {
    fd: OwnedFd,
}

impl ChildHandoff {
    /// The `--surfaces` argv value: the fd number, which survives exec.
    pub fn arg(&self) -> String {
        self.fd.as_raw_fd().to_string()
    }
}

impl Receiver {
    /// Creates both ends. The receiving end is `FD_CLOEXEC` and asks the
    /// kernel for sender credentials; the child end stays inheritable.
    pub fn pair() -> Result<(Self, ChildHandoff)> {
        let mut fds = [0 as libc::c_int; 2];
        if unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) }
            != 0
        {
            return Err(std::io::Error::last_os_error()).context("socketpair(SEQPACKET)");
        }
        let [parent, child] = fds;
        // SAFETY: both fds were just created and are owned by nothing else
        let parent = unsafe { OwnedFd::from_raw_fd(parent) };
        let child = unsafe { OwnedFd::from_raw_fd(child) };

        if unsafe { libc::fcntl(parent.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            return Err(std::io::Error::last_os_error()).context("set FD_CLOEXEC");
        }
        set_option(parent.as_raw_fd(), libc::SO_PASSCRED, 1).context("set SO_PASSCRED")?;

        Ok((
            Self {
                fd: parent,
                sender_pid: None,
            },
            ChildHandoff { fd: child },
        ))
    }

    /// Arms sender authentication with the pid of the helper this session
    /// spawned (the kernel-truth `Child` pid, not anything self-reported).
    pub const fn expect_sender(&mut self, pid: u32) {
        self.sender_pid = Some(pid);
    }

    /// One transferred slot plane: the tag, the descriptor the caller now
    /// owns, and the mapping length the sender declared. `Ok(None)` on
    /// timeout or on a rejected message (wrong sender or shape: closed,
    /// never surfaced); an error means the channel is gone.
    pub fn recv(&self, timeout_ms: u32) -> Result<Option<(SurfaceTag, OwnedFd, usize)>> {
        let mut pollfd = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout = libc::c_int::try_from(timeout_ms).unwrap_or(libc::c_int::MAX);
        match unsafe { libc::poll(&mut pollfd, 1, timeout) } {
            0 => return Ok(None),
            n if n > 0 => {}
            _ => {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    return Ok(None);
                }
                return Err(err).context("poll surface channel");
            }
        }

        let mut header = [0u8; HEADER_LEN];
        let mut iov = libc::iovec {
            iov_base: header.as_mut_ptr().cast(),
            iov_len: HEADER_LEN,
        };
        let mut control = [0u8; CMSG_SPACE_LEN];
        let mut msg: libc::msghdr = unsafe { zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = CMSG_SPACE_LEN;

        let received = unsafe {
            libc::recvmsg(
                self.fd.as_raw_fd(),
                &mut msg,
                libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT,
            )
        };
        if received < 0 {
            let err = std::io::Error::last_os_error();
            return match err.raw_os_error() {
                Some(libc::EAGAIN | libc::EINTR) => Ok(None),
                _ => Err(err).context("recvmsg on surface channel"),
            };
        }
        if received == 0 {
            bail!("surface channel peer closed");
        }

        let (descriptor, sender) = take_ancillary(&msg);

        let truncated = msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0;
        let accepted = !truncated
            && usize::try_from(received).unwrap_or(0) == HEADER_LEN
            && sender.is_some()
            && sender == self.sender_pid;
        // a rejected message's descriptor closes with `descriptor` here
        if !accepted {
            return Ok(None);
        }
        let Some(descriptor) = descriptor else {
            return Ok(None);
        };

        // SAFETY: Header is repr(C) plain data and the buffer is exactly its size
        let parsed: Header = unsafe { std::ptr::read_unaligned(header.as_ptr().cast()) };
        if parsed.magic != SURFACE_MAGIC {
            return Ok(None);
        }
        let len = usize::try_from(parsed.len).context("mapping length")?;
        Ok(Some((
            SurfaceTag {
                player: parsed.player,
                generation: parsed.generation,
                slot: parsed.slot,
                plane: parsed.plane,
            },
            descriptor,
            len,
        )))
    }
}

/// The first `SCM_RIGHTS` descriptor (every further one is closed) and
/// the kernel-attached sender pid of a received message.
fn take_ancillary(msg: &libc::msghdr) -> (Option<OwnedFd>, Option<u32>) {
    let mut descriptor: Option<OwnedFd> = None;
    let mut sender: Option<u32> = None;
    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(msg) };
    while !cmsg.is_null() {
        let level = unsafe { (*cmsg).cmsg_level };
        let kind = unsafe { (*cmsg).cmsg_type };
        let data = unsafe { libc::CMSG_DATA(cmsg) };
        let payload = unsafe { (*cmsg).cmsg_len } as usize;
        let header_len = unsafe { libc::CMSG_LEN(0) } as usize;
        if level == libc::SOL_SOCKET && kind == libc::SCM_RIGHTS {
            let count = payload
                .saturating_sub(header_len)
                .checked_div(FD_LEN)
                .unwrap_or(0);
            for index in 0..count {
                let raw = unsafe { data.cast::<RawFd>().add(index).read_unaligned() };
                // SAFETY: the kernel just created this descriptor for us;
                // every one past the first closes right here
                let owned = unsafe { OwnedFd::from_raw_fd(raw) };
                if descriptor.is_none() {
                    descriptor = Some(owned);
                }
            }
        } else if level == libc::SOL_SOCKET && kind == libc::SCM_CREDENTIALS {
            let creds = unsafe { data.cast::<libc::ucred>().read_unaligned() };
            sender = u32::try_from(creds.pid).ok();
        }
        cmsg = unsafe { libc::CMSG_NXTHDR(msg, cmsg) };
    }
    (descriptor, sender)
}

/// Helper side: the inherited sending end.
pub struct Sender {
    fd: OwnedFd,
}

impl Sender {
    /// Adopts the inherited fd passed as the `--surfaces` argv value.
    pub fn from_arg(value: &str) -> Result<Self> {
        let fd: RawFd = value.parse().context("--surfaces is not an fd number")?;
        ensure!(fd >= 0, "--surfaces is a negative fd");
        // SAFETY: the fd was inherited for exactly this purpose and nothing
        // else in the helper owns it
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        })
    }

    /// Transfers a duplicate of `memfd` (the receiver owns its copy) with
    /// the tag and the mapping length.
    pub fn send(&self, tag: SurfaceTag, memfd: BorrowedFd<'_>, len: usize) -> Result<()> {
        let header = Header {
            magic: SURFACE_MAGIC,
            generation: tag.generation,
            player: tag.player,
            len: u64::try_from(len).context("mapping length")?,
            slot: tag.slot,
            plane: tag.plane,
            _pad: [0; 6],
        };
        let mut bytes = [0u8; HEADER_LEN];
        // SAFETY: Header is repr(C) plain data of exactly this size
        unsafe { std::ptr::write_unaligned(bytes.as_mut_ptr().cast(), header) };
        let mut iov = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: HEADER_LEN,
        };

        let fd_len = u32::try_from(FD_LEN).context("descriptor size")?;
        let space = unsafe { libc::CMSG_SPACE(fd_len) } as usize;
        let mut control = vec![0u8; space];
        let mut msg: libc::msghdr = unsafe { zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = space;
        let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
        ensure!(
            !cmsg.is_null(),
            "no room for the descriptor control message"
        );
        unsafe {
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len = libc::CMSG_LEN(fd_len) as usize;
            libc::CMSG_DATA(cmsg)
                .cast::<RawFd>()
                .write_unaligned(memfd.as_raw_fd());
        }

        loop {
            let sent = unsafe { libc::sendmsg(self.fd.as_raw_fd(), &msg, libc::MSG_NOSIGNAL) };
            if sent >= 0 {
                return Ok(());
            }
            let err = std::io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::EINTR) => {}
                Some(libc::EPIPE) => bail!("surface channel peer closed"),
                _ => return Err(err).context("sendmsg on surface channel"),
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::os::fd::AsFd;

    const TAG: SurfaceTag = SurfaceTag {
        player: 7,
        generation: 3,
        slot: 1,
        plane: 0,
    };

    fn memfd(len: usize) -> OwnedFd {
        let fd = unsafe { libc::memfd_create(c"uuav-test".as_ptr(), libc::MFD_CLOEXEC) };
        assert!(fd >= 0);
        let size = libc::off_t::try_from(len).unwrap();
        assert_eq!(unsafe { libc::ftruncate(fd, size) }, 0);
        unsafe { OwnedFd::from_raw_fd(fd) }
    }

    fn in_process_pair() -> (Receiver, Sender) {
        let (receiver, handoff) = Receiver::pair().unwrap();
        let sender = Sender::from_arg(&handoff.arg()).unwrap();
        // the sender now owns the fd; the handoff must not close it again
        std::mem::forget(handoff);
        (receiver, sender)
    }

    #[test]
    fn armed_receiver_accepts_the_expected_sender() {
        let (mut receiver, sender) = in_process_pair();
        receiver.expect_sender(std::process::id());
        let fd = memfd(4096);
        sender.send(TAG, fd.as_fd(), 4096).unwrap();

        let (tag, got, len) = receiver
            .recv(2000)
            .unwrap()
            .expect("authentic message rejected");
        assert_eq!(tag.player, TAG.player);
        assert_eq!(tag.generation, TAG.generation);
        assert_eq!(tag.slot, TAG.slot);
        assert_eq!(tag.plane, TAG.plane);
        assert_eq!(len, 4096);
        // a distinct descriptor to the same file
        assert_ne!(got.as_raw_fd(), fd.as_raw_fd());
        let mut stat: libc::stat = unsafe { zeroed() };
        assert_eq!(unsafe { libc::fstat(got.as_raw_fd(), &mut stat) }, 0);
        assert_eq!(stat.st_size, 4096);
    }

    #[test]
    fn unarmed_receiver_rejects_everything() {
        let (receiver, sender) = in_process_pair();
        let fd = memfd(16);
        sender.send(TAG, fd.as_fd(), 16).unwrap();
        assert!(receiver.recv(500).unwrap().is_none());
    }

    #[test]
    fn wrong_sender_pid_is_rejected_and_the_channel_survives() {
        let (mut receiver, sender) = in_process_pair();
        receiver.expect_sender(u32::MAX);
        let fd = memfd(16);
        sender.send(TAG, fd.as_fd(), 16).unwrap();
        assert!(receiver.recv(500).unwrap().is_none());

        receiver.expect_sender(std::process::id());
        sender.send(TAG, fd.as_fd(), 16).unwrap();
        assert!(receiver.recv(2000).unwrap().is_some());
    }

    #[test]
    fn idle_recv_times_out() {
        let (mut receiver, _sender) = in_process_pair();
        receiver.expect_sender(std::process::id());
        assert!(receiver.recv(50).unwrap().is_none());
    }
}

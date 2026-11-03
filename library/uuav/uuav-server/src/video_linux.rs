//! The video half of the adapter on Linux: drives the core's render events,
//! copies its CPU presentation planes into cross-process shared memory
//! slots, and publishes frame availability to the client.
//!
//! Per player: a generation of 3 slots x 2 single-plane memfds (Y 8-bit at
//! w x h, UV interleaved 8-bit at w/2 x h/2, both tightly packed). A
//! generation is announced once (descriptors over the fd channel +
//! `TextureSet` over the control channel) and becomes active on
//! `TextureSetAck`; until then the previous generation keeps publishing,
//! mirroring the core's own retire-grace on resolution change.

use crate::device::ProbeDevice;
use crate::state;
use anyhow::{Context as _, Result, anyhow, ensure};
use std::collections::HashMap;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::raw::c_void;
use uuav_core as core;
use uuav_ipc::channel::Channel;
use uuav_ipc::fd_channel;
use uuav_ipc::protocol::{PlayerId, ToClient};

const SLOTS: usize = 3;

/// One sealed, mapped memfd: the helper writes it, the client maps the
/// transferred descriptor read-only.
struct Shm {
    fd: OwnedFd,
    ptr: *mut u8,
    len: usize,
}

// the mapping is only touched from the serve-loop thread
unsafe impl Send for Shm {}

impl Shm {
    fn new(len: usize) -> Result<Self> {
        ensure!(len > 0, "shared plane needs a positive size");
        let raw = unsafe {
            libc::memfd_create(
                c"uuav-plane".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        if raw < 0 {
            return Err(std::io::Error::last_os_error()).context("memfd_create");
        }
        // SAFETY: freshly created, owned by nothing else
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let size = libc::off_t::try_from(len).context("plane size")?;
        if unsafe { libc::ftruncate(fd.as_raw_fd(), size) } != 0 {
            return Err(std::io::Error::last_os_error()).context("ftruncate(memfd)");
        }
        // the size is fixed for the file's lifetime: the client maps it
        // without ever being able to hit a shrunken page (SIGBUS)
        let seals = libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_ADD_SEALS, seals) } != 0 {
            return Err(std::io::Error::last_os_error()).context("seal memfd");
        }
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error()).context("mmap(memfd)");
        }
        Ok(Self {
            fd,
            ptr: ptr.cast::<u8>(),
            len,
        })
    }

    fn write(&mut self, src: *const c_void, len: usize) -> Result<()> {
        ensure!(
            len <= self.len,
            "frame plane ({len} bytes) exceeds the slot ({})",
            self.len
        );
        // SAFETY: the core's plane holds `len` bytes behind `src` (the
        // visible size the core reported) and the mapping holds `self.len`
        unsafe { std::ptr::copy_nonoverlapping(src.cast::<u8>(), self.ptr, len) };
        Ok(())
    }
}

impl Drop for Shm {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.ptr.cast::<c_void>(), self.len) };
    }
}

struct SlotPair {
    y: Shm,
    uv: Shm,
}

struct GenSlots {
    generation: u32,
    width: u32,
    height: u32,
    slots: [SlotPair; SLOTS],
    next_slot: usize,
}

impl GenSlots {
    const fn matches(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

#[derive(Default)]
struct PlayerVideo {
    next_generation: u32,
    /// Announced and acked: the generation frames are published into.
    active: Option<GenSlots>,
    /// Announced, awaiting `TextureSetAck`; `active` keeps publishing.
    pending: Option<GenSlots>,
}

pub struct VideoPump {
    surfaces: fd_channel::Sender,
    render_event: core::UUAVRenderEvent,
    players: HashMap<PlayerId, PlayerVideo>,
}

impl VideoPump {
    // the Result is the cross-platform shape: the GPU pumps can fail here
    #[allow(clippy::unnecessary_wraps)]
    pub fn new(_device: &ProbeDevice, surfaces: fd_channel::Sender) -> Result<Self> {
        Ok(Self {
            surfaces,
            render_event: core::uuav_get_render_callback(),
            players: HashMap::new(),
        })
    }

    /// One video tick over all live players: render event, generation
    /// management, slot copy, publish.
    pub fn tick(&mut self, ids: impl Iterator<Item = PlayerId>, channel: &mut Channel) {
        for id in ids {
            if let Err(e) = self.tick_player(id, channel) {
                // per-player video hiccups are not protocol failures; the
                // helper's own logs are the diagnostic channel
                eprintln!("uuav-helper: video tick for player {id}: {e}");
            }
        }
    }

    pub fn ack(&mut self, id: PlayerId, generation: u32) {
        if let Some(player) = self.players.get_mut(&id)
            && player
                .pending
                .as_ref()
                .is_some_and(|pending| pending.generation == generation)
        {
            // the client mapped the new set; switch over (the old mappings
            // stay alive through the client's own copies until it drops them)
            player.active = player.pending.take();
        }
    }

    pub fn remove_player(&mut self, id: PlayerId) {
        self.players.remove(&id);
    }

    fn tick_player(&mut self, id: PlayerId, channel: &mut Channel) -> Result<()> {
        // the core presents the due frame into its presentation planes
        (self.render_event)(id as i32);

        let Some((core_y, core_uv, width, height)) = query_core_planes(id) else {
            // no frame presented yet (opening, audio-only, closed)
            return Ok(());
        };

        let player = self.players.entry(id).or_default();

        if !player
            .active
            .as_ref()
            .is_some_and(|g| g.matches(width, height))
            && !player
                .pending
                .as_ref()
                .is_some_and(|g| g.matches(width, height))
        {
            player.next_generation = player.next_generation.wrapping_add(1);
            let generation = player.next_generation;
            let slots = create_generation(width, height)?;
            announce(
                &self.surfaces,
                channel,
                id,
                generation,
                width,
                height,
                &slots,
            )?;
            let created = GenSlots {
                generation,
                width,
                height,
                slots,
                next_slot: 0,
            };
            if player.active.is_none() {
                // nothing to keep publishing into; activate optimistically —
                // publishes are ignored by the client until it maps and acks
                player.active = Some(created);
            } else {
                player.pending = Some(created);
            }
        }

        let Some(active) = player.active.as_mut() else {
            return Ok(());
        };
        if !active.matches(width, height) {
            // resolution changed and the new generation is still pending
            // ack; keep the last published frame instead of writing stale
            // dimensions into the active slots
            return Ok(());
        }

        let slot_index = active.next_slot;
        active.next_slot = (active.next_slot + 1) % SLOTS;
        let slot = active
            .slots
            .get_mut(slot_index)
            .ok_or_else(|| anyhow!("slot index {slot_index} out of range"))?;

        let y_len = plane_len(width, height)?;
        slot.y.write(core_y, y_len)?;
        slot.uv.write(core_uv, y_len / 2)?;

        channel.send(&ToClient::FramePublished {
            id,
            generation: active.generation,
            slot: slot_index as u8,
        })
    }
}

/// Bytes of a tightly packed Y plane; the UV plane is half of it.
fn plane_len(width: u32, height: u32) -> Result<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| anyhow!("plane size overflow"))
}

/// The core's presentation plane pointers + visible size, or `None` while
/// unavailable.
fn query_core_planes(id: PlayerId) -> Option<(*const c_void, *const c_void, u32, u32)> {
    let mut y: *const c_void = std::ptr::null();
    state::consume_result(unsafe { core::uuav_player_get_video_texture(id, 0, &mut y) }).ok()?;
    let mut uv: *const c_void = std::ptr::null();
    state::consume_result(unsafe { core::uuav_player_get_video_texture(id, 1, &mut uv) }).ok()?;

    let mut size = core::VideoSize {
        width: 0,
        height: 0,
    };
    state::consume_result(unsafe { core::uuav_player_get_video_size(id, &mut size) }).ok()?;
    if y.is_null() || uv.is_null() || size.width == 0 || size.height == 0 {
        return None;
    }
    // the core presents at even dimensions (NV12), the stream size may be odd
    Some((y, uv, size.width & !1, size.height & !1))
}

fn create_generation(width: u32, height: u32) -> Result<[SlotPair; SLOTS]> {
    let y_len = plane_len(width, height)?;
    let make_pair = || -> Result<SlotPair> {
        Ok(SlotPair {
            y: Shm::new(y_len)?,
            uv: Shm::new(y_len / 2)?,
        })
    };
    Ok([make_pair()?, make_pair()?, make_pair()?])
}

fn announce(
    surfaces: &fd_channel::Sender,
    channel: &mut Channel,
    id: PlayerId,
    generation: u32,
    width: u32,
    height: u32,
    slots: &[SlotPair; SLOTS],
) -> Result<()> {
    for (slot_index, slot) in slots.iter().enumerate() {
        for (plane, shm) in [(0u8, &slot.y), (1u8, &slot.uv)] {
            surfaces.send(
                fd_channel::SurfaceTag {
                    player: id,
                    generation,
                    slot: slot_index as u8,
                    plane,
                },
                shm.fd.as_fd(),
                shm.len,
            )?;
        }
    }
    channel.send(&ToClient::TextureSet {
        id,
        generation,
        width,
        height,
        // planes travel as descriptors, not handles
        handles: Vec::new(),
    })
}

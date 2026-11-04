//! Client-side presentation on Linux: assembles shared-memory generations
//! announced by the helper (sealed memfds, mapped read-only), and on the
//! host's render event copies the latest published slot into client-owned
//! presentation buffers — the same stable-pointer contract the GPU
//! platforms offer (plane 0 = Y 8-bit at w x h, plane 1 = UV interleaved
//! 8-bit at w/2 x h/2, both tightly packed, retire-grace on resolution
//! change), so the host's poll-and-rewrap flow is identical. The host
//! uploads the two buffers to its own textures.

use crate::platform::UnityDevice;
use anyhow::{Context as _, Result, anyhow, ensure};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::raw::c_void;
use uuav_ipc::fd_channel::SurfaceTag;

const SLOTS: usize = 3;
const PLANES: usize = 2;

/// A read-only mapping of one transferred slot plane.
pub struct Mapping {
    ptr: *const u8,
    len: usize,
}

// the mapping is immutable shared memory; access is synchronized by the
// publish protocol, not by the mapping
unsafe impl Send for Mapping {}

impl Mapping {
    /// Maps a received memfd. The helper seals every slot against size
    /// changes; a descriptor without those seals is refused, because a
    /// shrunk mapping would raise SIGBUS in the host on the next read.
    pub fn map(fd: OwnedFd, len: usize) -> Result<Self> {
        ensure!(len > 0, "slot plane has no size");
        let seals = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 {
            return Err(std::io::Error::last_os_error()).context("F_GET_SEALS");
        }
        let required = libc::F_SEAL_SHRINK | libc::F_SEAL_GROW;
        ensure!(
            seals & required == required,
            "slot plane memfd is not size-sealed"
        );

        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } != 0 {
            return Err(std::io::Error::last_os_error()).context("fstat(memfd)");
        }
        let size = usize::try_from(stat.st_size).unwrap_or(0);
        ensure!(
            size >= len,
            "slot plane memfd ({size} bytes) is smaller than announced ({len})"
        );

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error()).context("mmap(memfd)");
        }
        // the mapping keeps the memory alive; the descriptor is not needed
        drop(fd);
        Ok(Self {
            ptr: ptr.cast::<u8>().cast_const(),
            len,
        })
    }

    fn bytes(&self, len: usize) -> Result<&[u8]> {
        ensure!(
            len <= self.len,
            "slot plane ({} bytes) is smaller than the frame ({len})",
            self.len
        );
        // SAFETY: the mapping covers `self.len` bytes for its whole lifetime
        Ok(unsafe { std::slice::from_raw_parts(self.ptr, len) })
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.ptr.cast_mut().cast::<c_void>(), self.len) };
    }
}

/// Largest edge a generation may announce. The helper is the untrusted
/// side of this channel: its dimensions size the presentation planes the
/// host allocates inside the render event, so an absurd pair must be
/// refused as a protocol error rather than reach `vec!`.
const MAX_DIMENSION: u32 = 16_384;

/// A generation being assembled: mappings arrive over the fd channel,
/// dimensions over the control channel (`TextureSet`), in either order.
struct PendingGen {
    generation: u32,
    dims: Option<(u32, u32)>,
    planes: [[Option<Mapping>; PLANES]; SLOTS],
}

impl PendingGen {
    const fn empty(generation: u32) -> Self {
        Self {
            generation,
            dims: None,
            planes: [[None, None], [None, None], [None, None]],
        }
    }

    fn complete(&self) -> bool {
        self.dims.is_some()
            && self
                .planes
                .iter()
                .all(|slot| slot.iter().all(Option::is_some))
    }
}

/// An assembled generation: every slot plane mapped.
struct ActiveGen {
    generation: u32,
    width: u32,
    height: u32,
    slots: [[Mapping; PLANES]; SLOTS],
}

/// The presentation buffers the host reads, plus the size they were
/// created for.
struct SizedPlanes {
    y: Box<[u8]>,
    uv: Box<[u8]>,
    width: u32,
    height: u32,
}

/// Per-player video state behind the mirror's mutex. Written by the IO and
/// surface threads (assembly, publishes), consumed on the host's render
/// thread.
#[derive(Default)]
pub struct PlayerVideo {
    pending: Option<PendingGen>,
    active: Option<ActiveGen>,
    presentation: Option<SizedPlanes>,
    /// Previous presentation generation, kept alive for one resolution
    /// change so the pointers the host still holds stay valid until its
    /// next poll.
    retired: Option<SizedPlanes>,
    /// Newest (generation, slot) the helper finished writing.
    published: Option<(u32, u8)>,
    presented: Option<(u32, u8)>,
    /// Ack the render event owes the helper after mapping a generation.
    ack_due: Option<u32>,
    /// Frames copied into the presentation buffers.
    frames: u64,
}

impl PlayerVideo {
    /// One transferred slot plane from the fd channel.
    pub fn store_surface(&mut self, tag: &SurfaceTag, mapping: Mapping) {
        let pending = self.pending_for(tag.generation);
        if let Some(cell) = pending
            .planes
            .get_mut(tag.slot as usize)
            .and_then(|slot| slot.get_mut(tag.plane as usize))
        {
            *cell = Some(mapping);
        }
    }

    /// The control-channel `TextureSet` announcement for a generation.
    pub fn store_texture_set(&mut self, generation: u32, width: u32, height: u32) {
        self.pending_for(generation).dims = Some((width, height));
    }

    pub const fn store_published(&mut self, generation: u32, slot: u8) {
        self.published = Some((generation, slot));
    }

    /// Drops all helper-side shared state after the helper died (the
    /// mappings release with it). The presentation buffers and their
    /// retire grace survive so the pointers the host holds stay valid —
    /// the last frame freezes until the resurrected helper publishes again.
    pub fn reset_for_recovery(&mut self) {
        self.pending = None;
        self.active = None;
        self.published = None;
        self.presented = None;
        self.ack_due = None;
    }

    fn pending_for(&mut self, generation: u32) -> &mut PendingGen {
        let stale = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.generation != generation);
        if stale || self.pending.is_none() {
            // a newer announcement supersedes whatever was half-assembled
            self.pending = Some(PendingGen::empty(generation));
        }
        // just ensured above
        self.pending.as_mut().unwrap_or_else(|| unreachable!())
    }

    /// The stable presentation-plane pointer the host reads, once the
    /// first frame was presented.
    pub fn texture_ptr(&self, plane: i32) -> Result<*const c_void, String> {
        let planes = self
            .presentation
            .as_ref()
            .ok_or_else(|| "video texture is not available yet".to_owned())?;
        let buffer = match plane {
            0 => &planes.y,
            1 => &planes.uv,
            _ => return Err(format!("unknown plane {plane}")),
        };
        Ok(buffer.as_ptr().cast())
    }

    /// Render-thread presentation: promotes a completed pending generation,
    /// then copies the latest published slot into the presentation
    /// buffers. Returns the generation to ack, if one was just promoted.
    pub fn present(&mut self, _unity: &UnityDevice) -> Result<Option<u32>> {
        let mut ack = None;
        if self.pending.as_ref().is_some_and(PendingGen::complete)
            && let Some(pending) = self.pending.take()
        {
            self.active = Some(activate(pending)?);
            self.ack_due = Some(self.active.as_ref().map_or(0, |active| active.generation));
        }
        if self.ack_due.is_some() {
            ack = self.ack_due.take();
        }

        let Some((generation, slot)) = self.published else {
            return Ok(ack);
        };
        if self.presented == Some((generation, slot)) {
            return Ok(ack);
        }
        let Some((active_generation, width, height)) = self
            .active
            .as_ref()
            .map(|active| (active.generation, active.width, active.height))
        else {
            return Ok(ack);
        };
        if active_generation != generation {
            // publish for a generation this side hasn't mapped (yet, or
            // anymore); the helper keeps publishing the acked one
            return Ok(ack);
        }

        self.ensure_presentation(width, height)?;
        let Some(active) = self.active.as_ref() else {
            return Ok(ack);
        };
        let planes = self
            .presentation
            .as_mut()
            .ok_or_else(|| anyhow!("presentation planes are missing"))?;
        let sources = active
            .slots
            .get(slot as usize)
            .ok_or_else(|| anyhow!("published slot {slot} out of range"))?;

        planes.y.copy_from_slice(sources[0].bytes(planes.y.len())?);
        planes
            .uv
            .copy_from_slice(sources[1].bytes(planes.uv.len())?);
        self.presented = Some((generation, slot));
        self.frames = self.frames.wrapping_add(1);
        Ok(ack)
    }

    /// The size the presentation planes were created for, once the first
    /// frame was presented: the decoded frame's (even-rounded), which the
    /// stream header behind the state update need not match.
    pub fn presented_size(&self) -> Option<(u32, u32)> {
        self.presentation
            .as_ref()
            .map(|planes| (planes.width, planes.height))
    }

    /// Frames copied into the presentation planes so far; unchanged while
    /// nothing new was published.
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    fn ensure_presentation(&mut self, width: u32, height: u32) -> Result<()> {
        if self
            .presentation
            .as_ref()
            .is_some_and(|planes| planes.width == width && planes.height == height)
        {
            return Ok(());
        }
        let y_len = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| anyhow!("plane size overflow"))?;
        // the generation before last dies here; the host has had a full
        // poll cycle to stop reading it
        let _previous = self.retired.take();
        self.retired = self.presentation.take();
        self.presentation = Some(SizedPlanes {
            y: vec![0u8; y_len].into_boxed_slice(),
            uv: vec![0u8; y_len / 2].into_boxed_slice(),
            width,
            height,
        });
        Ok(())
    }
}

/// Turns a completed pending generation into the active one.
fn activate(pending: PendingGen) -> Result<ActiveGen> {
    let (width, height) = pending
        .dims
        .ok_or_else(|| anyhow!("generation has no dims"))?;
    ensure!(
        (1..=MAX_DIMENSION).contains(&width) && (1..=MAX_DIMENSION).contains(&height),
        "helper announced an unpresentable size {width}x{height}"
    );
    ensure!(
        width % 2 == 0 && height % 2 == 0,
        "helper announced odd NV12 dimensions {width}x{height}"
    );
    let y_len = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| anyhow!("plane size overflow"))?;
    let generation = pending.generation;
    let [slot0, slot1, slot2] = pending.planes;
    let take = |slot: [Option<Mapping>; PLANES]| -> Result<[Mapping; PLANES]> {
        let [y, uv] = slot;
        let y = y.ok_or_else(|| anyhow!("missing Y plane"))?;
        let uv = uv.ok_or_else(|| anyhow!("missing UV plane"))?;
        // the mappings must cover the frame the dims describe; a short one
        // would fail every present, so refuse the generation up front
        y.bytes(y_len)?;
        uv.bytes(y_len / 2)?;
        Ok([y, uv])
    };
    Ok(ActiveGen {
        generation,
        width,
        height,
        slots: [take(slot0)?, take(slot1)?, take(slot2)?],
    })
}

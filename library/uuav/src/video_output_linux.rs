//! Linux presentation target: two tightly packed CPU planes (Y 8-bit at
//! w x h, UV interleaved 8-bit at w/2 x h/2) the render event copies the
//! due frame into. The consumer (the helper's video pump) reads them
//! through the same stable-pointer contract the GPU platforms offer:
//! `uuav_player_get_video_texture` returns the plane base pointer, the
//! visible size comes from `uuav_player_get_video_size`, and both planes
//! are recreated together on a resolution change with a one-poll retire
//! grace.

use anyhow::{Result, anyhow};
use std::os::raw::c_void;

use crate::hw_device::HwDevice;
use crate::video_decoder::VideoFrame;

/// Non-owning view of one plane buffer; the pointer is valid until the
/// generation after next replaces it (see the module docs).
pub(crate) struct VideoTextureView {
    ptr: *const u8,
}

impl VideoTextureView {
    pub(crate) const fn raw_ptr_mut(&self) -> *mut c_void {
        self.ptr.cast_mut().cast::<c_void>()
    }
}

pub(crate) struct VideoOutput {
    planes: Option<SizedPlanes>,
    /// Previous plane generation, kept alive for one resolution change so
    /// the pointers the consumer still holds stay valid until its next
    /// poll notices the new plane-0 pointer.
    retired: Option<SizedPlanes>,
}

struct SizedPlanes {
    y: Box<[u8]>,
    uv: Box<[u8]>,
    width: u32,
    height: u32,
}

impl SizedPlanes {
    const fn matches(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

impl VideoOutput {
    pub(crate) const fn new(_device: &HwDevice) -> Self {
        Self {
            planes: None,
            retired: None,
        }
    }

    /// One plane buffer, once the first frame has been presented.
    pub(crate) fn texture(&self, plane: i32) -> Option<VideoTextureView> {
        let planes = self.planes.as_ref()?;
        let buffer = match plane {
            0 => &planes.y,
            1 => &planes.uv,
            _ => return None,
        };
        Some(VideoTextureView {
            ptr: buffer.as_ptr(),
        })
    }

    pub(crate) fn present(&mut self, frame: &VideoFrame) -> Result<()> {
        // NV12 requires even dimensions; negative decoder sizes fold to 0
        let width = u32::try_from(frame.width()).unwrap_or(0) & !1;
        let height = u32::try_from(frame.height()).unwrap_or(0) & !1;
        if width == 0 || height == 0 {
            return Err(anyhow!("frame has no presentable size"));
        }
        let source = frame
            .cpu_planes()
            .ok_or_else(|| anyhow!("decoded frame is not a CPU frame"))?;
        if source.y.is_null() || source.uv.is_null() {
            return Err(anyhow!("decoded frame has no plane data"));
        }

        self.ensure_planes(width, height)?;
        let planes = self
            .planes
            .as_mut()
            .ok_or_else(|| anyhow!("presentation planes are missing"))?;

        let row = width as usize;
        copy_rows(
            &mut planes.y,
            source.y,
            source.y_stride,
            row,
            height as usize,
        )?;
        copy_rows(
            &mut planes.uv,
            source.uv,
            source.uv_stride,
            row,
            (height / 2) as usize,
        )
    }

    fn ensure_planes(&mut self, width: u32, height: u32) -> Result<()> {
        if self
            .planes
            .as_ref()
            .is_some_and(|planes| planes.matches(width, height))
        {
            return Ok(());
        }
        let y_len = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| anyhow!("plane size overflow"))?;
        // the UV plane has half the rows at the same byte width
        let uv_len = y_len / 2;
        // the generation before last dies here; the consumer has had a
        // full poll cycle to stop reading it
        let _previous = self.retired.take();
        self.retired = self.planes.take();
        self.planes = Some(SizedPlanes {
            y: vec![0u8; y_len].into_boxed_slice(),
            uv: vec![0u8; uv_len].into_boxed_slice(),
            width,
            height,
        });
        Ok(())
    }
}

/// Copies `rows` rows of `row_bytes` from a strided source into a tightly
/// packed destination.
fn copy_rows(
    dst: &mut [u8],
    src: *const u8,
    src_stride: usize,
    row_bytes: usize,
    rows: usize,
) -> Result<()> {
    if src_stride < row_bytes {
        return Err(anyhow!(
            "source stride {src_stride} is narrower than the visible row {row_bytes}"
        ));
    }
    for row in 0..rows {
        let dst_start = row
            .checked_mul(row_bytes)
            .ok_or_else(|| anyhow!("row offset overflow"))?;
        let dst_end = dst_start
            .checked_add(row_bytes)
            .ok_or_else(|| anyhow!("row offset overflow"))?;
        let dst_row = dst
            .get_mut(dst_start..dst_end)
            .ok_or_else(|| anyhow!("destination plane is too small"))?;
        let src_start = row
            .checked_mul(src_stride)
            .ok_or_else(|| anyhow!("row offset overflow"))?;
        // SAFETY: the decoder allocated `rows` rows of at least `src_stride`
        // bytes behind `src`, and the frame outlives this call
        let src_row = unsafe { std::slice::from_raw_parts(src.add(src_start), row_bytes) };
        dst_row.copy_from_slice(src_row);
    }
    Ok(())
}

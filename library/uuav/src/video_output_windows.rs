use anyhow::{Result, anyhow};
use std::os::raw::c_void;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CPU_ACCESS_WRITE, D3D11_MAP_WRITE,
    D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
    ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::core::Interface;

use crate::hw_device::{HwDevice, HwDeviceContext};
use crate::video_decoder::{CpuPlanes, VideoFrame};

/// Non-owning view of [`VideoOutput`]'s presentation texture: one NV12
/// `ID3D11Texture2D` covering both planes, whatever plane was asked for -
/// the engine creates its own format-cast views over it (R8 -> Y,
/// R8G8 -> UV). The COM reference held here only pins the resource while
/// the view lives; `VideoOutput` stays the owner and recreates the texture
/// on resolution change, after which consumers re-query (contract of
/// `uuav_player_get_video_texture`).
pub(crate) struct VideoTextureView {
    texture: ID3D11Texture2D,
}

impl VideoTextureView {
    pub(crate) fn raw_ptr_mut(&self) -> *mut c_void {
        self.texture.as_raw()
    }
}

/// Presentation target: an NV12 texture on the engine-provided device that
/// decoded frames are copied into on the render thread. The engine consumes
/// the raw `ID3D11Texture2D*` and creates its own views over the planes.
///
/// Hardware frames are copied GPU-side from the decoder surface pool. CPU
/// frames (software decoding) go through a staging NV12 texture the CPU
/// maps and fills, then `CopyResource`.
pub(crate) struct VideoOutput {
    device: ID3D11Device,
    texture: Option<SizedTexture>,
}

/// The NV12 texture together with the dimensions it was created for.
struct SizedTexture {
    texture: ID3D11Texture2D,
    /// CPU-mappable copy for software-decoded frames; allocated on the
    /// first such frame, never for hardware playback.
    staging: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
}

impl SizedTexture {
    const fn matches(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

// The COM pointers are only used behind the player's mutex and D3D11
// devices are free-threaded.
unsafe impl Send for VideoOutput {}

impl VideoOutput {
    pub(crate) fn new(device: &HwDevice) -> Self {
        Self {
            device: device.device().clone(),
            texture: None,
        }
    }

    /// The shared NV12 texture, once the first frame has been presented.
    pub(crate) fn texture(&self) -> Option<VideoTextureView> {
        self.texture.as_ref().map(|sized| VideoTextureView {
            texture: sized.texture.clone(),
        })
    }

    /// `hw` is the D3D11VA context of a hardware playback: its immediate
    /// context (under FFmpeg's device lock) runs the copy. Without one
    /// (software playback) the device's immediate context is used
    /// directly; it is multithread-protected since init.
    pub(crate) fn present(
        &mut self,
        hw: Option<&HwDeviceContext>,
        frame: &VideoFrame,
    ) -> Result<()> {
        // NV12 requires even dimensions; negative decoder sizes fold to 0
        let width = u32::try_from(frame.width()).unwrap_or(0) & !1;
        let height = u32::try_from(frame.height()).unwrap_or(0) & !1;
        if width == 0 || height == 0 {
            return Err(anyhow!("frame has no presentable size"));
        }
        self.ensure_texture(width, height)?;

        let ctx = match hw {
            Some(hw) => {
                let ctx_raw = hw.immediate_context_raw();
                unsafe { ID3D11DeviceContext::from_raw_borrowed(&ctx_raw) }
                    .cloned()
                    .ok_or_else(|| anyhow!("hw device context has no immediate context"))?
            }
            None => unsafe { self.device.GetImmediateContext() }
                .map_err(|e| anyhow!("GetImmediateContext failed: {e}"))?,
        };
        // FFmpeg's own decoder threads drive the same immediate context;
        // its device lock serializes us against them.
        let _guard = hw.map(HwDeviceContext::lock);

        match frame.cpu_planes() {
            None => self.copy_surface(&ctx, frame, width, height),
            Some(cpu) => self.upload_cpu(&ctx, cpu, width, height),
        }
    }

    /// GPU copy of the visible box out of the decoder's surface array.
    fn copy_surface(
        &self,
        ctx: &ID3D11DeviceContext,
        frame: &VideoFrame,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let (src_raw, subresource) = frame
            .d3d11_surface()
            .ok_or_else(|| anyhow!("decoded frame has no D3D11 texture"))?;
        let src = unsafe { ID3D11Resource::from_raw_borrowed(&src_raw) }
            .ok_or_else(|| anyhow!("decoded frame has no D3D11 texture"))?;
        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| anyhow!("presentation texture is missing"))?;

        // The decoder surface can be part of a larger texture array with
        // padded (coded-size) dimensions; copy just the visible box.
        let src_box = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: width,
            bottom: height,
            back: 1,
        };
        unsafe {
            ctx.CopySubresourceRegion(
                &texture.texture,
                0,
                0,
                0,
                0,
                src,
                subresource,
                Some(&src_box),
            );
        }
        Ok(())
    }

    /// CPU upload: map the staging texture, write both planes (the UV rows
    /// follow the Y rows at the same pitch in a mapped NV12 subresource),
    /// then copy it whole into the presentation texture.
    fn upload_cpu(
        &mut self,
        ctx: &ID3D11DeviceContext,
        cpu: CpuPlanes,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if cpu.y.is_null() || cpu.uv.is_null() {
            return Err(anyhow!("decoded frame has no plane data"));
        }
        let device = self.device.clone();
        let sized = self
            .texture
            .as_mut()
            .ok_or_else(|| anyhow!("presentation texture is missing"))?;
        if sized.staging.is_none() {
            sized.staging = Some(Self::create_texture(&device, width, height, true)?);
        }
        let staging = sized
            .staging
            .as_ref()
            .ok_or_else(|| anyhow!("staging texture is missing"))?;

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { ctx.Map(staging, 0, D3D11_MAP_WRITE, 0, Some(&mut mapped)) }
            .map_err(|e| anyhow!("Map(staging NV12) failed: {e}"))?;
        let pitch = mapped.RowPitch as usize;
        let row = width as usize;
        let rows = height as usize;
        let result = (|| -> Result<()> {
            if pitch < row {
                return Err(anyhow!(
                    "mapped pitch {pitch} is narrower than the row {row}"
                ));
            }
            let base = mapped.pData.cast::<u8>();
            // SAFETY: the mapped subresource holds height * 1.5 rows of
            // `pitch` bytes; the source frame outlives this call
            unsafe {
                copy_rows(base, pitch, cpu.y, cpu.y_stride, row, rows)?;
                let uv_base = base.add(
                    pitch
                        .checked_mul(rows)
                        .ok_or_else(|| anyhow!("offset overflow"))?,
                );
                copy_rows(uv_base, pitch, cpu.uv, cpu.uv_stride, row, rows / 2)?;
            }
            Ok(())
        })();
        unsafe { ctx.Unmap(staging, 0) };
        result?;

        unsafe { ctx.CopyResource(&sized.texture, staging) };
        Ok(())
    }

    /// Returns the presentation texture, recreating it when the size
    /// changed or none exists yet.
    fn ensure_texture(&mut self, width: u32, height: u32) -> Result<()> {
        if self
            .texture
            .as_ref()
            .is_some_and(|existing| existing.matches(width, height))
        {
            return Ok(());
        }
        // the consumer polls texture ptr + size each frame and re-wraps
        // after the pointer changes
        let created = Self::create_texture(&self.device, width, height, false)?;
        self.texture = Some(SizedTexture {
            texture: created,
            staging: None,
            width,
            height,
        });
        Ok(())
    }

    /// An NV12 texture: a GPU presentation target, or a CPU-writable
    /// staging copy.
    fn create_texture(
        device: &ID3D11Device,
        width: u32,
        height: u32,
        staging: bool,
    ) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: if staging {
                D3D11_USAGE_STAGING
            } else {
                D3D11_USAGE_DEFAULT
            },
            BindFlags: if staging {
                0
            } else {
                D3D11_BIND_SHADER_RESOURCE.0 as u32
            },
            CPUAccessFlags: if staging {
                D3D11_CPU_ACCESS_WRITE.0 as u32
            } else {
                0
            },
            MiscFlags: 0,
        };

        let mut created: Option<ID3D11Texture2D> = None;
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut created)) }
            .map_err(|e| anyhow!("CreateTexture2D(NV12 {width}x{height}) failed: {e}"))?;
        created.ok_or_else(|| anyhow!("CreateTexture2D(NV12 {width}x{height}) returned no texture"))
    }
}

/// Copies `rows` rows of `row_bytes` between two strided buffers.
///
/// # Safety
/// `dst` must hold `rows` rows of `dst_stride` bytes and `src` `rows` rows
/// of `src_stride` bytes.
unsafe fn copy_rows(
    dst: *mut u8,
    dst_stride: usize,
    src: *const u8,
    src_stride: usize,
    row_bytes: usize,
    rows: usize,
) -> Result<()> {
    if src_stride < row_bytes || dst_stride < row_bytes {
        return Err(anyhow!(
            "stride narrower than the visible row ({src_stride}/{dst_stride} < {row_bytes})"
        ));
    }
    for row in 0..rows {
        let dst_offset = row
            .checked_mul(dst_stride)
            .ok_or_else(|| anyhow!("row offset overflow"))?;
        let src_offset = row
            .checked_mul(src_stride)
            .ok_or_else(|| anyhow!("row offset overflow"))?;
        unsafe {
            std::ptr::copy_nonoverlapping(src.add(src_offset), dst.add(dst_offset), row_bytes);
        }
    }
    Ok(())
}

//! Linux: VAAPI decode with no engine GPU object in sight. Frames are
//! downloaded to system memory and leave the helper as memfd-backed NV12
//! planes, so there is no device to capture from the host and the probe
//! pointer `uuav_init` receives is accepted and ignored.

use anyhow::{Result, anyhow};
use ffmpeg_sys_next as ff;
use std::os::raw::c_void;
use std::ptr;

use crate::ffutil::av_err;
use crate::video_decoder::HW_DEVICE_TYPE;

/// Placeholder for the engine device the other platforms capture. Clone
/// but deliberately not Copy: the call sites are shared with the platforms
/// where the device is a retained handle.
#[derive(Clone)]
pub(crate) struct HwDevice;

impl HwDevice {
    /// Accepts anything, null included: nothing on Linux derives from a
    /// host texture.
    ///
    /// # Safety
    /// Trivially safe; the pointer is never dereferenced.
    // the Result is the cross-platform shape of this entry point
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) const unsafe fn from_texture(_texture: *const c_void) -> Result<Self> {
        Ok(Self)
    }
}

/// A VAAPI `AVHWDeviceContext` on the default DRM render node. Creation
/// fails on machines without a usable VA driver (CI runners, containers,
/// software renderers), which the caller answers with software decoding.
pub(crate) struct HwDeviceContext {
    buf: *mut ff::AVBufferRef,
}

// AVBufferRef refcounting is atomic; libva serializes its own contexts.
unsafe impl Send for HwDeviceContext {}
unsafe impl Sync for HwDeviceContext {}

impl HwDeviceContext {
    pub(crate) fn new() -> Result<Self> {
        let mut buf: *mut ff::AVBufferRef = ptr::null_mut();
        // a null device lets libavutil walk the DRM render nodes itself
        let ret = unsafe {
            ff::av_hwdevice_ctx_create(&mut buf, HW_DEVICE_TYPE, ptr::null(), ptr::null_mut(), 0)
        };
        if ret < 0 {
            return Err(av_err("av_hwdevice_ctx_create(VAAPI)", ret));
        }
        if buf.is_null() {
            return Err(anyhow!("av_hwdevice_ctx_create(VAAPI) returned no context"));
        }
        Ok(Self { buf })
    }

    pub(crate) const fn as_buffer_ptr(&self) -> *mut ff::AVBufferRef {
        self.buf
    }
}

impl Drop for HwDeviceContext {
    fn drop(&mut self) {
        unsafe { ff::av_buffer_unref(&mut self.buf) };
    }
}

//! Linux: the core derives nothing from a host GPU object (VAAPI frames are
//! downloaded to system memory and leave as memfd-backed planes), so the
//! probe is a null pointer the core's `uuav_init` accepts on this platform.

use anyhow::Result;
use std::os::raw::c_void;

pub struct ProbeDevice;

// the Result and the &self are the cross-platform shape of the adapter's
// device handling
#[allow(clippy::unnecessary_wraps, clippy::unused_self)]
impl ProbeDevice {
    /// `adapter` is unused: there is no GPU object to match against the
    /// client's; the decode device is whatever DRM render node libva opens.
    pub const fn new(_adapter: u64) -> Result<Self> {
        Ok(Self)
    }

    pub const fn probe_ptr(&self) -> *const c_void {
        std::ptr::null()
    }
}

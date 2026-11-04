//! Linux: no host GPU object is involved. Frames arrive as shared-memory
//! planes the host uploads itself, so the probe `uuav_init` receives is
//! accepted (null included) and ignored.

use anyhow::Result;
use std::os::raw::c_void;

/// Placeholder for the host device the other platforms capture.
pub struct UnityDevice;

/// # Safety
/// Trivially safe; the pointer is never dereferenced.
// the Result is the cross-platform shape of this entry point
#[allow(clippy::unnecessary_wraps)]
pub const unsafe fn capture_probe(_texture: *const c_void) -> Result<UnityDevice> {
    Ok(UnityDevice)
}

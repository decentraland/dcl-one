//! Windows: a frame reaches the renderer in three steps.
//!
//! 1. The helper decodes it with FFmpeg's D3D11VA and copies it into an NV12 texture shared
//!    across processes.
//! 2. The uuav client copies that into its NV12 presentation texture, on a D3D11 device this
//!    module creates on the renderer's adapter.
//! 3. The renderer opens the presentation texture through an NT handle without a copy: on DX12
//!    with `ID3D12Device::OpenSharedHandle`, on Vulkan as external memory
//!    (`VK_KHR_external_memory_win32`). It becomes one `NV12` wgpu texture whose two plane views
//!    the conversion samples.
//!
//! [`wait_for_client_copy`] orders step 2 before the renderer's read with a D3D11 fence.

mod decode_device;
mod import;

pub use decode_device::{Probe, wait_for_client_copy};
pub use import::make_planes;

/// Why the decode device could not be set up or a presentation texture imported: the step that
/// failed and the API error behind it, when there is one.
#[derive(Debug)]
pub struct InteropError {
    step: &'static str,
    cause: Option<Box<dyn std::error::Error>>,
}

impl InteropError {
    fn caused<E: std::error::Error + 'static>(step: &'static str) -> impl FnOnce(E) -> Self {
        move |cause| Self {
            step,
            cause: Some(Box::new(cause)),
        }
    }
}

impl From<&'static str> for InteropError {
    fn from(step: &'static str) -> Self {
        Self { step, cause: None }
    }
}

impl std::fmt::Display for InteropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.cause {
            Some(cause) => write!(f, "{}: {cause}", self.step),
            None => f.write_str(self.step),
        }
    }
}

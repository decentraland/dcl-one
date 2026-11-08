//! Windows: the client presents each frame into one NV12 `ID3D11Texture2D` on a D3D11 device
//! this module creates on the renderer's adapter (the decode device the helper shares its slots
//! with). The probe texture asks for NT-handle sharing, so the client creates its presentation
//! textures shareable too, and the renderer imports each one without a copy: a DX12 renderer
//! opens the handle on its `ID3D12Device`, a Vulkan renderer imports it as external memory
//! (`VK_KHR_external_memory_win32`). Either way it becomes one `NV12` wgpu texture whose two
//! plane views the conversion samples.
//!
//! Nothing orders the two devices on its own: [`wait_for_client_copy`] signals a D3D11 fence on
//! the immediate context after the client's copy and waits, bounded, for the copy to land before
//! the conversion is submitted, and the renderer holds a player's next copy until that
//! conversion has executed.

use std::{
    os::raw::c_void,
    sync::{Mutex, Once},
};

use bevy::{
    log::{debug, warn},
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        renderer::{RenderAdapter, RenderDevice},
    },
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
            Direct3D11::{
                D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_FLAG, D3D11_FENCE_FLAG_NONE,
                D3D11_RESOURCE_MISC_SHARED, D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_SDK_VERSION,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice, ID3D11Device,
                ID3D11Device5, ID3D11DeviceContext4, ID3D11Fence, ID3D11Texture2D,
            },
            Direct3D12::{ID3D12Device, ID3D12Resource},
            Dxgi::{
                Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
                CreateDXGIFactory1, DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE,
                IDXGIAdapter1, IDXGIFactory1, IDXGIResource1,
            },
        },
        System::Threading::{CreateEventW, WaitForSingleObject},
    },
    core::{Interface as _, PCWSTR},
};

use super::render::PlaneTextures;

/// How long [`wait_for_client_copy`] waits for a copy to land before returning `false`.
const COPY_WAIT_MS: u32 = 4;

/// The renderer's API, read once from the render device: where a presentation texture is
/// imported.
enum Renderer {
    Dx12(ID3D12Device),
    Vulkan,
}

/// The decode device and the fence that orders its copies before the renderer's reads.
struct Interop {
    /// Kept so the device outlives the probe texture: the client holds its own reference, this
    /// one is for the fence below.
    _device: ID3D11Device,
    /// The device's immediate context, where the client records its copies.
    context: ID3D11DeviceContext4,
    fence: ID3D11Fence,
    /// Signalled when the fence reaches a value (`SetEventOnCompletion`); auto-reset.
    event: HANDLE,
    /// The last value signalled on the fence.
    value: u64,
    renderer: Renderer,
}

// SAFETY: D3D11 devices, fences and the D3D12 device are free-threaded; the immediate context
// is only used from the render thread, which is the one thread that calls into this module
// after the probe.
unsafe impl Send for Interop {}

impl Drop for Interop {
    fn drop(&mut self) {
        unsafe {
            _ = CloseHandle(self.event);
        }
    }
}

/// The decode device of the running runtime; replaced by [`Probe::new`] on a restart.
static INTEROP: Mutex<Option<Interop>> = Mutex::new(None);

fn interop() -> std::sync::MutexGuard<'static, Option<Interop>> {
    INTEROP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A 1x1 texture on the decode device, created `SHARED | SHARED_NTHANDLE` so the client creates
/// its presentation textures the same way. The client captures the device from it and retains
/// the device itself, so the probe only has to outlive the call.
pub struct Probe {
    texture: ID3D11Texture2D,
}

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

impl Probe {
    /// Creates the decode device on the renderer's adapter and the fence between them. `Err`
    /// when the renderer is on neither DX12 nor Vulkan, or the adapter cannot be matched.
    pub fn new(device: &RenderDevice, adapter: &RenderAdapter) -> Result<Self, InteropError> {
        let (renderer, luid) = renderer_and_luid(device, adapter)?;
        let dxgi_adapter = adapter_with_luid(luid)?;

        let mut d3d11: Option<ID3D11Device> = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                &dxgi_adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d11),
                None,
                Some(&mut context),
            )
        }
        .map_err(InteropError::caused("D3D11CreateDevice failed"))?;
        let d3d11 = d3d11.ok_or("D3D11CreateDevice returned no device")?;
        let context = context.ok_or("D3D11CreateDevice returned no context")?;
        let device5: ID3D11Device5 = d3d11.cast().map_err(InteropError::caused(
            "the decode device has no ID3D11Device5 (fences need Windows 10 1703)",
        ))?;
        let context: ID3D11DeviceContext4 = context.cast().map_err(InteropError::caused(
            "the decode context has no ID3D11DeviceContext4",
        ))?;

        let mut fence: Option<ID3D11Fence> = None;
        unsafe { device5.CreateFence(0, D3D11_FENCE_FLAG_NONE, &mut fence) }
            .map_err(InteropError::caused("CreateFence failed"))?;
        let fence = fence.ok_or("CreateFence returned no fence")?;
        let event = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .map_err(InteropError::caused("CreateEventW failed"))?;

        let desc = D3D11_TEXTURE2D_DESC {
            Width: 1,
            Height: 1,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                as u32,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        unsafe { d3d11.CreateTexture2D(&desc, None, Some(&mut texture)) }
            .map_err(InteropError::caused("probe CreateTexture2D failed"))?;
        let texture = texture.ok_or("CreateTexture2D returned no texture")?;

        *interop() = Some(Interop {
            _device: d3d11,
            context,
            fence,
            event,
            value: 0,
            renderer,
        });
        Ok(Self { texture })
    }

    pub fn ptr(&self) -> *const c_void {
        self.texture.as_raw().cast_const()
    }
}

/// The renderer's API and its adapter's LUID.
fn renderer_and_luid(
    device: &RenderDevice,
    adapter: &RenderAdapter,
) -> Result<(Renderer, u64), InteropError> {
    let dx12 = unsafe {
        device
            .wgpu_device()
            .as_hal::<wgpu::hal::api::Dx12, _, _>(|hal| hal.map(|hal| hal.raw_device().clone()))
    };
    if let Some(d3d12) = dx12 {
        let luid = unsafe {
            adapter.as_hal::<wgpu::hal::api::Dx12, _, _>(|hal| {
                hal.map(|hal| {
                    hal.raw_adapter()
                        .GetDesc1()
                        .map(|desc| pack_luid(desc.AdapterLuid))
                })
            })
        }
        .ok_or("the DX12 renderer has no adapter")?
        .map_err(InteropError::caused("GetDesc1 failed"))?;
        return Ok((Renderer::Dx12(d3d12), luid));
    }

    let vulkan =
        unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan, _, _>(|hal| hal.map(vulkan_luid)) };
    match vulkan {
        Some(Ok(luid)) => Ok((Renderer::Vulkan, luid)),
        Some(Err(e)) => Err(e),
        None => Err("the renderer is on neither DX12 nor Vulkan".into()),
    }
}

/// The LUID Vulkan reports for the renderer's physical device, the same adapter identity DXGI
/// enumerates.
fn vulkan_luid(hal: &wgpu::hal::vulkan::Adapter) -> Result<u64, InteropError> {
    use ash::{khr::get_physical_device_properties2 as khr_properties, vk};

    let capabilities = hal.physical_device_capabilities();
    let instance = hal.shared_instance();
    let physical = hal.raw_physical_device();
    let mut id = vk::PhysicalDeviceIDProperties::default();
    let mut output = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
    // SAFETY: the instance owns this physical device and the output chain is live for the call
    unsafe {
        if instance.instance_api_version() >= vk::API_VERSION_1_1
            && capabilities.properties().api_version >= vk::API_VERSION_1_1
        {
            instance
                .raw_instance()
                .get_physical_device_properties2(physical, &mut output);
        } else if instance.extensions().contains(&khr_properties::NAME) {
            khr_properties::Instance::new(instance.entry(), instance.raw_instance())
                .get_physical_device_properties2(physical, &mut output);
        } else {
            return Err("the Vulkan renderer reports no device LUID".into());
        }
    }
    if id.device_luid_valid == vk::FALSE {
        return Err("the Vulkan device has no LUID".into());
    }
    Ok(u64::from_le_bytes(id.device_luid))
}

fn pack_luid(luid: windows::Win32::Foundation::LUID) -> u64 {
    (u64::from(luid.HighPart as u32) << 32) | u64::from(luid.LowPart)
}

/// The DXGI adapter with this LUID.
fn adapter_with_luid(luid: u64) -> Result<IDXGIAdapter1, InteropError> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
        .map_err(InteropError::caused("CreateDXGIFactory1 failed"))?;
    for index in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        let desc =
            unsafe { adapter.GetDesc1() }.map_err(InteropError::caused("GetDesc1 failed"))?;
        if pack_luid(desc.AdapterLuid) == luid {
            return Ok(adapter);
        }
    }
    Err("no DXGI adapter has the renderer's LUID".into())
}

/// Imports the presentation texture (`y` and `uv` are the same `ID3D11Texture2D`) into the
/// renderer. The client keeps it alive across frames and for one poll after a resolution change;
/// the import holds the shared memory on top.
pub fn make_planes(
    device: &RenderDevice,
    y: *const c_void,
    uv: *const c_void,
    width: u32,
    height: u32,
) -> Option<PlaneTextures> {
    if y.is_null() || y != uv {
        return None;
    }
    match import(device, y, width, height) {
        Ok(texture) => Some(PlaneTextures::nv12(texture)),
        Err(e) => {
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                warn!("video frames cannot be imported into the renderer: {e}");
            });
            debug!("uuav: presentation texture not imported: {e}");
            None
        }
    }
}

fn import(
    device: &RenderDevice,
    ptr: *const c_void,
    width: u32,
    height: u32,
) -> Result<wgpu::Texture, InteropError> {
    if !device
        .features()
        .contains(wgpu::Features::TEXTURE_FORMAT_NV12)
    {
        return Err("the render device has no NV12 texture support".into());
    }
    let raw = ptr.cast_mut();
    // SAFETY: the client hands out a live `ID3D11Texture2D` it keeps alive until one poll after
    // it replaces it; `from_raw_borrowed` adds no reference and the cast below takes its own
    let texture =
        unsafe { ID3D11Texture2D::from_raw_borrowed(&raw) }.ok_or("null presentation texture")?;
    let resource: IDXGIResource1 = texture.cast().map_err(InteropError::caused(
        "the presentation texture is not an IDXGIResource1",
    ))?;
    let handle = unsafe {
        resource.CreateSharedHandle(
            None,
            DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
            PCWSTR::null(),
        )
    }
    .map_err(InteropError::caused(
        "CreateSharedHandle failed (was the presentation texture created shareable?)",
    ))?;
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let imported = {
        let guard = interop();
        let interop = guard.as_ref().ok_or("no decode device")?;
        match &interop.renderer {
            Renderer::Dx12(d3d12) => open_dx12(device, d3d12, handle, size),
            Renderer::Vulkan => open_vulkan(device, handle, size),
        }
    };
    // neither import takes ownership of the handle; the resource lives on through its own
    // references
    unsafe {
        _ = CloseHandle(handle);
    }
    imported
}

fn descriptor(size: Extent3d) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("uuav presentation"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::NV12,
        usage: TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

fn open_dx12(
    device: &RenderDevice,
    d3d12: &ID3D12Device,
    handle: HANDLE,
    size: Extent3d,
) -> Result<wgpu::Texture, InteropError> {
    let mut resource: Option<ID3D12Resource> = None;
    unsafe { d3d12.OpenSharedHandle(handle, &mut resource) }.map_err(InteropError::caused(
        "ID3D12Device::OpenSharedHandle failed",
    ))?;
    let resource = resource.ok_or("OpenSharedHandle returned no resource")?;
    // SAFETY: the resource is a 2D single-mip NV12 texture of `size` opened on the renderer's
    // device, and the wrapper takes the reference `OpenSharedHandle` returned
    let hal = unsafe {
        wgpu::hal::dx12::Device::texture_from_raw(
            resource,
            TextureFormat::NV12,
            TextureDimension::D2,
            size,
            1,
            1,
        )
    };
    Ok(unsafe {
        device
            .wgpu_device()
            .create_texture_from_hal::<wgpu::hal::api::Dx12>(hal, &descriptor(size))
    })
}

fn open_vulkan(
    device: &RenderDevice,
    handle: HANDLE,
    size: Extent3d,
) -> Result<wgpu::Texture, InteropError> {
    let hal = unsafe {
        device
            .wgpu_device()
            .as_hal::<wgpu::hal::api::Vulkan, _, _>(|hal| {
                let hal = hal.ok_or("the renderer is not on Vulkan")?;
                hal.texture_from_d3d11_shared_handle(
                    handle,
                    &wgpu::hal::TextureDescriptor {
                        label: Some("uuav presentation"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::NV12,
                        usage: wgpu::hal::TextureUses::RESOURCE,
                        memory_flags: wgpu::hal::MemoryFlags::empty(),
                        view_formats: vec![],
                    },
                )
                .map_err(InteropError::caused(
                    "Vulkan import of the D3D11 texture failed",
                ))
            })
    }?;
    Ok(unsafe {
        device
            .wgpu_device()
            .create_texture_from_hal::<wgpu::hal::api::Vulkan>(hal, &descriptor(size))
    })
}

/// Orders the client's copy into the presentation texture before the renderer reads it: signals
/// the decode device's fence behind the copy and waits up to [`COPY_WAIT_MS`] for it. `false`
/// when the copy has not landed within that.
pub fn wait_for_client_copy() -> bool {
    let mut guard = interop();
    let Some(interop) = guard.as_mut() else {
        return false;
    };
    interop.value += 1;
    let value = interop.value;
    unsafe {
        if let Err(e) = interop.context.Signal(&interop.fence, value) {
            warn!("uuav: fence signal failed: {e}");
            return false;
        }
        interop.context.Flush();
        if interop.fence.GetCompletedValue() >= value {
            return true;
        }
        if let Err(e) = interop.fence.SetEventOnCompletion(value, interop.event) {
            warn!("uuav: fence wait failed: {e}");
            return false;
        }
        WaitForSingleObject(interop.event, COPY_WAIT_MS) == WAIT_OBJECT_0
    }
}

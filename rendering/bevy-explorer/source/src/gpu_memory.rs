use bevy::prelude::*;
use common::memory::GpuMemoryBudget;

#[cfg(test)]
#[path = "gpu_memory_tests.rs"]
mod gpu_memory_tests;

pub(crate) struct GpuMemoryPlugin;

impl Plugin for GpuMemoryPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        if !app
            .world()
            .contains_resource::<common::structs::NoRenderApp>()
        {
            app.insert_resource(GpuMemoryBudget::Unavailable)
                .add_systems(PreUpdate, native::sample_gpu_memory);
            return;
        }
        app.insert_resource(GpuMemoryBudget::NotApplicable);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{
        num::NonZeroU64,
        time::{Duration, Instant},
    };

    use bevy::prelude::*;
    #[cfg(any(windows, target_os = "linux"))]
    use bevy::render::renderer::RenderAdapter;
    #[cfg(target_os = "macos")]
    use bevy::render::renderer::RenderDevice;
    use common::memory::{GpuMemoryBudget, GpuMemoryPool, GpuMemoryPoolKind};

    pub(super) fn sample_gpu_memory(
        #[cfg(any(windows, target_os = "linux"))] adapter: Option<Res<RenderAdapter>>,
        #[cfg(target_os = "macos")] device: Option<Res<RenderDevice>>,
        mut last_query: Local<Option<Instant>>,
        mut budget: ResMut<GpuMemoryBudget>,
    ) {
        let now = Instant::now();
        if last_query.is_some_and(|time| now.duration_since(time) < Duration::from_secs(1)) {
            return;
        }
        #[cfg(windows)]
        let pools = adapter.and_then(|a| dxgi(&a).or_else(|| vulkan(&a)));
        #[cfg(target_os = "linux")]
        let pools = adapter.and_then(|a| vulkan(&a));
        #[cfg(target_os = "macos")]
        let pools = match device.as_deref().map(metal) {
            Some(Err(MetalDeviceBusy)) => return,
            Some(Ok(pools)) => pools,
            None => None,
        };
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        let pools = None;

        *last_query = Some(now);
        budget.set_if_neq(pools.map_or(GpuMemoryBudget::Unavailable, GpuMemoryBudget::Available));
    }

    #[cfg(windows)]
    fn dxgi(adapter: &RenderAdapter) -> Option<Vec<GpuMemoryPool>> {
        use windows::Win32::Graphics::Dxgi::{
            DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
        };

        // SAFETY: wgpu owns the borrowed adapter. Node 0 is the node used by its DX12 queue.
        // The query only writes to our output struct and neither retains nor modifies the adapter.
        unsafe {
            adapter.as_hal::<wgpu::hal::api::Dx12, _, _>(|hal| {
                let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                hal?.raw_adapter()
                    .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                    .ok()?;
                Some(vec![GpuMemoryPool {
                    kind: GpuMemoryPoolKind::DxgiLocal,
                    used_bytes: info.CurrentUsage,
                    budget_bytes: NonZeroU64::new(info.Budget)?,
                }])
            })
        }
    }

    #[cfg(any(windows, target_os = "linux"))]
    fn vulkan(adapter: &RenderAdapter) -> Option<Vec<GpuMemoryPool>> {
        use ash::{ext::memory_budget, khr::get_physical_device_properties2 as khr_properties, vk};

        // SAFETY: the instance owns this physical device. We use only supported entry points
        // and the driver writes to a live output chain. No raw handle escapes the callback.
        unsafe {
            adapter.as_hal::<wgpu::hal::api::Vulkan, _, _>(|hal| {
                let hal = hal?;
                let capabilities = hal.physical_device_capabilities();
                if !capabilities.supports_extension(memory_budget::NAME) {
                    return None;
                }
                let instance = hal.shared_instance();
                let physical = hal.raw_physical_device();
                let mut ext = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
                let properties = {
                    let mut output =
                        vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut ext);
                    if instance.instance_api_version() >= vk::API_VERSION_1_1
                        && capabilities.properties().api_version >= vk::API_VERSION_1_1
                    {
                        instance
                            .raw_instance()
                            .get_physical_device_memory_properties2(physical, &mut output);
                    } else if instance.extensions().contains(&khr_properties::NAME) {
                        khr_properties::Instance::new(instance.entry(), instance.raw_instance())
                            .get_physical_device_memory_properties2(physical, &mut output);
                    } else {
                        return None;
                    }
                    output.memory_properties
                };
                vulkan_pools(properties.memory_heaps_as_slice(), &ext)
            })
        }
    }

    #[cfg(any(windows, target_os = "linux"))]
    pub(super) fn vulkan_pools(
        heaps: &[ash::vk::MemoryHeap],
        reported: &ash::vk::PhysicalDeviceMemoryBudgetPropertiesEXT<'_>,
    ) -> Option<Vec<GpuMemoryPool>> {
        let pools: Vec<_> = heaps
            .iter()
            .enumerate()
            .filter(|(_, heap)| heap.flags.contains(ash::vk::MemoryHeapFlags::DEVICE_LOCAL))
            .filter_map(|(heap_index, _)| {
                Some(GpuMemoryPool {
                    kind: GpuMemoryPoolKind::VulkanDeviceLocal { heap_index },
                    used_bytes: reported.heap_usage[heap_index],
                    budget_bytes: NonZeroU64::new(reported.heap_budget[heap_index])?,
                })
            })
            .collect();
        (!pools.is_empty()).then_some(pools)
    }

    #[cfg(target_os = "macos")]
    struct MetalDeviceBusy;

    #[cfg(target_os = "macos")]
    fn metal(device: &RenderDevice) -> Result<Option<Vec<GpuMemoryPool>>, MetalDeviceBusy> {
        // SAFETY: the Metal device stays borrowed and locked for both read-only queries.
        unsafe {
            device
                .wgpu_device()
                .as_hal::<wgpu::hal::api::Metal, _, _>(|hal| {
                    let Some(hal) = hal else { return Ok(None) };
                    // Preserve the last sample and retry next frame if the renderer holds the device.
                    let raw = hal.raw_device().try_lock().ok_or(MetalDeviceBusy)?;
                    let Some(budget_bytes) =
                        NonZeroU64::new(raw.recommended_max_working_set_size())
                    else {
                        return Ok(None);
                    };
                    Ok(Some(vec![GpuMemoryPool {
                        kind: GpuMemoryPoolKind::MetalWorkingSet,
                        used_bytes: raw.current_allocated_size(),
                        budget_bytes,
                    }]))
                })
        }
    }
}

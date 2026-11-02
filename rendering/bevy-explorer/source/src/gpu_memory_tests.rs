use bevy::prelude::*;
use common::memory::GpuMemoryBudget;

use crate::gpu_memory::GpuMemoryPlugin;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn renderless_gpu_budget_is_not_applicable() {
    let mut app = App::new();
    app.insert_resource(common::structs::NoRenderApp)
        .add_plugins(GpuMemoryPlugin);
    app.update();
    assert_eq!(
        *app.world().resource::<GpuMemoryBudget>(),
        GpuMemoryBudget::NotApplicable
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn browser_gpu_budget_is_not_applicable_even_for_a_rendering_client() {
    let mut app = App::new();
    app.add_plugins(GpuMemoryPlugin);
    app.update();
    assert_eq!(
        *app.world().resource::<GpuMemoryBudget>(),
        GpuMemoryBudget::NotApplicable
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_renderer_without_a_query_is_unavailable() {
    let mut app = App::new();
    app.add_plugins(GpuMemoryPlugin);
    app.update();
    assert_eq!(
        *app.world().resource::<GpuMemoryBudget>(),
        GpuMemoryBudget::Unavailable
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a GPU with a native memory budget; set WGPU_BACKEND to select it"]
fn measures_the_active_gpu_memory_pool() {
    use bevy::{
        log::LogPlugin, render::renderer::RenderAdapterInfo, window::ExitCondition,
        winit::WinitPlugin,
    };
    use common::memory::GpuMemoryPoolKind;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .disable::<WinitPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            }),
    )
    .add_plugins(GpuMemoryPlugin);
    app.finish();
    app.cleanup();
    app.update();
    let adapter = app.world().resource::<RenderAdapterInfo>();
    let measured = app.world().resource::<GpuMemoryBudget>();
    println!("{} ({:?}): {measured:?}", adapter.name, adapter.backend);
    let GpuMemoryBudget::Available(pools) = measured else {
        panic!("no GPU memory reading")
    };
    assert!(!pools.is_empty());
    for pool in pools {
        assert!(matches!(
            (adapter.backend, pool.kind),
            (wgpu::Backend::Dx12, GpuMemoryPoolKind::DxgiLocal)
                | (
                    wgpu::Backend::Vulkan,
                    GpuMemoryPoolKind::VulkanDeviceLocal { .. }
                )
                | (wgpu::Backend::Metal, GpuMemoryPoolKind::MetalWorkingSet)
        ));
    }
}

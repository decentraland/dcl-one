// A highlighted mesh's silhouette in its highlight colour, widened along its normals by a share of
// the view's height, for the hover glow (scene_material/src/highlight). The vertex stage follows
// bevy_pbr's mesh.wgsl, so skinned and morphed meshes outline their animated pose.

#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_functions,
    skinning,
    morph::morph,
    forward_io::Vertex,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

struct HighlightItem {
    color: vec4<f32>,
    width: f32,
}

@group(2) @binding(0) var<uniform> item: HighlightItem;

#ifdef MORPH_TARGETS
fn morph_vertex(vertex_in: Vertex) -> Vertex {
    var vertex = vertex_in;
    let first_vertex = mesh[vertex.instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let weight_count = bevy_pbr::morph::layer_count();
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::morph::weight_at(i);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph(vertex_index, bevy_pbr::morph::position_offset, i);
#ifdef VERTEX_NORMALS
        vertex.normal += weight * morph(vertex_index, bevy_pbr::morph::normal_offset, i);
#endif
    }
    return vertex;
}
#endif

@vertex
fn vertex(vertex_no_morph: Vertex) -> @builtin(position) vec4<f32> {
#ifdef MORPH_TARGETS
    var vertex = morph_vertex(vertex_no_morph);
#else
    var vertex = vertex_no_morph;
#endif

#ifdef SKINNED
    let world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
#else
    // vertex_no_morph.instance_index works around a wgpu dx12 bug, as in mesh.wgsl
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);
#endif

    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
    var clip = position_world_to_clip(world_position.xyz);

#ifdef VERTEX_NORMALS
#ifdef SKINNED
    let world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    let world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );
#endif
    // the offset across the screen: the width is a share of the view's height
    let width_px = item.width * view.viewport.w;
    let clip_normal = (view.clip_from_world * vec4<f32>(world_normal, 0.0)).xy;
    if any(clip_normal != vec2<f32>(0.0)) {
        clip += vec4<f32>(
            normalize(clip_normal) / view.viewport.zw * width_px * clip.w * 2.0,
            0.0,
            0.0
        );
    }
#endif

    return clip;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return item.color;
}

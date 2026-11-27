#import "dcl_imposters/boimp_vertex.wgsl" as boimp_vertex;
#import boimp::shared::ImposterVertexOut;

#ifdef PREPASS_PIPELINE
    #import bevy_pbr::prepass_io::Vertex;

    @vertex
    fn vertex(in: Vertex) -> ImposterVertexOut {
        return boimp_vertex::vertex(in);
    }
#else
    #import bevy_pbr::forward_io::Vertex;
    #import bevy_pbr::mesh_functions::get_tag;

    struct DebugVertexOut {
        @builtin(position) position: vec4<f32>,
        @location(0) world_position: vec3<f32>,
        @location(1) base_world_position: vec3<f32>,
        @location(2) inverse_rotation_0c: vec3<f32>,
        @location(3) inverse_rotation_1c: vec3<f32>,
        @location(4) inverse_rotation_2c: vec3<f32>,
        @location(5) content_half_extent: vec3<f32>,
        @location(6) @interpolate(flat) tag: u32,
    }

    @vertex
    fn vertex(in: Vertex) -> DebugVertexOut {
        let out = boimp_vertex::vertex(in);
        return DebugVertexOut(out.position, out.world_position, out.base_world_position,
            out.inverse_rotation_0c, out.inverse_rotation_1c, out.inverse_rotation_2c,
            out.content_half_extent, get_tag(in.instance_index));
    }
#endif

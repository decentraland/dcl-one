#import dcl_imposters::boimp_fragment;
#import boimp::shared::ImposterVertexOut;

#ifdef PREPASS_PIPELINE
    #import bevy_pbr::prepass_io::FragmentOutput;

    @fragment
    fn fragment(in: ImposterVertexOut) -> FragmentOutput {
        return boimp_fragment::fragment(in);
    }
#else
    #import bevy_pbr::forward_io::FragmentOutput;

    @fragment
    fn fragment(in: ImposterVertexOut, @location(6) @interpolate(flat) tag: u32) -> FragmentOutput {
        var out = boimp_fragment::fragment(in);
        if tag != 0u {
            out.color = vec4<f32>(unpack4x8unorm(tag).rgb, out.color.a);
        }
        return out;
    }
#endif

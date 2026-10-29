// Effect particles (`fx_material.rs`): the batch meshes the effect renderer builds every frame, already in world
// space. Per vertex: the particle colour, the Niagara dynamic material parameters, the flipbook's next frame and
// its blend, the normal. With `FX_EXACT` the material instance's own formula (`fx_perms.wgsl`, from the cooked
// shaders) gives emissive and opacity; else texture x particle colour (the stand-in). Output by blend (the cooked
// shaders' convention): additive = emissive x opacity added (alpha 0); alpha = emissive with opacity as alpha;
// modulate = emissive multiplies what's behind.

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_world}
#import bevy_pbr::mesh_view_bindings::{view, globals}
#import bevy_pbr::view_transformations::{position_world_to_clip, depth_ndc_to_view_z}
#import db_client::fx_common::{FxIn, fx_tex_a, fx_smp_a}
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif
#ifdef FX_EXACT
#import fx_perms::fx_material
#endif
#ifdef FX_LIT
#import bevy_pbr::{pbr_functions::apply_pbr_lighting, pbr_types::pbr_input_new}
#import fx_perms::fx_base_color
#endif
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct FxParams {
    tint: vec4<f32>,
    /// x: blend (0 alpha, 1 additive, 2 modulate), y: debug view (`DB_FX_DEBUG`), z: dynamic parameters written (bits), w: unused.
    flags: vec4<u32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: FxParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) uv1: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) dyn0: vec4<f32>,
    @location(5) misc: vec4<f32>,
    @location(6) normal: vec3<f32>,
    @location(7) dyn1: vec4<f32>,
    @location(8) dyn2: vec4<f32>,
    @location(9) dyn3: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv0: vec2<f32>,
    @location(1) uv1: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) dyn0: vec4<f32>,
    @location(4) misc: vec4<f32>,
    @location(5) normal: vec3<f32>,
    @location(6) world: vec3<f32>,
    @location(7) dyn1: vec4<f32>,
    @location(8) dyn2: vec4<f32>,
    @location(9) dyn3: vec4<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world = mesh_position_local_to_world(get_world_from_local(v.instance_index), vec4<f32>(v.position, 1.0));
    out.clip_position = position_world_to_clip(world.xyz);
    out.world = world.xyz;
    out.uv0 = v.uv0;
    out.uv1 = v.uv1;
    out.color = v.color;
    out.dyn0 = v.dyn0;
    out.dyn1 = v.dyn1;
    out.dyn2 = v.dyn2;
    out.dyn3 = v.dyn3;
    out.misc = v.misc;
    out.normal = v.normal;
    return out;
}

/// The formula's inputs for this pixel.
fn fx_in(in: VertexOutput) -> FxIn {
    var i: FxIn;
    i.uv_a = in.uv0;
    i.uv_b = in.uv1;
    i.color = in.color;
    i.dyn_a = in.dyn0;
    i.dyn_b = in.dyn1;
    i.dyn_c = in.dyn2;
    i.dyn_d = in.dyn3;
    i.misc = in.misc;
    i.normal = normalize(in.normal);
    i.time = globals.time;
    i.dyn_valid = params.flags.z;
    let forward = -normalize(view.world_from_view[2].xyz);
    i.forward = forward;
    let depth_m = dot(in.world - view.world_position, forward);
    i.frag = vec4<f32>(in.clip_position.xy, in.clip_position.z, depth_m * 100.0);
#ifdef DEPTH_PREPASS
    let scene = prepass_depth(in.clip_position, 0u);
    i.scene_z = scene;
    // No scene behind (sky, the far plane): as good as infinitely far.
    i.depth_delta = select(1.0e6, (-depth_ndc_to_view_z(scene) - depth_m) * 100.0, scene > 0.0);
#else
    i.scene_z = 0.0;
    i.depth_delta = 1.0e6;
#endif
    // Pixel (x, y, device depth, 1) to camera-relative world: translate(-camera) x world_from_clip x pixel_to_ndc.
    let size = view.viewport.zw;
    let pixel_to_ndc = mat4x4<f32>(vec4<f32>(2.0 / size.x, 0.0, 0.0, 0.0), vec4<f32>(0.0, -2.0 / size.y, 0.0, 0.0), vec4<f32>(0.0, 0.0, 1.0, 0.0), vec4<f32>(-1.0 - 2.0 * view.viewport.x / size.x, 1.0 + 2.0 * view.viewport.y / size.y, 0.0, 1.0));
    let to_camera = mat4x4<f32>(vec4<f32>(1.0, 0.0, 0.0, 0.0), vec4<f32>(0.0, 1.0, 0.0, 0.0), vec4<f32>(0.0, 0.0, 1.0, 0.0), vec4<f32>(-view.world_position, 1.0));
    let m = to_camera * view.world_from_clip * pixel_to_ndc;
    i.row_a = m[0];
    i.row_b = m[1];
    i.row_c = m[2];
    i.row_d = m[3];
    return i;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let blend = params.flags.x;
#ifdef FX_EXACT
    let fi = fx_in(in);
    let r = fx_material(fi);
    // Dev (`DB_FX_DEBUG`, `fx_material.rs`): 1 shows the opacity, 2 the emissive, 3 the depth delta (red = 50 cm,
    // green = the pixel's depth / 10 m), all opaque; 9 draws with opacity 1.
    if params.flags.y == 3u {
        return vec4<f32>(clamp(fi.depth_delta / 50.0, 0.0, 1.0), clamp(fi.frag.w / 1000.0, 0.0, 1.0), 0.0, 1.0);
    }
    if params.flags.y == 1u {
        return vec4<f32>(vec3<f32>(clamp(r.a, 0.0, 1.0)), 1.0);
    }
    if params.flags.y == 2u {
        return vec4<f32>(r.rgb / (vec3<f32>(1.0) + r.rgb), 1.0);
    }
    var lit = max(r.rgb, vec3<f32>(0.0));
#ifdef FX_LIT
    // Lit: the game adds base colour (the formula's own, else its emissive saturated) x the translucency volume's
    // ambient light, which has no direction; Bevy's lighting for a surface facing the camera and one facing away,
    // averaged, stands in.
    var p = pbr_input_new();
    p.material.base_color = vec4<f32>(fx_base_color(fi), 1.0);
    p.material.perceptual_roughness = 1.0;
    p.material.metallic = 0.0;
    p.material.reflectance = vec3<f32>(0.0);
    p.frag_coord = in.clip_position;
    p.world_position = vec4<f32>(in.world, 1.0);
    p.V = normalize(view.world_position - in.world);
    p.N = p.V;
    p.world_normal = p.V;
    let front = apply_pbr_lighting(p).rgb;
    p.N = -p.V;
    p.world_normal = -p.V;
    lit += 0.5 * (front + apply_pbr_lighting(p).rgb);
#endif
    let rgb = lit * params.tint.rgb;
    let a = select(clamp(r.a * params.tint.a, 0.0, 1.0), 1.0, params.flags.y == 9u);
#else
    var c = textureSample(fx_tex_a, fx_smp_a, in.uv0);
    if in.misc.x > 0.0 {
        c = mix(c, textureSample(fx_tex_a, fx_smp_a, in.uv1), in.misc.x);
    }
    let rgb = c.rgb * in.color.rgb * params.tint.rgb;
    let a = clamp(c.a * in.color.a * params.tint.a, 0.0, 1.0);
#endif
    var out: vec4<f32>;
    if blend == 1u {
        out = vec4<f32>(rgb * a, 1.0);
    } else if blend == 2u {
        out = vec4<f32>(rgb, 1.0);
    } else {
        out = vec4<f32>(rgb, a);
    }
#ifdef TONEMAP_IN_SHADER
    if blend != 2u {
        out = vec4<f32>(tone_mapping(vec4<f32>(out.rgb, 1.0), view.color_grading).rgb, out.a);
    }
#endif
    if blend == 1u {
        out.a = 0.0;
    }
    return out;
}

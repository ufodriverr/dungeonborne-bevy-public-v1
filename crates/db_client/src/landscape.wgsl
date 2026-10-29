// Landscape blend (`landscape.rs`): the component's weightmap picks how much of each paint layer shows; the
// layers' base colour, normal and RMAH (roughness, ambient occlusion, height) come from texture stacks
// repeating in world space, a colour-only layer (water) is its colour.: the layers blend by height (where two
// meet, the higher texel wins over a band of `HEIGHT_BAND`; HYPOTHESIS, the layer functions aren't decoded)
// and the AO darkens the diffuse light.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct LandscapeParams {
    channels: vec4<i32>,
    stack: vec4<i32>,
    scale_bias: vec4<f32>,
    color: vec4<f32>,
    tiling: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> params: LandscapeParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var base_tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var base_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var normal_tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var normal_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var rough_tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var rough_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var weight_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var weight_smp: sampler;

// Height band of the blend (ours): weights within it of the highest layer mix, the rest drop out.
const HEIGHT_BAND: f32 = 0.2;

fn pick(w: vec4<f32>, c: i32) -> f32 {
    if c < 0 || c > 3 {
        return 0.0;
    }
    return w[c];
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_UVS_A
    let wuv = in.uv * params.scale_bias.xy + params.scale_bias.zw;
#else
    let wuv = vec2<f32>(0.5, 0.5);
#endif
    let wm = textureSample(weight_tex, weight_smp, wuv);
    var w = vec4<f32>(pick(wm, params.channels.x), pick(wm, params.channels.y), pick(wm, params.channels.z), pick(wm, params.channels.w));
    let total = w.x + w.y + w.z + w.w;
    if total < 0.001 {
        w = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    } else {
        w = w / total;
    }

    // World XZ in metres, repeating every 1 / color.w metres; sampled with explicit gradients so the
    // per-layer loop needs no uniform control flow.
    let uv0 = in.world_position.xz * params.color.w;
    let gx = dpdx(uv0);
    let gy = dpdy(uv0);
    var bases: array<vec3<f32>, 4>;
    var normals: array<vec3<f32>, 4>;
    var rmah: array<vec4<f32>, 4>;
    for (var i = 0; i < 4; i += 1) {
        let layer = params.stack[i];
        let t = params.tiling[i];
        if layer < 0 {
            bases[i] = params.color.rgb;
            normals[i] = vec3<f32>(0.0, 0.0, 1.0);
            rmah[i] = vec4<f32>(0.15, 0.15, 1.0, 0.5);
        } else {
            let uv = uv0 * t;
            bases[i] = textureSampleGrad(base_tex, base_smp, uv, layer, gx * t, gy * t).rgb;
            normals[i] = textureSampleGrad(normal_tex, normal_smp, uv, layer, gx * t, gy * t).rgb * 2.0 - 1.0;
            rmah[i] = textureSampleGrad(rough_tex, rough_smp, uv, layer, gx * t, gy * t);
        }
    }
    // Height blend: each painted layer's height plus its weight; the highest wins, those within the band mix.
    var h = vec4<f32>(-1.0);
    for (var i = 0; i < 4; i += 1) {
        if w[i] > 0.0 {
            h[i] = rmah[i].a + w[i];
        }
    }
    let top = max(max(h.x, h.y), max(h.z, h.w));
    let hw = max(h - vec4<f32>(top - HEIGHT_BAND), vec4<f32>(0.0)) * vec4<f32>(w > vec4<f32>(0.0));
    let hsum = hw.x + hw.y + hw.z + hw.w;
    if hsum > 0.0 {
        w = hw / hsum;
    }
    var color = vec3<f32>(0.0);
    var nt = vec3<f32>(0.0);
    var rough = 0.0;
    var ao = 0.0;
    for (var i = 0; i < 4; i += 1) {
        color += w[i] * bases[i];
        nt += w[i] * normals[i];
        rough += w[i] * rmah[i].r;
        ao += w[i] * rmah[i].b;
    }

    pbr_input.material.base_color = vec4<f32>(color, 1.0);
    pbr_input.material.perceptual_roughness = clamp(rough, 0.05, 1.0);
    pbr_input.material.metallic = 0.0;
    pbr_input.diffuse_occlusion = vec3<f32>(ao);
    // Tangent frame of the world-space mapping: +u along world X, the texture's up (-v) along -Z.
    let N = normalize(in.world_normal);
    let T = normalize(vec3<f32>(1.0, 0.0, 0.0) - N * N.x);
    let B = cross(N, T);
    let n_t = normalize(vec3<f32>(nt.xy, max(nt.z, 0.05)));
    pbr_input.N = normalize(n_t.x * T + n_t.y * B + n_t.z * N);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}

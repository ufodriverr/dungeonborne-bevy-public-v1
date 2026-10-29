// Shared pieces of the effect material (`fx_material.rs`): its textures and the inputs the game's material formulas
// read (`_data/assets/generated/effects/fx_perms.wgsl`, made from the cooked shaders). Units as Unreal's: cm for
// depths. The engine view rows the formulas use come from Bevy's view.

#define_import_path db_client::fx_common

@group(#{MATERIAL_BIND_GROUP}) @binding(1) var fx_tex_a: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var fx_smp_a: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var fx_tex_b: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var fx_smp_b: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var fx_tex_c: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var fx_smp_c: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var fx_tex_d: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var fx_smp_d: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var fx_tex_e: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var fx_smp_e: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(11) var fx_tex_f: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(12) var fx_smp_f: sampler;

/// What a material formula reads (its engine inputs, named as in the formulas' notes).
struct FxIn {
    /// UV0 (a flipbook's current frame) and UV1 (its next frame, a ribbon's second UV).
    uv_a: vec2<f32>,
    uv_b: vec2<f32>,
    /// `Particles.Color`.
    color: vec4<f32>,
    /// `DynamicMaterialParameter` 0-3.
    dyn_a: vec4<f32>,
    dyn_b: vec4<f32>,
    dyn_c: vec4<f32>,
    dyn_d: vec4<f32>,
    /// x: flipbook blend, y: particle random, z: normalized age.
    misc: vec4<f32>,
    /// The surface normal (Bevy space; only its dot with the camera vector is used).
    normal: vec3<f32>,
    /// `SV_Position`: pixel x, y, device depth, linear depth (cm).
    frag: vec4<f32>,
    /// The scene's device depth behind the pixel, and scene depth minus pixel depth (cm).
    scene_z: f32,
    depth_delta: f32,
    time: f32,
    /// Which dynamic parameters the emitter writes (bits 1, 2, 4, 8 for D0-D3).
    dyn_valid: u32,
    /// The engine's `SVPositionToTranslatedWorld` rows (View[80..83]).
    row_a: vec4<f32>,
    row_b: vec4<f32>,
    row_c: vec4<f32>,
    row_d: vec4<f32>,
    /// View forward (View[106]).
    forward: vec3<f32>,
}

/// One row of the engine's view uniforms as the formulas index it: 47 (its `.w` >= 1 means orthographic),
/// 80-83 (pixel to camera-relative world), 106 (view direction), 111 (device depth to linear, for the orthographic
/// branch only), 207 (a facing sign); others zero (the formulas that read more are the screen-space blurs, not drawn).
fn fx_view(i: FxIn, n: u32) -> vec4<f32> {
    switch n {
        case 80u: { return i.row_a; }
        case 81u: { return i.row_b; }
        case 82u: { return i.row_c; }
        case 83u: { return i.row_d; }
        case 106u: { return vec4<f32>(i.forward, 0.0); }
        case 111u: { return vec4<f32>(0.0, 0.0, 1.0, 0.0); }
        // `DefaultSpriteMaterial`'s facing sign (`.w`): 1, its colour at full (HYPOTHESIS).
        case 207u: { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
        default: { return vec4<f32>(0.0); }
    }
}

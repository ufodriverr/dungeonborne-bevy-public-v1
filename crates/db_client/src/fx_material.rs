//! The effect material: one material per effect emitter, drawing a batch mesh that the effect renderer
//! (`effects.rs`) rebuilds every frame from its particles. Per-particle values ride on the vertices (colour, the
//! Niagara dynamic material parameters, the flipbook's second frame and blend, the normal), so a particle needs no
//! material of its own. An emitter whose material instance has a recovered formula (static reading of the cooked
//! shaders, made WGSL into `fx_perms.wgsl`) draws by that formula; the rest keep the stand-in (texture x particle
//! colour, `fx.wgsl`).

use std::collections::HashMap;

use bevy::asset::embedded_asset;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, VertexFormat};
use bevy::shader::{ShaderDefVal, ShaderRef};

/// Niagara `DynamicMaterialParameter` 0-3 per vertex.
pub(crate) const ATTRIBUTE_FX_DYN: MeshVertexAttribute = MeshVertexAttribute::new("FxDyn", 731_000_117, VertexFormat::Float32x4);
pub(crate) const ATTRIBUTE_FX_DYN1: MeshVertexAttribute = MeshVertexAttribute::new("FxDyn1", 731_000_119, VertexFormat::Float32x4);
pub(crate) const ATTRIBUTE_FX_DYN2: MeshVertexAttribute = MeshVertexAttribute::new("FxDyn2", 731_000_120, VertexFormat::Float32x4);
pub(crate) const ATTRIBUTE_FX_DYN3: MeshVertexAttribute = MeshVertexAttribute::new("FxDyn3", 731_000_121, VertexFormat::Float32x4);
/// x: the flipbook blend towards `UV_1`, y: the particle's own random 0..1, z: its life fraction, w: unused.
pub(crate) const ATTRIBUTE_FX_MISC: MeshVertexAttribute = MeshVertexAttribute::new("FxMisc", 731_000_118, VertexFormat::Float32x4);

/// The generated formulas (asset path under `_data/assets`).
const PERMS: &str = "generated/effects/fx_perms.wgsl";

pub(crate) struct FxMaterialPlugin;

impl Plugin for FxMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "fx.wgsl");
        embedded_asset!(app, "fx_common.wgsl");
        app.add_plugins(MaterialPlugin::<FxMaterial>::default());
    }
}

/// How a material instance blends (the cooked material's blend mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Reflect)]
pub(crate) enum FxBlend {
    #[default]
    Alpha,
    Additive,
    Modulate,
}

/// One texture slot of a recovered material: the library PNG and how it's sampled.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FxTexture {
    pub path: String,
    pub srgb: bool,
    pub clamp: bool,
}

/// A material instance with a recovered formula.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FxExact {
    pub perm: u32,
    pub blend: FxBlend,
    pub textures: Vec<FxTexture>,
    /// Lit: its base colour (the emissive, saturated) also takes the scene's light.
    pub lit: bool,
}

/// The recovered formulas by material path (`_data/recovered/effects/fx_materials.json`), and the generated shader
/// kept loaded so its import path resolves. Materials listed with a reason not to draw them are in `skip`.
#[derive(Resource, Default)]
pub(crate) struct FxTable {
    pub exact: HashMap<String, FxExact>,
    pub skip: HashMap<String, String>,
    /// The shader modules imported by name (`fx_common`, `fx_perms`), kept loaded so their import paths resolve
    /// (a module is known only once its file has loaded).
    _modules: Vec<Handle<Shader>>,
    /// Every formula's textures, loading from the start and kept loaded: loaded on first use, a short effect (the
    /// Pyroblast hit's 0.6 s modulate glow) was over before its texture was, and drew with the empty fallback.
    _textures: Vec<Handle<Image>>,
}

impl FxTable {
    /// A material the game draws in a pass the port doesn't have (refraction only, a scene-colour blur): left out.
    pub(crate) fn hidden(&self, material: &str) -> bool {
        self.skip.get(material).is_some_and(|w| w.contains("refraction") || w.contains("scene colour"))
    }
}

/// Reads the recovered formulas (the effect library loads it with the effects).
pub(crate) fn read_fx_table(assets: &AssetServer) -> FxTable {
    let rel = "recovered/effects/fx_materials.json";
    let mut table = FxTable { _modules: vec![assets.load("embedded://db_client/fx_common.wgsl")], ..default() };
    let v = crate::map_view::data_file(rel).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    if let Some(v) = v
        && crate::map_view::data_file(&format!("assets/{PERMS}")).is_some()
    {
        for (path, e) in v["materials"].as_object().into_iter().flatten() {
            if let Some(why) = e["skip"].as_str() {
                table.skip.insert(path.clone(), why.to_string());
                continue;
            }
            let Some(perm) = e["perm"].as_u64() else { continue };
            let blend = match e["blend"].as_str() {
                Some("Additive") => FxBlend::Additive,
                Some("Modulate") => FxBlend::Modulate,
                _ => FxBlend::Alpha,
            };
            let textures = e["textures"].as_array().into_iter().flatten().map(|t| FxTexture { path: t["path"].as_str().unwrap_or("").to_string(), srgb: t["srgb"].as_bool().unwrap_or(true), clamp: t["clamp"].as_bool().unwrap_or(false) }).collect();
            table.exact.insert(path.clone(), FxExact { perm: perm as u32, blend, textures, lit: e["lit"].as_bool().unwrap_or(false) });
        }
        table._modules.push(assets.load(PERMS));
        let mut seen = std::collections::HashSet::new();
        table._textures = table.exact.values().flat_map(|x| &x.textures).filter(|t| seen.insert(t.path.clone())).filter_map(|t| load_fx_texture(assets, t)).collect();
        info!("effect materials: {} formulas, {} left out", table.exact.len(), table.skip.len());
    } else {
        warn!("no _data/{rel} or _data/assets/{PERMS} (tools/recon/fx_materials.py); effects keep the stand-in look");
    }
    table
}

/// A recovered material's texture, sampled as the game samples it (sRGB or linear, wrap or clamp).
pub(crate) fn load_fx_texture(assets: &AssetServer, t: &FxTexture) -> Option<Handle<Image>> {
    crate::map_view::data_file(&format!("assets/{}", t.path))?;
    let (srgb, mode) = (t.srgb, if t.clamp { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat });
    Some(assets.load_builder().with_settings(move |s: &mut ImageLoaderSettings| {
        s.is_srgb = srgb;
        s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: mode, address_mode_v: mode, ..ImageSamplerDescriptor::linear() });
    }).load(t.path.clone()))
}

/// The material's numbers (`fx.wgsl` `FxParams`).
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub(crate) struct FxParams {
    /// Multiplies emissive and opacity (1 unless a caller dims an effect).
    pub tint: Vec4,
    /// x: blend (0 alpha, 1 additive, 2 modulate), y: debug view (`DB_FX_DEBUG`), z: the dynamic parameters the emitter writes (bits 1, 2, 4, 8).
    pub flags: UVec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(FxKey)]
pub(crate) struct FxMaterial {
    #[uniform(0)]
    pub params: FxParams,
    #[texture(1)]
    #[sampler(2)]
    pub tex0: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    pub tex1: Option<Handle<Image>>,
    #[texture(5)]
    #[sampler(6)]
    pub tex2: Option<Handle<Image>>,
    #[texture(7)]
    #[sampler(8)]
    pub tex3: Option<Handle<Image>>,
    #[texture(9)]
    #[sampler(10)]
    pub tex4: Option<Handle<Image>>,
    #[texture(11)]
    #[sampler(12)]
    pub tex5: Option<Handle<Image>>,
    pub blend: FxBlend,
    /// The recovered formula's number in `fx_perms.wgsl` (0: the stand-in).
    pub perm: u32,
    /// Its base colour takes the scene's light (`FxExact::lit`).
    pub lit: bool,
}

impl FxMaterial {
    pub(crate) fn new(blend: FxBlend, perm: u32, dyn_valid: u32, textures: Vec<Option<Handle<Image>>>) -> Self {
        let mut t = textures.into_iter();
        let mut n = || t.next().flatten();
        let code = match blend {
            FxBlend::Alpha => 0,
            FxBlend::Additive => 1,
            FxBlend::Modulate => 2,
        };
        // Dev: `DB_FX_DEBUG=1` draws the recovered formulas' opacity, `2` their emissive, `3` the depth delta (opaque),
        // `9` the normal output at opacity 1 (checks; `fx.wgsl`).
        let debug = if perm > 0 { std::env::var("DB_FX_DEBUG").ok().and_then(|v| v.parse().ok()).unwrap_or(0) } else { 0 };
        Self { params: FxParams { tint: Vec4::ONE, flags: UVec4::new(code, debug, dyn_valid, 0) }, tex0: n(), tex1: n(), tex2: n(), tex3: n(), tex4: n(), tex5: n(), blend, perm, lit: false }
    }
}

impl Material for FxMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://db_client/fx.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://db_client/fx.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.blend {
            FxBlend::Alpha => AlphaMode::Blend,
            FxBlend::Additive => AlphaMode::Add,
            FxBlend::Modulate => AlphaMode::Multiply,
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(_pipeline: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, layout: &MeshVertexBufferLayoutRef, key: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_1.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(3),
            ATTRIBUTE_FX_DYN.at_shader_location(4),
            ATTRIBUTE_FX_MISC.at_shader_location(5),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(6),
            ATTRIBUTE_FX_DYN1.at_shader_location(7),
            ATTRIBUTE_FX_DYN2.at_shader_location(8),
            ATTRIBUTE_FX_DYN3.at_shader_location(9),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        let FxKey { blend, perm, lit } = key.bind_group_data;
        if let Some(fragment) = descriptor.fragment.as_mut() {
            if perm > 0 {
                fragment.shader_defs.push("FX_EXACT".into());
                fragment.shader_defs.push(ShaderDefVal::UInt("FX_PERM".into(), perm));
                if lit {
                    fragment.shader_defs.push("FX_LIT".into());
                }
            }
            // The blend set here, not from Bevy's alpha-mode keys: additive is one + one of the shader's premultiplied
            // colour, alpha is straight alpha, modulate multiplies the colour behind.
            let add = BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
            let keep = BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add };
            let state = match blend {
                FxBlend::Additive => BlendState { color: add, alpha: keep },
                FxBlend::Modulate => BlendState { color: BlendComponent { src_factor: BlendFactor::Dst, dst_factor: BlendFactor::Zero, operation: BlendOperation::Add }, alpha: keep },
                FxBlend::Alpha => BlendState::ALPHA_BLENDING,
            };
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(state);
            }
        }
        Ok(())
    }
}

/// Which pipeline a material needs (its blend and formula).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FxKey {
    pub blend: FxBlend,
    pub perm: u32,
    pub lit: bool,
}

impl From<&FxMaterial> for FxKey {
    fn from(m: &FxMaterial) -> Self {
        Self { blend: m.blend, perm: m.perm, lit: m.lit }
    }
}

#[cfg(test)]
mod tests {
    /// Every generated material formula is valid WGSL against `fx_common.wgsl` (skipped without the data).
    #[test]
    fn generated_formulas_validate() {
        let Some(p) = crate::map_view::data_file(&format!("assets/{}", super::PERMS)) else { return };
        let perms = std::fs::read_to_string(p).unwrap();
        let common: String = include_str!("fx_common.wgsl").lines().filter(|l| !l.starts_with("#define_import_path")).collect::<Vec<_>>().join("\n").replace("#{MATERIAL_BIND_GROUP}", "2");
        let mut failed = Vec::new();
        let mut n = 0;
        for block in perms.split("#if FX_PERM == ").skip(1) {
            let (num, rest) = block.split_once('\n').unwrap();
            let body = rest.split("#endif").next().unwrap().replace("db_client::fx_common::FxIn", "FxIn");
            let src = format!("{common}\n{body}");
            n += 1;
            let module = match naga::front::wgsl::parse_str(&src) {
                Ok(m) => m,
                Err(e) => {
                    failed.push(format!("{num}: {}", e.emit_to_string(&src)));
                    continue;
                }
            };
            if let Err(e) = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module) {
                failed.push(format!("{num}: {}", e.emit_to_string(&src)));
            }
        }
        assert!(failed.is_empty(), "{} of {n} formulas invalid:\n{}", failed.len(), failed.iter().take(3).cloned().collect::<Vec<_>>().join("\n"));
    }
}

//! Landscape (terrain) material. The converted landscape components carry an empty material, which glTF reads as
//! white and fully metallic (the castle's ground drew black and white). indexes each component's paint layers from
//! the map dump (`<level>-landscape.json`: layers, each component's weightmap, scale-bias and the weightmap channel
//! of each layer) and stacks the layers' base colour, normal and roughness textures into one image per kind. Here
//! each component gets a material that samples its weightmap (UV 0 is the component's quad grid, as in UE: weightmap
//! UV = UV 0 × scale + bias) and blends the layers by weight; a layer without textures (water) is its colour.
//! HYPOTHESIS: linear blend, textures repeating every `repeat_quads` landscape quads in world space (the layer
//! material functions aren't decoded).

use std::collections::HashMap;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{ImageAddressMode, ImageArrayLayout, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub(crate) type LandscapeMaterial = ExtendedMaterial<StandardMaterial, LandscapeExt>;

pub(crate) fn plugin(app: &mut App) {
    embedded_asset!(app, "landscape.wgsl");
    app.add_plugins(MaterialPlugin::<LandscapeMaterial>::default()).init_resource::<Landscapes>().add_systems(Update, (load_landscapes, apply_landscape, mip_landscape_images).chain());
}

/// Put on a level's root by `map_view::spawn_level`: the level (full name) whose landscape index to load.
#[derive(Component, Debug, Clone)]
pub(crate) struct LandscapeLevel(pub(crate) String);

#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub(crate) struct LandscapeParams {
    /// Weightmap channel (0-3) of each layer in this component, -1 where it isn't painted.
    channels: IVec4,
    /// Index of each layer in the texture stacks, -1 for a colour-only layer.
    stack: IVec4,
    /// Weightmap UV = UV 0 × xy + zw (`WeightmapScaleBias`).
    scale_bias: Vec4,
    /// Colour of each colour-only layer: rgb of the last one (water); w: texture repeats per metre.
    color: Vec4,
    /// Each layer's `Tiling` parameter.
    tiling: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(crate) struct LandscapeExt {
    #[uniform(100)]
    params: LandscapeParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    base: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    normal: Handle<Image>,
    #[texture(105, dimension = "2d_array")]
    #[sampler(106)]
    rough: Handle<Image>,
    #[texture(107)]
    #[sampler(108)]
    weights: Handle<Image>,
}

impl MaterialExtension for LandscapeExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://db_client/landscape.wgsl".into()
    }
}

/// Landscape materials by mesh name (`LandscapeComponent_<n>`), and the stacks still waiting for mipmaps.
#[derive(Resource, Debug, Default)]
pub(crate) struct Landscapes {
    by_mesh: HashMap<String, Handle<LandscapeMaterial>>,
    unmipped: Vec<Handle<Image>>,
}

fn repeat_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat, mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, anisotropy_clamp: 8, ..default() })
}

fn load_landscapes(levels: Query<&LandscapeLevel, Added<LandscapeLevel>>, assets: Res<AssetServer>, mut materials: ResMut<Assets<LandscapeMaterial>>, mut land: ResMut<Landscapes>) {
    for level in &levels {
        let Some(index) = Some(db_formats::data_path(format!("recovered/maps/{}-landscape.json", level.0))).filter(|p| p.is_file()).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { continue };
        let layers = index["layers"].as_array().cloned().unwrap_or_default();
        let textured = layers.iter().filter(|l| l["base"].is_string()).count().max(1) as u32;
        let stack = |kind: &str, srgb: bool| -> Handle<Image> {
            assets
                .load_builder()
                .with_settings(move |s: &mut ImageLoaderSettings| {
                    s.is_srgb = srgb;
                    s.sampler = repeat_sampler();
                    s.array_layout = Some(ImageArrayLayout::RowCount { rows: textured });
                    s.asset_usage = RenderAssetUsages::default();
                })
                .load(index["stacks"][kind].as_str().unwrap_or_default().to_string())
        };
        let (base, normal, rough) = (stack("base", true), stack("normal", false), stack("rough", false));
        land.unmipped.extend([base.clone(), normal.clone(), rough.clone()]);
        let mut stack_index = IVec4::splat(-1);
        let mut tiling = Vec4::ONE;
        let mut color = Vec3::splat(0.1);
        let mut k = 0;
        for (i, l) in layers.iter().take(4).enumerate() {
            if l["base"].is_string() {
                stack_index[i] = k;
                k += 1;
            } else if let Some(c) = l["color"].as_array() {
                color = Vec3::new(c[0].as_f64().unwrap_or(0.1) as f32, c[1].as_f64().unwrap_or(0.1) as f32, c[2].as_f64().unwrap_or(0.1) as f32);
            }
            tiling[i] = l["tiling"].as_f64().unwrap_or(1.0) as f32;
        }
        // The textures repeat every `repeat_quads` quads of 1 m (the castle landscape's scale 100).
        let per_m = 1.0 / index["repeat_quads"].as_f64().unwrap_or(8.0) as f32;
        let Some(comps) = index["components"].as_object() else { continue };
        for (name, c) in comps {
            let Some(weightmap) = c["weightmaps"].get(0).and_then(|v| v.as_str()) else { continue };
            let weights: Handle<Image> = assets
                .load_builder()
                .with_settings(|s: &mut ImageLoaderSettings| {
                    s.is_srgb = false;
                    s.sampler = ImageSampler::linear();
                })
                .load(weightmap.to_string());
            let mut channels = IVec4::splat(-1);
            for (i, ch) in c["channels"].as_array().into_iter().flatten().take(4).enumerate() {
                // Only the first weightmap is bound (every castle component has one).
                if ch[0].as_i64() == Some(0) {
                    channels[i] = ch[1].as_i64().unwrap_or(-1) as i32;
                }
            }
            let sb = |i: usize| c["scale_bias"][i].as_f64().unwrap_or(0.0) as f32;
            let params = LandscapeParams { channels, stack: stack_index, scale_bias: Vec4::new(sb(0), sb(1), sb(2), sb(3)), color: color.extend(per_m), tiling };
            let base_material = StandardMaterial { base_color: Color::WHITE, metallic: 0.0, perceptual_roughness: 0.9, reflectance: 0.3, ..default() };
            let h = materials.add(ExtendedMaterial { base: base_material, extension: LandscapeExt { params, base: base.clone(), normal: normal.clone(), rough: rough.clone(), weights } });
            land.by_mesh.insert(name.clone(), h);
        }
        info!("landscape of {}: {} components, {} layers", level.0, comps.len(), layers.len());
    }
}

#[allow(clippy::type_complexity)]
fn apply_landscape(mut commands: Commands, land: Res<Landscapes>, added: Query<(Entity, &bevy::gltf::GltfMeshName), (Added<bevy::gltf::GltfMeshName>, With<MeshMaterial3d<StandardMaterial>>)>) {
    if land.by_mesh.is_empty() {
        return;
    }
    for (e, name) in &added {
        if let Some(h) = land.by_mesh.get(&name.0) {
            commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().try_insert(MeshMaterial3d(h.clone()));
        }
    }
}

/// The stacks get a mip chain once loaded (the PNGs have none; without them the repeating ground shimmers
/// in the distance).
fn mip_landscape_images(mut land: ResMut<Landscapes>, mut images: ResMut<Assets<Image>>) {
    land.unmipped.retain(|h| {
        let Some(mut img) = images.get_mut(h) else { return true };
        add_mips(&mut img);
        false
    });
}

/// A box-filtered mip chain for an RGBA8 image (2D or array; layer-major data).
fn add_mips(img: &mut Image) {
    let d = img.texture_descriptor.size;
    if img.texture_descriptor.mip_level_count > 1 || img.texture_descriptor.format.block_copy_size(None) != Some(4) {
        return;
    }
    let Some(data) = img.data.as_ref() else { return };
    let (w0, h0, layers) = (d.width as usize, d.height as usize, d.depth_or_array_layers as usize);
    let levels = (w0.max(h0) as f32).log2().floor() as u32 + 1;
    let mut out = Vec::with_capacity(data.len() * 4 / 3 + 64);
    for layer in 0..layers {
        let mut cur = data[layer * w0 * h0 * 4..(layer + 1) * w0 * h0 * 4].to_vec();
        let (mut w, mut h) = (w0, h0);
        out.extend_from_slice(&cur);
        for _ in 1..levels {
            let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
            let mut next = vec![0u8; nw * nh * 4];
            for y in 0..nh {
                for x in 0..nw {
                    for c in 0..4 {
                        let px = |xx: usize, yy: usize| u32::from(cur[((yy.min(h - 1)) * w + xx.min(w - 1)) * 4 + c]);
                        next[(y * nw + x) * 4 + c] = ((px(2 * x, 2 * y) + px(2 * x + 1, 2 * y) + px(2 * x, 2 * y + 1) + px(2 * x + 1, 2 * y + 1) + 2) / 4) as u8;
                    }
                }
            }
            out.extend_from_slice(&next);
            cur = next;
            (w, h) = (nw, nh);
        }
    }
    img.data = Some(out);
    img.texture_descriptor.mip_level_count = levels;
}

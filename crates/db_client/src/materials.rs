//! Material overrides: fills what the converted glTF materials lost (blend mode and mask, two-sided, base
//! colour textures, albedo tints, emissive) from the recovered material data. The table
//! `_data/recovered/materials/overrides.json` comes; each glTF material names its Unreal material in
//! `extras.unrealMaterial`, which the glTF loader puts on the mesh entity as `GltfMaterialExtras`. A
//! changed copy of each material is made once and shared. Meshes with a recovered material also lose their
//! vertex colours: the converter kept the painted colours, which Bevy multiplies into the albedo, while
//! Unreal only uses them where a material graph reads them (here as layer-blend masks: raw red/green paint
//! on floors, rainbow webs; HYPOTHESIS). Without the file (or with `--raw-materials`) the glTF materials
//! stay as they are.
//!
//! A few map meshes lost their material in the conversion (glTF material "None"); the umaps set it per
//! component (`OverrideMaterials`). `unresolved_meshes.json` (same generator) says how to draw them by
//! mesh name: water planes as translucent water in their shader's colour, ground-fog volumes and
//! blocking volumes hidden (HYPOTHESIS). Bevy would otherwise draw them plain white.

use std::collections::HashMap;

use bevy::gltf::GltfMaterialExtras;
use bevy::prelude::*;

/// One material's changes (see the generator for the rules).
#[derive(Debug, Clone, Default, PartialEq)]
struct Override {
    alpha: Option<AlphaMode>,
    double_sided: bool,
    base_texture: Option<String>,
    tint: Option<[f32; 3]>,
    opacity: Option<f32>,
    emissive: Option<[f32; 3]>,
    emissive_texture: Option<String>,
}

/// How a map mesh whose material the conversion lost is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LostLook {
    Hidden,
    /// Translucent water in this linear colour.
    Water([f32; 3]),
}

/// Opacity of the stand-in water surface (ours; the water shaders' depth fade isn't rebuilt).
const WATER_ALPHA: f32 = 0.75;

/// The override table, and the changed copies made so far (original material → copy).
#[derive(Resource, Debug, Default)]
pub struct MaterialOverrides {
    table: HashMap<String, Override>,
    copies: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    /// Map meshes with a lost material, by glTF mesh name, and their water materials once made.
    lost: HashMap<String, LostLook>,
    water: HashMap<String, Handle<StandardMaterial>>,
    /// Meshes already checked for vertex colours.
    meshes: std::collections::HashSet<AssetId<Mesh>>,
    /// Applied count (for logs and checks).
    pub applied: usize,
    /// Override textures still loading (a mesh isn't drawn until its material's textures are in; the
    /// entry cover waits for them).
    loading: Vec<Handle<Image>>,
}

impl MaterialOverrides {
    /// Override textures still loading; drops the ones done (loaded or failed).
    pub(crate) fn still_loading(&mut self, assets: &AssetServer) -> usize {
        self.loading.retain(|h| matches!(assets.load_state(h.id()), bevy::asset::LoadState::Loading | bevy::asset::LoadState::NotLoaded));
        self.loading.len()
    }
}

/// Dev switch: keep the glTF materials as converted (before/after screenshots).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct RawMaterials(pub bool);

fn parse_lost(v: &serde_json::Value) -> LostLook {
    match (v["look"].as_str(), v["color"].as_array()) {
        (Some("water"), Some(c)) if c.len() >= 3 => LostLook::Water([0, 1, 2].map(|i| c[i].as_f64().unwrap_or(0.1) as f32)),
        _ => LostLook::Hidden,
    }
}

fn parse(v: &serde_json::Value) -> Override {
    let rgb = |k: &str| v[k].as_array().filter(|a| a.len() >= 3).map(|a| [0, 1, 2].map(|i| a[i].as_f64().unwrap_or(1.0) as f32));
    let alpha = match v["alpha"].as_str() {
        Some("mask") => Some(AlphaMode::Mask(v["clip"].as_f64().unwrap_or(0.3333) as f32)),
        Some("blend") => Some(AlphaMode::Blend),
        Some("add") => Some(AlphaMode::Add),
        Some("multiply") => Some(AlphaMode::Multiply),
        _ => None,
    };
    Override {
        alpha,
        double_sided: v["double_sided"].as_bool().unwrap_or(false),
        base_texture: v["base_texture"].as_str().map(str::to_string),
        tint: rgb("tint"),
        opacity: v["opacity"].as_f64().map(|x| x as f32),
        emissive: rgb("emissive"),
        emissive_texture: v["emissive_texture"].as_str().map(str::to_string),
    }
}

/// Startup: reads the table.
pub(crate) fn load_overrides(mut commands: Commands, raw: Option<Res<RawMaterials>>) {
    let mut res = MaterialOverrides::default();
    let rel = "recovered/materials/overrides.json";
    if raw.is_some_and(|r| r.0) {
        info!("--raw-materials: material overrides off");
    } else if let Some(v) = crate::map_view::data_file(rel).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        res.table = v.as_object().into_iter().flatten().map(|(k, o)| (k.clone(), parse(o))).collect();
        info!("{} material overrides", res.table.len());
        if let Some(v) = crate::map_view::data_file("recovered/materials/unresolved_meshes.json").and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
            res.lost = v.as_object().into_iter().flatten().map(|(k, o)| (k.clone(), parse_lost(o))).collect();
        }
    } else {
        warn!("no _data/{rel} (tools/recon/material_overrides.py); glTF materials as converted");
    }
    commands.insert_resource(res);
}

/// The Unreal material path in a glTF material's extras.
fn unreal_path(extras: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(extras).ok()?["unrealMaterial"].as_str().map(str::to_string)
}

/// Swaps newly spawned glTF meshes' materials for their overridden copies.
#[allow(clippy::type_complexity)]
pub(crate) fn apply_overrides(
    mut commands: Commands,
    mut ov: ResMut<MaterialOverrides>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AssetServer>,
    added: Query<(Entity, &GltfMaterialExtras, &MeshMaterial3d<StandardMaterial>, Option<&Mesh3d>, Option<&bevy::gltf::GltfMeshName>), Added<GltfMaterialExtras>>,
) {
    if ov.table.is_empty() {
        return;
    }
    for (e, extras, mat, mesh, mesh_name) in &added {
        let Some(path) = unreal_path(&extras.value) else {
            // A lost material: draw the mesh as the umap's material says (or hide it).
            let Some((name, look)) = mesh_name.and_then(|n| ov.lost.get(&n.0).map(|l| (n.0.clone(), *l))) else { continue };
            match look {
                LostLook::Hidden => {
                    commands.entity(e).try_insert(Visibility::Hidden);
                }
                LostLook::Water([r, g, b]) => {
                    let h = ov.water.entry(name).or_insert_with(|| mats.add(StandardMaterial { base_color: Color::linear_rgba(r, g, b, WATER_ALPHA), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.08, reflectance: 0.6, double_sided: true, cull_mode: None, ..default() })).clone();
                    commands.entity(e).try_insert(MeshMaterial3d(h));
                }
            }
            ov.applied += 1;
            continue;
        };
        if let Some(m) = mesh
            && ov.meshes.insert(m.0.id())
            && meshes.get(m.0.id()).is_some_and(|x| x.contains_attribute(Mesh::ATTRIBUTE_COLOR))
            && let Some(mut x) = meshes.get_mut(m.0.id())
        {
            x.remove_attribute(Mesh::ATTRIBUTE_COLOR);
        }
        let Some(o) = ov.table.get(&path).cloned() else { continue };
        let id = mat.0.id();
        let handle = match ov.copies.get(&id) {
            Some(h) => h.clone(),
            None => {
                let Some(mut m) = mats.get(id).cloned() else { continue };
                if let Some(a) = o.alpha {
                    m.alpha_mode = a;
                    // The converter writes base colour alpha 0 for many opaque materials; opacity now
                    // comes from the texture alone.
                    m.base_color.set_alpha(1.0);
                }
                if o.double_sided {
                    m.double_sided = true;
                    m.cull_mode = None;
                }
                if let Some(t) = &o.base_texture {
                    let h: Handle<Image> = assets.load(t.clone());
                    ov.loading.push(h.clone());
                    m.base_color_texture = Some(h);
                }
                if let Some([r, g, b]) = o.tint {
                    let a = m.base_color.alpha();
                    m.base_color = Color::linear_rgba(r, g, b, a);
                }
                if let Some(a) = o.opacity {
                    m.base_color.set_alpha(a);
                }
                if let Some([r, g, b]) = o.emissive {
                    m.emissive = LinearRgba::rgb(r, g, b);
                    m.emissive_texture = o.emissive_texture.as_ref().map(|t| assets.load(t.clone()));
                    ov.loading.extend(m.emissive_texture.clone());
                }
                let h = mats.add(m);
                ov.copies.insert(id, h.clone());
                h
            }
        };
        // The cell may unload in the same frame.
        commands.entity(e).try_insert(MeshMaterial3d(handle));
        ov.applied += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_generator_fields() {
        let v = serde_json::json!({"alpha": "mask", "clip": 0.5, "double_sided": true, "base_texture": "textures/a.png", "tint": [0.5, 0.25, 1.0], "opacity": 0.3, "emissive": [2.0, 1.0, 0.0], "emissive_texture": "textures/e.png", "reason": ["x"]});
        let o = parse(&v);
        assert_eq!(o.alpha, Some(AlphaMode::Mask(0.5)));
        assert!(o.double_sided);
        assert_eq!(o.base_texture.as_deref(), Some("textures/a.png"));
        assert_eq!(o.tint, Some([0.5, 0.25, 1.0]));
        assert_eq!(o.opacity, Some(0.3));
        assert_eq!(o.emissive, Some([2.0, 1.0, 0.0]));
        assert_eq!(parse(&serde_json::json!({"alpha": "add"})).alpha, Some(AlphaMode::Add));
        assert_eq!(parse(&serde_json::json!({})), Override::default());
        assert_eq!(unreal_path(r#"{"unrealMaterial":"TgfMaps/X/MI_A","pbrApproximation":true}"#).as_deref(), Some("TgfMaps/X/MI_A"));
        assert_eq!(unreal_path(r#"{"unrealMaterial":null}"#), None);
        assert_eq!(parse_lost(&serde_json::json!({"look": "water", "color": [0.1, 0.2, 0.3]})), LostLook::Water([0.1, 0.2, 0.3]));
        assert_eq!(parse_lost(&serde_json::json!({"look": "hidden"})), LostLook::Hidden);
        assert_eq!(parse_lost(&serde_json::json!({"look": "water"})), LostLook::Hidden);
    }

    #[test]
    fn the_generated_table_parses() {
        let Some(p) = crate::map_view::data_file("recovered/materials/overrides.json") else { return };
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        let table: Vec<Override> = v.as_object().unwrap().values().map(parse).collect();
        assert!(!table.is_empty());
        // Every entry changes something, and every texture it names is imported.
        for (k, o) in v.as_object().unwrap().iter().zip(&table) {
            assert_ne!(*o, Override::default(), "{}", k.0);
            for t in [&o.base_texture, &o.emissive_texture].into_iter().flatten() {
                assert!(crate::map_view::data_file(&format!("assets/{t}")).is_some(), "{} uses {t}, not imported", k.0);
            }
        }
    }
}

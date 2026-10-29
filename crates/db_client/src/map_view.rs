//! The real maps: `--map <01|02|castle1|level name>` shows a level (`L_Dungeon_Official_<NN>`,
//! `L_GothicCastle_WP_Main`) from the converted scene (`bevy_ready`, already metres and Y-up) with the
//! map's lights, which the converted scene lacks: resolves them from the recovered native umap dump into
//! `_data/recovered/maps/`. `--map-top` looks straight down (orthographic, clipped just under the
//! ceiling over the player starts) with the map's gameplay placement as markers; otherwise the camera
//! stands at player start `--map-start <n>`. `--map-walk` also loads the map's sim collision
//! (`db_sim::map`, cache) and puts the player at the start, walking ahead.
//!
//! World Partition: the persistent scene's HLOD distance proxy (its `InstancedStaticMeshComponent_*` roots) is
//! dropped once the scene is in, and the full-detail `MainGrid_*` cell scenes (`-cells.json`) stream in as
//! children of the floor's scene: a cell loads when the camera comes within [`LOAD_RANGE_M`] of its bounds and
//! unloads (freeing its meshes and textures) beyond [`UNLOAD_RANGE_M`], as Unreal streams its cells. Loading
//! every cell at once ran the GPU out of memory (a mesh shared by several cell files is loaded once per file).

use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::camera::ScalingMode;
use bevy::prelude::*;

use crate::{Look, PlayerCamera, to_bevy};

/// Dev: which crypt map to show, and how.
#[derive(Resource, Debug, Default, Clone)]
pub struct MapViewer {
    /// `01` or `02`.
    pub level: Option<String>,
    pub top: bool,
    pub start: usize,
    /// Load the sim collision and walk the player from the start.
    pub walk: bool,
}

/// Bevy lumens per unit of the map's unitless light intensity (HYPOTHESIS: chosen by eye; the maps' lights
/// are 0.5-3 units with 2-5 m radii and no inverse-square falloff).
pub(crate) const LUMENS_PER_UNIT: f32 = 60_000.0;
/// Radius for lights that keep the class default (Unreal's default attenuation radius, 10 m).
const DEFAULT_RANGE_M: f32 = 10.0;

/// A cell loads when the camera is this close to its bounds (m; ours: past the longest sight line in the
/// crypt's halls, with time to load before it is seen) and unloads beyond [`UNLOAD_RANGE_M`].
pub(crate) const LOAD_RANGE_M: f32 = 80.0;
pub(crate) const UNLOAD_RANGE_M: f32 = 120.0;

/// A World Partition cell scene of a map, its bounds in world space (Bevy, metres) and its loaded
/// instance (a child entity holding the scene).
#[derive(Component, Debug, Clone)]
pub(crate) struct MapCell {
    scene: String,
    /// Its node count, for the spawn budget.
    nodes: usize,
    /// Loaded at any distance: a part of the persistent level.
    always: bool,
    min: Vec3,
    max: Vec3,
    instance: Option<Entity>,
    /// The cell's scene while it loads, before it is spawned.
    pending: Option<Handle<WorldAsset>>,
    /// The cell's lights: position (Bevy, before the floor offset), colour, intensity (map units),
    /// radius (m); spawned with the instance, so they stream with it.
    lights: Vec<(Vec3, Color, f32, f32)>,
}

/// Nodes of loaded cells spawned per frame, nearest cell first, at least one cell: Bevy spawns every scene
/// that finished loading in the same frame, and entering the castle spawned its persistent scene (98,269
/// nodes) and a 116,276-node cell in single frames of 3-4 s. Big scenes are indexed as parts of up to
/// 1,500 nodes. A cell is spawned once its scene and textures have loaded.
const NODES_PER_FRAME: usize = 3_000;
/// Bytes of meshes and textures sent to the GPU per frame in play (Bevy's soft limit), so cells streaming in
/// as the player walks don't upload in one long frame; off behind the entry cover.
pub(crate) const UPLOAD_BYTES_PER_FRAME: usize = 48 << 20;

/// Cells within reach of the camera not spawned yet (the entry cover stays up until none are left).
#[derive(Resource, Debug, Default)]
pub(crate) struct StreamBacklog(pub(crate) usize);

/// Drops the HLOD distance proxy from a persistent map scene once it is in: its root nodes named
/// `InstancedStaticMeshComponent_*` (the full-detail cells replace it). Only the topmost such nodes, and
/// never inside a cell (cells keep their own instanced meshes).
fn drop_proxy(ready: On<bevy::world_serialization::WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, names: Query<&Name>, cells: Query<(), With<MapCell>>) {
    let mut dropped = 0;
    let mut stack: Vec<Entity> = children.get(ready.entity).map(|c| c.to_vec()).unwrap_or_default();
    while let Some(e) = stack.pop() {
        if cells.contains(e) {
            continue;
        }
        if names.get(e).is_ok_and(|n| n.as_str().starts_with("InstancedStaticMeshComponent")) {
            commands.entity(e).despawn();
            dropped += 1;
        } else if let Ok(c) = children.get(e) {
            stack.extend(c.iter());
        }
    }
    info!("map scene: dropped {dropped} HLOD proxy roots");
}

/// Loads the cells within [`LOAD_RANGE_M`] of the camera and unloads those beyond [`UNLOAD_RANGE_M`] (a
/// top-down, orthographic camera loads them all).
pub(crate) fn stream_cells(mut commands: Commands, assets: Res<AssetServer>, camera: Query<(&GlobalTransform, &Projection), With<PlayerCamera>>, mut cells: Query<(Entity, &mut MapCell)>, mut backlog: ResMut<StreamBacklog>) {
    let Some((eye, projection)) = camera.iter().next() else { return };
    let (eye, all) = (eye.translation(), matches!(projection, Projection::Orthographic(_)));
    let mut ready: Vec<(f32, Entity)> = Vec::new();
    let mut waiting = 0;
    for (e, mut cell) in &mut cells {
        let d = eye.clamp(cell.min, cell.max).distance(eye);
        if cell.instance.is_none() && (all || cell.always || d <= LOAD_RANGE_M) {
            waiting += 1;
            let cell = &mut *cell;
            let scene = cell.pending.get_or_insert_with(|| assets.load(GltfAssetLabel::Scene(0).from_asset(cell.scene.clone()))).id();
            // Spawned once loaded with its textures (a texture that failed to load doesn't hold it back).
            match assets.get_load_states(scene) {
                Some((LoadState::Loaded, _, RecursiveDependencyLoadState::Loaded | RecursiveDependencyLoadState::Failed(_))) => ready.push((d, e)),
                Some((LoadState::Failed(_), ..)) => {
                    warn!("cell scene {} failed to load", cell.scene);
                    cell.instance = Some(Entity::PLACEHOLDER);
                }
                _ => {}
            }
        } else if !all && !cell.always && d > UNLOAD_RANGE_M {
            cell.pending = None;
            if let Some(instance) = cell.instance.take() {
                commands.entity(instance).try_despawn();
            }
        }
    }
    // An overview (orthographic) shot spawns everything at once.
    ready.sort_by(|a, b| a.0.total_cmp(&b.0));
    backlog.0 = waiting;
    let mut spawned = 0;
    for &(_, e) in &ready {
        let Ok((_, mut cell)) = cells.get_mut(e) else { continue };
        if !all && spawned > 0 && spawned + cell.nodes > NODES_PER_FRAME {
            break;
        }
        spawned += cell.nodes;
        backlog.0 -= 1;
        let Some(scene) = cell.pending.take() else { continue };
        let instance = commands.spawn((WorldAssetRoot(scene), Transform::default(), Visibility::default(), ChildOf(e))).id();
        for &(pos, color, intensity, range) in &cell.lights {
            commands.spawn((PointLight { color, intensity: intensity * LUMENS_PER_UNIT, range, shadow_maps_enabled: false, ..default() }, Transform::from_translation(pos), crate::settings::MapLight, ChildOf(instance)));
        }
        cell.instance = Some(instance);
    }
}

/// A map's outdoor light (`<level>-sky.json`): the sky light and the height fog, applied to the camera
/// while the level is loaded ([`apply_map_sky`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct MapSky {
    ambient: Color,
    ambient_brightness: f32,
    fog: Option<(Color, f32)>,
}

/// Bevy units per map unit for the outdoor light (HYPOTHESIS, chosen by eye: the castle's moon is 0.6 and its
/// sky light 0.5, with Unreal's auto exposure behind them).
const LUX_PER_SUN_UNIT: f32 = 1_500.0;
const AMBIENT_PER_SKY_UNIT: f32 = 240.0;
/// Fog distances (m; ours): the map's start distance, then fully opaque (up to its max opacity) this much
/// farther (the height fog's density isn't modelled).
const FOG_DEPTH_M: f32 = 180.0;

/// Spawns a map's directional light and its [`MapSky`] (both level entities); none without `-sky.json`.
fn spawn_sky(commands: &mut Commands, name: &str) -> Vec<Entity> {
    let Some(sky) = data_file(&format!("recovered/maps/{name}-sky.json")).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { return Vec::new() };
    let rgb = |v: &serde_json::Value| v.as_array().map(|c| Color::srgb_u8(c[0].as_u64().unwrap_or(255) as u8, c[1].as_u64().unwrap_or(255) as u8, c[2].as_u64().unwrap_or(255) as u8));
    let mut out = Vec::new();
    if let Some(dir) = vec3(&sky["sun"]["dir"]) {
        let d = to_bevy_dir(dir);
        let illuminance = sky["sun"]["Intensity"].as_f64().map_or(1.0, |v| v as f32) * LUX_PER_SUN_UNIT;
        out.push(commands.spawn((DirectionalLight { color: rgb(&sky["sun"]["color"]).unwrap_or(Color::WHITE), illuminance, shadow_maps_enabled: false, ..default() }, Transform::default().looking_to(d, if d.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y }))).id());
    }
    let fog = sky["fog"]["inscattering"].as_array().map(|c| {
        // Inscattering luminance is linear and dim: shown as its colour brightened to a visible night haze.
        let (r, g, b) = (c[0].as_f64().unwrap_or(0.0) as f32, c[1].as_f64().unwrap_or(0.0) as f32, c[2].as_f64().unwrap_or(0.0) as f32);
        let k = 0.12 / r.max(g).max(b).max(1e-4);
        let alpha = sky["fog"]["FogMaxOpacity"].as_f64().map_or(0.8, |v| v as f32);
        (Color::linear_rgba(r * k, g * k, b * k, alpha), sky["fog"]["StartDistance"].as_f64().map_or(30.0, |v| v as f32 * 0.01))
    });
    out.push(commands.spawn(MapSky { ambient: rgb(&sky["sky"]["color"]).unwrap_or(Color::WHITE), ambient_brightness: sky["sky"]["Intensity"].as_f64().map_or(1.0, |v| v as f32) * AMBIENT_PER_SKY_UNIT, fog }).id());
    out
}

/// Gives the camera the loaded map's sky light and fog, and takes them off when no map with a sky is loaded.
pub(crate) fn apply_map_sky(mut commands: Commands, sky: Query<Ref<MapSky>>, mut removed: RemovedComponents<MapSky>, camera: Query<Entity, With<PlayerCamera>>) {
    let Ok(cam) = camera.single() else { return };
    if let Some(s) = sky.iter().next() {
        if s.is_added() {
            commands.entity(cam).insert(AmbientLight { color: s.ambient, brightness: s.ambient_brightness, ..default() });
            match s.fog {
                Some((color, start)) => {
                    commands.entity(cam).insert(DistanceFog { color, falloff: FogFalloff::Linear { start, end: start + FOG_DEPTH_M }, ..default() });
                }
                None => {
                    commands.entity(cam).remove::<DistanceFog>();
                }
            }
        }
    } else if removed.read().next().is_some() {
        commands.entity(cam).remove::<(AmbientLight, DistanceFog)>();
    }
}

/// A file under `_data/` ([`db_formats::data_root`]), if it is there.
pub(crate) fn data_file(rel: &str) -> Option<std::path::PathBuf> {
    Some(db_formats::data_path(rel)).filter(|p| p.is_file())
}

fn read_json(rel: &str) -> Vec<serde_json::Value> {
    data_file(rel).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn vec3(v: &serde_json::Value) -> Option<Vec3> {
    let a = v.as_array()?;
    Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
}

/// Unreal direction (Z-up) to Bevy.
fn to_bevy_dir(d: Vec3) -> Vec3 {
    Vec3::new(d.x, d.z, d.y).normalize_or_zero()
}

/// Spawns map `level`'s converted scene and its recovered lights, moved by `offset` (sim cm); returns the
/// spawned entities (none if the scene isn't imported) and the number of lights placed.
pub(crate) fn spawn_level(commands: &mut Commands, assets: &AssetServer, level: &str, offset: Vec3) -> (Vec<Entity>, usize) {
    // A full level name or a short key (`01`, `castle1`).
    let name = db_sim::crypt::level_name(level);
    let scene = format!("maps/Dungeonborne/Plugins/GameFeatures/TgfMaps/Content/Scenes/{}/{name}.gltf", db_sim::crypt::level_folder(&name));
    if !crate::model_view::imported(&scene) {
        error!("map {scene} isn't imported (tools/import_assets.py)");
        return (Vec::new(), 0);
    }
    let shift = to_bevy(offset);
    let cells = read_json(&format!("recovered/maps/{name}-cells.json"));
    // The persistent scene streams as parts without its proxy when the index has them; else whole,
    // dropping the proxy once spawned.
    let root = if cells.iter().any(|c| c["always"].as_bool() == Some(true)) {
        commands.spawn((Transform::from_translation(shift), Visibility::default())).id()
    } else {
        commands.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(scene))), Transform::from_translation(shift), Visibility::default())).observe(drop_proxy).id()
    };
    // Its landscape gets the blend material.
    commands.entity(root).insert(crate::landscape::LandscapeLevel(name.clone()));
    let mut out = vec![root];
    // The full-detail cells, children of the floor's scene so they share its offset and visibility.
    for c in &cells {
        let (Some(scene), Some(min), Some(max)) = (c["scene"].as_str(), vec3(&c["min"]), vec3(&c["max"])) else { continue };
        if !crate::model_view::imported(scene) {
            continue;
        }
        let lights = c["lights"].as_array().map_or(Vec::new(), |ls| {
            ls.iter()
                .filter_map(|l| {
                    // HLOD copies may lack a value (class default): white, intensity 1, the default radius.
                    let rgb = l["color"].as_array();
                    let channel = |i: usize| rgb.and_then(|c| c.get(i)).and_then(serde_json::Value::as_u64).unwrap_or(255) as u8;
                    let range = l["AttenuationRadius"].as_f64().map(|r| r as f32 * 0.01).filter(|r| *r > 0.0).unwrap_or(DEFAULT_RANGE_M);
                    Some((vec3(&l["pos"])?, Color::srgb_u8(channel(0), channel(1), channel(2)), l["Intensity"].as_f64().map_or(1.0, |v| v as f32), range))
                })
                .collect()
        });
        commands.spawn((Transform::default(), Visibility::default(), MapCell { scene: scene.to_string(), nodes: c["nodes"].as_u64().unwrap_or(1_000) as usize, always: c["always"].as_bool() == Some(true), min: min + shift, max: max + shift, instance: None, pending: None, lights }, ChildOf(root)));
    }
    info!("map {name}: {} streaming cells", cells.len());
    out.extend(spawn_sky(commands, &name));
    let lights = read_json(&format!("recovered/maps/{name}-lights.json"));
    let mut placed = 0;
    // Brightest first (the quality cap now picks the nearest). The HLOD actor's
    // lights come with the cells (`-cells.json` has them).
    let mut lights: Vec<&serde_json::Value> = lights.iter().filter(|l| l["Intensity"].as_f64().is_none_or(|v| v > 0.0) && l["actor"] != "WorldPartitionHLOD").collect();
    lights.sort_by(|a, b| b["Intensity"].as_f64().unwrap_or(1.0).total_cmp(&a["Intensity"].as_f64().unwrap_or(1.0)));
    let total = lights.len();
    for l in lights {
        let Some(pos) = vec3(&l["pos"]) else { continue };
        let intensity = l["Intensity"].as_f64().map_or(1.0, |v| v as f32);
        let color = l["color"].as_array().map_or(Color::WHITE, |c| Color::srgb_u8(c[0].as_u64().unwrap_or(255) as u8, c[1].as_u64().unwrap_or(255) as u8, c[2].as_u64().unwrap_or(255) as u8));
        let range = l["AttenuationRadius"].as_f64().map(|r| r as f32 * 0.01).filter(|r| *r > 0.0).unwrap_or(DEFAULT_RANGE_M);
        let at = Transform::from_translation(to_bevy(pos) + shift);
        let mut e = if l["kind"] == "spot" {
            let dir = vec3(&l["dir"]).map_or(Vec3::NEG_Y, to_bevy_dir);
            let outer = l["OuterConeAngle"].as_f64().map_or(44.0, |v| v as f32).to_radians();
            let inner = l["InnerConeAngle"].as_f64().map_or(0.0, |v| v as f32).to_radians().min(outer);
            commands.spawn((SpotLight { color, intensity: intensity * LUMENS_PER_UNIT, range, outer_angle: outer, inner_angle: inner, shadow_maps_enabled: false, ..default() }, at.looking_to(dir, if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y })))
        } else {
            commands.spawn((PointLight { color, intensity: intensity * LUMENS_PER_UNIT, range, shadow_maps_enabled: false, ..default() }, at))
        };
        let id = e.insert(crate::settings::MapLight).id();
        out.push(id);
        placed += 1;
    }
    info!("map {name}: {placed} of {total} lights");
    (out, placed)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_map(mut commands: Commands, mut queue: ResMut<db_sim::CommandQueue>, assets: Res<AssetServer>, viewer: Res<MapViewer>, mut look: ResMut<Look>, mut cam: Query<(Entity, &mut Transform), With<PlayerCamera>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let Some(level) = viewer.level.as_deref() else { return };
    let name = db_sim::crypt::level_name(level);
    let (spawned, _) = spawn_level(&mut commands, &assets, level, Vec3::ZERO);
    if spawned.is_empty() {
        return;
    }
    let starts = read_json(&format!("recovered/maps/{name}-starts.json"));
    info!("map {name}: {} player starts", starts.len());
    let Ok((cam, mut t)) = cam.single_mut() else { return };
    let start = starts.get(viewer.start.min(starts.len().saturating_sub(1)));
    let start_pos = start.and_then(|s| vec3(&s["pos"])).unwrap_or(Vec3::ZERO);
    if viewer.top {
        // Placement markers (`-placement.json`), unlit spheres seen from above: player starts green (waiting
        // points dark green), monster points red, chests yellow, altars violet, escape points cyan, their
        // portals blue, doors orange, traps white, pickups pink, patrol points and table-less spawn points
        // small grey.
        let marker = meshes.add(Sphere::new(1.0));
        let placement = read_json(&format!("recovered/maps/{name}-placement.json"));
        let mut glows: std::collections::HashMap<&str, Handle<StandardMaterial>> = std::collections::HashMap::new();
        for e in &placement {
            let Some(p) = vec3(&e["pos"]) else { continue };
            let cat = e["cat"].as_str().unwrap_or("");
            let waiting = e["waiting"].as_bool().unwrap_or(false);
            let (rgb, r) = match cat {
                "player_start" if waiting => ([0.05, 0.4, 0.1], 0.8),
                "player_start" => ([0.2, 1.0, 0.3], 1.0),
                "monster" => ([1.0, 0.15, 0.1], 1.2),
                "chest" => ([1.0, 0.85, 0.1], 1.0),
                "altar" => ([0.75, 0.3, 1.0], 1.3),
                "escape_point" | "extract" => ([0.1, 1.0, 1.0], 1.6),
                "arrival" => ([0.6, 1.0, 0.2], 0.8),
                "destroyable" => ([0.55, 0.35, 0.15], 0.5),
                "escape_portal" => ([0.2, 0.4, 1.0], 1.0),
                "escape_door" | "door" => ([1.0, 0.5, 0.1], 0.9),
                "trap" => ([1.0, 1.0, 1.0], 1.2),
                "pickup" => ([1.0, 0.5, 0.8], 0.8),
                "patrol_point" => ([0.6, 0.6, 0.6], 0.4),
                "spawn_unset" => ([0.35, 0.35, 0.35], 0.5),
                _ => continue,
            };
            let key = if waiting { "waiting" } else { cat };
            let mat = glows.entry(key).or_insert_with(|| materials.add(StandardMaterial { base_color: Color::srgb(rgb[0], rgb[1], rgb[2]), unlit: true, ..default() })).clone();
            commands.spawn((Mesh3d(marker.clone()), MeshMaterial3d(mat), Transform::from_translation(to_bevy(p)).with_scale(Vec3::splat(r))));
        }
        // Over the scene's mesh placements (1st-99th percentile, `-info.json`), just under the ceiling.
        let info = data_file(&format!("recovered/maps/{name}-info.json")).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).unwrap_or_default();
        let corner = |k: &str| Vec2::new(info[k][0].as_f64().unwrap_or(0.0) as f32, info[k][1].as_f64().unwrap_or(0.0) as f32);
        let (min, max) = (corner("min"), corner("max"));
        let size = (max - min) * 0.01;
        let centre = to_bevy(((min + max) * 0.5).extend(start_pos.z + 350.0));
        commands.entity(cam).insert(Projection::from(OrthographicProjection { scaling_mode: ScalingMode::AutoMin { min_width: size.x * 1.04, min_height: size.y * 1.04 }, near: 0.0, far: 200.0, ..OrthographicProjection::default_3d() }));
        *t = Transform::from_translation(centre).looking_to(Vec3::NEG_Y, Vec3::Z);
        return;
    }
    t.translation = to_bevy(start_pos + Vec3::Z * crate::EYE_OFFSET);
    look.yaw = start.and_then(|s| s["yaw"].as_f64()).map_or(0.0, |y| (y as f32).to_radians());
    look.pitch = -0.05;
    if viewer.walk {
        match db_sim::map::MapCollision::load_floor(level) {
            Ok(map) => {
                info!("map {name}: collision {} meshes, {} instances", map.meshes.len(), map.instances.len());
                queue.push(db_sim::SimCommand::AddMapCollision(std::sync::Arc::new(map)));
                queue.push(db_sim::SimCommand::Spawn { kind: db_sim::ActorKind::Player, pos: start_pos, speed: 320.0 });
            }
            Err(e) => error!("map {name}: {e:#}"),
        }
    }
}

/// `--map-walk`: the player keeps walking where the camera looks.
pub(crate) fn walk_ahead(viewer: Res<MapViewer>, look: Res<Look>, mut bot: ResMut<crate::autorun::BotMove>) {
    if viewer.walk {
        bot.0 = Some(Vec2::new(look.yaw.cos(), look.yaw.sin()));
    }
}

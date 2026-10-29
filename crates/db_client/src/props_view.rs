//! Real prop meshes: chests, altars, traps, doors and weapons on the ground. Each blueprint is rebuilt
//! from the generated table [`PROPS`]: its components with their original local transforms and parents,
//! so moving parts (a chest's `Lid`, a trap's `MoveNode`, a door's `Hinge`) stay addressable. Ground
//! weapons use [`crate::weapon_meshes::WEAPON_MESHES`]. Where a mesh isn't imported the greybox stays.

use bevy::prelude::*;
use db_sim::{Container, GameData, GroundItem, SimIndex};

use crate::items_view::{ChestLid, LootBody};
use crate::model_view::imported;
use crate::prop_meshes::{PROPS, PropNode};

/// The nodes of blueprint `class` (`BP_X` or `BP_X_C`), if all its meshes are imported.
pub(crate) fn prop(class: &str) -> Option<&'static [PropNode]> {
    let class = class.strip_suffix("_C").unwrap_or(class);
    PROPS.iter().find(|p| p.0 == class).map(|p| p.1).filter(|ns| ns.iter().any(|n| n.asset.is_some()) && ns.iter().filter_map(|n| n.asset).all(imported))
}

/// Index of the node named `name`.
pub(crate) fn node(nodes: &[PropNode], name: &str) -> Option<usize> {
    nodes.iter().position(|n| n.name == name)
}

/// A node's local transform.
pub(crate) fn local(n: &PropNode) -> Transform {
    Transform { translation: Vec3::from(n.t), rotation: Quat::from_array(n.r).normalize(), scale: Vec3::from(n.s) }
}

/// A node's transform relative to the blueprint root (its parents' transforms applied).
pub(crate) fn in_root(nodes: &[PropNode], i: usize) -> Transform {
    let mut t = local(&nodes[i]);
    let mut p = nodes[i].parent;
    while let Some(j) = p {
        t = local(&nodes[j as usize]).mul_transform(t);
        p = nodes[j as usize].parent;
    }
    t
}

/// Bounds (blueprint space) of the mesh on node `i`.
pub(crate) fn mesh_bounds(nodes: &[PropNode], i: usize) -> (Vec3, Vec3) {
    let t = in_root(nodes, i);
    let (lo, hi) = (Vec3::from(nodes[i].min), Vec3::from(nodes[i].max));
    let corners = (0..8).map(|k| t.transform_point(Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z })));
    corners.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), c| (a.min(c), b.max(c)))
}

/// Spawns a blueprint's nodes under `parent` at `at`; returns the entity of each node (by index).
pub(crate) fn spawn_prop(commands: &mut Commands, assets: &AssetServer, nodes: &[PropNode], parent: Entity, at: Transform) -> Vec<Entity> {
    let holder = commands.spawn((at, Visibility::default(), ChildOf(parent))).id();
    let mut ents: Vec<Entity> = Vec::with_capacity(nodes.len());
    for n in nodes {
        let up = n.parent.map_or(holder, |p| ents[p as usize]);
        let mut e = commands.spawn((local(n), Visibility::default(), ChildOf(up)));
        if let Some(path) = n.asset {
            e.insert(WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))));
        }
        ents.push(e.id());
    }
    ents
}

/// Chests and ground items already given their real meshes.
#[derive(Component)]
pub(crate) struct Dressed;

/// `ID_TeleportScroll`'s pickup mesh.
const SCROLL_MESH: &str = "models/Dungeonborne/Content/TGF/Equipment/Item/TeleportScroll/CSM_TeleportScroll/SM_TeleportScroll.glb";

/// Chest blueprint used when a chest's loot source names a class without imported meshes (HYPOTHESIS).
const DEFAULT_CHEST: &str = "BP_TreasureChest_Wood_02";

/// Swaps the greybox chest and ground-item bodies for real meshes: a chest's class comes from its loot
/// source (`ExcelLootSourceMap` names the `BP_*_C` class); a ground weapon shows its weapon mesh.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn dress_loot(mut commands: Commands, assets: Res<AssetServer>, data: Option<Res<GameData>>, index: Res<SimIndex>, bodies: Query<(Entity, &LootBody), Without<Dressed>>, containers: Query<&Container>, ground: Query<&GroundItem>, positions: Query<&db_sim::Position>) {
    let Some(data) = data else { return };
    for (e, body) in &bodies {
        let Some(sim) = index.get(body.0) else { continue };
        commands.entity(e).insert(Dressed);
        if let Ok(c) = containers.get(sim) {
            // Source 0 is the empty Goblin Thief chest, the one chest class in no loot table.
            let class = data.tables.excel_loot_source_map.iter().find(|r| r.id == c.source_id).map(|r| r.name.as_str()).or((c.source_id == 0).then_some("BP_GoblinThief_TreasureChest_Wood_02_C"));
            let Some(nodes) = class.and_then(prop).or_else(|| prop(DEFAULT_CHEST)) else { continue };
            // The sim chest's position is its collider centre; the blueprint's origin is on the floor.
            let half = Vec3::from(db_sim::overrides::world_items::CHEST_HALF_EXTENTS);
            commands.entity(e).despawn_related::<Children>();
            let ents = spawn_prop(&mut commands, &assets, nodes, e, Transform::from_xyz(0.0, -half.z * 0.01, 0.0));
            if let Some(lid) = node(nodes, "Lid") {
                commands.entity(ents[lid]).insert(ChestLid(body.0));
            }
            continue;
        }
        if let Ok(g) = ground.get(sim) {
            // The death zone's Ritual Scroll by its `InventoryFragment_PickupMesh`.
            let scroll = (g.0.item.item_id == db_sim::overrides::run::TELEPORT_SCROLL).then_some(SCROLL_MESH);
            if scroll.is_some()
                && let Some(p) = positions.get(sim).ok()
            {
                info!("ritual scroll {:?} at {:.0}", body.0, p.0);
            }
            let Some(path) = crate::weapon_meshes::WEAPON_MESHES.iter().find(|w| w.0 == g.0.item.item_id).map(|w| w.1).or(scroll).filter(|p| imported(p)) else { continue };
            // The weapon meshes lie flat along +X; turned to lie along the world's -Y (sim), across the arena's
            // weapon tables (which run along X), so pieces 80 cm apart don't overlap (a staff is 1.8 m long).
            commands.entity(e).remove::<(Mesh3d, MeshMaterial3d<StandardMaterial>)>().with_children(|c| {
                c.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))), Transform::from_xyz(0.0, -0.04, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))));
            });
        }
    }
}

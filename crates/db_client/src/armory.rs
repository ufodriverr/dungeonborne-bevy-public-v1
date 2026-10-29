//! The test arena's weapon racks: every regular weapon (`181xxxxxx`: swords, long swords, shields, crossbows, staffs,
//! relics, daggers, maces) lies on a table, a family to a table or two, on both sides of the lane from the spawn to
//! the monsters, with aisles to reach both edges. A taken piece comes back, freshly rolled, a few seconds later. One
//! more table holds the alchemical flasks (each kind three times; keys 3 to 5 throw them). The ice staffs
//! (`181052002`, `181053002`, `181054002`, `181055002`) are left off: their `WAID_Staff_Ice` grants no spell and no
//! loot, drop, market or trade table lists them (CONFIRMED: cut from the original), so on a table they only looked
//! broken.

use bevy::prelude::*;
use db_sim::world_items::GroundItem;
use db_sim::{CommandQueue, Position, SimCommand};

use crate::to_bevy;

/// Pieces per table, and their spacing along it and between tables (cm: a 70 cm table and a 1.5 m aisle).
const PER_ROW: usize = 10;
const ALONG: f32 = 80.0;
const ACROSS: f32 = 220.0;
/// Table depth and height (cm).
const DEPTH: f32 = 70.0;
const HEIGHT: f32 = 75.0;
/// The tables' first piece: 6 m left and right of the spawn's lane, from 4.5 m ahead of the chests and altars.
const NEAR_X: f32 = 600.0;
const FIRST_Y: f32 = -2250.0;
/// How often an empty spot is filled again (s), and how near a piece must lie to fill it (cm).
const RESTOCK_EVERY: f32 = 3.0;
const ON_SPOT: f32 = 40.0;

/// The weapon families' item-id codes (`181FFTVVV`), in rug order.
const FAMILIES: [u32; 8] = [5, 8, 1, 2, 3, 11, 12, 4];

/// The racks' spots: floor point and item.
#[derive(Resource, Default)]
pub(crate) struct Armory {
    spots: Vec<(Vec3, i32)>,
    wait: f32,
}

/// The regular weapons by family, in rug rows: ids `181FFxxxx` with a wear slot, but the cut ice staffs.
fn rows(tables: &db_formats::config::ConfigTables, data: &db_sim::GameData, staffs: Option<&db_sim::staffs::StaffDb>) -> Vec<Vec<i32>> {
    let ice = |id: i32| staffs.and_then(|s| s.0.get(&id)).is_some_and(|s| s.element == db_sim::staffs::StaffElement::Ice);
    let mut rows = Vec::new();
    for family in FAMILIES {
        let mut ids: Vec<i32> = tables
            .excel_item_type
            .iter()
            .map(|t| t.type_id)
            .filter(|id| *id / 1_000_000 == 181 && (*id / 10_000 % 100) as u32 == family && data.info(*id).is_some_and(|i| !i.slots.is_empty()) && !ice(*id))
            .collect();
        ids.sort();
        ids.dedup();
        rows.extend(ids.chunks(PER_ROW).map(<[i32]>::to_vec));
    }
    rows
}

/// Lays the racks out once the arena and the game data are up, then fills empty spots every few seconds.
#[allow(clippy::too_many_arguments)]
pub(crate) fn armory(
    mut commands: Commands,
    arena: Res<crate::Arena>,
    game: Option<Res<db_sim::GameData>>,
    mut armory: ResMut<Armory>,
    ground: Query<(&Position, &GroundItem)>,
    time: Res<Time>,
    mut queue: ResMut<CommandQueue>,
    (mut meshes, mut materials, assets): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>, Res<AssetServer>),
    (flasks, staffs): (Option<Res<db_sim::throwables::FlaskDb>>, Option<Res<db_sim::staffs::StaffDb>>),
) {
    let Some(game) = game.filter(|_| arena.0) else { return };
    if armory.spots.is_empty() {
        let mut rows = rows(game.tables, &game, staffs.as_deref());
        // The flasks (not their "Competitive" twins, `_0102`), each kind three times.
        let kinds: Vec<i32> = flasks.iter().flat_map(|f| f.0.iter().filter(|(_, s)| s.name.ends_with("_0101")).map(|(id, _)| *id)).collect();
        if !kinds.is_empty() {
            rows.push(kinds.iter().copied().cycle().take(kinds.len() * 3).collect());
        }
        let half = rows.len().div_ceil(2);
        let wood = materials.add(crate::surface_material(db_sim::level::Surface::Wood, Vec2::ONE, &assets));
        for (r, row) in rows.iter().enumerate() {
            // The first half on the lane's left (+X), the rest on its right (-X).
            let (side, r) = if r < half { (1.0, r) } else { (-1.0, r - half) };
            let y = FIRST_Y + r as f32 * ACROSS;
            for (c, id) in row.iter().enumerate() {
                armory.spots.push((Vec3::new(side * (NEAR_X + c as f32 * ALONG), y, HEIGHT), *id));
            }
            // The table: a sim box (walked around) and its mesh.
            let len = PER_ROW as f32 * ALONG;
            let (centre, half) = (Vec3::new(side * (NEAR_X + (len - ALONG) * 0.5), y, HEIGHT * 0.5), Vec3::new(len * 0.5, DEPTH * 0.5, HEIGHT * 0.5));
            queue.push(SimCommand::AddStaticBox { center: centre, half_extents: half });
            commands.spawn((Mesh3d(meshes.add(Cuboid::new(len / 100.0, HEIGHT / 100.0, DEPTH / 100.0))), MeshMaterial3d(wood.clone()), Transform::from_translation(to_bevy(centre)), crate::flow::LevelScoped));
        }
        info!("armory: {} weapons on {} tables", armory.spots.len(), rows.len());
        armory.wait = 0.0;
    }
    armory.wait -= time.delta_secs();
    if armory.wait > 0.0 {
        return;
    }
    armory.wait = RESTOCK_EVERY;
    for &(pos, item_id) in &armory.spots {
        let there = ground.iter().any(|(p, g)| g.0.item.item_id == item_id && (p.0 - pos).truncate().length() < ON_SPOT);
        if !there {
            queue.push(SimCommand::PlaceItem { pos, item_id });
        }
    }
}

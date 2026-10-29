//! Items in the world: item names, rarity colours and icons; chest and ground-item bodies; the
//! interaction prompt (F) and potion hotkeys (1 healing, 2 protection). Reads sim state only.

use bevy::prelude::*;
use bevy::text::FontSize;
use db_formats::config::generated::{EnGearEquipSubType, EnItemRarity};
use db_sim::level::Surface;
use db_sim::{ActorKind, CommandQueue, Container, GameData, GroundItem, Inventory, Kind, Position, SimCommand, SimId, SimIndex};

use crate::combat_view::{LocalPlayer, WorldLabel, spawn_label};
use crate::{Look, surface_material, to_bevy};

/// English names for the consumables and materials the greybox uses (the string tables are Chinese;
/// translations ours, from `Item_Name_<id>`; the Ritual Scroll's is the English `DisplayName` of `ID_TeleportScroll`;
/// the flasks' from their icons, `Icon_IncendiaryFlask` and so on; the legendary weapons' so the tables tell them apart).
const NAMES: &[(i32, &str)] = &[
    (100_010, "Torch"),
    (100_012, "Torch"),
    (100_015, "Old Torch"),
    (100_047, "Incendiary Flask"),
    (100_048, "Thunderclap Flask"),
    (100_049, "Stoneshaping Flask"),
    (100_050, "Smokescreen Flask"),
    (100_055, "Competitive Incendiary Flask"),
    (100_056, "Competitive Thunderclap Flask"),
    (100_057, "Competitive Stoneshaping Flask"),
    (100_058, "Competitive Smokescreen Flask"),
    (100_070, "Elixir of Ascension"),
    (100_106, "Superior Brutality Draught"),
    (100_107, "Superior Protection Draught"),
    (100_108, "Superior Regeneration Draught"),
    (1_037, "Ilmenite Ore"),
    (1_038, "Fine Steel Ore"),
    (1_039, "Starfall Ember"),
    (1_052, "Rough Iron"),
    (1_053, "Mithril Token"),
    (2_100_000, "Resonance Stone (Hard)"),
    (2_100_018, "Resonance Stone (Classic)"),
    // The legendary and unique weapons (`Item_Name_181xx5xxx` / `6xxx`).
    (181_015_000, "White Star Knight Sword"),
    (181_015_001, "High Elf Guard Sword"),
    (181_016_000, "Gift of the Treasure Tree"),
    (181_016_001, "Fang of the Earth"),
    (181_025_000, "Thunder Lord Sword"),
    (181_025_001, "The Seventeenth Key"),
    (181_026_000, "Lion King Sword"),
    (181_026_001, "Soul-Eater Greatsword"),
    (181_035_000, "Cloud-Pattern Shield"),
    (181_035_001, "Silver Shield"),
    (181_036_000, "Dragon Scale Shield"),
    (181_036_001, "Dark Elf Shield"),
    (181_045_000, "Goose Feather Crossbow"),
    (181_046_000, "Scorpion King Crossbow"),
    (181_055_001, "Endo's Fire Staff"),
    (181_055_002, "Endo's Frost Staff"),
    (181_055_003, "Thunder Rod"),
    (181_056_001, "Undying Flame Staff"),
    (181_056_003, "Undying Thunder Staff"),
    (181_085_000, "Endo's Orb"),
    (181_086_000, "Skull of the King of Men"),
    (181_115_000, "Lightning Dagger"),
    (181_115_001, "Thief's Shortsword"),
    (181_116_000, "Sword of the Lake"),
    (181_116_001, "Wizard's Shortsword"),
    (181_125_000, "Holy Light Mace"),
    (181_125_001, "Meteor Hammer"),
    (181_126_000, "Fairy's Blessing"),
    (181_126_001, "Beast King Hammer"),
    (100_013, "Healing Potion"),
    (100_016, "Bandage"),
    (100_017, "Protection Potion"),
    (100_018, "Mana Potion"),
    (100_019, "Mimic Potion"),
    (100_021, "Ritual Scroll"),
    (100_045, "Arrows"),
    (100_060, "Murky Life Essence"),
];

fn rarity_word(r: EnItemRarity) -> &'static str {
    match r {
        EnItemRarity::Rotted => "Rotted",
        EnItemRarity::Normal => "Normal",
        EnItemRarity::Uncommon => "Uncommon",
        EnItemRarity::Enhanced => "Enhanced",
        EnItemRarity::Epic => "Epic",
        EnItemRarity::Legendary => "Legendary",
        EnItemRarity::Unique => "Unique",
        EnItemRarity::Exceptional => "Exceptional",
        _ => "",
    }
}

pub fn rarity_color(r: EnItemRarity) -> Color {
    match r {
        EnItemRarity::Rotted => Color::srgb(0.45, 0.42, 0.38),
        EnItemRarity::Normal => Color::srgb(0.85, 0.85, 0.85),
        EnItemRarity::Uncommon => Color::srgb(0.35, 0.8, 0.3),
        EnItemRarity::Enhanced => Color::srgb(0.3, 0.55, 1.0),
        EnItemRarity::Epic => Color::srgb(0.7, 0.35, 0.95),
        EnItemRarity::Legendary => Color::srgb(1.0, 0.6, 0.15),
        EnItemRarity::Unique => Color::srgb(0.95, 0.85, 0.45),
        EnItemRarity::Exceptional => Color::srgb(1.0, 0.3, 0.3),
        _ => Color::WHITE,
    }
}

/// "Rotted Sword", "Healing Potion", …: rarity plus equipment sub-type for gear (our English).
pub fn item_name(data: &GameData, item_id: i32) -> String {
    if let Some((_, n)) = NAMES.iter().find(|(id, _)| *id == item_id) {
        return (*n).to_string();
    }
    let t = data.tables;
    let ui = t.uesource_item_type.iter().find(|u| u.type_id == item_id);
    let sub = ui.and_then(|u| t.ue_source_equipment.iter().find(|e| e.equip_id == u.ue_source_equipment_equip_id)).map(|e| e.equip_subtype);
    match (ui, sub) {
        (Some(u), Some(s)) => {
            let raw = s.name().unwrap_or("").trim_start_matches("EN_GEAR_EQUIP_SUBTYPE_").to_lowercase().replace('_', " ");
            let title = |w: &str| {
                let mut c = w.chars();
                c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect::<String>())
            };
            let noun = raw.split(' ').map(title).collect::<Vec<_>>().join(" ");
            let noun = if s == EnGearEquipSubType::default() || noun.is_empty() { "Item".to_string() } else { noun };
            format!("{} {noun}", rarity_word(u.rarity)).trim().to_string()
        }
        // No equipment sub-type (gems, crafting materials, …): the item's minor type from `ExcelItemType`.
        (u, None) => match t.excel_item_type.iter().find(|x| x.type_id == item_id).and_then(|x| x.item_minor_type.name()) {
            Some(minor) => {
                let raw = minor.trim_start_matches("EN_ITEM_MINOR_TYPE_").replace("_2_", "_").to_lowercase();
                let title = |w: &str| {
                    let mut c = w.chars();
                    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect::<String>())
                };
                let noun = match raw.strip_prefix("gem_") {
                    Some(kind) => format!("{} Gem", kind.split('_').map(title).collect::<Vec<_>>().join(" ")),
                    None => raw.split('_').filter(|w| !matches!(*w, "equipment" | "consumables")).map(title).collect::<Vec<_>>().join(" "),
                };
                format!("{} {noun}", u.map_or("", |u| rarity_word(u.rarity))).trim().to_string()
            }
            None => format!("Item {item_id}"),
        },
        (None, Some(_)) => format!("Item {item_id}"),
    }
}

/// The decoded icon PNG for an item (`textures_256/...`), if there is one.
pub fn icon_path(data: &GameData, item_id: i32) -> Option<String> {
    let u = data.tables.uesource_item_type.iter().find(|u| u.type_id == item_id)?;
    let res = &u.brush_data.as_ref()?.resource_name;
    let path = res.split('.').next()?.strip_prefix("/Game/")?;
    Some(format!("textures_256/Dungeonborne/Content/{path}.png"))
}

/// Chest lid (opens with the container).
#[derive(Component)]
pub(crate) struct ChestLid(pub SimId);

/// Body of a chest or ground item following its sim actor.
#[derive(Component)]
pub(crate) struct LootBody(pub SimId);

/// The chest or item the player would interact with now.
#[derive(Resource, Debug, Default)]
pub struct InteractTarget(pub Option<SimId>);

#[derive(Component)]
pub(crate) struct InteractPrompt;

/// Reach and aim cone for interaction (the sim checks reach again; HYPOTHESIS values, as in the sim).
const REACH_CM: f32 = 200.0;
/// Ground items' names show within this distance (cm): a little beyond reach.
const ITEM_LABEL_RANGE: f32 = 400.0;
const AIM_COS: f32 = 0.64;

pub(crate) fn setup_prompt(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(22.0), ..default() },
        TextColor(Color::srgb(0.95, 0.9, 0.7)),
        TextShadow::default(),
        Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(56.0), margin: UiRect::left(px(-300.0)), width: px(600.0), ..default() },
        TextLayout::justify(Justify::Center),
        InteractPrompt,
        crate::flow::GameHud,
    ));
}

/// Bodies for chests (box and lid) and ground items (a small rarity-coloured block plus a label).
pub(crate) fn spawn_loot_body(commands: &mut Commands, id: SimId, kind: ActorKind, item: Option<(String, Color)>, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, assets: &AssetServer) {
    if kind == ActorKind::Chest {
        let half = Vec3::from(db_sim::overrides::world_items::CHEST_HALF_EXTENTS);
        let size = to_bevy(half * 2.0).abs();
        let wood = materials.add(surface_material(Surface::Wood, Vec2::ONE, assets));
        let metal = materials.add(StandardMaterial { metallic: 0.5, perceptual_roughness: 0.5, ..surface_material(Surface::Metal, Vec2::ONE, assets) });
        commands.spawn((LootBody(id), Transform::default(), Visibility::default())).with_children(|c| {
            c.spawn((Mesh3d(meshes.add(Cuboid::new(size.x, size.y * 0.75, size.z))), MeshMaterial3d(wood.clone()), Transform::from_xyz(0.0, -size.y * 0.125, 0.0)));
            // Lid hinged at the back edge (-Z local = sim -Y, the wall side in the test room); the latch is at the front.
            c.spawn((ChestLid(id), Transform::from_xyz(0.0, size.y * 0.25, -size.z * 0.5), Visibility::default())).with_children(|l| {
                l.spawn((Mesh3d(meshes.add(Cuboid::new(size.x, size.y * 0.25, size.z))), MeshMaterial3d(wood), Transform::from_xyz(0.0, size.y * 0.125, size.z * 0.5)));
                l.spawn((Mesh3d(meshes.add(Cuboid::new(size.x * 0.12, size.y * 0.2, 0.02))), MeshMaterial3d(metal), Transform::from_xyz(0.0, size.y * 0.1, size.z + 0.01)));
            });
        });
        return;
    }
    let (name, color) = item.unwrap_or(("Item".into(), Color::WHITE));
    let mat = materials.add(StandardMaterial { base_color: color, emissive: LinearRgba::from(color) * 0.4, ..default() });
    commands.spawn((LootBody(id), Transform::default(), Visibility::default(), Mesh3d(meshes.add(Cuboid::new(0.18, 0.08, 0.28))), MeshMaterial3d(mat)));
    spawn_label(commands, name, color, WorldLabel::follow(id, 25.0, 16.0).within(ITEM_LABEL_RANGE));
}

pub(crate) fn sync_loot(mut commands: Commands, index: Res<SimIndex>, actors: Query<(&Position, Option<&Container>)>, mut bodies: Query<(Entity, &LootBody, &mut Transform), Without<ChestLid>>, mut lids: Query<(&ChestLid, &mut Transform), Without<LootBody>>) {
    for (e, body, mut t) in &mut bodies {
        match index.get(body.0).and_then(|s| actors.get(s).ok()) {
            Some((p, _)) => t.translation = to_bevy(p.0),
            None => commands.entity(e).despawn(),
        }
    }
    for (lid, mut t) in &mut lids {
        let open = index.get(lid.0).and_then(|s| actors.get(s).ok()).and_then(|(_, c)| c).is_some_and(|c| c.opened);
        t.rotation = Quat::from_rotation_x(if open { -1.9 } else { 0.0 });
    }
}

/// Picks the interaction target (nearest by aim among chests not yet opened, ground items, portals,
/// doors and unspent altars in reach) and shows the prompt.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn find_interact_target(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    look: Res<Look>,
    data: Option<Res<GameData>>,
    actors: Query<(&Kind, &Position, Option<&Container>, Option<&GroundItem>, Option<&db_sim::Portal>)>,
    features: Query<(Option<&db_sim::Door>, Option<&db_sim::Altar>, Option<&db_sim::ai::MonsterAi>, Option<&db_sim::swing_doors::SwingDoor>, Option<&db_sim::mechanisms::Lever>)>,
    holds: Query<&db_sim::swing_doors::Interacting>,
    tick: Res<db_sim::TickCount>,
    mut target: ResMut<InteractTarget>,
    mut prompt: Query<&mut Text, With<InteractPrompt>>,
) {
    target.0 = None;
    let me = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()).map(|a| a.1.0);
    let mut best: Option<(f32, SimId, String)> = None;
    if let Some(me) = me {
        let fwd = Vec2::from_angle(look.yaw);
        for (id, e) in index.iter() {
            let Ok((kind, pos, container, ground, portal)) = actors.get(e) else { continue };
            let label = match (kind.0, container, ground) {
                (ActorKind::Portal, _, _) => match portal.map(|p| (p.kind, p.hold > 0.0)) {
                    Some((db_sim::dungeon::PortalKind::Down, _)) => "Go down to floor 2".to_string(),
                    // The death zone's extraction devices are held.
                    Some((_, true)) => "Use extraction device".to_string(),
                    _ => "Extract".to_string(),
                },
                (ActorKind::Chest, Some(c), _) if !c.opened => "Open chest".to_string(),
                // A mimic still posing as a chest.
                (ActorKind::Monster, _, _) if features.get(e).ok().and_then(|f| f.2).is_some_and(|a| a.wake_on_open && a.dormant) => "Open chest".to_string(),
                (ActorKind::Door, _, _) => {
                    let f = features.get(e).ok();
                    if f.and_then(|f| f.0).is_some_and(|d| d.open) || f.and_then(|f| f.3).is_some_and(|d| d.open) { "Close door".to_string() } else { "Open door".to_string() }
                }
                (ActorKind::Lever, _, _) => "Pull lever".to_string(),
                (ActorKind::Altar, _, _) => match features.get(e).ok().and_then(|f| f.1) {
                    Some(a) if !a.used => format!("Pray: {}", a.kind.name()),
                    _ => continue,
                },
                (ActorKind::Item, _, Some(g)) => {
                    let name = data.as_ref().map_or_else(|| format!("item {}", g.0.item.item_id), |d| item_name(d, g.0.item.item_id));
                    if g.0.count > 1 { format!("Pick up {name} x{}", g.0.count) } else { format!("Pick up {name}") }
                }
                _ => continue,
            };
            // A swing door is aimed at by its nearest leaf.
            let at = features.get(e).ok().and_then(|f| f.3).and_then(|d| d.spec.leaves.iter().map(|l| d.leaf_box(pos.0, l).0).min_by(|a, b| a.distance(me).total_cmp(&b.distance(me)))).unwrap_or(pos.0);
            // A lever by its crank.
            let at = features.get(e).ok().and_then(|f| f.4).map_or(at, |l| l.handle());
            let d = at - me;
            let aim = fwd.dot(d.truncate().normalize_or_zero());
            // A swing door's own reach (`BaseInteractDistance`).
            let reach = features.get(e).ok().and_then(|f| f.3).map_or(REACH_CM, |d| d.spec.reach);
            let reach = features.get(e).ok().and_then(|f| f.4).map_or(reach, |l| l.placement.reach);
            if d.length() <= reach && aim >= AIM_COS && best.as_ref().is_none_or(|b| aim > b.0) {
                best = Some((aim, id, label));
            }
        }
    }
    // A held interaction: its progress while F is held.
    let hold = player.0.and_then(|id| index.get(id)).and_then(|e| holds.get(e).ok());
    if let Ok(mut t) = prompt.single_mut() {
        t.0 = match (hold, best.as_ref()) {
            (Some(h), Some(b)) if b.1 == h.target => {
                let k = (tick.0.saturating_sub(h.start)) as f32 / (h.done_at.saturating_sub(h.start)).max(1) as f32;
                let filled = (k.clamp(0.0, 1.0) * 10.0).round() as usize;
                format!("[F] {}  {}{}", b.2, "#".repeat(filled), "-".repeat(10 - filled))
            }
            _ => best.as_ref().map_or(String::new(), |b| format!("[F] {}", b.2)),
        };
    }
    target.0 = best.map(|b| b.1);
}

/// The interact key (default F) interacts; the potion keys (default 1 and 2) drink the first healing or
/// protection potion in the backpack.
pub(crate) fn item_keys(keys: Res<ButtonInput<KeyCode>>, binds: Res<crate::settings::Keys>, player: Res<LocalPlayer>, index: Res<SimIndex>, target: Res<InteractTarget>, inventories: Query<&Inventory>, mut queue: ResMut<CommandQueue>) {
    let Some(id) = player.0 else { return };
    use crate::settings::Bind;
    if keys.just_pressed(binds.get(Bind::Interact))
        && let Some(t) = target.0
    {
        queue.push(SimCommand::Interact { id, target: t });
    }
    // Letting go stops a held interaction (a door's).
    if keys.just_released(binds.get(Bind::Interact)) {
        queue.push(SimCommand::InteractCancel { id });
    }
    for (key, item_id) in [(binds.get(Bind::Potion1), 100_013), (binds.get(Bind::Potion2), 100_017)] {
        if keys.just_pressed(key)
            && let Some(uid) = index.get(id).and_then(|e| inventories.get(e).ok()).and_then(|inv| inv.bag.iter().find(|b| b.item.item.item_id == item_id).map(|b| b.item.uid))
        {
            queue.push(SimCommand::UseItem { id, uid });
        }
    }
}

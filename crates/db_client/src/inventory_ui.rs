//! Inventory window: Tab opens it and frees the cursor; worn slots on the left, the 10 × 5 backpack on
//! the right, decoded item icons sized by their grid footprint, and a tooltip for the item under the
//! cursor. Click a backpack item to wear or drink it, a worn item to take it off. The window only
//! sends inventory commands; the sim decides and the window redraws from the sim's `Inventory`.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use db_formats::config::generated::EnItemWearingPart as Slot;
// The worn slots and the second weapon set's.
use db_sim::attributes::STARTING_SLOTS as WORN_SLOTS;
use db_sim::combat::{AttackTerm, Element, HitZone, resolve_hit};
use db_sim::inventory::ItemInfo;
use db_sim::world_items::consumable;
use db_sim::{ActorKind, AttackKind, Combatant, CommandQueue, Effects, GameData, Inventory, InventoryCommand, ItemInstance, Kind, Position, SimCommand, SimEvent, SimId, SimIndex, SimRng};

use crate::Look;
use crate::combat_view::{FrameEvents, LocalPlayer};
use crate::items_view::{InteractTarget, icon_path, item_name, rarity_color};

/// Backpack cell size, px.
const CELL: f32 = 48.0;
const SLOT_ROW: f32 = 52.0;
const PANEL: Color = Color::srgb(0.06, 0.05, 0.045);
const CELL_BG: Color = Color::srgba(0.13, 0.11, 0.1, 1.0);
const CELL_LINE: Color = Color::srgba(0.25, 0.22, 0.19, 1.0);
const TEXT: Color = Color::srgb(0.92, 0.89, 0.82);

/// Whether the inventory window is open.
#[derive(Resource, Debug, Default)]
pub struct InventoryOpen(pub bool);

/// The item (uid) whose tooltip shows: the hovered one, else one pinned by the auto-loot demo.
#[derive(Resource, Debug, Default)]
pub struct Tooltip {
    pub hovered: Option<u32>,
    pub pinned: Option<u32>,
    /// Why the last click was refused, and the seconds it still shows.
    pub refused: Option<(&'static str, f32)>,
}

/// How long a refusal shows in the tooltip (s).
const REFUSED_SECS: f32 = 4.0;

/// Shows the sim's refusals of the inventory's commands while the inventory is open.
pub(crate) fn refusals(open: Res<InventoryOpen>, events: Res<crate::combat_view::FrameEvents>, time: Res<Time>, mut tooltip: ResMut<Tooltip>) {
    if let Some((_, left)) = tooltip.refused.as_mut() {
        *left -= time.delta_secs();
    }
    if tooltip.refused.is_some_and(|(_, left)| left <= 0.0) || !open.0 {
        tooltip.refused = None;
    }
    if !open.0 {
        return;
    }
    for e in &events.0 {
        if let db_sim::SimEvent::Rejected { reason } = e {
            tooltip.refused = Some((reason, REFUSED_SECS));
        }
    }
}

#[derive(Component)]
pub(crate) struct InvWindow;

#[derive(Component)]
pub(crate) struct BagGrid;

#[derive(Component)]
pub(crate) struct SlotBox(Slot);

/// A drawn item: backpack entry (`worn: None`) or worn in a slot.
#[derive(Component)]
pub(crate) struct InvItem {
    uid: u32,
    worn: Option<Slot>,
}

#[derive(Component)]
pub(crate) struct TooltipTitle;

#[derive(Component)]
pub(crate) struct TooltipBody;

/// Potion keys and Tab hint (bottom right).
#[derive(Component)]
pub(crate) struct BeltHud;

pub(crate) fn slot_label(s: Slot) -> &'static str {
    match s {
        Slot::Pair1rightweapon => "Main hand",
        Slot::Pair1leftweapon => "Off hand",
        Slot::Helmet => "Head",
        Slot::Upperarmor => "Chest",
        Slot::Glove => "Hands",
        Slot::Lowerarmor => "Legs",
        Slot::Shoe => "Feet",
        Slot::Pair2rightweapon => "Set 2 main hand",
        Slot::Pair2leftweapon => "Set 2 off hand",
        _ => "",
    }
}

/// Inventory text in Noto Sans, as the game's item tooltips.
fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), crate::fonts::sans(size), TextColor(color))
}

pub(crate) fn setup_inventory(mut commands: Commands) {
    let (grid_w, grid_h) = (f32::from(db_sim::overrides::inventory::BACKPACK_COLS) * CELL, f32::from(db_sim::overrides::inventory::BACKPACK_ROWS) * CELL);
    // Right column: grid, gap, up to about 12 tooltip lines.
    let body_h = (WORN_SLOTS.len() as f32 * SLOT_ROW).max(grid_h + 12.0 + 230.0);
    let (w, h) = (20.0 + 170.0 + 24.0 + grid_w + 20.0, 20.0 + 34.0 + body_h + 20.0);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                top: percent(50.0),
                width: px(w),
                height: px(h),
                margin: UiRect { left: px(-w / 2.0), top: px(-h / 2.0), ..default() },
                padding: UiRect::all(px(20.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(2.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(Color::srgb(0.42, 0.34, 0.24)),
            GlobalZIndex(10),
            Visibility::Hidden,
            InvWindow,
        ))
        .with_children(|win| {
            win.spawn((text("Inventory", 24.0, Color::srgb(0.95, 0.85, 0.6)), Node { height: px(34.0), ..default() }));
            win.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(24.0), ..default() }).with_children(|row| {
                // Worn slots.
                row.spawn(Node { width: px(170.0), flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                    for slot in WORN_SLOTS {
                        col.spawn(Node { height: px(SLOT_ROW), align_items: AlignItems::Center, column_gap: px(10.0), ..default() }).with_children(|r| {
                            r.spawn((Node { width: px(CELL), height: px(CELL), border: UiRect::all(px(1.0)), ..default() }, BackgroundColor(CELL_BG), BorderColor::all(CELL_LINE), SlotBox(slot)));
                            r.spawn(text(slot_label(slot), 16.0, Color::srgb(0.7, 0.66, 0.58)));
                        });
                    }
                });
                // Backpack grid plus tooltip under it.
                row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(12.0), ..default() }).with_children(|col| {
                    col.spawn((Node { width: px(grid_w), height: px(grid_h), ..default() }, BagGrid)).with_children(|g| {
                        for y in 0..db_sim::overrides::inventory::BACKPACK_ROWS {
                            for x in 0..db_sim::overrides::inventory::BACKPACK_COLS {
                                g.spawn((
                                    Node { position_type: PositionType::Absolute, left: px(f32::from(x) * CELL), top: px(f32::from(y) * CELL), width: px(CELL), height: px(CELL), border: UiRect::all(px(1.0)), ..default() },
                                    BackgroundColor(CELL_BG),
                                    BorderColor::all(CELL_LINE),
                                ));
                            }
                        }
                    });
                    col.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4.0), ..default() }).with_children(|t| {
                        t.spawn((text("", 20.0, TEXT), TooltipTitle));
                        t.spawn((text("Hover an item. Click: wear / drink / take off.", 15.0, Color::srgb(0.75, 0.72, 0.65)), TooltipBody));
                    });
                });
            });
        });
    commands.spawn((
        text("", 18.0, TEXT),
        TextShadow::default(),
        TextLayout::justify(Justify::Right),
        Node { position_type: PositionType::Absolute, right: px(24.0), bottom: px(24.0), ..default() },
        BeltHud,
        crate::flow::GameHud,
    ));
}

/// Tab toggles the window; open frees the cursor (closing leaves it free until the next click).
pub(crate) fn toggle_inventory(keys: Res<ButtonInput<KeyCode>>, binds: Res<crate::settings::Keys>, mut open: ResMut<InventoryOpen>, mut window: Query<&mut Visibility, With<InvWindow>>, mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if keys.just_pressed(binds.get(crate::settings::Bind::Inventory)) {
        open.0 = !open.0;
    }
    if keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
    }
    if let Ok(mut v) = window.single_mut() {
        *v = if open.0 { Visibility::Inherited } else { Visibility::Hidden };
    }
    if open.0
        && let Ok(mut c) = cursor.single_mut()
        && c.grab_mode != CursorGrabMode::None
    {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}

fn spawn_item(parent: &mut ChildSpawnerCommands, data: &GameData, assets: &AssetServer, item: &ItemInstance, worn: Option<Slot>, node: Node) {
    spawn_item_node(parent, data, assets, item, node, InvItem { uid: item.uid, worn });
}

/// An item drawn as a button: icon at its footprint, rarity border, stack count; `marker` says what it is.
pub(crate) fn spawn_item_node(parent: &mut ChildSpawnerCommands, data: &GameData, assets: &AssetServer, item: &ItemInstance, node: Node, marker: impl Bundle) {
    let color = rarity_color(item.item.rarity);
    parent
        .spawn((node, BackgroundColor(color.with_alpha(0.12)), BorderColor::all(color.with_alpha(0.8)), Button, marker))
        .with_children(|it| {
            if let Some(path) = icon_path(data, item.item.item_id) {
                it.spawn((ImageNode::new(assets.load(path)), Node { width: percent(100.0), height: percent(100.0), ..default() }));
            }
            if item.count > 1 {
                it.spawn((text(item.count.to_string(), 15.0, TEXT), TextShadow::default(), Node { position_type: PositionType::Absolute, right: px(3.0), bottom: px(1.0), ..default() }));
            }
        });
}

/// Redraws the items whenever the local player's inventory changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn redraw_items(
    mut commands: Commands,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    data: Option<Res<GameData>>,
    assets: Res<AssetServer>,
    inventories: Query<&Inventory>,
    drawn: Query<Entity, With<InvItem>>,
    grid: Query<Entity, With<BagGrid>>,
    slots: Query<(Entity, &SlotBox)>,
    mut last: Local<Option<Inventory>>,
) {
    let Some(data) = data else { return };
    let inv = player.0.and_then(|id| index.get(id)).and_then(|e| inventories.get(e).ok());
    if inv == last.as_ref() {
        return;
    }
    *last = inv.cloned();
    for e in &drawn {
        commands.entity(e).despawn();
    }
    let (Some(inv), Ok(grid)) = (inv, grid.single()) else { return };
    commands.entity(grid).with_children(|g| {
        for b in &inv.bag {
            let (cols, rows) = data.info(b.item.item.item_id).map_or((1, 1), |i| i.size);
            let node = Node {
                position_type: PositionType::Absolute,
                left: px(f32::from(b.x) * CELL),
                top: px(f32::from(b.y) * CELL),
                width: px(f32::from(cols) * CELL),
                height: px(f32::from(rows) * CELL),
                border: UiRect::all(px(2.0)),
                ..default()
            };
            spawn_item(g, &data, &assets, &b.item, None, node);
        }
    });
    for (e, slot) in &slots {
        let Some(item) = inv.worn.get(&slot.0) else { continue };
        // Fit the footprint's aspect into the slot box.
        let (cols, rows) = data.info(item.item.item_id).map_or((1, 1), |i| i.size);
        let k = (CELL - 2.0) / f32::from(cols.max(rows));
        let node = Node { width: px(f32::from(cols) * k), height: px(f32::from(rows) * k), margin: UiRect::AUTO, border: UiRect::all(px(1.0)), ..default() };
        commands.entity(e).with_children(|s| spawn_item(s, &data, &assets, item, Some(slot.0), node));
    }
}

/// The slot a backpack item goes to: its first free allowed slot, else its first allowed slot.
pub(crate) fn wear_slot(info: &ItemInfo, inv: &Inventory) -> Option<Slot> {
    let allowed: Vec<Slot> = WORN_SLOTS.iter().copied().filter(|s| info.slots.contains(s)).collect();
    allowed.iter().copied().find(|s| !inv.worn.contains_key(s)).or(allowed.first().copied())
}

/// the drop and destroy keys on the item under the cursor (its stack: one, or all).
pub(crate) fn drop_keys(open: Res<InventoryOpen>, player: Res<LocalPlayer>, keys: Res<ButtonInput<KeyCode>>, binds: Res<crate::settings::Keys>, tooltip: Res<Tooltip>, mut queue: ResMut<CommandQueue>) {
    use crate::settings::Bind;
    let (Some(id), Some(uid), true) = (player.0, tooltip.hovered, open.0) else { return };
    if keys.just_pressed(binds.get(Bind::DropOne)) {
        queue.push(SimCommand::Inventory(InventoryCommand::Drop { id, uid, count: Some(1) }));
    } else if keys.just_pressed(binds.get(Bind::DropAll)) {
        queue.push(SimCommand::Inventory(InventoryCommand::Drop { id, uid, count: None }));
    } else if keys.just_pressed(binds.get(Bind::Destroy)) {
        queue.push(SimCommand::Inventory(InventoryCommand::Destroy { id, uid }));
    }
}

/// Clicks send commands; hovering picks the tooltip item.
#[allow(clippy::too_many_arguments)]
pub(crate) fn item_clicks(
    open: Res<InventoryOpen>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    data: Option<Res<GameData>>,
    inventories: Query<&Inventory>,
    items: Query<(&Interaction, &InvItem)>,
    changed: Query<(&Interaction, &InvItem), Changed<Interaction>>,
    mut tooltip: ResMut<Tooltip>,
    mut queue: ResMut<CommandQueue>,
) {
    tooltip.hovered = items.iter().find(|(i, _)| **i != Interaction::None).map(|(_, it)| it.uid);
    let (Some(id), Some(data), true) = (player.0, data, open.0) else { return };
    let Some(inv) = index.get(id).and_then(|e| inventories.get(e).ok()) else { return };
    for (interaction, item) in &changed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(slot) = item.worn {
            queue.push(SimCommand::Inventory(InventoryCommand::Unequip { id, slot }));
            continue;
        }
        let Some(entry) = inv.bag.iter().find(|b| b.item.uid == item.uid) else { continue };
        if consumable(entry.item.item.item_id).is_some() {
            queue.push(SimCommand::UseItem { id, uid: item.uid });
        } else if let Some(slot) = data.info(entry.item.item.item_id).and_then(|i| wear_slot(i, inv)) {
            queue.push(SimCommand::Inventory(InventoryCommand::Equip { id, uid: item.uid, slot }));
        }
    }
}

/// `attack_weapon_attack_base` → "Weapon attack base".
fn pretty_attribute(a: &str) -> String {
    let s = a.replace('_', " ");
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

fn trim_number(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn tooltip_text(
    tooltip: Res<Tooltip>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    data: Option<Res<GameData>>,
    inventories: Query<&Inventory>,
    (binds, staffs, sheets): (Res<crate::settings::Keys>, Option<Res<db_sim::StaffDb>>, Query<&db_sim::inventory::CharacterSheet>),
    mut title: Query<(&mut Text, &mut TextColor), (With<TooltipTitle>, Without<TooltipBody>)>,
    mut body: Query<&mut Text, (With<TooltipBody>, Without<TooltipTitle>)>,
) {
    let (Ok((mut title, mut color)), Ok(mut body), Some(data)) = (title.single_mut(), body.single_mut(), data) else { return };
    let inv = player.0.and_then(|id| index.get(id)).and_then(|e| inventories.get(e).ok());
    let uid = tooltip.hovered.or(tooltip.pinned);
    let found = inv.zip(uid).and_then(|(inv, uid)| {
        inv.bag.iter().find(|b| b.item.uid == uid).map(|b| (&b.item, None)).or_else(|| inv.worn.iter().find(|(_, i)| i.uid == uid).map(|(s, i)| (i, Some(*s))))
    });
    let refused = tooltip.refused.map(|(r, _)| format!("Refused: {r}\n"));
    let Some((item, worn)) = found else {
        title.0 = String::new();
        body.0 = format!("{}Hover an item. Click: wear / drink / take off.", refused.unwrap_or_default());
        return;
    };
    title.0 = item_name(&data, item.item.item_id);
    color.0 = rarity_color(item.item.rarity);
    let info = data.info(item.item.item_id);
    let (cols, rows) = info.map_or((1, 1), |i| i.size);
    let mut lines = vec![format!("{:?}  |  {cols} x {rows} cells{}", item.item.rarity, info.filter(|i| i.stack > 1).map_or(String::new(), |i| format!("  |  stacks to {}", i.stack)))];
    if info.is_some_and(|i| i.two_handed) {
        lines.push("Two-handed".into());
    }
    // Who can wear it (with the class switch keys of the test room, `monster_view::CLASS_KEYS`).
    if let Some(i) = info.filter(|i| !i.careers.is_empty() && !i.slots.is_empty()) {
        let career = player.0.and_then(|id| index.get(id)).and_then(|e| sheets.get(e).ok()).map(|s| s.career);
        let names: Vec<String> = i.careers.iter().map(|c| crate::monster_view::class_label(*c)).collect();
        lines.push(format!("Classes: {}{}", names.join(", "), if career.is_some_and(|c| !i.careers.contains(&c)) { "  (not yours)" } else { "" }));
    }
    // a staff's element and its hold spell (the names don't say; ice staffs have none, as in the original).
    if let Some(spec) = staffs.as_ref().and_then(|db| db.0.get(&item.item.item_id)) {
        let spell = match (spec.beam.is_some(), spec.thunder.is_some()) {
            (true, _) => format!("hold left click: fire beam ({} energy)", spec.cost),
            (_, true) => format!("hold left click: thunder strike ({} energy)", spec.cost),
            _ => "no hold spell (melee only)".into(),
        };
        lines.push(format!("{:?} staff, {} energy: {spell}", spec.element, spec.max));
    }
    for a in &item.item.affixes {
        lines.push(format!("{}  {}{}", pretty_attribute(&a.attribute), if a.value >= 0.0 { "+" } else { "" }, trim_number(a.value)));
    }
    lines.push(match (worn, consumable(item.item.item_id)) {
        (Some(s), _) => format!("Worn ({}). Click to take off.", slot_label(s)),
        (None, Some(_)) => "Click (or 1 / 2) to drink.".into(),
        (None, None) if info.is_some_and(|i| !i.slots.is_empty()) => "Click to wear.".into(),
        _ => String::new(),
    });
    // the drop / destroy keys (heirlooms can't be dropped).
    use crate::settings::{Bind, key_name};
    let key = |b| key_name(binds.get(b)).trim_start_matches("Key").to_string();
    lines.push(if data.is_heirloom(item.item.item_id) {
        "Heirloom: can't be dropped.".into()
    } else if item.count > 1 {
        format!("[{}] drop one  [{}] drop all  [{}] destroy", key(Bind::DropOne), key(Bind::DropAll), key(Bind::Destroy))
    } else {
        format!("[{}] drop  [{}] destroy", key(Bind::DropOne), key(Bind::Destroy))
    });
    body.0 = format!("{}{}", refused.unwrap_or_default(), lines.join("\n"));
}

/// Potion counts and effects on the local player (bottom right).
pub(crate) fn belt_hud(player: Res<LocalPlayer>, index: Res<SimIndex>, inventories: Query<(&Inventory, Option<&Effects>)>, mut hud: Query<&mut Text, With<BeltHud>>) {
    let Ok(mut t) = hud.single_mut() else { return };
    let Some((_, fx)) = player.0.and_then(|id| index.get(id)).and_then(|e| inventories.get(e).ok()) else {
        t.0 = String::new();
        return;
    };
    let secs = |ticks: u32| (ticks as f32 * db_sim::DT).ceil();
    let mut lines = Vec::new();
    if let Some(fx) = fx {
        if let Some((per, pulses, _)) = fx.heal {
            lines.push(format!("Healing +{per:.0}/s  {pulses} s"));
        }
        if let Some((left, ticks)) = fx.shield {
            lines.push(format!("Shield {left:.0}  {:.0} s", secs(ticks)));
        }
        if fx.potion_cooldown > 0 {
            lines.push(format!("Protection cooldown {:.0} s", secs(fx.potion_cooldown)));
        }
    }
    // The potion counts are on their slots (`hud`).
    lines.push("[Tab] Inventory".to_string());
    t.0 = lines.join("\n");
}

/// Dev/demo (`--auto-loot`): hit the dummy, open the chest, pick everything up, wear the best new
/// weapon, hit the dummy again (both hits logged with the formula's non-crit value), then leave the
/// inventory open with the new weapon's tooltip for the screenshot.
#[derive(Resource, Debug, Default)]
pub struct AutoLoot(pub bool);

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) enum LootStep {
    #[default]
    Start,
    HitBefore,
    ToChest,
    Opening,
    Picking(Vec<SimId>),
    Equip,
    HitAfter,
    Show,
}

#[derive(Debug, Default)]
pub(crate) struct LootBot {
    step: LootStep,
    timer: f32,
    before: Option<f64>,
}

/// Non-crit light-hit damage of `attacker` on `target` (the formula the sim applies, without the crit roll).
fn expected_hit(attacker: &Combatant, target: &Combatant) -> f64 {
    let mut a = attacker.attack;
    a.critical_rate = 0.0;
    resolve_hit(&a, &target.defence, Element::Physical, AttackTerm::Weapon, HitZone::Body, &mut SimRng::new(0)).damage
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn auto_loot(
    auto: Res<AutoLoot>,
    time: Res<Time>,
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    target: Res<InteractTarget>,
    data: Option<Res<GameData>>,
    actors: Query<(&Kind, &Position, Option<&Combatant>)>,
    inventories: Query<&Inventory>,
    mut look: ResMut<Look>,
    mut open: ResMut<InventoryOpen>,
    mut tooltip: ResMut<Tooltip>,
    mut queue: ResMut<CommandQueue>,
    mut bot: Local<LootBot>,
) {
    let (true, Some(id), Some(data)) = (auto.0, player.0, data) else { return };
    let Some((_, me, my_stats)) = index.get(id).and_then(|e| actors.get(e).ok()) else { return };
    let find = |kind: ActorKind| index.iter().find(|(_, e)| actors.get(*e).is_ok_and(|a| a.0.0 == kind)).map(|(i, e)| (i, actors.get(e).unwrap()));
    let (Some((dummy, (_, dummy_pos, dummy_stats))), chest) = (find(ActorKind::Dummy), find(ActorKind::Chest)) else { return };
    let face = |look: &mut Look, p: Vec3| {
        let d = p - me.0;
        look.yaw = d.y.atan2(d.x).rem_euclid(std::f32::consts::TAU);
        look.pitch = (-(me.0.z - p.z + 40.0)).atan2(d.truncate().length()).clamp(-0.9, 0.2);
    };
    bot.timer += time.delta_secs();
    let hit_on_dummy = events.0.iter().find_map(|e| match e {
        SimEvent::Damaged { source, target, damage, .. } if *source == id && *target == dummy => Some(*damage),
        _ => None,
    });
    match bot.step.clone() {
        LootStep::Start if bot.timer > 1.0 => {
            face(&mut look, dummy_pos.0);
            queue.push(SimCommand::Attack { id, kind: AttackKind::Light });
            bot.step = LootStep::HitBefore;
        }
        LootStep::HitBefore => {
            if let Some(d) = hit_on_dummy {
                let expected = my_stats.zip(dummy_stats).map_or(0.0, |(a, t)| expected_hit(a, t));
                info!("auto-loot: dummy hit before {d:.2} (formula, non-crit {expected:.2})");
                bot.before = Some(d);
                (bot.step, bot.timer) = (LootStep::ToChest, 0.0);
            }
        }
        LootStep::ToChest if bot.timer > 1.0 => {
            let Some((chest, (_, chest_pos, _))) = chest else { return };
            face(&mut look, chest_pos.0);
            if target.0 == Some(chest) || bot.timer > 3.0 {
                if target.0 != Some(chest) {
                    warn!("auto-loot: the prompt never picked the chest; interacting directly");
                }
                queue.push(SimCommand::Interact { id, target: chest });
                (bot.step, bot.timer) = (LootStep::Opening, 0.0);
            }
        }
        LootStep::Opening => {
            if let Some(items) = events.0.iter().find_map(|e| if let SimEvent::ChestOpened { items, .. } = e { Some(items.clone()) } else { None }) {
                info!("auto-loot: chest opened, {} items", items.len());
                // Linger before the first pickup so a screenshot can catch the items on the floor.
                (bot.step, bot.timer) = (LootStep::Picking(items), -1.2);
            }
        }
        LootStep::Picking(mut items) if bot.timer > 0.4 => {
            match items.pop() {
                Some(item) => {
                    if let Some((_, p, _)) = index.get(item).and_then(|e| actors.get(e).ok()) {
                        face(&mut look, p.0);
                    }
                    queue.push(SimCommand::Interact { id, target: item });
                    (bot.step, bot.timer) = (LootStep::Picking(items), 0.0);
                }
                None => (bot.step, bot.timer) = (LootStep::Equip, 0.0),
            }
        }
        LootStep::Equip if bot.timer > 0.3 => {
            let Some(inv) = index.get(id).and_then(|e| inventories.get(e).ok()) else { return };
            open.0 = true;
            let weapon = inv.bag.iter().find(|b| data.info(b.item.item.item_id).is_some_and(|i| i.slots.contains(&Slot::Pair1rightweapon)));
            if let Some(w) = weapon {
                info!("auto-loot: wearing {} ({})", item_name(&data, w.item.item.item_id), w.item.item.item_id);
                queue.push(SimCommand::Inventory(InventoryCommand::Equip { id, uid: w.item.uid, slot: Slot::Pair1rightweapon }));
                tooltip.pinned = Some(w.item.uid);
            }
            (bot.step, bot.timer) = (LootStep::HitAfter, 0.0);
        }
        LootStep::HitAfter if bot.timer > 0.8 => {
            if bot.timer < 10.0 && hit_on_dummy.is_none() {
                if open.0 {
                    open.0 = false;
                    face(&mut look, dummy_pos.0);
                    queue.push(SimCommand::Attack { id, kind: AttackKind::Light });
                }
                return;
            }
            let expected = my_stats.zip(dummy_stats).map_or(0.0, |(a, t)| expected_hit(a, t));
            info!("auto-loot: dummy hit after {:.2} (formula, non-crit {expected:.2}); before {:.2}", hit_on_dummy.unwrap_or(0.0), bot.before.unwrap_or(0.0));
            (bot.step, bot.timer) = (LootStep::Show, 0.0);
        }
        LootStep::Show if bot.timer > 1.2 => {
            open.0 = true;
            // The main hand's tooltip: the new weapon if it went on, else what the class kept (a Wizard can't wear a sword).
            if let Some(w) = index.get(id).and_then(|e| inventories.get(e).ok()).and_then(|i| i.worn.get(&Slot::Pair1rightweapon)) {
                tooltip.pinned = Some(w.uid);
            }
        }
        _ => {}
    }
}

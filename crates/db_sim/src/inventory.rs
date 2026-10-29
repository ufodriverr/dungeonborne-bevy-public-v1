//! Inventory and equipment.
//!
//! CONFIRMED (tables): an item's footprint and stack limit (`UESourceItemType.position_cfg`: `column_size`
//! wide, `row_size` tall, `accumulation_limit`), where it can be worn and by whom (`UeSourceEquipment`:
//! weapon hand `ONLYRIGHT`/`ONLYLEFT`/`BOTHHAND`, `can_equip_double`, `career_restrict`; armour and
//! accessories `available_equip_slot`), affix rolls. HYPOTHESIS: backpack size
//! ([`crate::overrides::inventory`]); `can_equip_double` lets a one-hand weapon go in the left hand too; a
//! two-hand weapon also empties the left hand; equipping changes maximum health but never heals; worn
//! affixes add to the class sheet (guard takes the best item).

use std::collections::{BTreeMap, HashMap};

use anyhow::Context;
use bevy_ecs::prelude::*;
use db_formats::ConfigTables;
use db_formats::config::generated::{EnItemUeType, EnItemWearingPart, EnRoleCareerType, EnWeaponEquipType};

use crate::attributes::AttributeSheet;
use crate::items::{AffixSource, Item, ItemDb};
use crate::melee::{Combatant, Health};
use crate::overrides::inventory as tune;

/// What the tables say about an item id.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemInfo {
    /// (columns, rows) in the backpack grid.
    pub size: (u8, u8),
    pub stack: u16,
    /// Worn slots it can go in (empty: not wearable).
    pub slots: Vec<EnItemWearingPart>,
    /// Two-handed weapons occupy both hands.
    pub two_handed: bool,
    /// Classes allowed to wear it (empty: anyone).
    pub careers: Vec<EnRoleCareerType>,
    pub kind: EnItemUeType,
}

/// Static game data the sim reads (config tables). Inserted once as a resource.
#[derive(Resource)]
pub struct GameData {
    pub tables: &'static ConfigTables,
    pub items: ItemDb<'static>,
    pub loot: crate::loot::LootDb<'static>,
    info: HashMap<i32, ItemInfo>,
    /// heirloom items (`ExcelGearHeirloomGuide` type ids): they can't be dropped
    /// (`Heirloom_inside_ProhibitDiscarding`).
    heirlooms: std::collections::HashSet<i32>,
}

impl std::fmt::Debug for GameData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameData").field("items", &self.info.len()).finish_non_exhaustive()
    }
}

impl GameData {
    /// Takes ownership of the tables for the rest of the process (they are read-only game data).
    pub fn new(tables: ConfigTables) -> anyhow::Result<Self> {
        let tables: &'static ConfigTables = Box::leak(Box::new(tables));
        let items = ItemDb::new(tables)?;
        let equip: HashMap<i32, _> = tables.ue_source_equipment.iter().map(|e| (e.equip_id, e)).collect();
        let mut info = HashMap::new();
        for t in &tables.uesource_item_type {
            let pos = t.position_cfg.clone().unwrap_or_default();
            let mut slots = Vec::new();
            let mut careers = Vec::new();
            let mut two_handed = false;
            if let Some(e) = equip.get(&t.ue_source_equipment_equip_id) {
                for w in e.weapon_configs.iter().filter(|w| w.is_weapon) {
                    match w.weapon_equip_type {
                        EnWeaponEquipType::Onlyright => slots.push(EnItemWearingPart::Pair1rightweapon),
                        EnWeaponEquipType::Onlyleft => slots.push(EnItemWearingPart::Pair1leftweapon),
                        EnWeaponEquipType::Bothhand => {
                            slots.push(EnItemWearingPart::Pair1rightweapon);
                            two_handed = true;
                        }
                        _ => {}
                    }
                    if w.can_equip_double && w.weapon_equip_type == EnWeaponEquipType::Onlyright {
                        slots.push(EnItemWearingPart::Pair1leftweapon);
                    }
                    careers.extend(w.career_restrict.iter().copied());
                }
                for g in e.gear_configs.iter().filter(|g| g.is_gear) {
                    slots.extend(g.available_equip_slot.iter().copied());
                    careers.extend(g.career_restrict.iter().copied());
                }
            }
            careers.retain(|c| *c != EnRoleCareerType::Invalid);
            slots.dedup();
            // The second weapon set takes what the first does.
            for (held, sheathed) in crate::weapon_slots::PAIRS {
                if slots.contains(&held) {
                    slots.push(sheathed);
                }
            }
            let size = (pos.column_size.clamp(1, 255) as u8, pos.row_size.clamp(1, 255) as u8);
            info.insert(t.type_id, ItemInfo { size, stack: pos.accumulation_limit.clamp(1, 9999) as u16, slots, two_handed, careers, kind: t.item_type });
        }
        let heirlooms = tables.excel_gear_heirloom_guide.iter().map(|h| h.type_id).collect();
        Ok(GameData { tables, items, loot: crate::loot::LootDb::new(tables)?, info, heirlooms })
    }

    pub fn is_heirloom(&self, item_id: i32) -> bool {
        self.heirlooms.contains(&item_id)
    }

    pub fn info(&self, item_id: i32) -> Option<&ItemInfo> {
        self.info.get(&item_id)
    }

    /// A new item: affixes rolled when the item has an affix template (equipment), none otherwise
    /// (potions, torches, treasure).
    pub fn roll_item(&self, item_id: i32, rng: &mut crate::SimRng) -> anyhow::Result<Item> {
        if self.items.has_template(item_id) {
            return self.items.roll(item_id, rng);
        }
        let rarity = self.items.rarity(item_id).with_context(|| format!("item {item_id} is not in UESourceItemType"))?;
        Ok(Item { item_id, rarity, affixes: Vec::new() })
    }

    /// A starting item: only its fixed (BASIC) affixes (HYPOTHESIS, same as `starting_loadout`).
    /// What the merchant pays for one (`ExcelItemType.system_repurchase_price`; 0 = not bought).
    pub fn sell_price(&self, item_id: i32) -> u32 {
        self.tables.excel_item_type.iter().find(|t| t.type_id == item_id).map_or(0, |t| t.system_repurchase_price.max(0) as u32)
    }

    pub fn starting_item(&self, item_id: i32) -> anyhow::Result<Item> {
        let mut item = self.roll_item(item_id, &mut crate::SimRng::new(0))?;
        item.affixes.retain(|a| a.source == AffixSource::Fixed);
        Ok(item)
    }
}

/// An item in the world: a unique id, the rolled item and a stack count.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemInstance {
    pub uid: u32,
    pub item: Item,
    pub count: u16,
}

/// Next item uid (deterministic).
#[derive(Resource, Debug, Default)]
pub struct ItemUids(u32);

impl ItemUids {
    pub fn alloc(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BagEntry {
    pub item: ItemInstance,
    /// Top-left cell.
    pub x: u8,
    pub y: u8,
}

/// Backpack grid and worn items.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub cols: u8,
    pub rows: u8,
    pub bag: Vec<BagEntry>,
    pub worn: BTreeMap<EnItemWearingPart, ItemInstance>,
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory { cols: tune::BACKPACK_COLS, rows: tune::BACKPACK_ROWS, bag: Vec::new(), worn: BTreeMap::new() }
    }
}

impl Inventory {
    /// An empty grid of another size (the stash).
    pub fn with_size(cols: u8, rows: u8) -> Self {
        Inventory { cols, rows, ..Default::default() }
    }

    /// Every item, worn first, then the backpack.
    pub fn items(&self) -> impl Iterator<Item = &ItemInstance> {
        self.worn.values().chain(self.bag.iter().map(|b| &b.item))
    }

    /// Gives every item a fresh uid from `uids` (items loaded from a save or another run).
    pub fn renumber(&mut self, uids: &mut ItemUids) {
        for item in self.worn.values_mut() {
            item.uid = uids.alloc();
        }
        for b in &mut self.bag {
            b.item.uid = uids.alloc();
        }
    }

    /// Whether a `size` item fits with its top-left at (x, y), ignoring the entry `except`.
    pub fn fits(&self, size: (u8, u8), x: u8, y: u8, except: Option<u32>, data: &GameData) -> bool {
        if u16::from(x) + u16::from(size.0) > u16::from(self.cols) || u16::from(y) + u16::from(size.1) > u16::from(self.rows) {
            return false;
        }
        self.bag.iter().filter(|b| Some(b.item.uid) != except).all(|b| {
            let s = data.info(b.item.item.item_id).map_or((1, 1), |i| i.size);
            x + size.0 <= b.x || b.x + s.0 <= x || y + size.1 <= b.y || b.y + s.1 <= y
        })
    }

    /// The first free spot, scanning rows top to bottom, then columns left to right.
    pub fn first_fit(&self, size: (u8, u8), data: &GameData) -> Option<(u8, u8)> {
        (0..self.rows).flat_map(|y| (0..self.cols).map(move |x| (x, y))).find(|&(x, y)| self.fits(size, x, y, None, data))
    }

    /// Puts an item in the backpack: onto a matching stack with room, else the first free spot.
    pub fn add(&mut self, mut item: ItemInstance, data: &GameData) -> Result<(), ItemInstance> {
        let info = data.info(item.item.item_id).cloned();
        let (size, stack) = info.map_or(((1, 1), 1), |i| (i.size, i.stack));
        if stack > 1 {
            for b in self.bag.iter_mut().filter(|b| b.item.item.item_id == item.item.item_id && b.item.count < stack) {
                let moved = item.count.min(stack - b.item.count);
                b.item.count += moved;
                item.count -= moved;
                if item.count == 0 {
                    return Ok(());
                }
            }
        }
        match self.first_fit(size, data) {
            Some((x, y)) => {
                self.bag.push(BagEntry { item, x, y });
                Ok(())
            }
            None => Err(item),
        }
    }

    pub fn take(&mut self, uid: u32) -> Option<BagEntry> {
        let i = self.bag.iter().position(|b| b.item.uid == uid)?;
        Some(self.bag.remove(i))
    }
}

/// A character's class and base sheet (before worn items).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CharacterSheet {
    pub career: EnRoleCareerType,
    pub base: AttributeSheet,
}

/// Combat numbers from the base sheet plus every affix of every worn item.
pub fn combat_stats(sheet: &CharacterSheet, inv: &Inventory) -> (Combatant, f64) {
    let (s, guard) = worn_sheet(sheet, inv);
    (Combatant { attack: s.attack_stats(), defence: s.defence_stats(), guard_reduce: guard }, crate::gear_triggers::max_health(&s, 0.0))
}

/// The base sheet plus every affix of every worn item, and the best guard reduction (not summed).
pub fn worn_sheet(sheet: &CharacterSheet, inv: &Inventory) -> (AttributeSheet, f64) {
    let mut s = sheet.base.clone();
    let mut guard: f64 = 0.0;
    // The sheathed weapon set doesn't count.
    for item in inv.worn.iter().filter(|(s, _)| !matches!(s, EnItemWearingPart::Pair2rightweapon | EnItemWearingPart::Pair2leftweapon)).map(|(_, i)| i) {
        for a in &item.item.affixes {
            if a.attribute == "defend_guarded_damage_reduce_rate" {
                guard = guard.max(a.value);
            } else {
                *s.0.entry(format!("affix_{}", a.attribute)).or_insert(0.0) += a.value;
            }
        }
    }
    (s, guard)
}

/// Applies `combat_stats` to the actor: combat stats replaced, maximum health updated (current
/// health kept, capped at the new maximum; full health on the first call).
pub(crate) fn refresh_combat(world: &mut World, e: Entity) {
    let (Some(sheet), Some(inv)) = (world.get::<CharacterSheet>(e), world.get::<Inventory>(e)) else { return };
    let (worn, guard) = worn_sheet(sheet, inv);
    let mut combatant = Combatant { attack: worn.attack_stats(), defence: worn.defence_stats(), guard_reduce: guard };
    let growth = crate::gear_triggers::apply_growth(world, e, &mut combatant);
    let max = crate::gear_triggers::max_health(&worn, growth);
    let health = world.get::<Health>(e).copied();
    let health = match health {
        Some(h) => Health { current: h.current.min(max), max },
        None => Health { current: max, max },
    };
    world.entity_mut(e).insert((combatant, health, crate::gear_triggers::WornAttributes(worn)));
    if world.get::<crate::melee::Melee>(e).is_none() {
        world.entity_mut(e).insert(crate::melee::Melee::default());
    }
}

/// Why an inventory command was refused.
pub fn equip_check(info: &ItemInfo, slot: EnItemWearingPart, career: EnRoleCareerType) -> Result<(), &'static str> {
    if !info.slots.contains(&slot) {
        return Err("item can't be worn in that slot");
    }
    if !info.careers.is_empty() && !info.careers.contains(&career) {
        return Err("class can't wear this item");
    }
    Ok(())
}

/// The same character's items for another class (the test room's class switch, playtest: "don't clear my
/// inventory when I switch classes"), and what didn't fit in the backpack (to drop at the feet).
///
/// The class's own weapons go in hand (its skills and spells need them) and the held ones go to the backpack, as do
/// worn items the class can't wear; an empty starting slot takes that starting item from the backpack, else a new
/// one; starting backpack items (the crossbow classes' arrows) the bag lacks are added. Unchanged starting gear
/// taken off is left out: any class's starting item comes back new on a switch, and keeping them filled the bag
/// after two switches. New items have uid 0: `SetCharacterItems` renumbers them.
pub fn for_class(inv: &Inventory, data: &GameData, career: EnRoleCareerType, worn: &[(EnItemWearingPart, i32)], bag: &[(i32, u16)]) -> (Inventory, Vec<ItemInstance>) {
    let mut inv = inv.clone();
    let mut left = Vec::new();
    let starting = |item: &ItemInstance| data.starting_item(item.item.item_id).is_ok_and(|s| s == item.item);
    let weapon = |s: EnItemWearingPart| matches!(s, EnItemWearingPart::Pair1rightweapon | EnItemWearingPart::Pair1leftweapon | EnItemWearingPart::Pair2rightweapon | EnItemWearingPart::Pair2leftweapon);
    let mut taken_off = Vec::new();
    let slots: Vec<EnItemWearingPart> = inv.worn.keys().copied().collect();
    for slot in slots {
        let ok = !weapon(slot) && inv.worn.get(&slot).and_then(|i| data.info(i.item.item_id)).is_some_and(|i| equip_check(i, slot, career).is_ok());
        if !ok && let Some(item) = inv.worn.remove(&slot) {
            taken_off.push(item);
        }
    }
    for &(slot, item_id) in worn {
        if inv.worn.contains_key(&slot) {
            continue;
        }
        // Its own starting item: just taken off, from the backpack, else new.
        let item = if let Some(i) = taken_off.iter().position(|t| t.item.item_id == item_id) {
            Some(taken_off.remove(i))
        } else if let Some(i) = inv.bag.iter().position(|b| b.item.item.item_id == item_id) {
            Some(inv.bag.remove(i).item)
        } else {
            data.starting_item(item_id).ok().map(|item| ItemInstance { uid: 0, item, count: 1 })
        };
        if let Some(item) = item {
            inv.worn.insert(slot, item);
        }
    }
    // A two-handed weapon leaves its set's other hand empty.
    for (right, hand) in [(EnItemWearingPart::Pair1rightweapon, EnItemWearingPart::Pair1leftweapon), (EnItemWearingPart::Pair2rightweapon, EnItemWearingPart::Pair2leftweapon)] {
        if inv.worn.get(&right).and_then(|i| data.info(i.item.item_id)).is_some_and(|i| i.two_handed)
            && let Some(item) = inv.worn.remove(&hand)
        {
            taken_off.push(item);
        }
    }
    for item in taken_off.into_iter().filter(|i| !starting(i)) {
        let size = data.info(item.item.item_id).map_or((1, 1), |i| i.size);
        match inv.first_fit(size, data) {
            Some((x, y)) => inv.bag.push(BagEntry { item, x, y }),
            None => left.push(item),
        }
    }
    for &(item_id, count) in bag {
        if !inv.items().any(|i| i.item.item_id == item_id)
            && let Ok(item) = data.starting_item(item_id)
            && let Err(item) = inv.add(ItemInstance { uid: 0, item, count: count.max(1) }, data)
        {
            left.push(item);
        }
    }
    (inv, left)
}

/// Builds a character: class sheet plus starting items worn in the given slots.
pub fn starting_inventory(data: &GameData, worn: &[(EnItemWearingPart, i32)], uids: &mut ItemUids) -> anyhow::Result<Inventory> {
    let mut inv = Inventory::default();
    for &(slot, id) in worn {
        let item = data.starting_item(id).with_context(|| format!("starting item {id}"))?;
        inv.worn.insert(slot, ItemInstance { uid: uids.alloc(), item, count: 1 });
    }
    Ok(inv)
}

/// Applies an inventory command; reports `InventoryChanged` or `Rejected`.
pub(crate) fn apply(world: &mut World, cmd: InventoryCommand) {
    use crate::{SimEvent, SimEvents};
    let id = cmd.actor();
    let Some(e) = world.resource::<crate::SimIndex>().get(id) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
        return;
    };
    let result = world.resource_scope(|world, data: Mut<GameData>| run(world, e, &data, cmd));
    let event = match result {
        Ok(()) => {
            refresh_combat(world, e);
            SimEvent::InventoryChanged { id }
        }
        Err(reason) => SimEvent::Rejected { reason },
    };
    world.resource_mut::<SimEvents>().push(event);
}

/// Inventory commands (sent as `SimCommand::Inventory`).
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryCommand {
    /// Class sheet plus starting items worn in the given slots (replaces the actor's inventory).
    SetCharacter { id: crate::SimId, career: EnRoleCareerType, sheet: Box<AttributeSheet>, worn: Vec<(EnItemWearingPart, i32)> },
    /// Class sheet plus items carried in from the stash (uids are renumbered by the sim).
    SetCharacterItems { id: crate::SimId, career: EnRoleCareerType, sheet: Box<AttributeSheet>, inventory: Box<Inventory> },
    /// Roll a new item (sim RNG) into the backpack.
    Give { id: crate::SimId, item_id: i32 },
    /// Wear a backpack item; whatever it displaces goes back to the backpack.
    Equip { id: crate::SimId, uid: u32, slot: EnItemWearingPart },
    Unequip { id: crate::SimId, slot: EnItemWearingPart },
    /// Move a backpack item so its top-left cell is (x, y).
    Move { id: crate::SimId, uid: u32, x: u8, y: u8 },
    /// drop a carried or worn item on the floor in front of the actor: `count` of its stack (the game's
    /// "drop one by one"), or all of it with `None` ("drop all at once"). Heirlooms are refused.
    Drop { id: crate::SimId, uid: u32, count: Option<u16> },
    /// (port only, the request; the game has no such string as far as found): destroy a carried or worn
    /// item. Heirlooms are refused.
    Destroy { id: crate::SimId, uid: u32 },
    /// a starting item into the backpack, as the class starting gear has it (no rolled affixes), `count` of it.
    GiveStarting { id: crate::SimId, item_id: i32, count: u16 },
}

impl InventoryCommand {
    fn actor(&self) -> crate::SimId {
        match self {
            Self::SetCharacter { id, .. } | Self::SetCharacterItems { id, .. } | Self::Give { id, .. } | Self::Equip { id, .. } | Self::Unequip { id, .. } | Self::Move { id, .. } | Self::Drop { id, .. } | Self::Destroy { id, .. } | Self::GiveStarting { id, .. } => *id,
        }
    }
}

fn run(world: &mut World, e: Entity, data: &GameData, cmd: InventoryCommand) -> Result<(), &'static str> {
    if let InventoryCommand::SetCharacter { career, sheet, worn, .. } = cmd {
        let inv = starting_inventory(data, &worn, &mut world.resource_mut::<ItemUids>()).map_err(|_| "unknown starting item")?;
        world.entity_mut(e).insert((CharacterSheet { career, base: *sheet }, inv));
        return Ok(());
    }
    if let InventoryCommand::SetCharacterItems { career, sheet, mut inventory, .. } = cmd {
        inventory.renumber(&mut world.resource_mut::<ItemUids>());
        world.entity_mut(e).insert((CharacterSheet { career, base: *sheet }, *inventory));
        return Ok(());
    }
    let career = world.get::<CharacterSheet>(e).map_or(EnRoleCareerType::Invalid, |s| s.career);
    if let InventoryCommand::GiveStarting { item_id, count, .. } = cmd {
        let item = data.starting_item(item_id).map_err(|_| "unknown starting item")?;
        let uid = world.resource_mut::<ItemUids>().alloc();
        let mut inv = world.get_mut::<Inventory>(e).ok_or("actor has no inventory")?;
        return inv.add(ItemInstance { uid, item, count: count.max(1) }, data).map_err(|_| "no room in the backpack");
    }
    if let InventoryCommand::Give { item_id, .. } = cmd {
        let item = data.roll_item(item_id, &mut world.resource_mut::<crate::SimRng>()).map_err(|_| "item can't be rolled")?;
        let uid = world.resource_mut::<ItemUids>().alloc();
        let mut inv = world.get_mut::<Inventory>(e).ok_or("actor has no inventory")?;
        return inv.add(ItemInstance { uid, item, count: 1 }, data).map_err(|_| "no room in the backpack");
    }
    match cmd {
        InventoryCommand::Drop { uid, count, .. } => {
            let item = remove_item(world, e, data, uid, count)?;
            // On the floor a step in front (`tune::DROP_AHEAD`), like loot from the actor.
            let (pos, fwd) = (world.get::<crate::Position>(e).map_or(bevy_math::Vec3::ZERO, |p| p.0), world.get::<crate::Aim>(e).copied().unwrap_or_default().forward_2d());
            let floor = pos + fwd.extend(0.0) * tune::DROP_AHEAD - bevy_math::Vec3::Z * crate::movement::CAPSULE_HEIGHT * 0.5;
            crate::world_items::scatter(world, floor, vec![item]);
            return Ok(());
        }
        InventoryCommand::Destroy { uid, .. } => {
            remove_item(world, e, data, uid, None)?;
            return Ok(());
        }
        _ => {}
    }
    let mut inv = world.get_mut::<Inventory>(e).ok_or("actor has no inventory")?;
    let before = inv.clone();
    let r = edit(&mut inv, data, career, cmd);
    if r.is_err() {
        *inv = before;
    }
    r
}

/// Takes `count` of a carried or worn item (all with `None`) out of the actor's inventory; heirlooms are refused.
fn remove_item(world: &mut World, e: Entity, data: &GameData, uid: u32, count: Option<u16>) -> Result<ItemInstance, &'static str> {
    let fresh = world.resource_mut::<ItemUids>().alloc();
    let mut inv = world.get_mut::<Inventory>(e).ok_or("actor has no inventory")?;
    let held = inv.bag.iter().map(|b| &b.item).chain(inv.worn.values()).find(|i| i.uid == uid).ok_or("no such item")?;
    if data.is_heirloom(held.item.item_id) {
        return Err("heirlooms can't be dropped");
    }
    let n = count.unwrap_or(held.count).clamp(1, held.count.max(1));
    if n < held.count {
        let entry = inv.bag.iter_mut().find(|b| b.item.uid == uid).ok_or("no such item in the backpack")?;
        entry.item.count -= n;
        return Ok(ItemInstance { uid: fresh, item: entry.item.item.clone(), count: n });
    }
    if let Some(b) = inv.take(uid) {
        return Ok(b.item);
    }
    let slot = inv.worn.iter().find(|(_, i)| i.uid == uid).map(|(s, _)| *s).ok_or("no such item")?;
    inv.worn.remove(&slot).ok_or("no such item")
}

/// Equip / Unequip / Move on an inventory outside the sim (the stash screen); refused commands change
/// nothing. `SetCharacter*` and `Give` are refused here.
pub fn edit_inventory(inv: &mut Inventory, data: &GameData, career: EnRoleCareerType, cmd: InventoryCommand) -> Result<(), &'static str> {
    if matches!(cmd, InventoryCommand::SetCharacter { .. } | InventoryCommand::SetCharacterItems { .. } | InventoryCommand::Give { .. } | InventoryCommand::GiveStarting { .. } | InventoryCommand::Drop { .. }) {
        return Err("not an edit");
    }
    let before = inv.clone();
    let r = edit(inv, data, career, cmd);
    if r.is_err() {
        *inv = before;
    }
    r
}

fn edit(inv: &mut Inventory, data: &GameData, career: EnRoleCareerType, cmd: InventoryCommand) -> Result<(), &'static str> {
    match cmd {
        InventoryCommand::Equip { uid, slot, .. } => {
            // The set's hands (the second set's too).
            let (right, left) = if matches!(slot, EnItemWearingPart::Pair2rightweapon | EnItemWearingPart::Pair2leftweapon) { (EnItemWearingPart::Pair2rightweapon, EnItemWearingPart::Pair2leftweapon) } else { (EnItemWearingPart::Pair1rightweapon, EnItemWearingPart::Pair1leftweapon) };
            let entry = inv.take(uid).ok_or("no such item in the backpack")?;
            let info = data.info(entry.item.item.item_id).ok_or("unknown item")?;
            equip_check(info, slot, career)?;
            let mut displaced: Vec<ItemInstance> = inv.worn.remove(&slot).into_iter().collect();
            let right_two_handed = inv.worn.get(&right).and_then(|i| data.info(i.item.item_id)).is_some_and(|i| i.two_handed);
            if info.two_handed && slot == right {
                displaced.extend(inv.worn.remove(&left));
            }
            if slot == left && right_two_handed {
                displaced.extend(inv.worn.remove(&right));
            }
            inv.worn.insert(slot, entry.item);
            // Displaced items go where the new one was if they fit there, else anywhere.
            for item in displaced {
                let size = data.info(item.item.item_id).map_or((1, 1), |i| i.size);
                let spot = if inv.fits(size, entry.x, entry.y, None, data) { Some((entry.x, entry.y)) } else { inv.first_fit(size, data) };
                let (x, y) = spot.ok_or("no room for the item taken off")?;
                inv.bag.push(BagEntry { item, x, y });
            }
            Ok(())
        }
        InventoryCommand::Destroy { uid, .. } => {
            let held = inv.bag.iter().map(|b| &b.item).chain(inv.worn.values()).find(|i| i.uid == uid).ok_or("no such item")?;
            if data.is_heirloom(held.item.item_id) {
                return Err("heirlooms can't be dropped");
            }
            if inv.take(uid).is_none() {
                let slot = inv.worn.iter().find(|(_, i)| i.uid == uid).map(|(s, _)| *s).ok_or("no such item")?;
                inv.worn.remove(&slot);
            }
            Ok(())
        }
        InventoryCommand::Unequip { slot, .. } => {
            let item = inv.worn.remove(&slot).ok_or("nothing worn there")?;
            inv.add(item, data).map_err(|_| "no room in the backpack")
        }
        InventoryCommand::Move { uid, x, y, .. } => {
            let mut entry = inv.take(uid).ok_or("no such item in the backpack")?;
            let size = data.info(entry.item.item.item_id).map_or((1, 1), |i| i.size);
            if !inv.fits(size, x, y, None, data) {
                return Err("doesn't fit there");
            }
            (entry.x, entry.y) = (x, y);
            inv.bag.push(entry);
            Ok(())
        }
        InventoryCommand::SetCharacter { .. } | InventoryCommand::SetCharacterItems { .. } | InventoryCommand::Give { .. } | InventoryCommand::GiveStarting { .. } | InventoryCommand::Drop { .. } => unreachable!("handled in run"),
    }
}

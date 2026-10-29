//! Loot in the world and consumables: chests and monster drops roll their loot sources into items on
//! the ground; players pick them up; potions apply their effects over time.
//!
//! CONFIRMED: loot tables for the sources that have them (`ExcelLootSourceMap` → … → `ExcelItemDropItem`); potion effects
//! as the item text states them (`Item_Description_100013`: 10 HP per second for 30 s; `Item_Description_100017`: a
//! shield absorbing 250 damage for 25 s, 20 s before drinking again, shared with alchemical potions). The potion
//! GameplayEffects themselves are blocked by the mappings. Drinking (CONFIRMED `GA_TGF_UseItem_Base` bytecode and
//! `AM_Potion_Sub_Drink_1st`): the ability plays the drink montage and applies the item at its `UseItem` notify (2.173
//! s), ending at `EndAbility` (3.0 s); while it runs it holds `TGF.Ability.InterruptState.NoActiveAbility`, which blocks
//! other abilities ([`Drinking`]). HYPOTHESIS: the potions play `AM_Potion_Sub_Drink_1st` (their anim definitions decode
//! empty; the only other drink, `AM_Potion_Sub_DrinkTest_1st` (6 s, use at 5 s, moving at 0.3), lies in a `_Test`
//! folder); the drink isn't broken off by hits; healing comes in one pulse per second; interaction range, drop spread and
//! the chest's size ([`crate::overrides::world_items`]).

use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::command::{ActorKind, Kind};
use crate::id::{SimIdAllocator, SimIndex};
use crate::inventory::{GameData, Inventory, ItemInstance, ItemUids};
use crate::items::Item;
use crate::loot::Drop;
use crate::melee::Health;
use crate::movement::static_box;
use crate::overrides::world_items as tune;
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng};

/// A chest (or any container) that rolls `source_id` once when opened.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Container {
    pub source_id: i32,
    pub opened: bool,
}

/// An item lying on the ground.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GroundItem(pub ItemInstance);

/// The loot source a monster drops when it dies (rolled once).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LootSource {
    pub source_id: i32,
    pub dropped: bool,
}

/// What a consumable does when used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Consumable {
    /// `per_second` health every second for `seconds`.
    Heal { per_second: f64, seconds: u32 },
    /// Absorbs `amount` damage for `seconds`; `cooldown` seconds before another such potion.
    Shield { amount: f64, seconds: f32, cooldown: f32 },
    /// `per_second` mana every second for `seconds`.
    Mana { per_second: f64, seconds: u32 },
}

/// Consumable effects by item id, from the item descriptions (CONFIRMED text, see the module docs).
pub fn consumable(item_id: i32) -> Option<Consumable> {
    match item_id {
        // 治疗药水: 饮用后每秒恢复10点生命值，持续30秒。
        100_013 => Some(Consumable::Heal { per_second: 10.0, seconds: 30 }),
        // 护盾魔药: 饮用后获得一个可以吸收250点任意伤害的护盾，持续25秒，需要间隔20秒才能再次饮用。
        100_017 => Some(Consumable::Shield { amount: 250.0, seconds: 25.0, cooldown: 20.0 }),
        // 法力药水: 饮用后每秒恢复10点魔法值，持续20秒。
        100_018 => Some(Consumable::Mana { per_second: 10.0, seconds: 20 }),
        _ => None,
    }
}

/// Effects running on an actor.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Effects {
    /// (health per pulse, pulses left, ticks to the next pulse)
    pub heal: Option<(f64, u32, u32)>,
    /// (absorb left, ticks left)
    pub shield: Option<(f64, u32)>,
    /// Ticks before another shield/alchemical potion.
    pub potion_cooldown: u32,
    /// (mana per pulse, pulses left, ticks to the next pulse)
    pub mana: Option<(f64, u32, u32)>,
}

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

/// Spawns a chest: a static box collider plus a `Container`; `pos` is the floor point under it.
pub(crate) fn spawn_chest(world: &mut World, pos: Vec3, source_id: i32) {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let half = Vec3::from(tune::CHEST_HALF_EXTENTS);
    let e = world.spawn((id, Kind(ActorKind::Chest), Position(pos + Vec3::Z * half.z), Container { source_id, opened: false })).id();
    // The collider is its own static body (positions of static bodies come from `Position`).
    world.entity_mut(e).insert(static_box(pos + Vec3::Z * half.z, half));
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Chest });
}

pub(crate) fn spawn_ground_item(world: &mut World, pos: Vec3, item: ItemInstance) -> SimId {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let e = world.spawn((id, Kind(ActorKind::Item), Position(pos), GroundItem(item))).id();
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Item });
    id
}

/// A freshly rolled item `item_id` (its affixes rolled, as loot) lying at floor point `pos`; refused without game
/// data or a known item.
pub(crate) fn place_item(world: &mut World, pos: Vec3, item_id: i32) {
    let items = world.contains_resource::<GameData>().then(|| {
        world.resource_scope(|world, data: Mut<GameData>| {
            let item = data.roll_item(item_id, &mut world.resource_mut::<SimRng>()).ok()?;
            Some(instances(world, &data, vec![Drop::Gear(item)]))
        })
    });
    match items.flatten().and_then(|i| i.into_iter().next()) {
        Some(item) => {
            spawn_ground_item(world, pos + Vec3::Z * tune::ITEM_HEIGHT, item);
        }
        None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown item" }),
    }
}

/// One of item `item_id` on the ground at `pos` (the zone's scrolls). None without game data or a known item.
pub(crate) fn spawn_item_by_id(world: &mut World, pos: Vec3, item_id: i32) -> Option<SimId> {
    let items = world.resource_scope(|world, data: Mut<GameData>| instances(world, &data, vec![Drop::Stack { item_id, count: 1 }]));
    items.into_iter().next().map(|item| spawn_ground_item(world, pos, item))
}

/// Turns a loot roll into item instances (stacks split at their stack limit).
fn instances(world: &mut World, data: &GameData, drops: Vec<Drop>) -> Vec<ItemInstance> {
    let mut out = Vec::new();
    for d in drops {
        let (item, mut count) = match d {
            Drop::Gear(item) => (item, 1),
            Drop::Stack { item_id, count } => match data.items.rarity(item_id) {
                Some(rarity) => (Item { item_id, rarity, affixes: Vec::new() }, count.max(1) as u32),
                None => continue,
            },
        };
        let stack = data.info(item.item_id).map_or(1, |i| u32::from(i.stack));
        while count > 0 {
            let n = count.min(stack);
            count -= n;
            out.push(ItemInstance { uid: world.resource_mut::<ItemUids>().alloc(), item: item.clone(), count: n as u16 });
        }
    }
    out
}

/// Rolls `source_id` and scatters the items around `at` (floor point). Returns the ground item ids.
pub(crate) fn drop_loot(world: &mut World, source_id: i32, at: Vec3) -> Vec<SimId> {
    // Source 0: a container that holds nothing (`BP_GoblinThief_TreasureChest_Wood_02`, in no loot table).
    if source_id == 0 {
        return Vec::new();
    }
    let roll = world.resource_scope(|world, data: Mut<GameData>| {
        let roll = data.loot.roll(source_id, &mut world.resource_mut::<SimRng>());
        roll.map(|r| instances(world, &data, r.drops))
    });
    let items = match roll {
        Ok(items) => items,
        Err(_) => {
            world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "loot source can't be rolled" });
            return Vec::new();
        }
    };
    scatter(world, at, items)
}

/// Puts items on the ground in a ring around `at` (floor point). Returns the ground item ids.
pub(crate) fn scatter(world: &mut World, at: Vec3, items: Vec<ItemInstance>) -> Vec<SimId> {
    let n = items.len().max(1) as f32;
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let a = std::f32::consts::TAU * i as f32 / n;
            let offset = Vec3::new(a.cos(), a.sin(), 0.0) * tune::DROP_SPREAD + Vec3::Z * tune::ITEM_HEIGHT;
            spawn_ground_item(world, at + offset, item)
        })
        .collect()
}

/// Opens a chest or picks up a ground item within reach of `id`.
pub(crate) fn interact(world: &mut World, id: SimId, target: SimId) {
    let index = world.resource::<SimIndex>();
    let (Some(e), Some(t)) = (index.get(id), index.get(target)) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
        return;
    };
    let (Some(p), Some(q)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Position>(t).map(|p| p.0)) else { return };
    let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
    if !alive || (q - p).length() > tune::INTERACT_RANGE {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "out of reach" });
        return;
    }
    // A dormant mimic opened: it wakes on the opener (`BP_TGF_CharacterWake` -> `Wake`).
    if let Some(mut ai) = world.get_mut::<crate::ai::MonsterAi>(t)
        && ai.wake_on_open
    {
        if ai.dormant {
            ai.provoked_by = Some(id);
        } else {
            world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "not a chest" });
        }
        return;
    }
    if let Some(c) = world.get::<Container>(t).copied() {
        if c.opened {
            world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "already open" });
            return;
        }
        if let Some(mut c) = world.get_mut::<Container>(t) {
            c.opened = true;
        }
        let half = Vec3::from(tune::CHEST_HALF_EXTENTS);
        // Spilled in front of the chest towards the opener, at floor height.
        let floor = q - Vec3::Z * half.z;
        let towards = (p - q).truncate().normalize_or_zero().extend(0.0);
        let items = drop_loot(world, c.source_id, floor + towards * (half.x + tune::DROP_SPREAD));
        world.resource_mut::<SimEvents>().push(SimEvent::ChestOpened { id, chest: target, items });
        // The opener's first-opening rolls.
        if let Some(oe) = world.resource::<crate::SimIndex>().get(id) {
            crate::gear_triggers::on_chest_opened(world, id, oe);
        }
        return;
    }
    if let Some(GroundItem(item)) = world.get::<GroundItem>(t).cloned() {
        let added = world.resource_scope(|world, data: Mut<GameData>| match world.get_mut::<Inventory>(e) {
            Some(mut inv) => inv.add(item.clone(), &data).map_err(|_| "no room in the backpack"),
            None => Err("actor has no inventory"),
        });
        match added {
            Ok(()) => {
                world.resource_mut::<SimIndex>().remove(target);
                world.despawn(t);
                let mut ev = world.resource_mut::<SimEvents>();
                ev.push(SimEvent::PickedUp { id, item: target, uid: item.uid });
                ev.push(SimEvent::Despawned { id: target });
                ev.push(SimEvent::InventoryChanged { id });
            }
            Err(reason) => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason }),
        }
        return;
    }
    if let Some(portal) = world.get::<crate::run::Portal>(t).copied() {
        crate::run::use_portal_at(world, id, e, Some(target), portal);
        return;
    }
    world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "nothing to interact with" });
}

/// A potion being drunk: its stack, its item, the ticks it takes effect and ends.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Drinking {
    pub uid: u32,
    pub item_id: i32,
    pub start: u64,
    pub use_at: u64,
    pub end: u64,
}

/// The drink montage's `UseItem` notify and its end (s), from `AM_Potion_Sub_Drink_1st`; `None` without the data
/// (the item then applies at once).
fn drink_timing() -> Option<(f32, f32)> {
    static TIMING: std::sync::OnceLock<Option<(f32, f32)>> = std::sync::OnceLock::new();
    *TIMING.get_or_init(|| {
        let m = crate::staffs::Montage::load("/Game/TGF/Character/Shared/Animation/1st/OtherItems/AM_Potion_Sub_Drink_1st").ok()?;
        Some((m.event("UseItem")?, m.event("EndAbility").unwrap_or(m.length)))
    })
}

/// Busy with a drink (no attacks, skills, throws or torch meanwhile).
pub fn drinking(world: &World, id: SimId) -> bool {
    world.resource::<SimIndex>().get(id).is_some_and(|e| world.get::<Drinking>(e).is_some())
}

/// Uses one of a backpack stack (potions): starts drinking it, the effect at the drink's `UseItem`.
pub(crate) fn use_item(world: &mut World, id: SimId, uid: u32) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    let Some(inv) = world.get::<Inventory>(e) else { return };
    let Some(entry) = inv.bag.iter().find(|b| b.item.uid == uid) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "no such item in the backpack" });
        return;
    };
    let item_id = entry.item.item.item_id;
    let Some(effect) = consumable(item_id) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "can't be used" });
        return;
    };
    if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "dead" });
        return;
    }
    if world.get::<Drinking>(e).is_some() || world.get::<crate::throwables::Throwing>(e).is_some() {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "busy" });
        return;
    }
    if matches!(effect, Consumable::Shield { .. }) && world.get::<Effects>(e).is_some_and(|fx| fx.potion_cooldown > 0) {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "potion on cooldown" });
        return;
    }
    if let Some((use_at, end)) = drink_timing() {
        let now = world.resource::<crate::TickCount>().0;
        // (A torch in hand is put away.)
        world.entity_mut(e).remove::<crate::throwables::HeldTorch>();
        world.entity_mut(e).insert(Drinking { uid, item_id, start: now, use_at: now + u64::from(ticks(use_at)), end: now + u64::from(ticks(end)) });
        world.resource_mut::<SimEvents>().push(SimEvent::DrinkStarted { id, item_id });
        return;
    }
    apply_item(world, id, e, uid, item_id, effect);
}

/// The drinks' `UseItem` (the item applied, one off its stack) and their ends.
fn drinks_tick(world: &mut World) {
    let now = world.resource::<crate::TickCount>().0;
    let drinkers: Vec<(SimId, Entity, Drinking)> = world.resource::<SimIndex>().iter().filter_map(|(id, e)| world.get::<Drinking>(e).map(|d| (id, e, *d))).collect();
    for (id, e, d) in drinkers {
        if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
            world.entity_mut(e).remove::<Drinking>();
            continue;
        }
        if now == d.use_at
            && let Some(effect) = consumable(d.item_id)
            && world.get::<Inventory>(e).is_some_and(|inv| inv.bag.iter().any(|b| b.item.uid == d.uid))
        {
            apply_item(world, id, e, d.uid, d.item_id, effect);
        }
        if now >= d.end {
            world.entity_mut(e).remove::<Drinking>();
        }
    }
}

/// The item's effect, one off its stack.
fn apply_item(world: &mut World, id: SimId, e: Entity, uid: u32, item_id: i32, effect: Consumable) {
    let mut fx = world.get::<Effects>(e).copied().unwrap_or_default();
    match effect {
        Consumable::Heal { per_second, seconds } => fx.heal = Some((per_second, seconds, ticks(1.0))),
        Consumable::Mana { per_second, seconds } => fx.mana = Some((per_second, seconds, ticks(1.0))),
        Consumable::Shield { amount, seconds, cooldown } => {
            if fx.potion_cooldown > 0 {
                world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "potion on cooldown" });
                return;
            }
            fx.shield = Some((amount, ticks(seconds)));
            fx.potion_cooldown = ticks(cooldown);
        }
    }
    world.entity_mut(e).insert(fx);
    if let Some(mut inv) = world.get_mut::<Inventory>(e)
        && let Some(i) = inv.bag.iter().position(|b| b.item.uid == uid)
    {
        if inv.bag[i].item.count > 1 {
            inv.bag[i].item.count -= 1;
        } else {
            inv.bag.remove(i);
        }
    }
    let mut ev = world.resource_mut::<SimEvents>();
    ev.push(SimEvent::ItemUsed { id, item_id });
    ev.push(SimEvent::InventoryChanged { id });
}

/// Gives the actor a shield absorbing `amount` for `seconds` (replacing any shield; Ice Armor).
pub(crate) fn give_shield(world: &mut World, e: Entity, amount: f64, seconds: f32) {
    let mut fx = world.get::<Effects>(e).copied().unwrap_or_default();
    fx.shield = Some((amount, ticks(seconds)));
    world.entity_mut(e).insert(fx);
}

/// Whether `e` owns [`crate::monsters::NO_DAMAGE`] now: its swing's window (the Shadow King's `Attack8`, 0-21.3 s), or
/// its wake clip's (his wake-up, 0-29.7 s).
fn no_damage(world: &World, e: Entity) -> bool {
    let tag = crate::buffs::gameplay_tag(crate::monsters::NO_DAMAGE);
    if world.get::<crate::Melee>(e).is_some_and(|m| m.swing_has_tag(tag)) {
        return true;
    }
    let (Some(ai), Some(kit)) = (world.get::<crate::ai::MonsterAi>(e), world.get::<crate::monsters::MonsterKit>(e)) else { return false };
    let (Some((false, end)), Some((from, to)), Some(wake)) = (ai.pose, kit.dormancy.wake_no_damage, kit.dormancy.wake.as_ref()) else { return false };
    let left = end.saturating_sub(world.resource::<crate::TickCount>().0 as u32);
    let at = wake.1 - left as f32 / crate::TICK_RATE_HZ as f32;
    at >= from && at < to
}

/// Damage to `target` after its shield: returns the health left (also writes it).
pub(crate) fn apply_damage(world: &mut World, target: SimId, te: Entity, damage: f64) -> f64 {
    // The Goblin Thief stepping into its portal takes no damage (`AM_Goblin_Thief_ExitDungeons`' `NoDamage`
    // notify state from its `Touch`, 2.7 s, to the end).
    if world.get::<crate::ai::MonsterAi>(te).is_some_and(|a| a.thief.past_touch) || no_damage(world, te) {
        return world.get::<Health>(te).map_or(0.0, |h| h.current);
    }
    let mut rest = damage;
    if let Some(mut fx) = world.get_mut::<Effects>(te)
        && let Some((left, t)) = fx.shield
    {
        let absorbed = left.min(rest);
        rest -= absorbed;
        let remaining = left - absorbed;
        fx.shield = (remaining > 0.0).then_some((remaining, t));
        world.resource_mut::<SimEvents>().push(SimEvent::ShieldAbsorbed { id: target, amount: absorbed, remaining });
    }
    let Some(mut h) = world.get_mut::<Health>(te) else { return 0.0 };
    h.current = (h.current - rest).max(0.0);
    h.current
}

/// Effects over time, monster drops on death; runs after combat each tick, in id order.
pub(crate) fn world_items_tick(world: &mut World) {
    drinks_tick(world);
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for &(id, e) in &ids {
        if let Some(mut fx) = world.get::<Effects>(e).copied() {
            let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
            if let Some((per, pulses, next)) = fx.heal {
                if !alive {
                    fx.heal = None;
                } else if next <= 1 {
                    // Grievous wounds (Rend) cut healing received.
                    let amount = per * world.get::<crate::buffs::Buffs>(e).map_or(1.0, crate::buffs::Buffs::heal_factor);
                    if let Some(mut h) = world.get_mut::<Health>(e) {
                        h.current = (h.current + amount).min(h.max);
                    }
                    world.resource_mut::<SimEvents>().push(SimEvent::Healed { id, amount });
                    fx.heal = (pulses > 1).then_some((per, pulses - 1, ticks(1.0)));
                } else {
                    fx.heal = Some((per, pulses, next - 1));
                }
            }
            if let Some((left, t)) = fx.shield {
                fx.shield = (t > 1 && alive).then_some((left, t - 1));
            }
            if let Some((per, pulses, next)) = fx.mana {
                if !alive {
                    fx.mana = None;
                } else if next <= 1 {
                    if let Some(mut pw) = world.get_mut::<crate::class_skills::Power>(e)
                        && pw.kind == crate::class_skills::PowerKind::Mana
                    {
                        pw.current = (pw.current + per).min(pw.max);
                    }
                    fx.mana = (pulses > 1).then_some((per, pulses - 1, ticks(1.0)));
                } else {
                    fx.mana = Some((per, pulses, next - 1));
                }
            }
            fx.potion_cooldown = fx.potion_cooldown.saturating_sub(1);
            world.entity_mut(e).insert(fx);
        }
        if let Some(src) = world.get::<LootSource>(e).copied()
            && !src.dropped
            && world.get::<Health>(e).is_some_and(|h| h.current <= 0.0)
        {
            if let Some(mut s) = world.get_mut::<LootSource>(e) {
                s.dropped = true;
            }
            // A broken prop's loot falls to its own floor point (`CollectableBaseFallToFloor`).
            let at = match world.get::<crate::destroyables::Destroyable>(e) {
                Some(d) => d.origin,
                None => world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0) - Vec3::Z * crate::movement::CAPSULE_HEIGHT * 0.5,
            };
            let items = drop_loot(world, src.source_id, at);
            world.resource_mut::<SimEvents>().push(SimEvent::LootDropped { id, items });
        }
    }
}

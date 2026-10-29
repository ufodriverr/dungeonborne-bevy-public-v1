//! The Goblin Thief. Its controller (`AIC_Goblin_Thief`) and abilities, CONFIRMED from their Blueprint
//! bytecode and defaults: it lies dormant and invisible until it first enters combat ("reveal"), wakes, then
//! runs from its threat in `Distance` hops; once no player is within `SafeRange` and `EscapeTime` has passed
//! since the reveal it opens an extraction portal (`GA_Goblin_Thief_OpenPortal`, cooldown on activation),
//! cheers and steps through (`GA_Goblin_Thief_EnterPortal`): gone, with nothing dropped. A hit during the
//! ritual (before the `Touch` notify) stops it. The first hit that leaves it under `AccelerateHealth` makes
//! it fast for a while (dropping a loot roll every `DropInterval`), then slow. It never attacks.
//!
//! The behaviour tree nodes live in [`crate::ai`]; this module holds the data, the per-monster state and
//! the world side (portal, escape, speed, drops) that the AI tick asks for.

use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::ai::MonsterAi;
use crate::monsters::{MonsterKit, asset_json, monster_json, montage_from_json, notify_window_at};
use crate::movement::MoveSpeed;
use crate::id::SimIndex;
use crate::melee::Health;
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng, TickCount};

/// One of the thief's abilities (`AbilitySet_M_GoblinThief`): its montage's length and the notify times
/// the ability acts on (s).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThiefAbility {
    /// Class name, as the tree's `BTT_Goblin_Thief_ActivateAbility` names it (`GA_Goblin_Thief_OpenPortal_C`).
    pub class: String,
    pub montage: String,
    pub length: f32,
    /// `CooldownGameplayEffectClass` duration, committed on activation.
    pub cooldown: f32,
    /// OpenPortal's `Notify` ("Open"): the portal appears.
    pub open: Option<f32>,
    /// EnterPortal's `NotifyTouch` ("Touch"): the portal closes, damage no longer interrupts.
    pub touch: Option<f32>,
    /// EnterPortal's `NotifyEnter` ("Enter"): the thief is gone.
    pub enter: Option<f32>,
    /// Its montage's sound cues (time s, Wwise event).
    pub sounds: Vec<(f32, String)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThiefKit {
    /// `EscapeTime` (s after the reveal before it may open a portal).
    pub escape_time: f32,
    /// `SafeRange` (cm): no player this close for the escape.
    pub safe_range: f32,
    /// `BTT_Goblin_Thief_FindRunAwayLocation.Distance` (cm).
    pub run_distance: f32,
    /// `AccelerateHealth` (health share).
    pub accelerate_health: f64,
    /// `AccelerateGE` (`MoveSpeedBase` change cm/s, duration s) and `TiredGE` applied `TiredTime` later.
    pub fast: (f32, f32),
    pub slow: (f32, f32),
    pub tired_time: f32,
    /// `DropItemGE` duration, and `GA_TGF_DropItem`'s `DropInterval` / `DropChance`.
    pub drop_time: f32,
    pub drop_interval: f32,
    pub drop_chance: f32,
    pub abilities: Vec<ThiefAbility>,
}

impl ThiefKit {
    /// From the monster folder's controller (an `AIC_*` with `EscapeTime`) and ability set; `None` for
    /// every other monster.
    pub fn load(dir: &str) -> Option<ThiefKit> {
        let root = db_formats::data_path("json/assets/Dungeonborne/Content/TGF/Character/Monster").join(dir);
        let mut aics: Vec<_> = std::fs::read_dir(root.join("AI")).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("AIC_"))).collect();
        aics.sort();
        let cdo_of = |v: &serde_json::Value| v.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__"))).map(|o| o["Properties"].clone());
        let aic = aics.iter().filter_map(|p| serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).ok()?).ok()).filter_map(|v| cdo_of(&v)).find(|p| p.get("EscapeTime").is_some())?;
        let f = |v: &serde_json::Value| v.as_f64().map(|x| x as f32);
        // A gameplay effect's first modifier magnitude and its duration.
        let ge = |key: &str| -> Option<(f32, f32)> {
            let p = cdo_of(&asset_json(aic[key]["ObjectPath"].as_str()?)?)?;
            let magnitude = p["Modifiers"].as_array().and_then(|m| f(&m.first()?["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"])).unwrap_or(0.0);
            Some((magnitude, f(&p["DurationMagnitude"]["ScalableFloatMagnitude"]["Value"])?))
        };
        let ability_cdo = |name: &str| std::fs::read(root.join("Ability").join(format!("{name}.json"))).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| cdo_of(&v));
        let run = std::fs::read(root.join("AI").join("BTT_Goblin_Thief_FindRunAwayLocation.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| cdo_of(&v));
        let drop = ability_cdo("GA_TGF_DropItem");
        let mut kit = ThiefKit {
            escape_time: f(&aic["EscapeTime"])?,
            safe_range: f(&aic["SafeRange"])?,
            run_distance: run.as_ref().and_then(|p| f(&p["Distance"]))?,
            accelerate_health: aic["AccelerateHealth"].as_f64()?,
            fast: ge("AccelerateGE")?,
            slow: ge("TiredGE")?,
            tired_time: f(&aic["TiredTime"])?,
            drop_time: ge("DropItemGE")?.1,
            drop_interval: drop.as_ref().and_then(|p| f(&p["DropInterval"]))?,
            drop_chance: drop.as_ref().and_then(|p| f(&p["DropChance"]))?,
            abilities: Vec::new(),
        };
        // Every granted ability that plays a montage.
        let mut sets: Vec<_> = std::fs::read_dir(root.join("Ability")).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("AbilitySet_M_"))).collect();
        sets.sort();
        let set: serde_json::Value = serde_json::from_slice(&std::fs::read(sets.first()?).ok()?).ok()?;
        let granted = set.as_array()?.iter().find(|o| o["Type"] == "LyraAbilitySet")?["Properties"]["GrantedGameplayAbilities"].as_array()?.clone();
        for g in &granted {
            let Some(path) = g["Ability"]["ObjectPath"].as_str() else { continue };
            let Some(p) = monster_json(path).and_then(|v| cdo_of(&v)) else { continue };
            let Some(v) = p["CastMontageSet"]["DefaultMontage"]["ObjectPath"].as_str().and_then(monster_json) else { continue };
            let Ok(m) = montage_from_json(&v) else { continue };
            let Some(stem) = path.rsplit('/').next().and_then(|s| s.split('.').next()) else { continue };
            let class = format!("{stem}_C");
            let at = |key: &str| p[key].as_str().and_then(|n| notify_window_at(&v, n));
            let cooldown = p["CooldownGameplayEffectClass"]["ObjectPath"].as_str().and_then(asset_json).and_then(|v| cdo_of(&v)).and_then(|c| f(&c["DurationMagnitude"]["ScalableFloatMagnitude"]["Value"])).unwrap_or(0.0);
            kit.abilities.push(ThiefAbility { class, montage: m.name, length: m.length, cooldown, open: at("Notify"), touch: at("NotifyTouch"), enter: at("NotifyEnter"), sounds: m.sounds });
        }
        Some(kit)
    }

    pub fn ability(&self, class: &str) -> Option<usize> {
        self.abilities.iter().position(|a| a.class == class)
    }
}

/// An ability in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Active {
    pub ability: usize,
    pub start: u32,
    /// Its notify has acted (the portal opened, or the thief touched it).
    pub fired: bool,
}

/// What the AI tick asks the world to do (it can't spawn or despawn itself).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
    /// An extraction portal at this floor point.
    OpenPortal(Vec3),
    ClosePortal,
    /// Gone through the portal: removed without death or loot.
    Escape,
}

/// The controller's state (`AIC_Goblin_Thief`: `Revealed`, `RevealedTime`, `Escaped`, `Accelerated`,
/// `Portal`) plus the ability in progress and its cooldowns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThiefState {
    pub revealed_at: Option<u32>,
    pub escaped: bool,
    /// The tick the accelerate hit landed.
    pub accelerated_at: Option<u32>,
    pub active: Option<Active>,
    /// Past EnterPortal's `Touch`: damage no longer stops it.
    pub past_touch: bool,
    /// Ability index -> tick its cooldown ends.
    pub ready_at: Vec<(usize, u32)>,
    pub portal: Option<SimId>,
    pub requests: Vec<Request>,
    /// The curve's move speed, before the speed effects.
    pub base_speed: Option<f32>,
}

impl ThiefState {
    pub fn ready(&self, ability: usize, now: u32) -> bool {
        self.ready_at.iter().all(|&(a, t)| a != ability || now >= t)
    }
}

/// OpenPortal puts the portal this far ahead of the thief (cm; its `Open` handler's trace length and
/// offset, CONFIRMED from the bytecode), or at the wall if one is nearer.
pub const PORTAL_AHEAD: f32 = 150.0;

fn ticks(s: f32) -> u32 {
    (s.max(0.0) / DT).round() as u32
}

/// The world side, after the AI tick: portals and escapes it asked for, the speed effects, the loot spill.
pub(crate) fn thief_world(world: &mut World) {
    let now = world.resource::<TickCount>().0 as u32;
    let thieves: Vec<(SimId, Entity)> = {
        let index = world.resource::<SimIndex>();
        index.iter().filter(|(_, e)| world.get::<MonsterKit>(*e).is_some_and(|k| k.thief.is_some())).collect()
    };
    for (id, e) in thieves {
        let Some(kit) = world.get::<MonsterKit>(e).and_then(|k| k.thief.as_deref().cloned()) else { continue };
        let Some(mut ai) = world.get_mut::<MonsterAi>(e) else { continue };
        let requests = std::mem::take(&mut ai.thief.requests);
        let accelerated = ai.thief.accelerated_at;
        let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
        let at = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0) - Vec3::Z * crate::movement::CAPSULE_HEIGHT * 0.5;
        for r in requests {
            match r {
                Request::OpenPortal(p) => {
                    let portal = crate::run::spawn_portal(world, p, crate::dungeon::PortalKind::Extract, None, 0.0);
                    if let Some(mut ai) = world.get_mut::<MonsterAi>(e) {
                        ai.thief.portal = Some(portal);
                    }
                }
                Request::ClosePortal => close_portal(world, e),
                Request::Escape => {
                    close_portal(world, e);
                    world.resource_mut::<SimIndex>().remove(id);
                    world.despawn(e);
                    world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id });
                }
            }
        }
        if world.get_entity(e).is_err() || !alive {
            continue;
        }
        // Speed: the curve's, + `AccelerateGE` for its duration, then `TiredGE` from `TiredTime` on.
        let Some(base) = world.get::<MoveSpeed>(e).map(|s| s.0) else { continue };
        let base = match world.get_mut::<MonsterAi>(e) {
            Some(mut a) => *a.thief.base_speed.get_or_insert(base),
            None => base,
        };
        let mut speed = base;
        if let Some(t) = accelerated {
            let (fast_until, tired_at) = (t + ticks(kit.fast.1), t + ticks(kit.tired_time));
            if now < fast_until {
                speed += kit.fast.0;
            }
            if now >= tired_at && now < tired_at + ticks(kit.slow.1) {
                speed += kit.slow.0;
            }
            // `GA_TGF_DropItem`: a loot roll every `DropInterval` while `DropItemGE` lasts.
            let every = ticks(kit.drop_interval).max(1);
            let since = now.saturating_sub(t);
            if now > t
                && since % every == 0
                && since <= ticks(kit.drop_time)
                && world.resource_mut::<SimRng>().unit_f32() <= kit.drop_chance
                && let Some(src) = world.get::<crate::LootSource>(e).copied()
            {
                let items = crate::world_items::drop_loot(world, src.source_id, at);
                world.resource_mut::<SimEvents>().push(SimEvent::LootDropped { id, items });
            }
        }
        if let Some(mut s) = world.get_mut::<MoveSpeed>(e) {
            s.0 = speed.max(0.0);
        }
    }
}

fn close_portal(world: &mut World, thief: Entity) {
    let Some(portal) = world.get_mut::<MonsterAi>(thief).and_then(|mut a| a.thief.portal.take()) else { return };
    if let Some(p) = world.resource_mut::<SimIndex>().remove(portal) {
        world.despawn(p);
        world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id: portal });
    }
}

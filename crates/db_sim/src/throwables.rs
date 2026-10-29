//! The alchemical flasks: Incendiary, Thunderclap, Smokescreen and Stoneshaping (each with its "Competitive"
//! twin), and the torches.
//!
//! CONFIRMED (defaults and bytecode of `GA_TGF_ThrowItemAlongPath_Base`, its montage and the flasks' packages):
//! - The throw plays `AM_AlchemicalPotion_Sub_Throw_1st` (0.667 s); at its `ThrowItem` notify (0.204 s) the ability
//!   commits its cooldown (`GE_TGF_ThrowItemAlongPath_Cooldown`, 20 s), takes one flask off the stack and launches
//!   the flask's `SpawnActor` from the actor `BornUpAdjust` (80) up and `BornRightAdjust` (50) right along the camera,
//!   at the camera's pitch + `PitchAdjust` (30, at most 89) and yaw + `YawAdjust` (-2), at `VelocityMag` (1200 cm/s),
//!   with its movement's gravity scale (x 980 cm/s²); it is gone after its `MaxExistTime` without effect.
//! - Thunderclap: an explosive projectile; its `GameplayEffectClass` effects (80 fixed damage, move speed x 0.5 for 6
//!   s) go to the characters within its `ExplodeRadius` (200 cm), the thrower too (`CanHitInstigator`).
//! - Incendiary / Smokescreen: on its first hit the flask puts its `BurningEffectTemplate` on the ground under the hit
//!   (`FindProperGroundPos`, 500 cm). That area lives its `MaxExistTime` (10 s; the smoke's 8 s) and every `DeltaTime`
//!   (0.15 s; 0.5 s) applies its `GameplayEffectClass` to the characters within its `ExplodeRadius` (280 cm; 600 cm):
//!   the fire's `GE_BurningPotion` (5 fixed damage every `Period` 0.5 s for 8 s); the smoke's list is empty.
//! - Stoneshaping (`B_TGF_AlchemicalPotion_EarthPotion` bytecode): on its hit, `SpawnNum` (4) `EarthTemplate` walls on
//!   the ground under it, `SpawnSize` (200 cm) apart across the line from the thrower (at -1.5, -0.5, 0.5, 1.5 times
//!   it), turned along that line; they die with the bottle (`ReceiveDestroyed`), its `MaxExistTime` (10 s) after the
//!   throw. A wall (`BP_Destructible_Wall`) is a breakable prop: `TotalHP` 150, its `TraceBox` its hit box.
//! - Torches (`GA_TGF_ThrowTorch`, its base `GA_TGF_ThrowItem_Base` bytecode): no projectile class and no cooldown;
//!   at `AM_Torch_Sub_Throw_1st`'s `ThrowItem` and a 0.05 s delay (`DoThrowTorch`) one torch of the stack is launched
//!   as its world pickup (`BP_CollectableBase_Torch`) at `ImpluseForce` along the camera pitched up `PitchAdjust`,
//!   from `BornUpAdjust` / `BornRightAdjust`; it lies lit where it stops and can be picked up again. The port flies it
//!   as a flask and lays it where it first touches (HYPOTHESIS: its bounce 0.1 and friction 0.5 left out).
//! - A torch is held (`WID_Torch_001`, a weapon; `AbilitySet_Torch`): its throw on the right button's input tags
//!   (`FireRight`), `GA_TGF_BrightTorch` on the left's (`FireLeft`, `ChargeLeft`), and the tag
//!   `TGF.Ability.BuffState.Lighting` while held. `GA_TGF_BrightTorch_Base` (bytecode) raises it while the input is
//!   held (`TGF.Ability.BrightTorch`, the montage's start section) and lowers it on release, but not before
//!   `MinHoldTime` (0.75 s on `GA_TGF_BrightTorch`): a shorter press waits out the rest on a timer.
//!
//! HYPOTHESIS:
//! - The flasks share the one cooldown (they all commit the same effect and its tag).
//! - The burn's healing cut is `GE_Wound_<its name>` beside it (`HealWoundRate` +0.5 for 8 s): the burn imports it, the
//!   property holding it doesn't decode.
//! - The burn is one per character: the area's ticks refresh its 8 s, its first damage is on application.
//! - The radii are spheres around the blast point (the area's ground point) touching the character's capsule.
//! - The port throws from a key (3 to 5: Incendiary, Thunderclap, Smokescreen) instead of holding the flask in a quick
//!   slot and clicking (the slots aren't modelled). The torch is held: key 7 takes it out and puts it away
//!   ([`HeldTorch`]); the attack button throws it, the block button raises it; the weapon can't swing meanwhile.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context;
use bevy_ecs::prelude::*;
use bevy_math::{Dir3, Vec2, Vec3};
use db_formats::config::ConfigTables;
use serde_json::Value;

use crate::buffs::{Buff, BuffKind, Buffs};
use crate::id::SimIndex;
use crate::melee::{Aim, Health};
use crate::movement::CAPSULE_RADIUS;
use crate::skills::{HitSource, SkillKind};
use crate::staffs::{Montage, chain, obj_path, package_of, prop, sub_prop};
use crate::{DT, Position, SimEvent, SimEvents, SimId, TickCount};

/// `FindProperGroundPos`'s depth in the flasks' bytecode, cm.
const GROUND_DEPTH: f32 = 500.0;
/// The engine's gravity, cm/s² (scaled by a projectile's `ProjectileGravityScale`).
const GRAVITY: f32 = 980.0;
/// The flask's `BoxCollision` half extent, cm.
const FLASK_RADIUS: f32 = 5.0;

/// A burn: fixed damage every `period` s for `secs`, with healing received cut by `wound` (a rate, for `wound_secs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dot {
    pub damage: f64,
    pub period: f32,
    pub secs: f32,
    pub wound: Option<(f64, f32)>,
}

/// What a flask does where it lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlaskEffect {
    /// One blast: fixed damage and a slow (move speed x `1 - rate` for s) within `radius`.
    Blast { radius: f32, damage: f64, slow: Option<(f64, f32)> },
    /// An area on the ground for `secs` applying `dot` every `every` s within `radius` (none: smoke only).
    Area { radius: f32, secs: f32, every: f32, dot: Option<Dot> },
    /// `count` walls `spacing` cm apart across the throw, there until the flask's lifetime from the throw ends.
    Walls { count: u32, spacing: f32 },
    /// The torch itself, lit on the ground where it lands (a pickup again).
    Torch,
}

/// A thrown torch lying lit (its pickup's `PointLight1` and fire on, `LaunchCollectable`'s `EnableLight`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lit;

/// `DoThrowTorch`'s delay between the montage's `ThrowItem` and the launch, s (CONFIRMED bytecode).
const TORCH_DELAY: f32 = 0.05;

/// One flask item's throw.
#[derive(Debug, Clone, PartialEq)]
pub struct FlaskSpec {
    /// The flask's name in the data (`ThunderPotion_0101`).
    pub name: String,
    /// The `ThrowItem` notify and the montage's end, s.
    pub release_at: f32,
    pub length: f32,
    pub up: f32,
    pub right: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub speed: f32,
    pub gravity: f32,
    pub lifetime: f32,
    pub cooldown: f32,
    pub effect: FlaskEffect,
    /// The walls' prop (Stoneshaping).
    pub wall: Option<Arc<crate::destroyables::DestroyableSpec>>,
    /// A torch's raise: `GA_TGF_BrightTorch`'s `MinHoldTime` (s); 0 for a flask.
    pub raise_min: f32,
}

impl FlaskSpec {
    /// Its kind: the name without its version (`ThunderPotion_0101` -> `ThunderPotion`).
    pub fn kind(&self) -> &str {
        self.name.split('_').next().unwrap_or(&self.name)
    }
}

/// The flasks by item id.
#[derive(Resource, Debug, Default, Clone)]
pub struct FlaskDb(pub BTreeMap<i32, Arc<FlaskSpec>>);

/// A throw in progress: the flask's stack, its item, the release and end ticks.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Throwing {
    pub uid: u32,
    pub item_id: i32,
    pub release: u64,
    pub end: u64,
}

/// A torch in the hand (its stack's uid and item) instead of the weapon; `raised` the tick it went up, `raise_held`
/// whether the raise input is still down.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct HeldTorch {
    pub uid: u32,
    pub item_id: i32,
    pub since: u64,
    pub raised: Option<u64>,
    pub raise_held: bool,
}

/// The tick the flasks' shared cooldown ends.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct FlaskCooldown(pub u64);

/// A flask's area on the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct FlaskArea {
    pub owner: SimId,
    pub item_id: i32,
    pub at: Vec3,
    pub radius: f32,
    pub born: u64,
    pub until: u64,
    pub every: u32,
    pub dot: Option<Dot>,
}

#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct FlaskAreas(pub Vec<FlaskArea>);

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

/// A fixed-damage effect's damage (`TgfFixedDamageExecution`'s first modifier).
fn fixed_damage_of(ge: &[crate::staffs::Bp]) -> Option<f64> {
    prop(ge, "Executions")?.as_array()?.iter().find(|x| x["CalculationClass"]["ObjectName"].as_str().is_some_and(|c| c.contains("TgfFixedDamageExecution")))?["CalculationModifiers"].as_array()?.first()?["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()
}

fn duration_of(ge: &[crate::staffs::Bp]) -> Option<f32> {
    prop(ge, "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64().map(|v| v as f32)
}

fn effects_of(c: &[crate::staffs::Bp]) -> Vec<String> {
    prop(c, "GameplayEffectClass").and_then(Value::as_array).into_iter().flatten().filter_map(obj_path).collect()
}

/// A burn effect (`GE_BurningPotion_0101`) and its wound beside it.
fn dot_of(ge: &str) -> anyhow::Result<Dot> {
    let c = chain(ge);
    let damage = fixed_damage_of(&c).with_context(|| format!("{ge}: no fixed damage"))?;
    let period = prop(&c, "Period").and_then(|p| p["Value"].as_f64()).with_context(|| format!("{ge}: no Period"))? as f32;
    let secs = duration_of(&c).with_context(|| format!("{ge}: no duration"))?;
    let (dir, name) = ge.rsplit_once('/').context("effect path")?;
    let suffix = name.strip_prefix("GE_").unwrap_or(name);
    let w = chain(&format!("{dir}/GE_Wound_{suffix}"));
    let wound = prop(&w, "Modifiers").and_then(Value::as_array).and_then(|m| m.iter().find(|m| m["Attribute"]["AttributeName"] == "HealWoundRate")?["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()).zip(duration_of(&w));
    Ok(Dot { damage, period, secs, wound })
}

/// One flask from its throw ability (`GA_TGF_Throw<X>Potion_<n>`).
fn flask_of(ga: &str) -> anyhow::Result<FlaskSpec> {
    let c = chain(ga);
    let num = |k: &str| prop(&c, k).and_then(Value::as_f64).map(|v| v as f32).with_context(|| format!("{ga}: no {k}"));
    let montage = Montage::load("/Game/TGF/Character/Shared/Animation/1st/OtherItems/AM_AlchemicalPotion_Sub_Throw_1st")?;
    let release_at = montage.event("ThrowItem").context("AM_AlchemicalPotion_Sub_Throw_1st: no ThrowItem notify")?;
    let cooldown = prop(&c, "CooldownGameplayEffectClass").and_then(obj_path).and_then(|g| duration_of(&chain(&g))).with_context(|| format!("{ga}: no cooldown"))?;
    let shot = prop(&c, "SpawnActor").and_then(obj_path).with_context(|| format!("{ga}: no SpawnActor"))?;
    let p = chain(&shot);
    let lifetime = prop(&p, "MaxExistTime").and_then(Value::as_f64).with_context(|| format!("{shot}: no MaxExistTime"))? as f32;
    let gravity = sub_prop(&p, "ProjectileMovementComponent", "ProjectileGravityScale").and_then(Value::as_f64).unwrap_or(1.0) as f32 * GRAVITY;
    let mut wall = None;
    let effect = if let Some(area) = prop(&p, "BurningEffectTemplate").and_then(obj_path) {
        let a = chain(&area);
        let anum = |k: &str| prop(&a, k).and_then(Value::as_f64).map(|v| v as f32).with_context(|| format!("{area}: no {k}"));
        let dot = effects_of(&a).first().map(|g| dot_of(g)).transpose()?;
        FlaskEffect::Area { radius: anum("ExplodeRadius")?, secs: anum("MaxExistTime")?, every: anum("DeltaTime")?, dot }
    } else if let Some(class) = prop(&p, "EarthTemplate").and_then(obj_path) {
        let count = prop(&p, "SpawnNum").and_then(Value::as_u64).with_context(|| format!("{shot}: no SpawnNum"))? as u32;
        let spacing = prop(&p, "SpawnSize").and_then(Value::as_f64).with_context(|| format!("{shot}: no SpawnSize"))? as f32;
        wall = Some(Arc::new(wall_of(&class)?));
        FlaskEffect::Walls { count, spacing }
    } else {
        let radius = prop(&p, "ExplodeRadius").and_then(Value::as_f64).with_context(|| format!("{shot}: no ExplodeRadius"))? as f32;
        let ges = effects_of(&p);
        let damage = ges.iter().find_map(|g| fixed_damage_of(&chain(g))).with_context(|| format!("{shot}: no fixed damage effect"))?;
        let slow = ges.iter().find_map(|g| crate::monsters::ge_slow(g));
        FlaskEffect::Blast { radius, damage, slow }
    };
    let name = ga.rsplit('/').next().unwrap_or(ga).trim_start_matches("GA_TGF_Throw").to_string();
    Ok(FlaskSpec { name, release_at, length: montage.length, up: num("BornUpAdjust")?, right: num("BornRightAdjust")?, pitch: num("PitchAdjust")?, yaw: num("YawAdjust")?, speed: num("VelocityMag")?, gravity, lifetime, cooldown, effect, wall, raise_min: 0.0 })
}

/// A torch's throw (`GA_TGF_ThrowTorch`) and raise (`GA_TGF_BrightTorch` beside it).
fn torch_of(ga: &str) -> anyhow::Result<FlaskSpec> {
    let c = chain(ga);
    let num = |k: &str| prop(&c, k).and_then(Value::as_f64).map(|v| v as f32).with_context(|| format!("{ga}: no {k}"));
    let montage = Montage::load("/Game/TGF/Character/Shared/Animation/1st/OtherItems/AM_Torch_Sub_Throw_1st")?;
    let at = montage.event("ThrowItem").context("AM_Torch_Sub_Throw_1st: no ThrowItem notify")?;
    let name = ga.rsplit('/').next().unwrap_or(ga).trim_start_matches("GA_TGF_Throw").to_string();
    let release_at = at + num("ThrowDelayTime").unwrap_or(0.0) + TORCH_DELAY;
    let bright = format!("{}/GA_TGF_BrightTorch", ga.rsplit_once('/').map_or("", |(d, _)| d));
    let raise_min = prop(&chain(&bright), "MinHoldTime").and_then(Value::as_f64).with_context(|| format!("{bright}: no MinHoldTime"))? as f32;
    Ok(FlaskSpec { name, release_at, length: montage.length.max(release_at), up: num("BornUpAdjust")?, right: num("BornRightAdjust")?, pitch: num("PitchAdjust")?, yaw: 0.0, speed: num("ImpluseForce")?, gravity: GRAVITY, lifetime: crate::overrides::throwables::TORCH_FLIGHT, cooldown: 0.0, effect: FlaskEffect::Torch, wall: None, raise_min })
}

/// A wall prop (`BP_Destructible_Wall`): its `InteractorBattleComponent`'s `TotalHP`, its `TraceBox`, its `DeathSound`.
fn wall_of(class: &str) -> anyhow::Result<crate::destroyables::DestroyableSpec> {
    let v = crate::staffs::read_json("json/loose", class).with_context(|| format!("{class} not decoded"))?;
    let objs = v.as_array().context("wall JSON")?;
    let named = |n: &str| objs.iter().find(|o| o["Name"].as_str().is_some_and(|x| x.starts_with(n))).map(|o| &o["Properties"]);
    let hp = objs.iter().find_map(|o| (o["Type"] == "TgfInteractorBattleComponent").then(|| o["Properties"]["TotalHP"].as_f64()).flatten()).with_context(|| format!("{class}: no TotalHP"))?;
    let tb = named("TraceBox").with_context(|| format!("{class}: no TraceBox"))?;
    let v3 = |x: &Value| Vec3::new(x["X"].as_f64().unwrap_or(0.0) as f32, x["Y"].as_f64().unwrap_or(0.0) as f32, x["Z"].as_f64().unwrap_or(0.0) as f32);
    let sound = named("Default__").and_then(|p| p["DeathSound"]["ObjectName"].as_str()).and_then(|s| s.split('\'').nth(1)).unwrap_or_default().to_string();
    let name = class.rsplit('/').next().unwrap_or(class).to_string();
    Ok(crate::destroyables::DestroyableSpec { class: name, model: String::new(), scale: Vec3::ONE, center: v3(&tb["RelativeLocation"]), half: v3(&tb["BoxExtent"]), hp, sound, source: 0 })
}

/// Every throwable flask item (`EN_ITEM_MINOR_TYPE_CONSUMABLES_THROWABLE`) whose weapon definition's folder has a
/// throw ability; what can't be read is listed.
pub fn load_flask_db(t: &ConfigTables) -> (FlaskDb, Vec<String>) {
    let mut db = FlaskDb::default();
    let mut skipped = Vec::new();
    for item in t.excel_item_type.iter().filter(|i| i.item_minor_type == db_formats::config::generated::EnItemMinorType::ConsumablesThrowable) {
        let Some(wid) = t.uesource_item_type.iter().find(|u| u.type_id == item.ue_source_type_id).and_then(|u| t.ue_source_equipment.iter().find(|r| r.equip_id == u.ue_source_equipment_equip_id)).map(|r| package_of(&r.ue_path)) else {
            skipped.push(format!("{}: no weapon definition (a torch)", item.type_id));
            continue;
        };
        let Some((dir, _)) = wid.rsplit_once('/') else { continue };
        let Some(rel) = dir.strip_prefix("/Game/") else { continue };
        let throw_in = |rel: &str| std::fs::read_dir(db_formats::data_path(format!("json/loose/Dungeonborne/Content/{rel}"))).into_iter().flatten().flatten().map(|d| d.file_name().to_string_lossy().into_owned()).find_map(|f| f.strip_suffix(".json").filter(|s| s.starts_with("GA_TGF_Throw")).map(str::to_string));
        // A flask's throw ability lies beside its weapon definition; a torch's (`AbilitySet_Torch`'s) one folder up.
        let found = match throw_in(rel) {
            Some(ga) => Some((format!("{dir}/{ga}"), false)),
            None => dir.rsplit_once('/').and_then(|(up, _)| Some((up, throw_in(up.strip_prefix("/Game/")?)?))).map(|(up, ga)| (format!("{up}/{ga}"), true)),
        };
        let Some((ga, torch)) = found else {
            skipped.push(format!("{}: no throw ability beside {wid}", item.type_id));
            continue;
        };
        match if torch { torch_of(&ga) } else { flask_of(&ga) } {
            Ok(f) => {
                db.0.insert(item.type_id, Arc::new(f));
            }
            Err(e) => skipped.push(format!("{}: {e:#}", item.type_id)),
        }
    }
    (db, skipped)
}

/// `SimCommand::ThrowItem`: starts throwing one of a backpack stack of flasks.
pub(crate) fn throw(world: &mut World, id: SimId, uid: u32) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    let reject = |world: &mut World, reason: &'static str| world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason });
    let Some(item_id) = world.get::<crate::Inventory>(e).and_then(|inv| inv.bag.iter().find(|b| b.item.uid == uid)).map(|b| b.item.item.item_id) else {
        return reject(world, "no such item in the backpack");
    };
    let Some(spec) = world.get_resource::<FlaskDb>().and_then(|db| db.0.get(&item_id).cloned()) else {
        return reject(world, "can't be thrown");
    };
    let now = world.resource::<TickCount>().0;
    if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
        return reject(world, "dead");
    }
    if world.get::<Throwing>(e).is_some() {
        return reject(world, "already throwing");
    }
    // The flasks share one cooldown; a torch has none and doesn't wait for it.
    if spec.cooldown > 0.0 && world.get::<FlaskCooldown>(e).is_some_and(|c| now < c.0) {
        return reject(world, "flask on cooldown");
    }
    world.entity_mut(e).insert(Throwing { uid, item_id, release: now + u64::from(ticks(spec.release_at)), end: now + u64::from(ticks(spec.length)) });
    world.resource_mut::<SimEvents>().push(SimEvent::ThrowStarted { id, item_id });
}

/// `SimCommand::TorchKey`: takes the first torch of the backpack in hand, or puts the held one away.
pub(crate) fn torch_key(world: &mut World, id: SimId) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    let reject = |world: &mut World, reason: &'static str| world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason });
    if world.get::<HeldTorch>(e).is_some() {
        world.entity_mut(e).remove::<HeldTorch>();
        world.resource_mut::<SimEvents>().push(SimEvent::TorchHeld { id, on: false });
        return;
    }
    if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
        return reject(world, "dead");
    }
    if world.get::<Throwing>(e).is_some() {
        return reject(world, "already throwing");
    }
    let torch = world.get_resource::<FlaskDb>().and_then(|db| {
        let inv = world.get::<crate::Inventory>(e)?;
        inv.bag.iter().find(|b| db.0.get(&b.item.item.item_id).is_some_and(|f| f.effect == FlaskEffect::Torch)).map(|b| (b.item.uid, b.item.item.item_id))
    });
    let Some((uid, item_id)) = torch else { return reject(world, "no torch") };
    let now = world.resource::<TickCount>().0;
    world.entity_mut(e).insert(HeldTorch { uid, item_id, since: now, raised: None, raise_held: false });
    world.resource_mut::<SimEvents>().push(SimEvent::TorchHeld { id, on: true });
}

/// The attack button with a torch held: throws it (the hand is empty after). False when no torch is held.
pub(crate) fn torch_attack(world: &mut World, id: SimId) -> bool {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return false };
    let Some(held) = world.get::<HeldTorch>(e).copied() else { return false };
    if held.raised.is_none() && world.get::<Throwing>(e).is_none() {
        world.entity_mut(e).remove::<HeldTorch>();
        throw(world, id, held.uid);
    }
    true
}

/// The block button with a torch held: raises it while down; once let go it comes down after `raise_min` from the
/// raise. False when no torch is held.
pub(crate) fn torch_block(world: &mut World, id: SimId, on: bool) -> bool {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return false };
    let now = world.resource::<TickCount>().0;
    let Some(mut held) = world.get_mut::<HeldTorch>(e) else { return false };
    held.raise_held = on;
    if on && held.raised.is_none() {
        held.raised = Some(now);
    }
    true
}

/// The throws' releases, the held torches' raises, then the flask areas.
pub fn throwables_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let held: Vec<(Entity, HeldTorch)> = world.query::<(Entity, &HeldTorch)>().iter(world).map(|(e, h)| (e, *h)).collect();
    for (e, h) in held {
        let gone = world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) || world.get::<crate::Inventory>(e).is_none_or(|inv| !inv.bag.iter().any(|b| b.item.uid == h.uid));
        if gone {
            world.entity_mut(e).remove::<HeldTorch>();
            continue;
        }
        let min = world.get_resource::<FlaskDb>().and_then(|db| db.0.get(&h.item_id)).map_or(0.0, |f| f.raise_min);
        if let Some(up) = h.raised
            && !h.raise_held
            && now >= up + u64::from(ticks(min))
            && let Some(mut t) = world.get_mut::<HeldTorch>(e)
        {
            t.raised = None;
        }
    }
    let throwers: Vec<(SimId, Entity, Throwing)> = world.resource::<SimIndex>().iter().filter_map(|(id, e)| world.get::<Throwing>(e).map(|t| (id, e, *t))).collect();
    for (id, e, t) in throwers {
        if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
            world.entity_mut(e).remove::<Throwing>();
            continue;
        }
        if now == t.release {
            release(world, id, e, t);
        }
        if now >= t.end {
            world.entity_mut(e).remove::<Throwing>();
        }
    }
    areas_tick(world, now);
}

/// The `ThrowItem` notify: the cooldown, one flask off the stack, the flask in the air.
fn release(world: &mut World, id: SimId, e: Entity, t: Throwing) {
    let Some(spec) = world.get_resource::<FlaskDb>().and_then(|db| db.0.get(&t.item_id).cloned()) else { return };
    let Some(mut inv) = world.get_mut::<crate::Inventory>(e) else { return };
    let Some(i) = inv.bag.iter().position(|b| b.item.uid == t.uid) else {
        world.entity_mut(e).remove::<Throwing>();
        return;
    };
    if inv.bag[i].item.count > 1 {
        inv.bag[i].item.count -= 1;
    } else {
        inv.bag.remove(i);
    }
    let now = world.resource::<TickCount>().0;
    if spec.cooldown > 0.0 {
        world.entity_mut(e).insert(FlaskCooldown(now + u64::from(ticks(spec.cooldown))));
    }
    let (Some(from), aim) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied().unwrap_or_default()) else { return };
    // The camera's axes: its forward, its right (level) and its up (tilted with the pitch).
    let forward = aim.forward_2d();
    let right = Vec2::new(-forward.y, forward.x).extend(0.0);
    let (sp, cp) = aim.pitch.sin_cos();
    let up = (-forward * sp).extend(cp);
    let pos = from + right * spec.right + up * spec.up;
    let pitch = (aim.pitch.to_degrees() + spec.pitch).min(89.0).to_radians();
    let flat = Vec2::from_angle(spec.yaw.to_radians()).rotate(forward);
    let dir = (flat * pitch.cos()).extend(pitch.sin());
    let shot = crate::projectiles::ProjectileSpec {
        speed: vec![(0.0, spec.speed)],
        lifetime: spec.lifetime,
        radius: FLASK_RADIUS,
        turn_rate: 0.0,
        gravity: spec.gravity,
        attack: crate::combat::AttackStats::default(),
        factor: 0.0,
        element: crate::combat::Element::Physical,
        by: HitSource::Skill(SkillKind::Item),
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: None,
        // (Any value: it makes the flask stop at its first touch; [`landed`] does the rest.)
        burst: 1.0,
        fuse: false,
        ignore_guard: false,
        knockback: None,
    };
    crate::projectiles::spawn_flask(world, id, shot, pos, dir, t.item_id);
    let mut ev = world.resource_mut::<SimEvents>();
    ev.push(SimEvent::ItemUsed { id, item_id: t.item_id });
    ev.push(SimEvent::InventoryChanged { id });
}

/// A flask stopped at `at` (its first touch: a character, a wall or the floor).
pub(crate) fn landed(world: &mut World, owner: SimId, item_id: i32, at: Vec3, flown: u32) {
    let Some(spec) = world.get_resource::<FlaskDb>().and_then(|db| db.0.get(&item_id).cloned()) else { return };
    let now = world.resource::<TickCount>().0;
    match spec.effect {
        FlaskEffect::Blast { radius, damage, slow } => {
            let owner_e = world.resource::<SimIndex>().get(owner);
            for (_, te) in inside(world, at, radius) {
                if let Some(e) = owner_e {
                    crate::affixes::fixed_damage(world, e, te, damage);
                }
                if let Some((rate, secs)) = slow
                    && world.get::<Health>(te).is_some_and(|h| h.current > 0.0)
                {
                    crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Slow(rate), skill: SkillKind::Item, ticks_left: ticks(secs), age: 0 });
                }
            }
            world.resource_mut::<SimEvents>().push(SimEvent::FlaskLanded { owner, item_id, at, radius, secs: 0.0 });
        }
        FlaskEffect::Area { radius, secs, every, dot } => {
            let ground = ground_under(world, at);
            let area = FlaskArea { owner, item_id, at: ground, radius, born: now, until: now + u64::from(ticks(secs)), every: ticks(every).max(1), dot };
            world.resource_mut::<FlaskAreas>().0.push(area);
            world.resource_mut::<SimEvents>().push(SimEvent::FlaskLanded { owner, item_id, at: ground, radius, secs });
        }
        FlaskEffect::Walls { count, spacing } => {
            let Some(wall) = spec.wall.clone() else { return };
            let ground = ground_under(world, at);
            let from = world.resource::<SimIndex>().get(owner).and_then(|e| world.get::<Position>(e)).map_or(ground, |p| p.0);
            let dir = (ground - from).truncate().normalize_or(Vec2::X);
            let across = Vec2::new(-dir.y, dir.x) * spacing;
            let yaw = dir.y.atan2(dir.x).to_degrees();
            let until = now + u64::from(ticks(spec.lifetime).saturating_sub(flown));
            for i in 0..count {
                let offset = i as f32 - (count as f32 - 1.0) * 0.5;
                let e = crate::destroyables::spawn_spec(world, wall.clone(), ground + (across * offset).extend(0.0), yaw, Vec3::ONE);
                world.entity_mut(e).insert(crate::destroyables::Expires(until));
            }
            world.resource_mut::<SimEvents>().push(SimEvent::FlaskLanded { owner, item_id, at: ground, radius: spacing * count as f32 * 0.5, secs: (until - now) as f32 * DT });
        }
        FlaskEffect::Torch => {
            let ground = ground_under(world, at);
            if let Some(torch) = crate::world_items::spawn_item_by_id(world, ground, item_id)
                && let Some(te) = world.resource::<SimIndex>().get(torch)
            {
                world.entity_mut(te).insert(Lit);
            }
            world.resource_mut::<SimEvents>().push(SimEvent::FlaskLanded { owner, item_id, at: ground, radius: 0.0, secs: 0.0 });
        }
    }
}

/// `FindProperGroundPos`: the floor under `at` (within [`GROUND_DEPTH`]), else `at`.
fn ground_under(world: &mut World, at: Vec3) -> Vec3 {
    let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
    let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
    spatial.cast_ray_predicate(at + Vec3::Z, Dir3::NEG_Z, GROUND_DEPTH, true, &crate::layers::walk(), &|b| boxes.contains(b)).map_or(at, |h| at + Vec3::Z - Vec3::Z * h.distance)
}

/// The living characters whose capsule a sphere of `radius` around `at` touches, in id order.
fn inside(world: &mut World, at: Vec3, radius: f32) -> Vec<(SimId, Entity)> {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    ids.into_iter().filter(|(_, te)| world.get::<crate::Combatant>(*te).is_some() && world.get::<Health>(*te).is_some_and(|h| h.current > 0.0) && world.get::<Position>(*te).is_some_and(|p| p.0.distance(at) <= radius + CAPSULE_RADIUS)).collect()
}

/// Every area's tick: its effect on the characters inside; it goes at its end.
fn areas_tick(world: &mut World, now: u64) {
    let mut areas = std::mem::take(&mut world.resource_mut::<FlaskAreas>().0);
    areas.retain(|a| now < a.until);
    for a in &areas {
        let Some(dot) = a.dot else { continue };
        if !(now - a.born).is_multiple_of(u64::from(a.every)) {
            continue;
        }
        for (_, te) in inside(world, a.at, a.radius) {
            burn(world, a.owner, te, dot);
        }
    }
    world.resource_mut::<FlaskAreas>().0 = areas;
}

/// Burns `te` (one burn: a running one keeps its pulses and starts its time again) and cuts its healing.
fn burn(world: &mut World, by: SimId, te: Entity, dot: Dot) {
    let full = ticks(dot.secs);
    let running = world.get_mut::<Buffs>(te).and_then(|mut b| {
        let x = b.0.iter_mut().find(|x| x.skill == SkillKind::Item && matches!(x.kind, BuffKind::Dot { .. }))?;
        x.ticks_left = full;
        Some(())
    });
    if running.is_none() {
        // Its first pulse on application (`bExecutePeriodicEffectOnApplication`, true by default): the buffs' next
        // tick, which counts its age up first.
        let interval = ticks(dot.period).max(1);
        crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Dot { by, damage: dot.damage, interval }, skill: SkillKind::Item, ticks_left: full, age: interval - 1 });
    }
    if let Some((rate, secs)) = dot.wound {
        crate::buffs::add_buff(world, te, Buff { kind: BuffKind::GrievousWounds(rate), skill: SkillKind::Item, ticks_left: ticks(secs), age: 0 });
    }
}

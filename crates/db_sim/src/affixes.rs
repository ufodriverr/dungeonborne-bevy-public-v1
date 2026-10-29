//! Legendary affix passives: a legendary weapon's affix ability set grants a passive ability that reacts to the holder's
//! hits or guards. Item -> `ExcelAffixTemplate` `legendary_rule` -> `ExcelAffixLegendaryRule` -> ability set
//! (`UESourceAbilitySet`) -> its `GrantedGameplayAbilities` -> the ability, read by name; the values come from the
//! effects it applies (strict reads) and its defaults (loose reads).
//!
//! CONFIRMED (the abilities' bytecode, the effects' strict reads):
//! - `GA_TGF_Affix_SpeedUpOnHit` (sword 5001): a hit dealt with the item that isn't a friendly one applies
//!   `GE_TGF_Sword5001_SpeedUp` / `1` / `2` to the holder (`MoveSpeedBase` +30 for 3 s, +50 for 2 s, +70 for 1 s: +150
//!   falling to 0 over 3 s), then cools down (`GE_TGF_Sword5001_Cooldown`, 8 s);
//! - `GA_TGF_Affix_LifeStealWhileHit` (sword 5002): a melee hit with the item on another, not friendly, adds a stack of
//!   `GE_TGF_Affix_AddLifeSteal` (`LifeLeachRate` +0.02, 2 s, up to 10 stacks);
//! - `GA_TGF_Affix_BattleKing` (sword 6001): a melee hit with the item on a character slows it
//!   (`GE_Buff_SlowDownTarget`, `MoveSpeedPercent` -0.8, 1.5 s) and the holder (`GE_Buff_SlowDownSelf`, -0.6, 1.5 s);
//! - `GA_TGF_Affix_MoveSpeedBurstWhenGuardSucceed` (mace 5002): guarding a hostile's hit gives the holder
//!   `GE_TGF_Buff_Affix_MoveSpeed_Up` (`MoveSpeedPercent` +0.6, 1 s);
//! - `GA_TGF_Affix_BounceSpeedDown` (shield 5002): guarding a hostile's hit slows the attacker (`GE_Buff_FrozenTarget`,
//!   `MoveSpeedRate` × 0.2 for 1.5 s, read by `monsters::ge_slow`) and deals it `GE_TGF_BounceDMG` (fixed 5);
//! - `GA_TGF_SucceedGuardRecoverAmmo` (relic 5001): on `TGF.GameplayEvent.SucceedGuarded` from a hostile, while the
//!   item is in the held set, `AddAmmoCount` (15) energy to the weapons of both hands.
//! - `GA_TGF_ShieldProjectile` (shield 6002): the set's `GA_TGF_ShieldQuickSideTagMonitor` grants
//!   `TGF.Ability.CanShieldQuickSide` while the holder guards (`TGF.Ability.DefenceForQuickSide`) with the shield in hand
//!   and `TGF.Ability.Cooldown.ShieldProjectile` absent; the left button (`InputTag.Weapon.Fire.FireLeft`) then commits
//!   the throw (`GE_TGF_ShieldProjectile_Cooldown`: the base cooldown's 5 s), plays `AM_Sword_Shield_CastShield_1st` and
//!   launches `B_ShieldProjectile` at the first target `SearchTarget` finds (`Max Distance` 1400 cm, `Unselect Angle` 15
//!   degrees). The projectile (a `B_TGF_ThunderChain`: `MaxHitNum` 3, `QueryRadius` 500 cm, `MaxExistTime` 15 s; its own
//!   `InitialSpeed` 1000 cm/s) applies `GE_TGF_Damage_Projectile_Shield` (`MoveFactorRate` 0.5) and
//!   `GE_Buff_ShieldSlowDownTarget` (`MoveSpeedRate` x 0.2 for 1.5 s) to each hit, then flies at the nearest unhit target
//!   within the query radius.
//!
//! HYPOTHESIS: the abilities' triggers (their `AbilityTriggers` aren't in the loose reads) are read as the hit and the
//! guard their bytecode checks; the item counts while it is in the held set's either hand, and "dealt with the item"
//! is any of the holder's weapon swings (the port's swings don't say which hand dealt them); a guard succeeds when a
//! held block faces the attacker (`melee::guard_faces`), against melee and projectile hits; a move speed rate adds to
//! 1 and multiplies the speed (`buffs::Buffs::speed_factor`); the stacks refresh their time on each hit and expire
//! together; the relic's energy goes to held relics only (staffs' energy isn't refilled); the fixed damage passes
//! through the attacker's damage reduction.
//!
//! - `GA_MeleeWeapon_Dagger_Charge_6002` (dagger 6002, its own charge, from its weapon folder, not the affix set, which
//!   grants nothing): the charge's hit on a hostile character reads its `CurShield`, clears it
//!   (`GE_TGF_Dagger6002_Charge_ClearShield`, `CalculatedShield` overridden to 0) and disperses its `AddShield` buffs;
//!   anything taken goes to the holder as a fixed shield (`GE_TGF_Dagger6002_Charge_AddShield1`, `SetByCaller.AddShield`)
//!   held by `_AddShield2` (a `GE_TGF_AddShield_Affect`: 10 s).
//!
//! - `GA_TGF_Affix_ManaBreak` (dagger 5002, its set's loose read stops after its id, so it is read from the item's
//!   folder): a hit event tagged `TGF.AbilityHurtType.Melee`, not `TGF.Ability.Effect.Damage.Friendly`, with the
//!   item applies `GE_TGF_ManaBreak` to the target: a `TgfFixedDamageExecution` of 10 and a `TgfFixedMagicExecution`
//!   of -10 (its mana). The item text ("backstab and charged attack reach further") doesn't match the ability; the
//!   ability is followed.
//!
//! HYPOTHESIS (the taken shield): it adds to a shield the holder already has, and the 10 s start again.
//!
//! HYPOTHESIS (the thrown shield): it leaves the hand when the cast montage hides the shield (`AN_Tgf_WeaponVisibleState`
//! at 0.21 s); targets are the living non-player combatants inside the cone from the eye, nearest first; with none, it
//! flies straight ahead.
//!
//! Not here ('s list): the abilities that change a weapon's own attacks (thunder staffs' charges, the longswords'
//! moves, the daggers' and the mace 6002's charges, sword 6001's `GA_TGF_Affix_AddMaxHPWhenKilling`), the ring and
//! set enhancements, and the player-kill rewards.

use std::collections::BTreeMap;

use anyhow::Context;
use bevy_ecs::prelude::*;
use db_formats::config::generated::{ConfigTables, EnItemWearingPart as Slot};
use serde_json::Value;

use crate::buffs::{Buff, BuffKind, add_buff};
use crate::skills::SkillKind;
use crate::staffs::{chain, package_of, prop, read_json};
use crate::{DT, SimEvent, SimEvents, SimId, TickCount};

/// One affix passive with its effects' values.
#[derive(Debug, Clone, PartialEq)]
pub enum Passive {
    /// A hit adds each flat move speed (cm/s) for its seconds, then waits `cooldown` seconds.
    SpeedUpOnHit { steps: Vec<(f32, f32)>, cooldown: f32 },
    /// A melee hit adds a life steal stack: rate per stack, seconds, most stacks.
    LifeStealOnHit { rate: f64, secs: f32, max: u8 },
    /// A melee hit changes the target's and the holder's move speed rate: (rate, seconds) each.
    BattleKing { target: (f64, f32), own: (f64, f32) },
    /// A guard adds a move speed rate for some seconds.
    GuardSpeedUp { rate: f64, secs: f32 },
    /// A guard slows the attacker (rate, seconds) and deals it fixed damage.
    GuardSlowAttacker { slow: (f64, f32), damage: f64 },
    /// A guard gives the held relics this much energy.
    GuardRecoverAmmo { count: u32 },
    /// The left button while guarding throws a homing shield.
    ShieldThrow(ShieldThrow),
    /// The held weapon's charge hit takes the target's shields for this many seconds.
    StealShields { secs: f32 },
    /// A melee hit deals this fixed damage more and takes this much of the target's mana.
    ManaBreak { damage: f64, mana: f64 },
}

/// The shield 6002's thrown shield.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldThrow {
    /// The target search: reach (cm) and half-angle (degrees) from the view.
    pub range: f32,
    pub half_deg: f32,
    /// Seconds into the throw that it leaves the hand.
    pub at: f32,
    pub speed: f32,
    pub radius: f32,
    pub lifetime: f32,
    /// Targets hit in all, and the reach (cm) to the next one.
    pub hits: u8,
    pub query: f32,
    pub factor: f64,
    pub element: crate::combat::Element,
    /// Each hit's slow: (rate, seconds).
    pub slow: (f64, f32),
    pub cooldown: f32,
}

/// A throw on its way out of the hand: the tick it leaves, and how.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ShieldThrowing {
    pub due: u64,
    pub spec: ShieldThrow,
}

/// Per item id: its passives, each with its key (the ability name's hash, `buffs::gameplay_tag`).
#[derive(Resource, Debug, Default, Clone)]
pub struct AffixDb(pub BTreeMap<i32, Vec<(u32, Passive)>>);

/// When each passive (by key) may fire again, tick.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct AffixCooldowns(pub BTreeMap<u32, u64>);

/// Reads every legendary affix passive; the abilities not modelled come back by name.
pub fn load_affix_db(t: &ConfigTables) -> anyhow::Result<(AffixDb, Vec<String>)> {
    let tsv = std::fs::read_to_string(db_formats::data_path("recovered/weapon_imports.tsv")).context("recovered/weapon_imports.tsv missing (tools/rebuild_data.sh)")?;
    let imports = |ga: &str| -> Vec<String> { tsv.lines().filter_map(|l| l.split_once('\t')).filter(|(p, _)| p.trim_end_matches(".uasset").ends_with(&format!("/{ga}"))).map(|(_, i)| i.to_string()).collect() };
    let mut db = AffixDb::default();
    let mut left = Vec::new();
    for a in &t.excel_affix_template {
        let aliases = a.data.iter().flat_map(|d| d.legendary_rule.iter().map(|r| r.affix_alias_id));
        let sets = aliases.filter_map(|alias| t.excel_affix_legendary_rule.iter().find(|r| r.affix_alias_id == alias)?.offset.as_ref().map(|o| o.ability_set_id));
        for set in sets.filter_map(|id| t.uesource_ability_set.iter().find(|s| s.ue_source_id == id)) {
            let Some(v) = read_json("json/loose", &package_of(&set.ue_path)) else { continue };
            let granted = v.as_array().into_iter().flatten().flat_map(|o| o["Properties"]["GrantedGameplayAbilities"].as_array().into_iter().flatten()).filter_map(|g| g["Ability"]["ObjectPath"].as_str().map(package_of));
            for ga in granted.collect::<Vec<_>>() {
                let name = ga.rsplit('/').next().unwrap_or(&ga).to_string();
                match passive(&ga, &name, &imports(&name)) {
                    Some(p) => db.0.entry(a.item_id).or_default().push((crate::buffs::gameplay_tag(&name), p)),
                    None if !left.contains(&name) => left.push(name),
                    None => {}
                }
            }
        }
    }
    // The legendary weapons' own charge abilities: beside each item's weapon definition. Affix passives found there
    // count only for an item whose sets read nothing (a folder may hold another item's: the sword 5001's holds the
    // sword 5002's life steal).
    for a in &t.excel_affix_template {
        let from_sets = db.0.contains_key(&a.item_id);
        for (key, p) in own_abilities(t, a.item_id).into_iter().filter(|(_, p)| matches!(p, Passive::StealShields { .. }) || !from_sets) {
            let list = db.0.entry(a.item_id).or_default();
            // (One per kind: the dagger's single and paired charges both take shields.)
            if !list.iter().any(|x| x.0 == key || x.1 == p) {
                list.push((key, p));
            }
        }
    }
    anyhow::ensure!(!db.0.is_empty(), "no legendary affix passives decoded under _data/json");
    Ok((db, left))
}

/// The abilities in a weapon item's own folder (its definition's, `.../GameAbility/WP_<Kind>_<n>/WID_<Kind>_<n>`) that
/// the port models: the dagger 6002's shield-taking charge, and affix passives whose set reads no grants (the dagger
/// 5002's mana break).
fn own_abilities(t: &ConfigTables, item_id: i32) -> Vec<(u32, Passive)> {
    let Some(equip) = t.uesource_item_type.iter().find(|u| u.type_id == item_id).map(|u| u.ue_source_equipment_equip_id) else { return Vec::new() };
    let Some(wid) = t.ue_source_equipment.iter().find(|r| r.equip_id == equip).map(|r| package_of(&r.ue_path)) else { return Vec::new() };
    let Some((dir, _)) = wid.rsplit_once('/') else { return Vec::new() };
    let Some(rel) = dir.strip_prefix("/Game/").filter(|r| r.rsplit('/').next().is_some_and(|f| f.to_ascii_uppercase().starts_with("WP_"))) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in std::fs::read_dir(db_formats::data_path(format!("json/loose/Dungeonborne/Content/{rel}"))).into_iter().flatten().flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = file.strip_suffix(".json").filter(|s| s.starts_with("GA_")) else { continue };
        if stem.starts_with("GA_TGF_Affix_")
            && let Some(p) = passive(&format!("{dir}/{stem}"), stem, &[])
        {
            out.push((crate::buffs::gameplay_tag(stem), p));
            continue;
        }
        let ga = chain(&format!("{dir}/{stem}"));
        if prop(&ga, "GEAddShield1").is_some()
            && let Some(secs) = default_class(&ga, "GEAddShield2").and_then(|g| prop(&chain(&package_of(&g)), "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64())
        {
            out.push((crate::buffs::gameplay_tag(stem), Passive::StealShields { secs: secs as f32 }));
        }
    }
    out
}

/// An effect's modifier on `attribute` (its magnitude), its duration and its stack limit.
fn effect(path: &str, attribute: &str) -> Option<(f64, f32, u8)> {
    let ge = chain(&package_of(path));
    let m = prop(&ge, "Modifiers")?.as_array()?.iter().find(|m| m["Attribute"]["AttributeName"] == attribute)?;
    let value = m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()?;
    let secs = prop(&ge, "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64()? as f32;
    let stacks = prop(&ge, "StackLimitCount").and_then(Value::as_u64).map_or(1, |n| n.clamp(1, 255) as u8);
    Some((value, secs, stacks))
}

/// The class an ability's default names.
fn default_class(ga: &[crate::staffs::Bp], key: &str) -> Option<String> {
    prop(ga, key)?["ObjectPath"].as_str().map(str::to_string)
}

fn passive(path: &str, name: &str, imports: &[String]) -> Option<Passive> {
    let ga = chain(path);
    Some(match name {
        "GA_TGF_Affix_SpeedUpOnHit" => {
            let ges: Vec<&String> = imports.iter().filter(|i| i.rsplit('/').next().is_some_and(|n| n.starts_with("GE_"))).collect();
            let cooldown = ges.iter().find(|g| g.contains("Cooldown")).and_then(|g| prop(&chain(g), "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64())? as f32;
            let mut steps: Vec<(f32, f32)> = ges.iter().filter_map(|g| effect(g, "MoveSpeedBase")).map(|(v, s, _)| (v as f32, s)).collect();
            steps.sort_by(|a, b| b.1.total_cmp(&a.1));
            if steps.is_empty() {
                return None;
            }
            Passive::SpeedUpOnHit { steps, cooldown }
        }
        "GA_TGF_Affix_LifeStealWhileHit" => {
            let (rate, secs, max) = effect(&default_class(&ga, "Effect")?, "LifeLeachRate")?;
            Passive::LifeStealOnHit { rate, secs, max }
        }
        "GA_TGF_Affix_BattleKing" => {
            let (t, ts, _) = effect(&default_class(&ga, "Gameplay Effect Class")?, "MoveSpeedPercent")?;
            let (o, os, _) = effect(&default_class(&ga, "Effect")?, "MoveSpeedPercent")?;
            Passive::BattleKing { target: (t, ts), own: (o, os) }
        }
        "GA_TGF_Affix_MoveSpeedBurstWhenGuardSucceed" => {
            let (rate, secs, _) = effect(&default_class(&ga, "SpeedUpGE")?, "MoveSpeedPercent")?;
            Passive::GuardSpeedUp { rate, secs }
        }
        "GA_TGF_Affix_BounceSpeedDown" => {
            let slow = crate::monsters::ge_slow(&default_class(&ga, "SpeedDownGE")?)?;
            let dmg = chain(&package_of(&default_class(&ga, "DMG GE")?));
            let damage = prop(&dmg, "Executions")?.as_array()?.first()?["CalculationModifiers"].as_array()?.first()?["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()?;
            Passive::GuardSlowAttacker { slow, damage }
        }
        "GA_TGF_SucceedGuardRecoverAmmo" => Passive::GuardRecoverAmmo { count: prop(&ga, "AddAmmoCount")?.as_u64()? as u32 },
        "GA_TGF_ShieldProjectile" => Passive::ShieldThrow(shield_throw(&ga, imports)?),
        "GA_TGF_Affix_ManaBreak" => {
            let ge = chain(&package_of(&default_class(&ga, "ReduceManaGE")?));
            let execs = prop(&ge, "Executions")?.as_array()?;
            let value = |class: &str| execs.iter().find(|x| x["CalculationClass"]["ObjectName"].as_str().is_some_and(|c| c.contains(class)))?["CalculationModifiers"].as_array()?.first()?["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64();
            Passive::ManaBreak { damage: value("TgfFixedDamageExecution")?, mana: -value("TgfFixedMagicExecution")? }
        }
        _ => return None,
    })
}

/// The thrown shield from its ability, its imports (the projectile, the cooldown, the cast montage) and the
/// projectile's class chain.
fn shield_throw(ga: &[crate::staffs::Bp], imports: &[String]) -> Option<ShieldThrow> {
    let named = |pre: &str| imports.iter().find(|i| i.rsplit('/').next().is_some_and(|n| n.starts_with(pre))).cloned();
    let actor = named("B_ShieldProjectile")?;
    let pc = chain(&actor);
    let num = |k: &str| prop(&pc, k).and_then(Value::as_f64);
    let ges: Vec<String> = prop(&pc, "Level1GEList")?.as_array()?.iter().filter_map(|g| g["ObjectPath"].as_str().map(str::to_string)).collect();
    let damage = ges.iter().find(|g| g.contains("Damage"))?;
    let (factor, element) = crate::staffs::damage_effect(&package_of(damage)).ok()?;
    let slow = ges.iter().find_map(|g| crate::monsters::ge_slow(g))?;
    let cooldown = prop(&chain(&named("GE_TGF_ShieldProjectile_Cooldown")?), "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64()? as f32;
    let at = named("AM_Sword_Shield_CastShield_1st").and_then(|m| crate::staffs::Montage::load(&package_of(&m)).ok()).and_then(|m| m.first("AN_Tgf_WeaponVisibleState_C").map(|n| n.0)).unwrap_or(0.0);
    // The movement and the collision box sit on the class or a parent (as the thunder chain's, `weapons::lightning_of`).
    let (mut speed, mut radius, mut class) = (None, None, Some(actor.clone()));
    for _ in 0..4 {
        let Some(v) = class.take().and_then(|c| read_json("json/loose", &c).or_else(|| read_json("json/assets", &c))) else { break };
        for o in v.as_array().into_iter().flatten() {
            match o["Type"].as_str() {
                Some("ProjectileMovementComponent") => speed = speed.or(o["Properties"]["InitialSpeed"].as_f64()),
                Some("BoxComponent") => radius = radius.or(o["Properties"]["BoxExtent"]["X"].as_f64()),
                Some("BlueprintGeneratedClass") => class = o["Super"]["ObjectPath"].as_str().filter(|s| s.starts_with("/Game/")).map(package_of),
                _ => {}
            }
        }
    }
    Some(ShieldThrow {
        range: prop(ga, "Max Distance")?.as_f64()? as f32,
        half_deg: prop(ga, "Unselect Angle")?.as_f64()? as f32,
        at,
        speed: speed? as f32,
        radius: radius.unwrap_or(15.0) as f32,
        lifetime: num("MaxExistTime")? as f32,
        hits: num("MaxHitNum")? as u8,
        query: num("QueryRadius")? as f32,
        factor,
        element,
        slow,
        cooldown,
    })
}

/// A left click from `id`: while it guards with a shield that throws (and the throw is ready), the throw starts and
/// the click isn't a swing. Returns whether it threw.
pub(crate) fn try_shield_throw(world: &mut World, id: SimId) -> bool {
    let Some(e) = world.resource::<crate::SimIndex>().get(id) else { return false };
    if !world.get::<crate::melee::Melee>(e).is_some_and(|m| m.blocking()) || world.get::<ShieldThrowing>(e).is_some() {
        return false;
    }
    let Some((key, spec)) = held(world, e).into_iter().find_map(|(k, p)| if let Passive::ShieldThrow(s) = p { Some((k, s)) } else { None }) else { return false };
    let now = world.resource::<TickCount>().0;
    if world.get::<AffixCooldowns>(e).and_then(|c| c.0.get(&key)).is_some_and(|ready| now < *ready) {
        return false;
    }
    let ready = now + u64::from(ticks(spec.cooldown));
    match world.get_mut::<AffixCooldowns>(e) {
        Some(mut c) => {
            c.0.insert(key, ready);
        }
        None => {
            world.entity_mut(e).insert(AffixCooldowns(BTreeMap::from([(key, ready)])));
        }
    }
    world.entity_mut(e).insert(ShieldThrowing { due: now + u64::from(ticks(spec.at)), spec });
    world.resource_mut::<SimEvents>().push(SimEvent::ShieldThrown { id });
    true
}

/// Throws whose time came: the shield flies from the eye at the nearest target in the cone, or straight ahead.
pub fn affixes_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let due: Vec<(Entity, ShieldThrow)> = world.query::<(Entity, &ShieldThrowing)>().iter(world).filter(|(_, t)| now >= t.due).map(|(e, t)| (e, t.spec)).collect();
    for (e, spec) in due {
        world.entity_mut(e).remove::<ShieldThrowing>();
        let Some(id) = world.get::<crate::SimId>(e).copied() else { continue };
        let (Some(from), Some(c)) = (world.get::<crate::Position>(e).map(|p| p.0), world.get::<crate::Combatant>(e).copied()) else { continue };
        if world.get::<crate::Health>(e).is_none_or(|h| h.current <= 0.0) {
            continue;
        }
        let aim = world.get::<crate::melee::Aim>(e).copied().unwrap_or_default();
        let fwd = aim.forward_2d();
        let eye = from + bevy_math::Vec3::Z * crate::overrides::class_skills::SHOT_HEIGHT;
        let ids: Vec<(SimId, Entity)> = world.resource::<crate::SimIndex>().iter().collect();
        let target = ids
            .iter()
            .filter(|(tid, te)| *tid != id && world.get::<crate::Combatant>(*te).is_some() && world.get::<crate::Health>(*te).is_some_and(|h| h.current > 0.0) && world.get::<crate::command::Kind>(*te).is_none_or(|k| k.0 != crate::command::ActorKind::Player))
            .filter_map(|&(tid, te)| world.get::<crate::Position>(te).map(|p| (tid, p.0)))
            .filter(|(_, p)| {
                let v = (*p - from).truncate();
                v.length() <= spec.range && fwd.dot(v.normalize_or_zero()) >= spec.half_deg.to_radians().cos()
            })
            .min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)).then(a.0.cmp(&b.0)));
        let dir = target.map_or(fwd.extend(0.0), |(_, p)| p - eye).normalize_or(bevy_math::Vec3::X);
        let shot = crate::projectiles::ProjectileSpec {
            speed: vec![(0.0, spec.speed)],
            lifetime: spec.lifetime,
            radius: spec.radius,
            turn_rate: crate::overrides::affixes::CHAIN_TURN_RATE,
            gravity: 0.0,
            attack: crate::buffs::attack_of(world, e, c.attack),
            factor: spec.factor,
            element: spec.element,
            by: crate::skills::HitSource::Skill(SkillKind::Item),
            pierce: 0,
            chain: spec.hits.saturating_sub(1),
            chain_range: spec.query,
            pull: 0.0,
            on_hit: Some((BuffKind::Slow(spec.slow.0), spec.slow.1)),
            burst: 0.0,
            fuse: false,
            ignore_guard: false,
            knockback: None,
        };
        crate::projectiles::spawn(world, id, shot, eye + dir * 50.0, dir, target.map(|t| t.0));
    }
}

/// The passives of the held set's items.
fn held(world: &World, e: Entity) -> Vec<(u32, Passive)> {
    let (Some(db), Some(inv)) = (world.get_resource::<AffixDb>(), world.get::<crate::Inventory>(e)) else { return Vec::new() };
    [Slot::Pair1rightweapon, Slot::Pair1leftweapon].iter().filter_map(|s| inv.worn.get(s)).filter_map(|i| db.0.get(&i.item.item_id)).flatten().cloned().collect()
}

fn ticks(secs: f32) -> u32 {
    (secs / DT).round() as u32
}

/// A timed affix effect on `e` (a new one replaces the one with the same key).
fn timed(world: &mut World, e: Entity, secs: f32, kind: BuffKind) {
    add_buff(world, e, Buff { kind, skill: SkillKind::Item, ticks_left: ticks(secs), age: 0 });
}

/// An affix effect changing move speed.
fn speed(key: u32, speed: f32, speed_rate: f64) -> BuffKind {
    BuffKind::Affix { key, speed, speed_rate, life_steal: 0.0, stacks: 1 }
}

/// After a weapon swing of `e`'s hit `te` (damage dealt; `melee`: a melee hit).
pub(crate) fn on_hit(world: &mut World, e: Entity, te: Entity, melee: bool) {
    let now = world.resource::<TickCount>().0;
    for (key, p) in held(world, e) {
        match p {
            Passive::SpeedUpOnHit { steps, cooldown } => {
                if world.get::<AffixCooldowns>(e).and_then(|c| c.0.get(&key)).is_some_and(|ready| now < *ready) {
                    continue;
                }
                for (i, (cm_s, secs)) in steps.into_iter().enumerate() {
                    timed(world, e, secs, speed(key.wrapping_add(i as u32), cm_s, 0.0));
                }
                let ready = now + u64::from(ticks(cooldown));
                match world.get_mut::<AffixCooldowns>(e) {
                    Some(mut c) => {
                        c.0.insert(key, ready);
                    }
                    None => {
                        world.entity_mut(e).insert(AffixCooldowns(BTreeMap::from([(key, ready)])));
                    }
                }
            }
            Passive::LifeStealOnHit { rate, secs, max } if melee && te != e => {
                let stacks = world.get::<crate::buffs::Buffs>(e).and_then(|b| b.0.iter().find_map(|x| if let BuffKind::Affix { key: k, stacks, .. } = x.kind { (k == key).then_some(stacks) } else { None })).unwrap_or(0);
                timed(world, e, secs, BuffKind::Affix { key, speed: 0.0, speed_rate: 0.0, life_steal: rate, stacks: (stacks + 1).min(max) });
            }
            Passive::BattleKing { target, own } if melee => {
                timed(world, te, target.1, speed(key, 0.0, target.0));
                timed(world, e, own.1, speed(key, 0.0, own.0));
            }
            Passive::ManaBreak { damage, mana } if melee && te != e => {
                if let Some(mut p) = world.get_mut::<crate::Power>(te)
                    && p.kind == crate::PowerKind::Mana
                {
                    p.current = (p.current - mana).max(0.0);
                }
                fixed_damage(world, e, te, damage);
            }
            _ => {}
        }
    }
}

/// After `e`'s charge (heavy) swing hit `te`: the right-hand weapon's shield taking.
pub(crate) fn on_charge_hit(world: &mut World, e: Entity, te: Entity) {
    if e == te {
        return;
    }
    let (Some(db), Some(inv)) = (world.get_resource::<AffixDb>(), world.get::<crate::Inventory>(e)) else { return };
    let Some(secs) = inv.worn.get(&Slot::Pair1rightweapon).and_then(|i| db.0.get(&i.item.item_id)).and_then(|l| l.iter().find_map(|p| if let Passive::StealShields { secs } = p.1 { Some(secs) } else { None })) else { return };
    let taken = world.get::<crate::world_items::Effects>(te).and_then(|f| f.shield).map_or(0.0, |s| s.0);
    if let Some(mut fx) = world.get_mut::<crate::world_items::Effects>(te) {
        fx.shield = None;
    }
    if taken > 0.0 {
        let have = world.get::<crate::world_items::Effects>(e).and_then(|f| f.shield).map_or(0.0, |s| s.0);
        crate::world_items::give_shield(world, e, have + taken, secs);
    }
}

/// An affix's fixed damage from `e` to a living `te` (through its damage reduction).
pub(crate) fn fixed_damage(world: &mut World, e: Entity, te: Entity, damage: f64) {
    if world.get::<crate::Health>(te).is_none_or(|h| h.current <= 0.0) {
        return;
    }
    let (Some(&id), Some(&tid)) = (world.get::<SimId>(e), world.get::<SimId>(te)) else { return };
    let damage = crate::buffs::incoming_damage(world, te, damage, Some(e));
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    let mut events = world.resource_mut::<SimEvents>();
    events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: false, blocked: false, health: left, by: crate::skills::HitSource::Skill(SkillKind::Item) });
    if left <= 0.0 {
        events.push(SimEvent::Died { id: tid });
    }
}

/// `de` (`did`) guarded a hit by `ae` (`aid`): its block faced the attacker.
pub(crate) fn on_guarded(world: &mut World, did: SimId, de: Entity, aid: SimId, ae: Entity) {
    if did == aid {
        return;
    }
    // The rolled guard triggers.
    crate::gear_triggers::on_guarded(world, (did, de), (aid, ae));
    for (key, p) in held(world, de) {
        match p {
            Passive::GuardSpeedUp { rate, secs } => timed(world, de, secs, speed(key, 0.0, rate)),
            Passive::GuardSlowAttacker { slow, damage } => {
                add_buff(world, ae, Buff { kind: BuffKind::Slow(slow.0), skill: SkillKind::Item, ticks_left: ticks(slow.1), age: 0 });
                fixed_damage(world, de, ae, damage);
            }
            Passive::GuardRecoverAmmo { count } => {
                if let Some(mut r) = world.get_mut::<crate::weapons::RelicEnergy>(de)
                    && let Some((uid, spec)) = r.held
                {
                    let v = r.energy.entry(uid).or_insert(0);
                    *v = (*v + count).min(spec.max);
                }
            }
            _ => {}
        }
    }
}

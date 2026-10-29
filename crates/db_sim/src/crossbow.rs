//! The heavy crossbow: a left press shoots the loaded bolt, an empty crossbow reloads one from the arrows in the
//! bag (on the press or the reload key), and the right button aims down the sights instead of guarding.
//!
//! CONFIRMED (the loose v6 reads and bytecode of `GA_TGF_RangedWeapon_Player_Base` / `_Shoot`, `GA_TGF_Reload`,
//! `GA_TGF_ADS` and the crossbow's subclasses, `WID_HeavyCrossbow_Base`, `B_TGF_Arrow_CrossBow_Base` and its parents,
//! `Crossbow_DamageRate`, `GE_TGF_Crossbow_Cooldown`, `CM_Tgf_FirstPerson_ADS`, the first-person montages):
//! - the weapon holds `MaxAmmo` bolts (1) of its `AmmoDefinition` (`ID_Arrow`, item 100045); a shot uses
//!   `OnceUseAmmoCount` (1); a press with none loaded triggers the reload (`CheckNeedTriggerReload`), which needs
//!   arrows in the bag (else `Ability.ActivateFail.Cost.Ammo.Bow`);
//! - the shot (`DecideRot`): the camera's trace to `MaxTraceDistance` (3000 cm) or what it hits, off by a random
//!   angle up to `MaxScatterDegree` (2 degrees) unless aiming; the bolt `B_TGF_Arrow_CrossBow_Base` flies at
//!   `InitialSpeed` 10000 cm/s, no gravity (`B_TGF_Normal_ProjectileBase`), 10 s, damage x1.2
//!   (`GE_TGF_Damage_Instant_Projectile_CrossBow`), and `DamageRateCurve` adds -0.2 past 18 m and -0.3 past 25 m;
//!   the fire montages last 0.633 s (the aimed one moves at x0.6), the cooldown 0.05 s;
//! - the reload (`AM_HeavyCrossBow_HipReload_1st`, 3.733 s, moves at x0.3 from 0.195 to 3.091 s) loads at its
//!   `Reload` notify (3.08 s, `DoFullReload`) and stops the aim first;
//! - aiming (`GA_TGF_ADS`, held): `MoveRate` 0.5, the camera mode's field of view 60 (blend 0.33 s).
//!
//! HYPOTHESIS: a new crossbow starts empty; a press while busy is dropped; `MoveRate` multiplies the move speed and a
//! montage's move scale replaces it while it plays; the shot leaves from the shot height ahead of the capsule
//! (`ShootStart` socket offsets aren't used); the damage rate is read by the distance from the launch point; the
//! legendary 6001's on-hit freeze passive (`GA_TGF_HitCauseFrozen`, granted through its affix ability set) is 's.
//!
//! each crossbow item uses its own definition (`EquipDefId`): the legendary 5001 aims at full speed
//! (`GA_TGF_ADS_Crossbow_5001` `MoveRate` 1) and reloads without the slow (`AM_HeavyCrossBow_HipReload_1st_5001` has no
//! move scale); the 6001's bolt slows by 85% for 3 s (`GE_Buff_FrozenTarget`); the `_Temp` 2002's bolt burns, the
//! 2003's freezes (20% slow, 2 s).

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;
use serde_json::Value;

use crate::melee::{Aim, Melee};
use crate::skills::HitSource;
use crate::staffs::{Montage, chain, obj_path, package_of, prop, read_json};
use crate::{DT, SimEvent, SimEvents, SimId, TickCount};

const FIRE_LEFT_PRESS: &str = "InputTag.Weapon.Fire.FireLeftByPress";
const RELOAD: &str = "InputTag.Weapon.Fire.FireReload";
const ADS: &str = "InputTag.Weapon.Fire.FireRight";

/// A crossbow's shot, reload and aim.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossbowSpec {
    pub max_ammo: u32,
    /// The ammunition item (`AmmoDefinition`'s `ItemDefId`).
    pub ammo_item: i32,
    pub per_shot: u32,
    pub shot: crate::monsters::Shot,
    /// `DamageRateCurve`: (distance cm, added damage rate), read linearly.
    pub falloff: Vec<(f32, f32)>,
    pub scatter_degrees: f32,
    pub cooldown: f32,
    /// The fire montages' lengths (hip, aimed) and the aimed one's move scale.
    pub fire: (f32, f32),
    pub aim_fire_move: f32,
    /// The reload montage: its length, the `Reload` notify, its move scale window (from, to, scale).
    pub reload: f32,
    pub reload_at: f32,
    pub reload_move: (f32, f32, f32),
    /// Aiming: the move rate, the field of view and its blend time.
    pub ads_move: f32,
    pub ads_fov: f32,
    pub ads_blend: f32,
    /// After a switch to it, the draw (`AM_HeavyCrossBow_Draw_1st`) blocks the shot and the aim
    /// (`TGF.Ability.BlockAction.CrossBowCantShoot` / `CantADS`) for this long, seconds.
    pub draw_block: f32,
    /// What the bolt's hit leaves: a slow (rate, seconds) from a timed `MoveSpeedRate` effect in its list (the
    /// legendary 6001's `GE_Buff_FrozenTarget`) or its `Add Freezing Buff Arg` (`_Temp` 2003: the curve's
    /// `FreezingRateBase` for `FreezingDurationBase`), and burn stacks from its `Add Burning Buff Arg` (`_Temp` 2002,
    /// the default stack count).
    pub bolt_slow: Option<(f64, f32)>,
    pub bolt_burn: Option<crate::staffs::BurnSpec>,
}

/// What the crossbow is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossbowAction {
    Fire { tick: u32, aimed: bool },
    Reload { tick: u32 },
}

/// An actor's crossbows: the bolts loaded in each (by item uid), the held one, its action and the aim.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct CrossbowState {
    pub loaded: BTreeMap<u32, u32>,
    pub held: Option<(u32, Arc<CrossbowSpec>)>,
    pub action: Option<CrossbowAction>,
    pub aiming: bool,
    /// The tick the cooldown ends.
    pub ready_at: u64,
    /// The reload key was pressed.
    pub reload_wanted: bool,
}

impl CrossbowState {
    /// The held crossbow's loaded bolts and its capacity.
    pub fn held_ammo(&self) -> Option<(u32, u32)> {
        let (uid, spec) = self.held.as_ref()?;
        Some((self.loaded.get(uid).copied().unwrap_or(0), spec.max_ammo))
    }

    /// Movement scale: a montage's move scale while it plays, else the aim's move rate.
    pub fn move_factor(&self) -> f32 {
        let Some((_, spec)) = &self.held else { return 1.0 };
        let secs = |t: u32| t as f32 * DT;
        match self.action {
            Some(CrossbowAction::Reload { tick }) if (spec.reload_move.0..spec.reload_move.1).contains(&secs(tick)) => spec.reload_move.2,
            Some(CrossbowAction::Fire { aimed: true, .. }) => spec.aim_fire_move,
            _ if self.aiming => spec.ads_move,
            _ => 1.0,
        }
    }
}

/// The crossbow from its weapon definition and ability set (`imports`: a package's imports by name).
pub(crate) fn crossbow_of(wid: &str, set: &str, imports: &dyn Fn(&str) -> Vec<String>) -> anyhow::Result<CrossbowSpec> {
    let w = chain(wid);
    let num = |c: &[crate::staffs::Bp], k: &str| prop(c, k).and_then(Value::as_f64).map(|v| v as f32);
    let max_ammo = num(&w, "MaxAmmo").context("MaxAmmo")? as u32;
    let ammo = prop(&w, "AmmoDefinition").and_then(obj_path).context("AmmoDefinition")?;
    let ammo_item = prop(&chain(&ammo), "ItemDefId").and_then(Value::as_i64).context("the ammunition's ItemDefId")? as i32;
    let shoot = crate::weapons::granted(set, FIRE_LEFT_PRESS)?.into_iter().next().context("no shoot ability")?;
    let reload = crate::weapons::granted(set, RELOAD)?.into_iter().next().context("no reload ability")?;
    let ads = crate::weapons::granted(set, ADS)?.into_iter().next().context("no aim ability")?;
    let s = chain(&shoot);
    let actor = prop(&s, "ProjectileTemplate").and_then(obj_path).context("ProjectileTemplate")?;
    let bolt = chain(&actor);
    let shot = crate::weapons::shot_of(&actor, &bolt).with_context(|| format!("bolt {actor}"))?;
    let script = read_json("json/loose", &package_of(&actor)).map(|v| v.to_string()).unwrap_or_default();
    let arg = crate::staffs::buff_arg;
    let effects = prop(&bolt, "GameplayEffectClass").and_then(Value::as_array).into_iter().flatten().filter_map(obj_path);
    let bolt_slow = effects.filter_map(|g| crate::monsters::ge_slow(&g)).next().or_else(|| {
        script.contains("Add Freezing Buff Arg").then(|| Some((arg("Common.BuffArg.FreezingRateBase")?, arg("Common.BuffArg.FreezingDurationBase")? as f32))).flatten()
    });
    let bolt_burn = script.contains("Add Burning Buff Arg").then(|| {
        Some(crate::staffs::BurnSpec { damage: arg("Common.BuffArg.BurningDamage")?, interval: arg("Common.BuffArg.BurningIntervalBase")? as f32, stacks: arg("Common.BuffArg.BurningStack")? as u32 })
    }).flatten();
    let curve = prop(&s, "DamageRateCurve").and_then(obj_path).context("DamageRateCurve")?;
    let v = read_json("json/loose", &curve).or_else(|| read_json("json/assets", &curve)).with_context(|| format!("{curve} not decoded"))?;
    let keys = v.as_array().into_iter().flatten().find_map(|o| o["FloatCurve"]["Keys"].as_array().or(o["Properties"]["FloatCurve"]["Keys"].as_array())).context("damage rate keys")?;
    let falloff = keys.iter().filter_map(|k| Some((k["Time"].as_f64()? as f32, k["Value"].as_f64()? as f32))).collect();
    let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    // An ability's import, else its parent ability's (the legendary and `_Temp` shots and reloads derive from the base ones).
    fn own_or_parent(ga: &str, imports: &dyn Fn(&str) -> Vec<String>, want: &dyn Fn(&str) -> bool, depth: u8) -> Option<String> {
        let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        let own = imports(&name(ga));
        own.iter().find(|p| want(p)).cloned().or_else(|| if depth == 0 { None } else { own.iter().filter(|p| name(p).starts_with("GA_TGF_")).find_map(|parent| own_or_parent(parent, imports, want, depth - 1)) })
    }
    let first = |ga: &str, part: &str| own_or_parent(ga, imports, &|p| p.contains("/Animation/1st/") && name(p).contains(part), 3);
    let montage = |ga: &str, part: &str| -> anyhow::Result<Montage> { Montage::load(&package_of(&first(ga, part).with_context(|| format!("{ga}: no {part} montage"))?)) };
    let (hip, aimed) = (montage(&shoot, "HipFire")?, montage(&shoot, "AimFire")?);
    let cooldown = own_or_parent(&shoot, imports, &|p| name(p).contains("Cooldown"), 3).and_then(|g| prop(&chain(&g), "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64()).unwrap_or(0.0) as f32;
    let r = montage(&reload, "HipReload")?;
    let reload_at = r.event("Reload").context("Reload notify")?;
    let rm = r.first("AN_Tgf_Gameplay_AnimationMoveScale_C").map_or((0.0, 0.0, 1.0), |(t, d, p)| (t, t + d, p["Scale"].as_f64().unwrap_or(1.0) as f32));
    // The draw montage: the WID's anim setup's (or its parent setup's) first-person `Draw` import.
    let waid = prop(&w, "SingleAnimWeaponDefinition").and_then(obj_path).map(|p| name(&package_of(&p)));
    let is_draw = |p: &String| p.contains("/Animation/1st/") && name(p).contains("_Draw_");
    let draw = waid.and_then(|w| imports(&w).into_iter().find(is_draw).or_else(|| imports(&w).into_iter().filter(|p| name(p).starts_with("WAID_")).find_map(|parent| imports(&name(&parent)).into_iter().find(is_draw))));
    let draw_block = draw.and_then(|d| Montage::load(&package_of(&d)).ok()).and_then(|m| m.tag_window("TGF.Ability.BlockAction.CrossBowCantShoot")).map_or(0.0, |w| w.1);
    let a = chain(&ads);
    let camera = prop(&a, "CameraMode").and_then(obj_path).context("CameraMode")?;
    let cam = chain(&camera);
    Ok(CrossbowSpec {
        max_ammo,
        ammo_item,
        per_shot: num(&s, "OnceUseAmmoCount").unwrap_or(1.0) as u32,
        shot,
        falloff,
        scatter_degrees: num(&s, "MaxScatterDegree").unwrap_or(0.0),
        cooldown,
        fire: (hip.length, aimed.length),
        aim_fire_move: aimed.first("AN_Tgf_Gameplay_AnimationMoveScale_C").map_or(1.0, |(_, _, p)| p["Scale"].as_f64().unwrap_or(1.0) as f32),
        reload: r.length,
        reload_at,
        reload_move: rm,
        ads_move: num(&a, "MoveRate").unwrap_or(1.0),
        ads_fov: num(&cam, "FieldOfView").context("the aim camera's FieldOfView")?,
        ads_blend: num(&cam, "BlendTime").unwrap_or(0.0),
        draw_block,
        bolt_slow,
        bolt_burn,
    })
}

/// The damage rate added at `dist` cm (linear between keys, held outside them).
pub fn falloff_at(keys: &[(f32, f32)], dist: f32) -> f32 {
    match keys.iter().position(|k| k.0 > dist) {
        Some(0) => keys[0].1,
        Some(i) => {
            let (a, b) = (keys[i - 1], keys[i]);
            a.1 + (b.1 - a.1) * (dist - a.0) / (b.0 - a.0)
        }
        None => keys.last().map_or(0.0, |k| k.1),
    }
}

/// The held crossbow: the right hand's item if it is one.
fn held(world: &World, e: Entity) -> Option<(u32, Arc<CrossbowSpec>)> {
    let inv = world.get::<crate::Inventory>(e)?;
    let item = inv.worn.get(&db_formats::config::generated::EnItemWearingPart::Pair1rightweapon)?;
    let db = world.get_resource::<crate::weapons::WeaponCombos>()?;
    // The item's own definition, else its kind's.
    let spec = match db.crossbow_items.get(&item.item.item_id) {
        Some(s) => s.clone(),
        None => db.crossbows.get(&crate::weapons::subtype(world, item.item.item_id)?)?.clone(),
    };
    Some((item.uid, spec))
}

/// Arrows of `item` in the bag.
fn arrows(world: &World, e: Entity, item: i32) -> u32 {
    world.get::<crate::Inventory>(e).map_or(0, |inv| inv.bag.iter().filter(|b| b.item.item.item_id == item).map(|b| u32::from(b.item.count)).sum())
}

/// Takes up to `n` arrows of `item` from the bag; returns how many.
fn take_arrows(world: &mut World, e: Entity, item: i32, n: u32) -> u32 {
    let Some(mut inv) = world.get_mut::<crate::Inventory>(e) else { return 0 };
    let mut taken = 0;
    for b in inv.bag.iter_mut().filter(|b| b.item.item.item_id == item) {
        let k = (n - taken).min(u32::from(b.item.count));
        b.item.count -= k as u16;
        taken += k;
        if taken == n {
            break;
        }
    }
    inv.bag.retain(|b| b.item.count > 0);
    taken
}

/// The reload key (`SimCommand::Reload`).
pub(crate) fn want_reload(world: &mut World, e: Entity) {
    let mut st = world.get::<CrossbowState>(e).cloned().unwrap_or_default();
    st.reload_wanted = true;
    world.entity_mut(e).insert(st);
}

/// One tick of every crossbow holder: aim, the presses (taken from the melee queue), fire and reload.
pub fn crossbows_tick(world: &mut World) {
    if !world.get_resource::<crate::weapons::WeaponCombos>().is_some_and(|db| !db.crossbows.is_empty()) {
        return;
    }
    let now = world.resource::<TickCount>().0;
    let actors: Vec<(SimId, Entity)> = world.query_filtered::<(&SimId, Entity), (With<Melee>, With<crate::Inventory>)>().iter(world).map(|(id, e)| (*id, e)).collect();
    for (id, e) in actors {
        let held = held(world, e);
        let mut st = match (world.get::<CrossbowState>(e).cloned(), &held) {
            (Some(st), _) => st,
            (None, Some(_)) => CrossbowState::default(),
            (None, None) => continue,
        };
        st.held = held.clone();
        let Some((uid, spec)) = held else {
            st.action = None;
            st.aiming = false;
            st.reload_wanted = false;
            world.entity_mut(e).insert(st);
            continue;
        };
        let alive = world.get::<crate::melee::Health>(e).is_none_or(|h| h.current > 0.0);
        // The right button aims: no guard with a crossbow. A press goes to the crossbow, not a swing.
        let (pressed, aim_held) = match world.get_mut::<Melee>(e) {
            Some(mut m) => {
                // The raw button (`block_input`) aims; `block_held` (the guard) stays down.
                m.block_held = false;
                (m.queued.take().is_some(), m.block_input)
            }
            None => (false, false),
        };
        let reload_key = std::mem::take(&mut st.reload_wanted);
        let ticks = |s: f32| (s / DT).round() as u32;
        match st.action {
            Some(CrossbowAction::Fire { tick, aimed }) => {
                let len = if aimed { spec.fire.1 } else { spec.fire.0 };
                st.action = (tick + 1 < ticks(len)).then_some(CrossbowAction::Fire { tick: tick + 1, aimed });
            }
            Some(CrossbowAction::Reload { tick }) => {
                let tick = tick + 1;
                if tick == ticks(spec.reload_at) {
                    let have = st.loaded.get(&uid).copied().unwrap_or(0);
                    let got = take_arrows(world, e, spec.ammo_item, spec.max_ammo.saturating_sub(have));
                    st.loaded.insert(uid, have + got);
                    world.resource_mut::<SimEvents>().push(SimEvent::CrossbowLoaded { id, loaded: have + got });
                }
                st.action = (tick < ticks(spec.reload)).then_some(CrossbowAction::Reload { tick });
            }
            None => {}
        }
        let reloading = matches!(st.action, Some(CrossbowAction::Reload { .. }));
        // Drawn after a weapon switch: no shot and no aim until the draw's block ends.
        let drawing = world.get::<crate::weapon_slots::WeaponSlots>(e).and_then(|w| w.switched_at).is_some_and(|t| now < t + u64::from(ticks(spec.draw_block)));
        let pressed = pressed && !drawing;
        st.aiming = alive && aim_held && !reloading && !drawing;
        if alive && st.action.is_none() && now >= st.ready_at && (pressed || reload_key) {
            let loaded = st.loaded.get(&uid).copied().unwrap_or(0);
            if pressed && loaded >= spec.per_shot {
                st.loaded.insert(uid, loaded - spec.per_shot);
                st.action = Some(CrossbowAction::Fire { tick: 0, aimed: st.aiming });
                st.ready_at = now + u64::from(ticks(spec.cooldown));
                fire(world, id, e, &spec, st.aiming);
            } else if loaded < spec.max_ammo && arrows(world, e, spec.ammo_item) > 0 {
                st.action = Some(CrossbowAction::Reload { tick: 0 });
                st.aiming = false;
                world.resource_mut::<SimEvents>().push(SimEvent::CrossbowReloadStarted { id });
            } else if loaded < spec.per_shot {
                world.resource_mut::<SimEvents>().push(SimEvent::CrossbowEmpty { id });
            }
        }
        world.entity_mut(e).insert(st);
    }
}

/// Looses a bolt along the aim (scattered unless aiming).
fn fire(world: &mut World, id: SimId, e: Entity, spec: &CrossbowSpec, aimed: bool) {
    let (Some(from), Some(c)) = (world.get::<crate::Position>(e).map(|p| p.0), world.get::<crate::Combatant>(e).copied()) else { return };
    let aim = world.get::<Aim>(e).copied().unwrap_or_default();
    let ((sy, cy), (sp, cp)) = (aim.yaw.sin_cos(), aim.pitch.sin_cos());
    let forward = Vec3::new(cp * cy, cp * sy, sp);
    let dir = if aimed || spec.scatter_degrees <= 0.0 {
        forward
    } else {
        // `DecideRot`: a whole-degree angle round the aim, a radius of tan(0..MaxScatterDegree).
        let mut rng = world.resource_mut::<crate::SimRng>();
        let around = (rng.unit_f64() * 359.0).floor() as f32;
        let off = (rng.unit_f64() as f32 * spec.scatter_degrees).to_radians().tan();
        let right = Vec3::new(sy, -cy, 0.0);
        let up = right.cross(forward);
        (forward + (right * around.to_radians().cos() + up * around.to_radians().sin()) * off).normalize_or(forward)
    };
    let s = &spec.shot;
    let speed = if s.accel > 0.0 && s.max_speed > s.speed { vec![(0.0, s.speed), ((s.max_speed - s.speed) / s.accel, s.max_speed)] } else { vec![(0.0, s.speed)] };
    let shot = crate::projectiles::ProjectileSpec {
        speed,
        lifetime: s.lifetime.unwrap_or(10.0),
        radius: s.radius,
        turn_rate: 0.0,
        gravity: s.gravity,
        attack: crate::buffs::attack_of(world, e, c.attack),
        factor: s.factor,
        element: crate::combat::Element::Physical,
        by: HitSource::Bolt,
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: spec.bolt_slow.map(|(rate, secs)| (crate::buffs::BuffKind::Slow(rate), secs)),
        burst: s.burst,
        fuse: false,
        ignore_guard: false,
        knockback: None,
    };
    let pos = from + Vec3::Z * crate::overrides::class_skills::SHOT_HEIGHT + forward * (crate::movement::CAPSULE_RADIUS + 10.0);
    crate::projectiles::spawn_with_falloff(world, id, shot, pos, dir, spec.falloff.clone(), spec.bolt_burn);
    world.resource_mut::<SimEvents>().push(SimEvent::CrossbowFired { id, aimed });
}

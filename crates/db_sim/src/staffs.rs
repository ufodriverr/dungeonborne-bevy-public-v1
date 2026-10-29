//! Staffs: a staff is a weak blunt melee weapon (tap: `GA_TGF_Staff_Combo1/2`) plus its element's spell,
//! powered by the staff's own energy (weapon ammo). Fire holds a beam (`GA_TGF_Staff_Laser`), Thunder aims a
//! ground point while held and calls a thundercloud on release (`GA_TGF_Staff_Thunder`); holding the charge
//! input recharges (`GA_TGF_AdditionalRecoverAmmo_Staff*`). Ice staffs grant only the melee combo (their
//! `WAID_Staff_Ice` grants `AbilitySet_Staff_Common` alone and `AbilitySet_Staff_Ice` is empty: unfinished in
//! the original), so they stay melee.
//!
//! CONFIRMED (values from the decoded weapon definitions, ability sets, abilities, effects and montages, read
//! by [`load_staff_db`]; the weapon definitions' and abilities' defaults are non-strict "loose" reads):
//! - energy per weapon: `MaxAmmo`, `AmmoAutoRecover` added once a second (`GA_TGF_AutoRecoverAmmo`'s 1 s looping
//!   timer), `AmmoDisplayCnt` HUD segments; recharge while held adds `ceil(AmmoAutoRecover × AddiRecoverRate)` a
//!   second on top (bytecode), 4.0 for Fire, 2.0 (inherited) for Thunder;
//! - Fire beam: costs its ability's `TgfAbilityCost_WeaponAmmo` (180) once at activation; needs at least 20 % of
//!   the maximum to start (`K2_CanActivateAbility`); from the montage's `Cast` notify it triggers every
//!   `IntervalTime` (0.2 s), `MaxTime` (8) times: a trace from the eye along the aim up to `Max Distance`
//!   (3000 cm) and the `FireLaser_Normal` effect (0.14 × weapon fire damage) on the character it hits, the 8th
//!   with `FireLaser_Extra` (0.85); damage × (1 + `StaffFire_Distance_DamageRate`(trace distance)) (0 to 20 m,
//!   −0.2 beyond); letting go after the first trigger cancels (the montage's `ChargeCancel` section);
//! - Thunder: costs 450; aims a ground point up to `MaxDistance` (1600 cm) while held (any time: `MaxChargeTime`
//!   999); the release plays `AM_Staff_Skill_Thunder_Release`, whose `CastCharge` notify spawns
//!   `B_TGF_Staff_ThunderArea` there; the area strikes on spawn (next tick) and every `TriggerDeltaTime` (1 s)
//!   until `MaxExistTime` (3.5 s): every character in its capsule (r 180, half height 300, centred
//!   `CapsuleHalfHeight − HeightOffset` above the point) takes `GE_TGF_Damage_Instant_Staff_Thunder` (0.63
//!   thunder); the right button's press cancels (`CancelInputTags`);
//! - melee combo: hit windows and damage factors (0.8, 1.2, blunt) from the combo montages and effects;
//! - the legendary fire staff: a held legendary affix granting `TGF.Ability.StaffFireExtraExplode` turns the
//!   last trigger into a burst at the beam's end (`ExtraExplodeRadius` 100 cm, `FireLaser_Extra`, then 2 burn
//!   stacks of `Common.BuffArg.BurningDamage` every `BurningIntervalBase` s).
//!
//! HYPOTHESIS (`overrides::staffs`): a new staff starts full; the shortest charge before a thunder release
//! counts (the 3rd-person charge montage's `ChargeBegin` section); the ground probe below the aim point; when
//! the thunder cost is committed (at release); how the area splits damage among many targets (`Min Split Num`,
//! `Min DPHRate`); when a combo or a cast frees the actor (the montages' cancel-tag windows); movement scales
//! applied over the montages' `AnimationMoveScale` windows.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context;
use bevy_ecs::prelude::*;
use bevy_math::{Dir3, Vec2, Vec3};
use db_formats::config::generated::{ConfigTables, EnGearEquipSubType, EnItemWearingPart};
use serde_json::Value;

use crate::combat::{AttackTerm, Element, HitZone, resolve_hit};
use crate::id::SimIndex;
use crate::melee::{Aim, Combatant, Health, Melee, Swing, blocked_damage};
use crate::movement::{CAPSULE_HEIGHT, CAPSULE_RADIUS};
use crate::overrides::staffs as tune;
use crate::skills::HitSource;
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng, TickCount};

/// The eye above the capsule centre (cm): `BP_Tgf_Hero_Default` `BaseEyeHeight`. The beam and the thunder aim
/// trace from the view (`bTraceFromViewPoint`).
pub const EYE_HEIGHT: f32 = 80.773;
/// Seconds between recoveries: `GA_TGF_AutoRecoverAmmo` sets a 1 s looping timer (CONFIRMED bytecode).
pub const RECOVER_INTERVAL: f32 = 1.0;
/// Share of the maximum energy the beam needs to start: `GA_TGF_Staff_Laser`'s `K2_CanActivateAbility` compares
/// the current ammo with max × 0.2 (CONFIRMED bytecode).
pub const BEAM_START_SHARE: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StaffElement {
    Fire,
    Ice,
    Thunder,
}

/// What a staff hit was (for `HitSource::Staff`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StaffHit {
    /// A beam trigger (`FireLaser_Normal`).
    Beam,
    /// The beam's last trigger (`FireLaser_Extra`).
    BeamExtra,
    /// A thundercloud strike.
    Thunder,
    /// The legendary fire staff's burst.
    Explode,
    /// A burn tick (the burst's burn).
    Burn,
}

/// One melee combo swing (`AM_Staff_Combo<n>_1st`, `GE_TGF_Damage_Staff_Combo<n>`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComboSpec {
    /// Hit window (s, `AGR_Attack_NotifyState`).
    pub window: (f32, f32),
    pub factor: f64,
    /// When the next combo may cut in (s, the `CanQuickSide2` tag operation); Combo1 only.
    pub chain: Option<(f32, f32)>,
    /// When the swing frees the actor (s): the montage's cancel window that lets `Charge`/`Skills` abilities in.
    pub free_at: f32,
    pub length: f32,
    pub clip: String,
    /// The montage's slot segments (clip, segment): montage time to clip time ([`crate::monsters::clip_time`]).
    pub segments: Vec<(String, crate::monsters::MontageSegment)>,
    /// A weapon swing's hit windows with their damage factors (s; one per hand), where it has several.
    pub hits: Vec<(f32, f32, f64)>,
    /// A weapon swing's `QuickSide<n>`, and whether it opens the combo (`GA_TGF_MeleeWeapon_Combo_First_Base`).
    pub n: u8,
    pub opener: bool,
    /// The `CanQuickSide<n>` windows its montage grants (n, s, s): the follow-ups that may cut in then.
    pub grants: Vec<(u8, f32, f32)>,
    /// The montage's play-rate windows (montage time to real time; the times above are real).
    pub rates: PlayRates,
    /// The rebound montage a hit switches to (`ReboundMontageClient`), when it chains the combo itself (the mace's
    /// follow-ups come only from it).
    pub rebound: Option<Rebound>,
    /// The montage's `TGF.AbilityType.Active.Single.SwitchSlot` window (real s): a weapon switch may cut the swing
    /// there.
    pub switch_window: Option<(f32, f32)>,
    /// A guard counter: `Some(true)` the perfect guard's heavy one.
    pub counter: Option<bool>,
    /// The thunder chain it calls (the longsword 5001's heavy counter).
    pub lightning: Option<crate::weapons::LightningSpec>,
    /// Its hit value: its damage effects' `ChargeRollback`, both hands' added where they strike together.
    pub hit_value: f32,
}

/// A swing's rebound montage (the mace): played from the hit on, with its own `CanQuickSide<n>` windows and free
/// time (s from the hit).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rebound {
    pub grants: Vec<(u8, f32, f32)>,
    pub free_at: f32,
    pub length: f32,
    pub segments: Vec<(String, crate::monsters::MontageSegment)>,
    pub rates: PlayRates,
}

/// A montage's `TgfMontagePlayRate` windows: (start, end, rate) in montage time. CONFIRMED: the notify states and
/// their `PlayRate`; HYPOTHESIS: the rate holds for the state's span and the montage plays at 1 outside.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayRates(pub Vec<(f32, f32, f32)>);

impl PlayRates {
    /// Seconds into the montage's play at montage time `t`.
    pub fn real(&self, t: f32) -> f32 {
        self.0.iter().fold(t, |r, &(a, b, rate)| {
            let over = (t.min(b) - a).max(0.0);
            r - over + over / rate.max(1e-3)
        })
    }

    /// The montage time shown `r` s into the play (the inverse of [`PlayRates::real`]).
    pub fn montage(&self, r: f32) -> f32 {
        let mut ws = self.0.clone();
        ws.sort_by(|x, y| x.0.total_cmp(&y.0));
        let (mut t, mut real) = (0.0f32, 0.0f32);
        for (a, b, rate) in ws {
            let a = a.max(t);
            if b <= a {
                continue;
            }
            if r <= real + (a - t) {
                return t + (r - real);
            }
            real += a - t;
            let span = (b - a) / rate.max(1e-3);
            if r <= real + span {
                return a + (r - real) * rate;
            }
            real += span;
            t = b;
        }
        t + (r - real)
    }
}

/// The Fire beam (`GA_TGF_Staff_Laser`, `AM_Staff_Skill_Laser_1st`).
#[derive(Debug, Clone, PartialEq)]
pub struct BeamSpec {
    pub range: f32,
    pub interval: f32,
    pub triggers: u32,
    pub normal: f64,
    pub extra: f64,
    pub element: Element,
    /// `StaffFire_Distance_DamageRate` keys (cm, rate), linear.
    pub rate_curve: Vec<(f32, f32)>,
    /// Share of the maximum energy needed to start (`K2_CanActivateAbility`).
    pub start_share: f64,
    /// Montage times (s): the `Cast` and `End` notifies, the `ChargeCancel` section and its length, the
    /// cancel-tag window's start, the move-scale window and scale, the fire clip's play rate.
    pub cast_at: f32,
    pub end_at: f32,
    pub cancel_at: f32,
    pub cancel_len: f32,
    pub free_at: f32,
    pub move_window: (f32, f32),
    pub move_scale: f32,
    pub clip_rate: f32,
}

/// The thundercloud area (`B_TGF_Staff_ThunderArea`).
#[derive(Debug, Clone, PartialEq)]
pub struct AreaSpec {
    pub radius: f32,
    pub half_height: f32,
    pub height_offset: f32,
    pub interval: f32,
    pub life: f32,
    pub min_split: f64,
    pub min_rate: f64,
    pub factor: f64,
    pub element: Element,
    /// `TriggerImmediately`: it strikes the tick after it appears, then every `interval`; otherwise first at
    /// `interval` (the Wendigo's thunder area, the only class without it: 2 s, as its warning effect ends).
    pub immediate: bool,
}

/// The Thunder spell (`GA_TGF_Staff_Thunder`, its charge / release / fail montages).
#[derive(Debug, Clone, PartialEq)]
pub struct ThunderSpec {
    pub range: f32,
    pub area: AreaSpec,
    /// Charge montage: the `Loop` section's start and the loop's length (s), its move scale.
    pub ready_at: f32,
    pub loop_len: f32,
    pub charge_scale: f32,
    /// Release montage: the `CastCharge` notify, its length, when it frees the actor.
    pub cast_at: f32,
    pub release_len: f32,
    pub release_free: f32,
    /// Fail montage: length and free time.
    pub fail_len: f32,
    pub fail_free: f32,
    /// Where the release clip starts in `A_Staff_Skill_Thunder_1st` (s).
    pub release_clip_start: f32,
}

/// Recharge (`GA_TGF_AdditionalRecoverAmmo_Staff*`, `AM_Staff_Recharge*_1st`).
#[derive(Debug, Clone, PartialEq)]
pub struct RechargeSpec {
    pub rate: f64,
    /// Montage sections (s): `Loop` start, `End` start, length; move scale.
    pub loop_at: f32,
    pub end_at: f32,
    pub length: f32,
    pub move_scale: f32,
}

/// Everything a staff item does.
#[derive(Debug, Clone, PartialEq)]
pub struct StaffSpec {
    pub item_id: i32,
    pub element: StaffElement,
    pub max: u32,
    pub recover: u32,
    pub cost: u32,
    pub segments: u8,
    pub combo: [ComboSpec; 2],
    pub beam: Option<BeamSpec>,
    pub thunder: Option<ThunderSpec>,
    pub recharge: Option<RechargeSpec>,
    /// The burst at the beam's last trigger: legendary fire staffs whose held affix grants
    /// `TGF.Ability.StaffFireExtraExplode` (181055001).
    pub explode: Option<ExplodeSpec>,
}

/// The legendary fire staff's burst (`GA_TGF_Staff_Laser` with `TGF.Ability.StaffFireExtraExplode`): at the
/// beam's end on its last trigger, instead of that trigger's hit.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplodeSpec {
    /// `ExtraExplodeRadius`, cm.
    pub radius: f32,
    pub burn: BurnSpec,
}

/// A burn (`Add Burning Buff Arg` -> `GE_TGF_BuffArg_Burning`): `damage` per stack (`Common.BuffArg.BurningDamage`),
/// one stack every `interval` s (`Common.BuffArg.BurningIntervalBase`), `stacks` of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnSpec {
    pub damage: f64,
    pub interval: f32,
    pub stacks: u32,
}

impl StaffSpec {
    /// The spell's energy cost, if the staff has one.
    pub fn casts(&self) -> bool {
        self.beam.is_some() || self.thunder.is_some()
    }

    /// Energy a second while recharging on top of the auto recovery (bytecode: `FCeil(AutoRecover × AddiRecoverRate)`).
    pub fn recharge_bonus(&self) -> u32 {
        self.recharge.as_ref().map_or(0, |r| (f64::from(self.recover) * r.rate).ceil() as u32)
    }
}

/// Every staff item's spec, by item id (inserted by the caller, like `GameData`).
#[derive(Resource, Debug, Default, Clone)]
pub struct StaffDb(pub BTreeMap<i32, Arc<StaffSpec>>);

/// What a staff holder is doing.
#[derive(Debug, Clone, PartialEq)]
pub enum StaffAction {
    /// The Fire beam: ticks since it started, triggers so far, where the trace ends and what it hits, the tick
    /// a cancel began, whether the input was let go.
    Beam { tick: u32, triggers: u32, end: Vec3, target: Option<SimId>, cancel: Option<u32>, released: bool },
    /// Thunder aim: ticks held and the ground point (none: no valid point).
    ThunderAim { tick: u32, point: Option<Vec3> },
    ThunderRelease { tick: u32, point: Vec3 },
    /// Released too early, without a point, or cancelled.
    ThunderFail { tick: u32 },
    /// Recharging: ticks since it started and the tick its end section began.
    Recharge { tick: u32, ending: Option<u32> },
}

/// A staff holder's state: energy per staff, the held staff, the action, inputs.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct StaffState {
    /// Energy of every staff the actor has held, by item uid.
    pub energy: BTreeMap<u32, u32>,
    /// The held staff's uid and spec.
    pub held: Option<(u32, Arc<StaffSpec>)>,
    pub action: Option<StaffAction>,
    /// The cast input (left button held past the hold threshold) and a start not yet tried.
    pub cast_held: bool,
    pub cast_pending: bool,
    pub recharge_held: bool,
    /// Ticks to the next recovery tick.
    pub recover_in: u32,
    /// The last combo swing: index and the tick it started.
    pub last_combo: Option<(u8, u64)>,
}

impl StaffState {
    pub fn element(&self) -> Option<StaffElement> {
        self.held.as_ref().map(|h| h.1.element)
    }

    /// The held staff's energy and maximum.
    pub fn held_energy(&self) -> Option<(u32, u32)> {
        let (uid, spec) = self.held.as_ref()?;
        Some((self.energy.get(uid).copied().unwrap_or(0), spec.max))
    }

    /// Movement scale of the action (the montages' `AnimationMoveScale` notifies).
    pub fn move_factor(&self) -> f32 {
        let Some((_, spec)) = &self.held else { return 1.0 };
        let secs = |t: u32| t as f32 * DT;
        match &self.action {
            Some(StaffAction::Beam { tick, cancel: None, .. }) => spec.beam.as_ref().map_or(1.0, |b| if (b.move_window.0..b.move_window.1).contains(&secs(*tick)) { b.move_scale } else { 1.0 }),
            Some(StaffAction::ThunderAim { .. }) => spec.thunder.as_ref().map_or(1.0, |t| t.charge_scale),
            Some(StaffAction::Recharge { .. }) => spec.recharge.as_ref().map_or(1.0, |r| r.move_scale),
            _ => 1.0,
        }
    }
}

/// A thundercloud in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct ThunderArea {
    pub owner: SimId,
    /// The ground point it was called on.
    pub at: Vec3,
    pub spec: AreaSpec,
    pub attack: crate::combat::AttackStats,
    /// The tick it appeared on.
    pub born: u64,
    pub strikes: u32,
    /// What its hits count as (a monster's storm is its attack's).
    pub by: HitSource,
}

/// Thunderclouds in the world, in spawn order.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct ThunderAreas(pub Vec<ThunderArea>);

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

/// `StaffFire_Distance_DamageRate` at `d` (linear between keys, flat outside), clamped to [-1, 0] as the
/// bytecode does.
pub fn distance_rate(curve: &[(f32, f32)], d: f32) -> f64 {
    let v = match curve {
        [] => 0.0,
        [first, ..] if d <= first.0 => first.1,
        [.., last] if d >= last.0 => last.1,
        _ => {
            let i = curve.windows(2).position(|w| d >= w[0].0 && d <= w[1].0).unwrap_or(0);
            let (a, b) = (curve[i], curve[i + 1]);
            a.1 + (b.1 - a.1) * (d - a.0) / (b.0 - a.0).max(1e-6)
        }
    };
    f64::from(v).clamp(-1.0, 0.0)
}

fn aim_dir(aim: &Aim) -> Vec3 {
    let (sy, cy) = aim.yaw.sin_cos();
    let (sp, cp) = aim.pitch.sin_cos();
    Vec3::new(cy * cp, sy * cp, sp)
}

/// The actor's held staff, from its right-hand item and the [`StaffDb`].
fn held_staff(world: &World, e: Entity) -> Option<(u32, Arc<StaffSpec>)> {
    let db = world.get_resource::<StaffDb>()?;
    let item = world.get::<crate::Inventory>(e)?.worn.get(&EnItemWearingPart::Pair1rightweapon)?;
    db.0.get(&item.item.item_id).map(|s| (item.uid, s.clone()))
}

/// Sets an actor's staff inputs (`SimCommand::StaffCast` / `StaffRecharge`).
pub(crate) fn set_input(world: &mut World, id: SimId, cast: Option<bool>, recharge: Option<bool>) {
    let Some(e) = world.resource::<SimIndex>().get(id) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
        return;
    };
    if world.get::<StaffState>(e).is_none() {
        world.entity_mut(e).insert(StaffState::default());
    }
    let mut cancel = false;
    let mut release = false;
    {
        let mut s = world.get_mut::<StaffState>(e).expect("inserted above");
        if let Some(on) = cast {
            if on && !s.cast_held {
                s.cast_pending = true;
            }
            if !on && s.cast_held {
                s.cast_pending = false;
                release = true;
            }
            s.cast_held = on;
        }
        if let Some(on) = recharge {
            // The right button's press cancels a thunder aim (`CancelInputTags` `FireRightByPress`).
            cancel = on && !s.recharge_held && matches!(s.action, Some(StaffAction::ThunderAim { .. }));
            s.recharge_held = on;
        }
    }
    if release {
        let_go(world, id, e);
    }
    if cancel {
        thunder_fail(world, id, e);
    }
}

/// The cast input was let go: the beam cancels (after its first trigger), the thunder releases or fails.
fn let_go(world: &mut World, id: SimId, e: Entity) {
    let Some(mut s) = world.get::<StaffState>(e).cloned() else { return };
    let Some((_, spec)) = s.held.clone() else { return };
    match s.action.clone() {
        Some(StaffAction::Beam { tick, triggers, end, target, cancel: None, .. }) => {
            if triggers >= 1 && triggers < spec.beam.as_ref().map_or(0, |b| b.triggers) {
                s.action = Some(StaffAction::Beam { tick, triggers, end, target, cancel: Some(tick), released: true });
                world.resource_mut::<SimEvents>().push(SimEvent::StaffBeamEnded { id, cancelled: true });
            } else {
                s.action = Some(StaffAction::Beam { tick, triggers, end, target, cancel: None, released: true });
            }
            world.entity_mut(e).insert(s);
        }
        Some(StaffAction::ThunderAim { tick, point }) => {
            let t = spec.thunder.as_ref().expect("thunder aim needs a thunder spec");
            let uid = s.held.as_ref().map_or(0, |h| h.0);
            let energy = s.energy.get(&uid).copied().unwrap_or(0);
            match point {
                Some(point) if (!tune::THUNDER_RELEASE_NEEDS_LOOP || tick as f32 * DT >= t.ready_at) && energy >= spec.cost => {
                    s.energy.insert(uid, energy - spec.cost);
                    s.action = Some(StaffAction::ThunderRelease { tick: 0, point });
                    world.entity_mut(e).insert(s);
                    let mut ev = world.resource_mut::<SimEvents>();
                    ev.push(SimEvent::StaffEnergy { id, current: energy - spec.cost, max: spec.max });
                    ev.push(SimEvent::StaffThunderReleased { id, at: point });
                }
                _ => thunder_fail(world, id, e),
            }
        }
        _ => {}
    }
}

fn thunder_fail(world: &mut World, id: SimId, e: Entity) {
    if let Some(mut s) = world.get_mut::<StaffState>(e) {
        s.action = Some(StaffAction::ThunderFail { tick: 0 });
    }
    world.resource_mut::<SimEvents>().push(SimEvent::StaffThunderFailed { id });
}

/// A combo swing for a staff holder (`None`: not holding a staff). Combo2 follows Combo1 inside Combo1's chain
/// window, else Combo1.
pub(crate) fn combo_swing(world: &mut World, e: Entity) -> Option<Swing> {
    let now = world.resource::<TickCount>().0;
    let mut s = world.get::<StaffState>(e).cloned()?;
    let (_, spec) = s.held.clone()?;
    let index = match s.last_combo {
        Some((0, at)) => {
            let since = (now - at) as f32 * DT;
            u8::from(spec.combo[0].chain.is_some_and(|(a, b)| since >= a - DT * 0.5 && since <= b))
        }
        _ => 0,
    };
    let c = &spec.combo[usize::from(index)];
    s.last_combo = Some((index, now));
    world.entity_mut(e).insert(s);
    let (a, b) = (ticks(c.window.0), ticks(c.window.1).max(ticks(c.window.0) + 1));
    Some(Swing { hit_value: 0.0, by: HitSource::Swing(crate::AttackKind::Light), windows: vec![(a, b)], length: ticks(c.free_at).max(b), tick: 0, hit: Vec::new(), root: Vec::new(), facing: Vec2::X, factor: c.factor, guard: None, tags: Vec::new(), window_factors: Vec::new() })
}

/// Ray from `from` along `dir` up to `max`: the nearest level geometry and the nearest living character other
/// than `skip` (capsules), as (distance, hit character).
fn trace(world: &mut World, from: Vec3, dir: Vec3, max: f32, skip: Entity) -> (f32, Option<(SimId, Entity)>) {
    let wall = {
        let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
        let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
        Dir3::new(dir).ok().and_then(|d| spatial.cast_ray_predicate(from, d, max, true, &crate::layers::shots(), &|b| boxes.contains(b))).map_or(max, |h| h.distance)
    };
    let to = from + dir * wall;
    let half = CAPSULE_HEIGHT * 0.5 - CAPSULE_RADIUS;
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    let mut best: Option<(f32, SimId, Entity)> = None;
    for (tid, te) in ids {
        if te == skip || world.get::<Combatant>(te).is_none() || world.get::<Health>(te).is_none_or(|h| h.current <= 0.0) {
            continue;
        }
        let Some(c) = world.get::<Position>(te).map(|p| p.0) else { continue };
        let (dist, s) = crate::projectiles::segment_distance(from, to, c - Vec3::Z * half, c + Vec3::Z * half);
        if dist <= CAPSULE_RADIUS && best.is_none_or(|b| s * wall < b.0) {
            best = Some((s * wall, tid, te));
        }
    }
    match best {
        Some((d, tid, te)) => (d, Some((tid, te))),
        None => (wall, None),
    }
}

/// Whether `te` is on the caster's side: the caster itself or one of its summons (the staff hits enemies only:
/// the bytecode's `CompareTeams` `DifferentTeams`).
pub(crate) fn friendly(world: &World, caster: SimId, tid: SimId, te: Entity) -> bool {
    tid == caster || world.get::<crate::summon::Summoned>(te).is_some_and(|s| s.by == caster)
}

/// The ground point a thunder aim picks: where the view ray meets geometry or a character (within range), or
/// the ray's end, then straight down to the floor within the probe (HYPOTHESIS, `overrides::staffs`).
fn ground_point(world: &mut World, e: Entity, range: f32) -> Option<Vec3> {
    let (pos, aim) = (world.get::<Position>(e)?.0, world.get::<Aim>(e).copied().unwrap_or_default());
    let eye = pos + Vec3::Z * EYE_HEIGHT;
    let dir = aim_dir(&aim);
    let (d, hit) = trace(world, eye, dir, range, e);
    let mut p = eye + dir * d;
    if let Some((_, te)) = hit
        && let Some(c) = world.get::<Position>(te).map(|q| q.0)
    {
        p = Vec3::new(p.x, p.y, c.z);
    }
    // Back off the surface a little so the probe starts above a floor it hit.
    let start = p - dir * 2.0 + Vec3::Z * 5.0;
    let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
    let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
    spatial.cast_ray_predicate(start, Dir3::NEG_Z, tune::THUNDER_GROUND_PROBE, true, &crate::layers::walk(), &|b| boxes.contains(b)).map(|h| start - Vec3::Z * h.distance)
}

/// One staff spell hit: `resolve_hit` × `factor` × `rate`, a block facing the caster, the target's damage rules.
#[allow(clippy::too_many_arguments)]
fn strike(world: &mut World, id: SimId, e: Entity, tid: SimId, te: Entity, element: Element, factor: f64, from: Vec3, by: StaffHit, attack: Option<crate::combat::AttackStats>) {
    strike_as(world, id, e, tid, te, element, factor, from, HitSource::Staff(by), attack);
}

/// [`strike`] counted as `by` (a monster's area).
#[allow(clippy::too_many_arguments)]
pub(crate) fn strike_as(world: &mut World, id: SimId, e: Entity, tid: SimId, te: Entity, element: Element, factor: f64, from: Vec3, by: HitSource, attack: Option<crate::combat::AttackStats>) {
    let (Some(defender), Some(to)) = (world.get::<Combatant>(te).copied(), world.get::<Position>(te).map(|p| p.0)) else { return };
    let attack = match attack {
        Some(a) => a,
        None => match world.get::<Combatant>(e).copied() {
            Some(c) => crate::buffs::attack_of(world, e, c.attack),
            None => return,
        },
    };
    let defence = crate::buffs::defence_of(world, te, defender.defence);
    let hit = resolve_hit(&attack, &defence, element, AttackTerm::Weapon, HitZone::Body, &mut world.resource_mut::<SimRng>());
    let mut damage = hit.damage * factor;
    let blocking = world.get::<Melee>(te).is_some_and(Melee::blocking);
    if blocking {
        let taim = world.get::<Aim>(te).copied().unwrap_or_default();
        damage = blocked_damage(damage, &defender, &taim, world.get::<Melee>(te).and_then(|m| m.guard_angles), to, from);
    }
    let owner = world.resource::<SimIndex>().get(id);
    let damage = crate::buffs::incoming_damage(world, te, damage, owner);
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    {
        let mut events = world.resource_mut::<SimEvents>();
        events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by });
        if left <= 0.0 {
            events.push(SimEvent::Died { id: tid });
        }
    }
    if let Some(oe) = owner {
        crate::buffs::after_damage_dealt(world, id, oe, damage, blocking);
    }
}

/// One beam trigger: the trace, damage on the character hit (the last trigger with the extra effect; with the
/// legendary burst, the burst instead: bytecode `!(TriggerCount >= MaxTime && HasMatchingGameplayTag)`).
fn beam_trigger(world: &mut World, id: SimId, e: Entity, b: &BeamSpec, explode: Option<&ExplodeSpec>, n: u32) -> (Vec3, Option<SimId>) {
    let (Some(pos), aim) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied().unwrap_or_default()) else { return (Vec3::ZERO, None) };
    let eye = pos + Vec3::Z * EYE_HEIGHT;
    let dir = aim_dir(&aim);
    let (d, hit) = trace(world, eye, dir, b.range, e);
    let extra = n >= b.triggers;
    let to = eye + dir * d;
    let target = hit.filter(|&(tid, te)| !friendly(world, id, tid, te));
    let burst = explode.filter(|_| extra);
    if let Some((tid, te)) = target
        && burst.is_none()
    {
        let rate = distance_rate(&b.rate_curve, d);
        let factor = if extra { b.extra } else { b.normal } * (1.0 + rate);
        strike(world, id, e, tid, te, b.element, factor, eye, if extra { StaffHit::BeamExtra } else { StaffHit::Beam }, None);
    }
    let target = target.map(|t| t.0);
    world.resource_mut::<SimEvents>().push(SimEvent::StaffBeamTick { id, from: eye, to, target, extra });
    if let Some(x) = burst {
        burst_at(world, id, e, b, x, to);
    }
    (to, target)
}

/// The legendary burst at the beam's end (`ExplodeCenter` = the laser effect's location, its beam end): every
/// non-friendly character within the radius takes `FireLaser_Extra` x (1 + the distance rate from the caster) and,
/// if alive, burns. HYPOTHESIS (`GetExplodeHitActors` is native): the radius reaches the character's capsule,
/// walls don't block it.
fn burst_at(world: &mut World, id: SimId, e: Entity, b: &BeamSpec, x: &ExplodeSpec, at: Vec3) {
    let me = world.get::<Position>(e).map_or(at, |p| p.0);
    let half = CAPSULE_HEIGHT * 0.5 - CAPSULE_RADIUS;
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    let mut hits = Vec::new();
    for (tid, te) in ids {
        if world.get::<Combatant>(te).is_none() || world.get::<Health>(te).is_none_or(|h| h.current <= 0.0) || friendly(world, id, tid, te) {
            continue;
        }
        let Some(p) = world.get::<Position>(te).map(|p| p.0) else { continue };
        let near = Vec3::new(p.x, p.y, at.z.clamp(p.z - half, p.z + half));
        if at.distance(near) <= x.radius + CAPSULE_RADIUS {
            hits.push((tid, te, p));
        }
    }
    world.resource_mut::<SimEvents>().push(SimEvent::StaffExplode { id, at, radius: x.radius, hits: hits.len() as u32 });
    for (tid, te, p) in hits {
        let rate = distance_rate(&b.rate_curve, me.distance(p));
        strike(world, id, e, tid, te, b.element, b.extra * (1.0 + rate), at, StaffHit::Explode, None);
        if world.get::<Health>(te).is_some_and(|h| h.current > 0.0) {
            crate::buffs::add_burn(world, te, id, x.burn);
        }
    }
}

/// The staff's action stops now (a stun, death, the staff taken out of the hand), with the events that end it.
fn stop_action(world: &mut World, id: SimId, s: &mut StaffState) {
    let mut ev = world.resource_mut::<SimEvents>();
    match s.action.take() {
        Some(StaffAction::Beam { cancel: None, .. }) => ev.push(SimEvent::StaffBeamEnded { id, cancelled: true }),
        Some(StaffAction::Recharge { .. }) => ev.push(SimEvent::StaffRechargeEnded { id }),
        Some(StaffAction::ThunderAim { .. }) => ev.push(SimEvent::StaffThunderFailed { id }),
        _ => {}
    }
}

/// Staff holders in id order, then the thunderclouds.
pub(crate) fn staffs_tick(world: &mut World) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    let now = world.resource::<TickCount>().0;
    for &(id, e) in &ids {
        // Players with a staff in hand get their state on first sight (so energy recovers without input).
        if world.get::<StaffState>(e).is_none() {
            if held_staff(world, e).is_none() {
                continue;
            }
            world.entity_mut(e).insert(StaffState::default());
        }
        let held = held_staff(world, e);
        let mut s = world.get::<StaffState>(e).cloned().expect("present");
        // The held staff changed: the action stops, the new one's recovery timer starts (granted on
        // equip).: the end events go out as for a stun (a beam swapped away mid-cast kept its fire loop
        // sound).
        if s.held.as_ref().map(|h| h.0) != held.as_ref().map(|h| h.0) {
            stop_action(world, id, &mut s);
            s.recover_in = ticks(RECOVER_INTERVAL);
            if let Some((uid, spec)) = &held {
                s.energy.entry(*uid).or_insert(if tune::NEW_STAFF_FULL { spec.max } else { 0 });
            }
            s.held = held;
        }
        let Some((uid, spec)) = s.held.clone() else {
            world.entity_mut(e).insert(s);
            continue;
        };
        let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
        let stunned = world.get::<crate::buffs::Buffs>(e).is_some_and(|b| b.stunned() || b.rooted());
        if !alive || stunned {
            stop_action(world, id, &mut s);
            s.cast_pending = false;
        }
        // Recovery once a second; while recharging the bonus on top.
        s.recover_in = s.recover_in.saturating_sub(1);
        if s.recover_in == 0 {
            s.recover_in = ticks(RECOVER_INTERVAL);
            let bonus = if matches!(s.action, Some(StaffAction::Recharge { .. })) { spec.recharge_bonus() } else { 0 };
            let cur = s.energy.get(&uid).copied().unwrap_or(0);
            let next = (cur + spec.recover + bonus).min(spec.max);
            if next != cur {
                s.energy.insert(uid, next);
                world.resource_mut::<SimEvents>().push(SimEvent::StaffEnergy { id, current: next, max: spec.max });
            }
        }
        let swinging = world.get::<Melee>(e).is_some_and(|m| m.swing.is_some());
        let free = alive && !stunned && s.action.is_none() && !swinging;
        // Start a cast.
        if free && s.cast_pending && s.cast_held && spec.casts() {
            s.cast_pending = false;
            let energy = s.energy.get(&uid).copied().unwrap_or(0);
            if let Some(b) = &spec.beam {
                if f64::from(energy) >= f64::from(spec.max) * b.start_share && energy >= spec.cost {
                    s.energy.insert(uid, energy - spec.cost);
                    let end = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
                    s.action = Some(StaffAction::Beam { tick: 0, triggers: 0, end, target: None, cancel: None, released: false });
                    let mut ev = world.resource_mut::<SimEvents>();
                    ev.push(SimEvent::StaffEnergy { id, current: energy - spec.cost, max: spec.max });
                    ev.push(SimEvent::StaffCastStarted { id, element: spec.element });
                } else {
                    world.resource_mut::<SimEvents>().push(SimEvent::StaffRejected { id });
                }
            } else if spec.thunder.is_some() {
                if energy >= spec.cost {
                    s.action = Some(StaffAction::ThunderAim { tick: 0, point: None });
                    world.resource_mut::<SimEvents>().push(SimEvent::StaffCastStarted { id, element: spec.element });
                } else {
                    world.resource_mut::<SimEvents>().push(SimEvent::StaffRejected { id });
                }
            }
        } else if !spec.casts() {
            s.cast_pending = false;
        }
        // Start recharging (only when the cast input isn't asking).
        if free && s.action.is_none() && s.recharge_held && !s.cast_pending && spec.recharge.is_some() {
            s.action = Some(StaffAction::Recharge { tick: 0, ending: None });
            world.resource_mut::<SimEvents>().push(SimEvent::StaffRechargeStarted { id });
        }
        world.entity_mut(e).insert(s);
        advance(world, id, e, &spec, now);
        // While a staff action runs, attack and block input do nothing (the abilities block each other).
        let busy = world.get::<StaffState>(e).is_some_and(|s| s.action.is_some());
        if let Some(mut m) = world.get_mut::<Melee>(e) {
            if busy && m.swing.is_none() {
                m.queued = None;
            }
            // A staff has no guard (`defend_guarded_damage_reduce_rate` 0); the right button recharges.
            m.block_held = false;
        }
        chain_combo(world, id, e, &spec);
    }
    aoe_tick(world);
    areas_tick(world);
    crate::aura_areas::tick(world);
}

/// Combo1's chain window: a queued tap starts Combo2 at once.
fn chain_combo(world: &mut World, id: SimId, e: Entity, spec: &StaffSpec) {
    let now = world.resource::<TickCount>().0;
    let Some(m) = world.get::<Melee>(e) else { return };
    let (Some(swing), Some(_)) = (m.swing.as_ref(), m.queued) else { return };
    let Some(chain) = spec.combo[0].chain else { return };
    let Some((0, at)) = world.get::<StaffState>(e).and_then(|s| s.last_combo) else { return };
    let since = (now - at) as f32 * DT;
    if swing.by != HitSource::Swing(crate::AttackKind::Light) || since < chain.0 - DT * 0.5 || since > chain.1 {
        return;
    }
    let Some(next) = combo_swing(world, e) else { return };
    if let Some(mut m) = world.get_mut::<Melee>(e) {
        let kind = m.queued.take().unwrap_or(crate::AttackKind::Light);
        m.swing = Some(next);
        world.resource_mut::<SimEvents>().push(SimEvent::SwingStarted { id, kind });
    }
}

/// One tick of the actor's staff action.
fn advance(world: &mut World, id: SimId, e: Entity, spec: &StaffSpec, now: u64) {
    let Some(mut s) = world.get::<StaffState>(e).cloned() else { return };
    let Some(action) = s.action.clone() else { return };
    match action {
        StaffAction::Beam { tick, mut triggers, mut end, mut target, mut cancel, released } => {
            let b = spec.beam.as_ref().expect("beam action needs a beam spec");
            let t = tick + 1;
            if cancel.is_none() {
                // Triggers from the `Cast` notify, every `IntervalTime`, `MaxTime` of them.
                let first = ticks(b.cast_at);
                let every = ticks(b.interval).max(1);
                if t >= first && (t - first).is_multiple_of(every) && triggers < b.triggers {
                    if released && triggers >= 1 {
                        // Let go before: the next trigger cancels (`OnTriggerEffect` -> `ReleaseInput`).
                        cancel = Some(t);
                        world.resource_mut::<SimEvents>().push(SimEvent::StaffBeamEnded { id, cancelled: true });
                    } else {
                        triggers += 1;
                        (end, target) = beam_trigger(world, id, e, b, spec.explode.as_ref(), triggers);
                    }
                } else if t >= first && t < ticks(b.end_at) {
                    // The beam follows the aim between triggers (presentation; damage only on triggers).
                    if let (Some(pos), Some(aim)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied()) {
                        let eye = pos + Vec3::Z * EYE_HEIGHT;
                        let dir = aim_dir(&aim);
                        let (d, hit) = trace(world, eye, dir, b.range, e);
                        end = eye + dir * d;
                        target = hit.map(|h| h.0);
                    }
                }
                if t == ticks(b.end_at) && triggers >= b.triggers {
                    world.resource_mut::<SimEvents>().push(SimEvent::StaffBeamEnded { id, cancelled: false });
                }
            }
            let done = match cancel {
                Some(c) => t >= c + ticks(b.cancel_len),
                None => t >= ticks(b.free_at).min(ticks(b.cancel_at)),
            };
            // A finished beam frees the actor at its montage's cancel window (the rest of the clip plays inside it).
            s.action = if done { None } else { Some(StaffAction::Beam { tick: t, triggers, end, target, cancel, released }) };
        }
        StaffAction::ThunderAim { tick, .. } => {
            let th = spec.thunder.as_ref().expect("thunder aim needs a thunder spec");
            let point = ground_point(world, e, th.range);
            s.action = Some(StaffAction::ThunderAim { tick: tick + 1, point });
        }
        StaffAction::ThunderRelease { tick, point } => {
            let th = spec.thunder.as_ref().expect("thunder release needs a thunder spec");
            let t = tick + 1;
            if t == ticks(th.cast_at).max(1) {
                let attack = world.get::<Combatant>(e).copied().map(|c| crate::buffs::attack_of(world, e, c.attack)).unwrap_or_default();
                world.resource_mut::<ThunderAreas>().0.push(ThunderArea { owner: id, at: point, spec: th.area.clone(), attack, born: now, strikes: 0, by: HitSource::Staff(StaffHit::Thunder) });
                world.resource_mut::<SimEvents>().push(SimEvent::ThunderAreaSpawned { owner: id, at: point, radius: th.area.radius, life: th.area.life });
            }
            s.action = if t >= ticks(th.release_free) { None } else { Some(StaffAction::ThunderRelease { tick: t, point }) };
        }
        StaffAction::ThunderFail { tick } => {
            let th = spec.thunder.as_ref().expect("thunder fail needs a thunder spec");
            let t = tick + 1;
            s.action = if t >= ticks(th.fail_free) { None } else { Some(StaffAction::ThunderFail { tick: t }) };
        }
        StaffAction::Recharge { tick, ending } => {
            let r = spec.recharge.as_ref().expect("recharge needs a recharge spec");
            let t = tick + 1;
            let ending = ending.or((!s.recharge_held).then_some(t));
            // Its end section lets a cast in (the montage's tag operation lists `Charge`): a cast asked for ends it at once.
            let done = s.cast_pending || ending.is_some_and(|at| t >= at + ticks(r.length - r.end_at));
            if done {
                s.action = None;
                world.resource_mut::<SimEvents>().push(SimEvent::StaffRechargeEnded { id });
            } else {
                s.action = Some(StaffAction::Recharge { tick: t, ending });
            }
        }
    }
    world.entity_mut(e).insert(s);
}

/// Overlap areas whose own `DoTriggerCheck` applies a fixed effect (named in its bytecode, not a property: its
/// `GameplayEffectClass` is null) to each actor its capsule hits, one `ApplyAOEWithGA` per actor, so with no split:
/// (area class, effect)., CONFIRMED from the bytecode.
const OWN_TRIGGER_EFFECTS: &[(&str, &str)] = &[("/B_TGF_Wendigo_ThunderArea", "/Game/TGF/Character/Monster/M_Wendigo/Ability/GE_TGF_Wendigo_Thunder")];

/// A `B_TGF_Static_OverlapCheck_ProjectileBase` area a monster lays (the Shadow King's storms: the Wendigo's
/// thunder): strikes as the Thunder cloud's; its first `GameplayEffectClass` gives the factor and element. A value
/// its class chain leaves unset is the engine default (no `HeightOffset`, no split, not at once).
pub(crate) fn overlap_area(package: &str) -> Option<AreaSpec> {
    let c = chain(package);
    if !is_a(&c, "B_TGF_Static_OverlapCheck_ProjectileBase") {
        return None;
    }
    let own = OWN_TRIGGER_EFFECTS.iter().find(|(class, _)| package.ends_with(class)).map(|o| o.1.to_string());
    let ge = match &own {
        Some(ge) => ge.clone(),
        None => prop(&c, "GameplayEffectClass").and_then(Value::as_array).and_then(|g| g.first()).and_then(obj_path)?,
    };
    let (factor, element) = damage_effect(&ge).ok()?;
    let f = |k: &str| num(&c, k);
    let min_split = if own.is_some() { f64::INFINITY } else { f("Min Split Num").unwrap_or(f64::INFINITY) };
    let immediate = prop(&c, "TriggerImmediately").and_then(Value::as_bool).unwrap_or(false);
    Some(AreaSpec { radius: f("CapsuleRadius")? as f32, half_height: f("CapsuleHalfHeight")? as f32, height_offset: f("HeightOffset").unwrap_or(0.0) as f32, interval: f("TriggerDeltaTime")? as f32, life: f("MaxExistTime")? as f32, min_split, min_rate: f("Min DPHRate").unwrap_or(1.0), factor, element, immediate })
}

/// Monster `id`'s attack `attack` lays its area: `Spray` puts it on the floor `AreaEffectDistance` ahead (0 for the
/// storms: at its feet; HYPOTHESIS: the floor under its capsule), with its current stats.
pub(crate) fn spawn_monster_area(world: &mut World, id: SimId, e: Entity, attack: u8, spec: AreaSpec) {
    let Some(p) = world.get::<Position>(e).map(|p| p.0) else { return };
    spawn_area_at(world, id, e, attack, spec, p - Vec3::Z * crate::movement::CAPSULE_HEIGHT * 0.5);
}

/// Monster `id`'s attack `attack` lays its area on the ground point `at`, with its current stats.
pub(crate) fn spawn_area_at(world: &mut World, id: SimId, e: Entity, attack: u8, spec: AreaSpec, at: Vec3) {
    let Some(c) = world.get::<Combatant>(e).copied() else { return };
    let stats = crate::buffs::attack_of(world, e, c.attack);
    let now = world.resource::<TickCount>().0;
    world.resource_mut::<SimEvents>().push(SimEvent::ThunderAreaSpawned { owner: id, at, radius: spec.radius, life: spec.life });
    world.resource_mut::<ThunderAreas>().0.push(ThunderArea { owner: id, at, spec, attack: stats, born: now, strikes: 0, by: HitSource::Monster(attack) });
}

/// A multi-pass area ability's pass still laying areas one per interval ([`crate::monsters::AoePass`]): its
/// ability's looping timer, which outlives the montage; gone with its caster.
#[derive(Debug, Clone, PartialEq)]
pub struct AoeRun {
    pub owner: SimId,
    pub attack: u8,
    pub pass: crate::monsters::AoePass,
    /// `ValidTargets`, taken at the notify.
    pub targets: Vec<SimId>,
    /// Areas laid so far (`Index`).
    pub laid: u32,
    /// The tick of the next one.
    pub next: u64,
}

#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct AoeRuns(pub Vec<AoeRun>);

/// A pass's notify: its targets are the caster's threat target (one per monster in the sim; HYPOTHESIS for more
/// than one player), then `max` areas at once, or one now and the rest by [`aoe_tick`].
pub(crate) fn start_aoe_pass(world: &mut World, id: SimId, e: Entity, attack: u8, pass: crate::monsters::AoePass) {
    let targets: Vec<SimId> = world.get::<crate::ai::MonsterAi>(e).and_then(|a| a.target).into_iter().collect();
    let now = world.resource::<TickCount>().0;
    let mut run = AoeRun { owner: id, attack, targets, laid: 0, next: now + u64::from(ticks(pass.interval).max(1)), pass };
    if run.pass.interval.abs() <= 1.0e-6 {
        while run.laid < run.pass.max {
            lay_next(world, e, &mut run);
        }
        return;
    }
    lay_next(world, e, &mut run);
    if run.laid < run.pass.max {
        world.resource_mut::<AoeRuns>().0.push(run);
    }
}

fn aoe_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let mut runs = std::mem::take(&mut world.resource_mut::<AoeRuns>().0);
    runs.retain_mut(|r| {
        let Some(e) = world.resource::<SimIndex>().get(r.owner).filter(|&e| world.get::<Health>(e).is_some_and(|h| h.current > 0.0)) else { return false };
        if now >= r.next {
            lay_next(world, e, r);
            r.next = now + u64::from(ticks(r.pass.interval).max(1));
        }
        r.laid < r.pass.max
    });
    world.resource_mut::<AoeRuns>().0.extend(runs);
}

/// `SpawnProjectile` (CONFIRMED bytecode): the next area, on the next target's current spot, past the last by its
/// overflow (`MultiTrigger` the targets again, `RandomPos` within `Radius` of the caster); on the ground found
/// within `GroundCheckDepth` below (`FindProperGroundPos` is native: HYPOTHESIS, a ray down onto the level, else the
/// spot's feet height). No target to cycle: none (the Blueprint would index an empty list).
fn lay_next(world: &mut World, e: Entity, r: &mut AoeRun) {
    if r.laid >= r.pass.max {
        return;
    }
    r.laid += 1;
    let i = r.laid as usize - 1;
    let target_at = |world: &World, t: SimId| world.resource::<SimIndex>().get(t).and_then(|te| world.get::<Position>(te)).map(|p| p.0);
    let spot = match (r.targets.get(i), r.pass.overflow) {
        (Some(&t), _) => target_at(world, t),
        (None, crate::monsters::AoeOverflow::MultiTrigger) if !r.targets.is_empty() => target_at(world, r.targets[i % r.targets.len()]),
        (None, crate::monsters::AoeOverflow::RandomPos) => world.get::<Position>(e).map(|p| p.0).map(|from| {
            let mut rng = world.resource_mut::<SimRng>();
            let d = rng.unit_f32() * r.pass.radius;
            let yaw = (rng.unit_f32() * 360.0).to_radians();
            from + Vec3::new(yaw.cos(), yaw.sin(), 0.0) * d
        }),
        _ => None,
    };
    let Some(spot) = spot else { return };
    let ground = {
        let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
        let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
        spatial.cast_ray_predicate(spot, Dir3::NEG_Z, r.pass.ground_depth, true, &crate::layers::walk(), &|b| boxes.contains(b)).map_or(spot.z - crate::movement::CAPSULE_HEIGHT * 0.5, |h| spot.z - h.distance)
    };
    spawn_area_at(world, r.owner, e, r.attack, r.pass.area.clone(), Vec3::new(spot.x, spot.y, ground));
}

/// The thunderclouds: a strike on the tick after they appear, then every interval until their life ends.
fn areas_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let mut areas = std::mem::take(&mut world.resource_mut::<ThunderAreas>().0);
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    areas.retain_mut(|a| {
        // `TriggerImmediately`: `DelayUntilNextTick`, then the `TriggerDeltaTime` looping timer; gone at `MaxExistTime`.
        // Without it the timer's first round is the first strike.
        let age = (now - a.born) as u32;
        let every = ticks(a.spec.interval).max(1);
        let first = if a.spec.immediate { 1 } else { 1 + every };
        if age >= first && (age - 1).is_multiple_of(every) && age < ticks(a.spec.life) {
            a.strikes += 1;
            strike_area(world, &ids, a);
        }
        age + 1 < ticks(a.spec.life)
    });
    world.resource_mut::<ThunderAreas>().0 = areas;
}

fn strike_area(world: &mut World, ids: &[(SimId, Entity)], a: &ThunderArea) {
    // `GetCheckResult`: a capsule `CapsuleHalfHeight − HeightOffset` above the point.
    let centre = a.at + Vec3::Z * (a.spec.half_height - a.spec.height_offset);
    let mut hits = Vec::new();
    for &(tid, te) in ids {
        if world.get::<Combatant>(te).is_none() || world.get::<Health>(te).is_none_or(|h| h.current <= 0.0) || friendly(world, a.owner, tid, te) {
            continue;
        }
        let Some(c) = world.get::<Position>(te).map(|p| p.0) else { continue };
        let flat = (c - centre).truncate().length();
        let dz = (c.z - centre.z).abs();
        if flat <= a.spec.radius + CAPSULE_RADIUS && dz <= a.spec.half_height + CAPSULE_HEIGHT * 0.5 {
            hits.push((tid, te));
        }
    }
    world.resource_mut::<SimEvents>().push(SimEvent::ThunderStruck { owner: a.owner, at: a.at, radius: a.spec.radius, hits: hits.len() as u32 });
    let n = hits.len() as f64;
    // HYPOTHESIS (`ApplyAOEWithGA` is native): beyond `Min Split Num` targets the damage splits, never below `Min DPHRate`.
    let split = if n > a.spec.min_split { (a.spec.min_split / n).max(a.spec.min_rate) } else { 1.0 };
    let oe = world.resource::<SimIndex>().get(a.owner);
    for (tid, te) in hits {
        match oe {
            Some(oe) => strike_as(world, a.owner, oe, tid, te, a.spec.element, a.spec.factor * split, centre, a.by, Some(a.attack)),
            None => {
                // The caster is gone: the cloud still strikes with the stats it was cast with.
                let (Some(defender), Some(_)) = (world.get::<Combatant>(te).copied(), world.get::<Position>(te)) else { continue };
                let defence = crate::buffs::defence_of(world, te, defender.defence);
                let hit = resolve_hit(&a.attack, &defence, a.spec.element, AttackTerm::Weapon, HitZone::Body, &mut world.resource_mut::<SimRng>());
                let damage = crate::buffs::incoming_damage(world, te, hit.damage * a.spec.factor * split, None);
                let left = crate::world_items::apply_damage(world, tid, te, damage);
                let mut events = world.resource_mut::<SimEvents>();
                events.push(SimEvent::Damaged { source: a.owner, target: tid, damage, crit: hit.crit, blocked: false, health: left, by: a.by });
                if left <= 0.0 {
                    events.push(SimEvent::Died { id: tid });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Data: weapon definitions, ability sets, abilities, effects, montages under `_data/json`.

/// A Blueprint class's decoded defaults: strict (`json/assets`) where its defaults decoded, else the non-strict
/// read (`json/loose`, the properties before the one that failed).
pub(crate) struct Bp {
    name: String,
    sup: Option<String>,
    props: serde_json::Map<String, Value>,
    /// Sub-objects of the defaults: (type, properties).
    subs: Vec<(String, serde_json::Map<String, Value>)>,
}

pub(crate) fn read_json(root: &str, package: &str) -> Option<Value> {
    // `/Game/` is the game's content, another mount a game feature plugin's (`/TgfMaps/...`).
    let file = match package.strip_prefix("/Game/") {
        Some(rel) => format!("Dungeonborne/Content/{rel}.json"),
        None => {
            let (plugin, rel) = package.strip_prefix('/')?.split_once('/')?;
            format!("Dungeonborne/Plugins/GameFeatures/{plugin}/Content/{rel}.json")
        }
    };
    let p = db_formats::data_path(root).join(file);
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// `/Game/A/B.3` or `Class'/Game/A/B.B_C'` -> `/Game/A/B`.
pub(crate) fn package_of(path: &str) -> String {
    let p = path.trim_end_matches('\'');
    let p = p.rsplit_once('\'').map_or(p, |(_, b)| b);
    p.split('.').next().unwrap_or(p).to_string()
}

fn bp(package: &str) -> Option<Bp> {
    let name = package.rsplit('/').next()?.to_string();
    let mut found: Option<Bp> = None;
    for root in ["json/assets", "json/loose"] {
        let Some(v) = read_json(root, package) else { continue };
        let objs = v.as_array()?;
        let class = objs.iter().find(|o| o["Type"] == "BlueprintGeneratedClass");
        let sup = class.and_then(|c| c["Super"]["ObjectPath"].as_str()).filter(|s| s.starts_with("/Game/")).map(package_of);
        let cdo = objs.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")));
        let props = cdo.and_then(|o| o["Properties"].as_object()).cloned().unwrap_or_default();
        let subs = objs.iter().filter(|o| o["Type"].as_str().is_some_and(|t| t.starts_with("TgfAbilityCost"))).map(|o| (o["Type"].as_str().unwrap_or_default().to_string(), o["Properties"].as_object().cloned().unwrap_or_default())).collect();
        let this = Bp { name: name.clone(), sup, props, subs };
        match &found {
            // The strict read had no defaults (they failed): take the loose one.
            Some(f) if !f.props.is_empty() => break,
            _ => found = Some(this),
        }
    }
    found
}

/// A class and its Blueprint parents, the class first.
pub(crate) fn chain(package: &str) -> Vec<Bp> {
    let mut out = Vec::new();
    let mut next = Some(package.to_string());
    while let Some(p) = next {
        if out.len() > 16 {
            break;
        }
        let Some(b) = bp(&p) else { break };
        next = b.sup.clone();
        out.push(b);
    }
    out
}

pub(crate) fn prop<'a>(chain: &'a [Bp], key: &str) -> Option<&'a Value> {
    chain.iter().find_map(|b| b.props.get(key))
}

/// A property of the defaults' first sub-object of type `kind` (a `ProjectileMovementComponent`'s), the class first.
pub(crate) fn sub_prop<'a>(chain: &'a [Bp], kind: &str, key: &str) -> Option<&'a Value> {
    chain.iter().find_map(|b| b.subs.iter().find(|(t, p)| t == kind && p.contains_key(key)).and_then(|(_, p)| p.get(key)))
}

fn num(chain: &[Bp], key: &str) -> Option<f64> {
    prop(chain, key).and_then(Value::as_f64)
}

pub(crate) fn is_a(chain: &[Bp], class: &str) -> bool {
    chain.iter().any(|b| b.name == class)
}

pub(crate) fn obj_path(v: &Value) -> Option<String> {
    v["ObjectPath"].as_str().map(package_of)
}

/// The weapon-ammo cost of an ability (`TgfAbilityCost_WeaponAmmo` `Quantity`).
pub(crate) fn ability_cost(chain: &[Bp]) -> Option<u32> {
    chain.iter().find_map(|b| b.subs.iter().find(|(t, _)| t == "TgfAbilityCost_WeaponAmmo").and_then(|(_, p)| p["Quantity"]["Value"].as_f64())).map(|q| q as u32)
}

fn element_of(code: f64) -> Element {
    // `TGF.Gameplay.Execution.Damage.AttackElement`: 1 fire, 3 thunder (the staff effects), 0 none; the rest in
    // `DBattleAttributeOffset` order.
    match code as i64 {
        1 => Element::Fire,
        2 => Element::Ice,
        3 => Element::Thunder,
        4 => Element::Light,
        5 => Element::Dark,
        _ => Element::Physical,
    }
}

/// A damage effect's hit value (HYPOTHESIS: its `ChargeRollback` is what the native `CalcHitValue` sums; every hit the
/// weapon tips call guard-breaking has 150, every other melee hit 80 or less); 0 without one.
pub(crate) fn hit_value(package: &str) -> f64 {
    prop(&chain(package), "ChargeRollback").and_then(Value::as_f64).unwrap_or(0.0)
}

/// A damage effect's `MoveFactorRate` and element.
pub(crate) fn damage_effect(package: &str) -> anyhow::Result<(f64, Element)> {
    let c = chain(package);
    let execs = prop(&c, "Executions").and_then(Value::as_array).with_context(|| format!("{package}: no Executions"))?;
    let mut factor = None;
    let mut element = Element::Physical;
    for m in execs.iter().flat_map(|x| x["CalculationModifiers"].as_array().into_iter().flatten()) {
        let value = m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64().unwrap_or(0.0);
        // An override (op 3) wins over the other modifiers (the longsword 5001's effects add a 0 after their override).
        if m["CapturedAttribute"]["AttributeToCapture"]["AttributeName"] == "MoveFactorRate" && !factor.is_some_and(|f: (f64, bool)| f.1) {
            factor = Some((value, m["ModifierOp"].as_str().is_some_and(|o| o.ends_with("::3"))));
        }
        if m["TransientAggregatorIdentifier"]["TagName"] == "TGF.Gameplay.Execution.Damage.AttackElement" {
            element = element_of(value);
        }
    }
    Ok((factor.with_context(|| format!("{package}: no MoveFactorRate"))?.0, element))
}

/// The hand a weapon trace belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hand {
    Right,
    Left,
}

/// A montage's length, sections (name, start) and notifies (name, start, duration, its export's properties).
pub(crate) struct Montage {
    pub(crate) length: f32,
    sections: Vec<(String, f32)>,
    notifies: Vec<(String, f32, f32, Value)>,
    /// Slot segments: (anim name, play rate).
    pub(crate) segments: Vec<(String, f32)>,
    /// The same with their times.
    pub(crate) timed: Vec<(String, crate::monsters::MontageSegment)>,
    pub(crate) rates: PlayRates,
}

impl Montage {
    pub(crate) fn load(package: &str) -> anyhow::Result<Montage> {
        let v = read_json("json/assets", package).or_else(|| read_json("json/loose", package)).with_context(|| format!("{package} not decoded"))?;
        let objs = v.as_array().context("montage JSON")?;
        let m = objs.iter().find(|o| o["Type"] == "AnimMontage").context("no AnimMontage")?;
        let p = &m["Properties"];
        let length = p["SequenceLength"].as_f64().context("SequenceLength")? as f32;
        let sections = p["CompositeSections"].as_array().into_iter().flatten().map(|s| (s["SectionName"].as_str().unwrap_or_default().to_string(), s["LinkValue"].as_f64().unwrap_or(0.0) as f32)).collect();
        let export = |n: &Value| -> Value {
            let obj = n["Notify"]["ObjectName"].as_str().or(n["NotifyStateClass"]["ObjectName"].as_str()).unwrap_or_default();
            let name = obj.rsplit(':').next().unwrap_or_default().trim_end_matches('\'');
            objs.iter().find(|o| o["Name"] == name).map_or(Value::Null, |o| o["Properties"].clone())
        };
        let notifies: Vec<(String, f32, f32, Value)> = p["Notifies"].as_array().into_iter().flatten().map(|n| (n["NotifyName"].as_str().unwrap_or_default().to_string(), n["LinkValue"].as_f64().unwrap_or(0.0) as f32, n["Duration"].as_f64().unwrap_or(0.0) as f32, export(n))).collect();
        let segments = p["SlotAnimTracks"].as_array().into_iter().flatten().flat_map(|t| t["AnimTrack"]["AnimSegments"].as_array().into_iter().flatten()).map(|s| {
            let name = s["AnimReference"]["ObjectName"].as_str().unwrap_or_default();
            let name = name.split('\'').nth(1).unwrap_or(name).to_string();
            (name, s["AnimPlayRate"].as_f64().unwrap_or(1.0) as f32)
        }).collect();
        let rates = PlayRates(notifies.iter().filter(|n| n.0 == "TgfMontagePlayRate").filter_map(|n| n.3["PlayRate"].as_f64().map(|r| (n.1, n.1 + n.2, r as f32))).collect());
        Ok(Montage { length, sections, notifies, segments, timed: crate::monsters::montage_segments_from_json(&v), rates })
    }

    /// This montage followed by `next` (a charge montage and its release montage): `next`'s sections, notifies,
    /// segments and rates shifted by this length, with a `ChargeRelease` section where `next` starts.
    pub(crate) fn joined(&self, next: &Montage) -> Montage {
        let at = self.length;
        let mut sections = self.sections.clone();
        sections.push(("ChargeRelease".to_string(), at));
        sections.extend(next.sections.iter().filter(|s| s.0 != "ChargeStart").map(|s| (s.0.clone(), s.1 + at)));
        let mut notifies = self.notifies.clone();
        notifies.extend(next.notifies.iter().map(|n| (n.0.clone(), n.1 + at, n.2, n.3.clone())));
        let mut timed = self.timed.clone();
        timed.extend(next.timed.iter().map(|(n, s)| (n.clone(), crate::monsters::MontageSegment { at: s.at + at, ..*s })));
        let mut rates = self.rates.0.clone();
        rates.extend(next.rates.0.iter().map(|r| (r.0 + at, r.1 + at, r.2)));
        Montage { length: at + next.length, sections, notifies, segments: self.segments.iter().chain(&next.segments).cloned().collect(), timed, rates: PlayRates(rates) }
    }

    /// The montage's sound notifies (`AN_Tgf_AkEvent_C` / `TimedAkEvent`: their `Event`): (time, event name).
    pub(crate) fn sounds(&self) -> Vec<(f32, String)> {
        self.notifies.iter().filter_map(|n| Some((n.1, n.3["Event"]["ObjectName"].as_str()?.split('\'').nth(1)?.to_string()))).collect()
    }

    pub(crate) fn section(&self, name: &str) -> Option<f32> {
        self.sections.iter().find(|s| s.0 == name).map(|s| s.1)
    }

    /// A `PlayMontageNotify` by its notify name.
    pub(crate) fn event(&self, name: &str) -> Option<f32> {
        self.notifies.iter().find(|n| n.3["NotifyName"] == name).map(|n| n.1)
    }

    /// The first notify of a class: (start, duration, properties).
    pub(crate) fn first(&self, class: &str) -> Option<(f32, f32, &Value)> {
        self.notifies.iter().filter(|n| n.0 == class).min_by(|a, b| a.1.total_cmp(&b.1)).map(|n| (n.1, n.2, &n.3))
    }

    /// Every hit window (montage time): `AGR_Attack_NotifyState` (no hand given) and `TgfAttack` with the hand it
    /// traces (`RightHandWeaponTakeEffect` / `LeftHandWeaponTakeEffect`), in time order.
    pub(crate) fn hits(&self) -> Vec<(f32, f32, Option<Hand>)> {
        let mut out: Vec<(f32, f32, Option<Hand>)> = self.notifies.iter().filter(|n| n.0 == "AGR_Attack_NotifyState" || n.0 == "TgfAttack").map(|n| {
            let hand = if n.3["LeftHandWeaponTakeEffect"] == true { Some(Hand::Left) } else if n.3["RightHandWeaponTakeEffect"] == true { Some(Hand::Right) } else { None };
            (n.1, n.1 + n.2, hand)
        }).collect();
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    /// Every `CanQuickSide<n>` tag window (n, start, end; montage time).
    pub(crate) fn quick_side_grants(&self) -> Vec<(u8, f32, f32)> {
        let tags = |n: &(String, f32, f32, Value)| n.3["Tags"].as_array().cloned().unwrap_or_default();
        let ops = self.notifies.iter().filter(|n| n.0 == "TgfGameplayTag" || n.0 == "AN_Tgf_GameplayTagOperation_C");
        ops.flat_map(|n| tags(n).into_iter().filter_map(|t| t.as_str()?.strip_prefix("TGF.Ability.CanQuickSide.CanQuickSide")?.parse().ok()).map(|k| (k, n.1, n.1 + n.2)).collect::<Vec<_>>()).collect()
    }

    /// The first tag window (`TgfAnimNotifyState_GameplayTag`) or tag operation listing `tag`.
    pub(crate) fn tag_window(&self, tag: &str) -> Option<(f32, f32)> {
        self.notifies.iter().filter(|n| (n.0 == "TgfGameplayTag" || n.0 == "AN_Tgf_GameplayTagOperation_C") && n.3["Tags"].as_array().is_some_and(|t| t.iter().any(|x| x == tag))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|n| (n.1, n.1 + n.2))
    }
}

const ANIM_1ST: &str = "/Game/TGF/Character/Shared/Animation/1st/Staff";
const EXPLODE_TAG: &str = "TGF.Ability.StaffFireExtraExplode";
const HERO_CURVE: &str = "/Game/TGF/Ability/Data/Attributes/HeroAttributeCurve";
/// The burst's burn stacks: `Add Burning Buff Arg(..., UseDefaultStackCount = false, InputStackCount = 2)` in
/// `GA_TGF_Staff_Laser` (a bytecode literal; the default, `Common.BuffArg.BurningStack`, is 5).
const EXPLODE_BURN_STACKS: u32 = 2;

/// Whether the item's held legendary affix grants `tag`: `ExcelAffixTemplate` `legendary_rule` ->
/// `ExcelAffixLegendaryRule` -> ability set (`UESourceAbilitySet`) -> its `GrantedGameTags`.
fn legendary_grants(t: &ConfigTables, item_id: i32, tag: &str) -> bool {
    let sets = t.excel_affix_template.iter().filter(|a| a.item_id == item_id).flat_map(|a| a.data.iter().flat_map(|d| d.legendary_rule.iter().map(|r| r.affix_alias_id)));
    sets.filter_map(|alias| t.excel_affix_legendary_rule.iter().find(|r| r.affix_alias_id == alias)?.offset.as_ref().map(|o| o.ability_set_id))
        .filter_map(|id| t.uesource_ability_set.iter().find(|s| s.ue_source_id == id))
        .filter_map(|s| read_json("json/assets", &package_of(&s.ue_path)))
        .any(|v| v.as_array().into_iter().flatten().any(|o| o["Properties"]["GrantedGameTags"].as_array().into_iter().flatten().any(|g| g["GamePlayTags"].as_array().is_some_and(|ts| ts.iter().any(|x| x == tag)))))
}

/// A `Common.BuffArg.*` row of the hero attribute curve (the burning / freezing / bleeding defaults).
pub(crate) fn buff_arg(name: &str) -> Option<f64> {
    read_json("json/assets", HERO_CURVE)?[0]["Rows"][name]["Keys"][0]["Value"].as_f64()
}

fn explode_spec(ga: &[Bp]) -> anyhow::Result<ExplodeSpec> {
    let curve = read_json("json/assets", HERO_CURVE).context("HeroAttributeCurve not decoded")?;
    let row = |n: &str| curve[0]["Rows"][n]["Keys"][0]["Value"].as_f64().with_context(|| format!("curve row {n}"));
    Ok(ExplodeSpec {
        radius: num(ga, "ExtraExplodeRadius").context("ExtraExplodeRadius")? as f32,
        burn: BurnSpec { damage: row("Common.BuffArg.BurningDamage")?, interval: row("Common.BuffArg.BurningIntervalBase")? as f32, stacks: EXPLODE_BURN_STACKS },
    })
}
const ABILITY: &str = "/Game/TGF/Equipment/Weapon/Staff/GameAbility";
pub(crate) const CHARGE_TAG: &str = "TGF.AbilityType.Active.Single.Charge";

fn combo_spec(n: u8) -> anyhow::Result<ComboSpec> {
    let ga = chain(&format!("{ABILITY}/GA_TGF_Staff_Combo{n}"));
    let ge = prop(&ga, "MeleeWeaponHitGE").and_then(Value::as_array).and_then(|a| a.first()).and_then(obj_path).with_context(|| format!("GA_TGF_Staff_Combo{n} MeleeWeaponHitGE"))?;
    let (factor, _) = damage_effect(&ge)?;
    let m = Montage::load(&format!("{ANIM_1ST}/AM_Staff_Combo{n}_1st"))?;
    let (t, d, _) = m.first("AGR_Attack_NotifyState").context("combo hit window")?;
    let chain = m.tag_window("TGF.Ability.CanQuickSide.CanQuickSide2");
    let free_at = m.tag_window(CHARGE_TAG).map_or(m.length, |w| w.0);
    Ok(ComboSpec { window: (t, t + d), factor, chain, free_at, length: m.length, clip: m.segments.first().map(|s| s.0.clone()).unwrap_or_default(), segments: m.timed.clone(), ..Default::default() })
}

fn beam_spec(ga: &[Bp]) -> anyhow::Result<BeamSpec> {
    let normal = prop(ga, "GE_FireAoeDMG").and_then(obj_path).context("GE_FireAoeDMG")?;
    let extra = prop(ga, "GE_FireAoeDMG_Extra").and_then(obj_path).context("GE_FireAoeDMG_Extra")?;
    let (normal, element) = damage_effect(&normal)?;
    let (extra, _) = damage_effect(&extra)?;
    let curve = prop(ga, "DamageRateCurve").and_then(obj_path).context("DamageRateCurve")?;
    let cv = read_json("json/assets", &curve).with_context(|| format!("{curve} not decoded"))?;
    let rate_curve = cv[0]["Properties"]["FloatCurve"]["Keys"].as_array().context("curve keys")?.iter().filter_map(|k| Some((k["Time"].as_f64()? as f32, k["Value"].as_f64()? as f32))).collect();
    let m = Montage::load(&format!("{ANIM_1ST}/AM_Staff_Skill_Laser_1st"))?;
    let cancel_at = m.section("ChargeCancel").context("ChargeCancel")?;
    let (mt, md, mp) = m.first("AN_Tgf_Gameplay_AnimationMoveScale_C").context("laser move scale")?;
    Ok(BeamSpec {
        range: num(ga, "Max Distance").context("Max Distance")? as f32,
        interval: num(ga, "IntervalTime").context("IntervalTime")? as f32,
        triggers: num(ga, "MaxTime").context("MaxTime")? as u32,
        normal,
        extra,
        element,
        rate_curve,
        start_share: BEAM_START_SHARE,
        cast_at: m.event("Cast").context("Cast notify")?,
        end_at: m.event("End").context("End notify")?,
        cancel_at,
        cancel_len: m.length - cancel_at,
        free_at: m.tag_window(CHARGE_TAG).map_or(cancel_at, |w| w.0),
        move_window: (mt, mt + md),
        move_scale: mp["Scale"].as_f64().unwrap_or(1.0) as f32,
        clip_rate: m.segments.first().map_or(1.0, |s| s.1),
    })
}

fn thunder_spec(ga: &[Bp]) -> anyhow::Result<ThunderSpec> {
    let area = prop(ga, "SpawnThunderArea").and_then(obj_path).context("SpawnThunderArea")?;
    let ac = chain(&area);
    let (factor, element) = damage_effect(&format!("{ABILITY}/GE_TGF_Damage_Instant_Staff_Thunder"))?;
    let charge = Montage::load(&format!("{ANIM_1ST}/AM_Staff_Skill_Thunder_Charge_1st"))?;
    let release = Montage::load(&format!("{ANIM_1ST}/AM_Staff_Skill_Thunder_Release_1st"))?;
    let fail = Montage::load(&format!("{ANIM_1ST}/AM_Staff_Skill_ThunderFail_1st"))?;
    let ready_at = charge.section("Loop").context("thunder Loop section")?;
    let release_clip_start = read_json("json/assets", &format!("{ANIM_1ST}/AM_Staff_Skill_Thunder_Release_1st")).and_then(|v| v.as_array()?.iter().find(|o| o["Type"] == "AnimMontage")?["Properties"]["SlotAnimTracks"][0]["AnimTrack"]["AnimSegments"][0]["AnimStartTime"].as_f64()).unwrap_or(0.0) as f32;
    let f = |k: &str| num(&ac, k).with_context(|| format!("thunder area {k}"));
    Ok(ThunderSpec {
        range: num(ga, "MaxDistance").context("MaxDistance")? as f32,
        area: AreaSpec { radius: f("CapsuleRadius")? as f32, half_height: f("CapsuleHalfHeight")? as f32, height_offset: f("HeightOffset")? as f32, interval: f("TriggerDeltaTime")? as f32, life: f("MaxExistTime")? as f32, min_split: f("Min Split Num")?, min_rate: f("Min DPHRate")?, factor, element, immediate: prop(&ac, "TriggerImmediately").and_then(Value::as_bool).unwrap_or(false) },
        ready_at,
        loop_len: charge.length - ready_at,
        charge_scale: charge.first("AN_Tgf_Gameplay_AnimationMoveScale_C").and_then(|n| n.2["Scale"].as_f64()).unwrap_or(1.0) as f32,
        cast_at: release.event("CastCharge").context("CastCharge notify")?,
        release_len: release.length,
        release_free: release.tag_window(CHARGE_TAG).map_or(release.length, |w| w.0),
        fail_len: fail.length,
        fail_free: fail.tag_window(CHARGE_TAG).map_or(fail.length, |w| w.0),
        release_clip_start,
    })
}

fn recharge_spec(ga: &[Bp], thunder: bool) -> anyhow::Result<RechargeSpec> {
    let m = Montage::load(&format!("{ANIM_1ST}/AM_Staff_Recharge{}_1st", if thunder { "_Thunder" } else { "" }))?;
    Ok(RechargeSpec {
        rate: num(ga, "AddiRecoverRate").context("AddiRecoverRate")?,
        loop_at: m.section("Loop").context("recharge Loop")?,
        end_at: m.section("End").context("recharge End")?,
        length: m.length,
        move_scale: m.first("AN_Tgf_Gameplay_AnimationMoveScale_C").and_then(|n| n.2["Scale"].as_f64()).unwrap_or(1.0) as f32,
    })
}

/// Reads every staff item's spec: `UeSourceEquipment` staff rows, their weapon definitions (`WID_*`, merged
/// over their parents), the anim definition's ability sets and those abilities.
pub fn load_staff_db(t: &ConfigTables) -> anyhow::Result<StaffDb> {
    let combo = [combo_spec(1)?, combo_spec(2)?];
    let mut db = BTreeMap::new();
    for row in t.ue_source_equipment.iter().filter(|r| r.equip_subtype == EnGearEquipSubType::Staff) {
        let mut wid = chain(&package_of(&row.ue_path));
        if wid.is_empty() {
            continue;
        }
        let own_waid = wid[0].props.get("SingleAnimWeaponDefinition").and_then(obj_path);
        // The element: the element weapon definition it derives from, else its own anim definition's, else (the
        // definitions whose loose read stopped early: 181056001) by item id (HYPOTHESIS).
        let element = if is_a(&wid, "WID_Staff_Fire") {
            StaffElement::Fire
        } else if is_a(&wid, "WID_Staff_Thunder") {
            StaffElement::Thunder
        } else if is_a(&wid, "WID_Staff_ice") || own_waid.as_deref().is_some_and(|p| p.ends_with("/WAID_Staff_Ice")) {
            StaffElement::Ice
        } else {
            tune::element_by_id(row.equip_id)
        };
        let base = match element {
            StaffElement::Fire => "Fire",
            StaffElement::Ice => "ice",
            StaffElement::Thunder => "Thunder",
        };
        // Without an element parent its numbers and anim definition come from the element's base definition
        // (HYPOTHESIS: its own values didn't decode).
        if !is_a(&wid, &format!("WID_Staff_{base}")) {
            let own = wid.remove(0);
            wid = std::iter::once(own).chain(chain(&format!("{ABILITY}/WID_Staff_{base}"))).collect();
        }
        let waid = chain(&prop(&wid, "SingleAnimWeaponDefinition").and_then(obj_path).context("SingleAnimWeaponDefinition")?);
        // The anim definition's ability sets name the abilities.
        let mut abilities = Vec::new();
        for set in prop(&waid, "AbilitySetsToGrant").and_then(Value::as_array).into_iter().flatten().filter_map(obj_path) {
            let Some(v) = read_json("json/assets", &set) else { continue };
            for o in v.as_array().into_iter().flatten() {
                for g in o["Properties"]["GrantedGameplayAbilities"].as_array().into_iter().flatten() {
                    if let Some(p) = g["Ability"]["ObjectPath"].as_str() {
                        abilities.push(chain(&package_of(p)));
                    }
                }
            }
        }
        let laser = abilities.iter().find(|a| is_a(a, "GA_TGF_Staff_Laser"));
        let thunder = abilities.iter().find(|a| is_a(a, "GA_TGF_Staff_Thunder"));
        let recharge = abilities.iter().find(|a| is_a(a, "GA_TGF_AdditionalRecoverAmmo"));
        let spell = laser.or(thunder);
        let cost = spell.and_then(|a| ability_cost(a)).or_else(|| num(&wid, "AmmoCost").map(|c| c as u32)).unwrap_or(0);
        let spec = StaffSpec {
            item_id: row.equip_id,
            element,
            max: num(&wid, "MaxAmmo").context("MaxAmmo")? as u32,
            recover: num(&wid, "AmmoAutoRecover").context("AmmoAutoRecover")? as u32,
            cost,
            segments: num(&wid, "AmmoDisplayCnt").map_or(1, |n| n.clamp(1.0, 5.0) as u8),
            combo: combo.clone(),
            beam: laser.map(|a| beam_spec(a)).transpose()?,
            thunder: thunder.map(|a| thunder_spec(a)).transpose()?,
            recharge: recharge.map(|a| recharge_spec(a, element == StaffElement::Thunder)).transpose()?,
            explode: laser.filter(|_| legendary_grants(t, row.equip_id, EXPLODE_TAG)).map(|a| explode_spec(a)).transpose()?,
        };
        db.insert(row.equip_id, Arc::new(spec));
    }
    anyhow::ensure!(!db.is_empty(), "no staff weapon definitions decoded under _data/json");
    Ok(StaffDb(db))
}

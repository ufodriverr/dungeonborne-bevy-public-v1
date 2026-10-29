//! The Shadow Dragon. The Shadow King's `Attack8` summons it. CONFIRMED from `AIC_ShadowDragon`, `BT_ShadowDragon` and
//! its tasks' bytecode: it plays its wake-up once, then forever circles, flies to the next section's corner and
//! breathes fire. Every move is its clips' root motion: the root tracks (height and heading included) applied as deltas
//! from where it is, so the four hover clips carry it round a circle 31 m in radius, 32 m above the King's spawn point.
//!
//! - **The circle** (`GA_TGF_ShadowDragon_Circle`): its controller turns it to the current section's rotation and the
//!   montage plays from `Section<CircleSection>`, looping. At a `PlayMontageNotify` named `Attack<N>` (1.04 s into a
//!   section), when the attack is ready (`AttackInterval` since the last breath ended; `AttackTime` starts at 0, so
//!   the first is at once), the ability ends and `CircleSection` := N.
//! - **`MoveToNextSection`**: that section's corner (from the King's spawn point), or the next one when the corner
//!   lies more than 90 degrees off its facing; `MoveToLocation` at its move speed, acceptance 100 cm; then the
//!   section's rotation.
//! - **The breath** (`GA_TGF_M_ShadowDragon_Attack1`, `Attack2` its child), one at random: its montage's root motion
//!   (a dive and back); from `SprayStartTime`, every `SprayInterval` while `count x interval < SprayDuration` (36
//!   sprays), a line trace `TraceDistance` along its forward turned `SprayDirection.Y` (-80) degrees down; where it
//!   hits, a fire patch: `BP_ShadowDragon_Attack_Decal_NoNiagara`, whose aura (`Aura_TGF_ShadowDragon_Attack1_Burning`,
//!   a capsule) puts `GE_TGF_ShadowDragon_Burning` (a periodic damage effect) on those in it. The patch goes when its
//!   decal has faded (`FadeStartDelay` + `FadeDuration`; the engine destroys a decal's owner after its fade).
//!
//! HYPOTHESIS: the flight ignores collision (the paths are open air over the throne room; the engine's root motion
//! would sweep); while it moves to a corner it faces the way it flies; a target keeps one burning per dragon,
//! refreshed while it stands in any of its patches ([`crate::overrides::shadow_dragon`]); the trace hits only the
//! level; the fire hurts only the monsters' enemies; a patch burns with the attack the dragon had when it was laid.

use std::sync::Arc;

use avian3d::prelude::SpatialQuery;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use bevy_math::{Dir3, Vec2, Vec3};

use crate::combat::{AttackStats, Element};
use crate::melee::{Aim, Combatant, Health};
use crate::monsters::{MontageSegment, root_track};
use crate::movement::{CAPSULE_HEIGHT, CAPSULE_RADIUS};
use crate::{DT, HitSource, Position, SimEvent, SimEvents, SimId, SimIndex, SimRng, TickCount};

/// A montage's root motion: its slot segments and length (s); its sound cues (s in, Wwise event) for the client.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub segments: Vec<(String, MontageSegment)>,
    pub length: f32,
    pub sounds: Vec<(f32, String)>,
}

/// One section of the circle montage: start and length (s, montage time), its `Attack<N>` notify (s into the section)
/// and that N.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lap {
    pub at: f32,
    pub length: f32,
    pub notify: f32,
    pub next: u32,
}

/// A breath ability.
#[derive(Debug, Clone, PartialEq)]
pub struct Breath {
    pub clip: Clip,
    /// `SprayStartTime`, `SprayInterval` (s) and the spray count.
    pub start: f32,
    pub every: f32,
    pub sprays: u32,
    /// `TraceDistance` (cm) and `SprayDirection.Y` (radians, negative is down).
    pub reach: f32,
    pub pitch: f32,
}

/// A fire patch: its aura's capsule (cm), how long it stays (s), and the burning effect it puts on (duration and
/// period, s; damage factor on the dragon's attack; element). For the client: its decal's half size (cm), when it
/// starts fading (s) and its material's `EmissiveColor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burn {
    pub radius: f32,
    pub half_height: f32,
    pub life: f32,
    pub decal: f32,
    pub fade_from: f32,
    pub color: [f32; 3],
    pub duration: f32,
    pub period: f32,
    pub factor: f64,
    pub element: Element,
}

/// `AIC_ShadowDragon`'s defaults and its abilities.
#[derive(Debug, Clone, PartialEq)]
pub struct DragonSpec {
    /// `CircleSectionLocation` (cm from the summoner's spawn point, world axes) and `CircleSectionRotation` (yaw, rad).
    pub corners: Vec<(Vec3, f32)>,
    /// `AttackInterval` (s).
    pub interval: f32,
    pub wake: Clip,
    pub circle: Clip,
    pub laps: Vec<Lap>,
    pub breaths: Vec<Breath>,
    pub burn: Burn,
    /// `MoveToLocation`'s acceptance radius (cm).
    pub accept: f32,
}

const MONSTER: &str = "/Game/TGF/Character/Monster/";

/// `BTT_ShadowDragon_MoveToNextSection`'s `MoveToLocation` acceptance radius (cm, a literal in its bytecode).
const MOVE_ACCEPTANCE: f32 = 100.0;

impl DragonSpec {
    /// From `<dir>/AI/AIC_ShadowDragon` (None: another monster, or not decoded).
    pub fn load(dir: &str) -> Option<DragonSpec> {
        let aic = format!("{MONSTER}{dir}/AI/AIC_ShadowDragon.0");
        crate::monsters::asset_json(&aic)?;
        let def = |path: &str, key: &str| crate::monsters::class_default(path, key);
        let path = |v: &serde_json::Value| v["ObjectPath"].as_str().map(str::to_string);
        let corners = def(&aic, "CircleSectionLocation")?.as_array()?.iter().zip(def(&aic, "CircleSectionRotation")?.as_array()?).filter_map(|(l, r)| Some((Vec3::new(l["X"].as_f64()? as f32, l["Y"].as_f64()? as f32, l["Z"].as_f64()? as f32), (r["Yaw"].as_f64()? as f32).to_radians()))).collect();
        let montage = |ga: &str| path(&def(ga, "CastMontageSet")?["DefaultMontage"]).and_then(|m| crate::monsters::monster_json(&m));
        let clip = |v: &serde_json::Value| {
            let m = crate::monsters::montage_from_json(v).ok()?;
            Some(Clip { segments: crate::monsters::montage_segments_from_json(v), length: m.length, sounds: m.sounds })
        };
        let wake = clip(&montage(&path(&def(&aic, "SpawnAbility")?)?)?)?;
        let circle_ga = path(&def(&aic, "CircleAbility")?)?;
        let circle_v = montage(&circle_ga)?;
        let circle = clip(&circle_v)?;
        let prefix = def(&circle_ga, "AttackNotifyPrefix")?.as_str()?.to_string();
        let laps = laps(&circle_v, &circle, &prefix)?;
        let breaths: Vec<Breath> = def(&aic, "AttackAbilities")?.as_array()?.iter().filter_map(|a| {
            let ga = path(a)?;
            let f = |k: &str| def(&ga, k)?.as_f64();
            let (duration, every) = (f("SprayDuration")?, f("SprayInterval")?);
            // `SprayOnce` at the start, then one per timer tick while `ExtraSprayTime x SprayInterval < SprayDuration`.
            let sprays = 1 + (0..).take_while(|&k| f64::from(k) * every < duration).count() as u32;
            Some(Breath { clip: clip(&montage(&ga)?)?, start: f("SprayStartTime")? as f32, every: every as f32, sprays, reach: f("TraceDistance")? as f32, pitch: (def(&ga, "SprayDirection")?["Y"].as_f64()? as f32).to_radians() })
        }).collect();
        let first = path(def(&aic, "AttackAbilities")?.as_array()?.first()?)?;
        let burn = burn(&path(&def(&first, "Decal")?)?)?;
        (!breaths.is_empty()).then_some(DragonSpec { corners, interval: def(&aic, "AttackInterval")?.as_f64()? as f32, wake, circle, laps, breaths, burn, accept: MOVE_ACCEPTANCE })
    }
}

/// The circle montage's sections (`CompositeSections`) with their `Attack<N>` notifies.
fn laps(v: &serde_json::Value, clip: &Clip, prefix: &str) -> Option<Vec<Lap>> {
    let objs = v.as_array()?;
    let m = &objs.iter().find(|o| o["Type"] == "AnimMontage")?["Properties"];
    let rate = m["RateScale"].as_f64().unwrap_or(1.0) as f32;
    let starts: Vec<f32> = m["CompositeSections"].as_array()?.iter().filter_map(|s| s["LinkValue"].as_f64().map(|x| x as f32 / rate)).collect();
    let notifies: Vec<(f32, u32)> = m["Notifies"]
        .as_array()?
        .iter()
        .filter_map(|n| {
            let export = n["Notify"]["ObjectName"].as_str()?.rsplit(':').next()?.trim_end_matches('\'');
            let name = objs.iter().find(|o| o["Name"] == export)?["Properties"]["NotifyName"].as_str()?;
            Some((n["LinkValue"].as_f64()? as f32 / rate, name.strip_prefix(prefix)?.parse().ok()?))
        })
        .collect();
    let laps: Vec<Lap> = starts
        .iter()
        .enumerate()
        .filter_map(|(i, &at)| {
            let end = starts.get(i + 1).copied().unwrap_or(clip.length);
            let &(t, next) = notifies.iter().find(|n| n.0 >= at && n.0 < end)?;
            Some(Lap { at, length: end - at, notify: t - at, next })
        })
        .collect();
    (laps.len() == starts.len() && !laps.is_empty()).then_some(laps)
}

/// The fire patch the breath lays (`Decal` of the first breath): its aura's capsule, its decal's fade, and the burning
/// effect's duration, period, factor and element.
fn burn(decal: &str) -> Option<Burn> {
    let d = |path: &str, key: &str| crate::monsters::class_default(path, key);
    let v = crate::monsters::asset_json(decal)?;
    let fade = v.as_array()?.iter().find(|o| o["Type"] == "DecalComponent")?["Properties"].clone();
    let aura = d(decal, "DamageArea")?["ObjectPath"].as_str()?.to_string();
    let ge = d(&aura, "GEsAppliedToTargets")?.as_array()?.first()?["ObjectPath"].as_str()?.to_string();
    let (factor, element) = crate::staffs::damage_effect(&crate::staffs::package_of(&ge)).ok()?;
    // The decal material's emissive colour (`MI_FX_DecalFire_103T`; white if it isn't decoded).
    let color = fade["DecalMaterial"]["ObjectPath"].as_str().map(crate::staffs::package_of).and_then(|m| crate::staffs::read_json("json/loose", &m).or_else(|| crate::staffs::read_json("json/assets", &m)));
    let color = color.as_ref().and_then(|v| v.as_array()?.iter().find_map(|o| o["Properties"]["VectorParameterValues"].as_array()?.iter().find(|p| p["ParameterInfo"]["Name"] == "EmissiveColor").map(|p| p["ParameterValue"].clone())));
    let color = color.map_or([1.0; 3], |c| ["R", "G", "B"].map(|k| c[k].as_f64().unwrap_or(1.0) as f32));
    Some(Burn {
        decal: fade["DecalSize"]["Y"].as_f64()? as f32,
        fade_from: fade["FadeStartDelay"].as_f64()? as f32,
        color,
        radius: d(&aura, "CapsuleRadius")?.as_f64()? as f32,
        half_height: d(&aura, "CapsuleHalfHeight")?.as_f64()? as f32,
        life: (fade["FadeStartDelay"].as_f64()? + fade["FadeDuration"].as_f64()?) as f32,
        duration: d(&ge, "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64()? as f32,
        period: d(&ge, "Period")?["Value"].as_f64()? as f32,
        factor,
        element,
    })
}

/// Where the dragon is and which way it faces (yaw, rad).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub at: Vec3,
    pub yaw: f32,
}

/// What its tree is doing; times are ticks into the ability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragonPhase {
    /// The wake-up (`Spawn`).
    Wake { tick: u32 },
    /// The circle, from section `from` (1-based).
    Circle { from: u32, tick: u32 },
    /// `MoveToNextSection` to that section's corner.
    Move { goal: Vec3, yaw: f32 },
    /// Breath `index`.
    Breath { index: usize, tick: u32 },
}

/// A summoned Shadow Dragon: its spec, the summoner's spawn point the corners are from, `CircleSection`, when its last
/// breath ended (`AttackTime`, s), its phase and pose, and its move speed (cm/s).
#[derive(Component, Debug, Clone)]
pub struct ShadowDragon {
    pub spec: Arc<DragonSpec>,
    pub centre: Vec3,
    pub section: u32,
    pub attack_time: f32,
    pub phase: DragonPhase,
    pub pose: Pose,
    pub speed: f32,
}

impl ShadowDragon {
    pub fn new(spec: Arc<DragonSpec>, centre: Vec3, pose: Pose, speed: f32) -> ShadowDragon {
        ShadowDragon { spec, centre, section: 1, attack_time: 0.0, phase: DragonPhase::Wake { tick: 0 }, pose, speed }
    }

    /// The montage playing and its time (s): the clip name for the client.
    pub fn montage(&self) -> Option<(&Clip, f32)> {
        match self.phase {
            DragonPhase::Wake { tick } => Some((&self.spec.wake, tick as f32 * DT)),
            DragonPhase::Circle { from, tick } => Some((&self.spec.circle, self.circle_time(from, tick))),
            DragonPhase::Breath { index, tick } => Some((&self.spec.breaths[index].clip, tick as f32 * DT)),
            DragonPhase::Move { .. } => None,
        }
    }

    fn circle_time(&self, from: u32, tick: u32) -> f32 {
        let start = self.spec.laps.get(from as usize - 1).map_or(0.0, |l| l.at);
        (start + tick as f32 * DT).rem_euclid(self.spec.circle.length)
    }

    fn corner(&self, section: u32) -> (Vec3, f32) {
        let (off, yaw) = self.spec.corners[(section.max(1) as usize - 1).min(self.spec.corners.len() - 1)];
        (self.centre + off, yaw)
    }
}

/// The root motion of `clip` from montage time `t0` to `t1` applied to `pose` (one section change at most between).
fn step(pose: &mut Pose, clip: &Clip, t0: f32, t1: f32) {
    let seg = |t: f32| clip.segments.iter().rposition(|s| s.1.at <= t).unwrap_or(0);
    let (i0, i1) = (seg(t0), seg(t1));
    if i0 == i1 {
        delta(pose, &clip.segments[i0], t0, t1);
    } else {
        let s = &clip.segments[i0].1;
        delta(pose, &clip.segments[i0], t0, s.at + (s.to - s.from) / s.rate.max(1e-3));
        delta(pose, &clip.segments[i1], clip.segments[i1].1.at, t1);
    }
}

/// One segment's root motion between montage times `t0` and `t1`: the track's move turned from its own heading to the
/// pose's, and its turn.
fn delta(pose: &mut Pose, (name, s): &(String, MontageSegment), t0: f32, t1: f32) {
    let Some(track) = root_track(name) else { return };
    let len = (s.to - s.from) / s.rate.max(1e-3);
    let ct = |t: f32| s.from + (t - s.at).clamp(0.0, len) * s.rate;
    let ((a, ya), (b, yb)) = (track.flight(ct(t0)), track.flight(ct(t1)));
    let d = b - a;
    pose.at += (Vec2::from_angle(pose.yaw - ya).rotate(d.truncate())).extend(d.z);
    pose.yaw += yb - ya;
}

/// A fire patch in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct FirePatch {
    pub owner: SimId,
    pub at: Vec3,
    pub burn: Burn,
    pub attack: AttackStats,
    pub until: u64,
}

/// A target burning: (target, dragon), the patch that set it (where its hits come from), its effect's end and next
/// damage tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burning {
    pub target: SimId,
    pub owner: SimId,
    pub at: Vec3,
    pub until: u64,
    pub next: u64,
    pub attack: AttackStats,
    pub burn: Burn,
}

/// The dragons' fire patches and what they set burning.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct DragonFires {
    pub patches: Vec<FirePatch>,
    pub burning: Vec<Burning>,
}

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

pub(crate) fn dragon_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let dragons: Vec<(Entity, SimId)> = world.query_filtered::<(Entity, &SimId), With<ShadowDragon>>().iter(world).map(|(e, id)| (e, *id)).collect();
    for (e, id) in dragons {
        if world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let mut d = world.get::<ShadowDragon>(e).expect("dragon").clone();
        let sprays = advance(world, &mut d, now);
        if let Some(mut p) = world.get_mut::<Position>(e) {
            p.0 = d.pose.at;
        }
        if let Some(mut a) = world.get_mut::<Aim>(e) {
            a.yaw = d.pose.yaw;
            a.pitch = 0.0;
        }
        for (from, dir, reach) in sprays {
            spray(world, id, e, &d.spec.burn, from, dir, reach, now);
        }
        *world.get_mut::<ShadowDragon>(e).expect("dragon") = d;
    }
    fires_tick(world, now);
}

/// One tick of its tree; the sprays due (from, direction, reach).
fn advance(world: &mut World, d: &mut ShadowDragon, now: u64) -> Vec<(Vec3, Vec3, f32)> {
    let mut sprays = Vec::new();
    let secs = now as f32 * DT;
    match d.phase {
        DragonPhase::Wake { tick } => {
            let (t0, t1) = (tick as f32 * DT, (tick + 1) as f32 * DT);
            let spec = d.spec.clone();
            step(&mut d.pose, &spec.wake, t0, t1.min(spec.wake.length));
            d.phase = if t1 >= spec.wake.length { start_circle(d) } else { DragonPhase::Wake { tick: tick + 1 } };
        }
        DragonPhase::Circle { from, tick } => {
            let spec = d.spec.clone();
            let (m0, m1) = (d.circle_time(from, tick), d.circle_time(from, tick + 1));
            let wraps = m1 < m0;
            if wraps {
                step(&mut d.pose, &spec.circle, m0, spec.circle.length);
                step(&mut d.pose, &spec.circle, 0.0, m1);
            } else {
                step(&mut d.pose, &spec.circle, m0, m1);
            }
            let crossed = |t: f32| if wraps { t > m0 || t <= m1 } else { t > m0 && t <= m1 };
            let ready = secs - d.attack_time > spec.interval;
            match spec.laps.iter().find(|l| crossed(l.at + l.notify)) {
                Some(l) if ready => {
                    d.section = l.next;
                    d.phase = pick_corner(d);
                }
                _ => d.phase = DragonPhase::Circle { from, tick: tick + 1 },
            }
        }
        DragonPhase::Move { goal, yaw } => {
            let to = goal - d.pose.at;
            if to.length() <= d.spec.accept {
                d.pose.yaw = yaw;
                let index = world.resource_mut::<SimRng>().range_u32(0, d.spec.breaths.len() as u32) as usize;
                d.phase = DragonPhase::Breath { index, tick: 0 };
            } else {
                let stride = (d.speed * DT).min(to.length());
                d.pose.at += to.normalize_or_zero() * stride;
                if to.truncate().length_squared() > 1.0 {
                    d.pose.yaw = to.y.atan2(to.x);
                }
            }
        }
        DragonPhase::Breath { index, tick } => {
            let spec = d.spec.clone();
            let b = &spec.breaths[index];
            let (t0, t1) = (tick as f32 * DT, (tick + 1) as f32 * DT);
            step(&mut d.pose, &b.clip, t0, t1.min(b.clip.length));
            for k in 0..b.sprays {
                if ticks(b.start + k as f32 * b.every) == tick + 1 {
                    let (s, c) = d.pose.yaw.sin_cos();
                    let dir = Vec3::new(c * b.pitch.cos(), s * b.pitch.cos(), b.pitch.sin());
                    sprays.push((d.pose.at, dir, b.reach));
                }
            }
            if t1 >= b.clip.length {
                // `AttackEnd`, then the circle again.
                d.attack_time = secs;
                d.phase = start_circle(d);
            } else {
                d.phase = DragonPhase::Breath { index, tick: tick + 1 };
            }
        }
    }
    sprays
}

/// `Circle`: face the current section's rotation and play the circle from that section.
fn start_circle(d: &mut ShadowDragon) -> DragonPhase {
    d.pose.yaw = d.corner(d.section).1;
    DragonPhase::Circle { from: d.section.clamp(1, d.spec.laps.len() as u32), tick: 0 }
}

/// `MoveToNextSection`'s `PickCircleSection`: the section's corner, or the next one when it is behind.
fn pick_corner(d: &mut ShadowDragon) -> DragonPhase {
    let forward = Vec2::from_angle(d.pose.yaw).extend(0.0);
    let (goal, _) = d.corner(d.section);
    if forward.angle_between(goal - d.pose.at).to_degrees() > 90.0 {
        d.section = if d.section + 1 > d.spec.corners.len() as u32 { 1 } else { d.section + 1 };
    }
    let (goal, yaw) = d.corner(d.section);
    DragonPhase::Move { goal, yaw }
}

/// One spray: a line trace against the level; a fire patch where it hits.
#[allow(clippy::too_many_arguments)]
fn spray(world: &mut World, id: SimId, e: Entity, burn: &Burn, from: Vec3, dir: Vec3, reach: f32, now: u64) {
    let hit = {
        let mut state: SystemState<(SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = SystemState::new(world);
        let (spatial, boxes) = state.get(world).expect("spatial query");
        Dir3::new(dir).ok().and_then(|d| spatial.cast_ray_predicate(from, d, reach, true, &crate::layers::shots(), &|b| boxes.contains(b))).map(|h| from + dir * h.distance)
    };
    let (Some(at), Some(c)) = (hit, world.get::<Combatant>(e).copied()) else { return };
    let attack = crate::buffs::attack_of(world, e, c.attack);
    world.resource_mut::<SimEvents>().push(SimEvent::FirePatchSpawned { owner: id, at, burn: *burn });
    world.resource_mut::<DragonFires>().patches.push(FirePatch { owner: id, at, burn: *burn, attack, until: now + u64::from(ticks(burn.life)) });
}

/// The patches: those in one burn (kept on while they stay in it); each burning does its damage when it starts and
/// every period until it runs out.
fn fires_tick(world: &mut World, now: u64) {
    let mut fires = std::mem::take(&mut *world.resource_mut::<DragonFires>());
    fires.patches.retain(|p| now < p.until);
    if fires.patches.is_empty() && fires.burning.is_empty() {
        *world.resource_mut::<DragonFires>() = fires;
        return;
    }
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for &(tid, te) in &ids {
        if world.get::<Combatant>(te).is_none() || world.get::<crate::monsters::MonsterKit>(te).is_some() || world.get::<Health>(te).is_none_or(|h| h.current <= 0.0) {
            continue;
        }
        let Some(c) = world.get::<Position>(te).map(|p| p.0) else { continue };
        for p in &fires.patches {
            let (flat, dz) = ((c - p.at).truncate().length(), (c.z - p.at.z).abs());
            if flat > p.burn.radius + CAPSULE_RADIUS || dz > p.burn.half_height + CAPSULE_HEIGHT * 0.5 {
                continue;
            }
            let until = now + u64::from(ticks(p.burn.duration));
            match fires.burning.iter_mut().find(|b| b.target == tid && b.owner == p.owner) {
                Some(b) if crate::overrides::shadow_dragon::REFRESH_WHILE_INSIDE => b.until = b.until.max(until),
                Some(_) => {}
                None => fires.burning.push(Burning { target: tid, owner: p.owner, at: p.at, until, next: now, attack: p.attack, burn: p.burn }),
            }
        }
    }
    let due: Vec<Burning> = fires.burning.iter().filter(|b| b.next == now && now < b.until).copied().collect();
    for b in &mut fires.burning {
        if b.next == now {
            b.next = now + u64::from(ticks(b.burn.period).max(1));
        }
    }
    fires.burning.retain(|b| now < b.until && world.resource::<SimIndex>().get(b.target).and_then(|e| world.get::<Health>(e)).is_some_and(|h| h.current > 0.0));
    *world.resource_mut::<DragonFires>() = fires;
    for b in due {
        let Some(te) = world.resource::<SimIndex>().get(b.target) else { continue };
        let owner = world.resource::<SimIndex>().get(b.owner).unwrap_or(te);
        crate::staffs::strike_as(world, b.owner, owner, b.target, te, b.burn.element, b.burn.factor, b.at, HitSource::Burn, Some(b.attack));
    }
}

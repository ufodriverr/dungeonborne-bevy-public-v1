//! The maps' lever mechanisms: the `BP_Mechanism` doors (`BP_Metal_Door_04` portcullises, the
//! `BP_FantasyDungeon_Door_13` stone gates) and the `BP_Handle_*` levers that work them, from the run
//! files.
//! - a mechanism starts shut (`bIsOn` false); on, its moving parts travel by `Movement` (actor space, so scaled
//!   with the actor) over `MoveDuration`, off they come back (HYPOTHESIS: linear, `MechanismMoveNode` is native);
//!   a part stops what its profile says (the portcullis bars, `TgfInteractableHollowDoor`: characters only);
//! - a lever is held by a player within `BaseInteractDistance` of its handle: `BaseInteractTime_TurnOn` (3 s) when
//!   off, `_TurnOff` (1 s) when on; then it flips, and `InteractionInterval` (1 s) later (`BP_SwitchBase`'s
//!   `Delay`) triggers its mechanism with `OnParam` / `OffParam` 2: a toggle. Busy until then;
//! - some turn instead (`Rotation`, the floor 2 arena's stone doors, ±75° over 5 s about `MoveBase`), and some are
//!   triggered by a monster's death (`DeathTriggerMechanism` from its spawn point's `ControlledMechanism`, with
//!   `TriggerParam` 0 off / 1 on / 2 toggle: the Wendigo opens its arena); a mechanism's `BoundMechanisms` are
//!   triggered with it (HYPOTHESIS: with the same param);
//! - while a mechanism isn't all the way on, its blocking parts are cut from the monsters' grid, as shut doors are;
//!   monsters don't pull levers (`CanPlayerInteract`, HYPOTHESIS as for the doors).
//!   HYPOTHESES: a toggle mid-travel turns it back from where it is; parts moving into a character push it out
//!   sideways (as the door leaves).

use std::sync::Arc;

use avian3d::prelude::Rotation;
use bevy_ecs::prelude::*;
use bevy_math::{Quat, Vec3};

use crate::command::{ActorKind, Kind, LevelBox};
use crate::id::{SimIdAllocator, SimIndex};
use crate::melee::Health;
use crate::movement::static_box;
use crate::swing_doors::Interacting;
use crate::{DT, Position, SimEvent, SimEvents, SimId, TickCount};

/// One moving part: a box in actor space where it stands shut, and what it stops ([`crate::layers::of_kind`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MechPart {
    pub center: Vec3,
    pub half: Vec3,
    pub kind: u32,
    /// its own move when its class moves several nodes (the escape doors' two leaves), else the mechanism's.
    pub motion: Option<PartMotion>,
}

/// A part's own travel, turn and pivot (actor space; as [`MechanismPlacement`]'s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartMotion {
    pub travel: Vec3,
    pub turn: Vec3,
    pub pivot: Vec3,
}

/// a node the client moves (blueprint component name) with its travel and turn about its own origin.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeMotion {
    pub node: String,
    pub travel: Vec3,
    pub turn: Vec3,
}

/// A placed lever.
#[derive(Debug, Clone, PartialEq)]
pub struct LeverPlacement {
    pub class: String,
    pub pos: Vec3,
    /// Degrees.
    pub yaw: f32,
    pub scale: Vec3,
    /// `BaseInteractTime_TurnOn` / `_TurnOff` (s), `BaseInteractDistance` (cm), `InteractionInterval` (s).
    pub hold_on: f32,
    pub hold_off: f32,
    pub reach: f32,
    pub interval: f32,
    /// Its `TraceBox` centre (actor space), where reach is measured to.
    pub handle: Vec3,
    /// Its sound events by role (`TurnOnSound`, `TurnOffSound`).
    pub sounds: Vec<(String, String)>,
}

/// A placed mechanism with its levers.
#[derive(Debug, Clone, PartialEq)]
pub struct MechanismPlacement {
    pub class: String,
    pub pos: Vec3,
    /// Degrees.
    pub yaw: f32,
    pub scale: Vec3,
    /// `Movement` (actor space, cm) and `MoveDuration` (s).
    pub travel: Vec3,
    pub secs: f32,
    /// `Rotation` about `pivot` (actor space, the `MoveBase`) over the same time: Unreal's vector, X roll, Y pitch,
    /// Z yaw, degrees (`MakeRotator(Roll = X, Pitch = Y, Yaw = Z)`; yaw, roll).
    pub turn: Vec3,
    pub pivot: Vec3,
    /// Its index on its floor (what a death trigger names) and the mechanisms triggered with it.
    pub index: u32,
    pub bound: Vec<u32>,
    pub parts: Vec<MechPart>,
    /// Its sound events by role (`TurnOnSound`, `TurnOnLoopSound`, `TurnOffSound`, ...).
    pub sounds: Vec<(String, String)>,
    pub levers: Vec<LeverPlacement>,
    /// A chained door's chain (drawn only).
    pub chain: Option<MechChain>,
    /// a `BP_LoopMechanism` (the lifts): it turns itself on and off.
    pub looping: Option<MechLoop>,
    /// it never moves and its parts are level geometry already (in the collision cache): drawn only.
    pub baked: bool,
    /// the escape doors move their own nodes (`nodes`), eased in and out (`ease`: p^2 (3 - 2p), CONFIRMED
    /// bytecode); opened by their escape point.
    pub nodes: Vec<NodeMotion>,
    pub ease: bool,
}

/// A `BP_LoopMechanism`'s cycle (CONFIRMED bytecode): at `BeginPlay` its timeline jumps to `start` s; while
/// `active` (`Loop`) it flips on and off, each flip waiting `MoveDuration` + `up` (on) or `down` (off) s;
/// `OnTriggered` with a param above 0 starts it, 0 stops it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MechLoop {
    pub active: bool,
    pub up: f32,
    pub down: f32,
    pub start: f32,
}

/// A chained door's chain (`BP_ChainedDoor.Update Chain`): links every `unit` cm from the `Fastener` (`fastener`,
/// on `MoveBase`, actor space at rest) toward the `FixPoint` (`fix`, actor space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MechChain {
    pub fix: Vec3,
    pub fastener: Vec3,
    pub unit: f32,
}

impl MechanismPlacement {
    /// One from a run file's `mechanisms` entry and the floor's `levers` (`mechanism` = its index `i`).
    pub fn from_json(m: &serde_json::Value, i: usize, levers: &[serde_json::Value]) -> Option<Self> {
        let f = |v: &serde_json::Value, d: f32| v.as_f64().map_or(d, |x| x as f32);
        let v3 = |v: &serde_json::Value, d: f32| Vec3::new(f(&v[0], d), f(&v[1], d), f(&v[2], d));
        let sounds = |v: &serde_json::Value| v.as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()).unwrap_or_default();
        let motion = |v: &serde_json::Value| v.is_object().then(|| PartMotion { travel: v3(&v["travel"], 0.0), turn: v3(&v["turn"], 0.0), pivot: v3(&v["pivot"], 0.0) });
        let parts: Vec<MechPart> = m["parts"].as_array()?.iter().map(|p| MechPart { center: v3(&p["center"], 0.0), half: v3(&p["half"], 0.0), kind: p["kind"].as_u64().unwrap_or(0) as u32, motion: motion(&p["motion"]) }).collect();
        let nodes = m["nodes"].as_array().map(|n| n.iter().map(|n| NodeMotion { node: n["node"].as_str().unwrap_or_default().to_string(), travel: v3(&n["travel"], 0.0), turn: v3(&n["turn"], 0.0) }).collect()).unwrap_or_default();
        let levers = levers
            .iter()
            .filter(|l| l["mechanism"].as_u64() == Some(i as u64))
            .map(|l| LeverPlacement {
                class: l["class"].as_str().unwrap_or_default().to_string(),
                pos: v3(&l["pos"], 0.0),
                yaw: f(&l["yaw"], 0.0),
                scale: v3(&l["scale"], 1.0),
                hold_on: f(&l["hold_on"], 1.0),
                hold_off: f(&l["hold_off"], 1.0),
                reach: f(&l["reach"], 250.0),
                interval: f(&l["interval"], 1.0),
                handle: v3(&l["handle"], 0.0),
                sounds: sounds(&l["sounds"]),
            })
            .collect();
        let bound = m["bound"].as_array().map(|b| b.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect()).unwrap_or_default();
        Some(MechanismPlacement { class: m["class"].as_str()?.to_string(), pos: v3(&m["pos"], 0.0), yaw: f(&m["yaw"], 0.0), scale: v3(&m["scale"], 1.0), travel: v3(&m["travel"], 0.0), secs: f(&m["secs"], 1.0), turn: m["turn"].as_f64().map_or_else(|| v3(&m["turn"], 0.0), |z| Vec3::new(0.0, 0.0, z as f32)), pivot: v3(&m["pivot"], 0.0), index: i as u32, bound, parts, sounds: sounds(&m["sounds"]), levers, looping: m["loop"].is_object().then(|| MechLoop { active: m["loop"]["loop"].as_bool().unwrap_or(false), up: f(&m["loop"]["up"], 1.0), down: f(&m["loop"]["down"], 1.0), start: f(&m["loop"]["start"], 0.0) }), baked: m["baked"].as_bool().unwrap_or(false), nodes, ease: m["ease"].as_bool().unwrap_or(false), chain: m["chain"].is_object().then(|| MechChain { fix: v3(&m["chain"]["fix"], 0.0), fastener: v3(&m["chain"]["fastener"], 0.0), unit: f(&m["chain"]["unit"], 25.0) }) })
    }

    /// The same, moved by `off` (a floor's world offset).
    pub fn offset(&self, off: Vec3) -> Self {
        let mut m = self.clone();
        m.pos += off;
        for l in &mut m.levers {
            l.pos += off;
        }
        m
    }

    /// The same with its index (and its bound ones') made unique across the dungeon: floor `floor`'s mechanisms
    /// count from `floor` × [`FLOOR_STRIDE`] (all floors stand in one world).
    pub fn on_floor(mut self, floor: usize) -> Self {
        let base = floor as u32 * FLOOR_STRIDE;
        self.index += base;
        for b in &mut self.bound {
            *b += base;
        }
        self
    }

    /// A sound event by role.
    pub fn sound(&self, role: &str) -> Option<&str> {
        self.sounds.iter().find(|s| s.0 == role).map(|s| s.1.as_str())
    }
}

/// A mechanism in the world: on or off, and how far along its travel (0 shut .. 1 on).
#[derive(Component, Debug, Clone)]
pub struct Mechanism {
    pub placement: Arc<MechanismPlacement>,
    pub on: bool,
    pub t: f32,
    /// Its parts' collider entities.
    pub parts: Vec<Entity>,
    /// A looping one's next flip (tick).
    pub next_flip: Option<u64>,
}

impl Mechanism {
    pub fn moving(&self) -> bool {
        self.t != if self.on { 1.0 } else { 0.0 }
    }

    /// How far along its move it shows at travel `t` (eased for the escape doors, else `t`).
    pub fn progress(&self, t: f32) -> f32 {
        if self.placement.ease { t * t * (3.0 - 2.0 * t) } else { t }
    }

    /// The `MoveBase` turn at travel `t`, in the actor's unscaled space (HYPOTHESIS: slerped).
    pub fn local_turn(&self, t: f32) -> Quat {
        turn_quat(self.placement.turn, self.progress(t))
    }

    /// The same in actor space (a mirrored actor's turn mirrored with it).
    pub fn turn_at(&self, t: f32) -> Quat {
        self.mirrored(self.local_turn(t))
    }

    /// A turn in the actor's unscaled space made actor space (a mirrored actor's turn mirrored with it).
    pub fn mirrored(&self, mut q: Quat) -> Quat {
        let p = &self.placement;
        for (s, flip) in [(p.scale.x, Vec3::new(1.0, -1.0, -1.0)), (p.scale.y, Vec3::new(-1.0, 1.0, -1.0)), (p.scale.z, Vec3::new(-1.0, -1.0, 1.0))] {
            if s < 0.0 {
                q = Quat::from_xyzw(q.x * flip.x, q.y * flip.y, q.z * flip.z, q.w);
            }
        }
        q
    }

    /// Where a point on `MoveBase` (actor space at rest) is at travel `t`, actor space (unscaled).
    pub fn moved(&self, at_rest: Vec3, t: f32) -> Vec3 {
        let p = &self.placement;
        p.pivot + self.local_turn(t) * (at_rest - p.pivot) + p.travel * t
    }

    /// A part's box at travel `t`: centre (world), rotation (world), half extents.
    pub fn part_pose(&self, origin: Vec3, part: &MechPart, t: f32) -> (Vec3, Quat, Vec3) {
        let p = &self.placement;
        let k = self.progress(t);
        let (travel, turn, pivot) = part.motion.map_or((p.travel, p.turn, p.pivot), |m| (m.travel, m.turn, m.pivot));
        let r = turn_quat(turn, k);
        let local = pivot + r * (part.center - pivot) + travel * k;
        let yaw = Quat::from_rotation_z(p.yaw.to_radians());
        (origin + yaw * (p.scale * local), yaw * self.mirrored(r), (p.scale * part.half).abs())
    }

    /// The same as an upright box: centre, yaw (radians: the turn's twist about Z) and the half extents of what
    /// tilts out of it (for the push-out and the nav cuts).
    pub fn part_box(&self, origin: Vec3, part: &MechPart, t: f32) -> (Vec3, f32, Vec3) {
        let (c, rot, half) = self.part_pose(origin, part, t);
        let twist = Quat::from_xyzw(0.0, 0.0, rot.z, rot.w).normalize();
        let rest = bevy_math::Mat3::from_quat(twist.inverse() * rot);
        let abs = bevy_math::Mat3::from_cols(rest.x_axis.abs(), rest.y_axis.abs(), rest.z_axis.abs());
        let yaw = (2.0 * twist.z.atan2(twist.w) + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        (c, yaw, abs * half)
    }

    /// Whether it stands in the way when on rather than when off (a flap the lever raises): its blocking parts
    /// stand taller on.
    pub fn blocks_on(&self) -> bool {
        let height = |t: f32| self.placement.parts.iter().filter(|p| p.kind != 2).map(|p| self.part_box(Vec3::ZERO, p, t).2.z).fold(0.0, f32::max);
        height(1.0) > height(0.0) + 50.0
    }
}

/// A lever in the world.
#[derive(Component, Debug, Clone)]
pub struct Lever {
    pub mechanism: SimId,
    pub placement: LeverPlacement,
    pub on: bool,
    /// The tick it may be pulled again, and when its pull reaches the mechanism.
    pub ready_at: u64,
    pub fire_at: Option<u64>,
}

impl Lever {
    /// Where reach is measured to (world).
    pub fn handle(&self) -> Vec3 {
        let p = &self.placement;
        p.pos + Quat::from_rotation_z(p.yaw.to_radians()) * (p.scale * p.handle)
    }
}

/// Unreal's rotator `turn` (X roll, Y pitch, Z yaw, degrees) a fraction `k` of the way, in the sim's (Unreal-numbered)
/// space: yaw about +Z, pitch and roll about -Y and -X (CONFIRMED: `Movement.Rotation` of roll 44 is X -0.3746; yaw 75
/// is Z +0.6088).
pub fn turn_quat(turn: Vec3, k: f32) -> Quat {
    let r = turn * std::f32::consts::PI / 180.0;
    Quat::IDENTITY.slerp(Quat::from_rotation_z(r.z) * Quat::from_rotation_y(-r.y) * Quat::from_rotation_x(-r.x), k)
}

/// Mechanism indices per floor (see [`MechanismPlacement::on_floor`]).
pub const FLOOR_STRIDE: u32 = 1000;

/// A monster whose death triggers its floor's mechanism `index` with `param`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DeathTrigger {
    pub index: u32,
    pub param: u8,
}

/// `OnTriggered(param)` on the mechanism with floor index `index` and those bound to it: 0 off, 1 on, 2 toggle.
pub(crate) fn trigger(world: &mut World, index: u32, param: u8, depth: u32) {
    let Some((e, id)) = world.query::<(Entity, &SimId, &Mechanism)>().iter(world).find(|(_, _, m)| m.placement.index == index).map(|(e, id, _)| (e, *id)) else { return };
    let now = world.resource::<TickCount>().0;
    let mut m = world.get_mut::<Mechanism>(e).expect("mechanism");
    let on = match param {
        0 => false,
        1 => true,
        _ => !m.on,
    };
    // A loop mechanism: above 0 starts the loop (and flips now), 0 stops it.
    if let Some(l) = m.placement.looping {
        let (secs, up, down) = (m.placement.secs, l.up, l.down);
        if param > 0 {
            m.on = !m.on;
            let on = m.on;
            m.next_flip = Some(now + ticks(secs + if on { up } else { down }));
            world.resource_mut::<SimEvents>().push(SimEvent::MechanismMoved { id, on });
        } else {
            m.next_flip = None;
        }
        return;
    }
    let changed = on != m.on;
    m.on = on;
    let bound = m.placement.bound.clone();
    if changed {
        world.resource_mut::<SimEvents>().push(SimEvent::MechanismMoved { id, on });
    }
    if depth < 4 {
        for b in bound {
            trigger(world, b, param, depth + 1);
        }
    }
}

/// A mechanism part's collider.
#[derive(Component, Debug, Clone, Copy)]
pub struct MechPartOf(pub Entity);

fn ticks(s: f32) -> u64 {
    (s / DT).round().max(0.0) as u64
}

/// Places a shut mechanism and its levers (`SimCommand::SpawnMechanism`).
pub(crate) fn spawn(world: &mut World, placement: MechanismPlacement) {
    let placement = Arc::new(placement);
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let e = world.spawn((id, Kind(ActorKind::Mechanism), Position(placement.pos))).id();
    let mut mech = Mechanism { placement: placement.clone(), on: false, t: 0.0, parts: Vec::new(), next_flip: None };
    // A loop mechanism starts its timeline `start` s in and, looping, flips on at once (`ReceiveBeginPlay`).
    if let Some(l) = placement.looping {
        mech.t = (l.start / placement.secs.max(DT)).clamp(0.0, 1.0);
        if l.active {
            let now = world.resource::<TickCount>().0;
            mech.on = true;
            mech.next_flip = Some(now + ticks(placement.secs + l.up));
        }
    }
    for part in placement.parts.iter().filter(|_| !placement.baked) {
        let (c, rot, half) = mech.part_pose(placement.pos, part, 0.0);
        let mut pe = world.spawn((static_box(c, half), Rotation(rot), LevelBox { half_extents: half }, MechPartOf(e)));
        if let Some(layers) = crate::layers::of_kind(part.kind) {
            pe.insert(layers);
        }
        mech.parts.push(pe.id());
    }
    world.entity_mut(e).insert(mech);
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Mechanism });
    for lever in &placement.levers {
        let lid = world.resource_mut::<SimIdAllocator>().alloc();
        let le = world.spawn((lid, Kind(ActorKind::Lever), Position(lever.pos), Lever { mechanism: id, placement: lever.clone(), on: false, ready_at: 0, fire_at: None })).id();
        world.resource_mut::<SimIndex>().insert(lid, le);
        world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id: lid, kind: ActorKind::Lever });
    }
}

/// Whether `target` is a lever.
pub(crate) fn handles(world: &World, target: SimId) -> bool {
    world.resource::<SimIndex>().get(target).is_some_and(|t| world.get::<Lever>(t).is_some())
}

/// Starts holding a lever (`SimCommand::Interact` on a lever).
pub(crate) fn interact(world: &mut World, id: SimId, target: SimId) {
    let index = world.resource::<SimIndex>();
    let (Some(e), Some(t)) = (index.get(id), index.get(target)) else {
        return world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
    };
    let reject = |world: &mut World, reason| world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason });
    if world.get::<Kind>(e).is_none_or(|k| k.0 != ActorKind::Player) {
        return reject(world, "only players pull levers");
    }
    if world.get::<Interacting>(e).is_some_and(|i| i.target == target) {
        return;
    }
    let Some(lever) = world.get::<Lever>(t).cloned() else { return };
    let p = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
    if world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) || p.distance(lever.handle()) > lever.placement.reach {
        return reject(world, "out of reach");
    }
    let now = world.resource::<TickCount>().0;
    if now < lever.ready_at || lever.fire_at.is_some() {
        return reject(world, "the lever is busy");
    }
    let hold = crate::gear_triggers::interact_time(world, e, if lever.on { lever.placement.hold_off } else { lever.placement.hold_on });
    world.entity_mut(e).insert(Interacting { target, start: now, done_at: now + ticks(hold) });
    world.resource_mut::<SimEvents>().push(SimEvent::InteractHold { id, target, secs: hold });
}

/// Lever holds, pulls reaching their mechanisms, and the mechanisms' travel; then the nav cuts.
pub fn mechanisms_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let holds: Vec<(Entity, Interacting)> = world.query::<(Entity, &Interacting)>().iter(world).map(|(e, i)| (e, *i)).collect();
    for (e, hold) in holds {
        let Some(t) = world.resource::<SimIndex>().get(hold.target).filter(|&t| world.get::<Lever>(t).is_some()) else { continue };
        let Some(id) = world.get::<SimId>(e).copied() else { continue };
        let p = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
        let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
        let lever = world.get::<Lever>(t).cloned().expect("lever");
        if !alive || p.distance(lever.handle()) > lever.placement.reach {
            world.entity_mut(e).remove::<Interacting>();
            world.resource_mut::<SimEvents>().push(SimEvent::InteractCancelled { id });
            continue;
        }
        if now < hold.done_at {
            continue;
        }
        world.entity_mut(e).remove::<Interacting>();
        let mut l = world.get_mut::<Lever>(t).expect("lever");
        l.on = !l.on;
        l.ready_at = now + ticks(l.placement.interval);
        l.fire_at = Some(now + ticks(l.placement.interval));
        let on = l.on;
        world.resource_mut::<SimEvents>().push(SimEvent::LeverPulled { id, lever: hold.target, on });
    }
    // Pulls reaching their mechanisms: a toggle.
    let due: Vec<(Entity, SimId)> = world.query::<(Entity, &Lever)>().iter(world).filter(|(_, l)| l.fire_at.is_some_and(|f| now >= f)).map(|(e, l)| (e, l.mechanism)).collect();
    for (le, mid) in due {
        world.get_mut::<Lever>(le).expect("lever").fire_at = None;
        let Some(index) = world.resource::<SimIndex>().get(mid).and_then(|me| world.get::<Mechanism>(me)).map(|m| m.placement.index) else { continue };
        trigger(world, index, 2, 0);
    }
    // Deaths that trigger a mechanism.
    let dead: Vec<(Entity, DeathTrigger)> = world.query::<(Entity, &DeathTrigger, &Health)>().iter(world).filter(|(_, _, h)| h.current <= 0.0).map(|(e, d, _)| (e, *d)).collect();
    for (e, d) in dead {
        world.entity_mut(e).remove::<DeathTrigger>();
        trigger(world, d.index, d.param, 0);
    }
    // Loop mechanisms flipping.
    let flips: Vec<(Entity, SimId)> = world.query::<(Entity, &SimId, &Mechanism)>().iter(world).filter(|(_, _, m)| m.next_flip.is_some_and(|f| now >= f)).map(|(e, id, _)| (e, *id)).collect();
    for (e, id) in flips {
        let mut m = world.get_mut::<Mechanism>(e).expect("mechanism");
        let l = m.placement.looping.expect("a loop");
        m.on = !m.on;
        let on = m.on;
        m.next_flip = Some(now + ticks(m.placement.secs + if on { l.up } else { l.down }));
        world.resource_mut::<SimEvents>().push(SimEvent::MechanismMoved { id, on });
    }
    // Travel.
    let mechs: Vec<Entity> = world.query_filtered::<Entity, With<Mechanism>>().iter(world).collect();
    for e in mechs {
        let Some(origin) = world.get::<Position>(e).map(|p| p.0) else { continue };
        let Some(mut m) = world.get_mut::<Mechanism>(e) else { continue };
        if !m.moving() {
            continue;
        }
        let step = DT / m.placement.secs.max(DT);
        m.t = if m.on { (m.t + step).min(1.0) } else { (m.t - step).max(0.0) };
        let m = m.clone();
        for (part, &pe) in m.placement.parts.iter().zip(&m.parts) {
            let (c, rot, _) = m.part_pose(origin, part, m.t);
            let (_, yaw, half) = m.part_box(origin, part, m.t);
            if let Some(mut q) = world.get_mut::<Position>(pe) {
                q.0 = c;
            }
            if let Some(mut r) = world.get_mut::<Rotation>(pe) {
                r.0 = rot;
            }
            // (Not a lift's block: it is a floor that carries who stands on it, HYPOTHESIS.)
            if part.kind != 2 && m.placement.looping.is_none() {
                crate::swing_doors::push_out(world, c, yaw, half, 1.0);
            }
        }
    }
    crate::swing_doors::cut_nav(world);
}

/// The blocking parts of mechanisms not all the way open, where they stand blocking (for the nav cuts), and a key
/// of which they are: shut (off) for most; on for those that stand in the way on (the chained doors).
pub(crate) fn shut_parts(world: &mut World) -> (Vec<(Vec3, f32, Vec3)>, Vec<u64>) {
    let mut shut: Vec<(SimId, Vec3, Mechanism, bool)> = world
        .query::<(&SimId, &Position, &Mechanism)>()
        .iter(world)
        // (Not the lifts: floors, not doors.)
        .filter(|(_, _, m)| m.placement.looping.is_none())
        .map(|(id, p, m)| (*id, p.0, m.clone(), m.blocks_on()))
        .filter(|(_, _, m, on)| m.moving() || m.on == *on)
        .collect();
    shut.sort_by_key(|s| s.0.0);
    let boxes = shut.iter().flat_map(|(_, o, m, on)| m.placement.parts.iter().filter(|p| p.kind != 2).map(|p| m.part_box(*o, p, if *on { 1.0 } else { 0.0 })).collect::<Vec<_>>()).collect();
    (boxes, shut.iter().map(|s| s.0.0).collect())
}

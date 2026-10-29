//! The maps' swing doors: the blueprint doors placed in the World Partition cells (`BP_Door_03`,
//! `BP_Door_Broken_01`, the crypts' children of `BP_SwingDoor`), held open or shut by a player.
//!
//! CONFIRMED (the loose reads and bytecode of `BP_InteractableBase`, `BP_SwingDoor` and its children):
//! - a door is one or two leaves, each a box (`TraceBox_Door` / `TraceBox_Door1`, active when it blocks the Pawn
//!   channel: by its named profile, or a `Custom` one's own response) on a hinge (`Hinge` / `Hinge1`); the boxes block pawns, weapons,
//!   projectiles and sight at all times, the leaf meshes are only drawn; the actor's scale scales hinges and boxes;
//! - only players use them: a hold of `OpenBaseInteractTime` to open, `CloseBaseInteractTime` to close, from within
//!   `BaseInteractDistance`; after a use the door waits `InteractionInterval` (1 s), and it can't be used while moving;
//! - opening swings to the side the opener faces (`sign(dot(door forward, pawn forward))`; none when 0) by
//!   `SwingAngle` over `MoveDuration` on the timeline's ease (3t² − 2t³): `Hinge` by −angle × side, `Hinge1` by
//!   +angle × side; closing swings back the same way; no auto-close, locks or damage, monsters don't use them.
//!
//! HYPOTHESIS: the hold runs on once started and stops when the player leaves the door's reach, dies or lets go
//! (`SimCommand::InteractCancel`); reach is measured to the nearest leaf's centre; characters a moving leaf
//! overlaps are pushed out along the leaf's normal (the game pushes only the first pawn its sweep hits, while
//! opening).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use avian3d::prelude::Rotation;
use bevy_ecs::prelude::*;
use bevy_math::{Quat, Vec2, Vec3};
use serde_json::Value;

use crate::command::{ActorKind, Kind, LevelBox};
use crate::id::{SimIdAllocator, SimIndex};
use crate::melee::{Aim, Health};
use crate::movement::{CAPSULE_HEIGHT, CAPSULE_RADIUS, static_box};
use crate::staffs::{package_of, read_json};
use crate::{DT, Position, SimEvent, SimEvents, SimId, TickCount};

/// One leaf: its hinge (actor space, cm, before the actor's scale), its box's centre from the hinge, its half
/// extents (the box's `BoxExtent` × its own scale), and which way it turns (−1 `Hinge`, +1 `Hinge1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeafSpec {
    pub hinge: Vec3,
    pub center: Vec3,
    pub half: Vec3,
    pub sign: f32,
    /// Stops only characters (`TgfInteractableHollowDoor`, the barred jail doors: Pawn blocks by the channel's
    /// default, `Projectile` and `Tgf_TraceChannel_Sight` aren't listed and default to Ignore); the others stop
    /// shots and sight too..
    pub hollow: bool,
}

/// A swing door class, from its blueprint and parents.
#[derive(Debug, Clone, PartialEq)]
pub struct SwingDoorSpec {
    /// The blueprint (`BP_Door_03`).
    pub class: String,
    pub leaves: Vec<LeafSpec>,
    /// `SwingAngle` (degrees) and `MoveDuration` (s).
    pub swing_deg: f32,
    pub move_secs: f32,
    /// `OpenBaseInteractTime` / `CloseBaseInteractTime` (s), `BaseInteractDistance` (cm), `InteractionInterval` (s).
    pub open_hold: f32,
    pub close_hold: f32,
    pub reach: f32,
    pub interval: f32,
}

/// Reads a swing door class (`package`: its package path, `/TgfMaps/...` or `/Game/...`): the class defaults and
/// component templates merged over its parents' (the class first).
pub fn load_spec(package: &str) -> anyhow::Result<SwingDoorSpec> {
    let mut defaults: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut parts: BTreeMap<String, serde_json::Map<String, Value>> = BTreeMap::new();
    let mut names = Vec::new();
    let mut next = Some(package_of(package));
    while let Some(p) = next.take() {
        if names.len() > 12 {
            break;
        }
        let v = read_json("json/loose", &p).or_else(|| read_json("json/assets", &p)).with_context(|| format!("{p} not decoded"))?;
        let objs = v.as_array().context("not an export list")?;
        names.push(p.rsplit('/').next().unwrap_or(&p).to_string());
        // Child first: a property the class sets wins. A box's `BodyInstance` merges field by field (a child sets
        // only what differs: its profile name, or its channel responses), its responses channel by channel.
        let merge = |into: &mut serde_json::Map<String, Value>, from: &Value| {
            for (k, v) in from.as_object().into_iter().flatten() {
                match (k.as_str(), into.get_mut(k)) {
                    ("BodyInstance", Some(Value::Object(have))) => {
                        for (bk, bv) in v.as_object().into_iter().flatten() {
                            if bk == "CollisionResponses" {
                                let list = have.entry("CollisionResponses").or_insert_with(|| serde_json::json!({ "ResponseArray": [] }));
                                let known: Vec<Value> = list["ResponseArray"].as_array().cloned().unwrap_or_default();
                                let mut merged = known.clone();
                                for r in bv["ResponseArray"].as_array().into_iter().flatten() {
                                    if !known.iter().any(|x| x["Channel"] == r["Channel"]) {
                                        merged.push(r.clone());
                                    }
                                }
                                list["ResponseArray"] = Value::Array(merged);
                            } else {
                                have.entry(bk.clone()).or_insert_with(|| bv.clone());
                            }
                        }
                    }
                    (_, Some(_)) => {}
                    (_, None) => {
                        into.insert(k.clone(), v.clone());
                    }
                }
            }
        };
        for o in objs {
            let name = o["Name"].as_str().unwrap_or_default();
            if name.starts_with("Default__") {
                merge(&mut defaults, &o["Properties"]);
            } else if let Some(part) = name.strip_suffix("_GEN_VARIABLE") {
                merge(parts.entry(part.to_string()).or_default(), &o["Properties"]);
            }
        }
        next = objs.iter().find(|o| o["Type"] == "BlueprintGeneratedClass").and_then(|c| c["Super"]["ObjectPath"].as_str()).filter(|s| s.starts_with('/') && !s.starts_with("/Script/")).map(package_of);
    }
    anyhow::ensure!(names.iter().any(|n| n == "BP_SwingDoor"), "{package} is not a swing door ({names:?})");
    let vec3 = |v: &Value, d: Vec3| if v.is_object() { Vec3::new(v["X"].as_f64().unwrap_or(0.0) as f32, v["Y"].as_f64().unwrap_or(0.0) as f32, v["Z"].as_f64().unwrap_or(0.0) as f32) } else { d };
    let num = |k: &str| defaults.get(k).and_then(Value::as_f64).map(|v| v as f32).with_context(|| format!("{k} not decoded"));
    let mut leaves = Vec::new();
    for (hinge, part, sign) in [("Hinge", "TraceBox_Door", -1.0), ("Hinge1", "TraceBox_Door1", 1.0)] {
        let (Some(h), Some(b)) = (parts.get(hinge), parts.get(part)) else { continue };
        // `SetDoorCollisionEnabled` switches on only a box that blocks pawns: a named profile's response (the door
        // profiles block pawns: HYPOTHESIS, the engine's default response), a `Custom` one's own Pawn response.
        let body = b.get("BodyInstance");
        let profile = body.and_then(|i| i["CollisionProfileName"].as_str()).unwrap_or_default();
        let pawn = body.and_then(|i| i["CollisionResponses"]["ResponseArray"].as_array()).and_then(|a| a.iter().find(|r| r["Channel"] == "Pawn")).and_then(|r| r["Response"].as_str());
        let blocks = match profile {
            "Custom" => pawn.is_some_and(|r| r.ends_with("::2")),
            p => matches!(p, "TgfInteractableDoor" | "TgfInteractableHollowDoor" | "InteractionBlockPawn"),
        };
        // A `Custom` box's own Projectile / Sight responses (HYPOTHESIS: none listed is the channels' Ignore).
        let listed = |ch: &str| body.and_then(|i| i["CollisionResponses"]["ResponseArray"].as_array()).and_then(|a| a.iter().find(|r| r["Channel"] == ch)).and_then(|r| r["Response"].as_str()).is_some_and(|r| r.ends_with("::2"));
        let hollow = match profile {
            "Custom" => !(listed("Projectile") || listed("Tgf_TraceChannel_Sight")),
            p => p == "TgfInteractableHollowDoor",
        };
        if !blocks {
            continue;
        }
        let none = Value::Null;
        // A box's default `BoxExtent` is 32 cm.
        let half = vec3(b.get("BoxExtent").unwrap_or(&none), Vec3::splat(32.0)) * vec3(b.get("RelativeScale3D").unwrap_or(&none), Vec3::ONE);
        leaves.push(LeafSpec { hinge: vec3(h.get("RelativeLocation").unwrap_or(&none), Vec3::ZERO), center: vec3(b.get("RelativeLocation").unwrap_or(&none), Vec3::ZERO), half, sign, hollow });
    }
    anyhow::ensure!(!leaves.is_empty(), "{package}: no blocking leaf");
    Ok(SwingDoorSpec {
        class: names[0].clone(),
        leaves,
        swing_deg: num("SwingAngle")?,
        move_secs: num("MoveDuration")?,
        open_hold: num("OpenBaseInteractTime").or_else(|_| num("BaseInteractTime"))?,
        close_hold: num("CloseBaseInteractTime").unwrap_or(0.0),
        reach: num("BaseInteractDistance")?,
        interval: num("InteractionInterval").unwrap_or(0.0),
    })
}

/// [`load_spec`], read once per class.
pub fn spec(package: &str) -> Option<Arc<SwingDoorSpec>> {
    static CACHE: Mutex<BTreeMap<String, Option<Arc<SwingDoorSpec>>>> = Mutex::new(BTreeMap::new());
    let mut cache = CACHE.lock().ok()?;
    cache.entry(package.to_string()).or_insert_with(|| load_spec(package).ok().map(Arc::new)).clone()
}

/// A placed swing door. `Position` is its origin (the actor's root).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SwingDoor {
    pub spec: Arc<SwingDoorSpec>,
    /// The actor's yaw (radians) and scale.
    pub yaw: f32,
    pub scale: Vec3,
    pub open: bool,
    /// The side it last swung to (+1 / −1).
    pub dir: f32,
    /// Timeline position: 0 shut, 1 open.
    pub t: f32,
    /// The tick it may be used again (`InteractionInterval`).
    pub ready_at: u64,
    /// Its leaves' collider entities.
    pub leaves: Vec<Entity>,
}

impl SwingDoor {
    /// The timeline's ease (`CurveFloat_0`: cubic keys (0,0) and (1,1), flat tangents).
    pub fn eased(&self) -> f32 {
        self.t * self.t * (3.0 - 2.0 * self.t)
    }

    pub fn moving(&self) -> bool {
        self.t != if self.open { 1.0 } else { 0.0 }
    }

    /// A leaf's box: centre, rotation (yaw, radians) and half extents. A mirrored door (a negative X or Y scale,
    /// placed so in the maps) turns its hinges the other way in the world, as the hinge turns inside the mirror.
    pub fn leaf_box(&self, origin: Vec3, leaf: &LeafSpec) -> (Vec3, f32, Vec3) {
        let mirror = if self.scale.x * self.scale.y < 0.0 { -1.0 } else { 1.0 };
        let turn = mirror * leaf.sign * self.eased() * self.spec.swing_deg.to_radians() * self.dir;
        let yaw = self.yaw + turn;
        let (r, h) = (Quat::from_rotation_z(self.yaw), Quat::from_rotation_z(yaw));
        let centre = origin + r * (self.scale * leaf.hinge) + h * (self.scale * leaf.center);
        (centre, yaw, (self.scale * leaf.half).abs())
    }
}

/// Collider of one leaf.
#[derive(Component, Debug, Clone, Copy)]
pub struct DoorLeaf {
    pub door: Entity,
}

/// A player holding a door's interaction.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Interacting {
    pub target: SimId,
    pub start: u64,
    pub done_at: u64,
}

/// Places a shut swing door (`SimCommand::SpawnSwingDoor`).
pub(crate) fn spawn(world: &mut World, package: &str, pos: Vec3, yaw_deg: f32, scale: Vec3) {
    let Some(spec) = spec(package) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "not a swing door" });
        return;
    };
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let mut door = SwingDoor { spec: spec.clone(), yaw: yaw_deg.to_radians(), scale, open: false, dir: 1.0, t: 0.0, ready_at: 0, leaves: Vec::new() };
    let e = world.spawn((id, Kind(ActorKind::Door), Position(pos))).id();
    for leaf in &spec.leaves {
        let (c, yaw, half) = door.leaf_box(pos, leaf);
        // A hollow leaf stops characters only ([`crate::layers::PAWN_ONLY`]).
        let mut leaf_e = world.spawn((static_box(c, half), Rotation(Quat::from_rotation_z(yaw)), DoorLeaf { door: e }, LevelBox { half_extents: half }));
        if let Some(layers) = leaf.hollow.then(|| crate::layers::of_kind(1)).flatten() {
            leaf_e.insert(layers);
        }
        door.leaves.push(leaf_e.id());
    }
    world.entity_mut(e).insert(door);
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Door });
}

/// Whether `target` is a swing door.
pub(crate) fn handles(world: &World, target: SimId) -> bool {
    world.resource::<SimIndex>().get(target).is_some_and(|t| world.get::<SwingDoor>(t).is_some())
}

/// Distance from `p` to the door's nearest leaf centre.
fn reach_to(world: &World, t: Entity, p: Vec3) -> f32 {
    let (Some(d), Some(o)) = (world.get::<SwingDoor>(t), world.get::<Position>(t)) else { return f32::MAX };
    d.spec.leaves.iter().map(|l| d.leaf_box(o.0, l).0.distance(p)).fold(f32::MAX, f32::min)
}

/// Starts holding the door's interaction (`SimCommand::Interact` on a swing door).
pub(crate) fn interact(world: &mut World, id: SimId, target: SimId) {
    let index = world.resource::<SimIndex>();
    let (Some(e), Some(t)) = (index.get(id), index.get(target)) else {
        return world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
    };
    let reject = |world: &mut World, reason| world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason });
    if world.get::<Kind>(e).is_none_or(|k| k.0 != ActorKind::Player) {
        return reject(world, "only players use doors");
    }
    if world.get::<Interacting>(e).is_some_and(|i| i.target == target) {
        return;
    }
    let p = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
    let Some(door) = world.get::<SwingDoor>(t).cloned() else { return };
    let now = world.resource::<TickCount>().0;
    if world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) || reach_to(world, t, p) > door.spec.reach {
        return reject(world, "out of reach");
    }
    if door.moving() || now < door.ready_at {
        return reject(world, "the door is busy");
    }
    let hold = crate::gear_triggers::interact_time(world, e, if door.open { door.spec.close_hold } else { door.spec.open_hold });
    let done_at = now + ticks(hold);
    world.entity_mut(e).insert(Interacting { target, start: now, done_at });
    world.resource_mut::<SimEvents>().push(SimEvent::InteractHold { id, target, secs: hold });
}

/// Lets go of a held interaction (`SimCommand::InteractCancel`).
pub(crate) fn cancel(world: &mut World, id: SimId) {
    if let Some(e) = world.resource::<SimIndex>().get(id)
        && world.entity_mut(e).take::<Interacting>().is_some()
    {
        world.resource_mut::<SimEvents>().push(SimEvent::InteractCancelled { id });
    }
}

fn ticks(s: f32) -> u64 {
    (s / DT).round().max(0.0) as u64
}

/// Holds, then the swings: finished holds toggle their door, doors move along their timeline and push out the
/// characters their leaves sweep into.
pub fn swing_doors_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let holds: Vec<(Entity, Interacting)> = world.query::<(Entity, &Interacting)>().iter(world).map(|(e, i)| (e, *i)).collect();
    for (e, hold) in holds {
        let id = world.get::<SimId>(e).copied();
        let t = world.resource::<SimIndex>().get(hold.target);
        let p = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
        let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
        // A lever's hold is the mechanisms'. A lever's hold is the
        // mechanisms', a device's the run's.
        if t.is_some_and(|t| world.get::<crate::mechanisms::Lever>(t).is_some() || world.get::<crate::run::Portal>(t).is_some()) {
            continue;
        }
        let reach = t.and_then(|t| world.get::<SwingDoor>(t)).map(|d| d.spec.reach);
        let (Some(id), Some(t), Some(reach)) = (id, t, reach) else {
            world.entity_mut(e).remove::<Interacting>();
            continue;
        };
        if !alive || reach_to(world, t, p) > reach {
            world.entity_mut(e).remove::<Interacting>();
            world.resource_mut::<SimEvents>().push(SimEvent::InteractCancelled { id });
            continue;
        }
        if now < hold.done_at {
            continue;
        }
        world.entity_mut(e).remove::<Interacting>();
        let facing = world.get::<Aim>(e).map_or(Vec2::X, |a| Vec2::from_angle(a.yaw));
        let Some(mut door) = world.get_mut::<SwingDoor>(t) else { continue };
        if !door.open {
            let side = Vec2::from_angle(door.yaw).dot(facing);
            if side == 0.0 {
                continue;
            }
            door.dir = side.signum();
        }
        door.open = !door.open;
        door.ready_at = now + ticks(door.spec.interval);
        let open = door.open;
        world.resource_mut::<SimEvents>().push(SimEvent::DoorToggled { id, door: hold.target, open });
    }
    let doors: Vec<Entity> = world.query_filtered::<Entity, With<SwingDoor>>().iter(world).collect();
    for e in doors {
        let Some(origin) = world.get::<Position>(e).map(|p| p.0) else { continue };
        let Some(mut door) = world.get_mut::<SwingDoor>(e) else { continue };
        if !door.moving() {
            continue;
        }
        let step = DT / door.spec.move_secs.max(DT);
        door.t = if door.open { (door.t + step).min(1.0) } else { (door.t - step).max(0.0) };
        let door = door.clone();
        for (leaf, &le) in door.spec.leaves.iter().zip(&door.leaves) {
            let (c, yaw, half) = door.leaf_box(origin, leaf);
            if let Some(mut q) = world.get_mut::<Position>(le) {
                q.0 = c;
            }
            if let Some(mut r) = world.get_mut::<Rotation>(le) {
                r.0 = Quat::from_rotation_z(yaw);
            }
            push_out(world, c, yaw, half, door.dir);
        }
    }
}

/// Takes the shut doors' leaves (where they stand closed) out of the monsters' nav grids (a closed door is null area
/// on the navmesh; a door counts as shut until it is all the way open), and the parts of mechanisms not all the way
/// on. Run after the mechanisms' tick.
pub(crate) fn cut_nav(world: &mut World) {
    if !world.contains_resource::<crate::nav::NavMaps>() {
        return;
    }
    let mut shut: Vec<(SimId, Vec3, SwingDoor)> = world.query::<(&SimId, &Position, &SwingDoor)>().iter(world).filter(|(_, _, d)| !d.open || d.moving()).map(|(id, p, d)| (*id, p.0, d.clone())).collect();
    shut.sort_by_key(|s| s.0.0);
    let (parts, mechs) = crate::mechanisms::shut_parts(world);
    // The whole props: a breakable prop is null area on the navmesh while it stands (its `NavModifier`).
    let (props, prop_ids) = crate::destroyables::nav_boxes(world);
    let key = shut.iter().map(|s| s.0.0).chain([u64::MAX]).chain(mechs).chain([u64::MAX - 1]).chain(prop_ids).fold(0xcbf2_9ce4_8422_2325u64, |h, s| (h ^ s).wrapping_mul(0x0100_0000_01b3));
    let mut boxes: Vec<(Vec3, f32, Vec3)> = shut.iter().flat_map(|(_, origin, d)| {
        let closed = SwingDoor { t: 0.0, ..d.clone() };
        d.spec.leaves.iter().map(move |l| closed.leaf_box(*origin, l)).collect::<Vec<_>>()
    }).collect();
    boxes.extend(parts);
    boxes.extend(props);
    world.resource_mut::<crate::nav::NavMaps>().set_cuts(&boxes, key);
}

/// Moves every living character whose capsule overlaps the leaf box out along the leaf's normal (to the side its
/// centre is on; `side` when on the leaf's plane).
pub(crate) fn push_out(world: &mut World, c: Vec3, yaw: f32, half: Vec3, side: f32) {
    let ids: Vec<Entity> = world.resource::<SimIndex>().iter().map(|(_, e)| e).collect();
    let (rot, inv) = (Quat::from_rotation_z(yaw), Quat::from_rotation_z(-yaw));
    for e in ids {
        let character = world.get::<Kind>(e).is_some_and(|k| matches!(k.0, ActorKind::Player | ActorKind::Monster | ActorKind::Wanderer | ActorKind::Dummy | ActorKind::Summon));
        if !character || world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(mut p) = world.get_mut::<Position>(e) else { continue };
        let d = inv * (p.0 - c);
        if d.x.abs() >= half.x + CAPSULE_RADIUS || d.y.abs() >= half.y + CAPSULE_RADIUS || d.z.abs() >= half.z + CAPSULE_HEIGHT * 0.5 {
            continue;
        }
        let s = if d.x == 0.0 { side } else { d.x.signum() };
        let out = Vec3::new(s * (half.x + CAPSULE_RADIUS + 1.0) - d.x, 0.0, 0.0);
        p.0 += rot * out;
    }
}

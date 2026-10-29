//! Run rules: portals, the run clock and how a run ends. A portal down moves its user to the next
//! floor; an extraction portal ends the run and the player keeps everything carried; death ends it
//! with everything carried dropped where the player fell; when the clock runs out, players still
//! inside die (HYPOTHESIS, as the game's end-of-time curse; the real rules are in blocked
//! blueprints). Portals are used instantly with `Interact` (HYPOTHESIS: no channel time yet).

use avian3d::prelude::Position;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::dungeon::PortalKind;
use crate::id::SimIdAllocator;
use crate::overrides::run as tune;
use crate::{ActorKind, CharacterState, DT, Health, Inventory, Kind, SimEvent, SimEvents, SimId, SimIndex};

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Portal {
    pub kind: PortalKind,
    /// Where a portal down takes its user (capsule centre, cm).
    pub dest: Option<Vec3>,
    /// Ticks until it can be used (the crypt's escape delays).
    pub closed_ticks: u32,
    /// An extraction device: held this long (s) to take the player out, then used up; 0 for a portal.
    pub hold: f32,
}

impl Portal {
    pub fn open(&self) -> bool {
        self.closed_ticks == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Extracted,
    Died,
    /// The run clock ran out before extraction.
    TimedOut,
}

/// A player taking part in the run: current floor (0-based) and, once over, how it ended.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct RunStatus {
    pub floor: u32,
    pub outcome: Option<RunOutcome>,
}

/// Ticks left in the run (`None`: no run, e.g. the test room).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct RunClock {
    pub ticks_left: Option<u32>,
    /// The run's full length (the time spent in it earns experience).
    pub ticks_total: Option<u32>,
}

impl RunClock {
    /// Seconds since the run began.
    pub fn elapsed_secs(&self) -> f32 {
        self.ticks_total.zip(self.ticks_left).map_or(0.0, |(t, l)| t.saturating_sub(l) as f32 * DT)
    }
}

/// A portal's escape doors: the mechanisms it triggers on (`OpenGate`: `OnTriggered(1)` on each of its
/// escape point's `ActivationMechanisms`, CONFIRMED bytecode) when it opens; removed once done.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct OpensMechanisms(pub Vec<u32>);

pub(crate) fn spawn_portal(world: &mut World, pos: Vec3, kind: PortalKind, dest: Option<Vec3>, opens_in: f32) -> SimId {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let closed_ticks = (opens_in.max(0.0) / DT).round() as u32;
    let e = world.spawn((id, Kind(ActorKind::Portal), Position(pos + Vec3::Z * tune::PORTAL_HEIGHT), Portal { kind, dest, closed_ticks, hold: 0.0 })).id();
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Portal });
    id
}

pub(crate) fn clear_world(world: &mut World) {
    let actors: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for (id, e) in actors {
        world.resource_mut::<SimIndex>().remove(id);
        world.despawn(e);
        world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id });
    }
    let boxes: Vec<Entity> = world.query_filtered::<Entity, Or<(With<crate::LevelBox>, With<crate::swing_doors::DoorLeaf>)>>().iter(world).collect();
    for e in boxes {
        world.despawn(e);
    }
    world.resource_mut::<RunClock>().ticks_left = None;
}

pub(crate) fn start_run(world: &mut World, seconds: u32) {
    let ticks = Some((seconds as f32 / DT).round() as u32);
    *world.resource_mut::<RunClock>() = RunClock { ticks_left: ticks, ticks_total: ticks };
    let players: Vec<Entity> = world.resource::<SimIndex>().iter().map(|(_, e)| e).filter(|&e| world.get::<Kind>(e).is_some_and(|k| k.0 == ActorKind::Player)).collect();
    for e in players {
        world.entity_mut(e).insert(RunStatus::default());
    }
}

/// An extraction device the death zone spawns: held `hold` s, then it takes the player out.
pub(crate) fn spawn_device(world: &mut World, pos: Vec3, hold: f32) -> SimId {
    let id = spawn_portal(world, pos, PortalKind::Extract, None, 0.0);
    if let Some(e) = world.resource::<SimIndex>().get(id)
        && let Some(mut p) = world.get_mut::<Portal>(e)
    {
        p.hold = hold;
    }
    id
}

/// `SimCommand::DevOnFloor`.
pub(crate) fn dev_on_floor(world: &mut World, floor: u32) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for (id, e) in ids {
        let Some(s) = world.get::<RunStatus>(e).copied() else { continue };
        world.entity_mut(e).insert(RunStatus { floor, ..s });
        world.resource_mut::<SimEvents>().push(SimEvent::FloorChanged { id, floor });
    }
}

/// `interact` found a portal in reach of a living actor.
pub(crate) fn use_portal(world: &mut World, id: SimId, e: Entity, portal: Portal) {
    use_portal_at(world, id, e, None, portal);
}

/// The same, knowing the portal's id (`target`; a device needs it for its hold).
pub(crate) fn use_portal_at(world: &mut World, id: SimId, e: Entity, target: Option<SimId>, portal: Portal) {
    let Some(status) = world.get::<RunStatus>(e).copied().filter(|s| s.outcome.is_none()) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "not in a run" });
        return;
    };
    if !portal.open() {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "portal not open yet" });
        return;
    }
    // A device is held first; `run_tick` finishes the hold.
    if portal.hold > 0.0 {
        let Some(target) = target else { return };
        if world.get::<crate::swing_doors::Interacting>(e).is_some_and(|i| i.target == target) {
            return;
        }
        let now = world.resource::<crate::TickCount>().0;
        let hold = crate::gear_triggers::interact_time(world, e, portal.hold);
        let done_at = now + (hold / DT).round() as u64;
        world.entity_mut(e).insert(crate::swing_doors::Interacting { target, start: now, done_at });
        world.resource_mut::<SimEvents>().push(SimEvent::InteractHold { id, target, secs: hold });
        return;
    }
    match (portal.kind, portal.dest) {
        (PortalKind::Down, Some(dest)) => {
            world.entity_mut(e).insert(Position(dest));
            if let Some(mut c) = world.get_mut::<CharacterState>(e) {
                c.velocity = Vec3::ZERO;
            }
            let floor = status.floor + 1;
            world.entity_mut(e).insert(RunStatus { floor, ..status });
            world.resource_mut::<SimEvents>().push(SimEvent::FloorChanged { id, floor });
        }
        (PortalKind::Down, None) => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "portal leads nowhere" }),
        (PortalKind::Extract, _) => {
            let kept = world.get::<Inventory>(e).cloned().unwrap_or_default();
            world.resource_mut::<SimIndex>().remove(id);
            world.despawn(e);
            let mut ev = world.resource_mut::<SimEvents>();
            ev.push(SimEvent::RunEnded { id, outcome: RunOutcome::Extracted, kept: Some(Box::new(kept)) });
            ev.push(SimEvent::Despawned { id });
        }
    }
}

/// Run clock and deaths in the run; after combat and effects each tick, in id order.
/// [`SimCommand::Abandon`](crate::SimCommand::Abandon): a living player in an unfinished run dies.
pub(crate) fn abandon(world: &mut World, id: SimId) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    if world.get::<RunStatus>(e).is_none_or(|s| s.outcome.is_some()) {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "not in a run" });
        return;
    }
    match world.get_mut::<Health>(e) {
        Some(mut h) if h.current > 0.0 => h.current = 0.0,
        _ => return,
    }
    world.resource_mut::<SimEvents>().push(SimEvent::Died { id });
}

/// Holds on extraction devices: let go, out of reach or dead cancels; done, the device takes the player out and is
/// gone.
fn device_holds(world: &mut World) {
    let now = world.resource::<crate::TickCount>().0;
    let holds: Vec<(Entity, crate::swing_doors::Interacting)> = world.query::<(Entity, &crate::swing_doors::Interacting)>().iter(world).map(|(e, i)| (e, *i)).collect();
    for (e, hold) in holds {
        let Some(t) = world.resource::<SimIndex>().get(hold.target) else { continue };
        let Some(portal) = world.get::<Portal>(t).copied().filter(|p| p.hold > 0.0) else { continue };
        let Some(id) = world.get::<SimId>(e).copied() else { continue };
        let p = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
        let q = world.get::<Position>(t).map_or(Vec3::ZERO, |p| p.0);
        let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
        if !alive || p.distance(q) > tune::PORTAL_RANGE {
            world.entity_mut(e).remove::<crate::swing_doors::Interacting>();
            world.resource_mut::<SimEvents>().push(SimEvent::InteractCancelled { id });
            continue;
        }
        if now < hold.done_at {
            continue;
        }
        world.entity_mut(e).remove::<crate::swing_doors::Interacting>();
        world.resource_mut::<SimIndex>().remove(hold.target);
        world.despawn(t);
        world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id: hold.target });
        // The device takes a Ritual Scroll if the player carries one (`ConsumeScroll`).
        let scroll = world.get::<Inventory>(e).and_then(|inv| inv.bag.iter().find(|b| b.item.item.item_id == tune::TELEPORT_SCROLL).map(|b| b.item.uid));
        if let Some(uid) = scroll
            && let Some(mut inv) = world.get_mut::<Inventory>(e)
        {
            inv.take(uid);
            world.resource_mut::<SimEvents>().push(SimEvent::InventoryChanged { id });
        }
        use_portal(world, id, e, Portal { hold: 0.0, ..portal });
    }
}

pub(crate) fn run_tick(world: &mut World) {
    let mut time_up = false;
    if let Some(t) = world.resource::<RunClock>().ticks_left {
        time_up = t == 1;
        world.resource_mut::<RunClock>().ticks_left = Some(t.saturating_sub(1));
    }
    for mut p in world.query::<&mut Portal>().iter_mut(world) {
        p.closed_ticks = p.closed_ticks.saturating_sub(1);
    }
    device_holds(world);
    let opening: Vec<(Entity, Vec<u32>)> = world.query::<(Entity, &Portal, &OpensMechanisms)>().iter(world).filter(|(_, p, _)| p.open()).map(|(e, _, o)| (e, o.0.clone())).collect();
    for (e, mechs) in opening {
        world.entity_mut(e).remove::<OpensMechanisms>();
        for m in mechs {
            crate::mechanisms::trigger(world, m, 1, 0);
        }
    }
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for (id, e) in ids {
        let Some(status) = world.get::<RunStatus>(e).copied().filter(|s| s.outcome.is_none()) else { continue };
        // No health yet (the character's stats arrive after the spawn) counts as alive.
        let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
        let outcome = if time_up && alive {
            if let Some(mut h) = world.get_mut::<Health>(e) {
                h.current = 0.0;
            }
            world.resource_mut::<SimEvents>().push(SimEvent::Died { id });
            RunOutcome::TimedOut
        } else if !alive {
            RunOutcome::Died
        } else {
            continue;
        };
        world.entity_mut(e).insert(RunStatus { outcome: Some(outcome), ..status });
        // Everything carried falls where the player died.
        let carried = world.get_mut::<Inventory>(e).map(|mut inv| {
            let items: Vec<_> = inv.items().cloned().collect();
            let empty = Inventory::with_size(inv.cols, inv.rows);
            *inv = empty;
            items
        });
        if let Some(items) = carried.filter(|i| !i.is_empty()) {
            let at = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0) - Vec3::Z * crate::movement::CAPSULE_HEIGHT * 0.5;
            let dropped = crate::world_items::scatter(world, at, items);
            let mut ev = world.resource_mut::<SimEvents>();
            ev.push(SimEvent::LootDropped { id, items: dropped });
            ev.push(SimEvent::InventoryChanged { id });
        }
        world.resource_mut::<SimEvents>().push(SimEvent::RunEnded { id, outcome, kept: None });
    }
}

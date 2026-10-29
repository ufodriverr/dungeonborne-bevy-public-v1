//! Doors, loop traps and altars. Doors block until a player opens them (their collider is switched
//! off, not removed) and close again unless something stands in the doorway. Traps run a loop of up
//! and down phases and hurt every character in their area while up: floor spikes and the swinging
//! blade once per up phase, the fire statue in pulses. Altars give a one-time blessing.
//!
//! CONFIRMED: `BP_LoopTrap` defaults `bLoop` true, `UpTime` 2.0, `DownTime` 2.0; `BP_LoopMechanism`
//! defaults `UpTime` 1.0, `DownTime` 1.0; `DT_ActorSpawn_Altar_Solo` rows and chances
//! ([`ALTAR_SPAWN_SOLO`]); the altar kinds by their prompts (`Interactable_Activate_Altar_*`: attack,
//! defence, healing, "energy" (Clarity) and speed blessings). HYPOTHESIS: trap damage, trap areas, the
//! fire statue on the loop-mechanism timing, every blessing's effect and length, door sizes and timing
//! ([`crate::overrides::features`]); no data names them.

use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use avian3d::prelude::ColliderDisabled;

use crate::buffs::{Buff, BuffKind, add_buff, incoming_damage};
use crate::command::{ActorKind, Kind};
use crate::id::{SimIdAllocator, SimIndex};
use crate::melee::Health;
use crate::movement::{CAPSULE_HEIGHT, CAPSULE_RADIUS, static_box};
use crate::overrides::features as tune;
use crate::skills::{HitSource, SkillKind};
use crate::{DT, Position, SimEvent, SimEvents, SimId};

/// `DT_ActorSpawn_Altar_Solo` (single-player altar spawn points): row, chance; `None` spawns nothing.
pub const ALTAR_SPAWN_SOLO: &[(Option<AltarKind>, f64)] = &[
    (Some(AltarKind::Attack), 0.19),
    (Some(AltarKind::Defend), 0.19),
    (Some(AltarKind::Heal), 0.19),
    (Some(AltarKind::Clarity), 0.19),
    (Some(AltarKind::Speed), 0.19),
    (None, 0.05),
];

/// A door: blocks while closed. Its leaf runs along sim X (`along_x`) or Y.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Door {
    pub open: bool,
    pub along_x: bool,
}

impl Door {
    /// Half extents of the door leaf (cm).
    pub fn half(along_x: bool) -> Vec3 {
        let [w, t, h] = tune::DOOR_HALF;
        if along_x { Vec3::new(w, t, h) } else { Vec3::new(t, w, h) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrapKind {
    /// Floor spikes (`Play_Trap_FloorSpike_Appear/Disappear`).
    Spikes,
    /// A blade swinging across a corridor (`Play_Trap_SwingBlade_Whoosh`).
    Blade,
    /// `BP_Trap_FireStatue_02`: a gargoyle breathing fire in front of it.
    FireStatue,
}

impl TrapKind {
    /// Area half extents in the trap's own frame (x along its facing), damage per hit, pulse interval
    /// (s; 0 = one hit per actor per up phase), up and down time (s).
    pub fn params(self) -> ([f32; 3], f64, f32, f32, f32) {
        match self {
            TrapKind::Spikes => (tune::SPIKES_HALF, tune::SPIKES_DAMAGE, 0.0, tune::LOOP_TRAP_UP, tune::LOOP_TRAP_DOWN),
            TrapKind::Blade => (tune::BLADE_HALF, tune::BLADE_DAMAGE, 0.0, tune::LOOP_TRAP_UP, tune::LOOP_TRAP_DOWN),
            TrapKind::FireStatue => (tune::FIRE_HALF, tune::FIRE_DAMAGE, tune::FIRE_INTERVAL, tune::LOOP_MECHANISM_UP, tune::LOOP_MECHANISM_DOWN),
        }
    }

    /// World half extents of the area for a facing along X (`along_x`) or Y.
    pub fn half(self, along_x: bool) -> Vec3 {
        let [x, y, z] = self.params().0;
        if along_x { Vec3::new(x, y, z) } else { Vec3::new(y, x, z) }
    }
}

/// A loop trap. `Position` is the centre of its damage area.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Trap {
    pub kind: TrapKind,
    /// Facing (radians, 0 = +X): the corridor's direction, or where the statue breathes.
    pub yaw: f32,
    /// World half extents of the damage area (cm).
    pub half: Vec3,
    pub damage: f64,
    /// Ticks between pulses while up; 0 = one hit per actor per up phase.
    pub interval: u32,
    pub up: u32,
    pub down: u32,
    /// Ticks into the loop (0..up + down); up while below `up`.
    pub tick: u32,
    /// Actors already hit in this up phase.
    pub hit: Vec<SimId>,
}

impl Trap {
    pub fn is_up(&self) -> bool {
        self.tick < self.up
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AltarKind {
    Attack,
    Defend,
    Heal,
    Clarity,
    Speed,
}

impl AltarKind {
    pub fn name(self) -> &'static str {
        match self {
            AltarKind::Attack => "Attack blessing",
            AltarKind::Defend => "Defence blessing",
            AltarKind::Heal => "Healing blessing",
            AltarKind::Clarity => "Energy blessing",
            AltarKind::Speed => "Speed blessing",
        }
    }
}

/// An altar: one blessing, once.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Altar {
    pub kind: AltarKind,
    pub used: bool,
}

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

fn register(world: &mut World, e: Entity, id: SimId, kind: ActorKind) {
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind });
}

/// A closed door; `pos` is the floor point under its centre.
pub(crate) fn spawn_door(world: &mut World, pos: Vec3, along_x: bool) {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let half = Door::half(along_x);
    let centre = pos + Vec3::Z * half.z;
    let e = world.spawn((id, Kind(ActorKind::Door), Door { open: false, along_x })).id();
    world.entity_mut(e).insert(static_box(centre, half));
    register(world, e, id, ActorKind::Door);
}

/// A loop trap; `pos` is the floor point under the centre of its area, `phase` (s) how far into its
/// loop it starts.
pub(crate) fn spawn_trap(world: &mut World, pos: Vec3, kind: TrapKind, yaw: f32, phase: f32) {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let (_, damage, interval, up, down) = kind.params();
    let along_x = yaw.cos().abs() >= yaw.sin().abs();
    let half = kind.half(along_x);
    let (up, down) = (ticks(up).max(1), ticks(down).max(1));
    let tick = ticks(phase.max(0.0)) % (up + down);
    let trap = Trap { kind, yaw, half, damage, interval: ticks(interval), up, down, tick, hit: Vec::new() };
    let e = world.spawn((id, Kind(ActorKind::Trap), Position(pos + Vec3::Z * half.z), trap)).id();
    register(world, e, id, ActorKind::Trap);
}

/// An altar (a solid pedestal); `pos` is the floor point under it.
pub(crate) fn spawn_altar(world: &mut World, pos: Vec3, kind: AltarKind) {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let half = Vec3::from(tune::ALTAR_HALF);
    let e = world.spawn((id, Kind(ActorKind::Altar), Altar { kind, used: false })).id();
    world.entity_mut(e).insert(static_box(pos + Vec3::Z * half.z, half));
    register(world, e, id, ActorKind::Altar);
}

/// Whether `target` is a door or an altar (handled here, without game data).
pub(crate) fn handles(world: &World, target: SimId) -> bool {
    world.resource::<SimIndex>().get(target).is_some_and(|t| world.get::<Door>(t).is_some() || world.get::<Altar>(t).is_some())
}

/// Characters (anything with health and a capsule) whose capsule overlaps the box.
fn in_box(world: &mut World, centre: Vec3, half: Vec3, skip: Option<SimId>) -> Vec<(SimId, Entity)> {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    ids.into_iter()
        .filter(|&(id, e)| {
            let character = world.get::<Kind>(e).is_some_and(|k| matches!(k.0, ActorKind::Player | ActorKind::Monster | ActorKind::Wanderer | ActorKind::Dummy | ActorKind::Summon));
            let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
            let Some(p) = world.get::<Position>(e).map(|p| p.0) else { return false };
            let d = (p - centre).abs();
            Some(id) != skip && character && alive && d.x <= half.x + CAPSULE_RADIUS && d.y <= half.y + CAPSULE_RADIUS && d.z <= half.z + CAPSULE_HEIGHT * 0.5
        })
        .collect()
}

/// Opens or closes a door, or takes an altar's blessing.
pub(crate) fn interact(world: &mut World, id: SimId, target: SimId) {
    let index = world.resource::<SimIndex>();
    let (Some(e), Some(t)) = (index.get(id), index.get(target)) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
        return;
    };
    let (Some(p), Some(q)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Position>(t).map(|p| p.0)) else { return };
    let alive = world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
    if !alive || (q - p).length() > crate::overrides::world_items::INTERACT_RANGE {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "out of reach" });
        return;
    }
    if let Some(door) = world.get::<Door>(t).copied() {
        if door.open {
            // A door doesn't close on anyone standing in the doorway.
            if !in_box(world, q, Door::half(door.along_x), None).is_empty() {
                world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "something is in the doorway" });
                return;
            }
            world.entity_mut(t).remove::<ColliderDisabled>();
        } else {
            world.entity_mut(t).insert(ColliderDisabled);
        }
        if let Some(mut d) = world.get_mut::<Door>(t) {
            d.open = !door.open;
        }
        world.resource_mut::<SimEvents>().push(SimEvent::DoorToggled { id, door: target, open: !door.open });
        return;
    }
    if let Some(altar) = world.get::<Altar>(t).copied() {
        if altar.used {
            world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "the altar is spent" });
            return;
        }
        if let Some(mut a) = world.get_mut::<Altar>(t) {
            a.used = true;
        }
        bless(world, e, altar.kind);
        world.resource_mut::<SimEvents>().push(SimEvent::AltarUsed { id, altar: target, kind: altar.kind });
    }
}

/// The blessing's effect (HYPOTHESIS values, `overrides::features`).
fn bless(world: &mut World, e: Entity, kind: AltarKind) {
    let buff = |kind: BuffKind, seconds: f32| Buff { kind, skill: SkillKind::Blessing, ticks_left: ticks(seconds), age: 0 };
    match kind {
        AltarKind::Attack => add_buff(world, e, buff(BuffKind::Battle { attack_boost: tune::ATTACK_BOOST, physical_resist: 0.0, life_steal: 0.0 }, tune::BLESSING_SECONDS)),
        AltarKind::Defend => add_buff(world, e, buff(BuffKind::DamageReduction(tune::DAMAGE_REDUCTION), tune::BLESSING_SECONDS)),
        AltarKind::Heal => add_buff(world, e, buff(BuffKind::Regen(tune::HEAL_PER_SECOND), tune::HEAL_SECONDS)),
        AltarKind::Speed => add_buff(world, e, buff(BuffKind::MoveSpeed(tune::SPEED_BONUS), tune::BLESSING_SECONDS)),
        AltarKind::Clarity => {
            // Full power and every skill ready.
            if let Some(mut pw) = world.get_mut::<crate::class_skills::Power>(e) {
                pw.current = pw.max;
            }
            if let Some(mut k) = world.get_mut::<crate::class_skills::ClassKit>(e) {
                k.cooldowns.clear();
            }
            if let Some(mut k) = world.get_mut::<crate::skills::SkillKit>(e) {
                k.whirlwind_cooldown = 0;
                k.war_cry_cooldown = 0;
                k.rend_cooldown = 0;
            }
        }
    }
}

/// Runs every trap's loop and its damage; after buffs, before deaths drop loot. In id order.
pub(crate) fn features_tick(world: &mut World) {
    let traps: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().filter(|&(_, e)| world.get::<Trap>(e).is_some()).collect();
    for (id, e) in traps {
        let Some(mut trap) = world.get::<Trap>(e).cloned() else { continue };
        // This tick's place in the loop; it moves on at the end.
        let (t, up) = (trap.tick, trap.is_up());
        if t == 0 || t == trap.up {
            if up {
                trap.hit.clear();
            }
            world.resource_mut::<SimEvents>().push(SimEvent::TrapCycled { id, up });
        }
        if up {
            let centre = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
            let pulse = trap.interval > 0 && t % trap.interval == 0;
            for (tid, te) in in_box(world, centre, trap.half, None) {
                let hits = if trap.interval == 0 { !trap.hit.contains(&tid) } else { pulse };
                if !hits {
                    continue;
                }
                if trap.interval == 0 {
                    trap.hit.push(tid);
                }
                let damage = incoming_damage(world, te, trap.damage, None);
                let left = crate::world_items::apply_damage(world, tid, te, damage);
                let mut events = world.resource_mut::<SimEvents>();
                events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: false, blocked: false, health: left, by: HitSource::Trap(trap.kind) });
                if left <= 0.0 {
                    events.push(SimEvent::Died { id: tid });
                }
            }
        }
        trap.tick = (t + 1) % (trap.up + trap.down);
        world.entity_mut(e).insert(trap);
    }
}

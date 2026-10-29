//! Arena respawns. `SetRespawn` arms a configured monster: its setup (spawn point, facing, speed, stats, kit,
//! senses, spawn options, loot source) is kept, and `after` seconds after it dies a fresh copy spawns there through
//! the same commands the client sends for a placed monster. The corpse stays (lootable) as any other. A dev rule,
//! not the game's.

use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::ai::{AiSenses, MonsterAi, SpawnOpts};
use crate::command::{ActorKind, SimCommand, apply, spawn_actor};
use crate::melee::{Aim, Combatant, Health};
use crate::monsters::MonsterKit;
use crate::movement::MoveSpeed;
use crate::world_items::LootSource;
use crate::{SimId, SimIndex, TICK_RATE_HZ, TickCount};

/// A monster's setup, kept to spawn it again.
#[derive(Debug, Clone, PartialEq)]
pub struct RespawnSpec {
    /// Its spawn point (capsule centre, cm) and facing.
    pub home: Vec3,
    pub yaw: f32,
    pub speed: f32,
    pub combatant: Combatant,
    pub max_health: f64,
    pub kit: MonsterKit,
    pub senses: AiSenses,
    pub opts: SpawnOpts,
    pub loot: Option<i32>,
    /// Seconds from its death to the new one.
    pub after: f32,
}

/// An armed monster (removed once its death has queued the respawn).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Respawn(pub Box<RespawnSpec>);

/// Respawns waiting for their tick.
#[derive(Resource, Debug, Default)]
pub struct PendingRespawns(pub Vec<(u64, Box<RespawnSpec>)>);

/// `SetRespawn`: keeps the monster's setup as it is now (after its `SetCombatant`, `SetMonster`, `SetLoot`).
pub(crate) fn arm(world: &mut World, id: SimId, after: f32, opts: SpawnOpts) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    let (Some(health), Some(combatant), Some(kit), Some(ai)) = (world.get::<Health>(e), world.get::<Combatant>(e), world.get::<MonsterKit>(e), world.get::<MonsterAi>(e)) else { return };
    let spec = RespawnSpec {
        home: ai.home,
        yaw: world.get::<Aim>(e).map_or(0.0, |a| a.yaw),
        speed: world.get::<MoveSpeed>(e).map_or(0.0, |s| s.0),
        combatant: *combatant,
        max_health: health.max,
        kit: kit.clone(),
        senses: ai.senses,
        opts,
        loot: world.get::<LootSource>(e).map(|l| l.source_id),
        after,
    };
    world.entity_mut(e).insert(Respawn(Box::new(spec)));
}

/// Each tick: armed monsters found dead queue their respawn; due ones spawn (in queue order, so ids stay stable).
pub(crate) fn respawn_tick(world: &mut World) {
    let tick = world.resource::<TickCount>().0;
    let mut dead: Vec<(SimId, Entity)> = Vec::new();
    {
        let index = world.resource::<SimIndex>();
        for (id, e) in index.iter() {
            if world.get::<Respawn>(e).is_some() && world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) {
                dead.push((id, e));
            }
        }
    }
    for (_, e) in dead {
        if let Some(Respawn(spec)) = world.entity_mut(e).take::<Respawn>() {
            let due = tick + (spec.after.max(0.0) * TICK_RATE_HZ as f32).round() as u64;
            world.get_resource_or_insert_with(PendingRespawns::default).0.push((due, spec));
        }
    }
    let Some(mut pending) = world.get_resource_mut::<PendingRespawns>() else { return };
    if !pending.0.iter().any(|(due, _)| *due <= tick) {
        return;
    }
    let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut pending.0).into_iter().partition(|(due, _)| *due <= tick);
    pending.0 = later;
    for (_, spec) in now {
        spawn(world, *spec);
    }
}

fn spawn(world: &mut World, s: RespawnSpec) {
    let id = spawn_actor(world, ActorKind::Monster, s.home, s.speed);
    apply(world, SimCommand::SetCombatant { id, combatant: Box::new(s.combatant), max_health: s.max_health });
    apply(world, SimCommand::SetAim { id, yaw: s.yaw, pitch: 0.0 });
    apply(world, SimCommand::SetMonster { id, kit: Box::new(s.kit), senses: s.senses });
    apply(world, SimCommand::SetSpawnOpts { id, opts: s.opts.clone() });
    if let Some(source_id) = s.loot {
        apply(world, SimCommand::SetLoot { id, source_id });
    }
    apply(world, SimCommand::SetRespawn { id, after: s.after, opts: s.opts });
}

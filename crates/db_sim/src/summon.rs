//! Summons (CONFIRMED from the defaults of `GA_TGF_Monster_Summon_CommonBase` and `GA_TGF_M_Totem_Summon`): a
//! summon ability spawns its `SummonMonsters` entry `SummonDelayTime` into its cast, at its `RelativeLocation`
//! in the summoner's frame when a capsule fits there, else at a free point `MinDistanceToOwner..SummoningRadius`
//! away (`SearchingTimes` tries). At most `SummonLimit` are alive at once; each starts on the summoner's target
//! (its threat list is synced), and with `DestroyWhenSummonerDied` they die with it. The summon itself has no
//! loot source (CONFIRMED for the Moose Ghost).

use avian3d::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use bevy_math::{Vec2, Vec3};

use crate::ai::{AiSenses, MonsterAi};
use crate::command::{ActorKind, SimEvent, SimEvents};
use crate::melee::{Aim, Combatant, Health, Melee};
use crate::monsters::MonsterKit;
use crate::{SimId, SimIndex, SimRng};

/// What a summon ability spawns, with the summoned monster's kit and stats (built when the kit loads).
#[derive(Debug, Clone, PartialEq)]
pub struct Summon {
    /// Roster row of the summoned monster.
    pub row: &'static str,
    /// `RelativeLocation` (cm, the summoner's frame: x forward, y right).
    pub offset: Vec3,
    pub limit: u32,
    /// `SummoningRadius`, `MinDistanceToOwner` (cm) and `SearchingTimes`: the search when `offset` is blocked.
    pub radius: f32,
    pub min_distance: f32,
    pub tries: u32,
    /// `DestroyWhenSummonerDied`.
    pub die_with_owner: bool,
    /// `DestroyDelayTime`: how long after the summoner they go, s.
    pub die_delay: f32,
    pub minion: Box<Minion>,
    /// The further `SummonMonsters` entries (the Reaper's bow and two sword skeletons), spawned the same way in
    /// order while fewer than `limit` are alive: (roster row, `RelativeLocation`, its minion).
    pub rest: Vec<(&'static str, Vec3, Box<Minion>)>,
    /// Fixed spots (`GA_TGF_Monster_Summon_FixedTransform_Base`): the offsets are from the summoner's spawn point in
    /// world axes, not its frame, and every entry spawns.
    pub fixed: bool,
}

/// A summoned monster's kit and curve stats.
#[derive(Debug, Clone, PartialEq)]
pub struct Minion {
    pub kit: MonsterKit,
    pub combatant: Combatant,
    pub max_health: f64,
    pub senses: AiSenses,
    /// cm/s
    pub speed: f32,
}

/// A summoned monster: who summoned it and its roster row (the client draws it by that row).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Summoned {
    pub by: SimId,
    pub row: &'static str,
    pub die_with_owner: bool,
    /// Its summon's `DestroyDelayTime`, s.
    pub die_delay: f32,
    /// The tick its summoner was first found dead or gone.
    pub orphaned: Option<u64>,
}

/// The summon of monster `id`'s attack `attack` (its own, else its kit's), at the start of the attack's window.
pub(crate) fn summon(world: &mut World, id: SimId, e: Entity, attack: u8) {
    let Some(s) = world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(attack)).and_then(|a| a.own_summon.clone()).or_else(|| k.summon.clone())) else { return };
    let entries = std::iter::once((s.row, s.offset, s.minion.clone())).chain(s.rest.iter().cloned());
    for (row, offset, minion) in entries {
        let alive = world.query::<(&Summoned, &Health)>().iter(world).filter(|(m, h)| m.by == id && h.current > 0.0).count() as u32;
        if alive >= s.limit {
            return;
        }
        spawn_one(world, id, e, &s, row, offset, &minion);
    }
}

/// One summoned monster at `offset` in the summoner's frame, or the first free point of the search.
fn spawn_one(world: &mut World, id: SimId, e: Entity, s: &Summon, row: &'static str, offset: Vec3, m: &Minion) {
    let (Some(at), Some(aim)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied()) else { return };
    let forward = aim.forward_2d();
    let right = Vec2::new(-forward.y, forward.x);
    // A fixed spot is from its spawn point (HYPOTHESIS: `SpawnLocationRelative` 0, its home is the point).
    let home = world.get::<MonsterAi>(e).map(|a| a.home);
    // The Shadow Dragon flies its script from the spot itself.
    if let (Some(spec), Some(home)) = (m.kit.dragon.clone(), home) {
        let spot = home + offset;
        let mid = crate::command::spawn_actor(world, ActorKind::Monster, spot, m.speed);
        let Some(me) = world.resource::<SimIndex>().get(mid) else { return };
        let dragon = crate::shadow_dragon::ShadowDragon::new(spec, home, crate::shadow_dragon::Pose { at: spot, yaw: 0.0 }, m.speed);
        world.entity_mut(me).insert((m.combatant, Health { current: m.max_health, max: m.max_health }, Melee::default(), Aim::default(), m.kit.clone(), dragon, Summoned { by: id, row, die_with_owner: s.die_with_owner, die_delay: s.die_delay, orphaned: None }));
        world.entity_mut(me).remove::<crate::movement::MoveInput>();
        return;
    }
    let (first, at) = match home {
        Some(home) if s.fixed => (home + offset, home + offset),
        _ => (at + (forward * offset.x + right * offset.y).extend(offset.z), at),
    };
    let mut state: SystemState<SpatialQuery> = SystemState::new(world);
    let mut candidates = vec![first];
    for _ in 0..s.tries {
        let mut rng = world.resource_mut::<SimRng>();
        let (angle, t) = (rng.unit_f32() * std::f32::consts::TAU, rng.unit_f32());
        candidates.push(at + (Vec2::from_angle(angle) * (s.min_distance + (s.radius - s.min_distance).max(0.0) * t)).extend(0.0));
    }
    let collider = crate::movement::character_collider();
    let spot = {
        let spatial = state.get(world).expect("spatial query");
        candidates.into_iter().find(|p| spatial.shape_intersections(&collider, *p, bevy_math::Quat::IDENTITY, &crate::layers::walk()).is_empty())
    };
    let Some(spot) = spot else { return };
    let target = world.get::<MonsterAi>(e).and_then(|a| a.target);
    let mid = crate::command::spawn_actor(world, ActorKind::Monster, spot, m.speed);
    let Some(me) = world.resource::<SimIndex>().get(mid) else { return };
    let mut ai = MonsterAi::new(spot, m.senses);
    ai.apply_spawn(&m.kit, crate::ai::SpawnOpts::default());
    // The summoner's threat list is synced to it (`BPFL_TGF_Summon_Base`): it starts on the summoner's target.
    ai.provoked_by = target;
    world.entity_mut(me).insert((m.combatant, Health { current: m.max_health, max: m.max_health }, Melee::default(), Aim { yaw: aim.yaw, pitch: 0.0 }, m.kit.clone(), ai, Summoned { by: id, row, die_with_owner: s.die_with_owner, die_delay: s.die_delay, orphaned: None }));
}

/// `DestroyWhenSummonerDied`: a summon dies `DestroyDelayTime` after its summoner is dead or gone. After combat each
/// tick.
pub(crate) fn summon_tick(world: &mut World) {
    let now = world.resource::<crate::TickCount>().0;
    let minions: Vec<(Entity, SimId, SimId)> = world.query::<(Entity, &SimId, &Summoned, &Health)>().iter(world).filter(|(_, _, s, h)| s.die_with_owner && h.current > 0.0).map(|(e, id, s, _)| (e, *id, s.by)).collect();
    for (e, id, by) in minions {
        let owner_alive = world.resource::<SimIndex>().get(by).and_then(|o| world.get::<Health>(o)).is_some_and(|h| h.current > 0.0);
        if owner_alive {
            continue;
        }
        let Some(mut s) = world.get_mut::<Summoned>(e) else { continue };
        let since = *s.orphaned.get_or_insert(now);
        if ((now - since) as f32) * crate::DT < s.die_delay {
            continue;
        }
        if let Some(mut h) = world.get_mut::<Health>(e) {
            h.current = 0.0;
        }
        world.resource_mut::<SimEvents>().push(SimEvent::Died { id });
    }
}

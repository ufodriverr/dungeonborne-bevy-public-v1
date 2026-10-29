//! Projectiles: straight, homing and thrown shots that are sim actors (`ActorKind::Projectile`). Each
//! tick a projectile sweeps from its position along its velocity; level boxes stop it, and living
//! combatants (not its owner) along the sweep are hit in order of distance. Options: pierce (pass
//! through more targets), chain (bounce to the nearest other target), pull (drag the target towards
//! the owner).
//!
//! CONFIRMED where a caller passes decoded data (e.g. the Wizard's `WizardQSpeedCurve`, keys
//! 400 → 600 → 800 cm/s at 0/1/2 s). HYPOTHESIS: everything the blocked abilities would set (speeds,
//! lifetimes, turn rates, sizes for the other shots), the linear reading of curve keys, and hitting the
//! capsule anywhere along its height.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use bevy_math::{Dir3, Vec3};

use crate::buffs::{Buff, BuffKind};
use crate::combat::{AttackStats, AttackTerm, Element, HitZone, resolve_hit};
use crate::command::{ActorKind, Kind, LevelBox};
use crate::id::{SimIdAllocator, SimIndex};
use crate::melee::{Aim, Combatant, Health, Melee, blocked_damage};
use crate::movement::{CAPSULE_HEIGHT, CAPSULE_RADIUS};
use crate::skills::{HitSource, SkillKind};
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng, SpatialQuery};

/// How a projectile flies and what it does on a hit.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileSpec {
    /// Speed keys (seconds since launch, cm/s), read linearly; one key = constant speed.
    pub speed: Vec<(f32, f32)>,
    /// Seconds before it vanishes.
    pub lifetime: f32,
    /// Collision radius, cm.
    pub radius: f32,
    /// Homing turn rate towards its target, degrees per second (0 = flies straight).
    pub turn_rate: f32,
    /// Downward acceleration, cm/s² (thrown shots; 0 = no drop).
    pub gravity: f32,
    /// The shooter's attack stats at launch, the damage factor and element.
    pub attack: AttackStats,
    pub factor: f64,
    pub element: Element,
    /// How its hits are reported.
    pub by: HitSource,
    /// Extra targets it may pass through after the first.
    pub pierce: u8,
    /// Times it may bounce to the nearest other target within `chain_range` (cm).
    pub chain: u8,
    pub chain_range: f32,
    /// Distance (cm) a hit target is pulled towards the owner, over `PULL_SECONDS`.
    pub pull: f32,
    /// A buff put on each target it hits, for this many seconds (e.g. the Rogue's 70% slow).
    pub on_hit: Option<(BuffKind, f32)>,
    /// Burst radius, cm: on touching a target, a wall or at the end of its flight it hits every
    /// combatant within this distance instead of one target (Pyroblast stage 2, 0 = no burst).
    pub burst: f32,
    /// Each target of a burst is pushed this far (cm) straight away from it over this many seconds (Pyroblast's
    /// large fireball: `Hitback Distance` 150 over 0.15 s).
    pub knockback: Option<(f32, f32)>,
    /// A fuse: it touches nothing and bursts only when its lifetime ends, as the engine's basic
    /// cylinder (radius = half height = `burst`) centred on it: the static-collision projectiles
    /// (`B_TGF_StaticCollision_ProjectileBase`, the Beholder's pillars of fire).
    pub fuse: bool,
    /// A raised guard doesn't reduce its hits (`bIgnoreGuard` on the projectile's class: the Shadow King's bullet).
    pub ignore_guard: bool,
}

impl ProjectileSpec {
    /// Speed after `t` seconds (linear between keys, held after the last).
    pub fn speed_at(&self, t: f32) -> f32 {
        let k = &self.speed;
        match k.iter().position(|p| p.0 > t) {
            Some(0) => k[0].1,
            Some(i) => {
                let (a, b) = (k[i - 1], k[i]);
                a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0)
            }
            None => k.last().map_or(0.0, |p| p.1),
        }
    }
}

/// How long a pull lasts, seconds (HYPOTHESIS).
pub const PULL_SECONDS: f32 = 0.3;

/// A flying projectile.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Projectile {
    pub owner: SimId,
    pub spec: Box<ProjectileSpec>,
    /// Unit flight direction (without the drop).
    pub dir: Vec3,
    /// Downward speed gained so far (thrown shots), cm/s.
    pub fall: f32,
    pub target: Option<SimId>,
    pub age: u32,
    /// Targets already hit (never hit twice).
    pub hit: Vec<SimId>,
    pub pierced: u8,
    pub chained: u8,
    /// Where it was launched, and a damage rate added by the distance from there (the crossbow's
    /// `DamageRateCurve`, empty: none).
    pub from: Vec3,
    pub falloff: Vec<(f32, f32)>,
    /// Burn stacks its hit adds (a crossbow bolt's `Add Burning Buff Arg`).
    pub burn: Option<crate::staffs::BurnSpec>,
    /// A thrown flask (its item): where it stops, [`crate::throwables::landed`] instead of its burst.
    pub flask: Option<i32>,
}

/// Launches a projectile from `pos` along `dir` (homing on `target` if its spec turns).
pub(crate) fn spawn(world: &mut World, owner: SimId, spec: ProjectileSpec, pos: Vec3, dir: Vec3, target: Option<SimId>) {
    launch(world, owner, spec, pos, dir, target, Vec::new(), None);
}

/// [`spawn`] for a thrown flask.
pub(crate) fn spawn_flask(world: &mut World, owner: SimId, spec: ProjectileSpec, pos: Vec3, dir: Vec3, item_id: i32) {
    launch(world, owner, spec, pos, dir, None, Vec::new(), None);
    // (The newest projectile: `launch` gave it the last id.)
    let last = world.resource::<SimIndex>().iter().filter(|(_, e)| world.get::<Projectile>(*e).is_some_and(|p| p.owner == owner)).max_by_key(|(id, _)| *id);
    if let Some((_, e)) = last
        && let Some(mut p) = world.get_mut::<Projectile>(e)
    {
        p.flask = Some(item_id);
    }
}

/// [`spawn`] with a damage rate by distance (the crossbow's bolts).
pub(crate) fn spawn_with_falloff(world: &mut World, owner: SimId, spec: ProjectileSpec, pos: Vec3, dir: Vec3, falloff: Vec<(f32, f32)>, burn: Option<crate::staffs::BurnSpec>) {
    launch(world, owner, spec, pos, dir, None, falloff, burn);
}

#[allow(clippy::too_many_arguments)]
fn launch(world: &mut World, owner: SimId, spec: ProjectileSpec, pos: Vec3, dir: Vec3, target: Option<SimId>, falloff: Vec<(f32, f32)>, burn: Option<crate::staffs::BurnSpec>) {
    let Ok(dir) = Dir3::new(dir) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "projectile without a direction" });
        return;
    };
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let p = Projectile { owner, spec: Box::new(spec), dir: *dir, fall: 0.0, target, age: 0, hit: Vec::new(), pierced: 0, chained: 0, from: pos, falloff, burn, flask: None };
    let e = world.spawn((id, Kind(ActorKind::Projectile), Position(pos), p)).id();
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Projectile });
}

/// Monsters' shots don't hurt other monsters (HYPOTHESIS: no friendly fire among monsters).
fn allied(world: &World, owner: SimId, te: Entity) -> bool {
    let monster = |e: Entity| world.get::<Kind>(e).is_some_and(|k| k.0 == ActorKind::Monster);
    monster(te) && world.resource::<SimIndex>().get(owner).is_some_and(monster)
}

/// Turns unit vector `from` towards `to` by at most `max` radians.
fn turn_towards(from: Vec3, to: Vec3, max: f32) -> Vec3 {
    let angle = from.angle_between(to);
    if angle <= max || angle < 1e-6 {
        return to;
    }
    let axis = from.cross(to);
    let axis = if axis.length_squared() < 1e-12 { from.any_orthonormal_vector() } else { axis.normalize() };
    bevy_math::Quat::from_axis_angle(axis, max) * from
}

/// Closest distance between segments `p0..p1` and `q0..q1`, with the parameter along the first.
pub(crate) fn segment_distance(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> (f32, f32) {
    let (d1, d2, r) = (p1 - p0, q1 - q0, p0 - q0);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (mut s, mut t);
    if a < 1e-9 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        let b = d1.dot(d2);
        let denom = a * e - b * b;
        s = if denom > 1e-9 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
        t = (b * s + f) / e;
        if t < 0.0 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else if t > 1.0 {
            t = 1.0;
            s = ((b - c) / a).clamp(0.0, 1.0);
        }
    }
    ((p0 + d1 * s - (q0 + d2 * t)).length(), s)
}

/// Moves every projectile one tick in id order, resolving wall and actor hits.
pub(crate) fn projectiles_tick(world: &mut World) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    let shots: Vec<(SimId, Entity)> = ids.iter().copied().filter(|(_, e)| world.get::<Projectile>(*e).is_some()).collect();
    if shots.is_empty() {
        return;
    }
    // Phase 1 (read-only): each projectile's sweep this tick and where a wall stops it.
    let mut sweeps = Vec::new();
    {
        let mut state: SystemState<(SpatialQuery, Query<(), With<LevelBox>>)> = SystemState::new(world);
        // SimPlugin always adds the physics (and its spatial query pipeline).
        let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
        for &(id, e) in &shots {
            let mut p = world.get::<Projectile>(e).cloned().expect("filtered");
            let from = world.get::<Position>(e).map_or(Vec3::ZERO, |q| q.0);
            if p.spec.turn_rate > 0.0
                && let Some(to) = p.target.and_then(|t| world.resource::<SimIndex>().get(t)).and_then(|te| world.get::<Position>(te)).map(|q| q.0)
                && let Ok(want) = Dir3::new(to - from)
            {
                p.dir = turn_towards(p.dir, *want, p.spec.turn_rate.to_radians() * DT).normalize();
            }
            let v = p.dir * p.spec.speed_at(p.age as f32 * DT) - Vec3::Z * p.fall;
            p.fall += p.spec.gravity * DT;
            let to = from + v * DT;
            let wall = Dir3::new(to - from).ok().and_then(|d| spatial.cast_ray_predicate(from, d, (to - from).length(), true, &crate::layers::shots(), &|b| boxes.contains(b))).map(|hit| hit.distance / (to - from).length().max(1e-6));
            sweeps.push((id, e, p, from, to, wall));
        }
    }
    // Phase 2: actor hits along each sweep, in id order.
    for (id, e, mut p, from, to, wall) in sweeps {
        let reach = wall.unwrap_or(1.0);
        let half = CAPSULE_HEIGHT * 0.5 - CAPSULE_RADIUS;
        let mut along: Vec<(f32, SimId, Entity)> = ids
            .iter()
            .filter(|(tid, te)| *tid != p.owner && *tid != id && !p.hit.contains(tid) && world.get::<Combatant>(*te).is_some() && world.get::<Health>(*te).is_some_and(|h| h.current > 0.0) && !allied(world, p.owner, *te))
            .filter_map(|&(tid, te)| {
                let c = world.get::<Position>(te)?.0;
                let (dist, s) = segment_distance(from, to, c - Vec3::Z * half, c + Vec3::Z * half);
                (dist <= CAPSULE_RADIUS + p.spec.radius && s <= reach).then_some((s, tid, te))
            })
            .collect();
        along.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if p.spec.burst > 0.0 {
            p.age += 1;
            let expired = p.age >= (p.spec.lifetime / DT).round() as u32;
            let at = match (along.first(), wall) {
                _ if p.spec.fuse => expired.then_some(from),
                (Some(&(s, _, _)), _) => Some(from + (to - from) * s),
                (None, Some(s)) => Some(from + (to - from) * s),
                (None, None) if expired => Some(to),
                _ => None,
            };
            match at {
                // A flask out of time in the air is just gone.
                Some(_) if p.flask.is_some() && expired && along.is_empty() && wall.is_none() => despawn(world, id, e),
                Some(at) if p.flask.is_some() => {
                    despawn(world, id, e);
                    crate::throwables::landed(world, p.owner, p.flask.unwrap_or_default(), at, p.age);
                }
                Some(at) => {
                    burst(world, &ids, &p, id, at);
                    despawn(world, id, e);
                }
                None => {
                    if let Some(mut q) = world.get_mut::<Position>(e) {
                        q.0 = to;
                    }
                    world.entity_mut(e).insert(p);
                }
            }
            continue;
        }
        let mut end = None;
        for (s, tid, te) in along {
            let at = from + (to - from) * s;
            hit_target(world, &p, at, tid, te);
            p.hit.push(tid);
            if p.pierced < p.spec.pierce {
                p.pierced += 1;
                continue;
            }
            // Out of pierces: bounce on, or stop here.
            if p.chained < p.spec.chain
                && let Some((next, npos)) = nearest_other(world, &ids, &p, id, at)
            {
                p.chained += 1;
                p.target = Some(next);
                p.dir = (npos - at).normalize_or(p.dir);
                p.pierced = 0;
                if let Some(mut q) = world.get_mut::<Position>(e) {
                    q.0 = at;
                }
                end = Some(false);
            } else {
                end = Some(true);
            }
            break;
        }
        p.age += 1;
        let expired = p.age >= (p.spec.lifetime / DT).round() as u32;
        match end {
            Some(true) => despawn(world, id, e),
            Some(false) => {
                world.entity_mut(e).insert(p);
            }
            None if wall.is_some() || expired => despawn(world, id, e),
            None => {
                if let Some(mut q) = world.get_mut::<Position>(e) {
                    q.0 = to;
                }
                world.entity_mut(e).insert(p);
            }
        }
    }
}

/// A burst at `at`: every living combatant (not the owner) whose capsule centre is within the burst
/// radius plus the capsule radius, in id order.
fn burst(world: &mut World, ids: &[(SimId, Entity)], p: &Projectile, me: SimId, at: Vec3) {
    let hits: Vec<(SimId, Entity)> = ids
        .iter()
        .copied()
        .filter(|(tid, te)| *tid != p.owner && *tid != me && world.get::<Combatant>(*te).is_some() && world.get::<Health>(*te).is_some_and(|h| h.current > 0.0) && !allied(world, p.owner, *te))
        .filter(|(_, te)| {
            world.get::<Position>(*te).is_some_and(|q| match p.spec.fuse {
                // The cylinder overlapping the capsule.
                true => q.0.truncate().distance(at.truncate()) <= p.spec.burst + CAPSULE_RADIUS && (q.0.z - at.z).abs() <= p.spec.burst + CAPSULE_HEIGHT * 0.5,
                false => q.0.distance(at) <= p.spec.burst + CAPSULE_RADIUS,
            })
        })
        .collect();
    world.resource_mut::<SimEvents>().push(SimEvent::Burst { owner: p.owner, at, radius: p.spec.burst, by: p.spec.by });
    for (tid, te) in hits {
        hit_target(world, p, at, tid, te);
        if let Some((dist, secs)) = p.spec.knockback
            && world.get::<Health>(te).is_some_and(|h| h.current > 0.0)
            && let Some(q) = world.get::<Position>(te).map(|q| q.0)
        {
            let skill = if let HitSource::Skill(k) = p.spec.by { k } else { SkillKind::Charge };
            // Whole ticks, the speed set so they cover the distance exactly (0.15 s is 4.5 ticks at 30 Hz).
            let n = ((secs / DT).round() as u32).max(1);
            let velocity = (q - at).truncate().normalize_or_zero() * dist / (n as f32 * DT);
            crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Pull(velocity), skill, ticks_left: n + 1, age: 0 });
        }
    }
}

fn despawn(world: &mut World, id: SimId, e: Entity) {
    world.resource_mut::<SimIndex>().remove(id);
    world.despawn(e);
    world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id });
}

/// The nearest living combatant within the chain range that the projectile has not hit.
fn nearest_other(world: &World, ids: &[(SimId, Entity)], p: &Projectile, me: SimId, at: Vec3) -> Option<(SimId, Vec3)> {
    ids.iter()
        .filter(|(tid, te)| *tid != p.owner && *tid != me && !p.hit.contains(tid) && world.get::<Combatant>(*te).is_some() && world.get::<Health>(*te).is_some_and(|h| h.current > 0.0))
        .filter_map(|&(tid, te)| world.get::<Position>(te).map(|q| (tid, q.0)))
        .filter(|(_, q)| q.distance(at) <= p.spec.chain_range)
        .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)).then(a.0.cmp(&b.0)))
}

/// One projectile hit: `resolve_hit` with the launch stats against the target's (buffed) defence,
/// reduced by a block facing the projectile; life steal for the owner; the pull if any.
fn hit_target(world: &mut World, p: &Projectile, at: Vec3, tid: SimId, te: Entity) {
    let (Some(defender), Some(to)) = (world.get::<Combatant>(te).copied(), world.get::<Position>(te).map(|q| q.0)) else { return };
    let defence = crate::buffs::defence_of(world, te, defender.defence);
    // The head: the hit's height above the target's feet from its body's head up (a monster's model, else a
    // human's; HYPOTHESIS: every body fills the one capsule, so a shot over a small monster's head still counts).
    let head = match world.get::<crate::monsters::MonsterKit>(te) {
        Some(k) => crate::monsters::head_of(&k.name),
        None => Some(crate::overrides::HUMAN_HEAD_CM),
    };
    let zone = if head.is_some_and(|h| at.z - (to.z - CAPSULE_HEIGHT * 0.5) >= h) { HitZone::Head } else { HitZone::Body };
    let hit = resolve_hit(&p.spec.attack, &defence, p.spec.element, AttackTerm::Weapon, zone, &mut world.resource_mut::<SimRng>());
    let mut damage = hit.damage * p.spec.factor;
    if !p.falloff.is_empty() {
        damage *= (1.0 + f64::from(crate::crossbow::falloff_at(&p.falloff, p.from.distance(at)))).max(0.0);
    }
    let blocking = !p.spec.ignore_guard && world.get::<Melee>(te).is_some_and(Melee::blocking);
    let mut guarded = false;
    if blocking {
        let taim = world.get::<Aim>(te).copied().unwrap_or_default();
        let angles = world.get::<Melee>(te).and_then(|m| m.guard_angles);
        damage = blocked_damage(damage, &defender, &taim, angles, to, at);
        guarded = crate::melee::guard_faces(&taim, angles, to, at);
        if guarded && crate::weapons::guarded(world, te) {
            damage = 0.0;
        }
    }
    let owner = world.resource::<SimIndex>().get(p.owner);
    // A faced block of a projectile is a guard too ('s guard passives).
    if guarded && let Some(oe) = owner {
        crate::affixes::on_guarded(world, tid, te, p.owner, oe);
    }
    let damage = crate::buffs::incoming_damage(world, te, damage, owner);
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    {
        let mut events = world.resource_mut::<SimEvents>();
        events.push(SimEvent::Damaged { source: p.owner, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by: p.spec.by });
        if zone == HitZone::Head {
            events.push(SimEvent::HeadHit { source: p.owner, target: tid });
        }
        if left <= 0.0 {
            events.push(SimEvent::Died { id: tid });
        }
    }
    if let Some(oe) = world.resource::<SimIndex>().get(p.owner) {
        crate::buffs::after_damage_dealt(world, p.owner, oe, damage, blocking);
        // A weapon's shot (a bolt, a staff spell): the rolled extra damage.
        if damage > 0.0 && matches!(p.spec.by, HitSource::Bolt | HitSource::Staff(_)) {
            crate::gear_triggers::on_weapon_hit(world, (p.owner, oe), (tid, te));
        }
        if let Some((kind, secs)) = p.spec.on_hit
            && left > 0.0
        {
            let skill = if let HitSource::Skill(k) = p.spec.by { k } else { SkillKind::Charge };
            crate::buffs::add_buff(world, te, Buff { kind, skill, ticks_left: (secs / DT).round() as u32 + 1, age: 0 });
        }
        if let Some(burn) = p.burn
            && left > 0.0
        {
            crate::buffs::add_burn(world, te, p.owner, burn);
        }
        if left > 0.0 {
            crate::class_skills::projectile_hit(world, p.owner, oe, p.spec.by, te);
        }
        if p.spec.pull > 0.0 && left > 0.0 {
            let owner_pos = world.get::<Position>(oe).map_or(to, |q| q.0);
            let gap = (owner_pos - to).truncate();
            let dist = p.spec.pull.min((gap.length() - 2.0 * CAPSULE_RADIUS).max(0.0));
            let skill = if let HitSource::Skill(k) = p.spec.by { k } else { SkillKind::Charge };
            let velocity = gap.normalize_or_zero() * dist / PULL_SECONDS;
            // One tick more: the buff ticks down once before the target's next move.
            crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Pull(velocity), skill, ticks_left: (PULL_SECONDS / DT).round() as u32 + 1, age: 0 });
        }
    }
}

/// The Wizard's Pyroblast speed keys (s, cm/s) from `WizardQSpeedCurve` (decoded; cubic in the game,
/// read linearly here).
pub fn load_wizard_q_speed() -> anyhow::Result<Vec<(f32, f32)>> {
    use anyhow::Context;
    let path = Some(db_formats::data_path("json/assets/Dungeonborne/Content/TGF/Character/Hero/Wizard/GameAbility/WizardQSpeedCurve.json")).filter(|p| p.is_file()).context("WizardQSpeedCurve.json not decoded")?;
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let keys = v[0]["Properties"]["FloatCurve"]["Keys"].as_array().context("no FloatCurve keys")?;
    keys.iter().map(|k| Ok((k["Time"].as_f64().context("Time")? as f32, k["Value"].as_f64().context("Value")? as f32))).collect()
}

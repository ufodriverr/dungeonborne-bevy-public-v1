//! Melee combat: light/heavy attacks and block as timed phases, hits resolved with
//! [`crate::combat::resolve_hit`].
//!
//! CONFIRMED: the damage formula structure and the stats it reads (class sheets, item affixes,
//! `MonsterAttributeCurve`). HYPOTHESIS: phase timings, reach, cone and the heavy multiplier
//! ([`crate::overrides::melee`]), body hits only (head hits need the blocked weapon trace data).

use bevy_ecs::prelude::*;
use bevy_math::{Vec2, Vec3};

use crate::combat::{AttackStats, AttackTerm, DefenceStats, Element, HitZone, resolve_hit};
use crate::monsters::{MonsterAttack, MonsterKit};
use crate::skills::HitSource;
use crate::id::SimIndex;
use crate::movement::CAPSULE_RADIUS;
use crate::overrides::melee as tune;
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng};

/// Hit points.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub current: f64,
    pub max: f64,
}

/// Everything a hit needs from either side.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Combatant {
    pub attack: AttackStats,
    pub defence: DefenceStats,
    /// Damage reduction while blocking (`defend_guarded_damage_reduce_rate` from the weapon/shield).
    pub guard_reduce: f64,
}

/// Where the actor is looking (radians; yaw 0 = +X, positive towards +Y; pitch up positive).
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Aim {
    pub yaw: f32,
    pub pitch: f32,
}

impl Aim {
    pub fn forward_2d(&self) -> Vec2 {
        Vec2::from_angle(self.yaw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackKind {
    Light,
    Heavy,
}

impl AttackKind {
    /// (wind-up, active, recovery) in ticks.
    pub fn ticks(self) -> (u32, u32, u32) {
        let (w, a, r) = match self {
            AttackKind::Light => tune::LIGHT_TIMING,
            AttackKind::Heavy => tune::HEAVY_TIMING,
        };
        let t = |s: f32| (s / DT).round() as u32;
        (t(w), t(a).max(1), t(r))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwingPhase {
    Windup,
    Active,
    Recovery,
}

/// One attack in progress: hit windows and length in ticks (player attacks from the overrides,
/// monster attacks from their montages).
#[derive(Debug, Clone, PartialEq)]
pub struct Swing {
    pub by: HitSource,
    /// Active windows [start, end) in ticks since the swing started.
    pub windows: Vec<(u32, u32)>,
    pub length: u32,
    /// Ticks since the swing started.
    pub tick: u32,
    /// Targets already hit by this swing (each at most once).
    pub hit: Vec<SimId>,
    /// A monster swing's root motion per tick, (forward, left) cm ([`MonsterAttack::root`]), and the
    /// facing it is applied along (the actor's aim, updated every tick).
    pub root: Vec<Vec2>,
    pub facing: Vec2,
    /// Damage factor on a hit (a heavy attack's, a staff combo's damage effect).
    pub factor: f64,
    /// A monster shield's guard window [start, end) in ticks.
    pub guard: Option<(u32, u32)>,
    /// Gameplay tags owned in windows [start, end) of ticks (a monster montage's tag states): the tag's id with its
    /// parents' ([`crate::buffs::tag_with_parents`]).
    pub tags: Vec<(u32, u32, Vec<u32>)>,
    /// A weapon swing's damage factor per window (one window per hand, each with its own damage effect); each
    /// window then hits its targets once. Empty: `factor` for all, each target once per swing.
    pub window_factors: Vec<f64>,
    /// Its hit value: at or above the `HurtInterruptBound` its hits aren't guarded (a guard break).
    pub hit_value: f32,
}

impl Swing {
    pub fn player(kind: AttackKind) -> Self {
        let (w, a, r) = kind.ticks();
        let factor = if kind == AttackKind::Heavy { tune::HEAVY_DAMAGE_FACTOR } else { 1.0 };
        Swing { hit_value: 0.0, by: HitSource::Swing(kind), windows: vec![(w, w + a)], length: w + a + r, tick: 0, hit: Vec::new(), root: Vec::new(), facing: Vec2::X, factor, guard: None, tags: Vec::new(), window_factors: Vec::new() }
    }

    /// A monster attack from its montage (`index` into the monster's attack list).
    pub fn monster(index: u8, attack: &MonsterAttack) -> Self {
        let windows = attack.windows.iter().map(|&(a, b)| (secs_to_ticks(a), secs_to_ticks(b).max(secs_to_ticks(a) + 1))).collect();
        let guard = attack.guard.map(|g| (secs_to_ticks(g.from), secs_to_ticks(g.until)));
        // An explosion's own factor, else its hit effect's.
        let factor = attack.burst.map_or(attack.factor, |b| b.factor);
        let tags = attack.tags.iter().map(|(a, b, t)| (secs_to_ticks(*a), secs_to_ticks(*b), crate::buffs::tag_with_parents(t))).collect();
        Swing { hit_value: 0.0, by: HitSource::Monster(index), windows, length: secs_to_ticks(attack.length).max(1), tick: 0, hit: Vec::new(), root: attack.root.clone(), facing: Vec2::X, factor, guard, tags, window_factors: Vec::new() }
    }

    /// The player attack kind, if this is a player swing.
    pub fn kind(&self) -> Option<AttackKind> {
        match self.by {
            HitSource::Swing(k) => Some(k),
            _ => None,
        }
    }

    pub fn phase(&self) -> SwingPhase {
        if self.windows.iter().any(|&(a, b)| (a..b).contains(&self.tick)) {
            SwingPhase::Active
        } else if self.windows.first().is_none_or(|&(a, _)| self.tick < a) {
            SwingPhase::Windup
        } else {
            SwingPhase::Recovery
        }
    }

    /// 0..1 over the whole swing (for presentation).
    pub fn progress(&self) -> f32 {
        self.tick as f32 / self.length.max(1) as f32
    }
}

fn secs_to_ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

/// Melee state of an actor.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Melee {
    pub swing: Option<Swing>,
    /// Blocking wanted: the input, or a guard kept up for its shortest time.
    pub block_held: bool,
    /// The block input itself (`SimCommand::SetBlock`).
    pub block_input: bool,
    /// The held weapon's guarded degrees left and right of the facing (`None`: the HYPOTHESIS half-angle).
    pub guard_angles: Option<(f32, f32)>,
    /// A charge attack's input was let go (`SimCommand::ReleaseCharge`).
    pub charge_release: bool,
    /// Attack requested by input, started on the next tick the actor is free.
    pub queued: Option<AttackKind>,
    /// Monster attack (index into its `MonsterKit`) requested by the AI.
    pub queued_montage: Option<u8>,
}

impl Melee {
    /// Blocking: held and not swinging, or inside a monster shield's guard window.
    pub fn blocking(&self) -> bool {
        match &self.swing {
            None => self.block_held,
            Some(s) => s.guard.is_some_and(|(a, b)| s.tick >= a && s.tick < b),
        }
    }

    /// Movement speed factor (attacking and blocking slow the actor; HYPOTHESIS values). The velocity
    /// (cm/s) a swing's root motion moves the actor at this tick: the displacement of the tick
    /// [`melee_tick`] just ran, along the swing's facing.
    pub fn root_velocity(&self) -> Option<Vec2> {
        let s = self.swing.as_ref()?;
        let d = *s.root.get(s.tick.checked_sub(1)? as usize)?;
        // Left of a facing in the sim's (Unreal's) axes: x forward, y right.
        let left = Vec2::new(s.facing.y, -s.facing.x);
        Some((s.facing * d.x + left * d.y) / crate::DT)
    }

    /// Whether the running swing owns the tag `id` now (or a tag under it).
    pub fn swing_has_tag(&self, id: u32) -> bool {
        self.swing.as_ref().is_some_and(|s| s.tags.iter().any(|(a, b, ids)| s.tick >= *a && s.tick < *b && ids.contains(&id)))
    }

    pub fn move_factor(&self) -> f32 {
        if self.swing.is_some() {
            tune::ATTACK_MOVE_FACTOR
        } else if self.blocking() {
            tune::BLOCK_MOVE_FACTOR
        } else {
            1.0
        }
    }
}

/// Whether `target` (capsule centre) is inside the swing of an attacker at `from` aiming `aim`.
pub fn in_swing(from: Vec3, aim: &Aim, target: Vec3) -> bool {
    in_swing_scaled(from, aim, target, 1.0)
}

/// [`in_swing`] with the reach scaled (bosses).
pub fn in_swing_scaled(from: Vec3, aim: &Aim, target: Vec3, scale: f32) -> bool {
    let d = (target - from).truncate();
    if (target.z - from.z).abs() > tune::VERTICAL_REACH * scale || d.length() - CAPSULE_RADIUS > tune::REACH * scale {
        return false;
    }
    let len = d.length();
    len < 1e-3 || aim.forward_2d().dot(d / len) >= tune::CONE_HALF_ANGLE_DEG.to_radians().cos()
}

/// Damage after the defender's block: a block faces the attacker when the attacker is inside the
/// defender's front arc.
pub fn blocked_damage(damage: f64, defender: &Combatant, defender_aim: &Aim, angles: Option<(f32, f32)>, defender_pos: Vec3, attacker_pos: Vec3) -> f64 {
    if guard_faces(defender_aim, angles, defender_pos, attacker_pos) { damage * (1.0 - defender.guard_reduce.clamp(0.0, 1.0)) } else { damage }
}

/// Whether a block faces the attacker (the guard succeeds).
pub fn guard_faces(defender_aim: &Aim, angles: Option<(f32, f32)>, defender_pos: Vec3, attacker_pos: Vec3) -> bool {
    let d = (attacker_pos - defender_pos).truncate();
    // Guarded `angles` (left, right) degrees off the facing; which side is left is HYPOTHESIS (+90 degrees of
    // yaw), as is the half-angle used without them.
    let (left, right) = angles.unwrap_or((tune::BLOCK_HALF_ANGLE_DEG, tune::BLOCK_HALF_ANGLE_DEG));
    d.length() < 1e-3 || {
        let (f, to) = (defender_aim.forward_2d(), d.normalize());
        let off = f.dot(to).clamp(-1.0, 1.0).acos().to_degrees();
        off <= if f.perp_dot(to) >= 0.0 { left } else { right }
    }
}

/// Advances swings in id order; during the active window each target in the swing is hit once.
pub(crate) fn melee_tick(world: &mut World) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for &(id, e) in &ids {
        let Some(mut melee) = world.get::<Melee>(e).cloned() else { continue };
        if world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) {
            // The dead do nothing.
            if melee != Melee::default()
                && let Some(mut m) = world.get_mut::<Melee>(e)
            {
                *m = Melee::default();
            }
            continue;
        }
        // Petrified, encased or channelling: no attacks, and a swing in progress stops.
        if world.get::<crate::buffs::Buffs>(e).is_some_and(|b| b.stunned() || b.rooted()) {
            if melee.swing.is_some() || melee.queued.is_some() || melee.queued_montage.is_some() {
                melee.swing = None;
                melee.queued = None;
                melee.queued_montage = None;
                if let Some(mut m) = world.get_mut::<Melee>(e) {
                    *m = melee;
                }
            }
            continue;
        }
        // A queued attack waits while the actor spins (Whirlwind).
        let spinning = world.get::<crate::skills::SkillKit>(e).is_some_and(|k| k.whirlwind.is_some());
        if melee.swing.is_none() && !spinning {
            if let Some(kind) = melee.queued.take() {
                // A staff in hand swings its combo; a sword or dagger (with what the other hand holds) its own on a
                // tap.
                let combo = crate::staffs::combo_swing(world, e).or_else(|| if kind == AttackKind::Light { crate::weapons::combo_swing(world, e) } else { None });
                melee.swing = Some(combo.unwrap_or_else(|| Swing::player(kind)));
                world.resource_mut::<SimEvents>().push(SimEvent::SwingStarted { id, kind });
            } else if let Some(index) = melee.queued_montage.take() {
                match world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(index))) {
                    Some(attack) => {
                        melee.swing = Some(Swing::monster(index, attack));
                        world.resource_mut::<SimEvents>().push(SimEvent::MonsterAttackStarted { id, index });
                    }
                    None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "no such monster attack" }),
                }
            }
        }
        // A montage chain: at its notify the attack may end and another start in its place.
        if let Some(swing) = melee.swing.as_ref()
            && let HitSource::Monster(i) = swing.by
            && let Some(chain) = world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).and_then(|a| a.chain.clone())
            && swing.tick == (chain.at / crate::DT).round() as u32
            && world.resource_mut::<crate::SimRng>().unit_f64() * 100.0 <= chain.percent
        {
            let total: f64 = chain.to.iter().map(|t| t.1).sum();
            let mut r = world.resource_mut::<crate::SimRng>().unit_f64() * total;
            let next = chain.to.iter().find(|t| {
                let hit = r <= t.1;
                r -= t.1;
                hit
            });
            melee.swing = None;
            let now = world.resource::<crate::TickCount>().0 as u32;
            if let Some(&(j, _)) = next
                && let Some(attack) = world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(j))).cloned()
                && world.get::<crate::ai::MonsterAi>(e).is_none_or(|ai| ai.cooldowns.get(usize::from(j)).is_none_or(|&ready| now >= ready))
            {
                melee.swing = Some(Swing::monster(j, &attack));
                world.resource_mut::<SimEvents>().push(SimEvent::MonsterAttackStarted { id, index: j });
                if attack.pick.cooldown > 0.0
                    && let Some(mut ai) = world.get_mut::<crate::ai::MonsterAi>(e)
                {
                    let j = usize::from(j);
                    if ai.cooldowns.len() <= j {
                        ai.cooldowns.resize(j + 1, 0);
                    }
                    ai.cooldowns[j] = now + (attack.pick.cooldown / crate::DT).round() as u32;
                }
            }
        }
        if let Some(swing) = melee.swing.as_mut() {
            let (shot, area, summons, effect, burst) = match swing.by {
                HitSource::Monster(i) => world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).map_or((None, None, false, false, None), |a| (a.shot, a.area, a.summon, a.effect.is_some(), a.burst)),
                _ => (None, None, false, false, None),
            };
            let cloud = match swing.by {
                HitSource::Monster(i) => world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).and_then(|a| a.cloud.as_deref().cloned()).map(|c| (i, c)),
                _ => None,
            };
            let opens = swing.windows.first().is_some_and(|w| w.0 == swing.tick);
            // Its montage's gameplay events as the swing reaches them.
            if let HitSource::Monster(i) = swing.by {
                let due: Vec<String> = world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).map(|a| a.events.iter().filter(|(t, _)| (t / crate::DT).round() as u32 == swing.tick).map(|(_, tag)| tag.clone()).collect()).unwrap_or_default();
                for tag in due {
                    crate::floor_collapse::gameplay_event(world, id, e, &tag);
                }
            }
            // A volley or multi-pass areas: the shots or passes of the windows that open now (two notifies may share a
            // tick).
            let (volley, passes): (Vec<crate::monsters::VolleyShot>, Vec<crate::monsters::AoePass>) = match swing.by {
                HitSource::Monster(i) => world
                    .get::<MonsterKit>(e)
                    .and_then(|k| k.attacks.get(usize::from(i)))
                    .map(|a| {
                        let open: Vec<usize> = swing.windows.iter().enumerate().filter(|(_, w)| w.0 == swing.tick).map(|(w, _)| w).collect();
                        (open.iter().filter_map(|&w| a.volley.get(w).copied()).collect(), open.iter().filter_map(|&w| a.aoe.get(w).cloned()).collect())
                    })
                    .unwrap_or_default(),
                _ => Default::default(),
            };
            let multi = match swing.by {
                HitSource::Monster(i) => world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).is_some_and(|a| !a.volley.is_empty() || !a.aoe.is_empty() || a.spray.is_some() || a.aura.is_some()),
                _ => false,
            };
            // An aura area is laid as its window opens.
            let aura = match swing.by {
                HitSource::Monster(i) if opens => world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).and_then(|a| a.aura.as_deref().cloned()),
                _ => None,
            };
            // A spray is thrown as its window opens.
            let spray = match swing.by {
                HitSource::Monster(i) if opens => world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(i))).and_then(|a| a.spray),
                _ => None,
            };
            match (shot, area) {
                _ if multi => {
                    for v in volley {
                        fire_volley(world, id, e, swing.by, v);
                    }
                    if let HitSource::Monster(i) = swing.by {
                        for p in passes {
                            crate::staffs::start_aoe_pass(world, id, e, i, p);
                        }
                    }
                    if let Some(s) = spray {
                        fire_spray(world, id, e, swing.by, s);
                    }
                    if let Some(a) = aura {
                        crate::aura_areas::spawn(world, id, e, a);
                    }
                }
                // An effect ability applies its effect then; it hits nothing.
                _ if effect => {
                    if opens && let HitSource::Monster(i) = swing.by {
                        crate::monster_effects::apply(world, e, i);
                    }
                }
                // An explosion hits around it then.
                _ if burst.is_some() => {
                    if opens && let Some(b) = burst {
                        burst_targets(world, id, e, swing, &ids, b);
                    }
                }
                // A spray's area is laid then; it hits nothing itself.
                _ if cloud.is_some() => {
                    if opens && let Some((i, c)) = cloud {
                        crate::staffs::spawn_monster_area(world, id, e, i, c);
                    }
                }
                // A summon spawns its monster then; it hits nothing itself.
                _ if summons => {
                    if opens && let HitSource::Monster(i) = swing.by {
                        crate::summon::summon(world, id, e, i);
                    }
                }
                // A ranged attack fires once, at the start of its window.
                (Some(shot), _) => {
                    if opens {
                        fire(world, id, e, swing.by, shot);
                    }
                }
                // An area attack puts its fuses down then.
                (None, Some(area)) => {
                    if opens {
                        spawn_area(world, id, e, swing.by, area);
                    }
                }
                (None, None) if swing.phase() == SwingPhase::Active => {
                    if let Some(i) = swing.windows.iter().position(|&(a, b)| (a..b).contains(&swing.tick))
                        && let Some(&f) = swing.window_factors.get(i)
                    {
                        // A hand's own window (HYPOTHESIS: each `TgfAttack` state traces anew).
                        if swing.windows[i].0 == swing.tick {
                            swing.hit.clear();
                        }
                        swing.factor = f;
                    }
                    let scale = world.get::<crate::monsters::MonsterKit>(e).map_or(1.0, |k| crate::monsters::reach_scale(&k.name));
                    hit_targets(world, id, e, swing, &ids, &|_, from, aim, to| in_swing_scaled(from, aim, to, scale));
                }
                (None, None) => {}
            }
            if let Some(aim) = world.get::<Aim>(e) {
                swing.facing = aim.forward_2d();
            }
            swing.tick += 1;
            if swing.tick >= swing.length {
                melee.swing = None;
            }
        }
        if let Some(mut m) = world.get_mut::<Melee>(e) {
            *m = melee;
        }
    }
}

/// A monster's shot: a projectile from its muzzle at its AI target's capsule centre (aimed up for
/// the drop), with the shooter's buffed attack stats; straight ahead without a target.
fn fire(world: &mut World, id: SimId, e: Entity, by: HitSource, shot: crate::monsters::Shot) {
    let (Some(from), Some(aim), Some(c)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied(), world.get::<Combatant>(e).copied()) else { return };
    // From the ability's `ForwardOffset` / `UpOffset` when it has them, else the muzzle.
    let muzzle = match shot.offset {
        Some((ahead, up)) => from + aim.forward_2d().extend(0.0) * ahead + Vec3::Z * up,
        None => from + Vec3::Z * crate::overrides::ranged::MUZZLE_HEIGHT + aim.forward_2d().extend(0.0) * (CAPSULE_RADIUS + 10.0),
    };
    let target = world.get::<crate::ai::MonsterAi>(e).and_then(|a| a.target);
    let to = target.and_then(|t| world.resource::<SimIndex>().get(t)).and_then(|te| world.get::<Position>(te)).map(|p| p.0);
    // `StopAim`'s `CheckFixing` (CONFIRMED bytecode): a target farther off the facing than `AimDegreeConfig` isn't
    // aimed at; the shot flies straight ahead (the archers' 1 degree, the wizard's 0).
    let to = to.filter(|t| shot.aim_degrees.is_none_or(|max| aim.forward_2d().angle_to((*t - from).truncate()).abs().to_degrees() <= max + 0.5));
    // A throw solves its launch velocity: at the target, else its range straight ahead (`GetTargetRange`).
    let toss = shot.toss.and_then(|s| crate::monsters::toss_velocity(muzzle, to.unwrap_or(from + aim.forward_2d().extend(0.0) * shot.range), s, shot.gravity));
    let dir = match (toss, to) {
        (Some(v), _) => v,
        (None, Some(to)) => {
            let flight = (to - muzzle).length() / shot.speed.max(1.0);
            to + Vec3::Z * 0.5 * shot.gravity * flight * flight - muzzle
        }
        (None, None) => aim.forward_2d().extend(0.0),
    };
    let speed = toss.map_or(shot.speed, |v| v.length());
    let spec = crate::projectiles::ProjectileSpec {
        // An accelerating shot (`ShouldAcc`) speeds up linearly to its `MaxSpeed`.
        speed: if shot.accel > 0.0 && shot.max_speed > speed && toss.is_none() { vec![(0.0, speed), ((shot.max_speed - speed) / shot.accel, shot.max_speed)] } else { vec![(0.0, speed)] },
        lifetime: shot.lifetime.unwrap_or(shot.range * 1.2 / speed.max(1.0)).max(if toss.is_some() { 5.0 } else { 0.0 }),
        radius: shot.radius,
        turn_rate: shot.turn_rate,
        gravity: shot.gravity,
        attack: crate::buffs::attack_of(world, e, c.attack),
        factor: shot.factor,
        element: Element::Physical,
        by,
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: shot.slow.map(|(rate, s)| (crate::buffs::BuffKind::Slow(rate), s)),
        burst: shot.burst,
        fuse: false,
        ignore_guard: shot.ignore_guard,
    knockback: None,
    };
    crate::projectiles::spawn(world, id, spec, muzzle, dir, target);
}

/// One shot of a volley ([`crate::monsters::VolleyShot`]): from the caster's centre plus its offsets along his axes,
/// along his facing turned by its pitch (HYPOTHESIS: positive up; every known one is 0) and yaw (positive to his
/// right, as Unreal's); a delayed one hangs at speed 0 for its `delay`, then flies.
fn fire_volley(world: &mut World, id: SimId, e: Entity, by: HitSource, v: crate::monsters::VolleyShot) {
    let (Some(from), Some(aim), Some(c)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied(), world.get::<Combatant>(e).copied()) else { return };
    let forward = aim.forward_2d();
    let right = Vec2::new(-forward.y, forward.x);
    let pos = from + forward.extend(0.0) * v.forward + right.extend(0.0) * v.right + Vec3::Z * v.up;
    let (sp, cp) = v.pitch.to_radians().sin_cos();
    let flat = Vec2::from_angle(v.yaw.to_radians()).rotate(forward);
    let dir = (flat * cp).extend(sp);
    let shot = v.shot;
    let mut speed = if shot.accel > 0.0 && shot.max_speed > shot.speed { vec![(0.0, shot.speed), ((shot.max_speed - shot.speed) / shot.accel, shot.max_speed)] } else { vec![(0.0, shot.speed)] };
    if shot.delay > 0.0 {
        speed = std::iter::once((0.0, 0.0)).chain(speed.into_iter().map(|(t, s)| (t + shot.delay + 1.0e-3, s))).collect();
        speed.insert(1, (shot.delay, 0.0));
    }
    let spec = crate::projectiles::ProjectileSpec {
        speed,
        lifetime: shot.lifetime.unwrap_or(shot.delay + shot.range * 1.2 / shot.speed.max(1.0)),
        radius: shot.radius,
        turn_rate: 0.0,
        gravity: shot.gravity,
        attack: crate::buffs::attack_of(world, e, c.attack),
        factor: shot.factor,
        element: Element::Physical,
        by,
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: shot.slow.map(|(rate, s)| (crate::buffs::BuffKind::Slow(rate), s)),
        burst: shot.burst,
        fuse: false,
        ignore_guard: shot.ignore_guard,
        knockback: None,
    };
    crate::projectiles::spawn(world, id, spec, pos, dir, None);
}

/// A spray ([`crate::monsters::Spray`]): its random count of lobbed projectiles, each on its own random heading and
/// speed, with the caster's buffed attack.
fn fire_spray(world: &mut World, id: SimId, e: Entity, by: HitSource, s: crate::monsters::Spray) {
    let (Some(from), Some(aim), Some(c)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied(), world.get::<Combatant>(e).copied()) else { return };
    let forward = aim.forward_2d();
    let right = Vec2::new(-forward.y, forward.x);
    let pos = from + forward.extend(0.0) * s.forward + right.extend(0.0) * s.right + Vec3::Z * s.up;
    let attack = crate::buffs::attack_of(world, e, c.attack);
    let lerp = |rng: &mut SimRng, (a, b): (f32, f32)| a + rng.unit_f32() * (b - a);
    let shots: Vec<(Vec3, f32)> = {
        let mut rng = world.resource_mut::<SimRng>();
        let n = rng.range_u32(s.count.0, s.count.1 + 1);
        (0..n)
            .map(|_| {
                let (v, h) = (lerp(&mut rng, s.vertical).to_radians(), lerp(&mut rng, s.horizontal).to_radians());
                let speed = lerp(&mut rng, s.speed);
                let flat = Vec2::from_angle(h).rotate(forward);
                ((flat * v.sin()).extend(v.cos()), speed)
            })
            .collect()
    };
    for (dir, speed) in shots {
        let spec = crate::projectiles::ProjectileSpec {
            speed: vec![(0.0, speed)],
            lifetime: s.shot.lifetime.unwrap_or(5.0),
            radius: s.shot.radius,
            turn_rate: 0.0,
            gravity: s.shot.gravity,
            attack,
            factor: s.shot.factor,
            element: Element::Physical,
            by,
            pierce: 0,
            chain: 0,
            chain_range: 0.0,
            pull: 0.0,
            on_hit: s.shot.slow.map(|(rate, t)| (crate::buffs::BuffKind::Slow(rate), t)),
            burst: s.shot.burst,
            fuse: false,
            ignore_guard: s.shot.ignore_guard,
            knockback: None,
        };
        crate::projectiles::spawn(world, id, spec, pos, dir, None);
    }
}

/// An area attack's fuses ([`crate::monsters::AreaSpawn`]) around the AI target (itself without one), on
/// the ground below each point (the first level box down from the target's centre height, at most 500 cm
/// below its feet; else at its feet). Fire damage
/// (HYPOTHESIS: from the effect's `DamageTaken.Target.Projectile.Fire` cue).
fn spawn_area(world: &mut World, id: SimId, e: Entity, by: HitSource, area: crate::monsters::AreaSpawn) {
    let (Some(from), Some(c)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Combatant>(e).copied()) else { return };
    let target = world.get::<crate::ai::MonsterAi>(e).and_then(|a| a.target);
    let center = target.and_then(|t| world.resource::<SimIndex>().get(t)).and_then(|te| world.get::<Position>(te)).map_or(from, |p| p.0);
    let feet = center.z - crate::movement::CAPSULE_HEIGHT * 0.5;
    let step = 360 / area.count.max(1);
    let mut points = Vec::new();
    for i in 0..area.count {
        let mut rng = world.resource_mut::<SimRng>();
        let yaw = ((i * step + rng.range_u32(0, step + 1)) as f32).to_radians();
        let r = rng.range_u32(area.radius_min, area.radius_max + 1) as f32;
        points.push(center + Vec3::new(yaw.cos(), yaw.sin(), 0.0) * r);
    }
    let grounds: Vec<f32> = {
        let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, bevy_ecs::system::Query<(), bevy_ecs::query::With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
        let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
        // Down from the target's centre height (from higher up it would find the ceiling's top).
        let depth = center.z - feet + crate::overrides::monster_area::GROUND_PROBE;
        points.iter().map(|p| spatial.cast_ray_predicate(Vec3::new(p.x, p.y, center.z), bevy_math::Dir3::NEG_Z, depth, true, &crate::layers::walk(), &|b| boxes.contains(b)).map_or(feet, |h| center.z - h.distance)).collect()
    };
    let attack = crate::buffs::attack_of(world, e, c.attack);
    for (p, ground) in points.into_iter().zip(grounds) {
        let spec = crate::projectiles::ProjectileSpec {
            speed: vec![(0.0, 0.0)],
            lifetime: area.fuse,
            radius: 0.0,
            turn_rate: 0.0,
            gravity: 0.0,
            attack,
            factor: area.factor,
            element: Element::Fire,
            by,
            pierce: 0,
            chain: 0,
            chain_range: 0.0,
            pull: 0.0,
            on_hit: None,
            burst: area.size,
            fuse: true,
            ignore_guard: false,
        knockback: None,
        };
        crate::projectiles::spawn(world, id, spec, Vec3::new(p.x, p.y, ground + area.size), Vec3::X, None);
    }
}

/// An explosion's hit: the living non-monsters within its radius and height with no level wall between.
fn burst_targets(world: &mut World, id: SimId, e: Entity, swing: &mut Swing, ids: &[(SimId, Entity)], b: crate::monsters::Burst) {
    let Some(from) = world.get::<Position>(e).map(|p| p.0) else { return };
    let candidates: Vec<(SimId, Vec3)> = ids
        .iter()
        .filter(|(tid, te)| *tid != id && world.get::<crate::command::Kind>(*te).is_none_or(|k| k.0 != crate::command::ActorKind::Monster))
        .filter_map(|&(tid, te)| world.get::<Position>(te).map(|p| (tid, p.0)))
        .filter(|(_, to)| (to.z - from.z).abs() <= b.height && (*to - from).length() <= b.radius + CAPSULE_RADIUS)
        .collect();
    let clear: Vec<SimId> = {
        let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
        let (spatial, boxes) = state.get(world).expect("spatial query");
        candidates.iter().filter(|(_, to)| !crate::ai::wall_between(&spatial, &boxes, from, *to)).map(|c| c.0).collect()
    };
    hit_targets(world, id, e, swing, ids, &|tid, _, _, _| clear.contains(&tid));
}

/// Hits the targets `inside` accepts (by id, attacker and target positions), each once per swing.
pub(crate) fn hit_targets(world: &mut World, id: SimId, e: Entity, swing: &mut Swing, ids: &[(SimId, Entity)], inside: &dyn Fn(SimId, Vec3, &Aim, Vec3) -> bool) {
    let (Some(from), Some(aim), Some(attacker)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied(), world.get::<Combatant>(e).copied()) else {
        return;
    };
    for &(tid, te) in ids {
        if tid == id || swing.hit.contains(&tid) {
            continue;
        }
        let (Some(to), Some(defender), Some(health)) = (world.get::<Position>(te).map(|p| p.0), world.get::<Combatant>(te).copied(), world.get::<Health>(te).copied()) else {
            continue;
        };
        if health.current <= 0.0 || !inside(tid, from, &aim, to) {
            continue;
        }
        swing.hit.push(tid);
        let (attack, defence) = (crate::buffs::attack_of(world, e, attacker.attack), crate::buffs::defence_of(world, te, defender.defence));
        let hit = resolve_hit(&attack, &defence, Element::Physical, AttackTerm::Weapon, HitZone::Body, &mut world.resource_mut::<SimRng>());
        let mut damage = hit.damage * swing.factor;
        // A swing whose hit value reaches the bound breaks the guard: its hit isn't guarded (`GA_TGF_Base`'s
        // `ApplyGuarded`: "Guarded Not Applied, Because Heavy Hit").
        let bound = world.get_resource::<crate::weapons::WeaponCombos>().map_or(0.0, |c| c.hurt_interrupt);
        let breaks = bound > 0.0 && swing.hit_value >= bound;
        let blocking = !breaks && world.get::<Melee>(te).is_some_and(Melee::blocking);
        let mut guarded = false;
        if blocking {
            let taim = world.get::<Aim>(te).copied().unwrap_or_default();
            let angles = world.get::<Melee>(te).and_then(|m| m.guard_angles);
            damage = blocked_damage(damage, &defender, &taim, angles, to, from);
            guarded = guard_faces(&taim, angles, to, from);
            // A perfect guard takes nothing (its full guarded damage reduction).
            if guarded && crate::weapons::guarded(world, te) {
                damage = 0.0;
            }
        }
        let damage = crate::buffs::incoming_damage(world, te, damage, Some(e));
        let left = crate::world_items::apply_damage(world, tid, te, damage);
        {
            let mut events = world.resource_mut::<SimEvents>();
            events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by: swing.by });
            if left <= 0.0 {
                events.push(SimEvent::Died { id: tid });
            }
        }
        crate::buffs::after_damage_dealt(world, id, e, damage, blocking);
        // The legendary affixes' passives: the hitter's on a weapon swing's hit, the guard's on a faced block.
        if damage > 0.0 && matches!(swing.by, HitSource::Swing(_)) {
            crate::affixes::on_hit(world, e, te, true);
            if swing.by == HitSource::Swing(AttackKind::Heavy) {
                crate::affixes::on_charge_hit(world, e, te);
            }
            crate::gear_triggers::on_weapon_hit(world, (id, e), (tid, te));
        }
        if guarded {
            crate::affixes::on_guarded(world, tid, te, id, e);
        }
        // A mace swing's hit plays its rebound montage, which chains the combo.
        if swing.by == HitSource::Swing(AttackKind::Light)
            && let Some(rest) = crate::weapons::rebound(world, e)
        {
            swing.windows.retain(|w| w.0 <= swing.tick);
            swing.length = swing.tick + rest;
        }
        if damage > 0.0 {
            crate::class_skills::weapon_hit(world, e);
        }
        // Rend and the Rogue's poison fire on an unblocked weapon hit.
        if !blocking && left > 0.0 && matches!(swing.by, HitSource::Swing(_)) {
            crate::skills::rend_on_hit(world, id, e, tid, te, damage);
            crate::class_skills::poison_on_hit(world, id, e, te);
            crate::weapons::enchant_on_hit(world, id, e, tid, te, swing.by);
        }
    }
}

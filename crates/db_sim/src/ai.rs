//! Monster AI: the game's behaviour trees (`BT_Monster_Default` and the subtrees it runs, from the
//! recovered graphs) run by [`crate::bt`], with the game's own tree nodes (`BTT_*`, `BTD_*`, `BTS_*`)
//! implemented here from their recovered Blueprint bytecode where it says what they do. The AI controller
//! (`AIC_Monster_Default`) and the monster pawn's tuning are not decoded, so the controller's part is a
//! HYPOTHESIS stand-in ([`crate::overrides::ai`]): perception (sight cone with line of sight, hearing,
//! being hit) puts a player on the threat list after a short notice; the threat is dropped when lost
//! (then the monster searches where it last saw it, `BT_Alert`); ability picking, the action roll after
//! an ability (flank), the retreat range, ranges and intervals. Numbers come from data where they exist
//! (`MonsterAttributeCurve`: sight, hearing, move and turn speed; the trees' own waits, time limits and
//! service intervals).

use avian3d::prelude::SpatialQuery;
use bevy_ecs::prelude::*;
use bevy_math::{Dir3, Vec2, Vec3};

use crate::bt::{BbValue, Blackboard, BtState, Host, Node, ServiceEvent, Status, TaskMem, Trees};
use crate::command::{ActorKind, Kind};
use crate::id::SimIndex;
use crate::melee::{Aim, Health, Melee};
use crate::monsters::MonsterKit;
use crate::movement::{CAPSULE_RADIUS, CharacterState, MoveInput};
use crate::overrides::ai as tune;
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng, TickCount};

/// What the monster is doing, for events, sounds and tests (a summary of where its tree is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiState {
    /// Out of combat: holding at its spawn point (or wandering, patrolling).
    Idle,
    /// Noticed a target, or searching where it last saw one (`BT_Alert`).
    Alert,
    /// Moving to its ability target (`BT_Ability` chase).
    Chase,
    /// Using an ability.
    Attack,
    /// Waiting out its ability interval.
    Recover,
    /// Circling the target (`BT_Flank`).
    Flank,
    /// Backing away from the target (`BT_Retreat`).
    Retreat,
    /// Walking back to the spawn point.
    Return,
    /// Dormant (`BT_Dormant`): asleep or lying in wait at its spawn point, or settling into it.
    Dormant,
    /// Waking from dormancy (`BT_WakeUp`): its wake clip plays.
    WakeUp,
    Dead,
}

/// What a monster's spawn point says about it (`BP_ActorSpawnPoint`'s `DataMonster*` fields, from the map
/// placement). `None` leaves it to the monster.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpawnOpts {
    /// `DataMonsterDormantable` (`EMonsterDormantable`: Enable / Disable).
    pub dormant: Option<bool>,
    /// `DataMonsterDormantSightRadius`, cm.
    pub dormant_sight: Option<f32>,
    /// `DataMonsterWanderable` (`IsWanderable`: this, else the Blueprint's `Wanderable`) and
    /// `DataMonsterWanderRadius` (cm; `GetWanderRadius`: this when above 0, else the Blueprint's `WanderRadius`).
    pub wander: Option<bool>,
    pub wander_radius: Option<f32>,
    /// its patrol (`DataMonsterPatrolPath` with a `DataMonsterPatrolMode` other than 0: `IsPatrolable`).
    pub patrol: Option<std::sync::Arc<Patrol>>,
    /// the floor's mechanism its death triggers, with the `TriggerParam` (`ControlledMechanism` on its spawn
    /// point, handed over as `DeathTriggerMechanism`).
    pub death_trigger: Option<(u32, u8)>,
}

/// A spawn point's patrol (`BP_ActorSpawnPoint`, `BP_PatrolPoint`): `EPatrolMode` (1 loop, 2 back and forth, 3
/// once) and the points in order (world capsule-centre positions, `WaitTime` s).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Patrol {
    pub mode: u8,
    pub points: Vec<(Vec3, f32)>,
}

/// Perception and movement numbers for one monster (from its curve row).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiSenses {
    /// cm
    pub sight: f32,
    /// cm
    pub hearing: f32,
    /// Turn speed, degrees per second.
    pub turn_speed: f32,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct MonsterAi {
    pub state: AiState,
    /// The threat target (in combat while set).
    pub target: Option<SimId>,
    pub home: Vec3,
    pub senses: AiSenses,
    /// Ticks spent in the current state.
    pub timer: u32,
    /// Ticks since the target was last seen or heard.
    pub unseen: u32,
    /// Ticks of noticing left before combat starts.
    pub notice: u32,
    /// Near enough to a player to think and move. A dormant monster keeps its place and its tree's
    /// state; the client doesn't animate it either.
    pub awake: bool,
    /// The tree's execution and blackboard.
    pub bt: BtState,
    /// Where the target was last perceived.
    last_seen: Vec3,
    /// Flank waypoints (`BTT_FlankBegin`).
    flank_path: Vec<Vec3>,
    /// Turn-speed factor from the active turn services.
    turn_factor: f32,
    /// Keep facing the threat target while moving (`BTS_Flank` sets the focus).
    focus: bool,
    started: bool,
    /// The sim tick, for the node callbacks.
    clock: u32,
    last_health: f64,
    /// Goes dormant at its spawn point when idle (`IsDormantable`), and is now (`IsDormant`).
    pub dormantable: bool,
    pub dormant: bool,
    /// A dormant monster notices players only this close (cm), any direction, with line of sight.
    pub dormant_sight: f32,
    /// A dormant clip playing: going to sleep (`true`) or waking (`false`), and the tick it ends.
    pub pose: Option<(bool, u32)>,
    /// A flyer's wanted height (capsule centre z, cm).
    fly_z: f32,
    /// Woken by opening it, not by its senses (the mimics: `BP_Tgf_Monster_Chest` spawns a
    /// `BP_TGF_CharacterWake` interaction whose `PerformInteraction` calls `Wake(InteractingPawn)`; its
    /// `Dormants` have `SensesEnabled` false). Openable while [`MonsterAi::dormant`].
    pub wake_on_open: bool,
    /// The player who opened it, until the next AI tick (`Wake`: `AddThreatWithForceTargetable(Instigator)`
    /// when it has no target yet).
    pub provoked_by: Option<SimId>,
    /// The Goblin Thief's controller state (unused by other monsters).
    pub thief: crate::thief::ThiefState,
    /// The Shadow King's countdowns (unused by other monsters).
    pub king: crate::shadow_king::KingState,
    /// The Wendigo's totem clock (unused by other monsters).
    pub wendigo: crate::wendigo::WendigoState,
    /// Class-valued blackboard keys (`SetBlackboardValueAsClass`: the Wendigo's `Summon`): key, ability class.
    pub class_keys: Vec<(String, String)>,
    /// Unseen while dormant (its Blueprint's `Dormants[0].Invisible`: the thief).
    pub invisible_dormant: bool,
    /// The tick each attack is off cooldown again (by kit index, missing = ready).
    pub cooldowns: Vec<u32>,
    /// `AddPriorAbility`: the ability `BTT_ActivateAbilityNextTime` keeps for the next pick.
    pub prior: Option<u8>,
    /// its spawn point's patrol, whether it wanders (`IsWanderable`) and how far (`GetWanderRadius`, cm).
    pub patrol: Option<std::sync::Arc<Patrol>>,
    pub wanderable: bool,
    pub wander_radius: f32,
    /// `FleeState` (0 not yet, 1 fleeing, 2 fled: once per life) and its flee path (`FleePath`).
    pub flee_state: u8,
    /// evading (`BP_Tgf_Monster`'s `Evade`, `ChaseFailed`, `ChaseFailedTime`): an elite that has failed to
    /// reach its target for longer than `EvadeTime` stops chasing and acts from where it stands; the tick its
    /// first failed chase was seen.
    pub evade: bool,
    pub chase_failed: Option<u32>,
    /// Ticks the combat-leave timer has run (`CombatLeaveTimer`).
    pub leave_timer: u32,
    pub flee_path: Vec<Vec3>,
}

impl MonsterAi {
    pub fn new(home: Vec3, senses: AiSenses) -> Self {
        MonsterAi { state: AiState::Idle, target: None, home, senses, timer: 0, unseen: 0, notice: 0, awake: false, bt: BtState::default(), last_seen: home, flank_path: Vec::new(), turn_factor: 1.0, focus: false, started: false, clock: 0, last_health: f64::NAN, dormantable: false, dormant: false, dormant_sight: tune::DORMANT_SIGHT, pose: None, fly_z: home.z, wake_on_open: false, provoked_by: None, thief: Default::default(), king: Default::default(), wendigo: Default::default(), class_keys: Vec::new(), invisible_dormant: false, cooldowns: Vec::new(), prior: None, patrol: None, wanderable: false, wander_radius: tune::WANDER_RADIUS, flee_state: 0, flee_path: Vec::new(), evade: false, chase_failed: None, leave_timer: 0 }
    }

    /// The spawn point's settings: a dormantable monster starts dormant (the trees' `DormantSpawn`).
    /// Unset ones come from the Blueprint (`Dormantable`, `Dormants[0].SightRadius`), else the HYPOTHESIS
    /// defaults.
    pub fn apply_spawn(&mut self, kit: &MonsterKit, opts: SpawnOpts) {
        let bp = (!kit.bp.folder.is_empty()).then_some(&kit.bp);
        self.dormantable = opts.dormant.or(bp.map(|b| b.dormantable)).unwrap_or_else(|| tune::DORMANT_BY_DEFAULT.contains(&kit.name.as_str()));
        self.dormant = self.dormantable;
        self.dormant_sight = opts.dormant_sight.or(bp.and_then(|b| b.dormant.as_ref()?.sight_radius)).unwrap_or(tune::DORMANT_SIGHT);
        // `SensesEnabled` false (the mimics): it perceives nothing while dormant and wakes when opened.
        if bp.and_then(|b| b.dormant.as_ref()?.senses) == Some(false) {
            self.dormant_sight = 0.0;
            self.wake_on_open = self.dormantable;
        }
        self.invisible_dormant = bp.and_then(|b| b.dormant.as_ref()?.invisible) == Some(true);
        // (`IsWanderable`, `GetWanderRadius`, `IsPatrolable`; CONFIRMED bytecode).
        self.wanderable = opts.wander == Some(true) || bp.is_some_and(|b| b.wanderable);
        self.wander_radius = opts.wander_radius.filter(|r| *r > 0.0).or(bp.and_then(|b| b.wander_radius)).unwrap_or(tune::WANDER_RADIUS);
        self.patrol = opts.patrol.filter(|p| p.mode != 0 && !p.points.is_empty());
        if self.dormant {
            self.state = AiState::Dormant;
        }
    }
}

fn ticks(s: f32) -> u32 {
    (s.max(0.0) / DT).round() as u32
}

/// Turns `yaw` towards `want` by at most `max_step` radians.
fn turn_towards(yaw: f32, want: f32, max_step: f32) -> f32 {
    let diff = angle_diff(yaw, want);
    yaw + diff.clamp(-max_step, max_step)
}

fn angle_diff(yaw: f32, want: f32) -> f32 {
    (want - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// The game's trees, loaded once per process.
fn trees() -> Option<&'static Trees> {
    static TREES: std::sync::OnceLock<Option<Trees>> = std::sync::OnceLock::new();
    TREES.get_or_init(|| Trees::load().ok()).as_ref()
}

#[derive(Clone, Copy)]
struct Candidate {
    id: SimId,
    entity: Entity,
    pos: Vec3,
    loud: bool,
    /// Stealthed: noticed only within the reveal distances.
    stealth: Option<(f32, f32)>,
}

type Monsters<'w, 's> = Query<'w, 's, (&'static mut MonsterAi, &'static mut MoveInput, &'static mut Aim, &'static mut Melee, &'static Position, &'static Health, &'static MonsterKit)>;
type Targets<'w, 's> = Query<'w, 's, (&'static Kind, &'static Position, Option<&'static Health>, Option<&'static CharacterState>, Option<&'static crate::buffs::Buffs>, Option<&'static crate::class_skills::ClassKit>)>;

/// One AI tick for every monster, in id order: perception and threat (the controller stand-in), then
/// the behaviour tree.
#[allow(clippy::too_many_arguments)]
pub(crate) fn ai_tick(index: Res<SimIndex>, tick: Res<TickCount>, mut rng: ResMut<SimRng>, mut events: ResMut<SimEvents>, spatial: SpatialQuery, boxes: Query<(), With<crate::command::LevelBox>>, targets: Targets, summoned: Query<(&crate::summon::Summoned, &Health)>, nav: Option<Res<crate::nav::NavMaps>>, mut monsters: Monsters) {
    let now = tick.0 as u32;
    let hits = events.take_hits();
    // Living players, in id order.
    let players: Vec<Candidate> = index
        .iter()
        .filter_map(|(id, e)| {
            let (kind, pos, health, st, buffs, class) = targets.get(e).ok()?;
            // Reveal distances from the Rogue's data (others use the Rogue default from overrides).
            let reveal = match class.map(|k| &k.data) {
                Some(crate::class_skills::ClassData::Rogue(d)) => (d.stealth_reveal, d.stealth_reveal_z),
                _ => (tune::STEALTH_REVEAL, tune::STEALTH_REVEAL_Z),
            };
            (kind.0 == ActorKind::Player && health.is_none_or(|h| h.current > 0.0)).then(|| Candidate {
                id,
                entity: e,
                pos: pos.0,
                // Heard when moving on foot and not crouched (HYPOTHESIS).
                loud: st.is_some_and(|s| s.velocity.truncate().length() > tune::LOUD_SPEED && !s.crouched),
                stealth: buffs.is_some_and(crate::buffs::Buffs::stealthed).then_some(reveal),
            })
        })
        .collect();
    // Every player, alive or not, for waking monsters up. A scene without players (some tests) keeps every
    // monster awake.
    let player_pos: Vec<Vec3> = index.iter().filter_map(|(_, e)| targets.get(e).ok().filter(|t| t.0.0 == ActorKind::Player).map(|t| t.1.0)).collect();
    let stunned: Vec<SimId> = index.iter().filter(|(_, e)| targets.get(*e).ok().and_then(|t| t.4).is_some_and(crate::buffs::Buffs::stunned)).map(|(id, _)| id).collect();
    // For the ability rules' filters: targetable foes (living players not in stealth), living monsters
    // (allies) and each summoner's living summons.
    let foes: Vec<Vec3> = players.iter().filter(|c| c.stealth.is_none()).map(|c| c.pos).collect();
    let friends: Vec<(SimId, Vec3)> = index.iter().filter_map(|(id, e)| monsters.get(e).ok().filter(|m| m.5.current > 0.0).map(|m| (id, m.4.0))).collect();
    let mut summons: std::collections::BTreeMap<SimId, u32> = Default::default();
    for (s, h) in &summoned {
        if h.current > 0.0 {
            *summons.entry(s.by).or_default() += 1;
        }
    }
    let Some(trees) = trees() else { return };
    let Some(&default_root) = trees.roots.get("BT_Monster_Default") else { return };
    for (id, e) in index.iter() {
        let Ok((mut ai, mut input, mut aim, mut melee, pos, health, kit)) = monsters.get_mut(e) else { continue };
        let root = trees.roots.get(kit.tree()).copied().unwrap_or(default_root);
        // A dormant clip ends.
        if ai.pose.is_some_and(|(_, until)| now >= until) {
            ai.pose = None;
        }
        // Flyers hold their height; the dead fall.
        input.fly = (kit.flyer && health.current > 0.0).then_some(ai.fly_z);
        let from = pos.0;
        let senses = ai.senses;
        let provoker = ai.provoked_by.take();
        let hurt = health.current < ai.last_health;
        let damaged = hurt || provoker.is_some();
        ai.last_health = health.current;
        // Its NPC state's `TGF.GameplayEvent.Damage` behaviours, once per hit taken since the last tick: the event
        // comes with the hit, whatever its tree is doing (noticing the attacker, too).
        if let Some(npc) = kit.npc.as_deref().filter(|n| !n.on_damage.is_empty()) {
            for &(_, by) in hits.iter().filter(|(t, _)| *t == id) {
                let Some((kind, at)) = index.get(by).and_then(|be| targets.get(be).ok()).map(|t| (t.0.0, t.1.0)) else { continue };
                let behind = kind != ActorKind::Monster && (at - from).truncate().dot(aim.forward_2d()) < 0.0;
                for b in &npc.on_damage {
                    if !matches!(b, crate::npc_state::Behavior::BehindHit { .. }) || behind {
                        let ai = &mut *ai;
                        crate::npc_state::apply(b, &mut ai.bt.bb, &mut ai.prior, kit);
                    }
                }
            }
        }
        // (`BP_Tgf_Monster`'s health change, CONFIRMED bytecode): under `FleeHealth`, not weak (the sim has no
        // weakness), never fled yet: it flees at `FleeChance`.
        let flee = kit.bp.flee.as_slice();
        if hurt
            && health.current > 0.0
            && ai.flee_state == 0
            && let Some(at) = flee.first().copied().flatten()
            && health.max > 0.0
            && ((health.current / health.max) as f32) < at
            && rng.unit_f32() < flee.get(1).copied().flatten().unwrap_or(0.0)
        {
            ai.flee_state = 1;
        }
        if hurt && let Some(t) = kit.thief.as_deref() {
            // `AIC_Goblin_Thief.OnDamaged`: the first hit that leaves it under `AccelerateHealth` speeds it
            // up; any hit stops the escape ritual until its portal touch (`BlockDamageInterrupt`).
            if ai.thief.accelerated_at.is_none() && health.max > 0.0 && health.current / health.max < t.accelerate_health {
                ai.thief.accelerated_at = Some(now);
            }
            if ai.bt.bb.bool("Escaping") && !ai.thief.past_touch {
                ai.bt.bb.set("Escaping", BbValue::Bool(false));
            }
        }
        // Dormancy (HYPOTHESIS: the game's own tick culling isn't decoded): awake within its senses plus a
        // margin of the nearest player, asleep again a margin farther, but only from its idle routine (so it
        // finishes walking home or searching first); a threat or a hit keeps it awake. Waking is farther
        // than it can see or hear, so it never changes what it notices.
        let near = player_pos.iter().map(|p| (*p - from).length()).fold(f32::INFINITY, f32::min);
        let wake = senses.sight.max(senses.hearing) + tune::WAKE_MARGIN;
        // A revealed thief keeps going until it has escaped.
        let busy = ai.target.is_some() || ai.notice > 0 || !matches!(ai.state, AiState::Idle | AiState::Dead) || (ai.thief.revealed_at.is_some() && !ai.thief.escaped);
        ai.awake = player_pos.is_empty() || damaged || near <= wake || (ai.awake && (busy || near <= wake + tune::SLEEP_MARGIN));
        if !ai.awake && health.current > 0.0 {
            input.dir = Vec2::ZERO;
            continue;
        }
        let previous = ai.state;
        if !ai.started {
            // The spawn point and facing (`SpawnLocation`, `SpawnRotation` as a point ahead).
            ai.started = true;
            let home = ai.home;
            ai.bt.bb.set("SpawnLocation", BbValue::Vector(home));
            ai.bt.bb.set("SpawnRotation", BbValue::Vector(home + aim.forward_2d().extend(0.0) * 100.0));
        }
        if health.current <= 0.0 {
            if ai.state != AiState::Dead {
                let mut bt = std::mem::take(&mut ai.bt);
                let mut host = MonsterHost { from, ai: &mut ai, input: &mut input, aim: &mut aim, melee: &mut melee, kit, target: None, stunned: false, rng: &mut rng, spatial: &spatial, boxes: &boxes, nav: nav.as_deref(), health: 0.0, nearest_player: f32::INFINITY, id, foes: &foes, friends: &friends, summons: 0, tags: Vec::new() };
                bt.reset(trees, &mut host, now);
                ai.bt = bt;
            }
            ai.state = AiState::Dead;
            input.dir = Vec2::ZERO;
        } else {
            // Perception.
            let hidden = |c: &Candidate| c.stealth.is_some_and(|(r, rz)| (c.pos - from).truncate().length() > r || (c.pos - from).z.abs() > rz);
            let dormant = ai.dormant;
            let dormant_sight = ai.dormant_sight;
            let sees = |c: &Candidate| -> bool {
                if hidden(c) {
                    return false;
                }
                let d = c.pos - from;
                let dist = d.length();
                // Dormant (HYPOTHESIS: `BeginDormant` is native): only within the spawn point's dormant sight
                // radius, any direction.
                if dormant {
                    return dist <= dormant_sight && line_of_sight(&spatial, e, c.entity, from, c.pos);
                }
                if dist > senses.sight || dist < 1e-3 {
                    return dist < 1e-3;
                }
                // Its Blueprint's `SightPeripheralVisionAngleDegrees` (degrees from its facing as in Unreal's sight
                // config; HYPOTHESIS that the native controller passes it on unchanged).
                let half_angle = kit.bp.sight_angle.unwrap_or(tune::SIGHT_HALF_ANGLE_DEG);
                let facing = aim.forward_2d().dot(d.truncate().normalize_or_zero()) >= half_angle.to_radians().cos();
                facing && line_of_sight(&spatial, e, c.entity, from, c.pos)
            };
            let hears = |c: &Candidate| !dormant && !hidden(c) && c.loud && (c.pos - from).length() <= senses.hearing;
            let target = ai.target.and_then(|t| players.iter().find(|c| c.id == t)).copied();
            // Losing the threat target. With the Blueprint's values (CONFIRMED `AIC_Monster_Default`'s tick): in
            // combat its senses are off (`OnEnterCombat`), so sight doesn't matter; a target at least
            // `ThreatTargetLostDistance` away (3D) for `ThreatTargetLostTime` is dropped (`ThreatTargetTimer`), an
            // untargetable one (stealthed, dead) at once (`FilterThreats`), and a monster more than
            // `CombatLeaveDistance` from its spawn point for `CombatLeaveTime` leaves combat (`CombatLeaveTimer`).
            // Both timers stand still while it flees or acts (`TGF.AI.ActivatingAbility`; HYPOTHESIS: while a
            // swing runs) unless it evades (`ShouldPauseCombatTimer`). HYPOTHESIS: counted every tick, not every
            // 0.2 s (`TickTime`). Without a Blueprint: lost after the default time unseen, or at once beyond 1.5 x
            // sight.
            let (lost_time, lost_distance) = (kit.bp.threat_lost_time, kit.bp.threat_lost_distance);
            let paused = (ai.flee_state == 1 || melee.swing.is_some() || melee.queued_montage.is_some() || ai.thief.active.is_some()) && !ai.evade;
            let mut lost = false;
            if let (Some(lost_time), Some(lost_distance)) = (lost_time, lost_distance) {
                if let Some(t) = target {
                    ai.last_seen = t.pos;
                    if (t.pos - from).length() < lost_distance {
                        ai.unseen = 0;
                    } else if !paused {
                        ai.unseen = ai.unseen.saturating_add(1);
                    }
                    let leave = match kit.bp.combat_leave.as_slice() {
                        [Some(time), Some(distance), ..] if *time > 0.0 && *distance > 0.0 && ai.notice == 0 => Some((*time, *distance)),
                        _ => None,
                    };
                    match leave {
                        Some((_, distance)) if (from - ai.home).length() <= distance => ai.leave_timer = 0,
                        Some(_) if !paused => ai.leave_timer = ai.leave_timer.saturating_add(1),
                        _ => {}
                    }
                    lost = hidden(&t) || ai.unseen >= ticks(lost_time) || leave.is_some_and(|(time, _)| ai.leave_timer >= ticks(time));
                }
            } else {
                match target {
                    Some(t) if sees(&t) || hears(&t) => {
                        ai.last_seen = t.pos;
                        ai.unseen = 0;
                    }
                    _ => ai.unseen = ai.unseen.saturating_add(1),
                }
                lost = ai.unseen >= ticks(tune::LOSE_TARGET_AFTER) || target.is_some_and(|t| (t.pos - from).length() > senses.sight * tune::LEASH_FACTOR);
            }
            match target {
                // Dropped: search where it was last seen.
                Some(_) if lost => drop_threat(&mut ai),
                None if ai.target.is_some() => drop_threat(&mut ai),
                None => {
                    // Seeing, hearing or being hit puts a player on the threat list (the nearest one
                    // when hit: HYPOTHESIS, the attacker isn't tracked).
                    let noticed = provoker.filter(|p| players.iter().any(|c| c.id == *p)).or_else(|| players.iter().find(|c| sees(c) || hears(c)).map(|c| c.id)).or_else(|| damaged.then(|| players.iter().min_by(|a, b| (a.pos - from).length().total_cmp(&(b.pos - from).length())).map(|c| c.id)).flatten());
                    if let Some(t) = noticed {
                        ai.target = Some(t);
                        ai.unseen = 0;
                        ai.notice = ticks(tune::ALERT_DELAY);
                        ai.bt.bb.set("AlertState", BbValue::Int(0));
                    }
                }
                _ => {}
            }
            let target = ai.target.and_then(|t| players.iter().find(|c| c.id == t)).copied();
            if ai.notice > 0 {
                // Noticing: stop and turn to the target, then combat starts.
                ai.notice -= 1;
                input.dir = Vec2::ZERO;
                if let Some(t) = target.filter(|_| !ai.dormant) {
                    let d = (t.pos - from).truncate();
                    aim.yaw = turn_towards(aim.yaw, d.y.atan2(d.x), senses.turn_speed.to_radians() * DT);
                }
                ai.state = if ai.dormant { AiState::Dormant } else { AiState::Alert };
            } else {
                // The thief reveals when it first enters combat (`OnCombatEntered`: `Revealed`, `RevealedTime`).
                if kit.thief.is_some() && ai.thief.revealed_at.is_none() && ai.target.is_some() {
                    ai.thief.revealed_at = Some(now);
                }
                // The Wendigo's totem clock (`AIC_Wendigo`): full on entering combat, counting while in it.
                if let Some(w) = kit.wendigo.as_deref() {
                    let fighting = ai.target.is_some() && ai.notice == 0;
                    ai.wendigo.step(w, fighting, crate::DT);
                }
                let nearest_player = players.iter().map(|c| (c.pos - from).length()).fold(f32::INFINITY, f32::min);
                let mut bt = std::mem::take(&mut ai.bt);
                {
                    let rate = if health.max > 0.0 { health.current / health.max } else { 1.0 };
                    // Its gameplay tags: its ability sets' and its effects'.
                    let tags: Vec<u32> = kit.tags.iter().copied().chain(targets.get(e).ok().and_then(|t| t.4).into_iter().flat_map(crate::buffs::Buffs::tags)).collect();
                    let mut host = MonsterHost { from, ai: &mut ai, input: &mut input, aim: &mut aim, melee: &mut melee, kit, target, stunned: stunned.contains(&id), rng: &mut rng, spatial: &spatial, boxes: &boxes, nav: nav.as_deref(), health: rate, nearest_player, id, foes: &foes, friends: &friends, summons: summons.get(&id).copied().unwrap_or(0), tags };
                    host.thief_step(now);
                    bt.tick(trees, root, &mut host, now);
                }
                ai.state = match ai.pose {
                    Some((false, _)) => AiState::WakeUp,
                    _ if ai.dormant => AiState::Dormant,
                    _ => summarize(trees, &bt, ai.state, &melee),
                };
                ai.bt = bt;
            }
        }
        if ai.state != previous {
            ai.timer = 0;
            events.push(SimEvent::AiStateChanged { id, state: ai.state, target: ai.target });
        } else {
            ai.timer += 1;
        }
    }
}

/// Drops the threat target: combat ends, and the monster searches where it was last seen.
fn drop_threat(ai: &mut MonsterAi) {
    ai.target = None;
    ai.evade = false;
    ai.chase_failed = None;
    ai.unseen = 0;
    ai.leave_timer = 0;
    ai.notice = 0;
    for k in ["AbilityTarget", "ActionTarget", "Ability", "Flank", "Retreat", "InAbilityRange"] {
        ai.bt.bb.clear(k);
    }
    let at = ai.last_seen;
    ai.bt.bb.set("AlertState", BbValue::Int(2));
    ai.bt.bb.set("AlertLocation", BbValue::Vector(at));
}

/// The state shown for where the tree is.
fn summarize(trees: &Trees, bt: &BtState, previous: AiState, melee: &Melee) -> AiState {
    let active = bt.active_trees(trees);
    let Some(task) = bt.task().map(|t| trees.node(t)) else { return previous };
    if active.contains(&"BT_Alert") {
        return AiState::Alert;
    }
    // (Monster-specific subtrees too: `BT_Flank_Ghost`, `BT_Retreat_Inscet_Fly`.)
    if active.iter().any(|t| t.starts_with("BT_Flank")) {
        return AiState::Flank;
    }
    if active.iter().any(|t| t.starts_with("BT_Retreat") || t.starts_with("BT_Flee")) {
        return AiState::Retreat;
    }
    let target = task.key("Target").or(task.key("TargetKey")).unwrap_or("");
    match task.class.as_str() {
        "BTT_AbilityBegin_C" | "BTT_WaitAbilityComplete_C" if active.contains(&"BT_Ability") => AiState::Attack,
        _ if melee.swing.is_some() => AiState::Attack,
        "BTT_AbilityInterval_C" | "BTT_ActivateAbilityNextTime_C" => AiState::Recover,
        "BTT_MoveTo_C" | "BTT_FlyTo_C" if target == "AbilityTarget" => AiState::Chase,
        "BTT_FaceTo_C" if target == "AbilityTarget" && matches!(previous, AiState::Idle | AiState::Return | AiState::Alert) => AiState::Chase,
        "BTT_MoveTo_C" | "BTT_FlyTo_C" if target == "SpawnLocation" => AiState::Return,
        "BTT_MoveTo_C" if target == "RunAwayLocation" => AiState::Retreat,
        "BTT_Hold_C" | "BTT_Stun_C" | "BTTask_Wait" if !active.contains(&"BT_Ability") => AiState::Idle,
        _ if active.contains(&"BT_Patrol") || active.contains(&"BT_Wander") => AiState::Idle,
        _ => previous,
    }
}

/// The sim side of the game's tree nodes for one monster.
struct MonsterHost<'a, 'w, 's> {
    from: Vec3,
    ai: &'a mut MonsterAi,
    input: &'a mut MoveInput,
    aim: &'a mut Aim,
    melee: &'a mut Melee,
    kit: &'a MonsterKit,
    target: Option<Candidate>,
    stunned: bool,
    rng: &'a mut SimRng,
    spatial: &'a SpatialQuery<'w, 's>,
    boxes: &'a Query<'w, 's, (), With<crate::command::LevelBox>>,
    nav: Option<&'a crate::nav::NavMaps>,
    /// Health / max health (`GetHealthPercent`).
    health: f64,
    /// The nearest living player (cm).
    nearest_player: f32,
    id: SimId,
    /// Targetable foes, living monsters (with itself) and its living summons, for the ability rules.
    foes: &'a [Vec3],
    friends: &'a [(SimId, Vec3)],
    summons: u32,
    /// Its gameplay tag ids, with their parents.
    tags: Vec<u32>,
}

impl MonsterHost<'_, '_, '_> {
    fn in_combat(&self) -> bool {
        self.ai.target.is_some() && self.ai.notice == 0
    }

    /// A key's position: an actor (the threat target), a vector, or the flank path's next point.
    fn point(&self, bb: &Blackboard, key: &str) -> Option<Vec3> {
        match bb.get(key) {
            Some(BbValue::Vector(v)) => Some(v),
            Some(BbValue::Actor(a)) => self.target.filter(|t| t.id == a).map(|t| t.pos),
            _ => None,
        }
    }

    fn turn_to(&mut self, p: Vec3) -> f32 {
        let d = (p - self.from).truncate();
        if d.length() < 1e-3 {
            return 0.0;
        }
        let want = d.y.atan2(d.x);
        self.aim.yaw = turn_towards(self.aim.yaw, want, self.ai.senses.turn_speed.to_radians() * self.ai.turn_factor * DT);
        angle_diff(self.aim.yaw, want).abs()
    }

    /// Swing and shot indices usable against a target `dist` cm away (centre to centre), and whether
    /// a shot has a clear line.
    fn usable(&self, dist: f32, los: bool) -> (Vec<u8>, Vec<u8>) {
        let swings = (0..self.kit.attacks.len() as u8).filter(|&i| self.kit.attacks[usize::from(i)].shot.is_none() && dist <= self.attack_range(usize::from(i))).collect();
        let shots = (0..self.kit.attacks.len() as u8).filter(|&i| los && self.kit.attacks[usize::from(i)].shot.is_some_and(|s| dist <= s.range)).collect();
        (swings, shots)
    }

    /// The controller's `PickAbility` (CONFIRMED bytecode): the prior ability if it passes (the list is used up
    /// either way), else a roll weighted by the selector's `Chance` among the abilities that pass
    /// [`Self::filter_ability`]. Range plays no part: `BT_Ability` chases into it afterwards. Nothing passing
    /// clears the blackboard's ability (`BTT_PickAbility` fails; the tree waits 0.1 s and picks again). A kit
    /// without selector weights (stand-ins from the overrides) keeps the old reach-based pick.
    fn pick_ability(&mut self, bb: &mut Blackboard) -> bool {
        let Some(t) = self.target else { return false };
        if self.kit.attacks.is_empty() {
            return false;
        }
        if !self.kit.attacks.iter().any(|a| a.pick.chance > 0.0) && self.kit.alternatives.is_empty() {
            return self.pick_by_reach(&t, bb);
        }
        // The first alternative selector whose tags it all has, else the Blueprint's (CONFIRMED
        // `FilterAbilitySelector`).
        let alt = self.kit.alternatives.iter().find(|a| a.tags.iter().all(|t| self.tags.contains(t)));
        let chances: Vec<f64> = match alt {
            Some(a) => a.chances.clone(),
            None => self.kit.attacks.iter().map(|a| a.pick.chance).collect(),
        };
        let prior = self.ai.prior.take().filter(|&i| self.filter_ability(usize::from(i), &t));
        let i = match prior {
            Some(i) => i,
            None => {
                let passed: Vec<u8> = (0..self.kit.attacks.len() as u8).filter(|&i| chances.get(usize::from(i)).is_some_and(|&c| c > 0.0) && self.filter_ability(usize::from(i), &t)).collect();
                let Some(&last) = passed.last() else {
                    bb.clear("Ability");
                    bb.clear("AbilityTarget");
                    return false;
                };
                let chance = |i: u8| chances[usize::from(i)];
                let roll = self.rng.unit_f64() * passed.iter().map(|&i| chance(i)).sum::<f64>();
                let mut acc = 0.0;
                passed.iter().copied().find(|&i| {
                    acc += chance(i);
                    roll <= acc
                })
                .unwrap_or(last)
            }
        };
        bb.set("Ability", BbValue::Int(i64::from(i)));
        bb.set("AbilityTarget", BbValue::Actor(t.id));
        true
    }

    /// `FilterAbility`: off cooldown, the rule's self filters (all, or any with `FilterOp` 1), then its target
    /// filters on the threat target (`ThreatHighest` for almost every attack): a `Range` filter's distances; an
    /// ally-targeting ability never passes (the sim has none). HYPOTHESIS: a `Tags` filter sees no tags on the
    /// monster (the sim keeps no gameplay tags yet).
    fn filter_ability(&self, i: usize, t: &Candidate) -> bool {
        let Some(a) = self.kit.attacks.get(i) else { return false };
        if self.ai.cooldowns.get(i).is_some_and(|&ready| self.ai.clock < ready) {
            return false;
        }
        use crate::monsters::SelfFilter as F;
        let within = |r: f32, p: &Vec3| (*p - self.from).length() <= r;
        let foes = |r: f32| self.foes.iter().filter(|p| within(r, p)).count() as f64;
        let test = |f: &F| match *f {
            F::None => true,
            F::HealthLow(share) => self.health <= share,
            F::HealthHigh(share) => self.health >= share,
            F::FriendlyCount(r, lo, hi) => {
                let n = self.friends.iter().filter(|(id, p)| *id != self.id && within(r, p)).count() as f64;
                lo < n && n < hi
            }
            F::HostileCount(r, lo, hi) => {
                let n = foes(r);
                lo < n && n < hi
            }
            F::Boss => crate::monsters::BOSSES.iter().any(|s| s.row == self.kit.name),
            F::EnemyBehind(r) => self.foes.iter().any(|p| within(r, p) && (*p - self.from).truncate().dot(self.aim.forward_2d()) < 0.0),
            F::SummonsCount(lo, hi) => lo < f64::from(self.summons) && f64::from(self.summons) < hi,
            F::Tags(has, ref tags) => tags.iter().any(|t| self.tags.contains(t)) == has,
        };
        let p = &a.pick;
        let passes = if p.any { p.filters.iter().any(test) } else { p.filters.iter().all(test) };
        if !passes {
            return false;
        }
        if p.self_target {
            return true;
        }
        // An ally-targeting ability: an ally within its `Range` target filter.
        if p.friendly {
            let r = p.target_range.map_or(f32::INFINITY, |t| t.0);
            return self.friends.iter().any(|(id, q)| *id != self.id && within(r, q));
        }
        let dist = (t.pos - self.from).length();
        p.target_range.is_none_or(|(max, min)| dist < max && (min <= 0.0 || dist > min))
    }

    /// The pick for a kit without selector weights (HYPOTHESIS, the pre- rule): a swing if the target is
    /// within reach, else a shot that reaches it with a clear line, else any swing (to chase into reach), else
    /// a shot.
    fn pick_by_reach(&mut self, t: &Candidate, bb: &mut Blackboard) -> bool {
        let t = *t;
        let dist = (t.pos - self.from).length();
        let los = self.clear_line(t.pos);
        let (swings, shots) = self.usable(dist, los);
        let all_swings: Vec<u8> = (0..self.kit.attacks.len() as u8).filter(|&i| self.kit.attacks[usize::from(i)].shot.is_none()).collect();
        let all_shots: Vec<u8> = (0..self.kit.attacks.len() as u8).filter(|&i| self.kit.attacks[usize::from(i)].shot.is_some()).collect();
        let pool = [swings, shots, all_swings, all_shots].into_iter().find(|p| !p.is_empty()).unwrap_or_default();
        let i = pool[self.rng.range_u32(0, pool.len() as u32) as usize];
        bb.set("Ability", BbValue::Int(i64::from(i)));
        bb.set("AbilityTarget", BbValue::Actor(t.id));
        true
    }

    /// `GetAbilityActivateRange`, centre to centre: the ability rule's `ActivateRange`, else the shot's
    /// range, else (HYPOTHESIS, a kit not from the ability set) melee reach to the target's capsule surface.
    /// (the swings' root motion carries a lunge to its target, so the full range.)
    fn attack_range(&self, i: usize) -> f32 {
        let scale = crate::monsters::reach_scale(&self.kit.name);
        match self.kit.attacks.get(i) {
            Some(a) if a.range.is_some() => a.range.unwrap_or_default(),
            Some(a) if a.shot.is_some() => a.shot.map_or(0.0, |s| s.range),
            _ => tune::ATTACK_RANGE * scale + CAPSULE_RADIUS,
        }
    }

    fn ability_range(&self, bb: &Blackboard) -> f32 {
        self.attack_range(bb.int("Ability") as usize)
    }

    /// `FindPathToActorSynchronously` is valid and not partial: a nav route reaches the target (HYPOTHESIS:
    /// without a nav grid every target counts as reachable).
    fn full_path_to(&self, to: Vec3) -> bool {
        let Some(nav) = self.nav.filter(|n| !n.0.is_empty()) else { return true };
        let half = Vec3::Z * (crate::movement::CAPSULE_HEIGHT * 0.5);
        nav.route(self.from - half, to - half).is_some()
    }

    /// `SetEvade` (only an evadable monster's changes), which also clears `ChaseFailed`.
    fn set_evade(&mut self, on: bool) {
        if self.kit.bp.evadable() {
            self.ai.evade = on;
            self.ai.chase_failed = None;
        }
    }

    /// `OnChaseFailed`: the first failure starts the clock; one more than `EvadeTime` later, it evades.
    fn on_chase_failed(&mut self, now: u32) {
        if self.ai.evade {
            return;
        }
        match self.ai.chase_failed {
            Some(at) => {
                if self.kit.bp.evade_time.is_some_and(|s| now.saturating_sub(at) > ticks(s)) {
                    self.ai.evade = true;
                }
            }
            None => self.ai.chase_failed = Some(now),
        }
    }

    /// `OnChaseSuccessed`.
    fn on_chase_succeeded(&mut self) {
        if self.ai.evade {
            self.set_evade(false);
        } else {
            self.ai.chase_failed = None;
        }
    }

    fn in_ability_range(&self, bb: &Blackboard) -> bool {
        let Some(t) = self.target.filter(|t| bb.actor("AbilityTarget") == Some(t.id)) else { return false };
        let dist = (t.pos - self.from).length();
        let i = bb.int("Ability") as usize;
        // A self ability is always in range; any other needs its range and a clear line (CONFIRMED
        // `BTS_CheckInAbilityRange`: the trace is skipped only for `ActorType` 12).
        match self.kit.attacks.get(i) {
            // An ally-targeting ability is cast where it stands (HYPOTHESIS: the real one walks to its ally).
            Some(a) if a.pick.self_target || a.pick.friendly => true,
            Some(_) => dist <= self.ability_range(bb) && self.clear_line(t.pos),
            None => false,
        }
    }

    /// `BTT_FindFleePath` (CONFIRMED bytecode): from its spot away from the target, `FleePointCount` points, each
    /// `FleeDistanceMin..Max` on along the last leg turned by up to `FleeAngle` either way; on foot each is a
    /// random navigable point within `FleePointRadius` of that (here a grid node of the area, HYPOTHESIS; without a
    /// grid the point itself), and a point that finds none is skipped. Fails without a target or any point.
    fn find_flee_path(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        self.ai.flee_path.clear();
        let Some(from) = self.point(bb, node.key("TargetAgainst").unwrap_or("ActionTarget")) else { return Status::Failed };
        let f = self.kit.bp.flee.clone();
        let get = |i: usize, d: f32| f.get(i).copied().flatten().unwrap_or(d);
        let (angle, (dmin, dmax), radius, count) = (get(2, 60.0), (get(3, 500.0), get(4, 600.0)), get(5, 50.0), get(6, 3.0) as usize);
        let half = Vec3::Z * (crate::movement::CAPSULE_HEIGHT * 0.5);
        let (mut prev, mut cur) = (from, self.from);
        for _ in 0..count {
            let leg = (cur - prev).truncate().normalize_or_zero();
            let dist = dmin + (dmax - dmin) * self.rng.unit_f32();
            let turn = (-angle + 2.0 * angle * self.rng.unit_f32()).to_radians();
            let ahead = cur + (Vec2::from_angle(turn).rotate(leg) * dist).extend(0.0);
            let next = if self.kit.flyer {
                Some(ahead)
            } else {
                match self.nav.filter(|n| !n.0.is_empty()) {
                    Some(nav) => nav.random_point(ahead - half, radius, self.rng).map(|p| p + half),
                    None => Some(ahead),
                }
            };
            if let Some(next) = next {
                self.ai.flee_path.push(next);
                prev = cur;
                cur = next;
            }
        }
        if self.ai.flee_path.is_empty() { Status::Failed } else { Status::Succeeded }
    }

    /// `BTT_PatrolBegin` (CONFIRMED bytecode): the patrol point nearest to it (the later of equals), forward.
    fn patrol_begin(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        let Some(patrol) = self.ai.patrol.clone() else { return Status::Failed };
        let mut best: Option<(f32, usize)> = None;
        for (i, p) in patrol.points.iter().enumerate() {
            let d = self.from.distance_squared(p.0);
            if best.is_none_or(|b| b.0 >= d) {
                best = Some((d, i));
            }
        }
        let i = best.map_or(0, |b| b.1);
        bb.set(node.key("PatrolPointIndex").unwrap_or("PatrolPointIndex"), BbValue::Int(i as i64));
        bb.set(node.key("PatrolPoint").unwrap_or("PatrolPoint"), BbValue::Vector(patrol.points[i].0));
        bb.set(node.key("PatrolDirection").unwrap_or("PatrolDirection"), BbValue::Int(1));
        bb.set(node.key("PatrolCompleted").unwrap_or("PatrolCompleted"), BbValue::Bool(false));
        Status::Succeeded
    }

    /// `BTT_PatrolReachPoint` after its wait (CONFIRMED bytecode): the next point along the direction; past an
    /// end, by `EPatrolMode`: 1 back to the first, 2 turn round, 3 the patrol is complete.
    fn patrol_next(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        let Some(patrol) = self.ai.patrol.clone() else { return Status::Failed };
        let (index_key, dir_key) = (node.key("PatrolPointIndex").unwrap_or("PatrolPointIndex"), node.key("PatrolDirection").unwrap_or("PatrolDirection"));
        let (index, mut dir) = (bb.int(index_key), bb.int(dir_key));
        let n = patrol.points.len() as i64;
        let mut next = index + dir;
        if next < 0 || next >= n {
            match patrol.mode {
                1 => next = 0,
                2 => {
                    dir = -dir;
                    next = (index + dir).clamp(0, n - 1);
                    bb.set(dir_key, BbValue::Int(dir));
                }
                _ => {
                    bb.set(node.key("PatrolCompleted").unwrap_or("PatrolCompleted"), BbValue::Bool(true));
                    return Status::Succeeded;
                }
            }
        }
        bb.set(index_key, BbValue::Int(next));
        bb.set(node.key("PatrolPoint").unwrap_or("PatrolPoint"), BbValue::Vector(patrol.points[next as usize].0));
        Status::Succeeded
    }

    /// `BTT_FindWanderLocation` (CONFIRMED bytecode): a random navigable point within `GetWanderRadius` of the
    /// wander base (`GetRandomLocationInNavigableRadius`: here a random node of its grid area, HYPOTHESIS; without a
    /// grid a random point of the disc), at the base's height for a flyer.
    fn find_wander(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        let Some(base) = self.point(bb, node.key("WanderBaseLocation").unwrap_or("WanderBaseLocation")) else { return Status::Failed };
        let half = Vec3::Z * (crate::movement::CAPSULE_HEIGHT * 0.5);
        let r = self.ai.wander_radius;
        let found = match self.nav.filter(|n| !n.0.is_empty()) {
            Some(nav) => nav.random_point(base - half, r, self.rng).map(|p| p + half),
            None => {
                let (a, t) = (self.rng.unit_f32() * std::f32::consts::TAU, self.rng.unit_f32().sqrt());
                Some(base + (Vec2::from_angle(a) * r * t).extend(0.0))
            }
        };
        let Some(mut p) = found else { return Status::Failed };
        if self.kit.flyer {
            p.z = base.z;
        }
        bb.set(node.key("WanderLocation").unwrap_or("WanderLocation"), BbValue::Vector(p));
        Status::Succeeded
    }

    /// Where a move on foot heads next: the next point of an A* route on the floor's nav grid (HYPOTHESIS: a
    /// stand-in for the navmesh path the original follows), re-planned when the goal moves or every
    /// [`tune::NAV_REPLAN_SECONDS`]; the goal itself without a grid or route, or when the way is straight: the
    /// route keeps near the straight line, nothing blocks at knee or eye height and the goal is within
    /// [`tune::NAV_STRAIGHT_RISE`] (rays alone pass over a drop beside a stair).
    fn waypoint(&self, goal: Vec3, mem: &mut TaskMem, now: u32) -> Vec3 {
        let Some(nav) = self.nav.filter(|n| !n.0.is_empty()) else { return goal };
        let half = Vec3::Z * (crate::movement::CAPSULE_HEIGHT * 0.5);
        let stale = mem.route_at.is_none_or(|at| now >= at + ticks(tune::NAV_REPLAN_SECONDS)) || mem.route_goal.distance(goal) > tune::NAV_REPLAN_DISTANCE;
        if stale {
            mem.route = nav.route(self.from - half, goal - half).unwrap_or_default();
            mem.route_index = 0;
            mem.route_goal = goal;
            mem.route_at = Some(now);
        }
        let feet = self.from - half;
        // Passed when it stands there: close sideways and at its height (a stair point beside the steps isn't).
        let passed = |p: Vec3| (p - feet).truncate().length() <= tune::NAV_POINT_REACH && (p.z - feet.z).abs() <= crate::overrides::movement::MAX_STEP_HEIGHT;
        while mem.route_index + 1 < mem.route.len() && passed(mem.route[mem.route_index]) {
            mem.route_index += 1;
        }
        let (a, b) = (feet.truncate(), (goal - half).truncate());
        let along = |p: &Vec3| {
            let ab = b - a;
            let t = ((p.truncate() - a).dot(ab) / ab.length_squared().max(1.0)).clamp(0.0, 1.0);
            (a + ab * t).distance(p.truncate()) <= tune::NAV_STRAIGHT_SLACK
        };
        let rest = mem.route.get(mem.route_index..).unwrap_or_default();
        if (goal.z - self.from.z).abs() <= tune::NAV_STRAIGHT_RISE && rest.iter().all(along) && self.clear_line(goal) && self.clear_knee(goal) {
            return goal;
        }
        mem.route.get(mem.route_index).map_or(goal, |p| *p + half)
    }

    /// No level geometry between here and `to` at knee height (low walls and ledges the eye ray passes over).
    fn clear_knee(&self, to: Vec3) -> bool {
        let knee = Vec3::Z * (crate::overrides::movement::MAX_STEP_HEIGHT + 10.0 - crate::movement::CAPSULE_HEIGHT * 0.5);
        let (a, b) = (self.from + knee, to + knee);
        let Ok(dir) = Dir3::new(b - a) else { return true };
        let boxes = self.boxes;
        self.spatial.cast_ray_predicate(a, dir, (b - a).length(), true, &crate::layers::walk(), &|e| boxes.contains(e)).is_none()
    }

    /// `BTS_CheckInAbilityRange`'s line trace (channel 9, not decoded): HYPOTHESIS, only level geometry
    /// blocks it (monster shots fly through monsters), between the eye points.
    fn clear_line(&self, to: Vec3) -> bool {
        let eye = Vec3::Z * tune::EYE_HEIGHT;
        let (a, b) = (self.from + eye, to + eye);
        let Ok(dir) = Dir3::new(b - a) else { return true };
        let boxes = self.boxes;
        self.spatial.cast_ray_predicate(a, dir, (b - a).length(), true, &crate::layers::shots(), &|e| boxes.contains(e)).is_none()
    }

    fn busy(&self) -> bool {
        self.melee.swing.is_some() || self.melee.queued_montage.is_some() || self.ai.pose.is_some() || self.ai.thief.active.is_some()
    }

    /// `BeginDormant(bDormant)` (native, HYPOTHESIS): dormant or awake from now on, playing the going-
    /// to-sleep or the wake clip meanwhile (`BTT_WaitAbilityComplete` waits for it).
    fn begin_dormant(&mut self, sleep: bool, now: u32) {
        self.input.dir = Vec2::ZERO;
        self.ai.dormant = sleep;
        let clip = if sleep { &self.kit.dormancy.sleep } else { &self.kit.dormancy.wake };
        let length = clip.as_ref().map_or(0.0, |c| c.1);
        // A sleep clip is a loop: settling into it takes one pass at most (3 s). A wake clip plays out
        // (the thief's 7.1 s; `BT_WakeUp` waits for its wake ability).
        let length = if sleep { length.min(3.0) } else { length };
        self.ai.pose = (length > 0.0).then(|| (sleep, now + ticks(length)));
    }

    /// The flying insect's flight paths (`BTT_<Hover|Rise|Feint|Flank>Begin_Monster_Inscet_Fly` `Calc*Point`,
    /// CONFIRMED arithmetic from their bytecode): points around the start, each turned about the
    /// target-to-start line, the side flipping every point; FlankTime from the node or the pawn (HYPOTHESIS).
    fn fly_path(&mut self, kind: &str, bb: &mut Blackboard) -> Status {
        let Some(t) = self.target else { return Status::Failed };
        let start = self.from;
        let away = (start - t.pos).truncate().normalize_or(Vec2::X);
        let mut dir = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let rand_i = |rng: &mut SimRng, lo: u32, hi: u32| (lo + rng.range_u32(0, hi - lo + 1)) as f32;
        let at = |angle: f32, dist: f32, dz: f32| (start.truncate() + Vec2::from_angle(angle.to_radians()).rotate(away) * dist).extend(start.z + dz);
        let mut points = Vec::new();
        let (count, time) = match kind {
            "Hover" => (rand_i(self.rng, 2, 3) as u32, 6.0 + 3.0 * self.rng.unit_f32()),
            "Rise" => (rand_i(self.rng, 8, 9) as u32, 6.0 + 3.0 * self.rng.unit_f32()),
            "Feint" => (rand_i(self.rng, 1, 2) as u32 * 2 + 1, tune::FLY_FLANK_TIME.0 + (tune::FLY_FLANK_TIME.1 - tune::FLY_FLANK_TIME.0) * self.rng.unit_f32()),
            _ => (rand_i(self.rng, tune::FLY_FLANK_POINTS.0, tune::FLY_FLANK_POINTS.1) as u32, tune::FLY_FLANK_TIME.0 + (tune::FLY_FLANK_TIME.1 - tune::FLY_FLANK_TIME.0) * self.rng.unit_f32()),
        };
        for i in 1..=count {
            dir = -dir;
            let fi = i as f32;
            let p = match kind {
                "Hover" if i < count => at(dir * 30.0, (fi + 3.0) * 30.0, fi * 50.0 + 200.0),
                "Hover" => at(dir * 30.0, (fi + 3.0) * 30.0, 200.0),
                "Rise" if i == 1 => at(dir * 5.0, rand_i(self.rng, 1, 2) * 30.0, 180.0),
                "Rise" if i == 2 => at(dir * 10.0, 300.0, 200.0),
                "Rise" if i < 5 => at(dir * 40.0, (fi + 3.0) * 50.0, rand_i(self.rng, 1, 3) * 50.0 + 100.0),
                "Rise" => at(dir * 30.0, (fi + 4.0) * 60.0, rand_i(self.rng, 1, 3) * 40.0 + 50.0),
                // Odd points dive at the target, even ones pull back.
                "Feint" if i % 2 == 1 => t.pos + Vec3::Z * 60.0,
                "Feint" => at(dir * fi * 30.0, 300.0, rand_i(self.rng, 1, 3) * 50.0 + 10.0),
                _ if i < 3 => at(dir * tune::FLANK_ANGLE.0, (fi + 2.0) * 40.0, 40.0 + 20.0 * self.rng.unit_f32() + if self.rng.chance(0.5) { 40.0 } else { 0.0 }),
                _ => at(dir * tune::FLANK_ANGLE.1, (fi + 3.0) * 40.0, 30.0 + 30.0 * self.rng.unit_f32() + if self.rng.chance(0.5) { 70.0 } else { 0.0 }),
            };
            points.push(p);
        }
        self.ai.flank_path = points;
        bb.set("FlankTime", BbValue::Float(time));
        Status::Succeeded
    }

    /// The flying insect's `PickActionTarget` (CONFIRMED from bytecode): the threat target, and a movement
    /// style 1-4 at random, never the same twice running (a repeat steps down, 1 wraps to 4).
    fn pick_fly_action(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        let Some(t) = self.target else {
            bb.clear("Flank");
            bb.clear("Retreat");
            return Status::Failed;
        };
        bb.set(node.key("ActionTarget").unwrap_or("ActionTarget"), BbValue::Actor(t.id));
        let key = node.key("MoveActionStyle").unwrap_or("MoveActionStyle");
        let new = 1 + self.rng.range_u32(0, 4) as i64;
        let style = match (new == bb.int(key), new) {
            (true, 1) => 4,
            (true, n) => n - 1,
            (false, n) => n,
        };
        bb.set(key, BbValue::Int(style));
        Status::Succeeded
    }

    /// The ghost's `PickActionTarget` (CONFIRMED from bytecode): the threat target, and a move style `10 *
    /// rand(1..2) + rand(1..2)` above half health, `10 * rand(1..2) + 2` at or below it. The tens pick the
    /// order (retreat-flank under 19, flank-retreat over 19), an even style adds a second flank.
    fn pick_ghost_action(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        let Some(t) = self.target else {
            bb.clear("Flank");
            bb.clear("Retreat");
            return Status::Failed;
        };
        bb.set(node.key("ActionTarget").unwrap_or("ActionTarget"), BbValue::Actor(t.id));
        let ones = if self.health > 0.5 { 1 + self.rng.range_u32(0, 2) as i64 } else { 2 };
        let tens = 1 + self.rng.range_u32(0, 2) as i64;
        bb.set(node.key("MoveActionStyle").unwrap_or("MoveActionStyle"), BbValue::Int(10 * tens + ones));
        Status::Succeeded
    }

    /// `BTT_MoveTo` / `BTT_FlyTo`: steer to the target point; done within the acceptance radius
    /// (5 cm, plus both capsules for an actor goal), failed when stuck for 0.5 s.
    fn move_to(&mut self, node: &Node, bb: &Blackboard, mem: &mut TaskMem, now: u32) -> Status {
        let key = node.key("Target").unwrap_or("");
        let goal = if key == "FlankPath" {
            match self.ai.flank_path.get(mem.index) {
                Some(&p) => Some(p),
                None => return Status::Succeeded,
            }
        } else {
            self.point(bb, key)
        };
        let Some(goal) = goal else { return Status::Failed };
        let actor_goal = matches!(bb.get(key), Some(BbValue::Actor(_)));
        let radius_key = node.key("AcceptanceRadiusBlackboardKey").or(node.key("AcceptanceRadiusDestinationBlackboardKey")).unwrap_or("");
        let mut accept = bb.float(radius_key).filter(|r| *r > 0.0).unwrap_or(tune::MOVE_ACCEPTANCE);
        if actor_goal {
            accept += 2.0 * CAPSULE_RADIUS;
        }
        if key == "FlankPath" {
            accept *= tune::WAYPOINT_ACCEPTANCE_FACTOR;
        }
        if self.kit.flyer {
            // `BTT_FlyTo`: flies at the goal's height (an actor's capsule centre, a point's own height).
            self.ai.fly_z = goal.z;
            self.input.fly = Some(goal.z);
        }
        let d = (goal - self.from).truncate();
        // On foot, round walls and up stairs: the next point of its nav route.
        let towards = if self.kit.flyer { d } else { (self.waypoint(goal, mem, now) - self.from).truncate() };
        let off_height = self.kit.flyer && (goal.z - self.from.z).abs() > accept.max(tune::MOVE_ACCEPTANCE * tune::WAYPOINT_ACCEPTANCE_FACTOR);
        // A goal it would step past this tick is reached.
        let step = if mem.started != now { (self.from - mem.last_pos).truncate().length() } else { 0.0 };
        if (d.length() <= accept || (d.length() <= step && d.length() <= tune::MOVE_STRAIGHT_NEAR)) && !off_height {
            if key == "FlankPath" && mem.index + 1 < self.ai.flank_path.len() {
                mem.index += 1;
                return Status::Running;
            }
            self.input.dir = Vec2::ZERO;
            return Status::Succeeded;
        }
        match self.target.filter(|_| self.ai.focus) {
            // A flyer over its goal only climbs or sinks.
            _ if d.length() <= accept => self.input.dir = Vec2::ZERO,
            // Focused: face the target, walk towards the goal.
            Some(t) => {
                self.turn_to(t.pos);
                self.input.dir = towards.normalize_or_zero();
            }
            None => {
                self.turn_to(self.from + towards.extend(0.0));
                // Near its goal it steps straight at it, else along its facing.
                self.input.dir = if d.length() <= tune::MOVE_STRAIGHT_NEAR { towards.normalize_or_zero() } else { self.aim.forward_2d() };
            }
        }
        // Stuck: hardly moved for 0.5 s (`StuckTime`).
        if mem.started != now {
            if (self.from - mem.last_pos).length() < tune::STUCK_SPEED * DT {
                mem.stuck += 1;
                if mem.stuck >= ticks(tune::STUCK_TIME) {
                    self.input.dir = Vec2::ZERO;
                    return Status::Failed;
                }
            } else {
                mem.stuck = 0;
            }
        }
        mem.last_pos = self.from;
        Status::Running
    }

    /// `BTT_FaceTo`: turn to the target until within the precision (a non-zero float key, else the
    /// node's `Precision`, else 10°).
    fn face_to(&mut self, node: &Node, bb: &Blackboard) -> Status {
        self.input.dir = Vec2::ZERO;
        let Some(p) = self.point(bb, node.key("TargetKey").unwrap_or("")) else { return Status::Failed };
        let precision = bb.float(node.key("PrecisionKey").unwrap_or("")).filter(|v| *v != 0.0).or(node.f32("Precision")).unwrap_or(tune::FACE_PRECISION_DEG);
        if precision >= 180.0 {
            return Status::Succeeded;
        }
        if self.turn_to(p) <= precision.to_radians() { Status::Succeeded } else { Status::Running }
    }

    /// `BTT_FlankBegin` (CONFIRMED bytecode): `FlankPointCount` +- `FlankPointCountRandomDeviation` waypoints,
    /// each the last turned around the target by `FlankAngleMin..Max` degrees, all to one side when
    /// `FlankDirectionConsistent` (else each its own), for `FlankTimeMin..Max` s. A kit without Blueprint values
    /// uses the HYPOTHESIS ones.
    fn flank_begin(&mut self, bb: &mut Blackboard) -> Status {
        let Some(t) = self.target else { return Status::Failed };
        let bp = &self.kit.bp;
        let count = match bp.flank_point_count {
            Some(n) => {
                let dev = bp.flank_point_deviation.unwrap_or(0) * if self.rng.chance(0.5) { 1 } else { -1 };
                (n + dev).max(0) as u32
            }
            None => {
                let (lo, hi) = tune::FLANK_POINTS;
                lo + self.rng.range_u32(0, hi - lo + 1)
            }
        };
        if count == 0 {
            return Status::Failed;
        }
        let (amin, amax) = (bp.flank_angle.0.unwrap_or(tune::FLANK_ANGLE.0), bp.flank_angle.1.unwrap_or(tune::FLANK_ANGLE.1));
        let consistent = bp.flank_direction_consistent.unwrap_or(true);
        let side = if self.rng.unit_f32() < 0.5 { 1.0 } else { -1.0 };
        let mut p = self.from;
        let mut points = Vec::new();
        for _ in 0..count {
            let side = if consistent { side } else if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            let angle = side * (amin + (amax - amin) * self.rng.unit_f32()).to_radians();
            let off = (p - t.pos).truncate();
            let rotated = Vec2::from_angle(angle).rotate(off);
            p = (t.pos.truncate() + rotated).extend(p.z.min(t.pos.z));
            points.push(p);
        }
        self.ai.flank_path = points;
        let (tmin, tmax) = (bp.flank_time.0.unwrap_or(tune::FLANK_TIME.0), bp.flank_time.1.unwrap_or(tune::FLANK_TIME.1));
        let time = tmin + (tmax - tmin) * self.rng.unit_f32();
        bb.set("FlankTime", BbValue::Float(time));
        Status::Succeeded
    }

    /// `PickRetreat` (CONFIRMED bytecode): a roll weighted by the `Retreats` entries' `Chance` (`roll
    /// <= running sum`); none without entries.
    fn pick_retreat(&mut self) -> Option<usize> {
        let r = &self.kit.bp.retreats;
        if r.is_empty() {
            return None;
        }
        let roll = self.rng.unit_f64() * r.iter().map(|e| e.chance).sum::<f64>();
        let mut acc = 0.0;
        r.iter().position(|e| {
            acc += e.chance;
            roll <= acc
        })
    }

    /// The `Retreats` entry the blackboard's `RetreatIndex` names.
    fn retreat_entry(&self, bb: &Blackboard) -> Option<&crate::monsters::Retreat> {
        self.kit.bp.retreats.get(bb.int("RetreatIndex").max(0) as usize)
    }

    /// `BTT_FindRetreatLocation` (CONFIRMED bytecode): from where it stands, `DistanceMin..Max` away from the
    /// target turned by up to `Angle` degrees either way; a walker's point then moves to a random spot within
    /// `LocationRadius` (`GetRandomLocationInNavigableRadius`; HYPOTHESIS: any point in the disc). A kit
    /// without entries uses the struct defaults.
    fn find_retreat(&mut self, bb: &mut Blackboard) -> Status {
        let Some(t) = self.target else { return Status::Failed };
        let (angle_max, dmin, dmax, radius) = self.retreat_entry(bb).map_or((tune::RETREAT_ANGLE, tune::RETREAT_DISTANCE.0, tune::RETREAT_DISTANCE.1, 0.0), |e| (e.angle, e.distance_min, e.distance_max, e.location_radius));
        let away = (self.from - t.pos).truncate().normalize_or(Vec2::X);
        let dist = dmin + (dmax - dmin) * self.rng.unit_f32();
        let angle = (self.rng.unit_f32() * 2.0 - 1.0) * angle_max.to_radians();
        let mut p = self.from + (Vec2::from_angle(angle).rotate(away) * dist).extend(0.0);
        if !self.kit.flyer && radius > 0.0 {
            let (a, r) = (self.rng.unit_f32() * std::f32::consts::TAU, radius * self.rng.unit_f32().sqrt());
            p += (Vec2::from_angle(a) * r).extend(0.0);
        }
        bb.set("RetreatLocation", BbValue::Vector(p));
        Status::Succeeded
    }
}

impl Host for MonsterHost<'_, '_, '_> {
    fn rng(&mut self) -> &mut SimRng {
        self.rng
    }

    fn condition(&mut self, node: &Node, bb: &Blackboard) -> bool {
        let alert = bb.int("AlertState");
        match node.class.as_str() {
            "BTD_CanFly_C" => self.kit.flyer,
            "BTD_IsDormant_C" => self.ai.dormant,
            "BTD_IsDormantable_C" => self.ai.dormantable,
            "BTD_ActionStyleRiseRetreatHover_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) == 1,
            "BTD_ActionStyleRetreatHover_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) == 2,
            "BTD_ActionStyleRetreatFlankHover_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) == 3,
            "BTD_ActionStyleFlankRetreatHover_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) == 4,
            // The ghost's (bytecode). The order checks read the key their node names: in
            // `BT_Monster_Ghost` that is `SelfActor`, an object, which reads as 0, so the ghost always
            // retreats before it flanks (as in the game).
            "BTD_MoveActionStyleRetreatFlank_Ghost_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) < 19,
            "BTD_MoveActionStyleFlankRetreat_Ghost_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) > 19,
            "BTD_MoveActionStyleExtraFlank_Ghost_C" => bb.int(node.key("MoveActionStyle").unwrap_or("MoveActionStyle")) % 2 == 0,
            "BTD_AttackStyleFeint_C" => bb.int(node.key("AttackActionStyle").unwrap_or("AttackActionStyle")) == 1,
            "BTD_AttackStyleAbility_C" => bb.int(node.key("AttackActionStyle").unwrap_or("AttackActionStyle")) == 2,
            "BTD_IsInCombat_C" => self.in_combat(),
            "BTD_HasAction_C" => bb.bool("Flank") || bb.bool("Retreat"),
            // the retreat entry's `FocusTarget`.
            "BTD_IsRetreatFocusTarget_C" => self.retreat_entry(bb).is_some_and(|e| e.focus_target),
            "BTD_IsStun_C" => self.stunned,
            "BTD_IsAlert_C" => alert != 0,
            "BTD_IsAlertSuspicious_C" => alert == 1,
            "BTD_IsAlertSearch_C" => alert == 2,
            "BTD_IsMovable_C" | "BTD_IsRotatable_C" | "BTD_IsAbiliable_C" | "BTD_ShouldReturnToSpawnPoint_C" | "BTD_StationaryInAbilityInterval_C" => true,
            "BTD_IsInAbilityInterval_C" => bb.float("AbilityActivateTime").is_some_and(|t| (self.now_hint() as f32) < t),
            // The Shadow King's (bytecode): his phase pair and his controller's countdowns.
            // (CONFIRMED bytecode): the Wendigo's.
            "BTD_Wendigo_IsMode_C" => bb.int(node.key("ModeKey").unwrap_or("Mode")) == node.int("Mode").unwrap_or(0),
            "BTD_Wendigo_IsAbilitySequence_C" => bb.int(node.key("AbilitySequenceKey").unwrap_or("AbilitySequence")) == node.int("AbilitySequence").unwrap_or(0),
            "BTD_Wendigo_CheckTotemAbility_C" => match self.kit.wendigo.as_deref() {
                Some(w) => self.ai.wendigo.check_totem(w, self.health, self.summons),
                None => false,
            },
            "BTD_Shadow_King_CheckPhase_C" => bb.int(node.key("Phase").unwrap_or("Phase")) == node.int("PhaseNum").unwrap_or(0),
            "BTD_Shadow_King_CheckPhaseState_C" => bb.int(node.key("PhaseState").unwrap_or("PhaseState")) == node.int("PhaseStateNum").unwrap_or(0),
            "BTD_Shadow_King_CheckSkillAttack5_C" => self.now_hint() >= self.ai.king.attack5,
            "BTD_Shadow_King_CheckSkillAttack7_C" => self.now_hint() >= self.ai.king.attack7,
            "BTD_Shadow_King_CheckSkillAttack9_C" => self.now_hint() >= self.ai.king.attack9,
            // It also writes `MoveState`; nothing in his tree reads that key, so only the test is kept.
            "BTD_Shadow_King_CheckMoveState_C" => self.now_hint() >= self.ai.king.moved,
            // The Goblin Thief's (bytecode).
            "BTD_Goblin_Thief_IsEscaped_C" => self.ai.thief.escaped,
            "BTD_Goblin_Thief_IsRevealed_C" => self.ai.thief.revealed_at.is_some(),
            "BTD_Goblin_Thief_CheckWanderAbility_C" => bb.int(node.key("WanderAbility").unwrap_or("WanderAbility")) == node.props.get("WanderAbilitySelect").and_then(serde_json::Value::as_i64).unwrap_or(0),
            "BTD_IsWanderable_C" => self.ai.wanderable,
            "BTD_CanPatrol_C" => self.ai.patrol.is_some(),
            "BTD_IsFleeing_C" => self.ai.flee_state == 1,
            // Its own tags or its running montage's tag states (`TGF.AI.ActivatingAbility.Movable`).
            "BTD_HasGameplayTag_C" => node.key("Tag").is_some_and(|t| {
                let id = crate::buffs::gameplay_tag(t);
                self.tags.contains(&id) || self.melee.swing_has_tag(id)
            }),
            // Not decoded or not in the sim yet: weakness, patrol paths, wandering, fleeing,
            // movable casts, retreat focus, flank after stare (HYPOTHESIS: off).
            _ => false,
        }
    }

    fn start(&mut self, node: &Node, bb: &mut Blackboard, mem: &mut TaskMem, now: u32) -> Status {
        self.ai.clock = now;
        match node.class.as_str() {
            "BTT_ClearInterrupts_C" => {
                bb.clear("InterruptAction");
                bb.clear("InterruptAbility");
                Status::Succeeded
            }
            "BTT_PickActionTarget_C" => match self.target {
                Some(t) => {
                    bb.set(node.key("ActionTarget").unwrap_or("ActionTarget"), BbValue::Actor(t.id));
                    Status::Succeeded
                }
                None => {
                    bb.clear("Flank");
                    bb.clear("Retreat");
                    Status::Failed
                }
            },
            "BTT_BeginDormant_C" => {
                // The node's `Dormant` (CDO default true; `BT_WakeUp` sets false).
                let sleep = node.props.get("Dormant").and_then(serde_json::Value::as_bool).unwrap_or(true);
                self.begin_dormant(sleep, now);
                Status::Succeeded
            }
            "BTT_PickActionTarget_Inscet_Fly_C" => self.pick_fly_action(node, bb),
            "BTT_PickActionTarget_Ghost_C" => self.pick_ghost_action(node, bb),
            "BTT_PickFeintTarget_Inscet_Fly_C" => match self.target {
                Some(t) => {
                    bb.set(node.key("ActionTarget").unwrap_or("ActionTarget"), BbValue::Actor(t.id));
                    Status::Succeeded
                }
                None => Status::Failed,
            },
            "BTT_TakeAttackStyle_Inscet_Fly_C" => {
                // CONFIRMED: 1 feint or 2 attack, at random.
                let style = 1 + self.rng.range_u32(0, 2) as i64;
                bb.set(node.key("AttackActionStyle").unwrap_or("AttackActionStyle"), BbValue::Int(style));
                Status::Succeeded
            }
            "BTT_HoverBegin_Monster_Inscet_Fly_C" => self.fly_path("Hover", bb),
            "BTT_RiseBegin_Monster_Inscet_Fly_C" => self.fly_path("Rise", bb),
            "BTT_FeintBegin_Monster_Inscet_Fly_C" => self.fly_path("Feint", bb),
            "BTT_FlankBegin_Monster_Inscet_Fly_C" => self.fly_path("Flank", bb),
            "BTT_RiseClear_C" => {
                bb.clear("Retreat");
                bb.clear("Flank");
                Status::Succeeded
            }
            // The Shadow King's.
            c if c.starts_with("BTT_Shadow_King_") => self.king_task(node, bb, now),
            // (CONFIRMED bytecode): `TryActivateAbilityByClass` / the controller's `PickAbilityDirectly`.
            "BTT_ActivateAbilityDirectly_C" => self.activate_directly(node, bb, now),
            "BTT_PickAbilityDirectly_C" => {
                let Some(i) = node.key("Ability").and_then(|c| self.kit.by_class(c)) else { return Status::Failed };
                match self.target {
                    Some(t) if self.filter_ability(usize::from(i), &t) => {
                        bb.set("Ability", BbValue::Int(i64::from(i)));
                        bb.set("AbilityTarget", BbValue::Actor(t.id));
                        Status::Succeeded
                    }
                    _ => {
                        bb.clear("Ability");
                        bb.clear("AbilityTarget");
                        Status::Failed
                    }
                }
            }
            // (CONFIRMED bytecode): `RandomInteger(2)`, `RandomInteger(3)` into their int keys.
            "BTT_Wendigo_PickMode_C" => {
                let m = self.rng.range_u32(0, 2);
                bb.set(node.key("Mode").unwrap_or("Mode"), BbValue::Int(i64::from(m)));
                Status::Succeeded
            }
            "BTT_Wendigo_PickAbilitySequence_C" => {
                let s = self.rng.range_u32(0, 3);
                bb.set(node.key("AbilitySequence").unwrap_or("AbilitySequence"), BbValue::Int(i64::from(s)));
                Status::Succeeded
            }
            // `BTT_PickSummon`: one of its `Abilities` at random, as a class key.
            "BTT_PickSummon_C" => {
                let list: Vec<String> = node.props.get("Abilities").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
                if list.is_empty() {
                    return Status::Failed;
                }
                let pick = list[self.rng.range_u32(0, list.len() as u32) as usize].clone();
                let key = node.key("Summon").unwrap_or("Summon").to_string();
                self.ai.class_keys.retain(|k| k.0 != key);
                self.ai.class_keys.push((key, pick));
                Status::Succeeded
            }
            // `BTT_Wendigo_SelectAbilityTarget`: the highest threat (as `SetThreatTargetAsAbilityTarget`).
            "BTT_SetThreatTargetAsAbilityTarget_C" | "BTT_Wendigo_SelectAbilityTarget_C" => match self.target {
                Some(t) => {
                    bb.set(node.key("AbilityTarget").unwrap_or("AbilityTarget"), BbValue::Actor(t.id));
                    Status::Succeeded
                }
                None => Status::Failed,
            },
            // (CONFIRMED bytecode): within `AcceptanceRadius` (50 cm) of the destination, flat.
            "BTT_CheckReached_C" => match self.point(bb, node.key("DestinationBlackboardKey").unwrap_or("")) {
                Some(p) if (p - self.from).truncate().length() < node.f32("AcceptanceRadius").unwrap_or(50.0) => Status::Succeeded,
                _ => Status::Failed,
            },
            "BTT_PickAbility_C" => {
                if self.pick_ability(bb) {
                    Status::Succeeded
                } else {
                    Status::Failed
                }
            }
            "BTT_CheckRetreatOnDistance_C" => {
                // (CONFIRMED bytecode): a target nearer than `RetreatRange` -> `PickRetreat`.
                let Some(t) = self.target else { return Status::Failed };
                let range = self.kit.bp.retreat_range.unwrap_or(0.0);
                if range > 0.0
                    && (t.pos - self.from).length() < range
                    && let Some(i) = self.pick_retreat()
                {
                    bb.set("Retreat", BbValue::Bool(true));
                    bb.set("RetreatIndex", BbValue::Int(i as i64));
                }
                Status::Succeeded
            }
            "BTT_CheckActionOnAbility_C" => {
                // The controller's `PickAction` (CONFIRMED bytecode): nothing if an action is set; else
                // `RetreatChance` > RandomFloat -> `PickRetreat`; else, without a `FlankBlockedTags` tag,
                // `FlankChance` > RandomFloat -> flank. A kit without Blueprint values keeps the HYPOTHESIS
                // (melee kits flank at `FLANK_CHANCE`).
                if bb.bool("Flank") || bb.bool("Retreat") {
                    return Status::Succeeded;
                }
                if self.kit.bp.folder.is_empty() {
                    if self.kit.attacks.iter().any(|a| a.shot.is_none()) && self.rng.chance(tune::FLANK_CHANCE) {
                        bb.set("Flank", BbValue::Bool(true));
                    }
                    return Status::Succeeded;
                }
                let retreat = self.kit.bp.retreat_chance.unwrap_or(0.0);
                if retreat > self.rng.unit_f64()
                    && let Some(i) = self.pick_retreat()
                {
                    bb.set("Retreat", BbValue::Bool(true));
                    bb.set("RetreatIndex", BbValue::Int(i as i64));
                    return Status::Succeeded;
                }
                let blocked = self.kit.bp.flank_blocked_tags.iter().any(|t| self.tags.contains(&crate::buffs::gameplay_tag(t)));
                if !blocked && self.kit.bp.flank_chance.unwrap_or(0.0) > self.rng.unit_f64() {
                    bb.set("Flank", BbValue::Bool(true));
                }
                Status::Succeeded
            }
            "BTT_RetreatAbility_C" => {
                // (CONFIRMED bytecode): RandomFloat < the entry's `AbilityChance` -> its ability, started
                // directly (no rule check; its cooldown still applies).
                let i = bb.int("RetreatIndex").max(0) as usize;
                let Some(chance) = self.kit.bp.retreats.get(i).map(|e| e.ability_chance) else { return Status::Failed };
                if let Some(j) = self.kit.retreat_abilities.get(i).copied().flatten()
                    && self.rng.unit_f64() < chance
                    && !self.busy()
                    && self.ai.cooldowns.get(usize::from(j)).is_none_or(|&ready| now >= ready)
                {
                    self.input.dir = Vec2::ZERO;
                    self.melee.queued_montage = Some(j);
                    let cooldown = self.kit.attacks.get(usize::from(j)).map_or(0.0, |a| a.pick.cooldown);
                    if cooldown > 0.0 {
                        let j = usize::from(j);
                        if self.ai.cooldowns.len() <= j {
                            self.ai.cooldowns.resize(j + 1, 0);
                        }
                        self.ai.cooldowns[j] = now + ticks(cooldown);
                    }
                }
                Status::Succeeded
            }
            "BTT_SetAbilityAngleRange_C" => {
                // The ability rule's `ActivateAngleRange` where it sets one (the totems' 180, so a totem that
                // never turns still attacks), else the HYPOTHESIS default.
                let angle = self.kit.attacks.get(bb.int("Ability") as usize).and_then(|a| a.angle).unwrap_or(tune::ATTACK_FACING_DEG);
                bb.set("AbilityAngleRange", BbValue::Float(angle));
                Status::Succeeded
            }
            "BTT_SetAbilityActivateRange_C" => {
                let r = self.ability_range(bb);
                bb.set("AbilityActivateRange", BbValue::Float(r));
                Status::Succeeded
            }
            "BTT_AbilityBegin_C" => {
                if self.busy() || self.target.is_none() {
                    return Status::Failed;
                }
                self.input.dir = Vec2::ZERO;
                let i = bb.int("Ability") as u8;
                self.melee.queued_montage = Some(i);
                // Its cooldown effect starts with it.
                let cooldown = self.kit.attacks.get(usize::from(i)).map_or(0.0, |a| a.pick.cooldown);
                if cooldown > 0.0 {
                    let i = usize::from(i);
                    if self.ai.cooldowns.len() <= i {
                        self.ai.cooldowns.resize(i + 1, 0);
                    }
                    self.ai.cooldowns[i] = now + ticks(cooldown);
                }
                Status::Succeeded
            }
            "BTT_AbilityEnd_C" => {
                // The Blueprint's `AbilityInterval`, else the HYPOTHESIS pause, give or take its
                // `AbilityIntervalRandomDeviation` (CONFIRMED `BTT_AbilityEnd`).
                let dev = self.kit.bp.ability_interval_deviation.unwrap_or(0.0);
                let wait = self.kit.bp.ability_interval.unwrap_or(tune::RECOVER_TIME) + (self.rng.unit_f32() * 2.0 - 1.0) * dev;
                bb.set("AbilityActivateTime", BbValue::Float((now + ticks(wait.max(0.0))) as f32));
                Status::Succeeded
            }
            "BTT_ActivateAbilityNextTime_C" => {
                // `AddPriorAbility(Ability, 1)`: the next pick keeps this ability if it still passes.
                self.ai.prior = Some(bb.int("Ability") as u8);
                mem.until = now + ticks(0.3);
                Status::Running
            }
            "BTT_WaitAbilityComplete_C" => {
                self.input.dir = Vec2::ZERO;
                mem.until = ticks(node.f32("Delay").unwrap_or(0.0));
                Status::Running
            }
            "BTT_AlertSuspiciousEnd_C" => Status::Running,
            "BTT_FlankClear_C" => {
                bb.clear("Flank");
                Status::Succeeded
            }
            "BTT_FlankBegin_C" => self.flank_begin(bb),
            "BTT_RetreatClear_C" => {
                bb.clear("Retreat");
                Status::Succeeded
            }
            "BTT_FindRetreatLocation_C" => self.find_retreat(bb),
            "BTT_LeaveCombat_C" => {
                self.ai.target = None;
                self.set_evade(false);
                Status::Succeeded
            }
            "BTT_Goblin_Thief_FindRunAwayLocation_C" => self.find_run_away(node, bb),
            "BTT_Goblin_Thief_ActivateAbility_C" => self.thief_activate(node, bb, now),
            "BTT_Goblin_Thief_WaitEscapable_C" => self.tick(node, bb, mem, now),
            "BTT_Goblin_Thief_SelectWanderAbility_C" => {
                // `RandomIntegerInRange(1, 4)`.
                bb.set(node.key("WanderAbility").unwrap_or("WanderAbility"), BbValue::Int(1 + i64::from(self.rng.range_u32(0, 4))));
                Status::Succeeded
            }
            "BTT_SetWanderInterval_C" => {
                // The pawn's `WanderInterval` +- `WanderIntervalRandomDeviation`.
                let w = &self.kit.bp.wander_interval;
                let (base, dev) = (w.first().copied().flatten().unwrap_or(0.0), w.get(1).copied().flatten().unwrap_or(0.0));
                let v = base + (self.rng.unit_f32() * 2.0 - 1.0) * dev;
                bb.set(node.key("WanderInterval").unwrap_or("WanderInterval"), BbValue::Float(v.max(0.0)));
                Status::Succeeded
            }
            "BTT_SetWanderOnce_C" => {
                bb.set("WanderOnce", BbValue::Bool(true));
                Status::Succeeded
            }
            // (CONFIRMED bytecode): an evadable monster's chase result; the first always fails, the second
            // always succeeds.
            "BTT_ChaseFailed_C" => {
                if self.kit.bp.evadable() {
                    self.on_chase_failed(now);
                }
                Status::Failed
            }
            "BTT_CheckChaseResult_C" => {
                if self.kit.bp.evadable()
                    && let Some(t) = self.target.filter(|t| bb.actor(node.key("Target").unwrap_or("")) == Some(t.id))
                {
                    if self.full_path_to(t.pos) { self.on_chase_succeeded() } else { self.on_chase_failed(now) }
                }
                Status::Succeeded
            }
            "BTT_FindFleePath_C" => self.find_flee_path(node, bb),
            "BTT_FleeBegin_C" => {
                let Some(&first) = self.ai.flee_path.first() else { return Status::Failed };
                bb.set(node.key("FleePointIndex").unwrap_or("FleePointIndex"), BbValue::Int(0));
                bb.set(node.key("FleePoint").unwrap_or("FleePoint"), BbValue::Vector(first));
                bb.set(node.key("FleeCompleted").unwrap_or("FleeCompleted"), BbValue::Bool(false));
                Status::Succeeded
            }
            "BTT_FleeReachPoint_C" => {
                // `FleePointHoldTimeMin..Max` there, then the next point (or done).
                let f = &self.kit.bp.flee;
                let (lo, hi) = (f.get(7).copied().flatten().unwrap_or(0.5), f.get(8).copied().flatten().unwrap_or(1.0));
                self.input.dir = Vec2::ZERO;
                mem.until = now + ticks(lo + (hi - lo).max(0.0) * self.rng.unit_f32());
                Status::Running
            }
            "BTT_FleeEnd_C" => Status::Succeeded,
            "BTT_PatrolBegin_C" => self.patrol_begin(node, bb),
            "BTT_PatrolReachPoint_C" => {
                let i = bb.int(node.key("PatrolPointIndex").unwrap_or("PatrolPointIndex")).max(0) as usize;
                let wait = self.ai.patrol.as_ref().and_then(|p| p.points.get(i)).map_or(0.0, |p| p.1);
                // `WaitTime` at the point first (`Delay`), then on to the next.
                if wait > 0.0 {
                    self.input.dir = Vec2::ZERO;
                    mem.until = now + ticks(wait);
                    return Status::Running;
                }
                self.patrol_next(node, bb)
            }
            "BTT_PatrolEnd_C" => Status::Succeeded,
            "BTT_SetWanderBaseLocation_C" => {
                bb.set(node.key("WanderBaseLocation").unwrap_or("WanderBaseLocation"), BbValue::Vector(self.from));
                Status::Succeeded
            }
            "BTT_FindWanderLocation_C" => self.find_wander(node, bb),
            "BTT_WanderIdle_C" => {
                // `WanderIdleChance`: wait `WanderIdleDelay` +- its deviation, then (HYPOTHESIS: not played) the
                // pawn's `WanderIdleAbility`.
                let w = &self.kit.bp.wander_idle;
                let (chance, delay, dev) = (w.first().copied().flatten().unwrap_or(0.0), w.get(1).copied().flatten().unwrap_or(0.0), w.get(2).copied().flatten().unwrap_or(0.0));
                if self.rng.unit_f32() >= chance {
                    return Status::Succeeded;
                }
                self.input.dir = Vec2::ZERO;
                mem.until = now + ticks((delay + (self.rng.unit_f32() * 2.0 - 1.0) * dev).max(0.0));
                Status::Running
            }
            "BTT_MoveTo_C" | "BTT_FlyTo_C" | "BTT_FaceTo_C" | "BTT_AbilityInterval_C" | "BTT_Hold_C" | "BTT_Stun_C" | "BTT_HoverFly_Monster_Inscet_Fly_C" | "BTT_RiseFly_Monster_Inscet_Fly_C" | "BTT_FeintFly_Monster_Inscet_Fly_C" | "BTT_FlyTo_Monster_Inscet_Fly_C" => {
                mem.last_pos = self.from;
                self.tick(node, bb, mem, now)
            }
            // Bookkeeping nodes and ones whose effect the sim doesn't model yet succeed.
            _ => Status::Succeeded,
        }
    }

    fn tick(&mut self, node: &Node, bb: &mut Blackboard, mem: &mut TaskMem, now: u32) -> Status {
        self.ai.clock = now;
        match node.class.as_str() {
            "BTT_MoveTo_C" | "BTT_FlyTo_C" | "BTT_HoverFly_Monster_Inscet_Fly_C" | "BTT_RiseFly_Monster_Inscet_Fly_C" | "BTT_FeintFly_Monster_Inscet_Fly_C" | "BTT_FlyTo_Monster_Inscet_Fly_C" => self.move_to(node, bb, mem, now),
            "BTT_FaceTo_C" => self.face_to(node, bb),
            "BTT_AbilityInterval_C" => {
                self.input.dir = Vec2::ZERO;
                if let Some(p) = self.point(bb, "AbilityTarget") {
                    self.turn_to(p);
                }
                if bb.float("AbilityActivateTime").is_none_or(|t| now as f32 >= t) { Status::Succeeded } else { Status::Running }
            }
            "BTT_ActivateAbilityNextTime_C" | "BTT_WanderIdle_C" => {
                if now >= mem.until { Status::Succeeded } else { Status::Running }
            }
            "BTT_PatrolReachPoint_C" => {
                self.input.dir = Vec2::ZERO;
                if now >= mem.until { self.patrol_next(node, bb) } else { Status::Running }
            }
            "BTT_FleeReachPoint_C" => {
                self.input.dir = Vec2::ZERO;
                if now < mem.until {
                    return Status::Running;
                }
                let key = node.key("FleePointIndex").unwrap_or("FleePointIndex");
                let next = bb.int(key) as usize + 1;
                match self.ai.flee_path.get(next) {
                    Some(&p) => {
                        bb.set(key, BbValue::Int(next as i64));
                        bb.set(node.key("FleePoint").unwrap_or("FleePoint"), BbValue::Vector(p));
                    }
                    None => bb.set(node.key("FleeCompleted").unwrap_or("FleeCompleted"), BbValue::Bool(true)),
                }
                Status::Succeeded
            }
            "BTT_WaitAbilityComplete_C" => {
                self.input.dir = Vec2::ZERO;
                if self.busy() {
                    mem.stuck = 0;
                    return Status::Running;
                }
                // Free of the ability for `Delay` s.
                mem.stuck += 1;
                if mem.stuck > mem.until { Status::Succeeded } else { Status::Running }
            }
            "BTT_AlertSuspiciousEnd_C" => {
                let since = bb.float("AlertStateTime").unwrap_or(now as f32);
                if now as f32 - since > ticks(tune::ALERT_SEARCH_END_TIME) as f32 {
                    bb.set(node.key("AlertCompleted").unwrap_or("AlertCompeleted"), BbValue::Bool(true));
                    Status::Succeeded
                } else {
                    Status::Running
                }
            }
            "BTT_Hold_C" | "BTT_Stun_C" => {
                self.input.dir = Vec2::ZERO;
                Status::Running
            }
            "BTT_Goblin_Thief_WaitEscapable_C" => {
                // `IsEscapable` (`EscapeTime` since the reveal) and the portal ability off cooldown.
                self.input.dir = Vec2::ZERO;
                let Some(t) = self.kit.thief.as_deref() else { return Status::Failed };
                let open = node.key("AbilityOpenPortal").and_then(|c| t.ability(c));
                let escapable = self.ai.thief.revealed_at.is_some_and(|r| now.saturating_sub(r) as f32 * DT > t.escape_time);
                if escapable && open.is_some_and(|i| self.ai.thief.ready(i, now)) { Status::Succeeded } else { Status::Running }
            }
            _ => Status::Succeeded,
        }
    }

    fn abort(&mut self, node: &Node, _bb: &mut Blackboard) {
        if matches!(node.class.as_str(), "BTT_MoveTo_C" | "BTT_FlyTo_C" | "BTT_FaceTo_C") || node.class.ends_with("Fly_Monster_Inscet_Fly_C") || node.class == "BTT_FlyTo_Monster_Inscet_Fly_C" {
            self.input.dir = Vec2::ZERO;
        }
    }

    fn service(&mut self, node: &Node, bb: &mut Blackboard, event: ServiceEvent, now: u32) {
        self.ai.clock = now;
        match (node.class.as_str(), event) {
            ("BTS_CheckInAbilityRange_C", ServiceEvent::Tick) => {
                // An evading monster counts as in range (CONFIRMED bytecode): it acts from where it stands.
                let v = self.ai.evade || self.in_ability_range(bb);
                bb.set("InAbilityRange", BbValue::Bool(v));
            }
            // (CONFIRMED bytecode): evading ends once a whole path to the ability target exists.
            ("BTS_CheckEvade_C", ServiceEvent::Tick) => {
                if self.ai.evade
                    && let Some(t) = self.target.filter(|t| bb.actor(node.key("AbilityTarget").unwrap_or("AbilityTarget")) == Some(t.id))
                    && self.full_path_to(t.pos)
                {
                    self.set_evade(false);
                }
            }
            ("BTS_CheckInCombatZone_C", ServiceEvent::Tick) => {
                let inside = (self.from - self.ai.home).truncate().length() <= tune::COMBAT_ZONE_RADIUS;
                bb.set("InCombatZone", BbValue::Bool(inside));
            }
            ("BTS_RepickAbility_C", ServiceEvent::Tick) => {
                // `TGF.GameplayEvent.AI.RepickAbility.Begin` first (the Wendigo's `PickDash`).
                if let Some(npc) = self.kit.npc.as_deref() {
                    for b in &npc.on_repick {
                        crate::npc_state::apply(b, bb, &mut self.ai.prior, self.kit);
                    }
                }
                self.pick_ability(bb);
            }
            ("BTS_Alert_C", ServiceEvent::Activate) => bb.set("AlertCompeleted", BbValue::Bool(false)),
            ("BTS_Alert_C", ServiceEvent::Deactivate) => bb.set("AlertState", BbValue::Int(0)),
            ("BTS_AlertSearch_C", ServiceEvent::Activate) => {
                // `AlertSearchDistance / speed`; the distance isn't decoded (HYPOTHESIS).
                bb.set("AlertSearchTimeLimit", BbValue::Float(tune::ALERT_SEARCH_DISTANCE / tune::ALERT_SEARCH_SPEED));
            }
            ("BTS_AlertSearch_C", ServiceEvent::Deactivate) => {
                if bb.int("AlertState") == 2 {
                    bb.set("AlertState", BbValue::Int(1));
                    bb.set("AlertStateTime", BbValue::Float(now as f32));
                }
            }
            ("BTS_TurnQuickly_C", ServiceEvent::Activate) => self.ai.turn_factor = 1.0,
            ("BTS_TurnSlowly_C", ServiceEvent::Activate) => self.ai.turn_factor = tune::TURN_SLOW_FACTOR,
            ("BTS_TurnQuickly_C" | "BTS_TurnSlowly_C", ServiceEvent::Deactivate) => self.ai.turn_factor = 1.0,
            ("BTS_WanderOnce_C", ServiceEvent::Deactivate) => bb.clear("WanderOnce"),
            // `SetFleed`: it flees once.
            ("BTS_Flee_C", ServiceEvent::Deactivate) => self.ai.flee_state = 2,
            // `ApplySpeedMod(SpeedTagLow)`: `SpeedLow` while patrolling or wandering.
            ("BTS_Patrol_C" | "BTS_Wander_C", ServiceEvent::Activate) => self.input.speed = self.kit.bp.speed_low.filter(|s| *s > 0.0),
            ("BTS_Patrol_C" | "BTS_Wander_C", ServiceEvent::Deactivate) => self.input.speed = None,
            ("BTS_Flank_C", ServiceEvent::Activate) => {
                self.ai.focus = true;
                // `ApplySpeedMod(SpeedTagFlank)`: `FlankSpeed` while flanking.
                self.input.speed = self.kit.bp.flank_speed.filter(|s| *s > 0.0);
            }
            ("BTS_Flank_C", ServiceEvent::Deactivate) => {
                self.ai.focus = false;
                self.input.speed = None;
            }
            ("BTS_Retreat_C", ServiceEvent::Activate) => {
                // (CONFIRMED bytecode): the entry's `Speed` (when above 0) and, with `FocusTarget`, facing the
                // target while it backs off.
                let e = self.retreat_entry(bb).cloned();
                self.input.speed = e.as_ref().map(|e| e.speed).filter(|s| *s > 0.0);
                self.ai.focus = e.is_some_and(|e| e.focus_target);
            }
            ("BTS_Retreat_C", ServiceEvent::Deactivate) => {
                self.input.speed = None;
                self.ai.focus = false;
            }
            // The thief's: `Escaping` = no player within `SafeRange`, or an escape ability running
            // (`BlockInterrupt`).
            ("BTS_Goblin_Thief_Escaping_C", ServiceEvent::Tick) => {
                let safe = self.kit.thief.as_deref().is_some_and(|t| self.nearest_player > t.safe_range);
                bb.set(node.key("Escaping").unwrap_or("Escaping"), BbValue::Bool(safe || self.ai.thief.active.is_some()));
            }
            // Leaving the branch interrupts the ability it started (`InterruptAbilityByClass`).
            ("BTS_Goblin_Thief_ActivatingAbility_C", ServiceEvent::Deactivate) => {
                let started = bb.int(node.key("AbilityActivating").unwrap_or("AbilityActivating"));
                if let Some(a) = self.ai.thief.active
                    && a.ability as i64 == started
                    && !self.ai.thief.past_touch
                {
                    self.ai.thief.active = None;
                }
            }
            _ => {}
        }
    }
}

impl MonsterHost<'_, '_, '_> {
    fn now_hint(&self) -> u32 {
        self.ai.clock
    }

    /// The thief's ability in progress: it stands; OpenPortal's `Open` notify puts the portal `PORTAL_AHEAD`
    /// cm ahead (less if a wall is nearer); EnterPortal's `Touch` closes it and `Enter` takes the thief
    /// away.
    fn thief_step(&mut self, now: u32) {
        let Some(kit) = self.kit.thief.as_deref() else { return };
        let Some(mut act) = self.ai.thief.active else { return };
        let a = &kit.abilities[act.ability];
        let t = now.saturating_sub(act.start) as f32 * DT;
        self.input.dir = Vec2::ZERO;
        if let Some(open) = a.open
            && !act.fired
            && t >= open
        {
            let fwd = self.aim.forward_2d().extend(0.0);
            let ahead = self.free_distance(fwd, crate::thief::PORTAL_AHEAD);
            let floor = self.from + fwd * ahead - Vec3::Z * crate::movement::CAPSULE_HEIGHT * 0.5;
            self.ai.thief.requests.push(crate::thief::Request::OpenPortal(floor));
            act.fired = true;
        }
        if let Some(touch) = a.touch
            && !act.fired
            && t >= touch
        {
            self.ai.thief.past_touch = true;
            self.ai.thief.requests.push(crate::thief::Request::ClosePortal);
            act.fired = true;
        }
        if a.enter.is_some_and(|enter| t >= enter) {
            self.ai.thief.escaped = true;
            self.ai.thief.active = None;
            self.ai.thief.requests.push(crate::thief::Request::Escape);
            return;
        }
        self.ai.thief.active = (t < a.length).then_some(act);
    }

    /// How far (cm, up to `max`) the monster can see along `dir` before level geometry, at eye height.
    fn free_distance(&self, dir: Vec3, max: f32) -> f32 {
        let Ok(d) = Dir3::new(dir) else { return 0.0 };
        let boxes = self.boxes;
        let eye = self.from + Vec3::Z * tune::EYE_HEIGHT;
        self.spatial.cast_ray_predicate(eye, d, max, true, &crate::layers::walk(), &|e| boxes.contains(e)).map_or(max, |h| h.distance)
    }

    /// `BTT_Goblin_Thief_FindRunAwayLocation` (bytecode): the kept `RunAwayDirection`, else away from the
    /// threat; tried as is, then turned -30, +30, -60... -180 degrees; the first heading with a point
    /// `Distance` ahead wins and is kept. The game asks the navmesh for a reachable point near it
    /// (`GetRandomReachablePointInRadius`, radii 50-500 cm) and a path; here (HYPOTHESIS until 's nav
    /// queries) a heading is free if level geometry is at least half the distance away, and the point
    /// stops short of it.
    fn find_run_away(&mut self, node: &Node, bb: &mut Blackboard) -> Status {
        let Some(kit) = self.kit.thief.as_deref() else { return Status::Failed };
        let (dir_key, loc_key) = (node.key("RunAwayDirection").unwrap_or("RunAwayDirection"), node.key("RunAwayLocation").unwrap_or("RunAwayLocation"));
        let kept = match bb.get(dir_key) {
            Some(BbValue::Vector(v)) if v.truncate().length() > 0.5 => Some(v.truncate().normalize()),
            _ => None,
        };
        let Some(base) = kept.or_else(|| self.target.map(|t| (self.from - t.pos).truncate().normalize_or_zero())).filter(|d| *d != Vec2::ZERO) else { return Status::Failed };
        let dist = kit.run_distance;
        for i in 1..=12 {
            let turn = (i / 2) as f32 * 30.0 * if i % 2 == 1 { 1.0 } else { -1.0 };
            let d = Vec2::from_angle(turn.to_radians()).rotate(base);
            let free = self.free_distance(d.extend(0.0), dist);
            if free >= dist * 0.5 {
                let reach = if free >= dist { dist } else { free - CAPSULE_RADIUS * 2.0 };
                bb.set(dir_key, BbValue::Vector(d.extend(0.0)));
                bb.set(loc_key, BbValue::Vector(self.from + d.extend(0.0) * reach));
                return Status::Succeeded;
            }
        }
        Status::Failed
    }

    /// `BTT_ActivateAbilityDirectly`: `TryActivateAbilityByClass(Ability)`: its attack starts unless another is
    /// under way (then it fails); its cooldown starts with it. No target needed.
    fn activate_directly(&mut self, node: &Node, bb: &mut Blackboard, now: u32) -> Status {
        // No `Ability`: the class in its `AbilityKey` (the Wendigo's picked `Summon`).
        let keyed = node.key("AbilityKey").and_then(|k| self.ai.class_keys.iter().find(|c| c.0 == k)).map(|c| c.1.clone());
        let class = node.key("Ability").map(str::to_string).or(keyed).unwrap_or_default();
        let class = class.as_str();
        if tune::MISSING_ABILITIES.contains(&class) {
            return Status::Succeeded;
        }
        let Some(i) = self.kit.by_class(class) else { return Status::Failed };
        if self.busy() {
            return Status::Failed;
        }
        self.input.dir = Vec2::ZERO;
        self.melee.queued_montage = Some(i);
        bb.set("Ability", BbValue::Int(i64::from(i)));
        let cooldown = self.kit.attacks.get(usize::from(i)).map_or(0.0, |a| a.pick.cooldown);
        if cooldown > 0.0 {
            let i = usize::from(i);
            if self.ai.cooldowns.len() <= i {
                self.ai.cooldowns.resize(i + 1, 0);
            }
            self.ai.cooldowns[i] = now + ticks(cooldown);
        }
        Status::Succeeded
    }

    /// The Shadow King's own tasks (CONFIRMED bytecode). A key his tree points at `SelfActor` (`MoveTimesClear`'s
    /// `MoveTimes`, `CanNotMoveToCentre`'s `MoveState`) is an object key: setting an int or bool on it does nothing,
    /// as in the game.
    fn king_task(&mut self, node: &Node, bb: &mut Blackboard, now: u32) -> Status {
        let Some(k) = self.kit.king.as_deref() else { return Status::Failed };
        let set = |bb: &mut Blackboard, name: &str, v: BbValue| {
            let key = node.key(name).unwrap_or(name);
            if key != "SelfActor" {
                bb.set(key, v);
            }
        };
        let key = |name: &str| node.key(name).unwrap_or(name).to_string();
        let after = crate::shadow_king::after;
        match node.class.as_str() {
            "BTT_Shadow_King_WakeUp_C" => {
                set(bb, "Phase", BbValue::Int(1));
                set(bb, "PhaseState", BbValue::Int(1));
                set(bb, "MoveTimes", BbValue::Int(0));
                set(bb, "MoveState", BbValue::Bool(true));
            }
            "BTT_Shadow_King_PhaseCheck_C" => {
                // 1 -> 2 under `Phase1HP%`, 2 -> 3 when `Phase2Timer` runs out, 3 -> 4 under `Phase3HP%`; only once the
                // phase has settled (`PhaseState` 2), each into its entry state (1).
                let (phase, state) = (bb.int(&key("Phase")), bb.int(&key("PhaseState")));
                let next = match phase {
                    1 if self.health < k.phase1_hp => Some(2),
                    2 if now >= self.ai.king.phase2 => Some(3),
                    3 if self.health < k.phase3_hp => Some(4),
                    _ => None,
                };
                if state == 2
                    && let Some(n) = next
                {
                    set(bb, "Phase", BbValue::Int(n));
                    set(bb, "PhaseState", BbValue::Int(1));
                }
            }
            "BTT_Shadow_King_PhaseStateChange_C" => set(bb, "PhaseState", BbValue::Int(2)),
            "BTT_Shadow_King_Phase2TimeIn_C" => self.ai.king.phase2 = after(now, k.phase2),
            "BTT_Shadow_King_Attack5TimeInP1_C" => self.ai.king.attack5 = after(now, k.attack5[0]),
            "BTT_Shadow_King_Attack5TimeInP2_C" => self.ai.king.attack5 = after(now, k.attack5[1]),
            "BTT_Shadow_King_Attack5TimeInP3_C" => self.ai.king.attack5 = after(now, k.attack5[2]),
            "BTT_Shadow_King_Attack5TimeInP4_C" => self.ai.king.attack5 = after(now, k.attack5[3]),
            "BTT_Shadow_King_Attack7TimeInP2_C" => self.ai.king.attack7 = after(now, k.attack7[0]),
            "BTT_Shadow_King_Attack7TimeInP4_C" => self.ai.king.attack7 = after(now, k.attack7[1]),
            "BTT_Shadow_King_Attack9TimeInP3_C" => self.ai.king.attack9 = after(now, k.attack9[0]),
            "BTT_Shadow_King_Attack9TimeInP4_C" => self.ai.king.attack9 = after(now, k.attack9[1]),
            "BTT_Shadow_King_CenterSet_C" => {
                // `Get Location by Spawn Offset`: his spawn point (+80 up for the move; HYPOTHESIS: his home is the point).
                let home = self.ai.home;
                set(bb, "CentreLocation", BbValue::Vector(home + Vec3::Z * 80.0));
                set(bb, "CentreRotation", BbValue::Vector(home));
            }
            "BTT_Shadow_King_CanNotMoveToCentre_C" => {
                let n = bb.int(&key("MoveTimes"));
                set(bb, "MoveTimes", BbValue::Int(n + 1));
                set(bb, "MoveState", BbValue::Bool(false));
                self.ai.king.moved = after(now, k.move_cd);
            }
            "BTT_Shadow_King_MoveTimesClear_C" => set(bb, "MoveTimes", BbValue::Int(0)),
            "BTT_Shadow_King_SelectSummonSkill_C" => set(bb, "SummonSkillIndex", BbValue::Int(i64::from(self.rng.range_u32(1, 101)))),
            _ => return Status::Failed,
        }
        Status::Succeeded
    }

    /// `BTT_Goblin_Thief_ActivateAbility`: `TryActivateAbilityByClass`, remembering it in
    /// `AbilityActivating`. Fails while another runs or on cooldown (committed on activation). EnterPortal
    /// without a portal ends at once.
    fn thief_activate(&mut self, node: &Node, bb: &mut Blackboard, now: u32) -> Status {
        let Some(kit) = self.kit.thief.as_deref() else { return Status::Failed };
        let Some(i) = node.key("Ability").and_then(|c| kit.ability(c)) else { return Status::Failed };
        if self.ai.thief.active.is_some() || !self.ai.thief.ready(i, now) {
            return Status::Failed;
        }
        let a = &kit.abilities[i];
        if a.cooldown > 0.0 {
            self.ai.thief.ready_at.retain(|r| r.0 != i);
            self.ai.thief.ready_at.push((i, now + ticks(a.cooldown)));
        }
        bb.set(node.key("AbilityActivating").unwrap_or("AbilityActivating"), BbValue::Int(i as i64));
        self.input.dir = Vec2::ZERO;
        if a.enter.is_some() && self.ai.thief.portal.is_none() {
            return Status::Succeeded;
        }
        self.ai.thief.active = Some(crate::thief::Active { ability: i, start: now, fired: false });
        Status::Succeeded
    }
}

/// True if level geometry (`LevelBox`) lies between two points (cm). For presentation too: the client
/// hides labels behind walls with it.
pub fn wall_between(spatial: &SpatialQuery, boxes: &Query<(), With<crate::command::LevelBox>>, a: Vec3, b: Vec3) -> bool {
    let Ok(dir) = Dir3::new(b - a) else { return false };
    spatial.cast_ray_predicate(a, dir, (b - a).length(), true, &crate::layers::shots(), &|e| boxes.contains(e)).is_some()
}

/// Nothing but the target between the monster's and the target's eye points (level boxes block).
fn line_of_sight(spatial: &SpatialQuery, me: Entity, target: Entity, from: Vec3, to: Vec3) -> bool {
    let eye = Vec3::Z * tune::EYE_HEIGHT;
    let (a, b) = (from + eye, to + eye);
    let Ok(dir) = Dir3::new(b - a) else { return true };
    let dist = (b - a).length();
    let filter = crate::layers::shots().with_excluded_entities([me]);
    match spatial.cast_ray(a, dir, dist, true, &filter) {
        None => true,
        Some(hit) => hit.entity == target,
    }
}

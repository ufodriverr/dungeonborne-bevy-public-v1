//! Dev/check (`--auto-run`): plays the whole loop unattended for the check. Title → class pick
//! (Fighter) → stash → run 1: walk to a floor-1 chest, open it, pick up, walk to the portal down, walk
//! to an extraction portal on floor 2 → result → stash → run 2 starts. Screenshots at each step, then
//! exit. Menus go through `flow::Action` like the buttons; in the run the bot steers like a player
//! (move direction and look) and interacts through commands. Walking follows the room graph
//! (`Floor::route`) and L-shaped legs that avoid pillars, or on the real crypt floors the floors'
//! walkable grids (`db_sim::crypt::route`).

use bevy::prelude::*;
use db_formats::config::generated::EnRoleCareerType;
use db_sim::crypt::CryptMaps;
use db_sim::dungeon::{Dungeon, Floor, PortalKind};
use db_sim::level::Surface;
use db_sim::{ActorKind, CommandQueue, Kind, Position, SimCommand, SimEvent, SimId, SimIndex};

use crate::Look;
use crate::combat_view::{FrameEvents, LocalPlayer};
use crate::flow::{Action, PendingAction, Screen, ShotRequests};

#[derive(Resource, Debug, Default)]
pub struct AutoRun(pub bool);

/// `--shot-every <s>` (dev): during a run the bot also saves `walk_NN` shots this often.
#[derive(Resource, Debug, Default)]
pub struct AutoRunShots(pub Option<f32>);

/// How near a monster targeting the bot must be for it to stop and fight back (cm; ours, about a sword's reach).
const FIGHT_REACH: f32 = 250.0;

/// Far chests a `--shot-every` run walks to before leaving.
const TOUR_CHESTS: u32 = 4;

/// The bot's move direction (sim x/y, length ≤ 1), used by `send_input` instead of the keys.
#[derive(Resource, Debug, Default)]
pub struct BotMove(pub Option<Vec2>);

/// The dungeon of the current run (set by `dungeon_view::build`).
#[derive(Resource, Debug, Default)]
pub struct CurrentDungeon(pub Option<Dungeon>);

#[derive(Debug, Default, Clone, PartialEq)]
enum Goal {
    #[default]
    None,
    /// Walk the points, then do `then`.
    Walk(Vec<Vec2>, Box<Goal>),
    OpenChest(Vec3),
    Pick(Vec<SimId>),
    Portal(PortalKind),
    /// pull the lever `.0` of the gate `.1` that shut the walk, wait for the gate, then walk `.2` and do `.3`; `.4`
    /// once pulled.
    PullLever(SimId, SimId, Vec<Vec2>, Box<Goal>, bool),
    WaitFloor2,
    /// Just arrived on floor 2 (seconds there).
    Arrived,
    WaitEnd,
}

#[derive(Debug, Default)]
pub(crate) struct Bot {
    runs_started: u32,
    /// The monster it last stood and fought, for the log.
    fighting: Option<SimId>,
    /// Seconds on this screen.
    on_screen: f32,
    /// Seconds in the current goal step (reset by steps).
    timer: f32,
    shots: Vec<&'static str>,
    /// Seconds since the last timed walk shot, and how many were taken (`--shot-every`).
    walk_shot: (f32, u32),
    /// `--shot-every` tour: more far chests to walk to before leaving, and the chests visited.
    tour_left: Option<u32>,
    visited: Vec<Vec3>,
    goal: Goal,
    stuck: (Vec2, f32),
    /// Unsticking: seconds left of a sidestep, its side (+1 left, -1 right), and the tries at the
    /// current point.
    sidestep: (f32, f32, u32),
    /// The Q skill was switched once on the stash screen (Whirlwind → War Cry).
    picked_skill: bool,
    picked_class: bool,
    /// An action was sent on this screen (one per screen).
    acted: bool,
    /// Seconds since the bot last tried a door.
    door_wait: f32,
    /// seconds since the bot last looked for a shut gate ahead, and the gates it has gone to pull open.
    gate_check: f32,
    gates_tried: Vec<SimId>,
    /// Frames spent paused in a run (time stands still there).
    paused_frames: u32,
}

fn shot(bot: &mut Bot, shots: &mut ShotRequests, name: &'static str) -> bool {
    if bot.shots.contains(&name) {
        return false;
    }
    bot.shots.push(name);
    shots.queue.push_back(name.to_string());
    true
}

/// Pillar footprints (min, max) of a floor.
fn pillars(f: &Floor) -> Vec<(Vec2, Vec2)> {
    let h = db_sim::overrides::dungeon::PILLAR_HALF;
    f.pieces.iter().filter(|p| p.surface == Surface::Wall && p.half_extents.x == h && p.half_extents.y == h).map(|p| (p.center.truncate() - h, p.center.truncate() + h)).collect()
}

fn segment_clear(a: Vec2, b: Vec2, boxes: &[(Vec2, Vec2)]) -> bool {
    let n = (a.distance(b) / 10.0).ceil().max(1.0) as usize;
    (0..=n).all(|i| {
        let p = a.lerp(b, i as f32 / n as f32);
        boxes.iter().all(|(lo, hi)| ((p - (*lo + *hi) * 0.5).abs() - (*hi - *lo) * 0.5).max_element() > 50.0)
    })
}

/// From a room centre to a spot in the same room: straight if clear, else an L either way round.
fn l_path(from: Vec2, to: Vec2, boxes: &[(Vec2, Vec2)]) -> Vec<Vec2> {
    if segment_clear(from, to, boxes) {
        return vec![to];
    }
    for corner in [Vec2::new(to.x, from.y), Vec2::new(from.x, to.y)] {
        if segment_clear(from, corner, boxes) && segment_clear(corner, to, boxes) {
            return vec![corner, to];
        }
    }
    vec![to]
}

/// Route from the room at `me` to `target` inside room `room`, ending `stop` cm short.
fn plan(f: &Floor, me: Vec2, target: Vec2, stop: f32) -> Option<Vec<Vec2>> {
    let (a, b) = (f.room_at(me)?, f.room_at(target)?);
    let mut pts = f.route(a, b)?;
    let centre = *pts.last()?;
    pts.extend(l_path(centre, target, &pillars(f)));
    // Stop short of the target (along the last leg).
    if stop > 0.0 && pts.len() >= 2 {
        let n = pts.len();
        let dir = (pts[n - 1] - pts[n - 2]).normalize_or_zero();
        let back = stop.min(pts[n - 1].distance(pts[n - 2]) * 0.5);
        pts[n - 1] -= dir * back;
    }
    Some(pts)
}

/// Route from capsule centre `me` to floor point `target`, ending `stop` cm short: over the crypt's walkable
/// grid on the real floors, else through the generated floor's rooms. What a grid point inside a sleeping or
/// idle monster's reach costs the bot's walk (cm, ours): a long way round rather than past them.
const AVOID_COST: f32 = 2000.0;
/// The reach the bot keeps from an awake monster with no target (cm, ours); a sleeping one's is its wake radius.
const AVOID_IDLE: f32 = 600.0;

/// Areas on the floor: world centre and radius (cm).
type Discs = [(Vec3, f32)];

/// A walk's cost in grid points: its length, with each point inside one of the `avoid` discs costing as the route
/// search weighs it (`AVOID_COST`), so a short walk into a pack loses to a longer clear one.
fn walk_score(pts: &[Vec2], avoid: &[(Vec3, f32)]) -> usize {
    let inside = pts.iter().filter(|p| avoid.iter().any(|&(c, r)| c.truncate().distance(**p) < r)).count();
    pts.len() + inside * (AVOID_COST / 100.0) as usize
}

fn route_to(crypt: Option<&CryptMaps>, avoid: &[(Vec3, f32)], f: &Floor, me: Vec3, target: Vec3, stop: f32) -> Option<Vec<Vec2>> {
    let Some(maps) = crypt else { return plan(f, me.truncate(), target.truncate(), stop) };
    let feet = me - Vec3::Z * (db_sim::overrides::dungeon::CAPSULE_CENTER_Z + 2.0);
    let mut pts = db_sim::crypt::route_avoiding(maps, f.index as usize, feet, target, avoid, AVOID_COST)?;
    let goal = target.truncate();
    while pts.len() > 1 && pts[pts.len() - 1].distance(goal) < stop {
        pts.pop();
    }
    Some(pts)
}

/// The doors' open state: the generated dungeon's and the maps' swing doors, the lever mechanisms and their
/// levers.
type DoorStates<'w, 's> = Query<'w, 's, (Option<&'static db_sim::Door>, Option<&'static db_sim::swing_doors::SwingDoor>, Option<&'static db_sim::mechanisms::Mechanism>, Option<&'static db_sim::mechanisms::Lever>)>;

/// Whether a mechanism stands in the way: shut for most, on for the ones that block on.
fn gate_shut(m: &db_sim::mechanisms::Mechanism) -> bool {
    m.on == m.blocks_on()
}

/// A shut gate with levers within the next 4 m of the walk (from `me` through `pts`): its id and its levers.
fn gate_ahead(index: &SimIndex, doors: &DoorStates, me: Vec3, pts: &[Vec2], tried: &[SimId]) -> Option<(SimId, Vec<SimId>)> {
    let mut samples = Vec::new();
    let mut at = me.truncate();
    let mut left = 400.0_f32;
    for &p in pts {
        let d = at.distance(p);
        let n = (d.min(left) / 20.0).ceil() as usize;
        samples.extend((1..=n).map(|i| at + (p - at) * ((i as f32 * 20.0).min(d) / d.max(1e-3))));
        left -= d;
        at = p;
        if left <= 0.0 {
            break;
        }
    }
    let gates = index.iter().filter(|(id, _)| !tried.contains(id)).filter_map(|(id, e)| doors.get(e).ok().and_then(|q| q.2).filter(|m| gate_shut(m)).map(|m| (id, m)));
    for (gid, m) in gates {
        let blocks = m.placement.parts.iter().filter(|p| p.kind != 2).any(|part| {
            let (c, yaw, half) = m.part_box(m.placement.pos, part, m.t);
            let inv = Quat::from_rotation_z(-yaw);
            // (Within a capsule's reach, at the walker's height.)
            (c.z - me.z).abs() < half.z + 100.0 && samples.iter().any(|s| { let d = inv * (s.extend(c.z) - c); d.x.abs() < half.x + 35.0 && d.y.abs() < half.y + 35.0 })
        });
        if !blocks {
            continue;
        }
        let levers: Vec<SimId> = index.iter().filter(|(_, e)| doors.get(*e).ok().and_then(|q| q.3).is_some_and(|l| l.mechanism == gid)).map(|(id, _)| id).collect();
        if !levers.is_empty() {
            return Some((gid, levers));
        }
    }
    None
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn auto_run(
    auto: Res<AutoRun>,
    time: Res<Time>,
    screen: Res<State<Screen>>,
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    (dungeon, crypt, paused, cover, every): (Res<CurrentDungeon>, Res<crate::CurrentCrypt>, Res<crate::pause::Paused>, Res<crate::flow::EntryCover>, Option<Res<AutoRunShots>>),
    actors: Query<(&Kind, &Position, Option<&db_sim::Portal>, Option<&db_sim::RunStatus>)>,
    (doors, probe, monsters, traps): (DoorStates, Query<Probe>, Query<(&db_sim::MonsterAi, &Position, &db_sim::Health)>, Query<(&db_sim::features::Trap, &Position)>),
    mut look: ResMut<Look>,
    mut bot_move: ResMut<BotMove>,
    mut pending: ResMut<PendingAction>,
    mut shots: ResMut<ShotRequests>,
    mut queue: ResMut<CommandQueue>,
    mut bot: Local<Bot>,
    mut last_screen: Local<Option<Screen>>,
) {
    if !auto.0 {
        return;
    }
    let now = *screen.get();
    if now == Screen::Run
        && let Some(every) = every.as_ref().and_then(|s| s.0)
    {
        // A tour across the map: after the first chest, the farthest reachable ones.
        bot.tour_left.get_or_insert(TOUR_CHESTS);
        bot.walk_shot.0 += time.delta_secs();
        if bot.walk_shot.0 >= every {
            bot.walk_shot = (0.0, bot.walk_shot.1 + 1);
            shots.queue.push_back(format!("walk_{:02}", bot.walk_shot.1));
        }
    }
    if *last_screen != Some(now) {
        *last_screen = Some(now);
        bot.timer = 0.0;
        bot.on_screen = 0.0;
        bot.goal = Goal::None;
        bot.acted = false;
        bot_move.0 = None;
    }
    let act = |bot: &mut Bot, pending: &mut PendingAction, a: Action| {
        if !bot.acted {
            bot.acted = true;
            pending.0 = Some(a);
        }
    };
    // Paused in a run (`--pause-at`): shoot the pause menu, then give the run up from it. Time
    // stands still while paused, so this counts frames.
    if paused.0 && now == Screen::Run {
        bot_move.0 = None;
        bot.paused_frames += 1;
        if !shot(&mut bot, &mut shots, "pause") && bot.paused_frames > 30 {
            act(&mut bot, &mut pending, Action::Abandon);
        }
        return;
    }
    bot.timer += time.delta_secs();
    bot.on_screen += time.delta_secs();
    let t = bot.on_screen;
    match now {
        Screen::Title if t > 1.0 => {
            if shot(&mut bot, &mut shots, "title") {
                return;
            }
            if t > 1.5 && !bot.picked_class {
                bot.picked_class = true;
                pending.0 = Some(Action::Play);
            } else if t > 2.5 && shot(&mut bot, &mut shots, "classpick") {
            } else if t > 3.2 {
                act(&mut bot, &mut pending, Action::PickClass(EnRoleCareerType::Fighter));
            }
        }
        Screen::Stash if t > 0.6 && !bot.picked_skill => {
            bot.picked_skill = true;
            pending.0 = Some(Action::NextSkill(0));
        }
        Screen::Stash if t > 1.2 && !bot.acted => {
            let name = if bot.runs_started == 0 { "stash_before" } else { "stash_after" };
            if shot(&mut bot, &mut shots, name) {
                return;
            }
            if t > 1.8 && !bot.acted {
                bot.runs_started += 1;
                act(&mut bot, &mut pending, Action::EnterCrypt);
            }
        }
        Screen::Result if t > 1.2 => {
            if shot(&mut bot, &mut shots, "result") {
                return;
            }
            if t > 1.8 {
                act(&mut bot, &mut pending, Action::Continue);
            }
        }
        // Behind the entry cover: only the cover to see.
        Screen::Run if cover.on => {
            shot(&mut bot, &mut shots, "entering");
        }
        Screen::Run => {
            // What to walk round: the traps' areas (a disc round each, with the bot's width), sleeping
            // monsters by their wake radius, idle ones by `AVOID_IDLE`.
            let on_traps: Vec<(Vec3, f32)> = traps.iter().map(|(t, p)| (p.0, t.half.x.max(t.half.y) + 60.0)).collect();
            let avoid: Vec<(Vec3, f32)> = monsters
                .iter()
                .filter(|(ai, _, h)| h.current > 0.0 && ai.target.is_none())
                .map(|(ai, p, _)| (p.0, if ai.dormant { ai.dormant_sight + 100.0 } else { AVOID_IDLE }))
                .chain(on_traps.iter().copied())
                .collect();
            run_step(&mut bot, t, &events, &player, &index, &dungeon, (crypt.0.as_deref(), &avoid, &on_traps), &actors, (&doors, &probe), &mut look, &mut bot_move, &mut shots, &mut queue, time.delta_secs())
        }
        _ => {}
    }
}

/// What the stuck log prints about an actor.
type Probe = (Option<&'static db_sim::ai::MonsterAi>, Option<&'static db_sim::buffs::Buffs>, Option<&'static db_sim::Melee>, Option<&'static db_sim::Health>, Option<&'static db_sim::CharacterState>, Option<&'static db_sim::MoveInput>, Option<&'static db_sim::MoveSpeed>);

fn describe(p: &Query<Probe>, e: Entity) -> String {
    let Ok((ai, buffs, melee, health, st, input, speed)) = p.get(e) else { return String::new() };
    format!(
        "ai {:?} buffs {:?} swing {} hp {:?} on_ground {:?} input {:?} speed {:?}",
        ai.map(|a| a.state),
        buffs.map(|b| b.0.iter().map(|b| format!("{:?}", b.kind)).collect::<Vec<_>>()),
        melee.is_some_and(|m| m.swing.is_some()),
        health.map(|h| h.current),
        st.map(|s| s.on_ground),
        input.map(|i| i.dir),
        speed.map(|s| s.0)
    )
}

#[allow(clippy::too_many_arguments)]
fn run_step(
    bot: &mut Bot,
    t: f32,
    events: &FrameEvents,
    player: &LocalPlayer,
    index: &SimIndex,
    dungeon: &CurrentDungeon,
    (crypt, avoid, on_traps): (Option<&CryptMaps>, &Discs, &Discs),
    actors: &Query<(&Kind, &Position, Option<&db_sim::Portal>, Option<&db_sim::RunStatus>)>,
    (doors, probe): (&DoorStates, &Query<Probe>),
    look: &mut Look,
    bot_move: &mut BotMove,
    shots: &mut ShotRequests,
    queue: &mut CommandQueue,
    dt: f32,
) {
    let (Some(id), Some(d)) = (player.0, dungeon.0.as_ref()) else { return };
    let Some((_, pos, _, status)) = index.get(id).and_then(|e| actors.get(e).ok()) else { return };
    let me = pos.0.truncate();
    let me3 = pos.0;
    let floor = &d.floors[status.map_or(0, |s| s.floor) as usize];
    // On the crypt floors a crypt run has no generated rooms.
    let crypt = crypt.filter(|_| floor.rooms.is_empty());
    for e in &events.0 {
        if let SimEvent::Rejected { reason } = e {
            info!("auto-run: refused: {reason} (at {me}, goal {:?})", std::mem::discriminant(&bot.goal));
        }
    }
    if t < 1.5 {
        return;
    }
    if bot.runs_started >= 2 {
        // Second run: show it started, then finish.
        if t > 2.0 && shot(bot, shots, "run2") {
            shots.exit_after = true;
        }
        return;
    }
    let find = |kind: ActorKind, near: Vec2, r: f32| index.iter().find(|(_, e)| actors.get(*e).is_ok_and(|a| a.0.0 == kind && a.1.0.truncate().distance(near) < r)).map(|(i, _)| i);
    let face = |look: &mut Look, p: Vec2| {
        let v = p - me;
        look.yaw = v.y.atan2(v.x).rem_euclid(std::f32::consts::TAU);
        look.pitch = -0.25;
    };
    // The way off this floor: down where the floor has a portal down, else out (one-floor maps).
    let leave = if floor.portals.iter().any(|p| p.kind == PortalKind::Down) { PortalKind::Down } else { PortalKind::Extract };
    match bot.goal.clone() {
        Goal::None => {
            shot(bot, shots, "floor1");
            // A floor-1 chest the walk can reach in front of (on the crypt grid: of the 8 nearest, the one with the
            // cheapest walk, `walk_score`, stopping short of it; the nearest can be a floor up), else straight to
            // the way off the floor.
            let (front, stop) = if crypt.is_some() { (Vec3::ZERO, 90.0) } else { (Vec3::new(0.0, 110.0, 0.0), 0.0) };
            let mut chests: Vec<Vec3> = floor.chests.iter().map(|c| c.0).collect();
            chests.sort_by(|a, b| a.truncate().distance(me).total_cmp(&b.truncate().distance(me)));
            let chest = chests.into_iter().take(8).filter_map(|c| route_to(crypt, avoid, floor, me3, c + front, stop).map(|r| (c, r))).min_by_key(|(_, r)| walk_score(r, avoid));
            match &chest {
                Some((c, route)) => info!("auto-run: first the chest at {c} ({} points from {me3}: {:?})", route.len(), &route[..route.len().min(6)]),
                None => info!("auto-run: no chest reachable from {me3}"),
            }
            bot.goal = match chest {
                Some((c, route)) => Goal::Walk(route, Box::new(Goal::OpenChest(c))),
                None => Goal::Portal(leave),
            };
            info!("auto-run: goal {:?} from {me3}", std::mem::discriminant(&bot.goal));
        }
        Goal::Walk(mut pts, then) => {
            // Going to pull a lever: done once its handle is in reach (the grid's route may lead on through the
            // shut gate).
            if let Goal::PullLever(lever, ..) = then.as_ref()
                && index.get(*lever).and_then(|e| doors.get(e).ok()).and_then(|q| q.3).is_some_and(|l| me3.distance(l.handle()) < l.placement.reach - 40.0)
            {
                pts.clear();
            }
            let Some(&next) = pts.first() else {
                bot_move.0 = None;
                bot.goal = *then;
                return;
            };
            let arrive = if pts.len() == 1 { 25.0 } else { 40.0 };
            if me.distance(next) < arrive {
                pts.remove(0);
                bot.sidestep.2 = 0;
                bot.goal = Goal::Walk(pts, then);
                return;
            }
            // A monster at it within reach (the Tentacles in seed 1's first corridor): stand and swing back like a
            // player would, then walk on.
            let foe = index
                .iter()
                .filter_map(|(sid, e)| {
                    let (k, p, ..) = actors.get(e).ok()?;
                    let (ai, _, _, hp, ..) = probe.get(e).ok()?;
                    let d = p.0.truncate().distance(me);
                    (k.0 == ActorKind::Monster && ai?.target == Some(id) && hp.is_some_and(|h| h.current > 0.0) && d < FIGHT_REACH).then_some((sid, p.0.truncate(), d))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2))
                // Not on a trap: walk on off it first.
                .filter(|_| !on_traps.iter().any(|&(c, r)| (c.z - pos.0.z).abs() < 300.0 && c.truncate().distance(me) < r));
            if let Some((foe, at, _)) = foe {
                face(look, at);
                bot_move.0 = None;
                if index.get(id).and_then(|e| probe.get(e).ok()).is_some_and(|q| q.2.is_none_or(|m| m.swing.is_none())) {
                    if bot.fighting != Some(foe) {
                        info!("auto-run: at {me}, fighting {foe:?} back");
                        bot.fighting = Some(foe);
                    }
                    queue.push(SimCommand::Attack { id, kind: db_sim::AttackKind::Light });
                }
                bot.stuck = (me, 0.0);
                bot.goal = Goal::Walk(pts, then);
                return;
            }
            face(look, next);
            let towards = (next - me).normalize_or_zero();
            if bot.sidestep.0 > 0.0 {
                // Unsticking: step aside like a player would, then try the point again.
                bot.sidestep.0 -= dt;
                bot_move.0 = Some((towards.perp() * bot.sidestep.1 - towards * 0.3).normalize_or_zero());
                bot.goal = Goal::Walk(pts, then);
                return;
            }
            bot_move.0 = Some(towards);
            // A closed door on the way: open it like a player would.
            bot.door_wait += dt;
            let door = index.iter().find(|(_, e)| actors.get(*e).is_ok_and(|a| a.0.0 == ActorKind::Door && a.1.0.truncate().distance(me) < 150.0) && doors.get(*e).is_ok_and(|(d, s, _, _)| d.is_some_and(|d| !d.open) || s.is_some_and(|s| !s.open)));
            if let Some((door, _)) = door
                && bot.door_wait > 1.0
            {
                info!("auto-run: at {me}, opening a door");
                queue.push(SimCommand::Interact { id, target: door });
                bot.door_wait = 0.0;
            }
            // A shut lever gate on the way (the crypt grid's routes don't know about them): walk to its
            // nearest reachable lever, pull it, wait for the gate, then go on.
            bot.gate_check += dt;
            if crypt.is_some() && bot.gate_check > 0.5 {
                bot.gate_check = 0.0;
                if let Some((gate, levers)) = gate_ahead(index, doors, me3, &pts, &bot.gates_tried) {
                    bot.gates_tried.push(gate);
                    let near = levers.iter().filter_map(|&l| {
                        let lever = index.get(l).and_then(|e| doors.get(e).ok()).and_then(|q| q.3)?;
                        route_to(crypt, avoid, floor, me3, lever.placement.pos, 100.0).map(|r| (l, r))
                    });
                    match near.min_by_key(|(_, r)| r.len()) {
                        Some((lever, route)) => {
                            info!("auto-run: at {me}, a shut gate {gate:?} ahead; going to its lever {lever:?} ({} points)", route.len());
                            bot_move.0 = None;
                            bot.goal = Goal::Walk(route, Box::new(Goal::PullLever(lever, gate, pts, then, false)));
                            return;
                        }
                        None => warn!("auto-run: at {me}, a shut gate {gate:?} ahead and none of its {} levers reachable", levers.len()),
                    }
                }
            }
            // Stuck: no progress for 4 s.
            if me.distance(bot.stuck.0) > 30.0 {
                bot.stuck = (me, 0.0);
            } else {
                bot.stuck.1 += dt;
                // A breakable prop in the way (the routes don't know them): break it.
                if bot.stuck.1 > 1.0
                    && let Some(prop) = find(ActorKind::Destroyable, me, 170.0)
                    && let Some(p) = index.get(prop).and_then(|e| actors.get(e).ok()).map(|a| a.1.0.truncate())
                {
                    info!("auto-run: at {me}, breaking the prop {prop:?} in the way");
                    face(look, p);
                    queue.push(SimCommand::Attack { id, kind: db_sim::AttackKind::Light });
                    bot.stuck = (me, 0.0);
                    return;
                }
                if bot.stuck.1 > 4.0 {
                    // What is around (for finding out why): actors within 3 m, with their height.
                    let near: Vec<String> = index.iter().filter_map(|(sid, e)| actors.get(e).ok().filter(|a| sid != id && a.1.0.truncate().distance(me) < 300.0).map(|a| format!("{:?} {sid:?} {:.0} cm dz {:.0} {}", a.0.0, a.1.0.truncate().distance(me), a.1.0.z - me3.z, describe(probe, e)))).collect();
                    let mine = index.get(id).map(|e| describe(probe, e)).unwrap_or_default();
                    // Sidestep (alternating sides) and retry; skip the point after three tries.
                    bot.sidestep.2 += 1;
                    let skip = bot.sidestep.2 > 3;
                    warn!("auto-run: stuck at {me} (z {:.0}) heading to {next}; {}; me: {mine}; near: {near:?}", me3.z, if skip { "skipping the point" } else { "stepping aside" });
                    if skip {
                        pts.remove(0);
                        bot.sidestep.2 = 0;
                    } else {
                        bot.sidestep.0 = 0.7;
                        bot.sidestep.1 = if bot.sidestep.2 % 2 == 1 { 1.0 } else { -1.0 };
                    }
                    bot.stuck = (me, 0.0);
                }
            }
            bot.goal = Goal::Walk(pts, then);
        }
        Goal::OpenChest(c) => {
            bot.visited.push(c);
            info!("auto-run: at {me}, opening the chest at {c}");
            face(look, c.truncate());
            // Look down at the chest and the items it spills.
            look.pitch = -1.05;
            if let Some(chest) = find(ActorKind::Chest, c.truncate(), 10.0) {
                queue.push(SimCommand::Interact { id, target: chest });
            }
            bot.goal = Goal::Pick(Vec::new());
            bot.timer = 0.0;
        }
        Goal::Pick(mut items) => {
            if let Some(opened) = events.0.iter().find_map(|e| if let SimEvent::ChestOpened { items, .. } = e { Some(items.clone()) } else { None }) {
                info!("auto-run: chest opened, {} items", opened.len());
                items = opened;
                bot.timer = 0.0;
            }
            // Linger a moment so the screenshot shows the items, then pick up one every 0.4 s.
            if bot.timer > 0.5 && shot(bot, shots, "chest") {
                bot.goal = Goal::Pick(items);
                return;
            }
            if bot.timer > 1.2 {
                match items.pop() {
                    Some(item) => {
                        queue.push(SimCommand::Interact { id, target: item });
                        bot.timer = 0.8;
                    }
                    None => {
                        bot.goal = Goal::Portal(leave);
                        if let Some(left) = bot.tour_left.filter(|n| *n > 0) {
                            bot.tour_left = Some(left - 1);
                            let (front, stop) = if crypt.is_some() { (Vec3::ZERO, 90.0) } else { (Vec3::new(0.0, 110.0, 0.0), 0.0) };
                            let mut far: Vec<Vec3> = floor.chests.iter().map(|c| c.0).filter(|c| !bot.visited.contains(c)).collect();
                            far.sort_by(|a, b| b.truncate().distance(me).total_cmp(&a.truncate().distance(me)));
                            let n = far.len();
                            match far.into_iter().find_map(|c| route_to(crypt, avoid, floor, me3, c + front, stop).map(|r| (c, r))) {
                                Some((c, route)) => {
                                    info!("auto-run: tour: walking to a far chest at {c} ({} points)", route.len());
                                    bot.goal = Goal::Walk(route, Box::new(Goal::OpenChest(c)));
                                }
                                None => warn!("auto-run: tour: none of {n} chests reachable"),
                            }
                        }
                        return;
                    }
                }
            }
            bot.goal = Goal::Pick(items);
        }
        Goal::Portal(kind) => {
            // Nearest portal of that kind on this floor (by route length).
            let target = floor.portals.iter().filter(|p| p.kind == kind).filter_map(|p| route_to(crypt, avoid, floor, me3, p.pos, 80.0).map(|r| (r, p.pos))).min_by_key(|(r, _)| r.len());
            match target {
                Some((route, p)) => {
                    let then = if kind == PortalKind::Down { Goal::WaitFloor2 } else { Goal::WaitEnd };
                    info!("auto-run: walking to the {kind:?} portal at {p} ({} points)", route.len());
                    bot.goal = Goal::Walk(route, Box::new(then));
                }
                // (Retried every frame; said every 5 s.)
                None if bot.timer > 5.0 => {
                    warn!("auto-run: at {me3}, no route to a {kind:?} portal");
                    bot.timer = 0.0;
                }
                None => {}
            }
        }
        Goal::PullLever(lever, gate, resume, then, pulled) => {
            bot_move.0 = None;
            let open = index.get(gate).and_then(|e| doors.get(e).ok()).and_then(|q| q.2).is_none_or(|m| !gate_shut(m) && !m.moving());
            if !pulled {
                if let Some(l) = index.get(lever).and_then(|e| doors.get(e).ok()).and_then(|q| q.3) {
                    face(look, l.handle().truncate());
                }
                info!("auto-run: at {me}, pulling the lever {lever:?}");
                queue.push(SimCommand::Interact { id, target: lever });
                bot.timer = 0.0;
                bot.goal = Goal::PullLever(lever, gate, resume, then, true);
            } else if open || bot.timer > 12.0 {
                if open {
                    info!("auto-run: the gate {gate:?} is open after {:.1} s", bot.timer);
                } else {
                    warn!("auto-run: the gate {gate:?} didn't open; going on");
                }
                bot.goal = Goal::Walk(resume, then);
            } else {
                bot.goal = Goal::PullLever(lever, gate, resume, then, true);
            }
        }
        Goal::WaitFloor2 | Goal::WaitEnd => {
            bot_move.0 = None;
            let kind = if bot.goal == Goal::WaitFloor2 { PortalKind::Down } else { PortalKind::Extract };
            let portal = index.iter().filter(|(_, e)| actors.get(*e).is_ok_and(|a| a.2.is_some_and(|p| p.kind == kind))).min_by(|a, b| {
                let da = actors.get(a.1).map_or(f32::MAX, |x| x.1.0.truncate().distance(me));
                let db = actors.get(b.1).map_or(f32::MAX, |x| x.1.0.truncate().distance(me));
                da.total_cmp(&db)
            });
            if events.0.iter().any(|e| matches!(e, SimEvent::FloorChanged { id: who, .. } if *who == id)) {
                info!("auto-run: on floor 2");
                bot.timer = 0.0;
                bot.goal = Goal::Arrived;
                return;
            }
            if bot.timer > 0.5
                && let Some((pid, pe)) = portal
            {
                info!("auto-run: at {me}, using portal {pid:?}");
                if let Ok(p) = actors.get(pe) {
                    face(look, p.1.0.truncate());
                }
                queue.push(SimCommand::Interact { id, target: pid });
                bot.timer = 0.0;
            }
        }
        Goal::Arrived => {
            if bot.timer > 1.0 && !shot(bot, shots, "floor2") {
                bot.goal = Goal::Portal(PortalKind::Extract);
            }
        }
    }
}

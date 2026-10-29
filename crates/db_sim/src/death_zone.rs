//! The run's death zone: each floor's placed `BP_DeathZone`, a series of discs the safe area shrinks through. From
//! its bytecode:
//! - `Start Round(i)`: the round's disc is `Discs[i]`, the next one `Discs[i + 1]` (after the last, with
//!   `bShouldShrinkLastDisc`, the same centre at radius 0); it waits `Pause`, then `Interval`, then shrinks over
//!   `Duration` (centre and radius lerped), then starts round `i + 1`. Past the last disc everyone still in the
//!   floor dies `AdditinalTimeToKillAll` (15 s) later.
//! - every second (`Update Damage`) a player outside the circle (2D distance) is in the zone: the round's damage
//!   effects run on them (`GE_TGF_Damage_DeathZone_InstantDeath`: 0.5 % of max health every 0.01 s) and their
//!   `DeathZoneTime` grows, capped at the round's `MaxDeathZoneTime`; back inside it falls by
//!   `DeathZoneTimeDeclineRate` per second.
//!
//! The crypt's floor 1 and the castle have one disc: 10 s, then 1490 s, then the circle closes and kills; floor 2
//! shrinks through seven, 9000 cm down to 1 cm, in about 15 minutes. Together they make the map's 2400 s timeout.
//!
//! - each round's `PortalSpawnSettings` spawn extraction devices `SpawnTime` s into the round (with their
//!   `SpawnProbability`, the class by its `Probability`): floor 1 twenty, two every 30 s from 300 s, the castle
//!   twenty-four. A device is held `BaseInteractTime` (12 s), then takes the player out (`SpawnExtractPortal`: a portal
//!   one player enters, then gone); it's used up.
//!
//! - floor 2's round 1 puts three Ritual Scrolls (`ID_TeleportScroll`, item 100021) down 10 s in
//!   (`TeleportScrollSpawnTimes`) on `PotentialTeleportScrollPlacements` spots; a device's use takes one from the
//!   player if they carry one (`ConsumeScroll`; its description: carried, the ritual doesn't give the player away).
//!   Each round's `Messages` come one after another, each `Delay` s after the last (`HandleDeathZoneMessage`), as
//!   `SimEvent::ZoneMessage`.
//!
//! HYPOTHESES: the native `Generate` that places the discs (here: the last from its `ImportanceBoxes`, each earlier
//! one around the next so it holds it, inside its own boxes when it has some); a floor's zone starting when the first
//! player reaches it (the original travels the waiting players to floor 2 when floor 1 ends); floor 2's discs carry
//! no damage effects, so a full `DeathZoneTime` kills (`overrides::death_zone`); fixed damage ignores defence
//! (`TgfFixedDamageExecution`); a device on a random free `PotentialPortalPlacements` spot inside the circle (the
//! native `SpawnPortalAndScrolls`); the device extracting at the end of the hold (the portal it spawns extracts its
//! first toucher 4 s later).

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_math::{Vec2, Vec3};

use crate::id::SimIndex;
use crate::melee::Health;
use crate::rng::SimRng;
use crate::run::RunStatus;
use crate::{ActorKind, DT, Kind, Position, SimEvent, SimEvents, SimId, TICK_RATE_HZ, TickCount};

/// A damage effect run on players in the zone: `rate` × max health + `flat` every `period` s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneDamage {
    pub rate: f32,
    pub flat: f32,
    pub period: f32,
}

/// Where a disc likes to go: a box (centre, half extents; map-local, then world) and its weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneBox {
    pub center: Vec2,
    pub extent: Vec2,
    pub importance: f32,
}

/// One of a round's device spawns (`PortalSpawnSettings`): seconds into the round, its chance, and the device
/// classes it picks from (class, weight, hold s).
#[derive(Debug, Clone, PartialEq)]
pub struct ZonePortal {
    pub time: f32,
    pub chance: f32,
    pub choices: Vec<(String, f32, f32)>,
}

/// One round's disc.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneDisc {
    pub radius: f32,
    pub padding: f32,
    pub pause: f32,
    pub interval: f32,
    pub duration: f32,
    pub max_time: f32,
    pub boxes: Vec<ZoneBox>,
    pub damage: Vec<ZoneDamage>,
    pub portals: Vec<ZonePortal>,
    /// `TeleportScrollSpawnTimes` (s into the round).
    pub scrolls: Vec<f32>,
    /// `Messages`: delay after the previous (s) and `GeneralStrings` key (none: a pause in the chain).
    pub messages: Vec<(f32, Option<String>)>,
}

/// A floor's zone as placed.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneSpec {
    pub discs: Vec<ZoneDisc>,
    pub shrink_last: bool,
    pub kill_all_after: f32,
    pub decline: f32,
    /// Where it may put devices and scrolls (`PotentialPortalPlacements`, `PotentialTeleportScrollPlacements`;
    /// map-local floor points).
    pub spots: Vec<Vec3>,
    pub scroll_spots: Vec<Vec3>,
}

impl ZoneSpec {
    /// From a run file's floor `zone`.
    pub fn from_json(z: &serde_json::Value) -> Option<Self> {
        let f = |v: &serde_json::Value, d: f32| v.as_f64().map_or(d, |x| x as f32);
        let discs = z["discs"]
            .as_array()?
            .iter()
            .map(|d| ZoneDisc {
                radius: f(&d["radius"], 0.0),
                padding: f(&d["padding"], 0.0),
                pause: f(&d["pause"], 0.0),
                interval: f(&d["interval"], 0.0),
                duration: f(&d["duration"], 0.0),
                max_time: f(&d["max_time"], 0.0),
                boxes: d["boxes"].as_array().into_iter().flatten().map(|b| ZoneBox { center: Vec2::new(f(&b[0], 0.0), f(&b[1], 0.0)), extent: Vec2::new(f(&b[2], 0.0), f(&b[3], 0.0)), importance: f(&b[4], 1.0) }).collect(),
                damage: d["damage"].as_array().into_iter().flatten().map(|g| ZoneDamage { rate: f(&g["rate"], 0.0), flat: f(&g["flat"], 0.0), period: f(&g["period"], 1.0) }).collect(),
                portals: d["portals"].as_array().into_iter().flatten().map(|p| ZonePortal { time: f(&p[0], 0.0), chance: f(&p[1], 1.0), choices: p[2].as_array().into_iter().flatten().map(|c| (c[0].as_str().unwrap_or_default().to_string(), f(&c[1], 1.0), f(&c[2], 0.0))).collect() }).collect(),
                scrolls: d["scrolls"].as_array().into_iter().flatten().map(|t| f(t, 0.0)).collect(),
                messages: d["messages"].as_array().into_iter().flatten().map(|m| (f(&m[0], 0.0), m[1].as_str().map(str::to_string))).collect(),
            })
            .collect::<Vec<_>>();
        let spots = |v: &serde_json::Value| v.as_array().into_iter().flatten().map(|p| Vec3::new(f(&p[0], 0.0), f(&p[1], 0.0), f(&p[2], 0.0))).collect();
        (!discs.is_empty()).then(|| ZoneSpec { discs, shrink_last: z["shrink_last"].as_bool().unwrap_or(true), kill_all_after: f(&z["kill_all_after"], 15.0), decline: f(&z["decline"], 1.0), spots: spots(&z["spots"]), scroll_spots: spots(&z["scroll_spots"]) })
    }

    /// The discs' centres for one run, world space (map-local + `off`). HYPOTHESIS (`Generate` is native): the last
    /// disc in one of its boxes (weighted by importance), then backwards each disc around the next so it holds it
    /// (and in one of its own boxes when it has some and one fits).
    pub fn plan(&self, rng: &mut SimRng, off: Vec3) -> Vec<Vec2> {
        let n = self.discs.len();
        let mut centres = vec![Vec2::ZERO; n];
        let in_box = |rng: &mut SimRng, boxes: &[ZoneBox]| -> Option<Vec2> {
            let total: f32 = boxes.iter().map(|b| b.importance.max(0.0)).sum();
            if total <= 0.0 {
                return None;
            }
            let mut pick = rng.unit_f32() * total;
            let b = boxes.iter().find(|b| {
                pick -= b.importance.max(0.0);
                pick < 0.0
            })?;
            Some(b.center + Vec2::new((rng.unit_f32() * 2.0 - 1.0) * b.extent.x, (rng.unit_f32() * 2.0 - 1.0) * b.extent.y))
        };
        for i in (0..n).rev() {
            let d = &self.discs[i];
            if i + 1 == n {
                centres[i] = in_box(rng, &d.boxes).unwrap_or(Vec2::ZERO);
                continue;
            }
            let inner = centres[i + 1];
            let room = (d.radius - self.discs[i + 1].radius - d.padding).max(0.0);
            let around = |rng: &mut SimRng| {
                let (a, r) = (rng.unit_f32() * std::f32::consts::TAU, room * rng.unit_f32().sqrt());
                inner + Vec2::new(a.cos(), a.sin()) * r
            };
            centres[i] = (0..32).find_map(|_| in_box(rng, &d.boxes).filter(|c| c.distance(inner) <= room)).unwrap_or_else(|| around(rng));
        }
        centres.into_iter().map(|c| c + off.truncate()).collect()
    }
}

/// A floor's zone for one run: its spec, where its discs stand (world) and the floor's offset (its spots are
/// map-local).
#[derive(Debug, Clone, PartialEq)]
pub struct ZonePlan {
    pub spec: Arc<ZoneSpec>,
    pub centres: Vec<Vec2>,
    pub off: Vec3,
}

/// `BP_DeathZone.State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneState {
    /// Waiting for a player on its floor.
    Idle,
    Pause,
    Interval,
    Shrinking,
    /// Past the last round (or holding the last disc).
    Ended,
}

/// A floor's death zone in the world.
#[derive(Component, Debug, Clone)]
pub struct DeathZone {
    pub floor: u32,
    pub plan: Arc<ZonePlan>,
    pub state: ZoneState,
    /// The round (`Current Disc Index`).
    pub disc: usize,
    pub prev: (Vec2, f32),
    pub next: (Vec2, f32),
    /// The circle now (`Dynamic Center`, `Dynamic Radius`).
    pub center: Vec2,
    pub radius: f32,
    /// Ticks the round's phases end (`Current Pause / Interval / Round End Time`).
    pub pause_end: u64,
    pub interval_end: u64,
    pub round_end: u64,
    /// When everyone left on the floor dies (past the last round).
    pub kill_at: Option<u64>,
    /// The round's start (`Current State Start Time`) and which of its device spawns have come.
    pub round_start: u64,
    pub fired: Vec<bool>,
    /// Spots already holding a device.
    pub used: Vec<usize>,
    /// Which of the round's scrolls are down, the scroll spots used.
    pub scrolls_fired: Vec<bool>,
    pub scroll_used: Vec<usize>,
    /// The message chain: its round, the next message and when it comes.
    pub messages: usize,
    pub next_message: usize,
    pub message_at: Option<u64>,
}

impl DeathZone {
    /// The round's disc; past the last round, the last (HYPOTHESIS: the native `GetDiscDefinitionByIndex` clamps, so
    /// the closed circle's effects keep running until everyone is killed).
    pub fn disc(&self) -> Option<&ZoneDisc> {
        let discs = &self.plan.spec.discs;
        discs.get(self.disc.min(discs.len().saturating_sub(1)))
    }

    /// Whether `p` (world) stands in the zone (outside the circle; 2D, `Vector_Distance2D`).
    pub fn outside(&self, p: Vec3) -> bool {
        self.state != ZoneState::Idle && p.truncate().distance(self.center) > self.radius
    }

    /// Seconds until the circle starts closing (`GetShrinkingCountdownTime`; 0 while it closes or after).
    pub fn closes_in(&self, now: u64) -> f32 {
        match self.state {
            ZoneState::Pause | ZoneState::Interval => self.interval_end.saturating_sub(now) as f32 * DT,
            _ => 0.0,
        }
    }

    /// `Start Round(i)`.
    fn start_round(&mut self, i: usize, now: u64) {
        let spec = self.plan.spec.clone();
        self.disc = i;
        self.round_start = now;
        self.fired = vec![false; spec.discs.get(i).map_or(0, |d| d.portals.len())];
        self.scrolls_fired = vec![false; spec.discs.get(i).map_or(0, |d| d.scrolls.len())];
        // A round with messages starts a new chain (`DeathZoneMessages` set only when the round has some).
        if let Some(m) = spec.discs.get(i).and_then(|d| d.messages.first()) {
            (self.messages, self.next_message, self.message_at) = (i, 0, Some(now + ticks(m.0)));
        }
        let Some(d) = spec.discs.get(i) else {
            self.state = ZoneState::Ended;
            self.kill_at = Some(now + ticks(spec.kill_all_after));
            return;
        };
        self.prev = (self.plan.centres[i], d.radius);
        self.next = match spec.discs.get(i + 1) {
            Some(n) => (self.plan.centres[i + 1], n.radius),
            None if spec.shrink_last => (self.prev.0, 0.0),
            None => self.prev,
        };
        (self.center, self.radius) = self.prev;
        self.state = if i + 1 == spec.discs.len() && !spec.shrink_last { ZoneState::Ended } else { ZoneState::Pause };
        self.pause_end = now + ticks(d.pause);
        self.interval_end = self.pause_end + ticks(d.interval);
        self.round_end = self.interval_end + ticks(d.duration);
    }
}

/// A player's time in the zone (`DeathZoneTime`) and when each of the round's damage effects next ticks on them (run
/// seconds: a period can be shorter than a tick).
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ZoneExposure {
    pub time: f32,
    pub in_zone: bool,
    pub next_hits: Vec<f64>,
}

fn ticks(s: f32) -> u64 {
    (s / DT).round().max(0.0) as u64
}

/// `SimCommand::SpawnDeathZone`.
pub(crate) fn spawn(world: &mut World, floor: u32, plan: ZonePlan) {
    let first = plan.spec.discs.first().map_or(0.0, |d| d.radius);
    let c = plan.centres.first().copied().unwrap_or(Vec2::ZERO);
    world.spawn(DeathZone { floor, plan: Arc::new(plan), state: ZoneState::Idle, disc: 0, prev: (c, first), next: (c, first), center: c, radius: first, pause_end: 0, interval_end: 0, round_end: 0, kill_at: None, round_start: 0, fired: Vec::new(), used: Vec::new(), scrolls_fired: Vec::new(), scroll_used: Vec::new(), messages: 0, next_message: 0, message_at: None });
}

/// The players in a run, alive, on each floor.
fn players(world: &mut World) -> Vec<(SimId, Entity, u32, Vec3)> {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    ids.into_iter()
        .filter(|(_, e)| world.get::<Kind>(*e).is_some_and(|k| k.0 == ActorKind::Player))
        .filter_map(|(id, e)| {
            let s = world.get::<RunStatus>(e).filter(|s| s.outcome.is_none())?;
            let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
            alive.then(|| (id, e, s.floor, world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0)))
        })
        .collect()
}

/// The zones' rounds, then damage and `DeathZoneTime` for the players in them; after the run rules each tick.
pub(crate) fn death_zone_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let players = players(world);
    let zones: Vec<Entity> = world.query_filtered::<Entity, With<DeathZone>>().iter(world).collect();
    for ze in zones {
        let mut z = world.get::<DeathZone>(ze).cloned().expect("zone");
        let here: Vec<&(SimId, Entity, u32, Vec3)> = players.iter().filter(|p| p.2 == z.floor).collect();
        if z.state == ZoneState::Idle {
            if here.is_empty() {
                continue;
            }
            z.start_round(0, now);
        }
        // The round's phases (`ReceiveTick`).
        match z.state {
            ZoneState::Pause if now >= z.pause_end => z.state = ZoneState::Interval,
            ZoneState::Interval if now >= z.interval_end => z.state = ZoneState::Shrinking,
            ZoneState::Shrinking if now >= z.round_end => {
                (z.center, z.radius) = z.next;
                z.start_round(z.disc + 1, now);
            }
            _ => {}
        }
        if z.state != ZoneState::Ended || z.kill_at.is_none() {
            let d = z.plan.spec.discs.get(z.disc).map_or(0.0, |d| d.duration);
            let a = if d > 0.0 { ((now as f32 - z.interval_end as f32) * DT / d).clamp(0.0, 1.0) } else if now >= z.interval_end { 1.0 } else { 0.0 };
            z.center = z.prev.0.lerp(z.next.0, a);
            z.radius = z.prev.1 + (z.next.1 - z.prev.1) * a;
        }
        let kill_all = z.kill_at.is_some_and(|k| now >= k);
        if kill_all {
            z.kill_at = None;
        }
        let disc = z.disc().cloned();
        spawn_devices(world, &mut z, now);
        spawn_scrolls(world, &mut z, now);
        messages(world, &mut z, now);
        world.entity_mut(ze).insert(z.clone());
        for &&(id, e, _, pos) in &here {
            if kill_all {
                kill(world, id, e);
                continue;
            }
            exposure(world, id, e, &z, disc.as_ref(), pos, now);
        }
    }
}

/// The round's device spawns that are due.
fn spawn_devices(world: &mut World, z: &mut DeathZone, now: u64) {
    let Some(d) = z.plan.spec.discs.get(z.disc).cloned() else { return };
    for (k, p) in d.portals.iter().enumerate() {
        if z.fired.get(k).copied().unwrap_or(true) || now < z.round_start + ticks(p.time) {
            continue;
        }
        z.fired[k] = true;
        let mut rng = world.resource_mut::<SimRng>();
        if !rng.chance(p.chance as f64) {
            continue;
        }
        let total: f32 = p.choices.iter().map(|c| c.1.max(0.0)).sum();
        let mut pick = rng.unit_f32() * total;
        let Some(choice) = p.choices.iter().find(|c| {
            pick -= c.1.max(0.0);
            pick < 0.0
        }) else {
            continue;
        };
        let spots = &z.plan.spec.spots;
        let free: Vec<usize> = (0..spots.len()).filter(|i| !z.used.contains(i) && (spots[*i] + z.plan.off).truncate().distance(z.center) <= z.radius).collect();
        if free.is_empty() {
            continue;
        }
        let i = free[world.resource_mut::<SimRng>().range_u32(0, free.len() as u32) as usize];
        z.used.push(i);
        crate::run::spawn_device(world, spots[i] + z.plan.off, choice.2);
    }
}

/// The round's scrolls that are due, on free scroll spots inside the circle.
fn spawn_scrolls(world: &mut World, z: &mut DeathZone, now: u64) {
    let Some(d) = z.plan.spec.discs.get(z.disc).cloned() else { return };
    for (k, t) in d.scrolls.iter().enumerate() {
        if z.scrolls_fired.get(k).copied().unwrap_or(true) || now < z.round_start + ticks(*t) {
            continue;
        }
        z.scrolls_fired[k] = true;
        let spots = &z.plan.spec.scroll_spots;
        let free: Vec<usize> = (0..spots.len()).filter(|i| !z.scroll_used.contains(i) && (spots[*i] + z.plan.off).truncate().distance(z.center) <= z.radius).collect();
        if free.is_empty() || !world.contains_resource::<crate::inventory::GameData>() {
            continue;
        }
        let i = free[world.resource_mut::<SimRng>().range_u32(0, free.len() as u32) as usize];
        z.scroll_used.push(i);
        let at = spots[i] + z.plan.off + Vec3::Z * crate::overrides::world_items::ITEM_HEIGHT;
        crate::world_items::spawn_item_by_id(world, at, crate::overrides::run::TELEPORT_SCROLL);
    }
}

/// The message chain (`HandleDeathZoneMessage`).
fn messages(world: &mut World, z: &mut DeathZone, now: u64) {
    let list = z.plan.spec.discs.get(z.messages).map(|d| d.messages.clone()).unwrap_or_default();
    while let Some(at) = z.message_at.filter(|&at| now >= at) {
        let Some((_, key)) = list.get(z.next_message) else {
            z.message_at = None;
            break;
        };
        if let Some(key) = key {
            world.resource_mut::<SimEvents>().push(SimEvent::ZoneMessage { floor: z.floor, key: key.clone() });
        }
        z.next_message += 1;
        z.message_at = list.get(z.next_message).map(|m| at + ticks(m.0));
    }
}

fn kill(world: &mut World, id: SimId, e: Entity) {
    if let Some(mut h) = world.get_mut::<Health>(e) {
        h.current = 0.0;
    }
    world.resource_mut::<SimEvents>().push(SimEvent::Died { id });
}

/// One player against the zone: membership every second (`Update Damage`), the damage effects at their period.
fn exposure(world: &mut World, id: SimId, e: Entity, z: &DeathZone, disc: Option<&ZoneDisc>, pos: Vec3, now: u64) {
    let mut x = world.get::<ZoneExposure>(e).cloned().unwrap_or_default();
    let second = now.is_multiple_of(TICK_RATE_HZ as u64);
    let now_s = now as f64 * DT as f64;
    let decline = z.plan.spec.decline;
    let (max_time, damage) = disc.map_or((f32::MAX, &[][..]), |d| (d.max_time, &d.damage[..]));
    if second {
        let out = z.outside(pos);
        if out && !x.in_zone {
            // Entering: the effects start, each ticking at once (`ApplyCurrentDamageGEs`).
            x.next_hits = vec![now_s; damage.len()];
        }
        x.in_zone = out;
    }
    if x.in_zone {
        x.time = (x.time + DT).min(max_time);
    } else {
        x.time = (x.time - DT * decline).max(0.0);
    }
    let mut dead = false;
    if x.in_zone {
        x.next_hits.resize(damage.len(), now_s);
        for (g, next) in damage.iter().zip(x.next_hits.iter_mut()) {
            while *next <= now_s + 1e-9 && !dead {
                *next += (g.period as f64).max(1e-3);
                let max = world.get::<Health>(e).map_or(0.0, |h| h.max);
                let dmg = (g.rate as f64 * max + g.flat.abs() as f64).max(0.0);
                if dmg <= 0.0 {
                    break;
                }
                let left = crate::world_items::apply_damage(world, id, e, dmg);
                let mut ev = world.resource_mut::<SimEvents>();
                ev.push(SimEvent::Damaged { source: id, target: id, damage: dmg, crit: false, blocked: false, health: left, by: crate::HitSource::Zone });
                if left <= 0.0 {
                    ev.push(SimEvent::Died { id });
                    dead = true;
                }
            }
        }
        if !dead && damage.is_empty() && crate::overrides::death_zone::FULL_TIME_KILLS && x.time >= max_time {
            kill(world, id, e);
        }
    }
    world.entity_mut(e).insert(x);
}

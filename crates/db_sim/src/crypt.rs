//! Runs on the real crypt floors: `L_Dungeon_Official_01` and `_02` as a [`Dungeon`] the run code already
//! plays. The layout comes (`_data/recovered/maps/crypt-run.json`: the maps' spawn points with their
//! spawn-table or fixed-class choices, spawn sets and rooms, escape points with delays, player starts,
//! floor 2 arrivals), the floors' collision from [`crate::map`] and their walkable grids from
//! [`crate::nav`]. A seed rolls the spawn sets (`Chance`), rooms (`FixedSpawnNumber` of their children),
//! each point's choice by its weights, the start and the escape delays. Only points on the walkable grid
//! spawn (the converted scene lacks some rooms). HYPOTHESIS ([`crate::overrides::crypt`]): blue escape
//! portals extract and red ones lead down, the floor offsets, the start rule and floor 2's stand-in
//! extraction portal.
//!
//! Maps as data: [`MapDef`] names a playable map's levels in floor order, its scene folder, its run
//! layout file and floor offsets; [`CRYPT`] (two floors) and [`CASTLE`] (the Gothic Castle's first floor,
//! `ExcelMapDetail` map 10: the castle with only `L_GothicCastle_WP_Main`) share all the code below.

use std::sync::Arc;

use bevy_math::{Vec2, Vec3};

use crate::SimRng;
use crate::dungeon::{Dungeon, Floor, Portal, PortalKind, TrapSpot};
use crate::features::{AltarKind, TrapKind};
use crate::map::MapCollision;
use crate::nav::NavGrid;
use crate::overrides::crypt as tune;
use crate::overrides::dungeon::CAPSULE_CENTER_Z;

/// A playable map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapDef {
    /// Short key (`crypt`, `castle`), saved in the profile.
    pub key: &'static str,
    pub title: &'static str,
    /// The scene folder under `TgfMaps/Content/Scenes/`.
    pub folder: &'static str,
    /// Level names in floor order.
    pub levels: &'static [&'static str],
    /// The run layout under `_data/recovered/maps/`.
    pub run_file: &'static str,
    /// Where each floor sits in the world (cm), so floors never overlap.
    pub offsets: &'static [[f32; 3]],
    /// `ExcelMapDetail` map id (its run time).
    pub map_id: i32,
}

/// The two crypt floors (`ExcelMapDetail` map 1).
pub const CRYPT: MapDef = MapDef { key: "crypt", title: "The Crypt", folder: "Dungeon", levels: &["L_Dungeon_Official_01", "L_Dungeon_Official_02"], run_file: "crypt-run.json", offsets: &tune::FLOOR_OFFSETS, map_id: 1 };
/// The Gothic Castle's two floors (`ExcelMapDetail` map 10201: `L_GothicCastle_WP_Main`, then the throne floor
/// `L_GothicCastle_WP_Main_SecondFloor`).
pub const CASTLE: MapDef = MapDef { key: "castle", title: "Gothic Castle", folder: "GothicCastle", levels: &["L_GothicCastle_WP_Main", "L_GothicCastle_WP_Main_SecondFloor"], run_file: "castle-run.json", offsets: &tune::CASTLE_FLOOR_OFFSETS, map_id: 10201 };
/// Every playable map; the first is the default.
pub const MAPS: [MapDef; 2] = [CASTLE, CRYPT];

/// A map by its key.
pub fn map_def(key: &str) -> Option<MapDef> {
    MAPS.into_iter().find(|m| m.key == key)
}

/// The full level name for a short key (`01`, `02`: the crypt floors; `castle1`, `castle2`: the castle's)
/// or a full name (as is).
pub fn level_name(level: &str) -> String {
    match level {
        "01" | "02" => format!("L_Dungeon_Official_{level}"),
        "castle1" => "L_GothicCastle_WP_Main".into(),
        "castle2" => "L_GothicCastle_WP_Main_SecondFloor".into(),
        other => other.into(),
    }
}

/// The scene folder of a level (by its full name).
pub fn level_folder(name: &str) -> &'static str {
    if name.starts_with("L_GothicCastle") { "GothicCastle" } else { "Dungeon" }
}

/// One spawn choice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice {
    Monster(&'static str),
    /// A chest table's mimic row: the monster, spawned dormant, turned by the row's `YawOffset` (deg).
    Mimic(&'static str, f32),
    Chest(i32),
    Altar(Option<AltarKind>),
    Trap(TrapKind),
    Portal(PortalKind),
    /// A breakable prop's class.
    Destroyable(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cat {
    Monster,
    Chest,
    Altar,
    Trap,
    Escape,
    Extract,
    Start,
    Arrival,
    Destroyable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub cat: Cat,
    /// Floor point (cm, map-local).
    pub pos: Vec3,
    /// Radians.
    pub yaw: f32,
    pub choices: Vec<(f64, Choice)>,
    pub group: Option<usize>,
    pub chance: f64,
    /// Escape activation delay range, seconds.
    pub delay: (f32, f32),
    /// A monster spawn point's settings.
    pub opts: crate::ai::SpawnOpts,
    /// A placed actor's scale (the props).
    pub scale: Vec3,
    /// An escape point's escape doors (indices of its floor's mechanisms).
    pub opens: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub parent: Option<usize>,
    /// A spawn room (`BP_ActorSpawnRoom`), else a spawn set (`BP_ActorSpawnSet`).
    pub room: bool,
    /// A set's weight among its room's sets.
    pub chance: f64,
    /// A room's `FixedSpawnNumber`: spawn this many points over its sets (none: one set).
    pub fixed: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CryptFloor {
    pub groups: Vec<Group>,
    pub entries: Vec<Entry>,
    /// The placed swing doors: blueprint package, origin, yaw (degrees), scale; the classes that aren't swing
    /// doors (the lifted mechanisms, the chained doors) are left out.
    pub doors: Vec<(String, Vec3, f32, Vec3)>,
    /// The lever mechanisms with their levers.
    pub mechanisms: Vec<crate::mechanisms::MechanismPlacement>,
    /// Its death zone.
    pub zone: Option<Arc<crate::death_zone::ZoneSpec>>,
    /// Its floor collapse.
    pub collapse: Option<Arc<crate::floor_collapse::CollapseSpec>>,
}

/// Everything a run on a map needs, loaded once.
#[derive(Debug, Clone)]
pub struct CryptMaps {
    pub def: MapDef,
    pub floors: Vec<CryptFloor>,
    /// Floor collision with the floor offset set.
    pub collision: Vec<Arc<MapCollision>>,
    pub nav: Vec<Arc<NavGrid>>,
}

impl MapDef {
    /// Where floor `floor` sits in the world (cm).
    pub fn offset(&self, floor: usize) -> Vec3 {
        self.offsets.get(floor).copied().map_or(Vec3::ZERO, Vec3::from_array)
    }
}

fn altar(name: Option<&str>) -> Option<AltarKind> {
    Some(match name? {
        "Attack" => AltarKind::Attack,
        "Defend" => AltarKind::Defend,
        "Heal" => AltarKind::Heal,
        "Clarity" => AltarKind::Clarity,
        "Speed" => AltarKind::Speed,
        _ => return None,
    })
}

/// Parses a run layout (`crypt-run.json`, `castle-run.json`) with `count` floors.
pub fn parse(json: &serde_json::Value, count: usize) -> anyhow::Result<Vec<CryptFloor>> {
    let mut floors: Vec<CryptFloor> = vec![CryptFloor::default(); count];
    let list = json["floors"].as_array().ok_or_else(|| anyhow::anyhow!("no floors"))?;
    anyhow::ensure!(list.len() == count, "expected {count} floors, the layout has {}", list.len());
    let f = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0);
    for (out, fl) in floors.iter_mut().zip(list) {
        for g in fl["groups"].as_array().into_iter().flatten() {
            out.groups.push(Group { parent: g["parent"].as_u64().map(|p| p as usize), room: g["room"].as_bool().unwrap_or(false), chance: g["chance"].as_f64().unwrap_or(1.0), fixed: g["fixed"].as_u64().map(|n| n as u32) });
        }
        let v3 = |v: &serde_json::Value, d: f32| Vec3::new(v[0].as_f64().map_or(d, |x| x as f32), v[1].as_f64().map_or(d, |x| x as f32), v[2].as_f64().map_or(d, |x| x as f32));
        for d in fl["doors"].as_array().into_iter().flatten() {
            if let Some(package) = d["package"].as_str().filter(|p| crate::swing_doors::spec(p).is_some()) {
                out.doors.push((package.to_string(), v3(&d["pos"], 0.0), f(&d["yaw"]) as f32, v3(&d["scale"], 1.0)));
            }
        }
        let levers = fl["levers"].as_array().cloned().unwrap_or_default();
        out.zone = crate::death_zone::ZoneSpec::from_json(&fl["zone"]).map(Arc::new);
        out.collapse = crate::floor_collapse::CollapseSpec::from_json(&fl["collapse"]).map(Arc::new);
        out.mechanisms = fl["mechanisms"].as_array().into_iter().flatten().enumerate().filter_map(|(i, m)| crate::mechanisms::MechanismPlacement::from_json(m, i, &levers)).collect();
        for e in fl["entries"].as_array().into_iter().flatten() {
            let cat = match e["cat"].as_str().unwrap_or("") {
                "monster" => Cat::Monster,
                "chest" => Cat::Chest,
                "altar" => Cat::Altar,
                "trap" => Cat::Trap,
                "escape_point" => Cat::Escape,
                "extract" => Cat::Extract,
                "player_start" => Cat::Start,
                "arrival" => Cat::Arrival,
                "destroyable" => Cat::Destroyable,
                other => anyhow::bail!("unknown category {other}"),
            };
            let mut choices = Vec::new();
            for c in e["choices"].as_array().into_iter().flatten() {
                let (w, v) = (f(&c[0]), &c[1]);
                let choice = match cat {
                    Cat::Monster => match v.as_str().and_then(crate::monsters::spec) {
                        Some(s) => Choice::Monster(s.row),
                        None => continue,
                    },
                    Cat::Chest => match v.as_i64() {
                        Some(id) => Choice::Chest(id as i32),
                        None => match v["mimic"].as_str().and_then(crate::monsters::spec) {
                            Some(s) => Choice::Mimic(s.row, f(&v["yaw"]) as f32),
                            None => continue,
                        },
                    },
                    Cat::Altar => Choice::Altar(altar(v.as_str())),
                    Cat::Trap => Choice::Trap(TrapKind::Spikes),
                    Cat::Escape | Cat::Extract => Choice::Portal(if v.as_str() == Some("Down") { PortalKind::Down } else { PortalKind::Extract }),
                    Cat::Start | Cat::Arrival => continue,
                    Cat::Destroyable => match v.as_str().and_then(crate::destroyables::spec) {
                        Some(s) => Choice::Destroyable(class_name(&s.class)),
                        None => continue,
                    },
                };
                choices.push((w, choice));
            }
            let p = &e["pos"];
            let delay = (f(&e["delay"][0]) as f32, f(&e["delay"][1]) as f32);
            // the spawn point's wandering and patrol (map-local floor points until the floor is built).
            let patrol = e["patrol"].as_array().filter(|p| !p.is_empty()).map(|pts| {
                let points = pts.iter().map(|q| (Vec3::new(f(&q[0]) as f32, f(&q[1]) as f32, f(&q[2]) as f32), f(&q[3]) as f32)).collect();
                Arc::new(crate::ai::Patrol { mode: e["patrol_mode"].as_u64().unwrap_or(0) as u8, points })
            });
            let opts = crate::ai::SpawnOpts { dormant: e["dormant"].as_bool(), dormant_sight: e["dormant_sight"].as_f64().map(|v| v as f32), wander: e["wander"].as_bool(), wander_radius: e["wander_radius"].as_f64().map(|v| v as f32), patrol, death_trigger: e["mechanism"].as_u64().map(|m| (m as u32, e["trigger"].as_u64().unwrap_or(1) as u8)) };
            out.entries.push(Entry { cat, pos: Vec3::new(f(&p[0]) as f32, f(&p[1]) as f32, f(&p[2]) as f32), yaw: (f(&e["yaw"]) as f32).to_radians(), choices, group: e["group"].as_u64().map(|g| g as usize), chance: e["chance"].as_f64().unwrap_or(1.0), delay, opts, scale: v3(&e["scale"], 1.0), opens: e["opens"].as_array().map(|o| o.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect()).unwrap_or_default() });
        }
    }
    Ok(floors)
}

/// A multi-floor map whose floor 1 has no down portal in its data (the castle): each of its escape points may lead
/// down, at [`tune::DOWN_SHARE_WITHOUT_DATA`].
fn down_without_data(floors: &mut [CryptFloor]) {
    let [first, _, ..] = floors else { return };
    let down = |c: &(f64, Choice)| matches!(c.1, Choice::Portal(PortalKind::Down));
    if first.entries.iter().any(|e| e.choices.iter().any(down)) {
        return;
    }
    for e in first.entries.iter_mut().filter(|e| e.cat == Cat::Escape) {
        let total: f64 = e.choices.iter().map(|c| c.0).sum();
        let share = tune::DOWN_SHARE_WITHOUT_DATA;
        e.choices.push((total * share / (1.0 - share), Choice::Portal(PortalKind::Down)));
    }
}

/// A file under `_data/` ([`db_formats::data_root`]).
fn data_path(rel: &str) -> anyhow::Result<std::path::PathBuf> {
    Some(db_formats::data_path(rel)).filter(|p| p.is_file()).ok_or_else(|| anyhow::anyhow!("missing _data/{rel}"))
}

impl CryptMaps {
    /// Loads the map's layout and each floor's collision (offset) and walkable grid.
    pub fn load(def: MapDef) -> anyhow::Result<Self> {
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(data_path(&format!("recovered/maps/{}", def.run_file))?)?)?;
        let mut floors = parse(&json, def.levels.len())?;
        down_without_data(&mut floors);
        let (mut collision, mut nav) = (Vec::new(), Vec::new());
        for (i, level) in def.levels.iter().enumerate() {
            let mut map = MapCollision::load_floor(level)?;
            map.offset = def.offset(i);
            let grid = NavGrid::load_floor(level)?;
            // Collision only where anyone can go: the walkable grid's extent, grown.
            if !grid.nodes.is_empty() {
                let (lo, hi) = grid.nodes.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
                map.retain_within(lo - Vec3::splat(tune::COLLIDE_BEYOND_NAV), hi + Vec3::splat(tune::COLLIDE_BEYOND_NAV));
            }
            collision.push(Arc::new(map));
            nav.push(Arc::new(grid));
        }
        Ok(CryptMaps { def, floors, collision, nav })
    }

    /// [`CryptMaps::load`] plus every floor's collision shapes built ([`MapCollision::prepare`]), for a
    /// background thread.
    pub fn load_prepared(def: MapDef) -> anyhow::Result<Self> {
        let maps = Self::load(def)?;
        for c in &maps.collision {
            c.prepare();
        }
        Ok(maps)
    }

    /// Where floor `floor` sits in the world (cm).
    pub fn offset(&self, floor: usize) -> Vec3 {
        self.def.offset(floor)
    }

    /// Every floor for `seed`. Positions are world (floor offset applied); characters are capsule centres.
    pub fn dungeon(&self, seed: u64) -> Dungeon {
        let mut rng = SimRng::new(seed);
        // The crypt's floor 2 has no reachable extraction in the converted scene: a stand-in.
        let stand_in = self.def.key == CRYPT.key;
        let floors = (0..self.floors.len()).map(|i| build_floor(&mut rng, &self.floors[i], &self.nav[i], i, self.offset(i), stand_in)).collect();
        Dungeon { seed, floors }
    }
}

fn pick<T: Copy>(rng: &mut SimRng, rows: &[(f64, T)]) -> Option<T> {
    let total: f64 = rows.iter().map(|r| r.0).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = rng.unit_f64() * total;
    for &(w, v) in rows {
        if x < w {
            return Some(v);
        }
        x -= w;
    }
    rows.last().map(|r| r.1)
}

/// Takes one of `items` (index, weight) by weight; None when no weight is left.
fn take_weighted(rng: &mut SimRng, items: &mut Vec<(usize, f64)>) -> Option<usize> {
    let total: f64 = items.iter().map(|i| i.1.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut r = rng.unit_f64() * total;
    let at = items.iter().position(|i| {
        r -= i.1.max(0.0);
        r < 0.0
    });
    Some(items.remove(at.unwrap_or(items.len() - 1)).0)
}

/// Which groups and entries are live after the room rolls (CONFIRMED from `BP_ActorSpawnRoom` /
/// `BP_ActorSpawnSet` bytecode): a room without `FixedSpawnNumber` spawns one of its sets, picked by the
/// sets' `Chance` weights, and each point of it with its own `Chance`; a room with `FixedSpawnNumber` N
/// over S sets gives N % S sets (by weight, no repeats) N / S + 1 points and the others N / S, each set
/// picking its points by weight without repeats.
fn roll_groups(rng: &mut SimRng, fl: &CryptFloor) -> Vec<bool> {
    let n = fl.groups.len();
    // Children of each group: groups first, then entries (offset by n).
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (g, gr) in fl.groups.iter().enumerate() {
        if let Some(p) = gr.parent.filter(|&p| p < n) {
            children[p].push(g);
        }
    }
    for (e, en) in fl.entries.iter().enumerate() {
        if let Some(g) = en.group.filter(|&g| g < n) {
            children[g].push(n + e);
        }
    }
    let mut live = vec![true; n + fl.entries.len()];
    // Points whose set picked them by count (their chance was a weight).
    let mut counted = vec![false; fl.entries.len()];
    for (g, gr) in fl.groups.iter().enumerate() {
        let sets: Vec<usize> = children[g].iter().copied().filter(|&c| c < n).collect();
        if !gr.room || sets.is_empty() {
            continue;
        }
        let mut left: Vec<(usize, f64)> = sets.iter().map(|&s| (s, fl.groups[s].chance)).collect();
        match gr.fixed {
            Some(0) if tune::FIXED_ZERO_ROOM_SPAWNS_ALL => {}
            Some(k) => {
                let (q, r) = (k as usize / sets.len(), k as usize % sets.len());
                let mut quota = vec![0; n];
                for i in 0..sets.len() {
                    let want = q + usize::from(i < r);
                    if want == 0 {
                        break;
                    }
                    if let Some(s) = take_weighted(rng, &mut left) {
                        quota[s] = want;
                    }
                }
                for &s in &sets {
                    let mut points: Vec<(usize, f64)> = children[s].iter().filter(|&&c| c >= n).map(|&c| (c - n, fl.entries[c - n].chance)).collect();
                    let mut keep = Vec::new();
                    for _ in 0..quota[s] {
                        keep.extend(take_weighted(rng, &mut points));
                    }
                    for &(e, _) in &points {
                        live[n + e] = false;
                    }
                    for e in keep {
                        counted[e] = true;
                    }
                    if quota[s] == 0 {
                        live[s] = false;
                    }
                }
            }
            None => {
                let pick = take_weighted(rng, &mut left);
                for &s in &sets {
                    live[s] = Some(s) == pick;
                }
            }
        }
    }
    for (e, en) in fl.entries.iter().enumerate() {
        if !counted[e] && en.chance < 1.0 && !rng.chance(en.chance) {
            live[n + e] = false;
        }
    }
    // A child is live only if every group above it is.
    let up = |mut g: Option<usize>, live: &[bool]| {
        let mut depth = 0;
        while let Some(x) = g.filter(|&x| x < n && depth < 64) {
            if !live[x] {
                return false;
            }
            g = fl.groups[x].parent;
            depth += 1;
        }
        true
    };
    (0..fl.entries.len()).map(|e| live[n + e] && up(fl.entries[e].group, &live)).collect()
}

fn build_floor(rng: &mut SimRng, fl: &CryptFloor, nav: &NavGrid, index: usize, off: Vec3, stand_in: bool) -> Floor {
    let live = roll_groups(rng, fl);
    let node = |p: Vec3| nav.nearest(p, tune::ON_GRID.0, tune::ON_GRID.1);
    let comp = |p: Vec3| node(p).map(|n| nav.component[n as usize]);
    // Start (floor 1) or arrival (floor 2): its walkable area is where the run happens.
    // Only the escape points this run spawns (rooms spawn some of their sets).
    let escapes: Vec<u32> = fl.entries.iter().zip(&live).filter(|(e, l)| **l && e.cat == Cat::Escape).filter_map(|(e, _)| comp(e.pos)).collect();
    let want = if index == 0 { Cat::Start } else { Cat::Arrival };
    let mut starts: Vec<&Entry> = fl.entries.iter().filter(|e| e.cat == want && node(e.pos - Vec3::Z * CAPSULE_CENTER_Z).is_some()).collect();
    if index == 0 && tune::START_NEEDS_ESCAPE {
        let ok: Vec<&Entry> = starts.iter().copied().filter(|e| comp(e.pos - Vec3::Z * CAPSULE_CENTER_Z).is_some_and(|c| escapes.contains(&c))).collect();
        if !ok.is_empty() {
            starts = ok;
        }
    } else if index == 1 {
        // The arrival group in the biggest walkable area.
        let sizes = nav.component_sizes();
        let best = starts.iter().filter_map(|e| comp(e.pos - Vec3::Z * CAPSULE_CENTER_Z)).max_by_key(|&c| sizes[c as usize]);
        starts.retain(|e| comp(e.pos - Vec3::Z * CAPSULE_CENTER_Z) == best);
    }
    let start = if starts.is_empty() { fl.entries.iter().find(|e| e.cat == want) } else { Some(starts[rng.range_u32(0, starts.len() as u32) as usize]) };
    // Starts and arrivals are capsule positions (the PlayerStart capsule); stand them on their node.
    let (start_pos, start_yaw) = start.map_or((Vec3::Z * CAPSULE_CENTER_Z, 0.0), |s| {
        let feet = node(s.pos - Vec3::Z * CAPSULE_CENTER_Z).map_or(s.pos - Vec3::Z * CAPSULE_CENTER_Z, |n| nav.nodes[n as usize]);
        (feet + Vec3::Z * (CAPSULE_CENTER_Z + 2.0), s.yaw)
    });
    let mut floor = Floor { index: index as u32, pieces: Vec::new(), rooms: Vec::new(), corridors: Vec::new(), links: Vec::new(), start: start_pos + off, start_yaw, monsters: Vec::new(), boss_room: None, chests: Vec::new(), portals: Vec::new(), doors: Vec::new(), swing_doors: fl.doors.iter().map(|(p, pos, yaw, scale)| (p.clone(), *pos + off, *yaw, *scale)).collect(), mechanisms: fl.mechanisms.iter().map(|m| m.offset(off).on_floor(index)).collect(), traps: Vec::new(), altars: Vec::new(), destroyables: Vec::new(), zone: None, collapse: fl.collapse.clone().map(|c| (c, off)) };
    // its zone's discs, from their own stream (the layout's rolls stay as they were).
    floor.zone = fl.zone.as_ref().map(|spec| crate::death_zone::ZonePlan { spec: spec.clone(), centres: spec.plan(&mut SimRng::new(rng.seed() ^ (0x5A0E_0000 + index as u64)), off), off });
    for (e, en) in fl.entries.iter().enumerate() {
        if !live[e] || matches!(en.cat, Cat::Start | Cat::Arrival) {
            continue;
        }
        // A prop stands where placed (on shelves and tables too), not on the grid.
        if en.cat == Cat::Destroyable {
            if let Some(Choice::Destroyable(class)) = pick(rng, &en.choices) {
                floor.destroyables.push((class, en.pos + off, en.yaw.to_degrees(), en.scale));
            }
            continue;
        }
        let Some(n) = node(en.pos) else { continue };
        let at = nav.nodes[n as usize] + off;
        match pick(rng, &en.choices) {
            // Flyers stay where their spawn point floats (the point is the capsule centre); walkers stand on
            // their node. Flyers (their Blueprint's `Flyable`) keep the placed height. Patrol points: a flyer's
            // where placed, a walker's on their grid node at capsule-centre height.
            Some(Choice::Monster(row)) if crate::monsters::spec(row).and_then(crate::monsters::blueprint).map_or(crate::overrides::ai::FLYERS.contains(&row), |b| b.flyable) => floor.monsters.push((en.pos + off, en.yaw, row, placed(&en.opts, index, |p| p + off))),
            Some(Choice::Monster(row)) => floor.monsters.push((at + Vec3::Z * CAPSULE_CENTER_Z, en.yaw, row, placed(&en.opts, index, |p| node(p).map_or(p, |n| nav.nodes[n as usize]) + off + Vec3::Z * CAPSULE_CENTER_Z))),
            Some(Choice::Chest(id)) => floor.chests.push((at, id)),
            Some(Choice::Mimic(row, yaw)) => floor.monsters.push((at + Vec3::Z * CAPSULE_CENTER_Z, en.yaw + yaw.to_radians(), row, crate::ai::SpawnOpts { dormant: Some(true), dormant_sight: None, ..Default::default() })),
            Some(Choice::Altar(Some(kind))) => floor.altars.push((at, kind)),
            Some(Choice::Trap(kind)) => floor.traps.push(TrapSpot { pos: at, kind, yaw: en.yaw, phase: 0.0 }),
            Some(Choice::Portal(kind)) => {
                let opens_in = if en.cat == Cat::Extract { tune::EXTRACT_DELAY } else { en.delay.0 + (en.delay.1 - en.delay.0).max(0.0) * rng.unit_f32() };
                let opens = en.opens.iter().map(|m| m + index as u32 * crate::mechanisms::FLOOR_STRIDE).collect();
                floor.portals.push(Portal { pos: at, kind, opens_in, opens });
            }
            _ => {}
        }
    }
    if index == 1 && stand_in && tune::STAND_IN_EXTRACT
        && let Some(a) = node(start_pos - Vec3::Z * (CAPSULE_CENTER_Z + 2.0))
        && let Some(far) = farthest(nav, a)
    {
        floor.portals.push(Portal { pos: nav.nodes[far as usize] + off, kind: PortalKind::Extract, opens_in: tune::EXTRACT_DELAY, opens: Vec::new() });
    }
    floor
}

/// The node of `a`'s area farthest from it by walking steps.
fn farthest(nav: &NavGrid, a: u32) -> Option<u32> {
    let mut dist = vec![u32::MAX; nav.nodes.len()];
    let mut queue = std::collections::VecDeque::from([a]);
    dist[a as usize] = 0;
    let mut last = a;
    while let Some(n) = queue.pop_front() {
        last = n;
        for &m in nav.neighbours(n) {
            if dist[m as usize] == u32::MAX {
                dist[m as usize] = dist[n as usize] + 1;
                queue.push_back(m);
            }
        }
    }
    (last != a).then_some(last)
}

/// Spawn options with the patrol points moved by `to` (map-local floor point -> world point).
fn placed(opts: &crate::ai::SpawnOpts, floor: usize, to: impl Fn(Vec3) -> Vec3) -> crate::ai::SpawnOpts {
    let mut o = opts.clone();
    // Its death trigger names the floor's mechanism by the dungeon-wide index.
    o.death_trigger = opts.death_trigger.map(|(m, p)| (m + floor as u32 * crate::mechanisms::FLOOR_STRIDE, p));
    o.patrol = opts.patrol.as_ref().map(|p| Arc::new(crate::ai::Patrol { mode: p.mode, points: p.points.iter().map(|&(q, w)| (to(q), w)).collect() }));
    o
}

/// Walking waypoints (world x/y) on floor `floor` from `from` to `to` (world floor points), or `None`.
pub fn route(maps: &CryptMaps, floor: usize, from: Vec3, to: Vec3) -> Option<Vec<Vec2>> {
    route_avoiding(maps, floor, from, to, &[], 0.0)
}

/// [`route`] with `cost` cm added for each grid point inside one of the `avoid` discs (world centre, radius; 3 m
/// either way in height), so the walk goes round them where it can (the dev bot past sleeping monsters).
pub fn route_avoiding(maps: &CryptMaps, floor: usize, from: Vec3, to: Vec3, avoid: &[(Vec3, f32)], cost: f32) -> Option<Vec<Vec2>> {
    let (nav, off) = (&maps.nav[floor], maps.offset(floor));
    let a = nav.nearest(from - off, 300.0, 200.0)?;
    let b = nav.nearest(to - off, 300.0, 200.0)?;
    let inside = |m: u32| {
        let p = nav.nodes[m as usize] + off;
        avoid.iter().any(|&(c, r)| (c.z - p.z).abs() < 300.0 && c.truncate().distance_squared(p.truncate()) < r * r)
    };
    Some(nav.path_weighted(a, b, 0.0, None, |m| if inside(m) { cost } else { 0.0 })?.into_iter().map(|p| (p + off).truncate()).collect())
}

/// A prop class's name as a `'static` str (the specs live for the whole run).
fn class_name(class: &str) -> &'static str {
    static NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut names = NAMES.lock().expect("class names");
    match names.iter().find(|n| **n == class) {
        Some(n) => n,
        None => {
            let n: &'static str = Box::leak(class.to_string().into_boxed_str());
            names.push(n);
            n
        }
    }
}

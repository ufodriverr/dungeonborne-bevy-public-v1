//! Seeded two-floor greybox dungeon. The real maps (`L_Dungeon_Official_01/02`) are blocked by the
//! mappings, so the layout is ours (HYPOTHESIS): a grid of cells, a random connected set of rooms grown
//! from the start cell (randomised Prim, so every room is reachable), corridors between neighbouring rooms,
//! a few extra corridors for loops. Each floor gets monsters (one spawn group per room), chests with real
//! loot sources, portals (one down on floor 1, extraction portals on floor 2), and doors, loop traps and
//! altars (on their own RNG stream). Units: cm, Z up; floor 2 sits `FLOOR_OFFSET_X` away along +X so both
//! floors can live in one physics world.

use std::collections::{BTreeSet, VecDeque};

use bevy_math::{Vec2, Vec3};
use db_formats::config::generated::ConfigTables;

use crate::SimRng;
use crate::features::{ALTAR_SPAWN_SOLO, AltarKind, TrapKind};
use crate::level::{LevelPiece, Surface};
use crate::overrides::dungeon as tune;

/// Map 1 (`ExcelMapSelect` "Map_1_*"): `L_Dungeon_Official_01` then `_02`, the two-floor crypt.
pub const DUNGEON_MAP_ID: i32 = 1;
/// Loot sources (`ExcelLootSourceMap`).
pub const GOBLIN_AXE_LOOT: i32 = 134;
pub const CHEST_WOOD_01: i32 = 173;
pub const CHEST_IRON_01: i32 = 159;
pub const CHEST_GOLD_01: i32 = 155;

/// Wall half thickness and height (cm), as in the test room.
const T: f32 = 25.0;
const H: f32 = 400.0;

/// An axis-aligned floor area (interior, cm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }
    fn shrink(&self, m: f32) -> Rect {
        Rect { min: self.min + m, max: self.max - m }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalKind {
    /// Floor 1 → floor 2.
    Down,
    /// Ends the run with the inventory kept.
    Extract,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Portal {
    /// Floor point (cm).
    pub pos: Vec3,
    pub kind: PortalKind,
    /// Seconds into the run before it can be used (the crypt's escape delays, 0 for the generator).
    pub opens_in: f32,
    /// the mechanisms (floor indices, as [`crate::mechanisms::MechanismPlacement::index`]) it opens then: its
    /// escape point's `ActivationMechanisms`, the escape doors.
    pub opens: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    /// Grid cell (column, row).
    pub cell: (u32, u32),
    pub rect: Rect,
    /// Corridor steps from the start room.
    pub depth: u32,
}

/// Where a loop trap goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrapSpot {
    /// Floor point under the centre of its damage area.
    pub pos: Vec3,
    pub kind: TrapKind,
    /// Facing (radians): along the corridor, or where the fire statue breathes.
    pub yaw: f32,
    /// Seconds into its loop at the start.
    pub phase: f32,
}

/// One floor: geometry plus where things go. All positions are absolute (floor offset included).
#[derive(Debug, Clone, PartialEq)]
pub struct Floor {
    pub index: u32,
    pub pieces: Vec<LevelPiece>,
    pub rooms: Vec<Room>,
    pub corridors: Vec<Rect>,
    /// The two rooms (indices into `rooms`) each corridor joins, same order as `corridors`.
    pub links: Vec<(usize, usize)>,
    /// Player capsule centre at the start (or arrival) and yaw (radians, 0 = +X).
    pub start: Vec3,
    pub start_yaw: f32,
    /// Monster capsule centres, facing yaw, curve-table row (a [`crate::monsters::ROSTER`] or
    /// [`crate::monsters::BOSSES`] entry) and the spawn point's settings.
    pub monsters: Vec<(Vec3, f32, &'static str, crate::ai::SpawnOpts)>,
    /// Floor 2's boss room (index into `rooms`); it holds only the boss.
    pub boss_room: Option<usize>,
    /// Chest floor points and loot sources.
    pub chests: Vec<(Vec3, i32)>,
    pub portals: Vec<Portal>,
    /// Doors, closed at the start: floor point under the centre, leaf along X.
    pub doors: Vec<(Vec3, bool)>,
    /// A map's swing doors: blueprint package, origin, yaw (degrees), scale.
    pub swing_doors: Vec<(String, Vec3, f32, Vec3)>,
    /// The lever mechanisms with their levers, world space.
    pub mechanisms: Vec<crate::mechanisms::MechanismPlacement>,
    pub traps: Vec<TrapSpot>,
    /// Altar floor points and kinds.
    pub altars: Vec<(Vec3, AltarKind)>,
    /// A map's breakable props: class, origin, yaw (degrees), scale.
    pub destroyables: Vec<(&'static str, Vec3, f32, Vec3)>,
    /// Its death zone: the generator's floors have none.
    pub zone: Option<crate::death_zone::ZonePlan>,
    /// Its floor collapse and the floor's offset.
    pub collapse: Option<(std::sync::Arc<crate::floor_collapse::CollapseSpec>, Vec3)>,
}

impl Floor {
    /// The walkable interior: rooms and corridors.
    pub fn walkable(&self) -> impl Iterator<Item = Rect> + '_ {
        self.rooms.iter().map(|r| r.rect).chain(self.corridors.iter().copied())
    }

    /// Walking waypoints (cm, x/y) from room `from` to room `to` through room centres and corridor
    /// middles (breadth-first over the corridors), or `None` if not connected.
    pub fn route(&self, from: usize, to: usize) -> Option<Vec<Vec2>> {
        let mut prev: Vec<Option<(usize, usize)>> = vec![None; self.rooms.len()];
        let mut queue = VecDeque::from([from]);
        let mut seen = vec![false; self.rooms.len()];
        seen[from] = true;
        while let Some(r) = queue.pop_front() {
            for (c, &(a, b)) in self.links.iter().enumerate() {
                let other = if a == r { b } else if b == r { a } else { continue };
                if !seen[other] {
                    seen[other] = true;
                    prev[other] = Some((r, c));
                    queue.push_back(other);
                }
            }
        }
        if !seen[to] {
            return None;
        }
        let mut points = vec![self.rooms[to].rect.center()];
        let mut at = to;
        while let Some((p, c)) = prev[at] {
            points.push(self.corridors[c].center());
            points.push(self.rooms[p].rect.center());
            at = p;
        }
        points.reverse();
        Some(points)
    }

    /// The room containing a point.
    pub fn room_at(&self, p: Vec2) -> Option<usize> {
        self.rooms.iter().position(|r| r.rect.contains(p))
    }

    /// Bounds of the floor slab (cm, x/y).
    pub fn bounds(&self) -> Rect {
        let o = Vec2::new(self.index as f32 * tune::FLOOR_OFFSET_X, 0.0);
        Rect { min: o, max: o + Vec2::new(tune::GRID.0 as f32, tune::GRID.1 as f32) * tune::CELL }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dungeon {
    pub seed: u64,
    pub floors: Vec<Floor>,
}

/// Index (0-based) of the floor whose slab lies under sim x `x` (cm); floors sit `FLOOR_OFFSET_X` apart
/// (the crypt floors lie wholly below 0 and above it, [`crate::overrides::crypt::FLOOR_OFFSETS`]).
pub fn floor_of(x: f32) -> u32 {
    (x / tune::FLOOR_OFFSET_X).floor().clamp(0.0, 1.0) as u32
}

/// Run length in seconds for a map (`ExcelMapDetail.timeout`; map 1: 2400 s = 40 minutes, CONFIRMED).
pub fn run_seconds(t: &ConfigTables, map_id: i32) -> Option<i32> {
    t.excel_map_detail.iter().find(|m| m.map_id == map_id).map(|m| m.timeout)
}

fn range(rng: &mut SimRng, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * rng.unit_f32()
}

fn pick<'a, T>(rng: &mut SimRng, v: &'a [T]) -> &'a T {
    &v[rng.range_u32(0, v.len() as u32) as usize]
}

/// Builds both floors from one seed.
pub fn generate(seed: u64) -> Dungeon {
    let mut rng = SimRng::new(seed);
    // Monster types on their own stream, so they don't move the layout.
    let mut types = SimRng::new(seed ^ tune::MONSTER_TYPE_STREAM);
    let mut bosses = SimRng::new(seed ^ tune::BOSS_STREAM);
    let mut features = SimRng::new(seed ^ crate::overrides::features::FEATURE_STREAM);
    let floors = (0..2).map(|i| generate_floor(&mut rng, &mut types, &mut bosses, &mut features, i)).collect();
    Dungeon { seed, floors }
}

fn neighbours((c, r): (u32, u32)) -> impl Iterator<Item = (u32, u32)> {
    let (w, h) = tune::GRID;
    [(c.wrapping_sub(1), r), (c + 1, r), (c, r.wrapping_sub(1)), (c, r + 1)].into_iter().filter(move |&(c, r)| c < w && r < h)
}

/// A wall along X (`horizontal`) or Y at `at`, spanning `from..to`, with gaps (centre, half width).
fn wall(pieces: &mut Vec<LevelPiece>, horizontal: bool, at: f32, from: f32, to: f32, mut gaps: Vec<f32>, surface: Surface) {
    gaps.sort_by(f32::total_cmp);
    let mut start = from;
    let mut push = |a: f32, b: f32| {
        if b - a > 1.0 {
            let (c, h) = ((a + b) * 0.5, (b - a) * 0.5);
            let (center, half) = if horizontal { (Vec3::new(c, at, H / 2.0), Vec3::new(h, T, H / 2.0)) } else { (Vec3::new(at, c, H / 2.0), Vec3::new(T, h, H / 2.0)) };
            pieces.push(LevelPiece { center, half_extents: half, surface });
        }
    };
    for g in gaps {
        push(start, g - tune::CORRIDOR_HALF_WIDTH);
        start = g + tune::CORRIDOR_HALF_WIDTH;
    }
    push(start, to);
}

/// A free spot inside a room: clear of pillars and of earlier spots (and the start point).
fn free_spot(rng: &mut SimRng, r: Rect, clearance: f32, blockers: &[(Vec2, Vec2)], used: &mut Vec<Vec2>) -> Option<Vec2> {
    let area = r.shrink(tune::SPAWN_MARGIN);
    for _ in 0..30 {
        let p = Vec2::new(range(rng, area.min.x, area.max.x), range(rng, area.min.y, area.max.y));
        let clear_of_pillars = blockers.iter().all(|(c, h)| ((p - *c).abs() - *h).max_element() > clearance);
        if clear_of_pillars && used.iter().all(|u| u.distance(p) > 2.0 * clearance) {
            used.push(p);
            return Some(p);
        }
    }
    None
}

/// A weighted pick of a row name.
fn weighted(rng: &mut SimRng, rows: &[(&'static str, f64)]) -> &'static str {
    let total: f64 = rows.iter().map(|r| r.1).sum();
    let mut x = rng.unit_f64() * total;
    for &(name, w) in rows {
        if x < w {
            return name;
        }
        x -= w;
    }
    rows[rows.len() - 1].0
}

fn generate_floor(rng: &mut SimRng, types: &mut SimRng, bosses: &mut SimRng, features: &mut SimRng, index: u32) -> Floor {
    let (gw, gh) = tune::GRID;
    let origin = Vec2::new(index as f32 * tune::FLOOR_OFFSET_X, 0.0);
    let cell_center = |(c, r): (u32, u32)| origin + (Vec2::new(c as f32, r as f32) + 0.5) * tune::CELL;
    let (lo, hi) = tune::ROOMS[index as usize];
    let target = rng.range_u32(lo, hi + 1) as usize;

    // Rooms: grow a connected set from a start cell on the grid's edge (randomised Prim).
    let start_cell = (0, rng.range_u32(0, gh));
    let mut cells = vec![start_cell];
    let mut taken: BTreeSet<(u32, u32)> = BTreeSet::from([start_cell]);
    let mut edges: BTreeSet<((u32, u32), (u32, u32))> = BTreeSet::new();
    while cells.len() < target {
        let frontier: Vec<((u32, u32), (u32, u32))> = cells.iter().flat_map(|&a| neighbours(a).filter(|b| !taken.contains(b)).map(move |b| (a, b))).collect();
        if frontier.is_empty() {
            break;
        }
        let &(a, b) = pick(rng, &frontier);
        taken.insert(b);
        cells.push(b);
        edges.insert((a.min(b), a.max(b)));
    }
    // A few loops.
    for &a in &cells {
        for b in neighbours(a).filter(|b| taken.contains(b) && a < *b) {
            if !edges.contains(&(a, b)) && rng.chance(tune::EXTRA_CORRIDOR_CHANCE) {
                edges.insert((a, b));
            }
        }
    }

    // Room rectangles: random size; the centre may move but the room always covers its cell centre
    // ± (corridor half width + 50), so straight corridors through cell centres always meet it.
    let half_cell = tune::CELL * 0.5;
    let mut rooms: Vec<Room> = cells
        .iter()
        .map(|&cell| {
            let half = Vec2::new(range(rng, tune::ROOM_HALF.0, tune::ROOM_HALF.1), range(rng, tune::ROOM_HALF.0, tune::ROOM_HALF.1));
            let jitter = (half - (tune::CORRIDOR_HALF_WIDTH + 50.0)).min(Vec2::splat(half_cell - 3.0 * T) - half).min(Vec2::splat(tune::ROOM_JITTER)).max(Vec2::ZERO);
            let c = cell_center(cell) + Vec2::new(range(rng, -jitter.x, jitter.x), range(rng, -jitter.y, jitter.y));
            Room { cell, rect: Rect { min: c - half, max: c + half }, depth: 0 }
        })
        .collect();
    let room_at = |rooms: &[Room], cell: (u32, u32)| rooms.iter().position(|r| r.cell == cell).expect("room for cell");

    // Depth from the start room (corridor steps).
    let mut seen = vec![false; rooms.len()];
    let mut queue = VecDeque::from([(0usize, 0u32)]);
    seen[0] = true;
    while let Some((i, d)) = queue.pop_front() {
        rooms[i].depth = d;
        for &(a, b) in &edges {
            let other = if a == rooms[i].cell { b } else if b == rooms[i].cell { a } else { continue };
            let j = room_at(&rooms, other);
            if !seen[j] {
                seen[j] = true;
                queue.push_back((j, d + 1));
            }
        }
    }

    // Geometry: one floor and one ceiling slab, room walls with door gaps, corridor side walls.
    let size = Vec2::new(gw as f32, gh as f32) * tune::CELL;
    let mid = origin + size * 0.5;
    let mut pieces = vec![
        LevelPiece { center: mid.extend(-T), half_extents: (size * 0.5).extend(T), surface: Surface::Floor },
        LevelPiece { center: mid.extend(H + T), half_extents: (size * 0.5).extend(T), surface: Surface::Ceiling },
    ];
    let mut corridors = Vec::new();
    let mut links = Vec::new();
    // Gaps per room side: (east, west, north, south) gap centres.
    let mut gaps: Vec<[Vec<f32>; 4]> = vec![Default::default(); rooms.len()];
    for &(a, b) in &edges {
        let (ia, ib) = (room_at(&rooms, a), room_at(&rooms, b));
        let (ra, rb) = (rooms[ia].rect, rooms[ib].rect);
        let w = tune::CORRIDOR_HALF_WIDTH;
        if a.1 == b.1 {
            // Along X: a is west of b (edges are ordered).
            let y = cell_center(a).y;
            let rect = Rect { min: Vec2::new(ra.max.x, y - w), max: Vec2::new(rb.min.x, y + w) };
            wall(&mut pieces, true, y - w - T, rect.min.x, rect.max.x, vec![], Surface::Wall);
            wall(&mut pieces, true, y + w + T, rect.min.x, rect.max.x, vec![], Surface::Wall);
            corridors.push(rect);
            links.push((ia, ib));
            gaps[ia][0].push(y);
            gaps[ib][1].push(y);
        } else {
            // Along Y: a is south of b.
            let x = cell_center(a).x;
            let rect = Rect { min: Vec2::new(x - w, ra.max.y), max: Vec2::new(x + w, rb.min.y) };
            wall(&mut pieces, false, x - w - T, rect.min.y, rect.max.y, vec![], Surface::Wall);
            wall(&mut pieces, false, x + w + T, rect.min.y, rect.max.y, vec![], Surface::Wall);
            corridors.push(rect);
            links.push((ia, ib));
            gaps[ia][2].push(x);
            gaps[ib][3].push(x);
        }
    }
    for (room, [east, west, north, south]) in rooms.iter().zip(gaps.iter().cloned()) {
        let r = room.rect;
        let moss = if room.depth % 3 == 2 { Surface::MossWall } else { Surface::Wall };
        wall(&mut pieces, false, r.max.x + T, r.min.y - 2.0 * T, r.max.y + 2.0 * T, east, moss);
        wall(&mut pieces, false, r.min.x - T, r.min.y - 2.0 * T, r.max.y + 2.0 * T, west, moss);
        wall(&mut pieces, true, r.max.y + T, r.min.x, r.max.x, north, moss);
        wall(&mut pieces, true, r.min.y - T, r.min.x, r.max.x, south, moss);
    }

    // Pillars in big rooms (at the quarter points, away from the corridor lines through the centre).
    let mut blockers: Vec<(Vec2, Vec2)> = Vec::new();
    for room in &rooms {
        let r = room.rect;
        let half = (r.max - r.min) * 0.5;
        if half.min_element() >= tune::PILLAR_ROOM_HALF && rng.chance(tune::PILLAR_CHANCE) {
            let c = r.center();
            for s in [Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0), Vec2::new(1.0, -1.0)] {
                let p = c + s * half * 0.5;
                pieces.push(LevelPiece { center: p.extend(H / 2.0), half_extents: Vec3::new(tune::PILLAR_HALF, tune::PILLAR_HALF, H / 2.0), surface: Surface::Wall });
                blockers.push((p, Vec2::splat(tune::PILLAR_HALF)));
            }
        }
    }

    // Start (or arrival) room: the first room; face along its first corridor.
    let start_room = &rooms[0];
    let start2 = start_room.rect.center();
    let start_yaw = edges
        .iter()
        .find_map(|&(a, b)| if a == start_room.cell { Some(b) } else if b == start_room.cell { Some(a) } else { None })
        .map_or(0.0, |other| {
            let d = cell_center(other) - cell_center(start_room.cell);
            d.y.atan2(d.x)
        });

    let mut used: Vec<Vec2> = vec![start2];

    // Portals in the deepest rooms: floor 1 one down, floor 2 the extraction portals.
    let mut by_depth: Vec<usize> = (1..rooms.len()).collect();
    by_depth.sort_by_key(|&i| std::cmp::Reverse(rooms[i].depth));
    let (count, kind) = if index == 0 { (1, PortalKind::Down) } else { (tune::EXTRACT_PORTALS, PortalKind::Extract) };
    let mut portals = Vec::new();
    for &i in by_depth.iter().take(count) {
        if let Some(p) = free_spot(rng, rooms[i].rect, tune::PORTAL_CLEARANCE, &blockers, &mut used) {
            portals.push(Portal { pos: p.extend(0.0), kind, opens_in: 0.0, opens: Vec::new() });
        }
    }

    // Monsters and chests in every room but the start room.
    let mut monsters = Vec::new();
    let mut chests = Vec::new();
    let (mlo, mhi) = tune::MONSTERS_PER_ROOM[index as usize];
    let groups: Vec<&[(&'static str, f64)]> = crate::monsters::SPAWN_GROUPS.iter().chain(tune::EXTRA_GROUPS).map(|g| g.1).collect();
    for room in rooms.iter().skip(1) {
        let group = *pick(types, &groups);
        for _ in 0..rng.range_u32(mlo, mhi + 1) {
            if let Some(p) = free_spot(rng, room.rect, tune::MONSTER_CLEARANCE, &blockers, &mut used) {
                let yaw = range(rng, 0.0, std::f32::consts::TAU);
                monsters.push((p.extend(tune::CAPSULE_CENTER_Z), yaw, weighted(types, group), Default::default()));
            }
        }
        if rng.chance(tune::CHEST_CHANCE) {
            // Against the south wall (the lid hinge side), clear of door gaps and pillars.
            let r = room.rect;
            let half = Vec3::from(crate::overrides::world_items::CHEST_HALF_EXTENTS);
            for _ in 0..10 {
                let x = range(rng, r.min.x + half.x + 20.0, r.max.x - half.x - 20.0);
                let p = Vec2::new(x, r.min.y + half.y + 5.0);
                let near_door = (x - cell_center(room.cell).x).abs() < tune::CORRIDOR_HALF_WIDTH + half.x + 30.0;
                let clear = blockers.iter().all(|(c, h)| ((p - *c).abs() - *h - half.truncate()).max_element() > 10.0) && used.iter().all(|u| u.distance(p) > 120.0);
                if !near_door && clear {
                    used.push(p);
                    let roll = rng.unit_f64();
                    let source = if index == 1 && roll < tune::GOLD_CHEST_CHANCE { CHEST_GOLD_01 } else if roll < tune::IRON_CHEST_CHANCE { CHEST_IRON_01 } else { CHEST_WOOD_01 };
                    chests.push((p.extend(0.0), source));
                    break;
                }
            }
        }
    }

    // Floor 2: the deepest room past the extraction portals' rooms holds a boss instead.
    let boss_room = (index == 1).then(|| by_depth.iter().copied().skip(count).find(|&i| i != 0)).flatten();
    if let Some(i) = boss_room {
        let r = rooms[i].rect;
        monsters.retain(|m| !r.contains(m.0.truncate()));
        let groups: Vec<&[(&'static str, f64)]> = crate::monsters::BOSS_GROUPS.iter().chain(std::iter::once(&tune::MAP_BOSS_GROUP)).map(|g| g.1).collect();
        let group = *pick(bosses, &groups);
        monsters.push((r.center().extend(tune::CAPSULE_CENTER_Z), range(bosses, 0.0, std::f32::consts::TAU), weighted(bosses, group), Default::default()));
    }

    let (doors, traps, altars) = place_features(features, &rooms, &corridors, &links, boss_room, &blockers, &used, cell_center);
    Floor { index, pieces, rooms, corridors, links, start: start2.extend(tune::CAPSULE_CENTER_Z + 2.0), start_yaw, monsters, boss_room, chests, portals, doors, swing_doors: Vec::new(), mechanisms: Vec::new(), traps, altars, destroyables: Vec::new(), zone: None, collapse: None }
}

/// Doors, traps and altars (HYPOTHESIS placement: the real ones sit in the blocked maps). A door in
/// the middle of some corridors; a spike patch or swinging blade in some other corridors (none next to
/// the start room); fire statues against some rooms' north walls; altar spawn points in some rooms,
/// each rolled from `DT_ActorSpawn_Altar_Solo`. All clear of what is already placed.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn place_features(rng: &mut SimRng, rooms: &[Room], corridors: &[Rect], links: &[(usize, usize)], boss_room: Option<usize>, blockers: &[(Vec2, Vec2)], used: &[Vec2], cell_center: impl Fn((u32, u32)) -> Vec2) -> (Vec<(Vec3, bool)>, Vec<TrapSpot>, Vec<(Vec3, AltarKind)>) {
    use crate::overrides::features as f;
    let cycle = f::LOOP_TRAP_UP + f::LOOP_TRAP_DOWN;
    let mut spots = used.to_vec();
    let (mut doors, mut traps, mut altars) = (Vec::new(), Vec::new(), Vec::new());
    for (rect, &(a, b)) in corridors.iter().zip(links) {
        let along_x = rooms[a].cell.1 == rooms[b].cell.1;
        let length = if along_x { rect.max.x - rect.min.x } else { rect.max.y - rect.min.y };
        if rng.chance(f::DOOR_CHANCE) {
            // The leaf runs across the corridor.
            doors.push((rect.center().extend(0.0), !along_x));
        } else if a != 0 && b != 0 && length >= f::TRAP_MIN_CORRIDOR && rng.chance(f::TRAP_CHANCE) {
            let kind = if rng.chance(0.5) { TrapKind::Spikes } else { TrapKind::Blade };
            let yaw = if along_x { 0.0 } else { std::f32::consts::FRAC_PI_2 };
            traps.push(TrapSpot { pos: rect.center().extend(0.0), kind, yaw, phase: range(rng, 0.0, cycle) });
        }
    }
    for (i, room) in rooms.iter().enumerate().skip(1) {
        if Some(i) == boss_room {
            continue;
        }
        let r = room.rect;
        let mut keep_out = blockers.to_vec();
        if rng.chance(f::FIRE_STATUE_CHANCE) {
            // Against the north wall, breathing south; the statue itself takes `STATUE_DEPTH`.
            let half = TrapKind::FireStatue.half(false).truncate();
            for _ in 0..10 {
                let x = range(rng, r.min.x + half.x + 20.0, r.max.x - half.x - 20.0);
                let c = Vec2::new(x, r.max.y - STATUE_DEPTH - half.y);
                let near_door = (x - cell_center(room.cell).x).abs() < tune::CORRIDOR_HALF_WIDTH + half.x + 30.0;
                let clear = keep_out.iter().all(|(p, h)| ((c - *p).abs() - *h - half).max_element() > 20.0) && spots.iter().all(|u| ((*u - c).abs() - half).max_element() > 60.0);
                if !near_door && clear && r.max.y - r.min.y > 2.0 * (half.y + STATUE_DEPTH) + 100.0 {
                    spots.push(c);
                    keep_out.push((c, half + 40.0));
                    traps.push(TrapSpot { pos: c.extend(0.0), kind: TrapKind::FireStatue, yaw: -std::f32::consts::FRAC_PI_2, phase: range(rng, 0.0, f::LOOP_MECHANISM_UP + f::LOOP_MECHANISM_DOWN) });
                    break;
                }
            }
        }
        if rng.chance(f::ALTAR_CHANCE) {
            let total: f64 = ALTAR_SPAWN_SOLO.iter().map(|a| a.1).sum();
            let mut x = rng.unit_f64() * total;
            let kind = ALTAR_SPAWN_SOLO.iter().find(|a| {
                let hit = x < a.1;
                x -= a.1;
                hit
            });
            if let Some(&(Some(kind), _)) = kind
                && let Some(p) = free_spot(rng, r, ALTAR_CLEARANCE, &keep_out, &mut spots)
            {
                altars.push((p.extend(0.0), kind));
            }
        }
    }
    (doors, traps, altars)
}

/// Depth the fire statue's body takes in front of its wall (cm); its fire starts there.
pub const STATUE_DEPTH: f32 = 60.0;
/// Altars keep this far from other things (cm).
const ALTAR_CLEARANCE: f32 = 80.0;

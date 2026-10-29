//! Walkable grid over a real map: baked offline from the map collision (`db_pipeline nav <NN>`) into
//! `_data/collision/L_Dungeon_Official_<NN>-nav.bin`. Every grid column (`step` cm apart) is probed from above, surface by
//! surface; each upward-facing surface with headroom and no wall within a capsule radius becomes a node (several per column
//! on stacked floors). Neighbouring nodes (8 directions) are joined when the height step is small, nothing blocks at knee
//! and chest height (swept balls above step height and at the chest), there is floor between them, and no single ledge on
//! the way is higher than a character steps (the floor sampled at every eighth: stairs pass, a 50 cm ledge doesn't). Paths
//! are A* over the nodes. Map-local space (cm, Z up, the floor offset not applied); positions are floor points (feet), not
//! capsule centres. HYPOTHESIS: a stand-in for the original navmesh (not converted); thin gaps and props block as drawn.
//!
//! File format (little-endian): `DBNAV1\0\0`, f32 step, u32 node count, f32 xyz per node, u32 edge
//! count, (node count + 1) u32 offsets into the edges, u32 neighbour per edge.

use std::collections::{BinaryHeap, HashMap};

use avian3d::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use bevy_math::{Dir3, Vec2, Vec3};

/// Bake settings (HYPOTHESIS: chosen for the player capsule, `movement::CAPSULE_RADIUS` ~ 35 cm).
const HEADROOM: f32 = 180.0;
const CLEAR_RADIUS: f32 = 28.0;
const CLEAR_HEIGHT: f32 = 90.0;
const MAX_STEP: f32 = 60.0;
/// Neighbours on a slope join up to this rise per cm across (about 43.5 degrees, under the character's walkable
/// ~45 degrees, `overrides::movement::WALKABLE_NORMAL_Z`); the castle's rocky ground rises 60-100 cm per metre,
/// which `MAX_STEP` alone cut into thousands of areas. The per-eighth rise test still stops ledges.
const SLOPE: f32 = 0.95;
const KNEE: f32 = 45.0;
const CHEST: f32 = 120.0;
/// A ball this big is swept between neighbours at chest height (rays slipped between the bars of
/// floor 2's iron fences, where the capsule can't pass).
const EDGE_RADIUS: f32 = 20.0;
/// The clearance ball is swept between neighbours this high, its bottom just over a full step
/// (`movement::MAX_STEP_HEIGHT`): a body-wide check above what a character steps over (a diagonal link
/// onto a stair's side, which a ray passes at its low corner but the capsule doesn't).
const HIP: f32 = crate::overrides::movement::MAX_STEP_HEIGHT + CLEAR_RADIUS + 2.0;
/// Monster routes keep off walls and drops: a node costs this much (cm) more per neighbour it lacks or that
/// lies more than [`LEVEL`] lower, so a route runs a node in from a wall or a stair's open side, where a
/// capsule wider than the bake's clearance would rub or slip off (ours, HYPOTHESIS).
const EDGE_COST: f32 = 40.0;
const LEVEL: f32 = 10.0;
/// Upward surfaces only (cos of the steepest walkable slope, ~45°).
const MIN_NORMAL_Z: f32 = 0.7;

/// The walkable grids of the loaded map's floors with their world offsets ([`crate::SimCommand::AddNav`]), and
/// the nodes shut doors cut from them. Monsters walk their `BTT_MoveTo` along these paths when the straight
/// way is blocked.
#[derive(Resource, Debug, Clone, Default)]
pub struct NavMaps(pub Vec<(std::sync::Arc<NavGrid>, Vec3)>, pub NavCuts);

/// Nodes cut from the grids by shut doors (a closed `BP_SwingDoor` is null area on the navmesh): per grid,
/// which nodes are cut and the areas left without them (empty while nothing is cut).
#[derive(Debug, Clone, Default)]
pub struct NavCuts {
    pub grids: Vec<NavCut>,
    /// Which doors and grids the cuts were made for ([`NavMaps::set_cuts`]).
    pub key: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NavCut {
    pub blocked: Vec<bool>,
    pub component: Vec<u32>,
}

impl NavCut {
    fn open(&self, n: u32) -> bool {
        self.blocked.get(n as usize).is_none_or(|b| !b)
    }

    fn area(&self, g: &NavGrid, n: u32) -> u32 {
        self.component.get(n as usize).copied().unwrap_or(g.component[n as usize])
    }
}

impl NavMaps {
    fn cut(&self, i: usize) -> Option<&NavCut> {
        self.1.grids.get(i).filter(|c| !c.blocked.is_empty())
    }

    /// Floor points (feet, world cm) from floor point `from` to floor point `to`, on the first grid with nodes
    /// near both in one connected area; `None` without one.
    pub fn route(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        self.0.iter().enumerate().find_map(|(i, (g, off))| {
            let cut = self.cut(i);
            let open = |n: u32| cut.is_none_or(|c| c.open(n));
            let a = g.nearest_where(from - *off, g.step * 1.5, 150.0, open)?;
            let b = g.nearest_where(to - *off, g.step * 3.0, 250.0, open)?;
            Some(g.path_cut(a, b, EDGE_COST, cut)?.into_iter().map(|p| p + *off).collect())
        })
    }

    /// A random floor point (world) within `radius` cm of floor point `at`, among the nodes of its area
    /// (`GetRandomLocationInNavigableRadius`); `None` off the grid.
    pub fn random_point(&self, at: Vec3, radius: f32, rng: &mut crate::SimRng) -> Option<Vec3> {
        self.0.iter().enumerate().find_map(|(i, (g, off))| {
            let local = at - *off;
            let cut = self.cut(i);
            let a = g.nearest_where(local, g.step * 1.5, 150.0, |n| cut.is_none_or(|c| c.open(n)))?;
            let area = |n: u32| cut.map_or(g.component[n as usize], |c| c.area(g, n));
            let home = area(a);
            let near = g.within(local, radius, |n| area(n) == home && cut.is_none_or(|c| c.open(n)));
            // A radius too small to take in a grid point: the nearest one (a castle monster's ability asked for one
            // and the empty pick panicked).
            let pick = if near.is_empty() { a } else { near[rng.range_u32(0, near.len() as u32) as usize] };
            Some(g.nodes[pick as usize] + *off)
        })
    }

    /// Cuts the nodes under `boxes` (world centre, yaw in radians, half extents: shut door leaves) from every grid,
    /// unless `key` (the doors' state) and the grid count are what the cuts were last made for.
    pub fn set_cuts(&mut self, boxes: &[(Vec3, f32, Vec3)], key: u64) {
        let key = key ^ (self.0.len() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        if key == self.1.key {
            return;
        }
        self.1.key = key;
        self.1.grids = self.0.iter().map(|(g, off)| {
            let mut blocked = vec![false; g.nodes.len()];
            let mut any = false;
            for &(c, yaw, half) in boxes {
                for n in g.in_box(c - *off, yaw, half, g.step) {
                    blocked[n as usize] = true;
                    any = true;
                }
            }
            if any { NavCut { component: g.components_without(&blocked), blocked } } else { NavCut::default() }
        }).collect();
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NavGrid {
    pub step: f32,
    pub nodes: Vec<Vec3>,
    pub offsets: Vec<u32>,
    pub edges: Vec<u32>,
    /// Connected component of each node (filled by [`NavGrid::index`]).
    pub component: Vec<u32>,
    columns: HashMap<(i32, i32), Vec<u32>>,
}

impl NavGrid {
    pub fn neighbours(&self, n: u32) -> &[u32] {
        &self.edges[self.offsets[n as usize] as usize..self.offsets[n as usize + 1] as usize]
    }

    fn cell(&self, p: Vec2) -> (i32, i32) {
        ((p.x / self.step).round() as i32, (p.y / self.step).round() as i32)
    }

    /// Builds the column lookup and the connected components.
    fn index(mut self) -> Self {
        self.columns.clear();
        for (i, p) in self.nodes.iter().enumerate() {
            let c = self.cell(p.truncate());
            self.columns.entry(c).or_default().push(i as u32);
        }
        let mut component = vec![u32::MAX; self.nodes.len()];
        let mut next = 0;
        for s in 0..self.nodes.len() {
            if component[s] != u32::MAX {
                continue;
            }
            let mut stack = vec![s as u32];
            component[s] = next;
            while let Some(n) = stack.pop() {
                for &m in self.neighbours(n) {
                    if component[m as usize] == u32::MAX {
                        component[m as usize] = next;
                        stack.push(m);
                    }
                }
            }
            next += 1;
        }
        self.component = component;
        self
    }

    /// Size of each component, by component id.
    pub fn component_sizes(&self) -> Vec<u32> {
        let mut sizes = vec![0; self.component.iter().max().map_or(0, |m| *m as usize + 1)];
        for &c in &self.component {
            sizes[c as usize] += 1;
        }
        sizes
    }

    /// The node closest to floor point `p` within `max_h` cm sideways and `max_v` cm up or down.
    pub fn nearest(&self, p: Vec3, max_h: f32, max_v: f32) -> Option<u32> {
        self.nearest_where(p, max_h, max_v, |_| true)
    }

    /// [`NavGrid::nearest`] among the nodes `keep` passes.
    pub fn nearest_where(&self, p: Vec3, max_h: f32, max_v: f32, keep: impl Fn(u32) -> bool) -> Option<u32> {
        let (cx, cy) = self.cell(p.truncate());
        let r = (max_h / self.step).ceil() as i32;
        let mut best: Option<(f32, u32)> = None;
        for dx in -r..=r {
            for dy in -r..=r {
                for &n in self.columns.get(&(cx + dx, cy + dy)).map_or(&[][..], |v| v.as_slice()) {
                    let q = self.nodes[n as usize];
                    let (h, v) = ((q - p).truncate().length(), (q.z - p.z).abs());
                    if h <= max_h && v <= max_v && keep(n) {
                        let d = h + v * 2.0;
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, n));
                        }
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The nodes `keep` passes (their area, say) within `radius` cm sideways of `p` (and 300 cm up or down), in id
    /// order.
    pub fn within(&self, p: Vec3, radius: f32, keep: impl Fn(u32) -> bool) -> Vec<u32> {
        let (cx, cy) = self.cell(p.truncate());
        let r = (radius / self.step).ceil() as i32;
        let mut out = Vec::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for &n in self.columns.get(&(cx + dx, cy + dy)).map_or(&[][..], |v| v.as_slice()) {
                    let q = self.nodes[n as usize];
                    if keep(n) && (q - p).truncate().length() <= radius && (q.z - p.z).abs() <= 300.0 {
                        out.push(n);
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The nodes under a box (centre `c`, `yaw` radians, half extents `half`) grown by `pad` cm sideways: a node
    /// counts from a step below the box's bottom to its top (a shut door's leaf).
    pub fn in_box(&self, c: Vec3, yaw: f32, half: Vec3, pad: f32) -> Vec<u32> {
        let (cx, cy) = self.cell(c.truncate());
        let r = ((half.truncate().length() + pad) / self.step).ceil() as i32;
        let inv = Vec2::from_angle(-yaw);
        let mut out = Vec::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for &n in self.columns.get(&(cx + dx, cy + dy)).map_or(&[][..], |v| v.as_slice()) {
                    let q = self.nodes[n as usize];
                    let d = inv.rotate((q - c).truncate());
                    if d.x.abs() <= half.x + pad && d.y.abs() <= half.y + pad && q.z >= c.z - half.z - MAX_STEP && q.z <= c.z + half.z {
                        out.push(n);
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The connected areas with the `blocked` nodes taken out (blocked nodes get `u32::MAX`).
    pub fn components_without(&self, blocked: &[bool]) -> Vec<u32> {
        let mut component = vec![u32::MAX; self.nodes.len()];
        let mut next = 0;
        for s in 0..self.nodes.len() {
            if component[s] != u32::MAX || blocked[s] {
                continue;
            }
            let mut stack = vec![s as u32];
            component[s] = next;
            while let Some(n) = stack.pop() {
                for &m in self.neighbours(n) {
                    if component[m as usize] == u32::MAX && !blocked[m as usize] {
                        component[m as usize] = next;
                        stack.push(m);
                    }
                }
            }
            next += 1;
        }
        component
    }

    /// A* from node `a` to node `b`: the node positions along the way (`a` and `b` included).
    pub fn path(&self, a: u32, b: u32) -> Option<Vec<Vec3>> {
        self.path_with(a, b, 0.0)
    }

    /// How many of its 8 neighbours node `n` lacks or has more than [`LEVEL`] below it.
    fn edge(&self, n: u32) -> u32 {
        let z = self.nodes[n as usize].z;
        8 - self.neighbours(n).iter().filter(|&&m| self.nodes[m as usize].z >= z - LEVEL).count().min(8) as u32
    }

    /// [`NavGrid::path`] with `edge_cost` cm added per neighbour a node lacks or has below it ([`EDGE_COST`]).
    pub fn path_with(&self, a: u32, b: u32, edge_cost: f32) -> Option<Vec<Vec3>> {
        self.path_cut(a, b, edge_cost, None)
    }

    /// [`NavGrid::path_with`] off the nodes `cut` blocks.
    pub fn path_cut(&self, a: u32, b: u32, edge_cost: f32, cut: Option<&NavCut>) -> Option<Vec<Vec3>> {
        self.path_weighted(a, b, edge_cost, cut, |_| 0.0)
    }

    /// [`NavGrid::path_cut`] with `extra(m)` cm added for stepping onto node `m` (the dev bot's detours).
    pub fn path_weighted(&self, a: u32, b: u32, edge_cost: f32, cut: Option<&NavCut>, extra: impl Fn(u32) -> f32) -> Option<Vec<Vec3>> {
        let area = |n: u32| cut.map_or(self.component.get(n as usize).copied(), |c| c.component.get(n as usize).copied());
        if area(a) != area(b) || area(a) == Some(u32::MAX) {
            return None;
        }
        let goal = self.nodes[b as usize];
        let mut cost = vec![f32::INFINITY; self.nodes.len()];
        let mut prev = vec![u32::MAX; self.nodes.len()];
        let mut open = BinaryHeap::new();
        cost[a as usize] = 0.0;
        open.push(Open(self.nodes[a as usize].distance(goal), a));
        while let Some(Open(_, n)) = open.pop() {
            if n == b {
                let mut out = vec![goal];
                let mut at = b;
                while prev[at as usize] != u32::MAX {
                    at = prev[at as usize];
                    out.push(self.nodes[at as usize]);
                }
                out.reverse();
                return Some(out);
            }
            let here = self.nodes[n as usize];
            for &m in self.neighbours(n) {
                if cut.is_some_and(|c| !c.open(m)) {
                    continue;
                }
                let c = cost[n as usize] + here.distance(self.nodes[m as usize]) + if edge_cost > 0.0 { edge_cost * self.edge(m) as f32 } else { 0.0 } + extra(m);
                if c < cost[m as usize] {
                    cost[m as usize] = c;
                    prev[m as usize] = n;
                    open.push(Open(c + self.nodes[m as usize].distance(goal), m));
                }
            }
        }
        None
    }

    pub fn read(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut at = 0usize;
        let mut take = |n: usize| -> anyhow::Result<&[u8]> {
            let s = bytes.get(at..at + n).ok_or_else(|| anyhow::anyhow!("nav file truncated"))?;
            at += n;
            Ok(s)
        };
        anyhow::ensure!(take(8)? == b"DBNAV1\0\0", "not a nav file");
        let f = |b: &[u8]| f32::from_le_bytes(b.try_into().unwrap());
        let u = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
        let step = f(take(4)?);
        let n = u(take(4)?) as usize;
        let mut nodes = Vec::with_capacity(n);
        for _ in 0..n {
            let b = take(12)?;
            nodes.push(Vec3::new(f(&b[0..4]), f(&b[4..8]), f(&b[8..12])));
        }
        let m = u(take(4)?) as usize;
        let offsets: Vec<u32> = take((n + 1) * 4)?.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect();
        let edges: Vec<u32> = take(m * 4)?.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect();
        anyhow::ensure!(at == bytes.len(), "trailing bytes in nav file");
        anyhow::ensure!(offsets.windows(2).all(|w| w[0] <= w[1]) && offsets.last() == Some(&(m as u32)), "bad nav offsets");
        anyhow::ensure!(edges.iter().all(|&e| (e as usize) < n), "nav edge out of range");
        Ok(NavGrid { step, nodes, offsets, edges, ..Default::default() }.index())
    }

    pub fn write(&self) -> Vec<u8> {
        let mut b = b"DBNAV1\0\0".to_vec();
        b.extend(self.step.to_le_bytes());
        b.extend((self.nodes.len() as u32).to_le_bytes());
        for p in &self.nodes {
            for v in [p.x, p.y, p.z] {
                b.extend(v.to_le_bytes());
            }
        }
        b.extend((self.edges.len() as u32).to_le_bytes());
        for v in self.offsets.iter().chain(&self.edges) {
            b.extend(v.to_le_bytes());
        }
        b
    }

    /// Loads `_data/collision/L_Dungeon_Official_<floor>-nav.bin` ([`db_formats::data_root`]).
    pub fn load_floor(floor: &str) -> anyhow::Result<Self> {
        let rel = format!("collision/{}-nav.bin", crate::crypt::level_name(floor));
        let path = Some(db_formats::data_path(&rel)).filter(|p| p.is_file()).ok_or_else(|| anyhow::anyhow!("no nav file _data/{rel} (db_pipeline nav)"))?;
        Self::read(&std::fs::read(path)?)
    }
}

#[derive(PartialEq)]
struct Open(f32, u32);
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Open {
    // Lowest estimate first (BinaryHeap is a max-heap).
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// Why two neighbouring floor points aren't joined (diagnostics; see [`link_cut`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cut {
    /// A ray at knee height (either way) or at chest height hits.
    Knee,
    Chest,
    /// The small ball swept at chest height hits (bars, fences).
    Bars,
    /// The clearance ball swept just over a step hits.
    Hip,
    /// No floor under the midpoint.
    NoFloor,
    /// A rise on the way higher than a character steps.
    Ledge,
}

fn blocked(spatial: &SpatialQuery, a: Vec3, b: Vec3) -> bool {
    Dir3::new(b - a).is_ok_and(|d| spatial.cast_ray(a, d, (b - a).length(), true, &crate::layers::walk()).is_some())
}

/// Why floor points `a` and `b` (a column apart) can't be joined, or `None` when they can: nothing blocks at
/// knee and chest height, the edge ball and the clearance ball pass, there is floor between them, and no rise
/// on the way (the floor sampled at every eighth) is over 80 % of a character's step
/// (`movement::MAX_STEP_HEIGHT`; the capsule meets a ledge's face higher than the sample below it).: a 50 cm
/// ledge on crypt floor 2 joined nodes the bot couldn't walk; stair risers (15-25 cm) pass.
pub fn link_cut(spatial: &SpatialQuery, a: Vec3, b: Vec3) -> Option<Cut> {
    let all = crate::layers::walk();
    let swept = |r: f32, a: Vec3, b: Vec3| Dir3::new(b - a).is_ok_and(|d| spatial.cast_shape(&Collider::sphere(r), a, bevy_math::Quat::IDENTITY, d, &ShapeCastConfig::from_max_distance((b - a).length()), &all).is_some());
    if blocked(spatial, a + Vec3::Z * KNEE, b + Vec3::Z * KNEE) || blocked(spatial, b + Vec3::Z * KNEE, a + Vec3::Z * KNEE) {
        return Some(Cut::Knee);
    }
    if blocked(spatial, a + Vec3::Z * CHEST, b + Vec3::Z * CHEST) {
        return Some(Cut::Chest);
    }
    if swept(EDGE_RADIUS, a + Vec3::Z * CHEST, b + Vec3::Z * CHEST) {
        return Some(Cut::Bars);
    }
    if swept(CLEAR_RADIUS, a + Vec3::Z * HIP, b + Vec3::Z * HIP) {
        return Some(Cut::Hip);
    }
    let mid = (a + b) * 0.5;
    if !blocked(spatial, mid + Vec3::Z * 80.0, mid - Vec3::Z * 80.0) {
        return Some(Cut::NoFloor);
    }
    let top = a.z.max(b.z) + MAX_STEP;
    let mut heights = vec![a.z];
    for t in [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875] {
        let p = a.lerp(b, t);
        let from = Vec3::new(p.x, p.y, top);
        if let Some(hit) = spatial.cast_ray(from, Dir3::NEG_Z, top - a.z.min(b.z) + MAX_STEP, true, &all) {
            heights.push(top - hit.distance);
        }
    }
    heights.push(b.z);
    (!heights.windows(2).all(|w| (w[1] - w[0]).abs() <= crate::overrides::movement::MAX_STEP_HEIGHT * 0.8)).then_some(Cut::Ledge)
}

/// A sim world with only map floor `floor`'s collision, and each collider's collision instance index
/// (diagnostics).
fn diagnostic_world(floor: &str) -> anyhow::Result<(bevy_app::App, HashMap<Entity, u32>)> {
    let map = std::sync::Arc::new(crate::map::MapCollision::load_floor(floor)?);
    let instances: HashMap<[i32; 3], u32> = map.instances.iter().enumerate().map(|(i, t)| ((t.translation + map.offset).to_array().map(|v| (v * 10.0).round() as i32), i as u32)).collect();
    let mut app = crate::headless_app(0);
    app.world_mut().resource_mut::<crate::CommandQueue>().push(crate::SimCommand::AddMapCollision(map));
    crate::step(&mut app, 1);
    let world = app.world_mut();
    let of = world.query::<(Entity, &Position, &crate::command::LevelBox)>().iter(world).filter_map(|(e, p, _)| Some((e, *instances.get(&p.0.to_array().map(|v| (v * 10.0).round() as i32))?))).collect();
    Ok((app, of))
}

/// [`link_cut`] for each pair, over the collision of map floor `floor`, with the collision instance a
/// knee or chest ray hits first (diagnostics).
pub fn link_cuts(floor: &str, pairs: &[(Vec3, Vec3)]) -> anyhow::Result<Vec<(Option<Cut>, Option<u32>)>> {
    let (mut app, of) = diagnostic_world(floor)?;
    let world = app.world_mut();
    let mut state: SystemState<SpatialQuery> = SystemState::new(world);
    let spatial = state.get(world).expect("spatial query");
    let all = crate::layers::walk();
    let hit = |a: Vec3, b: Vec3| Dir3::new(b - a).ok().and_then(|d| spatial.cast_ray(a, d, (b - a).length(), true, &all)).map(|h| of.get(&h.entity).copied().unwrap_or(u32::MAX));
    Ok(pairs
        .iter()
        .map(|&(a, b)| {
            let cut = link_cut(&spatial, a, b);
            let by = match cut {
                Some(Cut::Knee) => hit(a + Vec3::Z * KNEE, b + Vec3::Z * KNEE).or_else(|| hit(b + Vec3::Z * KNEE, a + Vec3::Z * KNEE)),
                Some(Cut::Chest) => hit(a + Vec3::Z * CHEST, b + Vec3::Z * CHEST),
                _ => None,
            };
            (cut, by)
        })
        .collect())
}

/// Why a column has no node near height `p.z` (diagnostics), on map floor `floor`: for each point, the floor surface found from a step (60
/// cm) above it down to 150 cm below and what rejects it, each blocker as its collision instance index.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnCut {
    NoSurface,
    Steep(f32),
    /// The headroom ray hits this instance this far above the floor (cm).
    Headroom(u32, f32),
    /// The clearance ball overlaps these instances.
    Clearance(Vec<u32>),
    /// Nothing rejects it (the node exists at another height, or the column is outside the bake).
    Fine(f32),
}

pub fn column_cuts(floor: &str, points: &[Vec3]) -> anyhow::Result<Vec<ColumnCut>> {
    let (mut app, of) = diagnostic_world(floor)?;
    let world = app.world_mut();
    let mut state: SystemState<SpatialQuery> = SystemState::new(world);
    let spatial = state.get(world).expect("spatial query");
    let all = crate::layers::walk();
    let ball = Collider::sphere(CLEAR_RADIUS);
    let inst = |e: Entity| of.get(&e).copied().unwrap_or(u32::MAX);
    Ok(points
        .iter()
        .map(|&p| {
            let Some(h) = spatial.cast_ray(p + Vec3::Z * MAX_STEP, Dir3::NEG_Z, MAX_STEP + 150.0, true, &all) else { return ColumnCut::NoSurface };
            let s = p + Vec3::Z * (MAX_STEP - h.distance);
            if h.normal.z < MIN_NORMAL_Z {
                return ColumnCut::Steep(h.normal.z);
            }
            if let Some(up) = spatial.cast_ray(s + Vec3::Z * 5.0, Dir3::Z, HEADROOM - 5.0, true, &all) {
                return ColumnCut::Headroom(inst(up.entity), up.distance + 5.0);
            }
            let over = spatial.shape_intersections(&ball, s + Vec3::Z * CLEAR_HEIGHT, bevy_math::Quat::IDENTITY, &all);
            if !over.is_empty() {
                return ColumnCut::Clearance(over.into_iter().map(inst).collect());
            }
            ColumnCut::Fine(s.z)
        })
        .collect())
}

/// Bakes the grid over `min..max` (cm, x/y) between heights `z_min..z_max` from the colliders in `world`
/// (a sim world with only the map's colliders spawned and one step run, so the query tree is built).
pub fn bake(world: &mut World, min: Vec2, max: Vec2, z_min: f32, z_max: f32, step: f32) -> NavGrid {
    let mut state: SystemState<SpatialQuery> = SystemState::new(world);
    let spatial = state.get(world).expect("spatial query");
    let all = crate::layers::walk();
    let blocked = |a: Vec3, b: Vec3| blocked(&spatial, a, b);
    let ball = Collider::sphere(CLEAR_RADIUS);
    let (nx, ny) = (((max.x - min.x) / step).ceil() as i32, ((max.y - min.y) / step).ceil() as i32);
    let (ix0, iy0) = ((min.x / step).floor() as i32, (min.y / step).floor() as i32);
    // The queries only read the world, so the columns are probed on all cores (the castle is about
    // 600,000 columns); each thread takes a run of `ix` rows and the runs are joined in order, so the
    // grid is the same as a single-threaded bake.
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let rows: Vec<i32> = (ix0..=ix0 + nx).collect();
    let chunk = rows.len().div_ceil(threads * 4).max(1);
    let probe = |ixs: &[i32]| -> Vec<((i32, i32), Vec3)> {
        let mut out = Vec::new();
        for &ix in ixs {
            for iy in iy0..=iy0 + ny {
                let (x, y) = (ix as f32 * step, iy as f32 * step);
                // Down the column surface by surface (one collider can hold a room's roof and floor).
                let mut from = Vec3::new(x, y, z_max);
                let mut surfaces = Vec::new();
                while let Some(h) = (surfaces.len() < 24).then(|| spatial.cast_ray(from, Dir3::NEG_Z, from.z - z_min, true, &all)).flatten() {
                    let p = from - Vec3::Z * h.distance;
                    if h.normal.z >= MIN_NORMAL_Z {
                        surfaces.push(p);
                    }
                    from = p - Vec3::Z * 1.0;
                }
                for p in surfaces {
                    if blocked(p + Vec3::Z * 5.0, p + Vec3::Z * HEADROOM) {
                        continue;
                    }
                    if !spatial.shape_intersections(&ball, p + Vec3::Z * CLEAR_HEIGHT, bevy_math::Quat::IDENTITY, &all).is_empty() {
                        continue;
                    }
                    out.push(((ix, iy), p));
                }
            }
        }
        out
    };
    let probed: Vec<((i32, i32), Vec3)> = std::thread::scope(|s| {
        let jobs: Vec<_> = rows.chunks(chunk).map(|part| s.spawn(move || probe(part))).collect();
        jobs.into_iter().flat_map(|j| j.join().expect("nav probe thread")).collect()
    });
    let mut nodes = Vec::with_capacity(probed.len());
    let mut columns: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    for (key, p) in probed {
        columns.entry(key).or_default().push(nodes.len() as u32);
        nodes.push(p);
    }
    // Edges: each node to the closest-height node of its 8 neighbour columns (checked on all cores in node
    // order, then joined in that order).
    let link = |range: std::ops::Range<usize>| -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        for n in range {
            let a = nodes[n];
            let (ix, iy) = ((a.x / step).round() as i32, (a.y / step).round() as i32);
            for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                let Some(other) = columns.get(&(ix + dx, iy + dy)) else { continue };
                let rise = MAX_STEP.max(step * ((dx * dx + dy * dy) as f32).sqrt() * SLOPE);
                let Some(&m) = other.iter().filter(|&&m| (nodes[m as usize].z - a.z).abs() <= rise).min_by(|&&p, &&q| (nodes[p as usize].z - a.z).abs().total_cmp(&(nodes[q as usize].z - a.z).abs())) else { continue };
                if link_cut(&spatial, a, nodes[m as usize]).is_some() {
                    continue;
                }
                out.push((n as u32, m));
            }
        }
        out
    };
    let count = nodes.len();
    let per = count.div_ceil(threads * 4).max(1);
    let pairs: Vec<(u32, u32)> = std::thread::scope(|s| {
        let link = &link;
        let jobs: Vec<_> = (0..count).step_by(per).map(|start| s.spawn(move || link(start..(start + per).min(count)))).collect();
        jobs.into_iter().flat_map(|j| j.join().expect("nav link thread")).collect()
    });
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
    for (n, m) in pairs {
        adj[n as usize].push(m);
        adj[m as usize].push(n);
    }
    let mut offsets = Vec::with_capacity(nodes.len() + 1);
    let mut edges = Vec::new();
    offsets.push(0);
    for mut a in adj {
        a.sort_unstable();
        edges.extend(a);
        offsets.push(edges.len() as u32);
    }
    NavGrid { step, nodes, offsets, edges, ..Default::default() }.index()
}

/// Bakes floor `floor`'s grid from its collision cache (`map::MapCollision::load_floor`) over the
/// extent of its placed meshes.
pub fn bake_floor(floor: &str, step: f32) -> anyhow::Result<NavGrid> {
    bake_floor_within(floor, step, None)
}

/// [`bake_floor`] over the extent of the level's gameplay points (`<level>-placement.json`: player starts,
/// monster, chest, altar and escape points) grown by `margin` cm, when given (the castle's far scenery
/// spans kilometres around a playable area of about 490 x 310 m).
pub fn bake_floor_within(floor: &str, step: f32, margin: Option<f32>) -> anyhow::Result<NavGrid> {
    let map = std::sync::Arc::new(crate::map::MapCollision::load_floor(floor)?);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for i in &map.instances {
        lo = lo.min(i.translation);
        hi = hi.max(i.translation);
    }
    let (mut lo, mut hi) = (lo - Vec3::new(500.0, 500.0, 1000.0), hi + Vec3::new(500.0, 500.0, 1000.0));
    if let Some(margin) = margin {
        let rel = format!("recovered/maps/{}-placement.json", crate::crypt::level_name(floor));
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(db_formats::data_path(&rel))?)?;
        let points: Vec<Vec3> = json
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| matches!(e["cat"].as_str(), Some("player_start" | "monster" | "chest" | "altar" | "escape_point" | "extract" | "arrival")))
            .filter_map(|e| Some(Vec3::new(e["pos"][0].as_f64()? as f32, e["pos"][1].as_f64()? as f32, e["pos"][2].as_f64()? as f32)))
            .collect();
        anyhow::ensure!(!points.is_empty(), "no gameplay points in _data/{rel}");
        lo = points.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p)) - Vec3::splat(margin);
        hi = points.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(*p)) + Vec3::splat(margin);
    }
    let mut app = crate::headless_app(0);
    app.world_mut().resource_mut::<crate::CommandQueue>().push(crate::SimCommand::AddMapCollision(map));
    crate::step(&mut app, 1);
    Ok(bake(app.world_mut(), lo.truncate(), hi.truncate(), lo.z, hi.z, step))
}

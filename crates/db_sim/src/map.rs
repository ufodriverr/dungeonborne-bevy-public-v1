//! Real crypt map collision: the static geometry of `L_Dungeon_Official_<NN>` from the cache made out of
//! the converted scene (sim space: cm, Z up). Each unique mesh becomes one shared trimesh (per scale) and
//! each placed instance a static collider marked [`LevelBox`], so movement, sight
//! ([`crate::ai::wall_between`]) and projectiles use the real level. HYPOTHESIS: render meshes stand in
//! for the original simple collision (not converted); see-through and tiny pieces are left out by the
//! tool.
//!
//! Cache format (little-endian): `DBCOL2\0\0`, u32 mesh count, per mesh u32 vertex count, u32 triangle count, f32
//! xyz per vertex, u32 abc per triangle; u32 instance count, per instance u32 mesh, f32 translation xyz, f32
//! rotation xyzw, f32 scale xyz, u32 kind (what it stops, [`crate::layers::of_kind`]). `DBCOL1` (no kind: every
//! instance stops everything) still reads.

use std::collections::HashMap;
use std::sync::Arc;

use avian3d::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::{Quat, Vec3};

use crate::command::LevelBox;

/// One unique mesh: vertices (cm, its own frame) and triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// One placed mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapInstance {
    pub mesh: u32,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// Bits 0-7: 0 stops everything, 1 characters only, 2 shots and sight only ([`crate::layers`]). Bits 8-15: the floor
    /// collapse stage it stops colliding at, 16-23: the stage it starts at ([`crate::floor_collapse::CollapsePiece`]). Bits
    /// 24-31: 1, the throne removes it when the King dies ([`crate::throne::ThronePiece`]).
    pub kind: u32,
}

/// A map's collision geometry.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapCollision {
    pub meshes: Vec<MapMesh>,
    pub instances: Vec<MapInstance>,
    /// Added to every instance when spawned (where the floor sits in the world).
    pub offset: Vec3,
    /// The trimesh colliders, built once ([`MapCollision::prepare`]).
    shapes: Shapes,
}

/// One shared trimesh per (mesh, scale) and each instance's index into them (built once per process, on
/// a background thread, instead of on every run start).
#[derive(Debug, Clone, Default)]
struct Shapes(std::sync::OnceLock<(Vec<Collider>, Vec<u32>)>);

impl PartialEq for Shapes {
    /// Derived from the geometry, so they never make two maps differ.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl MapCollision {
    /// Reads a cache file.
    pub fn read(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut r = Reader { b: bytes, at: 0 };
        let magic = r.take(8)?;
        let v2 = magic == b"DBCOL2\0\0";
        anyhow::ensure!(v2 || magic == b"DBCOL1\0\0", "not a collision cache");
        let mut map = MapCollision::default();
        for _ in 0..r.u32()? {
            let (nv, nt) = (r.u32()? as usize, r.u32()? as usize);
            let mut vertices = Vec::with_capacity(nv);
            for _ in 0..nv {
                vertices.push(Vec3::new(r.f32()?, r.f32()?, r.f32()?));
            }
            let mut triangles = Vec::with_capacity(nt);
            for _ in 0..nt {
                let t = [r.u32()?, r.u32()?, r.u32()?];
                anyhow::ensure!(t.iter().all(|&i| (i as usize) < nv), "triangle index out of range");
                triangles.push(t);
            }
            map.meshes.push(MapMesh { vertices, triangles });
        }
        for _ in 0..r.u32()? {
            let mesh = r.u32()?;
            anyhow::ensure!((mesh as usize) < map.meshes.len(), "instance of an unknown mesh");
            let translation = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let rotation = Quat::from_xyzw(r.f32()?, r.f32()?, r.f32()?, r.f32()?).normalize();
            let scale = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let kind = if v2 { r.u32()? } else { 0 };
            anyhow::ensure!(kind & 0xff <= 2 && kind >> 24 <= 1, "unknown collision kind {kind}");
            map.instances.push(MapInstance { mesh, translation, rotation, scale, kind });
        }
        anyhow::ensure!(r.at == bytes.len(), "trailing bytes in collision cache");
        Ok(map)
    }

    /// Keeps only the instances whose bounds (of the scaled, turned mesh; before [`MapCollision::offset`])
    /// touch the box `lo..hi` (the castle's far scenery, kilometres of it that no one reaches, was most of
    /// its 149,070 colliders and seconds of spawning). Call before the shapes are built.
    pub fn retain_within(&mut self, lo: Vec3, hi: Vec3) {
        let bounds: Vec<(Vec3, Vec3)> = self.meshes.iter().map(|m| m.vertices.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), v| (a.min(*v), b.max(*v)))).collect();
        self.instances.retain(|inst| {
            let (a, b) = bounds[inst.mesh as usize];
            if a.x > b.x {
                return false;
            }
            let (mut wa, mut wb) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for k in 0..8 {
                let corner = Vec3::new(if k & 1 == 0 { a.x } else { b.x }, if k & 2 == 0 { a.y } else { b.y }, if k & 4 == 0 { a.z } else { b.z });
                let w = inst.translation + inst.rotation * (corner * inst.scale);
                wa = wa.min(w);
                wb = wb.max(w);
            }
            wa.cmple(hi).all() && wb.cmpge(lo).all()
        });
    }

    /// Builds the trimesh colliders (once; later calls return at once). The unique (mesh, scale) pairs are
    /// taken in instance order and built on all cores; parry builds each one's tree on its own, so the
    /// result doesn't depend on the threads.
    pub fn prepare(&self) {
        self.shapes();
    }

    fn shapes(&self) -> &(Vec<Collider>, Vec<u32>) {
        self.shapes.0.get_or_init(|| {
            let mut keys: Vec<(u32, Vec3)> = Vec::new();
            let mut index: HashMap<(u32, [i32; 3]), u32> = HashMap::new();
            let of_instance = self
                .instances
                .iter()
                .map(|inst| {
                    let key = (inst.mesh, [inst.scale.x, inst.scale.y, inst.scale.z].map(|s| (s * 1000.0).round() as i32));
                    *index.entry(key).or_insert_with(|| {
                        keys.push((inst.mesh, inst.scale));
                        keys.len() as u32 - 1
                    })
                })
                .collect();
            let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
            let chunk = keys.len().div_ceil(threads).max(1);
            let colliders = std::thread::scope(|s| {
                let jobs: Vec<_> = keys
                    .chunks(chunk)
                    .map(|part| {
                        s.spawn(move || {
                            part.iter()
                                .map(|&(mesh, scale)| {
                                    let m = &self.meshes[mesh as usize];
                                    Collider::trimesh(m.vertices.iter().map(|v| *v * scale).collect(), m.triangles.clone())
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                jobs.into_iter().flat_map(|j| j.join().expect("collider build thread")).collect()
            });
            (colliders, of_instance)
        })
    }

    /// Loads `_data/collision/<level>.bin` ([`db_formats::data_root`]); `level` is a full level name or a
    /// short key ([`crate::crypt::level_name`]).
    pub fn load_floor(level: &str) -> anyhow::Result<Self> {
        let rel = format!("collision/{}.bin", crate::crypt::level_name(level));
        let path = Some(db_formats::data_path(&rel)).filter(|p| p.is_file()).ok_or_else(|| anyhow::anyhow!("no collision cache _data/{rel} (tools/recon/map_collision.py)"))?;
        Self::read(&std::fs::read(path)?)
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> anyhow::Result<&[u8]> {
        let s = self.b.get(self.at..self.at + n).ok_or_else(|| anyhow::anyhow!("collision cache truncated"))?;
        self.at += n;
        Ok(s)
    }
    fn u32(&mut self) -> anyhow::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> anyhow::Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }
}

/// A map collider's mesh (its index in [`MapCollision::meshes`]: the client reads the surface under foot by it).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPiece(pub u32);

/// Spawns the map's static colliders: one shared trimesh per (mesh, scale), one entity per instance, in one
/// batch. Static level pieces need no mass, but avian integrates every trimesh instance's mass when the
/// collider is inserted (about 7 s for the crypt with its cells) unless it is a sensor, and again on the
/// first physics step (4.5 s) while it has `ColliderMassProperties`. And avian re-adds a collider to its
/// tree on each of the three inserts a body's collider gets, each re-add scanning the tree's list of moved
/// colliders, which grows with the spawn (quadratic: 7.9 s for about 79,000 colliders). So they spawn as
/// disabled sensors; then, in this order, the mass properties go (so removing `Sensor` has nothing to
/// recompute), `Sensor` goes (movement sweeps skip sensors), and enabling adds each one to the tree once.
pub fn spawn_map(world: &mut World, map: &Arc<MapCollision>) -> usize {
    let (colliders, of_instance) = map.shapes();
    let bundles: Vec<_> = map.instances.iter().zip(of_instance).map(|(inst, &k)| (RigidBody::Static, colliders[k as usize].clone(), ColliderDisabled, Sensor, Position(inst.translation + map.offset), Rotation(inst.rotation), LevelBox { half_extents: Vec3::ZERO }, crate::layers::of_kind(inst.kind & 0xff).unwrap_or_default(), MapPiece(inst.mesh))).collect();
    let spawned: Vec<Entity> = world.spawn_batch(bundles).collect();
    world.flush();
    // The floor collapse's pieces: one that appears at a stage stays disabled until then.
    let mut hidden = std::collections::HashSet::new();
    for (inst, &e) in map.instances.iter().zip(&spawned) {
        let (drop, appear) = (((inst.kind >> 8) & 0xff) as u8, ((inst.kind >> 16) & 0xff) as u8);
        if drop != 0 || appear != 0 {
            world.entity_mut(e).insert(crate::floor_collapse::CollapsePiece { drop, appear, off: map.offset });
            if appear != 0 {
                hidden.insert(e);
            }
        }
        if inst.kind >> 24 == 1 {
            world.entity_mut(e).insert(crate::throne::ThronePiece { off: map.offset });
        }
    }
    for &e in &spawned {
        world.entity_mut(e).remove::<ColliderMassProperties>();
    }
    for &e in &spawned {
        world.entity_mut(e).remove::<Sensor>();
    }
    for &e in spawned.iter().filter(|e| !hidden.contains(*e)) {
        world.entity_mut(e).remove::<ColliderDisabled>();
    }
    spawned.len()
}

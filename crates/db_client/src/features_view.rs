//! Doors, traps and altars in the world: greybox bodies that follow the sim state. Door leaves swing
//! open, spikes rise out of their plate, the blade swings across its corridor, the fire statue breathes
//! fire, a spent altar goes dark. Since the bodies are the blueprints as built (`props_view`: original
//! components and local transforms) where imported, the greybox otherwise. Reads sim state only.

use bevy::prelude::*;
use db_sim::level::Surface;
use db_sim::{Altar, AltarKind, Door, SimId, SimIndex, Trap, TrapKind};

use crate::combat_view::{ActorView, WorldLabel, spawn_label};
use crate::props_view::{in_root, mesh_bounds, node, prop, spawn_prop};
use crate::{surface_material, to_bevy};

/// A moving part of a door, trap or altar body.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) enum Part {
    /// The leaf's hinge pivot; turns open.
    DoorLeaf(SimId),
    /// The spike bed; rises while the trap is up: its rest height (up) and how far it sinks (m).
    Spikes(SimId, f32, f32),
    /// The blade's pivot; swings about this axis (Bevy space).
    Blade(SimId, Vec3),
    /// The fire jet and its light; shown while up.
    Fire(SimId),
    /// The altar's orb; hidden once spent.
    Orb(SimId),
}

/// How far the blade swings to each side of hanging straight down (radians).
const BLADE_SWING: f32 = 0.6;
/// How far the real spike bed sinks when down: the spikes' height (HYPOTHESIS: the timeline's PositionZ
/// curve is normalised and its scale is in the update bytecode, not read).
const SPIKE_TRAVEL: f32 = 0.8;

pub(crate) fn altar_color(kind: AltarKind) -> Color {
    match kind {
        AltarKind::Attack => Color::srgb(1.0, 0.3, 0.2),
        AltarKind::Defend => Color::srgb(0.35, 0.6, 1.0),
        AltarKind::Heal => Color::srgb(0.3, 1.0, 0.4),
        AltarKind::Clarity => Color::srgb(0.75, 0.45, 1.0),
        AltarKind::Speed => Color::srgb(1.0, 0.85, 0.25),
    }
}

/// Door blueprint for the generated dungeon's doors (HYPOTHESIS: the crypt's own doors are in the map
/// cells, this one's leaf, `SM_Door01`, is a plain wooden door hinged at its edge).
const DOOR_CLASS: &str = "BP_FantasyDungeon_Door_07";

/// Bodies for doors, traps and altars as they appear in the sim.
#[allow(clippy::type_complexity)]
pub(crate) fn spawn_feature_views(
    mut commands: Commands,
    added: Query<(&SimId, Option<&Door>, Option<&Trap>, Option<&Altar>), Or<(Added<Door>, Added<Trap>, Added<Altar>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    for (&id, door, trap, altar) in &added {
        let root_id = commands.spawn((ActorView(id), Transform::default(), Visibility::default(), crate::flow::LevelScoped)).id();
        if let Some(door) = door {
            let half = Door::half(door.along_x);
            let size = to_bevy(half * 2.0).abs();
            // Hinged at the leaf's low end; the leaf hangs centred on the doorway when closed.
            let hinge = to_bevy(if door.along_x { Vec3::new(-half.x, 0.0, 0.0) } else { Vec3::new(0.0, -half.y, 0.0) });
            let wood = materials.add(surface_material(Surface::Door, Vec2::ONE, &assets));
            let iron = materials.add(StandardMaterial { metallic: 0.6, perceptual_roughness: 0.5, ..surface_material(Surface::Metal, Vec2::ONE, &assets) });
            if let Some(nodes) = prop(DOOR_CLASS) {
                // The blueprint as built (frame, `Hinge`, leaf), scaled evenly so the leaf spans the doorway;
                // the leaf spans blueprint Z, so along-X doorways turn it a quarter.
                let (lo, hi) = node(nodes, "Door").map_or((Vec3::ZERO, Vec3::ONE), |i| mesh_bounds(nodes, i));
                let span = if door.along_x { size.x } else { size.z };
                let turn = if door.along_x { Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
                let at = Transform { translation: Vec3::new(0.0, -size.y * 0.5, 0.0), rotation: turn, scale: Vec3::splat(span / (hi.z - lo.z).max(0.01)) };
                let ents = spawn_prop(&mut commands, &assets, nodes, root_id, at);
                if let Some(h) = node(nodes, "Hinge") {
                    commands.entity(ents[h]).insert(Part::DoorLeaf(id));
                }
                continue;
            }
            commands.entity(root_id).with_children(|c| {
                c.spawn((Part::DoorLeaf(id), Transform::from_translation(hinge), Visibility::default())).with_children(|p| {
                    p.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(wood), Transform::from_translation(-hinge)));
                    // Two iron bands across the leaf.
                    for y in [-0.35, 0.35] {
                        let band = Vec3::new(if door.along_x { size.x } else { size.x + 0.02 }, 0.08, if door.along_x { size.z + 0.02 } else { size.z });
                        p.spawn((Mesh3d(meshes.add(Cuboid::from_size(band))), MeshMaterial3d(iron.clone()), Transform::from_translation(-hinge + Vec3::Y * y * size.y)));
                    }
                });
            });
            continue;
        }
        if let Some(trap) = trap {
            let size = to_bevy(trap.half * 2.0).abs();
            let floor = -size.y * 0.5;
            let iron = materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.32, 0.34), metallic: 0.7, perceptual_roughness: 0.45, ..default() });
            let real = prop(match trap.kind {
                TrapKind::Spikes => "BP_Trap_FloorSpikes",
                TrapKind::Blade => "BP_Trap_SwingBlade",
                TrapKind::FireStatue => "BP_Trap_FireStatue_02",
            });
            match trap.kind {
                TrapKind::Spikes if real.is_some() => {
                    // The blueprint as built: the plate, and 81 spikes under `MoveNode`, which sinks and rises.
                    let nodes = real.expect("checked");
                    let ents = spawn_prop(&mut commands, &assets, nodes, root_id, Transform::from_xyz(0.0, floor, 0.0));
                    if let Some(m) = node(nodes, "MoveNode") {
                        commands.entity(ents[m]).insert(Part::Spikes(id, nodes[m].t[1], SPIKE_TRAVEL));
                    }
                }
                TrapKind::Spikes => {
                    commands.entity(root_id).with_children(|c| {
                        c.spawn((Mesh3d(meshes.add(Cuboid::new(size.x, 0.04, size.z))), MeshMaterial3d(iron.clone()), Transform::from_xyz(0.0, floor + 0.02, 0.0)));
                        c.spawn((Part::Spikes(id, floor, 0.58), Transform::from_xyz(0.0, floor, 0.0), Visibility::default())).with_children(|s| {
                            let cone = meshes.add(Cone { radius: 0.05, height: 0.6 });
                            let (nx, nz) = ((size.x / 0.3) as i32, (size.z / 0.3) as i32);
                            for i in 0..nx.max(1) {
                                for j in 0..nz.max(1) {
                                    let x = (i as f32 + 0.5) / nx.max(1) as f32 * size.x - size.x * 0.5;
                                    let z = (j as f32 + 0.5) / nz.max(1) as f32 * size.z - size.z * 0.5;
                                    s.spawn((Mesh3d(cone.clone()), MeshMaterial3d(iron.clone()), Transform::from_xyz(x, 0.3, z)));
                                }
                            }
                        });
                    });
                }
                TrapKind::Blade if real.is_some() => {
                    // The blueprint as built: rails under `BoxTrigger`, the axe under `MoveNode`, hung from the
                    // pivot under the ceiling; the swing axis (blueprint X) along the trap's axis.
                    let nodes = real.expect("checked");
                    let along = to_bevy(Vec3::new(trap.yaw.cos(), trap.yaw.sin(), 0.0)).normalize_or_zero();
                    let at = Transform::from_xyz(0.0, 3.8 + floor, 0.0).with_rotation(Quat::from_rotation_arc(Vec3::X, along));
                    let ents = spawn_prop(&mut commands, &assets, nodes, root_id, at);
                    if let Some(m) = node(nodes, "MoveNode") {
                        commands.entity(ents[m]).insert(Part::Blade(id, Vec3::X));
                    }
                }
                TrapKind::Blade => {
                    // Pivot under the ceiling; the blade's broad side lies across the corridor.
                    let along = to_bevy(Vec3::new(trap.yaw.cos(), trap.yaw.sin(), 0.0)).normalize_or_zero();
                    let across = Vec3::Y.cross(along).normalize_or_zero();
                    let pivot_y = 3.8 + floor;
                    let arm = pivot_y - floor - 1.0;
                    let blade = Vec3::new(1.0, 0.32, 1.0) - along.abs() * 0.96 + across.abs() * 0.2;
                    commands.entity(root_id).with_children(|c| {
                        c.spawn((Part::Blade(id, along), Transform::from_xyz(0.0, pivot_y, 0.0), Visibility::default())).with_children(|p| {
                            p.spawn((Mesh3d(meshes.add(Cylinder::new(0.03, arm))), MeshMaterial3d(iron.clone()), Transform::from_xyz(0.0, -arm * 0.5, 0.0)));
                            p.spawn((Mesh3d(meshes.add(Cuboid::from_size(blade))), MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.75, 0.78), metallic: 0.9, perceptual_roughness: 0.25, ..default() })), Transform::from_xyz(0.0, -arm, 0.0)));
                        });
                    });
                }
                TrapKind::FireStatue => {
                    // The statue stands behind the fire's area, against its wall.
                    let face = Vec3::new(trap.yaw.cos(), trap.yaw.sin(), 0.0);
                    let reach = trap.half.truncate().dot(face.truncate().abs());
                    let depth = db_sim::dungeon::STATUE_DEPTH;
                    let body = to_bevy(-face * (reach + depth * 0.5) + Vec3::Z * (90.0 - trap.half.z));
                    let stone = materials.add(surface_material(Surface::Wall, Vec2::ONE, &assets));
                    let fire_color = Color::srgb(1.0, 0.45, 0.1);
                    let fire = materials.add(StandardMaterial { base_color: fire_color.with_alpha(0.55), emissive: LinearRgba::from(fire_color) * 8.0, alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
                    if let Some(nodes) = real {
                        // The blueprint as built (gargoyle on its stand, facing blueprint +Z), on the floor behind the fire.
                        let turn = Quat::from_rotation_arc(Vec3::Z, to_bevy(face).normalize_or_zero());
                        spawn_prop(&mut commands, &assets, nodes, root_id, Transform::from_xyz(body.x, floor, body.z).with_rotation(turn));
                    }
                    commands.entity(root_id).with_children(|c| {
                        if real.is_none() {
                            c.spawn((Mesh3d(meshes.add(Cuboid::new(0.6, 1.8, 0.6))), MeshMaterial3d(stone.clone()), Transform::from_translation(body)));
                            // The gargoyle's head leans out over the fire.
                            c.spawn((Mesh3d(meshes.add(Cuboid::new(0.45, 0.4, 0.45))), MeshMaterial3d(stone), Transform::from_translation(body + to_bevy(face * 30.0) + Vec3::Y * 0.75)));
                        }
                        c.spawn((Part::Fire(id), Transform::default(), Visibility::Hidden)).with_children(|f| {
                            f.spawn((Mesh3d(meshes.add(Cuboid::from_size(size * Vec3::new(0.9, 0.7, 0.9)))), MeshMaterial3d(fire)));
                            f.spawn((PointLight { color: fire_color, intensity: 120_000.0, range: 6.0, shadow_maps_enabled: false, ..default() }, Transform::from_xyz(0.0, 0.3, 0.0)));
                        });
                    });
                }
            }
            continue;
        }
        if let Some(altar) = altar {
            let half = Vec3::from(db_sim::overrides::features::ALTAR_HALF);
            let size = to_bevy(half * 2.0).abs();
            let color = altar_color(altar.kind);
            let stone = materials.add(surface_material(Surface::Wall, Vec2::ONE, &assets));
            let glow = materials.add(StandardMaterial { base_color: color, emissive: LinearRgba::from(color) * 6.0, ..default() });
            // The blueprint as built (`Altar` = `SM_Holyhands_02` at scale 0.3, about 2.45 m); the orb floats where
            // its effect sits (the `VFX` node).
            let real = prop(match altar.kind {
                AltarKind::Attack => "BP_Altar_Attack",
                AltarKind::Defend => "BP_Altar_Defend",
                AltarKind::Heal => "BP_Altar_Heal",
                AltarKind::Clarity => "BP_Altar_Clarity",
                AltarKind::Speed => "BP_Altar_Speed",
            });
            let mut orb_y = size.y * 0.5 + 0.3;
            if let Some(nodes) = real {
                spawn_prop(&mut commands, &assets, nodes, root_id, Transform::from_xyz(0.0, -size.y * 0.5, 0.0));
                orb_y = -size.y * 0.5 + node(nodes, "VFX").map_or(2.0, |i| in_root(nodes, i).translation.y);
            }
            commands.entity(root_id).with_children(|c| {
                if real.is_none() {
                    c.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(stone.clone())));
                    c.spawn((Mesh3d(meshes.add(Cuboid::new(size.x * 1.3, 0.08, size.z * 1.3))), MeshMaterial3d(stone), Transform::from_xyz(0.0, size.y * 0.5 + 0.04, 0.0)));
                }
                c.spawn((Part::Orb(id), Transform::from_xyz(0.0, orb_y, 0.0), Visibility::default())).with_children(|o| {
                    o.spawn((Mesh3d(meshes.add(Sphere::new(0.16))), MeshMaterial3d(glow)));
                    o.spawn((PointLight { color, intensity: 40_000.0, range: 3.0, shadow_maps_enabled: false, ..default() }, Transform::default()));
                });
            });
            let e = spawn_label(&mut commands, altar.kind.name().to_string(), color, WorldLabel::follow(id, 130.0, 16.0));
            commands.entity(e).insert(crate::flow::LevelScoped);
        }
    }
}

/// Moves the parts from the sim state.
pub(crate) fn animate_features(time: Res<Time>, index: Res<SimIndex>, doors: Query<&Door>, traps: Query<&Trap>, altars: Query<&Altar>, mut parts: Query<(&Part, &mut Transform, &mut Visibility)>) {
    let dt = time.delta_secs();
    for (part, mut t, mut vis) in &mut parts {
        match *part {
            Part::DoorLeaf(id) => {
                let open = index.get(id).and_then(|e| doors.get(e).ok()).is_some_and(|d| d.open);
                // About a third of a second to swing.
                let target = if open { Quat::from_rotation_y(1.6) } else { Quat::IDENTITY };
                t.rotation = t.rotation.slerp(target, (dt * 8.0).min(1.0));
            }
            Part::Spikes(id, rest, travel) => {
                let Some(trap) = index.get(id).and_then(|e| traps.get(e).ok()) else { continue };
                // Up: at rest (the tips over the plate); down: sunk under it.
                let target = if trap.is_up() { rest } else { rest - travel };
                t.translation.y += (target - t.translation.y) * (dt * 20.0).min(1.0);
            }
            Part::Blade(id, axis) => {
                let Some(trap) = index.get(id).and_then(|e| traps.get(e).ok()) else { continue };
                // Up: one swing across and back; down: resting against one side.
                let angle = if trap.is_up() { -BLADE_SWING * (std::f32::consts::TAU * trap.tick as f32 / trap.up as f32).cos() } else { -BLADE_SWING };
                t.rotation = Quat::from_axis_angle(axis, angle);
            }
            Part::Fire(id) => {
                let Some(trap) = index.get(id).and_then(|e| traps.get(e).ok()) else { continue };
                vis.set_if_neq(if trap.is_up() { Visibility::Inherited } else { Visibility::Hidden });
                // Flicker.
                let f = 1.0 + 0.08 * (time.elapsed_secs() * 23.0).sin();
                t.scale = Vec3::new(1.0, f, 1.0);
            }
            Part::Orb(id) => {
                let spent = index.get(id).and_then(|e| altars.get(e).ok()).is_some_and(|a| a.used);
                vis.set_if_neq(if spent { Visibility::Hidden } else { Visibility::Inherited });
                t.rotation = Quat::from_rotation_y(time.elapsed_secs());
            }
        }
    }
}

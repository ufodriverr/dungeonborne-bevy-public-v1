//! The maps' swing doors: each door's blueprint as built (`props_view`, the manifest's or our own reads of the
//! blueprints, `prop_meshes.py`), placed at the actor's origin, yaw and scale, its `Hinge` / `Hinge1` turned from
//! the sim's timeline (`SwingDoor::t`, the leaves' turn); plain boxes where a class's meshes aren't imported. Reads
//! sim state only.

use bevy::prelude::*;
use db_sim::swing_doors::SwingDoor;
use db_sim::{Position, SimId};

use crate::combat_view::ActorView;
use crate::props_view::{local, node, prop, spawn_prop};
use crate::to_bevy;

/// A hinge of a swing door's body: which way it turns (−1 `Hinge`, +1 `Hinge1`) from its rest rotation.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SwingHinge {
    door: SimId,
    sign: f32,
    rest: Quat,
}

/// Bodies for new swing doors.
pub(crate) fn spawn_swing_door_views(mut commands: Commands, assets: Res<AssetServer>, added: Query<(&SimId, &SwingDoor, &Position), Added<SwingDoor>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for (&id, door, pos) in &added {
        let s = door.scale;
        let at = Transform { translation: Vec3::ZERO, rotation: Quat::from_rotation_y(-door.yaw), scale: Vec3::new(s.x, s.z, s.y) };
        let root = commands.spawn((ActorView(id), Transform::from_translation(to_bevy(pos.0)), Visibility::default(), crate::flow::LevelScoped)).id();
        if let Some(nodes) = prop(&door.spec.class) {
            let ents = spawn_prop(&mut commands, &assets, nodes, root, at);
            for (name, sign) in [("Hinge", -1.0), ("Hinge1", 1.0)] {
                if let Some(i) = node(nodes, name) {
                    commands.entity(ents[i]).insert(SwingHinge { door: id, sign, rest: local(&nodes[i]).rotation });
                }
            }
            continue;
        }
        // No meshes: each leaf's box on its hinge.
        let wood = materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.24, 0.15), perceptual_roughness: 0.9, ..default() });
        let holder = commands.spawn((at, Visibility::default(), ChildOf(root))).id();
        for leaf in &door.spec.leaves {
            let hinge = commands.spawn((Transform::from_translation(to_bevy(leaf.hinge)), Visibility::default(), ChildOf(holder), SwingHinge { door: id, sign: leaf.sign, rest: Quat::IDENTITY })).id();
            commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(to_bevy(leaf.half * 2.0).abs()))), MeshMaterial3d(wood.clone()), Transform::from_translation(to_bevy(leaf.center)), ChildOf(hinge)));
        }
    }
}

/// Turns the hinges with their door's swing (Unreal yaw `a` is a Bevy turn of `-a` about Y).
pub(crate) fn swing_hinges(index: Res<db_sim::SimIndex>, doors: Query<&SwingDoor>, mut hinges: Query<(&SwingHinge, &mut Transform)>) {
    for (h, mut t) in &mut hinges {
        let Some(d) = index.get(h.door).and_then(|e| doors.get(e).ok()) else { continue };
        let turn = h.sign * d.eased() * d.spec.swing_deg.to_radians() * d.dir;
        t.rotation = h.rest * Quat::from_rotation_y(-turn);
    }
}

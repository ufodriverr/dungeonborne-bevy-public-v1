//! The Shadow King's floor collapse drawn (`db_sim::floor_collapse`). The warning: `BP_TriggerableEffect_
//! FloorCollapseWarning`'s three decals, one over each floor sector, for their 10 s: flat unlit quads in the decal
//! material's `EmissiveColor` (its crack and noise textures aren't rebuilt: HYPOTHESIS look). A stage: its sector's floor
//! goes from view. The game's visible floor is Chaos geometry collections (`FallGroups`, breaking at the stage), which
//! aren't converted; what we draw there is the sector's collision mesh, so that is what disappears (the collision itself
//! goes 1 s later, in the sim). Reads sim state and events only.

use bevy::gltf::GltfMeshName;
use bevy::prelude::*;
use db_sim::SimEvent;
use db_sim::floor_collapse::FloorCollapse;

use crate::combat_view::FrameEvents;
use crate::to_bevy;

/// A warning decal and the time it goes (s, `Time::elapsed_secs`).
#[derive(Component)]
pub(crate) struct CollapseDecal(f32);

/// Height of the decal quads over the floor (cm; ours, against z-fighting).
const DECAL_LIFT: f32 = 12.0;

#[allow(clippy::too_many_arguments)]
pub(crate) fn collapse_views(
    mut commands: Commands,
    events: Res<FrameEvents>,
    time: Res<Time>,
    collapses: Query<&FloorCollapse>,
    mut drawn: Query<(&GltfMeshName, &mut Visibility)>,
    decals: Query<(Entity, &CollapseDecal)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    for ev in &events.0 {
        match ev {
            SimEvent::FloorCollapseWarning { floor, on: true } => {
                let Some(c) = collapses.iter().find(|c| c.floor == *floor) else { continue };
                let v = &c.spec.view;
                let [r, g, b] = v.decal_color;
                // The emissive colour's hue at full brightness, half see-through.
                let peak = r.max(g).max(b).max(1e-3);
                let material = materials.add(StandardMaterial { base_color: Color::srgba(r / peak, g / peak, b / peak, 0.45), unlit: true, alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None, ..default() });
                for &(at, half) in &v.decals {
                    let mesh = meshes.add(Plane3d::new(Vec3::Y, half * 0.01));
                    commands.spawn((Mesh3d(mesh), MeshMaterial3d(material.clone()), Transform::from_translation(to_bevy(at + c.off + Vec3::Z * DECAL_LIFT)), CollapseDecal(now + v.decal_secs)));
                }
                info!("floor collapse: floor {floor} warning, {} decals", v.decals.len());
            }
            SimEvent::FloorCollapsed { floor, stage } => {
                let Some(c) = collapses.iter().find(|c| c.floor == *floor) else { continue };
                let Some((_, mesh, ..)) = c.spec.view.stages.iter().find(|s| s.0 == *stage) else { continue };
                let mut n = 0;
                for (name, mut vis) in &mut drawn {
                    if name.0 == *mesh {
                        *vis = Visibility::Hidden;
                        n += 1;
                    }
                }
                info!("floor collapse: floor {floor} stage {stage}, {mesh} hidden ({n} drawn)");
            }
            _ => {}
        }
    }
    for (e, d) in &decals {
        if now >= d.0 {
            commands.entity(e).despawn();
        }
    }
}

//! Ground indicators for placed skills. Blizzard (`GA_TGF_Lich_Q`, CONFIRMED bytecode): while Q is held the
//! selection indicator (`BP_Indicator_SnowStorm`) marks the floor point the storm will start at, within 12 m; while
//! the storm channels a second indicator (`BP_Indicator_SnowStorm_2`) marks the point it moves toward, within 25 m.
//! Drawn as flat glowing rings (their Niagara indicator effects don't play yet; a flat `Annulus` mesh never drew, a
//! flattened `Torus` does); the selection ring has the storm's radius. A third ring outlines the storm itself (80
//! cm, then the unfolded 4 m): its own effect barely shows in the arena. The selection ring also marks where Force
//! of Nature's seed will grow while it is aimed (within 10 m).

use bevy::prelude::*;
use db_sim::melee::Aim;
use db_sim::overrides::class_skills as tune;
use db_sim::{Position, SimIndex};

use crate::LocalPlayer;
use crate::to_bevy;

/// Q is held to place a Blizzard (`collect_input`).
#[derive(Resource, Default)]
pub(crate) struct GroundAim(pub bool);

/// The rings: the selection, the channelled storm's goal, and the storm.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AimRing {
    Select,
    Goal,
    Storm,
}

/// Ring sizes (m): the goal marker is a small ring inside the storm's path.
const GOAL_RADIUS: f32 = 0.35;
const RING_WIDTH: f32 = 0.08;
/// Above the floor (cm): the arena floor's mesh stands a little above the capsules' feet.
const LIFT: f32 = 8.0;

#[allow(clippy::too_many_arguments)]
pub(crate) fn ground_aim(
    mut commands: Commands,
    aiming: Res<GroundAim>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    actors: Query<(&Position, &Aim, &db_sim::ClassKit)>,
    mut rings: Query<(&AimRing, &mut Transform, &mut Visibility)>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
) {
    let me = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok());
    let storm_radius = me.and_then(|(_, _, k)| if let db_sim::ClassData::Lich(d) = &k.data { Some(d.storm_radius / 100.0) } else { None }).unwrap_or(0.8);
    if rings.is_empty() {
        // Lit by its own glow (an unlit base colour drew near black under the arena's exposure).
        let mat = materials.add(StandardMaterial { base_color: Color::srgb(0.5, 0.8, 1.0), emissive: LinearRgba::rgb(2.0, 4.0, 8.0), ..default() });
        for (ring, r) in [(AimRing::Select, storm_radius), (AimRing::Goal, GOAL_RADIUS), (AimRing::Storm, 1.0)] {
            commands.spawn((Mesh3d(meshes.add(Torus::new(r - RING_WIDTH, r))), MeshMaterial3d(mat.clone()), Transform::default(), Visibility::Hidden, ring));
        }
        return;
    }
    let place = |range: f32| me.map(|(p, a, _)| db_sim::class_skills::aim_floor(p.0, *a, range) + Vec3::Z * LIFT);
    let storm = me.and_then(|(_, _, k)| k.blizzard);
    let channelling = storm.is_some_and(|b| b.channel > 0);
    let seeding = me.is_some_and(|(_, _, k)| k.charge.is_some_and(|c| c.0 == db_sim::SkillKind::ForceOfNature));
    for (ring, mut t, mut v) in &mut rings {
        let at = match ring {
            AimRing::Select if aiming.0 => place(tune::BLIZZARD_SELECT_RANGE).map(|p| (p, 1.0)),
            AimRing::Select if seeding => place(db_sim::class_skills::FORCE_OF_NATURE_RANGE).map(|p| (p, 1.0)),
            AimRing::Goal if channelling => place(tune::BLIZZARD_RANGE).map(|p| (p, 1.0)),
            AimRing::Storm => storm.map(|b| (b.pos + Vec3::Z * LIFT, if b.channel > 0 { storm_radius } else { tune::BLIZZARD_UNFOLDED_RADIUS / 100.0 })),
            _ => None,
        };
        match at {
            Some((p, scale)) => {
                // The torus lies flat (in Bevy's XZ plane); flattened so it reads as a ring on the floor.
                *t = Transform::from_translation(to_bevy(p)).with_scale(Vec3::new(scale, 0.3, scale));
                *v = Visibility::Inherited;
            }
            None => *v = Visibility::Hidden,
        }
    }
}

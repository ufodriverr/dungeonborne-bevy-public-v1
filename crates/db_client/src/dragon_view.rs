//! The Shadow Dragon's fire patches drawn (`db_sim::shadow_dragon`). `BP_ShadowDragon_Attack_Decal_NoNiagara`'s
//! decal: a flat unlit quad its `DecalSize` across, in the hue of its material's `EmissiveColor`
//! (`MI_FX_DecalFire_103T`; its textures aren't rebuilt: HYPOTHESIS look), fading out over `FadeDuration` after
//! `FadeStartDelay`. The dragon itself is a model body (`model_view`). Reads sim events only.

use bevy::prelude::*;
use db_sim::SimEvent;

use crate::combat_view::FrameEvents;
use crate::to_bevy;

/// A fire patch: when it appeared and its decal's fade (s, `Time::elapsed_secs`), and its colour at full strength.
#[derive(Component)]
pub(crate) struct FirePatchDecal {
    born: f32,
    fade_from: f32,
    life: f32,
    alpha: f32,
}

/// Height of the quads over the floor (cm; ours, against z-fighting); patches of one breath overlap, so each sits a
/// little above the last.
const LIFT: f32 = 10.0;
const LIFT_STEP: f32 = 0.4;

/// How see-through a patch is at full strength (ours: a decal has no quad to see through).
const ALPHA: f32 = 0.5;

/// Logs each Shadow Dragon as it appears (where, for shots).
pub(crate) fn dragon_arrivals(dragons: Query<&db_sim::shadow_dragon::ShadowDragon, Added<db_sim::shadow_dragon::ShadowDragon>>) {
    for d in &dragons {
        info!("Shadow Dragon summoned at {:.0} (its circle round {:.0})", d.pose.at, d.centre);
    }
}

pub(crate) fn fire_patch_views(mut commands: Commands, events: Res<FrameEvents>, time: Res<Time>, patches: Query<(Entity, &FirePatchDecal, &MeshMaterial3d<StandardMaterial>)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut laid: Local<u32>) {
    let now = time.elapsed_secs();
    for ev in &events.0 {
        let SimEvent::FirePatchSpawned { at, burn, .. } = ev else { continue };
        let [r, g, b] = burn.color;
        let peak = r.max(g).max(b).max(1e-3);
        let material = materials.add(StandardMaterial { base_color: Color::srgba(r / peak, g / peak, b / peak, ALPHA), unlit: true, alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None, ..default() });
        let mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(burn.decal * 0.01)));
        let lift = LIFT + (*laid % 16) as f32 * LIFT_STEP;
        *laid += 1;
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), Transform::from_translation(to_bevy(*at + Vec3::Z * lift)), FirePatchDecal { born: now, fade_from: burn.fade_from, life: burn.life, alpha: ALPHA }));
    }
    for (e, p, m) in &patches {
        let age = now - p.born;
        if age >= p.life {
            commands.entity(e).despawn();
            continue;
        }
        if age > p.fade_from
            && let Some(mut mat) = materials.get_mut(&m.0)
        {
            let k = 1.0 - (age - p.fade_from) / (p.life - p.fade_from).max(1e-3);
            mat.base_color.set_alpha(p.alpha * k.clamp(0.0, 1.0));
        }
    }
}

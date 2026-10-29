//! Monster aura areas drawn (`db_sim::aura_areas`): the Slow totem's `BP_TGF_Monster_OverlapArea_Base` decal
//! (`Decal1`: `MI_FX_Decal_Ice_103T`, its `Creack_Color` through its mask `TX_FX_Mask_660`) as a flat see-through quad
//! on the floor, `DecalSize` across, scaled as the area's tick scales it: `RadiusCurve` / 100, at least 1 (CONFIRMED
//! bytecode). Its parent `M_Decal_Ice` (cracks, fresnel) isn't rebuilt: HYPOTHESIS look. Reads sim state only.

use bevy::prelude::*;
use db_sim::aura_areas::AuraAreas;
use db_sim::{SimId, SimIndex};

use crate::effects::MonsterActorVfx;
use crate::to_bevy;

/// One area's decal: its owner and point (the sim's area key).
#[derive(Component)]
pub(crate) struct AuraDecal(SimId, Vec3);

/// Height of the quad over the floor (cm; ours, against z-fighting).
const LIFT: f32 = 6.0;

/// The decal's opacity at its mask's full (ours: the quad stands in for its `OpacityEnhance` 5 over the cracks).
const OPACITY: f32 = 0.55;

#[allow(clippy::too_many_arguments)]
pub(crate) fn aura_decals(
    mut commands: Commands,
    areas: Option<Res<AuraAreas>>,
    tick: Option<Res<db_sim::TickCount>>,
    index: Res<SimIndex>,
    kits: Query<&db_sim::MonsterKit>,
    actor_vfx: Res<MonsterActorVfx>,
    assets: Res<AssetServer>,
    mut views: Query<(Entity, &AuraDecal, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(areas), Some(tick)) = (areas, tick) else { return };
    for (e, d, _) in &views {
        if !areas.0.iter().any(|a| a.owner == d.0 && a.at == d.1) {
            commands.entity(e).despawn();
        }
    }
    for a in &areas.0 {
        // Its decal: the owner's aura attack's actor (`SpawnActor`).
        let Some(kit) = index.get(a.owner).and_then(|e| kits.get(e).ok()) else { continue };
        let Some(decal) = kit.attacks.iter().filter(|x| x.aura.is_some()).find_map(|x| actor_vfx.actor(&x.class, &["SpawnActor"], None)?.decals.first()) else { continue };
        let v = (a.spec.radius_at((tick.0.saturating_sub(a.born)) as f32 * db_sim::DT) / 100.0).max(1.0);
        let at = to_bevy(a.at + Vec3::Z * LIFT);
        let scale = Vec3::new(decal.half.x * 2.0 * v * 0.01, 1.0, decal.half.y * 2.0 * v * 0.01);
        if let Some((_, _, mut t)) = views.iter_mut().find(|(_, d, _)| d.0 == a.owner && d.1 == a.at) {
            t.scale = scale;
            continue;
        }
        info!("aura area decal laid at {:.0} ({:?})", a.at, decal.mask);
        let [r, g, b] = decal.color;
        let material = materials.add(StandardMaterial {
            base_color: Color::linear_rgba(r, g, b, OPACITY),
            base_color_texture: decal.mask.clone().map(|m| assets.load(m)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(1.0, 1.0))), MeshMaterial3d(material), Transform { translation: at, rotation: Quat::IDENTITY, scale }, bevy::light::NotShadowCaster, AuraDecal(a.owner, a.at)));
    }
}

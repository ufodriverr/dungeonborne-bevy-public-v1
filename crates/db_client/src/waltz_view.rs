//! Blade Waltz seen. `GA_TGF_SwordMaster_E` (CONFIRMED bytecode) starts the hero's animation helper actor, attaches the
//! held sword's anim-driven projectile to it (`AttachProjectileSword`, its template by equip slot) and plays
//! `AM_Classes_SwordMaster_Skill2_RuneSword_3rd_Anim` on the helper for `SwordAnimLength` (8 s). The montage's one section
//! loops on itself; its clip `A_Classes_SwordMaster_Skill2_RuneSword_3rd_Anim` (2.73 s, on
//! `SK_SwordMaster_RuneSword_Skeleton`) carries the sword on `Weapon_Help006`: scaled in from nothing over 0.33 s, then
//! circling the feet 1.3-1.6 m out and 1 m up, about two turns a second. Here the helper rig (`SK_SwordMaster_RuneSword`,
//! its own mesh hidden) stands at the caster's feet, turned with the view, and is posed by the sim's waltz clock.
//! HYPOTHESIS: the sword is the glowing summoned sword the Flying Swords use (`SM_WP_Sword_5002_`; only the legendary
//! swords have their own E templates), on the bone as it lies, with the Flying Swords' glow and flight trail (the E
//! templates' own `FX_WP_Sword_5001` isn't converted); the cast's 1.5 s third-person camera (`CM_Tgf_SwordMaster_E`) isn't
//! shown yet.

use bevy::prelude::*;
use db_sim::{ClassKit, Position, SimIndex};

use crate::to_bevy;

const RIG: &str = "models/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/SwordMaster/SwordMaster_RuneSword/SK_SwordMaster_RuneSword.glb";
const CLIP: &str = "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/SwordMaster/SwordMaster_RuneSword/A_Classes_SwordMaster_Skill2_RuneSword_3rd_Anim.glb";
const CLIP_SECS: f32 = 2.733_333_3;
/// The bone the sword rides.
const BONE: &str = "Weapon_Help006";
const SWORD: &str = "models/Dungeonborne/Content/TGF/Equipment/Weapon/Sword/WP_Sword_5002/Mesh/SM_WP_Sword_5002_.glb";
/// The capsule's centre above its feet (cm).
const FEET: f32 = db_sim::movement::CAPSULE_HEIGHT * 0.5;

/// One caster's helper rig: whose waltz (sim entity), its graph and clip node, and its animation player once in.
#[derive(Component)]
pub(crate) struct WaltzRig {
    owner: Entity,
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
    player: Option<Entity>,
}

/// Spawns a rig for each waltzing caster, keeps it at their feet and posed by their waltz clock, and removes it
/// when the waltz ends.
#[allow(clippy::too_many_arguments)]
pub(crate) fn waltz_view(
    mut commands: Commands,
    assets: Res<AssetServer>,
    index: Res<SimIndex>,
    player: Res<crate::LocalPlayer>,
    casters: Query<(Entity, &ClassKit, &Position)>,
    mut rigs: Query<(Entity, &mut WaltzRig, &mut Transform)>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    cams: Query<&Transform, (With<crate::PlayerCamera>, Without<WaltzRig>)>,
    (children, names, meshes): (Query<&Children>, Query<&Name>, Query<(), With<Mesh3d>>),
    mut players: Query<&mut AnimationPlayer>,
    lib: Res<crate::effects::EffectLibrary>,
) {
    let me = player.0.and_then(|id| index.get(id));
    for (e, mut rig, mut t) in &mut rigs {
        let Some((_, kit, pos)) = casters.get(rig.owner).ok().filter(|(_, k, _)| k.waltz.is_some()) else {
            commands.entity(e).despawn();
            continue;
        };
        // (The rig faces +Z; the view looks down -Z.)
        let yaw = if Some(rig.owner) == me { cams.single().map_or(0.0, |c| c.rotation.to_euler(EulerRot::YXZ).0) + std::f32::consts::PI } else { 0.0 };
        *t = Transform::from_translation(to_bevy(pos.0 - Vec3::Z * FEET)).with_rotation(Quat::from_rotation_y(yaw));
        if rig.player.is_none() {
            // Once the rig is in: its own mesh hidden, the sword on the bone, the clip on the armature.
            let Some(arm) = children.iter_descendants(e).find(|c| names.get(*c).is_ok_and(|n| n.as_str() == "Armature")) else { continue };
            let Some(bone) = children.iter_descendants(e).find(|c| names.get(*c).is_ok_and(|n| n.as_str() == BONE)) else { continue };
            for m in children.iter_descendants(e).filter(|c| meshes.contains(*c)) {
                commands.entity(m).insert(Visibility::Hidden);
            }
            let sword = commands.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(SWORD))), Transform::default(), Visibility::Inherited, bevy::light::NotShadowCaster, ChildOf(bone))).id();
            let fx = crate::projectile_view::SWORD_FLIGHT_FX.into_iter().chain([crate::projectile_view::SWORD_HELD_FX]);
            for key in fx {
                if let Some(f) = crate::effects::spawn_effect(&mut commands, &lib, key, Vec3::ZERO, Quat::IDENTITY, None) {
                    commands.entity(f).insert(ChildOf(sword));
                }
            }
            crate::model_view::drive_from_armature(&mut commands, &children, &names, arm);
            commands.entity(arm).insert(AnimationGraphHandle(rig.graph.clone()));
            rig.player = Some(arm);
            continue;
        }
        let since = kit.waltz.map_or(0, |w| w.1) as f32 * db_sim::DT;
        if let Some(mut anim) = rig.player.and_then(|p| players.get_mut(p).ok()) {
            if !anim.is_playing_animation(rig.node) {
                anim.play(rig.node);
            }
            if let Some(active) = anim.animation_mut(rig.node) {
                active.set_speed(0.0).seek_to(since % CLIP_SECS);
            }
        }
    }
    for (owner, kit, pos) in &casters {
        if kit.waltz.is_none() || rigs.iter().any(|(_, r, _)| r.owner == owner) {
            continue;
        }
        let mut graph = AnimationGraph::new();
        let node = graph.add_clip(assets.load(GltfAssetLabel::Animation(0).from_asset(CLIP)), 1.0, graph.root);
        let rig = WaltzRig { owner, graph: graphs.add(graph), node, player: None };
        commands.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(RIG))), Transform::from_translation(to_bevy(pos.0 - Vec3::Z * FEET)), Visibility::default(), rig, crate::flow::LevelScoped));
    }
}

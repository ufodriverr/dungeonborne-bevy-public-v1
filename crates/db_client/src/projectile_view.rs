//! Thrown weapons drawn as weapons. Psychic Flying Swords' shots are copies of the Swordmaster's own sword (each sword's
//! `B_TGF_Normal_Projectile_SwordMasterQ_*` names that sword's mesh, e.g. `SM_FX_WP_Sword_5001`), and Flying Blades' the
//! Rogue's daggers (HYPOTHESIS, by the same pattern): the shot's view drops its placeholder glow for the owner's held
//! right-hand weapon, tip first along the flight. The shield 6002's thrown shield is its holder's own left-hand shield,
//! spinning flat.

use bevy::prelude::*;
use db_sim::class_skills::{FLYING_SWORDS_MAX, swords_summoned};
use db_sim::projectiles::Projectile;
use db_sim::{ClassData, ClassKit, HitSource, Inventory, SimIndex, SkillKind};
use db_formats::config::generated::EnItemWearingPart;

use crate::combat_view::ActorView;
use crate::weapon_meshes::WEAPON_MESHES;

/// The shield 6002: its holder's item shots are the thrown shield.
const THROWING_SHIELD: i32 = 181_036_001;
/// The thrown shield's spin, turns a second (HYPOTHESIS: its flight's look isn't read).
const SHIELD_SPIN: f32 = 3.0;

/// A shot drawn as its owner's weapon.
#[derive(Component)]
pub(crate) struct ThrownWeapon;

/// A sword summoned by a Flying Swords hold, in the local player's view (its order).
#[derive(Component)]
pub(crate) struct HeldSword(u8);

/// Where the summoned swords hang in the view (m, camera space), filled in order: upright at the sides, the inner
/// pair first, tips leaning ahead (HYPOTHESIS: the game's `AM_Classes_SwordMaster_Skill1_Charge<n>_RuneSword`
/// montages place them; not converted).
const HELD_AT: [Vec3; 4] = [Vec3::new(0.42, -0.32, -0.85), Vec3::new(-0.42, -0.32, -0.85), Vec3::new(0.72, -0.38, -0.95), Vec3::new(-0.72, -0.38, -0.95)];
const HELD_SCALE: f32 = 0.8;
/// Their lean ahead from upright.
const HELD_LEAN: f32 = 0.45;

/// Flying Swords' sword, held and in flight: `B_TGF_Normal_Projectile_SwordMasterQ`'s `SkeletalMesh`
/// `SKM_WP_Sword_5002` (drawn as its static twin), whatever the caster holds (CONFIRMED blueprint).
const SUMMONED_SWORD: &str = "models/Dungeonborne/Content/TGF/Equipment/Weapon/Sword/WP_Sword_5002/Mesh/SM_WP_Sword_5002_.glb";
/// Its effects: in flight the projectile's `Trail` (`FX_common_attack06`) and `Niagara1` (`FX_SwordMaster_cast99_Temp2`);
/// held, the summoned sword's (`B_TGF_AnimDrive_NoHit_Projectile_SwordMasterQ`) `Niagara` (`FX_SwordMaster_Cast03`).
pub(crate) const SWORD_FLIGHT_FX: [&str; 2] = ["sys:FX_common_attack06", "sys:FX_SwordMaster_cast99_Temp2"];
pub(crate) const SWORD_HELD_FX: &str = "sys:FX_SwordMaster_Cast03";

/// A thrown flask's tumble, turns a second (HYPOTHESIS: its flight's look isn't read).
const FLASK_SPIN: f32 = 1.5;

/// The weapon meshes lie along +X, tip forward (as on the ground, `props_view`).
fn facing(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::X, crate::to_bevy(dir).normalize_or(Vec3::X))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn thrown_weapons(
    mut commands: Commands,
    time: Res<Time>,
    index: Res<SimIndex>,
    assets: Res<AssetServer>,
    shots: Query<&Projectile>,
    inventories: Query<&Inventory>,
    mut views: Query<(Entity, &ActorView, &mut Transform, Has<ThrownWeapon>)>,
    (flasks, lib): (Option<Res<db_sim::throwables::FlaskDb>>, Res<crate::effects::EffectLibrary>),
) {
    for (e, view, mut t, dressed) in &mut views {
        let Some(shot) = index.get(view.0).and_then(|s| shots.get(s).ok()) else { continue };
        // A flask: its kind's bottle, tumbling, with its trail.
        if let Some(kind) = shot.flask.and_then(|i| flasks.as_ref()?.0.get(&i).map(|f| f.kind().to_string())) {
            t.rotation = Quat::from_rotation_z(time.elapsed_secs() * std::f32::consts::TAU * FLASK_SPIN);
            if dressed {
                continue;
            }
            let path = crate::throwables_view::model_of(&kind);
            commands.entity(e).insert(ThrownWeapon).remove::<(Mesh3d, MeshMaterial3d<StandardMaterial>)>();
            if crate::model_view::imported(&path) {
                commands.entity(e).with_children(|c| {
                    c.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path))), Transform::default()));
                });
            }
            if let Some((trail, _)) = crate::throwables_view::effects_of(&kind)
                && let Some(fx) = crate::effects::spawn_effect(&mut commands, &lib, trail, Vec3::ZERO, Quat::IDENTITY, None)
            {
                commands.entity(fx).insert(ChildOf(e));
            }
            continue;
        }
        let worn = |part| index.get(shot.owner).and_then(|o| inventories.get(o).ok()).and_then(|inv| inv.worn.get(&part)).map(|i| i.item.item_id);
        let shield = shot.spec.by == HitSource::Skill(SkillKind::Item) && worn(EnItemWearingPart::Pair1leftweapon) == Some(THROWING_SHIELD);
        if !shield && !matches!(shot.spec.by, HitSource::Skill(SkillKind::FlyingSwords | SkillKind::FlyingBlades)) {
            continue;
        }
        t.rotation = if shield { Quat::from_rotation_y(time.elapsed_secs() * std::f32::consts::TAU * SHIELD_SPIN) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2) } else { facing(shot.dir) };
        if dressed {
            continue;
        }
        let swords = shot.spec.by == HitSource::Skill(SkillKind::FlyingSwords);
        let held = if shield { Some(THROWING_SHIELD) } else { worn(EnItemWearingPart::Pair1rightweapon) };
        let path = if swords { Some(SUMMONED_SWORD) } else { held.and_then(|id| WEAPON_MESHES.iter().find(|w| w.0 == id)).map(|w| w.1) };
        let Some(path) = path.filter(|p| crate::model_view::imported(p)) else { continue };
        commands.entity(e).insert(ThrownWeapon).remove::<(Mesh3d, MeshMaterial3d<StandardMaterial>)>().with_children(|c| {
            c.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))), Transform::default()));
        });
        if swords {
            for key in SWORD_FLIGHT_FX {
                if let Some(fx) = crate::effects::spawn_effect(&mut commands, &lib, key, Vec3::ZERO, Quat::IDENTITY, None) {
                    commands.entity(fx).insert(ChildOf(e));
                }
            }
        }
    }
}

/// While the local player holds Flying Swords, the swords summoned so far hang in the view, glowing (their
/// `FX_SwordMaster_Cast03`), tips forward, bobbing (one on the press, then one every 0.7 s); gone when the hold
/// ends.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn held_swords(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    player: Res<crate::LocalPlayer>,
    index: Res<SimIndex>,
    kits: Query<(&ClassKit, Option<&Inventory>)>,
    cams: Query<Entity, With<crate::PlayerCamera>>,
    mut held: Query<(Entity, &HeldSword, &mut Transform)>,
    lib: Res<crate::effects::EffectLibrary>,
) {
    let me = player.0.and_then(|id| index.get(id)).and_then(|e| kits.get(e).ok());
    let n = me.and_then(|(k, _)| {
        let most = if let ClassData::SwordMaster(d) = &k.data { d.q_swords } else { FLYING_SWORDS_MAX };
        k.charge.filter(|c| c.0 == SkillKind::FlyingSwords).map(|c| swords_summoned(c.1, most))
    });
    let n = n.unwrap_or(0).min(HELD_AT.len() as u8);
    let t = time.elapsed_secs();
    let mut have = [false; 4];
    for (e, s, mut tf) in &mut held {
        if s.0 >= n {
            commands.entity(e).despawn();
            continue;
        }
        have[usize::from(s.0)] = true;
        tf.translation = HELD_AT[usize::from(s.0)] + Vec3::Y * (0.015 * (t * 2.5 + f32::from(s.0)).sin());
    }
    let Ok(cam) = cams.single() else { return };
    if !crate::model_view::imported(SUMMONED_SWORD) {
        return;
    }
    for i in (0..n).filter(|&i| !have[usize::from(i)]) {
        // The mesh's +X (the tip) turned up, then leant ahead.
        let at = Transform::from_translation(HELD_AT[usize::from(i)]).with_rotation(Quat::from_rotation_x(-HELD_LEAN) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::splat(HELD_SCALE));
        let sword = commands
            .spawn((HeldSword(i), at, Visibility::default(), bevy::light::NotShadowCaster, ChildOf(cam)))
            .with_children(|c| {
                c.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(SUMMONED_SWORD))), Transform::default()));
            })
            .id();
        if let Some(fx) = crate::effects::spawn_effect(&mut commands, &lib, SWORD_HELD_FX, Vec3::ZERO, Quat::IDENTITY, None) {
            commands.entity(fx).insert(ChildOf(sword));
        }
    }
}

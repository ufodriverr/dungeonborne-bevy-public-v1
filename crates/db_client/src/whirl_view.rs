//! Whirlwind seen from behind. `GA_TGF_Fighter_QWhirlWind` sets the camera mode `CM_Tgf_Fighter_QWhirlWind` for the
//! spin (CONFIRMED bytecode), whose `TargetOffsetCurve` (`SwordMaster_E`) holds the camera 300 cm behind and 5 cm
//! above the view's pivot. The port has no third-person hero yet: the stand-in is the shared human body
//! (`SKM_Man_0001`) holding the player's weapon at `Point_Weapon_R01`, playing the whirl montage
//! `AM_Classes_Fighter_Skill1_WhirlWheel_1st_Iteration` (CONFIRMED loose dump: its segments are the third-person
//! `A_Classes_Fighter_Skill1_3rd_Iteration_start/_loop` clips, and the loop clip turns the body one full turn,
//! `Bip001` yaw 11 -> 89 -> -137 -> -49 -> 11 degrees over 0.47 s). It faces the view's way (its back to the camera),
//! and wears the player's armour: each worn piece's suit part (`armor_meshes`, on the same skeleton) is its own scene
//! under the body, its `Armature` driven by the same montage.
//!
//! Blade Waltz's cast shows the same way: `GA_TGF_SwordMaster_E` sets `CM_Tgf_SwordMaster_E` (the same
//! `SwordMaster_E` offset) for 1.5 s with `SetAutoRecoverCameraMode`; the stand-in plays the third-person cast clip
//! `A_Classes_SwordMaster_Skill2_3rd` (2.7 s, its first 1.5 s seen) while the waltz's sword starts circling.
//!
//! So does the Lich's Ice Barrier (sweep: nothing showed from inside the ice): `GA_TGF_Lich_E` sets `CM_TGF_Lich_E`
//! while encased (CONFIRMED bytecode; `CV_CameraMode_Lich_E` blends out to 300 cm behind over 0.4 s, free look),
//! and the stand-in holds the one-frame pose `A_Classes_Lich_Skill2_Loop_3rd` inside the ice block's effect, facing
//! the way it was cast (HYPOTHESIS: the blend is this view's ease).

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use db_formats::config::generated::EnItemWearingPart;
use db_sim::{Inventory, Position, SimIndex, SkillKit};

use crate::armor_meshes::ARMOR_MESHES;
use crate::to_bevy;
use crate::weapon_meshes::WEAPON_MESHES;

const BODY: &str = "models/Dungeonborne/Content/TGF/Character/Shared/Animation/Skeleton/SKM_Man_0001.glb";
/// The montage's clips: the wind-up and the one-turn loop.
const CLIPS: [&str; 4] = [
    "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Fighter/A_Classes_Fighter_Skill1_3rd_Iteration_start.glb",
    "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Fighter/A_Classes_Fighter_Skill1_3rd_Iteration_loop.glb",
    "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/SwordMaster/A_Classes_SwordMaster_Skill2_3rd.glb",
    "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_Lich/A_Classes_Lich_Skill2_Loop_3rd.glb",
];
/// The encased Lich's pose (in `CLIPS`).
const BARRIER_CLIP: usize = 3;
/// Blade Waltz's camera mode time (s) and its cast clip (in `CLIPS`).
const WALTZ_CAMERA_S: f32 = 1.5;
const WALTZ_CLIP: usize = 2;

/// What the stand-in shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    Whirl,
    Waltz,
    Barrier,
}
/// The montage's segments (clip, from, to, play rate): the wind-up, then the turn ever faster; the last repeats while
/// the whirl lasts (the montage's own end segments play after the whirl, when the camera is back).
const MONTAGE: [(usize, f32, f32, f32); 12] = [
    (0, 0.833, 1.133, 1.125),
    (0, 1.133, 1.467, 1.0),
    (1, 0.0, 0.467, 0.8),
    (1, 0.0, 0.467, 1.0),
    (1, 0.0, 0.467, 1.25),
    (1, 0.0, 0.467, 1.35),
    (1, 0.0, 0.467, 1.35),
    (1, 0.0, 0.467, 1.35),
    (1, 0.0, 0.467, 1.35),
    (1, 0.0, 0.467, 1.5),
    (1, 0.0, 0.467, 1.5),
    (1, 0.0, 0.467, 1.5),
];
/// The camera mode's offset (cm): behind the pivot, and up.
const BACK: f32 = 300.0;
const UP: f32 = 5.0;
/// Seconds to ease the camera out and back.
const EASE_S: f32 = 0.2;
/// The capsule's centre above its feet (cm).
const FEET: f32 = db_sim::movement::CAPSULE_HEIGHT * 0.5;

/// How far out the camera is (0 first person .. 1 the whirlwind's view), and seconds into the whirl.
#[derive(Resource, Default)]
pub(crate) struct WhirlView {
    pub(crate) out: f32,
    time: f32,
    /// What it shows (kept while the camera eases back).
    show: Option<Show>,
    /// The view model's parts hidden while out (with what they were), put back when the camera returns.
    hidden: Vec<(Entity, Visibility)>,
}

/// The spinning stand-in: its montage's graph and nodes, how many rigs it has (the body's and each armour part's),
/// and their animation players once all are in.
#[derive(Component)]
pub(crate) struct WhirlBody {
    graph: Handle<AnimationGraph>,
    nodes: [AnimationNodeIndex; 4],
    rigs: usize,
    players: Vec<Entity>,
}

/// The clip and the time in it `t` seconds into the montage.
/// What `showing` reads.
type Kits<'w, 's> = Query<'w, 's, (Option<&'static SkillKit>, Option<&'static db_sim::ClassKit>, Option<&'static db_sim::buffs::Buffs>)>;

fn montage_at(t: f32) -> (usize, f32) {
    let mut left = t.max(0.0);
    for &(clip, a, b, rate) in &MONTAGE {
        let len = (b - a) / rate;
        if left < len {
            return (clip, a + left * rate);
        }
        left -= len;
    }
    let (clip, a, b, rate) = MONTAGE[MONTAGE.len() - 1];
    (clip, a + (left % ((b - a) / rate)) * rate)
}

/// The local player's whirl, Blade Waltz's first 1.5 s, or the Lich inside an Ice Barrier.
fn showing(player: &crate::LocalPlayer, index: &SimIndex, kits: &Kits) -> Option<Show> {
    let (skills, class, buffs) = player.0.and_then(|id| index.get(id)).and_then(|e| kits.get(e).ok())?;
    if skills.is_some_and(|k| k.whirlwind.is_some()) {
        return Some(Show::Whirl);
    }
    if buffs.is_some_and(db_sim::buffs::Buffs::encased) {
        return Some(Show::Barrier);
    }
    class.and_then(|k| k.waltz).filter(|w| (w.1 as f32 * db_sim::DT) < WALTZ_CAMERA_S).map(|_| Show::Waltz)
}

/// Eases the camera out while the local player whirls (after `follow_camera` placed it at the eye).
#[allow(clippy::too_many_arguments)]
pub(crate) fn whirl_camera(time: Res<Time>, player: Res<crate::LocalPlayer>, index: Res<SimIndex>, kits: Kits, mut view: ResMut<WhirlView>, mut cam: Query<&mut Transform, With<crate::PlayerCamera>>, spatial: db_sim::SpatialQuery, statics: crate::panther_view::Statics) {
    let now = showing(&player, &index, &kits);
    if now.is_some() {
        view.show = now;
    }
    let goal = if now.is_some() { 1.0 } else { 0.0 };
    let step = time.delta_secs() / EASE_S;
    view.out = if goal > view.out { (view.out + step).min(goal) } else { (view.out - step).max(goal) };
    if view.out <= 0.0 {
        return;
    }
    let Ok(mut t) = cam.single_mut() else { return };
    let k = view.out * view.out * (3.0 - 2.0 * view.out);
    let back = t.forward().as_vec3() * (BACK * 0.01 * k);
    let eye = t.translation;
    // (Pulled in at walls.)
    t.translation = crate::panther_view::clear_of_walls(&spatial, &statics, eye, eye + Vec3::Y * (UP * 0.01 * k) - back);
}

/// The stand-in: spawned when the whirl starts, turned while it lasts, gone once the camera is back.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn whirl_body(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    player: Res<crate::LocalPlayer>,
    index: Res<SimIndex>,
    kits: Kits,
    actors: Query<(&Position, Option<&Inventory>)>,
    mut view: ResMut<WhirlView>,
    mut bodies: Query<(Entity, &mut Transform, &mut WhirlBody)>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    cams: Query<&Transform, (With<crate::PlayerCamera>, Without<WhirlBody>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let me = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok());
    if view.out <= 0.0 || me.is_none() {
        for (e, ..) in &bodies {
            commands.entity(e).despawn();
        }
        view.time = 0.0;
        view.show = None;
        return;
    }
    let Some((pos, inv)) = me else { return };
    if showing(&player, &index, &kits).is_some() {
        view.time += time.delta_secs();
    }
    // (The model faces +Z, the view -Z: turned half round to show its back.)
    let yaw = cams.single().map_or(0.0, |c| c.rotation.to_euler(EulerRot::YXZ).0) + std::f32::consts::PI;
    let at = Transform::from_translation(to_bevy(pos.0 - Vec3::Z * FEET)).with_rotation(Quat::from_rotation_y(yaw));
    if let Ok((e, mut t, mut body)) = bodies.single_mut() {
        // (Encased, it keeps facing the way it was cast while the view looks round it.)
        if view.show == Some(Show::Barrier) {
            t.translation = at.translation;
        } else {
            *t = at;
        }
        // Once every scene is in, each `Armature` gets a player with the montage's graph (as the first-person body's);
        // then the montage is posed by time on all of them.
        if body.players.is_empty() {
            let arms: Vec<Entity> = children.iter_descendants(e).filter(|c| names.get(*c).is_ok_and(|n| n.as_str() == "Armature")).collect();
            if arms.len() < body.rigs {
                return;
            }
            for &arm in &arms {
                crate::model_view::drive_from_armature(&mut commands, &children, &names, arm);
                commands.entity(arm).insert(AnimationGraphHandle(body.graph.clone()));
            }
            body.players = arms;
            return;
        }
        let (clip, secs) = match view.show {
            Some(Show::Waltz) => (WALTZ_CLIP, view.time),
            Some(Show::Barrier) => (BARRIER_CLIP, 0.0),
            _ => montage_at(view.time),
        };
        let node = body.nodes[clip];
        for &p in &body.players {
            let Ok(mut anim) = players.get_mut(p) else { continue };
            if !anim.is_playing_animation(node) {
                anim.stop_all();
                anim.play(node);
            }
            if let Some(active) = anim.animation_mut(node) {
                active.set_speed(0.0).seek_to(secs);
            }
        }
        return;
    }
    if !crate::model_view::imported(BODY) {
        return;
    }
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let nodes = CLIPS.map(|c| graph.add_clip(assets.load(GltfAssetLabel::Animation(0).from_asset(c)), 1.0, root));
    let suit: Vec<&str> = inv.into_iter().flat_map(|i| i.worn.values()).filter_map(|w| ARMOR_MESHES.iter().find(|a| a.0 == w.item.item_id)).flat_map(|a| a.1.iter().copied()).filter(|p| crate::model_view::imported(p)).collect();
    let whirl = WhirlBody { graph: graphs.add(graph), nodes, rigs: 1 + suit.len(), players: Vec::new() };
    let held = inv.and_then(|i| i.worn.get(&EnItemWearingPart::Pair1rightweapon)).map(|i| i.item.item_id);
    let weapon = held.and_then(|id| WEAPON_MESHES.iter().find(|w| w.0 == id)).map(|w| w.1).filter(|p| crate::model_view::imported(p));
    let mut body = commands.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(BODY))), at, Visibility::default(), whirl, crate::flow::LevelScoped));
    body.with_children(|c| {
        for part in suit {
            c.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(part))), Transform::default()));
        }
    });
    if let Some(path) = weapon {
        body.observe(move |ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, names: Query<&Name>, assets: Res<AssetServer>| {
            let hand = children.iter_descendants(ready.entity).find(|e| names.get(*e).is_ok_and(|n| n.as_str() == "Point_Weapon_R01"));
            if let Some(hand) = hand {
                commands.entity(hand).with_children(|c| {
                    // Turned and gripped as in first person (the meshes lie along +X: it pointed up).
                    c.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path))), Transform::default(), Visibility::Hidden, crate::fp_body::FpWeapon::new(path)));
                });
            }
        });
    }
}

/// The first-person arms and weapon are hidden while the camera is out, and put back as they were after.
pub(crate) fn hide_arms(mut view: ResMut<WhirlView>, cams: Query<Entity, With<crate::PlayerCamera>>, children: Query<&Children>, mut vis: Query<&mut Visibility>) {
    if view.out <= 0.05 {
        for (e, was) in std::mem::take(&mut view.hidden) {
            if let Ok(mut v) = vis.get_mut(e)
                && *v == Visibility::Hidden
            {
                *v = was;
            }
        }
        return;
    }
    for cam in &cams {
        for c in children.iter_descendants(cam) {
            if let Ok(mut v) = vis.get_mut(c)
                && *v != Visibility::Hidden
            {
                view.hidden.push((c, *v));
                *v = Visibility::Hidden;
            }
        }
    }
}

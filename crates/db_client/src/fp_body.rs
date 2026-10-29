//! First-person body and arms. The player's view shows the human body `SKM_Man_0001` (the hero's
//! visible `ShellMesh`) driven by the game's first-person clips for the equipped weapon set (generated
//! table [`FP_SETS`]), with the weapon meshes on the rig's `Point_Weapon_R01` / `Point_Weapon_L01`
//! bones. The clips come from separate files on the same skeleton, so the body gets one animation
//! player on `Armature` (as the monsters). Replaces the posed view weapon whenever a set matches the
//! worn weapons; the view stays otherwise.
//!
//! HYPOTHESIS: the camera sits at the rig's `Point_Camera` bone, facing the rig's front (the original
//! camera is on the capsule with native offsets that aren't in the cooked data); only the arm material
//! sections draw (the original visibility rules aren't recovered); which clip plays for which sim state and
//! the attack clips stretched over the sim swing are ours.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use db_formats::config::generated::EnItemWearingPart;
use db_sim::{AttackKind, Inventory, SimEvent, SimIndex};

use crate::combat_view::{FrameEvents, LocalPlayer, ViewSword};
use crate::fp_clips::{FP_BODY, FP_SETS, FpSet};
use crate::model_view::imported;
use crate::weapon_meshes::WEAPON_MESHES;
use crate::skill_clips::{SKILL_CLIPS, SkillClip, When, find as find_clip};

/// The material sections that draw (the arms; the rest of the body stays hidden in first person).
const ARM_MATERIAL: &str = "MI_Man0001_Arm";
/// Planar speed (cm/s) from which run plays instead of walk (walk 330, sprint 1.5 times that).
const RUN_FROM: f32 = 400.0;
/// Seconds after a light attack within which the next one continues the chain (ours).
const CHAIN_RESET: f32 = 1.0;
/// The guard clips raise the guard, hold it and lower it again (`A_Sword_Shield_Guard_1st`: raised by
/// 0.78 s of 3.13 s, lowered from 1.6 s); a held block plays the raise and stays at this share of the clip.
const GUARD_HOLD: f32 = 0.25;
/// Frames of the first clip before the camera is fitted to the posed rig.
const FIT_AFTER: u32 = 3;

/// Dev: hold the first-person body in one clip (`--fp-pose idle|walk|run|guard|heavy|light<n>[:<0..1>]`),
/// at a fixed point of it when a fraction is given.
#[derive(Resource, Debug, Default, Clone)]
pub struct FpPose(pub Option<(String, Option<f32>)>);

/// Dev: the off-hand item shown with `--weapon` (`--offhand <item id>`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ViewOffhand(pub Option<i32>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Role {
    Idle,
    Walk,
    Run,
    Guard,
    Light(u8),
    Heavy,
    /// A named extra clip of the set (the staff spells, [`STAFF_ROLES`]).
    Extra(&'static str),
}

/// The staff spell clips the client plays (`FpSet::extra` names).
const STAFF_ROLES: [&str; 6] = ["skill_fire", "skill_fire_cancel", "skill_thunder", "skill_thunder_fail", "recharge", "cast_failed"];
/// The crossbow's.
const CROSSBOW_ROLES: [&str; 4] = ["reload", "aim", "aim_fire", "draw"];
/// The panther form's: turning into it, Shadow Pounce.
const PANTHER_ROLES: [&str; 2] = ["transform", "pounce"];
/// The held torch's: taking it out, raising it, throwing it.
const TORCH_ROLES: [&str; 3] = ["torch_draw", "torch_raise", "torch_throw"];
/// A potion's drink.
const POTION_ROLES: [&str; 1] = ["drink"];

/// Drinking a potion: `AM_Potion_Sub_Drink_1st` (3 s; `world_items::Drinking`), the bottle in the left hand (the clip
/// brings the left hand to the mouth and keeps the right low), the weapons out of view. The bottles by the items' icon
/// names (`Icon_HealingPotion` and so on, HYPOTHESIS: `CSM_Potion_01`'s meshes).
static POTION_SET: FpSet = FpSet {
    name: "Potion",
    right: None,
    left: None,
    idle: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Potion_Sub_Idle_1st.glb",
    walk: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Potion_Sub_Walk_1st.glb",
    run: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Potion_Sub_Run_1st.glb",
    guard: None,
    light: &[],
    heavy: None,
    extra: &[("drink", "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Potion_Sub_Drink_1st.glb")],
    combo: &[],
};

/// The bottle drunk for an item.
fn potion_mesh(item_id: i32) -> Option<&'static str> {
    Some(match item_id {
        100_013 => "models/Dungeonborne/Content/TGF/Equipment/Item/Potion/CSM_Potion/CSM_Potion_01/SM_HealingPotion_01.glb",
        100_017 => "models/Dungeonborne/Content/TGF/Equipment/Item/Potion/CSM_Potion/CSM_Potion_01/SM_ShieldPotion_01.glb",
        100_018 => "models/Dungeonborne/Content/TGF/Equipment/Item/Potion/CSM_Potion/CSM_Potion_01/SM_ManaPotion_01.glb",
        _ => return None,
    })
}

/// The torch in hand: `AM_Torch_Sub_*_1st` with `A_Torch_Sub_*_1st` (CONFIRMED montages), the handheld mesh of `B_Torch_001` (`SM_Torch_Handheld_01`) in the right hand (the clips hold the left at
/// the hip and bring the right forward, up to the face when raised), the weapon put away. The raise montage `AM_Torch_Sub_Hold_1st` plays `StartBright` (0-0.445 s), loops `Loop` (to 2.333 s)
/// while held, and `EndBright` (to 3.0 s) as it comes down.
const TORCH_MESH: &str = "models/Dungeonborne/Content/TGF/Equipment/Item/Torch/WP_Torch_0001/Mesh/SM_Torch_Handheld_01.glb";
static TORCH_SET: FpSet = FpSet {
    name: "Torch",
    right: None,
    left: None,
    idle: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Torch_Sub_Idle_1st.glb",
    walk: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Torch_Sub_Walk_1st.glb",
    run: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Torch_Sub_Run_1st.glb",
    guard: None,
    light: &[],
    heavy: None,
    extra: &[
        ("torch_draw", "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Torch_Sub_Draw_1st.glb"),
        ("torch_raise", RAISE_BAKED),
        ("torch_throw", "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Torch_Sub_Throw_1st.glb"),
    ],
    combo: &[],
};
const RAISE_LOOP: (f32, f32) = (0.445, 2.333);
/// The raise is an additive montage (`DefaultUpperBodyAdditiveSlot`): its right arm's tracks are offsets from the pose
/// (played alone the arm folds onto the shoulder). puts them on the torch idle's first frame as a plain clip
/// (HYPOTHESIS: the game adds them to whatever plays, walking too).
const RAISE_BAKED: &str = "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/OtherItems/A_Torch_Sub_Hold_1st_OnIdle.glb";
const RAISE_END: f32 = 3.0;

/// The Druid's panther form: the panther rig `SKM_Panther` (`Classes_DruidPanther/ Leopard`) in place of the human body,
/// every material section drawn, fitted at its own `Point_Camera` bone (1.22 m up, 0.29 m behind its root, about 0.9 m
/// behind its head: so the view takes in its head and back; HYPOTHESIS as the human camera). Its clips are the rig's own
/// (`A_Panther_*`); which plays when is ours (HYPOTHESIS): idle, walk, run, the two attacks in turn, `Jump01` over
/// Shadow Pounce's leap, and `DruidElf_PantherSkillQ_Panthe_3rd` (the change, on the panther rig) as it appears (the
/// form is seen in third person, `panther_view`; the `_1st` clip threw the body up and away from its place).
const PANTHER_BODY: &str = "models/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/SKM_Panther.glb";
static PANTHER_SET: FpSet = FpSet {
    name: "Panther",
    right: None,
    left: None,
    idle: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_Idle01.glb",
    walk: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_WalkFront.glb",
    run: "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_RunFront.glb",
    guard: None,
    light: &["animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_Attack01.glb", "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_Attack02.glb"],
    heavy: Some("animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_Attack02.glb"),
    extra: &[("transform", "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Classes_DruidElf_PantherSkillQ_Panthe_3rd.glb"), ("pounce", "animations/Dungeonborne/Content/TGF/Character/Shared/Animation/3rd/Classes_DruidPanther/Leopard/Animation/A_Panther_Jump01.glb")],
    combo: &[],
};

impl Role {
    fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "idle" => Role::Idle,
            "walk" => Role::Walk,
            "run" => Role::Run,
            "guard" => Role::Guard,
            "heavy" => Role::Heavy,
            x if STAFF_ROLES.contains(&x) || CROSSBOW_ROLES.contains(&x) || TORCH_ROLES.contains(&x) => Role::Extra(STAFF_ROLES.into_iter().chain(CROSSBOW_ROLES).chain(TORCH_ROLES).find(|r| *r == x)?),
            // A class skill's clip by its path.
            x if let Some(c) = SKILL_CLIPS.iter().find(|c| c.clip == x) => Role::Extra(c.clip),
            l => Role::Light(l.strip_prefix("light")?.parse::<u8>().ok()?.saturating_sub(1)),
        })
    }

    /// This role, then what plays when the set has no clip for it.
    fn with_fallbacks(self) -> [Role; 3] {
        match self {
            Role::Run => [Role::Run, Role::Walk, Role::Idle],
            Role::Light(_) => [self, Role::Heavy, Role::Idle],
            Role::Heavy => [Role::Heavy, Role::Light(0), Role::Idle],
            _ => [self, Role::Idle, Role::Idle],
        }
    }
}

fn clip_path(set: &FpSet, role: Role) -> Option<&'static str> {
    match role {
        Role::Idle => Some(set.idle),
        Role::Walk => Some(set.walk),
        Role::Run => Some(set.run),
        Role::Guard => set.guard,
        // Past the chain: the combo montages' other clips, which the client finds by name.
        Role::Light(i) => set.light.iter().chain(set.combo).nth(usize::from(i)).copied(),
        Role::Heavy => set.heavy,
        // The class skills' clips are named by their paths, whatever the set (`skill_clips`).
        Role::Extra(name) => set.extra.iter().find(|e| e.0 == name).map(|e| e.1).or_else(|| SKILL_CLIPS.iter().find(|c| c.clip == name).map(|c| c.clip)),
    }
}

/// The weapon type of an item: its mesh's `Equipment/Weapon/<type>/` folder.
pub(crate) fn weapon_type(item: i32) -> Option<(&'static str, &'static str)> {
    let path = WEAPON_MESHES.iter().find(|w| w.0 == item)?.1;
    let ty = path.split("/Weapon/").nth(1)?.split('/').next()?;
    Some((ty, path))
}

/// The clip set for the worn weapons: main hand and off hand, else the main hand alone; no main hand is
/// unarmed. `None` when the main-hand item's type is unknown or its set isn't imported.
fn pick_set(right: Option<i32>, left: Option<i32>) -> Option<&'static FpSet> {
    let r = match right {
        Some(id) => Some(weapon_type(id)?.0),
        None => None,
    };
    let l = left.and_then(weapon_type).map(|w| w.0);
    let set = FP_SETS.iter().find(|s| s.right == r && s.left == l && l.is_some()).or_else(|| FP_SETS.iter().find(|s| s.right == r && s.left.is_none()))?;
    let files = [Some(set.idle), Some(set.walk), Some(set.run), set.guard, set.heavy].into_iter().flatten().chain(set.light.iter().copied());
    (imported(FP_BODY) && files.into_iter().all(imported)).then_some(set)
}

/// A first-person body that is posed and fitted to the camera (shown from then on; until then the
/// view stays, e.g. while a run's first load is still bringing in its clips).
#[derive(Component)]
pub(crate) struct FpReady;

/// The first-person body (child of the view sword, which keeps its death and top-down handling).
#[derive(Component)]
pub(crate) struct FpBody {
    set: &'static FpSet,
    /// Every material section draws (the panther), not only the arms.
    whole: bool,
    /// Seconds since it was fitted and shown (the panther's change plays from then; it's fitted on its idle pose).
    age: f32,
    /// That first fit, on the idle pose (the panther in third person keeps it: its clips move the camera bone).
    idle_fit: Option<Transform>,
    /// Weapon mesh paths for the right and left hand.
    weapons: [Option<&'static str>; 2],
    clips: Vec<(Role, Handle<AnimationClip>)>,
    nodes: Option<HashMap<Role, (AnimationNodeIndex, f32)>>,
    graph: Option<Handle<AnimationGraph>>,
    player: Option<Entity>,
    camera_bone: Option<Entity>,
    /// The panther's upper neck (`Bip001-Neck1`, the head below it): hidden in first person.
    head: Option<Entity>,
    /// The panther rig's `Root` (its attack clips' lunge, held in place: `hide_panther_head`).
    root: Option<Entity>,
    /// What plays and for which swing (a new swing restarts its clip).
    playing: Option<(Role, u32)>,
    swings: u32,
    last_tick: Option<u32>,
    chain: u8,
    since_light: f32,
    /// Seconds the guard has been held.
    guard_time: f32,
    /// The torch was up last frame; seconds into its lowering.
    torch_up: bool,
    torch_lower: Option<f32>,
    fitted: u32,
    /// Seconds into the cast-failed clip after a refused staff cast.
    failed: Option<f32>,
    /// A class skill's one-shot montage playing and the seconds into it; the skill charging last frame (a charge's
    /// end without a cast is a cancel); seconds into a looping one (the Lich's aim and storm).
    skill_clip: Option<(&'static SkillClip, f32)>,
    charging: Option<db_sim::SkillKind>,
    loop_t: f32,
}

/// Keeps the first-person body on the local player's worn weapons (or the dev override).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn sync_fp_body(
    mut commands: Commands,
    assets: Res<AssetServer>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    dev: Res<crate::ViewWeapon>,
    offhand: Res<ViewOffhand>,
    inventories: Query<&Inventory>,
    sword: Query<Entity, With<ViewSword>>,
    bodies: Query<Entity, With<FpBody>>,
    kits: Query<&db_sim::ClassKit>,
    (torches, flasks, drinks): (Query<(Option<&db_sim::throwables::HeldTorch>, Option<&db_sim::throwables::Throwing>)>, Option<Res<db_sim::throwables::FlaskDb>>, Query<&db_sim::world_items::Drinking>),
    mut shown: Local<Option<(Entity, Option<i32>, Option<i32>, bool, bool, Option<&'static str>)>>,
) {
    let Ok(sword) = sword.single() else { return };
    let inv = player.0.and_then(|id| index.get(id)).and_then(|e| inventories.get(e).ok());
    let worn = |slot| inv.and_then(|i: &Inventory| i.worn.get(&slot)).map(|i| i.item.item_id);
    let (right, left) = match dev.0 {
        Some(r) => (Some(r), offhand.0),
        None => (worn(EnItemWearingPart::Pair1rightweapon), offhand.0.or(worn(EnItemWearingPart::Pair1leftweapon))),
    };
    // The Druid in panther form shows the panther, without weapons.
    let panther = player.0.and_then(|id| index.get(id)).and_then(|e| kits.get(e).ok()).is_some_and(|k| k.panther) && imported(PANTHER_BODY);
    // A torch in hand, or being thrown from it, shows the torch set instead of the weapons.
    let torch = !panther && held_torch(&player, &index, &torches, flasks.as_deref()) && [TORCH_SET.idle, TORCH_SET.walk, TORCH_SET.run, TORCH_MESH].into_iter().all(imported);
    // A potion being drunk shows the potion set with its bottle.
    let bottle = player.0.and_then(|id| index.get(id)).and_then(|e| drinks.get(e).ok()).and_then(|d| potion_mesh(d.item_id)).filter(|p| !panther && [POTION_SET.idle, POTION_SET.walk, POTION_SET.run, POTION_SET.extra[0].1, p].into_iter().all(imported));
    let torch = torch && bottle.is_none();
    // Nothing to show before the player and its gear exist (menus, the first frames of a run).
    if player.0.is_none() || (inv.is_none() && dev.0.is_none()) || *shown == Some((sword, right, left, panther, torch, bottle)) {
        return;
    }
    *shown = Some((sword, right, left, panther, torch, bottle));
    for b in &bodies {
        commands.entity(b).despawn();
    }
    let Some(set) = (if panther { Some(&PANTHER_SET) } else if bottle.is_some() { Some(&POTION_SET) } else if torch { Some(&TORCH_SET) } else { pick_set(right, left) }) else { return };
    let mesh = |id: Option<i32>| id.and_then(weapon_type).map(|w| w.1).filter(|p| imported(p)).filter(|_| !panther && !torch && bottle.is_none());
    let roles = [Role::Idle, Role::Walk, Role::Run, Role::Guard, Role::Heavy].into_iter().chain((0..(set.light.len() + set.combo.len()) as u8).map(Role::Light)).chain(STAFF_ROLES.into_iter().chain(CROSSBOW_ROLES).chain(PANTHER_ROLES).chain(TORCH_ROLES).chain(POTION_ROLES).map(Role::Extra)).chain(SKILL_CLIPS.iter().map(|c| Role::Extra(c.clip)));
    let clips = roles.filter_map(|r| clip_path(set, r).filter(|p| !matches!(r, Role::Extra(_)) || imported(p)).map(|p| (r, assets.load(GltfAssetLabel::Animation(0).from_asset(p.to_string()))))).collect();
    let body = FpBody { set, whole: panther, age: 0.0, idle_fit: None, weapons: if bottle.is_some() { [None, bottle] } else if torch { [Some(TORCH_MESH), None] } else { [mesh(right), if set.left.is_some() { mesh(left) } else { None }] }, clips, nodes: None, graph: None, player: None, camera_bone: None, head: None, root: None, playing: None, swings: 0, last_tick: None, chain: 0, since_light: 0.0, guard_time: 0.0, torch_up: false, torch_lower: None, fitted: 0, failed: None, skill_clip: None, charging: None, loop_t: 0.0 };
    info!("first-person body: set {}", set.name);
    let model = if panther { PANTHER_BODY } else { FP_BODY };
    let e = commands.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_string()))), Transform::default(), Visibility::Hidden, body)).observe(fp_body_ready).id();
    commands.entity(sword).add_child(e);
}

/// Once the body's scene is in: the animation player, the bones it needs, arm-only drawing, no shadows,
/// and the weapons on the hand bones.
#[allow(clippy::too_many_arguments)]
fn fp_body_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    children: Query<&Children>,
    names: Query<&Name>,
    meshes: Query<Option<&bevy::gltf::GltfMaterialName>, With<Mesh3d>>,
    mut bodies: Query<&mut FpBody>,
) {
    let Ok(mut body) = bodies.get_mut(ready.entity) else { return };
    let mut armature = None;
    let mut sockets = [None; 2];
    for e in children.iter_descendants(ready.entity) {
        match names.get(e).map(Name::as_str) {
            Ok("Armature") if armature.is_none() => armature = Some(e),
            Ok("Point_Camera") => body.camera_bone = Some(e),
            Ok("Bip001-Neck1") if body.whole => body.head = Some(e),
            Ok("Root") if body.whole => body.root = Some(e),
            Ok("Point_Weapon_R01") => sockets[0] = Some(e),
            Ok("Point_Weapon_L01") => sockets[1] = Some(e),
            _ => {}
        }
        if let Ok(material) = meshes.get(e) {
            let arm = body.whole || material.is_some_and(|m| m.0 == ARM_MATERIAL);
            commands.entity(e).insert((bevy::light::NotShadowCaster, if arm { Visibility::Inherited } else { Visibility::Hidden }));
        }
    }
    if let Some(arm) = armature {
        crate::model_view::drive_from_armature(&mut commands, &children, &names, arm);
        body.player = Some(arm);
    }
    for (socket, path) in sockets.into_iter().zip(body.weapons) {
        if let (Some(socket), Some(path)) = (socket, path) {
            commands.entity(socket).with_children(|s| {
                // Hidden until it's placed in the hand (its bounds are known once its meshes are in).
                s.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))), Transform::default(), Visibility::Hidden, FpWeapon::new(path)));
            });
        }
    }
}

/// A weapon mesh on a hand bone of the first-person body; `kind` is its weapon type folder.
#[derive(Component)]
pub(crate) struct FpWeapon {
    kind: &'static str,
    placed: bool,
}

impl FpWeapon {
    /// For the weapon mesh at `path` (also the third-person stand-ins' weapons: Whirlwind's body).
    pub(crate) fn new(path: &'static str) -> Self {
        let kind = if path.contains("/Item/Torch/") {
            "Torch"
        } else if path.contains("/Item/Potion/") {
            "Potion"
        } else { path.split("/Weapon/").nth(1).and_then(|s| s.split('/').next()).unwrap_or_default() };
        FpWeapon { kind, placed: false }
    }
}

/// How long after its body is shown the crossbow is fitted to the view (s; ours: the idle pose has settled).
const CROSSBOW_FIT_AFTER: f32 = 0.3;

/// Puts each weapon in its hand once its meshes have bounds (HYPOTHESIS: the weapons' attach offsets aren't in
/// the recovered data). The meshes lie along +X with the hilt (or a shield's bottom) towards -X; the hand
/// bones' +Y runs up through the fist and +Z across the palm. The long axis goes along the bone's +Y with the
/// grip on the bone: a hand's width from the butt end for hafted weapons, the middle for shields and relics;
/// staves at their mesh origin, the skeletal mesh's root (the static and skeletal staff meshes share it, and
/// the `FireStart` / `ThunderCast` sockets sit 51 cm up from it towards the head); shields face outwards. No
/// shadows (as the body's `CastShadow = false`).
#[allow(clippy::type_complexity)]
pub(crate) fn place_fp_weapons(
    mut commands: Commands,
    mut weapons: Query<(Entity, &mut FpWeapon, &mut Transform, &mut Visibility)>,
    children: Query<&Children>,
    bounds: Query<(&bevy::camera::primitives::Aabb, &Transform), Without<FpWeapon>>,
    (parents, globals, cams, bodies): (Query<&ChildOf>, Query<&GlobalTransform>, Query<&GlobalTransform, With<crate::PlayerCamera>>, Query<&FpBody>),
) {
    for (e, mut w, mut t, mut vis) in &mut weapons {
        if w.placed {
            continue;
        }
        // In first person the crossbow waits until its body has been fitted and shown a moment (posed by the idle clip).
        let body_age = parents.iter_ancestors(e).find_map(|a| bodies.get(a).ok()).map(|b| b.age);
        if w.kind == "HeavyCrossbow" && body_age.is_some_and(|a| a < CROSSBOW_FIT_AFTER) {
            continue;
        }
        let (mut lo, mut hi) = (Vec3::INFINITY, Vec3::NEG_INFINITY);
        let mut meshes = Vec::new();
        for c in children.iter_descendants(e) {
            if let Ok((aabb, local)) = bounds.get(c) {
                lo = lo.min(local.transform_point(Vec3::from(aabb.min())));
                hi = hi.max(local.transform_point(Vec3::from(aabb.max())));
                meshes.push(c);
            }
        }
        if meshes.is_empty() {
            continue;
        }
        let centre = (lo + hi) * 0.5;
        let grip = match w.kind {
            "Shield" | "Relic" => centre,
            "Staff" => Vec3::ZERO,
            _ => Vec3::new(lo.x + GRIP_FROM_BUTT, centre.y, centre.z),
        };
        // A shield's face is its -Y side: turned outwards (checked from in front of the body).
        let face = if w.kind == "Shield" { Quat::from_rotation_y(std::f32::consts::PI) } else { Quat::IDENTITY };
        let mut rot = face * Quat::from_mat3(&Mat3::from_cols(Vec3::Y, Vec3::Z, Vec3::X));
        let mut grip = grip;
        // The handheld torch stands along its mesh's +Y, the flame up: that axis along the bone's, gripped a little
        // below its middle (HYPOTHESIS: no attach offset in the data).
        if w.kind == "Torch" {
            rot = Quat::from_mat3(&Mat3::from_cols(Vec3::Z, Vec3::Y, Vec3::NEG_X));
            grip = Vec3::new(centre.x, lo.y + (hi.y - lo.y) * TORCH_GRIP, centre.z);
        }
        // A bottle the same way (its neck along the bone), held round its middle.
        if w.kind == "Potion" {
            rot = Quat::from_mat3(&Mat3::from_cols(Vec3::Z, Vec3::Y, Vec3::NEG_X));
            grip = centre;
        }
        // A crossbow isn't held like a blade: its stock (+X, the limbs at the front) points along the view, its
        // rail (+Y) up and its limbs (Z) level, gripped at its mesh origin (22 cm from the butt), as the hand
        // holds it in the idle pose it's fitted on (HYPOTHESIS, playtest: it was on edge).
        if w.kind == "HeavyCrossbow"
            && body_age.is_some()
            && let Ok(socket) = parents.get(e).and_then(|p| globals.get(p.parent()))
            && let Some(cam) = cams.iter().next()
        {
            let aim = Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_Z, Vec3::Y, Vec3::X));
            rot = socket.rotation().inverse() * cam.rotation() * aim;
            grip = Vec3::ZERO;
            debug!("crossbow grip at {:.2} in the camera's frame", cam.affine().inverse().transform_point3(socket.translation()));
        }
        *t = Transform::from_rotation(rot).with_translation(-(rot * grip));
        *vis = Visibility::Inherited;
        for m in meshes {
            commands.entity(m).insert(bevy::light::NotShadowCaster);
        }
        w.placed = true;
    }
}

/// Grip centre from the butt end of a hafted weapon, metres (about a hand's width; ours).
const GRIP_FROM_BUTT: f32 = 0.08;
/// Where along the handheld torch the hand holds it (share of its length from the butt; ours).
const TORCH_GRIP: f32 = 0.3;

/// The local player has a torch in hand or is throwing one.
fn held_torch(player: &LocalPlayer, index: &SimIndex, torches: &Query<(Option<&db_sim::throwables::HeldTorch>, Option<&db_sim::throwables::Throwing>)>, flasks: Option<&db_sim::throwables::FlaskDb>) -> bool {
    let Some((held, throwing)) = player.0.and_then(|id| index.get(id)).and_then(|e| torches.get(e).ok()) else { return false };
    held.is_some() || throwing.is_some_and(|t| flasks.and_then(|f| f.0.get(&t.item_id)).is_some_and(|f| f.effect == db_sim::throwables::FlaskEffect::Torch))
}

/// The held torch's light and flame (`B_Torch_001`, CONFIRMED: `PointLight` intensity 5, FFB67C, no shadows, 58 cm up
/// its actor; `TorchParameters` 5000: radius 600 cm, 800 raised; its flame `FX_Torch`, here the ground torch's
/// `PS_Torch_Fire_FX_01`), on the torch's head; the light widens over the raise's start (0.445 s), and both go out
/// when the throw lets go of it.
#[derive(Component)]
pub(crate) struct HeldTorchGlow(f32);

const HELD_TORCH_RADIUS: (f32, f32) = (600.0, 800.0);
const HELD_TORCH_INTENSITY: f32 = 5.0;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn held_torch_glow(
    mut commands: Commands,
    time: Res<Time>,
    lib: Res<crate::effects::EffectLibrary>,
    (player, index, tick): (Res<LocalPlayer>, Res<SimIndex>, Res<db_sim::TickCount>),
    sim: Query<(Option<&db_sim::throwables::HeldTorch>, Option<&db_sim::throwables::Throwing>)>,
    mut weapons: Query<(Entity, &FpWeapon, &mut Visibility)>,
    mut glows: Query<(&mut HeldTorchGlow, &mut PointLight)>,
    children: Query<&Children>,
    bounds: Query<(&bevy::camera::primitives::Aabb, &Transform), Without<FpWeapon>>,
) {
    let (held, throwing) = player.0.and_then(|id| index.get(id)).and_then(|e| sim.get(e).ok()).unwrap_or((None, None));
    for (e, w, mut vis) in &mut weapons {
        if w.kind != "Torch" || !w.placed {
            continue;
        }
        // Thrown: out of the hand once the throw lets go.
        if throwing.is_some_and(|t| tick.0 >= t.release) {
            *vis = Visibility::Hidden;
        }
        let light = children.get(e).ok().and_then(|c| c.iter().find(|c| glows.contains(*c)));
        let Some(light) = light else {
            let (mut lo, mut hi) = (Vec3::INFINITY, Vec3::NEG_INFINITY);
            for c in children.iter_descendants(e) {
                if let Ok((aabb, local)) = bounds.get(c) {
                    lo = lo.min(local.transform_point(Vec3::from(aabb.min())));
                    hi = hi.max(local.transform_point(Vec3::from(aabb.max())));
                }
            }
            if lo.x > hi.x {
                continue;
            }
            let head = Vec3::new((lo.x + hi.x) * 0.5, hi.y - 0.05, (lo.z + hi.z) * 0.5);
            let pl = PointLight { color: Color::srgb_u8(0xFF, 0xB6, 0x7C), intensity: HELD_TORCH_INTENSITY * crate::map_view::LUMENS_PER_UNIT, range: HELD_TORCH_RADIUS.0 * 0.01, shadow_maps_enabled: false, ..default() };
            let glow = commands.spawn((pl, Transform::from_translation(head), HeldTorchGlow(0.0), ChildOf(e))).id();
            if let Some(f) = crate::effects::spawn_effect(&mut commands, &lib, "sys:PS_Torch_Fire_FX_01", Vec3::ZERO, Quat::IDENTITY, None) {
                commands.entity(f).insert(ChildOf(glow));
            }
            continue;
        };
        let Ok((mut g, mut pl)) = glows.get_mut(light) else { continue };
        let up = held.is_some_and(|h| h.raised.is_some());
        g.0 = (g.0 + if up { 1.0 } else { -1.0 } * time.delta_secs() / RAISE_LOOP.0).clamp(0.0, 1.0);
        pl.range = (HELD_TORCH_RADIUS.0 + (HELD_TORCH_RADIUS.1 - HELD_TORCH_RADIUS.0) * g.0) * 0.01;
        pl.intensity = if *vis == Visibility::Hidden { 0.0 } else { HELD_TORCH_INTENSITY * crate::map_view::LUMENS_PER_UNIT };
    }
}

/// Plays the clip for the local player's state: swinging, the attack clip timed to the swing (light
/// attacks take the chain in turn); blocking, the guard raised and held; moving, walk or run; else idle.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn animate_fp_body(
    mut commands: Commands,
    time: Res<Time>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    events: Res<FrameEvents>,
    pose: Res<FpPose>,
    combos: Option<Res<db_sim::weapons::WeaponCombos>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    actors: Query<(&db_sim::Melee, Option<&db_sim::CharacterState>, Option<&db_sim::SkillKit>, Option<&db_sim::StaffState>, Option<&db_sim::weapons::ComboState>, Option<&db_sim::ClassKit>)>,
    crossbows: Query<&db_sim::crossbow::CrossbowState>,
    mut bodies: Query<(Entity, &mut FpBody, &mut Transform, &mut Visibility)>,
    globals: Query<&GlobalTransform>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    mut drawing: Local<Option<f32>>,
    (aiming, panther_view, look, torches, flasks, tick, drinks): (Res<crate::ground_aim::GroundAim>, Res<crate::panther_view::PantherView>, Res<crate::Look>, Query<(Option<&db_sim::throwables::HeldTorch>, Option<&db_sim::throwables::Throwing>)>, Option<Res<db_sim::throwables::FlaskDb>>, Res<db_sim::TickCount>, Query<&db_sim::world_items::Drinking>),
) {
    // A weapon set switch plays the new set's draw (`AM_<set>_Draw_1st`, 1.2-1.5 s) until it ends or the player
    // acts; the body is rebuilt for the new set, so the timer lives here.
    if events.0.iter().any(|ev| matches!(ev, SimEvent::WeaponPairSwitched { id, .. } if Some(*id) == player.0)) {
        *drawing = Some(0.0);
    }
    if let Some(t) = drawing.as_mut() {
        *t += time.delta_secs();
    }
    for (e, mut body, mut t, mut vis) in &mut bodies {
        if body.nodes.is_none() {
            if body.clips.iter().any(|(_, h)| clips.get(h).is_none()) {
                continue;
            }
            let mut graph = AnimationGraph::new();
            let mut nodes = HashMap::new();
            for (role, h) in body.clips.clone() {
                let duration = clips.get(&h).map_or(1.0, AnimationClip::duration);
                let root = graph.root;
                nodes.insert(role, (graph.add_clip(h, 1.0, root), duration));
            }
            body.nodes = Some(nodes);
            body.graph = Some(graphs.add(graph));
        }
        let Some(player_e) = body.player else { continue };
        if let Some(g) = body.graph.take() {
            commands.entity(player_e).try_insert((AnimationGraphHandle(g), AnimationTransitions::new()));
            continue;
        }
        let Ok((mut anim, Some(mut transitions))) = players.get_mut(player_e) else { continue };
        // Fit the view once the rig is posed: the camera bone at the eye.
        if body.playing.is_some() && body.fitted < FIT_AFTER {
            body.fitted += 1;
            if body.fitted == FIT_AFTER
                && let Some(fit) = fit_to_camera(e, &body, &globals)
            {
                body.idle_fit = Some(fit);
                *t = if body.whole { panther_view.body_offset(fit, look.pitch) } else { fit };
                *vis = Visibility::Inherited;
                commands.entity(e).insert(FpReady);
            }
        }
        // The panther's clips move its camera bone (its lunges, the change): it stays fitted each frame, as a
        // camera on the bone would (the bone's turn isn't followed: the view doesn't bob). In third person
        // it's put back at the eye and kept level (`panther_view`).
        if body.whole
            && body.fitted >= FIT_AFTER
            && let Some(fit) = if panther_view.out > 0.0 { body.idle_fit } else { fit_to_camera(e, &body, &globals) }
        {
            *t = panther_view.body_offset(fit, look.pitch);
        }
        let dt = time.delta_secs();
        body.since_light += dt;
        if body.fitted >= FIT_AFTER {
            body.age += dt;
        }
        let sim = player.0.and_then(|id| index.get(id)).and_then(|s| actors.get(s).ok());
        let swing = sim.and_then(|(m, _, _, _, _, _)| m.swing.as_ref());
        // A swing whose tick went back (or the first one seen) is a new swing.
        match swing {
            Some(s) if body.last_tick.is_none_or(|last| s.tick < last) => {
                body.swings += 1;
                // (A skill's one-shot montage gives way.)
                body.skill_clip = None;
                if s.kind() == Some(AttackKind::Light) {
                    body.chain = if body.since_light < CHAIN_RESET { body.chain + 1 } else { 0 };
                }
            }
            _ => {}
        }
        body.last_tick = swing.map(|s| s.tick);
        // A staff spell, or a staff combo swing, plays its clip at the montage's own time (seconds).
        let staff = sim.and_then(|s| s.3).and_then(|s| staff_clip(s, swing));
        // A crossbow's shot, reload or aim, the same way.
        let staff = staff.or_else(|| player.0.and_then(|id| index.get(id)).and_then(|s| crossbows.get(s).ok()).and_then(crossbow_clip));
        let draw_len = body.nodes.as_ref().and_then(|n| n.get(&Role::Extra("draw"))).map_or(0.0, |n| n.1);
        if swing.is_some() || staff.is_some() || drawing.is_some_and(|t| t >= draw_len) {
            *drawing = None;
        }
        let staff = staff.or(drawing.map(|t| (Role::Extra("draw"), t)));
        // The panther: Shadow Pounce's leap plays the jump over its flight; its change plays as it appears.
        let flight = (db_sim::overrides::class_skills::POUNCE_SECONDS / db_sim::DT).round().max(1.0);
        let pounce = sim.and_then(|s| s.5).and_then(|k| k.pounce).map(|t| (Role::Extra("pounce"), (1.0 - t as f32 / flight).clamp(0.0, 1.0)));
        let change = body.nodes.as_ref().and_then(|n| n.get(&Role::Extra("transform"))).map(|n| n.1).filter(|d| body.whole && body.fitted >= FIT_AFTER && body.age < *d).map(|_| (Role::Extra("transform"), body.age));
        // (In first person the change leaves the camera inside the panther's torso: the body shows once it's done.
        // Sweep. In third person it shows the change.)
        if body.whole && body.fitted >= FIT_AFTER {
            *vis = if change.is_some() && !panther_view.third_person() { Visibility::Hidden } else { Visibility::Inherited };
        }
        // A refused cast plays the cast-failed clip (`AM_Staff_Skill_CastFailed`: 2.3 s, upper body, no notifies)
        // until it ends or the staff acts.
        if events.0.iter().any(|ev| matches!(ev, SimEvent::StaffRejected { id } if Some(*id) == player.0)) {
            body.failed = Some(0.0);
            body.swings += 1;
        }
        if staff.is_some() {
            body.failed = None;
        }
        // The class skills' montages (`skill_clips`): a charge at the held time (looping its held section), a start,
        // a release or a cancel once, the Lich's aim and storm looping.
        let me = player.0;
        for ev in &events.0 {
            let started = match *ev {
                SimEvent::SkillStarted { id, skill } if Some(id) == me => find_clip(skill, |w| w == When::Start),
                // The shield 6002's throw.
                SimEvent::ShieldThrown { id } if Some(id) == me => find_clip(db_sim::SkillKind::Item, |w| w == When::Start),
                SimEvent::SkillCast { id, skill, stage } if Some(id) == me => {
                    body.charging = None;
                    find_clip(skill, |w| w == When::Cast(Some(stage))).or_else(|| find_clip(skill, |w| w == When::Cast(None)))
                }
                SimEvent::SkillEnded { id, skill } if Some(id) == me && body.charging == Some(skill) => find_clip(skill, |w| w == When::Cancel),
                _ => None,
            };
            if let Some(c) = started {
                body.skill_clip = Some((c, 0.0));
                body.swings += 1;
            }
        }
        let kit = sim.and_then(|s| s.5);
        let charge = kit.and_then(|k| k.charge).and_then(|(skill, held)| Some((find_clip(skill, |w| w == When::Charge)?, held as f32 * db_sim::DT, skill)));
        body.charging = charge.map(|c| c.2);
        let looping = if kit.is_some_and(|k| k.blizzard.is_some_and(|b| b.channel > 0)) {
            find_clip(db_sim::SkillKind::Blizzard, |w| w == When::Channel)
        } else if aiming.0 {
            find_clip(db_sim::SkillKind::Blizzard, |w| w == When::Aim)
        } else {
            None
        };
        body.loop_t = if looping.is_some() { body.loop_t + dt } else { 0.0 };
        let class_clip = if let Some((c, held, _)) = charge {
            body.skill_clip = None;
            Some((Role::Extra(c.clip), c.held_time(held)))
        } else if let Some((c, t)) = body.skill_clip {
            body.skill_clip = (t + dt < c.length()).then_some((c, t + dt));
            Some((Role::Extra(c.clip), c.clip_time(t)))
        } else {
            looping.map(|c| (Role::Extra(c.clip), c.looped_time(body.loop_t)))
        };
        // The torch in hand: taken out, raised (looping while held, lowered after), thrown.
        let torch = player.0.and_then(|id| index.get(id)).and_then(|s| torches.get(s).ok());
        let since = |t: u64| tick.0.saturating_sub(t) as f32 * db_sim::DT;
        let throw = torch.and_then(|(_, t)| t).and_then(|t| Some((t, flasks.as_ref()?.0.get(&t.item_id)?.clone()))).filter(|(_, f)| f.effect == db_sim::throwables::FlaskEffect::Torch);
        let raised = torch.and_then(|(h, _)| h).and_then(|h| h.raised);
        if body.torch_up && raised.is_none() {
            body.torch_lower = Some(0.0);
        }
        body.torch_up = raised.is_some();
        if raised.is_some() || throw.is_some() {
            body.torch_lower = None;
        }
        let draw_secs = body.nodes.as_ref().and_then(|n| n.get(&Role::Extra("torch_draw"))).map_or(0.0, |n| n.1);
        let raise_at = if throw.is_some() {
            None
        } else if let Some(up) = raised {
            let s = since(up);
            Some(if s < RAISE_LOOP.0 { s } else { RAISE_LOOP.0 + (s - RAISE_LOOP.0) % (RAISE_LOOP.1 - RAISE_LOOP.0) })
        } else if let Some(l) = body.torch_lower {
            body.torch_lower = Some(l + dt).filter(|l| *l < RAISE_END - RAISE_LOOP.1);
            Some(RAISE_LOOP.1 + l)
        } else {
            None
        };
        let drink = player.0.and_then(|id| index.get(id)).and_then(|s| drinks.get(s).ok());
        let torch_clip = if let Some(d) = drink {
            Some((Role::Extra("drink"), since(d.start)))
        } else if let Some((t, f)) = throw {
            Some((Role::Extra("torch_throw"), f.release_at + (tick.0 as f32 - t.release as f32) * db_sim::DT))
        } else if let Some(secs) = raise_at {
            Some((Role::Extra("torch_raise"), secs))
        } else {
            torch.and_then(|(h, _)| h).map(|h| since(h.since)).filter(|s| *s < draw_secs).map(|s| (Role::Extra("torch_draw"), s))
        };
        let staff = staff.or(torch_clip).or(class_clip);
        let mut weapon_combo = None;
        let (want, at) = if let Some((name, frac)) = &pose.0 {
            (Role::parse(name).unwrap_or(Role::Idle), *frac)
        } else if let Some((role, f)) = pounce {
            (role, Some(f))
        } else if let Some((role, _)) = change {
            (role, Some(0.0))
        } else if let Some((role, _)) = staff {
            (role, Some(0.0))
        } else if body.failed.is_some() {
            (Role::Extra("cast_failed"), Some(0.0))
        } else if let Some((melee, state, kit, _, combo, _)) = sim {
            let lights = body.set.light.len().max(1) as u8;
            if kit.is_some_and(|k| k.whirlwind.is_some()) {
                (Role::Heavy, None)
            } else if let Some(s) = swing {
                let f = Some(s.tick as f32 / s.length.max(1) as f32);
                match s.kind() {
                    // A sword's or dagger's combo swing: its clip by the sim's combo index, at the clip time its
                    // montage shows (the montage's segments).
                    Some(AttackKind::Light) if let Some((setup, i, _)) = combo.and_then(|c| c.last.as_ref()) => {
                        body.since_light = 0.0;
                        let t = s.tick as f32 * db_sim::DT;
                        let spec = combos.as_ref().and_then(|db| db.combos.get(setup)).and_then(|c| c.get(usize::from(*i)));
                        // After a hit that switched to the on-hit montage (the mace's): that montage from the hit.
                        let rebound = combo.and_then(|c| Some((c.rebound_at?, c.last.as_ref()?.2))).zip(spec.and_then(|c| c.rebound.as_ref()));
                        // The sim's times are real; the clip follows the montage's time (its play-rate windows).
                        let shown = match rebound {
                            Some(((hit, start), r)) => db_sim::monsters::clip_time(&r.segments, r.rates.montage(t - hit.saturating_sub(start) as f32 * db_sim::DT)),
                            None => spec.and_then(|c| db_sim::monsters::clip_time(&c.segments, c.rates.montage(t))),
                        };
                        let t = spec.map_or(t, |c| c.rates.montage(t));
                        weapon_combo = Some(shown.map_or(t, |c| c.1));
                        // A montage that moves on to another swing's clip (the shield's Quickside2 and 4 end in
                        // QuickAttack3's): that clip, when the set has it.
                        let clip = shown.and_then(|(name, _)| body.set.light.iter().chain(body.set.combo).position(|p| p.ends_with(&format!("/{name}.glb"))));
                        (Role::Light(clip.map_or(i % lights, |c| c as u8)), f)
                    }
                    Some(AttackKind::Light) => {
                        body.since_light = 0.0;
                        (Role::Light(body.chain % lights), f)
                    }
                    // A weapon's charge attack: charging loops the montage's `Loop` section, the release plays its
                    // `ChargeRelease`.
                    Some(AttackKind::Heavy) if let Some(c) = combo.and_then(|c| c.charge.as_ref()) => {
                        if let Some(h) = combos.as_ref().and_then(|db| db.heavies.get(&c.setup)) {
                            let t = h.montage_time(s.tick as f32 * db_sim::DT, c.released.is_some());
                            weapon_combo = Some(db_sim::monsters::clip_time(&h.segments, t).map_or(t, |c| c.1));
                        }
                        (Role::Heavy, f)
                    }
                    _ => (Role::Heavy, f),
                }
            } else if melee.blocking() {
                (Role::Guard, None)
            } else {
                let speed = state.map_or(0.0, |s| s.velocity.truncate().length());
                (if speed > RUN_FROM { Role::Run } else if speed > 20.0 { Role::Walk } else { Role::Idle }, None)
            }
        } else {
            (Role::Idle, None)
        };
        let nodes = body.nodes.as_ref().expect("built above");
        let Some((role, (node, duration))) = want.with_fallbacks().into_iter().find_map(|r| nodes.get(&r).map(|n| (r, *n))) else { continue };
        body.guard_time = if role == Role::Guard { body.guard_time + dt } else { 0.0 };
        let at = if role == Role::Guard && at.is_none() { Some((body.guard_time / duration.max(1e-3)).min(GUARD_HOLD)) } else { at };
        let at = match (staff.or(change), weapon_combo) {
            (Some((r, secs)), _) if r == role && pose.0.is_none() => Some(secs / duration.max(1e-3)),
            (None, Some(secs)) if matches!(role, Role::Light(_) | Role::Heavy) && pose.0.is_none() => Some(secs / duration.max(1e-3)),
            _ => at,
        };
        let (at, failed) = match body.failed {
            Some(secs) if role == Role::Extra("cast_failed") && pose.0.is_none() => (Some(secs / duration.max(1e-3)), Some(secs + dt).filter(|t| *t < duration)),
            _ => (at, None),
        };
        body.failed = failed;
        let key = (role, if at.is_some() { body.swings } else { 0 });
        if body.playing != Some(key) {
            let fade = if at.is_some() { 0.08 } else { 0.2 };
            let active = transitions.play(&mut anim, node, std::time::Duration::from_secs_f32(fade));
            if at.is_none() {
                active.repeat();
            }
            body.playing = Some(key);
        }
        // Timed clips follow the swing (or the dev fraction): stretched over it, HYPOTHESIS.
        if let (Some(f), Some(active)) = (at, anim.animation_mut(node)) {
            active.set_speed(0.0).seek_to(f.clamp(0.0, 1.0) * duration);
        }

    }
}

/// The crossbow clip for the holder's action and the time into it (s): the hip or aimed shot, the reload, the aim's
/// loop (held still at its start).
fn crossbow_clip(c: &db_sim::crossbow::CrossbowState) -> Option<(Role, f32)> {
    use db_sim::crossbow::CrossbowAction as A;
    c.held.as_ref()?;
    let secs = |t: u32| t as f32 * db_sim::DT;
    match c.action {
        Some(A::Fire { tick, aimed: false }) => Some((Role::Heavy, secs(tick))),
        Some(A::Fire { tick, aimed: true }) => Some((Role::Extra("aim_fire"), secs(tick))),
        Some(A::Reload { tick }) => Some((Role::Extra("reload"), secs(tick))),
        None if c.aiming => Some((Role::Extra("aim"), 0.0)),
        None => None,
    }
}

/// The staff clip for the holder's action (or its combo swing) and the time into it (s): the montages' sections
/// and segment rates from the spec (`db_sim::staffs`).
fn staff_clip(s: &db_sim::StaffState, swing: Option<&db_sim::melee::Swing>) -> Option<(Role, f32)> {
    use db_sim::StaffAction as A;
    let spec = &s.held.as_ref()?.1;
    let secs = |t: u32| t as f32 * db_sim::DT;
    let looped = |t: f32, from: f32, len: f32| if t < from { t } else { from + (t - from) % len.max(1e-3) };
    match &s.action {
        Some(A::Beam { tick, cancel: None, .. }) => Some((Role::Extra("skill_fire"), secs(*tick) * spec.beam.as_ref().map_or(1.0, |b| b.clip_rate))),
        Some(A::Beam { tick, cancel: Some(c), .. }) => Some((Role::Extra("skill_fire_cancel"), secs(tick.saturating_sub(*c)))),
        Some(A::ThunderAim { tick, .. }) => spec.thunder.as_ref().map(|t| (Role::Extra("skill_thunder"), looped(secs(*tick), t.ready_at, t.loop_len))),
        Some(A::ThunderRelease { tick, .. }) => spec.thunder.as_ref().map(|t| (Role::Extra("skill_thunder"), t.release_clip_start + secs(*tick))),
        Some(A::ThunderFail { tick }) => Some((Role::Extra("skill_thunder_fail"), secs(*tick))),
        Some(A::Recharge { tick, ending }) => spec.recharge.as_ref().map(|r| {
            let t = match ending {
                None => looped(secs(*tick), r.loop_at, r.end_at - r.loop_at),
                Some(e) => r.end_at + secs(tick.saturating_sub(*e)),
            };
            (Role::Extra("recharge"), t)
        }),
        None => {
            let sw = swing.filter(|w| w.kind() == Some(AttackKind::Light))?;
            Some((Role::Light(s.last_combo.map_or(0, |c| c.0)), secs(sw.tick)))
        }
    }
}

/// The panther's head isn't drawn in first person (sweep: its change and lunges swung the head and neck fur
/// through the camera; `Point_Camera` hangs off `Root`, so the view stays put): its upper neck is scaled to
/// nothing after the clips pose it (HYPOTHESIS: the game's first-person hiding isn't read).
pub(crate) fn hide_panther_head(bodies: Query<&FpBody>, mut bones: Query<&mut Transform, Without<FpBody>>, view: Res<crate::panther_view::PantherView>) {
    // (Shown again in third person.)
    let scale = if view.third_person() { Vec3::ONE } else { Vec3::ZERO };
    for body in &bodies {
        if let Some(mut t) = body.head.and_then(|h| bones.get_mut(h).ok()) {
            t.scale = scale;
        }
        // The attack clips carry the panther forward on `Root` (0.85 m in `A_Panther_Attack01`, 10 m in `Attack02`,
        // as their `AM_Panther_Attack0*_3rd_MoveActor_Curve`s): the sim doesn't move it, so the root stays put and
        // the claws strike where it stands (HYPOTHESIS: the curves' scale isn't read).
        if let Some(mut t) = body.root.and_then(|r| bones.get_mut(r).ok()) {
            t.translation = Vec3::ZERO;
        }
    }
}

/// The rig faces +Z: the idle clips hold the hands in front of `Point_Camera` along (0.05, 0, 1.0).
const BODY_FORWARD: Vec3 = Vec3::Z;

/// The body's transform under the camera that puts its `Point_Camera` bone at the eye, the rig's
/// forward along the view (the bone doesn't move in the clips checked: 1.708 m up, 5 cm back).
fn fit_to_camera(e: Entity, body: &FpBody, globals: &Query<&GlobalTransform>) -> Option<Transform> {
    let to_body = globals.get(e).ok()?.affine().inverse();
    let eye = to_body.transform_point3(globals.get(body.camera_bone?).ok()?.translation());
    let rot = Quat::from_rotation_arc(BODY_FORWARD, Vec3::NEG_Z);
    Some(Transform::from_rotation(rot).with_translation(-(rot * eye)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_pose_names_parse() {
        assert_eq!(Role::parse("idle"), Some(Role::Idle));
        assert_eq!(Role::parse("guard"), Some(Role::Guard));
        assert_eq!(Role::parse("light1"), Some(Role::Light(0)));
        assert_eq!(Role::parse("light3"), Some(Role::Light(2)));
        assert_eq!(Role::parse("jump"), None);
        assert_eq!(Role::Light(4).with_fallbacks()[1], Role::Heavy);
    }

    #[test]
    fn the_panther_plays_its_own_clips() {
        // the panther rig's clips (no guard: it falls back to idle), its change and Shadow Pounce's jump.
        let p = |r| clip_path(&PANTHER_SET, r).unwrap_or_default();
        assert!(p(Role::Idle).ends_with("/Leopard/Animation/A_Panther_Idle01.glb") && p(Role::Light(1)).ends_with("A_Panther_Attack02.glb"));
        assert!(p(Role::Extra("pounce")).ends_with("A_Panther_Jump01.glb") && p(Role::Extra("transform")).ends_with("A_Classes_DruidElf_PantherSkillQ_Panthe_3rd.glb"));
        assert_eq!(clip_path(&PANTHER_SET, Role::Guard), None);
        assert!(PANTHER_BODY.starts_with("models/") && PANTHER_SET.right.is_none());
    }

    #[test]
    fn weapons_map_to_their_clip_sets() {
        assert_eq!(weapon_type(181121000).map(|w| w.0), Some("Mace"));
        assert_eq!(weapon_type(4).map(|w| w.0), Some("Shield"));
        let set = |r: &str, l: Option<&str>| FP_SETS.iter().find(|s| s.right == Some(r) && s.left == l).map(|s| s.name);
        assert_eq!(set("Sword", Some("Shield")), Some("Sword_Shield"));
        assert_eq!(set("Mace", Some("Relic")), Some("Mace_Relic"));
        assert_eq!(set("Staff", None), Some("Staff"));
        // Every set has an idle clip and its light chain, guard and heavy clips come from its own folder.
        for s in FP_SETS {
            let folder = format!("/1st/{}/", s.name);
            assert!(s.idle.contains(&folder), "{}", s.name);
            assert!(s.light.iter().chain(s.guard.iter()).chain(s.heavy.iter()).all(|c| c.contains(&folder)), "{}", s.name);
        }
    }
}

//! Monster and player-state presentation: placeholder bodies per roster monster (facing, wind-up lean, weapon swing or
//! bite, death pose), the player's health HUD, the death screen and respawn, and the `--auto-fight` bot used by unattended
//! runs. Reads sim state only; respawn goes through commands.

use bevy::prelude::*;
use db_formats::config::generated::EnRoleCareerType;
use db_sim::level::{ARENA_SPAWN, ARENA_SPAWN_YAW, Surface, TEST_ROOM_SPAWN, TEST_ROOM_SPAWN_YAW};
use db_sim::melee::SwingPhase;
use db_sim::{ActorKind, AttackKind, CommandQueue, Health, Kind, Melee, Position, SimCommand, SimId, SimIndex};

use crate::combat_view::{LocalPlayer, WorldLabel, spawn_label};
use crate::{Look, surface_material, to_bevy};

/// Dev/demo: walk to the nearest monster, attack it and block its swings (`passive`: walk up and do
/// neither, to test the player's death), respawn after death.
#[derive(Resource, Debug, Default)]
pub struct AutoFight {
    pub on: bool,
    pub passive: bool,
    /// Dev (`--auto-fight-for <s>`): the bot stops after this many seconds of sim time and stands, still
    /// facing the nearest monster (a revealed Goblin Thief then escapes in view).
    pub for_seconds: Option<f32>,
}

/// Root of a monster's body (child meshes are posed relative to it).
#[derive(Component)]
pub(crate) struct MonsterBody(pub SimId);

/// The pivot the weapon (or a beast's head) hangs from, rotated for the swing by this share of the
/// full swing angle.
#[derive(Component)]
pub(crate) struct MonsterArm(f32);

/// What a placeholder monster holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Weapon {
    Axe,
    Dagger,
    Sword,
    Hammer,
    Polearm,
    Claws,
    // Ranged monsters.
    Bow,
    Crossbow,
    Staff,
}

/// Placeholder look of a roster monster (ours, HYPOTHESIS: the monster meshes are blocked).
/// Sizes in metres; four-legged bodies lie along local +X.
#[derive(Debug, Clone, Copy)]
struct BodyStyle {
    height: f32,
    radius: f32,
    skin: Color,
    body: Color,
    weapon: Weapon,
    four_legged: bool,
}

fn body_style(row: &str) -> BodyStyle {
    let goblin = BodyStyle { height: 1.1, radius: 0.28, skin: Color::srgb(0.25, 0.38, 0.18), body: Color::srgb(0.35, 0.24, 0.15), weapon: Weapon::Axe, four_legged: false };
    let bone = Color::srgb(0.82, 0.79, 0.68);
    let skeleton = BodyStyle { height: 1.8, radius: 0.2, skin: bone, body: bone, weapon: Weapon::Sword, four_legged: false };
    let beast = |height, radius, skin| BodyStyle { height, radius, skin, body: skin, weapon: Weapon::Claws, four_legged: true };
    match row {
        "GoblinDagger" => BodyStyle { weapon: Weapon::Dagger, body: Color::srgb(0.22, 0.2, 0.3), ..goblin },
        "SkeletonSword" => skeleton,
        "SkeletonHammerShield" => BodyStyle { weapon: Weapon::Hammer, ..skeleton },
        "SkeletonPolearm" => BodyStyle { weapon: Weapon::Polearm, ..skeleton },
        "ArmorHammer" => BodyStyle { height: 2.0, radius: 0.34, skin: Color::srgb(0.45, 0.46, 0.5), body: Color::srgb(0.38, 0.39, 0.43), weapon: Weapon::Hammer, four_legged: false },
        "Zombie" => BodyStyle { height: 1.75, radius: 0.26, skin: Color::srgb(0.45, 0.52, 0.4), body: Color::srgb(0.25, 0.23, 0.22), weapon: Weapon::Claws, four_legged: false },
        "InsectGround" => beast(1.0, 0.3, Color::srgb(0.28, 0.2, 0.3)),
        "Rat" => beast(0.7, 0.22, Color::srgb(0.4, 0.34, 0.3)),
        "Boar" => beast(1.2, 0.4, Color::srgb(0.3, 0.2, 0.14)),
        // Ranged monsters.
        "GoblinCrossBow" => BodyStyle { weapon: Weapon::Crossbow, body: Color::srgb(0.3, 0.28, 0.2), ..goblin },
        "GoblinShaman" => BodyStyle { weapon: Weapon::Staff, body: Color::srgb(0.4, 0.15, 0.12), ..goblin },
        "SkeletonBow" => BodyStyle { weapon: Weapon::Bow, ..skeleton },
        "SkeletonWizard" => BodyStyle { weapon: Weapon::Staff, body: Color::srgb(0.25, 0.2, 0.35), ..skeleton },
        // Bosses: bigger stand-ins (their meshes are blocked).
        "Werewolf" => BodyStyle { height: 2.4, radius: 0.42, skin: Color::srgb(0.32, 0.27, 0.22), body: Color::srgb(0.25, 0.2, 0.16), weapon: Weapon::Claws, four_legged: false },
        "Reaper" => BodyStyle { height: 2.6, radius: 0.36, skin: Color::srgb(0.8, 0.78, 0.7), body: Color::srgb(0.08, 0.08, 0.1), weapon: Weapon::Polearm, four_legged: false },
        "Cyclop" => BodyStyle { height: 3.6, radius: 0.7, skin: Color::srgb(0.55, 0.45, 0.35), body: Color::srgb(0.35, 0.25, 0.18), weapon: Weapon::Hammer, four_legged: false },
        "Wendigo" => BodyStyle { height: 3.2, radius: 0.5, skin: Color::srgb(0.7, 0.68, 0.62), body: Color::srgb(0.3, 0.27, 0.24), weapon: Weapon::Claws, four_legged: false },
        "ShadowKing" => BodyStyle { height: 3.0, radius: 0.45, skin: Color::srgb(0.2, 0.18, 0.25), body: Color::srgb(0.12, 0.1, 0.16), weapon: Weapon::Sword, four_legged: false },
        "Khazra" => BodyStyle { height: 2.6, radius: 0.5, skin: Color::srgb(0.45, 0.2, 0.18), body: Color::srgb(0.3, 0.12, 0.1), weapon: Weapon::Hammer, four_legged: false },
        "Executioner" => BodyStyle { height: 2.5, radius: 0.5, skin: Color::srgb(0.5, 0.4, 0.35), body: Color::srgb(0.2, 0.15, 0.12), weapon: Weapon::Axe, four_legged: false },
        _ => goblin,
    }
}

/// Feet below the capsule centre, m (the sim capsule centre stands 90 cm above the floor).
const FEET: f32 = -0.9;

#[derive(Component)]
pub(crate) struct PlayerHealthText;

#[derive(Component)]
pub(crate) struct PlayerHealthBar;

#[derive(Component)]
pub(crate) struct DeathScreen;

/// The test room's class panel: the class played and the keys that switch it.
#[derive(Component)]
pub(crate) struct ClassPanel;

/// Dev: switch to this class at this render frame, as its F key would (`--class-at <frame>:<class>`).
#[derive(Resource, Debug, Default)]
pub struct DevClassAt(pub Option<(u32, EnRoleCareerType)>);

/// The classes F1-F8 switch to, in order (`attributes::CLASSES`).
const CLASS_KEYS: [KeyCode; 8] = [KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4, KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8];

/// A class's name as the panel shows it.
/// A class's name with its switch key in the test room ("Wizard (F5)").
pub(crate) fn class_label(c: EnRoleCareerType) -> String {
    match db_sim::attributes::CLASSES.iter().position(|k| *k == c) {
        Some(i) if i < CLASS_KEYS.len() => format!("{} (F{})", class_name(c), i + 1),
        _ => class_name(c).to_string(),
    }
}

fn class_name(c: EnRoleCareerType) -> &'static str {
    match c {
        EnRoleCareerType::Fighter => "Fighter",
        EnRoleCareerType::Swordmaster => "Swordmaster",
        EnRoleCareerType::Priest => "Priest",
        EnRoleCareerType::Rogue => "Rogue",
        EnRoleCareerType::Wizard => "Wizard",
        EnRoleCareerType::Deathknight => "Death Knight",
        EnRoleCareerType::Lich => "Lich",
        EnRoleCareerType::DruidPanther => "Druid",
        _ => "?",
    }
}

/// Seconds the local player has been dead.
#[derive(Resource, Default)]
pub(crate) struct DeadFor(f32);

/// Auto-fight respawn delay, seconds.
const AUTO_RESPAWN: f32 = 4.0;

pub(crate) fn setup_player_hud(mut commands: Commands) {
    // Health is shown by the orb (`hud`).
    commands.spawn((
        Text::new("You died\nPress R to respawn"),
        crate::fonts::heading(48.0),
        TextColor(Color::srgb(0.85, 0.12, 0.1)),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(22.0), margin: UiRect::left(px(-350.0)), width: px(700.0), ..default() },
        Visibility::Hidden,
        DeathScreen,
    ));
    commands.spawn((
        Text::new(""),
        crate::fonts::sans(16.0),
        TextColor(Color::srgb(0.92, 0.88, 0.78)),
        TextShadow::default(),
        Node { position_type: PositionType::Absolute, left: px(16.0), top: px(16.0), ..default() },
        Visibility::Hidden,
        ClassPanel,
    ));
}

/// Shows the class panel in the test room and the arena.
pub(crate) fn class_panel(screen: Res<State<crate::flow::Screen>>, class: Res<crate::TestClass>, mut panel: Query<(&mut Text, &mut Visibility), With<ClassPanel>>) {
    let Ok((mut text, mut v)) = panel.single_mut() else { return };
    let on = *screen.get() == crate::flow::Screen::TestRoom;
    *v = if on { Visibility::Inherited } else { Visibility::Hidden };
    if !on || !(class.is_changed() || text.0.is_empty()) {
        return;
    }
    let now = class.0.unwrap_or(EnRoleCareerType::Fighter);
    let keys: Vec<String> = db_sim::attributes::CLASSES.iter().enumerate().map(|(i, c)| format!("{}F{} {}", if *c == now { "> " } else { "  " }, i + 1, class_name(*c))).collect();
    text.0 = format!("Class: {}\n{}", class_name(now), keys.join("\n"));
}

/// The test room's spawn point and facing (the arena's in the arena).
fn spawn_point(arena: &crate::Arena) -> (Vec3, f32) {
    if arena.0 { (ARENA_SPAWN, ARENA_SPAWN_YAW) } else { (TEST_ROOM_SPAWN, TEST_ROOM_SPAWN_YAW) }
}

/// F1-F8 in the test room: play another class. The player is replaced by a fresh one of that class at the spawn
/// with that class's skills, carrying the same items (`inventory::for_class`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn switch_class(keys: Res<ButtonInput<KeyCode>>, (dev, frames): (Res<DevClassAt>, Res<bevy::diagnostic::FrameCount>), arena: Res<crate::Arena>, data: Option<Res<crate::CombatData>>, mut class: ResMut<crate::TestClass>, mut player: ResMut<LocalPlayer>, mut look: ResMut<Look>, mut queue: ResMut<CommandQueue>, open: Res<crate::InventoryOpen>, (index, inventories, game, mut pending): (Res<SimIndex>, Query<&db_sim::Inventory>, Option<Res<db_sim::GameData>>, ResMut<crate::flow::PendingCharacter>)) {
    if open.0 {
        return;
    }
    let dev = dev.0.filter(|(f, _)| frames.0 == *f).map(|(_, c)| c);
    let Some(career) = CLASS_KEYS.iter().position(|k| keys.just_pressed(*k)).and_then(|i| db_sim::attributes::CLASSES.get(i).copied()).or(dev) else { return };
    let Some(id) = player.0 else { return };
    info!("class switch: {}", class_name(career));
    class.0 = Some(career);
    // The items come along, adapted to the class (`inventory::for_class`).
    if let (Some(inv), Some(game), Some((sheet, worn))) = (index.get(id).and_then(|e| inventories.get(e).ok()), game.as_ref(), data.as_ref().and_then(|d| d.character(career)))
    {
        let bag = db_sim::attributes::class_type_id(game.tables, career).ok().and_then(|ty| db_sim::attributes::starting_bag(game.tables, ty).ok()).unwrap_or_default();
        let (inventory, left) = db_sim::inventory::for_class(inv, game, career, worn, &bag);
        if !left.is_empty() {
            let (pos, _) = spawn_point(&arena);
            queue.push(SimCommand::SpawnItems { pos: pos - Vec3::Z * 90.0, items: left });
        }
        pending.0 = Some(db_sim::InventoryCommand::SetCharacterItems { id: SimId(0), career, sheet: sheet.clone(), inventory: Box::new(inventory) });
    }
    queue.push(SimCommand::Despawn { id });
    let (pos, yaw) = spawn_point(&arena);
    queue.push(SimCommand::Spawn { kind: ActorKind::Player, pos, speed: data.map_or(330.0, |d| d.speed(career)) });
    player.0 = None;
    look.yaw = yaw;
}

/// Placeholder body for the roster monster `row` (curve-table row): upright monsters get a torso,
/// a head and their weapon on the forward (+X local) side; beasts lie along +X and bite with the head.
/// The sim capsule is the same 1.8 m for all (HYPOTHESIS); bodies are drawn at their own size and
/// stand on the floor.
pub(crate) fn spawn_monster_body(commands: &mut Commands, id: SimId, row: &str, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, assets: &AssetServer) {
    let s = body_style(row);
    let skin = materials.add(StandardMaterial { base_color: s.skin, perceptual_roughness: 0.8, ..default() });
    let body = materials.add(StandardMaterial { base_color: s.body, perceptual_roughness: 0.9, ..default() });
    let wood = materials.add(surface_material(Surface::Wood, Vec2::ONE, assets));
    let metal = materials.add(StandardMaterial { metallic: 0.4, perceptual_roughness: 0.5, ..surface_material(Surface::Metal, Vec2::ONE, assets) });
    commands.spawn((MonsterBody(id), Transform::default(), Visibility::default())).with_children(|b| {
        if s.four_legged {
            let (r, len) = (s.radius, s.height);
            let y = FEET + r + 0.12;
            b.spawn((Mesh3d(meshes.add(Capsule3d::new(r, len - 2.0 * r))), MeshMaterial3d(body), Transform::from_xyz(0.0, y, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))));
            for (x, z) in [(0.3, 0.6), (0.3, -0.6), (-0.3, 0.6), (-0.3, -0.6)] {
                b.spawn((Mesh3d(meshes.add(Cylinder::new(r * 0.22, 0.24))), MeshMaterial3d(skin.clone()), Transform::from_xyz(x * len, FEET + 0.12, z * r)));
            }
            // The head bites: it hangs from the neck pivot and swings a little.
            b.spawn((MonsterArm(0.25), Transform::from_xyz(len * 0.5 - r * 0.3, y + r * 0.2, 0.0), Visibility::default())).with_children(|a| {
                a.spawn((Mesh3d(meshes.add(Sphere::new(r * 0.75))), MeshMaterial3d(skin.clone()), Transform::from_xyz(r * 0.6, 0.0, 0.0)));
                if row == "Boar" {
                    for z in [-0.15, 0.15] {
                        a.spawn((Mesh3d(meshes.add(Cone::new(0.04, 0.22))), MeshMaterial3d(materials.add(Color::srgb(0.9, 0.88, 0.8))), Transform::from_xyz(r * 1.1, -0.1, z)));
                    }
                }
            });
            return;
        }
        let torso = 0.62 * s.height;
        let (r, head) = (s.radius, (0.11 * s.height).max(0.13));
        b.spawn((Mesh3d(meshes.add(Capsule3d::new(r, (torso - 2.0 * r).max(0.05)))), MeshMaterial3d(body), Transform::from_xyz(0.0, FEET + torso * 0.5, 0.0)));
        b.spawn((Mesh3d(meshes.add(Sphere::new(head))), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.05, FEET + torso + head * 0.9, 0.0)));
        if row.starts_with("Goblin") {
            // Ears.
            for z in [-0.2, 0.2] {
                b.spawn((Mesh3d(meshes.add(Cuboid::new(0.05, 0.08, 0.18))), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, FEET + torso + head, z)));
            }
        }
        let shoulder = Transform::from_xyz(0.15, FEET + torso * 0.8, r + 0.05);
        b.spawn((MonsterArm(1.0), shoulder, Visibility::default())).with_children(|a| {
            let mut part = |mesh: Mesh, mat: &Handle<StandardMaterial>, t: Transform| {
                a.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat.clone()), t));
            };
            match s.weapon {
                Weapon::Axe => {
                    part(Cylinder::new(0.025, 0.7).into(), &wood, Transform::from_xyz(0.0, 0.3, 0.0));
                    part(Cuboid::new(0.22, 0.18, 0.04).into(), &metal, Transform::from_xyz(0.1, 0.6, 0.0));
                }
                Weapon::Dagger => part(Cuboid::new(0.05, 0.4, 0.02).into(), &metal, Transform::from_xyz(0.0, 0.2, 0.0)),
                Weapon::Sword => {
                    part(Cylinder::new(0.02, 0.18).into(), &wood, Transform::from_xyz(0.0, 0.05, 0.0));
                    part(Cuboid::new(0.07, 0.85, 0.02).into(), &metal, Transform::from_xyz(0.0, 0.55, 0.0));
                }
                Weapon::Hammer => {
                    part(Cylinder::new(0.03, 0.85).into(), &wood, Transform::from_xyz(0.0, 0.35, 0.0));
                    part(Cuboid::new(0.3, 0.2, 0.2).into(), &metal, Transform::from_xyz(0.0, 0.75, 0.0));
                }
                Weapon::Polearm => {
                    part(Cylinder::new(0.025, 1.8).into(), &wood, Transform::from_xyz(0.0, 0.6, 0.0));
                    part(Cuboid::new(0.16, 0.35, 0.02).into(), &metal, Transform::from_xyz(0.05, 1.55, 0.0));
                }
                Weapon::Claws => part(Capsule3d::new(0.06, 0.5).into(), &skin, Transform::from_xyz(0.0, 0.25, 0.0)),
                Weapon::Bow => {
                    // Limbs bent back from the grip, and the string.
                    part(Cuboid::new(0.03, 0.6, 0.03).into(), &wood, Transform::from_xyz(0.05, 0.6, 0.0).with_rotation(Quat::from_rotation_z(0.25)));
                    part(Cuboid::new(0.03, 0.6, 0.03).into(), &wood, Transform::from_xyz(0.05, 0.0, 0.0).with_rotation(Quat::from_rotation_z(-0.25)));
                    part(Cylinder::new(0.004, 1.1).into(), &metal, Transform::from_xyz(-0.03, 0.3, 0.0));
                }
                Weapon::Crossbow => {
                    part(Cuboid::new(0.5, 0.06, 0.06).into(), &wood, Transform::from_xyz(0.2, 0.15, 0.0));
                    part(Cuboid::new(0.04, 0.04, 0.5).into(), &metal, Transform::from_xyz(0.4, 0.17, 0.0));
                }
                Weapon::Staff => {
                    part(Cylinder::new(0.025, 1.5).into(), &wood, Transform::from_xyz(0.0, 0.5, 0.0));
                    part(Sphere::new(0.08).into(), &metal, Transform::from_xyz(0.0, 1.3, 0.0));
                }
            }
        });
    });
}

/// Faces the body along the monster's aim; leans back in the wind-up and swings the axe down in
/// the hit window; lies down when dead.
#[allow(clippy::type_complexity)]
pub(crate) fn animate_monsters(
    mut commands: Commands,
    index: Res<SimIndex>,
    actors: Query<(&Position, &db_sim::Aim, &Melee, &Health)>,
    mut bodies: Query<(Entity, &MonsterBody, &mut Transform, &Children), Without<MonsterArm>>,
    mut arms: Query<(&mut Transform, &MonsterArm)>,
) {
    for (e, body, mut t, children) in &mut bodies {
        if index.get(body.0).is_none() {
            commands.entity(e).despawn();
            continue;
        }
        let Some((pos, aim, melee, health)) = index.get(body.0).and_then(|e| actors.get(e).ok()) else { continue };
        // Sim yaw turns +X towards +Y; in Bevy that is a rotation about Y by -yaw.
        let facing = Quat::from_rotation_y(-aim.yaw);
        let (lean, arm) = match melee.swing.as_ref() {
            _ if health.current <= 0.0 => (0.0, 0.0),
            Some(s) => {
                let (w0, w1) = s.windows.first().copied().unwrap_or((0, 0));
                match s.phase() {
                    SwingPhase::Windup => {
                        let f = s.tick as f32 / w0.max(1) as f32;
                        (0.25 * f, 1.9 * f)
                    }
                    SwingPhase::Active => {
                        let f = (s.tick - w0) as f32 / (w1 - w0).max(1) as f32;
                        (0.25 - 0.55 * f, 1.9 - 3.4 * f)
                    }
                    SwingPhase::Recovery => {
                        let f = (s.tick - w1) as f32 / (s.length - w1).max(1) as f32;
                        (-0.3 * (1.0 - f), -1.5 * (1.0 - f))
                    }
                }
            }
            None => (0.0, 0.0),
        };
        let dead = health.current <= 0.0;
        t.translation = to_bevy(pos.0) - if dead { Vec3::Y * 0.6 } else { Vec3::ZERO };
        // Local +X is forward; leaning back tilts the top towards -X (rotation about local Z).
        t.rotation = facing * Quat::from_rotation_z(if dead { -1.45 } else { lean });
        for c in children.iter() {
            if let Ok((mut a, share)) = arms.get_mut(c) {
                a.rotation = Quat::from_rotation_z(arm * share.0);
            }
        }
    }
}

/// Health numbers, bar and death screen for the local player.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn player_hud(
    screen: Res<State<crate::flow::Screen>>,
    time: Res<Time>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    health: Query<&Health>,
    mut dead_for: ResMut<DeadFor>,
    mut text: Query<&mut Text, With<PlayerHealthText>>,
    mut bar: Query<&mut Node, With<PlayerHealthBar>>,
    mut death: Query<&mut Visibility, (With<DeathScreen>, Without<crate::combat_view::ViewSword>)>,
    mut sword: Query<&mut Visibility, (With<crate::combat_view::ViewSword>, Without<DeathScreen>)>,
) {
    let h = player.0.and_then(|id| index.get(id)).and_then(|e| health.get(e).ok()).copied();
    if let Ok(mut t) = text.single_mut() {
        t.0 = h.map_or(String::new(), |h| format!("{:.0} / {:.0}", h.current.ceil(), h.max));
    }
    if let Ok(mut n) = bar.single_mut() {
        n.width = percent(h.map_or(0.0, |h| (h.current / h.max.max(1.0)) as f32 * 100.0));
    }
    let dead = h.is_some_and(|h| h.current <= 0.0);
    dead_for.0 = if dead { dead_for.0 + time.delta_secs() } else { 0.0 };
    // In a run, death goes to the result screen instead.
    let test_room = *screen.get() == crate::flow::Screen::TestRoom;
    if let Ok(mut v) = death.single_mut() {
        *v = if dead && test_room { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Ok(mut v) = sword.single_mut() {
        *v = if dead || h.is_none() { Visibility::Hidden } else { Visibility::Inherited };
    }
}

/// R (or the auto-fight timer) respawns a dead player at the room spawn (the arena's in the arena) as a fresh actor
/// of the same class.
#[allow(clippy::too_many_arguments)]
pub(crate) fn respawn(keys: Res<ButtonInput<KeyCode>>, auto: Res<AutoFight>, dead_for: Res<DeadFor>, data: Option<Res<crate::CombatData>>, arena: Res<crate::Arena>, class: Res<crate::TestClass>, mut player: ResMut<LocalPlayer>, mut look: ResMut<Look>, mut queue: ResMut<CommandQueue>) {
    let Some(id) = player.0 else { return };
    if dead_for.0 <= 0.0 || !(keys.just_pressed(KeyCode::KeyR) || (auto.on && dead_for.0 >= AUTO_RESPAWN)) {
        return;
    }
    info!("respawn");
    queue.push(SimCommand::Despawn { id });
    let (pos, yaw) = spawn_point(&arena);
    let career = class.0.unwrap_or(EnRoleCareerType::Fighter);
    queue.push(SimCommand::Spawn { kind: ActorKind::Player, pos, speed: data.map_or(330.0, |d| d.speed(career)) });
    player.0 = None;
    look.yaw = yaw;
}

/// A monster's name label over its head.
pub(crate) fn spawn_monster_label(commands: &mut Commands, id: SimId, row: &str) -> Entity {
    let s = body_style(row);
    let top = if s.four_legged { 0.12 + 2.0 * s.radius } else { s.height };
    // cm above the capsule centre: the body's top plus a margin.
    let label = WorldLabel::follow(id, (top + FEET) * 100.0 + 35.0, 20.0);
    spawn_label(commands, String::new(), Color::srgb(0.85, 0.95, 0.8), label)
}

/// Auto-fight bot: faces the nearest living monster (camera too), walks into reach, attacks, and
/// blocks while the monster winds up or swings. Returns (move direction, attack, block).
pub(crate) fn auto_fight(player: SimId, index: &SimIndex, kinds: &Query<(&Kind, &Position, &Health, Option<&Melee>)>, look: &mut Look, passive: bool) -> Option<(Vec2, Option<AttackKind>, bool)> {
    let (_, me, my_health, my_melee) = kinds.get(index.get(player)?).ok()?;
    if my_health.current <= 0.0 {
        return None;
    }
    let target = index
        .iter()
        .filter_map(|(_, e)| kinds.get(e).ok())
        .filter(|(k, _, h, _)| k.0 == ActorKind::Monster && h.current > 0.0)
        .min_by(|a, b| (a.1.0 - me.0).length().total_cmp(&(b.1.0 - me.0).length()))?;
    let d = (target.1.0 - me.0).truncate();
    look.yaw = d.y.atan2(d.x);
    look.pitch = -0.12;
    let gap = d.length() - 70.0;
    let threat = target.3.and_then(|m| m.swing.as_ref()).is_some_and(|s| s.phase() != SwingPhase::Recovery);
    let busy = my_melee.is_some_and(|m| m.swing.is_some() || m.queued.is_some());
    let block = !passive && threat && !busy && gap < 200.0;
    let walk = if gap > 90.0 && !block { d.normalize_or_zero() } else { Vec2::ZERO };
    let attack = (!passive && !block && !busy && gap <= 120.0).then_some(AttackKind::Light);
    Some((walk, attack, block))
}

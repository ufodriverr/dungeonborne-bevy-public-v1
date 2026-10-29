//! The seeded dungeon in the client: builds both floors from `db_sim::dungeon` (colliders through the
//! sim, meshes here), spawns the monsters and chests through sim commands, lights each room, and marks
//! the portals (markers only; the portals become sim actors). `--topdown <floor>` swaps the player
//! camera for an orthographic view from above for layout screenshots. Only the player's floor is drawn:
//! the other floor's geometry, lights and actors are hidden (perf). Runs without `--dungeon <seed>` play
//! the real crypt floors instead ([`build_crypt`]).

use std::collections::VecDeque;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use db_sim::crypt::CryptMaps;
use db_sim::dungeon::{Dungeon, PortalKind, floor_of, generate};
use db_sim::level::Surface;
use db_sim::{ActorKind, CommandQueue, Position, SimCommand, SimEvent, SimId, SimIndex};

use crate::combat_view::{ActorView, CombatData, LocalPlayer, ViewSword, WorldLabel, spawn_label};
use crate::flow::LevelScoped;
use crate::{Look, spawn_piece, to_bevy};

/// Which space to play in: the test room (`None`) or the dungeon from a seed.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PlaySpace(pub Option<u64>);

/// Dev: look straight down at floor 1 or 2 (orthographic, no ceilings, no viewmodel).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TopDown(pub Option<u32>);

/// Dungeon geometry, lights and portal markers of one floor (0-based index).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct OnFloor(pub u32);

/// Facing, loot source and curve row for each monster spawn, in spawn order (consumed as `Spawned`
/// events arrive).
#[derive(Resource, Debug, Default)]
pub struct MonsterSpawns(pub VecDeque<(f32, i32, &'static str, db_sim::SpawnOpts)>);

/// Builds the dungeon for `seed`: geometry for both floors, the player on floor 1, monsters, chests,
/// room lights and portal markers.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    commands: &mut Commands,
    queue: &mut CommandQueue,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &AssetServer,
    data: &CombatData,
    seed: u64,
    top_down: bool,
    peaceful: bool,
    player_speed: f32,
    run_seconds: u32,
    spawns: &mut MonsterSpawns,
    look: &mut Look,
) -> Dungeon {
    let dungeon = generate(seed);
    for floor in &dungeon.floors {
        for piece in &floor.pieces {
            queue.push(piece.command());
            if !(top_down && piece.surface == Surface::Ceiling) {
                let e = spawn_piece(commands, meshes, materials, assets, piece);
                commands.entity(e).insert(OnFloor(floor.index));
            }
        }
        for room in &floor.rooms {
            let c = room.rect.center().extend(330.0);
            commands.spawn((PointLight { color: Color::srgb(1.0, 0.72, 0.45), intensity: 300_000.0, range: 16.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(to_bevy(c)), LevelScoped, OnFloor(floor.index)));
        }
    }
    spawn_actors(commands, queue, meshes, materials, data, &dungeon, top_down, peaceful, None, spawns);
    start_run(queue, &dungeon, player_speed, run_seconds, look);
    dungeon
}

/// Dev (`--escape-delay <s>`): crypt escape portals open after at most this long (the maps' delays are
/// 8-13 minutes), for unattended checks.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct EscapeDelay(pub Option<f32>);

/// A portal's marker (ring, disc, light, label): shown once the sim portal at `at` is open.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct PortalMarker(pub Vec3);

/// The crypt maps for runs, loaded on the first run.
#[derive(Resource, Debug, Default, Clone)]
pub struct CurrentCrypt(pub Option<std::sync::Arc<CryptMaps>>);

/// Dev (shots): the player's start on a real map, map-local cm, and its yaw in degrees (`--start-at x,y,z[,yaw]`), and
/// the floor the run starts on (`--start-floor <n>`: its arrival, 0-based here).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevStartAt(pub Option<(Vec3, f32)>, pub u32);

/// Builds a run on the real crypt floors: both maps' scenes and lights, their collision, and the
/// layout `maps` rolls for `seed`; the player at floor 1's start.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_crypt(
    commands: &mut Commands,
    queue: &mut CommandQueue,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &AssetServer,
    data: &CombatData,
    maps: &CryptMaps,
    seed: u64,
    peaceful: bool,
    escape_delay: Option<f32>,
    player_speed: f32,
    run_seconds: u32,
    spawns: &mut MonsterSpawns,
    look: &mut Look,
    start_at: Option<(Vec3, f32)>,
    start_floor: u32,
) -> Dungeon {
    let mut dungeon = maps.dungeon(seed);
    let start_floor = start_floor.min(dungeon.floors.len().saturating_sub(1) as u32);
    if start_floor > 0 {
        let f = &dungeon.floors[start_floor as usize];
        (dungeon.floors[0].start, dungeon.floors[0].start_yaw) = (f.start, f.start_yaw);
    }
    if let Some((p, yaw)) = start_at {
        dungeon.floors[0].start = p + maps.offset(start_floor as usize);
        dungeon.floors[0].start_yaw = yaw.to_radians();
    }
    for (i, level) in maps.def.levels.iter().enumerate() {
        queue.push(SimCommand::AddMapCollision(maps.collision[i].clone()));
        queue.push(SimCommand::AddNav(maps.nav[i].clone(), maps.offset(i)));
        let (spawned, _) = crate::map_view::spawn_level(commands, assets, level, maps.offset(i));
        for e in spawned {
            commands.entity(e).insert((LevelScoped, OnFloor(i as u32)));
        }
    }
    // Where this run's portals stand (map-local cm) and how soon they open: playtests and screenshots go to them.
    info!("portals: {}", dungeon.floors.iter().enumerate().flat_map(|(i, f)| f.portals.iter().map(move |p| { let q = p.pos - maps.offset(i); format!("floor {} {:?} at {:.0},{:.0},{:.0} in {:.0} s", i + 1, p.kind, q.x, q.y, q.z, p.opens_in) })).collect::<Vec<_>>().join("; "));
    info!("zones: {}", dungeon.floors.iter().enumerate().filter_map(|(i, f)| f.zone.as_ref().map(|z| (i, z))).map(|(i, z)| { let off = maps.offset(i).truncate(); format!("floor {}: {}", i + 1, z.centres.iter().zip(&z.spec.discs).map(|(c, d)| format!("{:.0} at {:.0},{:.0}", d.radius, (c - off).x, (c - off).y)).collect::<Vec<_>>().join(", ")) }).collect::<Vec<_>>().join("; "));
    spawn_actors(commands, queue, meshes, materials, data, &dungeon, false, peaceful, escape_delay, spawns);
    start_run(queue, &dungeon, player_speed, run_seconds, look);
    if start_floor > 0 {
        queue.push(SimCommand::DevOnFloor { floor: start_floor });
    }
    dungeon
}

fn start_run(queue: &mut CommandQueue, dungeon: &Dungeon, player_speed: f32, run_seconds: u32, look: &mut Look) {
    let first = &dungeon.floors[0];
    queue.push(SimCommand::Spawn { kind: ActorKind::Player, pos: first.start, speed: player_speed });
    queue.push(SimCommand::StartRun { seconds: run_seconds.max(1) });
    look.yaw = first.start_yaw.rem_euclid(std::f32::consts::TAU);
    info!("dungeon seed {}: {}", dungeon.seed, dungeon.floors.iter().map(|f| format!("floor {}: {} rooms, {} monsters, {} chests, {} portals, {} doors, {} traps, {} altars, {} props", f.index + 1, f.rooms.len(), f.monsters.len(), f.chests.len(), f.portals.len(), f.doors.len(), f.traps.len(), f.altars.len(), f.destroyables.len())).collect::<Vec<_>>().join("; "));
}

/// Monsters, chests, doors, traps, altars and portals (with their markers) of every floor, through sim commands.
#[allow(clippy::too_many_arguments)]
fn spawn_actors(commands: &mut Commands, queue: &mut CommandQueue, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, data: &CombatData, dungeon: &Dungeon, top_down: bool, peaceful: bool, escape_delay: Option<f32>, spawns: &mut MonsterSpawns) {
    for floor in &dungeon.floors {
        for (pos, yaw, row, opts) in floor.monsters.iter().filter(|_| !peaceful) {
            let Some(spec) = db_sim::monsters::spec(row) else { continue };
            queue.push(SimCommand::Spawn { kind: ActorKind::Monster, pos: *pos, speed: data.monster_speed(row) });
            spawns.0.push_back((*yaw, spec.loot, *row, opts.clone()));
        }
        for &(pos, source_id) in &floor.chests {
            queue.push(SimCommand::SpawnChest { pos, source_id });
            if top_down {
                let e = spawn_label(commands, format!("chest {source_id}"), Color::srgb(1.0, 0.85, 0.3), WorldLabel::at(pos + Vec3::Z * 60.0, 16.0));
                commands.entity(e).insert(LevelScoped);
            }
        }
        // Doors, traps and altars; their bodies come from `features_view`.
        for &(pos, along_x) in &floor.doors {
            queue.push(SimCommand::SpawnDoor { pos, along_x });
        }
        // The map's swing doors.
        for m in &floor.mechanisms {
            queue.push(SimCommand::SpawnMechanism(Box::new(m.clone())));
        }
        for (package, pos, yaw, scale) in &floor.swing_doors {
            queue.push(SimCommand::SpawnSwingDoor { package: package.clone(), pos: *pos, yaw: *yaw, scale: *scale });
        }
        for t in &floor.traps {
            queue.push(SimCommand::SpawnTrap { pos: t.pos, kind: t.kind, yaw: t.yaw, phase: t.phase });
        }
        for &(pos, kind) in &floor.altars {
            queue.push(SimCommand::SpawnAltar { pos, kind });
        }
        // The floor's death zone.
        if let Some(plan) = &floor.zone {
            queue.push(SimCommand::SpawnDeathZone { floor: floor.index, plan: Box::new(plan.clone()) });
        }
        // Its floor collapse.
        if let Some((spec, off)) = &floor.collapse {
            queue.push(SimCommand::SpawnFloorCollapse { floor: floor.index, spec: spec.clone(), off: *off });
        }
        // The map's breakable props.
        for &(class, pos, yaw, scale) in &floor.destroyables {
            queue.push(SimCommand::SpawnDestroyable { class: class.into(), pos, yaw, scale });
        }
        let next_start = dungeon.floors.get(floor.index as usize + 1).map(|f| f.start);
        for portal in &floor.portals {
            let opens_in = escape_delay.map_or(portal.opens_in, |d| portal.opens_in.min(d));
            queue.push(SimCommand::SpawnPortal { pos: portal.pos, kind: portal.kind, dest: next_start.filter(|_| portal.kind == PortalKind::Down), opens_in, opens: portal.opens.clone() });
            let (color, name) = match portal.kind {
                PortalKind::Down => (Color::srgb(0.45, 0.4, 1.0), "Portal down"),
                PortalKind::Extract => (Color::srgb(0.3, 1.0, 0.45), "Extraction"),
            };
            let mat = materials.add(StandardMaterial { base_color: color, emissive: LinearRgba::from(color) * 6.0, ..default() });
            let p = to_bevy(portal.pos);
            let marker = PortalMarker(portal.pos);
            commands.spawn((Mesh3d(meshes.add(Torus::new(0.7, 0.85))), MeshMaterial3d(mat.clone()), Transform::from_translation(p + Vec3::Y * 1.0).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), LevelScoped, OnFloor(floor.index), marker));
            commands.spawn((Mesh3d(meshes.add(Cylinder::new(0.9, 0.04))), MeshMaterial3d(mat), Transform::from_translation(p + Vec3::Y * 0.02), LevelScoped, OnFloor(floor.index), marker));
            commands.spawn((PointLight { color, intensity: 150_000.0, range: 8.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(p + Vec3::Y * 1.2), LevelScoped, OnFloor(floor.index), marker));
            let e = spawn_label(commands, name.into(), color, WorldLabel::at(portal.pos + Vec3::Z * 210.0, 20.0));
            commands.entity(e).insert((LevelScoped, marker));
        }
    }
}

/// A portal the sim opens during a run (the Goblin Thief's), drawn like the layout's extraction
/// portals while it exists.
#[derive(Component)]
pub(crate) struct RuntimePortal(SimId);

/// Draws portals that appear during a run (not from the layout, which has its own markers) and removes them
/// when the sim does.
#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_portals(mut commands: Commands, events: Res<crate::combat_view::FrameEvents>, index: Res<SimIndex>, portals: Query<(&Position, &db_sim::Portal)>, markers: Query<&PortalMarker>, drawn: Query<(Entity, &RuntimePortal)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for e in &events.0 {
        match *e {
            SimEvent::Spawned { id, kind: ActorKind::Portal } => {
                let Some((pos, portal)) = index.get(id).and_then(|e| portals.get(e).ok()) else { continue };
                let floor = pos.0 - Vec3::Z * db_sim::overrides::run::PORTAL_HEIGHT;
                if markers.iter().any(|m| (m.0.truncate() - floor.truncate()).length() < 1.0) {
                    continue;
                }
                // The death zone's extraction devices (`BlueDevice`) blue.
                let color = match portal.kind {
                    PortalKind::Down => Color::srgb(0.45, 0.4, 1.0),
                    PortalKind::Extract if portal.hold > 0.0 => Color::srgb(0.3, 0.65, 1.0),
                    PortalKind::Extract => Color::srgb(0.3, 1.0, 0.45),
                };
                let mat = materials.add(StandardMaterial { base_color: color, emissive: LinearRgba::from(color) * 6.0, ..default() });
                let p = to_bevy(floor);
                info!("runtime portal {id:?} ({:?}) at {:.0}", portal.kind, floor);
                commands.spawn((Mesh3d(meshes.add(Torus::new(0.7, 0.85))), MeshMaterial3d(mat.clone()), Transform::from_translation(p + Vec3::Y * 1.0).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), LevelScoped, RuntimePortal(id)));
                commands.spawn((Mesh3d(meshes.add(Cylinder::new(0.9, 0.04))), MeshMaterial3d(mat), Transform::from_translation(p + Vec3::Y * 0.02), LevelScoped, RuntimePortal(id)));
                commands.spawn((PointLight { color, intensity: 150_000.0, range: 8.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(p + Vec3::Y * 1.2), LevelScoped, RuntimePortal(id)));
                let name = if portal.hold > 0.0 { "Extraction device" } else { "Extraction" };
                let e = spawn_label(&mut commands, name.into(), color, WorldLabel::at(floor + Vec3::Z * 210.0, 20.0));
                commands.entity(e).insert((LevelScoped, RuntimePortal(id)));
            }
            SimEvent::Despawned { id } => {
                for (e, r) in &drawn {
                    if r.0 == id {
                        commands.entity(e).despawn();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Hides portal markers while their sim portal is still closed (the crypt's escape delays).
pub(crate) fn portal_markers(portals: Query<(&Position, &db_sim::Portal)>, mut markers: Query<(&PortalMarker, &mut Visibility)>, mut closed: Local<Vec<Vec3>>) {
    closed.clear();
    closed.extend(portals.iter().filter(|(_, p)| !p.open()).map(|(pos, _)| pos.0));
    for (m, mut vis) in &mut markers {
        if closed.iter().any(|c| (c.truncate() - m.0.truncate()).length() < 1.0) {
            vis.set_if_neq(Visibility::Hidden);
        } else if *vis == Visibility::Hidden {
            *vis = Visibility::Inherited;
        }
    }
}

/// Draws only the floor the player is on (or the top-down floor): hides the other floor's geometry, lights
/// and portal markers, and actor views (monsters, chests, items) standing on it. A position's floor is
/// capped at the highest floor loaded (the castle is one floor whose east side lies past `FLOOR_OFFSET_X`,
/// which hid the whole level there).
pub(crate) fn floor_visibility(
    top: Res<TopDown>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    positions: Query<&Position>,
    mut level: Query<(&OnFloor, &mut Visibility), Without<ActorView>>,
    mut views: Query<(&Transform, &mut Visibility), With<ActorView>>,
) {
    let Some(last) = level.iter().map(|(f, _)| f.0).max() else { return };
    let current = match top.0 {
        Some(f) => f.clamp(1, 2) - 1,
        None => match player.0.and_then(|id| index.get(id)).and_then(|e| positions.get(e).ok()) {
            Some(p) => floor_of(p.0.x).min(last),
            None => return,
        },
    };
    let shown = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for (f, mut vis) in &mut level {
        vis.set_if_neq(shown(f.0 == current));
    }
    for (t, mut vis) in &mut views {
        // Bevy x is sim x in metres.
        vis.set_if_neq(shown(floor_of(t.translation.x * 100.0).min(last) == current));
    }
}

/// The top-down camera for floor `floor` (1-based) of the dungeon from `seed`.
pub(crate) fn top_down_camera(seed: u64, floor: u32) -> (Projection, Transform) {
    let d = generate(seed);
    let f = &d.floors[(floor.clamp(1, 2) - 1) as usize];
    let b = f.bounds();
    let size = (b.max - b.min) * 0.01;
    let projection = Projection::from(OrthographicProjection { scaling_mode: ScalingMode::AutoMin { min_width: size.x * 1.04, min_height: size.y * 1.04 }, ..OrthographicProjection::default_3d() });
    let centre = to_bevy(b.center().extend(3000.0));
    // Sim +Y up on screen (Bevy +Z is sim +Y).
    (projection, Transform::from_translation(centre).looking_to(Vec3::NEG_Y, Vec3::Z))
}

/// No viewmodel in the top-down view.
pub(crate) fn top_down_cleanup(mut commands: Commands, top: Res<TopDown>, sword: Query<Entity, With<ViewSword>>) {
    if top.0.is_some() {
        for e in &sword {
            commands.entity(e).despawn();
        }
    }
}

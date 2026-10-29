//! The maps' lever mechanisms and levers: each blueprint as built (`props_view`), placed at the actor's origin, yaw and
//! scale. A mechanism's moving parts follow the sim's travel (`Mechanism::t`): the portcullis's `MoveBase` by its
//! `Movement`, the stone gate's seven bars (`Door03_*`) together (HYPOTHESIS: the original lowers each at its own speed),
//! the arena's stone doors turning by their `Rotation` about `MoveBase`; plain boxes where a class's meshes aren't
//! imported. A chained door's chain (`BP_ChainedDoor.Update Chain`): `Chain01` links every `UnitLength` from the `Fastener`
//! toward the `FixPoint`, all turned to look along it. The level cells carry some chains baked where they rest (the
//! construction script's `AddComponent` links of the floor 1 flaps, `MainGrid_L2_X-1_Y-2`); those level meshes are hidden
//! so only the moving chain shows. A lever's crank (`Handle`) turns a quarter while it is on (HYPOTHESIS: the `SpinHandle`
//! timeline isn't decoded, `AngleY` -90). The escape doors swing their own leaves (`MechanismPlacement::nodes`), eased; as
//! they start, their padlock hides and their chain plays its fall once (`Move`: `SM_Lock_04(_p).SetVisibility(false)`,
//! `SKM_lock*.PlayAnimation(A_lock*)`, CONFIRMED bytecode). Reads sim state only.

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use db_sim::mechanisms::{Lever, Mechanism};
use db_sim::{Position, SimId};

use crate::combat_view::ActorView;
use crate::props_view::{local, prop, spawn_prop};
use crate::to_bevy;

/// An escape door's padlock: hidden once the door moves.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct EscapeLock(SimId);

/// An escape door's chain (`SKM_lock*`): plays its clip (`A_lock*`) once as the door moves.
#[derive(Component, Debug, Clone)]
pub(crate) struct EscapeChain {
    mech: SimId,
    graph: Handle<AnimationGraph>,
    clip: AnimationNodeIndex,
    armature: Option<Entity>,
    played: bool,
}

/// Where an escape chain's clip is: `SKM_lock02` plays `A_lock02`.
fn chain_clip(node: &str) -> Option<String> {
    let path = format!("animations/TgfMaps/SceneComp/Animation/A_{}.glb", node.strip_prefix("SKM_")?);
    crate::model_view::imported(&path).then_some(path)
}

/// Once a chain's scene is in: drive its `Armature` from the clip file's bone paths.
fn chain_ready(ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, names: Query<&Name>, mut chains: Query<&mut EscapeChain>) {
    let Ok(mut chain) = chains.get_mut(ready.entity) else { return };
    let Some(arm) = children.iter_descendants(ready.entity).find(|&e| names.get(e).is_ok_and(|n| n.as_str() == "Armature")) else { return };
    crate::model_view::drive_from_armature(&mut commands, &children, &names, arm);
    commands.entity(arm).insert(AnimationGraphHandle(chain.graph.clone()));
    chain.armature = Some(arm);
}

/// A part of a mechanism's body that travels: its rest transform, its full travel and its full turn about `pivot`
/// (Bevy, local).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct MechMover {
    mech: SimId,
    rest: Vec3,
    rest_rot: Quat,
    travel: Vec3,
    turn: Quat,
    pivot: Vec3,
}

/// A chained door's chain: its links (as many as its longest reach needs; the unused hidden).
#[derive(Component, Debug, Clone)]
pub(crate) struct ChainLinks {
    mech: SimId,
    links: Vec<Entity>,
}

const CHAIN_LINK: &str = "models/TgfMaps/SceneAssets/FantasyDungeon/meshes/Chain/Chain01.glb";

/// The chain at travel `t` (actor space, Unreal cm): its links' positions and the look rotation (Unreal-numbered).
fn chain_links(m: &Mechanism, t: f32) -> (Vec<Vec3>, Quat) {
    let Some(c) = m.placement.chain else { return (Vec::new(), Quat::IDENTITY) };
    let from = m.moved(c.fastener, t);
    let d = c.fix - from;
    let n = (d.length() / c.unit).floor() as usize;
    let dir = d.normalize_or_zero();
    // `FindLookAtRotation`: yaw about +Z, then pitch (in the sim's numbering, about -Y).
    let look = Quat::from_rotation_z(dir.y.atan2(dir.x)) * Quat::from_rotation_y(-dir.z.atan2(dir.truncate().length()));
    ((0..n).map(|i| from + dir * c.unit * i as f32).collect(), look)
}

/// A lever's crank.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct LeverCrank {
    lever: SimId,
    rest: Quat,
}

fn class_nodes(class: &str) -> Option<&'static [crate::prop_meshes::PropNode]> {
    prop(class).or_else(|| prop(class.strip_suffix("_DistantLoad").unwrap_or(class)))
}

fn holder(commands: &mut Commands, id: SimId, pos: Vec3, yaw_deg: f32, s: Vec3) -> (Entity, Transform) {
    let root = commands.spawn((ActorView(id), Transform::from_translation(to_bevy(pos)), Visibility::default(), crate::flow::LevelScoped)).id();
    (root, Transform { translation: Vec3::ZERO, rotation: Quat::from_rotation_y(-yaw_deg.to_radians()), scale: Vec3::new(s.x, s.z, s.y) })
}

/// Bodies for new mechanisms and levers.
#[allow(clippy::type_complexity)]
pub(crate) fn spawn_mechanism_views(mut commands: Commands, assets: Res<AssetServer>, mechs: Query<(&SimId, &Mechanism, &Position), Added<Mechanism>>, levers: Query<(&SimId, &Lever), Added<Lever>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut graphs: ResMut<Assets<AnimationGraph>>) {
    for (&id, m, pos) in &mechs {
        let p = &m.placement;
        let (root, at) = holder(&mut commands, id, pos.0, p.yaw, p.scale);
        let travel = to_bevy(p.travel);
        // The sim's turn (Unreal-numbered, unmirrored: the holder carries the mirror) in Bevy's swapped Y/Z: the axis
        // is a pseudovector, so (x, y, z) -> -(x, z, y) (yaw +Z -> about -Y, as `holder`).
        let q = m.local_turn(1.0);
        let turn = Quat::from_xyzw(-q.x, -q.z, -q.y, q.w);
        if let Some(nodes) = class_nodes(&p.class) {
            let ents = spawn_prop(&mut commands, &assets, nodes, root, at);
            // an escape door's padlock and chain.
            if p.class.starts_with("BP_EscapeMechanism_") {
                for (i, n) in nodes.iter().enumerate() {
                    if n.name == "SM_Lock_04" || n.name == "SM_Lock_04_p" {
                        commands.entity(ents[i]).insert(EscapeLock(id));
                    } else if let Some(path) = chain_clip(n.name) {
                        let (graph, clip) = AnimationGraph::from_clip(assets.load(GltfAssetLabel::Animation(0).from_asset(path)));
                        commands.entity(ents[i]).insert(EscapeChain { mech: id, graph: graphs.add(graph), clip, armature: None, played: false }).observe(chain_ready);
                    }
                }
            }
            // the escape doors move their own nodes, each about its own origin.
            for n in &p.nodes {
                let Some(i) = nodes.iter().position(|x| x.name == n.node) else { continue };
                let rest = local(&nodes[i]);
                let q = db_sim::mechanisms::turn_quat(n.turn, 1.0);
                commands.entity(ents[i]).insert(MechMover { mech: id, rest: rest.translation, rest_rot: rest.rotation, travel: to_bevy(n.travel), turn: Quat::from_xyzw(-q.x, -q.z, -q.y, q.w), pivot: rest.translation });
            }
            let movers: Vec<usize> = match nodes.iter().position(|n| n.name == "MoveBase").filter(|_| p.nodes.is_empty()) {
                Some(i) => vec![i],
                None => nodes.iter().enumerate().filter(|(_, n)| p.nodes.is_empty() && n.name.starts_with("Door03_")).map(|(i, _)| i).collect(),
            };
            for i in movers {
                let rest = local(&nodes[i]);
                commands.entity(ents[i]).insert(MechMover { mech: id, rest: rest.translation, rest_rot: rest.rotation, travel, turn, pivot: rest.translation });
            }
            if m.placement.chain.is_some() {
                let most = (0..=10).map(|k| chain_links(m, k as f32 / 10.0).0.len()).max().unwrap_or(0) + 1;
                let h = commands.spawn((at, Visibility::default(), ChildOf(root))).id();
                let links = (0..most).map(|_| commands.spawn((Transform::default(), Visibility::Hidden, ChildOf(h), WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(CHAIN_LINK))))).id()).collect();
                commands.entity(h).insert(ChainLinks { mech: id, links });
            }
            continue;
        }
        let iron = materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.25, 0.27), metallic: 0.7, perceptual_roughness: 0.5, ..default() });
        let h = commands.spawn((at, Visibility::default(), ChildOf(root))).id();
        for part in &p.parts {
            let rest = to_bevy(part.center);
            commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(to_bevy(part.half * 2.0).abs()))), MeshMaterial3d(iron.clone()), Transform::from_translation(rest), ChildOf(h), MechMover { mech: id, rest, rest_rot: Quat::IDENTITY, travel, turn, pivot: to_bevy(p.pivot) }));
        }
    }
    for (&id, l) in &levers {
        let p = &l.placement;
        let (root, at) = holder(&mut commands, id, p.pos, p.yaw, p.scale);
        if let Some(nodes) = class_nodes(&p.class) {
            let ents = spawn_prop(&mut commands, &assets, nodes, root, at);
            if let Some(i) = nodes.iter().position(|n| n.name == "Handle") {
                commands.entity(ents[i]).insert(LeverCrank { lever: id, rest: local(&nodes[i]).rotation });
            }
            continue;
        }
        let wood = materials.add(StandardMaterial { base_color: Color::srgb(0.4, 0.3, 0.2), ..default() });
        commands.entity(root).with_children(|c| {
            c.spawn((Mesh3d(meshes.add(Cuboid::new(0.1, 0.6, 0.1))), MeshMaterial3d(wood), Transform::from_xyz(0.0, 0.3, 0.0), LeverCrank { lever: id, rest: Quat::IDENTITY }));
        });
    }
}

/// Escape doors starting to move: their padlocks hide, their chains fall.
pub(crate) fn escape_locks(index: Res<db_sim::SimIndex>, mechs: Query<&Mechanism>, mut locks: Query<(&EscapeLock, &mut Visibility)>, mut chains: Query<&mut EscapeChain>, mut players: Query<&mut AnimationPlayer>) {
    let moved = |id: SimId| index.get(id).and_then(|e| mechs.get(e).ok()).is_some_and(|m| m.on || m.t > 0.0);
    for (l, mut v) in &mut locks {
        if moved(l.0) && *v != Visibility::Hidden {
            *v = Visibility::Hidden;
        }
    }
    for mut c in &mut chains {
        if c.played || !moved(c.mech) {
            continue;
        }
        let Some(mut player) = c.armature.and_then(|a| players.get_mut(a).ok()) else { continue };
        player.play(c.clip);
        c.played = true;
    }
}

/// Lays each chain along its door's travel.
pub(crate) fn lay_chains(index: Res<db_sim::SimIndex>, mechs: Query<&Mechanism>, chains: Query<&ChainLinks>, mut links: Query<(&mut Transform, &mut Visibility)>) {
    for c in &chains {
        let Some(m) = index.get(c.mech).and_then(|e| mechs.get(e).ok()) else { continue };
        let (at, look) = chain_links(m, m.t);
        let rot = Quat::from_xyzw(-look.x, -look.z, -look.y, look.w);
        for (i, &e) in c.links.iter().enumerate() {
            let Ok((mut t, mut v)) = links.get_mut(e) else { continue };
            match at.get(i) {
                Some(&p) => {
                    t.translation = to_bevy(p);
                    t.rotation = rot;
                    *v = Visibility::Inherited;
                }
                None => *v = Visibility::Hidden,
            }
        }
    }
}

/// Hides level meshes standing on a chained door's resting links (baked copies of its chain, see the module docs):
/// checked as their transforms settle, our own links (under a [`ChainLinks`]) left alone.
#[allow(clippy::type_complexity)]
pub(crate) fn hide_baked_chains(mechs: Query<&Mechanism>, chains: Query<&ChainLinks>, index: Res<db_sim::SimIndex>, settled: Query<(Entity, &GlobalTransform), (With<Mesh3d>, Changed<GlobalTransform>)>, parents: Query<&ChildOf>, ours: Query<(), With<ChainLinks>>, mut vis: Query<&mut Visibility>) {
    if settled.is_empty() || chains.is_empty() {
        return;
    }
    let rest: Vec<Vec3> = chains
        .iter()
        .filter_map(|c| index.get(c.mech).and_then(|e| mechs.get(e).ok()))
        .flat_map(|m| {
            let p = &m.placement;
            let yaw = Quat::from_rotation_z(p.yaw.to_radians());
            chain_links(m, 0.0).0.into_iter().map(move |l| to_bevy(p.pos + yaw * (p.scale * l)))
        })
        .collect();
    for (e, gt) in &settled {
        let at = gt.translation();
        if !rest.iter().any(|r| r.distance_squared(at) < 0.02 * 0.02) {
            continue;
        }
        let mut up = Some(e);
        let mut mine = false;
        while let Some(x) = up {
            if ours.contains(x) {
                mine = true;
                break;
            }
            up = parents.get(x).ok().map(|c| c.parent());
        }
        if !mine && let Ok(mut v) = vis.get_mut(e) {
            *v = Visibility::Hidden;
        }
    }
}

/// Moves the parts with their mechanism's travel and turns the cranks.
pub(crate) fn animate_mechanisms(index: Res<db_sim::SimIndex>, mechs: Query<&Mechanism>, levers: Query<&Lever>, mut movers: Query<(&MechMover, &mut Transform), Without<LeverCrank>>, mut cranks: Query<(&LeverCrank, &mut Transform), Without<MechMover>>, time: Res<Time>) {
    for (m, mut t) in &mut movers {
        let Some(mech) = index.get(m.mech).and_then(|e| mechs.get(e).ok()) else { continue };
        let k = mech.progress(mech.t);
        let r = Quat::IDENTITY.slerp(m.turn, k);
        t.translation = m.pivot + r * (m.rest - m.pivot) + m.travel * k;
        t.rotation = r * m.rest_rot;
    }
    for (c, mut t) in &mut cranks {
        let Some(lever) = index.get(c.lever).and_then(|e| levers.get(e).ok()) else { continue };
        let target = c.rest * Quat::from_rotation_x(if lever.on { -std::f32::consts::FRAC_PI_2 } else { 0.0 });
        t.rotation = t.rotation.slerp(target, (time.delta_secs() * 4.0).min(1.0));
    }
}

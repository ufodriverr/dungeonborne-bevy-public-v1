//! Converted models in the client (milestone A): spawning an imported glTF/GLB scene and playing one
//! of its clips. `--model <path> [--clip <n>]` (dev) shows a model in the test room, in front of the
//! player. Paths are relative to the asset root `_data/assets` (files come in through). The
//! converted files are already metres and Y-up; no extra scale. Monsters with an imported model
//! ([`MONSTER_MODELS`]) get it instead of their greybox body, animated from sim state.

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use db_sim::{Position, SimId, SimIndex};

use crate::monster_models::MONSTER_MODELS;
use crate::to_bevy;

/// Dev: a converted model to show in the test room, and which of its clips to play.
#[derive(Resource, Debug, Default, Clone)]
pub struct ModelViewer {
    pub path: Option<String>,
    pub clip: usize,
    /// Extra turn (degrees) on top of facing the player, to find a model's forward axis.
    pub yaw: f32,
}

/// The clip a spawned model plays once its scene is in the world.
#[derive(Component)]
pub(crate) struct PlayClip {
    graph: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
}

/// Spawns `path`'s first scene at `at` (Bevy space) turned by `yaw` (radians about +Y), playing clip
/// `clip` on repeat.
pub(crate) fn spawn_model(commands: &mut Commands, assets: &AssetServer, graphs: &mut Assets<AnimationGraph>, path: &str, clip: usize, at: Vec3, yaw: f32) -> Entity {
    let (graph, index) = AnimationGraph::from_clip(assets.load(GltfAssetLabel::Animation(clip).from_asset(path.to_string())));
    commands
        .spawn((
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
            PlayClip { graph: graphs.add(graph), index },
            crate::flow::LevelScoped,
        ))
        .observe(play_when_ready)
        .id()
}

fn play_when_ready(ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, clips: Query<&PlayClip>, mut players: Query<&mut AnimationPlayer>) {
    let Ok(clip) = clips.get(ready.entity) else { return };
    for child in children.iter_descendants(ready.entity) {
        if let Ok(mut player) = players.get_mut(child) {
            player.play(clip.index).repeat();
            commands.entity(child).insert(AnimationGraphHandle(clip.graph.clone()));
        }
    }
}

/// A monster drawn with its converted skinned model: its mesh, the clip files it uses (empty when the
/// clips are embedded in the mesh file), the clip-name prefix and the clip played in each sim state.
/// Attack clips come from the attack montage's segments. Clip names match in any case. The models face
/// local +Z. The table is generated (`monster_models.rs`).
pub(crate) struct MonsterModel {
    pub row: &'static str,
    pub path: &'static str,
    pub clips: &'static [&'static str],
    pub prefix: &'static str,
    /// The bone that carries root motion (dropped; the sim moves the monster).
    pub root_bone: &'static str,
    pub idle: &'static str,
    pub walk: &'static str,
    pub run: &'static str,
    /// Planar speed (cm/s) from which the run clip plays instead of the walk clip.
    pub run_from: f32,
    pub hit: &'static str,
    pub death: &'static str,
}

/// Whether `path` has been imported into the asset root (`_data/assets`).
pub(crate) fn imported(path: &str) -> bool {
    db_formats::data_path("assets").join(path).is_file()
}

/// What a model body is playing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Playing {
    Idle,
    Walk,
    Run,
    /// An attack: the swing's montage index and the tick it started.
    Attack(u8, u32),
    Hit,
    Death,
    /// Dormant: its sleep clip looping, or the wake clip held at its first frame.
    Dormant,
    /// Waking: the wake clip once.
    WakeUp,
    /// A Goblin Thief ability: its index in the kit's thief abilities and the tick it started.
    Ability(u8, u32),
}

/// Monsters farther than this from the camera (m) keep their last pose.
const ANIMATE_RANGE_M: f32 = 40.0;

/// The skinned body of a sim monster.
#[derive(Component)]
pub(crate) struct ModelBody {
    pub(crate) id: SimId,
    model: &'static MonsterModel,
    gltf: Handle<bevy::gltf::Gltf>,
    /// The separate clip files' clips (empty when they're embedded in the mesh file).
    clip_files: Vec<(String, Handle<AnimationClip>)>,
    /// Clip name (lower case, without the prefix) to graph node and clip length (s), once loaded.
    nodes: Option<std::collections::HashMap<String, (AnimationNodeIndex, f32)>>,
    graph: Option<Handle<AnimationGraph>>,
    /// The graph once on the player, and whether it is on: it is taken off while the body isn't animated
    /// (dormant or far).
    graph_on: Option<Handle<AnimationGraph>>,
    animated: bool,
    player: Option<Entity>,
    root_bone: Option<Entity>,
    playing: Option<Playing>,
    last_health: f64,
    /// Seconds left of a hit reaction.
    hit_left: f32,
    /// Each attack's montage segments (loaded on its first swing) and the clip node a swing shows now.
    segments: std::collections::HashMap<u8, Vec<(String, db_sim::monsters::MontageSegment)>>,
    attack_node: Option<AnimationNodeIndex>,
    /// The thief abilities' montage segments, loaded on first use.
    ability_segments: std::collections::HashMap<u8, Vec<(String, db_sim::monsters::MontageSegment)>>,
}

impl ModelBody {
    /// Whether its model file (with its textures) and every clip file have loaded (or failed: nothing more comes).
    pub(crate) fn loaded(&self, assets: &AssetServer) -> bool {
        let done = |id: bevy::asset::UntypedAssetId| assets.is_loaded_with_dependencies(id) || matches!(assets.get_load_state(id), Some(bevy::asset::LoadState::Failed(_)));
        done(self.gltf.id().untyped()) && self.clip_files.iter().all(|(_, h)| done(h.id().untyped()))
    }
}

/// A clip's name in lower case without the model's prefix (`M_Goblin_Axe_Attack1` -> `attack1`).
fn short_name(name: &str, prefix: &str) -> String {
    let lower = name.to_lowercase();
    let prefix = prefix.to_lowercase();
    lower.strip_prefix(&prefix).map_or(lower.clone(), |s| s.trim_start_matches('_').to_string())
}

/// Builds a body's animation graph once its file and clips have loaded (false until they have).
fn ensure_graph(body: &mut ModelBody, gltfs: &Assets<bevy::gltf::Gltf>, clips: &Assets<AnimationClip>, graphs: &mut Assets<AnimationGraph>) -> bool {
    if body.nodes.is_some() {
        return true;
    }
    let named: Vec<(String, Handle<AnimationClip>)> = if body.clip_files.is_empty() {
        let Some(gltf) = gltfs.get(&body.gltf) else { return false };
        gltf.named_animations.iter().map(|(n, h)| (short_name(n, body.model.prefix), h.clone())).collect()
    } else {
        body.clip_files.clone()
    };
    if named.iter().any(|(_, h)| clips.get(h).is_none()) {
        return false;
    }
    let mut graph = AnimationGraph::new();
    let mut nodes = std::collections::HashMap::new();
    for (short, h) in named {
        let duration = clips.get(&h).map_or(1.0, AnimationClip::duration);
        let root = graph.root;
        nodes.insert(short, (graph.add_clip(h, 1.0, root), duration));
    }
    body.nodes = Some(nodes);
    body.graph = Some(graphs.add(graph));
    true
}

/// Spawns the converted model for a monster when one is imported; `false` if there is none (the
/// greybox body is used instead).
pub(crate) fn spawn_model_body(commands: &mut Commands, assets: &AssetServer, id: SimId, row: &str) -> bool {
    let Some(model) = MONSTER_MODELS.iter().find(|m| m.row == row && imported(m.path) && m.clips.iter().all(|c| imported(c))) else { return false };
    let clip_files = model.clips.iter().map(|c| (short_name(std::path::Path::new(c).file_stem().and_then(|s| s.to_str()).unwrap_or_default(), model.prefix), assets.load(GltfAssetLabel::Animation(0).from_asset(c.to_string())))).collect();
    commands
        .spawn((
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model.path.to_string()))),
            Transform::default(),
            Visibility::default(),
            ModelBody { id, model, gltf: assets.load(model.path.to_string()), clip_files, nodes: None, graph: None, graph_on: None, animated: true, player: None, root_bone: None, playing: None, last_health: f64::NAN, hit_left: 0.0, segments: Default::default(), attack_node: None, ability_segments: Default::default() },
            crate::flow::LevelScoped,
        ))
        .observe(find_player_and_root);
    true
}

/// Once the scene is in: remember its animation player and its root-motion bone (whose root motion is
/// dropped, since the sim moves the monster). A mesh file without clips has no player: the clips come
/// from separate files whose bone paths start at the common `Armature` node, so the player goes on
/// `Armature` and every bone below gets the target id the glTF loader gives those clips' curves (its
/// name path from `Armature`).
fn find_player_and_root(ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, players: Query<(), With<AnimationPlayer>>, names: Query<&Name>, mut bodies: Query<&mut ModelBody>) {
    let Ok(mut body) = bodies.get_mut(ready.entity) else { return };
    let root = body.model.root_bone;
    let mut armature = None;
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            body.player = Some(e);
        }
        let name = names.get(e).map(Name::as_str);
        if name == Ok(root) {
            body.root_bone = Some(e);
        }
        if name == Ok("Armature") && armature.is_none() {
            armature = Some(e);
        }
    }
    if let (None, Some(arm)) = (body.player, armature) {
        drive_from_armature(&mut commands, &children, &names, arm);
        body.player = Some(arm);
    }
}

/// Puts an animation player on `arm` (an `Armature` node) and gives every node below it the target id
/// the glTF loader gives separate clip files' curves: its name path from `Armature`.
pub(crate) fn drive_from_armature(commands: &mut Commands, children: &Query<&Children>, names: &Query<&Name>, arm: Entity) {
    commands.entity(arm).insert(AnimationPlayer::default());
    let mut stack = vec![(arm, vec![names.get(arm).cloned().unwrap_or_default()])];
    while let Some((e, path)) = stack.pop() {
        commands.entity(e).insert((bevy::animation::AnimationTargetId::from_names(path.iter()), bevy::animation::AnimatedBy(arm)));
        for &c in children.get(e).into_iter().flatten() {
            let mut p = path.clone();
            p.push(names.get(c).cloned().unwrap_or_default());
            stack.push((c, p));
        }
    }
}

/// Follows the sim actor and picks the clip from its state: dead, death; swinging, the attack clip
/// timed to the swing; just hurt, a hit reaction; moving, walk or run; else idle.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn animate_model_bodies(
    mut commands: Commands,
    time: Res<Time>,
    index: Res<SimIndex>,
    gltfs: Res<Assets<bevy::gltf::Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    actors: Query<(&Position, Option<&crate::PrevPosition>, &db_sim::Aim, &db_sim::Melee, &db_sim::Health, Option<&db_sim::CharacterState>, Option<&db_sim::MonsterKit>, Option<&db_sim::MonsterAi>, Option<&db_sim::shadow_dragon::ShadowDragon>)>,
    fixed: Res<Time<Fixed>>,
    camera: Query<&GlobalTransform, With<crate::PlayerCamera>>,
    mut bodies: Query<(Entity, &mut ModelBody, &mut Transform, &mut Visibility)>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    tick: Res<db_sim::TickCount>,
    (treants, positions, data): (Query<(&Position, Option<&crate::PrevPosition>, &db_sim::class_skills::Treant)>, Query<&Position>, Option<Res<crate::CombatData>>),
) {
    let eye = camera.iter().next().map(GlobalTransform::translation);
    for (e, mut body, mut t, mut vis) in &mut bodies {
        let Some(sim) = index.get(body.id) else {
            commands.entity(e).despawn();
            continue;
        };
        let Ok((pos, prev, aim, melee, health, state, kit, ai, dragon)) = actors.get(sim) else {
            // The Druid's treant: a summon with no aim, swing or health of its own (`class_skills::Treant`).
            let Ok((pos, prev, tr)) = treants.get(sim) else { continue };
            t.translation = to_bevy(crate::drawn(pos, prev, &fixed)) - Vec3::Y * 0.9;
            // Facing what it swings at (or the enemy Shadow Pounce marked); else as it was.
            let target = tr.swing.map(|s| s.0).or(tr.marked).and_then(|id| index.get(id)).and_then(|e| positions.get(e).ok());
            if let Some(d) = target.map(|p| p.0 - pos.0).filter(|d| d.truncate().length() > 1.0) {
                t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - d.y.atan2(d.x));
            }
            t.scale = Vec3::splat(data.as_ref().and_then(|d| d.monster("Treant")).and_then(|m| m.kit.bp.mesh_scale).unwrap_or(1.0));
            if !ensure_graph(&mut body, &gltfs, &clips, &mut graphs) {
                continue;
            }
            let Some(player_e) = body.player else { continue };
            if let Some(g) = body.graph.take() {
                commands.entity(player_e).try_insert((AnimationGraphHandle(g.clone()), AnimationTransitions::new()));
                body.graph_on = Some(g);
                continue;
            }
            let Ok((mut player, Some(mut transitions))) = players.get_mut(player_e) else { continue };
            // Each swing plays the Treant's `Attack2` from its start (the summon's swing is `AM_Treant_Attack2`'s,
            // HYPOTHESIS: its montage segments aren't applied); between swings it idles.
            let nodes = body.nodes.as_ref().expect("built above");
            let attack = nodes.get("attack2").copied();
            let age = |start: u32| (tick.0 as u32).saturating_sub(start) as f32 * db_sim::DT;
            let len = attack.map_or(1.0, |a| a.1);
            let want = match (tr.swing, body.playing) {
                (Some(_), Some(Playing::Attack(0, start))) if age(start) < len * 0.9 => Playing::Attack(0, start),
                (Some(_), _) => Playing::Attack(0, tick.0 as u32),
                (None, Some(Playing::Attack(0, start))) if age(start) < len => Playing::Attack(0, start),
                _ => Playing::Idle,
            };
            if body.playing != Some(want) {
                let (node, repeat) = if matches!(want, Playing::Attack(..)) { (attack.map(|a| a.0), false) } else { (nodes.get(&body.model.idle.to_lowercase()).map(|n| n.0), true) };
                if let Some(node) = node {
                    let active = transitions.play(&mut player, node, std::time::Duration::from_secs_f32(0.15));
                    if repeat {
                        active.repeat();
                    }
                }
                body.playing = Some(want);
            }
            continue;
        };
        // The capsule centre stands 90 cm above the feet; the model's origin is at its feet. Drawn between
        // the last two ticks.
        t.translation = to_bevy(crate::drawn(pos, prev, &fixed)) - Vec3::Y * 0.9;
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - aim.yaw);
        // Its Blueprint's mesh scale (the mesh's origin stays at the feet).
        t.scale = Vec3::splat(kit.and_then(|k| k.bp.mesh_scale).unwrap_or(1.0));
        // A dormant monster its Blueprint makes invisible (the thief) isn't drawn.
        vis.set_if_neq(if ai.is_some_and(|a| a.dormant && a.invisible_dormant) { Visibility::Hidden } else { Visibility::Inherited });
        // Build the graph once the file and its clips have loaded.
        if !ensure_graph(&mut body, &gltfs, &clips, &mut graphs) {
            continue;
        }
        let Some(player_e) = body.player else { continue };
        if let Some(g) = body.graph.take() {
            commands.entity(player_e).try_insert((AnimationGraphHandle(g.clone()), AnimationTransitions::new()));
            body.graph_on = Some(g);
            continue;
        }
        // Animation LOD: a dormant monster, or one farther than `ANIMATE_RANGE_M` from the camera, keeps
        // its last pose. Without the graph handle Bevy skips every bone of it with one lookup. The
        // `try_` commands: the body can be despawned in the same frame (run end, the sim actor gone).
        // (The Shadow Dragon flies 30-70 m up and away: always animated.)
        let animate = dragon.is_some() || (ai.is_none_or(|a| a.awake) && eye.is_none_or(|c| c.distance(t.translation) <= ANIMATE_RANGE_M));
        if !animate {
            if body.animated {
                commands.entity(player_e).try_remove::<AnimationGraphHandle>();
                body.animated = false;
                // Picked again on waking (a fade from the kept pose).
                body.playing = None;
            }
            continue;
        }
        if !body.animated {
            if let Some(g) = body.graph_on.clone() {
                commands.entity(player_e).try_insert(AnimationGraphHandle(g));
            }
            body.animated = true;
        }
        let Ok((mut player, Some(mut transitions))) = players.get_mut(player_e) else { continue };
        let dt = time.delta_secs();
        if health.current < body.last_health && health.current > 0.0 {
            body.hit_left = body.nodes.as_ref().and_then(|n| n.get(&body.model.hit.to_lowercase())).map_or(0.0, |c| c.1.min(0.6));
        }
        body.last_health = health.current;
        body.hit_left = (body.hit_left - dt).max(0.0);
        let speed = state.map_or(0.0, |s| s.velocity.truncate().length());
        // The Shadow Dragon shows the montage its script plays at the script's time; between corners it flies with
        // its `MoveFront` (HYPOTHESIS: its locomotion clip).
        if let Some(d) = dragon.filter(|_| health.current > 0.0) {
            let nodes = body.nodes.as_ref().expect("built above");
            let shown = match d.montage() {
                Some((clip, at)) => db_sim::monsters::clip_time(&clip.segments, at).and_then(|(c, at)| nodes.get(&short_name(c, body.model.prefix)).map(|n| (n.0, Some(at)))),
                None => nodes.get(&body.model.run.to_lowercase()).map(|n| (n.0, None)),
            };
            if let Some((node, at)) = shown {
                if body.attack_node != Some(node) {
                    let active = transitions.play(&mut player, node, std::time::Duration::from_secs_f32(0.15));
                    if at.is_none() {
                        active.repeat();
                    }
                    body.attack_node = Some(node);
                }
                if let (Some(at), Some(active)) = (at, player.animation_mut(node)) {
                    active.set_speed(0.0).seek_to(at);
                }
            }
            body.playing = None;
            continue;
        }
        let swing = melee.swing.as_ref().and_then(|s| if let db_sim::HitSource::Monster(i) = s.by { Some((i, s)) } else { None });
        let want = if health.current <= 0.0 {
            Playing::Death
        } else if let Some((i, s)) = swing {
            // The same swing as last frame keeps its start tick.
            match body.playing {
                Some(Playing::Attack(j, start)) if j == i && start <= s.tick => Playing::Attack(i, start),
                _ => Playing::Attack(i, s.tick),
            }
        } else if ai.is_some_and(|a| a.state == db_sim::ai::AiState::WakeUp) {
            Playing::WakeUp
        } else if ai.is_some_and(|a| a.state == db_sim::ai::AiState::Dormant) {
            Playing::Dormant
        } else if let Some(act) = ai.and_then(|a| a.thief.active) {
            Playing::Ability(act.ability as u8, act.start)
        } else if body.hit_left > 0.0 {
            Playing::Hit
        } else if speed > body.model.run_from {
            Playing::Run
        } else if speed > 20.0 {
            Playing::Walk
        } else {
            Playing::Idle
        };
        if let (Playing::Attack(i, _), Some((_, s))) = (want, swing) {
            // A swing shows its montage: the montage's segments say which part of the clip plays when
            // and how fast (e.g. `AM_Goblin_Axe_Attack2` uses 0-2.7 s of its 5.4 s clip). The sim's
            // swing spans the montage, so the swing tick gives the montage time.
            let attack = kit.and_then(|k| k.attacks.get(usize::from(i)));
            if !body.segments.contains_key(&i) {
                let dir = db_sim::monsters::spec(body.model.row).map_or("", |m| m.dir);
                let segments = attack.map_or(Vec::new(), |a| db_sim::monsters::load_montage_segments(dir, &a.name));
                body.segments.insert(i, segments);
            }
            // A shield loops its `ChargeLoop` section until the release.
            let t = attack.map_or(0.0, |a| a.montage_time(s.tick as f32 / s.length.max(1) as f32 * a.length));
            let nodes = body.nodes.as_ref().expect("built above");
            let shown = match db_sim::monsters::clip_time(&body.segments[&i], t) {
                Some((clip, at)) => nodes.get(&short_name(clip, body.model.prefix)).map(|n| (n.0, at)),
                // No segments: the clip of the montage's name, stretched over the swing.
                None => attack.and_then(|a| nodes.get(&short_name(&format!("M_{}", a.name.trim_start_matches("AM_")), body.model.prefix))).map(|n| (n.0, t / attack.map_or(1.0, |a| a.length.max(1e-3)) * n.1)),
            };
            let Some((node, at)) = shown else {
                body.playing = Some(want);
                continue;
            };
            if body.playing != Some(want) || body.attack_node != Some(node) {
                transitions.play(&mut player, node, std::time::Duration::from_secs_f32(0.1));
                body.attack_node = Some(node);
            }
            if let Some(active) = player.animation_mut(node) {
                active.set_speed(0.0).seek_to(at);
            }
            body.playing = Some(want);
            continue;
        }
        if let Playing::Ability(i, start) = want {
            // A thief ability shows its montage at the ability's time (ExitDungeons slows to a
            // near-stop in its second segment).
            let ability = kit.and_then(|k| k.thief.as_deref()).and_then(|k| k.abilities.get(usize::from(i)));
            if !body.ability_segments.contains_key(&i) {
                let dir = db_sim::monsters::spec(body.model.row).map_or("", |m| m.dir);
                let segments = ability.map_or(Vec::new(), |a| db_sim::monsters::load_montage_segments(dir, &a.montage));
                body.ability_segments.insert(i, segments);
            }
            let at = (tick.0 as u32).saturating_sub(start) as f32 * db_sim::DT;
            let nodes = body.nodes.as_ref().expect("built above");
            let Some((node, at)) = db_sim::monsters::clip_time(&body.ability_segments[&i], at).and_then(|(clip, at)| nodes.get(&short_name(clip, body.model.prefix)).map(|n| (n.0, at))) else {
                body.playing = Some(want);
                continue;
            };
            if body.playing != Some(want) || body.attack_node != Some(node) {
                transitions.play(&mut player, node, std::time::Duration::from_secs_f32(0.15));
                body.attack_node = Some(node);
            }
            if let Some(active) = player.animation_mut(node) {
                active.set_speed(0.0).seek_to(at);
            }
            body.playing = Some(want);
            continue;
        }
        if body.playing == Some(want) {
            continue;
        }
        let nodes = body.nodes.as_ref().expect("built above");
        if matches!(want, Playing::Dormant | Playing::WakeUp) {
            // The kit's dormant clips (sequence names; `db_sim::monsters::Dormancy`).
            let dormancy = kit.map(|k| &k.dormancy);
            let clip = |c: Option<&(String, f32)>| c.and_then(|(n, _)| nodes.get(&short_name(n, body.model.prefix))).map(|n| n.0);
            let (sleep, wake) = (clip(dormancy.and_then(|d| d.sleep.as_ref())), clip(dormancy.and_then(|d| d.wake.as_ref())));
            match (want, sleep, wake) {
                // A mimic's sleep clip shuts its lid: played once, it holds the closed chest (its
                // `Dormants` have `AnimationUsePose`). Other monsters' sleep clips loop.
                (Playing::Dormant, Some(node), _) if ai.is_some_and(|a| a.wake_on_open) => {
                    transitions.play(&mut player, node, std::time::Duration::ZERO);
                }
                (Playing::Dormant, Some(node), _) => {
                    transitions.play(&mut player, node, std::time::Duration::from_secs_f32(0.2)).repeat();
                }
                (Playing::Dormant, None, Some(node)) => {
                    transitions.play(&mut player, node, std::time::Duration::ZERO).set_speed(0.0).seek_to(0.0);
                }
                (Playing::WakeUp, _, Some(node)) => {
                    transitions.play(&mut player, node, std::time::Duration::from_secs_f32(0.1));
                }
                _ => {}
            }
            body.playing = Some(want);
            continue;
        }
        let (name, repeat, fade) = match want {
            Playing::Idle => (body.model.idle.to_string(), true, 0.25),
            Playing::Walk => (body.model.walk.to_string(), true, 0.2),
            Playing::Run => (body.model.run.to_string(), true, 0.2),
            Playing::Hit => (body.model.hit.to_string(), false, 0.1),
            Playing::Death => (body.model.death.to_string(), false, 0.15),
            // Handled above.
            Playing::Attack(_, _) | Playing::Dormant | Playing::WakeUp | Playing::Ability(_, _) => continue,
        };
        let Some(&(node, _)) = nodes.get(&name.to_lowercase()) else {
            body.playing = Some(want);
            continue;
        };
        let active = transitions.play(&mut player, node, std::time::Duration::from_secs_f32(fade));
        if repeat {
            active.repeat();
        }
        body.playing = Some(want);
    }
}

/// Drops the root motion: the `Root` bone goes back to its rest pose after the clips are applied.
pub(crate) fn drop_root_motion(bodies: Query<&ModelBody>, mut bones: Query<&mut Transform>) {
    for body in &bodies {
        if let Some(mut t) = body.root_bone.and_then(|b| bones.get_mut(b).ok()) {
            *t = Transform::IDENTITY;
        }
    }
}

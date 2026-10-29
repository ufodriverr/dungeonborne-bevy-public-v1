//! The breakable props: each prop's mesh (`ObjectNode`'s static mesh, `db_sim::destroyables`) at the actor's origin, yaw
//! and scale, hidden once it breaks. Then it shatters: its geometry collection's rigid pieces (all 39 classes) fly apart,
//! lie still for `DestroyDelayTime` (1.5 s) and fade over the 3 s `DestroyTrack`, as `BP_DestroyableBase` does. The
//! pieces' motion is ours (HYPOTHESIS): `BP_MasterField`'s push isn't in the data we read, and Chaos isn't run. The fade
//! is ours too: the pieces shrink away (the props' own materials can't be blended: the claypots' textures carry other
//! data in alpha, so a blended copy vanishes). Reads sim state only.

use std::collections::{BTreeSet, HashMap};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use db_sim::destroyables::Destroyable;
use db_sim::{Health, SimId, SimIndex};

use crate::to_bevy;

/// `BP_DestroyableBase`'s `DestroyDelayTime` (CONFIRMED defaults), then its `DestroyTrack` fade (3 s).
const HOLD_SECS: f32 = 1.5;
const FADE_SECS: f32 = 3.0;
/// The pieces' push (ours): outwards from the prop's middle and up (m/s), and their spin (rad/s).
const PUSH_OUT: f32 = 1.6;
const PUSH_UP: f32 = 1.2;
const SPIN: f32 = 4.0;
/// Gravity on the pieces (m/s^2) and how much of their slide a landing keeps.
const GRAVITY: f32 = 9.8;
const LANDING_KEEP: f32 = 0.35;

/// A prop's body (not an `ActorView`: the sim's `Position` is its hit box's centre, not its origin).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct PropView(SimId);

/// A flying piece of a broken prop: its velocity, spin axis and rate, the floor it lands on (Bevy y), its age and size.
#[derive(Component, Debug, Clone)]
pub(crate) struct Piece {
    vel: Vec3,
    spin: (Vec3, f32),
    floor: f32,
    age: f32,
    scale: Vec3,
}

#[derive(serde::Deserialize)]
struct PieceFile {
    pieces: Vec<PieceData>,
}

#[derive(serde::Deserialize)]
struct PieceData {
    at: [f32; 3],
    p: Vec<f32>,
    n: Vec<f32>,
    uv: Vec<f32>,
    i: Vec<u32>,
}

/// A class's pieces: each one's centre (Unreal cm in the actor's space) and mesh.
type Pieces = Vec<(Vec3, Handle<Mesh>)>;

/// Each class's pieces, loaded once; `None`: it has none.
#[derive(Resource, Default)]
pub(crate) struct PieceCache(HashMap<String, Option<Pieces>>);

/// Bodies for new props.
pub(crate) fn spawn_destroyable_views(mut commands: Commands, assets: Res<AssetServer>, added: Query<(&SimId, &Destroyable), Added<Destroyable>>) {
    for (&id, d) in &added {
        // A prop with no mesh of its own (a Stoneshaping wall) is drawn by the effect that raises it.
        let (model, look) = if d.spec.model.is_empty() {
            let Some((model, scale)) = crate::throwables_view::wall_model(&d.spec.class) else { continue };
            (model.to_string(), Some(scale))
        } else {
            (d.spec.model.clone(), None)
        };
        let s = d.scale * d.spec.scale * look.unwrap_or(1.0);
        let at = Transform { translation: to_bevy(d.origin), rotation: Quat::from_rotation_y(-d.yaw), scale: Vec3::new(s.x, s.z, s.y) };
        let mut view = commands.spawn((PropView(id), at, Visibility::default(), crate::flow::LevelScoped, WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model)))));
        if let Some(scale) = look {
            view.insert(crate::throwables_view::WallRise { age: 0.0, scale });
        }
    }
}

fn load_pieces(class: &str, meshes: &mut Assets<Mesh>) -> Option<Pieces> {
    let path = db_formats::data_path(format!("recovered/destroyable_pieces/{class}.json"));
    let file: PieceFile = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let triples = |v: &[f32]| v.as_chunks::<3>().0.to_vec();
    Some(
        file.pieces
            .into_iter()
            .map(|p| {
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, triples(&p.p));
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, triples(&p.n));
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, p.uv.as_chunks::<2>().0.to_vec());
                mesh.insert_indices(Indices::U32(p.i));
                (Vec3::from_array(p.at), meshes.add(mesh))
            })
            .collect(),
    )
}

/// Breaks props as the sim breaks them (their pieces fly) and removes the bodies of the ones the sim removed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_destroyable_views(
    mut commands: Commands,
    index: Res<SimIndex>,
    props: Query<(&Health, &Destroyable)>,
    mut views: Query<(Entity, &PropView, &mut Visibility)>,
    mut cache: ResMut<PieceCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (children, drawn): (Query<&Children>, Query<&MeshMaterial3d<StandardMaterial>>),
    mut broken: Local<BTreeSet<SimId>>,
) {
    for (e, view, mut vis) in &mut views {
        let Some(s) = index.get(view.0) else {
            commands.entity(e).despawn();
            broken.remove(&view.0);
            continue;
        };
        let Ok((h, d)) = props.get(s) else { continue };
        if h.current > 0.0 || !broken.insert(view.0) {
            continue;
        }
        *vis = Visibility::Hidden;
        let pieces = cache.0.entry(d.spec.class.clone()).or_insert_with(|| load_pieces(&d.spec.class, &mut meshes)).clone();
        let Some(pieces) = pieces else {
            info!("prop {} broke: no pieces", d.spec.class);
            continue;
        };
        info!("prop {} broke: {} pieces", d.spec.class, pieces.len());
        // The model's own material (HYPOTHESIS: the collections' inner faces share it).
        let material = children.iter_descendants(e).find_map(|c| drawn.get(c).ok()).map_or_else(|| materials.add(StandardMaterial::default()), |m| m.0.clone());
        let rot = Quat::from_rotation_y(-d.yaw);
        let scale = Vec3::new(d.scale.x, d.scale.z, d.scale.y);
        let floor = to_bevy(d.origin).y;
        let middle = to_bevy(d.origin + Vec3::Z * d.spec.center.z * d.scale.z);
        for (k, (at, mesh)) in pieces.into_iter().enumerate() {
            // Unreal cm in the actor's space -> Bevy, turned and scaled as the actor.
            let local = Vec3::new(at.x, at.z, at.y) * 0.01 * scale;
            let pos = to_bevy(d.origin) + rot * local;
            let out = (pos - middle).with_y(0.0).normalize_or(Vec3::X);
            // Spread by the piece's index (the client has no RNG of its own here; ours).
            let jitter = (k as f32 * 2.399).sin();
            let vel = out * PUSH_OUT * (1.0 + 0.3 * jitter) + Vec3::Y * PUSH_UP * (1.0 - 0.3 * jitter);
            let axis = out.cross(Vec3::Y).normalize_or(Vec3::Z);
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform { translation: pos, rotation: rot, scale },
                Piece { vel, spin: (axis, SPIN * (1.0 + 0.5 * jitter)), floor, age: 0.0, scale },
                crate::flow::LevelScoped,
            ));
        }
    }
}

/// The pieces fall, land, lie, fade and go.
pub(crate) fn update_pieces(mut commands: Commands, time: Res<Time>, mut pieces: Query<(Entity, &mut Piece, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut t) in &mut pieces {
        p.age += dt;
        if p.age >= HOLD_SECS + FADE_SECS {
            commands.entity(e).despawn();
            continue;
        }
        if p.vel != Vec3::ZERO {
            p.vel.y -= GRAVITY * dt;
            t.translation += p.vel * dt;
            t.rotate(Quat::from_axis_angle(p.spin.0, p.spin.1 * dt));
            if t.translation.y <= p.floor + 0.03 {
                t.translation.y = p.floor + 0.03;
                p.vel = (p.vel * LANDING_KEEP).with_y(0.0);
                p.spin.1 *= LANDING_KEEP;
                if p.vel.length() < 0.05 {
                    p.vel = Vec3::ZERO;
                }
            }
        }
        if p.age > HOLD_SECS {
            t.scale = p.scale * (1.0 - ((p.age - HOLD_SECS) / FADE_SECS).min(1.0));
        }
    }
}

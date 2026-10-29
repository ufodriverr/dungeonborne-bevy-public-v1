//! The breakable props: barrels, boxes and claypots (`TGF/Environment/Destroyable/`, all
//! `BP_DestroyableBase`), from `_data/recovered/destroyables.json`.
//!
//! CONFIRMED (blueprints, bytecode): a prop has the `InteractorBattleComponent`'s `TotalHP` (50 for every class); its
//! death hides its mesh, shows the shattering geometry collection, plays its `DeathSound`, drops its loot (`Loot(...)`
//! by the class's display name: collectables 100 cm above it, falling to the floor) and is destroyed after
//! `DestroyDelayTime` (1.5 s) and its fade timeline (3 s). Its hit box is its mesh's bounds (`TraceBox`). HYPOTHESIS:
//! it blocks characters while whole (a pawn-only box) and is null area for the monsters' grids when taller than a step;
//! its defence (`overrides::destroyables`).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use avian3d::prelude::Rotation;
use bevy_ecs::prelude::*;
use bevy_math::{Quat, Vec3};

use crate::command::{ActorKind, Kind, LevelBox};
use crate::id::{SimIdAllocator, SimIndex};
use crate::melee::{Combatant, Health};
use crate::movement::static_box;
use crate::{DT, Position, SimEvent, SimEvents, SimId, TickCount};

/// One breakable prop class.
#[derive(Debug, Clone, PartialEq)]
pub struct DestroyableSpec {
    pub class: String,
    /// Its mesh (`_data/assets/` path).
    pub model: String,
    /// The mesh's scale in the blueprint.
    pub scale: Vec3,
    /// Its hit box (the mesh's bounds) in actor space, cm.
    pub center: Vec3,
    pub half: Vec3,
    pub hp: f64,
    /// Its break sound (a Wwise event).
    pub sound: String,
    /// Its loot source (`ExcelLootSourceMap`); 0 = none.
    pub source: i32,
}

/// The spec of a prop class (e.g. `BP_ClayPot_4`); `None` if it isn't one (or the data is missing).
pub fn spec(class: &str) -> Option<Arc<DestroyableSpec>> {
    static SPECS: OnceLock<BTreeMap<String, Arc<DestroyableSpec>>> = OnceLock::new();
    SPECS.get_or_init(load).get(class).cloned()
}

fn load() -> BTreeMap<String, Arc<DestroyableSpec>> {
    let Some(json) = std::fs::read(db_formats::data_path("recovered/destroyables.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { return BTreeMap::new() };
    let v3 = |v: &serde_json::Value| Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
    json.as_object()
        .into_iter()
        .flatten()
        .map(|(class, s)| {
            let spec = DestroyableSpec { class: class.clone(), model: s["model"].as_str().unwrap_or("").into(), scale: v3(&s["scale"]), center: v3(&s["center"]), half: v3(&s["half"]), hp: s["hp"].as_f64().unwrap_or(50.0), sound: s["sound"].as_str().unwrap_or("").into(), source: s["source"].as_i64().unwrap_or(0) as i32 };
            (class.clone(), Arc::new(spec))
        })
        .collect()
}

/// A placed prop. `Position` is its hit box's centre.
#[derive(Component, Debug, Clone)]
pub struct Destroyable {
    pub spec: Arc<DestroyableSpec>,
    /// The actor's origin (its floor point) and yaw (radians) and scale.
    pub origin: Vec3,
    pub yaw: f32,
    pub scale: Vec3,
    /// When it broke (tick).
    pub broke_at: Option<u64>,
    /// Its blocking box while whole.
    pub collider: Option<Entity>,
}

impl Destroyable {
    /// Its hit box: world centre and half extents (yawed by `yaw`).
    pub fn hit_box(&self) -> (Vec3, Vec3) {
        (self.origin + Quat::from_rotation_z(self.yaw) * (self.spec.center * self.scale), self.spec.half * self.scale.abs())
    }
}

/// Places prop `class` at `pos` (its origin), turned `yaw_deg` degrees.
pub(crate) fn spawn(world: &mut World, class: &str, pos: Vec3, yaw_deg: f32, scale: Vec3) {
    let Some(spec) = spec(class) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "not a breakable prop" });
        return;
    };
    spawn_spec(world, spec, pos, yaw_deg, scale);
}

/// A prop that breaks by itself at this tick (a Stoneshaping wall).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Expires(pub u64);

/// Places a prop of `spec` at `pos` (its origin), turned `yaw_deg` degrees.
pub(crate) fn spawn_spec(world: &mut World, spec: Arc<DestroyableSpec>, pos: Vec3, yaw_deg: f32, scale: Vec3) -> Entity {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let mut prop = Destroyable { spec: spec.clone(), origin: pos, yaw: yaw_deg.to_radians(), scale, broke_at: None, collider: None };
    let (c, half) = prop.hit_box();
    let pe = world.spawn((static_box(c, half), Rotation(Quat::from_rotation_z(prop.yaw)), LevelBox { half_extents: half }, crate::layers::of_kind(1).expect("pawn-only layers"))).id();
    prop.collider = Some(pe);
    let combatant = Combatant { attack: Default::default(), defence: crate::overrides::destroyables::DEFENCE, guard_reduce: 0.0 };
    let e = world.spawn((id, Kind(ActorKind::Destroyable), Position(c), combatant, Health { current: spec.hp, max: spec.hp }, prop)).id();
    if spec.source != 0 {
        world.entity_mut(e).insert(crate::world_items::LootSource { source_id: spec.source, dropped: false });
    }
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: ActorKind::Destroyable });
    e
}

/// Seconds from a prop breaking to its removal: `DestroyDelayTime` and the fade timeline (CONFIRMED).
pub const GONE_AFTER: f32 = 1.5 + 3.0;

/// Broken props lose their box at once and are removed `GONE_AFTER` s later.
pub fn destroyables_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let props: Vec<(Entity, SimId)> = world.query::<(Entity, &SimId, &Destroyable)>().iter(world).map(|(e, id, _)| (e, *id)).collect();
    for (e, id) in props {
        // Out of time: it breaks as if broken.
        if world.get::<Expires>(e).is_some_and(|x| now >= x.0)
            && let Some(mut h) = world.get_mut::<Health>(e)
        {
            h.current = 0.0;
        }
        let dead = world.get::<Health>(e).is_some_and(|h| h.current <= 0.0);
        let Some(broke_at) = world.get::<Destroyable>(e).map(|d| d.broke_at) else { continue };
        match broke_at {
            None if dead => {
                let mut d = world.get_mut::<Destroyable>(e).expect("a prop");
                d.broke_at = Some(now);
                if let Some(c) = d.collider.take() {
                    world.despawn(c);
                }
            }
            Some(t) if now >= t + (GONE_AFTER / DT).round() as u64 => {
                world.resource_mut::<SimIndex>().remove(id);
                world.despawn(e);
                world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id });
            }
            _ => {}
        }
    }
}

/// The whole props taller than a step, as nav cuts (world centre, yaw, half extents), and their ids (for the cut key).
pub(crate) fn nav_boxes(world: &mut World) -> (Vec<(Vec3, f32, Vec3)>, Vec<u64>) {
    let mut props: Vec<(u64, Vec3, f32, Vec3)> = world
        .query::<(&SimId, &Destroyable)>()
        .iter(world)
        .filter(|(_, d)| d.broke_at.is_none())
        .filter_map(|(id, d)| {
            let (c, half) = d.hit_box();
            (half.z * 2.0 > crate::overrides::movement::MAX_STEP_HEIGHT).then_some((id.0, c, d.yaw, half))
        })
        .collect();
    props.sort_by_key(|p| p.0);
    (props.iter().map(|p| (p.1, p.2, p.3)).collect(), props.iter().map(|p| p.0).collect())
}

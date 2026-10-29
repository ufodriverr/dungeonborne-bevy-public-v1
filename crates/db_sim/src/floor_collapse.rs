//! The Shadow King's floor collapse. CONFIRMED (bytecode): his `Attack6_BreakFloor` montage sends
//! `TGF.GameplayEvent.ShadowKing.FloorCollapse` at 0 s; `BP_NpcState_ShadowKing` runs `BP_NpcBehavior_FloorCollapse`: the
//! warning effects on for 5 s, then the floor's `BP_Mechanism_FloorCollapse`s get stages 1, 2, 3 (0, 0.3, 0.8 s later). A
//! mechanism whose stage it is breaks its rocks and, 1 s on, drops its floor collision, raises its inner wall (characters
//! only) and drops its other volumes. The map's collision cache tags those pieces ([`CollapsePiece`]). `BP_Trap_Fall`
//! catches the fallen 100 m down: a floor, and on it a box that kills every character but a boss.

use std::sync::Arc;

use avian3d::prelude::ColliderDisabled;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::command::LevelBox;
use crate::melee::Health;
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimIndex, TickCount};

/// The gameplay event that starts it.
pub const FLOOR_COLLAPSE: &str = "TGF.GameplayEvent.ShadowKing.FloorCollapse";

/// A floor's collapse, from its run layout (floor-local cm).
#[derive(Debug, Clone, PartialEq)]
pub struct CollapseSpec {
    /// The warning's length (s).
    pub warn: f32,
    /// (stage, seconds after the warning ends).
    pub stages: Vec<(u8, f32)>,
    /// From a stage to its collision changing (s).
    pub settle: f32,
    /// The warning effects' spots.
    pub warnings: Vec<Vec3>,
    /// `BP_Trap_Fall`: its bottom floor and its kill box (centre, half extents).
    pub fall: Option<((Vec3, Vec3), (Vec3, Vec3))>,
    /// What the client shows (not used by the sim).
    pub view: CollapseView,
}

/// The collapse's look (floor-local cm): per stage its floor mesh, where its sound plays and the sound; the
/// warning's decals (centre, half size across x and y), how long they stay and their emissive colour.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CollapseView {
    pub stages: Vec<(u8, String, Vec3, String)>,
    pub decals: Vec<(Vec3, bevy_math::Vec2)>,
    pub decal_secs: f32,
    pub decal_color: [f32; 3],
}

impl CollapseSpec {
    pub fn from_json(v: &serde_json::Value) -> Option<CollapseSpec> {
        let v3 = |v: &serde_json::Value| Some(Vec3::new(v[0].as_f64()? as f32, v[1].as_f64()? as f32, v[2].as_f64()? as f32));
        let bx = |v: &serde_json::Value| Some((v3(&v[0])?, v3(&v[1])?));
        Some(CollapseSpec {
            warn: v["warn_secs"].as_f64()? as f32,
            stages: v["stages"].as_array()?.iter().filter_map(|s| Some((s[0].as_u64()? as u8, s[1].as_f64()? as f32))).collect(),
            settle: v["move_secs"].as_f64()? as f32,
            warnings: v["warnings"].as_array().into_iter().flatten().filter_map(v3).collect(),
            fall: (|| Some((bx(&v["fall"]["floor"])?, bx(&v["fall"]["kill"])?)))(),
            view: CollapseView {
                stages: v["view"]["stages"].as_array().into_iter().flatten().filter_map(|s| Some((s[0].as_u64()? as u8, s[1].as_str()?.to_string(), v3(&s[2])?, s[3].as_str().unwrap_or_default().to_string()))).collect(),
                decals: v["view"]["decals"].as_array().into_iter().flatten().filter_map(|d| Some((v3(d)?, bevy_math::Vec2::new(d[3].as_f64()? as f32, d[4].as_f64()? as f32)))).collect(),
                decal_secs: v["view"]["decal_secs"].as_f64().unwrap_or(0.0) as f32,
                decal_color: [0, 1, 2].map(|k| v["view"]["decal_color"][k].as_f64().unwrap_or(1.0) as f32),
            },
        })
    }
}

/// A map collision instance the collapse changes (the cache's kind bits 8-15: the stage it drops at; 16-23: the stage it
/// starts colliding at, disabled until then). `off`: its floor's offset.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CollapsePiece {
    pub drop: u8,
    pub appear: u8,
    pub off: Vec3,
}

/// A floor's collapse in the world.
#[derive(Component, Debug, Clone)]
pub struct FloorCollapse {
    pub floor: u32,
    pub spec: Arc<CollapseSpec>,
    pub off: Vec3,
    /// The tick it was set off.
    pub started: Option<u64>,
}

/// `BP_Trap_Fall`'s kill box (world cm): every character in it but a boss dies.
#[derive(Component, Debug, Clone, Copy)]
pub struct FallTrap {
    pub center: Vec3,
    pub half: Vec3,
}

/// `SimCommand::SpawnFloorCollapse`: the floor's collapse and its fall trap.
pub(crate) fn spawn(world: &mut World, floor: u32, spec: Arc<CollapseSpec>, off: Vec3) {
    if let Some(((fc, fh), (kc, kh))) = spec.fall {
        world.spawn((crate::movement::static_box(fc + off, fh), LevelBox { half_extents: fh }));
        world.spawn(FallTrap { center: kc + off, half: kh });
    }
    world.spawn(FloorCollapse { floor, spec, off, started: None });
}

/// A monster's montage sent `tag` (`AN_Tgf_GamePlayEvent`). The King's `FloorCollapse` sets off the collapse of the
/// floor he stands over (its fall trap's square; else the nearest), once.
pub(crate) fn gameplay_event(world: &mut World, _id: SimId, e: Entity, tag: &str) {
    if tag != FLOOR_COLLAPSE {
        return;
    }
    let Some(p) = world.get::<Position>(e).map(|p| p.0) else { return };
    let now = world.resource::<TickCount>().0;
    let mut q = world.query::<(Entity, &FloorCollapse)>();
    let over = |c: &FloorCollapse| c.spec.fall.map(|(_, (kc, kh))| ((p - c.off - kc).truncate().abs() - kh.truncate()).max_element() <= 0.0);
    let pick = q.iter(world).filter(|(_, c)| c.started.is_none()).min_by(|a, b| (!over(a.1).unwrap_or(false), (a.1.off - p).length()).partial_cmp(&(!over(b.1).unwrap_or(false), (b.1.off - p).length())).unwrap_or(std::cmp::Ordering::Equal)).map(|(ce, c)| (ce, c.floor));
    let Some((ce, _)) = pick else { return };
    start(world, ce, now);
}

/// Sets off collapse `ce` now (once).
fn start(world: &mut World, ce: Entity, now: u64) {
    let Some(mut c) = world.get_mut::<FloorCollapse>(ce).filter(|c| c.started.is_none()) else { return };
    c.started = Some(now);
    let floor = c.floor;
    world.resource_mut::<SimEvents>().push(SimEvent::FloorCollapseWarning { floor, on: true });
}

/// `SimCommand::TriggerFloorCollapse` (dev): floor `floor`'s collapse, as the King's event would.
pub(crate) fn trigger(world: &mut World, floor: u32) {
    let now = world.resource::<TickCount>().0;
    let found: Vec<Entity> = world.query::<(Entity, &FloorCollapse)>().iter(world).filter(|(_, c)| c.floor == floor).map(|(e, _)| e).collect();
    for ce in found {
        start(world, ce, now);
    }
}

pub(crate) fn floor_collapse_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let ticks = |s: f32| (s / DT).round() as u64;
    let live: Vec<FloorCollapse> = world.query::<&FloorCollapse>().iter(world).filter(|c| c.started.is_some()).cloned().collect();
    for c in live {
        let t = now - c.started.expect("started");
        let warn = ticks(c.spec.warn);
        if t == warn {
            world.resource_mut::<SimEvents>().push(SimEvent::FloorCollapseWarning { floor: c.floor, on: false });
        }
        for &(stage, at) in &c.spec.stages {
            if t == warn + ticks(at) {
                world.resource_mut::<SimEvents>().push(SimEvent::FloorCollapsed { floor: c.floor, stage });
            }
            if t == warn + ticks(at) + ticks(c.spec.settle) {
                let pieces: Vec<(Entity, CollapsePiece)> = world.query::<(Entity, &CollapsePiece)>().iter(world).filter(|(_, p)| p.off == c.off).map(|(e, p)| (e, *p)).collect();
                for (pe, piece) in pieces {
                    if piece.drop == stage {
                        world.entity_mut(pe).insert(ColliderDisabled);
                    } else if piece.appear == stage {
                        world.entity_mut(pe).remove::<ColliderDisabled>();
                    }
                }
                // The level changed under them: no one stays at rest unswept (`CharacterState::settled`).
                for mut st in world.query::<&mut crate::movement::CharacterState>().iter_mut(world) {
                    st.settled = false;
                }
            }
        }
    }
    fall_traps(world);
}

/// `BP_Trap_Fall`'s `DamageTrigger`: a living character overlapping it dies (`Kill Self`), but not a boss.
fn fall_traps(world: &mut World) {
    let traps: Vec<FallTrap> = world.query::<&FallTrap>().iter(world).copied().collect();
    if traps.is_empty() {
        return;
    }
    let reach = Vec3::new(crate::movement::CAPSULE_RADIUS, crate::movement::CAPSULE_RADIUS, crate::movement::CAPSULE_HEIGHT * 0.5);
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for (id, e) in ids {
        let (Some(p), Some(h)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Health>(e)) else { continue };
        if h.current <= 0.0 || world.get::<crate::monsters::MonsterKit>(e).is_some_and(|k| crate::monsters::is_boss(&k.name)) {
            continue;
        }
        if traps.iter().any(|t| ((p - t.center).abs() - t.half - reach).max_element() <= 0.0) {
            world.get_mut::<Health>(e).expect("health").current = 0.0;
            world.resource_mut::<SimEvents>().push(SimEvent::Died { id });
        }
    }
}

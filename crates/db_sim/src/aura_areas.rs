//! Aura areas monsters lay: the Slow totem's `GA_TGF_M_Totem_ReduceSpeed`.
//!
//! `GA_TGF_SpawnOverlapArea_Monster_Base` (CONFIRMED bytecode): at its montage's `SpawnOverlapArea` notify it puts its
//! `SpawnActor` on the ground (`FindProperGroundPos`, 500 cm) under its current target, else under itself. The Slow
//! totem's is the base's `BP_TGF_Monster_OverlapArea_Base`: it lives `MaxExistTime` (20.5 s) and carries the aura
//! `Aura_Totom_ReduceSpeed`, which applies `GEsAppliedToTargets` (`GE_TGF_ReduceMoveSpeedArea`: move speed x 0.5 for
//! 2 s) to the characters in its capsule, every one (`bHostileOnly` false) and the caster's side too. Its radius grows
//! and shrinks with `RadiusCurve` (`IceAreaScale`: 0 to 700 cm over 3 s, 700 to 12.9 s, 500 at 18.1 s, 50 from
//! 20.1 s); the area's own tick scales its decal by the same curve. HYPOTHESIS (the aura is native): the capsule's
//! radius is the curve's value, its height `CapsuleHalfHeight` either way of the area's point, and the effect is
//! applied again every tick while one stays inside.

use bevy_ecs::prelude::*;
use bevy_math::{Dir3, Vec3};

use crate::id::SimIndex;
use crate::melee::Health;
use crate::monsters::{asset_json, class_default, ge_slow};
use crate::{DT, Position, SimId, TickCount};

/// `FindProperGroundPos`'s depth in the base's bytecode, cm.
const GROUND_DEPTH: f32 = 500.0;

/// An aura area's values.
#[derive(Debug, Clone, PartialEq)]
pub struct AuraSpec {
    /// `RadiusCurve` keys (s since it appeared, cm), linear between, flat outside.
    pub radius: Vec<(f32, f32)>,
    pub half_height: f32,
    /// `MaxExistTime`, s.
    pub life: f32,
    /// Its effect's slow: (move speed reduction, s).
    pub slow: (f64, f32),
    pub hostile_only: bool,
}

impl AuraSpec {
    /// From the area class `package` (`/Game/...` path without the object): its first aura's values.
    pub fn load(package: &str) -> Option<AuraSpec> {
        let auras = class_default(package, "GameplayAuras")?;
        let aura = auras.as_array()?.first()?["ObjectPath"].as_str()?.split('.').next()?.to_string();
        let curve = class_default(&aura, "RadiusCurve")?["ObjectPath"].as_str()?.split('.').next()?.to_string();
        let keys = asset_json(&curve)?.as_array()?.iter().find(|o| o["Type"] == "CurveFloat")?["Properties"]["FloatCurve"]["Keys"].as_array()?.iter().filter_map(|k| Some((k["Time"].as_f64()? as f32, k["Value"].as_f64()? as f32))).collect();
        let ge = class_default(&aura, "GEsAppliedToTargets")?.as_array()?.first()?["ObjectPath"].as_str()?.to_string();
        Some(AuraSpec {
            radius: keys,
            half_height: class_default(&aura, "CapsuleHalfHeight")?.as_f64()? as f32,
            life: class_default(package, "MaxExistTime")?.as_f64()? as f32,
            slow: ge_slow(&ge)?,
            hostile_only: class_default(&aura, "bHostileOnly").and_then(|b| b.as_bool()).unwrap_or(false),
        })
    }

    /// Its radius `t` s after it appeared.
    pub fn radius_at(&self, t: f32) -> f32 {
        let k = &self.radius;
        match k.iter().position(|&(kt, _)| kt > t) {
            None => k.last().map_or(0.0, |p| p.1),
            Some(0) => k[0].1,
            Some(i) => {
                let ((t0, v0), (t1, v1)) = (k[i - 1], k[i]);
                v0 + (v1 - v0) * (t - t0) / (t1 - t0)
            }
        }
    }
}

/// An aura area in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct AuraArea {
    pub owner: SimId,
    pub at: Vec3,
    pub spec: AuraSpec,
    pub born: u64,
}

#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct AuraAreas(pub Vec<AuraArea>);

/// Monster `id` lays its area at its notify: on the ground under its target, else under itself.
pub(crate) fn spawn(world: &mut World, id: SimId, e: Entity, spec: AuraSpec) {
    let Some(own) = world.get::<Position>(e).map(|p| p.0) else { return };
    let target = world.get::<crate::ai::MonsterAi>(e).and_then(|a| a.target).and_then(|t| world.resource::<SimIndex>().get(t)).and_then(|te| world.get::<Position>(te)).map(|p| p.0);
    let center = target.unwrap_or(own);
    let ground = {
        let mut state: bevy_ecs::system::SystemState<(avian3d::prelude::SpatialQuery, Query<(), With<crate::command::LevelBox>>)> = bevy_ecs::system::SystemState::new(world);
        let (spatial, boxes) = state.get(world).expect("spatial query pipeline");
        spatial.cast_ray_predicate(center, Dir3::NEG_Z, GROUND_DEPTH, true, &crate::layers::walk(), &|b| boxes.contains(b)).map_or(center.z - crate::movement::CAPSULE_HEIGHT * 0.5, |h| center.z - h.distance)
    };
    let now = world.resource::<TickCount>().0;
    world.resource_mut::<AuraAreas>().0.push(AuraArea { owner: id, at: Vec3::new(center.x, center.y, ground), spec, born: now });
}

/// Every area slows the living characters inside it this tick; it goes at the end of its life.
pub(crate) fn tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let mut areas = std::mem::take(&mut world.resource_mut::<AuraAreas>().0);
    areas.retain(|a| ((now - a.born) as f32 * DT) < a.spec.life);
    if !areas.is_empty() {
        let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
        for a in &areas {
            let r = a.spec.radius_at((now - a.born) as f32 * DT);
            for &(tid, te) in &ids {
                if world.get::<Health>(te).is_none_or(|h| h.current <= 0.0) || (a.spec.hostile_only && crate::staffs::friendly(world, a.owner, tid, te)) {
                    continue;
                }
                let Some(p) = world.get::<Position>(te).map(|p| p.0) else { continue };
                if (p - a.at).truncate().length() <= r + crate::movement::CAPSULE_RADIUS && (p.z - a.at.z).abs() <= a.spec.half_height + crate::movement::CAPSULE_HEIGHT * 0.5 {
                    let buff = crate::buffs::Buff { kind: crate::buffs::BuffKind::Slow(a.spec.slow.0), skill: crate::skills::SkillKind::Charge, ticks_left: (a.spec.slow.1 / DT).round() as u32 + 1, age: 0 };
                    crate::buffs::add_buff(world, te, buff);
                }
            }
        }
    }
    world.resource_mut::<AuraAreas>().0 = areas;
}

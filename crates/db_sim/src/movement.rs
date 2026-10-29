//! Physics and first-person character movement.
//!
//! The sim keeps the source data's conventions: units are UE centimetres, Z is up, speeds are cm/s
//! (`speed_move_speed_base` from the class sheet). Collision uses avian3d; characters are kinematic
//! capsules moved with avian's `move_and_slide` against static level geometry, once per sim tick.
//! Values without a data source (sprint, crouch, jump, gravity, stamina) live in `overrides`.

use std::time::Duration;

use avian3d::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::{Dir3, Vec2, Vec3};

use crate::overrides::movement as tune;
use crate::{DT, SimIndex};

/// Movement speed in cm/s (`speed_move_speed_base`).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct MoveSpeed(pub f32);

/// What the actor wants to do this tick (set by commands or AI).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct MoveInput {
    /// Desired planar direction in world space (x, y); length is clamped to 1.
    pub dir: Vec2,
    pub sprint: bool,
    pub crouch: bool,
    /// Consumed on the tick it is applied.
    pub jump: bool,
    /// A flyer's wanted height (capsule centre z, cm): no gravity, it climbs or sinks towards it at up to its
    /// move speed. `None` walks.
    pub fly: Option<f32>,
    /// A base move speed in place of its own (cm/s: a monster retreating or flanking at its Blueprint's
    /// `Retreats.Speed` / `FlankSpeed`, `ApplySpeedMod`).
    pub speed: Option<f32>,
}

/// Movement state the client reads.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CharacterState {
    pub velocity: Vec3,
    pub on_ground: bool,
    pub crouched: bool,
    pub sprinting: bool,
    pub stamina: f32,
    /// Set when stamina hits 0; cleared above `SPRINT_RESUME_STAMINA`.
    pub exhausted: bool,
    /// The last sweep without steering left it on the ground and moved it under 0.05 cm: at rest, so
    /// later ticks skip the sweep until something moves it.
    pub settled: bool,
    /// Jumps it may still make in the air (the Wizard's full Pyroblast charge grants one, `JumpMaxCount` 2).
    pub air_jumps: u8,
}

impl Default for CharacterState {
    fn default() -> Self {
        Self { velocity: Vec3::ZERO, on_ground: false, crouched: false, sprinting: false, stamina: tune::STAMINA_MAX, exhausted: false, settled: false, air_jumps: 0 }
    }
}

/// Capsule size of a character (cm): radius and total standing height.
pub const CAPSULE_RADIUS: f32 = 35.0;
pub const CAPSULE_HEIGHT: f32 = 180.0;

/// Collider for a character whose `Position` is the capsule's centre.
pub fn character_collider() -> Collider {
    let half = CAPSULE_HEIGHT * 0.5 - CAPSULE_RADIUS;
    Collider::capsule_endpoints(CAPSULE_RADIUS, Vec3::new(0.0, 0.0, -half), Vec3::new(0.0, 0.0, half))
}

/// Components for a static axis-aligned box of level geometry (centre, half extents, cm).
pub fn static_box(center: Vec3, half_extents: Vec3) -> impl Bundle {
    (RigidBody::Static, Collider::cuboid(half_extents.x * 2.0, half_extents.y * 2.0, half_extents.z * 2.0), Position(center))
}

/// Planar speed (cm/s) for the current input and state; 0 without a direction.
pub fn target_speed(base: f32, input: &MoveInput, state: &CharacterState) -> f32 {
    if input.dir.length_squared() <= 1e-6 {
        return 0.0;
    }
    let factor = if state.crouched {
        tune::CROUCH_SPEED_FACTOR
    } else if state.sprinting {
        tune::SPRINT_SPEED_FACTOR
    } else {
        1.0
    };
    (base * factor * input.dir.length().min(1.0)).max(0.0)
}

type Actors<'w, 's> = Query<
    'w,
    's,
    (
        &'static MoveSpeed,
        &'static mut MoveInput,
        &'static mut CharacterState,
        &'static mut Position,
        &'static Collider,
        Option<&'static crate::melee::Melee>,
        Option<&'static crate::skills::SkillKit>,
        Option<&'static crate::melee::Health>,
        Option<&'static crate::buffs::Buffs>,
        Option<&'static crate::staffs::StaffState>,
        Option<&'static crate::crossbow::CrossbowState>,
        Option<&'static crate::gear_triggers::GearSpeed>,
    ),
>;

/// A dead character's capsule blocks no one: it leaves the spatial queries' default membership while dead and is back on it
/// alive (the player's respawn). HYPOTHESIS: the monsters' death ability isn't extracted; the game is built on Lyra, whose
/// character turns its capsule's collision off as its death starts (`DisableMovementAndCollision`). Before, corpses in a
/// narrow way pinned whoever came after for good (the castle auto-run). (`CollisionLayers` is immutable: it is replaced,
/// which also tells the physics' spatial index.)
pub(crate) fn corpse_collision(mut commands: Commands, actors: Query<(Entity, &crate::melee::Health, &CollisionLayers, &RigidBody)>) {
    for (e, health, layers, body) in &actors {
        if *body != RigidBody::Kinematic {
            continue;
        }
        let want = if health.current > 0.0 { LayerMask::DEFAULT } else { LayerMask::NONE };
        if layers.memberships != want {
            commands.entity(e).insert(CollisionLayers::new(want, layers.filters));
        }
    }
}

/// One tick of character movement for every actor, in id order (deterministic). Each actor sees the
/// positions already written by actors with smaller ids.
pub(crate) fn move_characters(index: Res<SimIndex>, mut set: ParamSet<(Actors, MoveAndSlide)>) {
    let config = MoveAndSlideConfig::default();
    for (_, e) in index.iter() {
        let mut charge;
        let (speed, mut input, mut st, pos0, collider, base_speed) = {
            let mut actors = set.p0();
            let Ok((speed, input, st, pos, collider, melee, kit, health, buffs, staff, crossbow, gear)) = actors.get_mut(e) else { continue };
            // The dead stop steering (gravity still applies).
            let alive = health.is_none_or(|h| h.current > 0.0);
            let stunned = buffs.is_some_and(crate::buffs::Buffs::stunned);
            let combat_factor = if alive && !stunned { melee.map_or(1.0, crate::melee::Melee::move_factor) * buffs.map_or(1.0, crate::buffs::Buffs::speed_factor) * staff.map_or(1.0, crate::staffs::StaffState::move_factor) * crossbow.map_or(1.0, crate::crossbow::CrossbowState::move_factor) } else { 0.0 };
            let bonus = kit.map_or(0.0, crate::skills::SkillKit::speed_bonus) + buffs.map_or(0.0, crate::buffs::Buffs::speed_bonus) + gear.map_or(0.0, |g| g.0);
            charge = if stunned { None } else { buffs.and_then(crate::buffs::Buffs::pull).or_else(|| kit.and_then(crate::skills::SkillKit::charge_velocity)) };
            // A monster swing's root motion moves it like a charge.
            if alive && !stunned && charge.is_none() {
                charge = melee.and_then(crate::melee::Melee::root_velocity).filter(|v| v.length_squared() > 1e-6);
            }
            (MoveSpeed((input.speed.unwrap_or(speed.0) + bonus) * combat_factor), *input, *st, pos.0, collider.clone(), speed.0)
        };
        let moving = input.dir.length_squared() > 1e-6;
        st.crouched = input.crouch;
        if st.exhausted && st.stamina >= tune::SPRINT_RESUME_STAMINA {
            st.exhausted = false;
        }
        // Attacking or blocking (speed factor below 1) also stops sprinting.
        st.sprinting = input.sprint && moving && !st.crouched && st.on_ground && !st.exhausted && st.stamina > 0.0 && speed.0 >= base_speed;
        // Stamina: drains while sprinting, regenerates otherwise (HYPOTHESIS values).
        st.stamina = if st.sprinting {
            let s = (st.stamina - tune::SPRINT_STAMINA_PER_S * DT).max(0.0);
            st.exhausted |= s == 0.0;
            s
        } else {
            (st.stamina + tune::STAMINA_REGEN_PER_S * DT).min(tune::STAMINA_MAX)
        };
        let dir = input.dir.clamp_length_max(1.0);
        let planar = match charge {
            // Charge overrides steering for its duration.
            Some(v) => v,
            None if moving => dir.normalize() * target_speed(speed.0, &input, &st),
            None => Vec2::ZERO,
        };
        // Flying: one sweep with the climb or sink, no gravity, step-up or ground probe.
        if let Some(fz) = input.fly {
            let climb = speed.0.max(tune::FLY_MIN_CLIMB);
            let velocity = Vec3::new(planar.x, planar.y, ((fz - pos0.z) / DT).clamp(-climb, climb));
            let filter = crate::layers::walk().with_excluded_entities([e]);
            let out = set.p1().move_and_slide(&collider, pos0, Rotation::default().0, velocity, Duration::from_secs_f32(DT), &config, &filter, |_| MoveAndSlideHitResponse::Accept);
            st.on_ground = false;
            st.settled = false;
            st.velocity = out.projected_velocity;
            input.jump = false;
            if let Ok((_, mut i, mut s, mut p, _, _, _, _, _, _, _, _)) = set.p0().get_mut(e) {
                *i = input;
                *s = st;
                p.0 = out.position;
            }
            continue;
        }
        // At rest (settled on the ground, no steering, charge, pull or jump, under 1 cm/s left to slide off):
        // nothing can move it against static level geometry, so the sweep is skipped and its position isn't
        // touched (it was most of the tick with a crypt full of idle monsters).
        if st.settled && st.on_ground && planar == Vec2::ZERO && !input.jump && st.velocity.length_squared() < 1.0 {
            input.jump = false;
            if let Ok((_, mut i, mut s, _, _, _, _, _, _, _, _, _)) = set.p0().get_mut(e) {
                *i = input;
                *s = st;
            }
            continue;
        }
        let mut vz = st.velocity.z;
        let air = !st.on_ground && st.air_jumps > 0;
        let jumped = (st.on_ground || air) && input.jump && !st.crouched && st.stamina >= tune::JUMP_STAMINA;
        if jumped {
            vz = tune::JUMP_Z_VELOCITY;
            st.stamina -= tune::JUMP_STAMINA;
            if air {
                st.air_jumps -= 1;
            }
        }
        input.jump = false;
        // On the ground gravity doesn't build up: the move is planar and the ground probe below keeps it
        // down (with gravity's 1 cm a tick the sweep often stopped short of the floor, so the ground flag
        // flickered and a jump pressed then did nothing).
        if !st.on_ground || jumped {
            vz -= tune::GRAVITY * DT;
        }
        let velocity = Vec3::new(planar.x, planar.y, vz);
        let filter = crate::layers::walk().with_excluded_entities([e]);
        let mut ground = false;
        let out = set.p1().move_and_slide(
            &collider,
            pos0,
            Rotation::default().0,
            velocity,
            Duration::from_secs_f32(DT),
            &config,
            &filter,
            |hit| {
                if hit.normal.z > tune::WALKABLE_NORMAL_Z {
                    ground = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );
        let mut end = out.position;
        let mut projected = out.projected_velocity;
        // Step up: walking on the ground into something low (a stair, a kerb), try up by the step height
        // and across, then straight down (no sliding, so the capsule may rest on the edge and ride over
        // it next tick); keep it if that got farther onto something no higher than a step.
        let wanted = planar * DT;
        let got = (out.position - pos0).truncate();
        if st.on_ground && wanted.length() > 1.0 && got.dot(wanted.normalize()) < wanted.length() * 0.5 {
            let mq = set.p1();
            let slide = |v: Vec3, from: Vec3| mq.move_and_slide(&collider, from, Rotation::default().0, v / DT, Duration::from_secs_f32(DT), &config, &filter, |_| MoveAndSlideHitResponse::Accept).position;
            let up = slide(Vec3::Z * tune::MAX_STEP_HEIGHT, pos0);
            let across = slide(wanted.extend(0.0), up);
            let drop = up.z - pos0.z + 5.0;
            let hit = mq.spatial_query.cast_shape(&collider, across, Rotation::default().0, Dir3::NEG_Z, &ShapeCastConfig::from_max_distance(drop), &filter);
            // Something under it that faces up (a tread or its edge, no higher than a step above the feet),
            // not the face it is pressed against.
            let feet = collider.aabb(pos0, Rotation::default()).min.z;
            if let Some(hit) = hit.filter(|h| h.distance > 0.0 && h.normal1.z > 0.1 && h.point1.z <= feet + tune::MAX_STEP_HEIGHT) {
                let down = across - Vec3::Z * (hit.distance - 0.5).max(0.0);
                if (down - pos0).truncate().dot(wanted.normalize()) > got.dot(wanted.normalize()) + 1.0 && down.z <= pos0.z + tune::MAX_STEP_HEIGHT + 1.0 {
                    end = down;
                    projected = planar.extend(0.0);
                    ground = true;
                }
            }
        }
        // Ground probe (as UE's walking floor check): a walking character that didn't jump follows the floor
        // down by up to a step (down stairs and slopes, instead of running off into the air for a few
        // ticks); in the air only touching counts. The hit must face up enough to walk on.
        let was_walking = st.on_ground && !jumped && vz <= 0.0;
        let reach = if was_walking { tune::MAX_STEP_HEIGHT } else { tune::GROUND_PROBE };
        let mq = set.p1();
        let probe = mq.spatial_query.cast_shape(&collider, end, Rotation::default().0, Dir3::NEG_Z, &ShapeCastConfig::from_max_distance(reach), &filter);
        // On a stair's edge the swept capsule touches a slanted face: then a ray straight down from the capsule's
        // centre decides, as UE's floor check does (walking down the crypt's stairs fell step by step).
        let half = CAPSULE_HEIGHT * 0.5;
        let walkable = |h: &ShapeHitData| h.normal1.z > tune::WALKABLE_NORMAL_Z || mq.spatial_query.cast_ray(end, Dir3::NEG_Z, half + reach + tune::GROUND_GAP, true, &filter).is_some_and(|r| r.normal.z > tune::WALKABLE_NORMAL_Z);
        if let Some(hit) = probe.filter(walkable) {
            ground = true;
            if was_walking {
                end.z -= (hit.distance - tune::GROUND_GAP).max(0.0);
            }
        } else if !jumped && vz <= 0.0 && probe.is_none() {
            // Nothing below within reach: walked off a ledge.
            ground = false;
        }
        st.on_ground = ground;
        st.settled = ground && planar == Vec2::ZERO && (end - pos0).length_squared() < 0.05 * 0.05;
        // A walker that didn't jump keeps no upward speed from sliding over a slope or an edge (as UE keeps
        // walking velocity horizontal): running down stairs, a riser's edge turned it into hops of up to
        // half a metre.
        st.velocity = if ground { Vec3::new(projected.x, projected.y, 0.0) } else if was_walking { Vec3::new(projected.x, projected.y, projected.z.min(0.0)) } else { projected };
        let mut actors = set.p0();
        if let Ok((_, mut i, mut s, mut p, _, _, _, _, _, _, _, _)) = actors.get_mut(e) {
            *i = input;
            *s = st;
            p.0 = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_factors() {
        let input = MoveInput { dir: Vec2::X, ..Default::default() };
        let mut st = CharacterState::default();
        assert_eq!(target_speed(330.0, &input, &st), 330.0);
        st.sprinting = true;
        assert_eq!(target_speed(330.0, &input, &st), 330.0 * tune::SPRINT_SPEED_FACTOR);
        st.crouched = true;
        assert_eq!(target_speed(330.0, &input, &st), 330.0 * tune::CROUCH_SPEED_FACTOR);
        assert_eq!(target_speed(330.0, &MoveInput::default(), &st), 0.0);
    }
}

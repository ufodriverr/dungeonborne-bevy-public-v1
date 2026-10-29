//! The Shadow King's throne. CONFIRMED from the bytecode: the throne (`BP_Throne_ShadowKing`, a `BP_Mechanism` turned by
//! `TGF.GameplayEvent.ShadowKing.TriggerThrone`) is turned off when the King's state relays his `Death.Finish`
//! (`EventMagnitude` 0); its `Reset` destroys its `BlockVolumes` at once. Those are in the map's collision cache with kind
//! bits 24-31 = 1 ([`ThronePiece`]): three walls west of his point. Turning on (his waking, magnitude 1) only plays its
//! effects. HYPOTHESIS: `Death.Finish` comes as his health reaches 0 (the event's sender isn't read).

use avian3d::prelude::ColliderDisabled;
use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::melee::Health;
use crate::monsters::MonsterKit;
use crate::{Position, SimEvent, SimEvents, SimId};

/// A map collision instance the throne removes when its floor's King dies; `off`: its floor's offset.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ThronePiece {
    pub off: Vec3,
}

/// On a Shadow King whose death has taken his floor's throne pieces.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ThroneDown;

pub(crate) fn throne_tick(world: &mut World) {
    let fallen: Vec<(Entity, SimId, Vec3)> = world
        .query_filtered::<(Entity, &SimId, &MonsterKit, &Health, &Position), Without<ThroneDown>>()
        .iter(world)
        .filter(|(_, _, k, h, _)| k.king.is_some() && h.current <= 0.0)
        .map(|(e, id, _, _, p)| (e, *id, p.0))
        .collect();
    for (e, id, at) in fallen {
        world.entity_mut(e).insert(ThroneDown);
        let pieces: Vec<(Entity, Vec3)> = world.query::<(Entity, &ThronePiece)>().iter(world).map(|(pe, p)| (pe, p.off)).collect();
        // His floor's: the pieces of the offset nearest him.
        let Some(off) = pieces.iter().map(|p| p.1).min_by(|a, b| (*a - at).length_squared().total_cmp(&(*b - at).length_squared())) else { continue };
        for (pe, _) in pieces.into_iter().filter(|p| p.1 == off) {
            world.entity_mut(pe).insert(ColliderDisabled);
        }
        // The level changed round them: no one stays at rest unswept (`CharacterState::settled`).
        for mut st in world.query::<&mut crate::movement::CharacterState>().iter_mut(world) {
            st.settled = false;
        }
        world.resource_mut::<SimEvents>().push(SimEvent::ThroneReset { king: id });
    }
}

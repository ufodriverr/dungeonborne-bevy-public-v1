//! Monster abilities that apply a gameplay effect instead of hitting: the Goblin Shaman's speed buff on an
//! ally (Attack2) and its `AttackBuff` on every goblin near it (Attack3), the Werewolf's `Change`; the
//! Khazra's Attack6_2 slow on every hostile near it. The effect's `MoveSpeedBase` add becomes a
//! [`BuffKind::MoveSpeed`], a `MoveSpeedRate` multiplier a [`BuffKind::Slow`] and its owned tags
//! [`BuffKind::Tag`]s, for the effect's duration. The tags switch the AI to an alternative selector.

use bevy_ecs::prelude::*;

use crate::buffs::{Buff, BuffKind, Buffs, add_buff};
use crate::command::{ActorKind, Kind};
use crate::melee::Health;
use crate::monsters::{EffectTo, MonsterKit};
use crate::skills::SkillKind;
use crate::{DT, Position, SimId};

/// Applies attack `index`'s effect of the monster `e` (at its `fire` notify).
pub(crate) fn apply(world: &mut World, e: Entity, index: u8) {
    let Some(effect) = world.get::<MonsterKit>(e).and_then(|k| k.attacks.get(usize::from(index))).and_then(|a| a.effect.clone()) else { return };
    let Some(from) = world.get::<Position>(e).map(|p| p.0) else { return };
    // Living monsters other than the caster, in id order: (entity, distance, tags).
    let mut others: Vec<(SimId, Entity, f32, Vec<u32>)> = world
        .query::<(Entity, &SimId, &Kind, &Position, &Health, Option<&MonsterKit>, Option<&Buffs>)>()
        .iter(world)
        .filter(|q| q.0 != e && q.2.0 == ActorKind::Monster && q.4.current > 0.0)
        .map(|(oe, id, _, pos, _, kit, buffs)| (*id, oe, pos.0.distance(from), kit.map(|k| k.tags.clone()).unwrap_or_default().into_iter().chain(buffs.into_iter().flat_map(Buffs::tags)).collect()))
        .collect();
    others.sort_by_key(|o| o.0);
    let targets: Vec<Entity> = match &effect.to {
        EffectTo::Own => vec![e],
        EffectTo::Ally(range) => others.iter().filter(|o| o.2 <= *range).min_by(|a, b| a.2.total_cmp(&b.2)).map(|o| o.1).into_iter().collect(),
        EffectTo::Area { radius, required, blocked, hostile: false } => others.iter().filter(|o| o.2 <= *radius && required.iter().all(|t| o.3.contains(t)) && !blocked.iter().any(|t| o.3.contains(t))).map(|o| o.1).collect(),
        // `GatherNearbyActors` then `CompareTeams` == different: the living characters that aren't
        // monsters, in id order.
        EffectTo::Area { radius, hostile: true, .. } => {
            let mut near: Vec<(SimId, Entity)> = world
                .query::<(Entity, &SimId, &Kind, &Position, &Health)>()
                .iter(world)
                .filter(|q| q.2.0 != ActorKind::Monster && q.4.current > 0.0 && q.3.0.distance(from) <= *radius)
                .map(|q| (*q.1, q.0))
                .collect();
            near.sort_by_key(|n| n.0);
            near.into_iter().map(|n| n.1).collect()
        }
    };
    let ticks_left = (effect.seconds / DT).round() as u32;
    for t in targets {
        let buff = |kind| Buff { kind, skill: SkillKind::MonsterAbility, ticks_left, age: 0 };
        if effect.speed != 0.0 {
            add_buff(world, t, buff(BuffKind::MoveSpeed(effect.speed)));
        }
        for &tag in &effect.tags {
            add_buff(world, t, buff(BuffKind::Tag(tag)));
        }
        if let Some(rate) = effect.slow {
            add_buff(world, t, buff(BuffKind::Slow(rate)));
        }
    }
}

//! Timed skill effects on actors: War Cry's stat buff, Inspire's move-speed bonus, Rend's bleed and grievous wounds; added Stealth, petrify,
//! slows, damage reduction, the Lich's Frost Curse, Ice Barrier and Ice Armor, and the root while channelling. Hits read them through
//! [`Buffs::attack`] / [`Buffs::defence`] and life steal, healing through [`heal`], movement through [`Buffs::speed_bonus`]. Potions keep their
//! own `world_items::Effects`. Ticked after skills, in id order.

use bevy_ecs::prelude::*;
use bevy_math::Vec3;

use crate::combat::{AttackStats, DefenceStats, Element};
use crate::id::SimIndex;
use crate::melee::Health;
use crate::skills::{HitSource, SkillKind};
use crate::{Position, SimEvent, SimEvents, SimId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuffKind {
    /// Attack boost rate, physical resistance and life steal (fractions), added to the actor's own.
    Battle { attack_boost: f64, physical_resist: f64, life_steal: f64 },
    /// Flat move-speed bonus, cm/s.
    MoveSpeed(f32),
    /// Rend: every `interval` ticks the actor takes `per_metre` damage per metre it moved (planar)
    /// since the last pulse; `by` dealt it.
    Rent { by: SimId, per_metre: f64, interval: u32, last: Vec3 },
    /// Healing received is multiplied by `1 - rate`.
    GrievousWounds(f64),
    /// Dragged at this planar velocity (cm/s) instead of walking (a pulling projectile).
    Pull(bevy_math::Vec2),
    /// Stealth (Rogue): monsters only notice the actor up close; crit damage rate + `crit_damage`.
    Stealth { crit_damage: f64 },
    /// Petrified (Rogue's poison): can't act, damage taken × `1 - damage_res`; a hit from close
    /// range ends it.
    Petrified { damage_res: f64 },
    /// Move speed × `1 - rate`.
    Slow(f64),
    /// Damage taken × `1 - rate`.
    DamageReduction(f64),
    /// Frost Curse (Lich): every `interval` ticks the actor takes `damage` (passive) and `by`
    /// heals `heal`.
    FrostCurse { by: SimId, damage: f64, heal: f64, interval: u32 },
    /// Ice Barrier (Lich): no damage taken, can't move, attack or use other skills; heals `heal`
    /// every second.
    IceBarrier { heal: f64 },
    /// Ice Armor (Lich): the first active hit from within `range` cm slows the attacker by
    /// `slow` for `seconds`, once.
    IceArmor { range: f32, slow: f64, seconds: f32 },
    /// Can't move (channelling Blizzard, Oracle).
    Rooted,
    /// Takes no damage (Priest's Divine Protection).
    Invincible,
    /// Heals this much every second (Druid's Rejuvenation).
    Regen(f64),
    /// Burning (`GE_TGF_BuffArg_Burning`): every `interval` ticks the actor takes `damage` (passive) from `by` and
    /// loses a stack; it ends with the last (HYPOTHESIS: the effect has no period, and its stack expiration policy
    /// is a custom value, read as one stack burning per interval).
    Burning { by: SimId, damage: f64, stacks: u32, interval: u32 },
    /// A fixed damage every `interval` ticks from `by`, from its application on (a flask's burn).
    Dot { by: SimId, damage: f64, interval: u32 },
    /// A gameplay tag the actor owns while this lasts ([`gameplay_tag`]: the Goblin Shaman's
    /// `AttackBuff`, the Werewolf's `Change`).
    Tag(u32),
    /// A relic's ice enchant (`GE_Buff_HitCauseSpeedDown` granting `GA_TGF_HitCauseSpeedDown`): each of the actor's
    /// direct melee hits slows the target by `slow` for `slow_secs` and adds ice damage at factor `ice`.
    IceEnchant { slow: f64, slow_secs: f32, ice: f64 },
    /// A legendary affix's timed effect (`affixes`): `key` tells the affixes' effects apart (they all come from
    /// [`SkillKind::Item`]); flat move speed (`MoveSpeedBase`, cm/s), a move speed rate (`MoveSpeedPercent`, summed and
    /// added to 1, HYPOTHESIS) and life steal (`LifeLeachRate`) per stack.
    Affix { key: u32, speed: f32, speed_rate: f64, life_steal: f64, stacks: u8 },
}

/// A gameplay tag's id (FNV-1a of its name). [`tag_with_parents`] gives a tag with its parents, so that
/// exact id matches behave like Unreal's tag matching (`A.B.C` matches a query for `A.B`).
pub fn gameplay_tag(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// The ids of a tag and of each of its parents (`A.B.C`: `A`, `A.B`, `A.B.C`).
pub fn tag_with_parents(name: &str) -> Vec<u32> {
    name.char_indices().filter(|(_, c)| *c == '.').map(|(i, _)| gameplay_tag(&name[..i])).chain([gameplay_tag(name)]).collect()
}

impl BuffKind {
    /// A stable number per kind and its main value (for the state hash).
    pub fn tag(&self) -> (u8, f64) {
        match *self {
            BuffKind::Battle { attack_boost, .. } => (0, attack_boost),
            BuffKind::MoveSpeed(v) => (1, f64::from(v)),
            BuffKind::Rent { per_metre, .. } => (2, per_metre),
            BuffKind::GrievousWounds(r) => (3, r),
            BuffKind::Pull(v) => (4, f64::from(v.x + v.y)),
            BuffKind::Stealth { crit_damage } => (5, crit_damage),
            BuffKind::Petrified { damage_res } => (6, damage_res),
            BuffKind::Slow(r) => (7, r),
            BuffKind::DamageReduction(r) => (8, r),
            BuffKind::FrostCurse { damage, .. } => (9, damage),
            BuffKind::IceBarrier { heal } => (10, heal),
            BuffKind::IceArmor { slow, .. } => (11, slow),
            BuffKind::Rooted => (12, 0.0),
            BuffKind::Invincible => (13, 0.0),
            BuffKind::Regen(v) => (14, v),
            BuffKind::Tag(t) => (15, f64::from(t)),
            BuffKind::Burning { damage, stacks, .. } => (16, damage * f64::from(stacks)),
            BuffKind::IceEnchant { ice, .. } => (17, ice),
            BuffKind::Affix { key, speed, speed_rate, life_steal, stacks } => (18, f64::from(key) + f64::from(speed) + speed_rate + life_steal * f64::from(stacks)),
            BuffKind::Dot { damage, .. } => (19, damage),
        }
    }

    /// Effects a cleanse removes (harmful ones).
    pub fn is_debuff(&self) -> bool {
        matches!(self, BuffKind::Rent { .. } | BuffKind::GrievousWounds(_) | BuffKind::Pull(_) | BuffKind::Petrified { .. } | BuffKind::Slow(_) | BuffKind::FrostCurse { .. } | BuffKind::Burning { .. } | BuffKind::Dot { .. })
            || matches!(self, BuffKind::Affix { speed_rate, .. } if *speed_rate < 0.0)
    }
}

impl BuffKind {
    fn same_kind(&self, other: &BuffKind) -> bool {
        match (self, other) {
            (BuffKind::Tag(a), BuffKind::Tag(b)) => a == b,
            (BuffKind::Affix { key: a, .. }, BuffKind::Affix { key: b, .. }) => a == b,
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buff {
    pub kind: BuffKind,
    /// The skill that applied it.
    pub skill: SkillKind,
    pub ticks_left: u32,
    /// Ticks since it was applied.
    pub age: u32,
}

/// An actor's running skill effects.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Buffs(pub Vec<Buff>);

impl Buffs {
    /// Adds a buff; the same kind from the same skill is refreshed, not stacked (HYPOTHESIS).
    pub fn add(&mut self, buff: Buff) {
        self.0.retain(|b| !(b.skill == buff.skill && b.kind.same_kind(&buff.kind)));
        self.0.push(buff);
    }

    pub fn has(&self, skill: SkillKind) -> bool {
        self.0.iter().any(|b| b.skill == skill)
    }

    /// Attack stats with every Battle buff's attack boost and Stealth's crit damage added.
    pub fn attack(&self, mut a: AttackStats) -> AttackStats {
        for b in &self.0 {
            match b.kind {
                BuffKind::Battle { attack_boost, .. } => a.attack_boost_rate += attack_boost,
                BuffKind::Stealth { crit_damage } => a.crit_damage_rate += crit_damage,
                _ => {}
            }
        }
        a
    }

    pub fn stealthed(&self) -> bool {
        self.0.iter().any(|b| matches!(b.kind, BuffKind::Stealth { .. }))
    }

    /// Can't move, attack or use skills (petrified, or inside an Ice Barrier).
    pub fn stunned(&self) -> bool {
        self.0.iter().any(|b| matches!(b.kind, BuffKind::Petrified { .. } | BuffKind::IceBarrier { .. }))
    }

    pub fn encased(&self) -> bool {
        self.0.iter().any(|b| matches!(b.kind, BuffKind::IceBarrier { .. }))
    }

    /// Can't move or attack (channelling).
    pub fn rooted(&self) -> bool {
        self.0.iter().any(|b| matches!(b.kind, BuffKind::Rooted))
    }

    /// Factor on move speed (the strongest slow counts, times 1 + the affixes' move speed rates; rooted = 0).
    pub fn speed_factor(&self) -> f32 {
        if self.rooted() {
            return 0.0;
        }
        let worst = self.0.iter().map(|b| if let BuffKind::Slow(r) = b.kind { r } else { 0.0 }).fold(0.0, f64::max);
        let rate: f64 = self.0.iter().map(|b| if let BuffKind::Affix { speed_rate, .. } = b.kind { speed_rate } else { 0.0 }).sum();
        ((1.0 - worst).max(0.0) * (1.0 + rate).max(0.0)) as f32
    }

    /// Defence stats with every Battle buff's physical resistance added.
    pub fn defence(&self, mut d: DefenceStats) -> DefenceStats {
        for b in &self.0 {
            if let BuffKind::Battle { physical_resist, .. } = b.kind {
                d.resistance.set(Element::Physical, d.resistance.get(Element::Physical) + physical_resist);
            }
        }
        d
    }

    pub fn life_steal(&self) -> f64 {
        self.0
            .iter()
            .map(|b| match b.kind {
                BuffKind::Battle { life_steal, .. } => life_steal,
                BuffKind::Affix { life_steal, stacks, .. } => life_steal * f64::from(stacks),
                _ => 0.0,
            })
            .sum()
    }

    /// The pull velocity, if being pulled (overrides steering like Charge).
    pub fn pull(&self) -> Option<bevy_math::Vec2> {
        self.0.iter().find_map(|b| if let BuffKind::Pull(v) = b.kind { Some(v) } else { None })
    }

    /// The gameplay tags its buffs grant ([`BuffKind::Tag`]).
    pub fn tags(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().filter_map(|b| if let BuffKind::Tag(t) = b.kind { Some(t) } else { None })
    }

    pub fn speed_bonus(&self) -> f32 {
        self.0
            .iter()
            .map(|b| match b.kind {
                BuffKind::MoveSpeed(v) => v,
                BuffKind::Affix { speed, .. } => speed,
                _ => 0.0,
            })
            .sum()
    }

    /// Factor on healing received (grievous wounds; the strongest one counts).
    pub fn heal_factor(&self) -> f64 {
        let worst = self.0.iter().map(|b| if let BuffKind::GrievousWounds(r) = b.kind { r } else { 0.0 }).fold(0.0, f64::max);
        (1.0 - worst).max(0.0)
    }
}

/// Adds `buff` to the actor (creating its `Buffs`).
/// Adds burn stacks (`Add Burning Buff Arg`). On a burning actor they add to its stacks and its timer runs on
/// (the effect's stack policies, read as "never refresh the duration" and "never reset the period"; HYPOTHESIS).
pub(crate) fn add_burn(world: &mut World, e: Entity, by: SimId, burn: crate::staffs::BurnSpec) {
    let interval = (burn.interval / crate::DT).round().max(1.0) as u32;
    if let Some(mut buffs) = world.get_mut::<Buffs>(e)
        && let Some(b) = buffs.0.iter_mut().find(|b| matches!(b.kind, BuffKind::Burning { .. }))
    {
        if let BuffKind::Burning { by: old_by, stacks, .. } = &mut b.kind {
            *stacks += burn.stacks;
            *old_by = by;
        }
        return;
    }
    add_buff(world, e, Buff { kind: BuffKind::Burning { by, damage: burn.damage, stacks: burn.stacks, interval }, skill: SkillKind::Item, ticks_left: u32::MAX, age: 0 });
}

pub(crate) fn add_buff(world: &mut World, e: Entity, buff: Buff) {
    match world.get_mut::<Buffs>(e) {
        Some(mut b) => b.add(buff),
        None => {
            let mut b = Buffs::default();
            b.add(buff);
            world.entity_mut(e).insert(b);
        }
    }
}

/// The actor's attack and defence with its buffs applied (and its worn full-health and out-of-combat rolls).
pub(crate) fn attack_of(world: &World, e: Entity, base: AttackStats) -> AttackStats {
    let base = crate::gear_triggers::conditional_attack(world, e, base);
    world.get::<Buffs>(e).map_or(base, |b| b.attack(base))
}

pub(crate) fn defence_of(world: &World, e: Entity, base: DefenceStats) -> DefenceStats {
    world.get::<Buffs>(e).map_or(base, |b| b.defence(base))
}

/// Heals a living actor by `amount` after grievous wounds; returns what was healed (and reports it).
pub(crate) fn heal(world: &mut World, id: SimId, e: Entity, amount: f64) -> f64 {
    let factor = world.get::<Buffs>(e).map_or(1.0, Buffs::heal_factor);
    let Some(mut h) = world.get_mut::<Health>(e) else { return 0.0 };
    if h.current <= 0.0 || amount <= 0.0 {
        return 0.0;
    }
    let healed = (amount * factor).min(h.max - h.current);
    h.current += healed;
    world.resource_mut::<SimEvents>().push(SimEvent::Healed { id, amount: healed });
    healed
}

/// After the actor dealt `damage`: life steal (buffs' and worn rolls') and the worn on-hit rolls; a hit from
/// Stealth ends it and, if it was not blocked, gives 50% damage reduction for 3 s (Rogue skill text).
pub(crate) fn after_damage_dealt(world: &mut World, id: SimId, e: Entity, damage: f64, blocked: bool) {
    let steal = world.get::<Buffs>(e).map_or(0.0, Buffs::life_steal) + crate::gear_triggers::life_leech(world, e);
    if steal > 0.0 && damage > 0.0 {
        heal(world, id, e, damage * steal);
    }
    if damage > 0.0 {
        crate::gear_triggers::on_hit(world, id, e);
    }
    let stealth = world.get::<Buffs>(e).and_then(|b| b.0.iter().find(|x| matches!(x.kind, BuffKind::Stealth { .. })).map(|x| x.skill));
    if let Some(skill) = stealth
        && damage > 0.0
    {
        if let Some(mut b) = world.get_mut::<Buffs>(e) {
            b.0.retain(|x| !matches!(x.kind, BuffKind::Stealth { .. }));
        }
        if !blocked {
            add_buff(world, e, Buff { kind: BuffKind::DamageReduction(crate::class_skills::STEALTH_HIT_REDUCTION), skill, ticks_left: (crate::class_skills::STEALTH_HIT_SECONDS / crate::DT).round() as u32, age: 0 });
        }
    }
}

/// The damage `te` actually takes from a hit of `damage` after its damage reduction, petrify and Ice
/// Barrier. `from` is the attacker of an active hit (`None` for bleeds and curses): a hit from within
/// `class_skills` petrify range ends petrify first, and one within Ice Armor's range sets off its slow.
pub(crate) fn incoming_damage(world: &mut World, te: Entity, damage: f64, from: Option<Entity>) -> f64 {
    let Some(b) = world.get::<Buffs>(te) else { return damage };
    if b.encased() || b.0.iter().any(|x| matches!(x.kind, BuffKind::Invincible)) {
        return 0.0;
    }
    let at = world.get::<Position>(te).map_or(Vec3::ZERO, |p| p.0);
    let gap = from.and_then(|ae| world.get::<Position>(ae)).map(|p| (p.0 - at).truncate().length());
    if let (Some(ae), Some(gap), Some((range, slow, seconds, skill))) = (from, gap, b.0.iter().find_map(|x| if let BuffKind::IceArmor { range, slow, seconds } = x.kind { Some((range, slow, seconds, x.skill)) } else { None }))
        && gap <= range
        && ae != te
    {
        if let Some(mut b) = world.get_mut::<Buffs>(te) {
            b.0.retain(|x| !matches!(x.kind, BuffKind::IceArmor { .. }));
        }
        add_buff(world, ae, Buff { kind: BuffKind::Slow(slow), skill, ticks_left: (seconds / crate::DT).round() as u32 + 1, age: 0 });
    }
    let Some(b) = world.get::<Buffs>(te) else { return damage };
    let close = gap.is_some_and(|g| g <= crate::overrides::class_skills::PETRIFY_BREAK_RANGE);
    let petrified = b.0.iter().find_map(|x| if let BuffKind::Petrified { damage_res } = x.kind { Some(damage_res) } else { None });
    let reduction = b.0.iter().map(|x| if let BuffKind::DamageReduction(r) = x.kind { r } else { 0.0 }).fold(0.0, f64::max);
    let mut d = damage * (1.0 - reduction).max(0.0);
    match petrified {
        Some(_) if close => {
            if let Some(mut b) = world.get_mut::<Buffs>(te) {
                b.0.retain(|x| !matches!(x.kind, BuffKind::Petrified { .. }));
            }
        }
        Some(res) => d *= (1.0 - res).max(0.0),
        None => {}
    }
    d
}

/// Removes the actor's debuffs.
pub(crate) fn cleanse(world: &mut World, e: Entity) {
    if let Some(mut b) = world.get_mut::<Buffs>(e) {
        b.0.retain(|x| !x.kind.is_debuff());
    }
}

/// Counts buffs down, removes expired ones (and all of a dead actor's), and pulses Rend.
pub(crate) fn buffs_tick(world: &mut World) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for &(id, e) in &ids {
        let Some(mut buffs) = world.get::<Buffs>(e).cloned() else { continue };
        if buffs.0.is_empty() {
            continue;
        }
        let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
        let pos = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
        // (dealt by, damage, reported as, heal for the dealer)
        let mut pulses: Vec<(SimId, f64, HitSource, f64)> = Vec::new();
        let mut self_heal = 0.0;
        for b in &mut buffs.0 {
            b.age += 1;
            b.ticks_left = b.ticks_left.saturating_sub(1);
            let mut burnt_out = false;
            match &mut b.kind {
                BuffKind::Rent { by, per_metre, interval, last } if alive && b.age % (*interval).max(1) == 0 => {
                    let metres = f64::from((pos - *last).truncate().length()) * 0.01;
                    *last = pos;
                    if metres > 0.0 {
                        pulses.push((*by, *per_metre * metres, HitSource::Bleed(SkillKind::Rend), 0.0));
                    }
                }
                BuffKind::FrostCurse { by, damage, heal, interval } if alive && b.age % (*interval).max(1) == 0 => pulses.push((*by, *damage, HitSource::Bleed(b.skill), *heal)),
                BuffKind::Dot { by, damage, interval } if alive && b.age % (*interval).max(1) == 0 => pulses.push((*by, *damage, HitSource::Bleed(b.skill), 0.0)),
                BuffKind::IceBarrier { heal } | BuffKind::Regen(heal) if alive && b.age % (1.0 / crate::DT).round() as u32 == 0 => self_heal += *heal,
                BuffKind::Burning { by, damage, stacks, interval } if alive && b.age % (*interval).max(1) == 0 => {
                    pulses.push((*by, *damage, HitSource::Staff(crate::staffs::StaffHit::Burn), 0.0));
                    *stacks = stacks.saturating_sub(1);
                    burnt_out = *stacks == 0;
                }
                _ => {}
            }
            if burnt_out {
                b.ticks_left = 0;
            }
        }
        buffs.0.retain(|b| alive && b.ticks_left > 0);
        if let Some(mut b) = world.get_mut::<Buffs>(e) {
            *b = buffs;
        }
        if self_heal > 0.0 {
            heal(world, id, e, self_heal);
        }
        for (by, damage, source, heal_by) in pulses {
            if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
                break;
            }
            let damage = incoming_damage(world, e, damage, None);
            let left = crate::world_items::apply_damage(world, id, e, damage);
            {
                let mut events = world.resource_mut::<SimEvents>();
                events.push(SimEvent::Damaged { source: by, target: id, damage, crit: false, blocked: false, health: left, by: source });
                if left <= 0.0 {
                    events.push(SimEvent::Died { id });
                }
            }
            if heal_by > 0.0
                && let Some(be) = world.resource::<SimIndex>().get(by)
            {
                heal(world, by, be, heal_by);
            }
        }
    }
}

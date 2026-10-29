//! What worn items' rolled attributes do beyond plain stats (the game's `Ability/GameAbility/Player/Attribute`
//! abilities and `GameplayEffect/Player/Attr` effects).
//!
//! on kill (`GA_TGF_Killing_Player`, on `TGF.GameplayEvent.Killing`; CONFIRMED by its bytecode):
//! - `GiveBonusToPlayer`: heal by `HealOnKill` (`GE_PlayerAttr_HealOnKilling`, `TgfFixedHealExecution`), and for a
//!   mana class (`CareerSpec.Magic`) `MagicOnKill` mana, for a soul energy class `SoulEnergyOnKill`, for the druid
//!   `NaturalEnergyOnKill` (only heal and mana on kill are rolled by items).
//! - `UpdateAllGEStack(1)`: each kill-growth effect gains a stack, clamped to its `KillingMaxLayer*` attribute (the
//!   item roll, 1-6): `GE_PlayerAttr_AddMaxHealthPercent` / `AddAttackBoostRate` / `AddCriticalRate`, infinite, each
//!   stack worth its `PlayerAttrCurveTable` row (0.025 max health, 0.02 attack boost, 0.055 critical rate).
//! - `trigger_effect_gain_gold_on_kill` adds gold coins (`ID_Loot_Gold_100069`, `run::GOLD_ITEM`); no item rolls it,
//!   so it isn't here.
//!
//! The heal and energy effects take their attribute x1 (`GE_PlayerAttr_HealOnKilling` / `MagicOnKilling` /
//! `SoulEnergyOnKilling` / `NaturalEnergyOnKilling`: attribute-based magnitudes, coefficient 1; CONFIRMED).
//!
//! on hit ([`on_hit`]): no blueprint or effect applies `HealOnHit`, `MagicOnHit` or `LifeLeachRate` (the damage
//! execution is native), so HYPOTHESIS: every hit that deals damage heals `recover_heal_on_hit_base`, gives
//! `magic_magic_on_hit` mana (or the class's `*_on_hit` energy), and heals `recover_life_leach_rate` of the damage,
//! added to the buffs' life steal.
//!
//! flat rolls:
//! - max health `base * (1 + percent) + extra` ([`max_health`]; `defend_max_health_extra_explain`: "your final health
//!   gains extra, equal to this value"; the kill-growth stacks add to the same `MaxHealthPercent`);
//! - `recover_heal_over_time_base` health and `magic_magic_over_time` mana (the items' share: the class's is its
//!   power's own regeneration) (`natural_energy_natural_energy_over_time`
//!   for the druid) per second, paid every 0.2 s (`GE_PlayerAttr_Recover_HeroPerSec`: period 0.2, x0.2; CONFIRMED);
//! - `recover_skill_cooldown_scale_rate`: skill cooldowns x `max(1 - rate, SkillCooldownScaleRateMin)`
//!   ([`cooldown_scale`]; the explain string: "your active skills' cooldowns shrink by this share"; the floor from
//!   `BattleCalculation`, 0.6);
//! - HYPOTHESIS: `magic_magic_generate_rate` ("mana recovery efficiency") multiplies mana regeneration by
//!   `1 + rate`, `soul_energy_soul_energy_generate_rate` the soul energy taken from the dead.
//! - `speed_level_interact_speed_rate` ("opening chests, searching, opening doors and statues get faster by this
//!   share, at most 50 %"): `BP_MapInteractionComponent` sets every held interaction's time to
//!   `GetLevelInteractSpeedFinal(BaseInteractTime, 0.5)` (CONFIRMED bytecode; the function is native): HYPOTHESIS
//!   `base / (1 + min(rate, 0.5))` ([`interact_time`]), on doors, levers and extraction devices.
//!
//! conditional rolls (the `_explain` strings and the abilities' bytecode, CONFIRMED):
//! - full health (`GA_TGF_FullHPEffect`: `GE_PlayerAttr_FullHPEffect` while `IsHPFull`): attack boost and critical
//!   rate `*_with_full_health`;
//! - out of combat (`GA_TGF_DisengageEffect`): with no damage dealt or taken for `EngageWaitTime` 8 s the actor is
//!   disengaged and `GE_PlayerAttr_DisengageEffect` adds the `*_with_hit_running` rolls to move speed, heal over time,
//!   critical rate and life leech; entering combat removes it `GEDelayRemoveTime` 2 s later (only the first hit of a
//!   fight starts that delay); a fresh actor starts disengaged ([`Engagement`]).
//! - the items' `speed_move_speed_base` rolls (-30 to +16) were ignored as well (the spawn speed is the class's): they
//!   and the out-of-combat speed are a [`GearSpeed`] bonus.
//!
//! triggers (the abilities' bytecode and `_explain` strings, CONFIRMED):
//! - a successful guard (`GA_TGF_GuardEffect`, on `TGF.GameplayEvent.SucceedGuarded`): heal
//!   `trigger_effect_heal_on_defend_success` and deal `trigger_effect_damage_on_defend_success` true damage to the
//!   attacker (`GE_PlayerAttr_GuardHeal` / `GuardCounterDamage`, x1);
//! - a chest's first opening (`GA_TGF_OpeningChest`): heal `trigger_effect_heal_on_open_box`, mana
//!   `trigger_effect_magic_on_open_box` (mana classes), and `trigger_effect_gain_gold_on_open_box` gold coins (rounded);
//! - a class skill cast (`GA_TGF_AddShieldOnCastSkill`, on `TGF.Ability.Commit` of an active skill): a shield of
//!   `trigger_effect_add_shield_on_cast_skill` for 2 s, at most once per `TakeEffectInv` 10 s;
//! - a weapon attack's damaging hit (`GA_TGF_AdditionalElementFixedDamage`): each
//!   `trigger_effect_additional_element_*_damage_on_causing_hit` roll as damage of its element, and
//!   `trigger_effect_additional_fixed_damage_on_causing_hit` as true damage. HYPOTHESIS: the element damage is the
//!   roll times the target's resistance term ("not affected by attack data but by defence data"; the effects'
//!   magnitude capture has a coefficient 2 that the explain strings don't mention, left out); weapon attacks are the
//!   swings, crossbow bolts and staff spells.
//!
//! HYPOTHESIS: a hit of any other actor counts (the game skips its own team's, `CompareTeams`; co-op comes later);
//! the attack bonuses apply as each hit is resolved ([`crate::buffs::attack_of`]).
//!
//! HYPOTHESIS: a kill-growth stack of max health percent adds to the percent in [`max_health`] and leaves current
//! health as it is; the stacks last for the run (the effects are infinite and the character is new each run).
//! The killer is the last actor to damage the victim before it died ([`crate::command::SimEvents`] keeps it).

use bevy_ecs::prelude::*;
use serde_json::Value;

use crate::attributes::AttributeSheet;
use crate::class_skills::{Power, PowerKind};
use crate::command::SimEvents;
use crate::id::SimIndex;
use crate::melee::{Combatant, Health};

/// The kill-growth attributes (item roll: the most stacks) and their effects' `PlayerAttrCurveTable` rows, in
/// [`KillGrowth`] order.
pub const GROWTH: [(&str, &str); 3] = [
    ("affix_buff_arg_killing_max_layer_max_health_percent", "GE_PlayerAttr_AddMaxHealthPercent"),
    ("affix_buff_arg_killing_max_layer_attack_boost_rate", "GE_PlayerAttr_AddAttackBoostRate"),
    ("affix_buff_arg_killing_max_layer_critical_rate", "GE_PlayerAttr_AddCriticalRate"),
];

/// Each kill-growth stack's worth, in [`GROWTH`] order: max health percent, attack boost rate, critical rate; and the
/// lowest skill cooldown scale (`BattleCalculation.SkillCooldownScaleRateMin`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct GearTriggerData {
    pub growth: [f64; 3],
    pub cooldown_floor: f64,
}

/// Reads [`GearTriggerData`] from `PlayerAttrCurveTable` (loose dump).
pub fn load_gear_triggers() -> anyhow::Result<GearTriggerData> {
    use anyhow::Context;
    let v = crate::staffs::read_json("json/loose", "/Game/TGF/Ability/GameplayEffect/Player/Attr/PlayerAttrCurveTable").context("PlayerAttrCurveTable not decoded (tools/rebuild_data.sh)")?;
    let mut growth = [0.0; 3];
    for (slot, (_, row)) in growth.iter_mut().zip(GROWTH) {
        *slot = curve_value(&v, row).with_context(|| format!("PlayerAttrCurveTable: no {row}"))?;
    }
    let bc = crate::staffs::read_json("json/assets", "/Game/TGF/Ability/Data/BattleCalculation").context("BattleCalculation not decoded")?;
    let cooldown_floor = bc.as_array().and_then(|a| a.first()).and_then(|o| o["Properties"]["SkillCooldownScaleRateMin"].as_f64()).context("BattleCalculation: no SkillCooldownScaleRateMin")?;
    Ok(GearTriggerData { growth, cooldown_floor })
}

/// The worn items' attributes added to the class sheet (set by [`crate::inventory::refresh_combat`]).
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct WornAttributes(pub AttributeSheet);

/// When the actor last dealt or took damage and when its current fight began.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Engagement {
    pub last: Option<u64>,
    pub since: u64,
}

/// `GA_TGF_DisengageEffect`'s `EngageWaitTime` and `GEDelayRemoveTime`, seconds.
const ENGAGE_WAIT: f32 = 8.0;
const DISENGAGE_LINGER: f32 = 2.0;

/// The items' move speed rolls plus the out-of-combat roll, cm/s, added to the actor's speed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct GearSpeed(pub f32);

/// Whether the out-of-combat rolls hold now.
pub fn disengaged(world: &World, e: Entity) -> bool {
    let now = world.resource::<crate::TickCount>().0;
    let ticks = |s: f32| (s / crate::DT).round() as u64;
    match world.get::<Engagement>(e) {
        Some(Engagement { last: Some(last), since }) => now.saturating_sub(*last) >= ticks(ENGAGE_WAIT) || now.saturating_sub(*since) < ticks(DISENGAGE_LINGER),
        _ => true,
    }
}

/// The full-health and out-of-combat attack rolls on a hit's attack.
pub fn conditional_attack(world: &World, e: Entity, mut a: crate::combat::AttackStats) -> crate::combat::AttackStats {
    let Some(w) = world.get::<WornAttributes>(e) else { return a };
    if world.get::<Health>(e).is_some_and(|h| h.current > 0.0 && h.current >= h.max) {
        a.attack_boost_rate += w.0.get("affix_stat_effect_attack_boost_rate_with_full_health");
        a.critical_rate += w.0.get("affix_stat_effect_critical_rate_with_full_health");
    }
    let running = w.0.get("affix_stat_effect_critical_rate_with_hit_running");
    if running != 0.0 && disengaged(world, e) {
        a.critical_rate += running;
    }
    a
}

/// Kill-growth stacks, in [`GROWTH`] order.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KillGrowth(pub [u8; 3]);

impl KillGrowth {
    /// The stacks each attribute allows now (a stack above a lowered roll is dropped, `UpdateGEStack`'s clamp).
    fn clamped(self, sheet: &AttributeSheet) -> Self {
        let mut out = self;
        for (n, (attr, _)) in out.0.iter_mut().zip(GROWTH) {
            *n = (*n).min(sheet.get(attr).trunc().clamp(0.0, 255.0) as u8);
        }
        out
    }
}

/// Max health from a sheet: `base * (1 + percent) + extra`, with `more_percent` (the kill-growth stacks') added to
/// the percent.
pub fn max_health(sheet: &AttributeSheet, more_percent: f64) -> f64 {
    sheet.get("affix_defend_max_health_base") * (1.0 + sheet.get("affix_defend_max_health_percent") + more_percent) + sheet.get("affix_defend_max_health_extra")
}

/// Applies the kill-growth stacks to freshly computed combat numbers; returns their max health percent.
pub(crate) fn apply_growth(world: &World, e: Entity, combatant: &mut Combatant) -> f64 {
    let (Some(g), Some(data)) = (world.get::<KillGrowth>(e), world.get_resource::<GearTriggerData>()) else { return 0.0 };
    let [hp, atk, crit] = g.0.map(f64::from);
    combatant.attack.attack_boost_rate += atk * data.growth[1];
    combatant.attack.critical_rate += crit * data.growth[2];
    hp * data.growth[0]
}

/// A worn roll of the actor (0 without one).
pub(crate) fn worn(world: &World, e: Entity, attr: &str) -> f64 {
    world.get::<WornAttributes>(e).map_or(0.0, |w| w.0.get(attr))
}

/// The actor's skill cooldown scale.
pub fn cooldown_scale(world: &World, e: Entity) -> f32 {
    let floor = world.get_resource::<GearTriggerData>().map_or(0.0, |d| d.cooldown_floor);
    (1.0 - worn(world, e, "affix_recover_skill_cooldown_scale_rate")).max(floor) as f32
}

/// `GetLevelInteractSpeedFinal`'s cap on the interact speed roll (its literal argument in `BP_MapInteractionComponent`).
const INTERACT_SPEED_MAX: f64 = 0.5;

/// How long the actor holds an interaction whose base time is `base` s.
pub fn interact_time(world: &World, e: Entity, base: f32) -> f32 {
    let rate = worn(world, e, "affix_speed_level_interact_speed_rate").clamp(0.0, INTERACT_SPEED_MAX);
    (f64::from(base) / (1.0 + rate)) as f32
}

/// The worn rolls' health and power over time, every 0.2 s.
fn regen(world: &mut World) {
    let period = (REGEN_PERIOD / crate::DT).round() as u64;
    if !world.resource::<crate::TickCount>().0.is_multiple_of(period) {
        return;
    }
    let mut q = world.query::<(Entity, &crate::SimId, &WornAttributes, &Health)>();
    let who: Vec<(Entity, crate::SimId, f64, [f64; 3])> = q
        .iter(world)
        .filter(|(_, _, _, h)| h.current > 0.0)
        .map(|(e, id, w, _)| (e, *id, w.0.get("affix_recover_heal_over_time_base"), [w.0.get("affix_magic_magic_over_time"), 0.0, w.0.get("affix_natural_energy_natural_energy_over_time")]))
        .collect();
    let share = f64::from(REGEN_PERIOD);
    for (e, id, mut heal, mut power) in who {
        if disengaged(world, e) {
            heal += worn(world, e, "affix_stat_effect_heal_over_time_with_hit_running");
        }
        // The class's own mana regeneration is already its `Power::per_second` (from the same attribute): only the
        // items' share is added here.
        power[0] -= world.get::<crate::inventory::CharacterSheet>(e).map_or(0.0, |c| c.base.get("affix_magic_magic_over_time"));
        if heal > 0.0 && world.get::<Health>(e).is_some_and(|h| h.current < h.max) {
            crate::buffs::heal(world, id, e, heal * share);
        }
        let mana_rate = 1.0 + worn(world, e, "affix_magic_magic_generate_rate");
        if let Some(mut p) = world.get_mut::<Power>(e) {
            let gain = match p.kind {
                PowerKind::Mana => power[0] * mana_rate,
                PowerKind::SoulEnergy => power[1],
                PowerKind::NaturalEnergy => power[2],
            } * share;
            if gain > 0.0 {
                p.current = (p.current + gain).min(p.max);
            }
        }
    }
}

/// Marks this tick's hits on both sides as combat, then sets each geared actor's [`GearSpeed`].
fn engagement(world: &mut World) {
    let now = world.resource::<crate::TickCount>().0;
    let hits = world.resource_mut::<SimEvents>().take_damage();
    for (target, source) in hits {
        for id in [target, source] {
            let Some(e) = world.resource::<SimIndex>().get(id) else { continue };
            if world.get::<WornAttributes>(e).is_none() {
                continue;
            }
            // A hit after 8 s of quiet (or the first) starts a fight.
            let fresh = world.get::<Engagement>(e).and_then(|g| g.last.filter(|l| now.saturating_sub(*l) < (ENGAGE_WAIT / crate::DT).round() as u64).map(|_| g.since));
            let since = fresh.unwrap_or(now);
            world.entity_mut(e).insert(Engagement { last: Some(now), since });
        }
    }
    let mut q = world.query::<(Entity, &WornAttributes)>();
    let geared: Vec<(Entity, f64, f64)> = q.iter(world).map(|(e, w)| (e, w.0.get("affix_speed_move_speed_base"), w.0.get("affix_stat_effect_move_speed_with_hit_running"))).collect();
    for (e, speed, running) in geared {
        let class = world.get::<crate::inventory::CharacterSheet>(e).map_or(speed, |c| c.base.get("affix_speed_move_speed_base"));
        let bonus = (speed - class + if running != 0.0 && disengaged(world, e) { running } else { 0.0 }) as f32;
        if world.get::<GearSpeed>(e).map_or(0.0, |g| g.0) != bonus {
            world.entity_mut(e).insert(GearSpeed(bonus));
        }
    }
}

/// Damage straight to `te` (after its damage reduction and shield), reported as an item's hit from `se`.
fn trigger_damage(world: &mut World, (sid, se): (crate::SimId, Entity), (tid, te): (crate::SimId, Entity), damage: f64) {
    if damage <= 0.0 || world.get::<Health>(te).is_none_or(|h| h.current <= 0.0) {
        return;
    }
    let damage = crate::buffs::incoming_damage(world, te, damage, Some(se));
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    let mut events = world.resource_mut::<SimEvents>();
    events.push(crate::SimEvent::Damaged { source: sid, target: tid, damage, crit: false, blocked: false, health: left, by: crate::skills::HitSource::Skill(crate::skills::SkillKind::Item) });
    if left <= 0.0 {
        events.push(crate::SimEvent::Died { id: tid });
    }
}

/// A successful guard of `de` against `ae`.
pub(crate) fn on_guarded(world: &mut World, (did, de): (crate::SimId, Entity), (aid, ae): (crate::SimId, Entity)) {
    let heal = worn(world, de, "affix_trigger_effect_heal_on_defend_success");
    if heal > 0.0 {
        crate::buffs::heal(world, did, de, heal);
    }
    let damage = worn(world, de, "affix_trigger_effect_damage_on_defend_success");
    trigger_damage(world, (did, de), (aid, ae), damage);
}

/// `id` opened a chest for the first time.
pub(crate) fn on_chest_opened(world: &mut World, id: crate::SimId, e: Entity) {
    let Some(sheet) = world.get::<WornAttributes>(e).map(|w| w.0.clone()) else { return };
    let heal = sheet.get("affix_trigger_effect_heal_on_open_box");
    if heal > 0.0 {
        crate::buffs::heal(world, id, e, heal);
    }
    let magic = sheet.get("affix_trigger_effect_magic_on_open_box") + sheet.get("affix_trigger_effect_magic_on_open_chest");
    if let Some(mut p) = world.get_mut::<Power>(e)
        && p.kind == PowerKind::Mana
        && magic > 0.0
    {
        p.current = (p.current + magic).min(p.max);
    }
    let gold = sheet.get("affix_trigger_effect_gain_gold_on_open_box").round();
    if gold >= 1.0 {
        crate::inventory::apply(world, crate::inventory::InventoryCommand::GiveStarting { id, item_id: crate::overrides::run::GOLD_ITEM, count: gold.min(f64::from(u16::MAX)) as u16 });
    }
}

/// When the actor's last cast shield came (once per [`CAST_SHIELD_EVERY`]).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastShield(pub u64);

/// `GA_TGF_AddShieldOnCastSkill`'s `TakeEffectInv` and `GE_PlayerAttr_AddShieldOnCastSkill02`'s duration, seconds.
const CAST_SHIELD_EVERY: f32 = 10.0;
const CAST_SHIELD_SECONDS: f32 = 2.0;

/// The actor started a class skill.
pub(crate) fn on_skill_cast(world: &mut World, e: Entity) {
    let amount = worn(world, e, "affix_trigger_effect_add_shield_on_cast_skill");
    if amount <= 0.0 {
        return;
    }
    let now = world.resource::<crate::TickCount>().0;
    let ticks = |s: f32| (s / crate::DT).round() as u64;
    if world.get::<CastShield>(e).is_some_and(|c| now.saturating_sub(c.0) <= ticks(CAST_SHIELD_EVERY)) {
        return;
    }
    world.entity_mut(e).insert(CastShield(now));
    let mut fx = world.get::<crate::world_items::Effects>(e).cloned().unwrap_or_default();
    let keep = fx.shield.map_or(0.0, |s| s.0);
    fx.shield = Some((keep.max(amount), ticks(CAST_SHIELD_SECONDS) as u32));
    world.entity_mut(e).insert(fx);
}

/// The element rolls' attributes, in `Element` order of [`crate::combat::Element::ALL`]'s names.
const EXTRA_ELEMENTS: [(&str, crate::combat::Element); 6] = [
    ("affix_trigger_effect_additional_element_none_damage_on_causing_hit", crate::combat::Element::Physical),
    ("affix_trigger_effect_additional_element_fire_damage_on_causing_hit", crate::combat::Element::Fire),
    ("affix_trigger_effect_additional_element_ice_damage_on_causing_hit", crate::combat::Element::Ice),
    ("affix_trigger_effect_additional_element_thunder_damage_on_causing_hit", crate::combat::Element::Thunder),
    ("affix_trigger_effect_additional_element_light_damage_on_causing_hit", crate::combat::Element::Light),
    ("affix_trigger_effect_additional_element_dark_damage_on_causing_hit", crate::combat::Element::Dark),
];

/// A weapon attack of `ae` dealt damage to `te`: the extra element and true damage rolls.
pub(crate) fn on_weapon_hit(world: &mut World, (aid, ae): (crate::SimId, Entity), (tid, te): (crate::SimId, Entity)) {
    if aid == tid {
        return;
    }
    let Some(sheet) = world.get::<WornAttributes>(ae).map(|w| w.0.clone()) else { return };
    let defence = world.get::<Combatant>(te).map(|c| crate::buffs::defence_of(world, te, c.defence));
    let mut damage = sheet.get("affix_trigger_effect_additional_fixed_damage_on_causing_hit");
    for (attr, element) in EXTRA_ELEMENTS {
        let roll = sheet.get(attr);
        if roll > 0.0 {
            let resist = defence.as_ref().map_or(0.0, |d| d.resistance.get(element));
            damage += roll * crate::combat::resistance_term(resist, 0.0);
        }
    }
    trigger_damage(world, (aid, ae), (tid, te), damage);
}

/// `GE_PlayerAttr_Recover_HeroPerSec`'s period, seconds.
const REGEN_PERIOD: f32 = 0.2;

/// Gives every killer this tick its on-kill rolls.
pub fn gear_triggers_tick(world: &mut World) {
    engagement(world);
    regen(world);
    let kills = world.resource_mut::<SimEvents>().take_kills();
    for (killer, _victim) in kills {
        let Some(e) = world.resource::<SimIndex>().get(killer) else { continue };
        let Some(sheet) = world.get::<WornAttributes>(e).map(|w| w.0.clone()) else { continue };
        if world.get::<Health>(e).is_none_or(|h| h.current <= 0.0) {
            continue;
        }
        let heal = sheet.get("affix_recover_heal_on_kill_base");
        if heal > 0.0 {
            crate::buffs::heal(world, killer, e, heal);
        }
        gain_power(world, e, &sheet, ["affix_magic_magic_on_kill", "affix_soul_energy_soul_energy_on_kill", "affix_natural_energy_natural_energy_on_kill"]);
        let before = world.get::<KillGrowth>(e).copied().unwrap_or_default();
        let after = KillGrowth(before.0.map(|n| n.saturating_add(1))).clamped(&sheet);
        if after != before {
            world.entity_mut(e).insert(after);
            crate::inventory::refresh_combat(world, e);
        }
    }
}

/// Adds the attribute matching the actor's power (mana, soul energy, shadow energy, in that order) to it.
fn gain_power(world: &mut World, e: Entity, sheet: &AttributeSheet, attrs: [&str; 3]) {
    let Some(mut p) = world.get_mut::<Power>(e) else { return };
    let gain = sheet.get(match p.kind {
        PowerKind::Mana => attrs[0],
        PowerKind::SoulEnergy => attrs[1],
        PowerKind::NaturalEnergy => attrs[2],
    });
    if gain > 0.0 {
        p.current = (p.current + gain).min(p.max);
    }
}

/// The worn rolls' life leech rate, with the out-of-combat roll.
pub(crate) fn life_leech(world: &World, e: Entity) -> f64 {
    let Some(w) = world.get::<WornAttributes>(e) else { return 0.0 };
    let running = w.0.get("affix_stat_effect_life_leach_rate_with_hit_running");
    w.0.get("affix_recover_life_leach_rate") + if running != 0.0 && disengaged(world, e) { running } else { 0.0 }
}

/// After the actor's hit dealt damage: heal and power on hit.
pub(crate) fn on_hit(world: &mut World, id: crate::SimId, e: Entity) {
    let Some(sheet) = world.get::<WornAttributes>(e).map(|w| w.0.clone()) else { return };
    let heal = sheet.get("affix_recover_heal_on_hit_base");
    if heal > 0.0 {
        crate::buffs::heal(world, id, e, heal);
    }
    gain_power(world, e, &sheet, ["affix_magic_magic_on_hit", "affix_soul_energy_soul_energy_on_hit", "affix_natural_energy_natural_energy_on_hit"]);
}

/// A `PlayerAttrCurveTable` row's first value.
fn curve_value(table: &Value, row: &str) -> Option<f64> {
    table.as_array()?.iter().find_map(|o| o.get("Rows"))?[row]["Keys"].as_array()?.first()?["Value"].as_f64()
}

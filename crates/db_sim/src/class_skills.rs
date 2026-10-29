//! SwordMaster and Rogue skills, Wizard and Lich skills with mana and soul energy. Classes other
//! than the Fighter carry a [`ClassKit`] (loadout, per-skill cooldowns, class state) and, for
//! casters, a [`Power`]; the Fighter keeps its `SkillKit`.
//!
//! CONFIRMED (`ExcelSkillNumExchange`, `CareerSkillDamageCurveTable`, skill text): SwordMaster: Psychic Flying
//! Swords up to 4 homing swords at 80% each, Blade Waltz 1 sword for 8 s at 45% per hit;
//! `SwordMasterQ_DamageRate` (0 to 20 m, −25% to 30 m, −50% beyond; read as a distance falloff, HYPOTHESIS).
//! Rogue: Petrifying Poison (next hit petrifies; 100% less damage taken; a close hit ends it), Fade into the
//! Void (3 s preparation, 30 s Stealth), Swift Concealment (6 s Stealth at once), Shadow Veil (3 s
//! preparation, 15 s Stealth), Flying Blades (6 daggers at 20%, 70% slow for 3 s, cleanses debuffs); a hit
//! from Stealth gives 50% damage reduction for 3 s; the Stealth aura `Aura_TGF_Rouge_Stealth` (`MinDistance2D`
//! 300, `MaxZDistance` 200), read as the range at which monsters see a stealthed Rogue (HYPOTHESIS reading).
//! HYPOTHESIS: cooldowns, projectile flight, targeting cone and range, Blade Waltz radius and hit interval,
//! Stealth's crit-damage bonus, petrify length and break range ([`crate::overrides::class_skills`]).
//!
//! CONFIRMED: Wizard mana 300 and 0.2 per second (`ExcelRoleLevelUpgrade` `magic_max_magic_base`,
//! `magic_magic_over_time`, labelled mana per second); Pyroblast stage 1 three homing fireballs at 60%
//! fire (speed `WizardQSpeedCurve`, distance falloff `WizardQ_DamageRate`), stage 2 one 240% fireball
//! bursting where it lands; Flame Shock 100 base fire damage (`CareerSkillDamageNumberTextFireBase`),
//! knockback, 85% slow for 1 s. Lich soul energy 100; Blizzard storm 8% (non-active) with a 40% slow,
//! unfolded 12% with an 80% slow, storm radius 80 and speed 600 over 5 s (`SnowStormAreaRadius`,
//! `SnowStormMoveSpeedFactor`, read as cm, cm/s and the channel length: HYPOTHESIS readings); Ice
//! Barrier 5 s, no damage, 1.25% max health per second; Frost Torment bolt or direct hit (16% / 36%
//! ice, 60% slow for 1 s) and Frost Curse (16% per second for 10 s, `DamageInterval` 1, the Lich heals
//! 6 + 2% of `HealBaseNumberLichHealOnlyText` per pulse, HYPOTHESIS reading of the two curve rows),
//! spreading after 2 s within 300 cm (`GA_TGF_Lich_Q_DOT_Explode_Passive`); Ice Armor shield 90% of
//! `AddShieldDPHNumberText` for 60 s, the first close active hit (within `CounterRange` 500) slows the
//! attacker 60% for 2 s. HYPOTHESIS: costs, cooldowns, charge stages, targeting, flight, burst and
//! blizzard areas, the Lich's starting soul energy and soul gain per kill
//! ([`crate::overrides::class_skills`]).
//!
//! CONFIRMED: Priest mana as the Wizard's; Oracle heals with the GameplayEffect's base 36, additive 39
//! and attack-boost factor 0.6 (the `HealBaseNumberPriestHealOnlyText_Single` shape) and deals 75%
//! holy damage; Heal: base 40 + 44 (stage 1) and 86 + 94 (stage 2), the client's `_Single` /
//! `_SingleQ1` tooltip constants and the `GE_TGF_HealSingle_Basic_Priest_Q(1)` curve rows (HYPOTHESIS:
//! the shown value is the applied heal); Divine Protection 3 s immune, 2 charges, 60 s
//! (`GA_TGF_Priest_E_DoubleCharge`); Guardian shield 75% of `AddShieldDPHNumberText` for 60 s, 2
//! charges, 30 s (`GA_TGF_Priest_E_ShieldAOE_DoubleCharge`). DeathKnight soul energy 100; Wraith Ring
//! slows 25% for 1 s on activation, then deals the `DeathKnightQ` term (38 + 0.31 × stats) × 100% dark
//! non-active damage within 300 cm every 0.5 s (`GA_TGF_DeathKnightQ_Passive`); Soul Storm the same at
//! × 50% (`MoveFactorRate2`, HYPOTHESIS that it is Soul Storm's rate) while draining soul energy; Soul
//! Chain 100% dark weapon damage and a pull to the DeathKnight, restoring soul energy on an enemy hit.
//!
//! CONFIRMED: Druid shadow (natural) energy 120; panther form costs 30, drains 4 (read as per second,
//! HYPOTHESIS), +100 cm/s; in panther form weapon hits and Shadow Pounce restore 20; Shadow Pounce 172%
//! dark around the landing spot; Force of Nature summons a treant for 30 s whose hits deal the Druid's
//! 165% physical (stationary, sight 800 from `DryadSummoned`, swing timing from `AM_Treant_Attack2`,
//! HYPOTHESIS that the summon uses them); Rejuvenation heals for 30 s (rate 0.05, formula HYPOTHESIS);
//! both E skills recharge in 40 s (`GA_TGF_DruidPanther_DoubleSeeds/DoubleHeal`).

use std::collections::BTreeMap;

use anyhow::Context;
use bevy_ecs::prelude::*;
use bevy_math::{Quat, Vec3};
use db_formats::config::generated::{ConfigTables, EnRoleCareerType};

use crate::buffs::{Buff, BuffKind, Buffs};
use crate::combat::{AttackStats, AttackTerm, Element, HitZone, resolve_hit, tooltip};
use crate::id::SimIndex;
use crate::melee::{Aim, Combatant, Health, Melee, blocked_damage};
use crate::movement::{CAPSULE_HEIGHT, CAPSULE_RADIUS};
use crate::overrides::class_skills as tune;
use crate::projectiles::ProjectileSpec;
use crate::skills::{HitSource, SkillKind};
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng};

/// A hit from Stealth: damage reduction and its length (Rogue skill text).
pub const STEALTH_HIT_REDUCTION: f64 = 0.5;
pub const STEALTH_HIT_SECONDS: f32 = 3.0;
/// Preparation before Fade into the Void and Shadow Veil take effect (skill text).
pub const STEALTH_PREPARATION: f32 = 3.0;
/// Skill text: slows (rate, seconds) of the Blizzard storm and the unfolded blizzard (refreshed while inside), Frost
/// Torment and Ice Armor.
pub const BLIZZARD_SLOW: f64 = 0.4;
pub const BLIZZARD_UNFOLDED_SLOW: f64 = 0.8;
pub const FROST_TORMENT_SLOW: (f64, f32) = (0.6, 1.0);
pub const ICE_ARMOR_SLOW: (f64, f32) = (0.6, 2.0);
/// Skill text: in panther form weapon hits and Shadow Pounce restore 20 shadow energy.
pub const PANTHER_HIT_ENERGY: f64 = 20.0;
/// Skill text: Wraith Ring slows 25% on activation (for `DeathKnightQ_Time`).
pub const WRAITH_RING_SLOW: (f64, f32) = (0.25, 1.0);
/// Psychic Flying Swords (CONFIRMED `GA_TGF_SwordMaster_Q` defaults and bytecode): a hold. A sword is summoned on
/// the press, the next `SpawnSwordInvFirst` 0.7 s later, then every `SpawnSwordInv` 0.7 s, up to `MaxSwordNum` 4;
/// the release (`MaxChargeTime` 9999: it never goes off by itself) launches them one every 0.133 s (the
/// `DoReleaseOneSword` timer) at the enemies picked within `MaxSelectionDistance` 2000 cm and `UnSelectAngle` 45
/// degrees. The cooldown is the summoned count's effect (`CDConfig`: `GE_TGF_SwordMaster_Cooldown_Q_<n>`).
pub const FLYING_SWORDS_SPAWN_EVERY: f32 = 0.7;
pub const FLYING_SWORDS_MAX: u8 = 4;
pub const FLYING_SWORDS_LAUNCH_EVERY: f32 = 0.133_333;
pub const FLYING_SWORDS_SELECT_RANGE: f32 = 2000.0;
pub const FLYING_SWORDS_SELECT_DEG: f32 = 45.0;
/// Force of Nature (CONFIRMED `GA_TGF_DruidPanther_E_Human` defaults): E starts the seed's selection
/// (`bReleaseChargeByConfirm`, `MaxChargeTime` 999: it never goes off by itself) within `Range` 1000 cm; E again or a
/// left click plants it (`ReleaseInputTags`), a right click drops it (`CancelInputTags`).
pub const FORCE_OF_NATURE_RANGE: f32 = 1000.0;
/// Seconds by summoned count 0..=4 (`_Q` 45 s; `_Q_1` the base cooldown's 5 s; `_Q_2` 10, `_Q_3` 15, `_Q_4` 30).
pub const FLYING_SWORDS_COOLDOWNS: [f32; 5] = [45.0, 5.0, 10.0, 15.0, 30.0];

/// How many swords a Flying Swords hold of `held` ticks has summoned (one on the press).
pub fn swords_summoned(held: u32, most: u8) -> u8 {
    let more = (held as f32 * DT / FLYING_SWORDS_SPAWN_EVERY + 1e-4).floor() as u32;
    (1 + more).min(u32::from(most.clamp(1, FLYING_SWORDS_MAX))) as u8
}

/// Step curve: the value of the last key at or below `d` (0 before the first).
fn step_rate(keys: &[(f32, f32)], d: f32) -> f64 {
    f64::from(keys.iter().rev().find(|k| k.0 <= d).map_or(0.0, |k| k.1))
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwordMasterData {
    pub q_swords: u8,
    pub q_factor: f64,
    /// (distance cm, damage rate change) keys of `SwordMasterQ_DamageRate`.
    pub q_distance_rate: Vec<(f32, f32)>,
    pub e_swords: u8,
    pub e_seconds: f32,
    pub e_factor: f64,
}

impl SwordMasterData {
    /// Damage factor change for a target `d` cm away (step curve: the value of the last key at or below).
    pub fn distance_rate(&self, d: f32) -> f64 {
        step_rate(&self.q_distance_rate, d)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WizardData {
    pub mana_max: f64,
    /// Mana per second.
    pub mana_regen: f64,
    pub q_small_factor: f64,
    pub q_big_factor: f64,
    /// `WizardQSpeedCurve` keys (s, cm/s).
    pub q_speed: Vec<(f32, f32)>,
    /// `WizardQ_DamageRate` keys (distance cm, damage rate change).
    pub q_distance_rate: Vec<(f32, f32)>,
    /// Flame Shock base damage.
    pub e_base: f64,
    /// (CONFIRMED data): Pyroblast's mana, paid on the press (`GE_TGF_MagicCost_Wizard_Q`'s coefficient), and on
    /// reaching each stage (`_Q1`, `_Q2`: 0).
    pub q_cost: f64,
    pub q_stage_costs: [f64; 2],
    /// Pyroblast's cooldown after the three fireballs (`GE_TGF_Cooldown_WizardQ_ApplyCD`, charge level 2) and after
    /// the large one (`_ApplyCD_2`, level 3), s.
    pub q_cooldowns: [f32; 2],
    /// Seconds Pyroblast stays charged before it goes off by itself (`MaxChargeTime`).
    pub q_max_charge: f32,
    /// Share of the press cost a cancelled Pyroblast returns (`ReturnEnergyRate`, from the multi-level charge base;
    /// not when interrupted, `ApplyEffectOnCastFailed`).
    pub q_refund_rate: f64,
    /// Pyroblast's target pick: range, cm (`MaxDistance`), and half-angle off the aim, degrees (the charge base's
    /// `MaxUnselectAngle`; HYPOTHESIS: the native selection keeps a target inside it, so it picks inside it too).
    pub q_target_range: f32,
    pub q_target_deg: f32,
    /// Weapons Pyroblast needs worn, one of (`RestrictWeapon`): equipment sub-type names (`STAFF`, `RELIC`).
    pub q_weapons: Vec<String>,
    /// Flame Shock's cooldown (`GE_TGF_Cooldown_Wizard_E3`), s; it imports no cost effect: free.
    pub e_cooldown: f32,
    /// Flame Shock (`GA_TGF_Wizard_E3`): the sphere's radius (cm, `Radius`, at the hand socket `EP2`), the
    /// knockback distance (cm, `HitBackDistance`) and time (s, `KnockBackDuration`), and its slow
    /// (`GE_TGF_Buff_Wizard_E3_SlowDown`: the share of move speed taken, seconds).
    pub e_radius: f32,
    pub e_knockback: f32,
    pub e_knockback_secs: f32,
    pub e_slow: (f64, f32),
    /// The air jump's slow (`GE_TGF_Buff_Floating_SpeedDown`: `MoveSpeedPercent` -0.5 for 2 s).
    pub float_slow: (f64, f32),
    /// Pyroblast's projectiles. Stage 1 (`B_TGF_Track_Projectile_WizardQ2_Spline_New`, x3): launch delays (s,
    /// `ProjectileSpawnData`), collision half-size (cm, `BoxExtent`), explosion radius (cm, `ExplodeRadius`).
    pub q_small_delays: Vec<f32>,
    pub q_small_radius: f32,
    pub q_small_burst: f32,
    /// Stage 2 (`B_TGF_Normal_Projectile_WizardQ3`): speed keys (s, cm/s: `InitialSpeed`, `AccMag` up to
    /// `MaxSpeed`), collision half-size, burst radius, and the knockback (cm, s: `Hitback Distance`, the
    /// passive move's 0.15 s).
    pub q_big_speed: Vec<(f32, f32)>,
    pub q_big_radius: f32,
    pub q_big_burst: f32,
    pub q_big_knockback: (f32, f32),
}

impl WizardData {
    pub fn distance_rate(&self, d: f32) -> f64 {
        step_rate(&self.q_distance_rate, d)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LichData {
    pub soul_max: f64,
    /// Blizzard storm radius (cm), speed (cm/s) and channel length (s).
    pub storm_radius: f32,
    pub storm_speed: f32,
    pub storm_seconds: f32,
    pub storm_factor: f64,
    pub unfolded_factor: f64,
    pub barrier_seconds: f32,
    /// Share of max health healed per second inside the barrier.
    pub barrier_heal: f64,
    pub curse_seconds: f32,
    pub curse_factor: f64,
    pub curse_interval: f32,
    pub curse_heal_base: f64,
    pub curse_heal_factor: f64,
    pub torment_factor: f64,
    /// Frost Torment stage 2: the curse spreads within this radius (cm) after this delay (s).
    pub spread_radius: f32,
    pub spread_delay: f32,
    pub armor_shield: f64,
    pub armor_seconds: f32,
    pub armor_range: f32,
}


#[derive(Debug, Clone, PartialEq)]
pub struct RogueData {
    pub petrify_damage_res: f64,
    pub fade_seconds: f32,
    pub swift_seconds: f32,
    pub veil_seconds: f32,
    pub blades: u8,
    pub blades_factor: f64,
    pub blades_slow: f64,
    pub blades_slow_seconds: f32,
    /// Monsters see a stealthed Rogue within this planar distance and height difference (cm).
    pub stealth_reveal: f32,
    pub stealth_reveal_z: f32,
}

/// A heal of the Priest's GameplayEffect shape: base, additive part and attack-boost factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealShape {
    pub base: f64,
    pub add: f64,
    pub boost_factor: f64,
}

impl HealShape {
    /// `(base × (1 + k × attack_boost) × (1 + heal_boost) + add)`, times the crit damage rate on a crit.
    pub fn amount(&self, a: &AttackStats, crit: bool) -> f64 {
        let v = self.base * (1.0 + self.boost_factor * a.attack_boost_rate) * (1.0 + a.heal_boost_rate) + self.add;
        if crit { v * a.crit_damage_rate } else { v }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PriestData {
    pub mana_max: f64,
    pub mana_regen: f64,
    pub oracle_heal: HealShape,
    pub oracle_factor: f64,
    pub heal: [HealShape; 2],
    pub invincible_seconds: f32,
    /// Divine Protection and Guardian: charges and seconds per charge.
    pub protection_charges: u8,
    pub protection_cooldown: f32,
    pub guardian_factor: f64,
    pub guardian_seconds: f32,
    pub guardian_charges: u8,
    pub guardian_cooldown: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeathKnightData {
    pub soul_max: f64,
    pub ring_factor: f64,
    pub storm_factor: f64,
    /// Damage ring radius (cm) and seconds between pulses; slow length on activation (s).
    pub ring_radius: f32,
    pub ring_interval: f32,
    pub ring_slow_seconds: f32,
    pub chain_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DruidData {
    pub energy_max: f64,
    pub transform_cost: f64,
    /// Shadow energy lost per second in panther form.
    pub panther_drain: f64,
    /// Move speed bonus in panther form, cm/s.
    pub panther_speed: f32,
    pub pounce_factor: f64,
    pub treant_factor: f64,
    pub treant_seconds: f32,
    /// The treant's sight (its reach, cm) and swing (length, hit time; s).
    pub treant_sight: f32,
    pub treant_swing: (f32, f32),
    pub rejuvenation_factor: f64,
    /// Seconds per charge of Force of Nature and Rejuvenation.
    pub seeds_cooldown: f32,
    pub heal_cooldown: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassData {
    SwordMaster(SwordMasterData),
    Rogue(RogueData),
    Wizard(WizardData),
    Lich(LichData),
    Priest(PriestData),
    DeathKnight(DeathKnightData),
    Druid(DruidData),
}

impl ClassData {
    pub fn career(&self) -> EnRoleCareerType {
        match self {
            ClassData::SwordMaster(_) => EnRoleCareerType::Swordmaster,
            ClassData::Rogue(_) => EnRoleCareerType::Rogue,
            ClassData::Wizard(_) => EnRoleCareerType::Wizard,
            ClassData::Lich(_) => EnRoleCareerType::Lich,
            ClassData::Priest(_) => EnRoleCareerType::Priest,
            ClassData::DeathKnight(_) => EnRoleCareerType::Deathknight,
            ClassData::Druid(_) => EnRoleCareerType::DruidPanther,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerKind {
    Mana,
    SoulEnergy,
    /// The Druid's shadow energy (`natural_energy_*` attributes).
    NaturalEnergy,
}

/// A caster's skill resource: the Wizard's mana, the Lich's soul energy.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Power {
    pub kind: PowerKind,
    pub current: f64,
    pub max: f64,
    /// Regained per second.
    pub per_second: f64,
}

impl Power {
    /// A new character's power: full mana, or the starting soul energy.
    pub fn of(data: &ClassData) -> Option<Power> {
        match data {
            ClassData::Wizard(d) => Some(Power { kind: PowerKind::Mana, current: d.mana_max, max: d.mana_max, per_second: d.mana_regen }),
            ClassData::Lich(LichData { soul_max, .. }) | ClassData::DeathKnight(DeathKnightData { soul_max, .. }) => Some(Power { kind: PowerKind::SoulEnergy, current: tune::INITIAL_SOUL_ENERGY.min(*soul_max), max: *soul_max, per_second: 0.0 }),
            ClassData::Priest(d) => Some(Power { kind: PowerKind::Mana, current: d.mana_max, max: d.mana_max, per_second: d.mana_regen }),
            ClassData::Druid(d) => Some(Power { kind: PowerKind::NaturalEnergy, current: d.energy_max, max: d.energy_max, per_second: 0.0 }),
            _ => None,
        }
    }
}

/// Reads a class's skill values from the config tables and the decoded assets under `_data/json`.
pub fn load_class_skills(t: &ConfigTables, career: EnRoleCareerType) -> anyhow::Result<ClassData> {
    let hero = Some(db_formats::data_path("json/assets/Dungeonborne/Content/TGF/Character/Hero")).filter(|p| p.is_dir()).context("decoded hero assets not found under _data/json")?;
    let read = |rel: &str| -> anyhow::Result<serde_json::Value> {
        let p = hero.join(rel);
        Ok(serde_json::from_slice(&std::fs::read(&p).with_context(|| format!("{}", p.display()))?)?)
    };
    let table = read("CareerSkillDamageCurveTable.json")?;
    let curve_keys = |rel: &str| -> anyhow::Result<Vec<(f32, f32)>> {
        let v = read(rel)?;
        let keys = v[0]["Properties"]["FloatCurve"]["Keys"].as_array().with_context(|| format!("{rel} keys"))?;
        keys.iter().map(|k| Ok((k["Time"].as_f64().context("Time")? as f32, k["Value"].as_f64().context("Value")? as f32))).collect()
    };
    // A GameplayEffect's or ability's defaults (`Default__*`).
    let defaults_of = |rel: &str| -> anyhow::Result<serde_json::Value> {
        let v = read(rel)?;
        v.as_array().and_then(|a| a.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))).map(|o| o["Properties"].clone()).with_context(|| format!("{rel}: no defaults"))
    };
    // A cost effect's first modifier: its custom calculation's coefficient (`TgfMagicModMagnitudeCalculation`; 0 when unset).
    let ge_cost = |rel: &str| -> anyhow::Result<f64> {
        let p = defaults_of(rel)?;
        let m = &p["Modifiers"][0]["ModifierMagnitude"];
        anyhow::ensure!(m.is_object(), "{rel}: no modifier");
        Ok(m["CustomMagnitude"]["Coefficient"]["Value"].as_f64().unwrap_or(0.0))
    };
    let ge_duration = |rel: &str| -> anyhow::Result<f32> {
        let p = defaults_of(rel)?;
        Ok(p["DurationMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64().with_context(|| format!("{rel}: duration"))? as f32)
    };
    // A Blueprint object's property: the object whose name starts with `obj` (`Default__` for the class defaults).
    let blueprint = |rel: &str, obj: &str, key: &str| -> anyhow::Result<serde_json::Value> {
        let v = read(rel)?;
        v.as_array()
            .and_then(|a| a.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with(obj)) && o["Properties"].get(key).is_some()))
            .map(|o| o["Properties"][key].clone())
            .with_context(|| format!("{rel}: {obj} {key}"))
    };
    const Q2: &str = "Wizard/GameAbility/B_TGF_Track_Projectile_WizardQ2_Spline_New.json";
    const Q3: &str = "Wizard/GameAbility/B_TGF_Normal_Projectile_WizardQ3.json";
    let ga_default = |rel: &str, key: &str| -> anyhow::Result<f64> { defaults_of(rel)?[key].as_f64().with_context(|| format!("{rel}: {key}")) };
    let defaults = |rel: &str, key: &str| -> anyhow::Result<f64> {
        let v = read(rel)?;
        v.as_array().and_then(|a| a.iter().find_map(|o| o.get("Properties").and_then(|p| p.get(key)).and_then(serde_json::Value::as_f64))).with_context(|| format!("{rel} {key}"))
    };
    let sheet = || -> anyhow::Result<crate::attributes::AttributeSheet> { crate::attributes::class_sheet(t, crate::attributes::class_type_id(t, career)?, 1) };
    let curve = |name: &str| table[0]["Rows"][name]["Keys"][0]["Value"].as_f64().with_context(|| format!("curve row {name}"));
    let num = |id: &str| -> anyhow::Result<f64> {
        let row = t.excel_skill_num_exchange.iter().find(|r| r.id == id).with_context(|| format!("ExcelSkillNumExchange {id}"))?;
        let v = row.value.trim();
        Ok(match v.strip_suffix('%') {
            Some(p) => p.parse::<f64>()? / 100.0,
            None => v.parse::<f64>()?,
        })
    };
    Ok(match career {
        EnRoleCareerType::Swordmaster => {
            let rate = read("SwordMaster/GameAbility/SwordMasterQ_DamageRate.json")?;
            let keys = rate[0]["Properties"]["FloatCurve"]["Keys"].as_array().context("SwordMasterQ_DamageRate keys")?;
            let q_distance_rate = keys.iter().map(|k| Ok((k["Time"].as_f64().context("Time")? as f32, k["Value"].as_f64().context("Value")? as f32))).collect::<anyhow::Result<Vec<_>>>()?;
            ClassData::SwordMaster(SwordMasterData {
                q_swords: num("SwordMasterQ_SwordNum")? as u8,
                q_factor: curve("GE_TGF_Damage_Instant_Projectile_SwordMasterQ_Sub.MoveFactorRate")?,
                q_distance_rate,
                e_swords: num("SwordMasterE_SwordNum")? as u8,
                e_seconds: num("SwordMasterE_Time")? as f32,
                e_factor: curve("GE_TGF_Damage_Instant_Projectile_SwordMasterE.MoveFactorRate")?,
            })
        }
        EnRoleCareerType::Rogue => {
            let aura = read("Rogue/GameAbility/Aura_TGF_Rouge_Stealth.json")?;
            let props = aura.as_array().and_then(|a| a.iter().find_map(|o| o.get("Properties").filter(|p| p.get("MinDistance2D").is_some()))).context("stealth aura defaults")?;
            ClassData::Rogue(RogueData {
                petrify_damage_res: num("RogueQ_DamageRes")?,
                fade_seconds: num("RogueE_Time")? as f32,
                swift_seconds: num("RogueE2_Time")? as f32,
                veil_seconds: num("RogueE3_Time")? as f32,
                blades: num("MithrilRogueQ_Num")? as u8,
                blades_factor: curve("GE_TGF_Damage_Instant_Projectile_MithrilRogue_Q.MoveFactor")?,
                blades_slow: curve("GE_TGF_MithrilRogue_Skill1_Wound.MoveSpeedPercent")?,
                blades_slow_seconds: num("MithrilRogueQ_Time")? as f32,
                stealth_reveal: props["MinDistance2D"].as_f64().context("MinDistance2D")? as f32,
                stealth_reveal_z: props["MaxZDistance"].as_f64().context("MaxZDistance")? as f32,
            })
        }
        EnRoleCareerType::Wizard => {
            let s = sheet()?;
            ClassData::Wizard(WizardData {
                mana_max: s.get("affix_magic_max_magic_base"),
                mana_regen: s.get("affix_magic_magic_over_time"),
                q_small_factor: curve("GE_TGF_Damage_Instant_Projectile_WizardQ2_New.MoveFactorRate")?,
                q_big_factor: curve("GE_TGF_Damage_Instant_Projectile_WizardQ3.MoveFactorRate")?,
                q_speed: curve_keys("Wizard/GameAbility/WizardQSpeedCurve.json")?,
                q_distance_rate: curve_keys("Wizard/GameAbility/WizardQ_DamageRate.json")?,
                e_base: curve("GE_TGF_Damage_Wizard_E3_AddDamage.Damage.Base")?,
                q_cost: ge_cost("Wizard/GameAbility/GE_TGF_MagicCost_Wizard_Q.json")?,
                q_stage_costs: [ge_cost("Wizard/GameAbility/GE_TGF_MagicCost_Wizard_Q1.json")?, ge_cost("Wizard/GameAbility/GE_TGF_MagicCost_Wizard_Q2.json")?],
                q_cooldowns: [ge_duration("Wizard/GameAbility/GE_TGF_Cooldown_WizardQ_ApplyCD.json")?, ge_duration("Wizard/GameAbility/GE_TGF_Cooldown_WizardQ_ApplyCD_2.json")?],
                q_max_charge: ga_default("Wizard/GameAbility/GA_TGF_Wizard_Q_New.json", "MaxChargeTime")? as f32,
                q_refund_rate: ga_default("Wizard/GameAbility/GA_TGF_Wizard_Q_New.json", "ReturnEnergyRate")
                    .or_else(|_| ga_default("../../Ability/GameAbility/Skill/GA_TGF_MultiLevel_Charge_Skill_Base.json", "ReturnEnergyRate"))?,
                q_target_range: ga_default("Wizard/GameAbility/GA_TGF_Wizard_Q_New.json", "MaxDistance")? as f32,
                q_target_deg: ga_default("Wizard/GameAbility/GA_TGF_Wizard_Q_New.json", "MaxUnselectAngle")
                    .or_else(|_| ga_default("../../Ability/GameAbility/Skill/GA_TGF_Charge_Skill_Base.json", "MaxUnselectAngle"))? as f32,
                q_weapons: defaults_of("Wizard/GameAbility/GA_TGF_Wizard_Q_New.json")?["RestrictWeapon"]
                    .as_array()
                    .context("RestrictWeapon")?
                    .iter()
                    .filter_map(|w| w.as_str()?.strip_prefix("ETgfEquipSubType::TgfEquipSubTypeEquip").map(str::to_uppercase))
                    .collect(),
                e_cooldown: ge_duration("Wizard/GameAbility/Wizard_E2/GE_TGF_Cooldown_Wizard_E3.json")?,
                e_radius: ga_default("Wizard/GameAbility/Wizard_E2/GA_TGF_Wizard_E3.json", "Radius")? as f32,
                e_knockback: ga_default("Wizard/GameAbility/Wizard_E2/GA_TGF_Wizard_E3.json", "HitBackDistance")? as f32,
                e_knockback_secs: ga_default("Wizard/GameAbility/Wizard_E2/GA_TGF_Wizard_E3.json", "KnockBackDuration")? as f32,
                // `MoveSpeedRate` multiplied by the magnitude (op 4, HYPOTHESIS: a compound multiply; the skill text
                // says 85%, which matches); the effect's 1.3 s against the text's "1 second": the effect runs.
                e_slow: {
                    let slow = "Wizard/GameAbility/Wizard_E2/GE_TGF_Buff_Wizard_E3_SlowDown.json";
                    let m = defaults_of(slow)?["Modifiers"][0]["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64().context("slow magnitude")?;
                    (1.0 - m, ge_duration(slow)?)
                },
                q_small_delays: {
                    let data = defaults_of("Wizard/GameAbility/GA_TGF_Wizard_Q_New.json")?;
                    let rows = data["ProjectileSpawnData"].as_array().context("ProjectileSpawnData")?;
                    rows.iter()
                        .map(|r| r.as_object().and_then(|o| o.iter().find(|(k, _)| k.starts_with("SpawnDelay")).and_then(|(_, v)| v.as_f64())).map(|v| v as f32).context("SpawnDelay"))
                        .collect::<anyhow::Result<_>>()?
                },
                q_small_radius: blueprint(Q2, "BoxCollision_GEN_VARIABLE", "BoxExtent")?["X"].as_f64().context("small box")? as f32,
                q_small_burst: blueprint(Q2, "Default__", "ExplodeRadius")?.as_f64().context("small ExplodeRadius")? as f32,
                q_big_speed: {
                    let num = |obj: &str, key: &str| -> anyhow::Result<f32> { Ok(blueprint(Q3, obj, key)?.as_f64().with_context(|| format!("{Q3} {key}"))? as f32) };
                    let (v0, max, acc) = (num("ProjectileMovement_GEN_VARIABLE", "InitialSpeed")?, num("ProjectileMovement_GEN_VARIABLE", "MaxSpeed")?, num("Default__", "AccMag")?);
                    vec![(0.0, v0), ((max - v0) / acc, max)]
                },
                q_big_radius: blueprint(Q3, "BoxCollision_GEN_VARIABLE", "BoxExtent")?["X"].as_f64().context("big box")? as f32,
                q_big_burst: blueprint(Q3, "Default__", "ExplodeRadius")?.as_f64().context("big ExplodeRadius")? as f32,
                // The passive move's duration is a literal in the bytecode (0.15 s).
                q_big_knockback: (blueprint(Q3, "Default__", "Hitback Distance")?.as_f64().context("Hitback Distance")? as f32, 0.15),
                float_slow: {
                    let slow = "Wizard/GameAbility/GE_TGF_Buff_Floating_SpeedDown.json";
                    let m = defaults_of(slow)?["Modifiers"][0]["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64().context("floating slow")?;
                    (-m, ge_duration(slow)?)
                },
            })
        }
        EnRoleCareerType::Lich => {
            let s = sheet()?;
            let storm_radius = curve_keys("Lich/GameAbility/SnowStormAreaRadius.json")?;
            let storm_speed = curve_keys("Lich/GameAbility/SnowStormMoveSpeedFactor.json")?;
            let spread = "Lich/GameAbility/Lich_Q_DOT/GA_TGF_Lich_Q_DOT_Explode_Passive.json";
            ClassData::Lich(LichData {
                soul_max: s.get("affix_soul_energy_max_soul_energy_base"),
                storm_radius: storm_radius.first().context("SnowStormAreaRadius")?.1,
                storm_speed: storm_speed.first().context("SnowStormMoveSpeedFactor")?.1,
                storm_seconds: storm_radius.last().context("SnowStormAreaRadius")?.0,
                storm_factor: curve("GE_TGF_Damage_Instant_Lich_Q_Ice.MoveFactorRate")?,
                unfolded_factor: curve("GE_TGF_Damage_Instant_Lich_Q_Ice_Stage2.MoveFactorRate")?,
                barrier_seconds: curve("GE_Lich_FridgeHeal.ScalableFloatMagnitude")? as f32,
                barrier_heal: curve("GE_Lich_FridgeHeal.Heal.MaxHPRate")?,
                curse_seconds: curve("GE_Buff_DOT_Lich_Q_Ice.Duration")? as f32,
                curse_factor: curve("GE_TGF_Damage_OverTime_Lich_Q_Dot.MoveFactorRate")?,
                curse_interval: defaults("Lich/GameAbility/Lich_Q_DOT/GA_TGF_Lich_Q_DOT_Passive.json", "DamageInterval")? as f32,
                curse_heal_base: curve("GE_TGF_Heal_Lich_Q_DOT.Base")?,
                curse_heal_factor: curve("GE_TGF_Heal_Lich_Q_DOT.MoveFactor")?,
                torment_factor: curve("GE_TGF_Damage_Instant_Lich_Q_Dot.MoveFactorRate")?,
                spread_radius: defaults(spread, "ExplodeRadius")? as f32,
                spread_delay: defaults(spread, "ExplodeDelayTime")? as f32,
                armor_shield: curve("GE_TGF_Buff_Lich_E_IceArmor_AddShield_1.MoveFactor")?,
                armor_seconds: curve("GE_TGF_Buff_Lich_E_IceArmor.Duration")? as f32,
                armor_range: defaults("Lich/GameAbility/Lich_E_IceArmor/GA_TGF_Lich_E_IceArmor_Passive.json", "CounterRange")? as f32,
            })
        }
        EnRoleCareerType::Priest => {
            let s = sheet()?;
            let shape = |base: &str, add: &str, k: Option<&str>| -> anyhow::Result<HealShape> { Ok(HealShape { base: curve(base)?, add: curve(add)?, boost_factor: k.map_or(Ok(0.6), curve)? }) };
            let protection = "Priest/GameAbility/Priest_E_Invincible/GA_TGF_Priest_E_DoubleCharge.json";
            let guardian = "Priest/GameAbility/Priest_E_ShieldAOE/GA_TGF_Priest_E_ShieldAOE_DoubleCharge.json";
            ClassData::Priest(PriestData {
                mana_max: s.get("affix_magic_max_magic_base"),
                mana_regen: s.get("affix_magic_magic_over_time"),
                oracle_heal: shape("GE_TGF_Heal_Priest_Q_AOE_Lv1.Heal.Base", "GE_TGF_Heal_Priest_Q_AOE_Lv1.Heal.AdditiveHeal", Some("GE_TGF_Heal_Priest_Q_AOE_Lv1.Heal.ArttackBoostRateToBase"))?,
                oracle_factor: curve("GE_TGF_Damage_Instant_Priest_Q_AOE.MoveFactorRate")?,
                // The `_Single` tooltip scripts use an attack-boost factor of 0.6.
                heal: [shape("GE_TGF_HealSingle_Basic_Priest_Q.Base", "GE_TGF_HealSingle_Basic_Priest_Q.Additive", None)?, shape("GE_TGF_HealSingle_Basic_Priest_Q1.Base", "GE_TGF_HealSingle_Basic_Priest_Q1.Additive", None)?],
                invincible_seconds: curve("GE_TGF_Buff_Priest_Invincible.ScalableFloatMagnitude")? as f32,
                protection_charges: defaults(protection, "DoubleChargeCount")? as u8,
                protection_cooldown: defaults(protection, "BaseCoolDown")? as f32,
                guardian_factor: curve("GE_TGF_Buff_Priest_E_ShieldAOE_AddShield_1.MoveFactor")?,
                guardian_seconds: curve("GE_TGF_Buff_Priest_E_ShieldAOE_AddShield.Duration")? as f32,
                guardian_charges: defaults(guardian, "DoubleChargeCount")? as u8,
                guardian_cooldown: defaults(guardian, "BaseCoolDown")? as f32,
            })
        }
        EnRoleCareerType::Deathknight => {
            let s = sheet()?;
            let ring = "DeathKnight/GameAbility/GA_TGF_DeathKnightQ_Passive.json";
            ClassData::DeathKnight(DeathKnightData {
                soul_max: s.get("affix_soul_energy_max_soul_energy_base"),
                ring_factor: curve("GE_TGF_Damage_Instant_DeathKnightQ.MoveFactorRate")?,
                storm_factor: curve("GE_TGF_Damage_Instant_DeathKnightQ.MoveFactorRate2")?,
                ring_radius: defaults(ring, "Radius")? as f32,
                ring_interval: defaults(ring, "DamageInterval")? as f32,
                ring_slow_seconds: num("DeathKnightQ_Time")? as f32,
                chain_factor: curve("GE_TGF_Damage_Instant_DeathKnightE.MoveFactorRate")?,
            })
        }
        EnRoleCareerType::DruidPanther => {
            let s = sheet()?;
            let dryad = crate::monsters::load_monster_db().context("MonsterAttributeCurve not decoded")?;
            let dryad = dryad.get("DryadSummoned").context("no DryadSummoned row")?;
            let kit = crate::monsters::load_monster_kit("Treant", "M_Treant")?;
            let swing = kit.attacks.iter().find(|a| a.name == "AM_Treant_Attack2").context("treant AM_Treant_Attack2")?;
            let ga = |name: &str| defaults(&format!("DruidPanther/GameAbility/{name}.json"), "BaseCoolDown");
            ClassData::Druid(DruidData {
                energy_max: s.get("affix_natural_energy_max_natural_energy_base"),
                transform_cost: curve("GE_TGF_NaturalEnergyCost_DruidPanther_ActiveQ.CurNaturalEnergy")?,
                panther_drain: curve("GE_TGF_DruidPanther_PantherNEReduce.CurNaturalEnergy")?,
                panther_speed: curve("GE_TGF_DruidPanther_Q_Transform.MoveSpeedBase")? as f32,
                pounce_factor: curve("GE_TGF_Damage_Instant_DruidPanther_E_Panther.MoveFactorRate")?,
                treant_factor: curve("GE_TGF_Damage_InstantDruidPanther_E_Human.MoveFactorRate")?,
                treant_seconds: num("DruidE_Time")? as f32,
                treant_sight: dryad.senses().sight,
                treant_swing: (swing.length, swing.windows.first().context("treant hit window")?.0),
                rejuvenation_factor: curve("GE_TGF_DruidPanther_E_Heal.MoveFactorRate")?,
                seeds_cooldown: ga("GA_TGF_DruidPanther_DoubleSeeds")? as f32,
                heal_cooldown: ga("GA_TGF_DruidPanther_DoubleHeal")? as f32,
            })
        }
        other => anyhow::bail!("no skills for {other:?} yet"),
    })
}

/// The skills and their state of a non-Fighter class.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ClassKit {
    pub data: ClassData,
    pub slots: [SkillKind; 2],
    /// Ticks until each skill is ready again (absent = ready).
    pub cooldowns: BTreeMap<SkillKind, u32>,
    /// A skill in its preparation: (skill, ticks left).
    pub preparing: Option<(SkillKind, u32)>,
    /// Blade Waltz: ticks left and ticks since it started.
    pub waltz: Option<(u32, u32)>,
    /// Petrifying Poison armed: ticks left.
    pub poison: Option<u32>,
    /// A charged skill being held: (skill, ticks held).
    pub charge: Option<(SkillKind, u32)>,
    /// The Lich's Blizzard.
    pub blizzard: Option<Blizzard>,
    /// Frost Torment stage 2 targets whose curse spreads, with ticks to go.
    pub spreads: Vec<(SimId, u32)>,
    /// The Priest's Oracle: where it goes off and ticks to go.
    pub oracle: Option<(Vec3, u32)>,
    /// The DeathKnight's Wraith Ring or Soul Storm: (skill, ticks since it started, ticks left; Soul
    /// Storm runs until switched off or out of soul energy).
    pub aura: Option<(SkillKind, u32, u32)>,
    /// Skills with charges: (charges, ticks until the next one).
    pub charges: BTreeMap<SkillKind, (u8, u32)>,
    /// The Druid is in panther form.
    pub panther: bool,
    /// Shadow Pounce in the air: ticks to landing.
    pub pounce: Option<u32>,
    /// The Wizard's air jump ("floating") granted by a full Pyroblast charge and not yet used.
    pub floating: bool,
    /// Shots still to go out: Pyroblast's staggered fireballs.
    pub volley: Volley,
}

/// The Druid's treant (Force of Nature): stands where it was summoned and swings at enemies in sight.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Treant {
    pub owner: SimId,
    /// Ticks before it goes.
    pub left: u32,
    /// Ticks before it may start the next swing.
    pub next: u32,
    /// A swing under way: (target, ticks to the hit).
    pub swing: Option<(SimId, u32)>,
    /// Shadow Pounce marked this enemy: the treant goes for it first.
    pub marked: Option<SimId>,
}

/// A Blizzard: a storm moving with the Lich's aim while channelled, then an unfolded blizzard in place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blizzard {
    /// Centre on the floor.
    pub pos: Vec3,
    /// Ticks since it started.
    pub age: u32,
    /// Channel ticks left (0 once unfolded).
    pub channel: u32,
    /// Unfolded ticks left.
    pub unfolded: u32,
}

impl ClassKit {
    /// What pressing `skill` costs in the kit's resource (for the HUD's not-enough mask).
    pub fn press_cost(&self, skill: SkillKind) -> f64 {
        cost_of(&self.data, skill)
    }

    pub fn new(data: ClassData) -> Self {
        let options = SkillKind::options(data.career());
        let mut charges = BTreeMap::new();
        match &data {
            ClassData::Priest(d) => {
                charges.insert(SkillKind::DivineProtection, (d.protection_charges, 0));
                charges.insert(SkillKind::Guardian, (d.guardian_charges, 0));
            }
            ClassData::Druid(_) => {
                charges.insert(SkillKind::ForceOfNature, (tune::DRUID_E_CHARGES, 0));
                charges.insert(SkillKind::Rejuvenation, (tune::DRUID_E_CHARGES, 0));
            }
            _ => {}
        }
        ClassKit { data, slots: [options[0][0], options[1][0]], cooldowns: BTreeMap::new(), preparing: None, waltz: None, poison: None, charge: None, blizzard: None, spreads: Vec::new(), oracle: None, aura: None, charges, panther: false, pounce: None, floating: false, volley: Vec::new() }
    }

    /// Ticks until the skill can be used (a skill with charges: until its next charge, if it has none).
    pub fn cooldown(&self, skill: SkillKind) -> u32 {
        match self.charges.get(&skill) {
            Some(&(0, next)) => next.max(1),
            Some(_) => 0,
            None => self.cooldowns.get(&skill).copied().unwrap_or(0),
        }
    }

    /// (charges, max charges, ticks to the next) of a skill with charges.
    pub fn charges_of(&self, skill: SkillKind) -> Option<(u8, u8, u32)> {
        let &(n, next) = self.charges.get(&skill)?;
        let (max, _) = self.charge_rule(skill)?;
        Some((n, max, next))
    }

    /// Max charges and seconds per charge.
    fn charge_rule(&self, skill: SkillKind) -> Option<(u8, f32)> {
        match (&self.data, skill) {
            (ClassData::Priest(d), SkillKind::DivineProtection) => Some((d.protection_charges, d.protection_cooldown)),
            (ClassData::Priest(d), SkillKind::Guardian) => Some((d.guardian_charges, d.guardian_cooldown)),
            (ClassData::Druid(d), SkillKind::ForceOfNature) => Some((tune::DRUID_E_CHARGES, d.seeds_cooldown)),
            (ClassData::Druid(d), SkillKind::Rejuvenation) => Some((tune::DRUID_E_CHARGES, d.heal_cooldown)),
            _ => None,
        }
    }

    /// Uses one charge (starting the recharge if it isn't running).
    fn spend_charge(&mut self, skill: SkillKind) {
        let Some((_, secs)) = self.charge_rule(skill) else { return };
        if let Some(c) = self.charges.get_mut(&skill) {
            c.0 = c.0.saturating_sub(1);
            if c.1 == 0 {
                c.1 = ticks(secs).max(1);
            }
        }
    }
}

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

fn reject(world: &mut World, id: SimId, skill: SkillKind) {
    world.resource_mut::<SimEvents>().push(SimEvent::SkillRejected { id, skill });
}

/// What a skill costs on the press (a charged skill's stage cost is checked again on release).
fn cost_of(data: &ClassData, skill: SkillKind) -> f64 {
    if let ClassData::Wizard(d) = data {
        match skill {
            SkillKind::Pyroblast => return d.q_cost,
            SkillKind::FlameShock => return 0.0,
            _ => {}
        }
    }
    match skill {
        SkillKind::Blizzard => tune::BLIZZARD_COST,
        SkillKind::FrostTorment => tune::FROST_TORMENT_COST[0],
        SkillKind::IceBarrier => tune::ICE_BARRIER_COST,
        SkillKind::IceArmor => tune::ICE_ARMOR_COST,
        SkillKind::Oracle => tune::ORACLE_COST,
        SkillKind::HealSingle => tune::HEAL_COST[0],
        SkillKind::DivineProtection => tune::DIVINE_PROTECTION_COST,
        SkillKind::Guardian => tune::GUARDIAN_COST,
        SkillKind::WraithRing => tune::WRAITH_RING_COST,
        SkillKind::SoulStorm => tune::SOUL_STORM_MIN,
        SkillKind::ShadowPounce => tune::POUNCE_COST,
        _ => 0.0,
    }
}

fn power(world: &World, e: Entity) -> f64 {
    world.get::<Power>(e).map_or(0.0, |p| p.current)
}

fn spend(world: &mut World, e: Entity, cost: f64) {
    if let Some(mut p) = world.get_mut::<Power>(e)
        && cost > 0.0
    {
        p.current = (p.current - cost).max(0.0);
    }
}

fn write_kit(world: &mut World, e: Entity, kit: ClassKit) {
    if let Some(mut k) = world.get_mut::<ClassKit>(e) {
        *k = kit;
    }
}

/// A `ClassKit` skill: refused if not in the loadout, on cooldown, while preparing or charging
/// another skill, stunned, or without the mana or soul energy it costs. Pressing Ice Barrier inside
/// the barrier ends it; pressing Blizzard while channelling unfolds the storm.
pub(crate) fn use_skill(world: &mut World, id: SimId, e: Entity, skill: SkillKind) {
    let Some(mut kit) = world.get::<ClassKit>(e).cloned() else { return reject(world, id, skill) };
    let buffs = world.get::<Buffs>(e).cloned().unwrap_or_default();
    // Panther form: the E key is Shadow Pounce, whatever the loadout's E; human form has no Pounce.
    let skill = match (&kit.data, kit.panther, skill.slot()) {
        (ClassData::Druid(_), true, 1) => SkillKind::ShadowPounce,
        _ if skill == SkillKind::ShadowPounce => return reject(world, id, skill),
        _ => skill,
    };
    if skill == SkillKind::ShadowPounce {
        if kit.cooldown(skill) > 0 || kit.pounce.is_some() || buffs.stunned() || power(world, e) < tune::POUNCE_COST {
            return reject(world, id, skill);
        }
        spend(world, e, tune::POUNCE_COST);
        let aim = world.get::<Aim>(e).copied().unwrap_or_default();
        let velocity = aim.forward_2d() * tune::POUNCE_DISTANCE / tune::POUNCE_SECONDS;
        let flight = ticks(tune::POUNCE_SECONDS).max(1);
        crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Pull(velocity), skill, ticks_left: flight, age: 0 });
        kit.pounce = Some(flight);
        kit.cooldowns.insert(skill, ticks(tune::POUNCE_COOLDOWN * crate::gear_triggers::cooldown_scale(world, e)).max(1));
        write_kit(world, e, kit);
        world.resource_mut::<SimEvents>().push(SimEvent::SkillStarted { id, skill });
        crate::gear_triggers::on_skill_cast(world, e);
        return;
    }
    // Wild Awakening again: Return to Reason (free).
    if skill == SkillKind::WildAwakening && kit.panther {
        leave_panther(world, e, &mut kit);
        write_kit(world, e, kit);
        world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
        return;
    }
    if kit.slots[skill.slot()] == skill {
        if skill == SkillKind::IceBarrier && buffs.encased() {
            if let Some(mut b) = world.get_mut::<Buffs>(e) {
                b.0.retain(|x| !matches!(x.kind, BuffKind::IceBarrier { .. }));
            }
            world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
            return;
        }
        if skill == SkillKind::SoulStorm && kit.aura.is_some_and(|a| a.0 == SkillKind::SoulStorm) {
            kit.aura = None;
            write_kit(world, e, kit);
            world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
            return;
        }
        if skill == SkillKind::Blizzard
            && let Some(mut b) = kit.blizzard.filter(|b| b.channel > 0)
        {
            unfold(world, e, &mut b);
            kit.blizzard = Some(b);
            write_kit(world, e, kit);
            world.resource_mut::<SimEvents>().push(SimEvent::SkillCast { id, skill, stage: 2 });
            return;
        }
    }
    // Flame Shock doesn't interrupt the current action (skill text): it goes off during a charge.
    let busy = kit.preparing.is_some() || (kit.charge.is_some() && skill != SkillKind::FlameShock) || (skill.slot() == 0 && (kit.blizzard.is_some_and(|b| b.channel > 0) || kit.oracle.is_some() || kit.aura.is_some()));
    let cost = cost_of(&kit.data, skill);
    // Pyroblast needs a staff or a relic worn (`RestrictWeapon`; a Wizard may also carry a sword or a dagger).
    let armed = match (&kit.data, skill) {
        (ClassData::Wizard(d), SkillKind::Pyroblast) => wears_one_of(world, e, &d.q_weapons),
        _ => true,
    };
    if kit.slots[skill.slot()] != skill || kit.cooldown(skill) > 0 || busy || buffs.stunned() || power(world, e) < cost || !armed {
        return reject(world, id, skill);
    }
    let mut paid = cost;
    let cooldown = match (&kit.data, skill) {
        // Psychic Flying Swords: held, the swords summoned while it lasts; launched and cooled down on release.
        (ClassData::SwordMaster(_), SkillKind::FlyingSwords) => {
            kit.charge = Some((skill, 0));
            0.0
        }
        (ClassData::SwordMaster(d), SkillKind::BladeWaltz) => {
            kit.waltz = Some((ticks(d.e_seconds), 0));
            tune::BLADE_WALTZ_COOLDOWN
        }
        (ClassData::Rogue(_), SkillKind::PetrifyingPoison) => {
            kit.poison = Some(ticks(tune::POISON_WINDOW));
            tune::PETRIFYING_POISON_COOLDOWN
        }
        (ClassData::Rogue(d), SkillKind::FlyingBlades) => {
            crate::buffs::cleanse(world, e);
            flying_blades(world, id, e, d);
            tune::FLYING_BLADES_COOLDOWN
        }
        (ClassData::Rogue(d), SkillKind::SwiftConcealment) => {
            stealth(world, e, skill, d.swift_seconds);
            tune::SWIFT_CONCEALMENT_COOLDOWN
        }
        (ClassData::Rogue(_), SkillKind::FadeIntoVoid) => {
            kit.preparing = Some((skill, ticks(STEALTH_PREPARATION)));
            tune::FADE_INTO_VOID_COOLDOWN
        }
        (ClassData::Rogue(_), SkillKind::ShadowVeil) => {
            kit.preparing = Some((skill, ticks(STEALTH_PREPARATION)));
            tune::SHADOW_VEIL_COOLDOWN
        }
        // Pyroblast: paid on the press (its ability's cost), cooled down on release.
        (ClassData::Wizard(_), SkillKind::Pyroblast) => {
            kit.charge = Some((skill, 0));
            0.0
        }
        // Other charged skills: paid and cooled down on release.
        (ClassData::Lich(_), SkillKind::FrostTorment) | (ClassData::Priest(_), SkillKind::HealSingle) => {
            kit.charge = Some((skill, 0));
            paid = 0.0;
            0.0
        }
        (ClassData::Wizard(d), SkillKind::FlameShock) => {
            flame_shock(world, id, e, d);
            d.e_cooldown
        }
        (ClassData::Lich(d), SkillKind::Blizzard) => {
            // Where the selection indicator stood when Q came up (`GA_TGF_Lich_Q`: `GetSelectLocation`).
            let pos = aim_floor_point(world, e, tune::BLIZZARD_SELECT_RANGE);
            let channel = ticks(d.storm_seconds).max(1);
            kit.blizzard = Some(Blizzard { pos, age: 0, channel, unfolded: 0 });
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Rooted, skill, ticks_left: channel + 1, age: 0 });
            tune::BLIZZARD_COOLDOWN
        }
        (ClassData::Lich(d), SkillKind::IceBarrier) => {
            let (max, boost) = (world.get::<Health>(e).map_or(0.0, |h| h.max), world.get::<Combatant>(e).map_or(0.0, |c| c.attack.heal_boost_rate));
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::IceBarrier { heal: d.barrier_heal * max * (1.0 + boost) }, skill, ticks_left: ticks(d.barrier_seconds), age: 0 });
            tune::ICE_BARRIER_COOLDOWN
        }
        (ClassData::Lich(d), SkillKind::IceArmor) => {
            // Cast on an ally with F in the game; single player casts it on the Lich.
            let attack = world.get::<Combatant>(e).map(|c| crate::buffs::attack_of(world, e, c.attack));
            let shield = attack.map_or(0.0, |a| d.armor_shield * tooltip::shield_weapon(&a));
            crate::world_items::give_shield(world, e, shield, d.armor_seconds);
            let (slow, seconds) = ICE_ARMOR_SLOW;
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::IceArmor { range: d.armor_range, slow, seconds }, skill, ticks_left: ticks(d.armor_seconds), age: 0 });
            tune::ICE_ARMOR_COOLDOWN
        }
        (ClassData::Priest(_), SkillKind::Oracle) => {
            let pos = aim_floor_point(world, e, tune::ORACLE_RANGE);
            let delay = ticks(tune::ORACLE_DELAY).max(1);
            kit.oracle = Some((pos, delay));
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Rooted, skill, ticks_left: delay + 1, age: 0 });
            tune::ORACLE_COOLDOWN
        }
        (ClassData::Priest(d), SkillKind::DivineProtection) => {
            // Cast on an ally with F in the game; single player casts it on the Priest.
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Invincible, skill, ticks_left: ticks(d.invincible_seconds), age: 0 });
            kit.spend_charge(skill);
            0.0
        }
        (ClassData::Priest(d), SkillKind::Guardian) => {
            // The Priest and nearby allies; single player: the Priest.
            let attack = world.get::<Combatant>(e).map(|c| crate::buffs::attack_of(world, e, c.attack));
            let shield = attack.map_or(0.0, |a| d.guardian_factor * tooltip::shield_weapon(&a));
            crate::world_items::give_shield(world, e, shield, d.guardian_seconds);
            kit.spend_charge(skill);
            0.0
        }
        (ClassData::DeathKnight(d), SkillKind::WraithRing) => {
            let (rate, _) = WRAITH_RING_SLOW;
            if let Some(from) = world.get::<Position>(e).map(|p| p.0) {
                for (_, te, to) in others(world, id) {
                    if in_ring(from, to, d.ring_radius) {
                        slow(world, te, skill, rate, d.ring_slow_seconds);
                    }
                }
            }
            kit.aura = Some((skill, 0, ticks(tune::WRAITH_RING_SECONDS)));
            tune::WRAITH_RING_COOLDOWN
        }
        (ClassData::DeathKnight(_), SkillKind::SoulStorm) => {
            // Switched on: drains while it runs instead of a cost.
            kit.aura = Some((skill, 0, u32::MAX));
            paid = 0.0;
            0.0
        }
        (ClassData::DeathKnight(d), SkillKind::SoulChain) => {
            soul_chain(world, id, e, d);
            tune::SOUL_CHAIN_COOLDOWN
        }
        (ClassData::Druid(d), SkillKind::WildAwakening) => {
            // Paid now; the panther comes at the change's end, rooted until then.
            paid = d.transform_cost;
            let n = ticks(tune::PANTHER_CHANGE_SECONDS);
            kit.preparing = Some((skill, n));
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Rooted, skill, ticks_left: n, age: 0 });
            0.0
        }
        // Force of Nature: the seed is aimed until confirmed (a charge spent then).
        (ClassData::Druid(_), SkillKind::ForceOfNature) => {
            kit.charge = Some((skill, 0));
            0.0
        }
        (ClassData::Druid(d), SkillKind::Rejuvenation) => {
            // On an ally with F in the game; single player: the Druid.
            let attack = world.get::<Combatant>(e).map(|c| crate::buffs::attack_of(world, e, c.attack));
            let per_second = attack.map_or(0.0, |a| d.rejuvenation_factor * tooltip::heal_lich(&a));
            crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Regen(per_second), skill, ticks_left: ticks(tune::REJUVENATION_SECONDS), age: 0 });
            kit.spend_charge(skill);
            0.0
        }
        _ => return reject(world, id, skill),
    };
    spend(world, e, paid);
    if cooldown > 0.0 {
        kit.cooldowns.insert(skill, ticks(cooldown * crate::gear_triggers::cooldown_scale(world, e)).max(1));
    }
    write_kit(world, e, kit);
    world.resource_mut::<SimEvents>().push(SimEvent::SkillStarted { id, skill });
    crate::gear_triggers::on_skill_cast(world, e);
}

/// Lets go of a charged skill: it fires at the stage reached (stage 2 falls back to stage 1 without
/// the cost or, for Frost Torment, without a target); before stage 1 it is cancelled for free.
pub(crate) fn release_skill(world: &mut World, id: SimId, skill: SkillKind) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    let Some(mut kit) = world.get::<ClassKit>(e).cloned() else { return };
    let Some((charging, held)) = kit.charge.filter(|c| c.0 == skill) else { return };
    // Force of Nature: the seed planted at the aim.
    if let (ClassData::Druid(d), SkillKind::ForceOfNature) = (&kit.data, charging) {
        let d = d.clone();
        kit.charge = None;
        let can = world.get::<Health>(e).is_some_and(|h| h.current > 0.0) && !world.get::<Buffs>(e).is_some_and(Buffs::stunned);
        if can {
            let at = aim_floor_point(world, e, FORCE_OF_NATURE_RANGE);
            summon_treant(world, id, at + Vec3::Z * CAPSULE_HEIGHT * 0.5, &d);
            kit.spend_charge(skill);
        }
        write_kit(world, e, kit);
        let mut events = world.resource_mut::<SimEvents>();
        if can {
            events.push(SimEvent::SkillCast { id, skill, stage: 1 });
        }
        events.push(SimEvent::SkillEnded { id, skill });
        return;
    }
    // Flying Swords: the summoned swords go, one at a time.
    if let (ClassData::SwordMaster(d), SkillKind::FlyingSwords) = (&kit.data, charging) {
        let d = d.clone();
        kit.charge = None;
        let can = world.get::<Health>(e).is_some_and(|h| h.current > 0.0) && !world.get::<Buffs>(e).is_some_and(Buffs::stunned);
        let n = swords_summoned(held, d.q_swords);
        if can {
            kit.volley.extend(flying_swords(world, id, e, &d, n));
            let scale = crate::gear_triggers::cooldown_scale(world, e);
            kit.cooldowns.insert(charging, ticks(FLYING_SWORDS_COOLDOWNS[usize::from(n)] * scale).max(1));
        }
        write_kit(world, e, kit);
        let mut events = world.resource_mut::<SimEvents>();
        if can {
            events.push(SimEvent::SkillCast { id, skill, stage: n });
        }
        events.push(SimEvent::SkillEnded { id, skill });
        return;
    }
    let [s1, s2] = tune::CHARGE_STAGES.map(ticks);
    let reached: u8 = if held >= s2 { 2 } else if held >= s1 { 1 } else { 0 };
    // Pyroblast can't be let go before its first stage (`BlockRelease` at charge level 1): it keeps charging.
    let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
    if charging == SkillKind::Pyroblast && reached == 0 && alive {
        return;
    }
    kit.charge = None;
    let costs = match (&kit.data, charging) {
        (ClassData::Wizard(d), SkillKind::Pyroblast) => d.q_stage_costs,
        (_, SkillKind::HealSingle) => tune::HEAL_COST,
        _ => tune::FROST_TORMENT_COST,
    };
    let have = power(world, e);
    let mut stage = (1..=reached).rev().find(|&s| have >= costs[usize::from(s) - 1]).unwrap_or(0);
    let target = match &kit.data {
        ClassData::Wizard(d) => pick_target(world, id, e, d.q_target_range, d.q_target_deg),
        _ => front_target(world, id, e),
    };
    if charging == SkillKind::FrostTorment && stage == 2 && target.is_none() {
        stage = 1;
    }
    if stage == 0 || world.get::<Buffs>(e).is_some_and(Buffs::stunned) {
        write_kit(world, e, kit);
        world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
        return;
    }
    let cooldown = match &kit.data {
        ClassData::Wizard(d) => {
            kit.volley.extend(pyroblast(world, id, e, d, stage, target));
            d.q_cooldowns[usize::from(stage) - 1]
        }
        ClassData::Lich(d) => {
            if let Some(spread) = frost_torment(world, id, e, d, stage, target) {
                kit.spreads.push(spread);
            }
            tune::FROST_TORMENT_COOLDOWN
        }
        ClassData::Priest(d) => {
            // On an ally with F in the game; single player heals the Priest.
            let shape = d.heal[usize::from(stage) - 1];
            heal_with(world, id, e, shape);
            tune::HEAL_COOLDOWN
        }
        _ => 0.0,
    };
    spend(world, e, costs[usize::from(stage) - 1]);
    kit.cooldowns.insert(charging, ticks(cooldown * crate::gear_triggers::cooldown_scale(world, e)).max(1));
    write_kit(world, e, kit);
    let mut events = world.resource_mut::<SimEvents>();
    events.push(SimEvent::SkillCast { id, skill, stage });
    events.push(SimEvent::SkillEnded { id, skill });
}

/// Cancels a charged skill without firing it (Pyroblast: the right button, its `CancelInputTags`). What the press
/// paid comes back (`ApplyEffectOnCastFailed` with `bReturnManaWithFailed`): Pyroblast at its `ReturnEnergyRate`,
/// the others at `tune::CHARGE_CANCEL_REFUND`. A stun ends a charge through `release_skill`, which returns nothing
/// (the bytecode skips the refund while `InInterrupting`).
pub(crate) fn cancel_skill(world: &mut World, id: SimId, skill: SkillKind) {
    let Some(e) = world.resource::<SimIndex>().get(id) else { return };
    let Some(mut kit) = world.get::<ClassKit>(e).cloned() else { return };
    if kit.charge.is_none_or(|c| c.0 != skill) {
        return;
    }
    kit.charge = None;
    let rate = match &kit.data {
        ClassData::Wizard(d) if skill == SkillKind::Pyroblast => d.q_refund_rate,
        _ => tune::CHARGE_CANCEL_REFUND,
    };
    let refund = cost_of(&kit.data, skill) * rate;
    if refund > 0.0
        && let Some(mut p) = world.get_mut::<Power>(e)
    {
        p.current = (p.current + refund).min(p.max);
    }
    write_kit(world, e, kit);
    world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
}

/// The nearest living combatant within the caster targeting range and cone of the facing.
fn front_target(world: &World, id: SimId, e: Entity) -> Option<(SimId, Entity, Vec3)> {
    pick_target(world, id, e, tune::CASTER_TARGET_RANGE, tune::CASTER_TARGET_HALF_ANGLE_DEG)
}

/// The nearest living combatant within `range` (cm) and `half_deg` of the facing.
fn pick_target(world: &World, id: SimId, e: Entity, range: f32, half_deg: f32) -> Option<(SimId, Entity, Vec3)> {
    let (from, fwd) = (world.get::<Position>(e)?.0, world.get::<Aim>(e).copied().unwrap_or_default().forward_2d());
    others(world, id)
        .into_iter()
        .filter(|(_, _, p)| {
            let v = (*p - from).truncate();
            v.length() <= range && fwd.dot(v.normalize_or_zero()) >= half_deg.to_radians().cos()
        })
        .min_by(|a, b| a.2.distance(from).total_cmp(&b.2.distance(from)).then(a.0.cmp(&b.0)))
}

/// Whether the actor wears a weapon of one of these equipment sub-types (`STAFF`, …). Actors without an inventory or
/// game data (tests, monsters) pass.
fn wears_one_of(world: &World, e: Entity, kinds: &[String]) -> bool {
    let (Some(data), Some(inv)) = (world.get_resource::<crate::GameData>(), world.get::<crate::Inventory>(e)) else { return true };
    let t = data.tables;
    inv.worn.values().any(|i| {
        let equip = t.uesource_item_type.iter().find(|u| u.type_id == i.item.item_id).map(|u| u.ue_source_equipment_equip_id);
        let sub = equip.and_then(|q| t.ue_source_equipment.iter().find(|r| r.equip_id == q)).and_then(|r| r.equip_subtype.name());
        sub.and_then(|s| s.strip_prefix("EN_GEAR_EQUIP_SUBTYPE_")).is_some_and(|s| kinds.iter().any(|k| k == s))
    })
}

/// Where the actor's aim meets the floor (from eye height), within `range`.
fn aim_floor_point(world: &World, e: Entity, range: f32) -> Vec3 {
    let (pos, aim) = (world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0), world.get::<Aim>(e).copied().unwrap_or_default());
    aim_floor(pos, aim, range)
}

/// Where an aim from capsule centre `pos` meets the floor (from eye height), within `range` (the client's ground
/// indicators use it too).
pub fn aim_floor(pos: Vec3, aim: Aim, range: f32) -> Vec3 {
    let eye = CAPSULE_HEIGHT * 0.5 + tune::SHOT_HEIGHT;
    let d = if aim.pitch < -0.01 { eye / (-aim.pitch).tan() } else { range };
    (pos.truncate() + aim.forward_2d() * d.min(range)).extend(pos.z - CAPSULE_HEIGHT * 0.5)
}

/// Within the DeathKnight's ring (planar reach from the capsule, height).
fn in_ring(from: Vec3, to: Vec3, radius: f32) -> bool {
    (to - from).truncate().length() - CAPSULE_RADIUS <= radius && (to - from).z.abs() <= tune::AURA_HEIGHT
}

fn leave_panther(world: &mut World, e: Entity, kit: &mut ClassKit) {
    kit.panther = false;
    if let Some(mut b) = world.get_mut::<Buffs>(e) {
        b.0.retain(|x| !(matches!(x.kind, BuffKind::MoveSpeed(_)) && x.skill == SkillKind::WildAwakening));
    }
}

/// A weapon hit by `e` that did damage: in panther form it restores 20 shadow energy (skill text).
pub(crate) fn weapon_hit(world: &mut World, e: Entity) {
    if world.get::<ClassKit>(e).is_some_and(|k| k.panther) {
        restore(world, e, PANTHER_HIT_ENERGY);
    }
}

fn restore(world: &mut World, e: Entity, amount: f64) {
    if let Some(mut p) = world.get_mut::<Power>(e) {
        p.current = (p.current + amount).min(p.max);
    }
}

fn summon_treant(world: &mut World, owner: SimId, at: Vec3, d: &DruidData) {
    let id = world.resource_mut::<crate::id::SimIdAllocator>().alloc();
    let t = Treant { owner, left: ticks(d.treant_seconds), next: 0, swing: None, marked: None };
    let e = world.spawn((id, crate::command::Kind(crate::command::ActorKind::Summon), Position(at), t)).id();
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind: crate::command::ActorKind::Summon });
}

/// Treants: count down, pick the marked or nearest enemy in sight, swing, hit with the Druid's stats
/// and shield the Druid; in id order.
fn treants_tick(world: &mut World, ids: &[(SimId, Entity)]) {
    for &(id, e) in ids {
        let Some(mut t) = world.get::<Treant>(e).cloned() else { continue };
        let owner = world.resource::<SimIndex>().get(t.owner);
        let data = owner.and_then(|oe| world.get::<ClassKit>(oe)).and_then(|k| if let ClassData::Druid(d) = &k.data { Some(d.clone()) } else { None });
        let (Some(oe), Some(d)) = (owner, data) else {
            despawn_actor(world, id, e);
            continue;
        };
        t.left = t.left.saturating_sub(1);
        if t.left == 0 {
            despawn_actor(world, id, e);
            continue;
        }
        let at = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
        let alive_in_sight = |world: &World, tid: SimId| -> Option<Entity> {
            let te = world.resource::<SimIndex>().get(tid)?;
            let ok = world.get::<Health>(te).is_some_and(|h| h.current > 0.0) && world.get::<Position>(te).is_some_and(|p| (p.0 - at).truncate().length() - CAPSULE_RADIUS <= d.treant_sight);
            ok.then_some(te)
        };
        t.next = t.next.saturating_sub(1);
        match t.swing {
            Some((tid, n)) if n > 1 => t.swing = Some((tid, n - 1)),
            Some((tid, _)) => {
                t.swing = None;
                if let Some(te) = alive_in_sight(world, tid) {
                    treant_hit(world, t.owner, oe, at, tid, te, d.treant_factor);
                }
            }
            None if t.next == 0 => {
                let marked = t.marked.filter(|m| alive_in_sight(world, *m).is_some());
                let target = marked.or_else(|| {
                    others(world, t.owner)
                        .into_iter()
                        .filter(|(tid, te, p)| *tid != id && world.get::<crate::command::Kind>(*te).is_none_or(|k| k.0 != crate::command::ActorKind::Player) && (*p - at).truncate().length() - CAPSULE_RADIUS <= d.treant_sight)
                        .min_by(|a, b| a.2.distance(at).total_cmp(&b.2.distance(at)).then(a.0.cmp(&b.0)))
                        .map(|(tid, _, _)| tid)
                });
                if let Some(tid) = target {
                    t.marked = marked;
                    t.swing = Some((tid, ticks(d.treant_swing.1).max(1)));
                    t.next = ticks(d.treant_swing.0).max(1);
                }
            }
            None => {}
        }
        world.entity_mut(e).insert(t);
    }
}

/// One treant hit: the Druid's attack × the factor (physical) against the target; a share of the
/// damage becomes a shield on the Druid.
#[allow(clippy::too_many_arguments)]
fn treant_hit(world: &mut World, owner: SimId, oe: Entity, at: Vec3, tid: SimId, te: Entity, factor: f64) {
    let (Some(attacker), Some(defender), Some(to)) = (world.get::<Combatant>(oe).copied(), world.get::<Combatant>(te).copied(), world.get::<Position>(te).map(|p| p.0)) else { return };
    let (attack, defence) = (crate::buffs::attack_of(world, oe, attacker.attack), crate::buffs::defence_of(world, te, defender.defence));
    let hit = resolve_hit(&attack, &defence, Element::Physical, AttackTerm::Weapon, HitZone::Body, &mut world.resource_mut::<SimRng>());
    let mut damage = hit.damage * factor;
    let blocking = world.get::<Melee>(te).is_some_and(Melee::blocking);
    if blocking {
        let taim = world.get::<Aim>(te).copied().unwrap_or_default();
        damage = blocked_damage(damage, &defender, &taim, world.get::<Melee>(te).and_then(|m| m.guard_angles), to, at);
    }
    let damage = crate::buffs::incoming_damage(world, te, damage, None);
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    {
        let mut events = world.resource_mut::<SimEvents>();
        events.push(SimEvent::Damaged { source: owner, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by: HitSource::Skill(SkillKind::ForceOfNature) });
        if left <= 0.0 {
            events.push(SimEvent::Died { id: tid });
        }
    }
    let shield = world.get::<crate::Effects>(oe).and_then(|fx| fx.shield).map_or(0.0, |s| s.0);
    crate::world_items::give_shield(world, oe, shield + damage * tune::TREANT_SHIELD_SHARE, tune::TREANT_SHIELD_SECONDS);
}

fn despawn_actor(world: &mut World, id: SimId, e: Entity) {
    world.resource_mut::<SimIndex>().remove(id);
    world.despawn(e);
    world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id });
}

/// Shadow Pounce lands: every enemy around the Druid takes the pounce damage; a hit restores energy
/// once, and the first enemy hit becomes every treant's mark.
fn pounce_land(world: &mut World, id: SimId, e: Entity, d: &DruidData) {
    let Some(from) = world.get::<Position>(e).map(|p| p.0) else { return };
    let mut first = None;
    for (tid, te, to) in others(world, id) {
        if world.get::<Treant>(te).is_some() || (to - from).truncate().length() - CAPSULE_RADIUS > tune::POUNCE_RADIUS || (to - from).z.abs() > tune::AURA_HEIGHT {
            continue;
        }
        strike(world, id, e, tid, te, Element::Dark, AttackTerm::Weapon, d.pounce_factor, SkillKind::ShadowPounce, true);
        first.get_or_insert(tid);
    }
    if let Some(tid) = first {
        restore(world, e, PANTHER_HIT_ENERGY);
        let treants: Vec<Entity> = world.resource::<SimIndex>().iter().map(|(_, te)| te).filter(|te| world.get::<Treant>(*te).is_some_and(|t| t.owner == id)).collect();
        for te in treants {
            if let Some(mut t) = world.get_mut::<Treant>(te) {
                // Starts on it at once (skill text), if it can see it.
                t.marked = Some(tid);
                t.next = 0;
                t.swing = None;
            }
        }
    }
}

/// A Priest heal on the caster: the crit is rolled now.
fn heal_with(world: &mut World, id: SimId, e: Entity, shape: HealShape) {
    let Some(attack) = world.get::<Combatant>(e).map(|c| crate::buffs::attack_of(world, e, c.attack)) else { return };
    let crit = world.resource_mut::<SimRng>().chance(attack.critical_rate);
    crate::buffs::heal(world, id, e, shape.amount(&attack, crit));
}

/// Soul Chain: a fast shot along the aim (at the target in front, if any) that pulls what it hits all
/// the way to the DeathKnight.
fn soul_chain(world: &mut World, id: SimId, e: Entity, d: &DeathKnightData) {
    let (Some(from), aim) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied().unwrap_or_default()) else { return };
    let eye = from + Vec3::Z * tune::SHOT_HEIGHT;
    let target = front_target(world, id, e);
    let dir = target.map_or(aim_dir(&aim), |(_, _, p)| p - eye).normalize_or_zero();
    if let Some(mut spec) = shot_spec(world, e, SkillKind::SoulChain, d.chain_factor, (tune::SOUL_CHAIN_SPEED, 0.0, tune::SOUL_CHAIN_LIFETIME, tune::SOUL_CHAIN_RADIUS)) {
        spec.element = Element::Dark;
        spec.pull = tune::SOUL_CHAIN_SPEED * tune::SOUL_CHAIN_LIFETIME;
        launch(world, id, spec, eye + dir * 50.0, dir, None);
    }
}

/// The aim as a 3D direction (yaw and pitch).
fn aim_dir(aim: &Aim) -> Vec3 {
    let ((sy, cy), (sp, cp)) = (aim.yaw.sin_cos(), aim.pitch.sin_cos());
    Vec3::new(cp * cy, cp * sy, sp)
}

/// One direct skill hit on `te`: `resolve_hit` × `factor`, a block facing the caster, the target's
/// damage rules. Active hits count for life steal, Stealth, petrify and Ice Armor; non-active ones
/// (the Blizzard storm) don't. Returns the health left.
#[allow(clippy::too_many_arguments)]
fn strike(world: &mut World, id: SimId, e: Entity, tid: SimId, te: Entity, element: Element, term: AttackTerm, factor: f64, by: SkillKind, active: bool) -> f64 {
    let (Some(from), Some(attacker), Some(defender), Some(to)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Combatant>(e).copied(), world.get::<Combatant>(te).copied(), world.get::<Position>(te).map(|p| p.0)) else { return 0.0 };
    let (attack, defence) = (crate::buffs::attack_of(world, e, attacker.attack), crate::buffs::defence_of(world, te, defender.defence));
    let hit = resolve_hit(&attack, &defence, element, term, HitZone::Body, &mut world.resource_mut::<SimRng>());
    let mut damage = hit.damage * factor;
    let blocking = world.get::<Melee>(te).is_some_and(Melee::blocking);
    if blocking {
        let taim = world.get::<Aim>(te).copied().unwrap_or_default();
        damage = blocked_damage(damage, &defender, &taim, world.get::<Melee>(te).and_then(|m| m.guard_angles), to, from);
    }
    let damage = crate::buffs::incoming_damage(world, te, damage, active.then_some(e));
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    {
        let mut events = world.resource_mut::<SimEvents>();
        events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by: HitSource::Skill(by) });
        if left <= 0.0 {
            events.push(SimEvent::Died { id: tid });
        }
    }
    if active {
        crate::buffs::after_damage_dealt(world, id, e, damage, blocking);
    }
    left
}

fn slow(world: &mut World, te: Entity, skill: SkillKind, rate: f64, secs: f32) {
    crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Slow(rate), skill, ticks_left: ticks(secs) + 1, age: 0 });
}

/// Pyroblast: stage 1 three homing fireballs at the target (fanned out), stage 2 one large fireball
/// along the aim that bursts where it lands.
/// Returns the shots still to go out (the stage 1 volley after its first fireball).
fn pyroblast(world: &mut World, id: SimId, e: Entity, d: &WizardData, stage: u8, target: Option<(SimId, Entity, Vec3)>) -> Volley {
    let (Some(from), aim) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied().unwrap_or_default()) else { return Vec::new() };
    let eye = from + Vec3::Z * tune::SHOT_HEIGHT;
    if stage >= 2 {
        let dir = aim_dir(&aim);
        if let Some(mut spec) = shot_spec(world, e, SkillKind::Pyroblast, d.q_big_factor, (0.0, 0.0, tune::PYROBLAST_BIG_LIFETIME, d.q_big_radius)) {
            spec.element = Element::Fire;
            spec.speed = d.q_big_speed.clone();
            spec.burst = d.q_big_burst;
            spec.knockback = Some(d.q_big_knockback);
            launch(world, id, spec, eye + dir * 50.0, dir, None);
        }
        return Vec::new();
    }
    let (dir, factor, tid) = match target {
        Some((tid, _, p)) => (p - eye, d.q_small_factor * (1.0 + d.distance_rate(p.distance(from))), Some(tid)),
        None => (aim.forward_2d().extend(0.0), d.q_small_factor, None),
    };
    // the fireballs go out one after another (`SpawnDelay` 0, 0.3, 0.6 s), each fanned out a little
    // (HYPOTHESIS: in place of the spline's 0 / 120 / 240 degree spiral around the flight line).
    let n = d.q_small_delays.len().max(1);
    let mut volley = Vec::new();
    for (i, &delay) in d.q_small_delays.iter().enumerate() {
        let a = if n == 1 { 0.0 } else { (i as f32 / (n - 1) as f32 - 0.5) * 2.0 * tune::PYROBLAST_SPREAD_DEG.to_radians() };
        let dir = (Quat::from_rotation_z(a) * dir).normalize_or_zero();
        if let Some(mut spec) = shot_spec(world, e, SkillKind::Pyroblast, factor, (0.0, tune::PYROBLAST_TURN_RATE, tune::PYROBLAST_LIFETIME, d.q_small_radius)) {
            spec.element = Element::Fire;
            spec.speed = d.q_speed.clone();
            spec.burst = d.q_small_burst;
            volley.push((ticks(delay), spec, dir, tid));
        }
    }
    fire_volley(world, id, e, volley)
}

/// Shots to go out: (ticks to wait, spec, direction, homing target).
pub type Volley = Vec<(u32, ProjectileSpec, Vec3, Option<SimId>)>;

/// Launches a volley's shots whose wait is over (from the caster's chest, along their direction); returns the rest
/// for the kit to keep.
fn fire_volley(world: &mut World, id: SimId, e: Entity, volley: Volley) -> Volley {
    let eye = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0) + Vec3::Z * tune::SHOT_HEIGHT;
    let mut later = Vec::new();
    for (wait, spec, dir, tid) in volley {
        if wait == 0 {
            launch(world, id, spec, eye + dir * 50.0, dir, tid);
        } else {
            later.push((wait, spec, dir, tid));
        }
    }
    later
}

/// Flame Shock: a fire ring around the Wizard; every enemy within reach takes the base damage, is
/// knocked back and slowed.
fn flame_shock(world: &mut World, id: SimId, e: Entity, d: &WizardData) {
    let Some(from) = world.get::<Position>(e).map(|p| p.0) else { return };
    // A sphere at the hand (HYPOTHESIS: at the capsule's centre; the socket's offset isn't in the data) against
    // each capsule: its segment within the radius plus the capsule's.
    let half = CAPSULE_HEIGHT * 0.5 - CAPSULE_RADIUS;
    for (tid, te, to) in others(world, id) {
        let gap = (to - from).truncate();
        let dz = ((to - from).z.abs() - half).max(0.0);
        if gap.length().hypot(dz) > d.e_radius + CAPSULE_RADIUS {
            continue;
        }
        if strike(world, id, e, tid, te, Element::Fire, AttackTerm::FireBase, d.e_base, SkillKind::FlameShock, true) <= 0.0 {
            continue;
        }
        let (rate, secs) = d.e_slow;
        slow(world, te, SkillKind::FlameShock, rate, secs);
        // Pushed straight away from the caster to `HitBackDistance` beyond where it stood, over `KnockBackDuration`.
        let away = gap.normalize_or(aim_dir(&world.get::<Aim>(e).copied().unwrap_or_default()).truncate().normalize_or_zero());
        // Applied before this tick's move, so the pull lasts exactly its ticks.
        let velocity = away * d.e_knockback / d.e_knockback_secs;
        crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Pull(velocity), skill: SkillKind::FlameShock, ticks_left: ticks(d.e_knockback_secs), age: 0 });
    }
}

/// Frost Torment: stage 1 an ice bolt (its hit curses, [`projectile_hit`]); stage 2 a direct hit that
/// curses the target and spreads later (returned as the pending spread).
fn frost_torment(world: &mut World, id: SimId, e: Entity, d: &LichData, stage: u8, target: Option<(SimId, Entity, Vec3)>) -> Option<(SimId, u32)> {
    let (rate, secs) = FROST_TORMENT_SLOW;
    if stage >= 2
        && let Some((tid, te, _)) = target
    {
        if strike(world, id, e, tid, te, Element::Ice, AttackTerm::Weapon, d.torment_factor, SkillKind::FrostTorment, true) > 0.0 {
            slow(world, te, SkillKind::FrostTorment, rate, secs);
            frost_curse(world, id, e, te);
        }
        return Some((tid, ticks(d.spread_delay).max(1)));
    }
    let (from, aim) = (world.get::<Position>(e).map(|p| p.0)?, world.get::<Aim>(e).copied().unwrap_or_default());
    let eye = from + Vec3::Z * tune::SHOT_HEIGHT;
    let (dir, tid) = match target {
        Some((tid, _, p)) => (p - eye, Some(tid)),
        None => (aim_dir(&aim), None),
    };
    let dir = dir.normalize_or_zero();
    if let Some(mut spec) = shot_spec(world, e, SkillKind::FrostTorment, d.curse_factor, (tune::FROST_BOLT_SPEED, tune::FROST_BOLT_TURN_RATE, tune::FROST_BOLT_LIFETIME, tune::FROST_BOLT_RADIUS)) {
        spec.element = Element::Ice;
        spec.on_hit = Some((BuffKind::Slow(rate), secs));
        launch(world, id, spec, eye + dir * 50.0, dir, tid);
    }
    None
}

/// A projectile of `owner` hit a living `te` (called by the projectile system): Frost Torment's bolt
/// curses it.
pub(crate) fn projectile_hit(world: &mut World, owner: SimId, oe: Entity, by: HitSource, te: Entity) {
    if by == HitSource::Skill(SkillKind::FrostTorment) {
        frost_curse(world, owner, oe, te);
    }
    if by == HitSource::Skill(SkillKind::SoulChain)
        && let Some(mut p) = world.get_mut::<Power>(oe)
    {
        p.current = (p.current + tune::SOUL_CHAIN_SOUL).min(p.max);
    }
}

/// Frost Curse from the Lich `e` on `te` (one stack; a new curse refreshes it): its damage per pulse is
/// rolled now from the Lich's stats, as is the Lich's heal per pulse.
fn frost_curse(world: &mut World, id: SimId, e: Entity, te: Entity) {
    let Some(ClassData::Lich(d)) = world.get::<ClassKit>(e).map(|k| k.data.clone()) else { return };
    let (Some(attacker), Some(defender)) = (world.get::<Combatant>(e).copied(), world.get::<Combatant>(te).copied()) else { return };
    let (attack, defence) = (crate::buffs::attack_of(world, e, attacker.attack), crate::buffs::defence_of(world, te, defender.defence));
    let damage = resolve_hit(&attack, &defence, Element::Ice, AttackTerm::Weapon, HitZone::Body, &mut world.resource_mut::<SimRng>()).damage * d.curse_factor;
    let heal = d.curse_heal_base + d.curse_heal_factor * tooltip::heal_lich(&attack);
    crate::buffs::add_buff(world, te, Buff { kind: BuffKind::FrostCurse { by: id, damage, heal, interval: ticks(d.curse_interval).max(1) }, skill: SkillKind::FrostTorment, ticks_left: ticks(d.curse_seconds), age: 0 });
}

/// The Oracle goes off at `at`: the Priest is healed if inside (allies, in single player the Priest),
/// enemies inside take holy damage.
fn oracle_blast(world: &mut World, id: SimId, e: Entity, d: &PriestData, at: Vec3) {
    let me = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
    let inside = |p: Vec3| (p - at).truncate().length() - CAPSULE_RADIUS <= tune::ORACLE_RADIUS && (p.z - at.z).abs() <= tune::AURA_HEIGHT;
    if inside(me) {
        heal_with(world, id, e, d.oracle_heal);
    }
    for (tid, te, to) in others(world, id) {
        if inside(to) {
            strike(world, id, e, tid, te, Element::Light, AttackTerm::Weapon, d.oracle_factor, SkillKind::Oracle, true);
        }
    }
}

/// The storm unfolds where it is; the Lich can move again.
fn unfold(world: &mut World, e: Entity, b: &mut Blizzard) {
    b.channel = 0;
    b.unfolded = ticks(tune::BLIZZARD_UNFOLDED_SECONDS).max(1);
    if let Some(mut buffs) = world.get_mut::<Buffs>(e) {
        buffs.0.retain(|x| !(matches!(x.kind, BuffKind::Rooted) && x.skill == SkillKind::Blizzard));
    }
}

/// One Blizzard pulse at `at` (floor point): every enemy within `radius` (plus its capsule) takes
/// `factor` ice damage and the slow.
#[allow(clippy::too_many_arguments)]
fn blizzard_pulse(world: &mut World, id: SimId, e: Entity, at: Vec3, radius: f32, factor: f64, rate: f64, active: bool) {
    for (tid, te, to) in others(world, id) {
        if (to - at).truncate().length() - CAPSULE_RADIUS > radius || (to.z - at.z).abs() > tune::BLIZZARD_HEIGHT {
            continue;
        }
        if strike(world, id, e, tid, te, Element::Ice, AttackTerm::Weapon, factor, SkillKind::Blizzard, active) > 0.0 {
            slow(world, te, SkillKind::Blizzard, rate, tune::BLIZZARD_PULSE);
        }
    }
}

/// Mana regained over time; a creature that died gives each soul-energy user within range its soul.
fn power_tick(world: &mut World, ids: &[(SimId, Entity)]) {
    let alive = |world: &World, e: Entity| world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
    let mut users = Vec::new();
    for &(_, e) in ids {
        let Some(p) = world.get::<Power>(e).copied() else { continue };
        if !alive(world, e) {
            continue;
        }
        // A mana rolled recovery efficiency (HYPOTHESIS).
        let rate = if p.kind == PowerKind::Mana { 1.0 + crate::gear_triggers::worn(world, e, "affix_magic_magic_generate_rate") } else { 1.0 };
        if p.per_second > 0.0
            && let Some(mut q) = world.get_mut::<Power>(e)
        {
            q.current = (q.current + p.per_second * rate * f64::from(DT)).min(q.max);
        }
        if p.kind == PowerKind::SoulEnergy {
            users.push((e, world.get::<Position>(e).map_or(Vec3::ZERO, |q| q.0)));
        }
    }
    for &(_, e) in ids {
        if world.get::<SoulReleased>(e).is_some() || world.get::<Combatant>(e).is_none() || world.get::<Health>(e).is_none_or(|h| h.current > 0.0) {
            continue;
        }
        world.entity_mut(e).insert(SoulReleased);
        let at = world.get::<Position>(e).map_or(Vec3::ZERO, |q| q.0);
        for &(ue, upos) in &users {
            // The rolled soul energy recovery efficiency (HYPOTHESIS).
            let rate = 1.0 + crate::gear_triggers::worn(world, ue, "affix_soul_energy_soul_energy_generate_rate");
            if ue != e
                && upos.distance(at) <= tune::SOUL_ABSORB_RANGE
                && let Some(mut q) = world.get_mut::<Power>(ue)
            {
                q.current = (q.current + tune::SOUL_PER_KILL * rate).min(q.max);
            }
        }
    }
}

/// A dead creature whose soul was handed out.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SoulReleased;

fn stealth(world: &mut World, e: Entity, skill: SkillKind, secs: f32) {
    crate::buffs::add_buff(world, e, Buff { kind: BuffKind::Stealth { crit_damage: tune::STEALTH_CRIT_DAMAGE }, skill, ticks_left: ticks(secs), age: 0 });
}

/// Living combatants other than `me`, in id order, with their positions.
fn others(world: &World, me: SimId) -> Vec<(SimId, Entity, Vec3)> {
    world
        .resource::<SimIndex>()
        .iter()
        .filter(|(tid, te)| *tid != me && world.get::<Combatant>(*te).is_some() && world.get::<Health>(*te).is_some_and(|h| h.current > 0.0))
        .filter_map(|(tid, te)| world.get::<Position>(te).map(|p| (tid, te, p.0)))
        .collect()
}

fn launch(world: &mut World, owner: SimId, spec: ProjectileSpec, pos: Vec3, dir: Vec3, target: Option<SimId>) {
    crate::projectiles::spawn(world, owner, spec, pos, dir, target);
}

/// Flight of a skill shot: speed (cm/s), homing turn rate (deg/s), lifetime (s), radius (cm).
type Flight = (f32, f32, f32, f32);

fn shot_spec(world: &World, e: Entity, by: SkillKind, factor: f64, (speed, turn_rate, lifetime, radius): Flight) -> Option<ProjectileSpec> {
    let attack = crate::buffs::attack_of(world, e, world.get::<Combatant>(e)?.attack);
    Some(ProjectileSpec {
        speed: vec![(0.0, speed)],
        lifetime,
        radius,
        turn_rate,
        gravity: 0.0,
        attack,
        factor,
        element: Element::Physical,
        by: HitSource::Skill(by),
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: None,
        burst: 0.0,
        fuse: false,
        ignore_guard: false,
    knockback: None,
    })
}

/// Psychic Flying Swords' launch: `n` homing swords, one every [`FLYING_SWORDS_LAUNCH_EVERY`], over the enemies in front
/// (nearest first, round again when there are more swords than enemies: HYPOTHESIS, how the game's selection shares
/// them out isn't read);
/// with nobody in front, straight ahead. Returns the volley.
fn flying_swords(world: &mut World, id: SimId, e: Entity, d: &SwordMasterData, n: u8) -> Volley {
    let (Some(from), aim) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied().unwrap_or_default()) else { return Vec::new() };
    let eye = from + Vec3::Z * tune::SHOT_HEIGHT;
    let fwd = aim.forward_2d();
    let mut targets: Vec<(SimId, Vec3)> = others(world, id)
        .into_iter()
        .filter(|(_, _, p)| {
            let v = (*p - from).truncate();
            v.length() <= FLYING_SWORDS_SELECT_RANGE && fwd.dot(v.normalize_or_zero()) >= FLYING_SWORDS_SELECT_DEG.to_radians().cos()
        })
        .map(|(tid, _, p)| (tid, p))
        .collect();
    targets.sort_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)).then(a.0.cmp(&b.0)));
    let every = ticks(FLYING_SWORDS_LAUNCH_EVERY);
    (0..u32::from(n))
        .filter_map(|i| {
            let (target, dir, factor) = match targets.get(i as usize % targets.len().max(1)) {
                Some(&(t, p)) => (Some(t), p - eye, d.q_factor * (1.0 + d.distance_rate(p.distance(from)))),
                None => (None, fwd.extend(0.0), d.q_factor),
            };
            let spec = shot_spec(world, e, SkillKind::FlyingSwords, factor, (tune::FLYING_SWORDS_SPEED, tune::FLYING_SWORDS_TURN_RATE, tune::FLYING_SWORDS_LIFETIME, tune::FLYING_SWORDS_RADIUS))?;
            Some((i * every, spec, dir.normalize_or_zero(), target))
        })
        .collect()
}

/// Flying Blades: `blades` daggers fanned out ahead, each slowing what it hits.
fn flying_blades(world: &mut World, id: SimId, e: Entity, d: &RogueData) {
    let (Some(from), aim) = (world.get::<Position>(e).map(|p| p.0), world.get::<Aim>(e).copied().unwrap_or_default()) else { return };
    let eye = from + Vec3::Z * tune::SHOT_HEIGHT;
    let fwd = aim.forward_2d().extend(0.0);
    let n = d.blades.max(1);
    for i in 0..n {
        let a = if n == 1 { 0.0 } else { (f32::from(i) / f32::from(n - 1) - 0.5) * 2.0 * tune::FLYING_BLADES_HALF_SPREAD_DEG.to_radians() };
        let dir = Quat::from_rotation_z(a) * fwd;
        if let Some(mut spec) = shot_spec(world, e, SkillKind::FlyingBlades, d.blades_factor, (tune::FLYING_BLADES_SPEED, 0.0, tune::FLYING_BLADES_LIFETIME, tune::FLYING_BLADES_RADIUS)) {
            spec.on_hit = Some((BuffKind::Slow(d.blades_slow), d.blades_slow_seconds));
            launch(world, id, spec, eye + dir * 50.0, dir, None);
        }
    }
}

/// Petrifying Poison on an unblocked weapon hit: the target is petrified.
pub(crate) fn poison_on_hit(world: &mut World, _id: SimId, e: Entity, te: Entity) {
    let Some(kit) = world.get::<ClassKit>(e).filter(|k| k.poison.is_some()) else { return };
    let ClassData::Rogue(d) = &kit.data else { return };
    let res = d.petrify_damage_res;
    if let Some(mut k) = world.get_mut::<ClassKit>(e) {
        k.poison = None;
    }
    crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Petrified { damage_res: res }, skill: SkillKind::PetrifyingPoison, ticks_left: ticks(tune::PETRIFY_SECONDS), age: 0 });
}

/// Cooldowns, preparations, Blade Waltz hits and the poison window, in id order.
pub(crate) fn class_skills_tick(world: &mut World) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    power_tick(world, &ids);
    treants_tick(world, &ids);
    for &(id, e) in &ids {
        let Some(mut kit) = world.get::<ClassKit>(e).cloned() else { continue };
        let mut ended = Vec::new();
        let mut cast = None;
        let mut release = None;
        let alive = world.get::<Health>(e).is_some_and(|h| h.current > 0.0);
        if let Some((skill, held)) = kit.charge {
            kit.charge = Some((skill, held + 1));
            // Flying Swords: another sword summoned (its "stage" is the count).
            if skill == SkillKind::FlyingSwords {
                if let ClassData::SwordMaster(d) = &kit.data {
                    let (was, now) = (swords_summoned(held, d.q_swords), swords_summoned(held + 1, d.q_swords));
                    if now > was {
                        world.resource_mut::<SimEvents>().push(SimEvent::SkillCharged { id, skill, stage: now });
                    }
                }
            // Reaching a charge stage (what `release_skill` would fire at): for the client's effects.
            } else if let Some(stage) = tune::CHARGE_STAGES.iter().position(|&s| held + 1 == ticks(s)) {
                world.resource_mut::<SimEvents>().push(SimEvent::SkillCharged { id, skill, stage: stage as u8 + 1 });
                // Pyroblast's full charge (`ChargeThirdLevelSucceed`) grants `CanFloating`: one jump in the air while
                // the charge is held (`GA_TGF_Wizard_Floating` sets `JumpMaxCount` 2 on the jump).
                if stage == 1 && skill == SkillKind::Pyroblast && matches!(kit.data, ClassData::Wizard(_)) {
                    kit.floating = true;
                    if let Some(mut st) = world.get_mut::<crate::movement::CharacterState>(e) {
                        st.air_jumps = 1;
                    }
                }
            }
            // A full charge goes off by itself after the hold time (Pyroblast: its `MaxChargeTime`).
            let hold = match &kit.data {
                ClassData::Wizard(d) if skill == SkillKind::Pyroblast => d.q_max_charge,
                ClassData::SwordMaster(_) | ClassData::Druid(_) => f32::MAX,
                _ => tune::CHARGE_STAGES[1] + tune::CHARGE_HOLD,
            };
            if held + 1 >= ticks(hold) || !alive {
                release = Some(skill);
            }
        }
        // The volley's next shots (a dead caster's stay unfired).
        if !kit.volley.is_empty() {
            let due: Volley = std::mem::take(&mut kit.volley).into_iter().map(|(w, s, d, t)| (w.saturating_sub(1), s, d, t)).collect();
            if alive {
                kit.volley = fire_volley(world, id, e, due);
            }
        }
        // The air jump: used (its slow, `GE_TGF_Buff_Floating_SpeedDown`), or gone with the charge (the ability's end
        // removes `CanFloating`).
        if kit.floating {
            let left = world.get::<crate::movement::CharacterState>(e).map_or(0, |s| s.air_jumps);
            if left == 0 {
                kit.floating = false;
                if let ClassData::Wizard(d) = &kit.data {
                    let (rate, secs) = d.float_slow;
                    slow(world, e, SkillKind::Pyroblast, rate, secs);
                }
            } else if kit.charge.is_none_or(|c| c.0 != SkillKind::Pyroblast) {
                kit.floating = false;
                if let Some(mut st) = world.get_mut::<crate::movement::CharacterState>(e) {
                    st.air_jumps = 0;
                }
            }
        }
        if let (Some(mut b), ClassData::Lich(d)) = (kit.blizzard, &kit.data) {
            let pulse = ticks(tune::BLIZZARD_PULSE).max(1);
            b.age += 1;
            if !alive {
                unfold(world, e, &mut b);
                b.unfolded = 0;
            } else if b.channel > 0 {
                // The storm follows the aim's floor point.
                let goal = aim_floor_point(world, e, tune::BLIZZARD_RANGE);
                let step = (goal - b.pos).truncate().clamp_length_max(d.storm_speed * DT);
                b.pos = (b.pos.truncate() + step).extend(goal.z);
                if b.age % pulse == 0 {
                    blizzard_pulse(world, id, e, b.pos, d.storm_radius, d.storm_factor, BLIZZARD_SLOW, false);
                }
                b.channel -= 1;
                if b.channel == 0 {
                    unfold(world, e, &mut b);
                    cast = Some((SkillKind::Blizzard, 2));
                }
            } else if b.unfolded > 0 {
                if b.age % pulse == 0 {
                    blizzard_pulse(world, id, e, b.pos, tune::BLIZZARD_UNFOLDED_RADIUS, d.unfolded_factor, BLIZZARD_UNFOLDED_SLOW, true);
                }
                b.unfolded -= 1;
            }
            kit.blizzard = (b.channel > 0 || b.unfolded > 0).then_some(b);
            if kit.blizzard.is_none() {
                ended.push(SkillKind::Blizzard);
            }
        }
        if !kit.spreads.is_empty()
            && let ClassData::Lich(d) = &kit.data
        {
            let radius = d.spread_radius;
            let mut due = Vec::new();
            kit.spreads.retain_mut(|(tid, t)| {
                *t = t.saturating_sub(1);
                if *t == 0 {
                    due.push(*tid);
                }
                *t > 0
            });
            for tid in due {
                let Some(at) = world.resource::<SimIndex>().get(tid).and_then(|te| world.get::<Position>(te)).map(|p| p.0) else { continue };
                for (oid, oe, to) in others(world, id) {
                    if oid != tid && (to - at).truncate().length() <= radius {
                        frost_curse(world, id, e, oe);
                    }
                }
            }
        }
        for c in kit.cooldowns.values_mut() {
            *c = c.saturating_sub(1);
        }
        kit.cooldowns.retain(|_, c| *c > 0);
        if let Some((skill, t)) = kit.preparing {
            if t <= 1 {
                kit.preparing = None;
                if let (ClassData::Druid(d), SkillKind::WildAwakening) = (&kit.data, skill) {
                    kit.panther = true;
                    crate::buffs::add_buff(world, e, Buff { kind: BuffKind::MoveSpeed(d.panther_speed), skill, ticks_left: u32::MAX, age: 0 });
                }
                if let ClassData::Rogue(d) = &kit.data {
                    let secs = if skill == SkillKind::ShadowVeil { d.veil_seconds } else { d.fade_seconds };
                    stealth(world, e, skill, secs);
                }
            } else {
                kit.preparing = Some((skill, t - 1));
            }
        }
        if let (Some((left, age)), ClassData::SwordMaster(d)) = (kit.waltz, &kit.data) {
            let interval = ticks(tune::BLADE_WALTZ_HIT_INTERVAL).max(1);
            if age % interval == interval / 2 {
                waltz_hit(world, id, e, d.e_factor);
            }
            kit.waltz = (left > 1).then_some((left - 1, age + 1));
            if kit.waltz.is_none() {
                ended.push(SkillKind::BladeWaltz);
            }
        }
        if let Some(t) = kit.poison {
            kit.poison = (t > 1).then_some(t - 1);
            if kit.poison.is_none() {
                ended.push(SkillKind::PetrifyingPoison);
            }
        }
        let rules: Vec<(SkillKind, Option<(u8, f32)>)> = kit.charges.keys().map(|&s| (s, kit.charge_rule(s))).collect();
        for (skill, rule) in rules {
            if let (Some(c), Some((max, secs))) = (kit.charges.get_mut(&skill), rule)
                && c.1 > 0
            {
                c.1 -= 1;
                if c.1 == 0 {
                    c.0 = (c.0 + 1).min(max);
                    if c.0 < max {
                        c.1 = ticks(secs).max(1);
                    }
                }
            }
        }
        if let (Some((at, t)), ClassData::Priest(d)) = (kit.oracle, &kit.data) {
            if t <= 1 || !alive {
                kit.oracle = None;
                if let Some(mut b) = world.get_mut::<Buffs>(e) {
                    b.0.retain(|x| !(matches!(x.kind, BuffKind::Rooted) && x.skill == SkillKind::Oracle));
                }
                if alive {
                    oracle_blast(world, id, e, d, at);
                    cast = Some((SkillKind::Oracle, 1));
                }
            } else {
                kit.oracle = Some((at, t - 1));
            }
        }
        if let ClassData::Druid(d) = &kit.data {
            if let Some(t) = kit.pounce {
                if t <= 1 {
                    kit.pounce = None;
                    if alive {
                        pounce_land(world, id, e, d);
                        cast = Some((SkillKind::ShadowPounce, 1));
                    }
                } else {
                    kit.pounce = Some(t - 1);
                }
            }
            if kit.panther {
                let left = world.get::<Power>(e).map_or(0.0, |p| p.current) - d.panther_drain * f64::from(DT);
                if let Some(mut p) = world.get_mut::<Power>(e) {
                    p.current = left.max(0.0);
                }
                if left <= 0.0 || !alive {
                    leave_panther(world, e, &mut kit);
                    ended.push(SkillKind::WildAwakening);
                }
            }
        }
        if let (Some((skill, age, left)), ClassData::DeathKnight(d)) = (kit.aura, &kit.data) {
            let age = age + 1;
            let interval = ticks(d.ring_interval).max(1);
            let factor = if skill == SkillKind::SoulStorm { d.storm_factor } else { d.ring_factor };
            if alive && (age - 1) % interval == 0
                && let Some(from) = world.get::<Position>(e).map(|p| p.0)
            {
                for (tid, te, to) in others(world, id) {
                    if in_ring(from, to, d.ring_radius) {
                        strike(world, id, e, tid, te, Element::Dark, AttackTerm::DeathKnightQ, factor, skill, false);
                    }
                }
            }
            let mut on = alive && left > 1;
            if skill == SkillKind::SoulStorm
                && let Some(mut p) = world.get_mut::<Power>(e)
            {
                p.current = (p.current - tune::SOUL_STORM_DRAIN * f64::from(DT)).max(0.0);
                on = alive && p.current > 0.0;
            }
            kit.aura = on.then_some((skill, age, if left == u32::MAX { left } else { left - 1 }));
            if !on {
                ended.push(skill);
            }
        }
        if let Some(mut k) = world.get_mut::<ClassKit>(e) {
            *k = kit;
        }
        if let Some((skill, stage)) = cast {
            world.resource_mut::<SimEvents>().push(SimEvent::SkillCast { id, skill, stage });
        }
        for skill in ended {
            world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
        }
        if let Some(skill) = release {
            release_skill(world, id, skill);
        }
    }
}

/// One Blade Waltz hit: every living combatant within the waltz radius, in id order.
fn waltz_hit(world: &mut World, id: SimId, e: Entity, factor: f64) {
    let (Some(from), Some(attacker)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Combatant>(e).copied()) else { return };
    for (tid, te, to) in others(world, id) {
        if (to - from).truncate().length() - CAPSULE_RADIUS > tune::BLADE_WALTZ_RADIUS || (to - from).z.abs() > tune::BLADE_WALTZ_HEIGHT {
            continue;
        }
        let Some(defender) = world.get::<Combatant>(te).copied() else { continue };
        let (attack, defence) = (crate::buffs::attack_of(world, e, attacker.attack), crate::buffs::defence_of(world, te, defender.defence));
        let hit = resolve_hit(&attack, &defence, Element::Physical, AttackTerm::Weapon, HitZone::Body, &mut world.resource_mut::<SimRng>());
        let mut damage = hit.damage * factor;
        let blocking = world.get::<Melee>(te).is_some_and(Melee::blocking);
        if blocking {
            let taim = world.get::<Aim>(te).copied().unwrap_or_default();
            damage = blocked_damage(damage, &defender, &taim, world.get::<Melee>(te).and_then(|m| m.guard_angles), to, from);
        }
        let damage = crate::buffs::incoming_damage(world, te, damage, Some(e));
        let left = crate::world_items::apply_damage(world, tid, te, damage);
        {
            let mut events = world.resource_mut::<SimEvents>();
            events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by: HitSource::Skill(SkillKind::BladeWaltz) });
            if left <= 0.0 {
                events.push(SimEvent::Died { id: tid });
            }
        }
        // Non-active damage (skill text): it doesn't break Stealth or count as a hit from it.
        let steal = world.get::<Buffs>(e).map_or(0.0, Buffs::life_steal);
        if steal > 0.0 {
            crate::buffs::heal(world, id, e, damage * steal);
        }
    }
}

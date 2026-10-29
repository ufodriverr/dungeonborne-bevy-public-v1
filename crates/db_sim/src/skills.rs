//! Fighter skills: Whirlwind (Q, `107001`) and Charge (E, `107002`); War Cry (Q, `107004`), Inspire
//! (E, `107005`) and Rend (Q, `107006`). An actor has one Q and one E skill (its loadout,
//! `SkillKit::slots`).
//!
//! CONFIRMED: War Cry attack boost 0.3, physical resistance 0.3, life steal 0.3 for 5 s
//! (`CareerSkillDamageCurveTable` `GE_TGF_Buff_AtkDef_Add_Fighter_Q_BattleCry.*`); Inspire +150 cm/s
//! for 3 s (`GE_TGF_Buff_MoveSpeed_Add_Fighter_E_Inspire.*`), `BaseCoolDown` 30 s with 2 charges
//! (`GA_TGF_Fighter_DoubleE_Inspire`); Rend pulses every 0.25 s with distance factor 0.6
//! (`GA_TGF_Fighter_Q_Rupture_To_Target`: `Trigger Interval`, `Dist Factor`) and, from its skill text,
//! a 15 s window for the next unblocked weapon hit, 6 s on players / 10 s on monsters, 30% grievous
//! wounds. HYPOTHESIS: Rend's initial damage share of current health, the pulse damage scale, the War
//! Cry and Rend cooldowns ([`crate::overrides::skills`]).
//!
//! CONFIRMED (decoded data): Whirlwind damage factor 0.75 (`CareerSkillDamageCurveTable`
//! `GE_TGF_Damage_Instant_Fighter_QWhirlWind.MoveFactorRate`); the Whirlwind curve
//! `CV_TGF_Fighter_QWhirlWind` (0 → 102 → 163 over 2.97 s); Charge (`GA_TGF_Fighter_DoubleE`):
//! `BaseCoolDown` 30 s with charge-count tags 0..2; the curve `CV_TGF_Fighter_E3` (0 → 500 over 0.5 s).
//! HYPOTHESIS: the Whirlwind curve is a move-speed bonus in cm/s (the skill text says the Fighter
//! "gradually gains movement speed"); `CV_TGF_Fighter_E3` is the Charge distance; hit interval,
//! Whirlwind cooldown and the Charge easing ([`crate::overrides::skills`]).

use anyhow::Context;
use bevy_ecs::prelude::*;
use bevy_math::{Vec2, Vec3};

use crate::combat::{AttackTerm, Element, HitZone, resolve_hit};
use crate::id::SimIndex;
use crate::melee::{Aim, Combatant, Health, Melee, blocked_damage};
use crate::movement::CAPSULE_RADIUS;
use crate::overrides::{melee as melee_tune, skills as tune};
use crate::buffs::{Buff, BuffKind};
use crate::{DT, Position, SimEvent, SimEvents, SimId, SimRng};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum SkillKind {
    Whirlwind,
    Charge,
    WarCry,
    Inspire,
    Rend,
    // SwordMaster.
    FlyingSwords,
    BladeWaltz,
    // Rogue.
    PetrifyingPoison,
    FadeIntoVoid,
    FlyingBlades,
    SwiftConcealment,
    ShadowVeil,
    // Wizard.
    Pyroblast,
    FlameShock,
    // Lich.
    Blizzard,
    IceBarrier,
    FrostTorment,
    IceArmor,
    // Priest.
    Oracle,
    DivineProtection,
    HealSingle,
    Guardian,
    // DeathKnight.
    WraithRing,
    SoulChain,
    SoulStorm,
    // Druid. Return to Reason is Wild Awakening pressed again; Shadow Pounce replaces the E skill
    // in panther form.
    WildAwakening,
    ForceOfNature,
    ShadowPounce,
    Rejuvenation,
    /// Not a class skill: an altar's blessing, so blessings can live in `Buffs`. Not in [`SkillKind::ALL`].
    Blessing,
    /// Not a class skill: a monster ability's effect (the Shaman's buffs, the Werewolf's change). Not in
    /// [`SkillKind::ALL`].
    MonsterAbility,
    /// Not a class skill: an item's effect (the legendary fire staff's burn). Not in [`SkillKind::ALL`].
    Item,
}

impl SkillKind {
    /// 0 for Q skills, 1 for E skills (`ExcelRoleSkill` key, as).
    pub fn slot(self) -> usize {
        match self {
            SkillKind::Whirlwind | SkillKind::WarCry | SkillKind::Rend | SkillKind::FlyingSwords | SkillKind::PetrifyingPoison | SkillKind::FlyingBlades | SkillKind::Pyroblast | SkillKind::Blizzard | SkillKind::FrostTorment | SkillKind::Oracle | SkillKind::HealSingle | SkillKind::WraithRing | SkillKind::SoulStorm | SkillKind::WildAwakening => 0,
            SkillKind::Charge | SkillKind::Inspire | SkillKind::BladeWaltz | SkillKind::FadeIntoVoid | SkillKind::SwiftConcealment | SkillKind::ShadowVeil | SkillKind::FlameShock | SkillKind::IceBarrier | SkillKind::IceArmor | SkillKind::DivineProtection | SkillKind::Guardian | SkillKind::SoulChain | SkillKind::ForceOfNature | SkillKind::ShadowPounce | SkillKind::Rejuvenation | SkillKind::Blessing | SkillKind::MonsterAbility | SkillKind::Item => 1,
        }
    }

    /// A class's Q and E choices in the game's display order (empty for classes without skills yet).
    pub fn options(career: db_formats::config::generated::EnRoleCareerType) -> [&'static [SkillKind]; 2] {
        use db_formats::config::generated::EnRoleCareerType as C;
        match career {
            C::Fighter => SkillKind::FIGHTER,
            C::Swordmaster => [&[SkillKind::FlyingSwords], &[SkillKind::BladeWaltz]],
            C::Rogue => [&[SkillKind::PetrifyingPoison, SkillKind::FlyingBlades], &[SkillKind::FadeIntoVoid, SkillKind::SwiftConcealment, SkillKind::ShadowVeil]],
            C::Wizard => [&[SkillKind::Pyroblast], &[SkillKind::FlameShock]],
            C::Lich => [&[SkillKind::Blizzard, SkillKind::FrostTorment], &[SkillKind::IceBarrier, SkillKind::IceArmor]],
            C::Priest => [&[SkillKind::Oracle, SkillKind::HealSingle], &[SkillKind::DivineProtection, SkillKind::Guardian]],
            C::Deathknight => [&[SkillKind::WraithRing, SkillKind::SoulStorm], &[SkillKind::SoulChain]],
            C::DruidPanther => [&[SkillKind::WildAwakening], &[SkillKind::ForceOfNature, SkillKind::Rejuvenation]],
            _ => [&[], &[]],
        }
    }

    /// Every skill built so far.
    pub const ALL: [SkillKind; 29] = [
        SkillKind::Whirlwind,
        SkillKind::Charge,
        SkillKind::WarCry,
        SkillKind::Inspire,
        SkillKind::Rend,
        SkillKind::FlyingSwords,
        SkillKind::BladeWaltz,
        SkillKind::PetrifyingPoison,
        SkillKind::FadeIntoVoid,
        SkillKind::FlyingBlades,
        SkillKind::SwiftConcealment,
        SkillKind::ShadowVeil,
        SkillKind::Pyroblast,
        SkillKind::FlameShock,
        SkillKind::Blizzard,
        SkillKind::IceBarrier,
        SkillKind::FrostTorment,
        SkillKind::IceArmor,
        SkillKind::Oracle,
        SkillKind::DivineProtection,
        SkillKind::HealSingle,
        SkillKind::Guardian,
        SkillKind::WraithRing,
        SkillKind::SoulChain,
        SkillKind::SoulStorm,
        SkillKind::WildAwakening,
        SkillKind::ForceOfNature,
        SkillKind::ShadowPounce,
        SkillKind::Rejuvenation,
    ];

    /// The Fighter's Q and E choices, in the game's display order.
    pub const FIGHTER: [&'static [SkillKind]; 2] = [&[SkillKind::Whirlwind, SkillKind::WarCry, SkillKind::Rend], &[SkillKind::Charge, SkillKind::Inspire]];

    /// English names (from the string tables).
    pub fn name(self) -> &'static str {
        match self {
            SkillKind::Whirlwind => "Whirlwind",
            SkillKind::Charge => "Charge",
            SkillKind::WarCry => "War Cry",
            SkillKind::Inspire => "Inspire",
            SkillKind::Rend => "Rend",
            SkillKind::FlyingSwords => "Psychic Flying Swords",
            SkillKind::BladeWaltz => "Blade Waltz",
            SkillKind::PetrifyingPoison => "Petrifying Poison",
            SkillKind::FadeIntoVoid => "Fade into the Void",
            SkillKind::FlyingBlades => "Flying Blades",
            SkillKind::SwiftConcealment => "Swift Concealment",
            SkillKind::ShadowVeil => "Shadow Veil",
            SkillKind::Pyroblast => "Pyroblast",
            SkillKind::FlameShock => "Flame Shock",
            SkillKind::Blizzard => "Blizzard",
            SkillKind::IceBarrier => "Ice Barrier",
            SkillKind::FrostTorment => "Frost Torment",
            SkillKind::IceArmor => "Ice Armor",
            SkillKind::Oracle => "Oracle",
            SkillKind::DivineProtection => "Divine Protection",
            SkillKind::HealSingle => "Heal",
            SkillKind::Guardian => "Guardian",
            SkillKind::WraithRing => "Wraith Ring",
            SkillKind::SoulChain => "Soul Chain",
            SkillKind::SoulStorm => "Soul Storm",
            SkillKind::WildAwakening => "Wild Awakening",
            SkillKind::ForceOfNature => "Force of Nature",
            SkillKind::ShadowPounce => "Shadow Pounce",
            SkillKind::Rejuvenation => "Rejuvenation",
            SkillKind::Blessing => "Blessing",
            SkillKind::MonsterAbility => "Monster ability",
            SkillKind::Item => "Item",
        }
    }
}

/// Rend, from its skill text (`Skill_Description_107006`): the window for the next weapon hit, the
/// effect's length on players and on monsters (s), and the grievous-wounds rate.
pub const REND_WINDOW: f32 = 15.0;
pub const REND_ON_PLAYERS: f32 = 6.0;
pub const REND_ON_MONSTERS: f32 = 10.0;
pub const REND_GRIEVOUS_WOUNDS: f64 = 0.3;

/// What dealt a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitSource {
    Swing(crate::melee::AttackKind),
    Skill(SkillKind),
    /// A monster attack: index into the monster's attack list (`MonsterKit`).
    Monster(u8),
    /// Damage over time left by a skill (Rend's bleed), apart from the skill's own hit.
    Bleed(SkillKind),
    /// A loop trap.
    Trap(crate::features::TrapKind),
    /// A staff spell.
    Staff(crate::staffs::StaffHit),
    /// A crossbow bolt.
    Bolt,
    /// The death zone.
    Zone,
    /// A monster's fire patch burning (the Shadow Dragon's breath).
    Burn,
}

/// Fighter skill parameters read from the decoded assets.
#[derive(Debug, Clone, PartialEq)]
pub struct FighterSkillData {
    /// Whirlwind damage per hit as a factor on a weapon hit.
    pub whirlwind_factor: f64,
    /// (seconds, move-speed bonus cm/s), linear keys; the last key ends the spin.
    pub whirlwind_curve: Vec<(f32, f32)>,
    /// Charge distance (cm) and duration (s).
    pub charge_distance: f32,
    pub charge_duration: f32,
    /// Start slope of the Charge curve (cm/s, the first key's leave tangent).
    pub charge_start_slope: f32,
    /// Charge cooldown per charge (s) and maximum stored charges.
    pub charge_cooldown: f32,
    pub charge_max: u8,
    /// War Cry: attack boost, physical resistance, life steal (fractions) and duration (s).
    pub war_cry: (f64, f64, f64, f32),
    /// Inspire: move-speed bonus (cm/s), duration (s), cooldown per charge (s), charges.
    pub inspire_speed: f32,
    pub inspire_duration: f32,
    pub inspire_cooldown: f32,
    pub inspire_max: u8,
    /// Rend: pulse interval (s) and distance factor.
    pub rend_interval: f32,
    pub rend_dist_factor: f64,
}

impl FighterSkillData {
    pub fn whirlwind_duration(&self) -> f32 {
        self.whirlwind_curve.last().map_or(0.0, |k| k.0)
    }

    /// Linear interpolation of the Whirlwind curve.
    pub fn whirlwind_bonus(&self, t: f32) -> f32 {
        let k = &self.whirlwind_curve;
        match k.iter().position(|p| p.0 > t) {
            Some(0) => k[0].1,
            Some(i) => {
                let (a, b) = (k[i - 1], k[i]);
                a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0)
            }
            None => k.last().map_or(0.0, |p| p.1),
        }
    }

    /// Charge distance covered after `t` seconds: cubic Hermite from 0 to `charge_distance` with the
    /// curve's start slope and a flat end (tangent weights ignored; HYPOTHESIS shape).
    pub fn charge_offset(&self, t: f32) -> f32 {
        let s = (t / self.charge_duration).clamp(0.0, 1.0);
        let m0 = self.charge_start_slope * self.charge_duration;
        (s * s * s - 2.0 * s * s + s) * m0 + (-2.0 * s * s * s + 3.0 * s * s) * self.charge_distance
    }
}

/// Reads the Fighter skill data from the strict JSON exports under `_data/json` (gitignored).
pub fn load_fighter_skills() -> anyhow::Result<FighterSkillData> {
    let root = Some(db_formats::data_path("json/assets/Dungeonborne/Content/TGF/Character/Hero")).filter(|p| p.is_dir()).context("decoded hero assets not found under _data/json")?;
    let read = |rel: &str| -> anyhow::Result<serde_json::Value> {
        let p = root.join(rel);
        Ok(serde_json::from_slice(&std::fs::read(&p).with_context(|| format!("{}", p.display()))?)?)
    };
    let keys = |v: &serde_json::Value| -> anyhow::Result<Vec<(f32, f32)>> {
        let ks = v[0]["Properties"]["FloatCurves"]["Keys"].as_array().context("curve has no FloatCurves keys")?;
        ks.iter().map(|k| Ok((k["Time"].as_f64().context("Time")? as f32, k["Value"].as_f64().context("Value")? as f32))).collect()
    };
    let table = read("CareerSkillDamageCurveTable.json")?;
    let row = |name: &str| table[0]["Rows"][name]["Keys"][0]["Value"].as_f64().with_context(|| format!("curve row {name}"));
    let whirlwind_factor = row("GE_TGF_Damage_Instant_Fighter_QWhirlWind.MoveFactorRate")?;
    const CRY: &str = "GE_TGF_Buff_AtkDef_Add_Fighter_Q_BattleCry";
    let war_cry = (row(&format!("{CRY}.AttackBoostRate"))?, row(&format!("{CRY}.ElementResistanceRateNone"))?, row(&format!("{CRY}.LifeLeachRate"))?, row(&format!("{CRY}.ScalableFloatMagnitude"))? as f32);
    const INSPIRE: &str = "GE_TGF_Buff_MoveSpeed_Add_Fighter_E_Inspire";
    let (inspire_speed, inspire_duration) = (row(&format!("{INSPIRE}.MoveSpeedBase"))? as f32, row(&format!("{INSPIRE}.ScalableFloatMagnitude"))? as f32);
    let defaults = |v: &serde_json::Value| -> Option<serde_json::Value> { v.as_array()?.iter().find_map(|o| o.get("Name").and_then(|n| n.as_str()).filter(|n| n.starts_with("Default__")).map(|_| o["Properties"].clone())) };
    let inspire_ga = defaults(&read("Fighter/GameAbility/Fighter_E_Inspire/GA_TGF_Fighter_DoubleE_Inspire.json")?).context("Inspire GA defaults")?;
    let inspire_cooldown = inspire_ga["BaseCoolDown"].as_f64().context("Inspire BaseCoolDown")? as f32;
    let inspire_max = inspire_ga["CountTagConfig"].as_array().map_or(1, |a| a.len().saturating_sub(1).max(1)) as u8;
    let rend_ga = defaults(&read("Fighter/GameAbility/Fighter_Q_Rupture/GA_TGF_Fighter_Q_Rupture_To_Target.json")?).context("Rend GA defaults")?;
    let rend_interval = rend_ga["Trigger Interval"].as_f64().context("Rend Trigger Interval")? as f32;
    let rend_dist_factor = rend_ga["Dist Factor"].as_f64().context("Rend Dist Factor")?;
    let whirlwind_curve = keys(&read("Fighter/GameAbility/Fighter_Q_WhirlWind/CV_TGF_Fighter_QWhirlWind.json")?)?;
    let charge_json = read("Fighter/GameAbility/Fighter_E2/CV_TGF_Fighter_E3.json")?;
    let charge = keys(&charge_json)?;
    let charge_start_slope = charge_json[0]["Properties"]["FloatCurves"]["Keys"][0]["LeaveTangent"].as_f64().unwrap_or(0.0) as f32;
    let ga = read("Fighter/GameAbility/Fighter_E2/GA_TGF_Fighter_DoubleE.json")?;
    let props = ga.as_array().and_then(|a| a.iter().find_map(|o| o.get("Properties").filter(|p| p.get("BaseCoolDown").is_some()))).context("GA_TGF_Fighter_DoubleE has no BaseCoolDown")?;
    let charge_cooldown = props["BaseCoolDown"].as_f64().context("BaseCoolDown")? as f32;
    // `CountTagConfig` lists the UI states Count0..CountN: N charges.
    let charge_max = props["CountTagConfig"].as_array().map_or(1, |a| a.len().saturating_sub(1).max(1)) as u8;
    let (&(t0, v0), &(t1, v1)) = (charge.first().context("charge curve empty")?, charge.last().context("charge curve empty")?);
    Ok(FighterSkillData {
        whirlwind_factor,
        whirlwind_curve,
        charge_distance: v1 - v0,
        charge_duration: t1 - t0,
        charge_start_slope,
        charge_cooldown,
        charge_max,
        war_cry,
        inspire_speed,
        inspire_duration,
        inspire_cooldown,
        inspire_max,
        rend_interval,
        rend_dist_factor,
    })
}

/// An actor's skills and their state.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SkillKit {
    pub data: FighterSkillData,
    /// Ticks until Whirlwind is ready again (0 = ready).
    pub whirlwind_cooldown: u32,
    /// Ticks since Whirlwind started, while spinning.
    pub whirlwind: Option<u32>,
    pub charges: u8,
    /// Ticks until the next charge comes back (counts only while below the maximum).
    pub charge_recharge: u32,
    /// Ticks since Charge started and its direction, while charging.
    pub charge: Option<(u32, Vec2)>,
    /// The Q and E skills this actor uses.
    pub slots: [SkillKind; 2],
    pub war_cry_cooldown: u32,
    pub inspire_charges: u8,
    pub inspire_recharge: u32,
    pub rend_cooldown: u32,
    /// Ticks left for Rend's next weapon hit, while armed.
    pub rend_armed: Option<u32>,
}

impl SkillKit {
    pub fn new(data: FighterSkillData) -> Self {
        let (charges, inspire_charges) = (data.charge_max, data.inspire_max);
        SkillKit { data, whirlwind_cooldown: 0, whirlwind: None, charges, charge_recharge: 0, charge: None, slots: [SkillKind::Whirlwind, SkillKind::Charge], war_cry_cooldown: 0, inspire_charges, inspire_recharge: 0, rend_cooldown: 0, rend_armed: None }
    }

    /// Ticks until `skill` can be used again (0 = ready; charge skills: until the next charge).
    pub fn cooldown(&self, skill: SkillKind) -> u32 {
        match skill {
            SkillKind::Whirlwind => self.whirlwind_cooldown,
            SkillKind::WarCry => self.war_cry_cooldown,
            SkillKind::Rend => self.rend_cooldown,
            SkillKind::Charge if self.charges == 0 => self.charge_recharge,
            SkillKind::Inspire if self.inspire_charges == 0 => self.inspire_recharge,
            _ => 0,
        }
    }

    /// (charges, maximum) for charge skills.
    pub fn charges_of(&self, skill: SkillKind) -> Option<(u8, u8)> {
        match skill {
            SkillKind::Charge => Some((self.charges, self.data.charge_max)),
            SkillKind::Inspire => Some((self.inspire_charges, self.data.inspire_max)),
            _ => None,
        }
    }

    /// Move-speed bonus from Whirlwind this tick (cm/s).
    pub fn speed_bonus(&self) -> f32 {
        self.whirlwind.map_or(0.0, |t| self.data.whirlwind_bonus(t as f32 * DT))
    }

    /// Planar Charge velocity this tick (cm/s), if charging.
    pub fn charge_velocity(&self) -> Option<Vec2> {
        self.charge.map(|(t, dir)| dir * (self.data.charge_offset((t + 1) as f32 * DT) - self.data.charge_offset(t as f32 * DT)) / DT)
    }
}

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

/// Starts a skill if it is available; reports why not otherwise.
pub(crate) fn use_skill(world: &mut World, id: SimId, skill: SkillKind) {
    let Some(e) = world.resource::<SimIndex>().get(id) else {
        world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" });
        return;
    };
    // The dead cast nothing (a held key or a bot kept starting a skill whose release then did nothing).
    if world.get::<Health>(e).is_some_and(|h| h.current <= 0.0) {
        world.resource_mut::<SimEvents>().push(SimEvent::SkillRejected { id, skill });
        return;
    }
    if world.get::<crate::class_skills::ClassKit>(e).is_some() {
        return crate::class_skills::use_skill(world, id, e, skill);
    }
    if world.get::<crate::buffs::Buffs>(e).is_some_and(crate::buffs::Buffs::stunned) {
        world.resource_mut::<SimEvents>().push(SimEvent::SkillRejected { id, skill });
        return;
    }
    let swinging = world.get::<Melee>(e).is_some_and(|m| m.swing.is_some());
    let aim = world.get::<Aim>(e).copied().unwrap_or_default();
    let cd = crate::gear_triggers::cooldown_scale(world, e);
    let Some(mut kit) = world.get_mut::<SkillKit>(e) else {
        world.resource_mut::<SimEvents>().push(SimEvent::SkillRejected { id, skill });
        return;
    };
    let mut buff = None;
    let ok = kit.slots[skill.slot()] == skill
        && match skill {
        // Whirlwind waits for nothing: it is refused mid-swing or on cooldown.
        SkillKind::Whirlwind if kit.whirlwind.is_none() && kit.whirlwind_cooldown == 0 && !swinging => {
            kit.whirlwind = Some(0);
            kit.whirlwind_cooldown = ticks(tune::WHIRLWIND_COOLDOWN * cd);
            true
        }
        // Charge does not interrupt the current action (skill text).
        SkillKind::Charge if kit.charge.is_none() && kit.charges > 0 => {
            if kit.charges == kit.data.charge_max {
                kit.charge_recharge = ticks(kit.data.charge_cooldown * cd);
            }
            kit.charges -= 1;
            kit.charge = Some((0, aim.forward_2d()));
            true
        }
        SkillKind::WarCry if kit.war_cry_cooldown == 0 => {
            let (attack_boost, physical_resist, life_steal, secs) = kit.data.war_cry;
            kit.war_cry_cooldown = ticks(tune::WAR_CRY_COOLDOWN * cd);
            buff = Some(Buff { kind: BuffKind::Battle { attack_boost, physical_resist, life_steal }, skill, ticks_left: ticks(secs), age: 0 });
            true
        }
        // Inspire buffs the caster (and nearby allies, once there are any: co-op).
        SkillKind::Inspire if kit.inspire_charges > 0 => {
            if kit.inspire_charges == kit.data.inspire_max {
                kit.inspire_recharge = ticks(kit.data.inspire_cooldown * cd);
            }
            kit.inspire_charges -= 1;
            buff = Some(Buff { kind: BuffKind::MoveSpeed(kit.data.inspire_speed), skill, ticks_left: ticks(kit.data.inspire_duration), age: 0 });
            true
        }
        SkillKind::Rend if kit.rend_cooldown == 0 && kit.rend_armed.is_none() => {
            kit.rend_cooldown = ticks(tune::REND_COOLDOWN * cd);
            kit.rend_armed = Some(ticks(REND_WINDOW));
            true
        }
        _ => false,
    };
    if let Some(b) = buff.filter(|_| ok) {
        crate::buffs::add_buff(world, e, b);
    }
    world.resource_mut::<SimEvents>().push(if ok { SimEvent::SkillStarted { id, skill } } else { SimEvent::SkillRejected { id, skill } });
    if ok {
        crate::gear_triggers::on_skill_cast(world, e);
    }
}

/// Sets an actor's Q and E skills; a skill in the wrong slot or of another class is refused.
pub(crate) fn set_loadout(world: &mut World, id: SimId, q: SkillKind, e_skill: SkillKind) {
    use db_formats::config::generated::EnRoleCareerType;
    let e = world.resource::<SimIndex>().get(id);
    let career = e.and_then(|e| match (world.get::<SkillKit>(e), world.get::<crate::class_skills::ClassKit>(e)) {
        (_, Some(k)) => Some(k.data.career()),
        (Some(_), None) => Some(EnRoleCareerType::Fighter),
        (None, None) => None,
    });
    let ok = career.is_some_and(|c| {
        let [qs, es] = SkillKind::options(c);
        qs.contains(&q) && es.contains(&e_skill)
    });
    match e.filter(|_| ok) {
        Some(e) => {
            if let Some(mut kit) = world.get_mut::<SkillKit>(e) {
                kit.slots = [q, e_skill];
            }
            if let Some(mut kit) = world.get_mut::<crate::class_skills::ClassKit>(e) {
                kit.slots = [q, e_skill];
            }
        }
        None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "bad skill loadout" }),
    }
}

/// Rend on a weapon hit: if the attacker's Rend is armed and the hit was an unblocked weapon hit,
/// Rend fires: initial damage from the target's current health, then the bleed and grievous wounds.
pub(crate) fn rend_on_hit(world: &mut World, id: SimId, e: Entity, tid: SimId, te: Entity, hit_damage: f64) {
    let Some(kit) = world.get::<SkillKit>(e).filter(|k| k.rend_armed.is_some()) else { return };
    let (interval, factor) = (ticks(kit.data.rend_interval).max(1), kit.data.rend_dist_factor);
    if let Some(mut k) = world.get_mut::<SkillKit>(e) {
        k.rend_armed = None;
    }
    let Some(current) = world.get::<Health>(te).map(|h| h.current).filter(|&c| c > 0.0) else { return };
    let initial = crate::buffs::incoming_damage(world, te, current * tune::REND_CURRENT_HEALTH_SHARE, Some(e));
    let left = crate::world_items::apply_damage(world, tid, te, initial);
    {
        let mut events = world.resource_mut::<SimEvents>();
        events.push(SimEvent::Damaged { source: id, target: tid, damage: initial, crit: false, blocked: false, health: left, by: HitSource::Skill(SkillKind::Rend) });
        if left <= 0.0 {
            events.push(SimEvent::Died { id: tid });
            return;
        }
    }
    crate::buffs::after_damage_dealt(world, id, e, initial, false);
    let on_player = world.get::<crate::Kind>(te).is_some_and(|k| k.0 == crate::ActorKind::Player);
    let secs = if on_player { REND_ON_PLAYERS } else { REND_ON_MONSTERS };
    let last = world.get::<Position>(te).map_or(Vec3::ZERO, |p| p.0);
    let per_metre = factor * hit_damage * tune::REND_PULSE_HIT_SHARE;
    crate::buffs::add_buff(world, te, Buff { kind: BuffKind::Rent { by: id, per_metre, interval, last }, skill: SkillKind::Rend, ticks_left: ticks(secs), age: 0 });
    crate::buffs::add_buff(world, te, Buff { kind: BuffKind::GrievousWounds(REND_GRIEVOUS_WOUNDS), skill: SkillKind::Rend, ticks_left: ticks(secs), age: 0 });
}

/// Advances skills in id order: Whirlwind hits, timers, cooldowns and charge recharge. Runs after
/// movement, so movement sees a skill from the tick it starts on (tick 0).
pub(crate) fn skills_tick(world: &mut World) {
    let ids: Vec<(SimId, Entity)> = world.resource::<SimIndex>().iter().collect();
    for &(id, e) in &ids {
        let Some(mut kit) = world.get::<SkillKit>(e).cloned() else { continue };
        let cd = crate::gear_triggers::cooldown_scale(world, e);
        let mut ended = Vec::new();
        if let Some(t) = kit.whirlwind {
            let interval = ticks(tune::WHIRLWIND_HIT_INTERVAL).max(1);
            if t % interval == interval / 2 {
                whirlwind_hit(world, id, e, kit.data.whirlwind_factor, &ids);
            }
            kit.whirlwind = (t + 1 < ticks(kit.data.whirlwind_duration())).then_some(t + 1);
            if kit.whirlwind.is_none() {
                ended.push(SkillKind::Whirlwind);
            }
        }
        if let Some((t, dir)) = kit.charge {
            kit.charge = (t + 1 < ticks(kit.data.charge_duration)).then_some((t + 1, dir));
            if kit.charge.is_none() {
                ended.push(SkillKind::Charge);
            }
        }
        kit.whirlwind_cooldown = kit.whirlwind_cooldown.saturating_sub(1);
        kit.war_cry_cooldown = kit.war_cry_cooldown.saturating_sub(1);
        kit.rend_cooldown = kit.rend_cooldown.saturating_sub(1);
        if let Some(t) = kit.rend_armed {
            kit.rend_armed = (t > 1).then_some(t - 1);
            if kit.rend_armed.is_none() {
                ended.push(SkillKind::Rend);
            }
        }
        if kit.inspire_charges < kit.data.inspire_max {
            kit.inspire_recharge = kit.inspire_recharge.saturating_sub(1);
            if kit.inspire_recharge == 0 {
                kit.inspire_charges += 1;
                if kit.inspire_charges < kit.data.inspire_max {
                    kit.inspire_recharge = ticks(kit.data.inspire_cooldown * cd);
                }
            }
        }
        if kit.charges < kit.data.charge_max {
            kit.charge_recharge = kit.charge_recharge.saturating_sub(1);
            if kit.charge_recharge == 0 {
                kit.charges += 1;
                if kit.charges < kit.data.charge_max {
                    kit.charge_recharge = ticks(kit.data.charge_cooldown * cd);
                }
            }
        }
        if let Some(mut k) = world.get_mut::<SkillKit>(e) {
            *k = kit;
        }
        for skill in ended {
            world.resource_mut::<SimEvents>().push(SimEvent::SkillEnded { id, skill });
        }
    }
}

/// One Whirlwind hit: every living target within melee reach all around, in id order.
fn whirlwind_hit(world: &mut World, id: SimId, e: Entity, factor: f64, ids: &[(SimId, Entity)]) {
    let (Some(from), Some(attacker)) = (world.get::<Position>(e).map(|p| p.0), world.get::<Combatant>(e).copied()) else { return };
    for &(tid, te) in ids {
        if tid == id {
            continue;
        }
        let (Some(to), Some(defender), Some(health)) = (world.get::<Position>(te).map(|p| p.0), world.get::<Combatant>(te).copied(), world.get::<Health>(te).copied()) else {
            continue;
        };
        let d: Vec3 = to - from;
        if health.current <= 0.0 || d.z.abs() > melee_tune::VERTICAL_REACH || d.truncate().length() - CAPSULE_RADIUS > melee_tune::REACH {
            continue;
        }
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
            events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: hit.crit, blocked: blocking, health: left, by: HitSource::Skill(SkillKind::Whirlwind) });
            if left <= 0.0 {
                events.push(SimEvent::Died { id: tid });
            }
        }
        crate::buffs::after_damage_dealt(world, id, e, damage, blocking);
    }
}

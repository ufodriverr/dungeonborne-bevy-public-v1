//! Melee weapons' quick-attack combos: a left-click tap swings the held weapon's combo, as the staff's does,
//! instead of one generic timing (`overrides::melee`).
//!
//! CONFIRMED (the packages' imports in `_data/recovered/weapon_imports.tsv`, the loose v6 reads of the
//! abilities, ability sets and damage effects, the first-person montages):
//! - a weapon's definition (`WID_<Kind>_Base`) names its own anim setup (`WAID_<Kind>_Base`) and its pair setups
//!   (`WAID_<Kind><Other>_Base`, from `DoubleHandsConfig`); each setup grants one ability set, whose
//!   `InputTag.Weapon.Fire.FireLeft` abilities are the combo, in order;
//! - each swing: its first-person montage (the ability's one `1st` montage import that isn't the `OnHit` rebound;
//!   `CastMontageSet` doesn't decode), the montage's hit window (`AGR_Attack_NotifyState`), its chain window to
//!   the next swing (`TGF.Ability.CanQuickSide.CanQuickSide<n+1>`), and when a fresh combo may start (the
//!   `TGF.AbilityType.Active.Single.QuickSide1` window); the damage factor of its `MeleeWeaponHitGE` (inherited
//!   from `GE_TGF_Damage_Instant`, 1.0, where it sets none).
//!
//! - its guard (the set's `InputTag.Weapon.Charge.ChargeRight` ability, `GA_TGF_MeleeWeapon_Shield_Base`): the
//!   guarded degrees left and right of the facing (`LeftShieldAngle` / `RightShieldAngle` of its
//!   `GuardAngleHandGE`: 60 / 60 from `GE_TGF_GuardAngle_BothHand_Base` or `_RightHand_Base`, the relic's
//!   `GE_TGF_GuardAngle_LeftHand_Relic` overrides them to 90 / 90), and `MinShieldTime` (a guard let go sooner
//!   stays up until then: `OnRelease` sets a timer for the rest).
//!
//! - its charge attack (the set's `InputTag.Weapon.Charge.ChargeLeft` ability, `GA_TGF_MeleeWeapon_Charge_Base`): the
//!   montage plays `ChargeStart`, loops `Loop` while held, and plays `ChargeRelease` (with the hit window) on the
//!   release; held to `MaxChargeTime` it releases by itself (`ExceedMaxChargeTime`); `MinChargeTime`; the damage
//!   factor of its `MeleeWeaponHitGE`; free again at the release section's `Charge` tag window (else its end).
//!
//! HYPOTHESIS: let go before `MinChargeTime`, the charge releases at `MinChargeTime` (native timing); the relic
//! pairs' charge abilities (`GA_TGF_MeleeWeapon_Charge_Enchant_Base`: an ice enchant for 30 relic energy, the
//! dagger's with an ice bolt) aren't modelled and they keep the generic heavy attack;
//!
//! - pairs (part 2): an ability named `Quickside<n>` is step `QuickSide<n>`; a follow-up may start while the last
//!   swing's montage (or the last charge release's) grants `CanQuickSide<n>` (its tag windows), the opener
//!   (`GA_TGF_MeleeWeapon_Combo_First_Base`) when the actor is free, which is the montage's `QuickSide<n>` window of
//!   the opener's own number (`QuickSide3` for the dagger pair's `Quickside3_new`); hit windows are
//!   `AGR_Attack_NotifyState` or `TgfAttack` states, a `TgfAttack` naming the hand it traces, and a
//!   `CheckLeftRightGE` ability deals each hand's own effect (`MeleeWeaponHitGERightHand` / `LeftHand`);
//!   `TgfMontagePlayRate` states play their span at their `PlayRate`; the sword and shield's charge guards
//!   (`TGF.Ability.Defence.Shield` while it charges).
//!
//! HYPOTHESIS: the step number from the ability's name (the derived classes' own `AbilityTags` / `ActivationRequiredTags`
//! don't decode; the base classes' `QuickSide1-3` don't match the montages' chains, as the sword pair's swings 2-6 all
//! derive from `Combo_Third_Base`); each hand's window hits a target once (both hands at once: one window, the two factors
//! added); the counters (`..._Counter`, `..._Counter_Heavy`; their trigger is native) start on a tap while a faced guard's
//! counter is open (the setup's `PerfectGuardAutoEndShineTime`), the heavy one after a perfect guard (the hit within its
//! `DelayShieldGE`'s time of raising the guard, which also takes no damage: that effect's full guarded damage reduction);
//! `AdditionalTags` (`AdditionalTagsDelay` 0.6) aren't applied;
//!
//! HYPOTHESIS: which off-hand item picks which pair setup (by name: `SwordRelic` = a sword with a relic in the left hand; `DoubleHandsConfig`'s struct doesn't decode); a held pair without a loaded
//! setup swings the main hand's own combo; an item whose weapon definition names its own anim setups (the legendaries') swings those, with what they lack taken from the base setup of the same name
//! before its `_`; reach and cone stay the generic ones (the original traces the weapon's sockets); the weapon cooldown (`GE_TGF_Cooldown_Weapon_Base_*`, 0.3-0.65 s from a combo's start) isn't
//! applied: every montage reopens `QuickSide1` later than it.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context;
use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use db_formats::config::generated::EnItemWearingPart;
use serde_json::Value;

use crate::melee::{Melee, Swing};
use crate::skills::HitSource;
use crate::staffs::{CHARGE_TAG, ComboSpec, Montage, chain, damage_effect, obj_path, package_of, prop, read_json};
use crate::{DT, SimEvent, SimEvents, SimId, TickCount};

/// The package imports (`db_extract imports`): package path, imported package.
const IMPORTS: &str = "recovered/weapon_imports.tsv";
/// The main-hand kinds with combos, as the folders name them (`EN_GEAR_EQUIP_SUBTYPE_<KIND>`).
const KINDS: &[&str] = &["Sword", "Dagger", "Mace", "LongSword"];
const FIRE_LEFT: &str = "InputTag.Weapon.Fire.FireLeft";
const CHARGE_RIGHT: &str = "InputTag.Weapon.Charge.ChargeRight";
const CHARGE_LEFT: &str = "InputTag.Weapon.Charge.ChargeLeft";
const QUICK_SIDE: &str = "TGF.AbilityType.Active.Single.QuickSide";
const SHIELD_TAG: &str = "TGF.Ability.Defence.Shield";
const SWITCH_TAG: &str = "TGF.AbilityType.Active.Single.SwitchSlot";
/// [`ComboState::last`]'s index for a charge release.
pub const HEAVY: u8 = u8::MAX;

/// Combos and guards by setup name (`Sword`, `SwordRelic`, `Dagger`, ...).
#[derive(Resource, Debug, Clone, Default)]
pub struct WeaponCombos {
    pub combos: BTreeMap<String, Arc<Vec<ComboSpec>>>,
    pub guards: BTreeMap<String, GuardSpec>,
    pub heavies: BTreeMap<String, HeavySpec>,
    /// The ranged weapons by kind (`HeavyCrossbow`).
    pub crossbows: BTreeMap<String, Arc<crate::crossbow::CrossbowSpec>>,
    /// The melee weapon definitions' own anim setups by name (`WID_Dagger_6002` -> `Dagger_6002`, `DaggerDagger_6002`,
    /// `DaggerRelic_6002`). (Keyed by definition: some loose reads stop before their `EquipDefId`.)
    pub item_setups: BTreeMap<String, Vec<String>>,
    /// Each crossbow item's own (its weapon definition's `EquipDefId`).
    pub crossbow_items: BTreeMap<i32, Arc<crate::crossbow::CrossbowSpec>>,
    /// `BattleCalculation`'s `HurtInterruptBound` (100): a hit value at or above it interrupts and isn't guarded.
    pub hurt_interrupt: f32,
    /// The weapon set switch: `SwitchWeaponDuration` and the cooldown effect's duration, seconds.
    pub switch: (f32, f32),
    /// Each setup's draw sounds (its `AM_<set>_Draw_1st` montage's sound notifies: time, event).
    pub draw_sounds: BTreeMap<String, Vec<(f32, String)>>,
}

/// A setup's charge attack (seconds; the hit window and the free time from the release).
#[derive(Debug, Clone, PartialEq)]
pub struct HeavySpec {
    pub min_charge: f32,
    pub max_charge: f32,
    pub loop_at: f32,
    pub release_at: f32,
    pub window: (f32, f32),
    pub free: f32,
    pub factor: f64,
    pub segments: Vec<(String, crate::monsters::MontageSegment)>,
    /// The release's hit windows with their factors (s from the release; one per hand).
    pub hits: Vec<(f32, f32, f64)>,
    /// The `CanQuickSide<n>` windows after the release (n, s from the release): the follow-ups that may cut in.
    pub grants: Vec<(u8, f32, f32)>,
    /// It guards while it charges (`TGF.Ability.Defence.Shield` from the montage's start: the sword and shield).
    pub shield: bool,
    /// Its hit value (as [`ComboSpec::hit_value`]).
    pub hit_value: f32,
    /// Its `MoveSpeedDebuffGE`'s move speed rate (`MoveSpeedPercent`), on its holder from the charge's start to its end
    /// (`GA_TGF_Base::ApplyMoveSpeedDebuff`, applied at the start and the release: the longsword 5002's +0.25).
    pub move_rate: f64,
    /// A relic pair's charge is an enchant.
    pub enchant: Option<EnchantSpec>,
    /// A ground hit after the release (the mace 6002 with a shield).
    pub ground: Option<GroundHit>,
    /// Its montage's play rates (the mace and shield's charge starts at x0.75 to 2 s).
    pub rates: crate::staffs::PlayRates,
}

/// A charge's ground hit (`GA_MeleeWeapon_Mace6002Shield_Heavy` bytecode and
/// `B_MeleeWeapon_Mace6002Shield_Heavy_DamageArea`, CONFIRMED): at the montage's `AOE` notify the ability spawns its
/// damage area `GROUND_FORWARD` cm ahead of the holder and `GROUND_DROP` cm below its centre (`CastAOE`), which
/// applies `GE_Damage_Aoe` once to the characters it overlaps (`CapsuleRadius`) whose centres are within
/// `HeightDeltaCheck` of its height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundHit {
    /// s from the release.
    pub at: f32,
    pub radius: f32,
    pub height: f32,
    pub factor: f64,
}

/// `CastAOE`'s offsets (bytecode literals: `GetActorForwardVector() * 100`, `- (0, 0, 80)`).
pub const GROUND_FORWARD: f32 = 100.0;
pub const GROUND_DROP: f32 = 80.0;

/// A relic enchant (`GA_TGF_MeleeWeapon_Charge_Enchant_Base` and its sword / dagger subclasses, CONFIRMED
/// values): its relic energy cost (`TgfAbilityCost_WeaponAmmo`, paid when the charge starts), the `CastCharge`
/// notify (s from the release) that throws the projectile (`SpawnActor`) and puts the enchant on the caster
/// (`EnchantGEWeaponPair1`: its duration, its granted ability's `SpeedGE` slow and `Add ice damage GE` factor).
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantSpec {
    pub cost: u32,
    pub cast_at: f32,
    pub shot: crate::monsters::Shot,
    pub shot_element: crate::combat::Element,
    pub buff_secs: f32,
    pub slow: (f64, f32),
    pub ice: f64,
}

impl HeavySpec {
    /// The montage time shown `t` s into the charge (its play rates applied), or `t` s after the release.
    pub fn montage_time(&self, t: f32, released: bool) -> f32 {
        let m = self.rates.montage(t);
        if released {
            self.release_at + t
        } else if m < self.loop_at {
            m
        } else {
            self.loop_at + (m - self.loop_at) % (self.release_at - self.loop_at).max(DT)
        }
    }
}

/// A charge in progress: the setup, its start tick, the release tick, and whether the input let go.
#[derive(Debug, Clone, PartialEq)]
pub struct Charge {
    pub setup: String,
    pub from: u64,
    pub released: Option<u64>,
    pub let_go: bool,
    /// A relic enchant's cast (`CastCharge`) done.
    pub cast: bool,
}

/// A setup's guard: degrees guarded left and right of the facing, and the shortest guard (s); the perfect guard's
/// time from raising it (its `DelayShieldGE`'s duration: full guarded damage reduction) and how long a guard leaves
/// its counter open (`PerfectGuardAutoEndShineTime`, HYPOTHESIS), s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuardSpec {
    pub left: f32,
    pub right: f32,
    pub min_hold: f32,
    pub perfect: f32,
    pub counter: f32,
}

/// The last combo swing an actor started: the setup, the swing's index (or [`HEAVY`] for a charge release) and the
/// tick.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ComboState {
    pub last: Option<(String, u8, u64)>,
    /// The tick the current guard was raised.
    pub guard_from: Option<u64>,
    /// The charge attack in progress.
    pub charge: Option<Charge>,
    /// The tick the last combo swing hit and switched to its rebound montage.
    pub rebound_at: Option<u64>,
    /// A guarded hit's counter: open until this tick; a perfect guard's (the heavy counter).
    pub counter: Option<(u64, bool)>,
    /// A thunder chain the current swing calls at this tick.
    pub lightning: Option<(u64, LightningSpec)>,
}

/// Reads every setup's combo. A setup whose data is missing is left out (the error names it).
pub fn load_weapon_combos() -> anyhow::Result<(WeaponCombos, Vec<String>)> {
    let tsv = std::fs::read_to_string(db_formats::data_path(IMPORTS)).with_context(|| format!("{IMPORTS} missing (tools/rebuild_data.sh)"))?;
    let mut imports: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    for line in tsv.lines() {
        let Some((pkg, import)) = line.split_once('\t') else { continue };
        let name = pkg.rsplit('/').next().unwrap_or(pkg).trim_end_matches(".uasset");
        paths.entry(name.to_string()).or_insert_with(|| format!("/Game/{}", pkg.trim_start_matches("Dungeonborne/Content/").trim_end_matches(".uasset")));
        imports.entry(name.to_string()).or_default().push(import.trim().to_string());
    }
    let of = |name: &str| imports.get(name).map(Vec::as_slice).unwrap_or_default();
    let short = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let mut db = WeaponCombos::default();
    let mut skipped = Vec::new();
    for kind in KINDS {
        let mut setups = of(&format!("WID_{kind}_Base")).iter().filter(|p| short(p).starts_with("WAID_")).cloned().collect::<Vec<_>>();
        setups.sort();
        setups.dedup();
        for waid in setups {
            let name = short(&waid).trim_start_matches("WAID_").trim_end_matches("_Base").to_string();
            load_setup(&mut db, &mut skipped, name, &short(&waid), &|n| of(n).to_vec());
        }
    }
    // Each melee item's own anim setups: a weapon definition whose anim definitions aren't its kind's base ones (the
    // legendaries': `WAID_LongSword_6002`, `WAID_DaggerRelic_6002`...) brings those setups, named by them
    // (`LongSword_6002`); an item with that definition swings its own (`held_setup`).
    let base: Vec<String> = db.combos.keys().chain(db.heavies.keys()).chain(db.guards.keys()).cloned().collect();
    for wname in paths.keys().filter(|n| KINDS.iter().any(|k| n.starts_with(&format!("WID_{k}_"))) && !n.ends_with("_Base")) {
        let own: Vec<String> = of(wname).iter().map(|p| short(p)).filter(|n| n.starts_with("WAID_") && !base.contains(&n.trim_start_matches("WAID_").trim_end_matches("_Base").to_string())).collect();
        if own.is_empty() {
            continue;
        }
        let mut names = Vec::new();
        for waid in own {
            let name = waid.trim_start_matches("WAID_").to_string();
            load_setup(&mut db, &mut skipped, name.clone(), &waid, &|n| of(n).to_vec());
            // What its own setup lacks (abilities whose reads miss their montages: the dagger 6002's charges) it takes
            // from its base setup (`DaggerRelic_6002` -> `DaggerRelic`; HYPOTHESIS).
            let base = name.split('_').next().unwrap_or(&name).to_string();
            if let Some(h) = db.heavies.get(&base).filter(|_| !db.heavies.contains_key(&name)).cloned() {
                db.heavies.insert(name.clone(), h);
            }
            if let Some(g) = db.guards.get(&base).filter(|_| !db.guards.contains_key(&name)).copied() {
                db.guards.insert(name.clone(), g);
            }
            if let Some(d) = db.draw_sounds.get(&base).filter(|_| !db.draw_sounds.contains_key(&name)).cloned() {
                db.draw_sounds.insert(name.clone(), d);
            }
            names.push(name);
        }
        db.item_setups.insert(wname.clone(), names);
    }
    // The guard break bound.
    match read_json("json/assets", "/Game/TGF/Ability/Data/BattleCalculation").and_then(|v| v.as_array()?.first()?["Properties"]["HurtInterruptBound"].as_f64()) {
        Some(b) => db.hurt_interrupt = b as f32,
        None => skipped.push("guard breaks: BattleCalculation's HurtInterruptBound not decoded".to_string()),
    }
    // The weapon set switch.
    let switch = chain("/Game/TGF/Ability/GameAbility/Equipment/GA_TGF_SwitchWeaponSlot_Base");
    let cooldown = chain("/Game/TGF/Ability/GameAbility/Equipment/GE_TGF_SwitchWeapon_Cooldown");
    match (prop(&switch, "SwitchWeaponDuration").and_then(Value::as_f64), prop(&cooldown, "DurationMagnitude").and_then(|d| d["ScalableFloatMagnitude"]["Value"].as_f64())) {
        (Some(d), Some(c)) => db.switch = (d as f32, c as f32),
        _ => skipped.push("weapon switch: GA_TGF_SwitchWeaponSlot_Base / GE_TGF_SwitchWeapon_Cooldown not decoded".to_string()),
    }
    // The crossbow: its own definition's anim setup grants the shoot, reload and aim abilities.
    {
        let kind = "HeavyCrossbow";
        let wid = of(&format!("WID_{kind}_Base")).iter().find(|p| short(p) == format!("WID_{kind}_Base")).cloned();
        let waid = of(&format!("WID_{kind}_Base")).iter().find(|p| short(p) == format!("WAID_{kind}_Base")).cloned();
        let set = waid.as_deref().and_then(|w| of(&short(w)).iter().find(|p| short(p).starts_with("AbilitySet_")).cloned());
        if let Some(draw) = waid.as_deref().and_then(|w| of(&short(w)).iter().find(|p| p.contains("/Animation/1st/") && short(p).contains("_Draw_")).cloned())
            && let Ok(m) = Montage::load(&package_of(&draw))
        {
            db.draw_sounds.insert(kind.to_string(), m.sounds());
        }
        let wid = wid.unwrap_or_else(|| format!("/Game/TGF/Equipment/Weapon/{kind}/GameAbility/WID_{kind}_Base"));
        match set.context("no ability set import").and_then(|s| crate::crossbow::crossbow_of(&wid, &s, &|n| of(n).to_vec())) {
            Ok(c) => {
                db.crossbows.insert(kind.to_string(), Arc::new(c));
            }
            Err(e) => skipped.push(format!("{kind}: {e:#}")),
        }
        // Each item's own definition: its anim setup's ability set, else the parent setup's.
        for (wname, path) in paths.iter().filter(|(n, _)| (n.starts_with("WID_HeavyCrossbow_") || n.starts_with("WID_HeirloomHeavyCrossbow_")) && n.as_str() != "WID_HeavyCrossbow_Base") {
            let w = chain(path);
            let Some(item) = prop(&w, "EquipDefId").and_then(Value::as_i64) else {
                skipped.push(format!("{wname}: no EquipDefId"));
                continue;
            };
            let waid = prop(&w, "SingleAnimWeaponDefinition").and_then(obj_path).map(|p| short(&package_of(&p)));
            let set_of = |n: &str| of(n).iter().find(|p| short(p).starts_with("AbilitySet_")).cloned();
            let set = waid.as_deref().and_then(|n| set_of(n).or_else(|| of(n).iter().filter(|p| short(p).starts_with("WAID_")).find_map(|parent| set_of(&short(parent)))));
            match set.context("no ability set").and_then(|s| crate::crossbow::crossbow_of(path, &s, &|n| of(n).to_vec())) {
                Ok(c) => {
                    db.crossbow_items.insert(item as i32, Arc::new(c));
                }
                Err(e) => skipped.push(format!("{wname}: {e:#}")),
            }
        }
    }
    Ok((db, skipped))
}

/// One anim setup (`WAID_*`, by `name`): its draw sounds, guard, charge and combo.
fn load_setup(db: &mut WeaponCombos, skipped: &mut Vec<String>, name: String, waid: &str, of: &dyn Fn(&str) -> Vec<String>) {
    let short = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let imports = of(waid);
    if let Some(draw) = imports.iter().find(|p| p.contains("/Animation/1st/") && short(p).contains("_Draw_"))
        && let Ok(m) = Montage::load(&package_of(draw))
    {
        db.draw_sounds.insert(name.clone(), m.sounds());
    }
    let set = imports.iter().find(|p| short(p).starts_with("AbilitySet_")).cloned();
    if let Some(g) = set.as_deref().and_then(|s| guard_of(s).ok()) {
        db.guards.insert(name.clone(), g);
    }
    match set.as_deref().map(|s| heavy_of(s, of)) {
        Some(Ok(Some(h))) => {
            db.heavies.insert(name.clone(), h);
        }
        Some(Err(e)) => skipped.push(format!("{name} charge: {e:#}")),
        _ => {}
    }
    match set.context("no ability set import").and_then(|s| combo_of(&s, of)) {
        Ok(c) if !c.is_empty() => {
            db.combos.insert(name, Arc::new(c));
        }
        Ok(_) => skipped.push(format!("{name}: no left-click abilities")),
        Err(e) => skipped.push(format!("{name}: {e:#}")),
    }
}

/// An ability set's abilities on `input`.
pub(crate) fn granted(set: &str, input: &str) -> anyhow::Result<Vec<String>> {
    let v = read_json("json/loose", set).or_else(|| read_json("json/assets", set)).with_context(|| format!("{set} not decoded"))?;
    let granted = v.as_array().into_iter().flatten().flat_map(|o| o["Properties"]["GrantedGameplayAbilities"].as_array().into_iter().flatten());
    Ok(granted.filter(|g| g["InputTagContainer"].as_array().is_some_and(|t| t.iter().any(|x| x == input))).filter_map(|g| obj_path(&g["Ability"])).collect())
}

/// An ability set's guard (its right-button ability).
fn guard_of(set: &str) -> anyhow::Result<GuardSpec> {
    let ga = granted(set, CHARGE_RIGHT)?.into_iter().next().context("no guard ability")?;
    let c = chain(&ga);
    let min_hold = prop(&c, "MinShieldTime").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let ge = prop(&c, "GuardAngleHandGE").and_then(obj_path).context("GuardAngleHandGE")?;
    let gc = chain(&ge);
    let mods = prop(&gc, "Modifiers").and_then(Value::as_array).context("guard angle modifiers")?;
    let angle = |attr: &str| mods.iter().find(|m| m["Attribute"]["AttributeName"] == attr).and_then(|m| m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()).map(|v| v as f32);
    let perfect = prop(&c, "DelayShieldGE").and_then(obj_path).and_then(|g| prop(&chain(&g), "DurationMagnitude")?["ScalableFloatMagnitude"]["Value"].as_f64()).unwrap_or(0.0) as f32;
    let counter = prop(&c, "PerfectGuardAutoEndShineTime").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    Ok(GuardSpec { left: angle("LeftShieldAngle").context("LeftShieldAngle")?, right: angle("RightShieldAngle").context("RightShieldAngle")?, min_hold, perfect, counter })
}

/// An ability set's charge attack (`None`: a relic enchant, not modelled).
fn heavy_of(set: &str, imports: &dyn Fn(&str) -> Vec<String>) -> anyhow::Result<Option<HeavySpec>> {
    let Some(ga) = granted(set, CHARGE_LEFT)?.into_iter().next() else { return Ok(None) };
    let c = chain(&ga);
    let enchanting = crate::staffs::is_a(&c, "GA_TGF_MeleeWeapon_Charge_Enchant_Base");
    let name = ga.rsplit('/').next().unwrap_or(&ga);
    let num = |k: &str| prop(&c, k).and_then(Value::as_f64).map(|v| v as f32);
    let (right, left) = hand_factors(&c, name)?;
    let firsts: Vec<String> = imports(name).into_iter().filter(|p| p.contains("/Animation/1st/") && p.rsplit('/').next().is_some_and(|n| n.starts_with("AM_") && { let l = n.to_ascii_lowercase(); !l.contains("onhit") && !l.contains("hitenv") })).collect();
    let part = |k: &str| firsts.iter().find(|p| p.rsplit('/').next().is_some_and(|n| n.contains(k))).cloned();
    // `bUseReleaseMontage` (the long sword): the charge montage (`ChargeStart`, `Loop`) and a release montage of
    // its own, played from its start on the release; both cut the same sequence back to back, so they are joined
    // into one timeline with the release at the charge montage's end. (The `_End_` montage is the base's second
    // release, `bUseSecondReleaseMontage`, which the long sword leaves off.)
    let joined = match (prop(&c, "bUseReleaseMontage").and_then(Value::as_bool), part("_Charge_"), part("_Release_")) {
        (Some(true), Some(charge), Some(release)) => Some((Montage::load(&package_of(&charge))?, Montage::load(&package_of(&release))?)),
        _ => None,
    };
    let montage = firsts.first().cloned().with_context(|| format!("{name}: no 1st-person montage import"))?;
    let m = match joined {
        Some((charge, release)) => charge.joined(&release),
        None => Montage::load(&package_of(&montage))?,
    };
    let loop_at = m.section("Loop").context("Loop section")?;
    let release_at = m.section("ChargeRelease").context("ChargeRelease section")?;
    // (The hit windows are in real time, play rates applied: from the release's own real time. The charge's play
    // rates, `TgfMontagePlayRate` over `ChargeStart`, come before it: the mace and shield's x0.75 to 2 s.)
    let release_real = m.rates.real(release_at);
    let hits: Vec<(f32, f32, f64)> = hit_windows(&m, right, left).into_iter().filter(|h| h.1 > release_real).map(|(a, b, f)| (a - release_real, b - release_real, f)).collect();
    let enchant = if enchanting { Some(enchant_of(&c, &m, release_at, name)?) } else { None };
    // An enchant may throw without a slash of its own (the dagger's).
    let &(t0, t1, factor) = match (hits.first(), &enchant) {
        (Some(h), _) => h,
        (None, Some(en)) => &(en.cast_at, en.cast_at, 0.0),
        (None, None) => anyhow::bail!("{montage}: no hit window"),
    };
    let free = m.tag_window(CHARGE_TAG).map_or(m.length, |w| w.0);
    let max = num("MaxChargeTime").filter(|v| *v > 0.0).unwrap_or(f32::MAX);
    let grants = m.quick_side_grants().into_iter().filter(|g| g.2 > release_at).map(|(n, a, b)| (n, a - release_at, b - release_at)).collect();
    let shield = m.tag_window(SHIELD_TAG).is_some_and(|w| w.0 <= loop_at && w.1 >= loop_at);
    let (rv, lv) = hand_hit_values(&c);
    let hit_value = hit_windows(&m, rv, lv).into_iter().filter(|h| h.1 > release_real).map(|h| h.2).fold(0.0, f64::max) as f32;
    let move_rate = prop(&c, "MoveSpeedDebuffGE").and_then(obj_path).and_then(|g| prop(&chain(&g), "Modifiers")?.as_array()?.iter().find(|m| m["Attribute"]["AttributeName"] == "MoveSpeedPercent")?["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()).unwrap_or(0.0);
    // A damage area the ability spawns at the `AOE` notify.
    let ground = match (imports(name).into_iter().find(|p| p.ends_with("_DamageArea")), m.event("AOE")) {
        (Some(area), Some(at)) => {
            let a = chain(&area);
            let f = |k: &str| prop(&a, k).and_then(Value::as_f64).with_context(|| format!("{area} {k}"));
            let ge = prop(&a, "GE_Damage_Aoe").and_then(obj_path).with_context(|| format!("{area} GE_Damage_Aoe"))?;
            Some(GroundHit { at: at - release_at, radius: f("CapsuleRadius")? as f32, height: f("HeightDeltaCheck")? as f32, factor: damage_effect(&ge)?.0 })
        }
        _ => None,
    };
    Ok(Some(HeavySpec { hit_value, move_rate, min_charge: num("MinChargeTime").unwrap_or(0.0), max_charge: max, loop_at, release_at, window: (t0, t1), free: free - release_at, factor, segments: m.timed.clone(), hits, grants, shield, enchant, ground, rates: m.rates.clone() }))
}

/// A relic enchant's cost, cast time, projectile and enchant effect.
fn enchant_of(c: &[crate::staffs::Bp], m: &Montage, release_at: f32, name: &str) -> anyhow::Result<EnchantSpec> {
    let cost = crate::staffs::ability_cost(c).with_context(|| format!("{name}: no relic energy cost"))?;
    let cast_at = m.event("CastCharge").with_context(|| format!("{name}: no CastCharge notify"))? - release_at;
    let actor = prop(c, "SpawnActor").and_then(|v| v["ObjectPath"].as_str()).with_context(|| format!("{name} SpawnActor"))?;
    let actor_chain = chain(&package_of(actor));
    let shot = shot_of(&package_of(actor), &actor_chain).with_context(|| format!("{name}: projectile {actor}"))?;
    let shot_ge = prop(&actor_chain, "GameplayEffectClass").and_then(Value::as_array).and_then(|a| a.iter().find_map(obj_path)).context("projectile damage effect")?;
    let (_, shot_element) = damage_effect(&shot_ge)?;
    let ge = prop(c, "EnchantGEWeaponPair1").and_then(obj_path).context("EnchantGEWeaponPair1")?;
    let gc = chain(&ge);
    let buff_secs = prop(&gc, "DurationMagnitude").and_then(|d| d["ScalableFloatMagnitude"]["Value"].as_f64()).context("enchant duration")? as f32;
    let ga = prop(&gc, "GrantedAbilities").and_then(Value::as_array).and_then(|a| a.iter().find_map(|g| obj_path(&g["Ability"]))).context("enchant ability")?;
    let ac = chain(&ga);
    let slow_ge = chain(&prop(&ac, "SpeedGE").and_then(obj_path).context("SpeedGE")?);
    // A timed `MoveSpeedRate` multiplier (op 4, as the monsters' slows read it): a slow of 1 - magnitude.
    let secs = prop(&slow_ge, "DurationMagnitude").and_then(|d| d["ScalableFloatMagnitude"]["Value"].as_f64()).context("SpeedGE duration")? as f32;
    let rate = prop(&slow_ge, "Modifiers").and_then(Value::as_array).and_then(|m| m.iter().find(|m| m["Attribute"]["AttributeName"] == "MoveSpeedRate" && m["ModifierOp"].as_str().is_some_and(|o| o.ends_with("::4")))).and_then(|m| m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()).context("SpeedGE MoveSpeedRate")?;
    let slow = (1.0 - rate, secs);
    let ice_ge = prop(&ac, "Add ice damage GE").and_then(obj_path).context("ice damage effect")?;
    let (ice, _) = damage_effect(&ice_ge)?;
    Ok(EnchantSpec { cost, cast_at, shot, shot_element, buff_secs, slow, ice })
}

/// A projectile class's flight (the monsters' `projectile_shot` reads strict dumps only; the relic projectiles
/// decode loosely): `ProjectileMovementComponent` merged along the parents, `ShouldAcc` / `AccMag`,
/// `MaxExistTime`, `Explosive` / `ExplodeRadius`, the box's smaller half-width as the radius, the damage factor.
pub(crate) fn shot_of(package: &str, c: &[crate::staffs::Bp]) -> Option<crate::monsters::Shot> {
    fn component(package: &str, ty: &str) -> serde_json::Map<String, Value> {
        let Some(v) = read_json("json/assets", package).or_else(|| read_json("json/loose", package)) else { return Default::default() };
        let objs = v.as_array().cloned().unwrap_or_default();
        let parent = objs.iter().find(|o| o["Type"] == "BlueprintGeneratedClass").and_then(|o| o["Super"]["ObjectPath"].as_str()).filter(|p| p.starts_with("/Game/")).map(package_of);
        let mut props = parent.map(|p| component(&p, ty)).unwrap_or_default();
        if let Some(own) = objs.iter().find(|o| o["Type"] == ty).and_then(|o| o["Properties"].as_object()) {
            props.extend(own.clone());
        }
        props
    }
    let pm = component(package, "ProjectileMovementComponent");
    let speed = pm.get("InitialSpeed")?.as_f64()? as f32;
    let max_speed = pm.get("MaxSpeed").and_then(Value::as_f64).map_or(speed, |m| m as f32);
    let gravity = pm.get("ProjectileGravityScale").and_then(Value::as_f64).unwrap_or(1.0) as f32 * 980.0;
    let flag = |k: &str| prop(c, k).and_then(Value::as_bool) == Some(true);
    let num = |k: &str| prop(c, k).and_then(Value::as_f64).map(|v| v as f32);
    let accel = if flag("ShouldAcc") { num("AccMag").unwrap_or(0.0) } else { 0.0 };
    let ext = component(package, "BoxComponent").get("BoxExtent").cloned();
    let radius = ext.and_then(|e| Some(e["Y"].as_f64()?.min(e["Z"].as_f64()?) as f32)).unwrap_or(10.0);
    let burst = if flag("Explosive") { num("ExplodeRadius").unwrap_or(0.0) } else { 0.0 };
    let ge = prop(c, "GameplayEffectClass").and_then(Value::as_array).and_then(|a| a.iter().find_map(obj_path))?;
    let factor = damage_effect(&ge).ok()?.0;
    Some(crate::monsters::Shot { speed, range: 0.0, gravity, turn_rate: 0.0, radius, burst, accel, max_speed, lifetime: num("MaxExistTime"), offset: None, aim_degrees: None, slow: None, factor, toss: None, ignore_guard: false, delay: 0.0 })
}

/// The held relic's energy: (uid, energy).
fn relic_energy(world: &World, e: Entity) -> Option<(u32, u32)> {
    let st = world.get::<RelicEnergy>(e)?;
    let (uid, _) = st.held?;
    Some((uid, st.energy.get(&uid).copied().unwrap_or(0)))
}

/// An enchant's cast (`DealCastCharge` -> `ProcessProjectile` and the enchant effect): the projectile straight ahead
/// from the shot height (HYPOTHESIS: the native `SpawnParam`; level flight), the enchant on the caster. A charge's
/// ground hit: every character but the holder in the damage area takes its factor's hit.
fn ground_hit(world: &mut World, id: SimId, e: Entity, g: GroundHit, hit_value: f32) {
    let Some(from) = world.get::<crate::Position>(e).map(|p| p.0) else { return };
    let at = from + world.get::<crate::Aim>(e).copied().unwrap_or_default().forward_2d().extend(0.0) * GROUND_FORWARD - bevy_math::Vec3::Z * GROUND_DROP;
    let ids: Vec<(SimId, Entity)> = world.resource::<crate::SimIndex>().iter().collect();
    let mut area = Swing { by: HitSource::Swing(crate::AttackKind::Heavy), windows: Vec::new(), length: 1, tick: 0, hit: Vec::new(), root: Vec::new(), facing: Vec2::X, factor: g.factor, guard: None, tags: Vec::new(), window_factors: Vec::new(), hit_value };
    // (Its overlap with the target's capsule: the area's radius plus the capsule's.)
    let reach = g.radius + crate::movement::CAPSULE_RADIUS;
    crate::melee::hit_targets(world, id, e, &mut area, &ids, &|_, _, _, to| (to.z - at.z).abs() <= g.height && (to - at).truncate().length() <= reach);
    world.resource_mut::<SimEvents>().push(SimEvent::GroundHit { id, at });
}

fn cast_enchant(world: &mut World, id: SimId, e: Entity, en: &EnchantSpec) {
    let (Some(from), Some(c)) = (world.get::<crate::Position>(e).map(|p| p.0), world.get::<crate::Combatant>(e).copied()) else { return };
    let dir = world.get::<crate::Aim>(e).copied().unwrap_or_default().forward_2d().extend(0.0);
    let s = &en.shot;
    let life = s.lifetime.unwrap_or(2.5);
    // `ShouldAcc` / `AccMag`: the sword's slows (-300 cm/s²), the dagger's speeds up to its `MaxSpeed`.
    let speed = if s.accel > 0.0 && s.max_speed > s.speed {
        vec![(0.0, s.speed), ((s.max_speed - s.speed) / s.accel, s.max_speed)]
    } else if s.accel < 0.0 {
        vec![(0.0, s.speed), (life, (s.speed + s.accel * life).max(0.0))]
    } else {
        vec![(0.0, s.speed)]
    };
    let spec = crate::projectiles::ProjectileSpec {
        speed,
        lifetime: life,
        radius: s.radius,
        turn_rate: 0.0,
        gravity: 0.0,
        attack: crate::buffs::attack_of(world, e, c.attack),
        factor: s.factor,
        element: en.shot_element,
        by: HitSource::Swing(crate::AttackKind::Heavy),
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: None,
        burst: s.burst,
        fuse: false,
        ignore_guard: false,
        knockback: None,
    };
    let pos = from + bevy_math::Vec3::Z * crate::overrides::class_skills::SHOT_HEIGHT + dir * (crate::movement::CAPSULE_RADIUS + 10.0);
    crate::projectiles::spawn(world, id, spec, pos, dir, None);
    let ticks_left = (en.buff_secs / DT).round() as u32 + 1;
    crate::buffs::add_buff(world, e, crate::buffs::Buff { kind: crate::buffs::BuffKind::IceEnchant { slow: en.slow.0, slow_secs: en.slow.1, ice: en.ice }, skill: crate::skills::SkillKind::Item, ticks_left, age: 0 });
    world.resource_mut::<SimEvents>().push(SimEvent::RelicEnchantCast { id });
}

/// The ice enchant on a direct melee hit (`GA_TGF_HitCauseSpeedDown`, CONFIRMED bytecode: a melee hit on another
/// actor whose effect isn't a secondary one, `ActiveType` 1 such as this ice damage itself): the target is slowed
/// (`SpeedGE`) and takes the ice damage effect. HYPOTHESIS: only the hits the port's weapon swings deal (the original
/// checks the hit's source weapon is the enchanted one); blocked hits don't count (as Rend's).
pub(crate) fn enchant_on_hit(world: &mut World, id: SimId, e: Entity, tid: SimId, te: Entity, by: HitSource) {
    let Some((slow, slow_secs, ice)) = world.get::<crate::buffs::Buffs>(e).and_then(|b| b.0.iter().find_map(|b| if let crate::buffs::BuffKind::IceEnchant { slow, slow_secs, ice } = b.kind { Some((slow, slow_secs, ice)) } else { None })) else { return };
    let (Some(attacker), Some(defender)) = (world.get::<crate::Combatant>(e).copied(), world.get::<crate::Combatant>(te).copied()) else { return };
    crate::buffs::add_buff(world, te, crate::buffs::Buff { kind: crate::buffs::BuffKind::Slow(slow), skill: crate::skills::SkillKind::Item, ticks_left: (slow_secs / DT).round() as u32 + 1, age: 0 });
    let (attack, defence) = (crate::buffs::attack_of(world, e, attacker.attack), crate::buffs::defence_of(world, te, defender.defence));
    let hit = crate::combat::resolve_hit(&attack, &defence, crate::combat::Element::Ice, crate::combat::AttackTerm::Weapon, crate::combat::HitZone::Body, &mut world.resource_mut::<crate::SimRng>());
    let damage = crate::buffs::incoming_damage(world, te, hit.damage * ice, Some(e));
    let left = crate::world_items::apply_damage(world, tid, te, damage);
    let mut events = world.resource_mut::<SimEvents>();
    events.push(SimEvent::Damaged { source: id, target: tid, damage, crit: hit.crit, blocked: false, health: left, by });
    if left <= 0.0 {
        events.push(SimEvent::Died { id: tid });
    }
}

/// An ability set's left-click abilities as combo swings, in the set's order (the first-person sets' clip order).
fn combo_of(set: &str, imports: &dyn Fn(&str) -> Vec<String>) -> anyhow::Result<Vec<ComboSpec>> {
    let abilities = granted(set, FIRE_LEFT)?;
    // The opener's own `QuickSide<n>` window frees the actor for a fresh combo.
    let opener = abilities.iter().find(|ga| crate::staffs::is_a(&chain(ga), "GA_TGF_MeleeWeapon_Combo_First_Base")).and_then(|ga| step(ga)).unwrap_or(1);
    abilities.iter().map(|ga| swing_of(ga, opener, imports)).collect()
}

/// An ability's step number (`GA_MeleeWeapon_SwordSword_Quickside4_V2` -> 4; HYPOTHESIS, see the module notes).
fn step(ga: &str) -> Option<u8> {
    let name = ga.rsplit('/').next().unwrap_or(ga).to_ascii_lowercase();
    let at = name.find("quickside")? + "quickside".len();
    let digits: String = name[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// An ability's damage factor for its right-hand (or handless) and left-hand traces: a `CheckLeftRightGE`
/// ability names one effect per hand, the rest one `MeleeWeaponHitGE` for both.
fn hand_factors(c: &[crate::staffs::Bp], name: &str) -> anyhow::Result<(f64, f64)> {
    let ge = |key: &str| prop(c, key).and_then(Value::as_array).and_then(|a| a.iter().find_map(obj_path));
    let factor = |key: &str| ge(key).with_context(|| format!("{name} {key}")).and_then(|g| damage_effect(&g)).map(|f| f.0);
    if prop(c, "CheckLeftRightGE") == Some(&Value::Bool(true)) {
        Ok((factor("MeleeWeaponHitGERightHand")?, factor("MeleeWeaponHitGELeftHand")?))
    } else {
        let f = factor("MeleeWeaponHitGE")?;
        Ok((f, f))
    }
}

/// Each hand's hit value (`staffs::hit_value` of its damage effect), as [`hand_factors`] picks the effects.
fn hand_hit_values(c: &[crate::staffs::Bp]) -> (f64, f64) {
    let value = |key: &str| prop(c, key).and_then(Value::as_array).and_then(|a| a.iter().find_map(obj_path)).map_or(0.0, |g| crate::staffs::hit_value(&g));
    if prop(c, "CheckLeftRightGE") == Some(&Value::Bool(true)) { (value("MeleeWeaponHitGERightHand"), value("MeleeWeaponHitGELeftHand")) } else { (value("MeleeWeaponHitGE"), value("MeleeWeaponHitGE")) }
}

/// A montage's hit windows in real time with their hands' factors; overlapping windows (both hands at once) are
/// one window with the factors added.
fn hit_windows(m: &Montage, right: f64, left: f64) -> Vec<(f32, f32, f64)> {
    let mut out: Vec<(f32, f32, f64)> = Vec::new();
    for (a, b, hand) in m.hits() {
        let (a, b) = (m.rates.real(a), m.rates.real(b));
        let f = if hand == Some(crate::staffs::Hand::Left) { left } else { right };
        match out.last_mut() {
            Some(last) if a < last.1 => {
                last.1 = last.1.max(b);
                last.2 += f;
            }
            _ => out.push((a, b, f)),
        }
    }
    out
}

/// One combo swing: its ability's damage effects and first-person montage, in real time (the montage's play
/// rates applied).
fn swing_of(ga: &str, opener: u8, imports: &dyn Fn(&str) -> Vec<String>) -> anyhow::Result<ComboSpec> {
    let name = ga.rsplit('/').next().unwrap_or(ga);
    let c = chain(ga);
    let (right, left) = hand_factors(&c, name)?;
    let montage = imports(name).into_iter().find(|p| p.contains("/Animation/1st/") && p.rsplit('/').next().is_some_and(|n| n.starts_with("AM_") && { let l = n.to_ascii_lowercase(); !l.contains("onhit") && !l.contains("hitenv") })).with_context(|| format!("{name}: no 1st-person montage import"))?;
    let m = Montage::load(&package_of(&montage))?;
    let real = |t: f32| m.rates.real(t);
    let hits = hit_windows(&m, right, left);
    let &(t0, t1, factor) = hits.first().with_context(|| format!("{montage}: no hit window"))?;
    let (rv, lv) = hand_hit_values(&c);
    let hit_value = hit_windows(&m, rv, lv).into_iter().map(|h| h.2).fold(0.0, f64::max) as f32;
    let n = step(ga).unwrap_or(0);
    // A guard counter (`..._Counter`, `..._Counter_Heavy` after a perfect guard).
    let counter = name.contains("_Counter").then(|| name.contains("_Counter_Heavy"));
    // The longsword 5001's heavy counter: its montage's `Thunder` notify calls a thunder chain.
    let lightning = match prop(&c, "JudgeRadius").and_then(Value::as_f64).zip(m.event("Thunder")) {
        Some((judge, t)) => Some(lightning_of(&imports(name), judge as f32, real(t)).with_context(|| format!("{name} thunder chain"))?),
        None => None,
    };
    let grants: Vec<(u8, f32, f32)> = m.quick_side_grants().into_iter().map(|(k, a, b)| (k, real(a), real(b))).collect();
    let chain = grants.iter().find(|g| g.0 == n + 1).map(|g| (g.1, g.2));
    let free_at = real(m.tag_window(&format!("{QUICK_SIDE}{opener}")).or_else(|| m.tag_window(CHARGE_TAG)).map_or(m.length, |w| w.0));
    let opens = crate::staffs::is_a(&c, "GA_TGF_MeleeWeapon_Combo_First_Base");
    // The montage a hit on a character switches to: the ability's first-person `OnHit` import (HYPOTHESIS: the
    // property naming it doesn't decode; `ReboundMontageClient` is the wall-hit `HitEnv` one), when it carries tag
    // windows of its own (the mace's; the swords' play only a pose).
    let on_hit = imports(name).into_iter().find(|p| p.contains("/Animation/1st/") && p.rsplit('/').next().is_some_and(|n| n.starts_with("AM_") && n.to_ascii_lowercase().contains("onhit")));
    let rebound = on_hit.and_then(|r| Montage::load(&package_of(&r)).ok()).and_then(|r| {
        let real = |t: f32| r.rates.real(t);
        let grants: Vec<(u8, f32, f32)> = r.quick_side_grants().into_iter().map(|(k, a, b)| (k, real(a), real(b))).collect();
        let free = r.tag_window(&format!("{QUICK_SIDE}{opener}")).or_else(|| r.tag_window(CHARGE_TAG)).map(|w| real(w.0));
        (!grants.is_empty() || free.is_some()).then(|| crate::staffs::Rebound { grants, free_at: free.unwrap_or(real(r.length)), length: real(r.length), segments: r.timed.clone(), rates: r.rates.clone() })
    });
    Ok(ComboSpec { window: (t0, t1), factor, chain, free_at, length: real(m.length), clip: m.segments.first().map(|s| s.0.clone()).unwrap_or_default(), segments: m.timed.clone(), hits, n, opener: opens, grants, rates: m.rates.clone(), rebound, switch_window: m.tag_window(SWITCH_TAG).map(|w| (real(w.0), real(w.1))), counter, lightning, hit_value })
}

/// A thunder chain a swing calls (CONFIRMED from `GA_MeleeWeapon_LongSword_Counter_Heavy_Child_Lightening`,
/// `B_TGF_ThunderChain_LongSword_5001` and its parent `B_TGF_ThunderChain`): at `at` s into the swing, the nearest
/// hostile character within `judge` cm becomes the target of a projectile (`speed` cm/s, `radius` cm) that hits it and
/// then the nearest unhit one within `query` cm of the hit, `hits` in all, each taking `factor` (`element`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightningSpec {
    pub at: f32,
    pub judge: f32,
    pub query: f32,
    pub hits: u8,
    pub factor: f64,
    pub element: crate::combat::Element,
    pub speed: f32,
    pub radius: f32,
}

fn lightning_of(imports: &[String], judge: f32, at: f32) -> anyhow::Result<LightningSpec> {
    let actor = imports.iter().find(|p| p.rsplit('/').next().is_some_and(|n| n.starts_with("B_TGF_ThunderChain_"))).context("no thunder chain import")?;
    let ac = chain(actor);
    let num = |k: &str| prop(&ac, k).and_then(Value::as_f64).with_context(|| format!("{actor}: {k}"));
    let ge = prop(&ac, "Level1GEList").and_then(Value::as_array).and_then(|l| l.first()).and_then(obj_path).context("Level1GEList")?;
    let (factor, element) = damage_effect(&package_of(&ge))?;
    // The movement and the collision box sit on the class or a parent (`B_TGF_ThunderChain`: 7500 cm/s, 15 cm).
    let (mut speed, mut radius, mut class) = (None, None, Some(actor.clone()));
    for _ in 0..4 {
        let Some(v) = class.take().and_then(|c| read_json("json/loose", &c).or_else(|| read_json("json/assets", &c))) else { break };
        for o in v.as_array().into_iter().flatten() {
            match o["Type"].as_str() {
                Some("ProjectileMovementComponent") => speed = speed.or(o["Properties"]["InitialSpeed"].as_f64()),
                Some("BoxComponent") => radius = radius.or(o["Properties"]["BoxExtent"]["X"].as_f64()),
                Some("BlueprintGeneratedClass") => class = o["Super"]["ObjectPath"].as_str().filter(|s| s.starts_with("/Game/")).map(package_of),
                _ => {}
            }
        }
    }
    Ok(LightningSpec { at, judge, query: num("QueryRadius")? as f32, hits: num("MaxHitNum")? as u8, factor, element, speed: speed.context("thunder chain speed")? as f32, radius: radius.unwrap_or(15.0) as f32 })
}

/// The follow-up a tap may start now: the first swing (in the set's order) whose step the last swing, or the last
/// charge release, grants as `CanQuickSide<n>` at this moment.
fn follow_up(world: &World, setup: &str, combo: &[ComboSpec], last: Option<&(String, u8, u64)>, rebound: Option<u64>, now: u64) -> Option<usize> {
    let (s, i, at) = last?;
    if s != setup {
        return None;
    }
    // After a hit that switched to the rebound montage, its windows count from the hit.
    let spec = (*i != HEAVY).then(|| combo.get(usize::from(*i))).flatten();
    let (grants, from) = match (rebound, spec.and_then(|c| c.rebound.as_ref())) {
        (Some(t), Some(r)) => (r.grants.clone(), t),
        _ if *i == HEAVY => (world.get_resource::<WeaponCombos>()?.heavies.get(setup)?.grants.clone(), *at),
        _ => (spec?.grants.clone(), *at),
    };
    let since = (now - from) as f32 * DT;
    let open: Vec<u8> = grants.iter().filter(|g| since >= g.1 - DT * 0.5 && since <= g.2).map(|g| g.0).collect();
    combo.iter().position(|c| !c.opener && c.n > 0 && open.contains(&c.n))
}

/// A swing's windows in ticks and, with several, each one's factor.
fn swing_windows(hits: &[(f32, f32, f64)]) -> (Vec<(u32, u32)>, Vec<f64>) {
    let windows = hits.iter().map(|h| (ticks(h.0), ticks(h.1).max(ticks(h.0) + 1))).collect();
    let factors = if hits.len() > 1 { hits.iter().map(|h| h.2).collect() } else { Vec::new() };
    (windows, factors)
}

/// The setup an actor holds: its right-hand weapon's kind with its left-hand item's (`SwordRelic`) when that
/// pair has a combo, else the right hand's alone; `None` without a combo for it.
fn held_setup(world: &World, e: Entity) -> Option<(String, Arc<Vec<ComboSpec>>)> {
    let db = world.get_resource::<WeaponCombos>()?;
    let inv = world.get::<crate::Inventory>(e)?;
    let kind = |part| inv.worn.get(&part).and_then(|i| subtype(world, i.item.item_id));
    let main = kind(EnItemWearingPart::Pair1rightweapon)?;
    let off = kind(EnItemWearingPart::Pair1leftweapon);
    let get = |n: String| db.combos.get(&n).map(|c| (n, c.clone()));
    // The right-hand item's own setups: its pair's (`DaggerRelic_6002`: the kinds and its own suffix), else its single
    // one.
    let own = inv.worn.get(&EnItemWearingPart::Pair1rightweapon).and_then(|i| definition(world, i.item.item_id)).and_then(|w| db.item_setups.get(&w));
    // (A pair setup of its own named otherwise: the mace 6002's `MaceShield_Base_Mace6002` beside `Mace_Base_6002`.)
    if let Some(off) = off.as_ref()
        && let Some(found) = own.and_then(|o| o.iter().find(|n| n.starts_with(&format!("{main}{off}_")))).and_then(|n| get(n.clone()))
    {
        return Some(found);
    }
    if let Some(single) = own.and_then(|o| o.iter().find(|n| n.starts_with(&format!("{main}_")))) {
        let suffix = &single[main.len()..];
        if let Some(found) = off.as_ref().and_then(|off| get(format!("{main}{off}{suffix}"))).or_else(|| get(single.clone())) {
            return Some(found);
        }
    }
    off.and_then(|off| get(format!("{main}{off}"))).or_else(|| get(main))
}

/// An item's weapon definition's name (`WID_LongSword_6002`, from its equipment row's path).
pub(crate) fn definition(world: &World, item_id: i32) -> Option<String> {
    let t = world.get_resource::<crate::GameData>()?.tables;
    let equip = t.uesource_item_type.iter().find(|u| u.type_id == item_id)?.ue_source_equipment_equip_id;
    let path = package_of(&t.ue_source_equipment.iter().find(|r| r.equip_id == equip)?.ue_path);
    path.rsplit('/').next().map(str::to_string)
}

/// An item's equipment kind as the folders name it (`EN_GEAR_EQUIP_SUBTYPE_SWORD` -> `Sword`, `_LONG_SWORD` ->
/// `LongSword`).
pub(crate) fn subtype(world: &World, item_id: i32) -> Option<String> {
    let t = world.get_resource::<crate::GameData>()?.tables;
    let equip = t.uesource_item_type.iter().find(|u| u.type_id == item_id)?.ue_source_equipment_equip_id;
    let sub = t.ue_source_equipment.iter().find(|r| r.equip_id == equip)?.equip_subtype.name()?;
    let s = sub.strip_prefix("EN_GEAR_EQUIP_SUBTYPE_")?.to_ascii_lowercase();
    Some(s.split('_').filter_map(|w| {
        let mut c = w.chars();
        Some(c.next()?.to_ascii_uppercase().to_string() + c.as_str())
    }).collect())
}

/// A guard faced a hit: whether it was a perfect guard (the hit within the setup's perfect time of raising it,
/// `GuardSpec::perfect`); the guard's counter opens.
pub(crate) fn guarded(world: &mut World, e: Entity) -> bool {
    let now = world.resource::<TickCount>().0;
    let Some(g) = held_setup(world, e).and_then(|(name, _)| world.get_resource::<WeaponCombos>()?.guards.get(&name).copied()) else { return false };
    let mut st = world.get::<ComboState>(e).cloned().unwrap_or_default();
    let perfect = st.guard_from.is_some_and(|from| (now.saturating_sub(from) as f32) * DT <= g.perfect);
    if g.counter > 0.0 {
        st.counter = Some((now + u64::from(ticks(g.counter)), perfect));
        world.entity_mut(e).insert(st);
    }
    perfect
}

/// A swing's thunder chain when its time comes, while that swing still runs: at the nearest living non-player
/// combatant within its judge radius (from its capsule's edge, HYPOTHESIS), from the shot height (HYPOTHESIS: the
/// original spawns it at the weapon); none in reach, none at all.
fn lightning(world: &mut World, id: SimId, e: Entity, now: u64) {
    let Some((due, l)) = world.get::<ComboState>(e).and_then(|s| s.lightning) else { return };
    if now < due {
        return;
    }
    if let Some(mut st) = world.get_mut::<ComboState>(e) {
        st.lightning = None;
    }
    let swinging = world.get::<Melee>(e).and_then(|m| m.swing.as_ref()).is_some_and(|s| s.by == HitSource::Swing(crate::AttackKind::Light));
    let (Some(from), Some(c)) = (world.get::<crate::Position>(e).map(|p| p.0), world.get::<crate::Combatant>(e).copied()) else { return };
    if !swinging {
        return;
    }
    let ids: Vec<(SimId, Entity)> = world.resource::<crate::SimIndex>().iter().collect();
    let target = ids
        .iter()
        .filter(|(tid, te)| *tid != id && world.get::<crate::Combatant>(*te).is_some() && world.get::<crate::Health>(*te).is_some_and(|h| h.current > 0.0) && world.get::<crate::command::Kind>(*te).is_none_or(|k| k.0 != crate::command::ActorKind::Player))
        .filter_map(|&(tid, te)| world.get::<crate::Position>(te).map(|p| (tid, p.0)))
        .filter(|(_, p)| p.distance(from) - crate::movement::CAPSULE_RADIUS <= l.judge)
        .min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)).then(a.0.cmp(&b.0)));
    let Some((tid, to)) = target else { return };
    let pos = from + bevy_math::Vec3::Z * crate::overrides::class_skills::SHOT_HEIGHT;
    let spec = crate::projectiles::ProjectileSpec {
        speed: vec![(0.0, l.speed)],
        lifetime: crate::overrides::affixes::CHAIN_LIFETIME,
        radius: l.radius,
        turn_rate: crate::overrides::affixes::CHAIN_TURN_RATE,
        gravity: 0.0,
        attack: crate::buffs::attack_of(world, e, c.attack),
        factor: l.factor,
        element: l.element,
        by: HitSource::Skill(crate::skills::SkillKind::Item),
        pierce: 0,
        chain: l.hits.saturating_sub(1),
        chain_range: l.query,
        pull: 0.0,
        on_hit: None,
        burst: 0.0,
        fuse: false,
        ignore_guard: false,
        knockback: None,
    };
    crate::projectiles::spawn(world, id, spec, pos, (to - pos).normalize_or(bevy_math::Vec3::X), Some(tid));
}

/// The held setup's guard: its angles, and a guard let go before `MinShieldTime` kept up until then.
fn guard(world: &mut World, e: Entity, now: u64) {
    let spec = held_setup(world, e).and_then(|(name, _)| world.resource::<WeaponCombos>().guards.get(&name).copied());
    // Without a known guard the block stays as the input set it (a staff clears it: no guard).
    let Some(spec) = spec else {
        if let Some(mut m) = world.get_mut::<Melee>(e)
            && m.guard_angles.is_some()
        {
            m.guard_angles = None;
        }
        return;
    };
    let spec = Some(spec);
    let Some(input) = world.get::<Melee>(e).map(|m| m.block_input) else { return };
    let mut st = world.get::<ComboState>(e).cloned().unwrap_or_default();
    let held = if input {
        st.guard_from.get_or_insert(now);
        true
    } else {
        let keep = st.guard_from.is_some_and(|from| spec.is_some_and(|g| ((now - from) as f32) * DT < g.min_hold));
        if !keep {
            st.guard_from = None;
        }
        keep
    };
    if let Some(mut m) = world.get_mut::<Melee>(e) {
        m.guard_angles = spec.map(|g| (g.left, g.right));
        m.block_held = held;
    }
    if world.get::<ComboState>(e) != Some(&st) {
        world.entity_mut(e).insert(st);
    }
}

/// The held setup's charge attack: a queued heavy attack starts the charge (an actor without one keeps the generic
/// heavy swing); let go (`SimCommand::ReleaseCharge`) after `MinChargeTime`, or held to `MaxChargeTime`, it
/// releases into the hit.
fn charge(world: &mut World, id: SimId, e: Entity, now: u64) {
    let setup = held_setup(world, e).map(|(name, _)| name);
    let spec = setup.as_ref().and_then(|name| world.resource::<WeaponCombos>().heavies.get(name).cloned());
    let Some(m) = world.get::<Melee>(e).cloned() else { return };
    let mut st = world.get::<ComboState>(e).cloned().unwrap_or_default();
    let before = st.clone();
    let mut melee = m.clone();
    let swing = |windows: Vec<(u32, u32)>, window_factors: Vec<f64>, length: u32, factor: f64, hit_value: f32| Swing { by: HitSource::Swing(crate::AttackKind::Heavy), windows, length, tick: 0, hit: Vec::new(), root: Vec::new(), facing: Vec2::X, factor, guard: None, tags: Vec::new(), window_factors, hit_value };
    match (st.charge.clone(), spec) {
        (None, Some(h)) if melee.queued == Some(crate::AttackKind::Heavy) && melee.swing.is_none() => {
            melee.queued = None;
            melee.charge_release = false;
            // A relic enchant pays its relic energy when it starts; without enough, nothing starts.
            if let Some(en) = &h.enchant {
                let Some((uid, energy)) = relic_energy(world, e).filter(|(_, v)| *v >= en.cost) else {
                    world.resource_mut::<SimEvents>().push(SimEvent::RelicEnchantFailed { id, cancelled: false });
                    if let Some(mut w) = world.get_mut::<Melee>(e) {
                        *w = melee;
                    }
                    return;
                };
                let max = world.get::<RelicEnergy>(e).and_then(|r| r.held).map_or(0, |h| h.1.max);
                if let Some(mut r) = world.get_mut::<RelicEnergy>(e) {
                    r.energy.insert(uid, energy - en.cost);
                }
                world.resource_mut::<SimEvents>().push(SimEvent::RelicEnergy { id, current: energy - en.cost, max });
            }
            st.charge = Some(Charge { setup: setup.clone().unwrap_or_default(), from: now, released: None, let_go: false, cast: false });
            // Charging: nothing hits until the release; the sword and shield's holds its shield up (its guard's angles).
            let mut charging = swing(Vec::new(), Vec::new(), u32::MAX / 4, h.factor, 0.0);
            charging.guard = h.shield.then_some((0, u32::MAX / 4));
            melee.swing = Some(charging);
        }
        (Some(mut c), Some(h)) if c.released.is_none() => {
            if melee.swing.is_none() {
                // Stopped (a stun): the charge is gone.
                st.charge = None;
            } else if h.enchant.is_some() && melee.block_input {
                // An enchant's `CancelInputTags`: the right button cancels it (the energy stays spent).
                st.charge = None;
                melee.swing = None;
                world.resource_mut::<SimEvents>().push(SimEvent::RelicEnchantFailed { id, cancelled: true });
            } else {
                c.let_go |= std::mem::take(&mut melee.charge_release);
                let held = (now - c.from) as f32 * DT;
                if (c.let_go && held + DT * 0.5 >= h.min_charge) || held + DT * 0.5 >= h.max_charge {
                    c.released = Some(now);
                    let (windows, factors) = swing_windows(&h.hits);
                    let end = windows.last().map_or(0, |w| w.1);
                    melee.swing = Some(swing(windows, factors, ticks(h.free).max(end), h.factor, h.hit_value));
                    st.last = Some((c.setup.clone(), HEAVY, now));
                    world.resource_mut::<SimEvents>().push(SimEvent::SwingStarted { id, kind: crate::AttackKind::Heavy });
                }
                st.charge = Some(c);
            }
        }
        // An enchant casts at its `CastCharge` notify, a ground hit lands at its `AOE` notify.
        (Some(mut c), Some(h)) if c.released.is_some_and(|r| (now - r) as f32 * DT + DT * 0.5 >= h.enchant.as_ref().map(|en| en.cast_at).or(h.ground.map(|g| g.at)).unwrap_or(f32::MAX)) && !c.cast && melee.swing.as_ref().is_some_and(|s| s.by == HitSource::Swing(crate::AttackKind::Heavy)) => {
            if let Some(en) = &h.enchant {
                cast_enchant(world, id, e, en);
            }
            if let Some(g) = h.ground {
                ground_hit(world, id, e, g, h.hit_value);
            }
            c.cast = true;
            st.charge = Some(c);
        }
        (Some(c), _) if c.released.is_some() && melee.swing.as_ref().is_none_or(|s| s.by != HitSource::Swing(crate::AttackKind::Heavy)) => st.charge = None,
        (Some(_), None) => st.charge = None,
        _ => {}
    }
    if st.charge.is_none() {
        melee.charge_release = false;
    }
    // The charge's own move effect while it lasts.
    let key = crate::buffs::gameplay_tag("MoveSpeedDebuffGE");
    let rate = setup.as_ref().and_then(|name| world.resource::<WeaponCombos>().heavies.get(name)).map_or(0.0, |h| h.move_rate);
    match (before.charge.is_some(), st.charge.is_some()) {
        (false, true) if rate != 0.0 => {
            let kind = crate::buffs::BuffKind::Affix { key, speed: 0.0, speed_rate: rate, life_steal: 0.0, stacks: 1 };
            crate::buffs::add_buff(world, e, crate::buffs::Buff { kind, skill: crate::skills::SkillKind::Item, ticks_left: u32::MAX / 4, age: 0 });
        }
        (true, false) => {
            if let Some(mut b) = world.get_mut::<crate::buffs::Buffs>(e) {
                b.0.retain(|x| !matches!(x.kind, crate::buffs::BuffKind::Affix { key: k, .. } if k == key));
            }
        }
        _ => {}
    }
    if melee != m
        && let Some(mut w) = world.get_mut::<Melee>(e)
    {
        *w = melee;
    }
    if st != before {
        world.entity_mut(e).insert(st);
    }
}

fn ticks(s: f32) -> u32 {
    (s / DT).round() as u32
}

/// The next swing of the held weapon's combo (`None`: no combo for what it holds): the swing after the last one
/// inside that one's chain window, else the first.
pub(crate) fn combo_swing(world: &mut World, e: Entity) -> Option<Swing> {
    let now = world.resource::<TickCount>().0;
    let (setup, combo) = held_setup(world, e)?;
    let last = world.get::<ComboState>(e).and_then(|s| s.last.clone());
    // An open counter: the heavy one after a perfect guard (else the plain one), before any follow-up.
    let counter = world.get::<ComboState>(e).and_then(|s| s.counter).filter(|c| now <= c.0).and_then(|(_, perfect)| combo.iter().position(|c| c.counter == Some(perfect)).or_else(|| combo.iter().position(|c| c.counter == Some(false))));
    let index = counter.or_else(|| follow_up(world, &setup, &combo, last.as_ref(), world.get::<ComboState>(e).and_then(|s| s.rebound_at), now)).unwrap_or_else(|| combo.iter().position(|c| c.opener).unwrap_or(0));
    let c = &combo[index];
    let mut st = world.get::<ComboState>(e).cloned().unwrap_or_default();
    st.counter = None;
    st.lightning = c.lightning.map(|l| (now + u64::from(ticks(l.at)), l));
    st.last = Some((setup, index as u8, now));
    st.rebound_at = None;
    world.entity_mut(e).insert(st);
    let (windows, window_factors) = if c.hits.is_empty() { (vec![(ticks(c.window.0), ticks(c.window.1).max(ticks(c.window.0) + 1))], Vec::new()) } else { swing_windows(&c.hits) };
    let end = windows.last().map_or(0, |w| w.1);
    Some(Swing { hit_value: c.hit_value, by: HitSource::Swing(crate::AttackKind::Light), windows, length: ticks(c.free_at).max(end), tick: 0, hit: Vec::new(), root: Vec::new(), facing: Vec2::X, factor: c.factor, guard: None, tags: Vec::new(), window_factors })
}

/// The held setup's guard on the actor's [`Melee`] (its angles; held at least `MinShieldTime`), and a tap queued
/// during a swing's chain window starts the next swing at once.
pub fn weapons_tick(world: &mut World) {
    relics_tick(world);
    if !world.contains_resource::<WeaponCombos>() {
        return;
    }
    let now = world.resource::<TickCount>().0;
    let actors: Vec<(SimId, Entity)> = world.query_filtered::<(&SimId, Entity), (With<Melee>, With<crate::Inventory>)>().iter(world).map(|(id, e)| (*id, e)).collect();
    for (id, e) in actors {
        guard(world, e, now);
        charge(world, id, e, now);
        lightning(world, id, e, now);
        let Some(m) = world.get::<Melee>(e) else { continue };
        let (Some(swing), Some(queued)) = (m.swing.as_ref(), m.queued) else { continue };
        // A combo swing, or a charge's release swing (the sword and shield's opens `CanQuickSide4`).
        let released = world.get::<ComboState>(e).and_then(|s| s.charge.as_ref()).is_some_and(|c| c.released.is_some());
        let from_heavy = swing.by == HitSource::Swing(crate::AttackKind::Heavy) && released && queued == crate::AttackKind::Light;
        if swing.by != HitSource::Swing(crate::AttackKind::Light) && !from_heavy {
            continue;
        }
        let Some((setup, combo)) = held_setup(world, e) else { continue };
        let last = world.get::<ComboState>(e).and_then(|s| s.last.clone());
        if follow_up(world, &setup, &combo, last.as_ref(), world.get::<ComboState>(e).and_then(|s| s.rebound_at), now).is_none() {
            continue;
        }
        let Some(next) = combo_swing(world, e) else { continue };
        if let Some(mut m) = world.get_mut::<Melee>(e) {
            let kind = m.queued.take().unwrap_or(crate::AttackKind::Light);
            m.swing = Some(next);
            world.resource_mut::<SimEvents>().push(SimEvent::SwingStarted { id, kind });
        }
    }
}

/// A relic's energy (CONFIRMED: `WID_Relic_Base` and the heirloom relics' own definitions): `MaxAmmo`,
/// `AmmoAutoRecover` a second, `AmmoDisplayCnt` HUD segments; the relic pairs' enchants spend it (`AmmoCost`
/// 30). Its `AmmoType` (3) is the ice staff's, so the HUD shows the ice energy bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelicSpec {
    pub max: u32,
    pub recover: u32,
    pub segments: u8,
}

/// Every relic item's energy, by item id (inserted by the caller, like the staffs' `StaffDb`).
#[derive(Resource, Debug, Default, Clone)]
pub struct RelicDb(pub BTreeMap<i32, RelicSpec>);

/// Reads every relic's weapon definition (`EN_GEAR_EQUIP_SUBTYPE_RELIC`).
pub fn load_relic_db(t: &db_formats::config::generated::ConfigTables) -> anyhow::Result<RelicDb> {
    let mut db = BTreeMap::new();
    for row in t.ue_source_equipment.iter().filter(|r| r.equip_subtype.name() == Some("EN_GEAR_EQUIP_SUBTYPE_RELIC")) {
        let wid = chain(&package_of(&row.ue_path));
        let num = |k: &str| prop(&wid, k).and_then(Value::as_f64);
        let (Some(max), Some(recover)) = (num("MaxAmmo"), num("AmmoAutoRecover")) else { continue };
        db.insert(row.equip_id, RelicSpec { max: max as u32, recover: recover as u32, segments: num("AmmoDisplayCnt").map_or(1, |n| n.clamp(1.0, 5.0) as u8) });
    }
    anyhow::ensure!(!db.is_empty(), "no relic weapon definitions decoded under _data/json");
    Ok(RelicDb(db))
}

/// A relic holder's energy: per relic (item uid), the one in the left hand, ticks to the next recovery.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct RelicEnergy {
    pub energy: BTreeMap<u32, u32>,
    pub held: Option<(u32, RelicSpec)>,
    pub recover_in: u32,
}

impl RelicEnergy {
    /// The held relic's energy, maximum and HUD segments.
    pub fn held_energy(&self) -> Option<(u32, u32, u8)> {
        let (uid, spec) = self.held?;
        Some((self.energy.get(&uid).copied().unwrap_or(0), spec.max, spec.segments))
    }
}

/// Relic energy recovers once a second (the staffs' `GA_TGF_AutoRecoverAmmo` 1 s timer, HYPOTHESIS: the relic is
/// granted the same ability), the held relic's only; a relic seen for the first time starts full (as a staff).
fn relics_tick(world: &mut World) {
    if !world.contains_resource::<RelicDb>() {
        return;
    }
    let actors: Vec<(SimId, Entity)> = world.query_filtered::<(&SimId, Entity), With<crate::Inventory>>().iter(world).map(|(id, e)| (*id, e)).collect();
    for (id, e) in actors {
        let held = world.get::<crate::Inventory>(e).and_then(|inv| inv.worn.get(&EnItemWearingPart::Pair1leftweapon)).and_then(|i| world.resource::<RelicDb>().0.get(&i.item.item_id).map(|s| (i.uid, *s)));
        let Some(mut st) = world.get::<RelicEnergy>(e).cloned().or_else(|| held.map(|_| RelicEnergy::default())) else { continue };
        let before = st.clone();
        if st.held.map(|h| h.0) != held.map(|h| h.0) {
            st.recover_in = ticks(crate::staffs::RECOVER_INTERVAL);
            if let Some((uid, spec)) = held {
                st.energy.entry(uid).or_insert(if crate::overrides::staffs::NEW_STAFF_FULL { spec.max } else { 0 });
            }
            st.held = held;
        }
        if let Some((uid, spec)) = st.held {
            st.recover_in = st.recover_in.saturating_sub(1);
            if st.recover_in == 0 {
                st.recover_in = ticks(crate::staffs::RECOVER_INTERVAL);
                let cur = st.energy.get(&uid).copied().unwrap_or(0);
                let next = (cur + spec.recover).min(spec.max);
                if next != cur {
                    st.energy.insert(uid, next);
                    world.resource_mut::<SimEvents>().push(SimEvent::RelicEnergy { id, current: next, max: spec.max });
                }
            }
        }
        if st != before {
            world.entity_mut(e).insert(st);
        }
    }
}

/// Whether the actor's combo swing is inside its montage's `SwitchSlot` window (a weapon switch may cut it).
pub(crate) fn in_switch_window(world: &World, e: Entity) -> bool {
    let (Some(m), Some(st)) = (world.get::<Melee>(e), world.get::<ComboState>(e)) else { return false };
    let (Some(swing), Some((setup, i, _))) = (m.swing.as_ref(), st.last.as_ref()) else { return false };
    if swing.by != HitSource::Swing(crate::AttackKind::Light) || st.rebound_at.is_some() || *i == HEAVY {
        return false;
    }
    let t = swing.tick as f32 * DT;
    world.get_resource::<WeaponCombos>().and_then(|db| db.combos.get(setup)?.get(usize::from(*i))?.switch_window).is_some_and(|(a, b)| (a..b).contains(&t))
}

/// A combo swing's first hit switches to its rebound montage when it has one that chains (the mace): the swing then
/// lasts until the rebound frees the actor, and its follow-ups come from the rebound's windows. Returns the ticks the
/// swing has left from now.
pub(crate) fn rebound(world: &mut World, e: Entity) -> Option<u32> {
    let now = world.resource::<TickCount>().0;
    let st = world.get::<ComboState>(e)?.clone();
    let (setup, i, _) = st.last.clone()?;
    if st.rebound_at.is_some() || i == HEAVY {
        return None;
    }
    let free = world.get_resource::<WeaponCombos>()?.combos.get(&setup)?.get(usize::from(i))?.rebound.as_ref()?.free_at;
    let mut st = st;
    st.rebound_at = Some(now);
    world.entity_mut(e).insert(st);
    Some(ticks(free).max(1))
}

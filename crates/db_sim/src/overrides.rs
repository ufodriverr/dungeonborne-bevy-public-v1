//! The single overrides file: every gameplay value that has no source in the extracted data.
//! Each constant says why it exists and what would replace it. Keep this file short.

/// Lowest value of the defence term's `D − A + 40` denominator part `D` after defence ignore.
/// HYPOTHESIS: the tooltip Lua calls `math.max(6 - def_ignore_base)` with one argument (a no-op);
/// the second argument was most likely `0`, so defence ignore can't push defence below zero.
/// Source to replace it: the server's `TgfDamageExecution` (not in the client).
pub const DEFENCE_LEVEL_FLOOR: f64 = 0.0;

/// Smallest allowed defence denominator, so `20 / (D − A + 40)` can never divide by zero or flip sign
/// when an attacker's gear level exceeds 40 + D (impossible with the data seen: levels are 0–6).
/// HYPOTHESIS, a safety clamp only.
pub const DEFENCE_DENOMINATOR_MIN: f64 = 1.0;

/// Which defender gear level counts for a hit. HYPOTHESIS: head hits use the helmet's defence
/// gear level, everything else the chest's (monsters carry `HelmetDefGearLevel` and
/// `ChestDefGearLevel`). Footage check pending.
pub const HEAD_HITS_USE_HELMET_LEVEL: bool = true;

/// Where the head starts on a player or training dummy, cm above the feet. HYPOTHESIS: the avatars' models aren't
/// imported; the humanoid monster skeletons' head joints sit at 157-168 cm (`_data/recovered/head_heights.json`), and a
/// level shot from the eye height (`class_skills::SHOT_HEIGHT` above the capsule's centre, 160 cm) meets it.
pub const HUMAN_HEAD_CM: f32 = 160.0;

/// Minor affix pools named by `ExcelAffixTemplate` (ids `1002xx`, `1005xx`, `1009xx`, …) are not in the
/// client's `ExcelAffixPool` (only `ExcelAffixTemplate` mentions them). When a MINOR rule names a missing
/// pool, draw uniformly from the attributes that the item's value template gives a MINOR range at the
/// item's rarity. HYPOTHESIS: keeps every value inside the table ranges, but the real pool weights and
/// attribute subsets are unknown. Replace when the pools turn up.
pub const MINOR_POOL_FALLBACK: bool = true;

/// Fallback loot for a source kind (HYPOTHESIS): only 11 of 305 loot sources have loot in the client
/// tables. Rates are placeholders to tune in playtests; rarity weights are for Rotted, Normal,
/// Uncommon, Enhanced, Epic, Legendary, Unique, Exceptional.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FallbackLoot {
    pub chance: f64,
    pub min_items: u32,
    pub max_items: u32,
    pub rarity_weights: [u64; 8],
}

pub fn fallback_loot(kind: crate::loot::SourceKind) -> FallbackLoot {
    use crate::loot::SourceKind as K;
    let f = |chance, min_items, max_items, rarity_weights| FallbackLoot { chance, min_items, max_items, rarity_weights };
    match kind {
        K::NoLoot => f(0.0, 0, 0, [0; 8]),
        K::Container => f(0.25, 1, 1, [30, 40, 20, 8, 2, 0, 0, 0]),
        K::Chest => f(1.0, 1, 3, [10, 35, 30, 15, 8, 2, 0, 0]),
        K::ChestGold => f(1.0, 2, 4, [0, 10, 30, 30, 20, 8, 2, 0]),
        K::Mimic => f(1.0, 2, 3, [0, 15, 30, 30, 18, 6, 1, 0]),
        K::Monster => f(0.35, 1, 1, [25, 40, 22, 9, 3, 1, 0, 0]),
        K::MonsterHard => f(0.45, 1, 2, [5, 25, 30, 22, 12, 5, 1, 0]),
        K::Boss => f(1.0, 3, 5, [0, 5, 25, 30, 25, 12, 3, 0]),
        K::BossHard => f(1.0, 4, 6, [0, 0, 10, 25, 30, 25, 8, 2]),
        K::LegendWeapon => f(1.0, 1, 1, [0, 0, 0, 0, 0, 1, 0, 0]),
        // The Goblin Thief: rolled at its death and once a second for the 12 s it spills loot while fast
        // (`GA_TGF_DropItem`), so about 7 items from a thief hit under half health. Leaning to Enhanced for its
        // blue loot glow (`GE_TGF_WakeUp_LootBlue`: `FX_Common_Drop_Blue`).
        K::Thief => f(0.5, 1, 1, [0, 10, 30, 40, 15, 5, 0, 0]),
    }
}

/// Character movement constants. None of these are in the client data; values are UE
/// CharacterMovement defaults or guesses from footage, all HYPOTHESIS, to tune in playtests.
pub mod movement {
    /// Gravity, cm/s² (UE default 980).
    pub const GRAVITY: f32 = 980.0;
    /// Jump take-off speed, cm/s (UE `JumpZVelocity` default 420).
    pub const JUMP_Z_VELOCITY: f32 = 420.0;
    /// Sprint multiplier on `speed_move_speed_base`.
    pub const SPRINT_SPEED_FACTOR: f32 = 1.5;
    /// Crouch multiplier on `speed_move_speed_base`.
    pub const CROUCH_SPEED_FACTOR: f32 = 0.5;
    /// Stamina pool and rates (per second).
    pub const STAMINA_MAX: f32 = 100.0;
    pub const SPRINT_STAMINA_PER_S: f32 = 20.0;
    pub const STAMINA_REGEN_PER_S: f32 = 15.0;
    pub const JUMP_STAMINA: f32 = 10.0;
    /// After running out, sprint is locked until stamina is back above this.
    pub const SPRINT_RESUME_STAMINA: f32 = 25.0;
    /// A surface counts as ground when its normal's Z is above this (≈ 45° slope).
    pub const WALKABLE_NORMAL_Z: f32 = 0.7;
    /// Highest ledge a walking character steps onto, cm (UE `CharacterMovementComponent::MaxStepHeight`
    /// default; the game's own value isn't decoded). Needed for the real maps' stairs.
    pub const MAX_STEP_HEIGHT: f32 = 45.0;
    /// How far below the capsule a floor still counts as touched when not walking (cm; UE's floor check
    /// reaches about 2.4 cm, `MAX_FLOOR_DIST`)..
    pub const GROUND_PROBE: f32 = 3.0;
    /// The gap a walking character is snapped down to above the floor (cm; avian's skin width at our
    /// length unit is 1 cm, so the next sweep starts clear of the floor)..
    pub const GROUND_GAP: f32 = 1.0;
    /// A flyer climbs and sinks at its move speed but at least this fast, cm/s (ours: the flying movement
    /// component's vertical speed isn't decoded).
    pub const FLY_MIN_CLIMB: f32 = 150.0;
}

/// Melee timing and reach. The player attack montages and the weapon trace data are blocked by the partial
/// mappings, so every value here is a HYPOTHESIS picked to feel like the footage and the decoded monster
/// windows (e.g. Boar Attack1 active 0.75–1.04 s real time).
pub mod melee {
    /// (wind-up, active, recovery) in seconds.
    pub const LIGHT_TIMING: (f32, f32, f32) = (0.40, 0.20, 0.35);
    pub const HEAVY_TIMING: (f32, f32, f32) = (0.80, 0.25, 0.55);
    /// Heavy attacks multiply the resolved damage (no source for a heavy/light ratio).
    pub const HEAVY_DAMAGE_FACTOR: f64 = 1.5;
    /// Reach from the attacker's capsule axis to the target's capsule surface, cm (sword plus arm).
    pub const REACH: f32 = 170.0;
    /// Half-angle of the swing cone around the aim direction, degrees.
    pub const CONE_HALF_ANGLE_DEG: f32 = 55.0;
    /// Targets whose centre is more than this far above/below the attacker's are not hit, cm.
    pub const VERTICAL_REACH: f32 = 150.0;
    /// Movement speed factor while swinging or blocking.
    pub const ATTACK_MOVE_FACTOR: f32 = 0.6;
    pub const BLOCK_MOVE_FACTOR: f32 = 0.5;
    /// Blocking only stops hits from in front, within this half-angle, degrees.
    pub const BLOCK_HALF_ANGLE_DEG: f32 = 75.0;
}

/// Fighter skills. The ability blueprints that would hold these are blocked by the partial mappings;
/// HYPOTHESIS values.
pub mod skills {
    /// Seconds between Whirlwind hits (the first lands half an interval in).
    pub const WHIRLWIND_HIT_INTERVAL: f32 = 0.5;
    /// Whirlwind cooldown, seconds.
    pub const WHIRLWIND_COOLDOWN: f32 = 20.0;
    /// War Cry and Rend cooldowns, seconds (their abilities are blocked; Rend's starts when it is cast).
    pub const WAR_CRY_COOLDOWN: f32 = 20.0;
    pub const REND_COOLDOWN: f32 = 20.0;
    /// Rend's initial damage as a share of the target's current health (the text says "based on the
    /// target's current health" without a number).
    pub const REND_CURRENT_HEALTH_SHARE: f64 = 0.1;
    /// Rend bleed per metre moved = `Dist Factor` × the triggering hit's damage × this.
    pub const REND_PULSE_HIT_SHARE: f64 = 0.25;
}

/// SwordMaster and Rogue skills. Their abilities are blocked by the mappings; these fill what the
/// tables and skill text leave out. All HYPOTHESIS.
pub mod class_skills {
    /// Cooldowns, seconds (Flying Swords': `class_skills::FLYING_SWORDS_COOLDOWNS`).
    pub const BLADE_WALTZ_COOLDOWN: f32 = 20.0;
    pub const PETRIFYING_POISON_COOLDOWN: f32 = 20.0;
    pub const FLYING_BLADES_COOLDOWN: f32 = 12.0;
    pub const FADE_INTO_VOID_COOLDOWN: f32 = 40.0;
    pub const SWIFT_CONCEALMENT_COOLDOWN: f32 = 20.0;
    pub const SHADOW_VEIL_COOLDOWN: f32 = 45.0;
    /// Height above the capsule centre that shots leave from, cm (eye height, as the player camera).
    pub const SHOT_HEIGHT: f32 = 70.0;
    /// Flying Swords' flight (the pick: `class_skills::FLYING_SWORDS_SELECT_*`).
    pub const FLYING_SWORDS_SPEED: f32 = 1800.0;
    pub const FLYING_SWORDS_TURN_RATE: f32 = 360.0;
    pub const FLYING_SWORDS_LIFETIME: f32 = 2.5;
    pub const FLYING_SWORDS_RADIUS: f32 = 15.0;
    /// Blade Waltz: reach from the caster's capsule (cm), height, seconds between hits.
    pub const BLADE_WALTZ_RADIUS: f32 = 200.0;
    pub const BLADE_WALTZ_HEIGHT: f32 = 150.0;
    pub const BLADE_WALTZ_HIT_INTERVAL: f32 = 1.0;
    /// Flying Blades: fan half-angle (narrow enough that an even fan still hits a target dead ahead
    /// at 5 m), flight.
    pub const FLYING_BLADES_HALF_SPREAD_DEG: f32 = 15.0;
    pub const FLYING_BLADES_SPEED: f32 = 2000.0;
    pub const FLYING_BLADES_LIFETIME: f32 = 1.0;
    pub const FLYING_BLADES_RADIUS: f32 = 8.0;
    /// Petrifying Poison: seconds the coat lasts, seconds a petrify lasts at most, and the planar
    /// distance (cm) within which a hit ends it ("close range").
    pub const POISON_WINDOW: f32 = 15.0;
    pub const PETRIFY_SECONDS: f32 = 5.0;
    pub const PETRIFY_BREAK_RANGE: f32 = 300.0;
    /// Crit damage rate added while in Stealth (the text says only "increased").
    pub const STEALTH_CRIT_DAMAGE: f64 = 0.5;

    // Wizard and Lich. Their GameAbilities (costs, cooldowns, charge times, flight, areas) are
    // blocked by the mappings; only the values in `ClassData` come from the data.
    /// Mana (Wizard) and soul energy (Lich) costs. Charged skills: [stage 1, stage 2], paid on release.
    pub const BLIZZARD_COST: f64 = 30.0;
    pub const FROST_TORMENT_COST: [f64; 2] = [15.0, 25.0];
    pub const ICE_BARRIER_COST: f64 = 25.0;
    pub const ICE_ARMOR_COST: f64 = 20.0;
    /// Cooldowns, seconds (from the cast or the release).
    pub const BLIZZARD_COOLDOWN: f32 = 18.0;
    pub const FROST_TORMENT_COOLDOWN: f32 = 6.0;
    pub const ICE_BARRIER_COOLDOWN: f32 = 30.0;
    pub const ICE_ARMOR_COOLDOWN: f32 = 20.0;
    /// Charged skills: seconds held to reach stage 1 and stage 2; a full charge fires by itself after
    /// this many more seconds.
    pub const CHARGE_STAGES: [f32; 2] = [0.5, 1.5];
    pub const CHARGE_HOLD: f32 = 3.0;
    /// Share of a charged skill's press cost returned when it is cancelled, for the skills other than Pyroblast
    /// (HYPOTHESIS: as Pyroblast's `ReturnEnergyRate` 1.0, which the sim reads; their own abilities aren't decoded).
    pub const CHARGE_CANCEL_REFUND: f64 = 1.0;
    /// Pyroblast and Frost Torment pick the nearest enemy within this range and half-angle of the facing.
    pub const CASTER_TARGET_RANGE: f32 = 3000.0;
    pub const CASTER_TARGET_HALF_ANGLE_DEG: f32 = 30.0;
    /// Pyroblast stage 1: fan half-angle of the homing fireballs (in place of the spline spiral), turn rate
    /// (deg/s), lifetime. Their speed (`WizardQSpeedCurve`), size, explosion and launch delays come from the data.
    pub const PYROBLAST_SPREAD_DEG: f32 = 15.0;
    pub const PYROBLAST_TURN_RATE: f32 = 180.0;
    pub const PYROBLAST_LIFETIME: f32 = 4.0;
    /// Pyroblast stage 2: the large fireball's lifetime (its speed, size and burst come from the data).
    pub const PYROBLAST_BIG_LIFETIME: f32 = 3.0;
    /// Blizzard: the storm starts at the aim's floor point within the selection range (`GA_TGF_Lich_Q`
    /// `MaxDistance`) and then follows it within the guide range (`SecondStageMaxDistance`; sourced, kept here
    /// beside the rest); seconds between damage pulses; the unfolded blizzard's radius, length and height (cm, s).
    pub const BLIZZARD_SELECT_RANGE: f32 = 1200.0;
    pub const BLIZZARD_RANGE: f32 = 2500.0;
    pub const BLIZZARD_PULSE: f32 = 0.5;
    pub const BLIZZARD_UNFOLDED_RADIUS: f32 = 400.0;
    pub const BLIZZARD_UNFOLDED_SECONDS: f32 = 4.0;
    pub const BLIZZARD_HEIGHT: f32 = 200.0;
    /// Frost Torment stage 1: the ice bolt's speed, turn rate, lifetime, radius.
    pub const FROST_BOLT_SPEED: f32 = 2000.0;
    pub const FROST_BOLT_TURN_RATE: f32 = 180.0;
    pub const FROST_BOLT_LIFETIME: f32 = 2.0;
    pub const FROST_BOLT_RADIUS: f32 = 12.0;
    /// Soul energy: a Lich starts with this much; a creature dying within the absorb range (cm) gives
    /// each soul-energy user this much (undead "absorb souls by killing creatures", `RaceIntro_3`).
    pub const INITIAL_SOUL_ENERGY: f64 = 50.0;
    pub const SOUL_PER_KILL: f64 = 20.0;
    pub const SOUL_ABSORB_RANGE: f32 = 2000.0;

    // Priest and DeathKnight. Divine Protection's and Guardian's charges and cooldowns come from
    // their decoded GameAbilities; the rest of their abilities is blocked.
    /// Mana costs (Heal: [stage 1, stage 2], paid on release) and cooldowns, seconds.
    pub const ORACLE_COST: f64 = 30.0;
    pub const HEAL_COST: [f64; 2] = [20.0, 35.0];
    pub const DIVINE_PROTECTION_COST: f64 = 25.0;
    pub const GUARDIAN_COST: f64 = 30.0;
    pub const ORACLE_COOLDOWN: f32 = 8.0;
    pub const HEAL_COOLDOWN: f32 = 1.0;
    /// Oracle: placed at the aim's floor point within this range, it goes off after the delay over the
    /// radius; the Priest channels (can't move or attack) meanwhile.
    pub const ORACLE_RANGE: f32 = 1500.0;
    pub const ORACLE_DELAY: f32 = 1.0;
    pub const ORACLE_RADIUS: f32 = 300.0;
    /// Soul energy: Wraith Ring's cost, cooldown and length; Soul Storm needs this much to switch on and
    /// drains this much per second.
    pub const WRAITH_RING_COST: f64 = 30.0;
    pub const WRAITH_RING_COOLDOWN: f32 = 15.0;
    pub const WRAITH_RING_SECONDS: f32 = 8.0;
    pub const SOUL_STORM_MIN: f64 = 10.0;
    pub const SOUL_STORM_DRAIN: f64 = 10.0;
    /// Height of the DeathKnight's damage ring around him, cm.
    pub const AURA_HEIGHT: f32 = 200.0;
    /// Soul Chain: cooldown, soul energy restored on an enemy hit ("a little"), flight.
    pub const SOUL_CHAIN_COOLDOWN: f32 = 12.0;
    pub const SOUL_CHAIN_SOUL: f64 = 10.0;
    pub const SOUL_CHAIN_SPEED: f32 = 2500.0;
    pub const SOUL_CHAIN_LIFETIME: f32 = 0.8;
    pub const SOUL_CHAIN_RADIUS: f32 = 15.0;

    // Druid. Form cost, drain and speed, damage factors and both E skills' recharge come from the data;
    // the rest of the abilities is blocked.
    /// Both E skills: charges (their abilities are "Double…" abilities with Count0–2 tags, like the
    /// Priest's 2-charge ones, but the count field doesn't decode).
    pub const DRUID_E_CHARGES: u8 = 2;
    /// The change into the panther (s): `GA_TGF_DruidPanther_Q_Human` plays its cast montage at rate 1.5 and waits
    /// out its length over that rate, then 0.156 s, before the panther mesh (CONFIRMED bytecode); the montage's clip
    /// read as the first-person `A_Classes_DruidElf_PantherSkillQ_Elf_1st` (2.0 s; HYPOTHESIS). Meanwhile the Druid
    /// can't move, attack or use skills (HYPOTHESIS: the montage roots; playtest, "an animation you can't skip").
    pub const PANTHER_CHANGE_SECONDS: f32 = 2.0 / 1.5 + 0.156;
    /// Shadow Pounce: shadow-energy cost, cooldown, leap distance and time, landing radius (cm, s).
    pub const POUNCE_COST: f64 = 40.0;
    pub const POUNCE_COOLDOWN: f32 = 3.0;
    pub const POUNCE_DISTANCE: f32 = 500.0;
    pub const POUNCE_SECONDS: f32 = 0.4;
    pub const POUNCE_RADIUS: f32 = 250.0;
    /// Force of Nature (its range: `class_skills::FORCE_OF_NATURE_RANGE`): each of the treant's hits shields the
    /// Druid for this share of the damage, for this long (the text says only that it shields the Druid).
    pub const TREANT_SHIELD_SHARE: f64 = 0.2;
    pub const TREANT_SHIELD_SECONDS: f32 = 10.0;
    /// Rejuvenation lasts 30 s (skill text); its heal per second is `MoveFactorRate` × the Lich-style
    /// weapon heal formula (the GameplayEffect is blocked).
    pub const REJUVENATION_SECONDS: f32 = 30.0;
}

/// Bosses: what their blocked abilities and animations would set.
pub mod bosses {
    /// Attack reach and attack-start range relative to a normal monster's (the capsule stays player-sized).
    pub const REACH_SCALE: &[(&str, f32)] = &[("Werewolf", 1.3), ("Reaper", 1.6), ("Cyclop", 2.0), ("Wendigo", 1.6), ("ShadowKing", 1.6), ("Khazra", 1.5), ("Executioner", 1.5), ("CuresdKing", 1.6)];
    /// One attack: montage name, length (s), hit window (s).
    pub type Attack = (&'static str, f32, (f32, f32));
    /// Attacks for bosses without a decoded hit window: the Executioner has no decoded animations. (Khazra's
    /// HYPOTHESIS window went away: `AM_Khazra_Attack1` decodes with the repaired usmap.)
    pub const KITS: &[(&str, &[Attack])] = &[("Executioner", &[("Executioner_Chop", 3.0, (1.4, 1.7)), ("Executioner_Sweep", 2.5, (0.9, 1.3))])];
}

/// Monster area attacks.
pub mod monster_area {
    /// `FindProperGroundPos(.., 500)`: the ground is looked for down from the target's centre height to this
    /// far below its feet (cm; the 500 is the call's argument, CONFIRMED; how the native function uses it is
    /// HYPOTHESIS).
    pub const GROUND_PROBE: f32 = 500.0;
}

/// Monster AI. The behaviour trees are the game's (`crate::bt`); these fill the AI controller and
/// monster pawn values that aren't decoded (HYPOTHESIS unless marked CONFIRMED).
pub mod ai {
    /// Own trees imported but not yet runnable in the sim: their monsters keep `BT_Monster_Default` until then
    /// (`BT_Shadow_King` ran).
    pub const PENDING_TREES: &[&str] = &[];
    /// Abilities a tree casts by class that the sim can't play yet: `BTT_ActivateAbilityDirectly` passes them as done,
    /// so the rest of the sequence goes on (the Shadow King's `Attack8` summons the Shadow Dragon, not yet a monster
    /// here; failing would stall his phase 4 entry).: the Wendigo's (all built now).
    pub const MISSING_ABILITIES: &[&str] = &[
    ];
    /// Half-angle of the sight cone, degrees, for a monster without a decoded Blueprint (the rest use its
    /// `SightPeripheralVisionAngleDegrees`, 120).
    pub const SIGHT_HALF_ANGLE_DEG: f32 = 60.0;
    /// A player moving faster than this (cm/s) and not crouched can be heard.
    pub const LOUD_SPEED: f32 = 50.0;
    /// Delay between noticing a target and chasing it, seconds.
    pub const ALERT_DELAY: f32 = 0.5;
    /// Start an attack when the target's capsule surface is this close, cm (inside melee reach).
    pub const ATTACK_RANGE: f32 = 140.0;
    /// Attack only when the target is within this angle of the facing, degrees.
    pub const ATTACK_FACING_DEG: f32 = 30.0;
    /// Pause after an attack before chasing again, seconds.
    pub const RECOVER_TIME: f32 = 0.6;
    /// Give up after this long without seeing or hearing the target, seconds.
    pub const LOSE_TARGET_AFTER: f32 = 5.0;
    /// Stealthed players without Rogue data are noticed within these distances, cm (the Rogue's
    /// `Aura_TGF_Rouge_Stealth` values).
    pub const STEALTH_REVEAL: f32 = 300.0;
    pub const STEALTH_REVEAL_Z: f32 = 200.0;
    /// Give up when the target is farther than sight distance times this.
    pub const LEASH_FACTOR: f32 = 1.5;
    /// Dormancy (ours, for performance): a monster wakes when a player is within its sight or hearing
    /// distance (the larger) plus `WAKE_MARGIN`, and sleeps again `SLEEP_MARGIN` beyond that, cm.
    pub const WAKE_MARGIN: f32 = 1000.0;
    pub const SLEEP_MARGIN: f32 = 1000.0;
    /// Eye height above the capsule centre for line of sight, cm.
    pub const EYE_HEIGHT: f32 = 60.0;
    /// CONFIRMED (`BTT_MoveTo` defaults): acceptance radius, cm; flank waypoints are reached at 10x it;
    /// a move that hardly progresses for 0.5 s fails. "Hardly" is ours: below this speed, cm/s.
    pub const MOVE_ACCEPTANCE: f32 = 5.0;
    pub const WAYPOINT_ACCEPTANCE_FACTOR: f32 = 10.0;
    /// Within this distance (cm) of its goal a walker heads straight at it instead of along its turning facing, and a
    /// goal within the last tick's travel counts as reached. HYPOTHESIS: the engine's path following moves the pawn
    /// straight at its goal and ends a move that passes it; walking along the facing with whole-tick steps orbited a 5 cm
    /// goal (the Shadow King circling his centre at 500 cm/s for the whole 10 s `TimeLimit`).
    pub const MOVE_STRAIGHT_NEAR: f32 = 100.0;
    pub const STUCK_TIME: f32 = 0.5;
    pub const STUCK_SPEED: f32 = 5.0;
    /// nav paths (ours; the original walks its navmesh natively): a move goes straight when its grid route
    /// keeps within this distance (cm) of the straight line, nothing blocks at knee or eye height and the goal
    /// is within this rise (cm), else along the route, re-planned when the goal has moved this far (cm) or
    /// after this long (s); a route point counts as passed this close (cm).
    pub const NAV_STRAIGHT_SLACK: f32 = 75.0;
    pub const NAV_STRAIGHT_RISE: f32 = 50.0;
    pub const NAV_REPLAN_DISTANCE: f32 = 150.0;
    pub const NAV_REPLAN_SECONDS: f32 = 1.0;
    pub const NAV_POINT_REACH: f32 = 35.0;
    /// `BP_Tgf_Monster`'s `WanderRadius` default (CONFIRMED), for a kit without Blueprint values.
    pub const WANDER_RADIUS: f32 = 500.0;
    /// CONFIRMED (`BTT_FaceTo` default): facing precision, degrees.
    pub const FACE_PRECISION_DEG: f32 = 10.0;
    /// Turn-speed factor while `BTS_TurnSlowly` runs (the pawn's `TurnSpeedTagLow` value isn't decoded).
    pub const TURN_SLOW_FACTOR: f32 = 0.5;
    /// For a kit without Blueprint values only (every roster monster's `FlankChance`, `FlankPointCount`,
    /// `FlankAngleMin/Max` and `FlankTimeMin/Max` now come from its Blueprint): the flank chance after a melee
    /// attack, waypoints (count range), the angle each turns around the target (degrees) and the time (s).
    pub const FLANK_CHANCE: f64 = 0.3;
    pub const FLANK_POINTS: (u32, u32) = (2, 3);
    pub const FLANK_ANGLE: (f32, f32) = (30.0, 60.0);
    pub const FLANK_TIME: (f32, f32) = (2.0, 4.0);
    /// CONFIRMED struct defaults of a retreat entry (`BTS_Retreat` CDO `RetreatData`): angle off straight
    /// back (degrees) and distance range (cm); used only without the monster's own entries.
    pub const RETREAT_ANGLE: f32 = 60.0;
    pub const RETREAT_DISTANCE: (f32, f32) = (200.0, 300.0);
    /// The combat zone around the spawn point, cm (`AIC.IsInCombatZone`, not decoded).
    pub const COMBAT_ZONE_RADIUS: f32 = 2500.0;
    /// Searching where a lost target was last seen: `AlertSearchDistance` (cm) over a typical walking
    /// speed (cm/s) gives the search time limit; `AlertSearchEndTime` (s) the suspicious wait after.
    pub const ALERT_SEARCH_DISTANCE: f32 = 600.0;
    pub const ALERT_SEARCH_SPEED: f32 = 170.0;
    pub const ALERT_SEARCH_END_TIME: f32 = 2.0;
    /// Monsters that fly: `BTD_CanFly` reads the pawn's `Flyable`, whose per-monster value is in the
    /// undecoded monster Blueprints. The ghost's own tree only ever flies (`BT_Monster_Ghost`).
    pub const FLYERS: &[&str] = &["Bat", "InsectFly", "Ghost"];
    /// Monsters dormant at a spawn point that leaves it to the monster (`EMonsterDormantable` None): the
    /// monster's own default isn't decoded. Bats hang asleep until something comes near.
    pub const DORMANT_BY_DEFAULT: &[&str] = &["Bat"];
    /// A dormant monster notices a player within this distance (cm) when its spawn point sets none (the
    /// median of the castle's `DataMonsterDormantSightRadius` values), any direction, with line of sight.
    pub const DORMANT_SIGHT: f32 = 600.0;
    /// The flying insect's flank (`BTT_FlankBegin_Monster_Inscet_Fly`) reads the pawn's `FlankPointCount`
    /// (+ up to `FlankPointCountRandomDeviation`) and `FlankTimeMin/Max`; not decoded.
    pub const FLY_FLANK_POINTS: (u32, u32) = (3, 4);
    pub const FLY_FLANK_TIME: (f32, f32) = (3.0, 5.0);
}

/// Monster attacks without a decoded montage: the attack's AnimSequence gives its length and sound
/// cues, but the hit window lives in the montage. HYPOTHESIS: the window opens at this fraction of the
/// clip and lasts this long (s) (decoded montages put it at 36-46 %: `AM_Insect_Fly_Attack1` 0.95 of
/// 2.17 s, `AM_Goblin_Axe_Attack2` 0.96 of 2.68 s, `AM_Mimic01_Attack1` 0.70 of 1.52 s).
pub mod monster_clips {
    pub const WINDOW_AT: f32 = 0.4;
    pub const WINDOW_LENGTH: f32 = 0.25;
}

/// Ranged monsters. Since every shooter's projectile ability reads its shot from data
/// (`monsters::projectile_shot`); the HYPOTHESIS shots that stood in for them (crossbow 3000 cm/s over 1500
/// cm, bow with drop, homing wizard bolt, 150 cm fireball) are gone. A kit here adds a shot to a monster
/// whose ranged ability doesn't decode (none today).
pub mod ranged {
    use crate::monsters::Shot;

    /// (name, length s, release s, shot, sound cues (s, event)).
    pub type RangedAttack = (&'static str, f32, f32, Shot, &'static [(f32, &'static str)]);
    pub const KITS: &[(&str, RangedAttack)] = &[];
    /// Shots leave from this high above the capsule centre (cm).
    pub const MUZZLE_HEIGHT: f32 = 50.0;
}

/// Inventory. The backpack size is set at runtime by the game UI (`BP_InventorySlot:SetupGrid`) and is not
/// in the decoded data; HYPOTHESIS.
pub mod inventory {
    /// a dropped item lands this far in front of the actor, cm (ours: the game's drop point isn't in the data).
    pub const DROP_AHEAD: f32 = 60.0;
    pub const BACKPACK_COLS: u8 = 10;
    pub const BACKPACK_ROWS: u8 = 5;
}

/// Loot in the world; HYPOTHESIS values (the interaction and pickup blueprints are blocked).
pub mod world_items {
    /// How far a player can reach a chest or an item (centre to centre), cm.
    pub const INTERACT_RANGE: f32 = 200.0;
    /// Dropped items land this far from the drop point, cm.
    pub const DROP_SPREAD: f32 = 50.0;
    /// Ground items sit this far above the floor, cm.
    pub const ITEM_HEIGHT: f32 = 5.0;
    /// Chest collider half extents, cm.
    pub const CHEST_HALF_EXTENTS: [f32; 3] = [45.0, 30.0, 30.0];
}

/// The greybox dungeon generator. The real maps are blocked by the mappings, so the layout and all
/// of these numbers are ours (HYPOTHESIS), sized like the test room.
pub mod dungeon {
    /// Grid cells (columns, rows) per floor and the cell size, cm.
    pub const GRID: (u32, u32) = (5, 4);
    pub const CELL: f32 = 1400.0;
    /// Floor 2 is built this far along +X from floor 1 (out of every sight and hearing range).
    pub const FLOOR_OFFSET_X: f32 = 20000.0;
    /// Rooms per floor (inclusive range), floor 1 and floor 2.
    pub const ROOMS: [(u32, u32); 2] = [(10, 13), (9, 12)];
    /// Room half size range, cm; how far a room centre may move off its cell centre.
    pub const ROOM_HALF: (f32, f32) = (300.0, 560.0);
    pub const ROOM_JITTER: f32 = 100.0;
    /// Corridor half width, cm (3 m corridors).
    pub const CORRIDOR_HALF_WIDTH: f32 = 150.0;
    /// Chance of an extra corridor between neighbouring rooms not already joined (loops).
    pub const EXTRA_CORRIDOR_CHANCE: f64 = 0.2;
    /// Rooms at least this big (half size, both axes) may get four pillars.
    pub const PILLAR_ROOM_HALF: f32 = 450.0;
    pub const PILLAR_CHANCE: f64 = 0.5;
    pub const PILLAR_HALF: f32 = 40.0;
    /// Groups for the roster monsters that are in no `DT_Monster_*` table (their real placement is in
    /// the blocked maps); same weights inside a group.
    pub const EXTRA_GROUPS: &[(&str, &[(&str, f64)])] = &[("beasts", &[("Rat", 1.0), ("Boar", 1.0)]), ("undead", &[("Zombie", 1.0)])];
    /// Each room takes one spawn group, every group equally likely; its monsters are rolled by the
    /// group's row weights. Seeded from the dungeon seed xor this, apart from the layout stream.
    pub const MONSTER_TYPE_STREAM: u64 = 0x6d6f_6e73_7465_7273;
    /// Floor 2's boss room: the deepest room without an extraction portal. Its boss is rolled from the
    /// elite spawn groups plus this group of map bosses (Cyclops, Wendigo, Shadow King and, the Cursed King
    /// live in the blocked boss maps), every group equally likely; on its own stream.
    pub const MAP_BOSS_GROUP: (&str, &[(&str, f64)]) = ("map bosses", &[("Cyclop", 1.0), ("Wendigo", 1.0), ("ShadowKing", 1.0), ("CuresdKing", 1.0)]);
    pub const BOSS_STREAM: u64 = 0x626f_7373_726f_6f6d;
    /// Monsters per room (inclusive), floor 1 and floor 2.
    pub const MONSTERS_PER_ROOM: [(u32, u32); 2] = [(0, 2), (1, 2)];
    /// Chance a room has a chest; of those, iron below this roll, gold (floor 2 only) below the gold roll.
    pub const CHEST_CHANCE: f64 = 0.55;
    pub const IRON_CHEST_CHANCE: f64 = 0.3;
    pub const GOLD_CHEST_CHANCE: f64 = 0.12;
    /// Extraction portals on floor 2 (the real game opens them over time; ours are always there).
    pub const EXTRACT_PORTALS: usize = 2;
    /// Spawn points keep this far from room walls; clearances around things, cm.
    pub const SPAWN_MARGIN: f32 = 120.0;
    pub const MONSTER_CLEARANCE: f32 = 60.0;
    pub const PORTAL_CLEARANCE: f32 = 90.0;
    /// Capsule centre height for characters standing on the floor, cm (as the test room spawns).
    pub const CAPSULE_CENTER_Z: f32 = 90.0;
}

/// Runs on the real crypt floors. The placement, spawn tables, groups and escape delays are the
/// maps'; these fill what the converted maps lack (HYPOTHESIS).
pub mod crypt {
    /// Where each floor sits in the shared physics world (cm): floor 1 wholly at x < 0 and floor 2 at
    /// x > `dungeon::FLOOR_OFFSET_X`, so `dungeon::floor_of` tells them apart as for the generator.
    pub const FLOOR_OFFSETS: [[f32; 3]; 2] = [[-50_000.0, 0.0, 0.0], [100_000.0, 0.0, 0.0]];
    /// The Gothic Castle's floors (ours): floor 1's play area (map x about -26,000..23,000) moved wholly below x 0
    /// as the crypt's floor 1 is, floor 2 (map x 100,114..172,983) where it is, so `dungeon::floor_of` tells them
    /// apart.
    pub const CASTLE_FLOOR_OFFSETS: [[f32; 3]; 2] = [[-50_000.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
    /// The share of a multi-floor map's floor-1 escape points that lead down when its data has no down portal at all
    /// (HYPOTHESIS: the castle's floor 1 places only blue `BP_EscapeDevice_Blue_01`s, the same level as the one-floor
    /// map 10, so its way to the throne floor in map 10201 isn't in the data; this is the crypt floor 1's red share, 6
    /// of 16).
    pub const DOWN_SHARE_WITHOUT_DATA: f64 = 6.0 / 16.0;
    /// Runs collide only with map pieces within this far (cm) of the walkable grid's extent (ours): past
    /// it is scenery no one reaches. Wider than a jump or a thrown arc leaves the grid.
    pub const COLLIDE_BEYOND_NAV: f32 = 3_000.0;
    /// A spawn point counts as on the floor when a walkable grid node lies this close (cm, sideways and
    /// up/down); points with no node are in rooms the converted scene lacks and spawn nothing.
    pub const ON_GRID: (f32, f32) = (150.0, 150.0);
    /// Floor 1 starts only at player starts that share a walkable area with an escape point, since the
    /// converted scene splits the floor into islands (missing rooms); the original starts anywhere.
    pub const START_NEEDS_ESCAPE: bool = true;
    /// A spawn room with `FixedSpawnNumber` 0 spawns nothing by `BP_ActorSpawnRoom`'s own rule, but crypt
    /// floor 1's red (down) escape points sit only in such a room, and floor 2 is reached through them:
    /// something else (not traced) must spawn them. Such a room keeps all its sets.
    pub const FIXED_ZERO_ROOM_SPAWNS_ALL: bool = true;
    /// Floor 2's extraction devices stand in a room ~115 m east of the rest, which nothing in the
    /// converted map reaches (the way there isn't recovered). Until it is, an extraction portal also
    /// opens at the walkable point farthest from the arrival, in the arrival's area.
    pub const STAND_IN_EXTRACT: bool = true;
    /// Floor 2 extraction devices have no decoded delay: open from the start.
    pub const EXTRACT_DELAY: f32 = 0.0;
}

/// Doors, traps and altars. The loop timings are the blueprint defaults; everything else is HYPOTHESIS:
/// no table or decoded blueprint holds trap damage, trap areas or what a blessing does (the trap and
/// altar logic lives in blocked blueprint graphs and GameplayEffects).
pub mod features {
    /// `BP_LoopTrap` defaults (CONFIRMED): up 2 s, down 2 s, looping. Spikes and the blade use them.
    pub const LOOP_TRAP_UP: f32 = 2.0;
    pub const LOOP_TRAP_DOWN: f32 = 2.0;
    /// `BP_LoopMechanism` defaults (CONFIRMED): up 1 s, down 1 s. HYPOTHESIS: the fire statue
    /// (`BP_Trap_FireStatue_02`, meshes only) is driven by one.
    pub const LOOP_MECHANISM_UP: f32 = 1.0;
    pub const LOOP_MECHANISM_DOWN: f32 = 1.0;
    /// Damage areas, half extents in the trap's frame (x along its facing), cm: a spike patch across a
    /// corridor, the blade's thin slab across it, the fire's reach in front of the statue.
    pub const SPIKES_HALF: [f32; 3] = [75.0, 150.0, 40.0];
    pub const BLADE_HALF: [f32; 3] = [25.0, 150.0, 90.0];
    pub const FIRE_HALF: [f32; 3] = [150.0, 60.0, 100.0];
    /// Damage per hit (not reduced by armour; shields, Holy Shield and Ice Barrier still apply). Spikes
    /// and the blade hit each character once per up phase, the fire every `FIRE_INTERVAL` s.
    pub const SPIKES_DAMAGE: f64 = 20.0;
    pub const BLADE_DAMAGE: f64 = 30.0;
    pub const FIRE_DAMAGE: f64 = 6.0;
    pub const FIRE_INTERVAL: f32 = 0.2;
    /// Door leaf half extents (half width across the corridor, half thickness, half height), cm.
    pub const DOOR_HALF: [f32; 3] = [150.0, 8.0, 130.0];
    /// Altar pedestal half extents, cm.
    pub const ALTAR_HALF: [f32; 3] = [40.0, 40.0, 50.0];
    /// Blessings: attack boost rate, damage reduction, move-speed bonus (cm/s) for `BLESSING_SECONDS`;
    /// healing per second for `HEAL_SECONDS`; Clarity ("energy") fills the power bar and readies every
    /// skill at once.
    pub const BLESSING_SECONDS: f32 = 120.0;
    pub const ATTACK_BOOST: f64 = 0.15;
    pub const DAMAGE_REDUCTION: f64 = 0.15;
    pub const SPEED_BONUS: f32 = 40.0;
    pub const HEAL_PER_SECOND: f64 = 5.0;
    pub const HEAL_SECONDS: f32 = 20.0;
    /// Placement in the generated dungeon: chance a corridor gets a door; chance a corridor without a
    /// door (and not touching the start room) gets a trap, spikes or blade equally; chance a room gets
    /// a fire statue; chance a room gets an altar spawn point (then rolled from `DT_ActorSpawn_Altar_Solo`).
    pub const DOOR_CHANCE: f64 = 0.4;
    pub const TRAP_CHANCE: f64 = 0.3;
    pub const FIRE_STATUE_CHANCE: f64 = 0.15;
    pub const ALTAR_CHANCE: f64 = 0.3;
    /// Corridors shorter than this get no trap (cm).
    pub const TRAP_MIN_CORRIDOR: f32 = 250.0;
    /// Seeded from the dungeon seed xor this, apart from the layout, monster and boss streams.
    pub const FEATURE_STREAM: u64 = 0x6665_6174_7572_6573;
}

/// Run rules; HYPOTHESIS values (the portal and stash blueprints are blocked).
pub mod run {
    /// How close a player must be to use a portal (centre to centre), cm.
    pub const PORTAL_RANGE: f32 = 200.0;
    /// Portals stand this high above their floor point (the interaction point), cm.
    pub const PORTAL_HEIGHT: f32 = 90.0;
    /// Stash grid (columns, rows); the real stash size is set at runtime and isn't in the data.
    pub const STASH: (u8, u8) = (12, 10);
    /// Gold coins in the backpack (`ID_Loot_Gold_100069`: `Icon_CoinPile`, stacks of 999, sold for 1 each, dropped by
    /// the loot tables): carried out, they become gold ("sold automatically on a successful escape", the gold
    /// triggers' explain strings). Not `ExcelItemType` 1001, the account's gold currency, which has no backpack form.
    pub const GOLD_ITEM: i32 = 100069;
    /// The Ritual Scroll a device's use takes (`ID_TeleportScroll.ItemDefId`, the class `BP_ExtractDevice_Base`'s
    /// `ConsumeScroll` names). Sourced, kept here as the gold item is.
    pub const TELEPORT_SCROLL: i32 = 100021;
}

/// Staffs. The values with a source are read by `staffs::load_staff_db`; these have none.
pub mod staffs {
    use crate::staffs::StaffElement;

    /// A staff seen for the first time starts with full energy. HYPOTHESIS: `GA_TGF_AutoRecoverAmmo` calls the
    /// native `AmmoClearByInstance` when it is granted (on equip), which may instead empty it; full keeps a fresh
    /// run playable. Footage of a run start would settle it.
    pub const NEW_STAFF_FULL: bool = true;
    /// A thunder release counts only once the charge montage reached its `Loop` section (0.567 s); earlier it
    /// fails (`AM_Staff_Skill_ThunderFail`). HYPOTHESIS: the release check is the native `ReleaseConditionCheck`.
    pub const THUNDER_RELEASE_NEEDS_LOOP: bool = true;
    /// How far below the aim point the thunder aim looks for the floor (cm). HYPOTHESIS: the selection's
    /// `StartSelection` passes 1800 (also the ability's `ValidProjectileBornHorizontalDist`); its meaning is native.
    pub const THUNDER_GROUND_PROBE: f32 = 1800.0;

    /// A staff's element when its weapon definition doesn't say (the loose read of `WID_Staff_6001` stops before
    /// its anim definition). HYPOTHESIS from the item names and every decoded definition: staff ids `18105TTE`
    /// end in 1 fire, 2 ice, 3 thunder; heirlooms `2201105x0` fire, `2201105x1` thunder.
    pub fn element_by_id(id: i32) -> StaffElement {
        match (id / 1_000_000, id % 10) {
            (181, 2) => StaffElement::Ice,
            (181, 3) | (220, 1) => StaffElement::Thunder,
            _ => StaffElement::Fire,
        }
    }
}

/// The breakable props. Their battle component's stats beyond `TotalHP` come from the server's interactable stats
/// (`InterableStatIdList`), which the client lacks.
pub mod death_zone {
    /// A player whose `DeathZoneTime` reaches the round's `MaxDeathZoneTime` dies. HYPOTHESIS: floor 2's discs carry
    /// no damage effects, only the timer (60 s, shown to the player); what the full timer does is native.
    pub const FULL_TIME_KILLS: bool = true;
}

pub mod destroyables {
    use crate::combat::{DefenceStats, PerElement};

    /// A prop's defence. HYPOTHESIS: no armour (gear level 0) and no resistance.
    pub const DEFENCE: DefenceStats = DefenceStats { chest_def_gear_level: 0.0, helmet_def_gear_level: 0.0, resistance: PerElement([0.0; 6]) };
}

pub mod shadow_dragon {
    /// A target in the Shadow Dragon's fire keeps burning while it stays in any of its patches. HYPOTHESIS: the native
    /// aura's re-application isn't readable (its `StackingPolicy` decodes only as an unresolved enum, 1); false burns
    /// it once per entry, for the effect's 2.5 s.
    pub const REFRESH_WHILE_INSIDE: bool = true;
}

pub mod summon {
    /// A fixed-spot summon's search when its spot is taken: the native `FindUnoccupiedLocationForSpawn` isn't
    /// readable, so (HYPOTHESIS) this many tries within this radius (cm) of the spot.
    pub const FIXED_RADIUS: f32 = 200.0;
    pub const FIXED_TRIES: u32 = 8;
}

pub mod affixes {
    /// The longsword 5001's thunder chain: its homing turn rate, degrees/s (HYPOTHESIS: `B_TGF_Track_ProjectileBase`'s
    /// tracking is native; fast enough to reach a target 8 m away at 7500 cm/s), and how long it may fly, s (no source:
    /// five hops of at most 8 m take about 0.55 s).
    pub const CHAIN_TURN_RATE: f32 = 3600.0;
    pub const CHAIN_LIFETIME: f32 = 2.0;
}

pub mod throwables {
    /// How long a thrown torch may fly before it is lost, s (no source: the torch is launched as a physics pickup
    /// with no lifetime; only a throw into a pit flies that long).
    pub const TORCH_FLIGHT: f32 = 10.0;
}

pub mod arena {
    /// The test arena's monsters come back this long after their death, s (the choice, a dev rule, not the game's).
    pub const RESPAWN_SECONDS: f32 = 5.0;
}

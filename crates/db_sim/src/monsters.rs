//! Monster attributes from `MonsterAttributeCurve` (CurveTable, rows `<Monster>.<Group>.<Attribute>`,
//! one key each; CONFIRMED). Monsters have chest and helmet defence gear levels of their own.

use std::collections::{BTreeMap, HashMap};

use bevy_math::{Vec2, Vec3};

use anyhow::Context;

use crate::combat::{AttackStats, DefenceStats, Element, PerElement};

/// One monster's attributes by short name (`MaxHealthBase`, `RightWeaponAttack`, …).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonsterSheet(pub BTreeMap<String, f64>);

impl MonsterSheet {
    pub fn get(&self, name: &str) -> f64 {
        self.0.get(name).copied().unwrap_or(0.0)
    }

    pub fn max_health(&self) -> f64 {
        self.get("MaxHealthBase")
    }

    pub fn move_speed(&self) -> f64 {
        self.get("MoveSpeedBase")
    }

    pub fn guard_reduce(&self) -> f64 {
        self.get("RightGuardedDamageReduceRate")
    }

    /// Sight and hearing distances (cm) and turn speed (degrees/s) for the AI.
    pub fn senses(&self) -> crate::ai::AiSenses {
        crate::ai::AiSenses { sight: self.get("SightDistanceBase") as f32, hearing: self.get("HearingDistanceBase") as f32, turn_speed: self.get("AngleSpeedBase") as f32 }
    }

    pub fn attack_stats(&self) -> AttackStats {
        let mut pen = PerElement::default();
        for e in Element::ALL {
            pen.set(e, self.get(&format!("ElementResPierce{}", curve_suffix(e))));
        }
        AttackStats {
            right_weapon_attack: self.get("RightWeaponAttack"),
            att_gear_level: self.get("RightAttGearLevel"),
            critical_rate: self.get("CriticalRate"),
            crit_damage_rate: self.get("CritDamageRate"),
            penetration: pen,
            ..Default::default()
        }
    }

    pub fn defence_stats(&self) -> DefenceStats {
        let mut res = PerElement::default();
        for e in Element::ALL {
            res.set(e, self.get(&format!("ElementResistanceRate{}", curve_suffix(e))));
        }
        DefenceStats { chest_def_gear_level: self.get("ChestDefGearLevel"), helmet_def_gear_level: self.get("HelmetDefGearLevel"), resistance: res }
    }
}

fn curve_suffix(e: Element) -> &'static str {
    match e {
        Element::Physical => "None",
        Element::Fire => "Fire",
        Element::Ice => "Ice",
        Element::Thunder => "Thunder",
        Element::Light => "Light",
        Element::Dark => "Dark",
    }
}

/// All monsters in the curve table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonsterDb(pub BTreeMap<String, MonsterSheet>);

impl MonsterDb {
    /// From the strict CUE4Parse JSON export of the CurveTable.
    pub fn from_curve_json(v: &serde_json::Value) -> anyhow::Result<Self> {
        let rows = v.get(0).unwrap_or(v).get("Rows").and_then(|r| r.as_object()).context("no Rows in curve table")?;
        let mut db = MonsterDb::default();
        for (key, row) in rows {
            let (Some((monster, _)), Some((_, attr))) = (key.split_once('.'), key.rsplit_once('.')) else { continue };
            let value = row.get("Keys").and_then(|k| k.get(0)).and_then(|k| k.get("Value")).and_then(serde_json::Value::as_f64).with_context(|| format!("{key}: no key value"))?;
            db.0.entry(monster.to_string()).or_default().0.insert(attr.to_string(), value);
        }
        Ok(db)
    }

    /// A monster's curve row: for a roster monster the row its Blueprint names (the normal Mimic
    /// Book's is `InsectFly`), else `monster` itself. Unreal names match without case (the mimics'
    /// Blueprints name `Mimic01`, the curve's row is `mimic01`).
    pub fn get(&self, monster: &str) -> Option<&MonsterSheet> {
        let row = spec(monster).and_then(blueprint).and_then(|b| b.curve_row.as_deref()).unwrap_or(monster);
        self.0.get(row).or_else(|| self.0.iter().find(|(k, _)| k.eq_ignore_ascii_case(row)).map(|(_, v)| v))
    }
}

/// The decoded monster folders under `_data/`.
const MONSTER_JSON: &str = "json/assets/Dungeonborne/Content/TGF/Character/Monster";

/// The decoded curve table under `_data/json` (gitignored), if present.
pub fn load_monster_db() -> Option<MonsterDb> {
    let bytes = std::fs::read(db_formats::data_path("json/assets/Dungeonborne/Content/TGF/Ability/Data/Attributes/MonsterAttributeCurve.json")).ok()?;
    MonsterDb::from_curve_json(&serde_json::from_slice(&bytes).ok()?).ok()
}

/// One monster attack from its `AM_*_Attack*` montage. Times are real seconds (montage time divided
/// by `RateScale`). CONFIRMED: the windows (`AGR_Attack_NotifyState`) and sound cues
/// (`AN_Tgf_AkEvent_C` -> `AkAudioEvent`) as in the decoded montage.
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterAttack {
    /// Montage asset name, e.g. `AM_Goblin_Axe_Attack1`.
    pub name: String,
    pub length: f32,
    pub windows: Vec<(f32, f32)>,
    /// (time, Wwise event name).
    pub sounds: Vec<(f32, String)>,
    /// A ranged attack: its projectile flies at the start of the first window instead of the swing
    /// hitting anything.
    pub shot: Option<Shot>,
    /// The ability's `TgfAIAbilityRule.ActivateRange` (CONFIRMED since the v5 usmap): the AI starts it
    /// when the target's centre is this close to its own (`BTS_CheckInAbilityRange`). None: a kit not from
    /// the ability set.
    pub range: Option<f32>,
    /// An area attack: at the start of its window it puts fuses on the ground around the target
    /// instead of the swing hitting anything.
    pub area: Option<AreaSpawn>,
    /// A summon: at the start of its window it spawns the kit's [`MonsterKit::summon`] and hits nothing.
    pub summon: bool,
    /// An area it lays (`GA_TGF_Spray_Monster_Base`'s `AreaEffect`: the Shadow King's storms) at the start of its
    /// window, hitting what stands in it for its life.
    pub cloud: Option<Box<crate::staffs::AreaSpec>>,
    /// Its montage's gameplay events (`AN_Tgf_GamePlayEvent`'s `Event Tag`): (seconds in, tag), sent as the swing
    /// reaches them (the Shadow King's `FloorCollapse`).
    pub events: Vec<(f32, String)>,
    /// Its own summon instead of the kit's (the Shadow King's four `Attack5_*` each call up other monsters).
    pub own_summon: Option<Box<crate::summon::Summon>>,
    /// Its ability class (`GA_TGF_M_ShadowKing_Attack5_1_C`: what `BTT_ActivateAbilityDirectly` names). Empty for a kit
    /// not from an ability set.
    pub class: String,
    /// Its rule's `ActivateAngleRange` (degrees, CONFIRMED where the rule sets it): how far off its facing the
    /// target may be when it starts (`BTT_SetAbilityAngleRange`). None: not set (the AI's default).
    pub angle: Option<f32>,
    /// How the AI's `PickAbility` weighs and filters it.
    pub pick: AbilityPick,
    /// An effect it applies at its `fire` notify instead of hitting.
    pub effect: Option<Box<AbilityEffect>>,
    /// A chance to break into another attack partway.
    pub chain: Option<Box<Chain>>,
    /// A shield (`GA_TGF_Shield_Monster_Base`).
    pub guard: Option<Guard>,
    /// An explosion (`GA_TGF_Explode_Monster_Base`): its one-tick window hits around it instead of in front.
    pub burst: Option<Burst>,
    /// A volley (`GA_TGF_Multi_NoAim_NormalProjectile_Monster_Base`): its n-th window fires its n-th shot.
    pub volley: Vec<VolleyShot>,
    /// Multi-pass areas (`GA_TGF_Monster_MultiPassApplyAOE_Base`): its n-th window lays its n-th pass.
    pub aoe: Vec<AoePass>,
    /// A spray (`GA_TGF_Monster_SprayProjectile_Base`: the Lava totem): its window throws it.
    pub spray: Option<Spray>,
    /// An aura area (`GA_TGF_SpawnOverlapArea_Monster_Base`: the Slow totem): its window lays it.
    pub aura: Option<Box<crate::aura_areas::AuraSpec>>,
    /// Its hit's damage factor: the `MoveFactorRate` of its ability's `MeleeWeaponHitGE` (`TgfDamageExecution`'s
    /// modifier, inherited; 1 from `GE_TGF_Damage_Basic` for most, e.g. 0.85 for the Goblin Axe's Attack3, 2 for the
    /// Zombie's Attack3).
    pub factor: f64,
    /// Root motion: the monster's displacement during each tick of the attack, (forward, left) in cm of
    /// its facing, from its clips' `Root` bone tracks through the montage's segments. Empty: it stands
    /// still.
    pub root: Vec<Vec2>,
    /// Gameplay-tag windows (s from the start, tag) from its `AN_Tgf_GameplayTagOperation_C` states (the
    /// Hammer monsters' block owns `TGF.AI.ActivatingAbility.Movable` 0.5-4.07 s, which lets `BT_Ability`'s
    /// background move run).
    pub tags: Vec<(f32, f32, String)>,
}

/// An explosion (`GA_TGF_Explode_Monster_Base` bytecode, CONFIRMED): at its `Fire` notify, every living character
/// within `Radius` of its `FireSocketName` socket, no more than `Height` above or below it and with no wall in
/// between (`BlockCheck` true: a line trace to it), takes its `MeleeWeaponHitGE` (the damage effect's
/// `MoveFactorRate` = `factor`). HYPOTHESIS: the socket at the monster's centre; monsters don't hit monsters (the
/// overlap has no team check; `ApplyMeleeWeaponEffect` is native).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burst {
    pub radius: f32,
    pub height: f32,
    pub factor: f64,
}

/// A monster shield's timeline, seconds: it blocks in [`from`, `until`) of the attack; its montage plays to
/// `loop_to`, repeats [`loop_from`, `loop_to`) (`ChargeLoop`) and at `release_at` jumps to `loop_to`
/// (`ChargeRelease`) and plays out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guard {
    pub from: f32,
    pub until: f32,
    pub loop_from: f32,
    pub loop_to: f32,
    pub release_at: f32,
}

impl MonsterAttack {
    /// The montage time at `t` seconds into the attack (a shield's loop repeats; others play straight).
    pub fn montage_time(&self, t: f32) -> f32 {
        match self.guard {
            Some(g) if t >= g.release_at => g.loop_to + (t - g.release_at),
            Some(g) if t >= g.loop_from && g.loop_to > g.loop_from => g.loop_from + (t - g.loop_from) % (g.loop_to - g.loop_from),
            _ => t,
        }
    }
}

/// A monster Blueprint's values (CONFIRMED since the v6 usmap; `_data/recovered/monster_bp.json`). None:
/// not set in the class or its parents. A Blueprint `Retreats` entry (`TGF_FRetreatData`; CONFIRMED use
/// in `BT_Retreat`, `BTS_Retreat`, `BTT_FindRetreatLocation`, `BTT_RetreatAbility`).
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Retreat {
    pub chance: f64,
    /// Degrees either side of straight away from the target.
    pub angle: f32,
    pub distance_min: f32,
    pub distance_max: f32,
    /// A walker's point is then a random navigable point this close to it (cm).
    pub location_radius: f32,
    /// Its move speed while retreating (cm/s; 0: unchanged).
    pub speed: f32,
    /// Keeps facing the target while it backs off.
    pub focus_target: bool,
    /// After it gets there: this chance of starting `ability` (directly, no rule check).
    pub ability_chance: f64,
    pub ability: Option<String>,
}

/// A Blueprint ability selector: (ability class, chance) entries.
pub type Selector = Vec<(Option<String>, Option<f64>)>;

/// How the AI controller's `PickAbility` weighs and filters an ability (CONFIRMED from the
/// `AIC_Monster_Default` bytecode and the ability rules): a roll weighted by the selector's `Chance` among the
/// abilities that are off cooldown and pass their rule's filters. Range and facing are not part of the pick.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AbilityPick {
    /// Its `Chance` in the Blueprint's `AbilitySelector` (0: not in it, never picked from it).
    pub chance: f64,
    /// Its `CooldownGameplayEffectClass`'s duration (s; 0: none).
    pub cooldown: f32,
    /// The rule's `FilterOp` 1: any filter passing will do (0: all must pass).
    pub any: bool,
    /// The rule's self filters (`Filters`).
    pub filters: Vec<SelfFilter>,
    /// The rule's `TargetType` 0: no target (a self ability; always "in range").
    pub self_target: bool,
    /// A `Range` target filter: (max, min) cm; the target must be nearer than max and, if min > 0, farther than min.
    pub target_range: Option<(f32, f32)>,
    /// A `Friendly` target filter: it targets an ally (none of these abilities is in a kit yet).
    pub friendly: bool,
}

/// An ability rule's self filter (`EAbilityFilter`, CONFIRMED order). Counts are strict: min < n < max.
#[derive(Debug, Clone, PartialEq)]
pub enum SelfFilter {
    None,
    /// Own health at most this share (0-1).
    HealthLow(f64),
    /// Own health at least this share (0-1).
    HealthHigh(f64),
    /// Allies within the radius (cm): (radius, min, max).
    FriendlyCount(f32, f64, f64),
    /// Foes within the radius (cm): (radius, min, max).
    HostileCount(f32, f64, f64),
    /// A boss (`ActorType` 13).
    Boss,
    /// A foe within the radius (cm) more than 90 degrees off its facing.
    EnemyBehind(f32),
    /// Its living summons: (min, max).
    SummonsCount(f64, f64),
    /// Has any of the rule's tags (`true`) or none of them (`false`); tag ids ([`crate::buffs::gameplay_tag`]).
    Tags(bool, Vec<u32>),
}

/// An ability that applies a gameplay effect instead of hitting (CONFIRMED values): at its montage's `fire`
/// notify, to itself (`TargetSelf`), to an ally (its rule targets `Friendly`), or to every monster within
/// `Radius` that has the `RequiredTags` and none of the `BlockedTags` (`GA_TGF_ApplyAreaEffect_Monster_Base`).
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityEffect {
    pub to: EffectTo,
    /// The effect's duration (s).
    pub seconds: f32,
    /// Its `MoveSpeedBase` add (cm/s).
    pub speed: f32,
    /// The tags it grants (`InheritableOwnedTagsContainer`), with their parents.
    pub tags: Vec<u32>,
    /// A `MoveSpeedRate` multiplier it sets, as a slow: (1 - multiplier) (the Khazra's 0.5).
    pub slow: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EffectTo {
    Own,
    /// The nearest other living monster within this range (cm; HYPOTHESIS: the rule's `Friendly` target list
    /// is in overlap order, not decoded, so the nearest).
    Ally(f32),
    /// Within `Radius` of the caster: monsters (`Hostile` false), or (`Hostile`: the Khazra's Attack6_2) the
    /// characters not on its team, i.e. players and what isn't a monster.
    Area { radius: f32, required: Vec<u32>, blocked: Vec<u32>, hostile: bool },
}

/// A montage chain ("Quickside", CONFIRMED `GA_TGF_Melee_Monster_Base` / `TryTriggerQuickside` bytecode): when the
/// montage's `DoMonsterQuickside` notify window begins, the ability rolls `RandomFloatInRange(0, 100) <=
/// DoPercent`; on success it ends and one of its `MultiAbilityToActivate` (weighted by `ActivatePercent`) is
/// activated at once, with no range or facing check (its cooldown still applies).
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    /// The notify's time in the montage (s).
    pub at: f32,
    /// `DoPercent` (0-100).
    pub percent: f64,
    /// (kit attack index, `ActivatePercent`).
    pub to: Vec<(u8, f64)>,
}

/// An alternative ability selector (`AbilitySelectorAlternative`): used instead of the Blueprint's
/// `AbilitySelector` while the monster has all of `tags`; `chances` by kit attack index.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Alternative {
    pub tags: Vec<u32>,
    pub chances: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
pub struct MonsterBp {
    #[serde(default)]
    pub folder: String,
    pub curve_row: Option<String>,
    #[serde(default)]
    pub flyable: bool,
    #[serde(default)]
    pub dormantable: bool,
    pub ability_interval: Option<f32>,
    /// `AbilityRange`: `GetAbilityActivateRange` of an ability without an AI rule (unset in a class whose chain stops
    /// short of `BP_Tgf_Monster`, whose default is 150).
    #[serde(default)]
    pub ability_range: Option<f32>,
    /// Its NPC state class's object path (`TgfNpcStateManager.InitialState`).
    #[serde(default)]
    pub npc_state: Option<String>,
    pub threat_lost_time: Option<f32>,
    pub threat_lost_distance: Option<f32>,
    pub dormant: Option<BpDormant>,
    #[serde(default)]
    pub hard: bool,
    /// `Wanderable`, `WanderInterval` and its random deviation (s).
    #[serde(default)]
    pub wanderable: bool,
    #[serde(default)]
    pub wander_interval: Vec<Option<f32>>,
    /// Its mesh component's `RelativeScale3D` (uniform in every class: the Treant 0.5, the Wendigo 1.3).
    pub mesh_scale: Option<f32>,
    /// `SightPeripheralVisionAngleDegrees` (`BP_Tgf_Monster` sets 120 and no class overrides it): how far off
    /// its facing it sees, degrees.
    pub sight_angle: Option<f32>,
    /// Its parent class (a class without abilities of its own plays its parent's).
    pub parent: Option<String>,
    /// `AbilitySelector`: (ability class, chance) the AI picks from.
    #[serde(default)]
    pub abilities: Vec<(Option<String>, Option<f64>)>,
    /// `AbilityIntervalRandomDeviation` (s): the pause after an ability varies by up to this either way.
    #[serde(default)]
    pub ability_interval_deviation: Option<f32>,
    /// `AbilitySelectorAlternative`: (gameplay tags, selector) used instead while the monster has all the tags
    /// (recorded, not used yet: the sim has no gameplay tags).
    #[serde(default)]
    pub alternatives: Vec<(Vec<String>, Selector)>,
    /// The ability sets its attribute component grants (`DefaultAbilitySetsToGrant`, object paths: the
    /// actor-tag sets such as `AbilitySet_ActorTagGoblin`).
    #[serde(default)]
    pub ability_sets: Vec<String>,
    /// `RetreatChance` after an ability (`PickAction`; unset 0).
    #[serde(default)]
    pub retreat_chance: Option<f64>,
    /// `RetreatRange`: after an ability it retreats from a target nearer than this (`BTT_CheckRetreatOnDistance`).
    #[serde(default)]
    pub retreat_range: Option<f32>,
    /// `Retreats` (`PickRetreat` rolls their `Chance`).
    #[serde(default)]
    pub retreats: Vec<Retreat>,
    /// `FlankChance` after an ability unless it has a `FlankBlockedTags` tag (unset 0).
    #[serde(default)]
    pub flank_chance: Option<f64>,
    #[serde(default)]
    pub flank_blocked_tags: Vec<String>,
    /// `FlankPointCount` plus or minus `FlankPointCountRandomDeviation`.
    #[serde(default)]
    pub flank_point_count: Option<i32>,
    #[serde(default)]
    pub flank_point_deviation: Option<i32>,
    /// `FlankDirectionConsistent`: every flank point on one side (else each its own).
    #[serde(default)]
    pub flank_direction_consistent: Option<bool>,
    /// `FlankAngleMin/Max` (degrees each point turns around the target), `FlankTimeMin/Max` (s).
    #[serde(default)]
    pub flank_angle: (Option<f32>, Option<f32>),
    #[serde(default)]
    pub flank_time: (Option<f32>, Option<f32>),
    /// `FlankSpeed`: its move speed while flanking (cm/s; unset or 0: unchanged).
    #[serde(default)]
    pub flank_speed: Option<f32>,
    /// `WanderRadius` (cm; the spawn point's own radius wins), `SpeedLow` (its move speed while patrolling or
    /// wandering, cm/s; unset or 0: unchanged) and the wander idle (`WanderIdleChance`, `WanderIdleDelay`,
    /// `WanderIdleDelayRandomDeviation`).
    #[serde(default)]
    pub wander_radius: Option<f32>,
    #[serde(default)]
    pub speed_low: Option<f32>,
    #[serde(default)]
    pub wander_idle: Vec<Option<f32>>,
    /// fleeing: `FleeHealth`, `FleeChance`, `FleeAngle` (deg), `FleeDistanceMin/Max` (cm), `FleePointRadius`
    /// (cm), `FleePointCount`, `FleePointHoldTimeMin/Max` (s).
    #[serde(default)]
    pub flee: Vec<Option<f32>>,
    /// Voices (Wwise events): `EnterCombatSound`, `AliveLoopSound`, its hit-react montages' sounds (light,
    /// heavy), and its death sounds (HYPOTHESIS: matched by name, `monster_blueprints.py`).
    #[serde(default)]
    pub combat_sound: Option<String>,
    #[serde(default)]
    pub alive_loop_sound: Option<String>,
    #[serde(default)]
    pub hit_sounds: Vec<String>,
    #[serde(default)]
    pub hit_heavy_sounds: Vec<String>,
    #[serde(default)]
    pub death_sounds: Vec<String>,
    /// `ActorType` (12 the elites, 13 the bosses, 14 the rest) and `EvadeTime` (s).
    #[serde(default)]
    pub actor_type: Option<u8>,
    #[serde(default)]
    pub evade_time: Option<f32>,
    /// `CombatLeaveTime` (s) and `CombatLeaveDistance` (cm) (5 s and 6000 cm from `BP_Tgf_Monster`; 9999 for the
    /// Shadow King and his summons).
    #[serde(default)]
    pub combat_leave: Vec<Option<f32>>,
}

impl MonsterBp {
    /// `IsEvadable` (CONFIRMED bytecode): `ActorType` 12.
    pub fn evadable(&self) -> bool {
        self.actor_type == Some(12)
    }
}

/// A Blueprint's `Dormants[0]`: how it perceives while dormant.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
pub struct BpDormant {
    pub sight_radius: Option<f32>,
    pub sight_angle: Option<f32>,
    pub senses: Option<bool>,
    pub gravity: Option<bool>,
    /// Hidden while dormant (the Goblin Thief).
    pub invisible: Option<bool>,
    pub attackable: Option<bool>,
    /// `WakeUpAbility`: the ability that plays its wake clip.
    pub wake_ability: Option<String>,
}

/// Every monster Blueprint by class name, read once.
pub fn blueprints() -> &'static BTreeMap<String, MonsterBp> {
    static BPS: std::sync::OnceLock<BTreeMap<String, MonsterBp>> = std::sync::OnceLock::new();
    BPS.get_or_init(|| std::fs::read(db_formats::data_path("recovered/monster_bp.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default())
}

/// Roster rows whose Blueprint the folder and curve row don't single out (both totems live in `M_Totem` and
/// name the curve row `Totem`; the Moose Ghost names the Moose's).
pub const CLASSES: &[(&str, &str)] = &[("TotemIceBolt", "BP_Tgf_IceBolt_Totem_C"), ("TotemSummon", "BP_Tgf_Summon_Totem_C"), ("TotemLava", "BP_Tgf_Lava_Totem_C"), ("TotemSlow", "BP_Tgf_Slow_Totem_C"), ("MooseGhost", "BP_TGF_Monster_Moose_Ghost_C")];

/// A roster monster's Blueprint (by [`CLASSES`], else): the normal-mode class in its folder whose attribute row is its curve row
/// (the variants named `idle`, `Tutorial`, `Loot`, `Special` or `Summon` only when nothing else fits); failing that, the
/// folder's one normal-mode class (`BP_TGF_Monster_Mimic_Book` names `InsectFly`).
pub fn blueprint(spec: &MonsterSpec) -> Option<&'static MonsterBp> {
    if let Some((_, class)) = CLASSES.iter().find(|c| c.0 == spec.row) {
        return blueprints().get(*class);
    }
    let mine = |b: &&MonsterBp| b.folder == spec.dir && !b.hard;
    let fits = |b: &&MonsterBp| mine(b) && b.curve_row.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(spec.row));
    let variant = |n: &str| ["idle", "Tutorial", "Loot", "Special", "Summon"].iter().any(|v| n.contains(v));
    let bps = blueprints();
    let main: Vec<_> = bps.iter().filter(|(n, b)| mine(b) && !variant(n)).collect();
    bps.iter()
        .find(|(n, b)| fits(b) && !variant(n))
        .or_else(|| bps.iter().find(|(_, b)| fits(b)))
        .or_else(|| (main.len() == 1).then(|| main[0]))
        .map(|(_, b)| b)
}

/// Where a monster's head starts, cm above its feet: its model's head joint in the bind pose
/// (`_data/recovered/head_heights.json`), by its roster row. `None`: a body without a head joint (the Beholder, the
/// book mimic, the tentacle, the totems, the virus) or an unknown row.
pub fn head_of(row: &str) -> Option<f32> {
    #[derive(serde::Deserialize)]
    struct Head {
        head: f32,
    }
    static HEADS: std::sync::OnceLock<BTreeMap<String, Head>> = std::sync::OnceLock::new();
    let heads = HEADS.get_or_init(|| std::fs::read(db_formats::data_path("recovered/head_heights.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    let dir = ROSTER.iter().chain(BOSSES).find(|s| s.row == row)?.dir;
    heads.get(dir).map(|h| h.head)
}

/// One clip's root track (`_data/recovered/root_motion.json`): times (s), forward and left (cm); a clip that climbs or
/// turns also up (cm) and its heading (degrees, the game's yaw).
#[derive(serde::Deserialize)]
pub(crate) struct RootTrack {
    t: Vec<f32>,
    f: Vec<f32>,
    s: Vec<f32>,
    #[serde(default)]
    u: Vec<f32>,
    #[serde(default)]
    y: Vec<f32>,
}

impl RootTrack {
    /// The root at clip time `time`: where (x forward, y right, z up, cm) and its heading (rad).
    pub(crate) fn flight(&self, time: f32) -> (Vec3, f32) {
        let i = self.t.partition_point(|&x| x <= time).clamp(1, self.t.len().max(2) - 1);
        let (a, b) = (i - 1, i.min(self.t.len() - 1));
        let k = ((time - self.t[a]) / (self.t[b] - self.t[a]).max(1e-6)).clamp(0.0, 1.0);
        let lerp = |v: &[f32]| if v.is_empty() { 0.0 } else { v[a] + (v[b] - v[a]) * k };
        (Vec3::new(lerp(&self.f), -lerp(&self.s), lerp(&self.u)), lerp(&self.y).to_radians())
    }

    fn at(&self, time: f32) -> Vec2 {
        let i = self.t.partition_point(|&x| x <= time);
        match i {
            0 => Vec2::new(self.f[0], self.s[0]),
            i if i >= self.t.len() => Vec2::new(self.f[self.t.len() - 1], self.s[self.t.len() - 1]),
            i => {
                let k = ((time - self.t[i - 1]) / (self.t[i] - self.t[i - 1]).max(1e-6)).clamp(0.0, 1.0);
                Vec2::new(self.f[i - 1], self.s[i - 1]).lerp(Vec2::new(self.f[i], self.s[i]), k)
            }
        }
    }
}

/// Clip `clip`'s root track, if it has one.
pub(crate) fn root_track(clip: &str) -> Option<&'static RootTrack> {
    root_tracks().get(clip)
}

/// Every clip's root track, read once.
fn root_tracks() -> &'static HashMap<String, RootTrack> {
    static TRACKS: std::sync::OnceLock<HashMap<String, RootTrack>> = std::sync::OnceLock::new();
    TRACKS.get_or_init(|| std::fs::read(db_formats::data_path("recovered/root_motion.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default())
}

/// An attack's per-tick root motion ([`MonsterAttack::root`]) over `length` s of the montage.
fn root_motion(segments: &[(String, MontageSegment)], length: f32) -> Vec<Vec2> {
    let tracks = root_tracks();
    if !segments.iter().any(|(c, _)| tracks.contains_key(c)) {
        return Vec::new();
    }
    let at = |t: f32| clip_time(segments, t).and_then(|(c, ct)| tracks.get(c).map(|tr| (c.to_string(), tr.at(ct))));
    let n = (length / crate::DT).ceil() as usize;
    (0..n)
        .map(|k| match (at(k as f32 * crate::DT), at((k + 1) as f32 * crate::DT)) {
            (Some((c0, a)), Some((c1, b))) if c0 == c1 => b - a,
            _ => Vec2::ZERO,
        })
        .collect()
}

/// `GA_TGF_AreaSpawnProjectile_Monster_Base` (CONFIRMED from its bytecode and the ability's defaults): at
/// the montage's `SpawnProjectile` notify, `count` static-collision projectiles around the target's
/// location, the i-th at yaw `i * 360/count + rand(0..=360/count)` degrees and a distance
/// `rand(radius_min..=radius_max)`, each put on the ground (`FindProperGroundPos`, 500 cm). Each bursts once
/// `fuse` s later (`EnableCollisionDelay`) as the engine's basic cylinder (100 cm across and high, raised 50
/// cm: `size` 50), with the ability's damage effect (`MoveFactorRate` `factor`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaSpawn {
    pub count: u32,
    pub radius_min: u32,
    pub radius_max: u32,
    pub fuse: f32,
    pub size: f32,
    pub factor: f64,
}

/// A ranged monster attack's projectile: HYPOTHESIS values in [`crate::overrides::ranged`] for the shooters
/// whose ranged abilities don't decode, else from the ability's data ([`projectile_shot`]). It flies at the
/// AI's target; its hit goes through `resolve_hit` with the shooter's curve stats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    /// cm/s
    pub speed: f32,
    /// The monster shoots only at targets within this distance (cm); the projectile flies a bit farther.
    pub range: f32,
    /// Downward acceleration, cm/s² (the shooter aims up for it).
    pub gravity: f32,
    /// Homing turn rate, degrees per second (0 = straight).
    pub turn_rate: f32,
    /// Collision radius, cm.
    pub radius: f32,
    /// Burst radius on impact, cm (0 = one target).
    pub burst: f32,
    /// Acceleration along its flight (cm/s²) up to `max_speed` (`ShouldAcc` / `AccMag`); 0 = constant.
    pub accel: f32,
    pub max_speed: f32,
    /// Seconds it flies (`MaxExistTime`); None: `range` × 1.2 at `speed`.
    pub lifetime: Option<f32>,
    /// Launched this far ahead of and above the shooter's centre (its ability's `ForwardOffset`, `UpOffset`, cm)
    /// instead of from the muzzle.
    pub offset: Option<(f32, f32)>,
    /// The ability's `AimDegreeConfig` (`StopAim`'s `CheckFixing`): aimed at the target only when the target is
    /// within this many degrees of the shooter's facing, else flown straight ahead. None: always at the target.
    pub aim_degrees: Option<f32>,
    /// A slow its hit leaves: move speed × (1 - rate) for seconds (a timed `MoveSpeedRate` effect).
    pub slow: Option<(f64, f32)>,
    /// Damage factor (its damage effect's `MoveFactorRate`).
    pub factor: f64,
    /// A throw (`GA_TGF_ThrowProjectile_Monster_Base`: the Cyclops' rock): its `Speed`, the first launch speed
    /// its solver tries ([`toss_velocity`]).
    pub toss: Option<f32>,
    /// `bIgnoreGuard` on its class: a raised guard doesn't reduce it.
    pub ignore_guard: bool,
    /// `DelayLaunchTime` on its class (`B_TGF_Normal_ProjectileBase` bytecode, CONFIRMED): it hangs where it was
    /// spawned (movement off, still touching) this many seconds, then flies along its spawn facing. 0: none.
    pub delay: f32,
}

/// One shot of a volley (`GA_TGF_Multi_NoAim_NormalProjectile_Monster_Base` bytecode, CONFIRMED): each
/// `SpawnProjectile` notify spawns the next `SpawnConfig` entry at the caster's position plus `forward`, `up` and
/// `right` cm along his own axes, flying along his facing turned `pitch` degrees about his right axis, then `yaw`
/// about his up axis; no aim at a target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolleyShot {
    pub forward: f32,
    pub up: f32,
    pub right: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub shot: Shot,
}

/// One `AOEConfig` entry of a multi-pass area ability (`GA_TGF_Monster_MultiPassApplyAOE_Base` bytecode, CONFIRMED):
/// at its notify the caster's threat targets are sorted (`GetSortedTargets(SelectType, bAsc)`; every one passes
/// `CheckTargetValid`, whose failed checks all return true) and `max` areas are laid, all at once, or one now and
/// one every `interval` s: the n-th on the ground under the n-th target (`FindProperGroundPos`, `ground_depth` cm),
/// past the last as `overflow` says.
#[derive(Debug, Clone, PartialEq)]
pub struct AoePass {
    pub max: u32,
    /// `Radius`: how far from the caster a `RandomPos` overflow lands, cm.
    pub radius: f32,
    pub interval: f32,
    pub overflow: AoeOverflow,
    pub ground_depth: f32,
    pub area: crate::staffs::AreaSpec,
}

/// `ETgfMonsterTargetOverflowType`: the areas past the last target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AoeOverflow {
    /// A random point within `Radius` of the caster.
    RandomPos,
    /// The targets again, in order.
    MultiTrigger,
    /// Any other value: none.
    Skip,
}

/// A spray of projectiles (`GA_TGF_Monster_SprayProjectile_Base` bytecode, CONFIRMED): at its `SpawnName` notify, a
/// random `MinCount..=MaxCount` of them from the caster's centre plus `CenterOffsetForward` / `Right` / `Up` along its
/// axes, each along its up axis tipped a random `VerticalMin..MaxDegree` towards its facing (about its right axis),
/// then turned a random `HorizantalMin..MaxDegree` about the vertical, at a random `MinVelocity..MaxVelocity`; none
/// aimed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spray {
    pub count: (u32, u32),
    pub vertical: (f32, f32),
    pub horizontal: (f32, f32),
    pub speed: (f32, f32),
    pub forward: f32,
    pub right: f32,
    pub up: f32,
    pub shot: Shot,
}

/// A spray ability's notify time (real s) and [`Spray`], its values from the ability and its base class.
fn spray_of(p: &serde_json::Value, ga: &str, v: &serde_json::Value, range: f32) -> Option<(f32, Spray)> {
    let num = |key: &str| p.get(key).and_then(|x| x.as_f64()).or_else(|| class_default(ga, key)?.as_f64());
    let at = notify_window_at(v, p["SpawnName"].as_str().map(str::to_string).or_else(|| class_default(ga, "SpawnName")?.as_str().map(str::to_string))?.as_str())?;
    let pair = |a: &str, b: &str| Some((num(a)? as f32, num(b)? as f32));
    let speed = pair("MinVelocity", "MaxVelocity")?;
    // A lobbed projectile (gravity) reads as a throw: its speed is the spray's own.
    let shot = projectile_shot(&serde_json::json!({ "SpawnActor": p["SpawnActor"], "Speed": speed.1 }), range)?;
    let spray = Spray {
        count: (num("MinCount")? as u32, num("MaxCount")? as u32),
        vertical: pair("VerticalMinDegree", "VerticalMaxDegree")?,
        horizontal: pair("HorizantalMinDegree", "HorizantalMaxDegree")?,
        speed,
        forward: num("CenterOffsetForward").unwrap_or(0.0) as f32,
        right: num("CenterOffsetRight").unwrap_or(0.0) as f32,
        up: num("CenterOffsetUp").unwrap_or(0.0) as f32,
        shot,
    };
    Some((at, spray))
}

/// An `AOEConfig` entry's notify time (real s) and [`AoePass`].
fn aoe_pass(entry: &serde_json::Value, v: &serde_json::Value) -> Option<(f32, AoePass)> {
    let field = |name: &str| entry.as_object()?.iter().find(|(k, _)| k.split('_').next() == Some(name)).map(|(_, v)| v.clone());
    let num = |name: &str| field(name).and_then(|v| v.as_f64());
    let at = notify_window_at(v, field("NotifyName")?.as_str()?)?;
    let area = field("SpawnActor")?["ObjectPath"].as_str().map(|s| s.split('.').next().unwrap_or(s)).and_then(crate::staffs::overlap_area)?;
    // Only threat targets are known here (the Wendigo's both passes): one per monster, the AI's target.
    if field("SelectType")?.as_str()? != "ETgfMonsterSelectTargetType::Threat" {
        return None;
    }
    let overflow = match field("OverflowType")?.as_str()? {
        "ETgfMonsterTargetOverflowType::RandomPos" => AoeOverflow::RandomPos,
        "ETgfMonsterTargetOverflowType::MultiTrigger" => AoeOverflow::MultiTrigger,
        _ => AoeOverflow::Skip,
    };
    Some((at, AoePass { max: num("MaxNum")? as u32, radius: num("Radius")? as f32, interval: num("Interval").unwrap_or(0.0) as f32, overflow, ground_depth: num("GroundCheckDepth").unwrap_or(0.0) as f32, area }))
}

/// A `SpawnConfig` entry's [`VolleyShot`] (its keys carry Blueprint struct suffixes: `OffsetForward_2_...`).
fn volley_shot(entry: &serde_json::Value, range: f32) -> Option<VolleyShot> {
    let field = |name: &str| entry.as_object()?.iter().find(|(k, _)| k.split('_').next() == Some(name)).map(|(_, v)| v.clone());
    let num = |name: &str| field(name).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let shot = projectile_shot(&serde_json::json!({ "SpawnActor": field("SpawnActor")? }), range)?;
    Some(VolleyShot { forward: num("OffsetForward"), up: num("OffsetUp"), right: num("OffsetRight"), pitch: num("OffsetPitch"), yaw: num("OffsetYaw"), shot })
}

/// The times (real s) of every `PlayMontageNotifyWindow` named `name`, in order.
fn notify_windows_all(v: &serde_json::Value, name: &str) -> Vec<f32> {
    let Some(objs) = v.as_array() else { return Vec::new() };
    let Some(m) = objs.iter().find(|o| o["Type"] == "AnimMontage") else { return Vec::new() };
    let rate = m["Properties"]["RateScale"].as_f64().unwrap_or(1.0);
    let mut out: Vec<f32> = m["Properties"]["Notifies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let export = n["NotifyStateClass"]["ObjectName"].as_str().or(n["Notify"]["ObjectName"].as_str())?.rsplit(':').next()?.trim_end_matches('\'');
            let o = objs.iter().find(|o| o["Name"] == export)?;
            (o["Properties"]["NotifyName"] == name).then(|| (n["LinkValue"].as_f64().unwrap_or(0.0) / rate) as f32)
        })
        .collect();
    out.sort_by(f32::total_cmp);
    out
}

/// A throw's launch velocity (`GA_TGF_ThrowProjectile_Monster_Base` bytecode, CONFIRMED): for i = 0..=10 the
/// speed `speed + i x speed / 2` with `SuggestProjectileVelocity` (no trace, the low arc, the projectile's
/// gravity); the first that reaches `to` is used. None: none reaches it (the projectile keeps its own speed).
pub fn toss_velocity(from: Vec3, to: Vec3, speed: f32, gravity: f32) -> Option<Vec3> {
    let d = to - from;
    let flat = d.truncate();
    let x = flat.length();
    if x < 1e-3 {
        return None;
    }
    (0..=10).find_map(|i| {
        let s = speed + i as f32 * speed * 0.5;
        if gravity <= 0.0 {
            return Some(d.normalize_or_zero() * s);
        }
        let disc = s.powi(4) - gravity * (gravity * x * x + 2.0 * d.z * s * s);
        if disc < 0.0 {
            return None;
        }
        let angle = ((s * s - disc.sqrt()) / (gravity * x)).atan();
        Some((flat / x * angle.cos() * s).extend(angle.sin() * s))
    })
}

/// A monster's attack set, and what else the AI needs to know about the monster.
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq, Default)]
pub struct MonsterKit {
    /// Curve-table row name, e.g. `GoblinAxe`.
    pub name: String,
    pub attacks: Vec<MonsterAttack>,
    /// The behaviour tree it runs: its folder's controller (`AI/AIC_*`) names one, else the default
    /// (empty = `BT_Monster_Default`).
    pub tree: String,
    /// Flies (its Blueprint's `Flyable` or `NavAgentProps.bCanFly`, without a decoded Blueprint
    /// [`crate::overrides::ai::FLYERS`]).
    pub flyer: bool,
    /// Its Blueprint's values ([`MonsterBp`]).
    pub bp: MonsterBp,
    pub dormancy: Dormancy,
    /// The Goblin Thief's controller values and abilities.
    pub thief: Option<Box<crate::thief::ThiefKit>>,
    /// The Shadow King's controller values (unused by other monsters).
    pub king: Option<Box<crate::shadow_king::KingSpec>>,
    /// The Wendigo's controller values (unused by other monsters).
    pub wendigo: Option<Box<crate::wendigo::WendigoSpec>>,
    /// Its NPC state's known behaviours (the Wendigo's and the Cyclops').
    pub npc: Option<Box<crate::npc_state::NpcState>>,
    /// The Shadow Dragon's controller and abilities: it flies its own script, not a tree.
    pub dragon: Option<std::sync::Arc<crate::shadow_dragon::DragonSpec>>,
    /// What its summon attack spawns (the Summon totem's Moose Ghost).
    pub summon: Option<Box<crate::summon::Summon>>,
    /// The gameplay tags its ability sets grant, with their parents (`MonsterGroup.Goblin`,
    /// `MonsterGroup.Goblin.Shaman`, `Undead`).
    pub tags: Vec<u32>,
    /// Its alternative selectors.
    pub alternatives: Vec<Alternative>,
    /// Each `Retreats` entry's ability as a kit attack index.
    pub retreat_abilities: Vec<Option<u8>>,
    /// (ability class, montage) of every ability its set grants that became or joined an attack (the classes the
    /// Shadow King's tree casts by name).
    pub classes: Vec<(String, String)>,
}

/// A monster's dormant clips: `sleep` loops while it lies dormant (none: it holds the wake clip's first frame, a
/// skeleton in pieces), `wake` plays once when it wakes. (AnimSequence name, length s.) From the `AM_<M>_Dormant` /
/// `AM_<M>_WakeUp` montages' clips (or the `_2` pair the goblins use), else the `M_<M>_Dormant` / `M_<M>_WakeUp`
/// sequences (HYPOTHESIS: their montage doesn't decode). `wake_no_damage`: the wake montage's [`NO_DAMAGE`] window
/// (s from its start: the Shadow King's 0-29.7 s).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dormancy {
    pub sleep: Option<(String, f32)>,
    pub wake: Option<(String, f32)>,
    pub wake_no_damage: Option<(f32, f32)>,
}

/// The tag a montage window owns while its character takes no damage (`AN_Tgf_GameplayTagOperation`: the Shadow
/// King's wake-up and `Attack8`, the Goblin Thief's exit).
pub const NO_DAMAGE: &str = "TGF.Ability.BuffState.NoDamage";

impl MonsterKit {
    /// The attack that ability class `class` plays: the attack it became, else the one it was merged into.
    pub fn by_class(&self, class: &str) -> Option<u8> {
        let i = self.attacks.iter().position(|a| a.class == class).or_else(|| {
            let m = &self.classes.iter().find(|(c, _)| c == class)?.1;
            self.attacks.iter().position(|a| &a.name == m)
        })?;
        u8::try_from(i).ok()
    }

    /// The behaviour tree's name.
    pub fn tree(&self) -> &str {
        if self.tree.is_empty() || crate::overrides::ai::PENDING_TREES.contains(&self.tree.as_str()) { "BT_Monster_Default" } else { &self.tree }
    }

    /// Whether any attack is a melee swing (not a shot).
    pub fn has_melee(&self) -> bool {
        self.attacks.iter().any(|a| a.shot.is_none())
    }
}

/// Reads one montage export (strict CUE4Parse JSON). An AnimSequence reads the same way: its length and
/// sound cues; its hit windows, if any, are its own notifies.
pub fn montage_from_json(v: &serde_json::Value) -> anyhow::Result<MonsterAttack> {
    let objs = v.as_array().context("montage JSON is not an export list")?;
    let m = objs.iter().find(|o| o["Type"] == "AnimMontage").or_else(|| objs.iter().find(|o| o["Type"] == "AnimSequence")).context("no AnimMontage or AnimSequence export")?;
    let p = &m["Properties"];
    let rate = p["RateScale"].as_f64().unwrap_or(1.0) as f32;
    let length = p["SequenceLength"].as_f64().context("SequenceLength")? as f32 / rate;
    // Sound notifies point at exports by name; each of those names its AkAudioEvent.
    let event_of = |notify_obj: &str| -> Option<String> {
        let export = notify_obj.rsplit(':').next()?.trim_end_matches('\'').to_string();
        let o = objs.iter().find(|o| o["Name"] == export.as_str())?;
        let name = o["Properties"]["Event"]["ObjectName"].as_str()?;
        Some(name.trim_start_matches("AkAudioEvent'").trim_end_matches('\'').to_string())
    };
    let mut windows = Vec::new();
    let mut sounds = Vec::new();
    let mut tags = Vec::new();
    let mut events = Vec::new();
    for n in p["Notifies"].as_array().into_iter().flatten() {
        let t = n["LinkValue"].as_f64().unwrap_or(0.0) as f32 / rate;
        let d = n["Duration"].as_f64().unwrap_or(0.0) as f32 / rate;
        match n["NotifyName"].as_str() {
            Some("AGR_Attack_NotifyState") => windows.push((t, t + d)),
            Some("AN_Tgf_AkEvent_C") => {
                if let Some(ev) = n["Notify"]["ObjectName"].as_str().and_then(event_of) {
                    sounds.push((t, ev));
                }
            }
            // A timed sound state (the thief's `Play_AM_Goblin_Thief_OpenDoor` over 15.65 s): played at its
            // start. HYPOTHESIS: the native state stops the event at its end; a looping event plays once here.
            Some("AN_Tgf_TimedAkEvent_C") => {
                if let Some(ev) = n["NotifyStateClass"]["ObjectName"].as_str().and_then(event_of) {
                    sounds.push((t, ev));
                }
            }
            Some("AN_Tgf_GamePlayEvent_C") => {
                let export = n["Notify"]["ObjectName"].as_str().and_then(|o| o.rsplit(':').next()).map(|e| e.trim_end_matches('\''));
                if let Some(tag) = export.and_then(|e| objs.iter().find(|o| o["Name"] == e)).and_then(|o| o["Properties"]["Event Tag"]["TagName"].as_str()) {
                    events.push((t, tag.to_string()));
                }
            }
            // A tag owned while the state lasts (its export's `Tags`).
            Some("AN_Tgf_GameplayTagOperation_C") => {
                let export = n["NotifyStateClass"]["ObjectName"].as_str().and_then(|o| o.rsplit(':').next()).map(|e| e.trim_end_matches('\''));
                let state = export.and_then(|e| objs.iter().find(|o| o["Name"] == e));
                for tag in state.and_then(|o| o["Properties"]["Tags"].as_array()).into_iter().flatten().filter_map(|t| t.as_str()) {
                    tags.push((t, t + d, tag.to_string()));
                }
            }
            _ => {}
        }
    }
    windows.sort_by(|a, b| a.0.total_cmp(&b.0));
    sounds.sort_by(|a, b| a.0.total_cmp(&b.0));
    let name = m["Name"].as_str().unwrap_or_default().to_string();
    // A montage's root motion through its segments; a bare sequence plays itself.
    let mut segments = montage_segments_from_json(v);
    if segments.is_empty() && m["Type"] == "AnimSequence" {
        segments.push((name.clone(), MontageSegment { at: 0.0, from: 0.0, to: length * rate, rate }));
    }
    let root = root_motion(&segments, length);
    Ok(MonsterAttack { name, length, windows, sounds, shot: None, range: None, area: None, summon: false, own_summon: None, class: String::new(), cloud: None, events, angle: None, pick: AbilityPick::default(), effect: None, chain: None, guard: None, burst: None, volley: Vec::new(), aoe: Vec::new(), spray: None, aura: None, factor: 1.0, root, tags })
}

/// One piece of a montage's slot track: from `at` (s of montage time, after the montage's rate scale)
/// it plays its clip from `from` to `to` (clip seconds) at `rate`. Presentation data: the client maps
/// a swing's time onto the converted clip with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MontageSegment {
    pub at: f32,
    pub from: f32,
    pub to: f32,
    pub rate: f32,
}

/// Reads the slot-track segments of one montage export: (clip name, segment) in track order.
pub fn montage_segments_from_json(v: &serde_json::Value) -> Vec<(String, MontageSegment)> {
    let Some(m) = v.as_array().and_then(|o| o.iter().find(|o| o["Type"] == "AnimMontage")) else { return Vec::new() };
    let p = &m["Properties"];
    let scale = p["RateScale"].as_f64().unwrap_or(1.0) as f32;
    let f = |s: &serde_json::Value, k: &str, d: f32| s[k].as_f64().map_or(d, |x| x as f32);
    let mut out = Vec::new();
    for t in p["SlotAnimTracks"].as_array().into_iter().flatten() {
        for s in t["AnimTrack"]["AnimSegments"].as_array().into_iter().flatten() {
            let clip = s["AnimReference"]["ObjectName"].as_str().unwrap_or_default().trim_start_matches("AnimSequence'").trim_end_matches('\'').to_string();
            out.push((clip, MontageSegment { at: f(s, "StartPos", 0.0) / scale, from: f(s, "AnimStartTime", 0.0), to: f(s, "AnimEndTime", 0.0), rate: f(s, "AnimPlayRate", 1.0) * scale }));
        }
    }
    out
}

/// The segments of montage `name` in monster folder `dir`, if it decoded (see
/// [`montage_segments_from_json`]).
pub fn load_montage_segments(dir: &str, name: &str) -> Vec<(String, MontageSegment)> {
    std::fs::read(db_formats::data_path(MONSTER_JSON).join(dir).join("Animation").join(format!("{name}.json")))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .map_or(Vec::new(), |v| montage_segments_from_json(&v))
}

/// The time in `clip` shown at montage time `t` (s): the segment playing then, or the last one's end.
pub fn clip_time(segments: &[(String, MontageSegment)], t: f32) -> Option<(&str, f32)> {
    let mut last = None;
    for (clip, s) in segments {
        let len = (s.to - s.from) / s.rate.max(1e-3);
        if t < s.at {
            break;
        }
        last = Some((clip.as_str(), (s.from + (t - s.at).min(len) * s.rate).min(s.to)));
    }
    last
}

/// The attack montages of `monster` (curve row name) in folder `dir` (e.g. `M_Goblin_Axe`): every
/// `AM_<...>_Attack*` with at least one hit window, sorted by name. The `AM_H_*` variants (HYPOTHESIS:
/// a harder difficulty) are left out, and so are the `*_Broken*` ones (the skeleton's broken-limb
/// variants, `Struct_MonsterBrokenBoneConfig`) unless the monster has nothing else (Skeleton Polearm).
pub fn load_monster_kit(monster: &str, dir: &str) -> anyhow::Result<MonsterKit> {
    if let Some(kit) = ability_kit(monster, dir) {
        return Ok(kit);
    }
    let root = Some(db_formats::data_path(MONSTER_JSON).join(dir).join("Animation"))
        .filter(|p| p.is_dir())
        .with_context(|| format!("no decoded montages for {dir}"))?;
    let mut names: Vec<_> = std::fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("AM_") && !n.starts_with("AM_H_") && n.contains("_Attack") && n.ends_with(".json"))
        .collect();
    names.sort();
    let mut attacks = Vec::new();
    for n in names {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(&n))?)?;
        let a = montage_from_json(&v).with_context(|| n.clone())?;
        if !a.windows.is_empty() {
            attacks.push(a);
        }
    }
    if attacks.iter().any(|a| !a.name.contains("_Broken")) {
        attacks.retain(|a| !a.name.contains("_Broken"));
    }
    anyhow::ensure!(!attacks.is_empty(), "{dir}: no attack montage with a hit window");
    Ok(MonsterKit { name: monster.to_string(), attacks, ..Default::default() })
}

/// One decoded export list under the monster folders (`/Game/TGF/Character/Monster/...` object path).
pub(crate) fn monster_json(object_path: &str) -> Option<serde_json::Value> {
    let rel = object_path.rsplit_once('.').map_or(object_path, |p| p.0).strip_prefix("/Game/TGF/Character/Monster/")?;
    serde_json::from_slice(&std::fs::read(db_formats::data_path(MONSTER_JSON).join(format!("{rel}.json"))).ok()?).ok()
}

/// One decoded export list by `/Game/...` object path (anywhere in the asset JSON).
pub(crate) fn asset_json(object_path: &str) -> Option<serde_json::Value> {
    let rel = object_path.rsplit_once('.').map_or(object_path, |p| p.0).strip_prefix("/Game/")?;
    serde_json::from_slice(&std::fs::read(db_formats::data_path("json/assets/Dungeonborne/Content").join(format!("{rel}.json"))).ok()?).ok()
}

/// A Blueprint class default (`Default__*` property `key`), following the class's parents.
pub(crate) fn class_default(object_path: &str, key: &str) -> Option<serde_json::Value> {
    let v = asset_json(object_path)?;
    let objs = v.as_array()?;
    if let Some(x) = objs.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__"))).and_then(|o| o["Properties"].get(key)) {
        return Some(x.clone());
    }
    let parent = objs.iter().find(|o| o["Type"] == "BlueprintGeneratedClass")?["Super"]["ObjectPath"].as_str()?.to_string();
    class_default(&parent, key)
}

/// The time (real s) of a montage's `PlayMontageNotifyWindow` named `name` (e.g. `SpawnProjectile`). A shield
/// ability's guard (`GA_TGF_Shield_Monster_Base` bytecode, CONFIRMED): it plays its montage's `ChargeStart`
/// section into the looping `ChargeLoop` and, `GuardTime` s after it started, the `ChargeRelease` section, then
/// ends; it blocks while the montage's `TGF.Ability.Defence` tag window is on: from the window's start, through
/// the loop (montage time stays inside it), to as far into the release as the window reaches (the Armor
/// Hammer's 0.1 s, the Skeleton Hammer's to the end). Out: the guard and the attack's length, s.
fn guard_of(p: &serde_json::Value, ga: &str, v: &serde_json::Value) -> Option<(Guard, f32)> {
    let time = p["GuardTime"].as_f64().or_else(|| class_default(ga, "GuardTime")?.as_f64())? as f32;
    let objs = v.as_array()?;
    let m = &objs.iter().find(|o| o["Type"] == "AnimMontage")?["Properties"];
    let rate = m["RateScale"].as_f64().unwrap_or(1.0) as f32;
    let section = |name: &str| m["CompositeSections"].as_array()?.iter().find(|s| s["SectionName"] == name).and_then(|s| s["SegmentBegin"].as_f64().or(s["LinkValue"].as_f64())).map(|x| x as f32 / rate);
    let release = section("ChargeRelease")?;
    let loop_from = section("ChargeLoop").unwrap_or(release);
    let len = m["SequenceLength"].as_f64()? as f32 / rate;
    let (from, to) = m["Notifies"].as_array()?.iter().find_map(|n| {
        let export = n["NotifyStateClass"]["ObjectName"].as_str().or(n["Notify"]["ObjectName"].as_str())?.rsplit(':').next()?.trim_end_matches('\'');
        let o = objs.iter().find(|o| o["Name"] == export)?;
        let at = n["LinkValue"].as_f64().unwrap_or(0.0) as f32;
        o["Properties"]["Tags"].as_array()?.iter().any(|t| t == "TGF.Ability.Defence").then(|| (at / rate, (at + n["Duration"].as_f64().unwrap_or(0.0) as f32) / rate))
    })?;
    let release_at = time.max(release);
    Some((Guard { from, until: release_at + (to.min(len) - release).max(0.0), loop_from, loop_to: release, release_at }, release_at + (len - release)))
}

/// A shield ability that owns `TGF.Ability.Defence` while active (`GA_TGF_Shield_Monster_Base`'s
/// `ActivationOwnedTags`, CONFIRMED: every other subclass sets its own without it, the Wendigo's Dash inherits it) and
/// whose montage has no `ChargeRelease` section: the montage plays from its start (no `ChargeStart` either) and its
/// end ends the ability, so it guards for the montage's `length` s. When `GuardTime` comes first, the release would
/// play from the start again (a missing section); every such montage so far is shorter (the Dash: 2.97 s, 4.4 s).
fn owned_guard(p: &serde_json::Value, ga: &str, v: &serde_json::Value, length: f32) -> Option<Guard> {
    p["GuardTime"].as_f64().or_else(|| class_default(ga, "GuardTime")?.as_f64())?;
    let owned = p.get("ActivationOwnedTags").cloned().or_else(|| class_default(ga, "ActivationOwnedTags"))?;
    if !owned.as_array()?.iter().any(|t| t == "TGF.Ability.Defence") {
        return None;
    }
    let m = &v.as_array()?.iter().find(|o| o["Type"] == "AnimMontage")?["Properties"];
    if m["CompositeSections"].as_array().is_some_and(|c| c.iter().any(|s| s["SectionName"] == "ChargeRelease")) {
        return None;
    }
    Some(Guard { from: 0.0, until: length, loop_from: 0.0, loop_to: 0.0, release_at: 0.0 })
}

pub(crate) fn notify_window_at(v: &serde_json::Value, name: &str) -> Option<f32> {
    let objs = v.as_array()?;
    let m = objs.iter().find(|o| o["Type"] == "AnimMontage")?;
    let rate = m["Properties"]["RateScale"].as_f64().unwrap_or(1.0);
    m["Properties"]["Notifies"].as_array()?.iter().find_map(|n| {
        let export = n["NotifyStateClass"]["ObjectName"].as_str().or(n["Notify"]["ObjectName"].as_str())?.rsplit(':').next()?.trim_end_matches('\'');
        let o = objs.iter().find(|o| o["Name"] == export)?;
        (o["Properties"]["NotifyName"] == name).then(|| (n["LinkValue"].as_f64().unwrap_or(0.0) / rate) as f32)
    })
}

/// [`AreaSpawn`] from an ability's defaults (`SpawnActor`, `SpawnNumber`, `SpawnRadiusMin/Max`), its
/// projectile class's `EnableCollisionDelay` and its damage effect's `MoveFactorRate`.
fn area_spawn(p: &serde_json::Value) -> Option<AreaSpawn> {
    let actor = p["SpawnActor"]["ObjectPath"].as_str()?;
    let fuse = class_default(actor, "EnableCollisionDelay")?.as_f64()? as f32;
    let ge = p["MeleeWeaponHitGE"].as_array()?.first()?["ObjectPath"].as_str()?;
    let factor = ge_factor(ge).unwrap_or(1.0);
    Some(AreaSpawn { count: p["SpawnNumber"].as_u64()? as u32, radius_min: p["SpawnRadiusMin"].as_u64()? as u32, radius_max: p["SpawnRadiusMax"].as_u64()? as u32, fuse, size: 50.0, factor })
}

/// A damage effect's `MoveFactorRate` (its `TgfDamageExecution` modifier). An effect that doesn't set its own `Executions`
/// takes its parent's (`GE_TGF_M_Goblin_Axe_Attack1` -> `GE_TGF_Damage_Instant` -> `GE_TGF_Damage_Basic`, 1); executions
/// without the modifier give 1.
fn ge_factor(object_path: &str) -> Option<f64> {
    let v = asset_json(object_path)?;
    let objs = v.as_array()?;
    let cdo = objs.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))?;
    match cdo["Properties"]["Executions"].as_array() {
        Some(ex) => Some(ex.iter().flat_map(|e| e["CalculationModifiers"].as_array().into_iter().flatten()).find(|m| m["CapturedAttribute"]["AttributeToCapture"]["AttributeName"] == "MoveFactorRate").and_then(|m| m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()).unwrap_or(1.0)),
        None => objs.iter().find(|o| o["Type"] == "BlueprintGeneratedClass").and_then(|c| c["Super"]["ObjectPath"].as_str()).filter(|s| s.starts_with("/Game/")).and_then(ge_factor),
    }
}

/// An ability's hit factor: its `MeleeWeaponHitGE`'s `MoveFactorRate` (1 without one).
fn hit_factor(p: &serde_json::Value) -> f64 {
    p["MeleeWeaponHitGE"].as_array().and_then(|g| g.iter().find_map(|x| x["ObjectPath"].as_str())).and_then(ge_factor).unwrap_or(1.0)
}

/// The object name inside `Class'Name'` references.
fn quoted(r: &serde_json::Value) -> Option<&str> {
    r["ObjectName"].as_str()?.split('\'').nth(1)
}

/// The attacks the monster's ability set grants (CONFIRMED since the v5 usmap): `<dir>/Ability/
/// AbilitySet_M_*` (not the hard `_H_` set) lists its gameplay abilities; an attack is one with an AI rule
/// (`AIAbilityRule` -> `TgfAIAbilityRule.ActivateRange`) whose `CastMontageSet.DefaultMontage` decodes with
/// a hit window. Broken-limb attacks only when there is nothing else (as [`load_monster_kit`]). None: no
/// decoded set, or no such attack in it.
pub fn ability_kit(monster: &str, dir: &str) -> Option<MonsterKit> {
    let folder = db_formats::data_path(MONSTER_JSON).join(dir).join("Ability");
    let mut sets: Vec<_> = std::fs::read_dir(&folder).ok()?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("AbilitySet_M_") && n.ends_with(".json")).collect();
    sets.sort();
    let set: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join(sets.first()?)).ok()?).ok()?;
    let granted = set.as_array()?.iter().find(|o| o["Type"] == "LyraAbilitySet")?["Properties"]["GrantedGameplayAbilities"].as_array()?.clone();
    // A folder several roster rows share (`AbilitySet_M_Totem` grants all six totem abilities, each totem uses
    // its Blueprint's `AbilitySelector` one): only the selector's abilities.
    let shared = ROSTER.iter().chain(BOSSES).filter(|s| s.dir == dir).count() > 1;
    let bp = spec(monster).and_then(blueprint);
    let selector: Option<Vec<String>> = shared.then(|| bp.map(|b| b.abilities.iter().filter_map(|a| a.0.clone()).collect())).flatten();
    let mut attacks: Vec<MonsterAttack> = Vec::new();
    let mut summon = None;
    // (ability class, montage): the alternative selectors and chains name abilities.
    let mut montage_of: Vec<(String, String)> = Vec::new();
    // (montage, notify time, `DoPercent`, [(ability class, weight)]) of the chaining attacks.
    let mut chains: Vec<(String, f32, f64, ChainTo)> = Vec::new();
    // The abilities chains and retreats start: they need no AI rule (the Khazra's Attack6_2) nor a hit window
    // (the Werewolf's jumps, the Goblin Dagger's Attack4 back-step play their montage and move).
    let mut chain_targets: Vec<String> = granted
        .iter()
        .filter_map(|g| monster_json(g["Ability"]["ObjectPath"].as_str()?))
        .filter_map(|ga| ga.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__"))).map(|o| o["Properties"]["MultiAbilityToActivate"].clone()))
        .flat_map(|m| m.as_array().cloned().unwrap_or_default())
        .filter_map(|e| e.as_object()?.iter().find(|(k, _)| k.split('_').next() == Some("AbilityToActivate")).and_then(|(_, v)| quoted(v).map(str::to_string)))
        .collect();
    chain_targets.extend(bp.into_iter().flat_map(|b| b.retreats.iter().filter_map(|r| r.ability.clone())));
    for g in &granted {
        if let Some(sel) = &selector
            && !quoted(&g["Ability"]).is_some_and(|n| sel.iter().any(|s| s == n))
        {
            continue;
        }
        let ga_path = g["Ability"]["ObjectPath"].as_str().unwrap_or_default();
        let Some(ga) = monster_json(ga_path) else { continue };
        let Some(cdo) = ga.as_array().and_then(|a| a.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))) else { continue };
        let p = &cdo["Properties"];
        let rule = p["AIAbilityRule"]["ObjectPath"].as_str().and_then(monster_json).and_then(|r| r.as_array()?.iter().find(|o| o["Type"] == "TgfAIAbilityRule").map(|o| o["Properties"].clone()));
        // Its weight in the Blueprint's selector (summed when it is named twice).
        let chance: f64 = bp.map_or(0.0, |b| b.abilities.iter().filter(|s| s.0.as_deref().is_some_and(|n| quoted(&g["Ability"]) == Some(n))).map(|s| s.1.unwrap_or(0.0)).sum());
        let pick = ability_pick(p, ga_path, rule.as_ref(), chance);
        // A self ability's rule may leave its range out (none needed; the Werewolf's Change has 800).
        let chained = quoted(&g["Ability"]).is_some_and(|n| chain_targets.iter().any(|c| c == n));
        // Without a rule, a selector ability's range is the monster's `AbilityRange` (`GetAbilityActivateRange`,
        // CONFIRMED: the Wendigo's Dash, 250); a summon outside the selector has none (the Wendigo's tree casts his
        // totem summons directly, `BTT_ActivateAbilityDirectly`, no range check).
        let Some(range) = rule.as_ref().and_then(|r| r["ActivateRange"].as_f64()).or((pick.self_target || chained).then_some(0.0)).or((rule.is_none() && chance > 0.0).then(|| f64::from(bp.and_then(|b| b.ability_range).unwrap_or(150.0)))).or((rule.is_none() && p["SummonMonsters"].is_array()).then_some(0.0)) else { continue };
        let angle = rule.as_ref().and_then(|r| r["ActivateAngleRange"].as_f64()).map(|a| a as f32);
        let Some(montage) = quoted(&p["CastMontageSet"]["DefaultMontage"]) else { continue };
        if montage.starts_with("AM_H_") {
            // A hard-mode ability an alternative selector names (the buffed crossbow goblin's
            // `GA_TGF_H_Goblin_CrossBow_Attack1`) stands for the normal attack of the same name (HYPOTHESIS: its
            // hard-mode values are not used).
            if let Some(ga) = quoted(&g["Ability"]) {
                montage_of.push((ga.to_string(), montage.replacen("AM_H_", "AM_", 1)));
            }
            continue;
        }
        if let Some(ga) = quoted(&g["Ability"]) {
            montage_of.push((ga.to_string(), montage.to_string()));
        }
        // Two abilities playing one montage (a hard-mode copy, mostly) are one attack; their weights add up
        // (HYPOTHESIS: the first one's rule stands for both). Not summons: one montage may call up different monsters
        // (the Shadow King's `Attack5_1`..`_4`).
        if !p["SummonMonsters"].is_array()
            && let Some(prev) = attacks.iter_mut().find(|a| a.name == montage)
        {
            prev.pick.chance += chance;
            continue;
        }
        if let Some(mut a) = dash_attack(p, montage, range as f32) {
            a.angle = angle;
            a.pick = pick;
            // After the dash it plays its montage's `EndSectionName` section (the Shadow King's Attack10,
            // `DashEnd` 4.29-6.07 s; HYPOTHESIS: jumped to as the dash ends), and a chain notify inside it comes
            // that far into the section (the Attack10 dash breaks into Attack10_2 there).
            let v = p["CastMontageSet"]["DefaultMontage"]["ObjectPath"].as_str().and_then(monster_json);
            let dash_end = a.length - crate::DT;
            let section = v.as_ref().zip(p["EndSectionName"].as_str()).and_then(|(v, name)| {
                let m = &v.as_array()?.iter().find(|o| o["Type"] == "AnimMontage")?["Properties"];
                let rate = m["RateScale"].as_f64().unwrap_or(1.0);
                let s = m["CompositeSections"].as_array()?.iter().find(|s| s["SectionName"] == name)?;
                let start = s["SegmentBegin"].as_f64().or(s["LinkValue"].as_f64())?;
                Some(((start / rate) as f32, (m["SequenceLength"].as_f64()? / rate) as f32))
            });
            if let Some((start, len)) = section {
                a.length = dash_end + (len - start).max(0.0) + crate::DT;
            }
            if let Some((at, percent, to)) = v.as_ref().and_then(|v| chain_of(p, ga_path, v)) {
                let at = match section {
                    Some((start, _)) if at >= start => dash_end + (at - start),
                    _ => at,
                };
                chains.push((montage.to_string(), at, percent, to));
            }
            a.factor = hit_factor(p);
            a.class = quoted(&g["Ability"]).unwrap_or_default().to_string();
            attacks.push(a);
            continue;
        }
        // By the path the ability names: some monsters play another's montages (Mimic03 plays
        // `M_Mimic02/Animation/AM_Mimic02_Attack1`).
        let Some(v) = p["CastMontageSet"]["DefaultMontage"]["ObjectPath"].as_str().and_then(monster_json).or_else(|| animation_json(dir, montage)) else { continue };
        let Ok(mut a) = montage_from_json(&v) else { continue };
        // An area attack: its spawn notify is its one-tick window.
        if let Some(area) = area_spawn(p)
            && let Some(at) = notify_window_at(&v, "SpawnProjectile")
        {
            a.windows = vec![(at, at + crate::DT)];
            a.area = Some(area);
        }
        // A projectile ability (the totem's ice bolt: the shooters' bolts, arrows and the Shaman's missile):
        // its shot from data, fired at its spawn notify.
        if a.area.is_none()
            && let Some(shot) = projectile_shot(p, range as f32)
            && let Some(at) = notify_window_at(&v, "SpawnProjectile")
        {
            a.windows = vec![(at, at + crate::DT)];
            a.shot = Some(shot);
            // Its projectile class's `LaunchSound` (the ice bolt's `Play_M_Wendigo_Attack5_Ice_Release_Single`).
            if let Some(ev) = p["SpawnActor"]["ObjectPath"].as_str().and_then(|c| class_default(c, "LaunchSound")).and_then(|s| quoted(&s).map(str::to_string)) {
                a.sounds.push((at, ev));
            }
        }
        // A volley (the Wendigo's Attack4 fan and Attack5 ring): one `SpawnConfig` entry per `SpawnProjectile`.
        if a.area.is_none()
            && a.shot.is_none()
            && let Some(cfg) = p["SpawnConfig"].as_array()
        {
            let times = notify_windows_all(&v, "SpawnProjectile");
            let volley: Vec<VolleyShot> = cfg.iter().filter_map(|c| volley_shot(c, range as f32)).collect();
            if !times.is_empty() && volley.len() == cfg.len() {
                a.windows = times.iter().take(volley.len()).map(|&t| (t, t + crate::DT)).collect();
                a.volley = volley;
            }
        }
        // A spray (the Lava totem's lava): at its `SpawnName` notify.
        if a.area.is_none()
            && a.shot.is_none()
            && a.volley.is_empty()
            && class_default(ga_path, "HorizantalMinDegree").is_some()
            && let Some((at, spray)) = spray_of(p, ga_path, &v, range as f32)
        {
            a.windows = vec![(at, at + crate::DT)];
            a.spray = Some(spray);
        }
        // An aura area (the Slow totem's slowing ground): at its `SpawnOverlapArea` notify.
        if a.area.is_none()
            && a.shot.is_none()
            && let Some(actor) = p["SpawnActor"]["ObjectPath"].as_str().map(str::to_string).or_else(|| class_default(ga_path, "SpawnActor")?["ObjectPath"].as_str().map(str::to_string))
            && class_default(ga_path, "SpawnOffset").is_some()
            && let Some(aura) = crate::aura_areas::AuraSpec::load(actor.split('.').next().unwrap_or(&actor))
            && let Some(at) = notify_window_at(&v, "SpawnOverlapArea")
        {
            a.windows = vec![(at, at + crate::DT)];
            a.aura = Some(Box::new(aura));
        }
        // Multi-pass areas (the Wendigo's Thunder): one pass per `AOEConfig` entry, at its own notify.
        if a.area.is_none()
            && a.shot.is_none()
            && a.volley.is_empty()
            && let Some(cfg) = p["AOEConfig"].as_array()
        {
            let passes: Vec<(f32, AoePass)> = cfg.iter().filter_map(|c| aoe_pass(c, &v)).collect();
            if !passes.is_empty() && passes.len() == cfg.len() {
                a.windows = passes.iter().map(|(t, _)| (*t, *t + crate::DT)).collect();
                a.aoe = passes.into_iter().map(|p| p.1).collect();
            }
        }
        // A spray's area (`GA_TGF_Spray_Monster_Base`: the Shadow King's storms): `AreaEffect` laid
        // `AreaEffectDelay` after its `FireNotify`; its own capsule query (`Query*` 0 for the storms) hits nothing.
        if let Some(cloud) = p["AreaEffect"]["ObjectPath"].as_str().map(|s| s.split('.').next().unwrap_or(s)).and_then(crate::staffs::overlap_area)
            && let Some(at) = notify_window_at(&v, p["FireNotify"].as_str().unwrap_or("Fire"))
        {
            let delay = p["AreaEffectDelay"].as_f64().unwrap_or(0.0) as f32;
            a.windows = vec![(at + delay, at + delay + crate::DT)];
            a.cloud = Some(Box::new(cloud));
        }
        // A tracking shot (`GA_TGF_SpawnTrackProjectile_Monster_Base`: the Shadow King's bullet), fired at its
        // `SpawnProjectileNotifyName` ("Fire").
        if a.area.is_none()
            && a.shot.is_none()
            && let Some(shot) = track_shot(p, range as f32)
            && let Some(at) = notify_window_at(&v, p["SpawnProjectileNotifyName"].as_str().unwrap_or("SpawnProjectile"))
        {
            a.windows = vec![(at, at + crate::DT)];
            a.shot = Some(shot);
        }
        // A summon ability: its spawn, `SummonDelayTime` into the cast, is its one-tick window.
        if p["SummonMonsters"].is_array()
            && let Some(s) = summon_spec(p, ga_path)
        {
            let delay = p["SummonDelayTime"].as_f64().or_else(|| class_default(ga_path, "SummonDelayTime")?.as_f64()).unwrap_or(0.0) as f32;
            a.windows = vec![(delay, delay + crate::DT)];
            a.summon = true;
            // Fixed spots: the attack's own; else the kit's one summon, and a further one's its own (the Wendigo's totem
            // summons each call up their own totem).
            if s.fixed || summon.is_some() {
                a.own_summon = Some(Box::new(s));
            } else {
                summon = Some(Box::new(s));
            }
        }
        // An explosion: its hit at the `Fire` notify.
        if a.shot.is_none()
            && a.area.is_none()
            && a.cloud.is_none()
            && !a.summon
            && ga.as_array().is_some_and(|objs| objs.iter().any(|o| o["Type"] == "BlueprintGeneratedClass" && quoted(&o["Super"]) == Some("GA_TGF_Explode_Monster_Base_C")))
            && let (Some(radius), Some(height)) = (p["Radius"].as_f64(), p["Height"].as_f64())
            && let Some(at) = notify_window_at(&v, "Fire").or_else(|| notify_window_at(&v, "fire"))
        {
            let factor = p["MeleeWeaponHitGE"].as_array().and_then(|g| g.first()?["ObjectPath"].as_str().and_then(ge_factor)).unwrap_or(1.0);
            a.windows = vec![(at, at + crate::DT)];
            a.burst = Some(Burst { radius: radius as f32, height: height as f32, factor });
        }
        // An effect ability: its effect lands at the `fire` notify.
        if a.shot.is_none()
            && a.area.is_none()
            && a.burst.is_none()
            && a.cloud.is_none()
            && !a.summon
            && let Some(effect) = ability_effect(p, ga_path, &pick, range as f32)
            // `FireNotify` ("Fire"; Unreal names match without case: the Shaman's montages say "fire").
            && let Some(at) = notify_window_at(&v, "fire").or_else(|| notify_window_at(&v, "Fire"))
        {
            a.windows = vec![(at, at + crate::DT)];
            a.effect = Some(Box::new(effect));
        }
        // A shield: it hits nothing and blocks.
        if a.windows.is_empty()
            && let Some((guard, length)) = guard_of(p, ga_path, &v)
        {
            a.length = length;
            a.guard = Some(guard);
            a.root.clear();
            // Its tag windows in the ability's time, as the guard's: a time in the release section moves by the
            // loop's stretch; one reaching into the release from before stays on through the loop.
            let shift = guard.release_at - guard.loop_to;
            for t in &mut a.tags {
                if t.1 >= guard.loop_to {
                    t.1 = (t.1 + shift).min(length);
                }
                if t.0 >= guard.loop_to {
                    t.0 += shift;
                }
            }
        }
        // A shield that keeps the base's owned `TGF.Ability.Defence` (the Wendigo's Dash) guards while its montage
        // plays, moving by its root motion (his leap back).
        if a.windows.is_empty()
            && a.guard.is_none()
            && let Some(guard) = owned_guard(p, ga_path, &v, a.length)
        {
            a.guard = Some(guard);
        }
        if a.windows.is_empty() && !chained && a.guard.is_none() {
            continue;
        }
        if let Some((at, percent, to)) = chain_of(p, ga_path, &v) {
            chains.push((montage.to_string(), at, percent, to));
        }
        a.range = Some(range as f32);
        a.angle = angle;
        a.pick = pick;
        a.factor = hit_factor(p);
        a.class = quoted(&g["Ability"]).unwrap_or_default().to_string();
        attacks.push(a);
    }
    if attacks.iter().any(|a| !a.name.contains("_Broken")) {
        attacks.retain(|a| !a.name.contains("_Broken"));
    }
    attacks.sort_by(|a, b| a.name.cmp(&b.name));
    // The actor tags of its ability sets (`GrantedGameTags`) and its alternative selectors by kit index.
    let set_tags = |v: &serde_json::Value| -> Vec<u32> { v.as_array().into_iter().flatten().filter(|o| o["Type"] == "LyraAbilitySet").flat_map(|o| o["Properties"]["GrantedGameTags"].as_array().into_iter().flatten().flat_map(|g| owned_tag_ids(&g["GamePlayTags"]))).collect() };
    let mut tags: Vec<u32> = bp.map(|b| b.ability_sets.clone()).unwrap_or_default().iter().filter_map(|p| asset_json(p)).flat_map(|v| set_tags(&v)).chain(set_tags(&set)).collect();
    tags.sort_unstable();
    tags.dedup();
    let index_of = |ga: &str| montage_of.iter().find(|(g, _)| g == ga).and_then(|(_, m)| attacks.iter().position(|a| &a.name == m));
    let alternatives = bp
        .map(|b| {
            b.alternatives
                .iter()
                .map(|(alt_tags, sel)| {
                    let mut chances = vec![0.0; attacks.len()];
                    for (ga, c) in sel {
                        if let Some(i) = ga.as_deref().and_then(index_of) {
                            chances[i] += c.unwrap_or(0.0);
                        }
                    }
                    Alternative { tags: alt_tags.iter().map(|t| crate::buffs::gameplay_tag(t)).collect(), chances }
                })
                .collect()
        })
        .unwrap_or_default();
    let resolved: Vec<(usize, Chain)> = chains
        .into_iter()
        .filter_map(|(montage, at, percent, to)| {
            let to: Vec<(u8, f64)> = to.iter().filter_map(|(ga, w)| Some((index_of(ga)? as u8, *w))).collect();
            Some((attacks.iter().position(|a| a.name == montage)?, Chain { at, percent, to })).filter(|c| !c.1.to.is_empty())
        })
        .collect();
    let retreat_abilities: Vec<Option<u8>> = bp.map(|b| b.retreats.iter().map(|r| r.ability.as_deref().and_then(index_of).map(|i| i as u8)).collect()).unwrap_or_default();
    for (a, chain) in resolved {
        attacks[a].chain = Some(Box::new(chain));
    }
    (!attacks.is_empty()).then(|| MonsterKit { name: monster.to_string(), attacks, summon, tags, alternatives, retreat_abilities, classes: montage_of, ..Default::default() })
}

/// A chain's next abilities by class: (ability class, `ActivatePercent`).
type ChainTo = Vec<(String, f64)>;

/// An ability's montage chain: its `DoPercent` (the base class's 0 unless set), the time of its montage's
/// `DoMonsterQuickside` notify window and its `MultiAbilityToActivate` (ability class, `ActivatePercent`).
fn chain_of(p: &serde_json::Value, ga: &str, montage: &serde_json::Value) -> Option<(f32, f64, ChainTo)> {
    let percent = p["DoPercent"].as_f64().or_else(|| class_default(ga, "DoPercent")?.as_f64()).unwrap_or(0.0);
    if percent <= 0.0 {
        return None;
    }
    let at = notify_window_at(montage, "DoMonsterQuickside")?;
    let field = |e: &serde_json::Value, key: &str| e.as_object().and_then(|o| o.iter().find(|(k, _)| k.split('_').next() == Some(key)).map(|(_, v)| v.clone()));
    let to: Vec<(String, f64)> = p["MultiAbilityToActivate"].as_array().into_iter().flatten().filter_map(|e| Some((quoted(&field(e, "AbilityToActivate")?)?.to_string(), field(e, "ActivatePercent")?.as_f64()?))).collect();
    (!to.is_empty()).then_some((at, percent, to))
}

/// Tag ids from a list of tag names (plain strings or `{"TagName": ..}`): exact ids, for queries (a filter's
/// tags, `RequiredTags`, `BlockedTags`).
fn tag_ids(v: &serde_json::Value) -> Vec<u32> {
    v.as_array().into_iter().flatten().filter_map(|t| t.as_str().or(t["TagName"].as_str())).map(crate::buffs::gameplay_tag).collect()
}

/// Tag ids with their parents, for tags an actor owns (its sets' and effects' tags), so exact queries match
/// parent tags as Unreal's do.
fn owned_tag_ids(v: &serde_json::Value) -> Vec<u32> {
    v.as_array().into_iter().flatten().filter_map(|t| t.as_str().or(t["TagName"].as_str())).flat_map(crate::buffs::tag_with_parents).collect()
}

/// An effect ability's [`AbilityEffect`]: its `GameplayEffect`'s duration, `MoveSpeedBase` adds and owned tags,
/// and whom it lands on (`TargetSelf`, `Radius` with `RequiredTags` / `BlockedTags`, or an ally when its rule
/// targets `Friendly`). None for an ability without an effect or aimed at foes.
fn ability_effect(p: &serde_json::Value, ga: &str, pick: &AbilityPick, range: f32) -> Option<AbilityEffect> {
    let get = |key: &str| p.get(key).filter(|v| !v.is_null()).cloned().or_else(|| class_default(ga, key));
    let ge = get("GameplayEffect")?["ObjectPath"].as_str()?.to_string();
    let to = if get("TargetSelf").and_then(|v| v.as_bool()) == Some(true) {
        EffectTo::Own
    } else if let Some(radius) = get("Radius").and_then(|r| r.as_f64()) {
        EffectTo::Area { radius: radius as f32, required: get("RequiredTags").map(|t| tag_ids(&t)).unwrap_or_default(), blocked: get("BlockedTags").map(|t| tag_ids(&t)).unwrap_or_default(), hostile: get("Hostile").and_then(|v| v.as_bool()) == Some(true) }
    } else if pick.friendly {
        EffectTo::Ally(pick.target_range.map_or(range, |r| r.0))
    } else {
        return None;
    };
    let seconds = class_default(&ge, "DurationMagnitude").and_then(|d| d["ScalableFloatMagnitude"]["Value"].as_f64()).unwrap_or(0.0) as f32;
    let speed = class_default(&ge, "Modifiers").and_then(|m| m.as_array().cloned()).unwrap_or_default().iter().filter(|m| m["Attribute"]["AttributeName"] == "MoveSpeedBase" && m["ModifierOp"].as_str().is_some_and(|o| o.ends_with("::0"))).filter_map(|m| m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()).sum::<f64>() as f32;
    let owned = class_default(&ge, "InheritableOwnedTagsContainer").unwrap_or_default();
    let mut tags = owned_tag_ids(if owned["CombinedTags"].is_array() { &owned["CombinedTags"] } else { &owned["Added"] });
    tags.sort_unstable();
    tags.dedup();
    let slow = ge_slow(&ge).map(|s| s.0);
    Some(AbilityEffect { to, seconds, speed, tags, slow })
}

/// An ability's [`AbilityPick`]: its selector weight, its cooldown effect's duration and its AI rule's
/// filters (enum values by their number; a rule without `TargetType` has the default 0, no target).
fn ability_pick(p: &serde_json::Value, ga: &str, rule: Option<&serde_json::Value>, chance: f64) -> AbilityPick {
    let en = |v: &serde_json::Value| v.as_i64().or_else(|| v.as_str()?.rsplit("::").next()?.parse().ok());
    let cooldown = p.get("CooldownGameplayEffectClass").filter(|c| c.is_object()).cloned().or_else(|| class_default(ga, "CooldownGameplayEffectClass")).and_then(|c| c["ObjectPath"].as_str().map(str::to_string)).and_then(|ge| class_default(&ge, "DurationMagnitude")).and_then(|d| d["ScalableFloatMagnitude"]["Value"].as_f64()).unwrap_or(0.0) as f32;
    let Some(r) = rule else { return AbilityPick { chance, cooldown, ..Default::default() } };
    let args = |f: &serde_json::Value| -> Vec<f64> { f["Args"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default() };
    let arg = |a: &[f64], i: usize| a.get(i).copied().unwrap_or(0.0);
    let filters = r["Filters"].as_array().into_iter().flatten().map(|f| {
        let a = args(f);
        match en(&f["Type"]).unwrap_or(0) {
            1 => SelfFilter::HealthLow(arg(&a, 0) / 100.0),
            2 => SelfFilter::HealthHigh(arg(&a, 0) / 100.0),
            3 => SelfFilter::FriendlyCount(arg(&a, 0) as f32, arg(&a, 1), arg(&a, 2)),
            4 => SelfFilter::HostileCount(arg(&a, 0) as f32, arg(&a, 1), arg(&a, 2)),
            5 => SelfFilter::Boss,
            6 => SelfFilter::EnemyBehind(arg(&a, 0) as f32),
            7 => SelfFilter::SummonsCount(arg(&a, 0), arg(&a, 1)),
            8 => SelfFilter::Tags(arg(&a, 0) != 0.0, tag_ids(&f["Tags"])),
            _ => SelfFilter::None,
        }
    });
    let targets: Vec<(i64, Vec<f64>)> = r["TargetFilters"].as_array().into_iter().flatten().map(|f| (en(&f["Type"]).unwrap_or(0), args(f))).collect();
    AbilityPick {
        chance,
        cooldown,
        any: r.get("FilterOp").and_then(en) == Some(1),
        filters: filters.collect(),
        self_target: r.get("TargetType").and_then(en).unwrap_or(0) == 0,
        target_range: targets.iter().find(|t| t.0 == 1).map(|t| (arg(&t.1, 0) as f32, arg(&t.1, 1) as f32)),
        friendly: targets.iter().any(|t| t.0 == 3),
    }
}

/// The properties of a Blueprint's component of type `ty`, merged along the class's parents (the child's
/// override the parent's: the ice bolt's `ProjectileGravityScale` 0 comes from its base class).
fn component_props(object_path: &str, ty: &str) -> serde_json::Map<String, serde_json::Value> {
    let Some(v) = asset_json(object_path) else { return Default::default() };
    let Some(objs) = v.as_array() else { return Default::default() };
    let mut props = objs.iter().find(|o| o["Type"] == "BlueprintGeneratedClass").and_then(|c| c["Super"]["ObjectPath"].as_str()).map(|p| component_props(p, ty)).unwrap_or_default();
    if let Some(own) = objs.iter().find(|o| o["Type"] == ty).and_then(|o| o["Properties"].as_object()) {
        props.extend(own.clone());
    }
    props
}

/// A projectile ability's shot from data (CONFIRMED values): its `SpawnActor`'s `ProjectileMovement`
/// (`InitialSpeed`, `MaxSpeed`, `ProjectileGravityScale` × 980 cm/s²), `ShouldAcc` / `AccMag`,
/// `MaxExistTime`, `Explosive` / `ExplodeRadius`, its box's smaller half-width as the radius, the ability's
/// `ForwardOffset` / `UpOffset` and `AimDegreeConfig`, and its effects: the damage one's `MoveFactorRate`, a
/// timed `MoveSpeedRate` one as a slow. HYPOTHESIS: the slow's `ModifierOp` 4 multiplies
/// (`MultiplyCompound`), so magnitude 0.65 slows by 35 %.
fn projectile_shot(p: &serde_json::Value, range: f32) -> Option<Shot> {
    let actor = p["SpawnActor"]["ObjectPath"].as_str()?;
    let pm = component_props(actor, "ProjectileMovementComponent");
    let speed = pm.get("InitialSpeed")?.as_f64()? as f32;
    let max_speed = pm.get("MaxSpeed").and_then(|m| m.as_f64()).map_or(speed, |m| m as f32);
    let gravity = pm.get("ProjectileGravityScale").and_then(|g| g.as_f64()).unwrap_or(1.0) as f32 * 980.0;
    // A lob under gravity (the Cyclops' rock, Attack4) is a throw: its ability's `Speed`.
    let toss = p["Speed"].as_f64().map(|s| s as f32);
    if gravity > 0.0 && toss.is_none() {
        return None;
    }
    let accel = if class_default(actor, "ShouldAcc").and_then(|b| b.as_bool()) == Some(true) { class_default(actor, "AccMag").and_then(|a| a.as_f64()).unwrap_or(0.0) as f32 } else { 0.0 };
    let lifetime = class_default(actor, "MaxExistTime").and_then(|t| t.as_f64()).map(|t| t as f32);
    let ext = component_props(actor, "BoxComponent").get("BoxExtent").cloned();
    // A flat box (the Wendigo's Attack5 bolts, 40 x 30 x 0) touches with its other side (HYPOTHESIS).
    let radius = ext.and_then(|e| {
        let (y, z) = (e["Y"].as_f64()? as f32, e["Z"].as_f64()? as f32);
        Some(if y.min(z) > 0.0 { y.min(z) } else { y.max(z) })
    });
    let radius = radius.unwrap_or(10.0);
    let effects: Vec<String> = class_default(actor, "GameplayEffectClass").and_then(|v| v.as_array().map(|a| a.iter().filter_map(|g| g["ObjectPath"].as_str().map(str::to_string)).collect())).unwrap_or_default();
    let factor = effects.iter().find_map(|g| ge_factor(g)).unwrap_or(1.0);
    let slow = effects.iter().find_map(|g| ge_slow(g));
    let offset = (p["ForwardOffset"].is_number() || p["UpOffset"].is_number()).then(|| (p["ForwardOffset"].as_f64().unwrap_or(0.0) as f32, p["UpOffset"].as_f64().unwrap_or(0.0) as f32));
    // `Explosive` projectiles hit everyone within `ExplodeRadius` (the Shaman's missile, 40 cm).
    let burst = if class_default(actor, "Explosive").and_then(|b| b.as_bool()) == Some(true) { class_default(actor, "ExplodeRadius").and_then(|r| r.as_f64()).unwrap_or(0.0) as f32 } else { 0.0 };
    Some(Shot { speed, range, gravity, turn_rate: 0.0, radius, burst, accel, max_speed, lifetime, offset, aim_degrees: p["AimDegreeConfig"].as_f64().map(|a| a as f32), slow, factor, toss: toss.filter(|_| gravity > 0.0), ignore_guard: class_default(actor, "bIgnoreGuard").and_then(|b| b.as_bool()) == Some(true), delay: class_default(actor, "DelayLaunchTime").and_then(|t| t.as_f64()).unwrap_or(0.0) as f32 })
}

/// A tracking projectile (CONFIRMED `B_TGF_Track_ProjectileBase` / `B_TGF_Monster_Track_Bullet_Base` tick): every tick
/// its velocity points straight at its target (`FindLookAtRotation`; here an unlimited turn rate), its speed growing by
/// `Acc` cm/s² from `InitialSpeed` to `MaxSpeed`, no gravity; it is gone once it has flown `MaxExistDist` (or after
/// `MaxExistTime`). `bExplosive`: it hits everyone within `ExplodeRadius`. Launched at the ability's `SpawnOffset`
/// (forward, up; its sideways part is dropped). The Shadow King's bullet: 1 -> 270 cm/s at 30 cm/s², 150 cm burst.
fn track_shot(p: &serde_json::Value, range: f32) -> Option<Shot> {
    let actor = p["SpawnActor"]["ObjectPath"].as_str()?;
    let acc = class_default(actor, "Acc")?.as_f64()? as f32;
    let pm = component_props(actor, "ProjectileMovementComponent");
    let speed = pm.get("InitialSpeed")?.as_f64()? as f32;
    let max_speed = pm.get("MaxSpeed").and_then(|m| m.as_f64()).map_or(speed, |m| m as f32).max(speed);
    let num = |k: &str| class_default(actor, k).and_then(|v| v.as_f64()).map(|v| v as f32);
    let dist = num("MaxExistDist").unwrap_or(range);
    // How long flying `dist` takes: speeding up for `ramp` s, then at `max_speed`.
    let ramp = if acc > 0.0 { (max_speed - speed) / acc } else { 0.0 };
    let ramp_dist = (speed + max_speed) * 0.5 * ramp;
    let to_dist = if dist <= ramp_dist && acc > 0.0 { ((speed * speed + 2.0 * acc * dist).sqrt() - speed) / acc } else { ramp + (dist - ramp_dist) / max_speed.max(1.0) };
    let lifetime = num("MaxExistTime").map_or(to_dist, |t| t.min(to_dist));
    let ext = component_props(actor, "BoxComponent").get("BoxExtent").cloned();
    let radius = ext.and_then(|e| Some(e["Y"].as_f64()?.min(e["Z"].as_f64()?) as f32)).unwrap_or(10.0);
    let effects: Vec<String> = class_default(actor, "GameplayEffectClass").and_then(|v| v.as_array().map(|a| a.iter().filter_map(|g| g["ObjectPath"].as_str().map(str::to_string)).collect())).unwrap_or_default();
    let factor = effects.iter().find_map(|g| ge_factor(g)).unwrap_or(1.0);
    let burst = if class_default(actor, "bExplosive").and_then(|b| b.as_bool()) == Some(true) { num("ExplodeRadius").unwrap_or(0.0) } else { 0.0 };
    let off = &p["SpawnOffset"];
    let offset = off.is_object().then(|| (off["X"].as_f64().unwrap_or(0.0) as f32, off["Z"].as_f64().unwrap_or(0.0) as f32));
    Some(Shot { speed, range, gravity: 0.0, turn_rate: 1.0e6, radius, burst, accel: acc, max_speed, lifetime: Some(lifetime), offset, aim_degrees: None, slow: None, factor, toss: None, ignore_guard: class_default(actor, "bIgnoreGuard").and_then(|b| b.as_bool()) == Some(true), delay: class_default(actor, "DelayLaunchTime").and_then(|t| t.as_f64()).unwrap_or(0.0) as f32 })
}

/// A timed effect that sets `MoveSpeedRate` by a multiplier: (1 - magnitude, duration s).
pub(crate) fn ge_slow(object_path: &str) -> Option<(f64, f32)> {
    // The loose read when the full one has none (the weapons' own effects).
    let v = asset_json(object_path).or_else(|| crate::staffs::read_json("json/loose", &crate::staffs::package_of(object_path)))?;
    let cdo = v.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))?;
    let p = &cdo["Properties"];
    let seconds = p["DurationMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()? as f32;
    let m = p["Modifiers"].as_array()?.iter().find(|m| m["Attribute"]["AttributeName"] == "MoveSpeedRate" && m["ModifierOp"].as_str().is_some_and(|o| o.ends_with("::4")))?;
    let rate = m["ModifierMagnitude"]["ScalableFloatMagnitude"]["Value"].as_f64()?;
    Some((1.0 - rate, seconds))
}

/// A summon ability's [`crate::summon::Summon`]: its first `SummonMonsters` entry's first monster (a roster row by
/// its Blueprint class) with that row's kit and curve stats, the entry's `RelativeLocation`, and the summon values
/// (`SummonLimit`, `SummoningRadius`, `MinDistanceToOwner`, `SearchingTimes`, `DestroyWhenSummonerDied`) from the
/// ability or its parents.
fn summon_spec(p: &serde_json::Value, ga: &str) -> Option<crate::summon::Summon> {
    let field = |o: &serde_json::Value, key: &str| o.as_object()?.iter().find(|(k, _)| k.split('_').next() == Some(key)).map(|(_, v)| v.clone());
    let db = load_monster_db()?;
    // Each entry's first monster (every entry decoded so far names one, `Chance` 1.0).
    let entry = |entry: &serde_json::Value| -> Option<(&'static str, bevy_math::Vec3, Box<crate::summon::Minion>)> {
        // `GA_TGF_Monster_Summon_FixedTransform_Base` entries give a `Transform` instead.
        let rel = field(entry, "RelativeLocation").or_else(|| field(entry, "Transform").map(|t| t["Translation"].clone()))?;
        let offset = bevy_math::Vec3::new(rel["X"].as_f64()? as f32, rel["Y"].as_f64()? as f32, rel["Z"].as_f64()? as f32);
        let class = field(field(entry, "Monsters")?.as_array()?.first()?, "Actor")?;
        let bp = blueprints().get(quoted(&class)?)?;
        let row = ROSTER
            .iter()
            .chain(BOSSES)
            .find(|s| blueprint(s).is_some_and(|b| std::ptr::eq(b, bp)))
            // A `*_Summon` class (the Reaper's `BP_Tgf_Monster_Skeleton_Bow_Summon`) is its folder's monster: the
            // row of the same folder and curve row (HYPOTHESIS: its kit and stats are that monster's).
            .or_else(|| ROSTER.iter().chain(BOSSES).find(|s| s.dir == bp.folder && bp.curve_row.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(s.row))))?;
        let kit = load_kit(row).ok()?;
        let sheet = db.get(row.row)?;
        let minion = crate::summon::Minion { kit, combatant: crate::melee::Combatant { attack: sheet.attack_stats(), defence: sheet.defence_stats(), guard_reduce: sheet.guard_reduce() }, max_health: sheet.max_health(), senses: sheet.senses(), speed: sheet.move_speed() as f32 };
        Some((row.row, offset, Box::new(minion)))
    };
    let mut entries = p["SummonMonsters"].as_array()?.iter().filter_map(entry);
    let (row, offset, minion) = entries.next()?;
    let rest = entries.collect();
    let num = |key: &str| p.get(key).and_then(|v| v.as_f64()).or_else(|| class_default(ga, key)?.as_f64());
    let die_with_owner = p.get("DestroyWhenSummonerDied").and_then(|v| v.as_bool()).or_else(|| class_default(ga, "DestroyWhenSummonerDied")?.as_bool()).unwrap_or(false);
    // `DestroyDelayTime` (read natively: HYPOTHESIS, they go this long after the summoner).
    let die_delay = p.get("DestroyDelayTime").and_then(|v| v.as_f64()).or_else(|| class_default(ga, "DestroyDelayTime")?.as_f64()).unwrap_or(0.0) as f32;
    // Fixed spots (`PrepareSummonDataWithFixedTransform`): every entry, at the summoner's spawn point plus its
    // translation (`Get Location by Spawn Offset`, world axes), no limit; the native `FindUnoccupiedLocationForSpawn`
    // moves a blocked one (HYPOTHESIS: a short search round it, `overrides::summon`).
    if p["SummonMonsters"].as_array()?.iter().any(|e| field(e, "Transform").is_some()) {
        use crate::overrides::summon as tune;
        return Some(crate::summon::Summon { row, offset, limit: u32::MAX, radius: tune::FIXED_RADIUS, min_distance: 0.0, tries: tune::FIXED_TRIES, die_with_owner, die_delay, minion, rest, fixed: true });
    }
    Some(crate::summon::Summon { row, offset, limit: num("SummonLimit")? as u32, radius: num("SummoningRadius")? as f32, min_distance: num("MinDistanceToOwner")? as f32, tries: num("SearchingTimes")? as u32, die_with_owner, die_delay, minion, rest, fixed: false })
}

/// A dash ability (children of `GA_TGF_DashHit_Monster_Base`; CONFIRMED from its bytecode): a timer of
/// `WaitDashTime` s, then `ApplyRootMotionConstantForce` along the facing at `DashForce` cm/s for `DashTime`
/// s (additive, velocity set to zero at the end), its body weapon hitting meanwhile. The Virus's cast
/// montage is 0.1 s of idle whose end would end the ability before the timer; here the attack lasts until
/// the dash ends (HYPOTHESIS). Its damage is the curve's (the Virus's `RightWeaponAttack` is 0).
fn dash_attack(p: &serde_json::Value, montage: &str, range: f32) -> Option<MonsterAttack> {
    let wait = p["WaitDashTime"].as_f64()? as f32;
    let force = p["DashForce"].as_f64()? as f32;
    let time = p["DashTime"].as_f64().unwrap_or(0.0) as f32;
    let (start, ticks) = ((wait / crate::DT).round() as usize, ((time / crate::DT).ceil() as usize).max(1));
    let mut root = vec![Vec2::ZERO; start + ticks];
    let mut left = time;
    for r in root.iter_mut().skip(start) {
        *r = Vec2::new(force * left.min(crate::DT), 0.0);
        left -= crate::DT;
    }
    let end = (start + ticks) as f32 * crate::DT;
    Some(MonsterAttack { name: montage.to_string(), length: end + crate::DT, windows: vec![(wait, end)], sounds: Vec::new(), shot: None, range: Some(range), area: None, summon: false, own_summon: None, class: String::new(), cloud: None, events: Vec::new(), angle: None, pick: Default::default(), effect: None, chain: None, guard: None, burst: None, volley: Vec::new(), aoe: Vec::new(), spray: None, aura: None, factor: 1.0, root, tags: Vec::new() })
}

/// The attacks of a monster whose attack montages don't decode: every `M_<...>_Attack<n>` sequence (not
/// the `_H_` or `_Move` ones), its length and sound cues, with a HYPOTHESIS hit window
/// ([`crate::overrides::monster_clips`]). The name is the montage's (`AM_...`), so the client finds the
/// clip the same way, and a montage decoded later takes over.
pub fn sequence_kit(monster: &str, dir: &str) -> anyhow::Result<MonsterKit> {
    let root = db_formats::data_path(MONSTER_JSON).join(dir).join("Animation");
    let mut names: Vec<_> = std::fs::read_dir(&root)
        .with_context(|| format!("no decoded animations for {dir}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("M_") && !n.starts_with("M_H_") && n.contains("_Attack") && !n.contains("_Move") && n.ends_with(".json"))
        .collect();
    names.sort();
    let tune = crate::overrides::monster_clips::WINDOW_AT;
    let mut attacks = Vec::new();
    for n in names {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(&n))?)?;
        let mut a = montage_from_json(&v).with_context(|| n.clone())?;
        if a.length <= 0.2 {
            continue;
        }
        if a.windows.is_empty() {
            let at = a.length * tune;
            a.windows.push((at, (at + crate::overrides::monster_clips::WINDOW_LENGTH).min(a.length)));
        }
        a.name = format!("A{}", a.name);
        attacks.push(a);
    }
    anyhow::ensure!(!attacks.is_empty(), "{dir}: no attack sequence");
    Ok(MonsterKit { name: monster.to_string(), attacks, ..Default::default() })
}

/// One decoded animation export list of `dir`.
fn animation_json(dir: &str, name: &str) -> Option<serde_json::Value> {
    serde_json::from_slice(&std::fs::read(db_formats::data_path(MONSTER_JSON).join(dir).join("Animation").join(format!("{name}.json"))).ok()?).ok()
}

/// The decoded clip (AnimSequence name, montage length) of montage `name` in `dir`: its first segment.
fn montage_clip(dir: &str, name: &str) -> Option<(String, f32)> {
    let v = animation_json(dir, name)?;
    let a = montage_from_json(&v).ok()?;
    let clip = montage_segments_from_json(&v).into_iter().next().map(|(c, _)| c)?;
    Some((clip, a.length))
}

/// An AnimSequence's length, if it decoded.
fn sequence_length(dir: &str, name: &str) -> Option<f32> {
    let v = animation_json(dir, name)?;
    v.as_array()?.iter().find(|o| o["Type"] == "AnimSequence")?["Properties"]["SequenceLength"].as_f64().map(|l| l as f32)
}

/// A monster's dormant clips ([`Dormancy`]).
pub fn load_dormancy(dir: &str) -> Dormancy {
    let m = dir.trim_start_matches("M_");
    let pair = |sleep: &str, wake: &str| (montage_clip(dir, &format!("AM_{m}_{sleep}")), montage_clip(dir, &format!("AM_{m}_{wake}")));
    let (mut sleep, mut wake) = pair("Dormant", "WakeUp");
    if wake.is_none() {
        let (s2, w2) = pair("Dormant_2", "WakeUp_2");
        if w2.is_some() {
            (sleep, wake) = (s2, w2);
        }
    }
    let seq = |what: &str| sequence_length(dir, &format!("M_{m}_{what}")).map(|l| (format!("M_{m}_{what}"), l));
    // The montage of its own `GA_TGF_M_<m>_Dormant` / `_WakeUp` ability, wherever it lives (Mimic03's
    // play Mimic02's).
    let ability = |what: &str| ability_clip(dir, &format!("GA_TGF_M_{m}_{what}"));
    let wake_no_damage = ["WakeUp", "WakeUp_2"].iter().find_map(|w| {
        let a = montage_from_json(&animation_json(dir, &format!("AM_{m}_{w}"))?).ok()?;
        a.tags.iter().find(|t| t.2 == NO_DAMAGE).map(|t| (t.0, t.1))
    });
    Dormancy { sleep: sleep.or_else(|| seq("Dormant")).or_else(|| ability("Dormant")), wake: wake.or_else(|| seq("WakeUp")).or_else(|| ability("WakeUp")), wake_no_damage }
}

/// The behaviour tree a monster's own controller runs (`<dir>/AI/AIC_*`'s `BehaviorTree`), if it has one.
/// The hard-mode controllers (`AIC_H_*`, `AIC_Hard_*`) are skipped.
pub fn load_tree(dir: &str) -> Option<String> {
    let ai = db_formats::data_path(MONSTER_JSON).join(dir).join("AI");
    let mut names: Vec<_> = std::fs::read_dir(&ai).ok()?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        if !(n.starts_with("AIC_") && n.ends_with(".json")) || n.starts_with("AIC_H_") || n.starts_with("AIC_Hard_") {
            continue;
        }
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(ai.join(&n)).ok()?).ok()?;
        for o in v.as_array().into_iter().flatten() {
            if let Some(t) = o["Properties"]["BehaviorTree"]["ObjectName"].as_str() {
                return Some(t.trim_start_matches("BehaviorTree'").trim_end_matches('\'').to_string());
            }
        }
    }
    None
}

/// A melee monster the dungeon can spawn. CONFIRMED: the curve-table row (`MonsterAttributeCurve`),
/// the montage folder, and the loot source (`ExcelLootSourceMap` id of the monster's `BP_*_C` class).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterSpec {
    pub row: &'static str,
    pub dir: &'static str,
    pub loot: i32,
    pub name: &'static str,
}

/// Every monster with an attack kit and a loot source, normal difficulty. The ranged ones shoot with
/// HYPOTHESIS shots ([`crate::overrides::ranged`]); the Skeleton Bow and Skeleton Wizard also have
/// decoded melee attacks for close range. Left out: Armor Sword (no decoded animations), Skeleton
/// LongSword (no loot source for a non-tutorial class), bosses ([`BOSSES`]).
pub const ROSTER: &[MonsterSpec] = &[
    MonsterSpec { row: "GoblinAxe", dir: "M_Goblin_Axe", loot: 134, name: "Goblin Axe" },
    MonsterSpec { row: "GoblinDagger", dir: "M_Goblin_Dagger", loot: 110, name: "Goblin Dagger" },
    MonsterSpec { row: "SkeletonSword", dir: "M_Skeleton_Sword", loot: 147, name: "Skeleton Sword" },
    MonsterSpec { row: "SkeletonHammerShield", dir: "M_Skeleton_HammerShield", loot: 120, name: "Skeleton Hammer" },
    MonsterSpec { row: "SkeletonPolearm", dir: "M_Skeleton_Polearm", loot: 122, name: "Skeleton Polearm" },
    MonsterSpec { row: "ArmorHammer", dir: "M_Armor_Hammer", loot: 104, name: "Living Armor" },
    MonsterSpec { row: "InsectGround", dir: "M_Insect_Ground", loot: 114, name: "Crawler" },
    MonsterSpec { row: "Rat", dir: "M_Rat", loot: 119, name: "Rat" },
    MonsterSpec { row: "Zombie", dir: "M_Zombie", loot: 126, name: "Zombie" },
    MonsterSpec { row: "Boar", dir: "M_Boar", loot: 108, name: "Boar" },
    MonsterSpec { row: "GoblinCrossBow", dir: "M_Goblin_CrossBow", loot: 136, name: "Goblin Crossbow" },
    MonsterSpec { row: "GoblinShaman", dir: "M_Goblin_Shaman", loot: 111, name: "Goblin Shaman" },
    MonsterSpec { row: "SkeletonBow", dir: "M_Skeleton_Bow", loot: 146, name: "Skeleton Archer" },
    MonsterSpec { row: "SkeletonWizard", dir: "M_Skeleton_Wizard", loot: 124, name: "Skeleton Wizard" },
    // the castle's flyers.
    MonsterSpec { row: "Bat", dir: "M_Bat", loot: 106, name: "Bat" },
    MonsterSpec { row: "InsectFly", dir: "M_Insect_Fly", loot: 112, name: "Flying Insect" },
    // the castle's ghosts and sword armours.
    MonsterSpec { row: "Ghost", dir: "M_Ghost", loot: 109, name: "Ghost" },
    MonsterSpec { row: "ArmorSword", dir: "M_Armor_Sword", loot: 105, name: "Armor Sword" },
    // the castle's beholders (pillars of fire).
    MonsterSpec { row: "Beholder", dir: "M_Beholder", loot: 107, name: "Beholder" },
    // the mimics, chosen in the chest spawn tables (curve rows `mimic01`-`09`).
    MonsterSpec { row: "Mimic01", dir: "M_Mimic01", loot: 137, name: "Mimic" },
    MonsterSpec { row: "Mimic02", dir: "M_Mimic02", loot: 138, name: "Mimic" },
    MonsterSpec { row: "Mimic03", dir: "M_Mimic03", loot: 139, name: "Mimic" },
    MonsterSpec { row: "Mimic04", dir: "M_Mimic04", loot: 140, name: "Mimic" },
    MonsterSpec { row: "Mimic05", dir: "M_Mimic05", loot: 141, name: "Mimic" },
    MonsterSpec { row: "Mimic06", dir: "M_Mimic06", loot: 142, name: "Mimic" },
    MonsterSpec { row: "Mimic07", dir: "M_Mimic07", loot: 143, name: "Mimic" },
    MonsterSpec { row: "Mimic08", dir: "M_Mimic08", loot: 144, name: "Mimic" },
    MonsterSpec { row: "Mimic09", dir: "M_Mimic09", loot: 145, name: "Mimic" },
    // the Virus (a dash that does no damage: curve `RightWeaponAttack` 0, no GE).
    MonsterSpec { row: "Virus", dir: "M_Virus", loot: 149, name: "Virus" },
    // the other spawns the maps place (the tentacle doesn't move: curve speed 0).
    MonsterSpec { row: "Tentacle", dir: "M_Tentacle", loot: 116, name: "Tentacle" },
    MonsterSpec { row: "Moose", dir: "M_Moose", loot: 118, name: "Moose" },
    MonsterSpec { row: "Treant", dir: "M_Treant", loot: 125, name: "Treant" },
    MonsterSpec { row: "MimicBook", dir: "M_Mimic_Book", loot: 117, name: "Mimic Book" },
    // the Goblin Thief. The maps place its `LootBlue` class (loot source 237; `LootGreen` has weight 0).
    MonsterSpec { row: "GoblinThief", dir: "M_Goblin_Thief", loot: 237, name: "Goblin Thief" },
    // the crypt's two totems and the Summon totem's Moose Ghost. None has a loot source (CONFIRMED,
    // `ExcelLootSourceMap`): loot 0 = none. Their names are the Blueprints' `DefaultCharacterName`.
    MonsterSpec { row: "TotemIceBolt", dir: "M_Totem", loot: 0, name: "Totem" },
    MonsterSpec { row: "TotemSummon", dir: "M_Totem", loot: 0, name: "Totem" },
    // the Wendigo's Lava totem (summoned only; no loot, as the other totems).
    MonsterSpec { row: "TotemLava", dir: "M_Totem", loot: 0, name: "Totem" },
    // the Wendigo's Slow totem (summoned only; no loot).
    MonsterSpec { row: "TotemSlow", dir: "M_Totem", loot: 0, name: "Totem" },
    MonsterSpec { row: "MooseGhost", dir: "M_Moose_Ghost", loot: 0, name: "Moose" },
    // the Shadow King's `Attack8` summon. No loot source (CONFIRMED, `ExcelLootSourceMap`).
    MonsterSpec { row: "ShadowDragon", dir: "M_ShadowDragon", loot: 0, name: "Shadow Dragon" },
];

/// Bosses, normal difficulty: curve row, montage folder, loot source (`ExcelLootSourceMap` id of the
/// boss's `BP_*_C` class, category). Every boss but the Executioner has decoded attack montages with hit
/// windows (Khazra's, the Reaper's first two and the Shadow King's Attack4 since the repaired usmap); the
/// Executioner has no decoded animations, so its attacks are HYPOTHESIS ([`crate::overrides::bosses`]).
pub const BOSSES: &[MonsterSpec] = &[
    MonsterSpec { row: "Werewolf", dir: "M_Werewolf", loot: 246, name: "Werewolf" },
    MonsterSpec { row: "Reaper", dir: "M_Reaper", loot: 253, name: "Reaper" },
    MonsterSpec { row: "Cyclop", dir: "M_Cyclop", loot: 249, name: "Cyclops" },
    MonsterSpec { row: "Wendigo", dir: "M_Wendigo", loot: 255, name: "Wendigo" },
    MonsterSpec { row: "ShadowKing", dir: "M_Shadow_King", loot: 212, name: "Shadow King" },
    MonsterSpec { row: "Khazra", dir: "M_Khazra", loot: 251, name: "Khazra" },
    // the Cursed King (`BP_TGF_Monster_Curesd_King`, loot source 247), placed only in the blocked boss maps.
    MonsterSpec { row: "CuresdKing", dir: "M_Cursed_King", loot: 247, name: "Cursed King" },
    MonsterSpec { row: "Executioner", dir: "M_Executioner", loot: 250, name: "Executioner" },
];

/// CONFIRMED boss spawn groups (`Monster_RandomGroup/DT_Monster_Elite`, `DT_Monster_Elite_Middle`; every
/// row chance 1.0). The `_Hard` group is left out like the other hard rows.
pub const BOSS_GROUPS: &[(&str, &[(&str, f64)])] = &[("DT_Monster_Elite", &[("Khazra", 1.0), ("Reaper", 1.0)]), ("DT_Monster_Elite_Middle", &[("Executioner", 1.0), ("Werewolf", 1.0)])];

/// The roster or boss entry for a curve-table row.
pub fn spec(row: &str) -> Option<&'static MonsterSpec> {
    ROSTER.iter().chain(BOSSES).find(|s| s.row == row)
}

pub fn is_boss(row: &str) -> bool {
    BOSSES.iter().any(|s| s.row == row)
}

/// How much farther than a normal monster this one reaches (attack range and swing reach): bosses are
/// bigger (HYPOTHESIS, [`crate::overrides::bosses::REACH_SCALE`]); 1 for everything else.
pub fn reach_scale(row: &str) -> f32 {
    crate::overrides::bosses::REACH_SCALE.iter().find(|r| r.0 == row).map_or(1.0, |r| r.1)
}

/// A monster's attacks: its decoded montages, or for monsters in [`crate::overrides::bosses::KITS`]
/// without any (the Executioner) the HYPOTHESIS ones (a decoded montage of the same name would give
/// its timing and sounds); ranged monsters add their shot, and the crossbow goblin and shaman have
/// nothing else.
pub fn load_kit(spec: &MonsterSpec) -> anyhow::Result<MonsterKit> {
    let row_spec = spec;
    // The Shadow Dragon has no attack montages of its own to swing: its controller's script.
    if let Some(dragon) = crate::shadow_dragon::DragonSpec::load(spec.dir) {
        let bp = blueprint(spec).cloned().unwrap_or_default();
        return Ok(MonsterKit { name: spec.row.to_string(), flyer: bp.flyable, bp, dragon: Some(std::sync::Arc::new(dragon)), ..Default::default() });
    }
    // A class without abilities or clips of its own plays its parent class's (CONFIRMED: `M_Moose_Ghost` holds
    // only the ghost's mesh, its ABP and its Blueprint; it plays the Moose's).
    let parent_dir = if db_formats::data_path(MONSTER_JSON).join(spec.dir).join("Ability").is_dir() { None } else { blueprint(spec).and_then(|b| b.parent.as_deref()).and_then(|p| blueprints().get(p)).map(|b| b.folder.as_str()) };
    let kit_spec = MonsterSpec { dir: parent_dir.unwrap_or(spec.dir), ..*spec };
    let spec = &kit_spec;
    let shot = crate::overrides::ranged::KITS.iter().find(|k| k.0 == spec.row).map(|&(_, (name, length, release, shot, sounds))| MonsterAttack {
        name: name.to_string(),
        length,
        windows: vec![(release, release + crate::DT)],
        sounds: sounds.iter().map(|&(t, ev)| (t, ev.to_string())).collect(),
        shot: Some(shot),
        range: None,
        area: None,
        summon: false, own_summon: None, class: String::new(), cloud: None, events: Vec::new(), angle: None, pick: Default::default(), effect: None, chain: None, guard: None, burst: None, volley: Vec::new(), aoe: Vec::new(), spray: None, aura: None, factor: 1.0,
        root: Vec::new(),
        tags: Vec::new(),
    });
    let mut kit = match (melee_kit(spec), shot) {
        (Ok(mut kit), Some(s)) => {
            kit.attacks.push(s);
            kit
        }
        (Ok(kit), None) => kit,
        (Err(_), Some(s)) => MonsterKit { name: spec.row.to_string(), attacks: vec![s], ..Default::default() },
        // No montage, no HYPOTHESIS kit: the attack sequences, else none at all (the Goblin Thief's
        // abilities do no damage).
        (Err(e), None) => sequence_kit(spec.row, spec.dir).or_else(|_| passive_kit(spec)).map_err(|_| e)?,
    };
    kit.tree = load_tree(spec.dir).unwrap_or_default();
    kit.thief = crate::thief::ThiefKit::load(spec.dir).map(Box::new);
    kit.king = crate::shadow_king::KingSpec::load(spec.dir).map(Box::new);
    kit.wendigo = crate::wendigo::WendigoSpec::load(spec.dir).map(Box::new);
    kit.npc = blueprint(row_spec).and_then(|b| b.npc_state.as_deref()).and_then(crate::npc_state::NpcState::load).map(Box::new);
    match blueprint(row_spec) {
        Some(bp) => {
            kit.flyer = bp.flyable;
            kit.bp = bp.clone();
        }
        None => kit.flyer = crate::overrides::ai::FLYERS.contains(&spec.row),
    }
    kit.dormancy = load_dormancy(spec.dir);
    // Its Blueprint's `Dormants[0].WakeUpAbility` (the thief's `GA_TGF_M_GoblinThief_Wakeup1`).
    if kit.dormancy.wake.is_none()
        && let Some(ga) = kit.bp.dormant.as_ref().and_then(|d| d.wake_ability.as_deref())
    {
        kit.dormancy.wake = ability_clip(spec.dir, ga.trim_end_matches("_C"));
    }
    Ok(kit)
}

/// A monster with an ability set that grants no attack (the Goblin Thief, CONFIRMED: no ability of it does
/// damage): a kit without attacks.
fn passive_kit(spec: &MonsterSpec) -> anyhow::Result<MonsterKit> {
    let folder = db_formats::data_path(MONSTER_JSON).join(spec.dir).join("Ability");
    let has_set = std::fs::read_dir(&folder)?.filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().starts_with("AbilitySet_M_"));
    anyhow::ensure!(has_set, "no ability set in {}", spec.dir);
    Ok(MonsterKit { name: spec.row.to_string(), ..Default::default() })
}

/// The clip and length of the montage an ability of the monster's folder plays (`<dir>/Ability/<ga>.json`).
fn ability_clip(dir: &str, ga: &str) -> Option<(String, f32)> {
    let ga = std::fs::read(db_formats::data_path(MONSTER_JSON).join(dir).join("Ability").join(format!("{ga}.json"))).ok()?;
    let ga: serde_json::Value = serde_json::from_slice(&ga).ok()?;
    let cdo = ga.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))?;
    let v = cdo["Properties"]["CastMontageSet"]["DefaultMontage"]["ObjectPath"].as_str().and_then(monster_json)?;
    let a = montage_from_json(&v).ok()?;
    montage_segments_from_json(&v).into_iter().next().map(|(c, _)| (c, a.length))
}

fn melee_kit(spec: &MonsterSpec) -> anyhow::Result<MonsterKit> {
    match load_monster_kit(spec.row, spec.dir) {
        Ok(kit) => Ok(kit),
        Err(e) => {
            let Some(&(_, attacks)) = crate::overrides::bosses::KITS.iter().find(|k| k.0 == spec.row) else { return Err(e) };
            let root = db_formats::data_path(MONSTER_JSON);
            let attacks = attacks
                .iter()
                .map(|&(name, length, window)| {
                    // A decoded montage of that name gives its length and sound cues.
                    let decoded = std::fs::read(root.join(spec.dir).join("Animation").join(format!("{name}.json"))).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| montage_from_json(&v).ok());
                    match decoded {
                        Some(m) => MonsterAttack { windows: vec![window], ..m },
                        None => MonsterAttack { name: name.to_string(), length, windows: vec![window], sounds: Vec::new(), shot: None, range: None, area: None, summon: false, own_summon: None, class: String::new(), cloud: None, events: Vec::new(), angle: None, pick: Default::default(), effect: None, chain: None, guard: None, burst: None, volley: Vec::new(), aoe: Vec::new(), spray: None, aura: None, factor: 1.0, root: Vec::new(), tags: Vec::new() },
                    }
                })
                .collect();
            Ok(MonsterKit { name: spec.row.to_string(), attacks, ..Default::default() })
        }
    }
}

/// CONFIRMED spawn groups (`Monster_RandomGroup/DT_Monster_*`) with their row weights, keeping only rows whose
/// monster is in [`ROSTER`] (the `_H_` hard rows are dropped; Armor Sword joined). Rat, Zombie and Boar are in
/// no group; they come from `overrides::dungeon::EXTRA_GROUPS`.
pub const SPAWN_GROUPS: &[(&str, &[(&str, f64)])] = &[
    ("DT_Monster_Goblin_Melee", &[("GoblinAxe", 1.0), ("GoblinDagger", 1.0)]),
    ("DT_Monster_Skeleton_Melee", &[("SkeletonSword", 5.0), ("SkeletonHammerShield", 3.0), ("SkeletonPolearm", 2.0)]),
    ("DT_Monster_Armor", &[("ArmorHammer", 1.0), ("ArmorSword", 1.0)]),
    ("DT_Monster_Insect_Ground", &[("InsectGround", 0.95)]),
    ("DT_Monster_Goblin_Ranged", &[("GoblinCrossBow", 0.65), ("GoblinShaman", 0.3)]),
    ("DT_Monster_Skeleton_Ranged", &[("SkeletonBow", 0.65), ("SkeletonWizard", 0.3)]),
];

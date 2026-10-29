//! Attributes and classes.
//!
//! A class sheet is one `ExcelRoleLevelUpgrade` row (class × level): a `DBattleAttributeOffset` whose
//! set fields carry `fixed_value`. Attribute keys are the protocol field names with the `affix_`
//! prefix, the same names `ExcelBattleAttributeDescription` and the tooltip Lua use
//! (e.g. `affix_attack_critical_rate`). CONFIRMED: values as in the data.
//! HYPOTHESIS: how primary stats (strength, dexterity, …) feed derived stats is not in the client;
//! sheets carry the raw values only.

use std::collections::BTreeMap;

use anyhow::{Context, bail};
use db_formats::ConfigTables;
use db_formats::config::generated::{DBattleAttributeOffset, EnItemWearingPart, EnRoleCareerType};

use crate::combat::{AttackStats, DefenceStats, Element, PerElement};

/// The 8 playable adventurer classes (F11).
pub const CLASSES: [EnRoleCareerType; 8] = [
    EnRoleCareerType::Fighter,
    EnRoleCareerType::Swordmaster,
    EnRoleCareerType::Priest,
    EnRoleCareerType::Rogue,
    EnRoleCareerType::Wizard,
    EnRoleCareerType::Deathknight,
    EnRoleCareerType::Lich,
    EnRoleCareerType::DruidPanther,
];

/// Attribute name (`affix_*`) → value for one class at one level.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttributeSheet(pub BTreeMap<String, f64>);

impl AttributeSheet {
    /// Flatten a `DBattleAttributeOffset`: every set attribute with its `fixed_value`.
    pub fn from_offset(o: &DBattleAttributeOffset) -> anyhow::Result<Self> {
        let v = serde_json::to_value(o)?;
        let mut m = BTreeMap::new();
        for (k, f) in v.as_object().context("attribute offset is not an object")? {
            if let Some(x) = f.get("fixed_value").and_then(serde_json::Value::as_f64) {
                // Stored as f32: keep the shortest decimal form (0.2, not 0.2000000029…).
                let x = (x as f32).to_string().parse::<f64>()?;
                m.insert(format!("affix_{k}"), x);
            }
        }
        Ok(Self(m))
    }

    /// The value of an attribute, 0 when the sheet doesn't set it (protobuf default).
    pub fn get(&self, name: &str) -> f64 {
        self.0.get(name).copied().unwrap_or(0.0)
    }

    /// Attacker side for [`crate::combat`]. Element names: none→physical, thunder→lightning;
    /// `*_all` adds to every element (HYPOTHESIS, the name suggests it).
    pub fn attack_stats(&self) -> AttackStats {
        let all_boost = self.get("affix_attack_element_boost_rate_all");
        let mut boost = PerElement::default();
        let mut pen = PerElement::default();
        for e in Element::ALL {
            boost.set(e, self.get(&format!("affix_attack_element_boost_rate_{}", boost_suffix(e))) + all_boost);
            pen.set(e, self.get(&format!("affix_attack_{}_penetration", penetration_prefix(e))));
        }
        AttackStats {
            right_weapon_attack: self.get("affix_attack_right_weapon_attack"),
            attack_bonus: self.get("affix_attack_attack_bonus"),
            attack_boost_rate: self.get("affix_attack_attack_boost_rate"),
            def_ignore_base: self.get("affix_attack_def_ignore_base"),
            // The tooltip parameter is `att_gear_level`; the protocol field is `right_att_gear_level`.
            att_gear_level: self.get("affix_attack_right_att_gear_level"),
            extra_att_gear_level: self.get("affix_attack_extra_att_gear_level"),
            critical_rate: self.get("affix_attack_critical_rate"),
            crit_damage_rate: self.get("affix_attack_crit_damage_rate"),
            element_boost_rate: boost,
            skill_damage: self.get("affix_attack_skill_damage"),
            penetration: pen,
            strength: self.get("affix_first_level_strength"),
            intelligence: self.get("affix_first_level_intelligence"),
            will: self.get("affix_first_level_will"),
            heal_boost_rate: self.get("affix_recover_heal_boost_rate"),
            shield_boost_rate: self.get("affix_recover_shield_boost_rate"),
        }
    }

    /// Defender side for [`crate::combat`]. Players have one defence gear level
    /// (`def_gear_level + extra_def_gear_level`, from equipped armour) used for every hit zone
    /// (HYPOTHESIS); `*_all` resistance adds to every element (HYPOTHESIS).
    pub fn defence_stats(&self) -> DefenceStats {
        let all = self.get("affix_defend_element_resistance_rate_all");
        let mut res = PerElement::default();
        for e in Element::ALL {
            res.set(e, self.get(&format!("affix_defend_element_resistance_rate_{}", boost_suffix(e))) + all);
        }
        let level = self.get("affix_defend_def_gear_level") + self.get("affix_defend_extra_def_gear_level");
        DefenceStats { chest_def_gear_level: level, helmet_def_gear_level: level, resistance: res }
    }
}

fn boost_suffix(e: Element) -> &'static str {
    match e {
        Element::Physical => "none",
        Element::Fire => "fire",
        Element::Ice => "ice",
        Element::Thunder => "thunder",
        Element::Light => "light",
        Element::Dark => "dark",
    }
}

fn penetration_prefix(e: Element) -> &'static str {
    match e {
        Element::Physical => "physical",
        Element::Fire => "fire",
        Element::Ice => "cold",
        Element::Thunder => "lightning",
        Element::Light => "divine",
        Element::Dark => "shadow",
    }
}

/// `ExcelRoleType.type_id` of the playable (all-environments) row for a class.
pub fn class_type_id(t: &ConfigTables, career: EnRoleCareerType) -> anyhow::Result<i32> {
    use db_formats::config::generated::EnEnvironmentType;
    t.excel_role_type
        .iter()
        .find(|r| r.environment == EnEnvironmentType::AllAvailable && r.meta.as_ref().is_some_and(|m| m.career == career))
        .map(|r| r.type_id)
        .with_context(|| format!("no playable ExcelRoleType row for {career:?}"))
}

/// Levels a class has rows for, ascending.
pub fn class_levels(t: &ConfigTables, type_id: i32) -> Vec<i32> {
    let mut v: Vec<i32> = t.excel_role_level_upgrade.iter().filter(|r| r.role_type_id == type_id).map(|r| r.role_level).collect();
    v.sort_unstable();
    v
}

/// The attribute sheet of a class at a level.
pub fn class_sheet(t: &ConfigTables, type_id: i32, level: i32) -> anyhow::Result<AttributeSheet> {
    let row = t
        .excel_role_level_upgrade
        .iter()
        .find(|r| r.role_type_id == type_id && r.role_level == level)
        .with_context(|| format!("no ExcelRoleLevelUpgrade row for class {type_id} level {level}"))?;
    let off = row.role_attribute.as_ref().with_context(|| format!("class {type_id} level {level}: no attributes"))?;
    let sheet = AttributeSheet::from_offset(off)?;
    if sheet.0.is_empty() {
        bail!("class {type_id} level {level}: empty sheet");
    }
    Ok(sheet)
}

/// One starting item: (wearing slot, item id, count).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StartItem {
    pub slot: EnItemWearingPart,
    pub item_id: i32,
    pub count: i32,
}

/// A new character's equipment: `ExcelRoleType.create_default_item_config_ids` → `ExcelRoleDefaultEquipment`.
pub fn starting_gear(t: &ConfigTables, type_id: i32) -> anyhow::Result<Vec<StartItem>> {
    let role = t.excel_role_type.iter().find(|r| r.type_id == type_id).with_context(|| format!("no class {type_id}"))?;
    role.create_default_item_config_ids
        .iter()
        .map(|cid| {
            t.excel_role_default_equipment
                .iter()
                .find(|e| e.config_id == *cid)
                .map(|e| StartItem { slot: e.wear_part, item_id: e.item_id, count: e.count })
                .with_context(|| format!("class {type_id}: default equipment {cid} missing"))
        })
        .collect()
}

/// A new character's backpack: the starting rows with no wearing part (`EN_ITEM_STORAGE_TYPE_BACKPACK_WITHOUT_XY`): the
/// crossbow and 15 arrows of the Swordmaster, Fighter, Rogue and Death Knight (and the Swordmaster's four spare swords).
/// (item, count).
pub fn starting_bag(t: &ConfigTables, type_id: i32) -> anyhow::Result<Vec<(i32, u16)>> {
    Ok(starting_gear(t, type_id)?.into_iter().filter(|g| g.slot == EnItemWearingPart::Invalid).map(|g| (g.item_id, g.count.clamp(1, i32::from(u16::MAX)) as u16)).collect())
}

/// The slots a new character starts with items in: the worn ones and the second weapon set.
pub const STARTING_SLOTS: [EnItemWearingPart; 9] = [
    EnItemWearingPart::Pair1rightweapon,
    EnItemWearingPart::Pair1leftweapon,
    EnItemWearingPart::Helmet,
    EnItemWearingPart::Upperarmor,
    EnItemWearingPart::Glove,
    EnItemWearingPart::Lowerarmor,
    EnItemWearingPart::Shoe,
    EnItemWearingPart::Pair2rightweapon,
    EnItemWearingPart::Pair2leftweapon,
];

/// Slots that count as worn equipment (the second weapon pair and the bag entries do not).
pub const WORN_SLOTS: [EnItemWearingPart; 7] = [
    EnItemWearingPart::Pair1rightweapon,
    EnItemWearingPart::Pair1leftweapon,
    EnItemWearingPart::Helmet,
    EnItemWearingPart::Upperarmor,
    EnItemWearingPart::Glove,
    EnItemWearingPart::Lowerarmor,
    EnItemWearingPart::Shoe,
];

/// A class sheet plus worn items, ready for combat.
#[derive(Debug, Clone, PartialEq)]
pub struct Loadout {
    pub sheet: AttributeSheet,
    /// Damage reduction while blocking: the best `defend_guarded_damage_reduce_rate` among worn items.
    pub guard_reduce: f64,
    pub items: Vec<i32>,
}

/// A new character: class sheet at `level` plus the fixed (BASIC) affixes of the starting gear in the
/// worn slots. HYPOTHESIS: starting gear carries no random affixes; affix values add to the sheet;
/// guard reduction takes the best worn item instead of a sum (a sum can exceed 1).
pub fn starting_loadout(t: &ConfigTables, type_id: i32, level: i32, items: &crate::items::ItemDb) -> anyhow::Result<Loadout> {
    let mut sheet = class_sheet(t, type_id, level)?;
    let mut guard_reduce: f64 = 0.0;
    let mut worn = Vec::new();
    // Fixed affixes do not depend on the roll; a local RNG keeps the sim's untouched.
    let mut rng = crate::SimRng::new(0);
    for gear in starting_gear(t, type_id)?.iter().filter(|g| WORN_SLOTS.contains(&g.slot)) {
        let item = items.roll(gear.item_id, &mut rng)?;
        for a in item.affixes.iter().filter(|a| a.source == crate::items::AffixSource::Fixed) {
            if a.attribute == "defend_guarded_damage_reduce_rate" {
                guard_reduce = guard_reduce.max(a.value);
            } else {
                *sheet.0.entry(format!("affix_{}", a.attribute)).or_insert(0.0) += a.value;
            }
        }
        worn.push(gear.item_id);
    }
    Ok(Loadout { sheet, guard_reduce, items: worn })
}

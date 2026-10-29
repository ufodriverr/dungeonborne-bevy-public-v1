//! Loot: loot source → choose groups → group rate → random id → drop items.
//!
//! CONFIRMED (tables): `ExcelLootSourceMap` (305 sources: actor class → id), `ExcelLootUeSourceChooseGroup`
//! (source → loot ids), `ExcelLootChooseGroupRate` (loot id → random ids with `pr` weights, `default_rate`),
//! `ExcelItemDropItem` (random id → items with `cnt` and `rate`). The rare-pool, unlucky-value, magic-find
//! and disabled-loot tables are empty in the client, and only 11 sources (4 chests, 7 tutorial monsters)
//! have choose groups. HYPOTHESIS (roll code): a loot id drops with probability `default_rate / 10000`; one
//! random id is picked by `pr`; one drop row is picked by `rate` weight. Sources with no client loot use the
//! per-kind fallback in [`crate::overrides::FALLBACK_LOOT`].

use std::collections::HashMap;

use anyhow::Context;
use db_formats::ConfigTables;
use db_formats::config::generated::{EnItemRarity, ExcelItemDropItem, ExcelLootChooseGroupRate};

use crate::items::{Item, ItemDb, weighted};
use crate::rng::SimRng;

/// What kind of loot source an actor class is, from its name (HYPOTHESIS; the server's tables are missing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceKind {
    NoLoot,
    Container,
    Chest,
    ChestGold,
    Mimic,
    Monster,
    MonsterHard,
    Boss,
    BossHard,
    LegendWeapon,
    /// The Goblin Thief: no client loot for its sources (237/238; the hard ones 234-236).
    Thief,
}

const BOSSES: [&str; 8] = ["Curesd_King", "Cyclop", "Executioner", "Khazra", "Reaper", "Werewolf", "Wendigo", "Shadow_King"];
const NO_LOOT: [&str; 13] = [
    "Noloot", "Altar_Attack", "Altar_Clarity", "Altar_Defend", "Altar_Heal", "Altar_Recover", "Altar_Relive", "Altar_Speed",
    "Trap_", "Door", "Wall", "Dummy", "Summon",
];
const NO_LOOT_EXACT: [&str; 9] = [
    "BP_ActorSpawnPoint_C",
    "BP_AltarBase_C",
    "BP_Cave_Rock_C",
    "BP_CollectableBase_Torch_C",
    "BP_DestroyableBase_C",
    "BP_Destructible_Fridge_C",
    "BP_Tgf_Monster_Player_C",
    "BP_Tgf_Monster_Bot_C",
    "BP_TGF_VirtualMonster_GoblinThiefDrop_C",
];

pub fn classify(name: &str) -> SourceKind {
    let hard = name.contains("_H_");
    if NO_LOOT_EXACT.contains(&name) || NO_LOOT.iter().any(|k| name.contains(k)) {
        return SourceKind::NoLoot;
    }
    if name.contains("Goblin_Thief") || name.contains("GoblinThief") {
        return SourceKind::Thief;
    }
    if name.contains("Altar") {
        // Award / lock / special altars hold a reward.
        return SourceKind::Chest;
    }
    if name.contains("LegendWeapon") {
        return SourceKind::LegendWeapon;
    }
    if name.contains("CollectableBase") {
        // Potions, coffin flowers, homeland items: consumables are not modelled yet.
        return SourceKind::NoLoot;
    }
    if name.contains("Mimic") || name.contains("Monster_Chest") {
        return SourceKind::Mimic;
    }
    if BOSSES.iter().any(|b| name.contains(b)) {
        return if hard { SourceKind::BossHard } else { SourceKind::Boss };
    }
    if name.contains("Monster") {
        return if hard { SourceKind::MonsterHard } else { SourceKind::Monster };
    }
    if name.contains("Gold") || name.contains("Special") || name.contains("Guarded") {
        return SourceKind::ChestGold;
    }
    if name.contains("Chest") || name.contains("FlowerCoffin") {
        return SourceKind::Chest;
    }
    SourceKind::Container
}

/// One dropped thing.
#[derive(Debug, Clone, PartialEq)]
pub enum Drop {
    /// Equipment rolled with its affixes.
    Gear(Item),
    /// Anything without an affix template (consumables, materials): item id × count.
    Stack { item_id: i32, count: i32 },
}

impl Drop {
    pub fn item_id(&self) -> i32 {
        match self {
            Drop::Gear(i) => i.item_id,
            Drop::Stack { item_id, .. } => *item_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LootRoll {
    pub source_id: i32,
    pub kind: SourceKind,
    /// True when the client tables define this source's loot; false for the fallback.
    pub from_tables: bool,
    pub drops: Vec<Drop>,
}

#[derive(Debug)]
pub struct LootDb<'a> {
    items: ItemDb<'a>,
    names: HashMap<i32, &'a str>,
    groups: HashMap<i32, Vec<i64>>,
    rates: HashMap<i64, &'a ExcelLootChooseGroupRate>,
    drops: HashMap<i64, Vec<&'a ExcelItemDropItem>>,
}

impl<'a> LootDb<'a> {
    pub fn new(t: &'a ConfigTables) -> anyhow::Result<Self> {
        let mut drops: HashMap<i64, Vec<&ExcelItemDropItem>> = HashMap::new();
        for d in &t.excel_item_drop_item {
            drops.entry(d.random_id.0).or_default().push(d);
        }
        Ok(Self {
            items: ItemDb::new(t)?,
            names: t.excel_loot_source_map.iter().map(|s| (s.id, s.name.as_str())).collect(),
            groups: t
                .excel_loot_ue_source_choose_group
                .iter()
                .map(|g| (g.ue_loot_source_id, g.loot_ids.iter().map(|l| l.0).collect()))
                .collect(),
            rates: t.excel_loot_choose_group_rate.iter().map(|r| (r.loot_id.0, r)).collect(),
            drops,
        })
    }

    pub fn items(&self) -> &ItemDb<'a> {
        &self.items
    }

    /// Every loot source id, ascending.
    pub fn source_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.names.keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn name(&self, source_id: i32) -> Option<&'a str> {
        self.names.get(&source_id).copied()
    }

    pub fn kind(&self, source_id: i32) -> SourceKind {
        self.name(source_id).map_or(SourceKind::NoLoot, classify)
    }

    pub fn has_table_loot(&self, source_id: i32) -> bool {
        self.groups.contains_key(&source_id)
    }

    fn make_drop(&self, item_id: i32, count: i32, rng: &mut SimRng) -> anyhow::Result<Drop> {
        if self.items.rarity(item_id).is_some() && self.items.item_ids().binary_search(&item_id).is_ok() {
            Ok(Drop::Gear(self.items.roll(item_id, rng)?))
        } else {
            Ok(Drop::Stack { item_id, count: count.max(1) })
        }
    }

    pub fn roll(&self, source_id: i32, rng: &mut SimRng) -> anyhow::Result<LootRoll> {
        let name = self.name(source_id).with_context(|| format!("unknown loot source {source_id}"))?;
        let kind = classify(name);
        let mut drops = Vec::new();
        if let Some(loot_ids) = self.groups.get(&source_id) {
            for lid in loot_ids {
                let rate = self.rates.get(lid).with_context(|| format!("source {source_id}: loot id {lid} has no group rate"))?;
                if !rng.chance(f64::from(rate.default_rate) / 10_000.0) {
                    continue;
                }
                let Some(rid) = weighted(rate.random_ids.iter().zip(&rate.pr).map(|(r, p)| (r.0, (*p).max(0) as u64)), rng) else {
                    continue;
                };
                let rows = self.drops.get(&rid).with_context(|| format!("loot id {lid}: random id {rid} has no drop items"))?;
                if let Some(row) = weighted(rows.iter().map(|d| (*d, d.rate.max(0) as u64)), rng) {
                    drops.push(self.make_drop(row.item_id, row.cnt, rng)?);
                }
            }
            return Ok(LootRoll { source_id, kind, from_tables: true, drops });
        }
        let f = crate::overrides::fallback_loot(kind);
        if f.chance > 0.0 && rng.chance(f.chance) {
            let n = rng.range_u32(f.min_items, f.max_items + 1);
            let by_rarity = self.items.by_rarity();
            for _ in 0..n {
                let Some(r) = weighted(f.rarity_weights.iter().enumerate().map(|(i, w)| (i as i32 + 1, *w)), rng) else { break };
                let Some(ids) = by_rarity.get(&r) else { continue };
                let id = ids[rng.range_u32(0, ids.len() as u32) as usize];
                drops.push(Drop::Gear(self.items.roll(id, rng)?));
            }
        }
        Ok(LootRoll { source_id, kind, from_tables: false, drops })
    }

    /// Rarity of a drop, if it is gear.
    pub fn rarity(&self, d: &Drop) -> Option<EnItemRarity> {
        match d {
            Drop::Gear(i) => Some(i.rarity),
            Drop::Stack { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_examples() {
        assert_eq!(classify("BP_ClayPot_12_Noloot_C"), SourceKind::NoLoot);
        assert_eq!(classify("BP_ClayPot_12_C"), SourceKind::Container);
        assert_eq!(classify("BP_TreasureChest_Wood_02_C"), SourceKind::Chest);
        assert_eq!(classify("BP_TreasureChest_Gold_01_C"), SourceKind::ChestGold);
        assert_eq!(classify("BP_Tgf_Monster_Mimic04_C"), SourceKind::Mimic);
        assert_eq!(classify("BP_TGF_Monster_Rat_C"), SourceKind::Monster);
        assert_eq!(classify("BP_TGF_H_Monster_Rat_C"), SourceKind::MonsterHard);
        assert_eq!(classify("BP_TGF_Monster_Shadow_King_C"), SourceKind::Boss);
        assert_eq!(classify("BP_TGF_H_Werewolf_C"), SourceKind::BossHard);
        assert_eq!(classify("BP_TGF_Monster_Zombie_Summon_ShadowKing_1_C"), SourceKind::NoLoot);
        assert_eq!(classify("BP_Altar_Heal_C"), SourceKind::NoLoot);
        assert_eq!(classify("BP_Altar_Award_C"), SourceKind::Chest);
        assert_eq!(classify("BP_CollectableBase_LegendWeapon_Sword5001_C"), SourceKind::LegendWeapon);
        assert_eq!(classify("BP_Trap_Stone_01_C"), SourceKind::NoLoot);
        assert_eq!(classify("BP_TGF_Monster_Goblin_Thief_LootBlue_C"), SourceKind::Thief);
        assert_eq!(classify("BP_TGF_VirtualMonster_GoblinThiefDrop_C"), SourceKind::NoLoot);
    }
}

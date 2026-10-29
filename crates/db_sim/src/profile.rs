//! What lasts between runs: the character's items, the stash, gold and run counts, saved as versioned
//! JSON. Not part of the simulation state: the game layer loads it before a run, sends the character's
//! items in with `InventoryCommand::SetCharacterItems`, and folds `RunEnded` back in. HYPOTHESIS: the
//! stash size (overrides), that an empty-handed character gets the class starting gear again, and that
//! gold comes from selling to the merchant at `system_repurchase_price`.
//!
//! experience: a run earns its map's `ExcelMapExp` experience for the time the character stayed in it (the row with
//! the longest `alive_time` reached; CONFIRMED table), and `ExcelRoleLevelExp`'s `max_exp` per level takes it up to
//! level 20, whose class sheet (`ExcelRoleLevelUpgrade`) the next run uses. HYPOTHESIS: every outcome earns it (the
//! table pays for time alive), past experience carries over a level, and the top level keeps at most its own
//! `max_exp`; `recover_exp_boost_rate_out_of_portal` and the table's `team_bonus` aren't applied yet.

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::inventory::{BagEntry, GameData, ItemUids};
use crate::overrides::run as tune;
use crate::{Inventory, ItemInstance, RunOutcome};

/// Bumped whenever the file layout changes; other versions are refused (never silently reinterpreted).
pub const PROFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    /// The character's class, once picked.
    #[serde(default)]
    pub career: Option<db_formats::config::generated::EnRoleCareerType>,
    pub gold: u64,
    /// Worn items and backpack the character takes into the next run.
    pub character: Inventory,
    pub stash: Inventory,
    pub runs: u32,
    pub extractions: u32,
    pub deaths: u32,
    /// The Q and E skills picked on the stash screen; `None` = the class default.
    #[serde(default)]
    pub skills: Option<[crate::SkillKind; 2]>,
    /// The map picked on the stash screen (a [`crate::crypt::MapDef`] key); `None` = the default.
    #[serde(default)]
    pub map: Option<String>,
    /// The character's level (1-20) and experience into it.
    #[serde(default = "first_level")]
    pub level: u32,
    #[serde(default)]
    pub exp: u32,
}

fn first_level() -> u32 {
    1
}

/// The worn gear's `recover_exp_boost_rate_out_of_portal` (329 items roll it, 0.5-1.0 on the heirlooms' fixed rules):
/// a run left through a portal earns its experience times 1 + this (HYPOTHESIS: added to the run's own, the sheathed
/// weapon pair not counting, as `worn_sheet`).
pub fn portal_exp_boost(inv: &Inventory) -> f64 {
    use db_formats::config::generated::EnItemWearingPart as W;
    inv.worn.iter().filter(|(s, _)| !matches!(s, W::Pair2rightweapon | W::Pair2leftweapon)).flat_map(|(_, i)| &i.item.affixes).filter(|a| a.attribute == "recover_exp_boost_rate_out_of_portal").map(|a| a.value).sum()
}

/// A run's experience with the portal boost when it was left by extraction.
pub fn boosted_exp(exp: u32, outcome: RunOutcome, inv: &Inventory) -> u32 {
    if outcome != RunOutcome::Extracted {
        return exp;
    }
    (f64::from(exp) * (1.0 + portal_exp_boost(inv).max(0.0))).round() as u32
}

/// The experience a run on map `map_id` earns for `alive_secs` s in it: the row with the longest `alive_time`
/// reached.
pub fn run_exp(t: &db_formats::ConfigTables, map_id: i32, alive_secs: f32) -> u32 {
    t.excel_map_exp.iter().filter(|r| r.map_id == map_id && r.alive_time.0 as f32 <= alive_secs).max_by_key(|r| r.alive_time.0).map_or(0, |r| r.exp.max(0) as u32)
}

impl Default for Profile {
    fn default() -> Self {
        Profile { version: PROFILE_VERSION, career: None, gold: 0, character: Inventory::default(), stash: Inventory::with_size(tune::STASH.0, tune::STASH.1), runs: 0, extractions: 0, deaths: 0, skills: None, map: None, level: 1, exp: 0 }
    }
}

/// Which grid an item sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Backpack,
    Stash,
}

impl Profile {
    /// Reads a profile; `Ok(None)` when there is no file yet.
    pub fn load(path: &Path) -> anyhow::Result<Option<Profile>> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let v: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let version = v.get("version").and_then(serde_json::Value::as_u64).unwrap_or(0);
        anyhow::ensure!(version == u64::from(PROFILE_VERSION), "{} is profile version {version}, this build reads {PROFILE_VERSION}", path.display());
        let mut p: Profile = serde_json::from_value(v).with_context(|| format!("reading {}", path.display()))?;
        p.renumber();
        Ok(Some(p))
    }

    /// Writes the profile (to a temporary file first, then renamed over the old one).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))
    }

    /// Unique uids across the character and the stash (items from different runs may share one).
    pub fn renumber(&mut self) {
        let mut uids = ItemUids::default();
        self.character.renumber(&mut uids);
        self.stash.renumber(&mut uids);
    }

    /// Folds a finished run in: an extraction keeps what was carried (gold coins become gold), a
    /// death or the clock running out loses it.
    pub fn end_run(&mut self, outcome: RunOutcome, kept: Option<&Inventory>) {
        self.runs += 1;
        match (outcome, kept) {
            (RunOutcome::Extracted, Some(inv)) => {
                let mut inv = inv.clone();
                let gold: u64 = inv.bag.iter().filter(|b| b.item.item.item_id == tune::GOLD_ITEM).map(|b| u64::from(b.item.count)).sum();
                inv.bag.retain(|b| b.item.item.item_id != tune::GOLD_ITEM);
                self.gold += gold;
                self.character = inv;
                self.extractions += 1;
            }
            _ => {
                self.character = Inventory::default();
                self.deaths += 1;
            }
        }
        self.renumber();
    }

    /// The experience the current level needs.
    pub fn level_exp(&self, t: &db_formats::ConfigTables) -> u32 {
        t.excel_role_level_exp.iter().find(|r| r.level as u32 == self.level).map_or(0, |r| r.max_exp.max(0) as u32)
    }

    /// Adds experience; returns how many levels it gained.
    pub fn add_exp(&mut self, t: &db_formats::ConfigTables, exp: u32) -> u32 {
        let need = |level: u32| t.excel_role_level_exp.iter().find(|r| r.level as u32 == level).map(|r| r.max_exp.max(1) as u32);
        let top = t.excel_role_level_exp.iter().map(|r| r.level as u32).max().unwrap_or(1);
        let before = self.level;
        self.exp = self.exp.saturating_add(exp);
        while self.level < top
            && let Some(n) = need(self.level)
            && self.exp >= n
        {
            self.exp -= n;
            self.level += 1;
        }
        if self.level >= top {
            self.exp = self.exp.min(need(top).unwrap_or(0));
        }
        self.level - before
    }

    /// The character goes in empty-handed: give it the class starting gear again.
    pub fn needs_starting_gear(&self) -> bool {
        self.character.items().next().is_none()
    }

    fn grid(&mut self, place: Place) -> &mut Inventory {
        match place {
            Place::Backpack => &mut self.character,
            Place::Stash => &mut self.stash,
        }
    }

    /// Takes a backpack or stash entry out by uid.
    fn take(&mut self, uid: u32) -> Option<(Place, BagEntry)> {
        for place in [Place::Backpack, Place::Stash] {
            if let Some(entry) = self.grid(place).take(uid) {
                return Some((place, entry));
            }
        }
        None
    }

    /// Moves a backpack item to the stash or a stash item to the backpack (first free spot).
    pub fn transfer(&mut self, uid: u32, data: &GameData) -> Result<Place, &'static str> {
        let (from, entry) = self.take(uid).ok_or("no such item")?;
        let to = if from == Place::Backpack { Place::Stash } else { Place::Backpack };
        let size = data.info(entry.item.item.item_id).map_or((1, 1), |i| i.size);
        match self.grid(to).first_fit(size, data) {
            Some((x, y)) => {
                self.grid(to).bag.push(BagEntry { item: entry.item, x, y });
                Ok(to)
            }
            None => {
                self.grid(from).bag.push(entry);
                Err("no room there")
            }
        }
    }

    /// Takes off a worn item into the backpack.
    pub fn unequip(&mut self, slot: db_formats::config::generated::EnItemWearingPart, data: &GameData) -> Result<(), &'static str> {
        let item = self.character.worn.remove(&slot).ok_or("nothing worn there")?;
        self.character.add(item, data).map_err(|item| {
            self.character.worn.insert(slot, item);
            "no room in the backpack"
        })
    }

    /// Wears a backpack item in `slot` (the class rules apply; displaced items go to the backpack).
    pub fn equip(&mut self, uid: u32, slot: db_formats::config::generated::EnItemWearingPart, data: &GameData) -> Result<(), &'static str> {
        let career = self.career.ok_or("no class picked")?;
        crate::inventory::edit_inventory(&mut self.character, data, career, crate::InventoryCommand::Equip { id: crate::SimId(0), uid, slot })
    }

    /// Changes class: worn items go to the backpack, then to the stash if the backpack is full.
    pub fn set_career(&mut self, career: db_formats::config::generated::EnRoleCareerType, data: &GameData) {
        if self.career != Some(career) {
            let worn: Vec<_> = std::mem::take(&mut self.character.worn).into_values().collect();
            for item in worn {
                if let Err(item) = self.character.add(item, data) {
                    let _ = self.stash.add(item, data);
                }
            }
        }
        if self.career != Some(career) {
            // Skill choices belong to the class.
            self.skills = None;
        }
        self.career = Some(career);
    }

    /// Sells a backpack or stash entry to the merchant. Returns the gold received.
    pub fn sell(&mut self, uid: u32, data: &GameData) -> Result<u64, &'static str> {
        let (from, entry) = self.take(uid).ok_or("no such item")?;
        let price = u64::from(data.sell_price(entry.item.item.item_id)) * u64::from(entry.item.count);
        if price == 0 {
            self.grid(from).bag.push(entry);
            return Err("the merchant doesn't buy this");
        }
        self.gold += price;
        Ok(price)
    }

    /// Buys a merchant offer into the stash (else the backpack) for its gold.
    pub fn buy(&mut self, offer: &crate::merchant::Offer, data: &GameData) -> Result<(), &'static str> {
        if self.gold < offer.price {
            return Err("not enough gold");
        }
        let rarity = data.items.rarity(offer.item_id).ok_or("the merchant's item isn't known")?;
        let uid = self.all_items().map(|i| i.uid).max().unwrap_or(0) + 1;
        let item = ItemInstance { uid, item: crate::items::Item { item_id: offer.item_id, rarity, affixes: Vec::new() }, count: offer.count };
        // (A stack may fill partly before the room runs out: put both grids back then.)
        let before = (self.stash.clone(), self.character.clone());
        let left = match self.stash.add(item, data) {
            Ok(()) => None,
            Err(rest) => self.character.add(rest, data).err(),
        };
        if left.is_some() {
            (self.stash, self.character) = before;
            return Err("no room in the stash or the backpack");
        }
        self.gold -= offer.price;
        self.renumber();
        Ok(())
    }

    /// All items with where they are (for the stash screen).
    pub fn all_items(&self) -> impl Iterator<Item = &ItemInstance> {
        self.character.items().chain(self.stash.items())
    }
}

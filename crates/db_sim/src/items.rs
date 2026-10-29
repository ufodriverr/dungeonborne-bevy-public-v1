//! Item generation and affix rolls.
//!
//! CONFIRMED (tables): an item id's rarity (`UESourceItemType`), its affix template (`ExcelAffixTemplate`:
//! fixed BASIC rules plus random MAJOR/MINOR rules), count pools (`ExcelAffixCountRandomPool`), attribute
//! pools with weights (`ExcelAffixPool`) and value ranges per value template × attribute × affix type ×
//! rarity (`ExcelAffixValueTemplate`).
//! HYPOTHESIS (roll code; the server did this): a rule with no pool rolls nothing; one pool id is chosen
//! uniformly when a rule lists several; the count is a weighted pick; attributes are drawn by weight
//! without repeats within the item; a value is uniform on the `unit` grid inside [min, max].
//! The minor affix pools the templates name (`1002xx`, `1005xx`, …) are not in the client data; see
//! [`crate::overrides::MINOR_POOL_FALLBACK`].

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, bail};
use db_formats::ConfigTables;
use db_formats::config::generated::{EnBattleAffixType, EnItemRarity, ExcelAffixTemplate};

use crate::rng::SimRng;

/// Attribute names whose protocol field and description name differ.
const ATTRIBUTE_ALIASES: [(&str, &str); 1] = [("all_first_level", "affix__first_level_all")];

/// Where an affix came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AffixSource {
    /// A fixed BASIC rule of the template.
    Fixed,
    /// Drawn from an `ExcelAffixPool`.
    Pool(i32),
    /// The named minor pool is missing from the client data; drawn from the value template instead.
    MinorFallback,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Affix {
    /// Protocol attribute name without `affix_` (e.g. `attack_critical_rate`).
    pub attribute: String,
    pub kind: EnBattleAffixType,
    pub value: f64,
    pub source: AffixSource,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub item_id: i32,
    pub rarity: EnItemRarity,
    pub affixes: Vec<Affix>,
}

/// A value range: [min, max] on a grid of `unit`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub unit: f64,
}

impl Range {
    pub fn contains(&self, v: f64) -> bool {
        let eps = 1e-6 * self.unit.abs().max(1e-6);
        if v < self.min - eps || v > self.max + eps {
            return false;
        }
        self.unit <= 0.0 || (((v - self.min) / self.unit).round() * self.unit - (v - self.min)).abs() <= eps * 10.0
    }

    fn roll(&self, rng: &mut SimRng) -> f64 {
        if self.unit <= 0.0 || self.max <= self.min {
            return self.min;
        }
        let steps = ((self.max - self.min) / self.unit + 1e-9).floor() as u32;
        self.min + f64::from(rng.range_u32(0, steps + 1)) * self.unit
    }
}

/// Lookup tables for rolling, built once from [`ConfigTables`].
#[derive(Debug)]
pub struct ItemDb<'a> {
    templates: HashMap<i32, &'a ExcelAffixTemplate>,
    rarity: HashMap<i32, EnItemRarity>,
    counts: HashMap<i32, Vec<(i32, u64)>>,
    pools: HashMap<i32, Vec<(String, u64)>>,
    attr_type: HashMap<String, i32>,
    /// (affix_value_id, attribute_type, affix type, rarity number) → range
    values: HashMap<(i32, i32, EnBattleAffixType, i32), Range>,
}

fn f32_clean(x: f32) -> f64 {
    x.to_string().parse().unwrap_or(f64::from(x))
}

/// Set attribute fields of a `DBattleAttributeOffset`, as (name, `DBattleAttributeValueOffset` object).
/// Ability sets, gem slots and gear sets (other value types) are left out; they are not rolled here.
fn attribute_fields(v: &impl serde::Serialize) -> anyhow::Result<Vec<(String, serde_json::Value)>> {
    let v = serde_json::to_value(v)?;
    Ok(v.as_object()
        .context("attribute offset is not an object")?
        .iter()
        .filter(|(_, f)| f.get("fixed_value").is_some())
        .map(|(k, f)| (k.clone(), f.clone()))
        .collect())
}

impl<'a> ItemDb<'a> {
    pub fn new(t: &'a ConfigTables) -> anyhow::Result<Self> {
        let mut counts: HashMap<i32, Vec<(i32, u64)>> = HashMap::new();
        for c in &t.excel_affix_count_random_pool {
            counts.entry(c.affix_count_pool_id).or_default().push((c.count, c.random_weight.0.max(0) as u64));
        }
        let mut pools = HashMap::new();
        for p in &t.excel_affix_pool {
            let mut v = Vec::new();
            if let Some(w) = &p.attribute_weight {
                for (name, f) in attribute_fields(w)? {
                    let weight = f.get("weight").and_then(|x| x.as_str().and_then(|s| s.parse::<u64>().ok()).or(x.as_u64())).unwrap_or(0);
                    if weight > 0 {
                        v.push((name, weight));
                    }
                }
            }
            pools.insert(p.affix_pool_id, v);
        }
        let mut attr_type: HashMap<String, i32> = t
            .excel_battle_attribute_description
            .iter()
            .filter_map(|d| d.name.strip_prefix("affix_").map(|n| (n.to_string(), d.attribute_type)))
            .collect();
        for (field, desc) in ATTRIBUTE_ALIASES {
            if let Some(ty) = t.excel_battle_attribute_description.iter().find(|d| d.name == desc).map(|d| d.attribute_type) {
                attr_type.insert(field.to_string(), ty);
            }
        }
        let mut values = HashMap::new();
        for v in &t.excel_affix_value_template {
            let Some(d) = &v.data else { continue };
            for a in &d.affix_data {
                for (rarity, r) in &a.rarity_range {
                    let rarity: i32 = rarity.parse().with_context(|| format!("rarity key `{rarity}`"))?;
                    let range = Range { min: f32_clean(r.min_value), max: f32_clean(r.max_value), unit: f32_clean(d.unit) };
                    values.insert((v.affix_value_id, v.attribute_type, a.affix_type, rarity), range);
                }
            }
        }
        Ok(Self {
            templates: t.excel_affix_template.iter().map(|a| (a.item_id, a)).collect(),
            rarity: t.uesource_item_type.iter().map(|i| (i.type_id, i.rarity)).collect(),
            counts,
            pools,
            attr_type,
            values,
        })
    }

    /// Whether `item_id` has an affix template (equipment does; consumables don't).
    pub fn has_template(&self, item_id: i32) -> bool {
        self.templates.contains_key(&item_id)
    }

    /// Item ids that have an affix template.
    pub fn item_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.templates.keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn rarity(&self, item_id: i32) -> Option<EnItemRarity> {
        self.rarity.get(&item_id).copied()
    }

    /// The value range an affix may take on this item, if the tables define one.
    pub fn range(&self, item_id: i32, attribute: &str, kind: EnBattleAffixType) -> Option<Range> {
        let tpl = self.templates.get(&item_id)?.data.as_ref()?;
        let rarity = self.rarity(item_id)?.number();
        let ty = *self.attr_type.get(attribute)?;
        self.values.get(&(tpl.affix_value_id, ty, kind, rarity)).copied()
    }

    /// Attributes the value template allows for this item, affix type and rarity (sorted).
    fn template_attributes(&self, item_id: i32, kind: EnBattleAffixType) -> Vec<String> {
        let mut v: Vec<String> = self.attr_type.keys().filter(|a| self.range(item_id, a, kind).is_some()).cloned().collect();
        v.sort();
        v
    }

    fn pick_count(&self, pool_id: i32, rng: &mut SimRng) -> anyhow::Result<i32> {
        let opts = self.counts.get(&pool_id).with_context(|| format!("count pool {pool_id} missing"))?;
        Ok(weighted(opts.iter().map(|(c, w)| (*c, *w)), rng).unwrap_or(0))
    }

    /// Roll a new item.
    pub fn roll(&self, item_id: i32, rng: &mut SimRng) -> anyhow::Result<Item> {
        let tpl = self.templates.get(&item_id).with_context(|| format!("no affix template for item {item_id}"))?;
        let data = tpl.data.as_ref().with_context(|| format!("item {item_id}: empty template"))?;
        let rarity = self.rarity(item_id).with_context(|| format!("item {item_id}: no UESourceItemType rarity"))?;
        let mut affixes = Vec::new();
        for rule in &data.fixed_rule {
            let Some(attrs) = &rule.attribute else { continue };
            for (name, f) in attribute_fields(attrs)? {
                let value = if let Some(o) = f.get("offset").filter(|o| o.is_object()) {
                    let g = |k: &str| o.get(k).and_then(serde_json::Value::as_f64).map(|x| f32_clean(x as f32)).unwrap_or(0.0);
                    Range { min: g("min_value"), max: g("max_value"), unit: g("unit") }.roll(rng)
                } else {
                    f.get("fixed_value").and_then(serde_json::Value::as_f64).map(|x| f32_clean(x as f32)).unwrap_or(0.0)
                };
                affixes.push(Affix { attribute: name, kind: rule.affix_type, value, source: AffixSource::Fixed });
            }
        }
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for rule in &data.random_rule {
            // HYPOTHESIS: pool entries with a condition (monster/level rule) apply only when that source drops
            // the item; a plain roll uses the unconditional entries.
            let Some(entry) = rule.pool_random.iter().find(|p| p.condition.is_none()) else { continue };
            if entry.affix_pool_id.is_empty() {
                continue;
            }
            let pool_id = entry.affix_pool_id[rng.range_u32(0, entry.affix_pool_id.len() as u32) as usize];
            let count = self.pick_count(rule.affix_count_pool_id, rng)?;
            let (candidates, source): (Vec<(String, u64)>, AffixSource) = match self.pools.get(&pool_id) {
                Some(p) => (p.clone(), AffixSource::Pool(pool_id)),
                None if crate::overrides::MINOR_POOL_FALLBACK && rule.affix_type == EnBattleAffixType::Minor => (
                    self.template_attributes(item_id, rule.affix_type).into_iter().map(|a| (a, 1)).collect(),
                    AffixSource::MinorFallback,
                ),
                None => bail!("item {item_id}: affix pool {pool_id} missing"),
            };
            let mut cands: Vec<(String, u64)> = candidates.into_iter().filter(|(a, _)| !taken.contains(a)).collect();
            for _ in 0..count {
                let Some(i) = weighted(cands.iter().enumerate().map(|(i, (_, w))| (i, *w)), rng) else { break };
                let (attr, _) = cands.remove(i);
                let range = self
                    .range(item_id, &attr, rule.affix_type)
                    .with_context(|| format!("item {item_id}: no {:?} value range for `{attr}`", rule.affix_type))?;
                taken.insert(attr.clone());
                affixes.push(Affix { attribute: attr, kind: rule.affix_type, value: range.roll(rng), source });
            }
        }
        Ok(Item { item_id, rarity, affixes })
    }

    /// Check an item against the tables: every random affix has a range for this item and rarity and its
    /// value lies inside it; no attribute repeats among random affixes.
    pub fn check(&self, item: &Item) -> anyhow::Result<()> {
        let mut seen = BTreeSet::new();
        for a in item.affixes.iter().filter(|a| a.source != AffixSource::Fixed) {
            let r = self
                .range(item.item_id, &a.attribute, a.kind)
                .with_context(|| format!("item {}: illegal {:?} affix `{}`", item.item_id, a.kind, a.attribute))?;
            if !r.contains(a.value) {
                bail!("item {}: `{}` = {} outside {:?}", item.item_id, a.attribute, a.value, r);
            }
            if !seen.insert(&a.attribute) {
                bail!("item {}: `{}` rolled twice", item.item_id, a.attribute);
            }
            if let AffixSource::Pool(p) = a.source
                && !self.pools.get(&p).is_some_and(|c| c.iter().any(|(n, _)| *n == a.attribute))
            {
                bail!("item {}: `{}` is not in pool {p}", item.item_id, a.attribute);
            }
        }
        Ok(())
    }

    /// Item ids per rarity.
    pub fn by_rarity(&self) -> BTreeMap<i32, Vec<i32>> {
        let mut m: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        for id in self.item_ids() {
            if let Some(r) = self.rarity(id) {
                m.entry(r.number()).or_default().push(id);
            }
        }
        m
    }
}

/// Weighted pick; `None` when the total weight is 0.
pub fn weighted<T>(opts: impl Iterator<Item = (T, u64)> + Clone, rng: &mut SimRng) -> Option<T> {
    let total: u64 = opts.clone().map(|(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut r = rng.next_u64() % total;
    for (v, w) in opts {
        if r < w {
            return Some(v);
        }
        r -= w;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_grid_and_bounds() {
        let r = Range { min: 0.02, max: 0.05, unit: 0.01 };
        assert!(r.contains(0.03));
        assert!(!r.contains(0.035));
        assert!(!r.contains(0.06));
        let mut rng = SimRng::new(9);
        for _ in 0..1000 {
            assert!(r.contains(r.roll(&mut rng)));
        }
    }

    #[test]
    fn weighted_respects_weights() {
        let mut rng = SimRng::new(4);
        let mut n = [0u32; 3];
        for _ in 0..30_000 {
            n[weighted([(0usize, 1u64), (1, 2), (2, 0)].into_iter(), &mut rng).unwrap()] += 1;
        }
        assert_eq!(n[2], 0);
        let ratio = f64::from(n[1]) / f64::from(n[0]);
        assert!((ratio - 2.0).abs() < 0.1, "{ratio}");
    }
}

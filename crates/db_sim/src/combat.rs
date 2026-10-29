//! Combat maths.
//!
//! [`tooltip`] reproduces the game's tooltip Lua (`Script/Text/*.lua`) exactly, in f64 like Lua.
//! [`resolve_hit`] is what the sim applies to a real target: the same structure with the tooltip's
//! reference target (defence level 6, resistance 0.3) replaced by the defender's values, and the
//! crit expectation replaced by a seeded roll. That substitution is HYPOTHESIS.

use crate::overrides;
use crate::rng::SimRng;

/// Damage elements: the six of `DBattleAttributeOffset` (boost/resistance name, penetration name):
/// physical (`none`, `physical`), fire, ice (`ice`, `cold`), thunder (`thunder`, `lightning`),
/// light (`light`, `divine`), dark (`dark`, `shadow`). CONFIRMED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Physical,
    Fire,
    Ice,
    Thunder,
    Light,
    Dark,
}

impl Element {
    pub const ALL: [Element; 6] = [Element::Physical, Element::Fire, Element::Ice, Element::Thunder, Element::Light, Element::Dark];

    fn idx(self) -> usize {
        self as usize
    }
}

/// Per-element values (boost rates, penetration, resistance).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PerElement(pub [f64; 6]);

impl PerElement {
    pub fn get(&self, e: Element) -> f64 {
        self.0[e.idx()]
    }

    pub fn set(&mut self, e: Element, v: f64) {
        self.0[e.idx()] = v;
    }

    pub fn splat(v: f64) -> Self {
        Self([v; 6])
    }
}

/// The attacker's attributes, named after the `affix_attack_*` / `affix_recover_*` / `affix_first_level_*`
/// parameters the tooltip Lua reads (`DBattleAttributeOffset`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AttackStats {
    pub right_weapon_attack: f64,
    pub attack_bonus: f64,
    pub attack_boost_rate: f64,
    pub def_ignore_base: f64,
    pub att_gear_level: f64,
    pub extra_att_gear_level: f64,
    pub critical_rate: f64,
    pub crit_damage_rate: f64,
    pub element_boost_rate: PerElement,
    pub skill_damage: f64,
    pub penetration: PerElement,
    pub strength: f64,
    pub intelligence: f64,
    pub will: f64,
    pub heal_boost_rate: f64,
    pub shield_boost_rate: f64,
}

/// What the defender contributes to a hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefenceStats {
    pub chest_def_gear_level: f64,
    pub helmet_def_gear_level: f64,
    pub resistance: PerElement,
}

impl DefenceStats {
    /// The tooltip's implied reference target: defence level 6, resistance 0.3 for every element.
    pub const REFERENCE: DefenceStats = DefenceStats {
        chest_def_gear_level: 6.0,
        helmet_def_gear_level: 6.0,
        resistance: PerElement([0.3; 6]),
    };
}

/// Where a hit landed (decides which defence gear level applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    Body,
    Head,
}

/// Which attack term a hit uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttackTerm {
    /// `right_weapon_attack + attack_bonus` (weapon hits and most skills).
    Weapon,
    /// Death Knight Q: `38 + 0.31 * (strength + intelligence + will)`.
    DeathKnightQ,
    /// `CareerSkillDamageNumberTextFireBase`: no attack term (1, the caller multiplies the
    /// GameplayEffect's base damage), no attack boost, no crit (Wizard's Flame Shock).
    FireBase,
    /// `CareerSkillDamageNumberTextIceBase`: no attack term, with attack boost and crit.
    IceBase,
}

/// The outcome of one resolved hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub damage: f64,
    pub crit: bool,
}

fn attack_term(a: &AttackStats, term: AttackTerm) -> f64 {
    match term {
        AttackTerm::Weapon => a.right_weapon_attack + a.attack_bonus,
        AttackTerm::DeathKnightQ => 38.0 + 0.31 * (a.strength + a.intelligence + a.will),
        AttackTerm::FireBase | AttackTerm::IceBase => 1.0,
    }
}

/// `20 / (D − A + 40)` with `D` = defender gear level minus defence ignore and `A` = attacker gear levels.
pub fn defence_term(defender_level: f64, a: &AttackStats) -> f64 {
    let d = (defender_level - a.def_ignore_base).max(overrides::DEFENCE_LEVEL_FLOOR);
    20.0 / (d - a.att_gear_level - a.extra_att_gear_level + 40.0).max(overrides::DEFENCE_DENOMINATOR_MIN)
}

/// `1 − resistance × (1 − penetration)`.
pub fn resistance_term(resistance: f64, penetration: f64) -> f64 {
    1.0 - resistance * (1.0 - penetration)
}

/// Resolve one damaging hit against a real defender. Crit is rolled with the sim's RNG.
/// HYPOTHESIS: the server used the tooltip structure with the defender's values.
pub fn resolve_hit(
    a: &AttackStats,
    d: &DefenceStats,
    element: Element,
    term: AttackTerm,
    zone: HitZone,
    rng: &mut SimRng,
) -> Hit {
    let level = if zone == HitZone::Head && overrides::HEAD_HITS_USE_HELMET_LEVEL {
        d.helmet_def_gear_level
    } else {
        d.chest_def_gear_level
    };
    let crit = rng.chance(a.critical_rate);
    // Death Knight Q's and FireBase's tooltips have no crit factor; keep that in play too.
    let crits = matches!(term, AttackTerm::Weapon | AttackTerm::IceBase);
    let crit_mult = if crit && crits { a.crit_damage_rate } else { 1.0 };
    let boost = if term == AttackTerm::FireBase { 1.0 } else { 1.0 + a.attack_boost_rate };
    let damage = attack_term(a, term)
        * boost
        * defence_term(level, a)
        * crit_mult
        * (1.0 + a.element_boost_rate.get(element) + a.skill_damage)
        * resistance_term(d.resistance.get(element), a.penetration.get(element));
    Hit { damage, crit: crit && crits }
}

/// The tooltip formulas, exactly as the client's Lua computes them (CONFIRMED, T0.7).
pub mod tooltip {
    use super::{AttackStats, Element};

    fn defence(a: &AttackStats) -> f64 {
        // Lua: 20 / (math.max(6 - def_ignore_base) - att_gear_level - extra_att_gear_level + 40);
        // math.max with one argument returns it unchanged.
        20.0 / ((6.0 - a.def_ignore_base) - a.att_gear_level - a.extra_att_gear_level + 40.0)
    }

    fn crit_expectation(a: &AttackStats) -> f64 {
        1.0 + a.critical_rate * (a.crit_damage_rate - 1.0)
    }

    /// The penetration attribute each tooltip script reads. The `Light` script pairs the light boost
    /// with `lightning_penetration` (CONFIRMED in the Lua text); kept as written.
    fn tooltip_penetration(a: &AttackStats, e: Element) -> f64 {
        match e {
            Element::Light => a.penetration.get(Element::Thunder),
            _ => a.penetration.get(e),
        }
    }

    fn element(a: &AttackStats, e: Element) -> f64 {
        (1.0 + a.element_boost_rate.get(e) + a.skill_damage) * (1.0 - 0.3 * (1.0 - tooltip_penetration(a, e)))
    }

    /// `CareerSkillDamageNumberText{Phy,Fire,Ice,Light,Dark}` (no thunder script exists; `Thunder`
    /// uses the same structure with its own boost and penetration).
    pub fn damage(a: &AttackStats, e: Element) -> f64 {
        (a.right_weapon_attack + a.attack_bonus) * (1.0 + a.attack_boost_rate) * defence(a) * crit_expectation(a) * element(a, e)
    }

    /// `CareerSkillDamageNumberTextFireBase`: no attack, no attack boost, no crit.
    pub fn damage_fire_base(a: &AttackStats) -> f64 {
        defence(a) * element(a, Element::Fire)
    }

    /// `CareerSkillDamageNumberTextIceBase`: no attack term, but attack boost and crit.
    pub fn damage_ice_base(a: &AttackStats) -> f64 {
        (1.0 + a.attack_boost_rate) * defence(a) * crit_expectation(a) * element(a, Element::Ice)
    }

    /// `CareerSkillDamageNumberDeathKnightQOnly`: stat-based attack term, dark, no crit.
    pub fn damage_death_knight_q(a: &AttackStats) -> f64 {
        (38.0 + 0.31 * (a.strength + a.intelligence + a.will)) * (1.0 + a.attack_boost_rate) * defence(a) * element(a, Element::Dark)
    }

    /// `HealBaseNumberText`: multiplier on the GameplayEffect's base heal.
    pub fn heal_base(a: &AttackStats) -> f64 {
        (1.0 + a.heal_boost_rate) * crit_expectation(a)
    }

    /// `HealBaseNumberPriestHealOnlyText` and `HealBaseNumberDruidPantherHealOnlyText` (identical).
    pub fn heal_weapon(a: &AttackStats) -> f64 {
        (a.right_weapon_attack + a.attack_bonus) * (1.0 + a.attack_boost_rate) * crit_expectation(a) * (1.0 + a.heal_boost_rate)
    }

    /// `HealBaseNumberLichHealOnlyText`: like [`heal_weapon`] without crit.
    pub fn heal_lich(a: &AttackStats) -> f64 {
        (a.right_weapon_attack + a.attack_bonus) * (1.0 + a.attack_boost_rate) * (1.0 + a.heal_boost_rate)
    }

    /// `HealBaseNumberPriestHealOnlyText_Single` (`heal_base` 40, `heal_add` 44) and `_SingleQ1` (86, 94);
    /// both use an attack-boost factor of 0.6.
    pub fn heal_priest_single(a: &AttackStats, heal_base: f64, heal_add: f64) -> f64 {
        (heal_base * (1.0 + 0.6 * a.attack_boost_rate) * (1.0 + a.heal_boost_rate) + heal_add) * crit_expectation(a)
    }

    /// `HealMaxHPRateNumberText`: percent of max HP.
    pub fn heal_max_hp_percent(a: &AttackStats) -> f64 {
        100.0 * (1.0 + a.heal_boost_rate)
    }

    /// `AddShieldBaseNumberText`: multiplier on the GameplayEffect's base shield.
    pub fn shield_base(a: &AttackStats) -> f64 {
        1.0 + a.shield_boost_rate
    }

    /// `AddShieldDPHNumberText`.
    pub fn shield_weapon(a: &AttackStats) -> f64 {
        (a.right_weapon_attack + a.attack_bonus) * (1.0 + a.shield_boost_rate) * (1.0 + a.attack_boost_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sample attacker with every term non-trivial.
    fn sample() -> AttackStats {
        let mut a = AttackStats {
            right_weapon_attack: 50.0,
            attack_bonus: 10.0,
            attack_boost_rate: 0.2,
            def_ignore_base: 1.0,
            att_gear_level: 3.0,
            extra_att_gear_level: 1.0,
            critical_rate: 0.25,
            crit_damage_rate: 1.5,
            skill_damage: 0.1,
            strength: 20.0,
            intelligence: 15.0,
            will: 10.0,
            heal_boost_rate: 0.3,
            shield_boost_rate: 0.4,
            ..Default::default()
        };
        a.element_boost_rate.set(Element::Physical, 0.05);
        a.element_boost_rate.set(Element::Fire, 0.15);
        a.element_boost_rate.set(Element::Ice, 0.1);
        a.element_boost_rate.set(Element::Dark, 0.2);
        a.penetration.set(Element::Physical, 0.1);
        a.penetration.set(Element::Fire, 0.2);
        a.penetration.set(Element::Ice, 0.5);
        a.penetration.set(Element::Dark, 0.3);
        a.penetration.set(Element::Thunder, 0.4);
        // Ignored by the tooltip's Light script, which reads lightning (thunder) penetration.
        a.penetration.set(Element::Light, 0.9);
        a
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "{a} != {b}");
    }

    // Expected values below are computed by hand from the Lua text, term by term:
    // defence = 20 / ((6 - 1) - 3 - 1 + 40) = 20 / 41; crit expectation = 1 + 0.25 * 0.5 = 1.125.

    #[test]
    fn tooltip_damage_matches_lua() {
        let a = sample();
        let def = 20.0 / 41.0;
        // Phy: 60 * 1.2 * def * 1.125 * (1 + 0.05 + 0.1) * (1 - 0.3 * 0.9)
        close(tooltip::damage(&a, Element::Physical), 60.0 * 1.2 * def * 1.125 * 1.15 * 0.73);
        // Fire: ... * (1 + 0.15 + 0.1) * (1 - 0.3 * 0.8)
        close(tooltip::damage(&a, Element::Fire), 60.0 * 1.2 * def * 1.125 * 1.25 * 0.76);
        // Ice: ... * 1.2 * (1 - 0.3 * 0.5)
        close(tooltip::damage(&a, Element::Ice), 60.0 * 1.2 * def * 1.125 * 1.2 * 0.85);
        // Light: no light boost (1.1), thunder penetration 0.4: (1 - 0.3 * 0.6)
        close(tooltip::damage(&a, Element::Light), 60.0 * 1.2 * def * 1.125 * 1.1 * 0.82);
        // Thunder (no script): same structure, own penetration 0.4
        close(tooltip::damage(&a, Element::Thunder), 60.0 * 1.2 * def * 1.125 * 1.1 * 0.82);
        // Dark: 1.3 * (1 - 0.3 * 0.7)
        close(tooltip::damage(&a, Element::Dark), 60.0 * 1.2 * def * 1.125 * 1.3 * 0.79);
        // The number for these inputs (rounded), as a regression anchor.
        // 72 * 20/41 * 1.125 * 1.15 * 0.73 = 33.1705…
        assert_eq!(format!("{:.4}", tooltip::damage(&a, Element::Physical)), "33.1705");
    }

    #[test]
    fn tooltip_variants_match_lua() {
        let a = sample();
        let def = 20.0 / 41.0;
        close(tooltip::damage_fire_base(&a), def * 1.25 * 0.76);
        close(tooltip::damage_ice_base(&a), 1.2 * def * 1.125 * 1.2 * 0.85);
        // DK Q: (38 + 0.31 * 45) * 1.2 * def * 1.3 * 0.79
        close(tooltip::damage_death_knight_q(&a), (38.0 + 0.31 * 45.0) * 1.2 * def * 1.3 * 0.79);
        close(tooltip::heal_base(&a), 1.3 * 1.125);
        close(tooltip::heal_weapon(&a), 60.0 * 1.2 * 1.125 * 1.3);
        close(tooltip::heal_lich(&a), 60.0 * 1.2 * 1.3);
        close(tooltip::heal_priest_single(&a, 40.0, 44.0), (40.0 * (1.0 + 0.6 * 0.2) * 1.3 + 44.0) * 1.125);
        close(tooltip::heal_priest_single(&a, 86.0, 94.0), (86.0 * (1.0 + 0.6 * 0.2) * 1.3 + 94.0) * 1.125);
        close(tooltip::heal_max_hp_percent(&a), 130.0);
        close(tooltip::shield_base(&a), 1.4);
        close(tooltip::shield_weapon(&a), 60.0 * 1.4 * 1.2);
    }

    #[test]
    fn base_character_against_reference_target() {
        // Bare attacker (gear level 0): defence term 20 / (6 + 40), resistance term 0.7.
        let a = AttackStats { right_weapon_attack: 100.0, crit_damage_rate: 1.2, ..Default::default() };
        close(tooltip::damage(&a, Element::Physical), 100.0 * (20.0 / 46.0) * 0.7);
        // Attacker gear level 6 against the reference level 6: 20 / 40 = 0.5 (example).
        let a6 = AttackStats { att_gear_level: 6.0, ..a };
        close(tooltip::damage(&a6, Element::Physical), 100.0 * 0.5 * 0.7);
    }

    #[test]
    fn resolved_hit_equals_tooltip_on_reference_target_without_crit_chance() {
        let mut a = sample();
        a.critical_rate = 0.0;
        let mut rng = SimRng::new(1);
        for e in [Element::Physical, Element::Fire, Element::Ice, Element::Thunder, Element::Dark] {
            let hit = resolve_hit(&a, &DefenceStats::REFERENCE, e, AttackTerm::Weapon, HitZone::Body, &mut rng);
            close(hit.damage, tooltip::damage(&a, e));
            assert!(!hit.crit);
        }
        let dk = resolve_hit(&a, &DefenceStats::REFERENCE, Element::Dark, AttackTerm::DeathKnightQ, HitZone::Head, &mut rng);
        close(dk.damage, tooltip::damage_death_knight_q(&a));
    }

    #[test]
    fn crit_expectation_over_100k_rolls() {
        let a = sample();
        let mut rng = SimRng::new(2026);
        let n = 100_000;
        let mut sum = 0.0;
        for _ in 0..n {
            sum += resolve_hit(&a, &DefenceStats::REFERENCE, Element::Fire, AttackTerm::Weapon, HitZone::Body, &mut rng).damage;
        }
        let mean = sum / f64::from(n);
        let expected = tooltip::damage(&a, Element::Fire);
        let err = (mean - expected).abs() / expected;
        assert!(err < 0.005, "mean {mean} vs tooltip {expected}: {:.3}% off", err * 100.0);
    }

    #[test]
    fn defender_values_change_the_hit() {
        let a = AttackStats { right_weapon_attack: 100.0, crit_damage_rate: 1.2, ..Default::default() };
        let weak = DefenceStats { chest_def_gear_level: 0.0, helmet_def_gear_level: 5.0, resistance: PerElement::splat(0.3) };
        let mut rng = SimRng::new(3);
        let body = resolve_hit(&a, &weak, Element::Physical, AttackTerm::Weapon, HitZone::Body, &mut rng).damage;
        let head = resolve_hit(&a, &weak, Element::Physical, AttackTerm::Weapon, HitZone::Head, &mut rng).damage;
        close(body, 100.0 * (20.0 / 40.0) * 0.7);
        close(head, 100.0 * (20.0 / 45.0) * 0.7);
        // Defence ignore can't push defence below the floor.
        let a2 = AttackStats { def_ignore_base: 10.0, ..a };
        close(defence_term(0.0, &a2), 20.0 / 40.0);
    }
}

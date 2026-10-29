//! The Wendigo's own tree (CONFIRMED bytecode of `AIC_Wendigo` and his tree's nodes).
//!
//! `BT_Wendigo`'s combat branch first summons when `BTD_Wendigo_CheckTotemAbility` passes (`BTT_PickSummon`: one of
//! its abilities at random), else `BTT_Wendigo_PickMode` rolls `Mode` 0 or 1: 0 fights as the default monsters do
//! (`BT_Ability` on his `AbilitySelector`), 1 rolls `AbilitySequence` 0..2 (`BTT_Wendigo_PickAbilitySequence`) and plays
//! that fixed chain of abilities. The totem check: his controller's `TotemDelta` starts at `TotemTime` on entering
//! combat and counts up while in it; once it reaches `TotemTime` the check resets it and passes with fewer than 5
//! summons alive under 40 % health, fewer than 3 under 70 %, else fewer than 2.

/// His controller's values (`AIC_Wendigo`'s defaults).
#[derive(Debug, Clone, PartialEq)]
pub struct WendigoSpec {
    /// `TotemTime`, seconds.
    pub totem_time: f32,
}

impl WendigoSpec {
    pub fn load(dir: &str) -> Option<WendigoSpec> {
        let v = crate::monsters::asset_json(&format!("/Game/TGF/Character/Monster/{dir}/AI/AIC_Wendigo.0"))?;
        let p = &v.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))?["Properties"];
        Some(WendigoSpec { totem_time: p["TotemTime"].as_f64()? as f32 })
    }
}

/// His controller's state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WendigoState {
    /// `TotemDelta`, seconds.
    pub totem_delta: f32,
    /// In combat last tick (`OnEnteredCombat` fills the clock).
    pub in_combat: bool,
}

impl WendigoState {
    /// `AIC_Wendigo`'s `OnEnteredCombat` and `ReceiveTick`.
    pub fn step(&mut self, spec: &WendigoSpec, in_combat: bool, dt: f32) {
        if in_combat && !self.in_combat {
            self.totem_delta = spec.totem_time;
        }
        self.in_combat = in_combat;
        if in_combat {
            self.totem_delta += dt;
        }
    }

    /// `BTD_Wendigo_CheckTotemAbility.PerformConditionCheckAI`: `health` his share of max health.
    pub fn check_totem(&mut self, spec: &WendigoSpec, health: f64, summons: u32) -> bool {
        if self.totem_delta < spec.totem_time {
            return false;
        }
        self.totem_delta = 0.0;
        if health < 0.4 {
            summons < 5
        } else if health < 0.7 {
            summons < 3
        } else {
            summons < 2
        }
    }
}

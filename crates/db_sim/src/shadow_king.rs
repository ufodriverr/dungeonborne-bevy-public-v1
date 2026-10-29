//! The Shadow King's controller. CONFIRMED from `AIC_Shadow_King`'s bytecode and defaults: five countdowns
//! (`Phase2Timer`, `Attack5Timer`, `Attack7Timer`, `Attack9Timer`, `MoveTimer`) his tree's nodes set and test, and
//! the health ratios his phases turn at. The nodes themselves live in [`crate::ai`]; here the values and the
//! per-monster state.

use crate::DT;

/// `AIC_Shadow_King`'s defaults (seconds, health ratios).
#[derive(Debug, Clone, PartialEq)]
pub struct KingSpec {
    /// `Attack5CDP1`..: the summon's countdown in each phase.
    pub attack5: [f32; 4],
    /// `Attack7CDP2`, `Attack7CDP4`.
    pub attack7: [f32; 2],
    /// `Attack9CDP3`, `Attack9CDP4`.
    pub attack9: [f32; 2],
    pub phase2: f32,
    /// `Phase1HP%` (to phase 2), `Phase3HP%` (to phase 4).
    pub phase1_hp: f64,
    pub phase3_hp: f64,
    pub move_cd: f32,
}

impl KingSpec {
    /// From `<dir>/AI/AIC_Shadow_King`'s defaults (None: another monster, or not decoded).
    pub fn load(dir: &str) -> Option<KingSpec> {
        let v = crate::monsters::asset_json(&format!("/Game/TGF/Character/Monster/{dir}/AI/AIC_Shadow_King.0"))?;
        let p = &v.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))?["Properties"];
        let f = |k: &str| p[k].as_f64();
        Some(KingSpec {
            attack5: [f("Attack5CDP1")? as f32, f("Attack5CDP2")? as f32, f("Attack5CDP3")? as f32, f("Attack5CDP4")? as f32],
            attack7: [f("Attack7CDP2")? as f32, f("Attack7CDP4")? as f32],
            attack9: [f("Attack9CDP3")? as f32, f("Attack9CDP4")? as f32],
            phase2: f("Phase2CD")? as f32,
            phase1_hp: f("Phase1HP%")?,
            phase3_hp: f("Phase3HP%")?,
            move_cd: f("MoveCD")? as f32,
        })
    }
}

/// His countdowns, as the tick each runs out (0: run out, as they start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KingState {
    pub phase2: u32,
    pub attack5: u32,
    pub attack7: u32,
    pub attack9: u32,
    pub moved: u32,
}

/// `s` seconds from `now`, in ticks.
pub(crate) fn after(now: u32, s: f32) -> u32 {
    now + (s / DT).round().max(0.0) as u32
}

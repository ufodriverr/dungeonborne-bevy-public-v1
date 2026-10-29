//! The class skills' first-person montages (playtest: the arms played no skill animation at all). Each entry is a
//! montage of `Character/Shared/Animation/1st/` (loose dumps: its segments' clip, start, end and play rate,
//! CONFIRMED) and when it plays. Which montage a skill plays: from the skill effects' chains (`skill_vfx.json`:
//! ability -> montage) where they name one, else by the montage's name (HYPOTHESIS, marked `~`).

use db_sim::SkillKind as K;

/// A clip's path under the first-person animation folder.
macro_rules! clip {
    ($p:literal) => {
        concat!("animations/Dungeonborne/Content/TGF/Character/Shared/Animation/1st/", $p)
    };
}

/// When an entry plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum When {
    /// Once, from the skill's start.
    Start,
    /// Once, from its release at this stage (any stage when `None`).
    Cast(Option<u8>),
    /// While charging: at the held time, looping from `hold_from` past the end.
    Charge,
    /// A charge let go without a cast.
    Cancel,
    /// Looping while the Lich's storm channels.
    Channel,
    /// Looping while the Lich aims it (Q held, `ground_aim`).
    Aim,
}

/// A montage: its clip and segments (clip from, clip to, play rate).
pub(crate) struct SkillClip {
    pub(crate) skill: K,
    pub(crate) when: When,
    pub(crate) clip: &'static str,
    pub(crate) segments: &'static [(f32, f32, f32)],
    /// For `Charge`: where the held loop starts in the clip.
    pub(crate) hold_from: f32,
}

const fn one(skill: K, when: When, clip: &'static str, segments: &'static [(f32, f32, f32)]) -> SkillClip {
    SkillClip { skill, when, clip, segments, hold_from: 0.0 }
}

const WIZ_Q: &str = clip!("Classes_Wizard/A_Classes_Wizard_SkillQ_Stage3_1st.glb");
const PRIEST_CHARGE: &str = clip!("priest/A_Classes_Priest_Skill2_ChargeBegin_1st.glb");
const PRIEST_CAST: &str = clip!("priest/A_Classes_Priest_Skill2_Cast_1st.glb");
const PRIEST_FAILED: &str = clip!("priest/A_Classes_Priest_Skill2_Failed_1st.glb");
const CAST_SELF: &str = clip!("priest/A_Classes_Priest_V2_Skill2_Cast_Self_1st.glb");
const FIGHTER_CAST: &str = clip!("Fighter/A_Classes_Fighter_Skill1_Cast_1st.glb");
/// `AM_Classes_Fighter_Skill2_1st_Iteration`.
const FIGHTER_CAST_SEGS: &[(f32, f32, f32)] = &[(0.0, 0.333, 1.25), (0.333, 0.667, 1.428_571), (0.667, 1.0, 1.0)];
const ROGUE_SKILL2: &str = clip!("Rogue/A_Classes_Rogue_Skill2_Cast_1st.glb");
const ROGUE_SKILL3: &str = clip!("Rogue/A_Classes_Rogue_Skill3_Cast_1st.glb");

pub(crate) const SKILL_CLIPS: &[SkillClip] = &[
    // Pyroblast: `AM_Classes_Wizard_SkillQ_Stage1/2_Charge` + `Stage2_Complete` (held), `Q1_Cast`, `Q2_Cast`,
    // `Cancle1`.
    SkillClip { skill: K::Pyroblast, when: When::Charge, clip: WIZ_Q, segments: &[(0.0, 2.333_333, 1.0)], hold_from: 1.0 },
    one(K::Pyroblast, When::Cast(Some(1)), WIZ_Q, &[(2.333_333, 3.833_333, 1.0)]),
    one(K::Pyroblast, When::Cast(Some(2)), clip!("Classes_Wizard/A_Classes_Wizard_SkillQ_Stage4_1st.glb"), &[(0.0, 1.933_333, 1.0)]),
    one(K::Pyroblast, When::Cancel, clip!("Classes_Wizard/A_Classes_Wizard_SkillQ_Stage3_Cancle_1st.glb"), &[(0.0, 1.133_333, 1.0)]),
    // Flame Shock: `AM_Classes_Wizard_SkillE_1st` (chain).
    one(K::FlameShock, When::Start, clip!("Classes_Wizard/A_Classes_Wizard_SkillE_1st.glb"), &[(0.0, 0.930_556, 1.0)]),
    // Frost Torment: `AM_Classes_Lich_SkillQ_DOT_ChargeBegin` (its held clip), `_Cast`, `_Failed` (chain).
    SkillClip { skill: K::FrostTorment, when: When::Charge, clip: PRIEST_CHARGE, segments: &[(0.0, 1.667, 1.0)], hold_from: 0.0 },
    one(K::FrostTorment, When::Cast(None), PRIEST_CAST, &[(0.0, 1.233, 1.0)]),
    one(K::FrostTorment, When::Cancel, PRIEST_FAILED, &[(0.0, 0.9, 1.0)]),
    // Heal: `AM_Classes_Priest_V2_Skill2_ChargeBegin`, `_Cast` (chain), `_Failed` (~).
    SkillClip { skill: K::HealSingle, when: When::Charge, clip: PRIEST_CHARGE, segments: &[(0.0, 1.667, 1.0)], hold_from: 0.0 },
    one(K::HealSingle, When::Cast(None), PRIEST_CAST, &[(0.0, 1.233, 1.0)]),
    one(K::HealSingle, When::Cancel, PRIEST_FAILED, &[(0.0, 0.9, 1.0)]),
    // Blizzard: `AM_Classes_Lich_SkillQ_Aiming` while placing, `_Guiding` while it channels, `_FullRelease` on the
    // unfold (~, by name).
    one(K::Blizzard, When::Aim, clip!("Classes_Lich/A_Classes_Lich_SkillQ_Aiming_1st.glb"), &[(0.0, 3.1, 1.0)]),
    one(K::Blizzard, When::Channel, clip!("Classes_Lich/A_Classes_Lich_SkillQ_Guiding_1st.glb"), &[(0.0, 5.0, 1.0)]),
    one(K::Blizzard, When::Cast(Some(2)), clip!("Classes_Lich/A_Classes_Lich_SkillQ_FullRelease_1st.glb"), &[(0.0, 2.233, 1.0)]),
    // Self casts: `AM_Classes_Lich_SkillE_IceArmor_Cast_Self` (chain), the Priest's `V2_Skill2_Cast_Self` (chain),
    // `Skill2_Cast_Self` (chain: its clip is `Priest_Skill1_Cast_Self`), the Druid's `SkillE2_Cast_Self` (chain).
    one(K::IceArmor, When::Start, CAST_SELF, &[(0.0, 1.1, 1.0)]),
    one(K::DivineProtection, When::Start, CAST_SELF, &[(0.0, 1.1, 1.0)]),
    one(K::Guardian, When::Start, clip!("priest/A_Classes_Priest_Skill1_Cast_Self_1st.glb"), &[(0.0, 1.1, 1.0)]),
    one(K::Rejuvenation, When::Start, CAST_SELF, &[(0.0, 1.1, 1.0)]),
    // Oracle: `AM_Classes_Priest_Skill1_Cast` (~).
    one(K::Oracle, When::Start, clip!("priest/A_Classes_Priest_Skill1_Cast_1st.glb"), &[(0.0, 0.967, 1.0)]),
    // Soul Chain: `AM_Classes_DeathKnight_Skill2_Cast` (chain).
    one(K::SoulChain, When::Start, clip!("Classes_DeathKnight/A_Classes_DeathKnight_Skill2_Cast_1st.glb"), &[(0.0, 2.133, 1.0)]),
    // Psychic Flying Swords: held, `AM_Classes_SwordMaster_Skill1_ChargeBegin`'s held part (`Skill1_Idle`, 2 s,
    // looping; its 0.63 s raise isn't played), released with `_Release` (chain).
    SkillClip { skill: K::FlyingSwords, when: When::Charge, clip: clip!("SwordMaster/A_Classes_SwordMaster_Skill1_Idle_1st.glb"), segments: &[(0.0, 2.0, 1.0)], hold_from: 0.0 },
    one(K::FlyingSwords, When::Cast(None), clip!("SwordMaster/A_Classes_SwordMaster_Skill1_Release_1st.glb"), &[(0.0, 0.133, 1.0), (0.133, 0.4, 0.333_333), (0.4, 0.767, 1.0)]),
    // The Rogue: `AM_Classes_Rogue_Skill2_Cast` (chain: Fade into Void, Shadow Veil; ~ Swift Concealment),
    // `Skill3_Cast` (~: the thrown skills).
    one(K::FadeIntoVoid, When::Start, ROGUE_SKILL2, &[(0.0, 1.367, 1.0)]),
    one(K::ShadowVeil, When::Start, ROGUE_SKILL2, &[(0.0, 1.367, 1.0)]),
    one(K::SwiftConcealment, When::Start, ROGUE_SKILL2, &[(0.0, 1.367, 1.0)]),
    one(K::FlyingBlades, When::Start, ROGUE_SKILL3, &[(0.0, 0.867, 1.0)]),
    one(K::PetrifyingPoison, When::Start, ROGUE_SKILL3, &[(0.0, 0.867, 1.0)]),
    // The Fighter: Charge's dash `AM_Classes_Fighter_Skill1_1st_Iteration` (~), the shouts' and Rend's short cast
    // `AM_Classes_Fighter_Skill2_1st_Iteration` (~).
    one(K::Charge, When::Start, clip!("Fighter/A_Classes_Fighter_Skill2_1st.glb"), &[(0.0, 0.333, 1.428_571), (0.333, 1.833, 5.625), (1.833, 2.167, 1.0), (2.167, 2.5, 0.125), (2.5, 3.267, 1.0)]),
    one(K::WarCry, When::Start, FIGHTER_CAST, FIGHTER_CAST_SEGS),
    one(K::Inspire, When::Start, FIGHTER_CAST, FIGHTER_CAST_SEGS),
    one(K::Rend, When::Start, FIGHTER_CAST, FIGHTER_CAST_SEGS),
    // The shield 6002's throw: `AM_Sword_Shield_CastShield_1st` (its ability's imports).
    one(K::Item, When::Start, clip!("Sword_Shield/A_Sword_Shield_CastShield_1st.glb"), &[(0.0, 1.15, 1.0)]),
    // Wild Awakening: the human cast before the panther, `A_Classes_DruidElf_PantherSkillQ_Elf_1st` at the ability's
    // montage rate 1.5 (`GA_TGF_DruidPanther_Q_Human`; the montage by its clip's name, HYPOTHESIS), its last pose held
    // over the ability's 0.156 s wait after it, until the panther comes (else the idle staff flashed).
    one(K::WildAwakening, When::Start, clip!("Classes_DruidPanther/A_Classes_DruidElf_PantherSkillQ_Elf_1st.glb"), &[(0.0, 1.99, 1.5), (1.99, 2.0, 0.06)]),
    // Force of Nature: aimed with `AM_Classes_DruidPanther_SkillE_Charge` (its clip's 0-2.67 s, held at its end),
    // planted with `_Release` (2.67-4.17 s), dropped with `_Cancel` (chain).
    SkillClip { skill: K::ForceOfNature, when: When::Charge, clip: clip!("Classes_DruidPanther/A_Classes_DruidPanther_SkillE_1st.glb"), segments: &[(0.0, 2.67, 1.0)], hold_from: 2.66 },
    one(K::ForceOfNature, When::Cast(None), clip!("Classes_DruidPanther/A_Classes_DruidPanther_SkillE_1st.glb"), &[(2.67, 4.167, 1.0)]),
    one(K::ForceOfNature, When::Cancel, clip!("Classes_DruidPanther/A_Classes_DruidPanther_SkillE_Cancel_1st.glb"), &[(0.0, 0.933, 1.0)]),
];

impl SkillClip {
    /// The montage's length (s).
    pub(crate) fn length(&self) -> f32 {
        self.segments.iter().map(|&(a, b, r)| (b - a) / r.max(1e-3)).sum()
    }

    /// The clip time `t` seconds into the montage (the last frame past its end).
    pub(crate) fn clip_time(&self, t: f32) -> f32 {
        let mut left = t.max(0.0);
        for &(a, b, r) in self.segments {
            let len = (b - a) / r.max(1e-3);
            if left <= len {
                return a + left * r;
            }
            left -= len;
        }
        self.segments.last().map_or(0.0, |s| s.1)
    }

    /// For `Charge`: the clip time after `held` seconds, looping the held section past the end.
    pub(crate) fn held_time(&self, held: f32) -> f32 {
        let (a, b, _) = self.segments[0];
        let t = a + held;
        if t < b { t } else { self.hold_from + (t - b) % (b - self.hold_from).max(1e-3) }
    }

    /// For the loops: the clip time `t` seconds in.
    pub(crate) fn looped_time(&self, t: f32) -> f32 {
        self.clip_time(t % self.length().max(1e-3))
    }
}

/// The entry for `skill` playing `when`.
pub(crate) fn find(skill: K, when: impl Fn(When) -> bool) -> Option<&'static SkillClip> {
    SKILL_CLIPS.iter().find(|c| c.skill == skill && when(c.when))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montage_times() {
        let c = find(K::Charge, |w| w == When::Start).unwrap();
        // 0.233 s at 1.43x covers the first third of a second of the clip; then the fast part.
        assert!((c.clip_time(0.233) - 0.333).abs() < 1e-2);
        assert!((c.clip_time(0.233 + 0.1) - (0.333 + 0.5625)).abs() < 1e-2);
        let p = find(K::Pyroblast, |w| w == When::Charge).unwrap();
        assert!((p.held_time(1.0) - 1.0).abs() < 1e-4);
        assert!((p.held_time(2.333_333 + 0.5) - 1.5).abs() < 1e-3, "loops the held section");
    }
}

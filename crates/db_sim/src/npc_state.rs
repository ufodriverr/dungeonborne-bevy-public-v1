//! Monster NPC states (CONFIRMED bytecode of the states and their behaviours).
//!
//! A monster's `TgfNpcStateManagerComponent.InitialState` names an NPC state class whose `Events` map gameplay events
//! (`TriggerTags`) to behaviours, each run when its `DoChecks` passes. The sim knows the boss behaviours below by
//! class; their values sit in their bytecode, not in properties. The Shadow King's floor collapse is
//! [`crate::floor_collapse`]; the talking NPCs' `BP_NpcState_Idle` (a tutorial skeleton) is not built.

use crate::monsters::asset_json;

/// `TGF.GameplayEvent.Damage`: every hit the monster takes.
const DAMAGE: &str = "TGF.GameplayEvent.Damage";
/// `TGF.GameplayEvent.AI.RepickAbility.Begin`: sent by `BTS_RepickAbility` before it picks again.
const REPICK: &str = "TGF.GameplayEvent.AI.RepickAbility.Begin";

/// What a behaviour does to its controller (`AIC_Monster_Default`, CONFIRMED: `SetInterruptAction` and
/// `SetInterruptAbility` set those blackboard keys; `AddPriorAbility` makes the next pick that ability if it passes).
#[derive(Debug, Clone, PartialEq)]
pub enum Behavior {
    /// `SetInterruptAbility(true)` (the Wendigo's `PickDash`): the ability branch gives up, so his combat re-rolls.
    InterruptAbility,
    /// Hit from behind by an enemy (`CompareTeams` different, `IsAtBehind`: HYPOTHESIS, the attacker in the rear half
    /// of its facing): `SetInterruptAction(true)` and `AddPriorAbility(prior, 1)`; the Cyclops' also
    /// `ClearAbilityInterval` (the next ability without its pause).
    BehindHit { prior: &'static str, clear_interval: bool },
}

/// The behaviour classes the sim knows.
const BEHAVIORS: &[(&str, Behavior)] = &[
    ("BP_NpcBehavior_Wendigo_PickDash_C", Behavior::InterruptAbility),
    ("BP_NpcBehavior_Wendigo_Behitbehind_C", Behavior::BehindHit { prior: "GA_TGF_M_Wendigo_Attack1_C", clear_interval: false }),
    ("BP_NpcBehavior_Cyclop_Behitbehind_C", Behavior::BehindHit { prior: "GA_TGF_M_Cyclop_Attack5_C", clear_interval: true }),
];

/// A monster's NPC state: the known behaviours of each event it reacts to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NpcState {
    pub on_damage: Vec<Behavior>,
    pub on_repick: Vec<Behavior>,
}

impl NpcState {
    /// From the state class's `Events` (`object_path`, as `MonsterBp::npc_state`); None without a known behaviour.
    pub fn load(object_path: &str) -> Option<NpcState> {
        let v = asset_json(object_path)?;
        let p = &v.as_array()?.iter().find(|o| o["Name"].as_str().is_some_and(|n| n.starts_with("Default__")))?["Properties"];
        let mut state = NpcState::default();
        for ev in p["Events"].as_array()? {
            let tags: Vec<&str> = ev["TriggerTags"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).collect();
            // `BP_NpcBehavior_Wendigo_PickDash_C'Default__BP_NpcState_Wendigo_C:BP_NpcBehavior_Wendigo_PickDash_C_0'`
            let behaviors = ev["NpcBehaviors"].as_array().into_iter().flatten().filter_map(|b| b["ObjectName"].as_str()?.split('\'').next()).filter_map(|class| BEHAVIORS.iter().find(|(c, _)| *c == class).map(|(_, b)| b.clone()));
            for b in behaviors {
                if tags.contains(&DAMAGE) {
                    state.on_damage.push(b.clone());
                }
                if tags.contains(&REPICK) {
                    state.on_repick.push(b);
                }
            }
        }
        (!state.on_damage.is_empty() || !state.on_repick.is_empty()).then_some(state)
    }
}

/// Runs `b` on its monster's blackboard and next pick (`prior`, its kit's attack index).
pub(crate) fn apply(b: &Behavior, bb: &mut crate::bt::Blackboard, prior: &mut Option<u8>, kit: &crate::monsters::MonsterKit) {
    match b {
        Behavior::InterruptAbility => bb.set("InterruptAbility", crate::bt::BbValue::Bool(true)),
        Behavior::BehindHit { prior: class, clear_interval } => {
            bb.set("InterruptAction", crate::bt::BbValue::Bool(true));
            if *clear_interval {
                bb.clear("AbilityActivateTime");
            }
            if let Some(i) = kit.attacks.iter().position(|a| a.class == *class) {
                *prior = Some(i as u8);
            }
        }
    }
}

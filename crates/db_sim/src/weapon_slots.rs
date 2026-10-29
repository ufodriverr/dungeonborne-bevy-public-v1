//! The second weapon pair and the swap: a character wears two weapon sets and switches between them.
//!
//! CONFIRMED (the loose v6 reads and bytecode of `GA_TGF_SwitchWeaponSlot_Base` and its subclasses,
//! `GE_TGF_SwitchWeapon_Cooldown`, `AbilitySet_Equipment`, the key binding strings): `AbilitySet_Equipment` grants
//! `GA_TGF_SwitchWeaponSlot_WeaponPair1` / `_WeaponPair2` on `InputTag.Equipment.QuickSlot.Slot1` / `Slot2`
//! ("切换武器套装1/2") and `_SwitchNext` / `_SwitchPre` on `QuickSlot.Down` / `Up` ("切换下一个/上一个武器"); a switch needs a
//! right-hand weapon in the target set and a set other than the current one (`Is Valid Slot`), and an actor free to
//! act (`NoActiveAbility` absent) or in a montage's `SwitchSlot` window; it commits (the cooldown,
//! `GE_TGF_SwitchWeapon_Cooldown` 0.2 s), switches at once (`SwitchWeaponSlot`), and the ability lasts
//! `SwitchWeaponDuration` 0.2 s.
//!
//! HYPOTHESIS: the port swaps the two sets' items between the pair-1 and pair-2 slots (the held set is always pair 1)
//! instead of keeping a current-slot index; an idle actor switches, or one in a combo swing's `SwitchSlot` window
//! (which ends the swing; the charges', the staff's and the rebounds' windows aren't applied); a press during the
//! switch is dropped; the consumable and throwable quick slots (3-6) aren't modelled.

use bevy_ecs::prelude::*;
use db_formats::config::generated::EnItemWearingPart as Slot;

use crate::melee::Melee;
use crate::{DT, SimEvent, SimEvents, SimId, TickCount};

/// The weapon set slots: (held, sheathed) per hand.
pub const PAIRS: [(Slot, Slot); 2] = [(Slot::Pair1rightweapon, Slot::Pair2rightweapon), (Slot::Pair1leftweapon, Slot::Pair2leftweapon)];

/// Which set is held and the switch in progress.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct WeaponSlots {
    /// The second set is held (its items sit in the pair-1 slots).
    pub second: bool,
    /// The tick the switch ends (attacks wait until then).
    pub busy_until: u64,
    /// The tick the cooldown ends.
    pub ready_at: u64,
    /// The tick of the last switch (the new set's draw counts from it).
    pub switched_at: Option<u64>,
}

/// Switches to the other weapon set (`SimCommand::SwitchWeaponPair`).
pub(crate) fn switch(world: &mut World, id: SimId, e: Entity) {
    let now = world.resource::<TickCount>().0;
    let reject = |world: &mut World, reason| world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason });
    let Some(inv) = world.get::<crate::Inventory>(e) else { return reject(world, "actor has no inventory") };
    if !inv.worn.contains_key(&Slot::Pair2rightweapon) {
        return reject(world, "no weapon in the other set");
    }
    let st = world.get::<WeaponSlots>(e).copied().unwrap_or_default();
    // A combo swing inside its montage's `SwitchSlot` window may be cut.
    let cut = crate::weapons::in_switch_window(world, e);
    let busy = world.get::<Melee>(e).is_some_and(|m| m.swing.is_some() && !cut)
        || world.get::<crate::crossbow::CrossbowState>(e).is_some_and(|c| c.action.is_some())
        || world.get::<crate::StaffState>(e).is_some_and(|s| s.action.is_some())
        || world.get::<crate::weapons::ComboState>(e).is_some_and(|c| c.charge.is_some());
    if busy || now < st.busy_until {
        return reject(world, "busy");
    }
    if now < st.ready_at {
        return reject(world, "weapon switch cooling down");
    }
    let (duration, cooldown) = world.get_resource::<crate::weapons::WeaponCombos>().map_or((0.0, 0.0), |c| c.switch);
    if cut && let Some(mut m) = world.get_mut::<Melee>(e) {
        m.swing = None;
        m.queued = None;
    }
    if let Some(mut inv) = world.get_mut::<crate::Inventory>(e) {
        for (held, sheathed) in PAIRS {
            let a = inv.worn.remove(&held);
            let b = inv.worn.remove(&sheathed);
            if let Some(b) = b {
                inv.worn.insert(held, b);
            }
            if let Some(a) = a {
                inv.worn.insert(sheathed, a);
            }
        }
    }
    let ticks = |s: f32| u64::from((s / DT).round() as u32);
    world.entity_mut(e).insert(WeaponSlots { second: !st.second, busy_until: now + ticks(duration), ready_at: now + ticks(cooldown), switched_at: Some(now) });
    crate::inventory::refresh_combat(world, e);
    let mut events = world.resource_mut::<SimEvents>();
    events.push(SimEvent::WeaponPairSwitched { id, second: !st.second });
    events.push(SimEvent::InventoryChanged { id });
}

/// While a switch lasts, presses are dropped.
pub fn weapon_slots_tick(world: &mut World) {
    let now = world.resource::<TickCount>().0;
    let busy: Vec<Entity> = world.query::<(Entity, &WeaponSlots)>().iter(world).filter(|(_, s)| now < s.busy_until).map(|(e, _)| e).collect();
    for e in busy {
        if let Some(mut m) = world.get_mut::<Melee>(e)
            && m.queued.is_some()
        {
            m.queued = None;
        }
    }
}

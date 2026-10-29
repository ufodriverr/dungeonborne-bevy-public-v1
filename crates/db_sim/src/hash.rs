//! A fingerprint of the full simulation state, for determinism tests and (later) desync checks.

use bevy_ecs::prelude::*;

use crate::{CharacterState, Health, Melee, MoveSpeed, Position, SimIndex, SimRng, TickCount};

/// FNV-1a 64 over the tick, the RNG position and every actor's components in id order.
/// Floats are hashed by their bit patterns, so any difference at all changes the hash.
pub fn state_hash(world: &World) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    put(&world.resource::<TickCount>().0.to_le_bytes());
    let rng = world.resource::<SimRng>();
    put(&rng.seed().to_le_bytes());
    put(&rng.word_pos().to_le_bytes());
    let index = world.resource::<SimIndex>();
    put(&(index.len() as u64).to_le_bytes());
    put(&world.get_resource::<crate::RunClock>().and_then(|c| c.ticks_left).map_or(u64::MAX, u64::from).to_le_bytes());
    for (id, e) in index.iter() {
        put(&id.0.to_le_bytes());
        let p = world.get::<Position>(e).map_or(bevy_math::Vec3::NAN, |p| p.0);
        let c = world.get::<CharacterState>(e).copied().unwrap_or_default();
        let s = world.get::<MoveSpeed>(e).map_or(f32::NAN, |s| s.0);
        let flags = f32::from(u8::from(c.on_ground) | u8::from(c.crouched) << 1 | u8::from(c.sprinting) << 2 | u8::from(c.exhausted) << 3);
        for f in [p.x, p.y, p.z, c.velocity.x, c.velocity.y, c.velocity.z, c.stamina, flags, s] {
            put(&f.to_bits().to_le_bytes());
        }
        let hp = world.get::<Health>(e).map_or(f64::NAN, |h| h.current);
        put(&hp.to_bits().to_le_bytes());
        let m = world.get::<Melee>(e);
        let source = |s: &crate::melee::Swing| match s.by {
            crate::HitSource::Swing(k) => k as u64,
            crate::HitSource::Skill(k) => 16 + k as u64,
            crate::HitSource::Monster(i) => 256 + u64::from(i),
            crate::HitSource::Bleed(k) => 512 + k as u64,
            crate::HitSource::Trap(k) => 768 + k as u64,
            crate::HitSource::Staff(k) => 1024 + k as u64,
            crate::HitSource::Bolt => 1280,
            crate::HitSource::Zone => 1281,
            crate::HitSource::Burn => 1282,
        };
        let swing = m.and_then(|m| m.swing.as_ref()).map_or(u64::MAX, |s| u64::from(s.tick) | source(s) << 32);
        put(&swing.to_le_bytes());
        put(&[u8::from(m.is_some_and(|m| m.block_held))]);
        if let Some(c) = world.get::<crate::Container>(e) {
            put(&[u8::from(c.opened)]);
        }
        if let Some(g) = world.get::<crate::GroundItem>(e) {
            put(&g.0.uid.to_le_bytes());
        }
        if let Some(fx) = world.get::<crate::Effects>(e) {
            let heal = fx.heal.map_or(u64::MAX, |(_, n, t)| u64::from(n) << 32 | u64::from(t));
            let shield = fx.shield.map_or(f64::NAN, |(a, _)| a);
            put(&heal.to_le_bytes());
            put(&shield.to_bits().to_le_bytes());
            put(&fx.potion_cooldown.to_le_bytes());
            put(&fx.mana.map_or(u64::MAX, |(_, n, t)| u64::from(n) << 32 | u64::from(t)).to_le_bytes());
        }
        if let Some(inv) = world.get::<crate::Inventory>(e) {
            for b in &inv.bag {
                put(&[b.x, b.y]);
                put(&b.item.uid.to_le_bytes());
                put(&b.item.count.to_le_bytes());
            }
            for (slot, item) in &inv.worn {
                put(&slot.number().to_le_bytes());
                put(&item.uid.to_le_bytes());
            }
        }
        if let Some(r) = world.get::<crate::RunStatus>(e) {
            put(&r.floor.to_le_bytes());
            put(&[r.outcome.map_or(0, |o| o as u8 + 1)]);
        }
        if let Some(ai) = world.get::<crate::MonsterAi>(e) {
            for v in [ai.state as u64, u64::from(ai.timer), u64::from(ai.unseen), u64::from(ai.leave_timer), ai.target.map_or(u64::MAX, |t| t.0), u64::from(ai.notice), ai.bt.task().map_or(u64::MAX, u64::from)] {
                put(&v.to_le_bytes());
            }
        }
        if let Some(r) = world.get::<crate::weapons::RelicEnergy>(e) {
            for (uid, v) in &r.energy {
                put(&uid.to_le_bytes());
                put(&v.to_le_bytes());
            }
            put(&r.recover_in.to_le_bytes());
        }
        if let Some(w) = world.get::<crate::weapon_slots::WeaponSlots>(e) {
            put(&[u8::from(w.second)]);
            put(&w.busy_until.to_le_bytes());
            put(&w.ready_at.to_le_bytes());
            put(&w.switched_at.unwrap_or(u64::MAX).to_le_bytes());
        }
        if let Some(c) = world.get::<crate::crossbow::CrossbowState>(e) {
            for (uid, v) in &c.loaded {
                put(&uid.to_le_bytes());
                put(&v.to_le_bytes());
            }
            let action = match c.action {
                None => u64::MAX,
                Some(crate::crossbow::CrossbowAction::Fire { tick, aimed }) => u64::from(tick) | u64::from(aimed) << 32,
                Some(crate::crossbow::CrossbowAction::Reload { tick }) => 1 << 60 | u64::from(tick),
            };
            put(&action.to_le_bytes());
            put(&[u8::from(c.aiming)]);
            put(&c.ready_at.to_le_bytes());
        }
        if let Some(s) = world.get::<crate::StaffState>(e) {
            for (uid, v) in &s.energy {
                put(&uid.to_le_bytes());
                put(&v.to_le_bytes());
            }
            let action = match &s.action {
                None => u64::MAX,
                Some(crate::StaffAction::Beam { tick, triggers, cancel, .. }) => u64::from(*tick) | u64::from(*triggers) << 16 | u64::from(cancel.unwrap_or(u32::MAX) & 0xFFFF) << 32,
                Some(crate::StaffAction::ThunderAim { tick, .. }) => 1 << 60 | u64::from(*tick),
                Some(crate::StaffAction::ThunderRelease { tick, .. }) => 2 << 60 | u64::from(*tick),
                Some(crate::StaffAction::ThunderFail { tick }) => 3 << 60 | u64::from(*tick),
                Some(crate::StaffAction::Recharge { tick, .. }) => 4 << 60 | u64::from(*tick),
            };
            put(&action.to_le_bytes());
            put(&s.recover_in.to_le_bytes());
        }
        if let Some(k) = world.get::<crate::SkillKit>(e) {
            let charge = k.charge.map_or(u64::MAX, |(t, _)| u64::from(t));
            for v in [u64::from(k.whirlwind_cooldown), k.whirlwind.map_or(u64::MAX, u64::from), u64::from(k.charges), u64::from(k.charge_recharge), charge] {
                put(&v.to_le_bytes());
            }
            for v in [k.slots[0] as u64, k.slots[1] as u64, u64::from(k.war_cry_cooldown), u64::from(k.inspire_charges), u64::from(k.inspire_recharge), u64::from(k.rend_cooldown), k.rend_armed.map_or(u64::MAX, u64::from)] {
                put(&v.to_le_bytes());
            }
        }
        if let Some(k) = world.get::<crate::ClassKit>(e) {
            for v in [k.slots[0] as u64, k.slots[1] as u64, k.preparing.map_or(u64::MAX, |(s, t)| (s as u64) << 32 | u64::from(t)), k.waltz.map_or(u64::MAX, |(l, a)| u64::from(l) << 32 | u64::from(a)), k.poison.map_or(u64::MAX, u64::from)] {
                put(&v.to_le_bytes());
            }
            for (s, c) in &k.cooldowns {
                put(&[*s as u8]);
                put(&c.to_le_bytes());
            }
            put(&k.charge.map_or(u64::MAX, |(s, t)| (s as u64) << 32 | u64::from(t)).to_le_bytes());
            if let Some(b) = k.blizzard {
                for f in [b.pos.x, b.pos.y, b.pos.z] {
                    put(&f.to_bits().to_le_bytes());
                }
                for v in [b.age, b.channel, b.unfolded] {
                    put(&v.to_le_bytes());
                }
            }
            for (t, n) in &k.spreads {
                put(&t.0.to_le_bytes());
                put(&n.to_le_bytes());
            }
            if let Some((at, t)) = k.oracle {
                for f in [at.x, at.y, at.z] {
                    put(&f.to_bits().to_le_bytes());
                }
                put(&t.to_le_bytes());
            }
            if let Some((s, age, left)) = k.aura {
                put(&[s as u8]);
                put(&age.to_le_bytes());
                put(&left.to_le_bytes());
            }
            for (s, (n, next)) in &k.charges {
                put(&[*s as u8, *n]);
                put(&next.to_le_bytes());
            }
            put(&[u8::from(k.panther)]);
            put(&k.pounce.unwrap_or(u32::MAX).to_le_bytes());
        }
        if let Some(t) = world.get::<crate::class_skills::Treant>(e) {
            put(&t.owner.0.to_le_bytes());
            for v in [t.left, t.next, t.swing.map_or(u32::MAX, |s| s.1)] {
                put(&v.to_le_bytes());
            }
            put(&t.swing.map_or(u64::MAX, |s| s.0.0).to_le_bytes());
            put(&t.marked.map_or(u64::MAX, |m| m.0).to_le_bytes());
        }
        if let Some(d) = world.get::<crate::swing_doors::SwingDoor>(e) {
            put(&[u8::from(d.open)]);
            put(&d.t.to_bits().to_le_bytes());
            put(&d.dir.to_bits().to_le_bytes());
            put(&d.ready_at.to_le_bytes());
        }
        if let Some(m) = world.get::<crate::mechanisms::Mechanism>(e) {
            put(&[u8::from(m.on)]);
            put(&m.t.to_bits().to_le_bytes());
        }
        if let Some(l) = world.get::<crate::mechanisms::Lever>(e) {
            put(&[u8::from(l.on)]);
            put(&l.ready_at.to_le_bytes());
            put(&l.fire_at.unwrap_or(u64::MAX).to_le_bytes());
        }
        if let Some(i) = world.get::<crate::swing_doors::Interacting>(e) {
            put(&i.target.0.to_le_bytes());
            put(&i.done_at.to_le_bytes());
        }
        if let Some(d) = world.get::<crate::Door>(e) {
            put(&[u8::from(d.open), u8::from(world.entity(e).contains::<avian3d::prelude::ColliderDisabled>())]);
        }
        if let Some(t) = world.get::<crate::Trap>(e) {
            put(&t.tick.to_le_bytes());
            for h in &t.hit {
                put(&h.0.to_le_bytes());
            }
        }
        if let Some(a) = world.get::<crate::Altar>(e) {
            put(&[a.kind as u8, u8::from(a.used)]);
        }
        if let Some(pw) = world.get::<crate::class_skills::Power>(e) {
            put(&[pw.kind as u8]);
            put(&pw.current.to_bits().to_le_bytes());
        }
        if let Some(pr) = world.get::<crate::Projectile>(e) {
            for f in [pr.dir.x, pr.dir.y, pr.dir.z, pr.fall] {
                put(&f.to_bits().to_le_bytes());
            }
            put(&pr.age.to_le_bytes());
            put(&[pr.pierced, pr.chained, pr.hit.len() as u8]);
            put(&pr.target.map_or(u64::MAX, |t| t.0).to_le_bytes());
        }
        if let Some(b) = world.get::<crate::Buffs>(e) {
            for buff in &b.0 {
                put(&[buff.skill as u8]);
                put(&buff.ticks_left.to_le_bytes());
                put(&buff.age.to_le_bytes());
                let (tag, value) = buff.kind.tag();
                put(&[tag]);
                put(&value.to_bits().to_le_bytes());
                if let crate::BuffKind::Rent { per_metre, last, .. } = buff.kind {
                    put(&per_metre.to_le_bytes());
                    put(&last.x.to_le_bytes());
                    put(&last.y.to_le_bytes());
                }
            }
        }
    }
    h
}

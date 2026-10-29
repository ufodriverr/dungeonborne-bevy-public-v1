//! Stable simulation ids. `Entity` values depend on allocation history and are never used as identity;
//! every simulated actor has a [`SimId`] that is assigned in command order and never reused.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// Stable id of a simulated actor (the same in every run with the same commands and seed).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimId(pub u64);

/// Hands out ids 1, 2, 3, … in order.
#[derive(Resource, Debug, Default)]
pub struct SimIdAllocator {
    next: u64,
}

impl SimIdAllocator {
    pub fn alloc(&mut self) -> SimId {
        self.next += 1;
        SimId(self.next)
    }
}

/// `SimId` → `Entity`, ordered by id. Systems that draw random numbers per actor iterate this,
/// so the draw order does not depend on ECS storage order.
#[derive(Resource, Debug, Default)]
pub struct SimIndex(BTreeMap<SimId, Entity>);

impl SimIndex {
    pub fn get(&self, id: SimId) -> Option<Entity> {
        self.0.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (SimId, Entity)> + '_ {
        self.0.iter().map(|(i, e)| (*i, *e))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn insert(&mut self, id: SimId, e: Entity) {
        self.0.insert(id, e);
    }

    pub(crate) fn remove(&mut self, id: SimId) -> Option<Entity> {
        self.0.remove(&id)
    }
}

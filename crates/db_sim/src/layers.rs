//! What the level's colliders stop, as avian collision layers on the static geometry and masks on the sim's
//! queries. The original's collision profiles split by channel: `Pawn` (characters walking), and `Projectile` /
//! `Tgf_TraceChannel_Sight` / the weapon traces (shots, sight, melee). Most geometry blocks both (layer bit 0,
//! also every character and anything unmarked); a blocking volume (`InvisibleWall`) or barred jail door stops only
//! characters ([`PAWN_ONLY`]); bars (`HollowMesh`) stop only shots and sight ([`SHOTS_ONLY`]). Walking, ground
//! probes, the nav bake and the monsters' straight-way checks use [`walk`]; sight, shots and melee use [`shots`]
//! (and still pick level pieces by [`crate::LevelBox`]).

use avian3d::prelude::{CollisionLayers, LayerMask, SpatialQueryFilter};

/// Stops characters only.
pub const PAWN_ONLY: u32 = 1 << 1;
/// Stops shots and sight only.
pub const SHOTS_ONLY: u32 = 1 << 2;

/// The collision cache's instance kinds (`map.rs`): 0 both, 1 characters only, 2 shots and sight only.
pub fn of_kind(kind: u32) -> Option<CollisionLayers> {
    match kind {
        1 => Some(CollisionLayers::new(LayerMask(PAWN_ONLY), LayerMask::ALL)),
        2 => Some(CollisionLayers::new(LayerMask(SHOTS_ONLY), LayerMask::ALL)),
        _ => None,
    }
}

/// Queries for moving characters (sweeps, ground probes, the nav bake, straight-way checks).
pub fn walk() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(LayerMask(1 | PAWN_ONLY))
}

/// Queries for shots, sight and melee.
pub fn shots() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(LayerMask(1 | SHOTS_ONLY))
}

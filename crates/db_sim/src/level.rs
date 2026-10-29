//! Level geometry described once and used by both sides: the sim turns each piece into a static
//! collider, the client into a textured mesh. Units: cm, Z up.

use bevy_math::Vec3;

use crate::SimCommand;

/// What a piece of geometry looks like (the client maps this to a material).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    Floor,
    Wall,
    MossWall,
    Ceiling,
    Wood,
    Metal,
    Door,
}

/// An axis-aligned box: centre and half extents (cm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelPiece {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub surface: Surface,
}

impl LevelPiece {
    pub fn command(&self) -> SimCommand {
        SimCommand::AddStaticBox {
            center: self.center,
            half_extents: self.half_extents,
        }
    }
}

/// Where the player starts in the test room (capsule centre, cm) and the initial yaw (radians, 0 = +X).
pub const TEST_ROOM_SPAWN: Vec3 = Vec3::new(-650.0, -400.0, 92.0);
pub const TEST_ROOM_SPAWN_YAW: f32 = 0.55;
/// The training dummy, about 1.85 m in front of the spawn (capsule centre, cm).
pub const TEST_ROOM_DUMMY: Vec3 = Vec3::new(-492.0, -303.0, 90.0);
/// The Goblin Axe: across the room, out of sight range of the spawn, facing the spawn side.
pub const TEST_ROOM_GOBLIN: Vec3 = Vec3::new(350.0, -150.0, 90.0);
pub const TEST_ROOM_GOBLIN_YAW: f32 = std::f32::consts::PI;
/// A Wood_02 chest (loot source 174) near the spawn: floor point under it.
pub const TEST_ROOM_CHEST: Vec3 = Vec3::new(-560.0, -530.0, 0.0);
pub const TEST_ROOM_CHEST_SOURCE: i32 = 174;
/// The tutorial Goblin Axe's loot source.
pub const TEST_ROOM_GOBLIN_LOOT: i32 = 207;

/// The test arena (`--arena`: every monster at a sensible distance, back 5 s after it dies, chests and altars, light
/// to run instead of the maps): 64 m × 124 m, 10 m high (room for the flyers). The player's bay is at the south end,
/// looking north over the monsters.
pub const ARENA_SPAWN: Vec3 = Vec3::new(0.0, -2400.0, 92.0);
pub const ARENA_SPAWN_YAW: f32 = std::f32::consts::FRAC_PI_2;
/// The training dummy, 3.5 m ahead and 3.5 m right of the spawn (UE axes: facing +Y, right is -X), off the view's
/// centre line.
pub const ARENA_DUMMY: Vec3 = Vec3::new(-350.0, -2050.0, 90.0);
/// Monster spots (capsule centre, cm): regulars on a 10 m grid six wide from 15 m ahead of the spawn (beyond the 8-15
/// m sight and 6 m hearing of the monster sheets, neighbours don't see each other's fights), bosses 15 m apart in two
/// rows of four beyond them.
pub fn arena_spot(i: usize, boss: bool) -> Vec3 {
    if boss {
        Vec3::new(-2250.0 + (i % 4) as f32 * 1500.0, 5600.0 + (i / 4) as f32 * 1500.0, 90.0)
    } else {
        Vec3::new(-2500.0 + (i % 6) as f32 * 1000.0, -900.0 + (i / 6) as f32 * 1000.0, 90.0)
    }
}

/// The arena's floor, ceiling and walls (see [`ARENA_SPAWN`]).
pub fn arena() -> Vec<LevelPiece> {
    let p = |c: [f32; 3], h: [f32; 3], s| LevelPiece { center: Vec3::from(c), half_extents: Vec3::from(h), surface: s };
    let (hx, hy, cy, h) = (3200.0, 6200.0, 3200.0, 1000.0);
    let t = 25.0;
    vec![
        p([0.0, cy, -t], [hx, hy, t], Surface::Floor),
        p([0.0, cy, h + t], [hx, hy, t], Surface::Ceiling),
        p([hx + t, cy, h / 2.0], [t, hy + 2.0 * t, h / 2.0], Surface::Wall),
        p([-hx - t, cy, h / 2.0], [t, hy + 2.0 * t, h / 2.0], Surface::Wall),
        p([0.0, cy + hy + t, h / 2.0], [hx, t, h / 2.0], Surface::MossWall),
        p([0.0, cy - hy - t, h / 2.0], [hx, t, h / 2.0], Surface::Wall),
        // A low wall between the bay and the monsters' field: cover to step behind, a ledge to jump on.
        p([-1600.0, -1900.0, 60.0], [400.0, 30.0, 60.0], Surface::Wood),
        p([1600.0, -1900.0, 60.0], [400.0, 30.0, 60.0], Surface::Wood),
    ]
}

/// The greybox test room: 16 m × 12 m, 4 m high, with pillars, crates, a metal block and a door.
pub fn test_room() -> Vec<LevelPiece> {
    let p = |c: [f32; 3], h: [f32; 3], s| LevelPiece {
        center: Vec3::from(c),
        half_extents: Vec3::from(h),
        surface: s,
    };
    let (hx, hy, h) = (800.0, 600.0, 400.0);
    let t = 25.0; // wall half thickness
    vec![
        p([0.0, 0.0, -t], [hx, hy, t], Surface::Floor),
        p([0.0, 0.0, h + t], [hx, hy, t], Surface::Ceiling),
        p(
            [hx + t, 0.0, h / 2.0],
            [t, hy + 2.0 * t, h / 2.0],
            Surface::Wall,
        ),
        p(
            [-hx - t, 0.0, h / 2.0],
            [t, hy + 2.0 * t, h / 2.0],
            Surface::Wall,
        ),
        p([0.0, hy + t, h / 2.0], [hx, t, h / 2.0], Surface::MossWall),
        p([0.0, -hy - t, h / 2.0], [hx, t, h / 2.0], Surface::Wall),
        // Pillars
        p(
            [-250.0, 150.0, h / 2.0],
            [40.0, 40.0, h / 2.0],
            Surface::Wall,
        ),
        p(
            [250.0, 150.0, h / 2.0],
            [40.0, 40.0, h / 2.0],
            Surface::Wall,
        ),
        // Crates and a metal block to walk around and jump on
        p([450.0, -350.0, 40.0], [40.0, 40.0, 40.0], Surface::Wood),
        p([530.0, -350.0, 40.0], [40.0, 40.0, 40.0], Surface::Wood),
        p([490.0, -350.0, 120.0], [40.0, 40.0, 40.0], Surface::Wood),
        p([-500.0, 350.0, 30.0], [80.0, 50.0, 30.0], Surface::Metal),
        // Door in the far (+X) wall
        p([hx - 2.0, 0.0, 120.0], [4.0, 70.0, 120.0], Surface::Door),
    ]
}

//! Gameplay simulation. Runs one fixed tick per [`SimTick`] schedule run.
//!
//! Rules:
//! - input enters only as [`SimCommand`]s pushed into [`CommandQueue`];
//! - state leaves as components the client reads and [`SimEvent`]s it drains from [`SimEvents`];
//! - fixed timestep ([`DT`]), one seeded RNG ([`SimRng`]), stable ids ([`SimId`]) instead of `Entity`;
//! - [`state_hash`] fingerprints the whole state for determinism tests.

pub mod affixes;
pub mod ai;
pub mod layers;
pub mod attributes;
pub mod bt;
pub mod combat;
pub mod crossbow;
pub mod crypt;
pub mod death_zone;
pub mod destroyables;
pub mod dungeon;
pub mod features;
pub mod floor_collapse;
pub mod gear_triggers;
pub mod command;
pub mod hash;
pub mod buffs;
pub mod class_skills;
pub mod id;
pub mod inventory;
pub mod items;
pub mod level;
pub mod loot;
pub mod map;
pub mod melee;
pub mod merchant;
pub mod monsters;
pub mod movement;
pub mod nav;
pub mod overrides;
pub mod profile;
pub mod projectiles;
pub mod rng;
pub mod respawn;
pub mod run;
pub mod skills;
pub mod shadow_dragon;
pub mod shadow_king;
pub mod throne;
pub mod staffs;
pub mod summon;
pub mod swing_doors;
pub mod mechanisms;
pub mod monster_effects;
pub mod npc_state;
pub mod aura_areas;
pub mod thief;
pub mod throwables;
pub mod wendigo;
pub mod weapon_slots;
pub mod weapons;
pub mod world_items;

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use avian3d::prelude::{Gravity, PhysicsPlugins, PhysicsSystems};
use bevy_math::{Vec2, Vec3};
use bevy_time::Time;

pub use avian3d::prelude::{Position, SpatialQuery};
pub use ai::SpawnOpts;
pub use command::LevelBox;
pub use command::{ActorKind, CommandQueue, SimCommand, SimEvent, SimEvents};
pub use melee::{Aim, AttackKind, Combatant, Health, Melee};
pub use movement::{CharacterState, MoveInput, MoveSpeed};
pub use hash::state_hash;
pub use id::{SimId, SimIndex};
pub use rng::SimRng;
pub use skills::{HitSource, SkillKind, SkillKit};
pub use buffs::{Buff, BuffKind, Buffs};
pub use projectiles::{Projectile, ProjectileSpec};
pub use class_skills::{ClassData, ClassKit, Power, PowerKind};
pub use ai::{AiSenses, AiState, MonsterAi};
pub use command::Kind;
pub use inventory::{GameData, Inventory, InventoryCommand, ItemInstance};
pub use world_items::{Container, Effects, GroundItem, LootSource};
pub use monsters::{MonsterAttack, MonsterKit};
pub use run::{Portal, RunClock, RunOutcome, RunStatus};
pub use features::{Altar, AltarKind, Door, Trap, TrapKind};
pub use destroyables::Destroyable;
pub use staffs::{StaffAction, StaffDb, StaffElement, StaffHit, StaffSpec, StaffState};

/// Simulation rate in ticks per second.
pub const TICK_RATE_HZ: u32 = 30;

/// Seconds per tick.
pub const DT: f32 = 1.0 / TICK_RATE_HZ as f32;

/// The schedule that advances the simulation by exactly one tick.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimTick;

/// Number of ticks simulated so far.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TickCount(pub u64);

/// Placeholder AI: picks a new random direction every `period` ticks (until brings real AI).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Wander {
    pub period: u32,
}

/// Registers the simulation schedule and core resources. `seed` drives every random roll.
#[derive(Debug, Default)]
pub struct SimPlugin {
    pub seed: u64,
}

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(SimTick);
        // Physics runs inside SimTick. Headless runs need the engine resources the client's
        // DefaultPlugins would otherwise provide.
        if !app.world().contains_resource::<Time>() {
            app.init_resource::<Time>();
        }
        if !app.is_plugin_added::<bevy_transform::TransformPlugin>() {
            app.add_plugins(bevy_transform::TransformPlugin);
        }
        app.add_plugins(PhysicsPlugins::new(SimTick).with_length_unit(100.0))
            .insert_resource(Gravity(Vec3::new(0.0, 0.0, -overrides::movement::GRAVITY)))
            // The collider trees are optimized on the sim's own thread: as an async task (avian's default) the tick
            // waited for it behind the client's asset loads and shader builds on the same task pool, up to 3 s a
            // tick and 95 s in all while the arena loaded. Only characters move, so the optimization is small.
            .insert_resource(avian3d::collider_tree::ColliderTreeOptimization { use_async_tasks: false, ..Default::default() })
            // The sim moves everything through `Position` / `Rotation`, never `Transform`: avian's
            // Transform -> Position sync, its own Transform propagation and the Transform -> collider scale
            // sync only checked every map collider each tick (6.7 ms a tick for the castle's 149,000).
            .insert_resource(avian3d::physics_transform::PhysicsTransformConfig { propagate_before_physics: false, transform_to_position: false, position_to_transform: true, transform_to_collider_scale: false })
            .init_resource::<TickCount>()
            .init_resource::<CommandQueue>()
            .init_resource::<SimEvents>()
            .init_resource::<id::SimIdAllocator>()
            .init_resource::<SimIndex>()
            .init_resource::<inventory::ItemUids>()
            .init_resource::<run::RunClock>()
            .init_resource::<staffs::ThunderAreas>()
            .init_resource::<staffs::AoeRuns>()
            .init_resource::<aura_areas::AuraAreas>()
            .init_resource::<throwables::FlaskAreas>()
            .init_resource::<shadow_dragon::DragonFires>()
            .insert_resource(SimRng::new(self.seed))
            .add_systems(
                SimTick,
                (set_sim_clock, command::apply_commands, wander, ai::ai_tick, thief::thief_world, staffs::staffs_tick, weapon_slots::weapon_slots_tick, weapons::weapons_tick, crossbow::crossbows_tick, melee::melee_tick, movement::corpse_collision, movement::move_characters, skills::skills_tick, class_skills::class_skills_tick, projectiles::projectiles_tick, buffs::buffs_tick, summon::summon_tick, (features::features_tick, swing_doors::swing_doors_tick, mechanisms::mechanisms_tick).chain(), (world_items::world_items_tick, destroyables::destroyables_tick).chain(), (death_zone::death_zone_tick, floor_collapse::floor_collapse_tick, shadow_dragon::dragon_tick, throne::throne_tick, run::run_tick, respawn::respawn_tick, gear_triggers::gear_triggers_tick, affixes::affixes_tick, throwables::throwables_tick).chain())
                    .chain()
                    .before(PhysicsSystems::First),
            )
            .add_systems(SimTick, advance_tick.after(PhysicsSystems::Last));
    }
}

/// The generic `Time` that avian reads advances by exactly one tick (also correct under the
/// client's `FixedUpdate`, which runs at the same rate).
fn set_sim_clock(mut time: ResMut<Time>, mut clock: Local<Time>) {
    clock.advance_by(std::time::Duration::from_secs_f64(1.0 / f64::from(TICK_RATE_HZ)));
    *time = *clock;
}

fn wander(tick: Res<TickCount>, mut rng: ResMut<SimRng>, index: Res<SimIndex>, mut q: Query<(&Wander, &mut MoveInput)>) {
    // Walk in id order so the RNG draws happen in a stable sequence.
    for (_, e) in index.iter() {
        if let Ok((w, mut intent)) = q.get_mut(e)
            && tick.0.is_multiple_of(u64::from(w.period.max(1)))
        {
            let angle = rng.unit_f32() * std::f32::consts::TAU;
            intent.dir = Vec2::new(angle.cos(), angle.sin());
        }
    }
}

fn advance_tick(mut tick: ResMut<TickCount>) {
    tick.0 += 1;
}

/// A headless app with the simulation ready to step (plugin setup finished, as `App::run` would do).
pub fn headless_app(seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins((bevy_app::TaskPoolPlugin::default(), SimPlugin { seed }));
    app.finish();
    app.cleanup();
    app
}

/// Run `n` simulation ticks on a world that has [`SimPlugin`]'s schedule.
pub fn step(app: &mut App, n: u32) {
    for _ in 0..n {
        app.world_mut().run_schedule(SimTick);
    }
}

/// The config cache written by `db_pipeline config`, if present (gitignored data).
pub fn load_config_cache() -> Option<db_formats::ConfigTables> {
    std::fs::read(db_formats::data_path("cache/config/tables.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_advance_headless() {
        let mut app = headless_app(0);
        step(&mut app, 300);
        assert_eq!(app.world().resource::<TickCount>().0, 300);
    }

    #[test]
    fn commands_spawn_move_and_despawn() {
        let mut app = headless_app(0);
        app.world_mut().resource_mut::<CommandQueue>().push(SimCommand::Spawn {
            kind: ActorKind::Player,
            pos: Vec3::ZERO,
            speed: 180.0,
        });
        step(&mut app, 1);
        let events: Vec<_> = app.world_mut().resource_mut::<SimEvents>().drain().collect();
        let SimEvent::Spawned { id, .. } = events[0] else { panic!("{events:?}") };
        app.world_mut().resource_mut::<CommandQueue>().push(SimCommand::SetMoveIntent { id, dir: Vec3::X });
        step(&mut app, TICK_RATE_HZ);
        let e = app.world().resource::<SimIndex>().get(id).unwrap();
        let x = app.world().get::<Position>(e).unwrap().0.x;
        assert!((x - 180.0).abs() < 1e-3, "moved {x} cm in 1 s at 180 cm/s");
        app.world_mut().resource_mut::<CommandQueue>().push(SimCommand::Despawn { id });
        step(&mut app, 1);
        assert!(app.world().resource::<SimIndex>().get(id).is_none());
        let events: Vec<_> = app.world_mut().resource_mut::<SimEvents>().drain().collect();
        assert_eq!(events, vec![SimEvent::Despawned { id }]);
    }
}

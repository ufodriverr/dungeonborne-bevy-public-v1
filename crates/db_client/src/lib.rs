//! Client side: drives the simulation at a fixed rate, turns input into `SimCommand`s and presents
//! the sim state. No gameplay state lives here (only presentation: camera angles, materials).
//!
//! Coordinates: the sim uses UE centimetres with Z up; Bevy uses metres with Y up. [`to_bevy`]
//! swaps Y/Z (which also turns UE's left-handed frame into Bevy's right-handed one) and scales.

mod armor_meshes;
mod armory;
mod audio;
mod aura_view;
mod autorun;
mod combat_view;
mod dungeon_view;
mod destroyable_view;
mod effects;
mod fx_material;
mod skill_clips;
mod skill_vfx;
mod features_view;
mod fonts;
mod fp_body;
mod fp_clips;
mod hud;
mod staff_view;
mod swing_door_view;
mod collapse_view;
mod dragon_view;
mod mechanism_view;
mod zone_view;
mod flow;
mod inventory_ui;
mod items_view;
mod ground_aim;
mod landscape;
mod loading;
mod cooldown_sweep;
mod menus;
mod pause;
mod settings;
mod model_view;
mod monster_models;
mod map_view;
mod map_vfx;
mod materials;
mod prop_meshes;
mod projectile_view;
mod throwables_view;
mod props_view;
mod weapon_meshes;
mod weapon_view;
mod panther_view;
mod potion_view;
mod waltz_view;
mod whirl_view;
mod monster_view;

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::light::GlobalAmbientLight;
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use db_sim::level::{LevelPiece, Surface, TEST_ROOM_CHEST, TEST_ROOM_CHEST_SOURCE, TEST_ROOM_DUMMY, TEST_ROOM_GOBLIN, TEST_ROOM_SPAWN, TEST_ROOM_SPAWN_YAW, test_room};
use db_sim::{ActorKind, AttackKind, CharacterState, CommandQueue, Melee, Position, SimCommand, SimIndex, SimPlugin, SimTick, SkillKit, TICK_RATE_HZ};

pub use audio::AudioMuted;
pub use combat_view::{AutoAttack, AutoSkill, AutoWalk, CombatData, LocalPlayer};
pub use autorun::{AutoRun, AutoRunShots};
pub use effects::ShowEffect;
pub use skill_vfx::{AutoHeavy, AutoRelease, VfxShots};
pub use throwables_view::{AutoDrink, AutoFlask, AutoTorch};
pub use menus::PressArena;
pub use fp_body::{FpPose, ViewOffhand};
pub use staff_view::AutoCast;
pub use materials::RawMaterials;
pub use dungeon_view::{CurrentCrypt, DevStartAt, EscapeDelay, PlaySpace, TopDown};
pub use db_formats::{data_path, data_root};
pub use pause::{PauseAt, Paused};
pub use settings::{Quality, Settings, SettingsOverride, SettingsPath};
pub use flow::{FreshProfile, Peaceful, PlayMap, RunLength, SavePath, Screen, ShotRequests, StartScreen};
pub use inventory_ui::{AutoLoot, InventoryOpen};
pub use monster_view::AutoFight;
pub use model_view::ModelViewer;
pub use loading::BlockingDataLoad;
pub use monster_view::DevClassAt;
pub use weapon_view::{DevAttackAt, DevInteractAt, DevSummonDragonAt, DevSwitchAt, DevTriggerAt, DevWield, ViewWeapon};
pub use map_view::MapViewer;
pub use db_sim::SkillKind;
pub use db_sim::crypt::map_def;
pub use db_formats::config::generated::EnRoleCareerType;

/// Sim centimetres (Z up) → Bevy metres (Y up).
pub fn to_bevy(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, v.y) * 0.01
}

/// The Q/E skills given to the local player when it spawns (from the profile in a run, or
/// `--loadout` in the test room); `None` keeps the class default.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct SkillLoadout(pub Option<[SkillKind; 2]>);

/// Dev: the local player fires a projectile at the nearest monster every 1.2 s, cycling straight,
/// homing (the Wizard's decoded speed curve) and thrown shots (check).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoShoot(pub bool);

/// Dev: play the test room as this class instead of the Fighter (sheet and starting gear from data).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TestClass(pub Option<db_formats::config::generated::EnRoleCareerType>);

/// Dev: the curve row of the roster monster that replaces the test room's Goblin Axe.
#[derive(Resource, Debug, Default, Clone)]
pub struct TestMonster(pub Option<String>);

/// The first placed lever mechanism whose class ends `package`'s name, from the run files (dev).
fn test_mechanism(package: &str) -> Option<(db_sim::mechanisms::MechanismPlacement, Vec<db_sim::mechanisms::MechanismPlacement>)> {
    let class = package.rsplit('/').next()?;
    ["castle-run.json", "crypt-run.json"].iter().find_map(|f| {
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(db_formats::data_path(format!("recovered/maps/{f}"))).ok()?).ok()?;
        json["floors"].as_array()?.iter().find_map(|fl| {
            let levers = fl["levers"].as_array().cloned().unwrap_or_default();
            let all = fl["mechanisms"].as_array()?;
            let m = all.iter().enumerate().find(|(_, m)| m["class"].as_str() == Some(class)).and_then(|(i, m)| db_sim::mechanisms::MechanismPlacement::from_json(m, i, &levers))?;
            let bound = m.bound.iter().filter_map(|&b| db_sim::mechanisms::MechanismPlacement::from_json(all.get(b as usize)?, b as usize, &levers)).collect();
            Some((m, bound))
        })
    })
}

/// Dev: a swing door (blueprint package) 3 m ahead of the test room's start, facing the same way; or a lever
/// mechanism's first placement moved there.
#[derive(Resource, Debug, Default, Clone)]
pub struct TestDoor(pub Option<String>);

/// Dev: a breakable prop (its class, e.g. `BP_ClayPot_11`) 1.5 m ahead of the test room's start, in reach of a swing (with
/// `--class Wizard` it stays in sight: the Fighter's shield hides it).
#[derive(Resource, Debug, Default, Clone)]
pub struct TestProp(pub Option<String>);

/// Dev: put a door, the three traps and the five altars into the test room.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TestFeatures(pub bool);

/// Dev: the test arena instead of the test room (`--arena`): every monster spaced out, each back
/// [`db_sim::overrides::arena::RESPAWN_SECONDS`] after its death, chests and altars by the spawn.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Arena(pub bool);

/// Dev: an observer camera this many metres behind the player's eye along the view, the first-person body hidden and
/// a stand-in drawn for the player (`--observe <m>`, AR2: the effects first person can't show, as the Fighter's
/// Charge trail at the feet).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Observe(pub Option<f32>);

impl Arena {
    /// How long after its death an arena monster comes back (None outside the arena).
    pub fn respawn(&self) -> Option<f32> {
        self.0.then_some(db_sim::overrides::arena::RESPAWN_SECONDS)
    }
}

/// Dev: the test room's starting look direction (degrees, sim yaw), instead of the default.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct StartLook(pub Option<f32>, pub Option<f32>);

/// Horizontal field of view in degrees (the game's FOV setting; saved value 110).
#[derive(Resource, Debug, Clone, Copy)]
pub struct FovSetting(pub f32);

/// GPU occlusion culling on the player camera (on unless `--no-occlusion`, for comparing).
#[derive(Resource, Debug, Clone, Copy)]
pub struct OcclusionCullingOn(pub bool);

impl Default for OcclusionCullingOn {
    fn default() -> Self {
        Self(true)
    }
}

impl Default for FovSetting {
    fn default() -> Self {
        Self(110.0)
    }
}

/// Vertical FOV (radians) for a horizontal FOV (degrees) at an aspect ratio.
pub fn vertical_fov(horizontal_deg: f32, aspect: f32) -> f32 {
    2.0 * ((horizontal_deg.to_radians() * 0.5).tan() / aspect).atan()
}

/// Camera angles (radians). Yaw 0 looks along sim +X; positive yaw turns right (towards sim +Y).
#[derive(Resource, Debug, Clone, Copy)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
}

/// One-shot inputs pressed since the last sim tick.
#[derive(Resource, Debug, Default)]
struct PendingInput {
    jump: bool,
    attack: Option<AttackKind>,
    /// 0 = Q, 1 = E (the skill comes from the actor's loadout).
    skill: Option<usize>,
    /// The Q or E key was let go (charged skills fire).
    release: Option<usize>,
    /// The staff spell input went on (left click held) or off.
    cast: Option<bool>,
    /// A charged skill was cancelled (right click during Pyroblast).
    cancel: Option<usize>,
    /// A weapon's charge attack was let go.
    charge_release: bool,
    /// The reload key.
    reload: bool,
    /// Switch weapon sets.
    switch: bool,
}

/// Eye height above the capsule centre (cm), standing and crouched (visual only). Standing: CONFIRMED,
/// `BP_Tgf_Hero_Default` `BaseEyeHeight` 80.773 (its camera is a Lyra camera, which pivots on the capsule
/// centre plus the eye height). Crouched: ours (the data has `CrouchedEyeHeight` 40, but the sim keeps
/// the capsule's centre when crouching and the crouched half height isn't decoded).
pub(crate) const EYE_OFFSET: f32 = 80.773;
const EYE_OFFSET_CROUCHED: f32 = 30.0;
const EYE_OFFSET_DEAD: f32 = -60.0;
/// Radians per mouse count at sensitivity 1 (the settings multiply it).
const MOUSE_SENSITIVITY: f32 = 0.0025;
/// Holding left click this long makes a heavy attack; a shorter click is a light attack on release
/// (our binding, as in the game's controls description; the exact threshold is HYPOTHESIS).
const HEAVY_HOLD_S: f32 = 0.35;
/// Texture tile size in metres.
const TILE_M: f32 = 2.0;

#[derive(Component)]
pub(crate) struct PlayerCamera;

/// A character's position before the last sim tick: the sim steps at 30 Hz and the screen draws
/// faster, so characters and the camera are drawn between the last two ticks ([`drawn`]) instead of
/// holding each tick for two or three frames and then jumping.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct PrevPosition(pub(crate) Vec3);

/// Farther than this in one tick is a teleport (portal, respawn), drawn at once (cm).
const TELEPORT_CM: f32 = 300.0;
/// The eye follows the floor's height with this lag on the ground (s, ours): stair steps and seams in the
/// floor meshes move the capsule up to a few cm a tick.
const EYE_SMOOTH_S: f32 = 0.05;

/// Dev: write the camera and the local player every rendered frame of a run to this CSV
/// (`--trace-camera`): frame time, camera position (m), the player's sim height (cm) and ground flag.
#[derive(Resource, Debug, Default, Clone)]
pub struct CameraTrace(pub Option<std::path::PathBuf>);

#[allow(clippy::too_many_arguments)]
fn trace_camera(trace: Res<CameraTrace>, screen: Res<State<Screen>>, cover: Res<flow::EntryCover>, real: Res<Time<Real>>, player: Res<LocalPlayer>, index: Res<SimIndex>, actors: Query<(&Position, &CharacterState)>, cam: Query<&Transform, With<PlayerCamera>>, mut out: Local<Option<std::io::BufWriter<std::fs::File>>>) {
    use std::io::Write;
    let Some(path) = &trace.0 else { return };
    if *screen.get() != Screen::Run || cover.on {
        return;
    }
    let (Ok(t), Some((p, st))) = (cam.single(), player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok())) else { return };
    if out.is_none() {
        let Ok(f) = std::fs::File::create(path) else { return };
        let mut w = std::io::BufWriter::new(f);
        let _ = writeln!(w, "dt_ms,cam_x,cam_y,cam_z,sim_z,ground");
        *out = Some(w);
    }
    if let Some(w) = out.as_mut() {
        let _ = writeln!(w, "{:.3},{:.4},{:.4},{:.4},{:.3},{}", real.delta_secs() * 1000.0, t.translation.x, t.translation.y, t.translation.z, p.0.z, u8::from(st.on_ground));
    }
}

/// Where to draw a character: between its last two ticks by how far the frame is into the next tick.
pub(crate) fn drawn(pos: &Position, prev: Option<&PrevPosition>, fixed: &Time<Fixed>) -> Vec3 {
    match prev {
        Some(p) if p.0.distance_squared(pos.0) < TELEPORT_CM * TELEPORT_CM => p.0.lerp(pos.0, fixed.overstep_fraction().clamp(0.0, 1.0)),
        _ => pos.0,
    }
}

/// Adds the simulation, runs one [`SimTick`] per fixed update, and sets up presentation.
#[derive(Debug, Default)]
pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        effects::watch_pipelines(app);
        app.add_plugins(SimPlugin::default())
            .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_RATE_HZ)))
            .init_resource::<FovSetting>()
            .init_resource::<destroyable_view::PieceCache>()
            .init_resource::<map_vfx::MapVfx>()
            .init_resource::<effects::MonsterVfx>()
            .insert_resource(effects::MonsterActorVfx::load())
            .add_systems(Update, aura_view::aura_decals)
            .init_resource::<OcclusionCullingOn>()
            .init_resource::<SimTickTime>()
            .init_resource::<TestMonster>()
            .init_resource::<Arena>()
            .init_resource::<Observe>()
            .init_resource::<TestDoor>()
            .init_resource::<TestProp>()
            .init_resource::<TestFeatures>()
            .init_resource::<StartLook>()
            .init_resource::<ModelViewer>()
            .init_resource::<ViewWeapon>()
            .init_resource::<DevWield>()
            .init_resource::<DevSwitchAt>()
            .init_resource::<monster_view::DevClassAt>()
            .init_resource::<armory::Armory>()
            .init_resource::<ground_aim::GroundAim>()
            .add_systems(Update, ground_aim::ground_aim)
            .add_systems(Update, armory::armory.run_if(in_state(Screen::TestRoom)))
            .init_resource::<DevInteractAt>()
            .init_resource::<DevTriggerAt>()
            .init_resource::<DevSummonDragonAt>()
            .init_resource::<DevAttackAt>()
            .init_resource::<FpPose>()
            .init_resource::<ViewOffhand>()
            .init_resource::<MapViewer>()
            .init_resource::<SkillLoadout>()
            .init_resource::<AutoShoot>()
            .init_resource::<TestClass>()
            .init_resource::<LocalPlayer>()
            .init_resource::<PendingInput>()
            .init_resource::<AutoAttack>()
            .init_resource::<menus::PressArena>()
            .init_resource::<AutoTorch>()
            .init_resource::<AutoDrink>()
            .init_resource::<AutoSkill>()
            .init_resource::<AutoWalk>()
            .init_resource::<AutoFight>()
            .init_resource::<monster_view::DeadFor>()
            .init_resource::<combat_view::FrameEvents>()
            .init_resource::<InventoryOpen>()
            .init_resource::<PlaySpace>()
            .init_resource::<TopDown>()
            .init_resource::<dungeon_view::MonsterSpawns>()
            .init_resource::<AutoLoot>()
            .init_resource::<inventory_ui::Tooltip>()
            .init_resource::<items_view::InteractTarget>()
            .init_state::<Screen>()
            .init_resource::<StartScreen>()
            .init_resource::<SavePath>()
            .init_resource::<FreshProfile>()
            .init_resource::<Peaceful>()
            .init_resource::<RunLength>()
            .init_resource::<flow::ProfileRes>()
            .init_resource::<flow::PendingCharacter>()
            .init_resource::<flow::PendingAction>()
            .init_resource::<flow::LastRun>()
            .init_resource::<flow::AfterRun>()
            .init_resource::<pause::Paused>()
            .init_resource::<pause::PauseAt>()
            .init_resource::<settings::Settings>()
            .init_resource::<settings::SettingsPath>()
            .init_resource::<settings::SettingsOverride>()
            .init_resource::<settings::Keys>()
            .init_resource::<settings::SettingsOpen>()
            .init_resource::<ShotRequests>()
            .init_resource::<AutoRun>()
            .init_resource::<autorun::BotMove>()
            .init_resource::<autorun::CurrentDungeon>()
            .init_resource::<CurrentCrypt>()
            .init_resource::<flow::CryptLoader>()
            .init_resource::<flow::PlayMap>()
            .init_resource::<flow::EntryCover>()
            .init_resource::<map_view::StreamBacklog>()
            // Uncapped until a run's entry cover lifts (`flow::entry_cover` turns the cap on then). Capped from
            // startup, the test room's first uploads never showed (bisected).
            .insert_resource(bevy::render::render_asset::RenderAssetBytesPerFrame::default())
            .init_resource::<fonts::UiFonts>()
            .init_resource::<EscapeDelay>()
            .init_resource::<DevStartAt>()
            .init_resource::<CameraTrace>()
            .add_plugins(landscape::plugin)
            .add_plugins(fx_material::FxMaterialPlugin)
            .add_plugins(cooldown_sweep::plugin)
            .add_plugins(audio::AudioPlugin)
            .add_plugins(staff_view::plugin)
            .add_plugins(skill_vfx::plugin)
            .insert_resource(Look { yaw: TEST_ROOM_SPAWN_YAW, pitch: -0.08 })
            // No environment maps yet: a neutral ambient stands in for bounce light (a blue tint made metals blue).
            .insert_resource(GlobalAmbientLight { color: Color::srgb(0.78, 0.74, 0.7), brightness: 140.0, ..default() })
            .add_systems(Startup, (settings::load_settings, fonts::load_fonts, materials::load_overrides, effects::load_effects, load_data, setup_camera, combat_view::setup_hud, monster_view::setup_player_hud, hud::setup_art_hud, pause::setup_pause_menu, settings::setup_settings_screen, items_view::setup_prompt, inventory_ui::setup_inventory, flow::setup_run_hud, dungeon_view::top_down_cleanup, loading::setup_loading_screen).chain())
            .add_systems(Update, (loading::finish_data_load, flow::boot.run_if(in_state(Screen::Boot)).run_if(loading::data_loaded)).chain())
            .add_systems(Update, loading::loading_screen)
            .add_systems(OnEnter(Screen::TestRoom), build_test_room)
            .add_systems(OnEnter(Screen::MapView), map_view::build_map)
            .add_systems(Update, map_view::walk_ahead.run_if(in_state(Screen::MapView)))
            .add_systems(Update, (map_view::stream_cells, map_view::apply_map_sky))
            .add_systems(Update, flow::keep_map_loading.run_if(not(in_state(Screen::Boot))))
            .add_systems(Update, flow::entry_cover)
            .add_systems(Update, trace_camera.after(follow_camera))
            .init_resource::<whirl_view::WhirlView>()
            .add_systems(Update, waltz_view::waltz_view.after(follow_camera))
            .init_resource::<panther_view::PantherView>()
            .add_systems(Update, panther_view::panther_camera.after(whirl_view::whirl_camera).before(combat_view::place_labels).before(combat_view::sync_actor_views))
            .add_systems(Update, (whirl_view::whirl_camera.after(follow_camera).before(combat_view::place_labels).before(combat_view::sync_actor_views), whirl_view::whirl_body.after(whirl_view::whirl_camera)))
            .add_systems(PostUpdate, whirl_view::hide_arms.before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate))
            .add_systems(PostUpdate, hide_view_model.run_if(|o: Res<Observe>| o.0.is_some()).before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate))
            .add_systems(OnEnter(Screen::Run), flow::enter_run)
            .add_systems(OnExit(Screen::Run), flow::exit_run)
            .add_systems(OnEnter(Screen::Title), (flow::release_cursor, menus::spawn_title))
            .add_systems(OnEnter(Screen::Stash), (flow::release_cursor, menus::spawn_stash))
            .add_systems(OnEnter(Screen::Result), (flow::release_cursor, menus::spawn_result))
            .add_systems(OnExit(Screen::Title), menus::despawn_menu)
            .add_systems(OnExit(Screen::Stash), menus::despawn_menu)
            .add_systems(OnExit(Screen::Result), menus::despawn_menu)
            .add_systems(Update, (menus::menu_buttons, menus::button_looks, menus::press_arena.before(menus::apply_action), menus::apply_action, pause::pause_at, pause::apply_pause, settings::settings_screen, settings::apply_settings, settings::ui_scale, settings::update_fov, settings::light_quality).chain())
            .add_systems(Update, (menus::redraw_stash, menus::stash_clicks).chain().run_if(in_state(Screen::Stash)))
            .add_systems(Update, (flow::run_ended.run_if(in_state(Screen::Run)), flow::run_hud, flow::hud_visibility, autorun::auto_run).after(combat_view::read_events))
            .add_systems(
                Update,
                (combat_view::drain_sim_events, combat_view::read_events, pause::toggle_pause.run_if(playing).after(settings::settings_screen), inventory_ui::toggle_inventory.run_if(playing), grab_cursor.run_if(playing), mouse_look, collect_input, follow_camera).chain(),
            )
            .add_systems(
                Update,
                (items_view::sync_loot, items_view::find_interact_target, items_view::item_keys, inventory_ui::auto_loot, inventory_ui::redraw_items, inventory_ui::item_clicks, inventory_ui::refusals, inventory_ui::drop_keys, inventory_ui::tooltip_text, inventory_ui::belt_hud).chain().after(follow_camera),
            )
            .add_systems(Update, (combat_view::sync_actor_views, combat_view::animate_viewmodel, combat_view::place_labels, combat_view::skill_hud, combat_view::style_projectiles, combat_view::storm_view).after(follow_camera))
            .add_systems(Update, (dungeon_view::floor_visibility.after(combat_view::sync_actor_views), dungeon_view::portal_markers, dungeon_view::runtime_portals))
            .add_systems(Update, (features_view::spawn_feature_views, features_view::animate_features).chain().after(combat_view::read_events))
            .add_systems(Update, (swing_door_view::spawn_swing_door_views, swing_door_view::swing_hinges).chain().after(combat_view::read_events))
            .add_systems(Update, (destroyable_view::spawn_destroyable_views, destroyable_view::update_destroyable_views, destroyable_view::update_pieces).chain().after(combat_view::read_events))
            .add_systems(Update, (mechanism_view::spawn_mechanism_views, mechanism_view::animate_mechanisms, mechanism_view::escape_locks, mechanism_view::lay_chains, mechanism_view::hide_baked_chains, zone_view::spawn_zone_views, zone_view::update_zone_views, zone_view::zone_messages, collapse_view::collapse_views, dragon_view::dragon_arrivals, dragon_view::fire_patch_views).chain().after(combat_view::read_events))
            .add_systems(Update, model_view::animate_model_bodies.after(combat_view::read_events))
            .add_systems(Update, (weapon_view::sync_view_weapon, weapon_view::dev_wield, weapon_view::dev_switch, weapon_view::dev_interact, weapon_view::dev_trigger, weapon_view::dev_summon_dragon, weapon_view::dev_attack))
            .add_systems(Update, (hud::update_art_hud, hud::stamina_bar, hud::prompt_back, hud::hit_marks, hud::buff_row, hud::flask_belt).after(combat_view::read_events))
            .add_systems(Update, (fp_body::sync_fp_body, fp_body::animate_fp_body, fp_body::place_fp_weapons, fp_body::held_torch_glow).chain().after(combat_view::read_events).after(panther_view::panther_camera))
            .add_systems(Update, props_view::dress_loot)
            .add_systems(Update, (projectile_view::thrown_weapons.after(combat_view::sync_actor_views), projectile_view::held_swords, throwables_view::flask_keys, (throwables_view::auto_torch, throwables_view::auto_drink, potion_view::potion_materials), throwables_view::flask_effects, throwables_view::arena_flasks, throwables_view::fire_ground, throwables_view::stone_walls, throwables_view::lit_torches))
            .add_systems(Update, materials::apply_overrides)
            .add_systems(Update, effects::show_effect.run_if(in_state(Screen::TestRoom)))
            .init_resource::<effects::FxEvents>()
            .init_resource::<effects::FxPrewarmQueue>()
            .init_resource::<loading::BlockingDataLoad>()
            .add_systems(Update, (effects::event_effects.after(combat_view::read_events), map_vfx::placed_effects, effects::attached_effects.after(combat_view::sync_actor_views), effects::summon_lines, effects::thief_effects, effects::monster_effects, effects::tick_effects, effects::tick_particles).chain().after(follow_camera))
            .add_systems(Update, effects::prewarm_fx)
            .add_systems(PostUpdate, model_view::drop_root_motion.after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate))
            .add_systems(PostUpdate, fp_body::hide_panther_head.after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate))
            .add_systems(Update, auto_shoot.run_if(|a: Res<AutoShoot>| a.0))
            .add_systems(Update, (monster_view::animate_monsters, monster_view::player_hud, monster_view::respawn.run_if(in_state(Screen::TestRoom)), monster_view::switch_class.run_if(in_state(Screen::TestRoom)), monster_view::class_panel).chain().after(combat_view::read_events))
            .add_systems(FixedUpdate, (send_input, run_sim_tick).chain());
    }
}

/// In the dungeon or the test room (not in a menu).
fn playing(screen: Res<State<Screen>>) -> bool {
    matches!(screen.get(), Screen::Run | Screen::TestRoom)
}

/// Wall time spent in [`SimTick`] since the last frame and the ticks run (read and reset by the bench).
#[derive(Resource, Debug, Default)]
pub struct SimTickTime {
    pub secs: f64,
    pub ticks: u32,
}

fn run_sim_tick(world: &mut World) {
    let start = std::time::Instant::now();
    // Each character's position before the tick, for drawing between ticks.
    let mut fresh = Vec::new();
    let mut q = world.query_filtered::<(Entity, &Position, Option<&mut PrevPosition>), With<CharacterState>>();
    for (e, p, prev) in q.iter_mut(world) {
        match prev {
            Some(mut v) => v.0 = p.0,
            None => fresh.push((e, PrevPosition(p.0))),
        }
    }
    for (e, p) in fresh {
        world.entity_mut(e).insert(p);
    }
    world.run_schedule(SimTick);
    let mut t = world.resource_mut::<SimTickTime>();
    t.secs += start.elapsed().as_secs_f64();
    t.ticks += 1;
}

struct SurfaceTextures {
    color: &'static str,
    normal: Option<&'static str>,
    /// `_ORM` (occlusion, roughness, metallic) or `_SRM` (specular, roughness, metallic). Both keep
    /// roughness in G and metallic in B, as Bevy reads them (channel check in FINDINGS).
    packed: Option<&'static str>,
}

const ROOT: &str = "textures_starter/Dungeonborne/Plugins/GameFeatures/TgfMaps/Content/SceneAssets/";

/// starter set.
fn surface_textures(s: Surface) -> SurfaceTextures {
    let t = |c, n, p| SurfaceTextures { color: c, normal: Some(n), packed: Some(p) };
    match s {
        Surface::Floor => t("FantasyDungeon2/Textures/Floor/T_Floor01_D.png", "FantasyDungeon2/Textures/Floor/T_Floor01_N.png", "FantasyDungeon2/Textures/Floor/T_Floor01_SRM.png"),
        Surface::Wall => t(
            "Ft_Gothic_castle/Textures/InteriorBrickWall/T_InWall_BC.png",
            "Ft_Gothic_castle/Textures/InteriorBrickWall/T_InWall_N.png",
            "Ft_Gothic_castle/Textures/InteriorBrickWall/T_InWall_ORM.png",
        ),
        Surface::MossWall => t("FantasyDungeon/Textures/Wall/brick2moss_D.png", "FantasyDungeon/Textures/Wall/brick2moss_N.png", "FantasyDungeon/Textures/Wall/brick2moss_SRM.png"),
        Surface::Ceiling => t(
            "Ft_Gothic_castle/Textures/basic_stone/T_BaseStone_BC.png",
            "Ft_Gothic_castle/Textures/basic_stone/T_BaseStone_N.png",
            "Ft_Gothic_castle/Textures/basic_stone/T_BaseStone_ORM.png",
        ),
        Surface::Wood => t("Ft_Gothic_castle/Textures/Wood/T_Wood_BC.png", "Ft_Gothic_castle/Textures/Wood/T_Wood_N.png", "Ft_Gothic_castle/Textures/Wood/T_Wood_ORM.png"),
        Surface::Metal => t("Ft_Gothic_castle/Textures/Metal/T_Metal_BC.png", "Ft_Gothic_castle/Textures/Metal/T_Metal_N.png", "Ft_Gothic_castle/Textures/Metal/T_Metal_ORM.png"),
        Surface::Door => t("FantasyDungeon/Textures/Door/Door1_D.png", "FantasyDungeon/Textures/Door/Door1_N.png", "FantasyDungeon/Textures/Door/Door1_SRM.png"),
    }
}

fn load_tex(assets: &AssetServer, path: &str, srgb: bool) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| {
            s.is_srgb = srgb;
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::linear()
            });
        })
        .load(format!("{ROOT}{path}"))
}

/// A material for one surface; `uv` scales the texture repeat.
fn surface_material(surface: Surface, uv: Vec2, assets: &AssetServer) -> StandardMaterial {
    let tex = surface_textures(surface);
    let packed = tex.packed.map(|p| load_tex(assets, p, false));
    StandardMaterial {
        base_color_texture: Some(load_tex(assets, tex.color, true)),
        normal_map_texture: tex.normal.map(|n| load_tex(assets, n, false)),
        // UE normal maps use the DirectX convention (green down); Bevy expects OpenGL (known convention).
        flip_normal_map_y: true,
        metallic_roughness_texture: packed.clone(),
        // Only `_ORM` carries occlusion in R; `_SRM` has specular there.
        occlusion_texture: tex.packed.filter(|p| p.ends_with("_ORM.png")).and(packed.clone()),
        perceptual_roughness: if packed.is_some() { 1.0 } else { 0.85 },
        metallic: if packed.is_some() { 1.0 } else { 0.0 },
        uv_transform: Affine2::from_scale(uv),
        ..default()
    }
}

fn material_for(piece: &LevelPiece, assets: &AssetServer) -> StandardMaterial {
    // Tile by the size of the box's largest face (the door shows its atlas once).
    let size = piece.half_extents * 0.02; // full size in metres
    let mut dims = [size.x, size.y, size.z];
    dims.sort_by(f32::total_cmp);
    let uv = if piece.surface == Surface::Door { Vec2::ONE } else { Vec2::new(dims[2] / TILE_M, dims[1] / TILE_M) };
    surface_material(piece.surface, uv, assets)
}

/// A textured box mesh for one level piece.
fn spawn_piece(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, assets: &AssetServer, piece: &LevelPiece) -> Entity {
    let size = to_bevy(piece.half_extents * 2.0).abs();
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))), MeshMaterial3d(materials.add(material_for(piece, assets))), Transform::from_translation(to_bevy(piece.center)), flow::LevelScoped)).id()
}

/// Loads the combat data and item tables once at startup.
/// Reads the game data on a background thread (about 20 s; the loading screen shows meanwhile, `loading`); what it
/// inserts is applied once it's done (`loading::finish_data_load`), and the boot waits for that.
/// With `BlockingDataLoad` (hidden dev runs, whose frame counts mean frames of the level) it reads it in place.
fn load_data(mut commands: Commands, blocking: Res<loading::BlockingDataLoad>) {
    if blocking.0 {
        let mut q = read_game_data();
        commands.append(&mut q);
        commands.insert_resource(loading::DataLoad { done: true, handle: None });
        return;
    }
    let handle = std::thread::Builder::new().name("game data".into()).spawn(read_game_data).map_err(|e| error!("game data: no loading thread ({e})")).ok();
    commands.insert_resource(loading::DataLoad { done: handle.is_none(), handle });
}

/// The game data's resources, as world commands.
fn read_game_data() -> bevy::ecs::world::CommandQueue {
    let mut q = bevy::ecs::world::CommandQueue::default();
    let insert = |q: &mut bevy::ecs::world::CommandQueue, r: Box<dyn FnOnce(&mut World) + Send>| q.push(move |w: &mut World| r(w));
    let (data, game) = CombatData::load();
    if let Some(game) = game {
        // The staffs' energy and spells, from the decoded weapon definitions and abilities.
        match db_sim::staffs::load_staff_db(game.tables) {
            Ok(db) => {
                info!("staffs: {} staff specs", db.0.len());
                insert(&mut q, Box::new(move |w| w.insert_resource(db)));
            }
            Err(e) => warn!("staffs: no staff specs ({e:#}); staffs swing only"),
        }
        // The relics' energy.
        match db_sim::weapons::load_relic_db(game.tables) {
            Ok(db) => {
                info!("relics: {} relic specs", db.0.len());
                insert(&mut q, Box::new(move |w| w.insert_resource(db)));
            }
            Err(e) => warn!("relics: no relic specs ({e:#}); no relic energy"),
        }
        // The rolled attributes' kill-growth stacks.
        match db_sim::gear_triggers::load_gear_triggers() {
            Ok(d) => insert(&mut q, Box::new(move |w| w.insert_resource(d))),
            Err(e) => warn!("gear triggers: no kill-growth values ({e:#})"),
        }
        // The alchemical flasks.
        let (flasks, left) = db_sim::throwables::load_flask_db(game.tables);
        info!("throwables: {} flasks; not modelled: {left:?}", flasks.0.len());
        insert(&mut q, Box::new(move |w| w.insert_resource(flasks)));
        // The legendary affixes' passives.
        match db_sim::affixes::load_affix_db(game.tables) {
            Ok((db, left)) => {
                info!("affixes: passives for {} legendary items; not modelled: {left:?}", db.0.len());
                insert(&mut q, Box::new(move |w| w.insert_resource(db)));
            }
            Err(e) => warn!("affixes: no legendary passives ({e:#})"),
        }
        // The swords' and daggers' combos, alone and paired.
        match db_sim::weapons::load_weapon_combos() {
            Ok((db, skipped)) => {
                info!("weapons: combos for {:?}; left out: {skipped:?}", db.combos.keys().collect::<Vec<_>>());
                insert(&mut q, Box::new(move |w| w.insert_resource(db)));
            }
            Err(e) => warn!("weapons: no combos ({e:#}); generic swings"),
        }
        insert(&mut q, Box::new(move |w| w.insert_resource(game)));
    }
    insert(&mut q, Box::new(move |w| w.insert_resource(data)));
    q
}

/// The test room (dev flags).
#[allow(clippy::too_many_arguments)]
fn build_test_room(mut commands: Commands, mut queue: ResMut<CommandQueue>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, assets: Res<AssetServer>, data: Option<Res<CombatData>>, test_monster: Res<TestMonster>, (test_door, test_prop): (Res<TestDoor>, Res<TestProp>), test_features: Res<TestFeatures>, start_look: Res<StartLook>, viewer: Res<ModelViewer>, mut graphs: ResMut<Assets<AnimationGraph>>, mut look: ResMut<Look>, mut spawns: ResMut<dungeon_view::MonsterSpawns>, (arena, test_class, dev_at): (Res<Arena>, Res<TestClass>, Res<dungeon_view::DevStartAt>)) {
    let Some(data) = data else { return };
    if arena.0 {
        let speed = data.speed(test_class.0.unwrap_or(db_formats::config::generated::EnRoleCareerType::Fighter));
        build_arena(&mut commands, &mut queue, &mut meshes, &mut materials, &assets, &data, &start_look, &mut look, &mut spawns, speed, dev_at.0.map(|p| p.0));
        return;
    }
    for piece in test_room() {
        queue.push(piece.command());
        spawn_piece(&mut commands, &mut meshes, &mut materials, &assets, &piece);
    }
    // `--test-features` starts at the west wall looking east over the doors, traps and altars.
    let spawn = if test_features.0 { Vec3::new(-700.0, 0.0, 92.0) } else { TEST_ROOM_SPAWN };
    if test_features.0 {
        look.yaw = 0.0;
    }
    if let Some(deg) = start_look.0 {
        look.yaw = deg.to_radians();
    }
    if let Some(deg) = start_look.1 {
        look.pitch = deg.to_radians();
    }
    queue.push(SimCommand::Spawn { kind: ActorKind::Player, pos: spawn, speed: data.fighter_speed });
    queue.push(SimCommand::Spawn { kind: ActorKind::Dummy, pos: TEST_ROOM_DUMMY, speed: 0.0 });
    if let Some(package) = &test_door.0 {
        let ahead = Vec2::from_angle(look.yaw) * 300.0;
        let at = Vec3::new(spawn.x + ahead.x, spawn.y + ahead.y, 0.0);
        match test_mechanism(package) {
            // A lever mechanism: the first placed one of that class with its levers, moved so its first lever
            // stands 1.5 m ahead (the mechanism on the floor). (With the mechanisms bound to it: the arena's left
            // stone door with the right.)
            Some((m, bound)) => {
                let lever = m.levers.first().map_or(m.pos, |l| l.pos);
                // (One with no lever, 6 m ahead: its own origin.)
                let k = if m.levers.is_empty() { 2.0 } else { 0.5 };
                // (The mechanism's origin 20 cm above the room's floor: a flap lying on the floor stays in sight.)
                let target = Vec3::new(spawn.x + ahead.x * k, spawn.y + ahead.y * k, lever.z - m.pos.z + 20.0);
                for b in bound.into_iter().chain([m.clone()]) {
                    queue.push(SimCommand::SpawnMechanism(Box::new(b.offset(target - lever))));
                }
            }
            None => queue.push(SimCommand::SpawnSwingDoor { package: package.clone(), pos: at, yaw: look.yaw.to_degrees(), scale: Vec3::ONE }),
        }
    }
    if let Some(class) = &test_prop.0 {
        let ahead = Vec2::from_angle(look.yaw) * 150.0;
        queue.push(SimCommand::SpawnDestroyable { class: class.clone(), pos: Vec3::new(spawn.x + ahead.x, spawn.y + ahead.y, 0.0), yaw: 0.0, scale: Vec3::ONE });
    }
    // The Goblin Axe, or the roster monster named by `--test-monster` (dev).
    let spec = test_monster.0.as_deref().and_then(db_sim::monsters::spec).unwrap_or(&db_sim::monsters::ROSTER[0]);
    // The Shadow King's tree takes its first phase only on waking (`BT_Shadow_King`'s `WakeUp` sequence); every
    // placed King lies dormant, so here too. Every placed Goblin Thief lies dormant too, seeing 900 cm (shots of its
    // wake).
    let opts = match spec.row {
        "ShadowKing" => db_sim::SpawnOpts { dormant: Some(true), ..Default::default() },
        "GoblinThief" => db_sim::SpawnOpts { dormant: Some(true), dormant_sight: Some(900.0), ..Default::default() },
        _ => db_sim::SpawnOpts::default(),
    };
    spawns.0.push_back((db_sim::level::TEST_ROOM_GOBLIN_YAW, if spec.row == "GoblinAxe" { db_sim::level::TEST_ROOM_GOBLIN_LOOT } else { spec.loot }, spec.row, opts));
    queue.push(SimCommand::Spawn { kind: ActorKind::Monster, pos: TEST_ROOM_GOBLIN, speed: data.monster_speed(spec.row) });
    queue.push(SimCommand::SpawnChest { pos: TEST_ROOM_CHEST, source_id: TEST_ROOM_CHEST_SOURCE });
    if let Some(path) = &viewer.path {
        // `--model`: the converted model on the floor a little left of the dummy, facing the player.
        let at = Vec3::new(-395.0, -348.0, 0.0);
        let d = to_bevy(spawn - at);
        model_view::spawn_model(&mut commands, &assets, &mut graphs, path, viewer.clip, to_bevy(at), (-d.z).atan2(d.x) + viewer.yaw.to_radians());
    }
    if test_features.0 {
        // In view from the spawn: an attack altar within reach, the other altars along the north side,
        // spikes, a blade, a fire statue and a door facing the player (floor points, cm).
        use db_sim::{AltarKind, TrapKind};
        queue.push(SimCommand::SpawnAltar { pos: Vec3::new(-540.0, -90.0, 0.0), kind: AltarKind::Attack });
        for (x, kind) in [(250.0, AltarKind::Defend), (400.0, AltarKind::Heal), (550.0, AltarKind::Clarity), (700.0, AltarKind::Speed)] {
            queue.push(SimCommand::SpawnAltar { pos: Vec3::new(x, 450.0, 0.0), kind });
        }
        queue.push(SimCommand::SpawnTrap { pos: Vec3::new(-250.0, -250.0, 0.0), kind: TrapKind::Spikes, yaw: 0.0, phase: 0.0 });
        queue.push(SimCommand::SpawnTrap { pos: Vec3::new(-150.0, 250.0, 0.0), kind: TrapKind::Blade, yaw: 0.0, phase: 0.0 });
        queue.push(SimCommand::SpawnTrap { pos: Vec3::new(-450.0, 200.0, 0.0), kind: TrapKind::FireStatue, yaw: -std::f32::consts::FRAC_PI_2, phase: 0.0 });
        queue.push(SimCommand::SpawnDoor { pos: Vec3::new(100.0, 0.0, 0.0), along_x: false });
    }
    // Torches: warm point lights below the ceiling (cm, sim coordinates).
    for (i, p) in [Vec3::new(-500.0, -300.0, 320.0), Vec3::new(500.0, 300.0, 320.0), Vec3::new(-500.0, 400.0, 320.0), Vec3::new(500.0, -350.0, 320.0)]
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            PointLight { color: Color::srgb(1.0, 0.72, 0.45), intensity: 400_000.0, range: 25.0, shadow_maps_enabled: false, ..default() },
            Transform::from_translation(to_bevy(p)),
            // Shadows by the quality preset (`settings::light_quality`).
            settings::Torch(i),
        ));
    }
}

/// The test arena (`--arena`): its walls, the player, a dummy, the chests and altars in the bay, and every monster
/// kind on its spot (the Shadow Dragon only comes with the Shadow King; one Mimic stands for its nine looks). The
/// monsters come back after their death (`Arena::respawn`, armed in `combat_view::read_events`).
#[allow(clippy::too_many_arguments)]
/// `at`: a dev spawn point (`--start-at`) instead of the arena's.
fn build_arena(commands: &mut Commands, queue: &mut CommandQueue, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, assets: &AssetServer, data: &CombatData, start_look: &StartLook, look: &mut Look, spawns: &mut dungeon_view::MonsterSpawns, speed: f32, at: Option<Vec3>) {
    use db_sim::level::{ARENA_DUMMY, ARENA_SPAWN, ARENA_SPAWN_YAW, arena, arena_spot};
    for piece in arena() {
        queue.push(piece.command());
        spawn_piece(commands, meshes, materials, assets, &piece);
    }
    look.yaw = start_look.0.map_or(ARENA_SPAWN_YAW, f32::to_radians);
    look.pitch = start_look.1.map_or(0.0, f32::to_radians);
    queue.push(SimCommand::Spawn { kind: ActorKind::Player, pos: at.unwrap_or(ARENA_SPAWN), speed });
    queue.push(SimCommand::Spawn { kind: ActorKind::Dummy, pos: ARENA_DUMMY, speed: 0.0 });
    // Chests along the bay's west side (wood, iron, gold, special: their loot tables), altars along the east.
    for (i, source_id) in [174, 175, 180, 160, 159, 155, 172].into_iter().enumerate() {
        queue.push(SimCommand::SpawnChest { pos: Vec3::new(-700.0 - i as f32 * 250.0, -2700.0, 0.0), source_id });
    }
    use db_sim::AltarKind;
    for (i, kind) in [AltarKind::Attack, AltarKind::Defend, AltarKind::Heal, AltarKind::Clarity, AltarKind::Speed].into_iter().enumerate() {
        queue.push(SimCommand::SpawnAltar { pos: Vec3::new(700.0 + i as f32 * 350.0, -2700.0, 0.0), kind });
    }
    let regulars = db_sim::monsters::ROSTER.iter().filter(|s| s.row != "ShadowDragon" && !(s.row.starts_with("Mimic0") && s.row != "Mimic01"));
    let spots = regulars.enumerate().map(|(i, s)| (s, arena_spot(i, false))).chain(db_sim::monsters::BOSSES.iter().enumerate().map(|(i, s)| (s, arena_spot(i, true))));
    for (spec, pos) in spots {
        // As in the test room: the Shadow King and the Goblin Thief lie dormant until someone comes near.
        let opts = match spec.row {
            "ShadowKing" => db_sim::SpawnOpts { dormant: Some(true), ..Default::default() },
            "GoblinThief" => db_sim::SpawnOpts { dormant: Some(true), dormant_sight: Some(900.0), ..Default::default() },
            _ => db_sim::SpawnOpts::default(),
        };
        // Facing the bay.
        spawns.0.push_back((-std::f32::consts::FRAC_PI_2, spec.loot, spec.row, opts));
        queue.push(SimCommand::Spawn { kind: ActorKind::Monster, pos, speed: data.monster_speed(spec.row) });
    }
    // A cool moonlight over the whole field and warm lights over the bay (no shadows: light to run).
    commands.spawn((DirectionalLight { color: Color::srgb(0.75, 0.8, 1.0), illuminance: 2500.0, shadow_maps_enabled: false, ..default() }, Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.1, 0.4, 0.0))));
    for p in [Vec3::new(-1200.0, -2500.0, 800.0), Vec3::new(1200.0, -2500.0, 800.0), Vec3::new(0.0, -1500.0, 800.0)] {
        commands.spawn((PointLight { color: Color::srgb(1.0, 0.72, 0.45), intensity: 2_000_000.0, range: 40.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(to_bevy(p))));
    }
}

fn setup_camera(mut commands: Commands, fov: Res<FovSetting>, occlusion: Res<OcclusionCullingOn>, space: Res<PlaySpace>, top: Res<TopDown>, windows: Query<&Window, With<PrimaryWindow>>) {
    if let (Some(seed), Some(floor)) = (space.0, top.0) {
        let (projection, transform) = dungeon_view::top_down_camera(seed, floor);
        commands.spawn((Camera3d::default(), projection, transform, PlayerCamera));
        return;
    }
    let aspect = windows.single().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection { fov: vertical_fov(fov.0, aspect), near: 0.05, ..default() }),
            Transform::from_translation(to_bevy(TEST_ROOM_SPAWN + Vec3::Z * EYE_OFFSET)),
            PlayerCamera,
            // HDR: effects blend in linear HDR and are tonemapped once after, as in the game; tonemapped one by one,
            // an emissive of 18 at opacity 0.1 (Flame Shock's floor fire) came out as almost nothing.
            bevy::camera::Hdr,
        ))
        .id();
    // Two-phase GPU occlusion culling: meshes hidden behind last frame's depth aren't drawn. It needs a
    // depth prepass.
    if occlusion.0 {
        commands.entity(camera).insert((bevy::core_pipeline::prepass::DepthPrepass, bevy::render::occlusion_culling::OcclusionCulling));
    }
}

fn grab_cursor(mouse: Res<ButtonInput<MouseButton>>, keys: Res<ButtonInput<KeyCode>>, open: Res<InventoryOpen>, paused: Res<pause::Paused>, mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    let Ok(mut c) = cursor.single_mut() else { return };
    // The inventory window and the pause menu keep the cursor free for clicking.
    if mouse.just_pressed(MouseButton::Left) && !open.0 && !paused.0 {
        c.grab_mode = CursorGrabMode::Locked;
        c.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}

fn cursor_locked(cursor: &Query<&CursorOptions, With<PrimaryWindow>>) -> bool {
    cursor.single().is_ok_and(|c| c.grab_mode != CursorGrabMode::None)
}

fn mouse_look(motion: Res<AccumulatedMouseMotion>, cursor: Query<&CursorOptions, With<PrimaryWindow>>, settings: Res<settings::Settings>, mut look: ResMut<Look>) {
    if !cursor_locked(&cursor) {
        return;
    }
    let k = MOUSE_SENSITIVITY * settings.sensitivity;
    look.yaw = (look.yaw + motion.delta.x * k).rem_euclid(2.0 * PI);
    look.pitch = (look.pitch - motion.delta.y * k).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
}

/// Space jump; left click light attack (on release), hold left click heavy attack, Q/E skills; F is
/// interact (`items_view`). The click that grabs the cursor does not attack (`grab_cursor` runs first in
/// the same frame, so check last frame's state). The staffs whose holders have no torch in hand (a held
/// torch takes the mouse buttons).
type UnlitStaffs<'w, 's> = Query<'w, 's, &'static db_sim::StaffState, Without<db_sim::throwables::HeldTorch>>;

#[allow(clippy::too_many_arguments)]
fn collect_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<settings::Keys>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut was_locked: Local<bool>,
    // Seconds left click has been held, and whether the heavy attack already went out.
    mut held: Local<Option<(f32, bool)>>,
    mut pending: ResMut<PendingInput>,
    (player, index, staffs, class_kits): (Res<LocalPlayer>, Res<SimIndex>, UnlitStaffs, Query<&db_sim::ClassKit>),
    mut aiming: ResMut<ground_aim::GroundAim>,
) {
    use settings::Bind;
    pending.jump |= keys.just_pressed(binds.get(Bind::Jump));
    pending.reload |= keys.just_pressed(binds.get(Bind::Reload));
    pending.switch |= keys.just_pressed(binds.get(Bind::SwapWeapons)) || (*was_locked && scroll.delta.y.abs() > 0.0);
    let (q, e) = (binds.get(Bind::SkillQ), binds.get(Bind::SkillE));
    // (CONFIRMED data): Pyroblast charges from one press of Q (`bReleaseChargeByConfirm`); Q again or a left click
    // fires it (`ReleaseInputTags`), a right click cancels it (`CancelInputTags`). Force of Nature the same on E.
    let confirming = player.0.and_then(|id| index.get(id)).and_then(|e| class_kits.get(e).ok()).and_then(|k| {
        let c = k.charge.filter(|c| CONFIRMED_CHARGES.contains(&c.0))?;
        k.slots.iter().position(|s| *s == c.0)
    });
    if let Some(slot) = confirming
        && *was_locked
    {
        let (key, other) = if slot == 0 { (q, e) } else { (e, q) };
        if keys.just_pressed(key) || mouse.just_pressed(MouseButton::Left) {
            pending.release = Some(slot);
        }
        if mouse.just_pressed(MouseButton::Right) {
            pending.cancel = Some(slot);
        }
        *held = None;
        if keys.just_pressed(other) {
            pending.skill = Some(1 - slot);
        }
        *was_locked = cursor_locked(&cursor);
        return;
    }
    // A staff: tap = its combo, hold = its spell (`InputTag.Weapon.Charge.ChargeLeft`), let go ends or
    // releases it; a staff without a spell (ice) only taps.
    let staff = player.0.and_then(|id| index.get(id)).and_then(|e| staffs.get(e).ok()).and_then(db_sim::StaffState::element);
    let spell = staff.is_some_and(|e| e != db_sim::StaffElement::Ice);
    if *was_locked && mouse.just_pressed(MouseButton::Left) {
        *held = Some((0.0, false));
    }
    if let Some((secs, fired)) = held.as_mut() {
        *secs += time.delta_secs();
        if !*fired && *secs >= HEAVY_HOLD_S && mouse.pressed(MouseButton::Left) && (staff.is_none() || spell) {
            if spell {
                pending.cast = Some(true);
            } else {
                pending.attack = Some(AttackKind::Heavy);
            }
            *fired = true;
        }
        if !mouse.pressed(MouseButton::Left) {
            if !*fired {
                pending.attack = Some(AttackKind::Light);
            } else if spell {
                pending.cast = Some(false);
            } else {
                pending.charge_release = true;
            }
            *held = None;
        }
    }
    // Blizzard (`GA_TGF_Lich_Q`, CONFIRMED bytecode): holding Q shows the selection indicator, letting go casts
    // the storm there; a right click drops the aim. Q while the storm channels unfolds it, as before.
    let kit = player.0.and_then(|id| index.get(id)).and_then(|e| class_kits.get(e).ok());
    let places = kit.is_some_and(|k| k.slots[0] == SkillKind::Blizzard && k.blizzard.is_none());
    if keys.just_pressed(q) && places {
        aiming.0 = true;
    } else if keys.just_pressed(q) {
        pending.skill = Some(0);
    }
    if aiming.0 && (!places || mouse.just_pressed(MouseButton::Right)) {
        aiming.0 = false;
    } else if aiming.0 && !keys.pressed(q) {
        aiming.0 = false;
        pending.skill = Some(0);
    }
    if keys.just_pressed(e) {
        pending.skill = Some(1);
    }
    // (A just-started Pyroblast or Force of Nature keeps charging when its key comes up.)
    let confirms = |slot: usize| player.0.and_then(|id| index.get(id)).and_then(|e| class_kits.get(e).ok()).is_some_and(|k| CONFIRMED_CHARGES.contains(&k.slots[slot]));
    if keys.just_released(q) && !confirms(0) {
        pending.release = Some(0);
    }
    if keys.just_released(e) && !confirms(1) {
        pending.release = Some(1);
    }
    *was_locked = cursor_locked(&cursor);
}

/// Charged skills let go by a second press or a left click, not by letting the key up (`bReleaseChargeByConfirm`).
const CONFIRMED_CHARGES: [SkillKind; 2] = [SkillKind::Pyroblast, SkillKind::ForceOfNature];

/// Input → commands, once per sim tick. Move, sprint and crouch on the bound keys (default W/S/A/D, Shift,
/// Ctrl), right mouse block; aim follows the camera.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn send_input(
    (keys, binds): (Res<ButtonInput<KeyCode>>, Res<settings::Keys>),
    mouse: Res<ButtonInput<MouseButton>>,
    mut look: ResMut<Look>,
    player: Res<LocalPlayer>,
    auto: Res<AutoAttack>,
    auto_skill: Res<AutoSkill>,
    auto_walk: Res<AutoWalk>,
    auto_fight: Res<AutoFight>,
    bot_move: Res<autorun::BotMove>,
    actors: Query<(&db_sim::Kind, &Position, &db_sim::Health, Option<&Melee>)>,
    tick: Res<db_sim::TickCount>,
    index: Res<SimIndex>,
    melee: Query<&Melee>,
    kits: Query<(Option<&SkillKit>, Option<&db_sim::ClassKit>)>,
    (mut pending, mut queue, staffs, auto_cast, frames): (ResMut<PendingInput>, ResMut<CommandQueue>, UnlitStaffs, Res<staff_view::AutoCast>, Res<bevy::diagnostic::FrameCount>),
    (mut cast_from, auto_release, auto_heavy): (Local<Option<u64>>, Res<skill_vfx::AutoRelease>, Option<Res<skill_vfx::AutoHeavy>>),
) {
    let Some(id) = player.0 else { return };
    let axis = |pos: KeyCode, neg: KeyCode| f32::from(u8::from(keys.pressed(pos))) - f32::from(u8::from(keys.pressed(neg)));
    use settings::Bind;
    let (fwd, mut right) = (axis(binds.get(Bind::Forward), binds.get(Bind::Back)), axis(binds.get(Bind::Right), binds.get(Bind::Left)));
    if auto_walk.0 {
        right = if (tick.0 / 45).is_multiple_of(2) { 1.0 } else { -1.0 };
    }
    let fighting = auto_fight.on && auto_fight.for_seconds.is_none_or(|s| (tick.0 as f32) * db_sim::DT < s);
    // Stopped (`--auto-fight-for`): it still turns to the nearest monster, but stands.
    let bot = if auto_fight.on { monster_view::auto_fight(id, &index, &actors, &mut look, auto_fight.passive).filter(|_| fighting) } else { None };
    let (s, c) = look.yaw.sin_cos();
    let dir = bot.map(|b| b.0).or(bot_move.0).unwrap_or(Vec2::new(c, s) * fwd + Vec2::new(-s, c) * right);
    queue.push(SimCommand::SetMoveIntent { id, dir: dir.clamp_length_max(1.0).extend(0.0) });
    queue.push(SimCommand::SetMoveFlags { id, sprint: keys.pressed(binds.get(Bind::Sprint)) || auto_walk.1, crouch: keys.pressed(binds.get(Bind::Crouch)) });
    queue.push(SimCommand::SetAim { id, yaw: look.yaw, pitch: look.pitch });
    // With a staff the right button is its charge (recharge; its press cancels a thunder aim), not a guard. (A held
    // torch takes both buttons whatever the weapon: thrown, raised.)
    let staff = index.get(id).and_then(|e| staffs.get(e).ok()).and_then(db_sim::StaffState::element);
    if staff.is_some() {
        // (Not while a Pyroblast charges: its right click cancels it.)
        let charging = index.get(id).and_then(|e| kits.get(e).ok()).and_then(|k| k.1).and_then(|k| k.charge).is_some();
        queue.push(SimCommand::StaffRecharge { id, on: (mouse.pressed(MouseButton::Right) && !charging) || auto_cast.2 });
        // `--auto-cast`: from its start frame on, hold 1.5 s, let go 1.5 s.
        if auto_cast.0 && frames.0 >= auto_cast.1 {
            cast_from.get_or_insert(tick.0);
        }
        if let Some(on) = pending.cast.take().or_else(|| cast_from.map(|t0| (tick.0 - t0) % 90 < 45)) {
            queue.push(SimCommand::StaffCast { id, on });
        }
    } else {
        pending.cast = None;
        // (`--auto-recharge` holds it here too: a crossbow's aim, shots.)
        queue.push(SimCommand::SetBlock { id, on: mouse.pressed(MouseButton::Right) || bot.is_some_and(|b| b.2) || auto_cast.2 });
    }
    if let Some((_, Some(kind), _)) = bot {
        pending.attack = Some(kind);
    }
    if std::mem::take(&mut pending.jump) {
        queue.push(SimCommand::Jump { id });
    }
    if std::mem::take(&mut pending.charge_release) {
        queue.push(SimCommand::ReleaseCharge { id });
    }
    if std::mem::take(&mut pending.reload) {
        queue.push(SimCommand::Reload { id });
    }
    if std::mem::take(&mut pending.switch) {
        queue.push(SimCommand::SwitchWeaponPair { id });
    }
    let e = index.get(id);
    let idle = e.and_then(|e| melee.get(e).ok()).is_some_and(|m| m.swing.is_none() && m.queued.is_none());
    // `--auto-heavy` lets go of its charge as a player does (it releases at its `MinChargeTime`: a charge with no
    // `MaxChargeTime`, the mace and shield's, never went off).
    if auto_heavy.as_ref().is_some_and(|a| a.0) && e.and_then(|e| melee.get(e).ok()).is_some_and(|m| m.swing.as_ref().is_some_and(|s| s.by == db_sim::HitSource::Swing(AttackKind::Heavy))) {
        queue.push(SimCommand::ReleaseCharge { id });
    }
    let (kit, class_kit) = e.and_then(|e| kits.get(e).ok()).unwrap_or((None, None));
    let slots = kit.map(|k| k.slots).or(class_kit.map(|k| k.slots));
    // The bot uses its Q skill whenever it is ready.
    // (A charged skill held by the bot goes off by itself at full charge.)
    let q_ready = kit.is_some_and(|k| k.whirlwind.is_none() && k.cooldown(k.slots[0]) == 0) || class_kit.is_some_and(|k| k.preparing.is_none() && k.charge.is_none() && k.blizzard.is_none() && k.oracle.is_none() && k.aura.is_none() && !k.panther && k.cooldown(k.slots[0]) == 0);
    // The Fighter's kit too (its Charge was never pressed, so `--vfx-shots` waited for nothing).
    let e_ready = kit.is_some_and(|k| k.charge.is_none() && k.cooldown(k.slots[1]) == 0) || class_kit.is_some_and(|k| k.preparing.is_none() && k.charge.is_none() && k.cooldown(k.slots[1]) == 0 && (!k.panther || k.cooldown(db_sim::SkillKind::ShadowPounce) == 0));
    let idle = idle && frames.0 >= auto_skill.2;
    let bot_skill = if auto_skill.0 && idle && q_ready { Some(0) } else if auto_skill.1 && idle && e_ready { Some(1) } else { None };
    if let Some(skill) = pending.skill.take().or(bot_skill).and_then(|slot| slots.map(|s| s[slot])) {
        queue.push(SimCommand::UseSkill { id, skill });
    } else if let Some(kind) = pending.attack.take().or((auto.0 && idle && frames.0 >= auto_skill.2).then_some(AttackKind::Light)).or((auto_heavy.is_some_and(|a| a.0) && idle).then_some(AttackKind::Heavy)) {
        queue.push(SimCommand::Attack { id, kind });
    }
    // The bot lets its Pyroblast go at the full charge (it no longer goes off by itself), unless
    // `--auto-release` times it. Its Flying Swords once all four are out.
    let bot_release = (auto_skill.0
        && auto_release.0.is_none()
        && class_kit.and_then(|k| k.charge).is_some_and(|c| {
            (c.0 == db_sim::SkillKind::Pyroblast && c.1 > (db_sim::overrides::class_skills::CHARGE_STAGES[1] / db_sim::DT) as u32)
                || (c.0 == db_sim::SkillKind::FlyingSwords && db_sim::class_skills::swords_summoned(c.1, db_sim::class_skills::FLYING_SWORDS_MAX) == db_sim::class_skills::FLYING_SWORDS_MAX)
        }))
    .then_some(0)
    // Its Force of Nature after a second's aim.
    .or((auto_skill.1 && auto_release.0.is_none() && class_kit.and_then(|k| k.charge).is_some_and(|c| c.0 == db_sim::SkillKind::ForceOfNature && c.1 as f32 * db_sim::DT >= 1.0)).then_some(1));
    if let Some(skill) = pending.release.take().or(bot_release).and_then(|slot| slots.map(|s| s[slot])) {
        queue.push(SimCommand::ReleaseSkill { id, skill });
    }
    if let Some(skill) = pending.cancel.take().and_then(|slot| slots.map(|s| s[slot])) {
        queue.push(SimCommand::CancelSkill { id, skill });
    }
}

/// `--auto-shoot`: a projectile at the nearest living monster every 1.2 s (straight, homing, thrown).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn auto_shoot(time: Res<Time>, mut next: Local<f32>, mut n: Local<u32>, mut wizard: Local<Option<Vec<(f32, f32)>>>, player: Res<LocalPlayer>, index: Res<SimIndex>, actors: Query<(&db_sim::Kind, &Position, Option<&db_sim::Health>, Option<&db_sim::Combatant>)>, mut queue: ResMut<CommandQueue>) {
    *next -= time.delta_secs();
    if *next > 0.0 {
        return;
    }
    *next = 1.2;
    let Some((me, Ok((_, my_pos, _, Some(c))))) = player.0.and_then(|id| index.get(id).map(|e| (id, actors.get(e)))) else { return };
    let from = my_pos.0 + Vec3::Z * EYE_OFFSET;
    let Some((target, to)) = index
        .iter()
        .filter_map(|(id, e)| actors.get(e).ok().map(|a| (id, a)))
        .filter(|(_, (k, _, h, _))| k.0 == ActorKind::Monster && h.is_some_and(|h| h.current > 0.0))
        .map(|(id, (_, p, _, _))| (id, p.0))
        .min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)))
    else {
        return;
    };
    let wizard = wizard.get_or_insert_with(|| db_sim::projectiles::load_wizard_q_speed().unwrap_or_else(|_| vec![(0.0, 600.0)]));
    let base = db_sim::ProjectileSpec {
        speed: vec![(0.0, 1500.0)],
        lifetime: 3.0,
        radius: 10.0,
        turn_rate: 0.0,
        gravity: 0.0,
        attack: c.attack,
        factor: 1.0,
        element: db_sim::combat::Element::Fire,
        by: db_sim::HitSource::Swing(AttackKind::Light),
        pierce: 0,
        chain: 0,
        chain_range: 0.0,
        pull: 0.0,
        on_hit: None,
        burst: 0.0,
        fuse: false,
        ignore_guard: false,
    knockback: None,
    };
    let (spec, dir) = match *n % 3 {
        0 => (base, to - from),
        // Homing, launched 40 degrees off to show the turn.
        1 => (db_sim::ProjectileSpec { speed: wizard.clone(), turn_rate: 150.0, ..base }, Quat::from_rotation_z(0.7) * (to - from)),
        // Thrown: aimed up so it arcs (speed 900 cm/s, 980 cm/s² drop).
        _ => (db_sim::ProjectileSpec { speed: vec![(0.0, 900.0)], gravity: 980.0, ..base }, (to - from).normalize() + Vec3::Z * 0.35),
    };
    *n += 1;
    queue.push(SimCommand::SpawnProjectile { owner: me, spec: Box::new(spec), pos: from + (to - from).normalize_or_zero() * 45.0, dir, target: Some(target) });
}

/// `--observe`: everything under the camera (the first-person body, weapons, their effects) hidden, every frame
/// after the view systems set it (a `Visible` child would show under a hidden parent).
fn hide_view_model(cams: Query<Entity, With<PlayerCamera>>, children: Query<&Children>, mut vis: Query<&mut Visibility>) {
    for cam in &cams {
        for c in children.iter_descendants(cam) {
            if let Ok(mut v) = vis.get_mut(c)
                && *v != Visibility::Hidden
            {
                *v = Visibility::Hidden;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn follow_camera(look: Res<Look>, top: Res<TopDown>, observe: Res<Observe>, player: Res<LocalPlayer>, index: Res<SimIndex>, actors: Query<(&Position, Option<&PrevPosition>, &CharacterState, Option<&db_sim::Health>)>, mut cam: Query<&mut Transform, With<PlayerCamera>>, (fixed, time): (Res<Time<Fixed>>, Res<Time>), mut eye_z: Local<Option<f32>>) {
    let Ok(mut t) = cam.single_mut() else { return };
    if top.0.is_some() {
        return;
    }
    if let Some((pos, prev, st, health)) = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()) {
        // Dead: the view drops to the floor.
        let eye = if health.is_some_and(|h| h.current <= 0.0) { EYE_OFFSET_DEAD } else if st.crouched { EYE_OFFSET_CROUCHED } else { EYE_OFFSET };
        let at = drawn(pos, prev, &fixed) + Vec3::Z * eye;
        // On the ground the eye's height eases after the floor (a step up shows as a short rise, not a jolt);
        // in the air, after a teleport or a big change it follows at once.
        let z = match *eye_z {
            Some(z) if st.on_ground && (at.z - z).abs() < 60.0 => z + (at.z - z) * (1.0 - (-time.delta_secs() / EYE_SMOOTH_S).exp()),
            _ => at.z,
        };
        *eye_z = Some(z);
        t.translation = to_bevy(Vec3::new(at.x, at.y, z));
    }
    let (sy, cy) = look.yaw.sin_cos();
    let (sp, cp) = look.pitch.sin_cos();
    let forward_sim = Vec3::new(cy * cp, sy * cp, sp);
    // Raised by a quarter of the distance, so the player's stand-in sits low in the view, not over what it faces.
    if let Some(m) = observe.0 {
        t.translation += to_bevy(Vec3::new(0.0, 0.0, m * 25.0) - forward_sim * m * 100.0);
    }
    t.look_to(to_bevy(forward_sim).normalize(), Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_conversion() {
        assert_eq!(to_bevy(Vec3::new(100.0, 200.0, 300.0)), Vec3::new(1.0, 3.0, 2.0));
    }

    #[test]
    fn fov_conversion() {
        // 90° horizontal at 1:1 is 90° vertical; 110° at 16:9 is about 77.6° vertical.
        assert!((vertical_fov(90.0, 1.0).to_degrees() - 90.0).abs() < 1e-3);
        assert!((vertical_fov(110.0, 16.0 / 9.0).to_degrees() - 77.6).abs() < 0.1);
    }
}

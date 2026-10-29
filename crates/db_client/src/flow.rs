//! The game loop around the sim: screens (title, stash, run, result), the save file, and entering and
//! leaving a run. The profile is meta-game state outside the sim (`db_sim::profile`): it goes into a
//! run with `SetCharacterItems` and comes back from `RunEnded`.

use std::collections::VecDeque;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use db_formats::config::generated::EnRoleCareerType;
use db_sim::attributes::{STARTING_SLOTS, class_sheet, class_type_id, starting_gear};
use db_sim::profile::Profile;
use db_sim::{CommandQueue, GameData, InventoryCommand, RunClock, RunOutcome, RunStatus, SimCommand, SimEvent, SimId, SimIndex};

use crate::combat_view::{CombatData, FrameEvents, LocalPlayer};
use crate::dungeon_view::{self, MonsterSpawns, PlaySpace, TopDown};
use crate::{InventoryOpen, Look};

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    /// First frame: data is loaded, then `StartScreen` is entered.
    #[default]
    Boot,
    Title,
    Stash,
    Run,
    Result,
    /// The test room (dev flags).
    TestRoom,
    /// A real crypt map, view only (dev `--map`).
    MapView,
}

/// Where the app starts after loading (dev flags choose the test room or a run).
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartScreen(pub Screen);

impl Default for StartScreen {
    fn default() -> Self {
        StartScreen(Screen::Title)
    }
}

/// The save file (`None`: keep the profile in memory only, e.g. `--dungeon` dev runs).
#[derive(Resource, Debug, Clone, Default)]
pub struct SavePath(pub Option<PathBuf>);

/// Start from an empty profile even if the save file exists (scripted checks; the file is still written).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct FreshProfile(pub bool);

/// Dev: run length override in seconds (tests the clock running out).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct RunLength(pub Option<u32>);

/// Dev: no monsters in the dungeon.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct Peaceful(pub bool);

/// The loaded profile; `rev` changes whenever it does (menus redraw on it).
#[derive(Resource, Debug, Default)]
pub struct ProfileRes {
    pub profile: Profile,
    pub rev: u32,
}

impl ProfileRes {
    pub fn changed(&mut self, save: &SavePath) {
        self.rev += 1;
        if let Some(path) = &save.0
            && let Err(e) = self.profile.save(path)
        {
            error!("saving the profile: {e:#}");
        }
    }
}

/// The character command for the player about to spawn (sent with its id by `read_events`).
#[derive(Resource, Debug, Default)]
pub struct PendingCharacter(pub Option<InventoryCommand>);

/// How the last run ended (for the result screen).
#[derive(Resource, Debug, Default, Clone)]
pub struct LastRun {
    pub outcome: Option<RunOutcome>,
    pub items: usize,
    pub gold: u64,
    pub floor: u32,
    pub seconds: u32,
    /// The experience earned and the levels gained.
    pub exp: u32,
    pub levels: u32,
}

/// Screenshots the client asks for (file stem); devtools takes them and may then exit.
#[derive(Resource, Debug, Default)]
pub struct ShotRequests {
    pub queue: VecDeque<String>,
    pub exit_after: bool,
}

/// Menu and bot actions (buttons and `--auto-run` go through the same path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Play,
    /// The test arena (every class to switch between, every monster, the weapon tables).
    Arena,
    PickClass(EnRoleCareerType),
    NextClass,
    /// Next skill choice for slot 0 (Q) or 1 (E).
    NextSkill(u8),
    EnterCrypt,
    Continue,
    Quit,
    /// Pause menu.
    Resume,
    Settings,
    Abandon,
    QuitToTitle,
    /// Settings screen.
    SettingsDefaults,
    SettingsBack,
    /// Stash screen: the next playable map.
    NextMap,
}

/// Where the run's end leads instead of the result screen (quit to title from the pause menu).
#[derive(Resource, Debug, Default)]
pub struct AfterRun(pub Option<Screen>);

#[derive(Resource, Debug, Default)]
pub struct PendingAction(pub Option<Action>);

/// Entities that belong to the current level (removed when a run ends).
#[derive(Component)]
pub struct LevelScoped;

/// Run timer and floor (top centre).
#[derive(Component)]
pub(crate) struct RunHud;

/// HUD shown only while playing (crosshair, bars, belt, prompts).
#[derive(Component)]
pub struct GameHud;

/// Loads the profile, then leaves `Boot` for the start screen.
pub(crate) fn boot(start: Res<StartScreen>, mut save: ResMut<SavePath>, fresh: Res<FreshProfile>, mut profile: ResMut<ProfileRes>, mut next: ResMut<NextState<Screen>>) {
    if let Some(path) = save.0.clone().filter(|_| !fresh.0) {
        match Profile::load(&path) {
            Ok(Some(p)) => {
                info!("profile loaded from {}: {} gold, {} runs", path.display(), p.gold, p.runs);
                profile.profile = p;
            }
            Ok(None) => info!("no profile at {}; starting a new one", path.display()),
            // Never overwrite a file we couldn't read: drop the path for this session.
            Err(e) => {
                error!("profile unreadable ({e:#}); playing without saving");
                save.0 = None;
            }
        }
    }
    next.set(start.0);
}

pub(crate) fn release_cursor(mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut c) = cursor.single_mut() {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}

/// The character the profile sends into a run: its items, or the class starting gear when empty-handed.
fn character_command(data: &GameData, profile: &Profile) -> Option<InventoryCommand> {
    let career = profile.career.unwrap_or(EnRoleCareerType::Fighter);
    let t = data.tables;
    let ty = class_type_id(t, career).ok()?;
    let sheet = Box::new(class_sheet(t, ty, profile.level.max(1) as i32).ok()?);
    if profile.needs_starting_gear() {
        let worn = starting_gear(t, ty).ok()?.into_iter().filter(|g| STARTING_SLOTS.contains(&g.slot)).map(|g| (g.slot, g.item_id)).collect();
        Some(InventoryCommand::SetCharacter { id: SimId(0), career, sheet, worn })
    } else {
        Some(InventoryCommand::SetCharacterItems { id: SimId(0), career, sheet, inventory: Box::new(profile.character.clone()) })
    }
}

/// `enter_run`'s map resources: the loaded map, the escape delay, the loader, the chosen map, the dev start.
type RunMap<'w> = (ResMut<'w, dungeon_view::CurrentCrypt>, Res<'w, dungeon_view::EscapeDelay>, ResMut<'w, CryptLoader>, Res<'w, PlayMap>, Res<'w, dungeon_view::DevStartAt>, Res<'w, crate::StartLook>);

/// Builds the dungeon and queues the character for the player spawn.
#[allow(clippy::too_many_arguments)]
pub(crate) fn enter_run(
    mut commands: Commands,
    mut queue: ResMut<CommandQueue>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    space: Res<PlaySpace>,
    top: Res<TopDown>,
    peaceful: Res<Peaceful>,
    length: Res<RunLength>,
    profile: Res<ProfileRes>,
    data: Option<Res<CombatData>>,
    game: Option<Res<GameData>>,
    mut pending: ResMut<PendingCharacter>,
    mut spawns: ResMut<MonsterSpawns>,
    mut look: ResMut<Look>,
    (mut crypt, escape_delay, mut loader, play, start_at, start_look): RunMap,
) {
    let Some(data) = data else { return };
    // `--dungeon <seed>` plays the generator's layout; otherwise each run gets the next seed.
    let seed = space.0.unwrap_or(u64::from(profile.profile.runs) * 7919 + 1);
    // Runs play the chosen real map; the generator if it can't load.
    let want = chosen_map(&profile.profile, &play);
    if space.0.is_none() && crypt.0.as_ref().is_none_or(|m| m.def.key != want.key) {
        // Loading in the background since launch or the map change; waits here only if it hasn't finished.
        let started = std::time::Instant::now();
        let handle = loader.0.take().filter(|(key, _)| *key == want.key).map(|(_, h)| h);
        let loaded = handle.map_or_else(|| load_map(want), |h| h.join().unwrap_or_else(|_| Err("the map load thread panicked".into())));
        match loaded {
            Ok(maps) => {
                info!("map {} ready (waited {:.1} s, {} colliders)", want.title, started.elapsed().as_secs_f32(), maps.collision.iter().map(|c| c.instances.len()).sum::<usize>());
                crypt.0 = Some(std::sync::Arc::new(maps));
            }
            Err(e) => {
                warn!("map {} not loaded ({e}); runs use the generated dungeon", want.title);
                crypt.0 = None;
            }
        }
    }
    pending.0 = game.as_ref().and_then(|g| character_command(g, &profile.profile));
    // The Q/E skills picked on the stash screen.
    if let Some(s) = profile.profile.skills {
        commands.insert_resource(crate::SkillLoadout(Some(s)));
    }
    let career = profile.profile.career.unwrap_or(EnRoleCareerType::Fighter);
    let speed = data.speed(career);
    let seconds = length.0.unwrap_or(data.run_seconds);
    let dungeon = match crypt.0.as_ref().filter(|_| space.0.is_none()) {
        Some(maps) => dungeon_view::build_crypt(&mut commands, &mut queue, &mut meshes, &mut materials, &assets, &data, maps, seed, peaceful.0, escape_delay.0, speed, seconds, &mut spawns, &mut look, start_at.0, start_at.1),
        None => dungeon_view::build(&mut commands, &mut queue, &mut meshes, &mut materials, &assets, &data, seed, top.0.is_some(), peaceful.0, speed, seconds, &mut spawns, &mut look),
    };
    // `--look <yaw>,<pitch>`'s pitch from a `--start-at` too (dev: the Shadow Dragon flies 30 m up).
    if let (Some(deg), Some(_)) = (start_look.1, start_at.0) {
        look.pitch = deg.to_radians();
    }
    commands.insert_resource(crate::autorun::CurrentDungeon(Some(dungeon)));
}

/// The chosen map's data (layout, collision with its shapes built, walkable grids), loading on a background
/// thread, so a run doesn't build thousands of trimeshes on the main thread; tagged with the map's key.
#[derive(Resource, Default)]
pub(crate) struct CryptLoader(Option<(&'static str, std::thread::JoinHandle<Result<db_sim::crypt::CryptMaps, String>>)>);

/// The entry cover: a black screen with the map's name from the stash screen's Enter until the cells
/// around the player have spawned, so the map's load (the sim's colliders, about 1.8 s on the
/// castle) and the first streaming hitches aren't seen.
#[derive(Resource, Debug, Default)]
pub(crate) struct EntryCover {
    /// Enter was pressed: the cover shows first and the run starts once it has been drawn.
    pub(crate) requested: bool,
    /// Up in a run, until the nearby cells are in.
    pub(crate) on: bool,
    entity: Option<Entity>,
    frames: u32,
    waited: f32,
    /// Frames in a row shorter than [`COVER_CALM_MS`].
    calm: u32,
    /// The sim tick the run started at: the sim runs [`COVER_SIM_TICKS`] ticks (the player placed, the camera
    /// on it) and then waits behind the cover (`pause::apply_pause`).
    pub(crate) tick: u64,
}

/// Sim ticks run behind the entry cover before the sim waits for it to lift.
pub(crate) const COVER_SIM_TICKS: u64 = 3;

/// The entry cover stays up at least this long in the run (s; the camera reaches the player first) and at
/// most this long (ours).
const COVER_MIN_S: f32 = 1.0;
const COVER_MAX_S: f32 = 20.0;
/// The cover lifts once no nearby cell is waiting and this many frames in a row took under
/// [`COVER_CALM_MS`] (the meshes and textures uploaded; ours).
const COVER_CALM_FRAMES: u32 = 15;
const COVER_CALM_MS: f32 = 33.0;

/// Shows the entry cover, starts the run once it has been drawn, and lifts it when the cells around the
/// camera are in ([`crate::map_view::StreamBacklog`]) and frames are calm again. GPU uploads aren't capped
/// while it is up.
#[allow(clippy::too_many_arguments)]
pub(crate) fn entry_cover(
    mut commands: Commands,
    mut cover: ResMut<EntryCover>,
    screen: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
    backlog: Res<crate::map_view::StreamBacklog>,
    (profile, play, tick): (Res<ProfileRes>, Res<PlayMap>, Res<db_sim::TickCount>),
    real: Res<Time<Real>>,
    (mut upload, mut overrides, assets): (ResMut<bevy::render::render_asset::RenderAssetBytesPerFrame>, ResMut<crate::materials::MaterialOverrides>, Res<AssetServer>),
) {
    if cover.requested {
        if cover.entity.is_none() {
            let title = chosen_map(&profile.profile, &play).title;
            let node = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
            cover.entity = Some(commands.spawn((node, BackgroundColor(Color::BLACK), GlobalZIndex(1_000), children![(Text::new(format!("Entering {title}...")), crate::fonts::heading(40.0), TextColor(Color::srgb(0.85, 0.78, 0.6)))])).id());
            cover.frames = 0;
        }
        cover.frames += 1;
        // Drawn before the run's first, long frame.
        if cover.frames >= 3 {
            cover.requested = false;
            cover.on = true;
            cover.waited = 0.0;
            cover.calm = 0;
            cover.tick = tick.0;
            upload.max_bytes = None;
            next.set(Screen::Run);
        }
        return;
    }
    if !cover.on {
        return;
    }
    // Real time: virtual time is held while the cover is up (`pause::apply_pause`).
    cover.waited += real.delta_secs();
    let textures = overrides.still_loading(&assets);
    cover.calm = if real.delta_secs() * 1000.0 < COVER_CALM_MS && backlog.0 == 0 && textures == 0 { cover.calm + 1 } else { 0 };
    if *screen.get() != Screen::Run || cover.waited > COVER_MAX_S || (cover.waited > COVER_MIN_S && cover.calm >= COVER_CALM_FRAMES) {
        if let Some(e) = cover.entity.take() {
            commands.entity(e).try_despawn();
        }
        cover.on = false;
        upload.max_bytes = Some(crate::map_view::UPLOAD_BYTES_PER_FRAME);
        info!("entry cover lifted after {:.1} s ({} cells, {textures} override textures waiting)", cover.waited, backlog.0);
    }
}

/// Dev: play this map (a [`db_sim::crypt::MapDef`] key, `--play-map`) whatever the profile says.
#[derive(Resource, Debug, Default, Clone)]
pub struct PlayMap(pub Option<String>);

/// The map the next run plays: `--play-map`, else the profile's pick, else the first of
/// [`db_sim::crypt::MAPS`] (the Gothic Castle, choice).
pub(crate) fn chosen_map(profile: &Profile, play: &PlayMap) -> db_sim::crypt::MapDef {
    play.0.as_deref().or(profile.map.as_deref()).and_then(db_sim::crypt::map_def).unwrap_or(db_sim::crypt::MAPS[0])
}

fn load_map(def: db_sim::crypt::MapDef) -> Result<db_sim::crypt::CryptMaps, String> {
    db_sim::crypt::CryptMaps::load_prepared(def).map_err(|e| format!("{e:#}"))
}

/// Keeps the chosen map loading in the background (from the first screen, and again after a map change on
/// the stash screen) unless runs use the generated dungeon (`--dungeon`).
pub(crate) fn keep_map_loading(space: Res<dungeon_view::PlaySpace>, profile: Res<ProfileRes>, play: Res<PlayMap>, crypt: Res<dungeon_view::CurrentCrypt>, mut loader: ResMut<CryptLoader>) {
    if space.0.is_some() {
        return;
    }
    let want = chosen_map(&profile.profile, &play);
    if crypt.0.as_ref().is_some_and(|m| m.def.key == want.key) || loader.0.as_ref().is_some_and(|(key, _)| *key == want.key) {
        return;
    }
    // A load of another map still running finishes on its own and is dropped.
    loader.0 = std::thread::Builder::new().name(format!("{} load", want.key)).spawn(move || load_map(want)).ok().map(|h| (want.key, h));
}

/// Clears the sim and every level entity.
pub(crate) fn exit_run(mut commands: Commands, mut queue: ResMut<CommandQueue>, level: Query<Entity, With<LevelScoped>>, mut player: ResMut<LocalPlayer>, mut open: ResMut<InventoryOpen>, mut spawns: ResMut<MonsterSpawns>) {
    queue.push(SimCommand::ClearWorld);
    for e in &level {
        commands.entity(e).despawn();
    }
    player.0 = None;
    open.0 = false;
    spawns.0.clear();
}

/// `RunEnded` for the local player: fold it into the profile, save, show the result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_ended(
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    status: Query<&RunStatus>,
    clock: Res<RunClock>,
    save: Res<SavePath>,
    mut profile: ResMut<ProfileRes>,
    mut last: ResMut<LastRun>,
    mut next: ResMut<NextState<Screen>>,
    mut after: ResMut<AfterRun>,
    (play, data): (Res<PlayMap>, Option<Res<GameData>>),
) {
    let Some(me) = player.0 else { return };
    for e in &events.0 {
        let SimEvent::RunEnded { id, outcome, kept } = e else { continue };
        if *id != me {
            continue;
        }
        let gold_before = profile.profile.gold;
        profile.profile.end_run(*outcome, kept.as_deref());
        let map_id = chosen_map(&profile.profile, &play).map_id;
        let exp = data.as_ref().map_or(0, |d| db_sim::profile::run_exp(d.tables, map_id, clock.elapsed_secs()));
        // (Left through a portal: the gear's experience boost.)
        let exp = db_sim::profile::boosted_exp(exp, *outcome, &profile.profile.character);
        let levels = data.as_ref().map_or(0, |d| profile.profile.add_exp(d.tables, exp));
        let floor = index.get(me).and_then(|e| status.get(e).ok()).map_or(if *outcome == RunOutcome::Extracted { 1 } else { 0 }, |s| s.floor);
        *last = LastRun {
            outcome: Some(*outcome),
            items: kept.as_ref().map_or(0, |k| k.items().count()),
            gold: profile.profile.gold - gold_before,
            floor,
            seconds: clock.ticks_left.map_or(0, |t| (t as f32 * db_sim::DT) as u32),
            exp,
            levels,
        };
        info!("run ended: {outcome:?}, kept {} items, +{} gold", last.items, last.gold);
        profile.changed(&save);
        next.set(after.0.take().unwrap_or(Screen::Result));
    }
}

pub(crate) fn setup_run_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(26.0), ..default() },
        TextColor(Color::srgb(0.95, 0.9, 0.75)),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, left: percent(50.0), top: px(18.0), margin: UiRect::left(px(-200.0)), width: px(400.0), ..default() },
        RunHud,
        GameHud,
    ));
}

/// Floor and time left: until the floor's death zone closes (`GetShrinkingCountdownTime`), else the run's.
pub(crate) fn run_hud(clock: Res<RunClock>, tick: Res<db_sim::TickCount>, player: Res<LocalPlayer>, index: Res<SimIndex>, status: Query<&RunStatus>, zones: Query<&db_sim::death_zone::DeathZone>, mut hud: Query<&mut Text, With<RunHud>>) {
    use db_sim::death_zone::ZoneState;
    let Ok(mut t) = hud.single_mut() else { return };
    let (Some(ticks), Some(s)) = (clock.ticks_left, player.0.and_then(|id| index.get(id)).and_then(|e| status.get(e).ok())) else {
        t.0 = String::new();
        return;
    };
    let zone = zones.iter().find(|z| z.floor == s.floor && z.state != ZoneState::Idle);
    let secs = match zone {
        Some(z) if matches!(z.state, ZoneState::Shrinking | ZoneState::Ended) => {
            t.0 = format!("Floor {}   closing", s.floor + 1);
            return;
        }
        Some(z) => z.closes_in(tick.0).ceil() as u32,
        None => (ticks as f32 * db_sim::DT).ceil() as u32,
    };
    t.0 = format!("Floor {}   {:02}:{:02}", s.floor + 1, secs / 60, secs % 60);
}

/// The playing HUD only while playing.
pub(crate) fn hud_visibility(screen: Res<State<Screen>>, mut hud: Query<&mut Visibility, With<GameHud>>) {
    let show = matches!(screen.get(), Screen::Run | Screen::TestRoom);
    for mut v in &mut hud {
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}

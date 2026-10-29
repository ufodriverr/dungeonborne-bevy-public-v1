//! Unattended checks: `--screenshot <png> --frames <n>` saves one frame and exits; `--bench
//! <seconds>` measures frame times (vsync off) and writes `_data/shots/bench.txt`.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::PresentMode;

/// What the command line asked for.
#[derive(Debug, Default, Clone)]
pub struct DevArgs {
    pub screenshot: Option<PathBuf>,
    pub frames: u32,
    pub bench: Option<f64>,
    /// Seconds ignored before the bench measures (default [`BENCH_WARMUP_S`]); e.g. past the crypt's load
    /// in an `--auto-run`.
    pub bench_warmup: Option<f64>,
    /// Show the window during a screenshot/bench run (only when a hidden run can't do the job).
    pub visible: bool,
    /// The player attacks whenever idle (shows combat in screenshots).
    pub auto_attack: bool,
    /// The player uses Whirlwind whenever it is ready.
    pub auto_skill: bool,
    /// The player also uses its E skill whenever it is ready (`--auto-skill-e`).
    pub auto_skill_e: bool,
    /// With a staff: hold its spell 1.5 s, let go 1.5 s, over and over (`--auto-cast`).
    pub auto_cast: bool,
    /// The render frame `--auto-cast` starts at (`--auto-cast-at <frame>`).
    pub auto_cast_at: u32,
    /// The render frame `--auto-skill` / `--auto-skill-e` start at (`--auto-skill-at <frame>`: casts once the room shows).
    pub auto_skill_at: u32,
    /// Hold the right button all the time: a staff's recharge, another weapon's guard or a crossbow's aim
    /// (`--auto-recharge`, shots).
    pub auto_recharge: bool,
    /// The player strafes left and right.
    pub auto_walk: bool,
    /// Open the pause menu after this many frames on a playing screen.
    pub pause_at: Option<u32>,
    /// Settings file to use and save (hidden runs otherwise use the defaults and save nothing).
    pub settings: Option<String>,
    /// Window resolution override, `WxH` (one of the settings' resolutions).
    pub resolution: Option<(u32, u32)>,
    /// Quality preset override (`low`, `medium`, `high`).
    pub quality: Option<db_client::Quality>,
    /// Open the settings screen at start.
    pub settings_open: bool,
    /// ... and sprints while doing it (`--auto-sprint`, implies `--auto-walk`).
    pub auto_sprint: bool,
    /// The player fights the nearest monster (and respawns after dying); `--auto-passive` walks up and
    /// lets it attack (tests the player's death and respawn).
    pub auto_fight: bool,
    pub auto_passive: bool,
    pub auto_fight_for: Option<f32>,
    /// Scripted check: hit the dummy, open the chest, pick up, wear the new weapon, hit again.
    pub auto_loot: bool,
    /// Play in the seeded dungeon instead of the test room.
    pub dungeon: Option<u64>,
    /// Orthographic view from above of dungeon floor 1 or 2 (layout screenshots).
    pub topdown: Option<u32>,
    /// Scripted check: play the whole loop, screenshots `_data/shots/<prefix>_<step>.png`, exit.
    pub auto_run: Option<String>,
    /// No monsters in the dungeon.
    pub peaceful: bool,
    /// No GPU occlusion culling (to compare).
    pub no_occlusion: bool,
    /// Runs play this map (`castle`, `crypt`) whatever the profile says.
    pub play_map: Option<String>,
    /// Save file to use instead of `_data/save/profile.json`.
    pub profile: Option<PathBuf>,
    /// Start in the test room.
    pub test_room: bool,
    /// Run length override (seconds).
    pub run_seconds: Option<u32>,
    /// Crypt escape portals open after at most this many seconds (checks).
    pub escape_delay: Option<f32>,
    /// The player's start on a real map's floor 1, map-local cm, yaw degrees (`--start-at x,y,z[,yaw]`).
    pub start_at: Option<(bevy::math::Vec3, f32)>,
    /// `--start-floor <n>`: the run starts at floor n's arrival (1-based on the command line).
    pub start_floor: u32,
    /// Write the camera and player every frame of a run to this CSV.
    pub trace_camera: Option<PathBuf>,
    /// Test room: this roster monster (curve row) instead of the Goblin Axe.
    pub test_monster: Option<String>,
    /// A swing door ahead in the test room (`--test-door <package>`).
    pub test_door: Option<String>,
    /// A breakable prop ahead in the test room (`--test-prop <class>`).
    pub test_prop: Option<String>,
    /// Q and E skills for the local player, e.g. `WarCry,Inspire`.
    pub loadout: Option<[db_client::SkillKind; 2]>,
    /// Test room: fire projectiles at the nearest monster.
    pub auto_shoot: bool,
    /// Test room: play as this class (`swordmaster`, `rogue`, ...).
    pub class: Option<String>,
    /// Test room: a door, the three traps and the five altars.
    pub test_features: bool,
    /// Start in the test arena instead (every monster, respawns, chests).
    pub arena: bool,
    /// Observer camera this many metres behind the eye (AR2).
    pub observe: Option<f32>,
    /// Test room: a converted model (path under the asset root) and the clip it plays.
    pub model: Option<String>,
    pub clip: usize,
    pub yaw: f32,
    /// Show this item's weapon mesh in view instead of the equipped one.
    pub weapon: Option<i32>,
    /// Give the player this item and wear it in the right hand, and this one in the left (`--wield <r>,<l>`).
    pub wield: Option<i32>,
    pub wield_left: Option<i32>,
    /// Switch weapon sets at this render frame (`--switch-at <frame>`).
    pub switch_at: Option<u32>,
    /// Switch class at this render frame, as its F key in the test room (`--class-at <frame>:<class>`).
    pub class_at: Option<(u32, db_client::EnRoleCareerType)>,
    /// Press interact on the current target at this render frame (`--interact-at <frame>`).
    pub interact_at: Option<u32>,
    /// Turn every mechanism on at this render frame (`--trigger-at <frame>`).
    pub trigger_at: Option<u32>,
    /// Every Shadow King casts `Attack8`, the Shadow Dragon, at this render frame (`--summon-dragon-at <frame>`).
    pub summon_dragon_at: Option<u32>,
    /// The player swings once at this render frame (`--attack-at <frame>`: breaking a prop on screen).
    pub attack_at: Option<u32>,
    /// View a real crypt map (`01`/`02`), from above or from a player start.
    pub map: Option<String>,
    pub map_top: bool,
    pub map_start: usize,
    pub map_walk: bool,
    /// `--shot-every <s>`: timed shots during an `--auto-run` run.
    pub shot_every: Option<f32>,
    /// Test room: starting look direction (degrees), and pitch (`--look <yaw>,<pitch>`, shots).
    pub look: Option<f32>,
    pub look_pitch: Option<f32>,
    /// Keep the glTF materials as converted, without the overrides (before/after shots).
    pub raw_materials: bool,
    /// Test room: play one effect from `effects.json` in front of the camera.
    pub effect: Option<String>,
    /// Release a held charged skill after this many seconds (`--auto-release`, stage-1 shots).
    pub auto_release: Option<f32>,
    /// `--auto-heavy`: the bot charges heavy attacks.
    pub auto_heavy: bool,
    /// `--auto-flask <1-5>`: the bot throws that flask key's flask whenever it can.
    pub auto_flask: Option<usize>,
    /// `--auto-torch <frame>`: the torch drawn, raised and thrown from that frame.
    pub auto_torch: Option<u32>,
    /// `--auto-drink <frame>`: a healing potion given and drunk then.
    pub auto_drink: Option<u32>,
    /// `--press-arena`: the title's Test Arena pressed after a second (checks).
    pub press_arena: bool,
    /// Screenshots timed from a skill start (`--vfx-shots <name>:<skip>:<s1,s2...>`).
    pub vfx_shots: Option<db_client::VfxShots>,
    /// Hold the first-person body in one clip, optionally at a fraction of it.
    pub fp_pose: Option<(String, Option<f32>)>,
    /// Off-hand item shown with `--weapon`.
    pub offhand: Option<i32>,
}

impl DevArgs {
    pub fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut d = DevArgs { frames: 120, ..default() };
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let mut val = || it.next().cloned().ok_or_else(|| anyhow::anyhow!("{a} needs a value"));
            match a.as_str() {
                "--screenshot" => d.screenshot = Some(PathBuf::from(val()?)),
                "--frames" => d.frames = val()?.parse()?,
                "--bench" => d.bench = Some(val()?.parse()?),
                "--bench-warmup" => d.bench_warmup = Some(val()?.parse()?),
                "--visible" => d.visible = true,
                "--auto-attack" => d.auto_attack = true,
                "--auto-skill" => d.auto_skill = true,
                "--auto-skill-e" => d.auto_skill_e = true,
                "--auto-cast" => d.auto_cast = true,
                "--auto-skill-at" => d.auto_skill_at = val()?.parse()?,
                "--auto-cast-at" => (d.auto_cast, d.auto_cast_at) = (true, val()?.parse()?),
                "--auto-recharge" => d.auto_recharge = true,
                "--auto-walk" => d.auto_walk = true,
                "--pause-at" => d.pause_at = Some(val()?.parse()?),
                "--settings" => d.settings = Some(val()?),
                "--settings-open" => d.settings_open = true,
                "--resolution" => {
                    let v = val()?;
                    let (w, h) = v.split_once('x').ok_or_else(|| anyhow::anyhow!("--resolution expects WxH, got `{v}`"))?;
                    d.resolution = Some((w.parse()?, h.parse()?));
                }
                "--quality" => {
                    d.quality = Some(match val()?.to_lowercase().as_str() {
                        "low" => db_client::Quality::Low,
                        "medium" => db_client::Quality::Medium,
                        "high" => db_client::Quality::High,
                        q => anyhow::bail!("--quality expects low, medium or high, got `{q}`"),
                    })
                }
                "--auto-sprint" => (d.auto_walk, d.auto_sprint) = (true, true),
                "--auto-fight" => d.auto_fight = true,
                "--auto-passive" => d.auto_passive = true,
                "--auto-fight-for" => d.auto_fight_for = Some(val()?.parse()?),
                "--auto-loot" => d.auto_loot = true,
                "--dungeon" => d.dungeon = Some(val()?.parse()?),
                "--topdown" => d.topdown = Some(val()?.parse()?),
                "--auto-run" => d.auto_run = Some(val()?),
                "--peaceful" => d.peaceful = true,
                "--shot-every" => d.shot_every = Some(val()?.parse()?),
                "--no-occlusion" => d.no_occlusion = true,
                "--play-map" => {
                    let key = val()?;
                    if db_client::map_def(&key).is_none() {
                        anyhow::bail!("unknown map `{key}` (castle, crypt)");
                    }
                    d.play_map = Some(key);
                }
                "--profile" => d.profile = Some(PathBuf::from(val()?)),
                "--test-room" => d.test_room = true,
                "--run-seconds" => d.run_seconds = Some(val()?.parse()?),
                "--escape-delay" => d.escape_delay = Some(val()?.parse()?),
                "--start-floor" => d.start_floor = val()?.parse::<u32>()?.saturating_sub(1),
                "--start-at" => {
                    let v: Vec<f32> = val()?.split(',').map(str::parse).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() >= 3, "--start-at x,y,z[,yaw]");
                    d.start_at = Some((bevy::math::Vec3::new(v[0], v[1], v[2]), v.get(3).copied().unwrap_or(0.0)));
                }
                "--trace-camera" => d.trace_camera = Some(PathBuf::from(val()?)),
                "--test-monster" => d.test_monster = Some(val()?),
                "--test-door" => d.test_door = Some(val()?),
                "--test-prop" => d.test_prop = Some(val()?),
                "--loadout" => d.loadout = Some(parse_loadout(&val()?)?),
                "--auto-shoot" => d.auto_shoot = true,
                "--class" => d.class = Some(val()?),
                "--test-features" => d.test_features = true,
                "--arena" => d.arena = true,
                // The title screen: the default (kept for older shortcuts).
                "--title" => {}
                "--observe" => d.observe = Some(val()?.parse()?),
                "--model" => d.model = Some(val()?),
                "--clip" => d.clip = val()?.parse()?,
                "--yaw" => d.yaw = val()?.parse()?,
                "--weapon" => d.weapon = Some(val()?.parse()?),
                "--wield" => {
                    let v = val()?;
                    let (right, left) = v.split_once(',').map_or((v.as_str(), None), |(r, l)| (r, Some(l)));
                    d.wield = Some(right.parse()?);
                    d.wield_left = left.map(str::parse).transpose()?;
                }
                "--switch-at" => d.switch_at = Some(val()?.parse()?),
                "--class-at" => {
                    let v = val()?;
                    let (frame, class) = v.split_once(':').ok_or_else(|| anyhow::anyhow!("--class-at <frame>:<class>"))?;
                    d.class_at = Some((frame.parse()?, parse_class(class).ok_or_else(|| anyhow::anyhow!("unknown class `{class}`"))?));
                }
                "--interact-at" => d.interact_at = Some(val()?.parse()?),
                "--trigger-at" => d.trigger_at = Some(val()?.parse()?),
                "--summon-dragon-at" => d.summon_dragon_at = Some(val()?.parse()?),
                "--attack-at" => d.attack_at = Some(val()?.parse()?),
                "--map" => d.map = Some(val()?),
                "--map-top" => d.map_top = true,
                "--map-start" => d.map_start = val()?.parse()?,
                "--map-walk" => d.map_walk = true,
                "--look" => {
                    let v = val()?;
                    let (yaw, pitch) = v.split_once(',').map_or((v.as_str(), None), |(y, p)| (y, Some(p)));
                    d.look = Some(yaw.parse()?);
                    d.look_pitch = pitch.map(str::parse).transpose()?;
                }
                "--raw-materials" => d.raw_materials = true,
                "--effect" => d.effect = Some(val()?),
                "--auto-release" => d.auto_release = Some(val()?.parse()?),
                "--auto-heavy" => d.auto_heavy = true,
                "--auto-flask" => d.auto_flask = Some(val()?.parse()?),
                "--auto-torch" => d.auto_torch = Some(val()?.parse()?),
                "--auto-drink" => d.auto_drink = Some(val()?.parse()?),
                "--press-arena" => d.press_arena = true,
                "--vfx-shots" => d.vfx_shots = Some(db_client::VfxShots::parse(&val()?).ok_or_else(|| anyhow::anyhow!("--vfx-shots <name>:<skip>:<s1,s2,...>[:<monster attack>]"))?),
                "--fp-pose" => {
                    let v = val()?;
                    d.fp_pose = Some(match v.split_once(':') {
                        Some((role, f)) => (role.to_string(), Some(f.parse()?)),
                        None => (v, None),
                    });
                }
                "--offhand" => d.offhand = Some(val()?.parse()?),
                _ => anyhow::bail!(
                    "unknown argument `{a}` (expected --screenshot <png> [--frames <n>], --bench <seconds> [--bench-warmup <s>], [--visible], [--auto-attack], [--auto-heavy], [--auto-flask <1-5>], [--auto-skill], [--auto-walk], [--auto-sprint], [--pause-at <frames>], [--settings <json>], [--settings-open], [--resolution <WxH>], [--quality <low|medium|high>], [--auto-fight], [--auto-passive], [--auto-fight-for <s>], [--auto-loot], [--dungeon <seed>], [--topdown <floor>], [--auto-run <prefix>], [--peaceful], [--no-occlusion], [--play-map <castle|crypt>], [--profile <json>], [--test-room], [--arena], [--title], [--class-at <frame>:<class>], [--observe <m>], [--run-seconds <n>], [--escape-delay <s>], [--start-floor <n>], [--trace-camera <csv>], [--test-monster <curve row>], [--loadout <Q>,<E>], [--auto-shoot], [--class <name>], [--test-features], [--model <path> [--clip <n>] [--yaw <deg>]], [--weapon <item id>], [--wield <item id>], [--map <01|02> [--map-top] [--map-start <n>] [--map-walk]], [--look <deg>], [--raw-materials], [--effect <key>], [--auto-release <s>], [--auto-cast [--auto-cast-at <frame>]], [--auto-skill-at <frame>], [--auto-recharge], [--vfx-shots <name>:<skip>:<s1,s2,...>[:<monster attack>]], [--fp-pose <clip>[:<0..1>]], [--offhand <item id>])"
                ),
            }
        }
        // Started with no arguments (the exe double-clicked): the title screen, where Play starts the game and Test Arena
        // the arena.
        Ok(d)
    }

    /// Screenshot and bench runs use a hidden window unless `--visible` is given.
    pub fn hidden(&self) -> bool {
        (self.screenshot.is_some() || self.bench.is_some() || self.auto_run.is_some() || self.vfx_shots.is_some()) && !self.visible
    }

    /// Test-room dev flags start there; `--dungeon` starts a run; otherwise the title screen.
    pub fn start_screen(&self) -> db_client::Screen {
        let test_room = self.test_room || self.arena || self.effect.is_some() || self.fp_pose.is_some() || self.test_features || self.model.is_some() || self.test_monster.is_some() || self.test_door.is_some() || self.test_prop.is_some() || self.auto_shoot || self.class.is_some() || self.auto_attack || self.auto_skill || self.auto_skill_e || self.auto_cast || self.auto_recharge || self.auto_walk || self.auto_fight || self.auto_passive || self.auto_loot;
        if self.map.is_some() {
            db_client::Screen::MapView
        } else if test_room {
            db_client::Screen::TestRoom
        } else if self.dungeon.is_some() || self.start_at.is_some() || self.start_floor > 0 {
            db_client::Screen::Run
        } else {
            db_client::Screen::Title
        }
    }

    /// Bench mode runs without vsync so the numbers show the real headroom.
    pub fn present_mode(&self) -> PresentMode {
        if self.bench.is_some() { PresentMode::AutoNoVsync } else { PresentMode::AutoVsync }
    }
}

#[derive(Resource, Debug)]
struct ShotPlan {
    path: PathBuf,
    frames: u32,
    seen: u32,
    requested: bool,
    exit_in: Option<u32>,
}

/// Seconds of frames ignored before measuring (shader compilation, asset loading).
const BENCH_WARMUP_S: f64 = 3.0;

#[derive(Resource, Debug)]
struct BenchPlan {
    seconds: f64,
    warmup: f64,
    elapsed: f64,
    frames_ms: Vec<f64>,
}

pub struct DevToolsPlugin(pub DevArgs);

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(db_client::AutoAttack(self.0.auto_attack)).insert_resource(db_client::AutoSkill(self.0.auto_skill, self.0.auto_skill_e, self.0.auto_skill_at))
            .insert_resource(db_client::AutoCast(self.0.auto_cast, self.0.auto_cast_at, self.0.auto_recharge))
            .insert_resource(db_client::AutoWalk(self.0.auto_walk, self.0.auto_sprint))
            .insert_resource(db_client::PauseAt(self.0.pause_at))
            .insert_resource(db_client::SettingsOverride { resolution: self.0.resolution, quality: self.0.quality, open: self.0.settings_open })
            .insert_resource(db_client::AutoFight { on: self.0.auto_fight || self.0.auto_passive, passive: self.0.auto_passive, for_seconds: self.0.auto_fight_for })
            .insert_resource(db_client::AutoLoot(self.0.auto_loot))
            .insert_resource(db_client::PlaySpace(self.0.dungeon))
            // The map's top view keeps the camera where `map_view` puts it (`TopDown` stops the follow).
            .insert_resource(db_client::TopDown(if self.0.map_top { Some(1) } else { self.0.topdown }))
            .insert_resource(db_client::StartScreen(self.0.start_screen()))
            .insert_resource(db_client::Peaceful(self.0.peaceful))
            .insert_resource(db_client::OcclusionCullingOn(!self.0.no_occlusion))
            .insert_resource(db_client::PlayMap(self.0.play_map.clone()))
            .insert_resource(db_client::RunLength(self.0.run_seconds))
            .insert_resource(db_client::EscapeDelay(self.0.escape_delay))
            .insert_resource(db_client::DevStartAt(self.0.start_at, self.0.start_floor))
            .insert_resource(db_client::CameraTrace(self.0.trace_camera.clone()))
            .insert_resource(db_client::TestMonster(self.0.test_monster.clone()))
            .insert_resource(db_client::TestDoor(self.0.test_door.clone()))
            .insert_resource(db_client::TestProp(self.0.test_prop.clone()))
            .insert_resource(db_client::TestFeatures(self.0.test_features))
            .insert_resource(db_client::Arena(self.0.arena))
            .insert_resource(db_client::Observe(self.0.observe))
            .insert_resource(db_client::ModelViewer { path: self.0.model.clone(), clip: self.0.clip, yaw: self.0.yaw })
            .insert_resource(db_client::ViewWeapon(self.0.weapon))
            .insert_resource(db_client::DevWield(self.0.wield, self.0.wield_left))
            .insert_resource(db_client::DevSwitchAt(self.0.switch_at))
            .insert_resource(db_client::DevClassAt(self.0.class_at))
            // Hidden runs count frames from the level: the game data is read before the first frame (`DB_ASYNC_LOAD`:
            // as when played, to shoot the loading screen).
            .insert_resource(db_client::BlockingDataLoad(self.0.hidden() && std::env::var_os("DB_ASYNC_LOAD").is_none()))
            .insert_resource(db_client::DevInteractAt(self.0.interact_at))
            .insert_resource(db_client::DevTriggerAt(self.0.trigger_at))
            .insert_resource(db_client::DevSummonDragonAt(self.0.summon_dragon_at))
            .insert_resource(db_client::DevAttackAt(self.0.attack_at))
            .insert_resource(db_client::StartLook(self.0.look, self.0.look_pitch))
            .insert_resource(db_client::RawMaterials(self.0.raw_materials))
            .insert_resource(db_client::ShowEffect(self.0.effect.clone()))
            .insert_resource(db_client::AutoRelease(self.0.auto_release))
            .insert_resource(db_client::AutoHeavy(self.0.auto_heavy))
            .insert_resource(db_client::AutoFlask(self.0.auto_flask))
            .insert_resource(db_client::AutoTorch(self.0.auto_torch))
            .insert_resource(db_client::AutoDrink(self.0.auto_drink))
            .insert_resource(db_client::PressArena(self.0.press_arena))
            .insert_resource(db_client::FpPose(self.0.fp_pose.clone()))
            .insert_resource(db_client::ViewOffhand(self.0.offhand))
            .insert_resource(db_client::MapViewer { level: self.0.map.clone(), top: self.0.map_top, start: self.0.map_start, walk: self.0.map_walk })
            .insert_resource(db_client::SkillLoadout(self.0.loadout))
            .insert_resource(db_client::AutoShoot(self.0.auto_shoot))
            .insert_resource(db_client::TestClass(self.0.class.as_deref().and_then(parse_class)))
            .insert_resource(db_client::AutoRun(self.0.auto_run.is_some()))
            .insert_resource(db_client::AutoRunShots(self.0.shot_every))
            .insert_resource(db_client::FreshProfile(self.0.auto_run.is_some()));
        // Dev runs never touch the real save: `--profile` picks a file, otherwise test-room and
        // `--dungeon` runs keep the profile in memory.
        if let Some(p) = &self.0.profile {
            app.insert_resource(db_client::SavePath(Some(p.clone())));
        } else if self.0.start_screen() != db_client::Screen::Title || self.0.auto_run.is_some() {
            app.insert_resource(db_client::SavePath(None));
        }
        if let Some(prefix) = &self.0.auto_run {
            app.insert_resource(AutoShots { prefix: prefix.clone(), in_flight: false, exit_in: None }).add_systems(Last, auto_shots);
        } else if let Some(plan) = &self.0.vfx_shots {
            app.insert_resource(plan.clone()).insert_resource(AutoShots { prefix: "vfx".into(), in_flight: false, exit_in: None }).add_systems(Last, auto_shots);
        }
        // Unattended runs make no noise either; sounds are still chosen, loaded and logged.
        app.insert_resource(db_client::AudioMuted(self.0.hidden()));
        if let Some(path) = &self.0.screenshot {
            app.insert_resource(ShotPlan { path: path.clone(), frames: self.0.frames.max(1), seen: 0, requested: false, exit_in: None })
                .add_systems(Last, screenshot_frame);
        }
        if let Some(seconds) = self.0.bench {
            app.insert_resource(BenchPlan { seconds, warmup: self.0.bench_warmup.unwrap_or(BENCH_WARMUP_S), elapsed: 0.0, frames_ms: Vec::new() }).add_systems(Last, bench_frame);
        }
    }
}

/// Screenshots the client asks for (`ShotRequests`), one at a time; exits after the last if asked.
#[derive(Resource, Debug)]
struct AutoShots {
    prefix: String,
    in_flight: bool,
    exit_in: Option<u32>,
}

/// A scripted run that hasn't finished after this long is stuck: exit with an error (the crypt runs of
/// walk much farther than the generated floors; the castle's extracts open 300-600 s into a run, from
/// its escape points' activation delays).
const AUTO_RUN_LIMIT_S: f32 = 900.0;

fn auto_shots(mut commands: Commands, time: Res<Time<Real>>, mut shots: ResMut<AutoShots>, mut requests: ResMut<db_client::ShotRequests>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs() > AUTO_RUN_LIMIT_S && shots.exit_in.is_none() {
        error!("auto-run: not finished after {AUTO_RUN_LIMIT_S} s; giving up");
        exit.write(AppExit::error());
        return;
    }
    if let Some(n) = shots.exit_in.as_mut() {
        if *n == 0 {
            exit.write(AppExit::Success);
        } else {
            *n -= 1;
        }
        return;
    }
    if shots.in_flight {
        return;
    }
    if let Some(name) = requests.queue.pop_front() {
        let path = db_client::data_path(format!("shots/{}_{name}.png", shots.prefix));
        // (Its folder too: a prefix may name one, e.g. `c6b4/crypt`.)
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        info!("auto-run: screenshot {}", path.display());
        shots.in_flight = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path)).observe(|_: On<ScreenshotCaptured>, mut shots: ResMut<AutoShots>| {
            shots.in_flight = false;
        });
    } else if requests.exit_after {
        shots.exit_in = Some(3);
    }
}

fn screenshot_frame(mut commands: Commands, mut plan: ResMut<ShotPlan>, mut exit: MessageWriter<AppExit>) {
    plan.seen += 1;
    if let Some(n) = plan.exit_in.as_mut() {
        // A few frames after capture so the file write (in the observer) has finished.
        if *n == 0 {
            exit.write(AppExit::Success);
        } else {
            *n -= 1;
        }
        return;
    }
    if !plan.requested && plan.seen >= plan.frames {
        plan.requested = true;
        if let Some(dir) = plan.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let path = plan.path.clone();
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path)).observe(
            |_: On<ScreenshotCaptured>, mut plan: ResMut<ShotPlan>| {
                plan.exit_in = Some(3);
            },
        );
    }
}

/// A class by name, case-insensitive (`swordmaster`, `rogue`, `fighter`, ...).
fn parse_class(name: &str) -> Option<db_client::EnRoleCareerType> {
    use db_client::EnRoleCareerType as C;
    // Spaces, `_` and `-` ignored, and `druid` for the panther form (`death_knight`, `Death Knight`).
    let n = name.to_ascii_lowercase().replace([' ', '_', '-'], "");
    let n = if n == "druid" { "druidpanther".to_string() } else { n };
    [C::Swordmaster, C::Fighter, C::Rogue, C::Priest, C::Wizard, C::Deathknight, C::Lich, C::DruidPanther].into_iter().find(|c| format!("{c:?}").to_ascii_lowercase() == n)
}

/// `Whirlwind,Charge`-style Q/E names (case-insensitive, spaces ignored).
fn parse_loadout(s: &str) -> anyhow::Result<[db_client::SkillKind; 2]> {
    use db_client::SkillKind as K;
    let one = |name: &str| -> anyhow::Result<K> {
        let n = name.trim().to_ascii_lowercase().replace(' ', "");
        K::ALL.into_iter().find(|k| k.name().to_ascii_lowercase().replace(' ', "") == n).ok_or_else(|| anyhow::anyhow!("unknown skill `{name}`"))
    };
    let (q, e) = s.split_once(',').ok_or_else(|| anyhow::anyhow!("--loadout needs <Q>,<E>"))?;
    Ok([one(q)?, one(e)?])
}

fn bench_frame(time: Res<Time<Real>>, mut plan: ResMut<BenchPlan>, mut sim: ResMut<db_client::SimTickTime>, mut exit: MessageWriter<AppExit>) {
    let sim = std::mem::take(&mut *sim);
    let dt = time.delta();
    if dt == Duration::ZERO {
        return;
    }
    plan.elapsed += dt.as_secs_f64();
    if plan.elapsed < plan.warmup {
        return;
    }
    plan.frames_ms.push(dt.as_secs_f64() * 1000.0);
    // Slow frames go to the log with their time, to line them up with what was happening.
    if dt.as_secs_f64() > 0.03 {
        warn!("bench: slow frame {:.1} ms at {:.2} s (sim {:.1} ms in {} ticks)", dt.as_secs_f64() * 1000.0, plan.elapsed, sim.secs * 1000.0, sim.ticks);
    }
    if plan.elapsed >= plan.warmup + plan.seconds {
        let report = bench_report(&plan.frames_ms);
        let _ = std::fs::create_dir_all(db_client::data_path("shots"));
        if let Err(e) = std::fs::write(db_client::data_path("shots/bench.txt"), &report) {
            error!("cannot write bench report: {e}");
        }
        info!("bench: {}", report.trim());
        exit.write(AppExit::Success);
    }
}

/// `frames=<n> avg_ms=<a> low1_ms=<99th percentile> worst_ms=<max> avg_fps=<1000/a> low1_fps=<1000/p99>`.
pub fn bench_report(frames_ms: &[f64]) -> String {
    if frames_ms.is_empty() {
        return "frames=0\n".into();
    }
    let mut v = frames_ms.to_vec();
    v.sort_by(f64::total_cmp);
    let avg = v.iter().sum::<f64>() / v.len() as f64;
    let p99 = v[((v.len() as f64 * 0.99).ceil() as usize).clamp(1, v.len()) - 1];
    let worst = v[v.len() - 1];
    format!(
        "frames={} avg_ms={avg:.3} low1_ms={p99:.3} worst_ms={worst:.3} avg_fps={:.1} low1_fps={:.1}\n",
        v.len(),
        1000.0 / avg,
        1000.0 / p99
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_names() {
        use db_client::EnRoleCareerType as C;
        for (name, class) in [("druid", C::DruidPanther), ("DruidPanther", C::DruidPanther), ("death_knight", C::Deathknight), ("Death Knight", C::Deathknight), ("wizard", C::Wizard)] {
            assert_eq!(parse_class(name), Some(class), "{name}");
        }
        assert_eq!(parse_class("bard"), None);
    }

    #[test]
    fn report_percentiles() {
        let mut f = vec![10.0; 99];
        f.push(40.0);
        let r = bench_report(&f);
        assert!(r.contains("frames=100"), "{r}");
        assert!(r.contains("low1_ms=10.000"), "{r}");
        assert!(r.contains("worst_ms=40.000"), "{r}");
        assert!(r.contains("avg_ms=10.300"), "{r}");
    }

    #[test]
    fn parses_args() {
        let a: Vec<String> = ["--screenshot", "x.png", "--frames", "5"].iter().map(|s| s.to_string()).collect();
        let d = DevArgs::parse(&a).unwrap();
        assert_eq!(d.screenshot, Some(PathBuf::from("x.png")));
        assert_eq!(d.frames, 5);
        assert!(DevArgs::parse(&["--nope".to_string()]).is_err());
    }
}

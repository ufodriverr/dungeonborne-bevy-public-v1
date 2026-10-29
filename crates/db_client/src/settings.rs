//! Settings: resolution, window mode, quality preset, field of view, mouse sensitivity, master and
//! effects volume, key bindings. Saved as JSON to `_data/save/settings.json` (`SettingsPath`; dev runs
//! that are hidden use the defaults and save nothing unless `--settings <path>` is given). The settings
//! screen opens from the title and the pause menu; every change applies at once and is saved.
//!
//! Ours (the game's own settings screen and its presets aren't decoded): the resolution list, the ranges,
//! the presets. FOV default 110 is the saved game setting.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use bevy::light::PointLightShadowMap;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use serde::{Deserialize, Serialize};

use crate::flow::Action;
use crate::menus::{GOLD, button, text};

/// The resolutions offered (16:9, 720p to 1440p).
pub const RESOLUTIONS: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
pub const FOV_RANGE: (f32, f32) = (80.0, 120.0);
const FOV_STEP: f32 = 5.0;
pub const SENSITIVITY_RANGE: (f32, f32) = (0.25, 3.0);
const SENSITIVITY_STEP: f32 = 0.25;
const VOLUME_STEP: f32 = 0.1;
/// The UI is laid out for this window height and scaled to the real one.
const UI_DESIGN_HEIGHT: f32 = 1080.0;
const SETTINGS_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WindowKind {
    #[default]
    Windowed,
    Borderless,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Quality {
    Low,
    #[default]
    Medium,
    High,
}

impl Quality {
    /// Map lights drawn at once, the nearest the camera (Medium and High draw all of them: the crypt ~630 over
    /// both floors with its cells' 3,465, the castle 1,746).
    pub fn map_lights(self) -> usize {
        if self == Quality::Low { 200 } else { usize::MAX }
    }
    /// Lights that effects (fire, spells, portals) carry.
    pub fn effect_lights(self) -> bool {
        self != Quality::Low
    }
    /// Test room torches that cast shadows.
    pub fn shadow_torches(self) -> usize {
        match self {
            Quality::Low => 0,
            Quality::Medium => 2,
            Quality::High => 4,
        }
    }
    pub fn shadow_map_size(self) -> usize {
        match self {
            Quality::Low => 512,
            Quality::Medium => 1024,
            Quality::High => 2048,
        }
    }
    pub fn msaa(self) -> Msaa {
        if self == Quality::Low { Msaa::Off } else { Msaa::Sample4 }
    }
    fn name(self) -> &'static str {
        match self {
            Quality::Low => "Low",
            Quality::Medium => "Medium",
            Quality::High => "High",
        }
    }
}

/// The rebindable actions.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bind {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Crouch,
    Jump,
    SkillQ,
    SkillE,
    Interact,
    Inventory,
    Potion1,
    Potion2,
    /// the item under the cursor in the inventory: drop one (`SettingKeyBindingsItem_DropItemsonebyone`), drop the
    /// stack (`_DropItemsAllatonce`), destroy (ours). The game's default keys aren't in the data (HYPOTHESIS).
    DropOne,
    DropAll,
    Destroy,
    /// the crossbow's reload (`InputTag.Weapon.Fire.FireReload`; the key is ours, HYPOTHESIS).
    Reload,
    /// switch weapon sets (the game's "切换武器套装1/2" sit on keys the port gives the potions; X is ours). The mouse
    /// wheel also switches (`QuickSlot.Up` / `Down`, "切换上一个/下一个武器").
    SwapWeapons,
    /// throw an Incendiary, Thunderclap or Smokescreen flask from the backpack (the game throws the flask held in a
    /// quick slot, keys 3-5, with the left button; the port's keys throw at once, HYPOTHESIS).
    Flask1,
    Flask2,
    Flask3,
    Flask4,
    Torch,
}

impl Bind {
    pub const ALL: [Bind; 23] = [Bind::Forward, Bind::Back, Bind::Left, Bind::Right, Bind::Sprint, Bind::Crouch, Bind::Jump, Bind::SkillQ, Bind::SkillE, Bind::Interact, Bind::Inventory, Bind::Potion1, Bind::Potion2, Bind::DropOne, Bind::DropAll, Bind::Destroy, Bind::Reload, Bind::SwapWeapons, Bind::Flask1, Bind::Flask2, Bind::Flask3, Bind::Flask4, Bind::Torch];

    pub fn default_key(self) -> KeyCode {
        match self {
            Bind::Forward => KeyCode::KeyW,
            Bind::Back => KeyCode::KeyS,
            Bind::Left => KeyCode::KeyA,
            Bind::Right => KeyCode::KeyD,
            Bind::Sprint => KeyCode::ShiftLeft,
            Bind::Crouch => KeyCode::ControlLeft,
            Bind::Jump => KeyCode::Space,
            Bind::SkillQ => KeyCode::KeyQ,
            Bind::SkillE => KeyCode::KeyE,
            Bind::Interact => KeyCode::KeyF,
            Bind::Inventory => KeyCode::Tab,
            Bind::Potion1 => KeyCode::Digit1,
            Bind::Potion2 => KeyCode::Digit2,
            Bind::DropOne => KeyCode::KeyG,
            Bind::DropAll => KeyCode::KeyH,
            Bind::Destroy => KeyCode::Delete,
            Bind::Reload => KeyCode::KeyR,
            Bind::SwapWeapons => KeyCode::KeyX,
            Bind::Flask1 => KeyCode::Digit3,
            Bind::Flask2 => KeyCode::Digit4,
            Bind::Flask3 => KeyCode::Digit5,
            Bind::Flask4 => KeyCode::Digit6,
            Bind::Torch => KeyCode::Digit7,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Bind::Forward => "Forward",
            Bind::Back => "Back",
            Bind::Left => "Left",
            Bind::Right => "Right",
            Bind::Sprint => "Sprint",
            Bind::Crouch => "Crouch",
            Bind::Jump => "Jump",
            Bind::SkillQ => "Skill (Q)",
            Bind::SkillE => "Skill (E)",
            Bind::Interact => "Interact",
            Bind::Inventory => "Inventory",
            Bind::Potion1 => "Healing potion",
            Bind::Potion2 => "Protection potion",
            Bind::DropOne => "Drop one (inventory)",
            Bind::DropAll => "Drop all (inventory)",
            Bind::Destroy => "Destroy (inventory)",
            Bind::Reload => "Reload",
            Bind::SwapWeapons => "Switch weapon set",
            Bind::Flask1 => "Incendiary flask",
            Bind::Flask2 => "Thunderclap flask",
            Bind::Flask3 => "Smokescreen flask",
            Bind::Flask4 => "Stoneshaping flask",
            Bind::Torch => "Torch (take out / put away)",
        }
    }
}

/// Keys that can be bound (Escape stays the pause key; the mouse buttons stay attack and block).
const BINDABLE: [KeyCode; 76] = [
    KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI, KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM,
    KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4, KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8, KeyCode::F9, KeyCode::F10, KeyCode::F11, KeyCode::F12,
    KeyCode::Space, KeyCode::Tab, KeyCode::Enter, KeyCode::Backspace, KeyCode::CapsLock, KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::AltLeft, KeyCode::AltRight,
    KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight,
    KeyCode::Backquote, KeyCode::Minus, KeyCode::Equal, KeyCode::BracketLeft, KeyCode::BracketRight, KeyCode::Semicolon, KeyCode::Quote, KeyCode::Comma, KeyCode::Period, KeyCode::Slash, KeyCode::Backslash,
    KeyCode::Insert, KeyCode::Delete,
];

/// A key's saved name (its `KeyCode` variant name).
pub fn key_name(k: KeyCode) -> String {
    format!("{k:?}")
}

/// The bindable key with this saved name.
pub fn key_from_name(name: &str) -> Option<KeyCode> {
    BINDABLE.iter().copied().find(|k| key_name(*k) == name)
}

/// A key as the settings screen shows it ("W", "1", "Left Shift").
fn key_label(k: KeyCode) -> String {
    let n = key_name(k);
    if let Some(rest) = n.strip_prefix("Key").or_else(|| n.strip_prefix("Digit")) {
        return rest.to_string();
    }
    for (suffix, side) in [("Left", "Left "), ("Right", "Right ")] {
        if let Some(base) = n.strip_suffix(suffix).filter(|b| !b.starts_with("Arrow") && !b.is_empty()) {
            return format!("{side}{}", if base == "Control" { "Ctrl" } else { base });
        }
    }
    n
}

/// Everything the settings screen sets (saved as JSON; missing fields take their defaults).
#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub resolution: (u32, u32),
    pub window: WindowKind,
    pub quality: Quality,
    /// Horizontal field of view, degrees.
    pub fov: f32,
    /// Multiplies the base mouse turn rate.
    pub sensitivity: f32,
    pub master_volume: f32,
    /// Everything but music.
    pub effects_volume: f32,
    /// Action → saved key name; actions not listed use their default key.
    pub keys: BTreeMap<Bind, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { version: SETTINGS_VERSION, resolution: (1920, 1080), window: WindowKind::Windowed, quality: Quality::Medium, fov: 110.0, sensitivity: 1.0, master_volume: 1.0, effects_volume: 1.0, keys: BTreeMap::new() }
    }
}

impl Settings {
    /// Reads the file; a missing or broken file gives the defaults, out-of-range values are pulled back.
    pub fn load(path: &Path) -> Settings {
        let s = std::fs::read_to_string(path).ok().and_then(|t| match serde_json::from_str::<Settings>(&t) {
            Ok(s) => Some(s),
            Err(e) => {
                warn!("settings: {} unreadable ({e}); using defaults", path.display());
                None
            }
        });
        s.unwrap_or_default().validated()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).map_err(std::io::Error::other)?)
    }

    /// Values pulled into their ranges; an unknown resolution goes to 1920 × 1080, unknown key names to
    /// the action's default.
    pub fn validated(mut self) -> Settings {
        self.version = SETTINGS_VERSION;
        if !RESOLUTIONS.contains(&self.resolution) {
            self.resolution = (1920, 1080);
        }
        self.fov = if self.fov.is_finite() { self.fov.clamp(FOV_RANGE.0, FOV_RANGE.1) } else { 110.0 };
        self.sensitivity = if self.sensitivity.is_finite() { self.sensitivity.clamp(SENSITIVITY_RANGE.0, SENSITIVITY_RANGE.1) } else { 1.0 };
        for v in [&mut self.master_volume, &mut self.effects_volume] {
            *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 };
        }
        self.keys.retain(|_, name| key_from_name(name).is_some());
        self
    }

    /// The key bound to an action.
    pub fn key(&self, b: Bind) -> KeyCode {
        self.keys.get(&b).and_then(|n| key_from_name(n)).unwrap_or(b.default_key())
    }

    /// Binds `key` to `b`; an action that had `key` takes `b`'s old key (no key does two things).
    pub fn bind(&mut self, b: Bind, key: KeyCode) {
        let old = self.key(b);
        if let Some(other) = Bind::ALL.into_iter().find(|o| *o != b && self.key(*o) == key) {
            self.keys.insert(other, key_name(old));
        }
        self.keys.insert(b, key_name(key));
    }

    /// Volume for a new sound: master × effects (music only master).
    pub fn volume(&self, music: bool) -> f32 {
        self.master_volume * if music { 1.0 } else { self.effects_volume }
    }
}

/// Where the settings are saved (`None`: not saved, e.g. hidden dev runs).
#[derive(Resource, Debug, Default, Clone)]
pub struct SettingsPath(pub Option<PathBuf>);

/// Dev overrides applied after loading (`--resolution WxH`, `--quality`).
#[derive(Resource, Debug, Default, Clone)]
pub struct SettingsOverride {
    pub resolution: Option<(u32, u32)>,
    pub quality: Option<Quality>,
    /// Open the settings screen at start (`--settings-open`).
    pub open: bool,
}

/// The keys in use (from [`Settings`], refreshed when they change).
#[derive(Resource, Debug, Clone)]
pub struct Keys([KeyCode; Bind::ALL.len()]);

impl Default for Keys {
    fn default() -> Self {
        Self(Bind::ALL.map(Bind::default_key))
    }
}

impl Keys {
    pub fn get(&self, b: Bind) -> KeyCode {
        self.0[Bind::ALL.iter().position(|x| *x == b).unwrap_or(0)]
    }
}

/// Whether the settings screen is open, and the action waiting for a key.
#[derive(Resource, Debug, Default)]
pub struct SettingsOpen {
    pub open: bool,
    /// Closed by Esc this frame (so the same Esc doesn't also resume the game).
    pub just_closed: bool,
    rebinding: Option<Bind>,
}

impl SettingsOpen {
    pub fn show(&mut self) {
        self.open = true;
        self.rebinding = None;
    }
}

/// A map light (the floor's own or a cell's), for the quality's light cap.
#[derive(Component)]
pub(crate) struct MapLight;

/// Where the camera was when the capped preset last picked its nearest lights.
#[derive(Default)]
pub(crate) struct LightPick(Option<Vec3>);

/// A test room torch (its index; the first `shadow_torches` cast shadows).
#[derive(Component)]
pub(crate) struct Torch(pub usize);

/// A sound's mix group (music follows only the master volume).
#[derive(Component, Clone, Copy)]
pub(crate) struct SoundGroup {
    pub music: bool,
}

pub(crate) fn load_settings(path: Res<SettingsPath>, over: Res<SettingsOverride>, mut settings: ResMut<Settings>, mut open: ResMut<SettingsOpen>) {
    info!("data folder: {}", db_formats::data_root().display());
    if over.open {
        open.show();
    }
    let mut s = path.0.as_deref().map_or_else(Settings::default, Settings::load);
    if let Some(r) = over.resolution.filter(|r| RESOLUTIONS.contains(r)) {
        s.resolution = r;
    }
    if let Some(q) = over.quality {
        s.quality = q;
    }
    info!("settings: {:?} {:?} {:?}, fov {}, from {}", s.resolution, s.window, s.quality, s.fov, path.0.as_deref().map_or("defaults (not saved)".into(), |p| p.display().to_string()));
    *settings = s;
}

/// Applies changed settings: window, keys, FOV, MSAA, shadow map size, sound volumes, light caps; saves.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn apply_settings(
    settings: Res<Settings>,
    path: Res<SettingsPath>,
    mut keys: ResMut<Keys>,
    mut fov: ResMut<crate::FovSetting>,
    mut shadow: ResMut<PointLightShadowMap>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut cams: Query<&mut Msaa, With<Camera>>,
    mut sinks: Query<(&mut AudioSink, &SoundGroup)>,
    mut first: Local<bool>,
) {
    if !settings.is_changed() {
        return;
    }
    let s = &*settings;
    if let Ok(mut w) = windows.single_mut() {
        let (rw, rh) = s.resolution;
        if w.resolution.physical_width() != rw || w.resolution.physical_height() != rh {
            w.resolution.set_physical_resolution(rw, rh);
        }
        // A hidden check window stays windowed.
        let mode = if s.window == WindowKind::Borderless && w.visible { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
        if w.mode != mode {
            w.mode = mode;
        }
    }
    keys.0 = Bind::ALL.map(|b| s.key(b));
    fov.0 = s.fov;
    shadow.size = s.quality.shadow_map_size();
    for mut m in &mut cams {
        if *m != s.quality.msaa() {
            *m = s.quality.msaa();
        }
    }
    for (mut sink, g) in &mut sinks {
        sink.set_volume(Volume::Linear(s.volume(g.music)));
    }
    // The first application is the loaded file: nothing to save.
    if *first
        && let Some(p) = &path.0
        && let Err(e) = s.save(p)
    {
        warn!("settings: can't save {}: {e}", p.display());
    }
    *first = true;
}

/// The UI is laid out for 1080 px high windows; scale it to the window.
pub(crate) fn ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let Ok(w) = windows.single() else { return };
    let s = (w.resolution.height() / UI_DESIGN_HEIGHT).max(0.25);
    if (scale.0 - s).abs() > 1e-4 {
        scale.0 = s;
    }
}

/// The player camera's projection follows the FOV setting and the window's aspect; aiming a crossbow blends to its
/// aim camera's field of view (`CM_Tgf_FirstPerson_ADS`: 60, over 0.33 s; linear here, HYPOTHESIS).
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_fov(
    fov: Res<crate::FovSetting>,
    time: Res<Time>,
    player: Res<crate::LocalPlayer>,
    index: Res<db_sim::SimIndex>,
    crossbows: Query<&db_sim::crossbow::CrossbowState>,
    mut aim: Local<f32>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cams: Query<&mut Projection, With<crate::PlayerCamera>>,
) {
    let Ok(w) = windows.single() else { return };
    let aspect = w.resolution.width() / w.resolution.height().max(1.0);
    let held = player.0.and_then(|id| index.get(id)).and_then(|e| crossbows.get(e).ok()).and_then(|c| Some((c.aiming, c.held.as_ref()?.1.clone())));
    let (aiming, ads_fov, blend) = held.map_or((false, fov.0, 0.0), |(a, s)| (a, s.ads_fov, s.ads_blend));
    let step = if blend > 0.0 { time.delta_secs() / blend } else { 1.0 };
    *aim = if aiming { (*aim + step).min(1.0) } else { (*aim - step).max(0.0) };
    let horizontal = fov.0 + (ads_fov - fov.0) * *aim;
    for mut p in &mut cams {
        if let Projection::Perspective(pp) = &mut *p {
            let v = crate::vertical_fov(horizontal, aspect);
            if (pp.fov - v).abs() > 1e-5 {
                pp.fov = v;
            }
        }
    }
}

/// Light caps and shadows by quality, for new lights and when the quality changes.
#[allow(clippy::type_complexity)]
pub(crate) fn light_quality(
    settings: Res<Settings>,
    camera: Query<&GlobalTransform, With<crate::PlayerCamera>>,
    mut last: Local<LightPick>,
    mut map: Query<(Ref<MapLight>, Entity, &GlobalTransform, &mut Visibility), Without<crate::effects::EffectLight>>,
    mut effects: Query<(Ref<crate::effects::EffectLight>, &mut Visibility), Without<MapLight>>,
    mut torches: Query<(Ref<Torch>, &mut PointLight)>,
) {
    let q = settings.quality;
    let all = settings.is_changed();
    // (playtest): a capped preset draws the lights nearest the camera, picked again as it moves (half a metre) or
    // as cells stream lights in. A fixed rank put every cell light (rank 100 000 on) behind the floor's own
    // lights, so on Low the castle (one own light) drew none of its 1,745 candles and lamps.
    let eye = camera.iter().next().map(GlobalTransform::translation);
    let added = map.iter().any(|(l, ..)| l.is_added());
    let moved = eye.is_some_and(|p| last.0.is_none_or(|o| o.distance(p) > 0.5));
    if all || added || moved {
        last.0 = eye;
        let cap = q.map_lights();
        let mut order: Vec<(f32, Entity)> = map.iter().map(|(_, e, at, _)| (eye.map_or(0.0, |p| at.translation().distance_squared(p)), e)).collect();
        if cap < order.len() {
            order.select_nth_unstable_by(cap, |a, b| a.0.total_cmp(&b.0));
        }
        let shown: std::collections::HashSet<Entity> = order.iter().take(cap).map(|x| x.1).collect();
        for (_, e, _, mut v) in &mut map {
            v.set_if_neq(if shown.contains(&e) { Visibility::Inherited } else { Visibility::Hidden });
        }
        if all || added {
            info!("quality {}: {} of {} map lights drawn", q.name(), shown.len(), order.len());
        }
    }
    for (l, mut v) in &mut effects {
        if all || l.is_added() {
            v.set_if_neq(if q.effect_lights() { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
    for (t, mut p) in &mut torches {
        if all || t.is_added() {
            p.shadow_maps_enabled = t.0 < q.shadow_torches();
        }
    }
}

// ---- The settings screen ----

/// What a settings control changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Field {
    Resolution,
    Window,
    Quality,
    Fov,
    Sensitivity,
    Master,
    Effects,
}

impl Field {
    const ALL: [Field; 7] = [Field::Resolution, Field::Window, Field::Quality, Field::Fov, Field::Sensitivity, Field::Master, Field::Effects];

    fn label(self) -> &'static str {
        match self {
            Field::Resolution => "Resolution",
            Field::Window => "Window",
            Field::Quality => "Quality",
            Field::Fov => "Field of view",
            Field::Sensitivity => "Mouse sensitivity",
            Field::Master => "Master volume",
            Field::Effects => "Effects volume",
        }
    }

    fn value(self, s: &Settings) -> String {
        match self {
            Field::Resolution => format!("{} × {}", s.resolution.0, s.resolution.1),
            Field::Window => if s.window == WindowKind::Windowed { "Windowed" } else { "Borderless" }.into(),
            Field::Quality => s.quality.name().into(),
            Field::Fov => format!("{:.0}°", s.fov),
            Field::Sensitivity => format!("{:.2}", s.sensitivity),
            Field::Master => format!("{:.0} %", s.master_volume * 100.0),
            Field::Effects => format!("{:.0} %", s.effects_volume * 100.0),
        }
    }

    /// One step down (-1) or up (+1).
    fn step(self, s: &mut Settings, dir: i32) {
        let cycle = |i: usize, n: usize| (i as i32 + dir).rem_euclid(n as i32) as usize;
        match self {
            Field::Resolution => {
                let i = RESOLUTIONS.iter().position(|r| *r == s.resolution).unwrap_or(2);
                s.resolution = RESOLUTIONS[cycle(i, RESOLUTIONS.len())];
            }
            Field::Window => s.window = if s.window == WindowKind::Windowed { WindowKind::Borderless } else { WindowKind::Windowed },
            Field::Quality => {
                let all = [Quality::Low, Quality::Medium, Quality::High];
                let i = all.iter().position(|q| *q == s.quality).unwrap_or(1);
                s.quality = all[cycle(i, all.len())];
            }
            Field::Fov => s.fov = (s.fov + FOV_STEP * dir as f32).clamp(FOV_RANGE.0, FOV_RANGE.1),
            Field::Sensitivity => s.sensitivity = (s.sensitivity + SENSITIVITY_STEP * dir as f32).clamp(SENSITIVITY_RANGE.0, SENSITIVITY_RANGE.1),
            Field::Master => s.master_volume = ((s.master_volume + VOLUME_STEP * dir as f32) * 10.0).round().clamp(0.0, 10.0) / 10.0,
            Field::Effects => s.effects_volume = ((s.effects_volume + VOLUME_STEP * dir as f32) * 10.0).round().clamp(0.0, 10.0) / 10.0,
        }
    }
}

#[derive(Component, Clone, Copy)]
pub(crate) enum SettingsControl {
    Step(Field, i32),
    Rebind(Bind),
}

#[derive(Component)]
pub(crate) struct SettingsScreen;

#[derive(Component)]
pub(crate) struct FieldValue(Field);

#[derive(Component)]
pub(crate) struct BindValue(Bind);

const ROW_BG: Color = Color::srgba(0.12, 0.1, 0.08, 0.9);
const ROW_LINE: Color = Color::srgb(0.35, 0.28, 0.18);
const LABEL: Color = Color::srgb(0.85, 0.82, 0.75);

fn small_button(parent: &mut ChildSpawnerCommands, label: &str, control: SettingsControl, width: f32) -> Entity {
    parent
        .spawn((
            Button,
            Node { width: px(width), height: px(32.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(px(1.0)), ..default() },
            BackgroundColor(ROW_BG),
            BorderColor::all(ROW_LINE),
            control,
        ))
        .with_children(|b| {
            b.spawn(text(label, 18.0, LABEL));
        })
        .id()
}

pub(crate) fn setup_settings_screen(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.92)),
            GlobalZIndex(40),
            Visibility::Hidden,
            SettingsScreen,
        ))
        .with_children(|r| {
            r.spawn(Node { width: px(1180.0), padding: UiRect::axes(px(48.0), px(32.0)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(10.0), ..default() }).with_children(|p| {
                p.spawn((
                    ImageNode { image: crate::menus::ui(&assets, "Textures/Common/PopupBox/T_Trade_Popup_Bg1"), image_mode: NodeImageMode::Stretch, ..default() },
                    Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
                ));
                p.spawn(text("Settings", 44.0, GOLD));
                p.spawn(Node { column_gap: px(48.0), ..default() }).with_children(|cols| {
                    // Left: display, mouse, sound.
                    cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(8.0), ..default() }).with_children(|c| {
                        for f in Field::ALL {
                            c.spawn(Node { align_items: AlignItems::Center, column_gap: px(8.0), ..default() }).with_children(|row| {
                                row.spawn((text(f.label(), 18.0, LABEL), Node { width: px(190.0), ..default() }));
                                small_button(row, "<", SettingsControl::Step(f, -1), 36.0);
                                row.spawn((Node { width: px(170.0), justify_content: JustifyContent::Center, ..default() },)).with_children(|v| {
                                    v.spawn((text("", 18.0, Color::WHITE), FieldValue(f)));
                                });
                                small_button(row, ">", SettingsControl::Step(f, 1), 36.0);
                            });
                        }
                        c.spawn((text("Resolution and window apply at once. Quality: Low draws the 200 brightest map lights per floor, no effect lights, no shadows, no anti-aliasing; High adds test room shadows.", 14.0, LABEL), Node { width: px(460.0), margin: UiRect::top(px(6.0)), ..default() }));
                    });
                    // Right: keys.
                    cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4.0), ..default() }).with_children(|c| {
                        c.spawn(text("Keys (click, then press a key; Esc cancels)", 18.0, GOLD));
                        for b in Bind::ALL {
                            c.spawn(Node { align_items: AlignItems::Center, column_gap: px(8.0), ..default() }).with_children(|row| {
                                row.spawn((text(b.label(), 17.0, LABEL), Node { width: px(170.0), ..default() }));
                                let e = small_button(row, "", SettingsControl::Rebind(b), 170.0);
                                row.commands().entity(e).with_children(|v| {
                                    v.spawn((text("", 17.0, Color::WHITE), BindValue(b)));
                                });
                            });
                        }
                    });
                });
                p.spawn(Node { column_gap: px(20.0), margin: UiRect::top(px(10.0)), ..default() }).with_children(|b| {
                    button(b, &assets, "Defaults", Action::SettingsDefaults, 240.0);
                    button(b, &assets, "Back", Action::SettingsBack, 240.0);
                });
            });
        });
}

/// Shows the screen, handles its arrows and key rebinding, and keeps the shown values current.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn settings_screen(
    keys_in: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<SettingsOpen>,
    mut settings: ResMut<Settings>,
    controls: Query<(&Interaction, &SettingsControl), Changed<Interaction>>,
    mut screen: Query<&mut Visibility, With<SettingsScreen>>,
    mut values: Query<(&mut Text, &FieldValue), Without<BindValue>>,
    mut binds: Query<(&mut Text, &BindValue), Without<FieldValue>>,
) {
    if open.just_closed {
        open.just_closed = false;
    }
    if let Ok(mut v) = screen.single_mut() {
        v.set_if_neq(if open.open { Visibility::Inherited } else { Visibility::Hidden });
    }
    if !open.open {
        return;
    }
    if let Some(b) = open.rebinding {
        if keys_in.just_pressed(KeyCode::Escape) {
            open.rebinding = None;
        } else if let Some(k) = keys_in.get_just_pressed().copied().find(|k| key_from_name(&key_name(*k)).is_some()) {
            settings.bind(b, k);
            open.rebinding = None;
        }
    } else if keys_in.just_pressed(KeyCode::Escape) {
        open.open = false;
        open.just_closed = true;
        return;
    }
    for (i, c) in &controls {
        if *i != Interaction::Pressed {
            continue;
        }
        match *c {
            SettingsControl::Step(f, dir) => f.step(&mut settings, dir),
            SettingsControl::Rebind(b) => open.rebinding = Some(b),
        }
    }
    let changed = settings.is_changed() || open.is_changed();
    if !changed {
        return;
    }
    for (mut t, v) in &mut values {
        t.0 = v.0.value(&settings);
    }
    for (mut t, b) in &mut binds {
        t.0 = if open.rebinding == Some(b.0) { "press a key".into() } else { key_label(settings.key(b.0)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_documented_ones() {
        let s = Settings::default();
        assert_eq!((s.resolution, s.window, s.quality, s.fov, s.sensitivity, s.master_volume, s.effects_volume), ((1920, 1080), WindowKind::Windowed, Quality::Medium, 110.0, 1.0, 1.0, 1.0));
        assert_eq!(s.key(Bind::Forward), KeyCode::KeyW);
        assert_eq!(s.key(Bind::Inventory), KeyCode::Tab);
        // Every default key is bindable and no two actions share one.
        let mut seen = std::collections::HashSet::new();
        for b in Bind::ALL {
            assert!(key_from_name(&key_name(b.default_key())).is_some(), "{b:?}");
            assert!(seen.insert(b.default_key()), "{b:?}");
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("db_settings_{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings { resolution: (2560, 1440), window: WindowKind::Borderless, quality: Quality::High, fov: 95.0, sensitivity: 1.5, master_volume: 0.6, effects_volume: 0.3, ..Default::default() };
        s.bind(Bind::Jump, KeyCode::KeyV);
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        // A missing file gives the defaults.
        assert_eq!(Settings::load(&dir.join("missing.json")), Settings::default());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn broken_partial_and_out_of_range_files_are_mended() {
        let dir = std::env::temp_dir().join(format!("db_settings_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::write(&path, r#"{"fov": 300, "sensitivity": -2, "master_volume": 4, "resolution": [1000, 1000], "keys": {"Jump": "NotAKey", "Interact": "KeyG"}}"#).unwrap();
        let s = Settings::load(&path);
        assert_eq!((s.fov, s.sensitivity, s.master_volume, s.resolution), (120.0, 0.25, 1.0, (1920, 1080)));
        assert_eq!(s.key(Bind::Jump), KeyCode::Space, "an unknown key name falls back to the default");
        assert_eq!(s.key(Bind::Interact), KeyCode::KeyG);
        assert_eq!(s.quality, Quality::Medium, "missing fields take their defaults");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn binding_a_used_key_swaps() {
        let mut s = Settings::default();
        s.bind(Bind::Jump, KeyCode::KeyW);
        assert_eq!(s.key(Bind::Jump), KeyCode::KeyW);
        assert_eq!(s.key(Bind::Forward), KeyCode::Space, "forward takes jump's old key");
    }

    #[test]
    fn steps_stay_in_range() {
        let mut s = Settings::default();
        for _ in 0..20 {
            Field::Fov.step(&mut s, 1);
            Field::Master.step(&mut s, -1);
        }
        assert_eq!((s.fov, s.master_volume), (120.0, 0.0));
        Field::Resolution.step(&mut s, 1);
        assert_eq!(s.resolution, (2560, 1440));
        Field::Resolution.step(&mut s, 1);
        assert_eq!(s.resolution, (1280, 720), "the list wraps");
        assert_eq!(key_label(KeyCode::ShiftLeft), "Left Shift");
        assert_eq!(key_label(KeyCode::ControlLeft), "Left Ctrl");
        assert_eq!(key_label(KeyCode::Digit1), "1");
        assert_eq!(key_label(KeyCode::ArrowLeft), "ArrowLeft");
    }
}

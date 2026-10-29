//! Sounds: sim events and the local player's movement → the game's own Wwise sounds, converted
//! to OGG in `_data/audio/ogg` and named by `_data/audio/index.json`.
//!
//! Event choice is by name (HYPOTHESIS: the Wwise switch/state logic that picks variants in the game
//! is not modelled; variants are filtered by surface/material words in the source file name and picked
//! at random). Every played sound is logged (`sound:` lines) so unattended runs can check them; runs
//! with a hidden window are muted but still resolve and load the files.

use std::collections::BTreeMap;

use bevy::audio::{AudioPlayer, PlaybackSettings, SpatialListener, SpatialScale};
use bevy::prelude::*;
use db_sim::{ActorKind, AiState, AttackKind, CharacterState, HitSource, Kind, MonsterKit, Position, SimEvent, SimIndex, SimRng, SkillKind};

use crate::combat_view::{FrameEvents, LocalPlayer};

/// Event name → OGG media ids with their original source file names.
#[derive(Resource, Debug, Default)]
pub struct SoundBank(BTreeMap<String, Vec<(u64, String)>>);

impl SoundBank {
    pub fn load() -> Self {
        let Ok(bytes) = std::fs::read(db_formats::data_path("audio/index.json")) else {
            warn!("_data/audio/index.json missing (run the S9 audio steps); no sounds");
            return Self::default();
        };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            warn!("_data/audio/index.json does not parse; no sounds");
            return Self::default();
        };
        let mut bank = BTreeMap::new();
        for e in v["events"].as_array().into_iter().flatten() {
            let (Some(name), Some(media)) = (e["name"].as_str(), e["media"].as_array()) else { continue };
            let list: Vec<(u64, String)> = media.iter().filter_map(|m| Some((m["id"].as_u64()?, m["source"].as_str()?.to_string()))).collect();
            bank.insert(name.to_string(), list);
        }
        Self(bank)
    }

    /// Variants of `event` whose source names contain every word of the first filter that matches any.
    fn variants(&self, event: &str, filters: &[&[&str]]) -> Vec<&(u64, String)> {
        let all = self.0.get(event).map(Vec::as_slice).unwrap_or_default();
        for f in filters {
            let v: Vec<_> = all.iter().filter(|(_, s)| f.iter().all(|w| s.contains(w))).collect();
            if !v.is_empty() {
                return v;
            }
        }
        all.iter().collect()
    }
}

/// Mute playback (hidden dev runs); sounds are still chosen, logged and loaded.
#[derive(Resource, Debug, Default)]
pub struct AudioMuted(pub bool);

/// Presentation-only randomness for variant picks (never the sim's RNG).
#[derive(Resource)]
struct SoundRng(SimRng);

/// A Wwise playback limit: at most `max` sounds of `group` play at once; a new one is dropped (`kill_newest`) or
/// stops the oldest. `global`: across all emitters (else per game object).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Limit {
    group: u32,
    max: u16,
    kill_newest: bool,
    global: bool,
}

/// The limits by media id, and each limited group's playing sounds (oldest first, with when they started, s).
#[derive(Resource, Default)]
struct SoundLimits {
    by_media: BTreeMap<u64, Limit>,
    playing: BTreeMap<u32, Vec<(Entity, f32)>>,
}

impl SoundLimits {
    fn load() -> Self {
        let j = std::fs::read(db_formats::data_path("recovered/sound_limits.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let by_media = j
            .as_ref()
            .and_then(|j| j.as_object())
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.parse().ok()?, Limit { group: v["group"].as_u64()? as u32, max: v["max"].as_u64()? as u16, kill_newest: v["kill_newest"].as_bool()?, global: v["global"].as_bool()? })))
            .collect();
        Self { by_media, playing: BTreeMap::new() }
    }
}

/// What a new sound does in its group: play, play after stopping the oldest, or not play (the group is full and
/// kills the newest).
#[derive(Debug, PartialEq)]
enum Admit {
    Play,
    StopOldest(Entity),
    Drop,
}

/// A new sound against its group's `playing` sounds (oldest first).
fn admit(playing: &[(Entity, f32)], lim: Limit) -> Admit {
    if playing.len() < lim.max as usize {
        Admit::Play
    } else if lim.kill_newest {
        Admit::Drop
    } else {
        Admit::StopOldest(playing[0].0)
    }
}

/// Keeps muted sounds' handles alive so their loads finish (and load errors show in the log).
#[derive(Resource, Default)]
struct MutedHandles(Vec<Handle<AudioSource>>);

/// A looping sound tied to an actor's skill.
#[derive(Component)]
struct SkillLoop(db_sim::SimId, SkillKind);

/// A staff loop: the beam's `Play_Staff_Skill_Fire_Loop` (0) or the recharge's `Play_Staff_ChargeEnerge_1st_Loop` (1).
#[derive(Component)]
struct StaffLoop(db_sim::SimId, u8);

/// A thundercloud's `Play_Staff_Skill_Thunder_Loop`, stopped when the cloud goes (seconds left).
#[derive(Component)]
struct CloudHum(f32);

/// A loop on an actor's body that follows it: a monster's `AliveLoopSound` while it lives, the burn loop
/// while it burns.
#[derive(Component)]
struct BodyLoop(db_sim::SimId, String);

/// `GCNL_TGF_Character_Burning`'s loop (the burn's cue).
const BURN_LOOP: &str = "Play_Item_Body_Burning_Loop";

/// An open portal's hum (`BP_ExtractPortal`'s `Play_Portal_Active_Loop`).
#[derive(Component)]
struct PortalHum(db_sim::SimId);

/// Montage sound cues waiting to play: (seconds left, owner, Wwise event). A cue is dropped when its
/// owner dies (the montage is interrupted).
#[derive(Resource, Default)]
struct Cues(Vec<(f32, db_sim::SimId, String)>);

/// Local player's footstep tracking.
#[derive(Resource, Default)]
struct Steps {
    last: Option<Vec3>,
    distance: f32,
    on_ground: bool,
    /// Seconds off the ground so far.
    airborne: f32,
}

/// A landing sound needs at least this long in the air (s; ours): stepping up stairs and over small
/// ledges leaves the ground for a frame or two, which isn't a landing.
const LAND_AFTER: f32 = 0.2;

/// Distance between footsteps while walking, cm (HYPOTHESIS; the game uses animation notifies).
const STEP_CM: f32 = 130.0;
/// More than this in one frame is a move between places, not a walk (cm; ours).
const TELEPORT_CM: f32 = 300.0;
/// Footstep surface: the test room floor is stone.
const FLOOR: &[&[&str]] = &[&["Boots", "Stone"], &["Stone"]];

/// The footstep events' sounds per surface (`_data/recovered/footsteps.json`: the `Footstep` switch's
/// `Boots_<surface>` media): event -> (the default switch's surface, surface -> media ids).
#[derive(Resource, Debug, Default)]
struct FootstepTable(BTreeMap<String, (String, BTreeMap<String, Vec<u64>>)>);

impl FootstepTable {
    fn load() -> Self {
        let Some(j) = std::fs::read(db_formats::data_path("recovered/footsteps.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { return Self::default() };
        Self(
            j.as_object()
                .into_iter()
                .flatten()
                .map(|(event, t)| {
                    let by = t["surfaces"].as_object().into_iter().flatten().map(|(s, ids)| (s.clone(), ids.as_array().into_iter().flatten().filter_map(|i| i.as_u64()).collect())).collect();
                    (event.clone(), (t["default"].as_str().unwrap_or("Stone").to_string(), by))
                })
                .collect(),
        )
    }

    /// The media ids `event` plays on `surface` (a surface with none of its own: the default's), or none.
    fn ids(&self, event: &str, surface: &str) -> &[u64] {
        self.0.get(event).and_then(|(default, by)| by.get(surface).or_else(|| by.get(default))).map_or(&[], Vec::as_slice)
    }
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SoundBank::load())
            .init_resource::<AudioMuted>()
            .init_resource::<MutedHandles>()
            .init_resource::<Steps>()
            .init_resource::<Cues>()
            .init_resource::<Ears>()
            .init_resource::<MapAmbience>()
            .insert_resource(FootstepTable::load())
            .insert_resource(SoundLimits::load())
            .insert_resource(SoundRng(SimRng::new(0x50_4E_44)))
            .add_systems(Update, (add_listener, update_ears, event_sounds, staff_sounds, shot_sounds, thief_sounds, dragon_sounds, portal_sounds, body_loops, play_cues, footsteps, run_sounds, guard_sound, crossbow_sounds, draw_sounds, ui_sounds, background, emitters, countdown, collapse_sounds).chain().after(crate::combat_view::drain_sim_events));
    }
}

#[derive(SystemParam)]
struct Player<'w, 's> {
    commands: Commands<'w, 's>,
    bank: Res<'w, SoundBank>,
    muted: Res<'w, AudioMuted>,
    rng: ResMut<'w, SoundRng>,
    handles: ResMut<'w, MutedHandles>,
    assets: Res<'w, AssetServer>,
    ears: Res<'w, Ears>,
    mix: Res<'w, crate::settings::Settings>,
    limits: ResMut<'w, SoundLimits>,
    live: Query<'w, 's, (), With<AudioPlayer>>,
    time: Res<'w, Time>,
}

use bevy::ecs::system::SystemParam;

impl Player<'_, '_> {
    /// Plays one variant of `event` for the player (not placed in the world); returns the spawned entity
    /// when not muted.
    fn play(&mut self, event: &str, filters: &[&[&str]], looped: bool) -> Option<Entity> {
        self.play_from(None, event, filters, looped)
    }

    /// Plays one variant of `event` from `at` (sim cm; `None`: not placed): placed sounds pan and fade with
    /// distance and aren't played beyond [`HEARING`] from the player.
    fn play_from(&mut self, at: Option<Vec3>, event: &str, filters: &[&[&str]], looped: bool) -> Option<Entity> {
        self.play_among(at, event, filters, &[], looped)
    }

    /// [`Player::play_from`] among the variants with these media ids (none of them in the bank, or `ids` empty: by
    /// `filters`).
    fn play_among(&mut self, at: Option<Vec3>, event: &str, filters: &[&[&str]], ids: &[u64], looped: bool) -> Option<Entity> {
        // "" = this monster has no such voice.
        if event.is_empty() {
            return None;
        }
        if let (Some(p), Some(me)) = (at, self.ears.0)
            && p.distance(me) > HEARING
        {
            return None;
        }
        let mut v = self.bank.variants(event, filters);
        if !ids.is_empty() {
            let among: Vec<_> = self.bank.variants(event, &[]).into_iter().filter(|(id, _)| ids.contains(id)).collect();
            if !among.is_empty() {
                v = among;
            }
        }
        if v.is_empty() {
            warn!("sound: {event} has no media");
            return None;
        }
        let (id, source) = v[self.rng.0.range_u32(0, v.len() as u32) as usize];
        // Its playback limit: a global one, or a 2D sound's (the one listener object). A placed sound's
        // per-object limit isn't kept: the calls name no emitter (HYPOTHESIS that this rarely matters).
        let now = self.time.elapsed_secs();
        let limit = self.limits.by_media.get(id).copied().filter(|l| (l.global || at.is_none()) && !self.muted.0);
        if let Some(l) = limit {
            let live = &self.live;
            let list = self.limits.playing.entry(l.group).or_default();
            // (A sound spawned this frame isn't in the world yet.)
            list.retain(|(e, t)| live.contains(*e) || now - t < 0.25);
            match admit(list, l) {
                Admit::Drop => {
                    info!("sound: {event} dropped ({} of {} playing)", list.len(), l.max);
                    return None;
                }
                Admit::StopOldest(e) => {
                    list.remove(0);
                    self.commands.entity(e).try_despawn();
                }
                Admit::Play => {}
            }
        }
        info!("sound: {event} -> {id}.ogg ({source}){}", if self.muted.0 { " [muted]" } else { "" });
        let handle: Handle<AudioSource> = self.assets.load(format!("audio://{id}.ogg"));
        if self.muted.0 {
            self.handles.0.push(handle);
            let n = self.handles.0.len();
            if n > 64 {
                self.handles.0.drain(..n - 64);
            }
            return None;
        }
        // Volume from the settings: music follows the master volume, everything else master × effects.
        let group = crate::settings::SoundGroup { music: event.starts_with("Play_Music") };
        let settings = if looped { PlaybackSettings::LOOP } else { PlaybackSettings::DESPAWN }.with_volume(bevy::audio::Volume::Linear(self.mix.volume(group.music)));
        let e = match at {
            Some(p) => self.commands.spawn((AudioPlayer::new(handle), settings.with_spatial(true).with_spatial_scale(SpatialScale::new(SPATIAL_SCALE)), Transform::from_translation(crate::to_bevy(p)), group)).id(),
            None => self.commands.spawn((AudioPlayer::new(handle), settings, group)).id(),
        };
        if let Some(l) = limit {
            self.limits.playing.entry(l.group).or_default().push((e, now));
        }
        Some(e)
    }

    /// Stops a sound this played (and frees its place in its limit group).
    fn stop(&mut self, e: Entity) {
        for list in self.limits.playing.values_mut() {
            list.retain(|(x, _)| *x != e);
        }
        self.commands.entity(e).try_despawn();
    }

    /// The global limit of `event`'s sounds (`ids` when given, else its first variant's).
    fn global_limit(&self, event: &str, ids: &[u64]) -> Option<Limit> {
        let id = ids.first().copied().or_else(|| self.bank.variants(event, &[]).first().map(|v| v.0))?;
        self.limits.by_media.get(&id).copied().filter(|l| l.global)
    }
}

/// The floor collapse: each stage's mechanism plays its `TurnOnSound` (`Play_L_Common_Throne_Rock`) where it
/// stands.
fn collapse_sounds(events: Res<crate::combat_view::FrameEvents>, collapses: Query<&db_sim::floor_collapse::FloorCollapse>, mut out: Player) {
    for ev in &events.0 {
        if let db_sim::SimEvent::FloorCollapsed { floor, stage } = ev
            && let Some(c) = collapses.iter().find(|c| c.floor == *floor)
            && let Some((_, _, at, sound)) = c.spec.view.stages.iter().find(|s| s.0 == *stage)
        {
            out.play_from(Some(*at + c.off), sound, &[], false);
        }
    }
}

/// Portals (the extraction portal Blueprint's sounds, CONFIRMED names): `Play_Portal_Active` when one opens (a
/// delayed escape point, the Goblin Thief's), then `Play_Portal_Active_Loop` while it stays open and in earshot
/// (tried again every 2 s while out of it or muted); the hum stops when it closes or goes.
#[allow(clippy::type_complexity)]
fn portal_sounds(mut commands: Commands, time: Res<Time>, portals: Query<(&db_sim::SimId, &Position, &db_sim::Portal)>, hums: Query<(Entity, &PortalHum)>, mut state: Local<(std::collections::BTreeSet<db_sim::SimId>, BTreeMap<db_sim::SimId, f32>)>, mut out: Player) {
    let now = time.elapsed_secs();
    let (opened, retry) = &mut *state;
    for (e, hum) in &hums {
        if !portals.iter().any(|(id, _, p)| *id == hum.0 && p.open()) {
            commands.entity(e).despawn();
        }
    }
    for (id, pos, portal) in &portals {
        if !portal.open() {
            opened.remove(id);
            retry.remove(id);
            continue;
        }
        if opened.insert(*id) {
            out.play_from(Some(pos.0), "Play_Portal_Active", &[], false);
        }
        if hums.iter().any(|(_, h)| h.0 == *id) || retry.get(id).is_some_and(|t| now < *t) {
            continue;
        }
        match out.play_from(Some(pos.0), "Play_Portal_Active_Loop", &[], true) {
            Some(e) => {
                out.commands.entity(e).insert(PortalHum(*id));
                retry.remove(id);
            }
            None => {
                retry.insert(*id, now + 2.0);
            }
        }
    }
}

/// The Wendigo's thunder strike (HYPOTHESIS: the bank's `Play_M_Wendigo_Attack8_Lightning`, which no asset we read
/// names, as his area's strike).
const WENDIGO_LIGHTNING: &str = "Play_M_Wendigo_Attack8_Lightning";

/// A monster shot's flight loop (its actor's `WooshSounds`), following it.
#[derive(Component)]
struct ShotLoop(db_sim::SimId);

/// Monster shots' own sounds (their actors' properties, `monster_actor_vfx.py`): `LaunchSound` and `FlybySound` once
/// as it launches (a hanging shot, the Wendigo's Attack5, after its hang; `FlybySound` there: HYPOTHESIS, the engine
/// plays it as it passes), `WooshSounds` looped on it while it flies.
#[allow(clippy::type_complexity)]
fn shot_sounds(
    index: Res<SimIndex>,
    shots: Query<(&db_sim::SimId, &Position, &db_sim::Projectile)>,
    (actor_vfx, kits): (Res<crate::effects::MonsterActorVfx>, Query<&MonsterKit>),
    mut loops: Query<(Entity, &ShotLoop, &mut Transform)>,
    mut launched: Local<std::collections::BTreeSet<db_sim::SimId>>,
    mut out: Player,
) {
    for (e, l, mut t) in &mut loops {
        match index.get(l.0).and_then(|s| shots.get(s).ok()) {
            Some((_, p, _)) => t.translation = crate::to_bevy(p.0),
            None => out.commands.entity(e).despawn(),
        }
    }
    for (&id, p, shot) in &shots {
        if launched.contains(&id) {
            continue;
        }
        let Some(a) = actor_vfx.shot(None, &index, &kits, shot) else { continue };
        // Its hang: the last still key before it flies.
        let hang = if shot.spec.speed.first().is_some_and(|k| k.1 <= 0.0) { shot.spec.speed.iter().take_while(|k| k.1 <= 0.0).last().map_or(0.0, |k| k.0) } else { 0.0 };
        if (shot.age as f32) * db_sim::DT < hang {
            continue;
        }
        launched.insert(id);
        for key in ["LaunchSound", "FlybySound"] {
            if let Some(ev) = a.sound(key) {
                out.play_from(Some(p.0), ev, &[], false);
            }
        }
        if let Some(ev) = a.sound("WooshSounds")
            && let Some(h) = out.play_from(Some(p.0), ev, &[], true)
        {
            out.commands.entity(h).insert(ShotLoop(id));
        }
    }
    launched.retain(|id| index.get(*id).is_some());
}

/// Body loops: a living monster's `AliveLoopSound` (the flying insects' `Play_M_Insect_Fly_Loop`, the lava totems'
/// `Play_Torch_Burning_Crackling_Loop_No_Cone`) and a burning character's `Play_Item_Body_Burning_Loop`. Each follows
/// its actor and stops when the actor dies, stops burning or goes out of hearing; one out of hearing (or muted) is
/// tried again every 2 s.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn body_loops(
    mut commands: Commands,
    time: Res<Time>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    actors: Query<(&Position, Option<&MonsterKit>, Option<&db_sim::Buffs>, Option<&db_sim::Health>)>,
    mut loops: Query<(Entity, &BodyLoop, &mut Transform)>,
    mut retry: Local<BTreeMap<(db_sim::SimId, String), f32>>,
    mut out: Player,
) {
    let now = time.elapsed_secs();
    let mut want: Vec<(db_sim::SimId, String, Vec3)> = Vec::new();
    for (id, e) in index.iter() {
        let Ok((pos, kit, buffs, health)) = actors.get(e) else { continue };
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        if let Some(ev) = kit.and_then(|k| k.bp.alive_loop_sound.clone()) {
            want.push((id, ev, pos.0));
        }
        if buffs.is_some_and(|b| b.0.iter().any(|b| matches!(b.kind, db_sim::BuffKind::Burning { .. }))) {
            want.push((id, BURN_LOOP.to_string(), pos.0));
        }
    }
    let ears = out.ears.0;
    for (e, l, mut t) in &mut loops {
        match want.iter().find(|w| w.0 == l.0 && w.1 == l.1) {
            Some(w) if Some(w.0) == player.0 => {}
            Some(w) if ears.is_none_or(|me| me.distance(w.2) <= HEARING) => t.translation = crate::to_bevy(w.2),
            _ => commands.entity(e).despawn(),
        }
    }
    for (id, ev, at) in want {
        let key = (id, ev);
        if loops.iter().any(|(_, l, _)| l.0 == key.0 && l.1 == key.1) || retry.get(&key).is_some_and(|t| now < *t) {
            continue;
        }
        // Your own burn isn't placed.
        let from = (Some(id) != player.0).then_some(at);
        match out.play_from(from, &key.1, &[], true) {
            Some(e) => {
                // Through `out`'s buffer, which spawns the sound (this system's `commands` applies first).
                out.commands.entity(e).insert(BodyLoop(key.0, key.1.clone()));
                retry.remove(&key);
            }
            None => {
                retry.insert(key, now + 2.0);
            }
        }
    }
    retry.retain(|_, t| now < *t + 10.0);
}

/// How near the Shadow Dragon one hears it (cm; ours). Its sounds play unplaced within it: it flies 30-70 m off, past
/// [`HEARING`] and our steep falloff, and its events' own attenuation isn't decoded (HYPOTHESIS: heard across its hall).
const DRAGON_HEARING: f32 = 10000.0;

/// The Shadow Dragon's montage cues: those its script's montage time passed since the last frame (the circle loops;
/// a new montage starts from its beginning), heard within [`DRAGON_HEARING`].
fn dragon_sounds(dragons: Query<(&db_sim::SimId, &db_sim::shadow_dragon::ShadowDragon, &db_sim::Health)>, mut out: Player, mut last: Local<BTreeMap<db_sim::SimId, (usize, f32)>>) {
    for (id, d, h) in &dragons {
        let Some((clip, t)) = d.montage().filter(|_| h.current > 0.0) else {
            last.remove(id);
            continue;
        };
        let key = std::ptr::from_ref(clip) as usize;
        let from = match last.insert(*id, (key, t)) {
            Some((k, t0)) if k == key => t0,
            _ => -1.0,
        };
        let passed = |s: f32| if t >= from { s > from && s <= t } else { s > from || s <= t };
        if out.ears.0.is_some_and(|me| me.distance(d.pose.at) <= DRAGON_HEARING) {
            for (_, ev) in clip.sounds.iter().filter(|s| passed(s.0)) {
                out.play_from(None, ev, &[], false);
            }
        }
    }
}

/// A Goblin Thief ability's montage cues (open the portal, cheer, step through), when it starts.
fn thief_sounds(thieves: Query<(&db_sim::SimId, &db_sim::ai::MonsterAi, &MonsterKit)>, mut cues: ResMut<Cues>, mut seen: Local<BTreeMap<db_sim::SimId, u32>>) {
    for (id, ai, kit) in &thieves {
        let (Some(act), Some(thief)) = (ai.thief.active, kit.thief.as_deref()) else { continue };
        if seen.insert(*id, act.start) == Some(act.start) {
            continue;
        }
        if let Some(a) = thief.abilities.get(act.ability) {
            cues.0.extend(a.sounds.iter().map(|(t, ev)| (*t, *id, ev.clone())));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn event_sounds(
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    kinds: Query<&Kind>,
    kits: Query<&MonsterKit>,
    buffs: Query<&db_sim::Buffs>,
    mut cues: ResMut<Cues>,
    mut out: Player,
    loops: Query<(Entity, &SkillLoop)>,
    (positions, traps, staffs): (Query<&db_sim::Position>, Query<&db_sim::Trap>, Query<&db_sim::StaffState>),
    (mechs, levers, props): (Query<&db_sim::mechanisms::Mechanism>, Query<&db_sim::mechanisms::Lever>, Query<&db_sim::Destroyable>),
    flasks: Option<Res<db_sim::throwables::FlaskDb>>,
) {
    let flask_kind = |item: i32| flasks.as_ref().and_then(|f| f.0.get(&item)).map(|f| f.kind().to_string()).unwrap_or_default();
    let kind_of = |id: db_sim::SimId| index.get(id).and_then(|e| kinds.get(e).ok()).map(|k| k.0);
    // Where another actor's sound comes from; the player's own sounds aren't placed.
    let from = |id: db_sim::SimId| if Some(id) == player.0 { None } else { index.get(id).and_then(|e| positions.get(e).ok()).map(|p| p.0) };
    let kit_of = |id: db_sim::SimId| index.get(id).and_then(|e| kits.get(e).ok());
    let bp_of = |id: db_sim::SimId| kit_of(id).map(|k| &k.bp);
    for e in &events.0 {
        match *e {
            // Montage cues of the monster's own kit at their notify times.
            SimEvent::MonsterAttackStarted { id, index } => {
                if let Some(a) = kit_of(id).and_then(|k| k.attacks.get(usize::from(index))) {
                    cues.0.extend(a.sounds.iter().map(|(t, ev)| (*t, id, ev.clone())));
                }
            }
            SimEvent::AiStateChanged { id, state: AiState::Alert, .. } => {
                if let Some(ev) = bp_of(id).and_then(|b| b.combat_sound.as_deref()) {
                    out.play_from(from(id), ev, &[], false);
                }
            }
            SimEvent::Died { id } => {
                cues.0.retain(|c| c.1 != id);
                match kind_of(id) {
                    Some(ActorKind::Monster) => {
                        for ev in bp_of(id).map_or(&[][..], |b| &b.death_sounds[..]) {
                            out.play_from(from(id), ev, &[], false);
                        }
                    }
                    // A breakable prop's `DeathSound`.
                    Some(ActorKind::Destroyable) => {
                        if let Some(d) = index.get(id).and_then(|e| props.get(e).ok()) {
                            out.play_from(from(id), &d.spec.sound, &[], false);
                        }
                    }
                    other => info!("sound: (no death sound for {other:?})"),
                }
            }
            SimEvent::Damaged { source, target, blocked: false, by: HitSource::Monster(_), .. } => {
                out.play_from(from(target), impact(kit_of(source).map_or("", |k| k.name.as_str())), &[&["Flesh"]], false);
            }
            SimEvent::Damaged { target, blocked: false, by: HitSource::Swing(kind), .. } if kind_of(target) == Some(ActorKind::Monster) => {
                out.play_from(from(target), "Play_Impact_Type_Sword", &[&["Flesh"]], false);
                // Its hit-react montage's voice: the heavy one for a heavy attack, else the light one (either
                // standing in for the other when a monster has only one).
                if let Some(b) = bp_of(target) {
                    let (light, heavy) = (&b.hit_sounds, &b.hit_heavy_sounds);
                    let evs = if (kind == AttackKind::Heavy && !heavy.is_empty()) || light.is_empty() { heavy } else { light };
                    for ev in evs {
                        out.play_from(from(target), ev, &[], false);
                    }
                }
                // The Rogue's poison just petrified it (the game's "Cockatrice" Q).
                if index.get(target).and_then(|e| buffs.get(e).ok()).is_some_and(|b| b.0.iter().any(|x| matches!(x.kind, db_sim::BuffKind::Petrified { .. }) && x.age <= 1)) {
                    out.play_from(from(target), "Play_Rogue_New_Spell_Q_Cockatrice_Start", &[], false);
                }
            }
            // A staff's combo has its own (`staff_sounds`).
            SimEvent::SwingStarted { id, .. } if index.get(id).and_then(|e| staffs.get(e).ok()).is_some_and(|s| s.held.is_some()) => {}
            SimEvent::SwingStarted { kind, .. } => {
                let event = if kind == AttackKind::Light { "Play_LongSword_QuickAttack" } else { "Play_LongSword_HeavyAttack_Stab" };
                out.play(event, &[], false);
            }
            SimEvent::Damaged { target, blocked: true, .. } => {
                out.play_from(from(target), "Play_Impact_Type_WoodenShield", &[&["WoodenShield"]], false);
            }
            // The local player's head hit.
            SimEvent::HeadHit { source, target } if Some(source) == player.0 => {
                out.play_from(from(target), "Play_Headshot_Common", &[], false);
            }
            SimEvent::Damaged { by: HitSource::Skill(SkillKind::Whirlwind), .. } => {
                out.play("Play_Fighter_New_Spell_Q_Single", &[], false);
            }
            // Rend: the impact when it fires; its bleed pulses stay silent.
            SimEvent::Damaged { by: HitSource::Skill(SkillKind::Rend), .. } => {
                out.play("Play_Impact_Fighter_Blood_Spell_Q", &[], false);
            }
            SimEvent::Damaged { by: HitSource::Skill(SkillKind::ForceOfNature), .. } => {
                out.play("Play_M_Treant_Attack2_02", &[], false);
            }
            SimEvent::Damaged { by: HitSource::Skill(SkillKind::SoulChain), .. } => {
                out.play("Play_DeathKnight_Spell_E_Catch", &[], false);
            }
            SimEvent::Damaged { by: HitSource::Bleed(_), .. } => {}
            // Staff spell hits sound in `staff_sounds` (the beam's impact, the cloud's strike).
            SimEvent::Damaged { by: HitSource::Staff(_), .. } => {}
            // matched by name (HYPOTHESIS for the fire statue and the Clarity and Speed altars,
            // which have no sounds of their own).
            SimEvent::Damaged { target, by: HitSource::Trap(_), .. } => {
                out.play_from(from(target), "Play_Impact_Type_Sword", &[&["Flesh"]], false);
            }
            SimEvent::DoorToggled { door, open, .. } => {
                out.play_from(from(door), if open { "Play_Door_Open" } else { "Play_Door_Close" }, &[], false);
            }
            // the lever's and the mechanism's own sound events (`TurnOnSound` / `TurnOffSound`; HYPOTHESIS: the
            // mechanisms' loops left out).
            SimEvent::LeverPulled { lever, on, .. } => {
                let role = if on { "TurnOnSound" } else { "TurnOffSound" };
                if let Some(ev) = index.get(lever).and_then(|e| levers.get(e).ok()).and_then(|l| l.placement.sounds.iter().find(|s| s.0 == role).map(|s| s.1.clone())) {
                    out.play_from(from(lever), &ev, &[], false);
                }
            }
            SimEvent::MechanismMoved { id, on } => {
                if let Some(m) = index.get(id).and_then(|e| mechs.get(e).ok()) {
                    let ev = m.placement.sound(if on { "TurnOnSound" } else { "TurnOffSound" }).or_else(|| m.placement.sound(if on { "TurnOnLoopSound" } else { "TurnOffLoopSound" }));
                    if let Some(ev) = ev.map(str::to_string) {
                        out.play_from(from(id), &ev, &[], false);
                    }
                }
            }
            SimEvent::TrapCycled { id, up } => {
                let event = match (index.get(id).and_then(|e| traps.get(e).ok()).map(|t| t.kind), up) {
                    (Some(db_sim::TrapKind::Spikes), true) => "Play_Trap_FloorSpike_Appear",
                    (Some(db_sim::TrapKind::Spikes), false) => "Play_Trap_FloorSpike_Disappear",
                    (Some(db_sim::TrapKind::Blade), true) => "Play_Trap_SwingBlade_Whoosh",
                    (Some(db_sim::TrapKind::FireStatue), true) => "Play_Torch_Burning_Light",
                    (Some(db_sim::TrapKind::FireStatue), false) => "Play_Torch_Burning_Extinguish",
                    _ => "",
                };
                out.play_from(from(id), event, &[], false);
            }
            SimEvent::AltarUsed { kind, .. } => {
                let event = match kind {
                    db_sim::AltarKind::Attack => "Play_Spell_Buff_Attack_Up_Start_Altar",
                    db_sim::AltarKind::Defend => "Play_Spell_Buff_Defence_Up_Start_Altar",
                    db_sim::AltarKind::Heal => "Play_Spell_Buff_Health_Up_Start_Altar",
                    db_sim::AltarKind::Clarity | db_sim::AltarKind::Speed => "Play_Spell_Buff_Shield_Up_Start_Altar",
                };
                out.play(event, &[], false);
            }
            SimEvent::Damaged { target, .. } => {
                // The dummy is wood.
                out.play_from(from(target), "Play_Impact_Type_Sword", &[&["_Wood_"], &["Wood"]], false);
            }
            SimEvent::SkillStarted { id, skill: SkillKind::Whirlwind } => {
                out.play("Play_Fighter_New_Spell_Q_Start", &[], false);
                if let Some(l) = out.play("Play_Fighter_New_Spell_Q_Loop", &[], true) {
                    out.commands.entity(l).insert(SkillLoop(id, SkillKind::Whirlwind));
                }
            }
            SimEvent::SkillEnded { id, skill: SkillKind::Whirlwind } => {
                for (l, tag) in &loops {
                    if tag.0 == id && tag.1 == SkillKind::Whirlwind {
                        out.commands.entity(l).despawn();
                    }
                }
                out.play("Play_Fighter_New_Spell_Q_End", &[], false);
            }
            // the flask throw's montage sound (`AM_AlchemicalPotion_Sub_Throw_1st`, 0.184 s; played on the press), and
            // where it lands its explode cue's (`GCNL_TGF_Damage_ThunderPotion`, `_Explode_BurningPotion`) or the smoke
            // area's. A torch: its montage's `Play_Item_Throw_Torch`; it lands quietly.
            SimEvent::ThrowStarted { id, item_id } => {
                let torch = flask_kind(item_id) == "Torch";
                out.play_from(from(id), if torch { "Play_Item_Throw_Torch" } else { "Play_Item_Throw_Potion" }, &[], false);
            }
            SimEvent::FlaskLanded { item_id, at, .. } if flask_kind(item_id) != "Torch" => {
                let sound = match flask_kind(item_id).as_str() {
                    "ThunderPotion" => "Play_Item_Thunder_Potion_Explosion",
                    "SmokePotion" => "Play_Item_Smoke_Potion_Smoke",
                    // The walls' own (`BP_Destructible_Wall`).
                    "EarthPotion" => "Play_Item_Alchemical_Potion_Wall_Rise",
                    _ => "Play_Item_Explode_Potion_Explosion",
                };
                out.play_from(Some(at), sound, &[], false);
            }
            // the ground hit's damage area's own `AK_Event_Damage`, where it lands.
            SimEvent::GroundHit { at, .. } => {
                out.play_from(Some(at), "Play_Shield_Heavy_DamageArea", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::Charge, .. } => {
                out.play("Play_Sword_Shield_Charge_Sprint_Attack", &[], false);
            }
            // matched by name (HYPOTHESIS): War Cry is the Q buff, Inspire the E buff, Rend the "Blood" Q.
            SimEvent::SkillStarted { skill: SkillKind::WarCry, .. } => {
                out.play("Play_Fighter_Spell_Q_Buff_Start", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::Inspire, .. } => {
                out.play("Play_Fighter_Spell_E_Buff_Start_Male", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::Rend, .. } => {
                out.play("Play_Fighter_Blood_Spell_Q_Start", &[], false);
            }
            // the hold's start is its charge montage's (`AM_Classes_SwordMaster_Skill1_ChargeBegin_1st`,
            // CONFIRMED); the launch on release matched by name (HYPOTHESIS).
            SimEvent::SkillStarted { skill: SkillKind::FlyingSwords, .. } => {
                out.play("Play_SwordMaster_Spell_Q_Charge", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::FlyingSwords, .. } => {
                out.play("Play_SwordMaster_Spell_Q_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::BladeWaltz, .. } => {
                out.play("Play_SwordMaster_Spell_E_Charge_Start", &[], false);
            }
            SimEvent::SkillEnded { skill: SkillKind::BladeWaltz, .. } => {
                out.play("Play_SwordMaster_Spell_E_Charge_End", &[], false);
            }
            // matched by name (HYPOTHESIS); the Lich's sounds are filed as the ice Wizard's.
            SimEvent::SkillStarted { skill: SkillKind::Pyroblast, .. } => {
                out.play("Play_Wizard_Q_1st_Charging", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::Pyroblast, stage, .. } => {
                out.play(if stage >= 2 { "Play_Wizard_Q_2nd_Charge_Release" } else { "Play_Wizard_Q_1st_Charge_Release" }, &[], false);
            }
            SimEvent::Burst { by: HitSource::Skill(SkillKind::Pyroblast), radius, .. } if radius > crate::skill_vfx::SMALL_BURST => {
                out.play("Play_Wizard_Q_2nd_Fire_Explosion", &[], false);
            }
            // The Beholder's pillars of fire bursting (`B_TGF_StaticCollision_Projectile_BeholderPillarOfFire`
            // posts it when its collision delay ends).
            SimEvent::Burst { owner, by: HitSource::Monster(_), .. } if kit_of(owner).is_some_and(|k| k.name == "Beholder") => {
                out.play("Play_M_Beholder_Attack1_FireBurst", &[], false);
            }
            // The shaman's fireball.
            SimEvent::Burst { owner, by: HitSource::Monster(_), .. } if kit_of(owner).is_some_and(|k| k.name == "GoblinShaman") => {
                out.play("Play_M_Goblin_Shaman_Attack2_Explo", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::FlameShock, .. } => {
                out.play("Play_Wizard_E_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::Blizzard, .. } => {
                out.play("Play_Wizard_Ice_Q_1st_Release", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::Blizzard, .. } => {
                out.play("Play_Wizard_Ice_Q_2nd_Release", &[], false);
            }
            SimEvent::SkillEnded { skill: SkillKind::Blizzard, .. } => {
                out.play("Play_Wizard_Ice_Q_2nd_End", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::IceBarrier, .. } => {
                out.play("Play_Wizard_Ice_E_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::FrostTorment, .. } => {
                out.play("Play_Wizard_Ice_Q2_Start", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::FrostTorment, .. } => {
                out.play("Play_Wizard_Ice_Q2_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::IceArmor, .. } => {
                out.play("Play_Wizard_Ice_E2_Release_Buff", &[], false);
            }
            // matched by name (HYPOTHESIS): the new Q (Heal) charges in two stages, the old one
            // (`Fix`) is the Oracle.
            SimEvent::SkillStarted { skill: SkillKind::Oracle, .. } => {
                out.play("Play_Priest_Fix_Spell_Q_Staff_Start", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::Oracle, .. } => {
                out.play("Play_Priest_Fix_Spell_Q_Blast", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::HealSingle, .. } => {
                out.play("Play_Priest_New_Spell_Q_2nd_Charge", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::HealSingle, .. } => {
                out.play("Play_Priest_New_Spell_Q_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::DivineProtection, .. } => {
                out.play("Play_Priest_Spell_E_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::Guardian, .. } => {
                out.play("Play_Priest_Spell_E_Holy_Heal", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::WraithRing, .. } => {
                out.play("Play_DeathKnight_Spell_Q_1st_Explosion", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::SoulStorm, .. } => {
                out.play("Play_DeathKnight_Spell_Q_2nd_Explosion", &[], false);
            }
            // matched by name (HYPOTHESIS).
            SimEvent::SkillStarted { skill: SkillKind::WildAwakening, .. } => {
                out.play("Play_Druid_Spell_Q_Change", &[], false);
            }
            SimEvent::SkillEnded { skill: SkillKind::WildAwakening, .. } => {
                out.play("Play_Druid_Spell_Q_End", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::ShadowPounce, .. } => {
                out.play("Play_Druid_Spell_E_Jump", &[], false);
            }
            SimEvent::SkillCast { skill: SkillKind::ShadowPounce, .. } => {
                out.play("Play_Druid_Spell_E_HitGround", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::ForceOfNature, .. } => {
                out.play("Play_M_Treant_WakeUp", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::SoulChain, .. } => {
                out.play("Play_DeathKnight_Spell_E_Grab", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::PetrifyingPoison, .. } => {
                out.play("Play_Rogue_New_Spell_Q_Ready", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::FlyingBlades, .. } => {
                out.play("Play_Rogue_Spell_Q_Dagger", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::SwiftConcealment, .. } => {
                out.play("Play_Rogue_Spell_E_Release", &[], false);
            }
            SimEvent::SkillStarted { skill: SkillKind::FadeIntoVoid | SkillKind::ShadowVeil, .. } => {
                out.play("Play_Rogue_Spell_E_Charge", &[], false);
            }
            SimEvent::SkillRejected { id, skill } if Some(id) == player.0 => info!("skill {skill:?} not ready"),
            _ => {}
        }
    }
}

/// The impact a monster type's attack makes on the player (Wwise impact types by its weapon; HYPOTHESIS:
/// matched by eye). Its voices come from its Blueprint and hit-react montages (`MonsterBp::combat_sound` etc.).
fn impact(row: &str) -> &'static str {
    match row {
        "GoblinAxe" | "Zombie" | "Boar" | "Werewolf" | "Wendigo" | "Executioner" => "Play_Impact_Type_Axe",
        "SkeletonSword" | "SkeletonPolearm" | "ArmorSword" | "Reaper" | "ShadowKing" | "Ghost" => "Play_Impact_Type_Sword",
        "SkeletonHammerShield" | "ArmorHammer" | "Cyclop" | "Khazra" | "Moose" | "MooseGhost" | "Treant" => "Play_Impact_Type_Hammer",
        "GoblinThief" => "",
        _ => "Play_Impact_Type_Dagger",
    }
}

/// Staff sounds, the Wwise events the staff montages, `B_Staff_Laser_Effect`, `B_TGF_Staff_ThunderArea` and the
/// dashboard Lua name (CONFIRMED names): the combo's `Play_Staff_Attack_1/2`; the beam's start, loop, end (or the
/// cancel section's release) and its impact; the thunder's ready, release, cancel, the cloud's loop and strikes; the
/// recharge's start, loop, end; the energy bar's `Play_Staff_Skill_Energy_Ready` for each segment filled and `_Full`
/// at full; `_Error` for a refused cast (HYPOTHESIS: which cue the refusal plays).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn staff_sounds(
    time: Res<Time>,
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    positions: Query<&Position>,
    staffs: Query<&db_sim::StaffState>,
    loops: Query<(Entity, &StaffLoop)>,
    mut hums: Query<(Entity, &mut CloudHum)>,
    mut last: Local<BTreeMap<db_sim::SimId, u32>>,
    (actor_vfx, kits, areas, mut own): (Res<crate::effects::MonsterActorVfx>, Query<&MonsterKit>, Option<Res<db_sim::staffs::ThunderAreas>>, Local<Vec<(db_sim::SimId, Vec3)>>),
    mut out: Player,
) {
    use db_sim::StaffElement as E;
    // A monster's thunder area with its own actor's sound (the Wendigo's, `LoopSound` `Play_M_Wendigo_Attack8_Ground`).
    let area_sound = |owner: db_sim::SimId, at: Vec3| -> Option<String> {
        let a = areas.as_ref()?.0.iter().find(|a| a.owner == owner && a.at == at)?;
        actor_vfx.of_attack(None, &index, &kits, owner, a.by, &crate::staff_view::AREA_VIA)?.sound("LoopSound").map(str::to_string)
    };
    let from = |id: db_sim::SimId| if Some(id) == player.0 { None } else { index.get(id).and_then(|e| positions.get(e).ok()).map(|p| p.0) };
    let staff = |id: db_sim::SimId| index.get(id).and_then(|e| staffs.get(e).ok());
    let stop = |out: &mut Player, id: db_sim::SimId, kind: u8| {
        for (l, tag) in &loops {
            if tag.0 == id && tag.1 == kind {
                out.commands.entity(l).despawn();
            }
        }
    };
    for e in &events.0 {
        match *e {
            SimEvent::SwingStarted { id, .. } if staff(id).is_some_and(|s| s.held.is_some()) => {
                let n = staff(id).and_then(|s| s.last_combo).map_or(0, |c| c.0);
                out.play_from(from(id), if n == 0 { "Play_Staff_Attack_1" } else { "Play_Staff_Attack_2" }, &[], false);
            }
            SimEvent::StaffCastStarted { id, element: E::Fire } => {
                out.play_from(from(id), "Play_Staff_Skill_Fire_Start", &[], false);
                if let Some(l) = out.play_from(from(id), "Play_Staff_Skill_Fire_Loop", &[], true) {
                    out.commands.entity(l).insert(StaffLoop(id, 0));
                }
            }
            SimEvent::StaffCastStarted { id, .. } => {
                out.play_from(from(id), "Play_Staff_Skill_Thunder_Ready", &[], false);
            }
            SimEvent::StaffBeamEnded { id, cancelled } => {
                stop(&mut out, id, 0);
                out.play_from(from(id), if cancelled { "Play_Staff_Skill_Fire_Release" } else { "Play_Staff_Skill_Fire_End" }, &[], false);
            }
            SimEvent::Damaged { target, by: HitSource::Staff(db_sim::StaffHit::Beam | db_sim::StaffHit::BeamExtra), .. } => {
                out.play_from(from(target), "Play_Impact_Staff_Skill_Fire", &[], false);
            }
            SimEvent::StaffThunderReleased { id, .. } => {
                out.play_from(from(id), "Play_Staff_Skill_Thunder_Release", &[], false);
            }
            SimEvent::StaffThunderFailed { id } => {
                out.play_from(from(id), "Play_Staff_Skill_Thunder_Cancel", &[], false);
            }
            SimEvent::ThunderAreaSpawned { owner, at, life, .. } => {
                let mine = area_sound(owner, at);
                if mine.is_some() {
                    own.push((owner, at));
                }
                if let Some(h) = out.play_from(Some(at), mine.as_deref().unwrap_or("Play_Staff_Skill_Thunder_Loop"), &[], true) {
                    out.commands.entity(h).insert(CloudHum(life));
                }
            }
            SimEvent::ThunderStruck { owner, at, .. } => {
                // The Wendigo's strike: his `Attack8_Lightning` (no asset we read plays it; HYPOTHESIS by name).
                let mine = own.iter().position(|o| *o == (owner, at)).map(|i| own.swap_remove(i));
                out.play_from(Some(at), if mine.is_some() { WENDIGO_LIGHTNING } else { "Play_Staff_Skill_Thunder_Strike" }, &[], false);
            }
            SimEvent::StaffRechargeStarted { id } => {
                out.play_from(from(id), "Play_Staff_ChargeEnerge_1st_Start", &[], false);
                if let Some(l) = out.play_from(from(id), "Play_Staff_ChargeEnerge_1st_Loop", &[], true) {
                    out.commands.entity(l).insert(StaffLoop(id, 1));
                }
            }
            SimEvent::StaffRechargeEnded { id } => {
                stop(&mut out, id, 1);
                out.play_from(from(id), "Play_Staff_ChargeEnerge_1st_End", &[], false);
            }
            SimEvent::StaffRejected { id } if Some(id) == player.0 => {
                out.play("Play_Staff_Skill_Energy_Error", &[], false);
            }
            SimEvent::StaffEnergy { id, current, max } if Some(id) == player.0 => {
                let segments = staff(id).and_then(|s| s.held.as_ref().map(|h| u32::from(h.1.segments.max(1)))).unwrap_or(1);
                let seg = |v: u32| v * segments / max.max(1);
                if let Some(&prev) = last.get(&id)
                    && current > prev
                {
                    if current >= max {
                        out.play("Play_Staff_Skill_Energy_Full", &[], false);
                    } else if seg(current) > seg(prev) {
                        out.play("Play_Staff_Skill_Energy_Ready", &[], false);
                    }
                }
                last.insert(id, current);
            }
            _ => {}
        }
    }
    for (h, mut hum) in &mut hums {
        hum.0 -= time.delta_secs();
        if hum.0 <= 0.0 {
            out.commands.entity(h).despawn();
        }
    }
}

/// Plays montage cues whose time has come.
fn play_cues(time: Res<Time>, mut cues: ResMut<Cues>, index: Res<SimIndex>, positions: Query<&Position>, mut out: Player) {
    let dt = time.delta_secs();
    let mut due = Vec::new();
    cues.0.retain_mut(|c| {
        c.0 -= dt;
        if c.0 <= 0.0 {
            due.push((c.1, c.2.clone()));
            false
        } else {
            true
        }
    });
    // From the monster that plays them.
    for (owner, ev) in due {
        let at = index.get(owner).and_then(|e| positions.get(e).ok()).map(|p| p.0);
        out.play_from(at, &ev, &[], false);
    }
}

/// Footsteps every [`STEP_CM`] on the ground, plus jump and land sounds, for the local player.
#[allow(clippy::too_many_arguments)]
fn footsteps(
    time: Res<Time>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    actors: Query<(&Position, &CharacterState)>,
    (spatial, pieces, amb, table): (db_sim::SpatialQuery, Query<&db_sim::map::MapPiece>, Res<MapAmbience>, Res<FootstepTable>),
    mut on: Local<Option<(String, u32)>>,
    mut steps: ResMut<Steps>,
    mut out: Player,
) {
    let Some((p, st)) = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()) else { return };
    // The surface under foot: the map piece straight below, by its mesh's physical material.
    let mut step = |out: &mut Player, event: &str| {
        let below = spatial.cast_ray_predicate(p.0, Dir3::NEG_Z, 300.0, true, &db_sim::layers::walk(), &|e| pieces.contains(e));
        let mesh = below.and_then(|h| pieces.get(h.entity).ok()).map(|m| m.0);
        let surface = mesh.map_or("", |m| amb.surface(p.0, m));
        if on.as_ref().is_none_or(|o| o.0 != surface) {
            info!("footsteps: on {} (mesh {mesh:?} at {})", if surface.is_empty() { "the default" } else { surface }, p.0.round());
            *on = Some((surface.to_string(), mesh.unwrap_or(u32::MAX)));
        }
        out.play_among(None, event, FLOOR, table.ids(event, surface), false);
    };
    let Some(last) = steps.last.replace(p.0) else {
        // First frame: take the current state without a landing sound.
        steps.on_ground = st.on_ground;
        return;
    };
    let moved = (p.0 - last).truncate().length();
    // A jump between places (a new run's start, a portal): not walked (it played a burst of a step a frame).
    if moved > TELEPORT_CM {
        steps.distance = 0.0;
        return;
    }
    if !st.on_ground {
        steps.airborne += time.delta_secs();
    }
    if st.on_ground != steps.on_ground {
        steps.on_ground = st.on_ground;
        if st.on_ground {
            if steps.airborne >= LAND_AFTER {
                step(&mut out, "Play_Footstep_Land_1P");
            }
            steps.airborne = 0.0;
        } else if st.velocity.z > 0.0 {
            step(&mut out, "Play_Footstep_Jump_Common_1P");
        }
        steps.distance = 0.0;
        return;
    }
    if !st.on_ground {
        return;
    }
    steps.distance += moved;
    if steps.distance >= STEP_CM {
        steps.distance -= STEP_CM;
        step(&mut out, if st.crouched { "Play_Footstep_Crouch_1P" } else { "Play_Footstep_Walk_1P" });
    }
}

// ---- where sounds come from, and the sounds of the run around the fights ----

/// Sounds of other actors farther than this from the player aren't played (cm; ours).
const HEARING: f32 = 3000.0;
/// Distance scale for spatial sounds: Bevy's spatial audio falls off with the inverse square of the
/// distance in world units (metres), far steeper than the game's attenuation curves (not decoded); at
/// this scale a sound is full volume within about 6.7 m and about a tenth at 20 m (ours).
const SPATIAL_SCALE: f32 = 0.15;

/// The local player's position (sim cm), where the listener is.
#[derive(Resource, Default)]
struct Ears(Option<Vec3>);

fn update_ears(player: Res<LocalPlayer>, index: Res<SimIndex>, positions: Query<&Position>, mut ears: ResMut<Ears>) {
    ears.0 = player.0.and_then(|id| index.get(id)).and_then(|e| positions.get(e).ok()).map(|p| p.0);
}

/// The camera hears (the listener's ears 20 cm apart).
fn add_listener(mut commands: Commands, cams: Query<Entity, (With<crate::PlayerCamera>, Without<SpatialListener>)>) {
    for c in &cams {
        commands.entity(c).insert(SpatialListener::new(0.2));
    }
}

/// The pickup sound's item word (`Play_Item_Pickup_<word>`) for an item, by its kind, its worn slots and,
/// for weapons, its mesh's weapon folder (HYPOTHESIS: the game picks by the item's own sound setting,
/// not decoded; armour material isn't known, so all armour is leather).
fn pickup_word(item_id: i32, info: Option<&db_sim::inventory::ItemInfo>) -> &'static str {
    use db_formats::config::generated::{EnItemUeType as T, EnItemWearingPart as W};
    let Some(info) = info else { return "Coin" };
    match info.kind {
        T::EquipWeapon => match crate::fp_body::weapon_type(item_id).map(|w| w.0) {
            Some("LongSword") => "LongSword",
            Some("Dagger") => "Dagger",
            Some("Mace") => "Hammer",
            Some("Staff") => "Staff",
            Some("HeavyCrossbow") => "HeavyCrossbow",
            Some("Shield") => "Woodshield",
            Some("Relic") => "Gem",
            _ => "Sword",
        },
        T::EquipArmor => "Leather",
        T::EquipAccessory if info.slots.contains(&W::Necklace) => "Necklace",
        T::EquipAccessory => "Ring",
        T::ItemConsumable | T::ItemThrowable => "Potion_2D",
        T::ItemAmmo => "Arrow",
        T::ItemTreasure => "Loot_Cup_Golden",
        T::ItemGem => "Gem",
        _ => "Coin",
    }
}

/// The sounds of the run itself: chests, pickups, potions, portals, the end of the run, the player's
/// own voice when hurt or killed (event names matched by meaning; HYPOTHESIS where the game's trigger
/// isn't decoded).
#[allow(clippy::too_many_arguments)]
fn run_sounds(events: Res<FrameEvents>, player: Res<LocalPlayer>, index: Res<SimIndex>, positions: Query<&Position>, inventories: Query<&db_sim::Inventory>, data: Option<Res<db_sim::GameData>>, mut out: Player) {
    let me = player.0;
    let pos = |id: db_sim::SimId| index.get(id).and_then(|e| positions.get(e).ok()).map(|p| p.0);
    for e in &events.0 {
        match e {
            SimEvent::ChestOpened { chest, .. } => {
                out.play_from(pos(*chest), "Play_Chest_Metal_Open", &[], false);
            }
            SimEvent::PickedUp { id, uid, .. } if Some(*id) == me => {
                let inv = index.get(*id).and_then(|e| inventories.get(e).ok());
                let item = inv.and_then(|i| i.bag.iter().map(|b| &b.item).chain(i.worn.values()).find(|x| x.uid == *uid)).map(|x| x.item.item_id);
                let word = item.map_or("Coin", |it| pickup_word(it, data.as_ref().and_then(|d| d.info(it))));
                out.play(&format!("Play_Item_Pickup_{word}"), &[], false);
            }
            SimEvent::ItemUsed { id, .. } if Some(*id) == me => {
                out.play("Play_Item_Drink_Potion", &[], false);
            }
            SimEvent::FloorChanged { id, .. } if Some(*id) == me => {
                out.play("Play_Portal_Enter", &[], false);
            }
            SimEvent::RunEnded { id, outcome, .. } if Some(*id) == me => {
                let (ui, vo) = match outcome {
                    db_sim::RunOutcome::Extracted => ("Play_UI_Escape_Success", "Play_UI_Evacuate_Success_Vo"),
                    _ => ("Play_UI_Escape_Fail", "Play_UI_Evacuate_Fail_Vo"),
                };
                out.play(ui, &[], false);
                out.play(vo, &[], false);
            }
            SimEvent::Died { id } if Some(*id) == me => {
                out.play("Play_Characters_Voice_Human_Male_Death", &[], false);
            }
            SimEvent::Damaged { target, blocked: false, damage, .. } if Some(*target) == me && *damage > 0.0 => {
                out.play("Play_Characters_Voice_Human_Male_Pain", &[], false);
            }
            SimEvent::ShieldAbsorbed { id, .. } => {
                out.play_from(pos(*id), "Play_Shield_DreamShield_Block", &[], false);
            }
            _ => {}
        }
    }
}

/// The guard sound when the local player raises the guard, by the worn weapons (the bank has one per
/// weapon set: `Play_Shield_Guard`, `Play_Sword_Guard`, ...).
fn guard_sound(player: Res<LocalPlayer>, index: Res<SimIndex>, actors: Query<(&db_sim::Melee, &db_sim::Inventory)>, mut was: Local<bool>, mut out: Player) {
    use db_formats::config::generated::EnItemWearingPart as W;
    let Some((melee, inv)) = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()) else { return };
    let now = melee.blocking();
    if now && !*was {
        let kind = |slot| inv.worn.get(&slot).and_then(|i| crate::fp_body::weapon_type(i.item.item_id)).map(|w| w.0);
        let event = match (kind(W::Pair1rightweapon), kind(W::Pair1leftweapon)) {
            (_, Some("Shield")) => "Play_Shield_Guard",
            (Some("Dagger"), Some("Dagger")) => "Play_Dagger_Dagger_Guard",
            (Some("Dagger"), _) => "Play_Dagger_Guard",
            (Some("LongSword"), _) => "Play_LongSword_Guard",
            (Some("Mace"), _) => "Play_Mace_Guard",
            _ => "Play_Sword_Guard",
        };
        out.play(event, &[], false);
    }
    *was = now;
}

/// The local player's crossbow, at its montages' sound notifies: the shot (`AM_HeavyCrossBow_HipFire_1st` /
/// `AimFire` at 0) and the reload's two (`HipReload` 0.592 and 2.877 s). The aim's (`Play_Crossbow_Aim_On` / `_Off`
/// in `AimStart` / `AimEnd`) have no media in the decoded bank.
fn crossbow_sounds(player: Res<LocalPlayer>, index: Res<SimIndex>, crossbows: Query<&db_sim::crossbow::CrossbowState>, mut was: Local<Option<db_sim::crossbow::CrossbowAction>>, mut out: Player) {
    use db_sim::crossbow::CrossbowAction as A;
    let Some(c) = player.0.and_then(|id| index.get(id)).and_then(|e| crossbows.get(e).ok()) else { return };
    let at = |t: u32| t as f32 * db_sim::DT;
    match (c.action, *was) {
        (Some(A::Fire { tick, .. }), prev) if !matches!(prev, Some(A::Fire { tick: p, .. }) if p <= tick) => {
            out.play("Play_Crossbow_Shot", &[], false);
        }
        (Some(A::Reload { tick }), prev) => {
            let before = match prev {
                Some(A::Reload { tick: p }) if p <= tick => at(p),
                _ => -1.0,
            };
            for (time, event) in [(0.592, "Play_Crossbow_Reload_1"), (2.877, "Play_Crossbow_Reload_2")] {
                if before < time && at(tick) >= time {
                    out.play(event, &[], false);
                }
            }
        }
        _ => {}
    }
    *was = c.action;
}

/// A weapon set switch: the new set's draw montage sounds at their times (`WeaponCombos::draw_sounds`, by the setup
/// the held items name: `Sword` + `Shield` -> `SwordShield`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_sounds(
    time: Res<Time>,
    events: Res<crate::combat_view::FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    inventories: Query<&db_sim::Inventory>,
    combos: Option<Res<db_sim::weapons::WeaponCombos>>,
    mut pending: Local<Option<(f32, Vec<(f32, String)>)>>,
    mut out: Player,
) {
    use db_formats::config::generated::EnItemWearingPart as W;
    if events.0.iter().any(|e| matches!(e, SimEvent::WeaponPairSwitched { id, .. } if Some(*id) == player.0))
        && let Some(inv) = player.0.and_then(|id| index.get(id)).and_then(|e| inventories.get(e).ok())
    {
        let kind = |slot| inv.worn.get(&slot).and_then(|i| crate::fp_body::weapon_type(i.item.item_id)).map(|w| w.0);
        let setup = format!("{}{}", kind(W::Pair1rightweapon).unwrap_or_default(), kind(W::Pair1leftweapon).unwrap_or_default());
        *pending = combos.as_ref().and_then(|c| c.draw_sounds.get(&setup)).map(|s| (0.0, s.clone()));
    }
    let Some((t, sounds)) = pending.as_mut() else { return };
    let before = *t;
    *t += time.delta_secs();
    for (at, event) in sounds.iter() {
        if before <= *at && *at < *t {
            out.play(event, &[], false);
        }
    }
    if sounds.iter().all(|(at, _)| *at < *t) {
        *pending = None;
    }
}

/// Button clicks and the inventory window opening and closing.
fn ui_sounds(buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>, inventory: Res<crate::InventoryOpen>, mut was_open: Local<Option<bool>>, mut out: Player) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        out.play("Play_UI_Click_Common_1", &[], false);
    }
    // By value: the resource is written every frame by some systems.
    if was_open.is_some_and(|w| w != inventory.0) {
        out.play(if inventory.0 { "Play_UI_Item_Open_Inventory" } else { "Play_UI_Item_Close_Inventory" }, &[], false);
    }
    *was_open = Some(inventory.0);
}

/// A map floor's placed ambience: its `BP_Tgf_AmbientSound` loop and entry sound, its placed `AkAmbientSound` emitters
/// (event, world point), and the audio volumes (world box, priority) that set the `Amb` state while the listener is
/// inside.
#[derive(Debug, Default, Clone)]
struct FloorAmbience {
    looped: Option<String>,
    enter: Option<String>,
    emitters: Vec<(String, Vec3)>,
    volumes: Vec<(Vec3, Vec3, String, f32)>,
}

/// The emitters being heard, by (floor, index): a loop's sound, and when a one-shot emitter plays next (s).
type Heard = BTreeMap<(usize, usize), (Option<Entity>, f32)>;

/// Placed emitters whose event isn't a `_Loop` play one of their sounds this often while heard (s; ours: Wwise's
/// random-container delays aren't decoded).
const EMITTER_GAP: (f32, f32) = (6.0, 14.0);

/// The current map's floors' ambience, and how a state picks a state-keyed ambience's sound
/// (`_data/recovered/ambience_states.json`: event -> (its `None` sound, state -> source file)).
#[derive(Resource, Debug, Default)]
struct MapAmbience {
    key: Option<&'static str>,
    floors: Vec<FloorAmbience>,
    /// Each floor's collision meshes' surfaces ("": none).
    surfaces: Vec<Vec<String>>,
    states: BTreeMap<String, (String, BTreeMap<String, String>)>,
    /// The emitters' events whose switch container's default sounds play (the map torches' `Totch` switch is left at
    /// `Default_01`, not the skins), media ids.
    sounds: BTreeMap<String, Vec<u64>>,
}

impl MapAmbience {
    fn load(maps: &db_sim::crypt::CryptMaps) -> Self {
        let read = |rel: String| std::fs::read(db_formats::data_path(rel)).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let v3 = |v: &serde_json::Value| Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
        let floors = maps
            .def
            .levels
            .iter()
            .enumerate()
            .map(|(i, level)| {
                let Some(j) = read(format!("recovered/maps/{level}-audio.json")) else { return FloorAmbience::default() };
                let events: Vec<&str> = j["ambient"].as_array().into_iter().flatten().filter_map(|a| a["event"].as_str()).collect();
                let off = maps.def.offset(i);
                FloorAmbience {
                    looped: events.iter().find(|e| !e.starts_with("Play_UI_")).map(|e| e.to_string()),
                    enter: events.iter().find(|e| e.starts_with("Play_UI_")).map(|e| e.to_string()),
                    emitters: j["emitters"].as_array().into_iter().flatten().filter_map(|e| Some((e["event"].as_str()?.to_string(), v3(&e["pos"]) + off))).collect(),
                    volumes: j["volumes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| Some((v3(&v["min"]) + off, v3(&v["max"]) + off, v["state"].as_str()?.to_string(), v["priority"].as_f64().unwrap_or(1.0) as f32)))
                        .collect(),
                }
            })
            .collect();
        let states = read("recovered/ambience_states.json".into())
            .and_then(|j| j.as_object().cloned())
            .into_iter()
            .flatten()
            .map(|(event, t)| {
                let map = t["states"].as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
                (event, (t["default"].as_str().unwrap_or("").to_string(), map))
            })
            .collect();
        let surfaces = maps.def.levels.iter().map(|level| read(format!("recovered/maps/{level}-surfaces.json")).and_then(|j| serde_json::from_value(j).ok()).unwrap_or_default()).collect();
        let sounds = read("recovered/emitter_sounds.json".into()).and_then(|j| serde_json::from_value(j).ok()).unwrap_or_default();
        Self { key: Some(maps.def.key), floors, states, surfaces, sounds }
    }

    /// The surface of collision mesh `mesh` on the floor at `at` ("" when unknown).
    fn surface(&self, at: Vec3, mesh: u32) -> &str {
        let floor = (db_sim::dungeon::floor_of(at.x) as usize).min(self.surfaces.len().saturating_sub(1));
        self.surfaces.get(floor).and_then(|s| s.get(mesh as usize)).map_or("", String::as_str)
    }

    /// The loop for a listener at `at` (sim cm) and the source file it plays (empty: any), and the floor's entry sound.
    fn pick(&self, at: Option<Vec3>) -> Option<(String, String, Option<String>)> {
        let floor = at.map_or(0, |p| (db_sim::dungeon::floor_of(p.x) as usize).min(self.floors.len().saturating_sub(1)));
        let f = self.floors.get(floor)?;
        let event = f.looped.clone()?;
        let file = self.states.get(&event).map_or(String::new(), |(none, by)| {
            // The volume the listener is in: the highest priority (the castle's rooms, 2-3, over its outdoor volume, 1),
            // then the smallest (ours).
            let area = |v: &(Vec3, Vec3, String, f32)| (v.1 - v.0).truncate().element_product();
            let state = at.and_then(|p| f.volumes.iter().filter(|(lo, hi, ..)| p.cmpge(*lo).all() && p.cmple(*hi).all()).min_by(|a, b| b.3.total_cmp(&a.3).then(area(a).total_cmp(&area(b))))).map(|v| v.2.as_str());
            state.and_then(|s| by.get(s)).unwrap_or(none).clone()
        });
        Some((event, file, f.enter.clone()))
    }
}

/// The background loop for the screen: in runs on a real map, its floor's placed ambience (the castle's
/// `Play_Amb_2D`, its sound picked by the `Amb` state of the audio volume the listener is in, and
/// `Play_UI_Enter_Castle` when the run starts; the crypt's `Play_Amb_Dungeon_Loop` and `Play_UI_Enter_Dungeon`);
/// elsewhere in runs and the test room `Play_Amb_Dungeon_Loop`, the hall music on the title, stash and result screens
/// (HYPOTHESIS: the menus' music isn't decoded; `Play_Music_Hall` is the bank's match).
fn background(
    screen: Res<State<crate::Screen>>,
    crypt: Res<crate::CurrentCrypt>,
    mut amb: ResMut<MapAmbience>,
    mut playing: Local<Option<(crate::Screen, String, Option<Entity>)>>,
    mut out: Player,
) {
    use crate::Screen as S;
    let now = *screen.get();
    match &crypt.0 {
        Some(maps) if amb.key != Some(maps.def.key) => *amb = MapAmbience::load(maps),
        None if amb.key.is_some() => *amb = MapAmbience::default(),
        _ => {}
    }
    // (A run on a map waits for the player's place: its floor and volume pick the sound.)
    if now == S::Run && !amb.floors.is_empty() && out.ears.0.is_none() {
        return;
    }
    let placed = (now == S::Run).then(|| amb.pick(out.ears.0)).flatten();
    let (event, file) = match (&placed, now) {
        (Some((e, f, _)), _) => (e.clone(), f.clone()),
        (None, S::Run | S::TestRoom) => ("Play_Amb_Dungeon_Loop".to_string(), String::new()),
        (None, S::Title | S::Stash | S::Result) => ("Play_Music_Hall".to_string(), String::new()),
        (None, S::Boot | S::MapView) => (String::new(), String::new()),
    };
    let key = format!("{event}|{file}");
    if playing.as_ref().is_some_and(|p| p.0 == now && p.1 == key) {
        return;
    }
    let entered = playing.as_ref().is_none_or(|p| p.0 != now);
    if let Some((_, _, Some(e))) = playing.take() {
        out.commands.entity(e).despawn();
    }
    if now == S::Run && entered {
        let enter = placed.as_ref().and_then(|p| p.2.clone()).unwrap_or_else(|| "Play_UI_Enter_Dungeon".into());
        out.play(&enter, &[], false);
    }
    let e = if file.is_empty() { out.play(&event, &[], true) } else { out.play(&event, &[&[file.as_str()]], true) };
    *playing = Some((now, key, e));
}

/// The placed ambient emitters of the listener's floor within [`HEARING`]: a `_Loop` event loops at its point; the
/// others play one sound at a time with [`EMITTER_GAP`] between. Gone out of hearing or off the run.
fn emitters(time: Res<Time>, screen: Res<State<crate::Screen>>, amb: Res<MapAmbience>, mut active: Local<Heard>, mut last: Local<(usize, usize)>, mut out: Player) {
    let me = out.ears.0.filter(|_| *screen.get() == crate::Screen::Run);
    let floor = me.map(|p| (db_sim::dungeon::floor_of(p.x) as usize).min(amb.floors.len().saturating_sub(1)));
    // The heard ones: within hearing on the listener's floor; of a globally limited sound (the torches, 6) only the
    // nearest that fit beside the group's other sounds (the farther go virtual, as with Wwise's virtual voices).
    let mut heard = std::collections::BTreeSet::new();
    // (A run with no map ambience, the generated dungeon, has no floors:.)
    if let (Some(p), Some((f, fl))) = (me, floor.and_then(|f| amb.floors.get(f).map(|fl| (f, fl)))) {
        let mine: Vec<Entity> = active.values().filter_map(|v| v.0).collect();
        let mut near: Vec<_> = fl.emitters.iter().enumerate().filter(|(_, e)| e.1.distance(p) <= HEARING).map(|(i, e)| (e.1.distance(p), i, out.global_limit(&e.0, amb.sounds.get(&e.0).map_or(&[], Vec::as_slice)))).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut room: BTreeMap<u32, usize> = BTreeMap::new();
        let (within, mut held) = (near.len(), 0);
        for (_, i, lim) in near {
            let fits = lim.is_none_or(|l| {
                let left = room.entry(l.group).or_insert_with(|| (l.max as usize).saturating_sub(out.limits.playing.get(&l.group).map_or(0, |v| v.iter().filter(|(e, _)| !mine.contains(e)).count())));
                let fits = *left > 0;
                *left = left.saturating_sub(1);
                fits
            });
            if fits {
                heard.insert((f, i));
            } else {
                held += 1;
            }
        }
        if *last != (heard.len(), held) {
            *last = (heard.len(), held);
            info!("emitters: {} heard of {within} within hearing ({held} held back by a playback limit)", heard.len());
        }
    }
    let gone: Vec<_> = active.keys().filter(|k| !heard.contains(*k)).copied().collect();
    for k in gone {
        if let Some((Some(e), _)) = active.remove(&k) {
            out.stop(e);
        }
    }
    let Some(floor) = floor.filter(|_| me.is_some()) else { return };
    let now = time.elapsed_secs();
    for &(_, i) in &heard {
        let (event, at) = &amb.floors[floor].emitters[i];
        let looped = event.ends_with("_Loop");
        match active.get_mut(&(floor, i)) {
            Some(_) if looped => {}
            Some((_, next)) if now < *next => {}
            _ => {
                let e = out.play_among(Some(*at), event, &[], amb.sounds.get(event).map_or(&[], Vec::as_slice), looped);
                let gap = EMITTER_GAP.0 + (EMITTER_GAP.1 - EMITTER_GAP.0) * out.rng.0.unit_f32();
                active.insert((floor, i), (e.filter(|_| looped), now + gap));
            }
        }
    }
}

/// The countdown voice over the run's last five seconds.
fn countdown(clock: Option<Res<db_sim::RunClock>>, mut last: Local<Option<u32>>, mut out: Player) {
    let secs = clock.and_then(|c| c.ticks_left).map(|t| (t as f32 * db_sim::DT).ceil() as u32);
    if let Some(s) = secs.filter(|s| *s <= 5)
        && *last != Some(s)
    {
        out.play(&format!("Play_UI_CountDown_{s}"), &[], false);
    }
    *last = secs;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every event the sounds name is in the bank (skipped without `_data/audio/index.json`).
    #[test]
    fn the_run_sounds_exist_in_the_bank() {
        let bank = SoundBank::load();
        if bank.0.is_empty() {
            return;
        }
        let words = ["Sword", "LongSword", "Dagger", "Hammer", "Staff", "HeavyCrossbow", "Woodshield", "Gem", "Leather", "Necklace", "Ring", "Potion_2D", "Arrow", "Loot_Cup_Golden", "Coin"];
        let mut names: Vec<String> = words.iter().map(|w| format!("Play_Item_Pickup_{w}")).collect();
        names.extend((0..=5).map(|s| format!("Play_UI_CountDown_{s}")));
        names.extend(
            [
                "Play_Chest_Metal_Open", "Play_Item_Drink_Potion", "Play_Portal_Enter", "Play_UI_Escape_Success", "Play_UI_Escape_Fail", "Play_UI_Evacuate_Success_Vo", "Play_UI_Evacuate_Fail_Vo", "Play_Characters_Voice_Human_Male_Death",
                "Play_Characters_Voice_Human_Male_Pain", "Play_Shield_DreamShield_Block", "Play_Shield_Guard", "Play_Dagger_Dagger_Guard", "Play_Dagger_Guard", "Play_LongSword_Guard", "Play_Mace_Guard", "Play_Sword_Guard", "Play_UI_Click_Common_1",
                "Play_UI_Item_Open_Inventory", "Play_UI_Item_Close_Inventory", "Play_Amb_Dungeon_Loop", "Play_Music_Hall", "Play_UI_Enter_Dungeon",
                // the body loops.
                BURN_LOOP, "Play_M_Insect_Fly_Loop", "Play_Torch_Burning_Crackling_Loop_No_Cone",
                // the staff sounds.
                "Play_Staff_Attack_1", "Play_Staff_Attack_2", "Play_Staff_Skill_Fire_Start", "Play_Staff_Skill_Fire_Loop", "Play_Staff_Skill_Fire_End", "Play_Staff_Skill_Fire_Release", "Play_Impact_Staff_Skill_Fire",
                "Play_Staff_Skill_Thunder_Ready", "Play_Staff_Skill_Thunder_Release", "Play_Staff_Skill_Thunder_Cancel", "Play_Staff_Skill_Thunder_Loop", "Play_Staff_Skill_Thunder_Strike", "Play_Staff_ChargeEnerge_1st_Start",
                "Play_Staff_ChargeEnerge_1st_Loop", "Play_Staff_ChargeEnerge_1st_End", "Play_Staff_Skill_Energy_Ready", "Play_Staff_Skill_Energy_Full", "Play_Staff_Skill_Energy_Error",
                // the crossbow.
                "Play_Crossbow_Shot", "Play_Crossbow_Reload_1", "Play_Crossbow_Reload_2", "Play_Headshot_Common",
                // the draw sounds (the crossbow's `Play_Weapon_Select_HeavyCrossbow` has no media).
                "Play_Weapon_Select_Dagger", "Play_Weapon_Select_Dagger_Dagger", "Play_Weapon_Select_Mace_SingleHand", "Play_Weapon_Select_Mace_Shield", "Play_Select_Sword_TwoHands", "Play_Select_Sword_Shield",
            ]
            .map(str::to_string),
        );
        for n in names {
            assert!(!bank.variants(&n, &[]).is_empty(), "{n} has no media");
        }
    }

    /// the castle's placed ambience, and the `Amb` state that picks its sound, from the recon files (skipped without
    /// them): its rooms' volumes (priority 2-3) over its outdoor one (1).
    #[test]
    fn the_castle_plays_its_indoor_or_night_ambience_by_its_audio_volumes() {
        let bank = SoundBank::load();
        let Some(maps) = db_sim::crypt::CryptMaps::load(db_sim::crypt::CASTLE).ok() else { return };
        if bank.0.is_empty() {
            return;
        }
        let amb = MapAmbience::load(&maps);
        // Map-local points on floor 1 (the floor sits at its offset).
        let at = |x: f32, y: f32, z: f32| Some(Vec3::new(x, y, z) + maps.offset(0));
        // The first start's carpeted hall, then a start out in the courtyard.
        let (event, file, enter) = amb.pick(at(-5000.0, 60400.0, -150.0)).expect("the castle's ambience");
        assert_eq!((event.as_str(), file.as_str(), enter.as_deref()), ("Play_Amb_2D", "Amb_Castle_Indoor_Loop.wav", Some("Play_UI_Enter_Castle")));
        assert_eq!(amb.pick(at(-4500.0, 51800.0, 600.0)).map(|p| p.1).as_deref(), Some("Amb_Castle_Night.wav"));
        // Outside every volume: the state's `None` sound.
        assert_eq!(amb.pick(at(40000.0, 0.0, 0.0)).map(|p| p.1).as_deref(), Some("Amb_Castle_Indoor_Loop.wav"));
        for f in ["Amb_Castle_Night.wav", "Amb_Castle_Indoor_Loop.wav"] {
            assert!(bank.variants("Play_Amb_2D", &[]).iter().any(|(_, s)| s.ends_with(f)), "{f}");
        }
        // its torches burn at their places with the plain torch loop (not a skin's).
        let torches = amb.floors[0].emitters.iter().filter(|(e, _)| e == "Play_Torch_Burning_Crackling_Loop").count();
        assert!(torches >= 100, "{torches} torches");
        let ids = &amb.sounds["Play_Torch_Burning_Crackling_Loop"];
        let plain: Vec<_> = bank.variants("Play_Torch_Burning_Crackling_Loop", &[]).into_iter().filter(|(id, _)| ids.contains(id)).map(|(_, s)| s).collect();
        assert!(!plain.is_empty() && plain.iter().all(|s| s.ends_with("Torch_Burning_Loop.wav")), "{plain:?}");
    }

    /// a full group drops a new sound when it kills the newest, else stops its oldest.
    #[test]
    fn a_full_playback_limit_drops_the_newest_or_stops_the_oldest() {
        let (a, b) = (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap());
        let lim = |kill_newest| Limit { group: 7, max: 2, kill_newest, global: true };
        assert_eq!(admit(&[(a, 0.0)], lim(true)), Admit::Play);
        assert_eq!(admit(&[(a, 0.0), (b, 1.0)], lim(true)), Admit::Drop);
        assert_eq!(admit(&[(a, 0.0), (b, 1.0)], lim(false)), Admit::StopOldest(a));
    }

    /// the plain torch loop plays at most 6 at once, anywhere (skipped without the recon file).
    #[test]
    fn the_torch_loop_is_limited_to_six_at_once() {
        let limits = SoundLimits::load();
        if limits.by_media.is_empty() {
            return;
        }
        let torch = limits.by_media[&694698547];
        assert_eq!((torch.max, torch.global, torch.kill_newest), (6, true, false));
    }

    /// the castle's floor tiles are stone and its carpets carpet, and each surface's footsteps are its own
    /// (skipped without the recon files).
    #[test]
    fn footsteps_sound_by_the_surface_under_foot() {
        let read = |rel: &str| std::fs::read(db_formats::data_path(rel)).ok().and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok());
        let (Some(names), Some(surfaces)) = (read("collision/L_GothicCastle_WP_Main-meshnames.json"), read("recovered/maps/L_GothicCastle_WP_Main-surfaces.json")) else { return };
        assert_eq!(names.len(), surfaces.len());
        let of = |n: &str| names.iter().position(|m| m == n).map(|i| surfaces[i].as_str());
        assert_eq!((of("SM_FloorTiles01"), of("SM_carpet")), (Some("Stone"), Some("Carpet")));
        let table = FootstepTable::load();
        let bank = SoundBank::load();
        if table.0.is_empty() || bank.0.is_empty() {
            return;
        }
        let walk = |s: &str| table.ids("Play_Footstep_Walk_1P", s).to_vec();
        assert!(!walk("Stone").is_empty() && !walk("Carpet").is_empty() && walk("Stone") != walk("Carpet"));
        // No switch value of its own: the default's (`Boots_Stone`).
        assert_eq!(walk("Concrete"), walk("Stone"));
        assert!(walk("Carpet").iter().all(|id| bank.variants("Play_Footstep_Walk_1P", &[]).iter().any(|(m, _)| m == id)));
    }
}

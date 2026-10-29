//! Skill VFX triggers: which sim moment plays which effect, and where. The table comes from
//! `_data/recovered/effects/skill_vfx.json` (each skill's Niagara systems found through the ability
//! packages' imports and cue tags, CONFIRMED links; the phase, socket and view of each are the port's
//! reading, HYPOTHESIS). The effects themselves are `sys:<System>` keys of `effects.json`. A missing
//! file, skill or effect plays nothing.
//!
//! Phases: `charge0/1/2` loop at a charge level (the next level replaces it), `hold1/2` loop from a charge
//! stage to the release, `reach1/2` one-shot at a charge stage (sim `SkillCharged`), `cast`/`cast1`/`cast2`
//! one-shot at `SkillStarted` (uncharged) or `SkillCast` stage N, `projectile[N]` on the skill's shots,
//! `impact[N]` where they end, `burst` on `Burst`, `hit` on `Damaged`, `buff`/`buff_self` loop while a
//! monster / a player holds a sim buff of the skill, `buff_start`/`end` one-shot when it appears / goes,
//! `area` loop at the skill's area while it lasts (Blizzard storm, Oracle point), else once at the feet.
//!
//! Sockets (HYPOTHESIS, the body skeleton's socket transforms aren't decoded): on the local player's
//! first-person body the left hand is `Bip001-L-Hand`, the right hand and staff `Point_Weapon_R01` (the staff
//! head 51.5 cm up its bone: the staff skeletons' `FireStart`, CONFIRMED), without the body fixed offsets
//! from the camera; on other actors a hand is the chest, `Root` the feet, `EP_Head` 60 cm above the capsule
//! centre.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use db_sim::{ActorKind, CommandQueue, Position, SimCommand, SimEvent, SimId, SimIndex, SkillKind};

use crate::effects::{EffectLibrary, spawn_effect, stop};
use crate::to_bevy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Phase {
    Charge(u8),
    Hold(u8),
    Reach(u8),
    Cast(Option<u8>),
    Projectile(Option<u8>),
    Impact(Option<u8>),
    Burst,
    Hit,
    Buff,
    BuffSelf,
    BuffStart,
    End,
    Area,
}

impl Phase {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let stage = |p: &str| -> Option<Option<u8>> {
            let rest = s.strip_prefix(p)?;
            if rest.is_empty() { Some(None) } else { rest.parse().ok().map(Some) }
        };
        let need = |p: &str| stage(p).flatten();
        Some(match s {
            "burst" => Phase::Burst,
            "hit" => Phase::Hit,
            "buff" => Phase::Buff,
            "buff_self" => Phase::BuffSelf,
            "buff_start" => Phase::BuffStart,
            "end" => Phase::End,
            "area" => Phase::Area,
            _ if s.starts_with("charge") => Phase::Charge(need("charge")?),
            _ if s.starts_with("hold") => Phase::Hold(need("hold")?),
            _ if s.starts_with("reach") => Phase::Reach(need("reach")?),
            _ if s.starts_with("cast") => Phase::Cast(stage("cast")?),
            _ if s.starts_with("projectile") => Phase::Projectile(stage("projectile")?),
            _ if s.starts_with("impact") => Phase::Impact(stage("impact")?),
            _ => return None,
        })
    }
}

/// Where on an actor an effect goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Socket {
    LeftHand,
    RightHand,
    StaffTip,
    Root,
    Chest,
    Head,
}

impl Socket {
    /// The original socket name's place (HYPOTHESIS by name: `VFX-HandLeft*`, `Weapon_L*`, `EP_HandLeft`
    /// are the left hand; `Hand_R_VFX`, `Weapon_R`, `EP_HandRight` the right; the priest/druid/swordmaster
    /// VFX sockets and the staff mesh's `FireStart` (the laser's start, CONFIRMED by `GA_TGF_Staff_Laser`) the
    /// weapon's head; `Root` the feet).
    pub(crate) fn parse(s: &str) -> Self {
        let l = s.to_ascii_lowercase();
        if l.contains("handleft") || l.starts_with("weapon_l") {
            Socket::LeftHand
        } else if l.starts_with("vfx-priest") || l.starts_with("vfx-druid") || l.starts_with("vfx_swordmaster") || l.contains("help05") || l == "firestart" || l == "thundercast" {
            Socket::StaffTip
        } else if l.contains("handright") || l.starts_with("hand_r") || l.starts_with("weapon_r") {
            Socket::RightHand
        } else if l == "root" {
            Socket::Root
        } else if l.contains("head") {
            Socket::Head
        } else {
            Socket::Chest
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    First,
    Third,
    Both,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Trigger {
    pub phase: Phase,
    pub effect: String,
    pub socket: Socket,
    /// Cm up from the socket (the cue's `LocationOffset` Z; Flame Shock's ring plays 85 cm above the feet).
    pub up: f32,
    pub view: View,
}

/// A staff trigger: `phase` is one of `staff_view.rs`' staff phases (`beam`, `beam_end`, `beam_tip`,
/// `recharge`, `aim`, `release`, `cloud`, `strike`, `hit`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StaffTrigger {
    pub phase: String,
    pub effect: String,
    pub view: View,
}

/// The trigger table: skill -> its triggers; staff element -> its staff triggers.
#[derive(Resource, Debug, Default)]
pub struct SkillVfx {
    table: HashMap<SkillKind, Vec<Trigger>>,
    staffs: HashMap<db_sim::staffs::StaffElement, Vec<StaffTrigger>>,
}

fn view_of(v: &serde_json::Value) -> View {
    match v.as_str() {
        Some("1st") => View::First,
        Some("3rd") => View::Third,
        _ => View::Both,
    }
}

impl SkillVfx {
    pub(crate) fn from_json(v: &serde_json::Value) -> Self {
        let mut table: HashMap<SkillKind, Vec<Trigger>> = HashMap::new();
        for (name, s) in v["skills"].as_object().into_iter().flatten() {
            let Some(skill) = SkillKind::ALL.into_iter().find(|k| format!("{k:?}") == *name) else { continue };
            for t in s["triggers"].as_array().into_iter().flatten() {
                let (Some(phase), Some(effect)) = (t["phase"].as_str().and_then(Phase::parse), t["effect"].as_str()) else { continue };
                table.entry(skill).or_default().push(Trigger { phase, effect: effect.to_string(), socket: Socket::parse(t["socket"].as_str().unwrap_or("")), up: t["up_cm"].as_f64().unwrap_or(0.0) as f32, view: view_of(&t["view"]) });
            }
        }
        use db_sim::staffs::StaffElement;
        let mut staffs: HashMap<StaffElement, Vec<StaffTrigger>> = HashMap::new();
        for (name, s) in v["staffs"].as_object().into_iter().flatten() {
            let Some(el) = [StaffElement::Fire, StaffElement::Ice, StaffElement::Thunder].into_iter().find(|k| format!("{k:?}") == *name) else { continue };
            for t in s["triggers"].as_array().into_iter().flatten() {
                let (Some(phase), Some(effect)) = (t["phase"].as_str().filter(|p| crate::staff_view::PHASES.contains(p)), t["effect"].as_str()) else { continue };
                staffs.entry(el).or_default().push(StaffTrigger { phase: phase.to_string(), effect: effect.to_string(), view: view_of(&t["view"]) });
            }
        }
        Self { table, staffs }
    }

    /// The staff element's effects for `phase` that the local player sees (`me`) or others see.
    pub(crate) fn staff(&self, el: db_sim::staffs::StaffElement, phase: &str, me: bool) -> impl Iterator<Item = &str> {
        self.staffs.get(&el).into_iter().flatten().filter(move |t| t.phase == phase && !((t.view == View::First && !me) || (t.view == View::Third && me))).map(|t| t.effect.as_str())
    }

    pub(crate) fn triggers(&self, skill: SkillKind, phase: impl Fn(Phase) -> bool) -> impl Iterator<Item = &Trigger> {
        self.table.get(&skill).into_iter().flatten().filter(move |t| phase(t.phase))
    }

    /// The skill's shots carry a table effect (their trail).
    pub(crate) fn has_projectile(&self, skill: SkillKind) -> bool {
        self.triggers(skill, |p| matches!(p, Phase::Projectile(_))).next().is_some()
    }

    /// The skill's hits play a table effect (instead of the generic flesh hit).
    pub(crate) fn has_hit(&self, skill: SkillKind) -> bool {
        self.triggers(skill, |p| p == Phase::Hit).next().is_some()
    }

    /// The skill's shots end in a table effect (its impact or burst), not the generic element burst.
    pub(crate) fn has_impact(&self, skill: SkillKind) -> bool {
        self.triggers(skill, |p| matches!(p, Phase::Impact(_) | Phase::Burst)).next().is_some()
    }
}

/// Startup: the trigger table.
pub(crate) fn load_skill_vfx(mut commands: Commands) {
    let rel = "recovered/effects/skill_vfx.json";
    let table = match crate::map_view::data_file(rel).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        Some(v) => SkillVfx::from_json(&v),
        None => {
            warn!("no _data/{rel} (tools/recon/zen_imports.py); no skill effects");
            SkillVfx::default()
        }
    };
    info!("skill effects for {} skills", table.table.len());
    commands.insert_resource(table);
}

/// Bursts up to this radius (cm) are a shot's own small explosion, drawn by its impact (Pyroblast's small fireballs).
pub(crate) const SMALL_BURST: f32 = 100.0;

/// Dev: release a held charged skill after this many seconds (`--auto-release`, shots of stage 1).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoRelease(pub Option<f32>);

/// `--auto-heavy` (dev): the bot starts a heavy (charge) attack whenever it is free and never lets go.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoHeavy(pub bool);

/// Dev: `--vfx-shots <name>:<skip>:<s1,s2...>` (checks): screenshots `vfx_<name>_<ms>` this many real seconds
/// after the local player's skill start number `skip + 1`, then exit. Effects last a fraction of a second and
/// the frame a skill fires on varies run to run, so the shots follow the skill instead.
/// `<name>:<skip>:<times>:<attack>` follows a monster's attacks whose montage name contains `<attack>`.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct VfxShots {
    pub name: String,
    pub skip: u32,
    pub at: Vec<f32>,
    pub monster_attack: Option<String>,
}

impl VfxShots {
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.splitn(4, ':');
        let name = it.next()?.to_string();
        let skip = it.next()?.parse().ok()?;
        let at = it.next()?.split(',').map(|x| x.trim().parse().ok()).collect::<Option<Vec<f32>>>()?;
        let monster_attack = it.next().filter(|a| !a.is_empty()).map(str::to_string);
        (!name.is_empty() && !at.is_empty()).then_some(Self { name, skip, at, monster_attack })
    }
}

#[allow(clippy::too_many_arguments)]
fn vfx_shots(time: Res<Time<Real>>, plan: Res<VfxShots>, events: Res<crate::combat_view::FrameEvents>, player: Res<crate::LocalPlayer>, (index, kits): (Res<SimIndex>, Query<&db_sim::monsters::MonsterKit>), mut shots: ResMut<crate::ShotRequests>, mut seen: Local<u32>, mut from: Local<Option<f32>>, mut next: Local<usize>) {
    let now = time.elapsed_secs();
    if from.is_none() {
        for ev in &events.0 {
            let starts = match (plan.monster_attack.as_deref(), ev) {
                (None, SimEvent::SkillStarted { id, .. } | SimEvent::StaffCastStarted { id, .. } | SimEvent::StaffRejected { id } | SimEvent::GroundHit { id, .. } | SimEvent::FlaskLanded { owner: id, .. } | SimEvent::SwingStarted { id, .. } | SimEvent::TorchHeld { id, on: true } | SimEvent::DrinkStarted { id, .. }) => Some(*id) == player.0,
                (Some(want), SimEvent::MonsterAttackStarted { id, index: i }) => index.get(*id).and_then(|e| kits.get(e).ok()).and_then(|k| k.attacks.get(usize::from(*i))).is_some_and(|a| a.name.contains(want)),
                _ => false,
            };
            if starts {
                if *seen == plan.skip {
                    *from = Some(now);
                }
                *seen += 1;
            }
        }
    }
    let Some(t0) = *from else { return };
    while let Some(&t) = plan.at.get(*next) {
        if now - t0 < t {
            return;
        }
        shots.queue.push_back(format!("{}_{:04}", plan.name, (t * 1000.0).round() as u32));
        *next += 1;
    }
    shots.exit_after = true;
}

pub(crate) fn auto_release(release: Res<AutoRelease>, player: Res<crate::LocalPlayer>, index: Res<SimIndex>, kits: Query<&db_sim::ClassKit>, mut queue: ResMut<CommandQueue>) {
    let Some(secs) = release.0 else { return };
    let Some(id) = player.0 else { return };
    let Some((skill, held)) = index.get(id).and_then(|e| kits.get(e).ok()).and_then(|k| k.charge) else { return };
    if held as f32 * db_sim::DT >= secs {
        queue.push(SimCommand::ReleaseSkill { id, skill });
    }
}

/// An effect that follows a sim actor (`up` cm above its capsule centre); goes with it.
#[derive(Component, Debug)]
pub(crate) struct Follow {
    id: SimId,
    up: f32,
}

impl Follow {
    pub(crate) fn new(id: SimId, up: f32) -> Self {
        Follow { id, up }
    }
}

/// The first-person body's attach bones, once its scene is in.
#[derive(Resource, Debug, Default)]
pub(crate) struct FpSockets {
    left: Option<Entity>,
    right: Option<Entity>,
    /// The right hand bone (`Bip001-R-Hand`): hand effects; `right` (the weapon point) is the staff's.
    right_hand: Option<Entity>,
}

/// Feet below the capsule centre, cm (as 's floor effects).
const FEET: f32 = -85.0;
/// Head above the capsule centre, cm (HYPOTHESIS).
const HEAD: f32 = 60.0;
/// First-person sockets when the body isn't there: offsets in the camera's frame, metres (HYPOTHESIS).
const CAM_LEFT: Vec3 = Vec3::new(-0.24, -0.22, -0.5);
const CAM_RIGHT: Vec3 = Vec3::new(0.24, -0.24, -0.5);
const CAM_STAFF: Vec3 = Vec3::new(0.28, 0.08, -0.65);
/// On the hand bones: the open left palm 15 cm along the hand's -Z from the wrist joint (measured where the palm shows on screen while charging; the old 9 cm along +Y put Pyroblast's hand fire
/// below the view); the staff head 51.5 cm up the right weapon bone: every staff skeleton's `FireStart` socket (Z 51.49 cm from the root, 15 of 15 staff skeletons with sockets), with the staff
/// held at its root. Metres.
const BONE_PALM: Vec3 = Vec3::new(0.0, -0.03, -0.15);
const BONE_STAFF_TIP: Vec3 = Vec3::new(0.0, 0.515, 0.0);

/// Finds the first-person body's hand bones as they appear.
pub(crate) fn find_fp_sockets(mut sockets: ResMut<FpSockets>, names: Query<(Entity, &Name), Added<Name>>, parents: Query<&ChildOf>, bodies: Query<(), With<crate::fp_body::FpBody>>) {
    for (e, n) in &names {
        let slot = match n.as_str() {
            "Bip001-L-Hand" => &mut sockets.left,
            "Point_Weapon_R01" => &mut sockets.right,
            "Bip001-R-Hand" => &mut sockets.right_hand,
            _ => continue,
        };
        if parents.iter_ancestors(e).any(|a| bodies.contains(a)) {
            *slot = Some(e);
        }
    }
}

/// Per-caster and per-actor loops the triggers keep running.
#[derive(Resource, Debug, Default)]
pub(crate) struct SkillVfxState {
    /// Charge loops: (caster, phase) -> effects.
    charging: HashMap<SimId, Vec<(Phase, Entity)>>,
    /// The stage a caster last cast (its projectiles' stage).
    stage: HashMap<SimId, u8>,
    /// Projectiles given their trail; their last place, skill and stage.
    flying: HashMap<SimId, (Vec3, Vec3, SkillKind, Option<u8>)>,
    /// Buff loops: (actor, skill) -> effects.
    buffs: HashMap<(SimId, SkillKind), Vec<Entity>>,
    /// Area loops: (caster, skill) -> effects.
    areas: HashMap<(SimId, SkillKind), Vec<Entity>>,
}

/// Where to play a socket's effect.
enum Place {
    /// A child of this entity at this local offset (metres), turned back to world scale.
    Child(Entity, Vec3),
    /// Following a sim actor.
    Actor(SimId, f32),
}

struct Ctx<'a> {
    lib: &'a EffectLibrary,
    local: Option<SimId>,
    camera: Option<Entity>,
    fp: &'a FpSockets,
    globals: &'a Query<'a, 'a, &'static GlobalTransform>,
}

impl Ctx<'_> {
    fn place(&self, id: SimId, socket: Socket) -> Place {
        let me = self.local == Some(id);
        let bone = |b: Option<Entity>, off: Vec3, cam: Vec3| match b.filter(|b| self.globals.contains(*b)) {
            Some(b) => Place::Child(b, off),
            None => self.camera.map_or(Place::Actor(id, 0.0), |c| Place::Child(c, cam)),
        };
        match socket {
            Socket::LeftHand if me => bone(self.fp.left, BONE_PALM, CAM_LEFT),
            Socket::RightHand if me => bone(self.fp.right.or(self.fp.right_hand), Vec3::ZERO, CAM_RIGHT),
            Socket::StaffTip if me => bone(self.fp.right, BONE_STAFF_TIP, CAM_STAFF),
            Socket::Root => Place::Actor(id, FEET),
            Socket::Head => Place::Actor(id, HEAD),
            _ => Place::Actor(id, 0.0),
        }
    }

    /// Plays `t` for actor `id`; None when the view rule leaves it out or the effect isn't converted.
    fn play(&self, commands: &mut Commands, t: &Trigger, id: SimId, pos: Option<Vec3>) -> Option<Entity> {
        let me = self.local == Some(id);
        if (t.view == View::First && !me) || (t.view == View::Third && me) {
            return None;
        }
        // `RUST_LOG=db_client::skill_vfx=debug` lists what plays (screenshot timing).
        debug!("skill vfx {:?} {} on {:?}", t.phase, t.effect, id);
        match self.place(id, t.socket) {
            Place::Child(parent, off) => {
                let off = off + Vec3::Y * t.up / 100.0;
                let s = self.globals.get(parent).map(|g| g.scale()).unwrap_or(Vec3::ONE).max(Vec3::splat(1e-4));
                let e = spawn_effect(commands, self.lib, &t.effect, off / s, Quat::IDENTITY, None)?;
                commands.entity(e).insert(Transform::from_translation(off / s).with_scale(Vec3::ONE / s));
                crate::effects::dim_lights(commands, e, FP_LIGHT);
                commands.entity(parent).add_child(e);
                Some(e)
            }
            Place::Actor(a, up) => {
                let up = up + t.up;
                let at = pos?;
                let e = spawn_effect(commands, self.lib, &t.effect, to_bevy(at + Vec3::Z * up), Quat::IDENTITY, None)?;
                commands.entity(e).insert(Follow { id: a, up });
                Some(e)
            }
        }
    }

    fn at(&self, commands: &mut Commands, t: &Trigger, at: Vec3, rot: Quat) -> Option<Entity> {
        debug!("skill vfx {:?} {} at {at:?}", t.phase, t.effect);
        spawn_effect(commands, self.lib, &t.effect, to_bevy(at), rot, None)
    }
}

/// Light strength of effects in the first-person hands (HYPOTHESIS, by eye).
const FP_LIGHT: f32 = 0.08;

/// The local player's first-person left palm in the world (the hand bone, or the camera offset without the body:
/// the end of the recharge's `FireLine`, the montages' `Weapon_L_Common_Cast_1st` socket).
pub(crate) fn fp_left_palm(fp: &FpSockets, camera: Option<Entity>, globals: &Query<&GlobalTransform>) -> Option<Vec3> {
    match fp.left.and_then(|b| globals.get(b).ok()) {
        Some(g) => Some(g.transform_point(BONE_PALM / g.scale().max(Vec3::splat(1e-4)))),
        None => camera.and_then(|c| globals.get(c).ok()).map(|g| g.transform_point(CAM_LEFT)),
    }
}

/// The first-person body's weapon bone is in (else staff effects sit at a camera offset).
pub(crate) fn fp_staff_ready(fp: &FpSockets, globals: &Query<&GlobalTransform>) -> bool {
    fp.right.is_some_and(|b| globals.contains(b))
}

/// Plays effect `key` on the local player's first-person staff head (the weapon bone 75 cm up, or the camera
/// offset without the body), as a child so it follows the swing; lights dimmed as hand effects.
pub(crate) fn play_on_staff_tip(commands: &mut Commands, lib: &EffectLibrary, fp: &FpSockets, camera: Option<Entity>, globals: &Query<&GlobalTransform>, key: &str) -> Option<Entity> {
    debug!("staff tip: weapon bone {:?} (placed: {}), left {:?}", fp.right, fp.right.is_some_and(|b| globals.contains(b)), fp.left);
    let (parent, off) = match fp.right.filter(|b| globals.contains(*b)) {
        Some(b) => (b, BONE_STAFF_TIP),
        None => (camera?, CAM_STAFF),
    };
    let s = globals.get(parent).map(|g| g.scale()).unwrap_or(Vec3::ONE).max(Vec3::splat(1e-4));
    let e = spawn_effect(commands, lib, key, off / s, Quat::IDENTITY, None)?;
    commands.entity(e).insert(Transform::from_translation(off / s).with_scale(Vec3::ONE / s));
    crate::effects::dim_lights(commands, e, FP_LIGHT);
    commands.entity(parent).add_child(e);
    Some(e)
}

/// How far out of a target's centre its skill hit effects play (cm, about a body's radius; HYPOTHESIS).
const HIT_OUT: f32 = 55.0;

const CHARGED: [SkillKind; 3] = [SkillKind::Pyroblast, SkillKind::FrostTorment, SkillKind::HealSingle];

/// Casts, charges, hits, bursts and projectiles, from this frame's sim events and shots.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn skill_vfx_events(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    table: Res<SkillVfx>,
    mut state: ResMut<SkillVfxState>,
    fp: Res<FpSockets>,
    events: Res<crate::combat_view::FrameEvents>,
    player: Res<crate::LocalPlayer>,
    index: Res<SimIndex>,
    positions: Query<&Position>,
    shots: Query<(&SimId, &Position, &db_sim::Projectile)>,
    views: Query<(Entity, &crate::combat_view::ActorView)>,
    camera: Query<Entity, With<crate::PlayerCamera>>,
    globals: Query<&'static GlobalTransform>,
) {
    let ctx = Ctx { lib: &lib, local: player.0, camera: camera.iter().next(), fp: &fp, globals: &globals };
    let pos_of = |id: SimId| index.get(id).and_then(|e| positions.get(e).ok()).map(|p| p.0);
    let st = &mut *state;
    for ev in &events.0 {
        match *ev {
            SimEvent::SkillStarted { id, skill } => {
                if CHARGED.contains(&skill) {
                    stop_charge(&mut commands, st, id, |_| true);
                    let started: Vec<(Phase, Entity)> = table.triggers(skill, |p| p == Phase::Charge(0)).filter_map(|t| Some((t.phase, ctx.play(&mut commands, t, id, pos_of(id))?))).collect();
                    st.charging.insert(id, started);
                } else {
                    for t in table.triggers(skill, |p| p == Phase::Cast(None)) {
                        ctx.play(&mut commands, t, id, pos_of(id));
                    }
                    // An area with no tracked place plays once at the feet.
                    if !matches!(skill, SkillKind::Blizzard | SkillKind::Oracle) {
                        for t in table.triggers(skill, |p| p == Phase::Area) {
                            if let Some(p) = pos_of(id) {
                                ctx.at(&mut commands, t, p + Vec3::Z * FEET, Quat::IDENTITY);
                            }
                        }
                    }
                }
            }
            SimEvent::SkillCharged { id, skill, stage } => {
                stop_charge(&mut commands, st, id, |p| p == Phase::Charge(stage - 1));
                let mut started: Vec<(Phase, Entity)> = table.triggers(skill, |p| p == Phase::Charge(stage) || p == Phase::Hold(stage)).filter_map(|t| Some((t.phase, ctx.play(&mut commands, t, id, pos_of(id))?))).collect();
                for t in table.triggers(skill, |p| p == Phase::Reach(stage)) {
                    ctx.play(&mut commands, t, id, pos_of(id));
                }
                st.charging.entry(id).or_default().append(&mut started);
            }
            SimEvent::SkillCast { id, skill, stage } => {
                stop_charge(&mut commands, st, id, |_| true);
                st.stage.insert(id, stage);
                for t in table.triggers(skill, |p| p == Phase::Cast(Some(stage))) {
                    ctx.play(&mut commands, t, id, pos_of(id));
                }
            }
            SimEvent::SkillEnded { id, skill } if CHARGED.contains(&skill) => stop_charge(&mut commands, st, id, |_| true),
            SimEvent::Damaged { source, target, by: db_sim::HitSource::Skill(skill), .. } => {
                // On the target's surface towards the caster (the capsule centre hides a small burst inside a
                // body), local +X out of it like the generic hits.
                let Some(p) = pos_of(target) else { continue };
                let normal = pos_of(source).map_or(Vec3::X, |s| (s - p).with_z(0.0).normalize_or(Vec3::X));
                for t in table.triggers(skill, |p| p == Phase::Hit) {
                    ctx.at(&mut commands, t, p + normal * HIT_OUT, Quat::from_rotation_arc(Vec3::X, normal));
                }
            }
            // A charge's ground hit: its damage area's `FX_Common_Mace_Range01` (`ActivateEffect`).
            SimEvent::GroundHit { at, .. } => {
                debug!("ground hit at {at:?}");
                crate::effects::spawn_effect(&mut commands, &lib, "mace6002_ground_hit", to_bevy(at), Quat::IDENTITY, None);
            }
            // (Pyroblast's small fireballs explode too, 50 cm: their impact draws that; the burst effect is the large
            // fireball's.)
            SimEvent::Burst { at, by: db_sim::HitSource::Skill(skill), radius, .. } if skill != db_sim::SkillKind::Pyroblast || radius > SMALL_BURST => {
                for t in table.triggers(skill, |p| p == Phase::Burst) {
                    ctx.at(&mut commands, t, at, Quat::IDENTITY);
                }
            }
            SimEvent::Despawned { id } => {
                if let Some((at, dir, skill, stage)) = st.flying.remove(&id) {
                    for t in table.triggers(skill, |p| p == Phase::Impact(None) || (stage.is_some() && p == Phase::Impact(stage))) {
                        ctx.at(&mut commands, t, at, Quat::from_rotation_arc(Vec3::X, -dir));
                    }
                }
            }
            _ => {}
        }
    }
    // Trails on the skill's shots (once their view is up); remember where each is for its impact.
    let view_of: HashMap<SimId, Entity> = views.iter().map(|(e, v)| (v.0, e)).collect();
    for (&id, p, shot) in &shots {
        let db_sim::HitSource::Skill(skill) = shot.spec.by else { continue };
        if let Some(f) = st.flying.get_mut(&id) {
            (f.0, f.1) = (p.0, shot.dir);
            continue;
        }
        let Some(&view) = view_of.get(&id) else { continue };
        let stage = st.stage.get(&shot.owner).copied();
        for t in table.triggers(skill, |ph| ph == Phase::Projectile(None) || (stage.is_some() && ph == Phase::Projectile(stage))) {
            // The skill projectile systems are authored along local +Z (Bullet03's flame cone points down -Z
            // behind the ball, its trail fire streams to -Z: HYPOTHESIS); local to the view so it follows.
            if let Some(fx) = spawn_effect(&mut commands, &lib, &t.effect, Vec3::ZERO, Quat::from_rotation_arc(Vec3::Z, shot.dir), None) {
                commands.entity(view).add_child(fx);
            }
        }
        st.flying.insert(id, (p.0, shot.dir, skill, stage));
    }
    st.flying.retain(|id, _| index.get(*id).is_some());
}

fn stop_charge(commands: &mut Commands, st: &mut SkillVfxState, id: SimId, which: impl Fn(Phase) -> bool) {
    if let Some(list) = st.charging.get_mut(&id) {
        list.retain(|&(p, e)| {
            if which(p) {
                stop(commands, e);
                false
            } else {
                true
            }
        });
    }
}

/// Buff loops (while an actor holds a buff of a skill) and areas (Blizzard's storm, Oracle's point).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn skill_vfx_loops(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    table: Res<SkillVfx>,
    mut state: ResMut<SkillVfxState>,
    fp: Res<FpSockets>,
    player: Res<crate::LocalPlayer>,
    actors: Query<(&SimId, &Position, &db_sim::Kind, Option<&db_sim::Buffs>, Option<&db_sim::ClassKit>, Option<&db_sim::SkillKit>)>,
    camera: Query<Entity, With<crate::PlayerCamera>>,
    globals: Query<&'static GlobalTransform>,
    mut tf: Query<&mut Transform>,
) {
    let ctx = Ctx { lib: &lib, local: player.0, camera: camera.iter().next(), fp: &fp, globals: &globals };
    let st = &mut *state;
    let mut held: HashSet<(SimId, SkillKind)> = HashSet::new();
    let mut areas: HashMap<(SimId, SkillKind), Vec3> = HashMap::new();
    let mut at: HashMap<SimId, Vec3> = HashMap::new();
    let mut player_ids: HashSet<SimId> = HashSet::new();
    for (&id, p, kind, buffs, kit, fighter) in &actors {
        at.insert(id, p.0);
        // Rend armed (the next weapon hits bleed) is the Fighter kit's window, not a buff.
        if fighter.is_some_and(|k| k.rend_armed.is_some()) {
            held.insert((id, SkillKind::Rend));
        }
        if kind.0 == ActorKind::Player {
            player_ids.insert(id);
        }
        for b in buffs.into_iter().flat_map(|b| b.0.iter()) {
            if !matches!(b.kind, db_sim::BuffKind::Rooted) {
                held.insert((id, b.skill));
            }
        }
        if let Some(k) = kit {
            if let Some(b) = k.blizzard.filter(|b| b.channel > 0) {
                areas.insert((id, SkillKind::Blizzard), b.pos);
            }
            if let Some((p, _)) = k.oracle {
                areas.insert((id, SkillKind::Oracle), p);
            }
        }
    }
    // New buffs: their loops and one-shots.
    for &(id, skill) in &held {
        if st.buffs.contains_key(&(id, skill)) {
            continue;
        }
        let phase = if player_ids.contains(&id) { Phase::BuffSelf } else { Phase::Buff };
        let started: Vec<Entity> = table.triggers(skill, |p| p == phase).filter_map(|t| ctx.play(&mut commands, t, id, at.get(&id).copied())).collect();
        for t in table.triggers(skill, |p| p == Phase::BuffStart) {
            ctx.play(&mut commands, t, id, at.get(&id).copied());
        }
        st.buffs.insert((id, skill), started);
    }
    // Ended buffs.
    let gone: Vec<(SimId, SkillKind)> = st.buffs.keys().filter(|k| !held.contains(k)).copied().collect();
    for k in gone {
        for e in st.buffs.remove(&k).unwrap_or_default() {
            stop(&mut commands, e);
        }
        if let Some(&p) = at.get(&k.0) {
            for t in table.triggers(k.1, |ph| ph == Phase::End) {
                ctx.play(&mut commands, t, k.0, Some(p));
            }
        }
    }
    // Areas: started at their place, moved along with it, stopped when it goes.
    for (&key, &p) in &areas {
        match st.areas.get(&key) {
            Some(list) => {
                for &e in list {
                    if let Ok(mut t) = tf.get_mut(e) {
                        t.translation = to_bevy(p);
                    }
                }
            }
            None => {
                let started = table.triggers(key.1, |ph| ph == Phase::Area).filter_map(|t| ctx.at(&mut commands, t, p, Quat::IDENTITY)).collect();
                st.areas.insert(key, started);
            }
        }
    }
    let gone: Vec<(SimId, SkillKind)> = st.areas.keys().filter(|k| !areas.contains_key(k)).copied().collect();
    for k in gone {
        for e in st.areas.remove(&k).unwrap_or_default() {
            stop(&mut commands, e);
        }
    }
}

/// Effects on actors follow them (drawn between ticks); an actor that's gone takes its effects along.
pub(crate) fn follow_actors(mut commands: Commands, index: Res<SimIndex>, positions: Query<(&Position, Option<&crate::PrevPosition>)>, fixed: Res<Time<Fixed>>, mut q: Query<(Entity, &Follow, &mut Transform)>) {
    for (e, f, mut t) in &mut q {
        match index.get(f.id).and_then(|s| positions.get(s).ok()) {
            Some((p, prev)) => t.translation = to_bevy(crate::drawn(p, prev, &fixed) + Vec3::Z * f.up),
            None => stop(&mut commands, e),
        }
    }
}

/// Resources and systems; the particle systems themselves run in `effects`.
pub(crate) fn plugin(app: &mut App) {
    use crate::{combat_view, effects};
    app.init_resource::<SkillVfx>()
        .init_resource::<SkillVfxState>()
        .init_resource::<FpSockets>()
        .init_resource::<AutoRelease>()
        .add_systems(Startup, load_skill_vfx)
        .add_systems(Update, auto_release.run_if(|r: Res<AutoRelease>| r.0.is_some()))
        .add_systems(Update, vfx_shots.run_if(resource_exists::<VfxShots>).after(combat_view::read_events))
        .add_systems(Update, (find_fp_sockets, skill_vfx_events, skill_vfx_loops, follow_actors).chain().after(combat_view::read_events).after(combat_view::sync_actor_views).before(effects::tick_effects))
        .add_systems(Update, effects::tick_ribbons.after(effects::tick_particles));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfx_shot_plans_parse() {
        assert_eq!(VfxShots::parse("fs:1:0.1,0.25"), Some(VfxShots { name: "fs".into(), skip: 1, at: vec![0.1, 0.25], monster_attack: None }));
        assert_eq!(VfxShots::parse("rp:0:1,2:Reaper_Attack5").and_then(|p| p.monster_attack), Some("Reaper_Attack5".into()));
        assert_eq!(VfxShots::parse("fs:x:0.1"), None);
        assert_eq!(VfxShots::parse("fs:0:"), None);
    }

    #[test]
    fn phases_and_sockets_parse() {
        assert_eq!(Phase::parse("charge0"), Some(Phase::Charge(0)));
        assert_eq!(Phase::parse("hold2"), Some(Phase::Hold(2)));
        assert_eq!(Phase::parse("cast"), Some(Phase::Cast(None)));
        assert_eq!(Phase::parse("cast2"), Some(Phase::Cast(Some(2))));
        assert_eq!(Phase::parse("projectile1"), Some(Phase::Projectile(Some(1))));
        assert_eq!(Phase::parse("buff_self"), Some(Phase::BuffSelf));
        assert_eq!(Phase::parse("charge"), None);
        assert_eq!(Phase::parse("nonsense"), None);
        assert_eq!(Socket::parse("VFX-HandLeft_Spell01_1st"), Socket::LeftHand);
        assert_eq!(Socket::parse("Weapon_L_Help-VFX01"), Socket::LeftHand);
        assert_eq!(Socket::parse("Hand_R_VFX"), Socket::RightHand);
        assert_eq!(Socket::parse("VFX-Priest-Q"), Socket::StaffTip);
        assert_eq!(Socket::parse("FireStart"), Socket::StaffTip);
        assert_eq!(Socket::parse("ThunderCast"), Socket::StaffTip);
        assert_eq!(Socket::parse("Root"), Socket::Root);
        assert_eq!(Socket::parse("EP_Chest"), Socket::Chest);
        assert_eq!(Socket::parse(""), Socket::Chest);
    }

    #[test]
    fn a_missing_table_or_effect_plays_nothing() {
        let t = SkillVfx::from_json(&serde_json::json!({}));
        assert_eq!(t.triggers(SkillKind::Pyroblast, |_| true).count(), 0);
        assert!(!t.has_impact(SkillKind::Pyroblast));
        let t = SkillVfx::from_json(&serde_json::json!({"skills": {"Pyroblast": {"triggers": [{"phase": "burst", "effect": "sys:Nope", "socket": "", "view": "both"}, {"phase": "bad", "effect": "x"}]}, "NotASkill": {}}}));
        assert_eq!(t.triggers(SkillKind::Pyroblast, |_| true).count(), 1);
        assert!(t.has_impact(SkillKind::Pyroblast));
        // An unknown effect key spawns nothing (no panic).
        let mut world = World::new();
        let lib = EffectLibrary::default();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        assert!(spawn_effect(&mut commands, &lib, "sys:Nope", Vec3::ZERO, Quat::IDENTITY, None).is_none());
        queue.apply(&mut world);
    }

    /// The extracted table covers every class skill, every phase parses and every effect it names is
    /// converted (when the data is there).
    #[test]
    fn the_table_covers_every_skill() {
        let (Some(p), Some(fx)) = (crate::map_view::data_file("recovered/effects/skill_vfx.json"), crate::map_view::data_file("recovered/effects/effects.json")) else { return };
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        let effects: serde_json::Value = serde_json::from_slice(&std::fs::read(fx).unwrap()).unwrap();
        for s in v["skills"].as_object().unwrap().values() {
            for t in s["triggers"].as_array().unwrap() {
                assert!(Phase::parse(t["phase"].as_str().unwrap()).is_some(), "phase {}", t["phase"]);
                let key = t["effect"].as_str().unwrap();
                assert!(effects.get(key).is_some(), "{key} not in effects.json");
            }
        }
        let table = SkillVfx::from_json(&v);
        for k in SkillKind::ALL {
            assert!(table.triggers(k, |_| true).next().is_some(), "{k:?} has no effects");
        }
        assert_eq!(table.table.len(), SkillKind::ALL.len());
        // the staffs' effects are converted, each phase known, and the Fire and Thunder spells have theirs.
        for s in v["staffs"].as_object().unwrap().values() {
            for t in s["triggers"].as_array().unwrap() {
                assert!(crate::staff_view::PHASES.contains(&t["phase"].as_str().unwrap()), "staff phase {}", t["phase"]);
                let key = t["effect"].as_str().unwrap();
                assert!(effects.get(key).is_some(), "{key} not in effects.json");
            }
        }
        use db_sim::staffs::StaffElement;
        for (el, phase) in [(StaffElement::Fire, "beam"), (StaffElement::Fire, "recharge"), (StaffElement::Thunder, "cloud"), (StaffElement::Thunder, "aim"), (StaffElement::Ice, "hit")] {
            assert!(table.staff(el, phase, true).next().is_some(), "{el:?} {phase}");
        }
    }
}

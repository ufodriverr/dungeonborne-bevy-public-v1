//! Staffs on screen: the staff energy bar, the cast-fail tip, and stand-in visuals for the Fire beam, the
//! Thunder aim point, the thundercloud and its strikes. The sim (`db_sim::staffs`) decides everything; this only
//! draws its `StaffState` / `ThunderAreas` and events.
//!
//! CONFIRMED: the bar's logic from `WBP_TgfGame_Dashboard_Weapon.lua` (shown when the held weapon has an ammo
//! type; `AmmoDisplayCnt` equal segments; a filling segment uses the grey texture, full ones the coloured one) and
//! its art (`T_UI_WeaponEnergyBar_NewBG`, `T_UI_Energy_<Element>{,Grey}_Inner`). HYPOTHESIS: its place on screen
//! (the widget layout isn't decoded); the tip's text (an English rendering of
//! `CastFail_Tips_CostNotEnough_StaffPowner`).
//!
//! the staffs' own Niagara systems from the trigger table's `staffs` section (`skill_vfx.json`, links): the Fire
//! beam `FX_Common_Fire_Skill01_Laser` stretched as a beam from the staff head (the staff mesh's `FireStart`
//! socket, CONFIRMED) to the trace end, its end `FX_Common_Fire_Skill01_Hit` the recharge lines
//! `FX_Common_Cast_Fire` / `_Thunder` from the staff head into the left hand (the recharge montages' `FireLine`
//! notifies, CONFIRMED); the Thunder aim `FX_Indicator_Thunder_01` (by name), its release
//! `FX_Common_Lightning_Cast01` and the cloud `FX_Common_Lightning_Skill_Staff` (the area actor's system: it
//! bursts every second, as the sim strikes); ice staff hits `FX_Common_Hit_Ice02`. HYPOTHESIS: when each plays
//! (the phase table in `zen_imports.py`), the beam lasting the sim's firing window, the cloud's bolts drawn from
//! the ground point 7 m up. The plain shapes below stay as stand-ins when the converted data is missing.

use bevy::prelude::*;
use db_sim::staffs::{StaffAction, StaffElement, StaffState, ThunderAreas};
use db_sim::{Position, SimEvent, SimIndex};

use crate::combat_view::{FrameEvents, LocalPlayer};
use crate::effects::{EffectLibrary, MonsterActorVfx, spawn_actor_system, spawn_effect};
use crate::flow::GameHud;
use crate::skill_vfx::{FpSockets, SkillVfx, fp_left_palm, fp_staff_ready, play_on_staff_tip};
use crate::to_bevy;

const TEX: &str = "textures_256/Dungeonborne/Content/TGF/UI/Game/HUD/Dashboard/Texture/";
/// Bar background and its inner strip (px, the textures at 1:1).
const BAR: (f32, f32) = (185.0, 33.0);
const INNER: (f32, f32) = (162.0, 9.0);
/// Gap between segments (px; `T_UI_TgfGame_EnergySpace` is 8 px wide at its 18 px height, scaled to the strip).
const GAP: f32 = 4.0;
/// The bar's place: right of the dashboard's potion boxes, px from the screen centre and the bottom (ours).
const BAR_DX: f32 = 250.0;
const BAR_BOTTOM: f32 = 64.0;
const TIP_SECONDS: f32 = 1.5;
/// A strike bolt's height above the ground and how long it shows (m, s; ours).
const BOLT_HEIGHT: f32 = 7.0;
const BOLT_SECONDS: f32 = 0.18;

/// Dev: hold the staff spell 1.5 s, let go 1.5 s, over and over, from a render frame on (`--auto-cast [--auto-cast-at
/// <frame>]`; hidden checks: the test room runs many sim ticks while it loads); hold the recharge all the time
/// (`--auto-recharge`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoCast(pub bool, pub u32, pub bool);

#[derive(Component)]
struct EnergyBar;
/// Segment `n`: the clip box and the coloured / grey strips inside it.
#[derive(Component)]
struct Segment(usize);
#[derive(Component)]
struct SegmentFull(usize);
#[derive(Component)]
struct SegmentGrey(usize);
#[derive(Component)]
struct StaffTip(f32);

#[derive(Component)]
struct Beam;
#[derive(Component)]
struct AimRing;
/// A thundercloud's column: (owner, at) identify it.
#[derive(Component)]
struct Cloud(db_sim::SimId, Vec3);
#[derive(Component)]
struct Bolt(f32);

/// The staff phases the trigger table may name (`skill_vfx.json` `staffs`; `zen_imports.py` `STAFF_TRIGGERS`).
pub(crate) const PHASES: [&str; 11] = ["beam", "beam_end", "beam_tip", "recharge", "aim", "release", "cloud", "strike", "hit", "explode", "burn"];
/// A thundercloud's bolts: from the ground point this far up, wandering this far off the line (m, ours).
const CLOUD_BOLT_UP: f32 = 7.0;
const CLOUD_BOLT_JITTER: f32 = 0.35;

/// The local player's playing staff effects.
#[derive(Resource, Default)]
struct StaffFx {
    /// While the beam fires: the beams on the staff head, the end effects, the staff head's fire.
    beam: Vec<Entity>,
    beam_end: Vec<Entity>,
    beam_tip: Vec<Entity>,
    recharge: Vec<Entity>,
    aim: Vec<Entity>,
    /// Live clouds: (owner, point) and their effects.
    clouds: Vec<(db_sim::SimId, Vec3, Vec<Entity>)>,
    /// The local player was releasing a Thunder spell last frame.
    releasing: bool,
    /// The beam / recharge effects were started before the first-person body was in (at the camera offset):
    /// they restart on the weapon bone once it is.
    beam_fallback: bool,
    recharge_fallback: bool,
}

fn stop_all(commands: &mut Commands, v: &mut Vec<Entity>) {
    for e in v.drain(..) {
        crate::effects::stop(commands, e);
    }
}

#[derive(Resource)]
struct FxAssets {
    unit_cylinder: Handle<Mesh>,
    beam: Handle<StandardMaterial>,
    ring: Handle<StandardMaterial>,
    cloud: Handle<StandardMaterial>,
    bolt: Handle<StandardMaterial>,
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<AutoCast>()
        .init_resource::<StaffFx>()
        .add_systems(Startup, setup)
        .add_systems(Update, (energy_bar, tip, beam_fx, recharge_fx, thunder_fx, staff_hits, burn_fx).after(crate::combat_view::read_events).before(crate::effects::tick_effects));
}

fn element_textures(e: StaffElement) -> (&'static str, &'static str) {
    match e {
        StaffElement::Fire => ("T_UI_Energy_Fire_Inner", "T_UI_Energy_FireGrey_Inner"),
        StaffElement::Ice => ("T_UI_Energy_Ice_Inner", "T_UI_Energy_IceGrey_Inner"),
        StaffElement::Thunder => ("T_UI_Energy_Thunder_Inner", "T_UI_Energy_ThunderGrey_Inner"),
    }
}

fn abs(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: px(left), top: px(top), width: px(w), height: px(h), ..default() }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    // Additive, unlit: the colour is the light the shape adds (kept below the bloom's white-out).
    let glow = |r: f32, g: f32, b: f32, a: f32| StandardMaterial { base_color: Color::srgba(r, g, b, a), unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, ..default() };
    commands.insert_resource(FxAssets {
        unit_cylinder: meshes.add(Cylinder::new(1.0, 1.0)),
        beam: mats.add(glow(1.0, 0.42, 0.08, 0.9)),
        ring: mats.add(glow(0.35, 0.6, 1.0, 0.45)),
        cloud: mats.add(glow(0.12, 0.18, 0.45, 0.08)),
        bolt: mats.add(glow(0.75, 0.85, 1.0, 1.0)),
    });
    // Nodes that hide themselves sit under a `GameHud` root (`flow::hud_visibility` drives the marked nodes).
    // Full screen but never in the way of clicks (the inventory's buttons sit under it).
    let root = commands.spawn((Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() }, bevy::picking::Pickable::IGNORE, GameHud)).id();
    let bar = commands
        .spawn((
            ChildOf(root),
            Node { position_type: PositionType::Absolute, left: percent(50.0), bottom: px(BAR_BOTTOM), width: px(BAR.0), height: px(BAR.1), margin: UiRect::left(px(BAR_DX)), ..default() },
            ImageNode::new(assets.load(format!("{TEX}T_UI_WeaponEnergyBar_NewBG.png"))),
            GlobalZIndex(2),
            Visibility::Hidden,
            EnergyBar,
        ))
        .id();
    let (ox, oy) = ((BAR.0 - INNER.0) / 2.0, (BAR.1 - INNER.1) / 2.0);
    for n in 0..5 {
        commands.spawn((ChildOf(bar), Node { overflow: Overflow::clip(), ..abs(ox, oy, 0.0, INNER.1) }, Visibility::Hidden, Segment(n))).with_children(|s| {
            s.spawn((ImageNode::default(), abs(0.0, 0.0, INNER.0, INNER.1), SegmentFull(n)));
            s.spawn((ImageNode::default(), abs(0.0, 0.0, INNER.0, INNER.1), SegmentGrey(n)));
        });
    }
    commands.spawn((
        ChildOf(root),
        Text::new(""),
        crate::fonts::sans(18.0),
        TextColor(Color::srgb(1.0, 0.55, 0.35)),
        TextShadow::default(),
        Node { position_type: PositionType::Absolute, left: percent(50.0), bottom: px(BAR_BOTTOM + BAR.1 + 6.0), margin: UiRect::left(px(BAR_DX)), ..default() },
        Visibility::Hidden,
        StaffTip(TIP_SECONDS),
    ));
}

/// The held staff's energy in `AmmoDisplayCnt` segments: full ones coloured, the filling one grey to its share;
/// without a staff, the held relic's (its `AmmoType` is the ice staff's, so the ice bar).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn energy_bar(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    assets: Res<AssetServer>,
    states: Query<&StaffState>,
    relics: Query<&db_sim::weapons::RelicEnergy>,
    mut bar: Query<&mut Visibility, (With<EnergyBar>, Without<Segment>)>,
    mut segs: Query<(&Segment, &mut Node, &mut Visibility), Without<EnergyBar>>,
    mut strips: ParamSet<(Query<(&SegmentFull, &mut ImageNode, &mut Node), Without<Segment>>, Query<(&SegmentGrey, &mut ImageNode, &mut Node), Without<Segment>>)>,
    mut shown: Local<Option<StaffElement>>,
) {
    let st = player.0.and_then(|id| index.get(id)).and_then(|e| states.get(e).ok());
    let relic = || player.0.and_then(|id| index.get(id)).and_then(|e| relics.get(e).ok()).and_then(db_sim::weapons::RelicEnergy::held_energy).map(|(cur, max, n)| ((StaffElement::Ice, usize::from(n.max(1))), (cur, max)));
    let held = st.and_then(|s| s.held.as_ref().map(|h| (h.1.element, h.1.segments.max(1) as usize))).zip(st.and_then(StaffState::held_energy)).or_else(relic);
    let Ok(mut vis) = bar.single_mut() else { return };
    let Some(((element, n), (cur, max))) = held else {
        vis.set_if_neq(Visibility::Hidden);
        return;
    };
    vis.set_if_neq(Visibility::Inherited);
    let seg_w = (INNER.0 - GAP * (n as f32 - 1.0)) / n as f32;
    let per = f64::from(max) / n as f64;
    let ox = (BAR.0 - INNER.0) / 2.0;
    if *shown != Some(element) {
        *shown = Some(element);
        let (full, grey) = element_textures(element);
        for (_, mut img, _) in &mut strips.p0() {
            img.image = assets.load(format!("{TEX}{full}.png"));
        }
        for (_, mut img, _) in &mut strips.p1() {
            img.image = assets.load(format!("{TEX}{grey}.png"));
        }
    }
    let mut fill = [(0.0f32, false); 5];
    for (i, f) in fill.iter_mut().enumerate().take(n) {
        let share = ((f64::from(cur) - per * i as f64) / per).clamp(0.0, 1.0) as f32;
        *f = (share, share >= 1.0);
    }
    for (s, mut node, mut v) in &mut segs {
        if s.0 >= n {
            v.set_if_neq(Visibility::Hidden);
            continue;
        }
        v.set_if_neq(Visibility::Inherited);
        node.left = px(ox + s.0 as f32 * (seg_w + GAP));
        node.width = px(seg_w * fill[s.0].0);
    }
    // The strips keep the texture's own place under each segment's clip (cropped, not stretched).
    for (f, _, mut node) in &mut strips.p0() {
        node.left = px(-(f.0 as f32) * (seg_w + GAP));
        node.display = if fill.get(f.0).is_some_and(|x| x.1) { Display::Flex } else { Display::None };
    }
    for (g, _, mut node) in &mut strips.p1() {
        node.left = px(-(g.0 as f32) * (seg_w + GAP));
        node.display = if fill.get(g.0).is_some_and(|x| !x.1) { Display::Flex } else { Display::None };
    }
}

/// "Not enough staff energy" (`CastFail_Tips_CostNotEnough_StaffPowner`) for a moment after a refused cast.
fn tip(time: Res<Time>, events: Res<FrameEvents>, player: Res<LocalPlayer>, mut q: Query<(&mut StaffTip, &mut Text, &mut Visibility)>) {
    let Ok((mut t, mut text, mut vis)) = q.single_mut() else { return };
    if events.0.iter().any(|e| matches!(e, SimEvent::StaffRejected { id } if Some(*id) == player.0)) {
        t.0 = 0.0;
        text.0 = "Not enough staff energy".into();
    }
    t.0 += time.delta_secs();
    vis.set_if_neq(if t.0 < TIP_SECONDS { Visibility::Inherited } else { Visibility::Hidden });
}

/// A unit cylinder (along Y) stretched from `a` to `b` (Bevy space) with radius `r`.
fn segment(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(1e-3);
    Transform { translation: (a + b) * 0.5, rotation: Quat::from_rotation_arc(Vec3::Y, d / len), scale: Vec3::new(r, len, r) }
}

/// The beam from the staff head to the trace's end while it fires (the `Cast` to `End` notifies): the staff's
/// beam, end and staff-head effects, or the stand-in tube without them.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn beam_fx(
    mut commands: Commands,
    fx: Option<Res<FxAssets>>,
    lib: Res<EffectLibrary>,
    table: Res<SkillVfx>,
    fp: Res<FpSockets>,
    mut playing: ResMut<StaffFx>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    states: Query<(&StaffState, &Position)>,
    cam: Query<(Entity, &GlobalTransform), With<crate::PlayerCamera>>,
    globals: Query<&GlobalTransform>,
    mut beams: Query<(Entity, &mut Transform), With<Beam>>,
    mut lines: Query<&mut crate::effects::Beam>,
    mut ends: Query<&mut Transform, Without<Beam>>,
) {
    let Some(fx) = fx else { return };
    let mut want = None;
    if let Some((s, pos)) = player.0.and_then(|id| index.get(id)).and_then(|e| states.get(e).ok())
        && let (Some(StaffAction::Beam { tick, end, cancel: None, .. }), Some((_, spec))) = (&s.action, &s.held)
        && let Some(b) = &spec.beam
    {
        let t = *tick as f32 * db_sim::DT;
        if t >= b.cast_at && t < b.end_at {
            want = Some((spec.element, *end, pos.0));
        }
    }
    let st = &mut *playing;
    let real = want.is_some_and(|(el, ..)| table.staff(el, "beam", true).any(|k| lib.has(k)));
    let ready = fp_staff_ready(&fp, &globals);
    if st.beam_fallback && ready {
        stop_all(&mut commands, &mut st.beam);
        stop_all(&mut commands, &mut st.beam_end);
    }
    match want {
        Some((el, end, _)) if real => {
            let end = to_bevy(end);
            if st.beam.is_empty() {
                st.beam_fallback = !ready;
                let camera = cam.iter().next().map(|c| c.0);
                for k in table.staff(el, "beam", true) {
                    if let Some(e) = play_on_staff_tip(&mut commands, &lib, &fp, camera, &globals, k) {
                        commands.entity(e).insert(crate::effects::Beam { end, jitter: 0.0 });
                        st.beam.push(e);
                    }
                }
                for k in table.staff(el, "beam_tip", true) {
                    st.beam_tip.extend(play_on_staff_tip(&mut commands, &lib, &fp, camera, &globals, k));
                }
                for k in table.staff(el, "beam_end", true) {
                    st.beam_end.extend(spawn_effect(&mut commands, &lib, k, end, Quat::IDENTITY, None));
                }
            }
            for e in &st.beam {
                if let Ok(mut b) = lines.get_mut(*e) {
                    b.end = end;
                }
            }
            for e in &st.beam_end {
                if let Ok(mut t) = ends.get_mut(*e) {
                    t.translation = end;
                }
            }
        }
        _ => {
            stop_all(&mut commands, &mut st.beam);
            stop_all(&mut commands, &mut st.beam_tip);
            stop_all(&mut commands, &mut st.beam_end);
        }
    }
    // The stand-in: from just below and right of the view (roughly the staff head).
    let want = want.filter(|_| !real).map(|(_, end, pos)| {
        let from = cam.single().map_or(to_bevy(pos + Vec3::Z * db_sim::staffs::EYE_HEIGHT), |(_, c)| c.translation() + c.forward() * 0.55 + c.right() * 0.22 + c.down() * 0.22);
        (from, to_bevy(end))
    });
    match (want, beams.single_mut()) {
        (Some((a, b)), Ok((_, mut t))) => *t = segment(a, b, 0.03),
        (Some((a, b)), Err(_)) => {
            commands.spawn((Mesh3d(fx.unit_cylinder.clone()), MeshMaterial3d(fx.beam.clone()), segment(a, b, 0.03), bevy::light::NotShadowCaster, Beam));
        }
        (None, Ok((e, _))) => commands.entity(e).despawn(),
        (None, Err(_)) => {}
    }
}

/// The recharge line while the right button recharges: the montages' `FireLine` notify, a beam from the staff's
/// `FireStart` to the left hand's `Weapon_L_Common_Cast_1st` (`BeamEndBone`; CONFIRMED, `AM_Staff_Recharge_1st` /
/// `_Thunder_1st`).
#[allow(clippy::too_many_arguments)]
fn recharge_fx(mut commands: Commands, lib: Res<EffectLibrary>, table: Res<SkillVfx>, fp: Res<FpSockets>, mut playing: ResMut<StaffFx>, player: Res<LocalPlayer>, index: Res<SimIndex>, states: Query<&StaffState>, cam: Query<Entity, With<crate::PlayerCamera>>, globals: Query<&GlobalTransform>, mut lines: Query<&mut crate::effects::Beam>) {
    let el = player.0.and_then(|id| index.get(id)).and_then(|e| states.get(e).ok()).filter(|s| matches!(s.action, Some(StaffAction::Recharge { .. }))).and_then(StaffState::element);
    let camera = cam.iter().next();
    let Some(el) = el else {
        stop_all(&mut commands, &mut playing.recharge);
        return;
    };
    let Some(palm) = fp_left_palm(&fp, camera, &globals) else { return };
    let ready = fp_staff_ready(&fp, &globals);
    if playing.recharge_fallback && ready {
        stop_all(&mut commands, &mut playing.recharge);
    }
    if playing.recharge.is_empty() {
        playing.recharge_fallback = !ready;
        for k in table.staff(el, "recharge", true) {
            if let Some(e) = play_on_staff_tip(&mut commands, &lib, &fp, camera, &globals, k) {
                commands.entity(e).insert(crate::effects::Beam { end: palm, jitter: 0.0 });
                playing.recharge.push(e);
            }
        }
    }
    for e in &playing.recharge {
        if let Ok(mut b) = lines.get_mut(*e) {
            b.end = palm;
        }
    }
}

/// The ice staff's hit effect on each hit of an ice staff's swing (`GCNL_TGF_Staff_IceHit`; HYPOTHESIS: on every
/// combo hit, at the target's centre).
#[allow(clippy::too_many_arguments)]
fn staff_hits(mut commands: Commands, lib: Res<EffectLibrary>, table: Res<SkillVfx>, events: Res<FrameEvents>, player: Res<LocalPlayer>, index: Res<SimIndex>, states: Query<&StaffState>, positions: Query<&Position>) {
    for e in &events.0 {
        // The legendary fire staff's burst at the beam's end.
        if let SimEvent::StaffExplode { id, at, .. } = *e {
            for k in table.staff(StaffElement::Fire, "explode", player.0 == Some(id)) {
                spawn_effect(&mut commands, &lib, k, to_bevy(at), Quat::IDENTITY, None);
            }
            continue;
        }
        let SimEvent::Damaged { source, target, by: db_sim::HitSource::Swing(_), .. } = *e else { continue };
        let Some(el) = index.get(source).and_then(|x| states.get(x).ok()).and_then(StaffState::element) else { continue };
        let Some(at) = index.get(target).and_then(|x| positions.get(x).ok()) else { continue };
        for k in table.staff(el, "hit", player.0 == Some(source)) {
            spawn_effect(&mut commands, &lib, k, to_bevy(at.0), Quat::IDENTITY, None);
        }
    }
}

/// The burn loop (`FX_Common_Buff_Fire1`) on each burning character while it burns. Not on yourself: there the
/// cue draws a screen overlay (`FX_Debuff_Fire_1st`), not built.
fn burn_fx(mut commands: Commands, lib: Res<EffectLibrary>, table: Res<SkillVfx>, player: Res<LocalPlayer>, index: Res<SimIndex>, actors: Query<(&db_sim::Buffs, &Position)>, mut burns: Local<std::collections::HashMap<db_sim::SimId, Vec<Entity>>>) {
    let burning: Vec<(db_sim::SimId, Vec3)> = index
        .iter()
        .filter_map(|(id, e)| {
            let (b, p) = actors.get(e).ok()?;
            b.0.iter().any(|b| matches!(b.kind, db_sim::BuffKind::Burning { .. })).then_some((id, p.0))
        })
        .collect();
    for &(id, at) in &burning {
        if burns.contains_key(&id) {
            continue;
        }
        let started = table
            .staff(StaffElement::Fire, "burn", player.0 == Some(id))
            .filter_map(|k| {
                let e = spawn_effect(&mut commands, &lib, k, to_bevy(at), Quat::IDENTITY, None)?;
                commands.entity(e).insert(crate::skill_vfx::Follow::new(id, 0.0));
                Some(e)
            })
            .collect();
        burns.insert(id, started);
    }
    burns.retain(|id, list| {
        let keep = burning.iter().any(|b| b.0 == *id);
        if !keep {
            stop_all(&mut commands, list);
        }
        keep
    });
}

/// The ability property that names a monster's thunder area actor (the Wendigo's `AOEConfig`; the Shadow King's storms,
/// `AreaEffect`, keep the staff's look).
pub(crate) const AREA_VIA: [&str; 1] = ["AOEConfig"];

/// The thunder aim, release, clouds and strikes: the staff's effects (the cloud's own bursts are its strikes), or the
/// stand-in ring, columns and bolts (and a flash light at each strike) without them. A monster's area whose actor has
/// its own effects (the Wendigo's thunder, `AOEConfig`'s `B_TGF_Wendigo_ThunderArea`) shows those instead: the parts
/// that start with the actor (its warning) while it lives, the others (the bolt and the ground burst, `bAutoActivate`
/// off) at its strike (`FxActivateTime` 2 s = its strike: HYPOTHESIS, the activation is native).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn thunder_fx(
    mut commands: Commands,
    time: Res<Time>,
    fx: Option<Res<FxAssets>>,
    (lib, table, fp, actor_vfx, kits, mut known): (Res<EffectLibrary>, Res<SkillVfx>, Res<FpSockets>, Res<MonsterActorVfx>, Query<&db_sim::MonsterKit>, Local<Vec<(db_sim::SimId, Vec3, db_sim::HitSource)>>),
    mut playing: ResMut<StaffFx>,
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    areas: Option<Res<ThunderAreas>>,
    states: Query<&StaffState>,
    cam: Query<Entity, With<crate::PlayerCamera>>,
    globals: Query<&GlobalTransform>,
    mut rings: Query<(Entity, &mut Transform), (With<AimRing>, Without<Cloud>, Without<Bolt>)>,
    clouds: Query<(Entity, &Cloud)>,
    mut bolts: Query<(Entity, &mut Bolt, &mut Transform), (Without<AimRing>, Without<Cloud>)>,
    mut placed: Query<&mut Transform, (Without<AimRing>, Without<Cloud>, Without<Bolt>)>,
) {
    use db_sim::staffs::StaffElement::Thunder;
    let Some(fx) = fx else { return };
    let has = |phase: &str| table.staff(Thunder, phase, true).any(|k| lib.has(k));
    let st = &mut *playing;
    let me = player.0.and_then(|id| index.get(id)).and_then(|e| states.get(e).ok());
    let aim = me.and_then(|s| match (&s.action, &s.held) {
        (Some(StaffAction::ThunderAim { point: Some(p), .. }), Some((_, spec))) => spec.thunder.as_ref().map(|t| (*p, t.area.radius)),
        _ => None,
    });
    // The aim point's effect follows the point; without one, the stand-in ring.
    let real_aim = has("aim");
    match aim {
        Some((p, _)) if real_aim => {
            let at = to_bevy(p) + Vec3::Y * 0.03;
            if st.aim.is_empty() {
                st.aim = table.staff(Thunder, "aim", true).filter_map(|k| spawn_effect(&mut commands, &lib, k, at, Quat::IDENTITY, None)).collect();
            }
            for e in &st.aim {
                if let Ok(mut t) = placed.get_mut(*e) {
                    t.translation = at;
                }
            }
        }
        _ => stop_all(&mut commands, &mut st.aim),
    }
    let ring = |p: Vec3, r: f32| Transform { translation: to_bevy(p) + Vec3::Y * 0.03, rotation: Quat::IDENTITY, scale: Vec3::new(r * 0.01, 0.02, r * 0.01) };
    match (aim.filter(|_| !real_aim), rings.single_mut()) {
        (Some((p, r)), Ok((_, mut t))) => *t = ring(p, r),
        (Some((p, r)), Err(_)) => {
            commands.spawn((Mesh3d(fx.unit_cylinder.clone()), MeshMaterial3d(fx.ring.clone()), ring(p, r), bevy::light::NotShadowCaster, AimRing));
        }
        (None, Ok((e, _))) => commands.entity(e).despawn(),
        (None, Err(_)) => {}
    }
    // The release flash on the staff head, once as the release starts.
    let releasing = me.is_some_and(|s| matches!(s.action, Some(StaffAction::ThunderRelease { .. })));
    if releasing && !st.releasing {
        let camera = cam.iter().next();
        for k in table.staff(Thunder, "release", true) {
            play_on_staff_tip(&mut commands, &lib, &fp, camera, &globals, k);
        }
    }
    st.releasing = releasing;
    // Clouds follow the sim's live areas.
    // (owner, point, radius, half height, centre height): the sim's capsule, centred `CapsuleHalfHeight - HeightOffset` above the point.
    let live: Vec<(db_sim::SimId, Vec3, f32, f32, f32)> = areas.as_ref().map(|a| a.0.iter().map(|x| (x.owner, x.at, x.spec.radius, x.spec.half_height, x.spec.half_height - x.spec.height_offset)).collect()).unwrap_or_default();
    // Each live area's attack, kept past its last tick for its strike.
    for x in areas.as_ref().map(|a| a.0.as_slice()).unwrap_or_default() {
        if !known.iter().any(|k| k.0 == x.owner && k.1 == x.at) {
            known.push((x.owner, x.at, x.by));
        }
    }
    let own = |owner: db_sim::SimId, at: Vec3| {
        let &(_, _, by) = known.iter().find(|k| k.0 == owner && k.1 == at)?;
        actor_vfx.of_attack(Some(&lib), &index, &kits, owner, by, &AREA_VIA)
    };
    let real_cloud = has("cloud");
    st.clouds.retain_mut(|(owner, at, fxs)| {
        let keep = live.iter().any(|l| l.0 == *owner && l.1 == *at);
        if !keep {
            stop_all(&mut commands, fxs);
        }
        keep
    });
    for (e, c) in &clouds {
        if !live.iter().any(|l| l.0 == c.0 && l.1 == c.1) {
            commands.entity(e).despawn();
        }
    }
    for (owner, at, r, hh, centre) in &live {
        if let Some(a) = own(*owner, *at) {
            if !st.clouds.iter().any(|c| c.0 == *owner && c.1 == *at) {
                let g = to_bevy(*at);
                let fxs: Vec<Entity> = a.systems.iter().filter(|s| s.auto).filter_map(|s| spawn_actor_system(&mut commands, &lib, s, g, Quat::IDENTITY, None)).collect();
                st.clouds.push((*owner, *at, fxs));
            }
        } else if real_cloud {
            if !st.clouds.iter().any(|c| c.0 == *owner && c.1 == *at) {
                let g = to_bevy(*at);
                let fxs: Vec<Entity> = table
                    .staff(Thunder, "cloud", player.0 == Some(*owner))
                    .filter_map(|k| {
                        let e = spawn_effect(&mut commands, &lib, k, g, Quat::IDENTITY, None)?;
                        commands.entity(e).insert(crate::effects::Beam { end: g + Vec3::Y * CLOUD_BOLT_UP, jitter: CLOUD_BOLT_JITTER });
                        Some(e)
                    })
                    .collect();
                st.clouds.push((*owner, *at, fxs));
            }
        } else if !clouds.iter().any(|(_, c)| c.0 == *owner && c.1 == *at) {
            let t = Transform { translation: to_bevy(*at) + Vec3::Y * centre * 0.01, rotation: Quat::IDENTITY, scale: Vec3::new(r * 0.01, hh * 0.02, r * 0.01) };
            commands.spawn((Mesh3d(fx.unit_cylinder.clone()), MeshMaterial3d(fx.cloud.clone()), t, bevy::light::NotShadowCaster, Cloud(*owner, *at)));
        }
    }
    for e in &events.0 {
        if let SimEvent::ThunderStruck { owner, at, .. } = e {
            let g = to_bevy(*at);
            if let Some(a) = own(*owner, *at) {
                for s in a.systems.iter().filter(|s| !s.auto) {
                    spawn_actor_system(&mut commands, &lib, s, g, Quat::IDENTITY, None);
                }
            } else {
                for k in table.staff(Thunder, "strike", true) {
                    spawn_effect(&mut commands, &lib, k, g, Quat::IDENTITY, None);
                }
            }
            // The stand-in bolt and its flash only without the real effects: the strike's own light is enough, and
            // this 4 million lumen one on top washed the room white.
            if !real_cloud && own(*owner, *at).is_none() {
                commands.spawn((Mesh3d(fx.unit_cylinder.clone()), MeshMaterial3d(fx.bolt.clone()), segment(g + Vec3::Y * BOLT_HEIGHT, g, 0.06), bevy::light::NotShadowCaster, Bolt(0.0)));
                commands.spawn((PointLight { color: Color::srgb(0.6, 0.75, 1.0), intensity: 4.0e6, range: 12.0, shadow_maps_enabled: false, ..default() }, Transform::from_translation(g + Vec3::Y * 2.0), Bolt(0.0)));
            }
        }
    }
    known.retain(|k| live.iter().any(|l| l.0 == k.0 && l.1 == k.1));
    for (e, mut b, mut t) in &mut bolts {
        b.0 += time.delta_secs();
        if b.0 >= BOLT_SECONDS {
            commands.entity(e).despawn();
        } else {
            // A little flicker: the bolt thins as it fades.
            t.scale.x *= 0.92;
            t.scale.z *= 0.92;
        }
    }
}

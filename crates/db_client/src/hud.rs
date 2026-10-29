//! The run HUD from the game's own UI art: the dashboard bar, the health orb (frame, red sphere filled to the health share, glass highlight), the
//! Q and E skill slots (round frames with the skills' icons, shaded with the seconds left while cooling down, charges counted), the potions in
//! the bar's boxes, a sprint stamina bar (ours), the reticle, the interaction prompt's backing and the hit-direction mark. Textures come from the
//! converted library (`UI/Game/...`, `UI/Textures/Icon/...`; imported by `_data/assets/g6c_import.txt`).
//!
//! (playtest): a caster's orb is split, health in the left half (`T_UI_TgfGame_HPBar_Inner_HP_Half`) and its mana
//! or other skill resource in the right (`T_UI_TgfGame_MagicEnergy_Bar`), each filled to its share; a skill it
//! can't pay for shows the game's disabled mask (`T_UI_Skill_Disable_Mask`).
//!
//! CONFIRMED: which icon each skill shows (the game's skill widgets name them; [`icon_of`]). HYPOTHESIS:
//! the layout and sizes (the widgets' layout trees aren't decoded); the potion icons by name.

use bevy::prelude::*;
use db_sim::{Health, SimEvent, SimIndex, SkillKind};

use crate::combat_view::{FrameEvents, LocalPlayer};
use crate::cooldown_sweep::CooldownSweep;
use crate::flow::GameHud;

const UI: &str = "textures/Dungeonborne/Content/TGF/UI/";
/// Inner sphere of the health orb, px.
const ORB: f32 = 144.0;
/// Orb frame size, px (`T_UI_TgfGame_HPBar_BG`).
const ORB_FRAME: (f32, f32) = (199.0, 215.0);
/// Skill frame size, px (`T_UI_Skill_BG`), and the icon inside it.
const SLOT: (f32, f32) = (116.0, 117.0);
const ICON: f32 = 80.0;
/// Skill frames' bottom edge, px: on top of the dashboard bar's boxes.
const SKILL_BOTTOM: f32 = 92.0;
/// Centres of the dashboard bar's first two small boxes right of the orb, px from the screen centre
/// (measured on `T_UI_TgfGame_Dashboard_NewBG` at 1:1).
const POTION_BOX_DX: [f32; 2] = [111.0, 196.0];
/// The flask keys' slots: their kinds (`throwables_view` keys 3 to 5) and keys, in the bar's boxes left of the orb, 3
/// to 6 from the left (ours: the game's quick slots aren't modelled).
const FLASK_SLOTS: [(&str, &str); 4] = [("BurningPotion", "3"), ("ThunderPotion", "4"), ("SmokePotion", "5"), ("EarthPotion", "6")];
const FLASK_BOX_DX: [f32; 4] = [-366.0, -281.0, -196.0, -111.0];
/// Stamina bar width, px (ours).
const STAMINA_W: f32 = 160.0;
/// Hit-direction mark size, px (the texture at 1:1).
const HIT_MARK: f32 = 900.0;
/// Seconds the hit-direction mark takes to fade.
const HIT_MARK_LIFE: f32 = 1.2;
/// The buff row: icons at most, and their size, px.
const BUFF_SLOTS: usize = 8;
const BUFF_ICON: f32 = 44.0;
/// Healing and protection potions (item ids, as the belt keys 1 and 2 use them).
const POTIONS: [(i32, &str, &str); 2] = [(100_013, "1", "Icon_1300001skin_HealingPotion64"), (100_017, "2", "Icon_1300001skin_ShieldPotion64")];

fn tex(assets: &AssetServer, rel: &str) -> Handle<Image> {
    assets.load(format!("{UI}{rel}.png"))
}

/// The suffix of a skill's icon (`D_Icon_Skill_<suffix>`), as the game's skill widget for it names it
/// (`UI/Game/Dashboard/Skill/WBP_Skill_<Class>_<Q|E>[_<variant>]`, CONFIRMED). The icon numbers are not the
/// skill ids: the Fighter's Whirlwind (107001) shows `107002`, Charge (107002) shows `107001`. Skills with no
/// widget of their own: Flying Blades takes the one Rogue icon no widget uses (`103003`), Wraith Ring its
/// own id's first variant (`108001_1`) (both HYPOTHESIS); Soul Storm and Blessing have none (the slot shows
/// the name).
fn icon_of(skill: SkillKind) -> Option<&'static str> {
    use SkillKind as K;
    Some(match skill {
        K::Whirlwind => "107002",
        K::Charge => "107001",
        K::WarCry => "107003",
        K::Inspire => "107004",
        K::Rend => "107006",
        K::FlyingSwords => "101002",
        K::BladeWaltz => "101001",
        K::Oracle => "105001",
        K::HealSingle => "105002",
        K::DivineProtection => "105004",
        K::Guardian => "105003",
        K::PetrifyingPoison => "103002",
        K::FadeIntoVoid => "103001",
        K::FlyingBlades => "103003",
        K::SwiftConcealment => "103004",
        K::ShadowVeil => "103005",
        K::Pyroblast => "109001",
        K::FlameShock => "109002",
        K::WraithRing => "108001_1",
        K::SoulChain => "108002",
        K::Blizzard => "110001",
        K::FrostTorment => "110003",
        K::IceBarrier => "110002",
        K::IceArmor => "110004",
        K::WildAwakening => "106002",
        K::ForceOfNature => "106003",
        K::ShadowPounce => "106004",
        K::Rejuvenation => "106006",
        K::SoulStorm | K::Blessing | K::MonsterAbility | K::Item => return None,
    })
}

fn icon_path(suffix: &str) -> String {
    format!("{UI}Textures/Icon/SkillIcon/D_Icon_Skill_{suffix}.png")
}

/// The imported icon of a skill, if there is one.
fn skill_icon(skill: SkillKind) -> Option<String> {
    icon_of(skill).map(icon_path).filter(|p| crate::model_view::imported(p))
}

#[derive(Component)]
pub(crate) struct OrbFill;
/// A caster's half orbs: 0 health (left), 1 its skill resource (right).
#[derive(Component)]
pub(crate) struct OrbHalf(usize);
#[derive(Component)]
pub(crate) struct PowerText;
/// The not-enough-mana mask over skill slot 0 or 1.
#[derive(Component)]
pub(crate) struct SlotDisabled(usize);
#[derive(Component)]
pub(crate) struct OrbText;
/// Parts of skill slot 0 (Q) or 1 (E).
#[derive(Component)]
pub(crate) struct SlotIcon(usize);
#[derive(Component)]
pub(crate) struct SlotShade(usize);
#[derive(Component)]
pub(crate) struct SlotText(usize);
#[derive(Component)]
pub(crate) struct SlotName(usize);
#[derive(Component)]
pub(crate) struct PotionCount(i32);
/// A flask slot's icon, count and cooldown (its place in [`FLASK_SLOTS`]).
#[derive(Component)]
pub(crate) struct FlaskIcon(usize);
#[derive(Component)]
pub(crate) struct FlaskCount(usize);
#[derive(Component)]
pub(crate) struct FlaskWait;
#[derive(Component)]
pub(crate) struct PromptBack;
#[derive(Component)]
pub(crate) struct StaminaBar;
#[derive(Component)]
pub(crate) struct StaminaFill;
/// One of the buff row's icons (its place) and its seconds.
#[derive(Component)]
pub(crate) struct BuffIcon(usize);
#[derive(Component)]
pub(crate) struct BuffTime(usize);
#[derive(Component)]
pub(crate) struct HitMark {
    age: f32,
}

fn abs(left: f32, bottom: f32, w: f32, h: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: px(left), bottom: px(bottom), width: px(w), height: px(h), ..default() }
}

/// Builds the HUD (hidden outside runs and the test room with the other `GameHud` nodes).
pub(crate) fn setup_art_hud(mut commands: Commands, assets: Res<AssetServer>, mut sweeps: ResMut<Assets<CooldownSweep>>) {
    let centred = |w: f32, bottom: f32, h: f32, dx: f32| Node { position_type: PositionType::Absolute, left: percent(50.0), bottom: px(bottom), width: px(w), height: px(h), margin: UiRect::left(px(dx)), ..default() };
    let text = |s: &str, size: f32| (Text::new(s), crate::fonts::sans(size), TextColor(Color::srgb(0.95, 0.92, 0.85)), TextShadow::default());
    commands.spawn((ImageNode::new(tex(&assets, "Game/HUD/Dashboard/Texture/T_UI_TgfGame_Dashboard_NewBG")), centred(916.0, 0.0, 166.0, -458.0), GameHud));
    // Health orb.
    let (fw, fh) = ORB_FRAME;
    let (ix, iy) = ((fw - ORB) / 2.0, (fh - ORB) / 2.0);
    // The frame is an opaque stone disc: the red fill and the glass go on top of it.
    commands.spawn((centred(fw, 10.0, fh, -fw / 2.0), GlobalZIndex(1), GameHud)).with_children(|o| {
        o.spawn((ImageNode::new(tex(&assets, "Game/HUD/Dashboard/Texture/T_UI_TgfGame_HPBar_BG")), abs(0.0, 0.0, fw, fh)));
        o.spawn((Node { overflow: Overflow::clip(), ..abs(ix, iy, ORB, ORB) }, OrbFill)).with_children(|f| {
            f.spawn((ImageNode::new(tex(&assets, "Game/HUD/Dashboard/Texture/T_UI_TgfGame_HPBar_Inner_Full")), abs(0.0, 0.0, ORB, ORB)));
        });
        for (i, half) in [(0, "T_UI_TgfGame_HPBar_Inner_HP_Half"), (1, "T_UI_TgfGame_MagicEnergy_Bar")] {
            let left = ix + ORB / 2.0 * i as f32;
            o.spawn((Node { overflow: Overflow::clip(), ..abs(left, iy, ORB / 2.0, ORB) }, Visibility::Hidden, OrbHalf(i))).with_children(|f| {
                f.spawn((ImageNode::new(tex(&assets, &format!("Game/HUD/Dashboard/Texture/{half}"))), abs(0.0, 0.0, ORB / 2.0, ORB)));
            });
        }
        o.spawn((ImageNode::new(tex(&assets, "Game/HUD/Dashboard/Texture/T_UI_TgfGame_HPBar_HL")), abs(ix - 4.0, iy - 4.0, ORB + 8.0, ORB + 8.0)));
        o.spawn((text("", 20.0), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, width: percent(100.0), top: percent(44.0), ..default() }, OrbText));
        o.spawn((Text::new(""), crate::fonts::sans(15.0), TextColor(Color::srgb(0.62, 0.8, 1.0)), TextShadow::default(), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, width: percent(100.0), top: percent(57.0), ..default() }, PowerText));
    });
    // Skill slots either side of the orb, standing on the bar.
    let (sw, sh) = SLOT;
    for (i, dx, key) in [(0, -fw / 2.0 - sw + 6.0, "Q"), (1, fw / 2.0 - 6.0, "E")] {
        commands.spawn((centred(sw, SKILL_BOTTOM, sh, dx), GlobalZIndex(1), GameHud)).with_children(|s| {
            let (ox, oy) = ((sw - ICON) / 2.0, (sh - ICON) / 2.0);
            let round = |n: Node| Node { border_radius: BorderRadius::MAX, overflow: Overflow::clip(), ..n };
            // The slot's backplate is opaque: the icon and the cooldown shade go on top of it.
            s.spawn((ImageNode::new(tex(&assets, "Game/HUD/Dashboard/Texture/T_UI_Skill_BG")), abs(0.0, 0.0, sw, sh)));
            s.spawn((ImageNode::default(), round(abs(ox, oy, ICON, ICON)), Visibility::Hidden, SlotIcon(i)));
            s.spawn((text("", 13.0), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, left: px(ox), width: px(ICON), top: px(oy + ICON * 0.38), ..default() }, SlotName(i)));
            s.spawn((ImageNode::new(tex(&assets, "Game/HUD/Dashboard/Texture/T_UI_Skill_Disable_Mask")), round(abs(ox, oy, ICON, ICON)), Visibility::Hidden, SlotDisabled(i)));
            s.spawn((MaterialNode(sweeps.add(CooldownSweep::new())), abs(ox, oy, ICON, ICON), Visibility::Hidden, SlotShade(i)));
            s.spawn((text("", 26.0), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, width: percent(100.0), top: px(sh * 0.34), ..default() }, SlotText(i)));
            s.spawn((text(key, 18.0), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, width: percent(100.0), bottom: px(-4.0), ..default() }));
        });
    }
    // Potions in the bar's first two small boxes right of the orb.
    for (n, (item, key, icon)) in POTIONS.iter().enumerate() {
        let (dx, size) = (POTION_BOX_DX[n] - 28.0, 56.0);
        commands.spawn((centred(size, 42.0, size, dx), GlobalZIndex(1), GameHud)).with_children(|p| {
            p.spawn((ImageNode::new(tex(&assets, &format!("Textures/Icon/EquipIcon/{icon}"))), abs(0.0, 0.0, size, size)));
            p.spawn((text("0", 16.0), Node { position_type: PositionType::Absolute, right: px(2.0), bottom: px(0.0), ..default() }, PotionCount(*item)));
            p.spawn((text(key, 14.0), Node { position_type: PositionType::Absolute, left: px(3.0), top: px(0.0), ..default() }));
        });
    }
    // The flasks in the boxes left of the orb (their icons once the flasks are read).
    for (n, (_, key)) in FLASK_SLOTS.iter().enumerate() {
        let (dx, size) = (FLASK_BOX_DX[n] - 28.0, 56.0);
        commands.spawn((centred(size, 42.0, size, dx), GlobalZIndex(1), GameHud)).with_children(|p| {
            p.spawn((ImageNode::default(), abs(0.0, 0.0, size, size), Visibility::Hidden, FlaskIcon(n)));
            p.spawn((text("", 22.0), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, width: percent(100.0), top: px(size * 0.28), ..default() }, FlaskWait));
            p.spawn((text("0", 16.0), Node { position_type: PositionType::Absolute, right: px(2.0), bottom: px(0.0), ..default() }, FlaskCount(n)));
            p.spawn((text(key, 14.0), Node { position_type: PositionType::Absolute, left: px(3.0), top: px(0.0), ..default() }));
        });
    }
    // Nodes that show and hide themselves sit under one full-screen `GameHud` root: `flow::hud_visibility`
    // sets every `GameHud` node's visibility each frame, so they can't carry the marker themselves.
    let root = commands.spawn((Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() }, GameHud)).id();
    // The interaction prompt's backing (the prompt text is `items_view`'s, at 56 % height).
    commands.spawn((
        ChildOf(root),
        ImageNode::new(tex(&assets, "Game/InteractionPrompt/T_UI_TgfGame_InteractionPrompt_Bg")),
        Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(56.0), width: px(404.0), height: px(43.0), margin: UiRect { left: px(-202.0), top: px(-9.0), ..default() }, ..default() },
        Visibility::Hidden,
        PromptBack,
    ));
    // Sprint stamina over the orb, only while not full (ours: the game has no stamina bar art we know of).
    commands
        .spawn((centred(STAMINA_W + 4.0, 10.0 + fh + 6.0, 10.0, -(STAMINA_W + 4.0) / 2.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), Visibility::Hidden, StaminaBar, ChildOf(root)))
        .with_children(|b| {
            b.spawn((Node { position_type: PositionType::Absolute, left: px(2.0), top: px(2.0), width: px(STAMINA_W), height: px(6.0), ..default() }, BackgroundColor(Color::srgb(0.85, 0.72, 0.35)), StaminaFill));
        });
    // The player's buffs above the skill slots (the game's `D_Icon_Buff_*`; the row's place is ours).
    let row_w = BUFF_SLOTS as f32 * (BUFF_ICON + 4.0);
    commands.spawn((centred(row_w, SKILL_BOTTOM + sh + 10.0, BUFF_ICON + 16.0, -row_w / 2.0), ChildOf(root))).with_children(|r| {
        for i in 0..BUFF_SLOTS {
            let left = i as f32 * (BUFF_ICON + 4.0);
            // (On a dark disc: the icons are mostly transparent and vanish against a lit floor.)
            r.spawn((
                ImageNode::default(),
                Node { position_type: PositionType::Absolute, left: px(left), top: px(0.0), width: px(BUFF_ICON), height: px(BUFF_ICON), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                Visibility::Hidden,
                BuffIcon(i),
            ));
            r.spawn((text("", 12.0), TextLayout::justify(Justify::Center), Node { position_type: PositionType::Absolute, left: px(left), top: px(BUFF_ICON), width: px(BUFF_ICON), ..default() }, BuffTime(i)));
        }
    });
    // Hit direction: the mark at the texture's top points at the attacker.
    commands.spawn((
        ChildOf(root),
        ImageNode::new(tex(&assets, "Game/HUD/HitDirection/T_UI_Hud_HitDirection")),
        Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(50.0), width: px(HIT_MARK), height: px(HIT_MARK), margin: UiRect { left: px(-HIT_MARK / 2.0), top: px(-HIT_MARK / 2.0), ..default() }, ..default() },
        UiTransform::default(),
        Visibility::Hidden,
        HitMark { age: HIT_MARK_LIFE },
    ));
}

/// Health orb, skill slots and potion counts from the local player.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn update_art_hud(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    assets: Res<AssetServer>,
    actors: Query<(&Health, Option<&db_sim::SkillKit>, Option<&db_sim::ClassKit>, Option<&db_sim::Inventory>, Option<&db_sim::Power>)>,
    mut fill: Query<(&mut Node, &mut Visibility), (With<OrbFill>, Without<OrbHalf>)>,
    mut halves: Query<(&mut Node, &mut Visibility, &OrbHalf), Without<OrbFill>>,
    mut texts: ParamSet<(Query<&mut Text, With<OrbText>>, Query<(&mut Text, &SlotText)>, Query<(&mut Text, &SlotName)>, Query<(&mut Text, &PotionCount)>, Query<&mut Text, With<PowerText>>)>,
    mut icons: Query<(&mut ImageNode, &mut Visibility, &SlotIcon), (Without<SlotShade>, Without<SlotDisabled>, Without<OrbFill>, Without<OrbHalf>)>,
    (mut shades, mut sweeps, mut full): (Query<(&mut Visibility, &MaterialNode<CooldownSweep>, &SlotShade), (Without<SlotIcon>, Without<SlotDisabled>, Without<OrbFill>, Without<OrbHalf>)>, ResMut<Assets<CooldownSweep>>, Local<[u32; 2]>),
    mut disabled: Query<(&mut Visibility, &SlotDisabled), (Without<SlotIcon>, Without<SlotShade>, Without<OrbFill>, Without<OrbHalf>)>,
    mut shown: Local<[Option<SkillKind>; 2]>,
) {
    let Some((health, kit, class_kit, inv, power)) = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()) else { return };
    let share = (health.current / health.max.max(1.0)).clamp(0.0, 1.0) as f32;
    let power_share = power.map(|p| (p.current / p.max.max(1.0)).clamp(0.0, 1.0) as f32);
    if let Ok((mut n, mut vis)) = fill.single_mut() {
        n.height = px(ORB * share);
        *vis = if power_share.is_some() { Visibility::Hidden } else { Visibility::Inherited };
    }
    for (mut n, mut vis, half) in &mut halves {
        let s = if half.0 == 0 { Some(share) } else { power_share };
        *vis = if power_share.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        n.height = px(ORB * s.unwrap_or(0.0));
    }
    let set = |t: &mut Text, s: String| {
        if t.0 != s {
            t.0 = s;
        }
    };
    if let Ok(mut t) = texts.p0().single_mut() {
        set(&mut t, format!("{:.0}", health.current.max(0.0).ceil()));
    }
    if let Ok(mut t) = texts.p4().single_mut() {
        set(&mut t, power.map_or(String::new(), |p| format!("{:.0}", p.current.max(0.0).floor())));
    }
    for (mut vis, d) in &mut disabled {
        let short = class_kit.zip(power).is_some_and(|(k, p)| p.current < k.press_cost(k.slots[d.0]));
        *vis = if short { Visibility::Inherited } else { Visibility::Hidden };
    }
    // Each slot: skill, cooldown ticks, charges (n, max).
    let slots: [Option<(SkillKind, u32, Option<(u8, u8)>)>; 2] = match (class_kit, kit) {
        (Some(k), _) => k.slots.map(|s| Some((s, k.cooldown(s), k.charges_of(s).map(|c| (c.0, c.1))))),
        (None, Some(k)) => k.slots.map(|s| Some((s, k.cooldown(s), k.charges_of(s)))),
        _ => [None, None],
    };
    for (mut img, mut vis, slot) in &mut icons {
        let skill = slots[slot.0].map(|s| s.0);
        if shown[slot.0] != skill {
            shown[slot.0] = skill;
            match skill.and_then(skill_icon) {
                Some(p) => {
                    img.image = assets.load(p);
                    *vis = Visibility::Inherited;
                }
                None => *vis = Visibility::Hidden,
            }
        }
    }
    for (mut t, n) in &mut texts.p2() {
        let s = slots[n.0].filter(|s| skill_icon(s.0).is_none()).map_or(String::new(), |s| s.0.name().to_string());
        set(&mut t, s);
    }
    for (mut vis, sweep, s) in &mut shades {
        let cd = slots[s.0].filter(|(_, _, ch)| ch.is_none_or(|c| c.0 == 0)).map_or(0, |x| x.1);
        // The cooldown's length: the most it has been since it started.
        full[s.0] = if cd == 0 { 0 } else { full[s.0].max(cd) };
        *vis = if cd > 0 { Visibility::Inherited } else { Visibility::Hidden };
        let left = if cd > 0 { cd as f32 / full[s.0].max(1) as f32 } else { 0.0 };
        if let Some(mut m) = sweeps.get_mut(&sweep.0)
            && (m.sweep.left.x - left).abs() > 1e-3
        {
            m.sweep.left.x = left;
        }
    }
    for (mut t, s) in &mut texts.p1() {
        let v = match slots[s.0] {
            Some((_, _, Some((n, max)))) if n < max => format!("{n}/{max}"),
            Some((_, cd, _)) if cd > 0 => format!("{:.0}", (cd as f32 * db_sim::DT).ceil()),
            _ => String::new(),
        };
        set(&mut t, v);
    }
    let count = |id: i32| inv.map_or(0, |i| i.bag.iter().filter(|b| b.item.item.item_id == id).map(|b| u32::from(b.item.count)).sum::<u32>());
    for (mut t, p) in &mut texts.p3() {
        set(&mut t, count(p.0).to_string());
    }
}

/// The flask slots: each kind's icon, how many the backpack holds (both versions), and the shared cooldown's seconds
/// over a dimmed icon (dimmed too with none left).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn flask_belt(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    (flasks, data, tick): (Option<Res<db_sim::throwables::FlaskDb>>, Option<Res<db_sim::GameData>>, Option<Res<db_sim::TickCount>>),
    assets: Res<AssetServer>,
    actors: Query<(Option<&db_sim::Inventory>, Option<&db_sim::throwables::FlaskCooldown>)>,
    mut icons: Query<(&mut ImageNode, &mut Visibility, &FlaskIcon)>,
    mut counts: Query<(&mut Text, &FlaskCount), Without<FlaskWait>>,
    mut waits: Query<&mut Text, (With<FlaskWait>, Without<FlaskCount>)>,
) {
    let (Some(flasks), Some(data), Some(tick)) = (flasks, data, tick) else { return };
    let (inv, cooldown) = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()).unwrap_or((None, None));
    let held = |n: usize| inv.map_or(0, |i| i.bag.iter().filter(|b| flasks.0.get(&b.item.item.item_id).is_some_and(|f| f.kind() == FLASK_SLOTS[n].0)).map(|b| u32::from(b.item.count)).sum::<u32>());
    let wait = cooldown.map_or(0.0, |c| c.0.saturating_sub(tick.0) as f32 * db_sim::DT);
    for (mut img, mut vis, f) in &mut icons {
        if *vis == Visibility::Hidden
            && let Some(path) = flasks.0.iter().find(|(_, s)| s.kind() == FLASK_SLOTS[f.0].0 && s.name.ends_with("_0101")).and_then(|(id, _)| crate::items_view::icon_path(&data, *id))
        {
            img.image = assets.load(path);
            *vis = Visibility::Inherited;
        }
        let dim = wait > 0.0 || held(f.0) == 0;
        let c = if dim { Color::srgb(0.35, 0.35, 0.35) } else { Color::WHITE };
        if img.color != c {
            img.color = c;
        }
    }
    for (mut t, f) in &mut counts {
        let v = held(f.0).to_string();
        if t.0 != v {
            t.0 = v;
        }
    }
    for mut t in &mut waits {
        let v = if wait > 0.0 { format!("{:.0}", wait.ceil()) } else { String::new() };
        if t.0 != v {
            t.0 = v;
        }
    }
}

/// The sprint stamina bar: shown while stamina isn't full, filled to its share.
pub(crate) fn stamina_bar(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    states: Query<&db_sim::CharacterState>,
    mut bar: Query<&mut Visibility, With<StaminaBar>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<StaminaFill>>,
) {
    let Some(st) = player.0.and_then(|id| index.get(id)).and_then(|e| states.get(e).ok()) else { return };
    let share = (st.stamina / db_sim::overrides::movement::STAMINA_MAX).clamp(0.0, 1.0);
    if let Ok(mut vis) = bar.single_mut() {
        vis.set_if_neq(if share < 1.0 { Visibility::Inherited } else { Visibility::Hidden });
    }
    if let Ok((mut n, mut c)) = fill.single_mut() {
        n.width = px(STAMINA_W * share);
        // Grey while exhausted (sprint locked until it refills enough).
        c.0 = if st.exhausted { Color::srgb(0.45, 0.42, 0.38) } else { Color::srgb(0.85, 0.72, 0.35) };
    }
}

/// The prompt's backing shows while there is a prompt.
pub(crate) fn prompt_back(prompt: Query<&Text, With<crate::items_view::InteractPrompt>>, mut back: Query<&mut Visibility, With<PromptBack>>) {
    let on = prompt.iter().any(|t| !t.0.is_empty());
    for mut v in &mut back {
        *v = if on { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// When the local player is hit, the mark turns towards the attacker and fades.
pub(crate) fn hit_marks(
    time: Res<Time>,
    events: Res<FrameEvents>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    look: Res<crate::Look>,
    positions: Query<&db_sim::Position>,
    mut marks: Query<(&mut HitMark, &mut UiTransform, &mut ImageNode, &mut Visibility)>,
) {
    let Ok((mut mark, mut t, mut img, mut vis)) = marks.single_mut() else { return };
    let pos = |id| index.get(id).and_then(|e| positions.get(e).ok()).map(|p| p.0);
    for e in &events.0 {
        if let SimEvent::Damaged { source, target, .. } = e
            && Some(*target) == player.0
            && source != target
            && let (Some(me), Some(from)) = (pos(*target), pos(*source))
        {
            // Sim yaw turns clockwise seen from above (left-handed, +Y right), as UI rotation does.
            let d = (from - me).truncate();
            t.rotation = Rot2::radians(d.y.atan2(d.x) - look.yaw);
            mark.age = 0.0;
        }
    }
    mark.age += time.delta_secs();
    let a = (1.0 - mark.age / HIT_MARK_LIFE).clamp(0.0, 1.0);
    img.color = Color::srgba(1.0, 1.0, 1.0, a);
    *vis = if a > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
}

/// The icon a buff shows (`UI/Textures/Icon/BuffIcon/D_Icon_Buff_<name>`), if any: the nearest the game has for
/// each of the sim's kinds (pulls, stealth, roots and gameplay tags show none).
fn buff_icon(kind: &db_sim::BuffKind) -> Option<&'static str> {
    use db_sim::BuffKind as K;
    Some(match kind {
        K::MoveSpeed(v) if *v > 0.0 => "SpdUp",
        K::Slow(_) | K::MoveSpeed(_) => "SpdDown",
        K::Rent { .. } => "Bleed",
        K::GrievousWounds(_) => "HealReceiveDown",
        K::Petrified { .. } => "Petrochemical",
        K::DamageReduction(_) => "DefUp",
        K::FrostCurse { .. } => "Lich_Curse",
        K::IceBarrier { .. } => "Lich_Fridge",
        K::IceArmor { .. } | K::IceEnchant { .. } => "Enchant_Ice",
        K::Invincible => "Invincible",
        K::Regen(_) => "HealOnTime",
        K::Burning { .. } => "Burning",
        _ => return None,
    })
}

/// The local player's buffs in the row: one icon per kind (the longest left), with its seconds; the Wizard's
/// air jump (a full Pyroblast charge) as the game's `D_Icon_Buff_Floating`.
pub(crate) fn buff_row(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    assets: Res<AssetServer>,
    actors: Query<(Option<&db_sim::Buffs>, Option<&db_sim::ClassKit>)>,
    mut icons: Query<(&BuffIcon, &mut ImageNode, &mut Visibility)>,
    mut times: Query<(&BuffTime, &mut Text)>,
) {
    let (buffs, kit) = player.0.and_then(|id| index.get(id)).and_then(|e| actors.get(e).ok()).unwrap_or((None, None));
    let mut shown: Vec<(&'static str, Option<f32>)> = Vec::new();
    if kit.is_some_and(|k| k.floating) {
        shown.push(("Floating", None));
    }
    for b in buffs.map_or(&[][..], |b| &b.0[..]) {
        let Some(name) = buff_icon(&b.kind) else { continue };
        let secs = (b.ticks_left < 1_000_000).then_some(b.ticks_left as f32 * db_sim::DT);
        match shown.iter_mut().find(|s| s.0 == name) {
            Some(s) => s.1 = s.1.zip(secs).map(|(a, b)| a.max(b)),
            None => shown.push((name, secs)),
        }
    }
    for (i, mut image, mut vis) in &mut icons {
        match shown.get(i.0) {
            Some((name, _)) => {
                image.image = assets.load(format!("{UI}Textures/Icon/BuffIcon/D_Icon_Buff_{name}.png"));
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for (t, mut text) in &mut times {
        let s = shown.get(t.0).and_then(|s| s.1).map_or(String::new(), |s| format!("{}", s.ceil() as u32));
        if text.0 != s {
            text.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skill_icon_is_its_own_and_imported() {
        let mut seen = std::collections::HashSet::new();
        for s in SkillKind::ALL.iter() {
            let Some(i) = icon_of(*s) else { continue };
            assert!(seen.insert(i), "{s:?} shares {i}");
            assert!(crate::model_view::imported(&icon_path(i)), "{s:?}: {i} not imported");
        }
        assert!(seen.len() >= 28, "{}", seen.len());
    }
}

//! Menus: title with class pick, the stash between runs, the run result. Buttons and the
//! `--auto-run` bot both send `flow::Action`s.
//!
//! Art: the game's login background and logo on the title, its class crests (the class table's
//! `icon_career`) and portraits (`RoleIcon_<type id>`) on the class pick, the character-creation
//! background behind the stash, the post-game background and banner on the result, its login button for
//! every button. Layout is ours (HYPOTHESIS: the menu widgets' layouts aren't decoded).
//!
//! Stash controls: left click moves an item between backpack and stash, right click wears a backpack
//! item or takes off a worn one, shift + left click sells to the merchant.

use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::text::FontSize;
use db_formats::config::generated::{EnItemWearingPart as Slot, EnRoleCareerType};
use db_sim::attributes::{CLASSES, STARTING_SLOTS as WORN_SLOTS};
use db_sim::{GameData, Inventory, ItemInstance, RunOutcome, SkillKind};

use crate::flow::{Action, LastRun, PendingAction, ProfileRes, SavePath, Screen};
use crate::inventory_ui::{slot_label, spawn_item_node, wear_slot};
use crate::items_view::item_name;

const CELL: f32 = 44.0;
const BG: Color = Color::srgb(0.05, 0.045, 0.04);
const PANEL: Color = Color::srgba(0.07, 0.06, 0.05, 0.9);
const CELL_BG: Color = Color::srgb(0.13, 0.11, 0.1);
const CELL_LINE: Color = Color::srgb(0.25, 0.22, 0.19);
const TEXT: Color = Color::srgb(0.92, 0.89, 0.82);
const DIM: Color = Color::srgb(0.68, 0.64, 0.57);
pub(crate) const GOLD: Color = Color::srgb(0.95, 0.8, 0.4);
const CARD_LINE: Color = Color::srgb(0.35, 0.28, 0.18);
const CARD_HOVER: Color = Color::srgb(0.95, 0.75, 0.4);
/// Button tints: resting, hovered, pressed (multiplying the button art).
const BUTTON_TINT: [Color; 3] = [Color::srgb(0.82, 0.8, 0.78), Color::WHITE, Color::srgb(0.6, 0.58, 0.55)];

const UI: &str = "textures/Dungeonborne/Content/TGF/UI/";

pub(crate) fn ui(assets: &AssetServer, rel: &str) -> Handle<Image> {
    assets.load(format!("{UI}{rel}.png"))
}

/// A class's crest (the class table's `icon_career`, CONFIRMED) and portrait (`RoleIcon_<type id>`, by
/// name), as UI texture paths.
fn class_art(data: &GameData, c: EnRoleCareerType) -> Option<(String, String)> {
    let id = db_sim::attributes::class_type_id(data.tables, c).ok()?;
    let row = data.tables.excel_role_type.iter().find(|r| r.type_id == id)?;
    Some((format!("Textures/Icon/CareerIcon/{}", row.icon_career), format!("Textures/Icon/CareerIcon/RoleIcon_{id}")))
}

/// Everything of the current menu (removed on leaving the screen).
#[derive(Component)]
pub(crate) struct MenuScoped;

#[derive(Component)]
pub(crate) struct MenuButton(Action);

/// The class pick panel on the title screen (hidden until Play on a new profile).
#[derive(Component)]
pub(crate) struct ClassPick;

/// A class card on the class pick (its border lights up on hover).
#[derive(Component)]
pub(crate) struct ClassCard;

/// The class crest beside the stash header.
#[derive(Component)]
pub(crate) struct StashCrest;

/// Stash screen parts that redraw when the profile changes.
#[derive(Component)]
pub(crate) struct StashGrid(Place);

#[derive(Component)]
pub(crate) struct StashSlot(Slot);

#[derive(Component)]
pub(crate) struct StashHeader;

#[derive(Component)]
pub(crate) struct StashTooltip;
/// The merchant panel's list, and one of its offers.
#[derive(Component)]
pub(crate) struct MerchantList;
#[derive(Component)]
pub(crate) struct MerchantRow(db_sim::merchant::Offer);

/// The merchant row's icon size, px.
const OFFER_ICON: f32 = 34.0;

/// The merchant's goods this refresh: a new pick of the random tabs after every run (HYPOTHESIS).
fn merchant_offers(data: &GameData, profile: &db_sim::profile::Profile) -> Vec<db_sim::merchant::Offer> {
    db_sim::merchant::offers(data.tables, u64::from(profile.runs))
}

#[derive(Component)]
pub(crate) struct StashItem {
    uid: u32,
    worn: Option<Slot>,
    place: Place,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Backpack,
    Stash,
}

/// English class names (the string tables are Chinese; ours).
pub fn class_name(c: EnRoleCareerType) -> String {
    match c {
        EnRoleCareerType::Fighter => "Fighter",
        EnRoleCareerType::Swordmaster => "Swordmaster",
        EnRoleCareerType::Priest => "Priest",
        EnRoleCareerType::Rogue => "Rogue",
        EnRoleCareerType::Wizard => "Wizard",
        EnRoleCareerType::Deathknight => "Death Knight",
        EnRoleCareerType::Lich => "Lich",
        EnRoleCareerType::DruidPanther => "Druid",
        _ => "Unknown class",
    }
    .to_string()
}

/// Menu text in the default face (Libre Baskerville); headings (40 px and up) in its Bold face.
pub(crate) fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let font = if size >= 40.0 { crate::fonts::heading(size) } else { TextFont { font_size: FontSize::Px(size), ..default() } };
    (Text::new(s), font, TextColor(color))
}

/// A button on the game's login-button art (`T_Login_LoginButton`, 370 × 74), sliced so its angled ends
/// keep their shape at any width.
pub(crate) fn button(parent: &mut ChildSpawnerCommands, assets: &AssetServer, label: impl Into<String>, action: Action, width: f32) {
    let slicer = TextureSlicer { border: BorderRect::axes(36.0, 22.0), center_scale_mode: SliceScaleMode::Stretch, sides_scale_mode: SliceScaleMode::Stretch, max_corner_scale: 1.0 };
    parent
        .spawn((
            Button,
            Node { width: px(width), height: px(52.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            ImageNode { image: ui(assets, "Textures/Login/T_Login_LoginButton"), image_mode: NodeImageMode::Sliced(slicer), color: BUTTON_TINT[0], ..default() },
            MenuButton(action),
        ))
        .with_children(|b| {
            b.spawn(text(label, 22.0, TEXT));
        });
}

/// A full-screen menu over `bg` (the game's art, stretched; it is 16:9 like the window).
fn root<'a>(commands: &'a mut Commands<'_, '_>, bg: Handle<Image>) -> EntityCommands<'a> {
    commands.spawn((
        Node { width: percent(100.0), height: percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: px(16.0), ..default() },
        BackgroundColor(BG),
        ImageNode { image: bg, image_mode: NodeImageMode::Stretch, ..default() },
        GlobalZIndex(20),
        MenuScoped,
    ))
}

/// Buttons light up on hover and dim when pressed; class cards light their border.
#[allow(clippy::type_complexity)]
pub(crate) fn button_looks(mut buttons: Query<(&Interaction, &mut ImageNode), (Changed<Interaction>, With<MenuButton>)>, mut cards: Query<(&Interaction, &mut BorderColor), (Changed<Interaction>, With<ClassCard>)>) {
    for (i, mut img) in &mut buttons {
        img.color = BUTTON_TINT[match i {
            Interaction::None => 0,
            Interaction::Hovered => 1,
            Interaction::Pressed => 2,
        }];
    }
    for (i, mut b) in &mut cards {
        *b = BorderColor::all(if *i == Interaction::None { CARD_LINE } else { CARD_HOVER });
    }
}

/// A class card: portrait, crest over its foot, name.
fn class_card(parent: &mut ChildSpawnerCommands, assets: &AssetServer, data: Option<&GameData>, c: EnRoleCareerType) {
    let art = data.and_then(|d| class_art(d, c));
    parent
        .spawn((
            Button,
            Node { width: px(158.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::all(px(8.0)), border: UiRect::all(px(2.0)), row_gap: px(6.0), ..default() },
            BackgroundColor(Color::srgba(0.04, 0.035, 0.03, 0.88)),
            BorderColor::all(CARD_LINE),
            ClassCard,
            MenuButton(Action::PickClass(c)),
        ))
        .with_children(|k| {
            k.spawn(Node { width: px(125.0), height: px(266.0), ..default() }).with_children(|p| {
                if let Some((crest, portrait)) = &art {
                    p.spawn((ImageNode::new(ui(assets, portrait)), Node { width: percent(100.0), height: percent(100.0), ..default() }));
                    p.spawn((ImageNode::new(ui(assets, crest)), Node { position_type: PositionType::Absolute, width: px(76.0), height: px(76.0), left: px(24.5), bottom: px(-34.0), ..default() }));
                }
            });
            k.spawn(Node { height: px(30.0), ..default() });
            k.spawn(text(class_name(c), 20.0, TEXT));
        });
}

pub(crate) fn despawn_menu(mut commands: Commands, menu: Query<Entity, With<MenuScoped>>) {
    for e in &menu {
        commands.entity(e).despawn();
    }
}

pub(crate) fn spawn_title(mut commands: Commands, assets: Res<AssetServer>, data: Option<Res<GameData>>) {
    root(&mut commands, ui(&assets, "Textures/Login/T_Login_Background")).with_children(|r| {
        // The logo (`T_Login_Background_05`, 1448 × 203; `T_Login_Logo` is the "Project Crawl" codename).
        r.spawn((ImageNode::new(ui(&assets, "Textures/Login/T_Login_Background_05")), Node { width: px(905.0), height: px(127.0), ..default() }));
        r.spawn(text("offline single-player remake", 22.0, DIM));
        r.spawn(Node { height: px(14.0), ..default() });
        button(r, &assets, "Play", Action::Play, 300.0);
        button(r, &assets, "Test Arena", Action::Arena, 300.0);
        button(r, &assets, "Settings", Action::Settings, 300.0);
        button(r, &assets, "Quit", Action::Quit, 300.0);
        r.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(12.0), margin: UiRect::top(px(16.0)), ..default() }, Visibility::Hidden, ClassPick)).with_children(|p| {
            p.spawn(text("Choose your class", 28.0, TEXT));
            p.spawn(Node { justify_content: JustifyContent::Center, column_gap: px(12.0), ..default() }).with_children(|w| {
                for c in CLASSES {
                    // Every class has its skills.
                    class_card(w, &assets, data.as_deref(), c);
                }
            });
        });
    });
}

pub(crate) fn spawn_result(mut commands: Commands, assets: Res<AssetServer>, last: Res<LastRun>, profile: Res<ProfileRes>) {
    let (title, color, line) = match last.outcome {
        Some(RunOutcome::Extracted) => (
            "EXTRACTED",
            Color::srgb(0.4, 1.0, 0.5),
            format!("You got out with {} items{}, {}:{:02} before the crypt closed.", last.items, if last.gold > 0 { format!(" and {} gold", last.gold) } else { String::new() }, last.seconds / 60, last.seconds % 60),
        ),
        Some(RunOutcome::TimedOut) => ("TIME RAN OUT", Color::srgb(0.9, 0.3, 0.25), "The crypt claimed you. Everything you carried is lost.".to_string()),
        _ => ("YOU DIED", Color::srgb(0.9, 0.15, 0.1), format!("You fell on floor {}. Everything you carried is lost.", last.floor + 1)),
    };
    let p = &profile.profile;
    root(&mut commands, ui(&assets, "Game/PostGame/T_PostGame_Extraction_BG")).with_children(|r| {
        // The post-game banner (`T_PostGame_Extraction_Mid1`, 1529 × 206) behind the outcome.
        r.spawn((ImageNode::new(ui(&assets, "Game/PostGame/T_PostGame_Extraction_Mid1")), Node { width: px(1146.0), height: px(154.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })).with_children(|b| {
            b.spawn(text(title, 64.0, color));
        });
        r.spawn(text(line, 24.0, TEXT));
        let up = if last.levels > 0 { format!("  -  level up! now level {}", p.level) } else { String::new() };
        r.spawn(text(format!("+{} experience{up}", last.exp), 22.0, Color::srgb(0.95, 0.85, 0.45)));
        r.spawn(text(format!("Level {} ({} exp)   |   gold {}   |   runs {}, extractions {}, deaths {}", p.level, p.exp, p.gold, p.runs, p.extractions, p.deaths), 20.0, DIM));
        r.spawn(Node { height: px(20.0), ..default() });
        button(r, &assets, "Continue", Action::Continue, 300.0);
    });
}

pub(crate) fn spawn_stash(mut commands: Commands, assets: Res<AssetServer>) {
    let (bw, bh) = (f32::from(db_sim::overrides::inventory::BACKPACK_COLS) * CELL, f32::from(db_sim::overrides::inventory::BACKPACK_ROWS) * CELL);
    let (sw, sh) = (f32::from(db_sim::overrides::run::STASH.0) * CELL, f32::from(db_sim::overrides::run::STASH.1) * CELL);
    root(&mut commands, ui(&assets, "Textures/CreateRole/T_UI_CreateRole_Bg")).with_children(|r| {
        r.spawn((Node { align_items: AlignItems::Center, column_gap: px(12.0), padding: UiRect::axes(px(16.0), px(4.0)), ..default() }, BackgroundColor(PANEL))).with_children(|h| {
            h.spawn((ImageNode::default(), Node { width: px(56.0), height: px(56.0), ..default() }, StashCrest));
            h.spawn((text("", 22.0, TEXT), StashHeader));
        });
        r.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(40.0), ..default() }).with_children(|row| {
            // Character: worn slots and backpack.
            row.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(10.0), padding: UiRect::all(px(16.0)), ..default() }, BackgroundColor(PANEL))).with_children(|c| {
                c.spawn(Node { column_gap: px(10.0), align_items: AlignItems::Center, ..default() }).with_children(|h| {
                    h.spawn(text("Character", 24.0, GOLD));
                    button(h, &assets, "Change class", Action::NextClass, 200.0);
                    button(h, &assets, "Q skill", Action::NextSkill(0), 130.0);
                    button(h, &assets, "E skill", Action::NextSkill(1), 130.0);
                });
                c.spawn(Node { flex_wrap: FlexWrap::Wrap, width: px(bw), column_gap: px(8.0), row_gap: px(6.0), ..default() }).with_children(|s| {
                    for slot in WORN_SLOTS {
                        s.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }).with_children(|b| {
                            b.spawn((Node { width: px(CELL + 8.0), height: px(CELL + 8.0), border: UiRect::all(px(1.0)), ..default() }, BackgroundColor(CELL_BG), BorderColor::all(CELL_LINE), StashSlot(slot)));
                            b.spawn(text(slot_label(slot), 13.0, DIM));
                        });
                    }
                });
                c.spawn(text("Backpack", 18.0, DIM));
                grid(c, bw, bh, db_sim::overrides::inventory::BACKPACK_COLS, db_sim::overrides::inventory::BACKPACK_ROWS, Place::Backpack);
                c.spawn((Text::new(""), crate::fonts::sans(16.0), TextColor(TEXT), Node { width: px(bw), min_height: px(150.0), ..default() }, StashTooltip));
            });
            row.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(10.0), padding: UiRect::all(px(16.0)), ..default() }, BackgroundColor(PANEL))).with_children(|c| {
                c.spawn(text("Stash", 24.0, GOLD));
                grid(c, sw, sh, db_sim::overrides::run::STASH.0, db_sim::overrides::run::STASH.1, Place::Stash);
                c.spawn(text("Left click: move between backpack and stash.  Right click: wear / take off.\nShift + left click: sell to the merchant.", 15.0, DIM));
            });
            // The merchant: click to buy into the stash.
            row.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(6.0), padding: UiRect::all(px(16.0)), width: px(360.0), ..default() }, BackgroundColor(PANEL))).with_children(|c| {
                c.spawn(text("Merchant", 24.0, GOLD));
                c.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2.0), max_height: px(sh + 40.0), overflow: Overflow::scroll_y(), ..default() }, ScrollPosition::default(), MerchantList));
                c.spawn(text("Left click: buy (into the stash).", 15.0, DIM));
            });
        });
        r.spawn(Node { column_gap: px(20.0), margin: UiRect::top(px(10.0)), ..default() }).with_children(|b| {
            button(b, &assets, "Change map", Action::NextMap, 220.0);
            button(b, &assets, "Enter", Action::EnterCrypt, 260.0);
            button(b, &assets, "Quit", Action::Quit, 220.0);
        });
    });
}

fn grid(parent: &mut ChildSpawnerCommands, w: f32, h: f32, cols: u8, rows: u8, place: Place) {
    parent.spawn((Node { width: px(w), height: px(h), ..default() }, StashGrid(place))).with_children(|g| {
        for y in 0..rows {
            for x in 0..cols {
                g.spawn((
                    Node { position_type: PositionType::Absolute, left: px(f32::from(x) * CELL), top: px(f32::from(y) * CELL), width: px(CELL), height: px(CELL), border: UiRect::all(px(1.0)), ..default() },
                    BackgroundColor(CELL_BG),
                    BorderColor::all(CELL_LINE),
                ));
            }
        }
    });
}

fn draw_grid(commands: &mut Commands, grid: Entity, inv: &Inventory, place: Place, data: &GameData, assets: &AssetServer) {
    commands.entity(grid).with_children(|g| {
        for b in &inv.bag {
            let (cols, rows) = data.info(b.item.item.item_id).map_or((1, 1), |i| i.size);
            let node = Node { position_type: PositionType::Absolute, left: px(f32::from(b.x) * CELL), top: px(f32::from(b.y) * CELL), width: px(f32::from(cols) * CELL), height: px(f32::from(rows) * CELL), border: UiRect::all(px(2.0)), ..default() };
            spawn_item_node(g, data, assets, &b.item, node, StashItem { uid: b.item.uid, worn: None, place });
        }
    });
}

/// Redraws the stash screen when the profile changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn redraw_stash(
    mut commands: Commands,
    profile: Res<ProfileRes>,
    data: Option<Res<GameData>>,
    assets: Res<AssetServer>,
    drawn: Query<Entity, With<StashItem>>,
    grids: Query<(Entity, &StashGrid)>,
    slots: Query<(Entity, &StashSlot)>,
    mut header: Query<&mut Text, With<StashHeader>>,
    mut crest: Query<&mut ImageNode, With<StashCrest>>,
    mut last: Local<Option<u32>>,
    (lists, rows): (Query<Entity, With<MerchantList>>, Query<Entity, With<MerchantRow>>),
) {
    let Some(data) = data else { return };
    // Redraw on profile changes and when the screen was just built (no items drawn yet).
    let fresh_screen = drawn.is_empty() && profile.profile.all_items().next().is_some();
    if *last == Some(profile.rev) && !fresh_screen {
        return;
    }
    *last = Some(profile.rev);
    for e in &drawn {
        commands.entity(e).despawn();
    }
    let p = &profile.profile;
    for e in &rows {
        commands.entity(e).despawn();
    }
    if let Ok(list) = lists.single() {
        commands.entity(list).with_children(|l| {
            for o in merchant_offers(&data, p) {
                let afford = p.gold >= o.price;
                l.spawn((Button, Node { column_gap: px(8.0), align_items: AlignItems::Center, padding: UiRect::axes(px(4.0), px(2.0)), ..default() }, BackgroundColor(CELL_BG), MerchantRow(o))).with_children(|r| {
                    if let Some(path) = crate::items_view::icon_path(&data, o.item_id) {
                        r.spawn((ImageNode::new(assets.load(path)), Node { width: px(OFFER_ICON), height: px(OFFER_ICON), ..default() }));
                    }
                    let name = if o.count > 1 { format!("{} x{}", item_name(&data, o.item_id), o.count) } else { item_name(&data, o.item_id) };
                    r.spawn(text(format!("{name}  -  {} gold", o.price), 16.0, if afford { TEXT } else { DIM }));
                });
            }
        });
    }
    if let (Ok(mut img), Some((path, _))) = (crest.single_mut(), class_art(&data, p.career.unwrap_or(EnRoleCareerType::Fighter))) {
        img.image = ui(&assets, &path);
    }
    if let Ok(mut h) = header.single_mut() {
        let class = p.career.map_or("no class".to_string(), class_name);
        let career = p.career.unwrap_or(EnRoleCareerType::Fighter);
        let [qs, es] = SkillKind::options(career);
        let skills = if qs.is_empty() {
            "   |   skills come with G5".to_string()
        } else {
            let [q, e] = p.skills.unwrap_or([qs[0], es[0]]);
            format!("  |  Q {}, E {}", q.name(), e.name())
        };
        let map = p.map.as_deref().and_then(db_sim::crypt::map_def).unwrap_or(db_sim::crypt::MAPS[0]);
        h.0 = format!("{class} level {} ({}/{} exp)  |  {}  |  {} gold  |  {} runs ({} out, {} died){skills}{}", p.level, p.exp, p.level_exp(data.tables), map.title, p.gold, p.runs, p.extractions, p.deaths, if p.needs_starting_gear() { "  |  no gear: class kit" } else { "" });
    }
    for (e, g) in &grids {
        let inv = if g.0 == Place::Backpack { &p.character } else { &p.stash };
        draw_grid(&mut commands, e, inv, g.0, &data, &assets);
    }
    for (e, slot) in &slots {
        let Some(item) = p.character.worn.get(&slot.0) else { continue };
        let (cols, rows) = data.info(item.item.item_id).map_or((1, 1), |i| i.size);
        let k = (CELL + 6.0) / f32::from(cols.max(rows));
        let node = Node { width: px(f32::from(cols) * k), height: px(f32::from(rows) * k), margin: UiRect::AUTO, border: UiRect::all(px(1.0)), ..default() };
        commands.entity(e).with_children(|s| spawn_item_node(s, &data, &assets, item, node, StashItem { uid: item.uid, worn: Some(slot.0), place: Place::Backpack }));
    }
}

fn find(inv: &Inventory, uid: u32) -> Option<&ItemInstance> {
    inv.items().find(|i| i.uid == uid)
}

/// Clicks on stash items and the tooltip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stash_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    items: Query<(&Interaction, &StashItem)>,
    offers: Query<(&Interaction, &MerchantRow)>,
    data: Option<Res<GameData>>,
    save: Res<SavePath>,
    mut profile: ResMut<ProfileRes>,
    mut tooltip: Query<&mut Text, With<StashTooltip>>,
    mut message: Local<String>,
) {
    let Some(data) = data else { return };
    let hovered = items.iter().find(|(i, _)| **i != Interaction::None).map(|(_, s)| (s.uid, s.worn, s.place));
    let offer = offers.iter().find(|(i, _)| **i != Interaction::None).map(|(_, r)| r.0);
    if let Some(o) = offer
        && mouse.just_pressed(MouseButton::Left)
    {
        match profile.profile.buy(&o, &data) {
            Ok(()) => {
                *message = format!("Bought {} for {} gold.", item_name(&data, o.item_id), o.price);
                profile.changed(&save);
            }
            Err(e) => *message = format!("Can't: {e}."),
        }
    }
    if let Some((uid, worn, place)) = hovered {
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let result = if mouse.just_pressed(MouseButton::Left) {
            Some(match (worn, shift) {
                (Some(_), _) => Err("right click takes it off"),
                (None, true) => profile.profile.sell(uid, &data).map(|g| format!("Sold for {g} gold.")),
                (None, false) => profile.profile.transfer(uid, &data).map(|to| format!("Moved to the {}.", if to == db_sim::profile::Place::Stash { "stash" } else { "backpack" })),
            })
        } else if mouse.just_pressed(MouseButton::Right) {
            Some(match worn {
                Some(slot) => profile.profile.unequip(slot, &data).map(|()| "Taken off.".to_string()),
                None if place == Place::Backpack => {
                    let slot = find(&profile.profile.character, uid).and_then(|i| data.info(i.item.item_id)).and_then(|info| wear_slot(info, &profile.profile.character));
                    match slot {
                        Some(slot) => profile.profile.equip(uid, slot, &data).map(|()| "Worn.".to_string()),
                        None => Err("can't be worn"),
                    }
                }
                None => Err("move it to the backpack first"),
            })
        } else {
            None
        };
        match result {
            Some(Ok(m)) => {
                *message = m;
                profile.changed(&save);
            }
            Some(Err(e)) => *message = format!("Can't: {e}."),
            None => {}
        }
    }
    let Ok(mut t) = tooltip.single_mut() else { return };
    let p = &profile.profile;
    let item = hovered.and_then(|(uid, _, _)| find(&p.character, uid).or_else(|| find(&p.stash, uid)));
    t.0 = match item {
        Some(i) => {
            let price = data.sell_price(i.item.item_id) * u32::from(i.count);
            let mut lines = vec![format!("{}  ({:?})", item_name(&data, i.item.item_id), i.item.rarity)];
            for a in i.item.affixes.iter().take(6) {
                lines.push(format!("  {}  {:+}", a.attribute.replace('_', " "), (a.value * 1000.0).round() / 1000.0));
            }
            lines.push(if price > 0 { format!("Merchant pays {price} gold") } else { "The merchant doesn't buy this".into() });
            lines.join("\n")
        }
        None => match offer {
            Some(o) => format!("{}  ({:?})\nThe merchant sells it for {} gold (you have {}).", item_name(&data, o.item_id), data.items.rarity(o.item_id).unwrap_or_default(), o.price, p.gold),
            None => message.clone(),
        },
    };
}

/// Buttons → actions.
pub(crate) fn menu_buttons(buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>, mut pending: ResMut<PendingAction>) {
    for (i, b) in &buttons {
        if *i == Interaction::Pressed {
            pending.0 = Some(b.0);
        }
    }
}

/// Dev (`--press-arena`): presses the title's Test Arena a second after the title shows (checks of that path).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PressArena(pub bool);

pub(crate) fn press_arena(press: Res<PressArena>, screen: Res<State<Screen>>, time: Res<Time>, mut pending: ResMut<PendingAction>, mut since: Local<f32>, mut done: Local<bool>) {
    if !press.0 || *done || *screen.get() != Screen::Title {
        return;
    }
    *since += time.delta_secs();
    if *since > 1.0 {
        pending.0 = Some(Action::Arena);
        *done = true;
    }
}

/// Applies the pending action.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_action(
    mut pending: ResMut<PendingAction>,
    screen: Res<State<Screen>>,
    data: Option<Res<GameData>>,
    save: Res<SavePath>,
    mut profile: ResMut<ProfileRes>,
    mut next: ResMut<NextState<Screen>>,
    mut pick: Query<&mut Visibility, With<ClassPick>>,
    mut exit: MessageWriter<AppExit>,
    (mut paused, player, mut queue, mut after): (ResMut<crate::pause::Paused>, Res<crate::LocalPlayer>, ResMut<db_sim::CommandQueue>, ResMut<crate::flow::AfterRun>),
    (mut settings_open, mut settings, mut cover, mut arena): (ResMut<crate::settings::SettingsOpen>, ResMut<crate::settings::Settings>, ResMut<crate::flow::EntryCover>, ResMut<crate::Arena>),
) {
    let Some(action) = pending.0.take() else { return };
    let valid = match action {
        Action::Play | Action::Arena | Action::PickClass(_) => *screen.get() == Screen::Title,
        Action::NextClass | Action::NextSkill(_) | Action::NextMap | Action::EnterCrypt => *screen.get() == Screen::Stash,
        Action::Continue => *screen.get() == Screen::Result,
        Action::Resume => paused.0 && !settings_open.open,
        Action::Settings => (paused.0 || *screen.get() == Screen::Title) && !settings_open.open,
        Action::SettingsDefaults | Action::SettingsBack => settings_open.open,
        Action::Abandon | Action::QuitToTitle => paused.0 && *screen.get() == Screen::Run,
        Action::Quit => true,
    };
    if !valid {
        return;
    }
    info!("menu: {action:?} on {:?}", screen.get());
    match action {
        Action::Play if profile.profile.career.is_some() => next.set(Screen::Stash),
        Action::Arena => {
            arena.0 = true;
            next.set(Screen::TestRoom);
        }
        Action::Play => {
            if let Ok(mut v) = pick.single_mut() {
                *v = Visibility::Inherited;
            }
        }
        Action::PickClass(c) => {
            if let Some(d) = &data {
                profile.profile.set_career(c, d);
            } else {
                profile.profile.career = Some(c);
            }
            profile.changed(&save);
            next.set(Screen::Stash);
        }
        Action::NextClass => {
            let now = profile.profile.career.unwrap_or(EnRoleCareerType::Fighter);
            let i = CLASSES.iter().position(|&c| c == now).map_or(0, |i| (i + 1) % CLASSES.len());
            if let Some(d) = &data {
                profile.profile.set_career(CLASSES[i], d);
                profile.changed(&save);
            }
        }
        Action::NextSkill(slot) => {
            // The class's own choices (classes without skills yet have none).
            let slot = usize::from(slot.min(1));
            let all = SkillKind::options(profile.profile.career.unwrap_or(EnRoleCareerType::Fighter));
            if !all[slot].is_empty() {
                let mut now = profile.profile.skills.unwrap_or([all[0][0], all[1][0]]);
                let i = all[slot].iter().position(|&s| s == now[slot]).map_or(0, |i| (i + 1) % all[slot].len());
                now[slot] = all[slot][i];
                profile.profile.skills = Some(now);
                profile.changed(&save);
            }
        }
        Action::NextMap => {
            let maps = db_sim::crypt::MAPS;
            let now = profile.profile.map.as_deref().and_then(|k| maps.iter().position(|m| m.key == k)).unwrap_or(0);
            profile.profile.map = Some(maps[(now + 1) % maps.len()].key.to_string());
            profile.changed(&save);
        }
        // The entry cover starts the run once drawn.
        Action::EnterCrypt => cover.requested = true,
        Action::Continue => next.set(Screen::Stash),
        Action::Resume => paused.0 = false,
        Action::Settings => settings_open.show(),
        // Back to the defaults, keeping the window as it is.
        Action::SettingsDefaults => {
            let keep = (settings.resolution, settings.window);
            *settings = crate::settings::Settings { resolution: keep.0, window: keep.1, ..Default::default() };
        }
        Action::SettingsBack => settings_open.open = false,
        // Giving up is a death (the sim drops what was carried); quitting to the title gives up too and
        // skips the result screen.
        Action::Abandon | Action::QuitToTitle => {
            paused.0 = false;
            if let Some(id) = player.0 {
                queue.push(db_sim::SimCommand::Abandon { id });
            }
            if action == Action::QuitToTitle {
                after.0 = Some(Screen::Title);
            }
        }
        Action::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

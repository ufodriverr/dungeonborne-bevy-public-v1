//! The pause menu: Esc while playing pauses the game (virtual time stops, so the sim, animations and
//! effects all hold) and opens Resume, Settings, Abandon run and Quit to title (the test room has Quit
//! game instead of the run buttons). An open inventory closes on Esc first; Settings opens the
//! settings screen over it.
//!
//! HYPOTHESIS: the online game had no pause (the world kept going); offline it stops, as a single-player
//! game would. Its panel art is `T_Trade_Popup_Bg1`, the game's popup box.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::flow::{Action, Screen};
use crate::menus::{GOLD, button, text, ui};

/// Whether the game is paused (Esc; `--pause-at` in dev runs).
#[derive(Resource, Debug, Default)]
pub struct Paused(pub bool);

/// Dev (`--pause-at <frames>`): pause after this many frames on a playing screen.
#[derive(Resource, Debug, Default)]
pub struct PauseAt(pub Option<u32>);

#[derive(Component)]
pub(crate) struct PauseMenu;

/// Buttons only in a run (abandon, quit to title).
#[derive(Component)]
pub(crate) struct RunOnly;

/// Buttons only in the test room (quit game).
#[derive(Component)]
pub(crate) struct RoomOnly;

pub(crate) fn setup_pause_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(30),
            Visibility::Hidden,
            PauseMenu,
        ))
        .with_children(|r| {
            r.spawn(Node { width: px(560.0), padding: UiRect::vertical(px(40.0)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(14.0), ..default() }).with_children(|p| {
                // The art fills the panel without sizing it (the buttons do).
                p.spawn((
                    ImageNode { image: ui(&assets, "Textures/Common/PopupBox/T_Trade_Popup_Bg1"), image_mode: NodeImageMode::Stretch, ..default() },
                    Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
                ));
                p.spawn(text("Paused", 48.0, GOLD));
                p.spawn(Node { height: px(6.0), ..default() });
                button(p, &assets, "Resume", Action::Resume, 320.0);
                button(p, &assets, "Settings", Action::Settings, 320.0);
                p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(14.0), ..default() }, RunOnly)).with_children(|b| {
                    button(b, &assets, "Abandon run", Action::Abandon, 320.0);
                    button(b, &assets, "Quit to title", Action::QuitToTitle, 320.0);
                });
                p.spawn((Node { flex_direction: FlexDirection::Column, ..default() }, RoomOnly)).with_children(|b| {
                    button(b, &assets, "Quit game", Action::Quit, 320.0);
                });
            });
        });
}

pub(crate) fn pause_at(screen: Res<State<Screen>>, at: Res<PauseAt>, mut frames: Local<u32>, mut paused: ResMut<Paused>) {
    let Some(n) = at.0 else { return };
    if matches!(screen.get(), Screen::Run | Screen::TestRoom) {
        *frames += 1;
        if *frames == n {
            paused.0 = true;
        }
    }
}

/// Esc pauses and resumes while playing; with the inventory open it only closes the inventory (runs
/// before `inventory_ui::toggle_inventory`).
pub(crate) fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, open: Res<crate::InventoryOpen>, settings: Res<crate::settings::SettingsOpen>, mut paused: ResMut<Paused>) {
    // Esc on the settings screen only closes it (`settings_screen` runs first).
    if keys.just_pressed(KeyCode::Escape) && !open.0 && !settings.open && !settings.just_closed {
        paused.0 = !paused.0;
    }
}

/// Holds virtual time while paused or while the run's entry cover is up after its first few sim ticks (the
/// run doesn't go on behind it: a 25 s load once let a Virus kill the idle player before the cover lifted),
/// shows the menu with the buttons for the screen and frees the cursor.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn apply_pause(
    screen: Res<State<Screen>>,
    (cover, tick): (Res<crate::flow::EntryCover>, Res<db_sim::TickCount>),
    mut paused: ResMut<Paused>,
    mut time: ResMut<Time<Virtual>>,
    mut menu: Query<&mut Visibility, With<PauseMenu>>,
    mut run_only: Query<&mut Node, (With<RunOnly>, Without<RoomOnly>)>,
    mut room_only: Query<&mut Node, (With<RoomOnly>, Without<RunOnly>)>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let in_run = *screen.get() == Screen::Run;
    if !(in_run || *screen.get() == Screen::TestRoom) && paused.0 {
        paused.0 = false;
    }
    let hold = paused.0 || (cover.on && tick.0 >= cover.tick + crate::flow::COVER_SIM_TICKS);
    if hold != time.is_paused() {
        if hold {
            time.pause();
            if paused.0
                && let Ok(mut c) = cursor.single_mut()
            {
                c.grab_mode = CursorGrabMode::None;
                c.visible = true;
            }
        } else {
            time.unpause();
        }
    }
    if let Ok(mut v) = menu.single_mut() {
        v.set_if_neq(if paused.0 { Visibility::Inherited } else { Visibility::Hidden });
    }
    let show = |on: bool| if on { Display::Flex } else { Display::None };
    for mut n in &mut run_only {
        if n.display != show(in_run) {
            n.display = show(in_run);
        }
    }
    for mut n in &mut room_only {
        if n.display != show(!in_run) {
            n.display = show(!in_run);
        }
    }
}

//! The loading screen. The game data is read on a background thread ([`DataLoad`], about 20 s) while the window
//! shows "Loading game data"; then the level's monster models and their clips load ("Loading models 12 / 48"); the
//! effect warm-up (`effects::prewarm_fx`) goes on after that under a small corner note.

use bevy::prelude::*;

use crate::flow::Screen;

/// Read the game data in place at startup instead (hidden dev runs: their frame counts are counted from the level).
#[derive(Resource, Debug, Default)]
pub struct BlockingDataLoad(pub bool);

/// The game data being read on a background thread: what it inserts, as world commands, once read.
#[derive(Resource, Default)]
pub(crate) struct DataLoad {
    pub(crate) handle: Option<std::thread::JoinHandle<bevy::ecs::world::CommandQueue>>,
    pub(crate) done: bool,
}

/// Inserts the game data once its thread is done.
pub(crate) fn finish_data_load(mut commands: Commands, mut load: ResMut<DataLoad>) {
    if load.done || !load.handle.as_ref().is_some_and(std::thread::JoinHandle::is_finished) {
        return;
    }
    if let Some(h) = load.handle.take() {
        match h.join() {
            Ok(mut queue) => commands.append(&mut queue),
            Err(_) => error!("game data: the loading thread panicked; starting without it"),
        }
    }
    load.done = true;
}

/// Whether the game data is in (the boot waits for it).
pub(crate) fn data_loaded(load: Res<DataLoad>) -> bool {
    load.done
}

#[derive(Component)]
pub(crate) struct LoadingScreen;

#[derive(Component)]
pub(crate) struct LoadingText;

#[derive(Component)]
pub(crate) struct WarmupNote;

pub(crate) fn setup_loading_screen(mut commands: Commands) {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgb(0.04, 0.035, 0.03)),
            GlobalZIndex(100),
            LoadingScreen,
        ))
        .with_children(|c| {
            c.spawn((Text::new("Loading game data..."), crate::fonts::heading(36.0), TextColor(Color::srgb(0.9, 0.85, 0.72)), TextLayout::justify(Justify::Center), LoadingText));
        });
    commands.spawn((
        Text::new(""),
        crate::fonts::sans(14.0),
        TextColor(Color::srgba(0.9, 0.85, 0.72, 0.7)),
        Node { position_type: PositionType::Absolute, right: px(16.0), top: px(16.0), ..default() },
        Visibility::Hidden,
        WarmupNote,
    ));
}

/// Shows the loading screen while the data or the level's models load.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn loading_screen(
    screen: Res<State<Screen>>,
    (load, blocking): (Res<DataLoad>, Res<BlockingDataLoad>),
    assets: Res<AssetServer>,
    models: Query<&crate::model_view::ModelBody>,
    prewarm: Res<crate::effects::FxPrewarmQueue>,
    mut panel: Query<&mut Visibility, (With<LoadingScreen>, Without<WarmupNote>)>,
    mut text: Query<&mut Text, (With<LoadingText>, Without<WarmupNote>)>,
    mut note: Query<(&mut Text, &mut Visibility), (With<WarmupNote>, Without<LoadingText>, Without<LoadingScreen>)>,
) {
    let (total, ready) = (models.iter().count(), models.iter().filter(|m| m.loaded(&assets)).count());
    let message = if !load.done || *screen.get() == Screen::Boot {
        Some("Loading game data...".to_string())
    } else if ready < total && !blocking.0 {
        // (Not over hidden dev runs' shots.)
        Some(format!("Loading models {ready} / {total}"))
    } else {
        None
    };
    if let (Ok(mut v), Ok(mut t)) = (panel.single_mut(), text.single_mut()) {
        *v = if message.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if let Some(m) = message
            && t.0 != m
        {
            t.0 = m;
        }
    }
    if let Ok((mut t, mut v)) = note.single_mut() {
        let left = prewarm.left.len();
        *v = if left > 0 { Visibility::Inherited } else { Visibility::Hidden };
        if left > 0 {
            t.0 = format!("Preparing effects {} / {}", prewarm.total - left, prewarm.total);
        }
    }
}

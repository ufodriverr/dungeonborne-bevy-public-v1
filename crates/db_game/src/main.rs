//! Dungeonborne offline (Rust + Bevy). Single player; co-op later on top of db_sim.
//!
//! Started with no arguments it opens the title screen (Play, Test Arena); `--arena` starts in the test arena.
//! Dev flags: `--screenshot <png> [--frames <n>]`, `--bench <seconds>`, `--auto-attack`, `--auto-skill`, `--auto-walk`, `--auto-fight`, `--auto-passive` (see `devtools`).

mod devtools;

use bevy::app::TaskPoolThreadAssignmentPolicy;
use bevy::asset::io::AssetSourceBuilder;
use bevy::prelude::*;
use db_client::ClientPlugin;
use devtools::{DevArgs, DevToolsPlugin};

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dev = match DevArgs::parse(&args) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return AppExit::error();
        }
    };
    // No game data: say where it comes from instead of opening a black window.
    if !std::path::Path::new(&data_dir("assets")).is_dir() {
        eprintln!(
            "No game data found. Dungeonborne on Bevy reads the data you extract yourself from your own Dungeonborne \
             install: put that `_data` folder next to dungeonborne.exe, or set DUNGEONBORNE_DATA to its path. See \
             README.md and EXTRACTION.md."
        );
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            eprintln!("Press Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        return AppExit::error();
    }
    App::new()
        // `audio://<media id>.ogg` → `_data/audio/ogg`; registered before the AssetPlugin.
        .register_asset_source("audio", AssetSourceBuilder::platform_default(&data_dir("audio/ogg"), None))
        // Asset loading (glTF parsing, PNG decoding) runs on the IO pool, 4 threads by default; entering the
        // castle loads about 130 cell scenes and their textures, so it gets 8, and compute keeps 8.
        .add_plugins(DefaultPlugins.set(TaskPoolPlugin { task_pool_options: TaskPoolOptions { io: TaskPoolThreadAssignmentPolicy { min_threads: 1, max_threads: 8, percent: 0.5, on_thread_spawn: None, on_thread_destroy: None }, compute: TaskPoolThreadAssignmentPolicy { min_threads: 8, max_threads: usize::MAX, percent: 1.0, on_thread_spawn: None, on_thread_destroy: None }, ..default() } }).set(AssetPlugin { file_path: data_dir("assets"), ..default() })
        // The converted meshes carry UV channels 2-7, which Bevy ignores with one warning per primitive: about
        // 166,000 lines per castle load. Printed to a console window they stalled the map load for minutes and
        // kept the console busy after the game closed, so that warning is off (`RUST_LOG` still overrides the
        // filter).
        .set(bevy::log::LogPlugin { filter: format!("{},bevy_gltf::loader=error", bevy::log::DEFAULT_FILTER), ..default() })
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dungeonborne (offline)".into(),
                resolution: (1920, 1080).into(),
                present_mode: dev.present_mode(),
                // Unattended checks never show a window; `--visible` overrides.
                visible: !dev.hidden(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ClientPlugin)
        .insert_resource(db_client::SavePath(Some(std::path::Path::new(&data_dir("")).join("save").join("profile.json"))))
        // Hidden checks run on the defaults and leave the player's settings alone, unless `--settings`.
        .insert_resource(db_client::SettingsPath(match (&dev.settings, dev.hidden()) {
            (Some(p), _) => Some(p.into()),
            (None, false) => Some(std::path::Path::new(&data_dir("")).join("save").join("settings.json")),
            (None, true) => None,
        }))
        .add_plugins(DevToolsPlugin(dev))
        .run()
}

/// A folder under the gitignored `_data` (extracted from the install, never shipped in git; beside the
/// exe in a packaged copy, `db_formats::data_root`).
fn data_dir(sub: &str) -> String {
    db_client::data_path(sub).to_string_lossy().into_owned()
}

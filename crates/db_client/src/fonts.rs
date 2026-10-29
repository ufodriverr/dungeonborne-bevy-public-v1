//! The game's UI fonts, from the install's `TGF/UI/Font/*.ufont` (plain TrueType, copied to
//! `_data/assets/fonts/game/`). CONFIRMED by the widget blueprints' imports: 176 widgets use
//! `LibreBaskerville-Regular_Font`, 32 the `Common/NotoSans` composite font (the HUD dashboard's weapon
//! and buff widgets, item tooltips, task and trade lists), 1 `LibreBaskerville-Bold_Font`. So Libre
//! Baskerville Regular replaces Bevy's built-in default font (every `TextFont` without a font uses it),
//! headings use its Bold face and the HUD dashboard and inventory use Noto Sans. Without the files, text
//! keeps Bevy's default font.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource, FontWeight};

const DIR: &str = "fonts/game";
const SERIF: &str = "Libre Baskerville";
const SANS: &str = "Noto Sans";

/// Keeps the extra faces loaded (they are picked by family name).
#[derive(Resource, Debug, Default)]
pub struct UiFonts(pub Vec<Handle<Font>>);

/// Startup, before the first text layout: Libre Baskerville into the default font slot, the other faces
/// loaded as assets.
pub(crate) fn load_fonts(mut fonts: ResMut<Assets<Font>>, assets: Res<AssetServer>, mut keep: ResMut<UiFonts>) {
    let rel = format!("assets/{DIR}/LibreBaskerville-Regular.ttf");
    match crate::map_view::data_file(&rel).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => {
            if fonts.insert(AssetId::default(), Font::from_bytes(bytes)).is_err() {
                warn!("couldn't replace the default font");
            }
        }
        None => {
            warn!("no _data/{rel}; text keeps Bevy's default font");
            return;
        }
    }
    for face in ["LibreBaskerville-Bold", "NotoSans-Regular", "NotoSans-Bold"] {
        keep.0.push(assets.load(format!("{DIR}/{face}.ttf")));
    }
}

/// Headings: Libre Baskerville Bold.
pub(crate) fn heading(px: f32) -> TextFont {
    TextFont { font: FontSource::Family(SERIF.into()), weight: FontWeight::BOLD, font_size: FontSize::Px(px), ..default() }
}

/// HUD dashboard and inventory text: Noto Sans.
pub(crate) fn sans(px: f32) -> TextFont {
    TextFont { font: FontSource::Family(SANS.into()), font_size: FontSize::Px(px), ..default() }
}

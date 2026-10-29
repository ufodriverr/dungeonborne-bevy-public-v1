//! the skill slots' round cooldown sweep (`cooldown_sweep.wgsl`): the share of a cooldown left, shaded over the
//! icon and clearing clockwise, as the game's round progress bar does.

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub(crate) fn plugin(app: &mut App) {
    embedded_asset!(app, "cooldown_sweep.wgsl");
    app.add_plugins(UiMaterialPlugin::<CooldownSweep>::default());
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub(crate) struct Sweep {
    /// x: the share of the cooldown left (1 just used, 0 ready).
    pub left: Vec4,
    pub color: LinearRgba,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(crate) struct CooldownSweep {
    #[uniform(0)]
    pub sweep: Sweep,
}

impl CooldownSweep {
    /// Black at 0.9: blending is in linear light, so this reads as about three quarters darker on screen.
    pub(crate) fn new() -> Self {
        CooldownSweep { sweep: Sweep { left: Vec4::ZERO, color: LinearRgba::new(0.0, 0.0, 0.0, 0.9) } }
    }
}

impl UiMaterial for CooldownSweep {
    fn fragment_shader() -> ShaderRef {
        "embedded://db_client/cooldown_sweep.wgsl".into()
    }
}

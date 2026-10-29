// a skill slot's cooldown, as the game's round progress bar (`M_UI_TgfGame_RoundProgressBar` over
// `T_UI_TgfGame_Skill_CircleMask`): the part of the disc still cooling is shaded, and it clears clockwise from
// 12 o'clock as the cooldown runs out (HYPOTHESIS: the direction; the cooked material keeps no graph).
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Sweep {
    // x: the share of the cooldown left (1 just used, 0 ready).
    left: vec4<f32>,
    color: vec4<f32>,
};

@group(1) @binding(0) var<uniform> sweep: Sweep;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv - vec2<f32>(0.5, 0.5);
    let r = length(p);
    // A soft one-pixel edge on the disc.
    let edge = clamp((0.5 - r) * in.size.x, 0.0, 1.0);
    var a = atan2(p.x, -p.y) / 6.2831853;
    if a < 0.0 {
        a = a + 1.0;
    }
    if a < 1.0 - sweep.left.x {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(sweep.color.rgb, sweep.color.a * edge);
}

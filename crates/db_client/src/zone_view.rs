//! The death zone drawn: `BP_DeathZoneVisuals` scales an open cylinder (`SM_Cylinder_NoCap`) to the circle, `Dynamic
//! Radius` × 0.01 across and `Default Scale Z` 100 tall, `Default Offset Z` -5000 cm down, with
//! `MI_PoisonousCircleMesh` (red edge `EdgeCol` 960018, `Opacity` 0.2). Here a translucent red tube 100 m tall from 50
//! m below; the shader's noise, refraction and depth fade aren't rebuilt (HYPOTHESIS look). In the zone the screen
//! takes a red tint (the post-process `MI_PoisonousCircle` isn't rebuilt either). Only the local player's floor's zone
//! shows, once it has started. Its messages (`SimEvent::ZoneMessage`) show for a few seconds under the clock; the data
//! has their text in Chinese only (`GeneralStrings`), so the English here is our translation. Reads sim state only.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use db_sim::death_zone::{DeathZone, ZoneExposure, ZoneState};
use db_sim::{RunStatus, SimIndex};

use crate::combat_view::LocalPlayer;
use crate::to_bevy;

/// The tube drawn for a floor's zone.
#[derive(Component)]
pub(crate) struct ZoneWall(u32);

/// The red tint over the screen while the local player is in the zone.
#[derive(Component)]
pub(crate) struct ZoneTint;

/// The zone's message line.
#[derive(Component)]
pub(crate) struct ZoneToast;

/// How long a message stays (s). HYPOTHESIS: `MessageDuration` is 0 in the data (the widget's own default).
const TOAST_SECS: f32 = 6.0;

/// A message key's text: our English for the `GeneralStrings` Chinese (仪式 / 城堡 lines), else the key.
fn message_text(key: &str) -> &str {
    match key {
        // 剩余时间还有3分钟… / 1分钟… / 超过时间…
        "Shadowking_ClosingAlert" => "3 minutes left...",
        "Shadowking_ClosingCountDown" => "1 minute left...",
        "Shadowking_Overtime" => "Time is up...",
        // 血月降临，城堡即将长眠黑暗，请尽快逃离… / 城堡即将长眠黑暗，请尽快逃离…
        "Castle_ClosingAlert" => "The Blood Moon falls. The castle will soon sleep in darkness - escape now...",
        "Castle_ClosingCountdown" => "The castle will soon sleep in darkness - escape now...",
        _ => key,
    }
}

/// An open tube of radius 1 and height 1 (base at 0), `n` sides, normals out.
fn tube(n: usize) -> Mesh {
    let mut pos = Vec::with_capacity(2 * (n + 1));
    let mut nor = Vec::with_capacity(2 * (n + 1));
    let mut uv = Vec::with_capacity(2 * (n + 1));
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        for y in [0.0, 1.0] {
            pos.push([c, y, s]);
            nor.push([c, 0.0, s]);
            uv.push([i as f32 / n as f32 * 64.0, y]);
        }
    }
    let mut idx = Vec::with_capacity(6 * n);
    for i in 0..n as u32 {
        let (a, b) = (2 * i, 2 * i + 2);
        idx.extend([a, a + 1, b, b, a + 1, b + 1]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

/// A wall for each new zone, and the tint once.
pub(crate) fn spawn_zone_views(mut commands: Commands, zones: Query<&DeathZone, Added<DeathZone>>, tint: Query<(), With<ZoneTint>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut made = false;
    for z in &zones {
        made = true;
        let mat = materials.add(StandardMaterial { base_color: Color::srgba(0.59, 0.0, 0.09, 0.2), emissive: LinearRgba::rgb(0.3, 0.0, 0.01), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, ..default() });
        commands.spawn((Mesh3d(meshes.add(tube(128))), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, ZoneWall(z.floor), bevy::light::NotShadowCaster, crate::flow::LevelScoped));
    }
    if made && tint.is_empty() {
        commands.spawn((Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() }, BackgroundColor(Color::srgba(0.45, 0.0, 0.05, 0.18)), Visibility::Hidden, GlobalZIndex(-1), ZoneTint, crate::flow::LevelScoped));
        commands.spawn((
            Text::new(""),
            TextFont { font_size: bevy::text::FontSize::Px(24.0), ..default() },
            TextColor(Color::srgb(0.95, 0.45, 0.4)),
            TextShadow::default(),
            TextLayout::justify(Justify::Center),
            Node { position_type: PositionType::Absolute, left: percent(50.0), top: px(56.0), margin: UiRect::left(px(-450.0)), width: px(900.0), ..default() },
            Visibility::Hidden,
            ZoneToast,
            crate::flow::LevelScoped,
        ));
    }
}

/// Follows the circle; shows the local player's floor's zone and the tint while they're in it.
#[allow(clippy::type_complexity)]
pub(crate) fn update_zone_views(zones: Query<&DeathZone>, player: Res<LocalPlayer>, index: Res<SimIndex>, status: Query<(&RunStatus, Option<&ZoneExposure>)>, mut walls: Query<(&ZoneWall, &mut Transform, &mut Visibility), Without<ZoneTint>>, mut tint: Query<&mut Visibility, (With<ZoneTint>, Without<ZoneWall>)>) {
    let me = player.0.and_then(|id| index.get(id)).and_then(|e| status.get(e).ok());
    let floor = me.map(|(s, _)| s.floor);
    for (w, mut t, mut v) in &mut walls {
        let Some(z) = zones.iter().find(|z| z.floor == w.0) else { continue };
        let show = Some(z.floor) == floor && z.state != ZoneState::Idle && z.radius > 0.0;
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        if show {
            let r = z.radius * 0.01;
            *t = Transform { translation: to_bevy(z.center.extend(-5000.0)), rotation: Quat::IDENTITY, scale: Vec3::new(r, 100.0, r) };
        }
    }
    let inside = me.and_then(|(_, x)| x).is_some_and(|x| x.in_zone);
    for mut v in &mut tint {
        let want = if inside { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}

/// Shows the local player's floor's zone messages for a few seconds.
pub(crate) fn zone_messages(events: Res<crate::combat_view::FrameEvents>, player: Res<LocalPlayer>, index: Res<SimIndex>, status: Query<&RunStatus>, time: Res<Time>, mut left: Local<f32>, mut toast: Query<(&mut Text, &mut Visibility), With<ZoneToast>>) {
    let Ok((mut text, mut v)) = toast.single_mut() else { return };
    let floor = player.0.and_then(|id| index.get(id)).and_then(|e| status.get(e).ok()).map(|s| s.floor);
    for e in &events.0 {
        if let db_sim::SimEvent::ZoneMessage { floor: f, key } = e
            && Some(*f) == floor
        {
            text.0 = message_text(key).to_string();
            *left = TOAST_SECS;
        }
    }
    *left -= time.delta_secs();
    let want = if *left > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    if *v != want {
        *v = want;
    }
}

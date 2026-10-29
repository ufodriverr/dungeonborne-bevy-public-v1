//! The Druid's panther form in third person. `GA_TGF_DruidPanther_Q_Human` sets the camera mode
//! `CM_ThirdPerson_Panther` (CONFIRMED bytecode) for the whole form (`bForceUseThirdPersonMesh`); its
//! `TargetOffsetCurve` `PantherCameraCurve` holds the camera 200 cm behind, 75 cm right and 50 cm below the view's
//! pivot. Here the camera eases out to that offset while the local player is the panther, and the panther body
//! (`fp_body`, fitted at the eye) is put back at the eye and kept level ([`body_offset`]). Level geometry between
//! the eye and the camera pulls it in ([`clear_of_walls`], as a camera boom would). HYPOTHESIS: it goes out once
//! the panther appears (the human cast before it stays in first person, there being no third-person human body).

use bevy::prelude::*;
use db_sim::{ClassKit, SimIndex};

/// The camera mode's offset in the view's frame (m): right, up, back.
const OFFSET: Vec3 = Vec3::new(0.75, -0.5, 2.0);
/// Seconds to ease the camera out and back.
const EASE_S: f32 = 0.3;
/// How far short of a wall a pulled-in camera stops (cm).
const WALL_MARGIN: f32 = 20.0;

/// Bevy metres (Y up) → sim centimetres (Z up): `to_bevy` undone.
fn to_sim(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, v.y) * 100.0
}

/// The level's geometry for [`clear_of_walls`]: its boxes and its map pieces.
pub(crate) type Statics<'w, 's> = (Query<'w, 's, (), With<db_sim::LevelBox>>, Query<'w, 's, (), With<db_sim::map::MapPiece>>);

/// A third-person camera's spot `want` pulled in towards the `eye` (both Bevy) so no level geometry comes between
/// them, a little short of the wall.
pub(crate) fn clear_of_walls(spatial: &db_sim::SpatialQuery, (boxes, pieces): &Statics, eye: Vec3, want: Vec3) -> Vec3 {
    let (a, b) = (to_sim(eye), to_sim(want));
    let Ok(dir) = Dir3::new(b - a) else { return want };
    let len = (b - a).length();
    match spatial.cast_ray_predicate(a, dir, len, true, &db_sim::layers::shots(), &|e| boxes.contains(e) || pieces.contains(e)) {
        Some(hit) => eye + (want - eye) * ((hit.distance - WALL_MARGIN).max(0.0) / len),
        None => want,
    }
}

/// How far out the camera is (0 first person .. 1 the panther's view).
#[derive(Resource, Default)]
pub(crate) struct PantherView {
    pub(crate) out: f32,
    /// The offset the camera got this frame, in its own frame (m; less than the mode's at a wall).
    applied: Vec3,
}

impl PantherView {
    /// The eased amount.
    fn k(&self) -> f32 {
        self.out * self.out * (3.0 - 2.0 * self.out)
    }

    /// The panther body's transform under the camera for its first-person `fit` (its camera bone at the eye): put
    /// back at the eye and levelled against the view's `pitch`, as far as the camera is out.
    pub(crate) fn body_offset(&self, fit: Transform, pitch: f32) -> Transform {
        let k = self.k();
        if k <= 0.0 {
            return fit;
        }
        Transform::from_translation(-self.applied).with_rotation(Quat::from_rotation_x(-pitch * k)) * fit
    }

    /// Out far enough to show the whole panther (its head, its change).
    pub(crate) fn third_person(&self) -> bool {
        self.out > 0.5
    }
}

/// Eases the camera out while the local player is the panther (after `follow_camera` placed it at the eye).
#[allow(clippy::too_many_arguments)]
pub(crate) fn panther_camera(time: Res<Time>, player: Res<crate::LocalPlayer>, index: Res<SimIndex>, kits: Query<&ClassKit>, mut view: ResMut<PantherView>, mut cam: Query<&mut Transform, With<crate::PlayerCamera>>, spatial: db_sim::SpatialQuery, statics: Statics) {
    let panther = player.0.and_then(|id| index.get(id)).and_then(|e| kits.get(e).ok()).is_some_and(|k| k.panther);
    let step = time.delta_secs() / EASE_S;
    view.out = if panther { (view.out + step).min(1.0) } else { (view.out - step).max(0.0) };
    if view.out <= 0.0 {
        view.applied = Vec3::ZERO;
        return;
    }
    let Ok(mut t) = cam.single_mut() else { return };
    let k = view.k();
    let eye = t.translation;
    t.translation = clear_of_walls(&spatial, &statics, eye, eye + t.rotation * (OFFSET * k));
    view.applied = t.rotation.inverse() * (t.translation - eye);
}

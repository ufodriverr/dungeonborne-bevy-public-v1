//! The maps' placed effects (-> `_data/recovered/maps/<level>-vfx.json`): the torches', braziers', candles'
//! and lanterns' fires, and the dust, mist, rain and moths the levels place, played through the effect library
//! (`sys:<system>`) while near the camera. Presentation only.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::effects::{EffectLibrary, spawn_effect_scaled, stop};

/// Placed effects play within this of the camera (m; ours: the CPU particles' budget, the effects' size).
const RANGE_M: f32 = 25.0;
/// At most this many play at once, the nearest (ours).
const MAX_PLACED: usize = 32;

/// The current map's placed effects (library key, Bevy position, UE-space rotation, scale) and the ones playing. The
/// escape points' start effects are among them but wait for their point's portal (`gated`, `started`).
#[derive(Resource, Default)]
pub(crate) struct MapVfx {
    key: Option<&'static str>,
    placed: Vec<(String, Vec3, Quat, f32)>,
    live: BTreeMap<usize, Entity>,
    /// Each escape point (sim cm) and its effects' indices in `placed`.
    escapes: Vec<(Vec3, Vec<usize>)>,
    gated: std::collections::BTreeSet<usize>,
    started: std::collections::BTreeSet<usize>,
}

impl MapVfx {
    /// `has`: whether the effect library holds a key.
    fn load(maps: &db_sim::crypt::CryptMaps, has: impl Fn(&str) -> bool) -> Self {
        let mut placed = Vec::new();
        let mut escapes: Vec<(Vec3, Vec<usize>)> = Vec::new();
        let mut missing = 0;
        let read = |rel: String| std::fs::read(db_formats::data_path(rel)).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let f = |v: &serde_json::Value, k: usize| v[k].as_f64().unwrap_or(0.0) as f32;
        let v3 = |v: &serde_json::Value| Vec3::new(f(v, 0), f(v, 1), f(v, 2));
        // One placed effect: its library key, Bevy position, UE rotation and scale (`None` when the library lacks it;
        // the scale is the mean of the component's axes, mirrored ones (-1) as 1: 199 of 1,671 aren't uniform).
        let mut one = |e: &serde_json::Value, off: Vec3| {
            let system = e["system"].as_str()?;
            let key = format!("sys:{}", system.rsplit('/').next().unwrap_or(system));
            if !has(&key) {
                missing += 1;
                return None;
            }
            // Rows of the UE rotation matrix (world = x * row0 + y * row1 + z * row2): its columns here.
            let r = &e["rot"];
            let m = Mat3::from_cols(Vec3::new(f(r, 0), f(r, 1), f(r, 2)), Vec3::new(f(r, 3), f(r, 4), f(r, 5)), Vec3::new(f(r, 6), f(r, 7), f(r, 8)));
            let s = &e["scale"];
            let scale = if s.is_array() { (f(s, 0).abs() + f(s, 1).abs() + f(s, 2).abs()) / 3.0 } else { 1.0 };
            Some((key, crate::to_bevy(v3(&e["pos"]) + off), Quat::from_mat3(&m).normalize(), scale))
        };
        for (i, level) in maps.def.levels.iter().enumerate() {
            let off = maps.def.offset(i);
            if let Some(j) = read(format!("recovered/maps/{level}-vfx.json")) {
                placed.extend(j.as_array().into_iter().flatten().filter_map(|e| one(e, off)));
            }
            for p in read(format!("recovered/maps/{level}-escape-vfx.json")).as_ref().and_then(|j| j.as_array()).into_iter().flatten() {
                let fx: Vec<_> = p["effects"].as_array().into_iter().flatten().filter_map(|e| one(e, off)).collect();
                escapes.push((v3(&p["point"]) + off, (placed.len()..placed.len() + fx.len()).collect()));
                placed.extend(fx);
            }
        }
        let gated = escapes.iter().flat_map(|e| e.1.iter().copied()).collect();
        info!("map vfx: {} placed effects on {}, {} escape points' ({missing} not in the library)", placed.len(), maps.def.key, escapes.len());
        Self { key: Some(maps.def.key), placed, live: BTreeMap::new(), escapes, gated, started: Default::default() }
    }

    /// Starts the effects of the escape point at `point` (sim cm, its floor point: the portal's): its
    /// `StartMechanisms` (the magic circle, the portal) play while it's in use. False when none stands there.
    pub(crate) fn start_escape(&mut self, point: Vec3) -> bool {
        let Some((_, fx)) = self.escapes.iter().find(|(p, _)| p.truncate().distance(point.truncate()) < 100.0 && (p.z - point.z).abs() < 300.0) else { return false };
        info!("map vfx: the escape point at {point:.0} starts its {} effects", fx.len());
        self.started.extend(fx.iter().copied());
        true
    }

    /// The placed effects to play for a camera at `eye` (Bevy m): the nearest [`MAX_PLACED`] within [`RANGE_M`].
    fn wanted(&self, eye: Vec3) -> Vec<usize> {
        let mut near: Vec<(f32, usize)> = self.placed.iter().enumerate().filter(|(i, _)| !self.gated.contains(i) || self.started.contains(i)).map(|(i, p)| (p.1.distance(eye), i)).filter(|(d, _)| *d <= RANGE_M).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.into_iter().take(MAX_PLACED).map(|(_, i)| i).collect()
    }
}

/// Plays the map's placed effects near the camera during a run and stops those left behind.
pub(crate) fn placed_effects(mut commands: Commands, crypt: Res<crate::CurrentCrypt>, lib: Res<EffectLibrary>, screen: Res<State<crate::Screen>>, camera: Query<&GlobalTransform, With<Camera3d>>, mut vfx: ResMut<MapVfx>, mut shown: Local<usize>) {
    let reload = match &crypt.0 {
        Some(maps) => vfx.key != Some(maps.def.key),
        None => vfx.key.is_some(),
    };
    if reload {
        for e in std::mem::take(&mut vfx.live).into_values() {
            stop(&mut commands, e);
        }
        *vfx = crypt.0.as_ref().map_or_else(MapVfx::default, |m| MapVfx::load(m, |k| lib.has(k)));
    }
    let eye = camera.iter().next().map(|c| c.translation()).filter(|_| *screen.get() == crate::Screen::Run);
    if eye.is_none() {
        // (A new run picks its escape points again.)
        vfx.started.clear();
    }
    let want = eye.map_or_else(Vec::new, |eye| vfx.wanted(eye));
    let gone: Vec<usize> = vfx.live.keys().filter(|i| !want.contains(i)).copied().collect();
    for i in gone {
        if let Some(e) = vfx.live.remove(&i) {
            stop(&mut commands, e);
        }
    }
    for i in want {
        if vfx.live.contains_key(&i) {
            continue;
        }
        let (key, pos, rot, scale) = &vfx.placed[i];
        if let Some(e) = spawn_effect_scaled(&mut commands, &lib, key, *pos, *rot, None, *scale) {
            vfx.live.insert(i, e);
        }
    }
    if *shown != vfx.live.len() {
        *shown = vfx.live.len();
        debug!("map vfx: {} playing", vfx.live.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the castle's placed fires come from the recon files, every one in the effect library, a wall torch's
    /// flame by the first start's hall (skipped without the files).
    #[test]
    fn the_castle_places_its_torch_fires_from_the_effect_library() {
        let Some(maps) = db_sim::crypt::CryptMaps::load(db_sim::crypt::CASTLE).ok() else { return };
        let Some(lib) = std::fs::read(db_formats::data_path("recovered/effects/effects.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else { return };
        let lib = crate::effects::parse_library(&lib);
        let mut vfx = MapVfx::load(&maps, |k| lib.contains_key(k));
        if vfx.placed.is_empty() {
            return;
        }
        let fires = vfx.placed.iter().filter(|p| p.0 == "sys:FX_Scene_Brazier_01Instance2").count();
        assert!(fires >= 350, "{fires} torch fires");
        // The wall torch at (-5400, 60774, -58) cm on floor 1 (at its offset).
        let torch = crate::to_bevy(Vec3::new(-5400.0, 60773.6, -58.3) + maps.offset(0));
        assert!(vfx.placed.iter().any(|p| p.0 == "sys:FX_Scene_Brazier_01Instance2" && p.1.distance(torch) < 0.05));
        let near = vfx.wanted(torch);
        assert!(!near.is_empty() && near.len() <= MAX_PLACED && vfx.placed[near[0]].1.distance(torch) < 0.05);
        // No two the same at one spot.
        let mut spots: Vec<_> = vfx.placed.iter().map(|p| (p.0.clone(), (p.1 * 100.0).round().as_ivec3().to_array())).collect();
        let n = spots.len();
        spots.sort();
        spots.dedup();
        assert_eq!(spots.len(), n);
        // the five escape points' circles and portals wait for their point's portal.
        assert_eq!(vfx.escapes.len(), 5);
        let (point, fx) = vfx.escapes[0].clone();
        let at = vfx.placed[fx[0]].1;
        assert!(!vfx.wanted(at).contains(&fx[0]));
        assert!(!vfx.start_escape(point + Vec3::X * 500.0));
        assert!(vfx.start_escape(point + Vec3::Z * 5.0));
        assert!(vfx.wanted(at).contains(&fx[0]));
        // Each keeps its placed scale (eight of the castle's wall torches are placed at 0.72).
        assert!(vfx.placed.iter().any(|p| p.0 == "sys:FX_Scene_Brazier_01Instance2" && (p.3 - 0.7177).abs() < 0.001));
    }
}

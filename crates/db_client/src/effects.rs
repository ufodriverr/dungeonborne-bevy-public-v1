//! Particle effects: a small CPU particle system that plays the game's effects from the numbers read out of the
//! recovered Niagara systems (→ `_data/recovered/effects/effects.json`). Each emitter spawns particles at its
//! rate, its bursts and per distance moved, with a lifetime, a velocity (speed range in a cone around a
//! direction), gravity and drag, a colour and size over life, and its renderer's texture and blend mode
//! (additive or alpha). Three renderers: camera-facing sprites, mesh particles (the VFX meshes from the library,
//! scaled over life) and camera-facing ribbons through an emitter's recent particles. GPU sprite emitters, whose
//! particle numbers aren't in the data, get CPU stand-ins from their spawn data, curves and bounds (HYPOTHESIS).
//! Each emitter of a playing effect is one batch: its particles are data, drawn as one mesh rebuilt every frame
//! with the effect material (`fx_material.rs`), so per-particle colour, the dynamic material parameter and the
//! flipbook blend ride on the vertices. Presentation only: nothing here feeds back into the sim. Skill triggers
//! live in `skill_vfx.rs`.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::fx_material::{ATTRIBUTE_FX_DYN, ATTRIBUTE_FX_DYN1, ATTRIBUTE_FX_DYN2, ATTRIBUTE_FX_DYN3, ATTRIBUTE_FX_MISC, FxMaterial};
use crate::to_bevy;

/// Upper bound on live particles (protects the frame time).
const MAX_PARTICLES: usize = 4000;
/// HDR colours are scaled down to this brightest channel, keeping their hue (some sparks carry 28+).
const MAX_BRIGHTNESS: f32 = 2.0;
/// Widest a sprite may be, as a share of its distance from the camera.
const NEAR_SPRITE: f32 = 1.0;
/// Most ribbon points one ribbon keeps (older ones drop first).
const MAX_RIBBON_POINTS: usize = 96;
/// An effect moving farther than this in one frame jumped: its ribbons start over, nothing spawns along
/// the jump.
const TELEPORT_CM: f32 = 1000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Blend {
    Add,
    Alpha,
}

/// How an emitter's particles are drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Draw {
    /// Camera-facing quads.
    Sprite,
    /// A mesh particle: the library GLB, its scale over life (t, Bevy axes; the GLBs are in metres and the
    /// Niagara `Scale` multiplies them), turned to face the camera when the renderer says so.
    Mesh { glb: String, scale: Vec<(f32, Vec3)>, face_camera: bool },
    /// A camera-facing strip through the emitter's live particles in spawn order; width over life (t, cm).
    Ribbon { width: Vec<(f32, f32)> },
    /// Not drawn: an emitter without a renderer whose particles only send events to others.
    Hidden,
}

/// An emitter spawned by another emitter's events (a Niagara event handler): `count` particles (a random number
/// in the range) at each event of emitter `source` (its index in the effect), at most `per_frame` events a frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EventSpawn {
    source_name: String,
    source: usize,
    /// A `DeathEvent` (a particle dying), else a `LocationEvent` (every live particle, every frame).
    death: bool,
    count: (u32, u32),
    per_frame: u32,
}

/// One emitter's numbers (UE units: cm, s, cm/s, cm/s²).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EmitterSpec {
    pub name: String,
    pub rate: f32,
    pub bursts: Vec<(u32, f32)>,
    /// Particles per cm the effect moves (Niagara SpawnPerUnit: projectile trails).
    pub per_cm: f32,
    /// Emitter loop length, s.
    pub duration: f32,
    /// Keeps spawning (rate and per-loop bursts) for as long as the effect plays; else one loop only.
    pub loops: bool,
    pub lifetime: (f32, f32),
    pub speed: (f32, f32),
    /// Cone half angle around `direction`, degrees (180 = all round).
    pub cone: f32,
    /// UE direction (Z up) in the effect's frame; +Z when the data gives none.
    pub direction: Vec3,
    /// Spawn offset radius on the effect's floor plane (a disc; GPU mesh stand-ins), cm.
    pub radius: f32,
    /// Acceleration, UE cm/s² (Z up).
    pub gravity: Vec3,
    pub drag: f32,
    pub colors: Vec<(f32, [f32; 4])>,
    pub size: (f32, f32),
    pub size_over_life: Vec<(f32, f32)>,
    /// Absolute size over life (t, min cm, max cm) when the extractor stepped it; preferred.
    pub size_cm: Vec<(f32, f32, f32)>,
    pub texture: Option<String>,
    pub blend: Blend,
    pub subuv: (u32, u32),
    /// The flipbook blends into its next frame (the renderer's `SubImageBlend`); else it steps.
    pub subuv_blend: bool,
    /// Niagara `DynamicMaterialParameter` 0-3 over life (t, xyzw): the material's per-particle inputs.
    pub dyns: [Vec<(f32, [f32; 4])>; 4],
    /// A beam's colour along it (link order 0..1, rgba), as a multiple of its middle's (the Fire staff
    /// laser's alpha is a curve along the beam, not over life).
    pub along: Vec<(f32, [f32; 4])>,
    /// Spawned by these events of other emitters.
    pub events: Vec<EventSpawn>,
    /// Events its particles send (bit 1: location, every frame; bit 2: death).
    pub sends: u8,
    /// The renderer's material instance (its path).
    pub material: Option<String>,
    /// Its recovered formula: then the colours are the particles' own and the formula does the rest.
    pub exact: Option<crate::fx_material::FxExact>,
    /// Point light (colour, intensity in UE units, radius cm).
    pub light: Option<([f32; 3], f32, f32)>,
    /// Particles move with the effect (Niagara local space) instead of staying where they spawned.
    pub local: bool,
    /// The emitter reads the beam end's socket (`GetFilteredSocketTransform (User.BeamEndBone)` in the converter's
    /// notes): in a [`Beam`] effect its particles travel from the start to the end over their life (the staff
    /// recharge's fire streaming into the left hand; HYPOTHESIS: the socket read is a seek, not evaluated).
    pub seek_end: bool,
    /// Sprite renderer modes: `FacingMode` (0 camera, 1 camera plane, 2 custom vector, 3 camera position, 4
    /// distance blend) and `Alignment` (0 none, 1 velocity, 2 custom vector).
    pub facing: u8,
    pub align: u8,
    /// The particles' `SpriteFacing` / `SpriteAlignment` (UE, the emitter's frame). None when the emitter writes none
    /// (51 custom-facing sprites, 9 custom-aligned, the Wizard's explosion fire among them): those keep facing the
    /// camera, unaligned (HYPOTHESIS; the engine's fallback isn't in the data).
    pub facing_vec: Option<Vec3>,
    pub align_vec: Option<Vec3>,
    /// Spawn `SpriteRotation` range, degrees.
    pub spin: (f32, f32),
    /// A non-uniform `SpriteSize`'s width and height as multiples of the drawn `size` (its narrower axis).
    pub aspect: Vec2,
    pub draw: Draw,
}

fn pair(v: &serde_json::Value, d: (f32, f32)) -> (f32, f32) {
    match v {
        serde_json::Value::Array(a) if a.len() >= 2 => (a[0].as_f64().unwrap_or(d.0 as f64) as f32, a[1].as_f64().unwrap_or(d.1 as f64) as f32),
        serde_json::Value::Number(n) => (n.as_f64().unwrap_or(0.0) as f32, n.as_f64().unwrap_or(0.0) as f32),
        _ => d,
    }
}

fn vec3(v: &serde_json::Value) -> Option<Vec3> {
    let a = v.as_array().filter(|a| a.len() >= 3)?;
    Some(Vec3::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32))
}

fn keys4(v: &serde_json::Value) -> Vec<(f32, [f32; 4])> {
    v.as_array().into_iter().flatten().filter_map(|k| {
        let a = k.as_array()?;
        let f = |i: usize, d: f64| a.get(i).and_then(|x| x.as_f64()).unwrap_or(d) as f32;
        (a.len() >= 4).then(|| (f(0, 0.0), [f(1, 1.0), f(2, 1.0), f(3, 1.0), f(4, 1.0)]))
    }).collect()
}

impl EmitterSpec {
    /// An emitter from the extractor's JSON, or None when the port can't draw it: GPU emitters without a
    /// stand-in, distortion materials, renderers other than sprite / mesh / ribbon, meshes without a GLB.
    /// A light-only emitter comes back with no particles (`lifetime` 0) and its light.
    #[cfg(test)]
    pub(crate) fn parse(e: &serde_json::Value) -> Option<Self> {
        Self::parse_with(e, None)
    }

    /// [`EmitterSpec::parse`], drawing by the material's recovered formula when `table` has it: the stand-in's
    /// guesses (which emitters can't be drawn, colour and alpha rescaling, tints) are left to the formula then.
    pub(crate) fn parse_with(e: &serde_json::Value, table: Option<&crate::fx_material::FxTable>) -> Option<Self> {
        let sp = &e["spawn"];
        let vel = &e["velocity"];
        let col = &e["color"];
        let r = &e["renderer"];
        let light = e["light"].as_object().map(|l| {
            let c = l.get("color_initial").and_then(|c| c.as_array()).map(|a| [0, 1, 2].map(|i| a.get(i).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32)).unwrap_or([1.0, 0.7, 0.4]);
            let m = c.iter().cloned().fold(1e-6, f32::max);
            let radius = l.get("radius_cm").and_then(|x| x.as_f64()).unwrap_or(400.0) as f32;
            // UE's non-inverse-squared light falls off as (1 - (d/r)^2)^exponent: drawn with Bevy's inverse-square
            // light out to the full radius, the Thunder staff strike's (exponent 27.9) lit the whole room white.
            // The range is cut to where UE's falloff is down to 10 %.
            let exponent = l.get("exponent").and_then(|x| x.as_f64()).or(l.get("default_exponent").and_then(|x| x.as_f64())).unwrap_or(1.0) as f32;
            let inverse_squared = l.get("inverse_squared").and_then(|x| x.as_bool()).unwrap_or(true);
            let k = if inverse_squared || exponent <= 1.0 { 1.0 } else { (1.0 - 0.1f32.powf(1.0 / exponent)).sqrt() };
            (c.map(|x| (x / m).max(0.0)), 1.0, radius * k)
        });
        let kind = r["type"].as_str();
        let lifetime = pair(&e["lifetime"], (-1.0, -1.0));
        let loops = sp["lifecycle"]["final_state"].as_i64().is_none_or(|f| f == 0);
        let duration = sp["loop_duration"].as_f64().unwrap_or(0.0) as f32;
        let mut bursts: Vec<(u32, f32)> = sp["bursts"].as_array().into_iter().flatten().map(|b| (b["count"].as_u64().unwrap_or(0) as u32, b["time"].as_f64().unwrap_or(0.0) as f32)).collect();
        if loops && duration > 0.0 {
            // The extractor ticks 5 s of the system: a looping emitter's bursts repeat every loop.
            bursts.retain(|b| b.1 < duration);
        }
        let per_cm = sp["per_unit"]["per_100cm"].as_f64().unwrap_or(0.0) as f32 / 100.0;
        let mut spec = Self {
            name: e["name"].as_str().unwrap_or("").to_string(),
            rate: sp["rate"]["per_second"].as_f64().or(sp["rate"].as_f64()).unwrap_or(0.0) as f32,
            bursts,
            per_cm,
            duration,
            loops,
            lifetime,
            speed: pair(&vel["speed"], (0.0, 0.0)),
            cone: vel["cone"]["max_angle_deg"].as_f64().unwrap_or(180.0) as f32,
            direction: vec3(&vel["cone"]["axis"]).map(|d| d.normalize_or(Vec3::Z)).unwrap_or(Vec3::Z),
            radius: e["spawn_radius_cm"].as_f64().unwrap_or(0.0) as f32,
            gravity: match &e["gravity"] {
                serde_json::Value::Number(n) => Vec3::new(0.0, 0.0, n.as_f64().unwrap_or(0.0) as f32),
                v => vec3(v).unwrap_or(Vec3::ZERO),
            },
            drag: e["drag"].as_f64().unwrap_or(0.0) as f32,
            colors: Vec::new(),
            // Non-uniform (streak) sprites: their narrower axis (the streak's width), stretched by `aspect`.
            size: match (e["size"]["initial_xy"][0].as_array(), e["size"]["initial_xy"][1].as_array()) {
                (Some(x), Some(y)) if x.len() >= 2 && y.len() >= 2 => {
                    let f = |a: &serde_json::Value, b: &serde_json::Value| a.as_f64().unwrap_or(10.0).min(b.as_f64().unwrap_or(10.0)) as f32;
                    (f(&x[0], &y[0]), f(&x[1], &y[1]))
                }
                _ => pair(&e["size"]["initial"], (10.0, 10.0)),
            },
            size_over_life: e["size"]["over_life"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_f64()? as f32, k[1].as_f64()? as f32))).collect(),
            // Streak sprites scale their mean size by the relative curve instead.
            size_cm: if e["size"]["initial_xy"].is_array() { Vec::new() } else { e["size"]["over_life_cm"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_f64()? as f32, k[1].as_f64()? as f32, k[2].as_f64()? as f32))).collect() },
            // An engine texture (`DefaultDiffuse`, `DefaultTexture`) is the material's fallback, a white square:
            // the look is in the material graph, so it counts as none.
            texture: r["texture"].as_str().filter(|t| r["texture_exists"].as_bool().unwrap_or(true) && !t.contains("/Engine/Content/")).map(str::to_string),
            blend: if r["blend"].as_str().is_some_and(|b| b.eq_ignore_ascii_case("translucent") || b.eq_ignore_ascii_case("masked")) { Blend::Alpha } else { Blend::Add },
            subuv: r["subuv"].as_array().filter(|a| a.len() >= 2).map(|a| (a[0].as_u64().unwrap_or(1).max(1) as u32, a[1].as_u64().unwrap_or(1).max(1) as u32)).unwrap_or((1, 1)),
            // Niagara `bSubImageBlend`, Cascade `InterpolationMethod` linear / random blend (2, 4).
            subuv_blend: r["subuv_blend"].as_bool().unwrap_or(false) || matches!(r["subuv_interpolation"].as_i64(), Some(2) | Some(4)),
            dyns: std::array::from_fn(|k| keys4(&e["dynamic_params"][k.to_string()])),
            along: keys4(&e["color_along"]),
            // `mode` 1 = SpawnedParticles (`ENiagaraEventScriptExecutionMode`).
            events: sp["events"].as_array().into_iter().flatten().filter(|v| v["mode"].as_i64() == Some(1)).filter_map(|v| {
                let death = match v["source_event"].as_str()? {
                    "DeathEvent" => true,
                    "LocationEvent" => false,
                    _ => return None,
                };
                let n = v["spawn_number"].as_u64().unwrap_or(1) as u32;
                let lo = if v["random_spawn_number"].as_bool().unwrap_or(false) { (v["min_spawn_number"].as_u64().unwrap_or(0) as u32).min(n) } else { n };
                Some(EventSpawn { source_name: v["source_emitter"].as_str()?.to_string(), source: usize::MAX, death, count: (lo, n), per_frame: match v["max_events_per_frame"].as_u64() {
                    // Niagara's 0 is no limit (read as 1, only the first of Pyroblast's 8 flying sources left a trail).
                    Some(0) => u32::MAX,
                    n => n.unwrap_or(1) as u32,
                } })
            }).collect(),
            sends: 0,
            material: r["material"].as_str().map(str::to_string),
            exact: None,
            light: None,
            // world-space particles that `Inherit Source Movement` follow the effect too (amount 1, HYPOTHESIS).
            local: e["local_space"].as_bool().unwrap_or(false) || e["inherit_source_movement"].as_bool().unwrap_or(false),
            seek_end: e["notes"].as_array().is_some_and(|n| n.iter().any(|x| x.as_str().is_some_and(|x| x.contains("User.BeamEndBone")))),
            facing: r["facing"].as_u64().unwrap_or(0) as u8,
            align: r["alignment"].as_u64().unwrap_or(0) as u8,
            facing_vec: vec3(&e["sprite_facing"]).and_then(Vec3::try_normalize),
            align_vec: vec3(&e["sprite_alignment"]).and_then(Vec3::try_normalize),
            spin: pair(&e["rotation_deg"], (0.0, 0.0)),
            // Width (along the sprite's right) and height (its up; along the velocity when aligned) from the mean size.
            aspect: match (e["size"]["initial_xy"][0][2].as_f64(), e["size"]["initial_xy"][1][2].as_f64()) {
                (Some(x), Some(y)) if x > 1e-3 && y > 1e-3 => Vec2::new(x as f32, y as f32) / x.min(y) as f32,
                _ => Vec2::ONE,
            },
            draw: Draw::Sprite,
        };
        // An event source without a renderer (`parse_library_with` marks it): simulated, not drawn.
        if kind.is_none() && e["_event_source"].as_bool().unwrap_or(false) {
            spec.lifetime = (lifetime.0.max(0.02), lifetime.1.max(0.02));
            spec.draw = Draw::Hidden;
            return Some(spec);
        }
        if kind.is_none() && light.is_some() {
            spec.lifetime = (lifetime.1.max(0.0), lifetime.1.max(0.0));
            spec.light = light;
            return Some(spec);
        }
        // Screen-space distortion bends what's behind it; drawn, it's a visible square (HYPOTHESIS). No
        // texture: a procedural material the port can't rebuild (drawn, a solid square or shape). An
        // emitter named for refraction is a distortion whatever its material, and a modulate blend
        // darkens the scene behind it (the port has no multiply pass for it): left out (HYPOTHESIS).
        if table.zip(spec.material.as_ref()).is_some_and(|(t, m)| t.hidden(m)) {
            return None;
        }
        // Dev (checks): `DB_FX_ONLY=<text>` draws only the emitters whose material path contains it (and keeps the
        // material-less ones, the event sources others spawn from).
        if table.is_some()
            && let Ok(only) = std::env::var("DB_FX_ONLY")
            && spec.material.as_deref().is_some_and(|m| !m.contains(&only))
        {
            return None;
        }
        // `DB_FX_HIDE=<text>` leaves those out instead (keeps what spawns them by event).
        if table.is_some()
            && let Ok(hide) = std::env::var("DB_FX_HIDE")
            && spec.material.as_deref().is_some_and(|m| m.contains(&hide))
        {
            return None;
        }
        let exact = table.and_then(|t| spec.material.as_ref().and_then(|m| t.exact.get(m))).cloned();
        let lname = spec.name.to_ascii_lowercase();
        if exact.is_some() && lifetime.0 < 0.0 {
            return None;
        }
        if exact.is_none() && (r["distortion"].as_bool().unwrap_or(false) || lifetime.0 < 0.0 || spec.texture.is_none() || lname.starts_with("refr") || lname.contains("distort") || r["blend"] == "modulate") {
            return None;
        }
        spec.draw = match kind {
            // A "pattern" texture is a picture the material maps in screen space (the portal's view), not a
            // sprite (HYPOTHESIS).
            Some("Sprite") if exact.is_some() || !spec.texture.as_deref().is_some_and(|t| t.contains("TX_vfx_pattern_")) => Draw::Sprite,
            // A mesh without a particle colour is drawn by its material alone (a heat shell, a refraction
            // sphere: HYPOTHESIS); the port can't, and white it would hide the effect.
            Some("Mesh") if exact.is_none() && col["over_life"].as_array().is_none_or(|a| a.is_empty()) && col["initial"].is_null() => return None,
            Some("Mesh") => {
                let glb = r["mesh_glb"].as_array().into_iter().flatten().find_map(|g| g.as_str())?.to_string();
                // UE scale (X, Y, Z) on Bevy axes (X, Z, Y), as `to_bevy`.
                let b = |x: f32, y: f32, z: f32| Vec3::new(x, z, y);
                let mut scale: Vec<(f32, Vec3)> = e["mesh_scale_over_life"].as_array().into_iter().flatten().filter_map(|k| {
                    let a = k.as_array()?;
                    Some((a.first()?.as_f64()? as f32, b(a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32, a.get(3)?.as_f64()? as f32)))
                }).collect();
                if scale.is_empty() {
                    let m = |i: usize| e["mesh_scale"][i][2].as_f64().unwrap_or(1.0) as f32;
                    scale.push((0.0, b(m(0), m(1), m(2))));
                }
                if scale.iter().all(|s| s.1.max_element() < 1e-3) {
                    return None;
                }
                Draw::Mesh { glb, scale, face_camera: matches!(r["facing"].as_i64(), Some(2) | Some(3)) }
            }
            Some("Ribbon") => {
                let mut width: Vec<(f32, f32)> = e["ribbon_width_over_life"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_f64()? as f32, k[1].as_f64()? as f32))).collect();
                if width.is_empty() {
                    width.push((0.0, e["ribbon_width"][2].as_f64().unwrap_or(10.0) as f32));
                }
                Draw::Ribbon { width }
            }
            _ => return None,
        };
        // With the material's formula the particle colour goes in as the game's (alpha is opacity there).
        if let Some(x) = exact {
            let mut colors = keys4(&col["over_life"]);
            if colors.is_empty() {
                colors = keys4(&col["over_life_multiplier"]);
            }
            if colors.is_empty() {
                let c = col["initial"].as_array().map(|a| [0, 1, 2, 3].map(|i| a.get(i).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32)).unwrap_or([1.0; 4]);
                colors = vec![(0.0, c), (1.0, c)];
            }
            spec.colors = colors;
            spec.blend = if x.blend == crate::fx_material::FxBlend::Additive { Blend::Add } else { Blend::Alpha };
            spec.exact = Some(x);
            return Some(spec);
        }
        // Particle alpha is often a material parameter rather than opacity (blood 2.0, glows 0.001):
        // brightness from the colour, the fade shape from alpha relative to its peak (HYPOTHESIS).
        let mut colors = keys4(&col["over_life"]);
        if colors.is_empty() {
            colors = keys4(&col["over_life_multiplier"]);
        }
        if colors.is_empty() {
            let c = col["initial"].as_array().map(|a| [0, 1, 2, 3].map(|i| a.get(i).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32)).unwrap_or([1.0; 4]);
            colors = vec![(0.0, c), (1.0, [c[0], c[1], c[2], 0.0])];
        }
        // The material's main colour multiplies the particle colour (blood: white particles, dark red
        // `MainColor`).
        let tint = ["MainColor", "Color", "BaseColor", "Tint", "Main_Color"].iter().find_map(|k| r["vector_params"][*k].as_array()).map(|a| [0, 1, 2].map(|i| a.get(i).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32));
        // A smoke material's `Shadow` colour darkens a white particle (the Pyroblast hit's 4 m fire smoke
        // would be a white fog; HYPOTHESIS: the material lerps from Shadow by the texture, so about twice it).
        let shadow = r["vector_params"]["Shadow"].as_array().map(|a| [0, 1, 2].map(|i| (a.get(i).and_then(|x| x.as_f64()).unwrap_or(0.5) as f32 * 2.0).min(1.0)));
        let tint = match (tint, shadow) {
            (None, Some(s)) if colors.iter().all(|c| c.1[..3].iter().all(|x| *x >= 0.9)) => Some(s),
            (t, _) => t,
        };
        if let Some(t) = tint {
            for c in &mut colors {
                for (x, k) in c.1.iter_mut().zip(t) {
                    *x *= k;
                }
            }
        }
        let peak = colors.iter().map(|c| c.1[3]).fold(0.0, f32::max);
        // A translucent mesh or ribbon with an HDR colour is an emissive glow in its material (fire shells,
        // flame cones); alpha-blended it reads as a solid shape: drawn additively (HYPOTHESIS).
        if spec.draw != Draw::Sprite && colors.iter().any(|c| c.1[..3].iter().any(|x| *x > 1.0)) {
            spec.blend = Blend::Add;
        }
        // A colour that stays (near) black is a material-driven effect the port can't rebuild (a
        // refraction ring, a glow lit in the material); drawn it would be a black shape (HYPOTHESIS).
        if colors.iter().all(|c| c.1[..3].iter().all(|x| *x < 0.02)) {
            if spec.draw != Draw::Sprite {
                return None;
            }
            spec.blend = Blend::Add;
        }
        // Alpha above 1 or a tiny peak (a glow's 0.001) is a material parameter: shape relative to the peak;
        // a peak between those is opacity as it is (a smoke at 0.1 stays faint; HYPOTHESIS). Ribbon
        // materials scale their flow textures by it (Pyroblast's trails peak at 0.14-0.23): relative.
        // Additive glows go a little higher (the fire hit's 1.6 m flash peaks at 0.058 with colour 25).
        let tiny = if spec.blend == Blend::Add { 0.1 } else { 0.05 };
        // Alpha-blended ribbons keep it as opacity (a sword slash at 0.35 is a light streak, not a band).
        let relative = peak > 1.0 || peak < tiny || (matches!(spec.draw, Draw::Ribbon { .. }) && spec.blend == Blend::Add);
        for c in &mut colors {
            c.1[3] = if !relative { c.1[3].clamp(0.0, 1.0) } else if peak > 0.0 { (c.1[3] / peak).clamp(0.0, 1.0) } else { 1.0 };
            let m = c.1[..3].iter().cloned().fold(0.0, f32::max);
            let k = if m > MAX_BRIGHTNESS { MAX_BRIGHTNESS / m } else { 1.0 };
            for x in &mut c.1[..3] {
                *x = (*x * k).max(0.0);
            }
        }
        spec.colors = colors;
        Some(spec)
    }

    /// Colour (linear RGBA) at life fraction `t`.
    pub(crate) fn color_at(&self, t: f32) -> [f32; 4] {
        sample(&self.colors, t, [1.0; 4], |a, b, f| [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * f))
    }

    /// Size in cm at life fraction `t` for a particle at `r` (0..1) between the emitter's smallest and
    /// largest.
    pub(crate) fn size_at(&self, t: f32, r: f32) -> f32 {
        if !self.size_cm.is_empty() {
            let (lo, hi) = sample(&self.size_cm.iter().map(|k| (k.0, (k.1, k.2))).collect::<Vec<_>>(), t, (10.0, 10.0), |a, b, f| (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f));
            return lo + (hi - lo) * r;
        }
        (self.size.0 + (self.size.1 - self.size.0) * r) * sample(&self.size_over_life, t, 1.0, |a, b, f| a + (b - a) * f)
    }

    /// Dynamic material parameter `k` at life fraction `t` (zero without one).
    pub(crate) fn dyn_at(&self, k: usize, t: f32) -> [f32; 4] {
        sample(&self.dyns[k], t, [0.0; 4], |a, b, f| [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * f))
    }

    /// A beam's colour multiplier at fraction `f` along it.
    pub(crate) fn along_at(&self, f: f32) -> [f32; 4] {
        sample(&self.along, f, [1.0; 4], |a, b, k| [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * k))
    }

    /// Which dynamic parameters the emitter writes (bits 1, 2, 4, 8: the formulas' validity mask).
    pub(crate) fn dyn_valid(&self) -> u32 {
        (0..4).filter(|k| !self.dyns[*k].is_empty()).map(|k| 1 << k).sum()
    }

    /// A mesh particle's scale (Bevy axes) at life fraction `t`.
    pub(crate) fn mesh_scale_at(&self, t: f32) -> Vec3 {
        match &self.draw {
            Draw::Mesh { scale, .. } => sample(scale, t, Vec3::ONE, |a, b, f| a.lerp(b, f)),
            _ => Vec3::ONE,
        }
    }

    /// A ribbon's width in cm at life fraction `t`.
    pub(crate) fn ribbon_width_at(&self, t: f32) -> f32 {
        match &self.draw {
            Draw::Ribbon { width } => sample(width, t, 10.0, |a, b, f| a + (b - a) * f),
            _ => 0.0,
        }
    }
}

/// Piecewise-linear key lookup.
fn sample<T: Copy>(keys: &[(f32, T)], t: f32, default: T, lerp: impl Fn(T, T, f32) -> T) -> T {
    let Some(first) = keys.first() else { return default };
    if t <= first.0 {
        return first.1;
    }
    for w in keys.windows(2) {
        if t <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-6);
            return lerp(w[0].1, w[1].1, (t - w[0].0) / span);
        }
    }
    keys[keys.len() - 1].1
}

/// The effect library and the shared render pieces.
#[derive(Resource, Default)]
pub struct EffectLibrary {
    effects: HashMap<String, Vec<EmitterSpec>>,
    /// Per (effect, emitter): its material.
    materials: HashMap<(String, usize), Handle<FxMaterial>>,
    /// Mesh particles' meshes by GLB path, and their vertices once loaded (the batches copy them per particle).
    meshes: HashMap<String, Handle<Mesh>>,
    mesh_data: HashMap<String, std::sync::Arc<MeshData>>,
    /// The effects that lost a mesh emitter (its GLB isn't converted: the Cyclops' rock).
    lacking_mesh: std::collections::HashSet<String>,
}

impl EffectLibrary {
    pub(crate) fn has(&self, key: &str) -> bool {
        self.effects.contains_key(key)
    }

    /// Whether `key` lost a mesh emitter (so a body it would draw is missing).
    pub(crate) fn lacks_mesh(&self, key: &str) -> bool {
        self.lacking_mesh.contains(key)
    }
}

/// The effects with an enabled mesh emitter whose GLB is missing or not imported.
fn lacking_meshes(v: &serde_json::Value) -> std::collections::HashSet<String> {
    let found = |e: &serde_json::Value| e["renderer"]["mesh_glb"].as_array().into_iter().flatten().filter_map(|g| g.as_str()).any(|g| crate::map_view::data_file(&format!("assets/{g}")).is_some());
    v.as_object().into_iter().flatten().filter(|(_, fx)| fx["emitters"].as_array().into_iter().flatten().any(|e| e["enabled"].as_bool().unwrap_or(true) && e["renderer"]["type"] == "Mesh" && !found(e))).map(|(k, _)| k.clone()).collect()
}

/// Dev viewer: plays one effect in front of the test-room camera, re-triggering one-shots (`--effect`).
#[derive(Resource, Debug, Default, Clone)]
pub struct ShowEffect(pub Option<String>);

/// Test room: the `--effect` effect in front of the spawn view, one-shots again every 0.5 s (looping
/// ones once; they sweep to and fro so trail emitters spawn).
#[allow(clippy::too_many_arguments)]
pub(crate) fn show_effect(mut commands: Commands, lib: Res<EffectLibrary>, show: Res<ShowEffect>, look: Res<crate::Look>, time: Res<Time>, mut next_at: Local<f32>, mut looping: Local<Option<Entity>>, mut tf: Query<&mut Transform>) {
    let Some(key) = show.0.as_deref() else { return };
    if !lib.has(key) {
        if *next_at == 0.0 {
            error!("--effect {key}: no such effect in _data/recovered/effects/effects.json");
            *next_at = f32::INFINITY;
        }
        return;
    }
    let now = time.elapsed_secs();
    // Along the camera's look (`--look <deg>` turns it off the training dummy, which stands 1.85 m ahead of the
    // spawn view): bursts and casts 3 m ahead at chest height, a little right of centre (the shield covers the
    // left, the sword the right edge), effects on the floor there too (3 m out the floor is in view).
    let spawn = db_sim::level::TEST_ROOM_SPAWN;
    let ahead = Vec3::new(look.yaw.cos(), look.yaw.sin(), 0.0);
    let right = Vec3::new(-ahead.y, ahead.x, 0.0);
    let lower = key.to_ascii_lowercase();
    let burst = ["hit_", "cast", "bullet", "spell", "missile", "projectile"].iter().any(|k| lower.contains(k));
    let at = if burst { spawn + ahead * 300.0 + right * 80.0 + Vec3::Z * 20.0 } else { spawn + ahead * 300.0 + right * 80.0 - Vec3::Z * 90.0 };
    let trail = ["trail", "missile", "projectile", "bullet"].iter().any(|k| lower.contains(k));
    if let Some(e) = *looping {
        // Trails spawn per distance: the effect swings left and right (up to 4.5 m/s); the rest stand still.
        if trail && let Ok(mut t) = tf.get_mut(e) {
            let x = (now * 2.0).sin() * 1.5;
            t.translation = to_bevy(at + right * 150.0 * x);
        }
        return;
    }
    if now < *next_at {
        return;
    }
    // Hit bursts aim their local +X back at the viewer; others point +X to the right (trails fly across).
    // Projectile systems point their flight along local +Z (see `skill_vfx`).
    let rot = if key.starts_with("hit_") { Quat::from_rotation_arc(Vec3::X, -ahead) } else if lower.contains("bullet") { Quat::from_rotation_arc(Vec3::Z, right) } else { Quat::from_rotation_arc(Vec3::X, right) };
    let e = spawn_effect(&mut commands, &lib, key, to_bevy(at), rot, None);
    if lib.effects[key].iter().any(|s| s.loops) && !key.contains("hit") {
        *looping = e;
    }
    *next_at = now + 0.5;
}

/// A playing effect at its entity's position (world, or following a parent).
#[derive(Component, Debug)]
pub(crate) struct Effect {
    key: String,
    pub(crate) age: f32,
    /// Stop spawning after this many seconds (None: as long as the emitters say, looping ones forever).
    until: Option<f32>,
    /// Per emitter: rate and distance accumulators.
    acc: Vec<f32>,
    dist: Vec<f32>,
    /// UE-space rotation of the emitters' frame (hit bursts point local +X out of the surface).
    rot: Quat,
    seed: u32,
    /// Its global transform is valid (one frame after spawning).
    placed: bool,
    /// Where it was last frame (Bevy space), for per-distance spawning.
    last: Option<Vec3>,
    /// Per emitter: its ribbon entity, once it has spawned one.
    ribbons: Vec<Option<Entity>>,
    /// Per emitter: its particle batch, once it has spawned one.
    batches: Vec<Option<Entity>>,
    /// Uniform scale (a placed component's `RelativeScale3D`, the escape circles' 0.3): particle sizes, speeds,
    /// gravity, spawn radius and light range (ribbon widths aren't scaled).
    scale: f32,
}

/// One live particle (Bevy space: world, or its effect's frame for a local emitter).
#[derive(Debug, Clone, Copy)]
struct Part {
    pos: Vec3,
    vel: Vec3,
    accel: Vec3,
    drag: f32,
    age: f32,
    life: f32,
    /// Place between the emitter's smallest and largest size (0..1).
    size: f32,
    /// Its own random number (0..1), for the material.
    rand: f32,
    /// Bevy-space rotation of the effect at spawn (mesh particles keep it).
    rot: Quat,
    /// Its effect's scale.
    scale: f32,
    /// Sprite rotation, radians.
    spin: f32,
}

/// One emitter's particles of one playing effect, drawn as one mesh rebuilt every frame. It outlives its effect
/// until its particles have died (a shot's smoke stays after the hit); local particles go with the effect.
#[derive(Component, Debug)]
pub(crate) struct Batch {
    key: (String, usize),
    parts: Vec<Part>,
    owner: Entity,
    local: bool,
    mesh: Handle<Mesh>,
}

/// A mesh particle's vertices, copied once its GLB has loaded.
#[derive(Debug)]
pub(crate) struct MeshData {
    pos: Vec<Vec3>,
    nor: Vec<Vec3>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    center: Vec3,
    half: Vec3,
}

impl MeshData {
    fn from_mesh(m: &Mesh) -> Option<Self> {
        use bevy::mesh::VertexAttributeValues as V;
        let Some(V::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
        let pos: Vec<Vec3> = p.iter().map(|v| Vec3::from_array(*v)).collect();
        let nor = match m.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(V::Float32x3(n)) => n.iter().map(|v| Vec3::from_array(*v)).collect(),
            _ => vec![Vec3::Z; pos.len()],
        };
        let uv = match m.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(V::Float32x2(u)) => u.clone(),
            _ => vec![[0.0; 2]; pos.len()],
        };
        let idx: Vec<u32> = match m.indices() {
            Some(i) => i.iter().map(|x| x as u32).collect(),
            None => (0..pos.len() as u32).collect(),
        };
        let (lo, hi) = pos.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        Some(Self { pos, nor, uv, idx, center: (lo + hi) * 0.5, half: (hi - lo) * 0.5 })
    }
}

/// One point of a ribbon (world space).
#[derive(Debug, Clone, Copy)]
struct RibbonPoint {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    /// On a beam: its place from the start (0) to the end (1) and its fixed sideways offset (Bevy, m).
    beam: Option<(f32, Vec3)>,
}

/// An effect drawn as a beam (Niagara beam emitters, the Fire staff's laser and the thunder bolts): each
/// burst of a ribbon emitter lays its points evenly from the effect's position to `end` (Bevy space, world),
/// re-laid every frame as both move; interior points sit up to `jitter` m off the line (lightning; 0 for a
/// straight beam). Its ribbons go with it. HYPOTHESIS: the beam module's spawn positions aren't evaluated
/// (`BeamEmitterSetup` / `SetActor End` bind to the owner at run time), so even spacing is ours.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Beam {
    pub end: Vec3,
    pub jitter: f32,
}

/// A ribbon: a strip mesh rebuilt every frame through its points; outlives its effect until its points
/// have aged out.
#[derive(Component, Debug)]
pub(crate) struct Ribbon {
    key: (String, usize),
    /// Its effect (the events its points send go to its emitters).
    owner: Entity,
    points: Vec<RibbonPoint>,
    mesh: Handle<Mesh>,
    accel: Vec3,
    drag: f32,
    /// A beam's effect: the ribbon follows its line and goes when it goes.
    beam: Option<Entity>,
}

/// How often a particle sends its location events at the least (Hz; HYPOTHESIS: the game's frame rate).
const EVENT_HZ: f32 = 60.0;

/// Events particles sent this frame, spawned from on the next.
#[derive(Resource, Debug, Default)]
pub(crate) struct FxEvents(Vec<FxEvent>);

#[derive(Debug, Clone, Copy)]
struct FxEvent {
    owner: Entity,
    emitter: usize,
    death: bool,
    /// World (Bevy space).
    pos: Vec3,
}

/// A light emitter's point light: full intensity, fading over `life` unless it loops.
#[derive(Component)]
pub(crate) struct EffectLight {
    base: f32,
    life: f32,
    loops: bool,
}

/// Bevy lumens for an effect light (HYPOTHESIS: the data's light brightness is in units the port can't
/// map; one strength for all, by eye).
const LIGHT_LUMENS: f32 = 120_000.0;

/// GPU emitters keep no particle numbers in the data. Stand-ins from what survives (HYPOTHESIS): the hit
/// sparks' two bursts get hand-set values from their bounds and module list; every other GPU sprite emitter
/// with spawn data gets a generic one: life by kind (sparks short), speed so particles reach about the
/// emitter's fixed bounds, gravity and drag when the module list has them, a small size for spark and
/// particle textures, colour from its colour / alpha multiplier curves.
fn gpu_standin(key: &str, e: &mut serde_json::Value) {
    if e["sim"] != "GPU" {
        return;
    }
    if key == "hit_metal_sparks" {
        e["lifetime"] = serde_json::json!([0.3, 0.6]);
        e["velocity"] = serde_json::json!({"speed": [300.0, 800.0], "cone": {"axis": [1.0, 0.0, 0.0], "max_angle_deg": 80.0}});
        e["gravity"] = serde_json::json!(-500.0);
        e["drag"] = serde_json::json!(3.0);
        e["size"] = serde_json::json!({"initial": [1.0, 3.0]});
        e["color"] = serde_json::json!({"over_life": [[0.0, 4.0, 2.4, 0.9, 1.0], [0.6, 4.0, 2.0, 0.6, 1.0], [1.0, 2.0, 0.6, 0.1, 0.0]]});
        return;
    }
    if e["renderer"]["type"] != "Sprite" || !e["lifetime"].is_null() {
        return;
    }
    let name = e["name"].as_str().unwrap_or("").to_ascii_lowercase();
    let tex = e["renderer"]["texture"].as_str().unwrap_or("").to_ascii_lowercase();
    let mods: Vec<String> = e["gpu_modules"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(str::to_ascii_lowercase)).collect();
    let has = |m: &str| mods.iter().any(|x| x.contains(m));
    let spark = ["spark", "particle", "parts", "ember", "dot", "point"].iter().any(|k| name.contains(k) || tex.contains(k));
    let rock = name.contains("rock") || name.contains("debris");
    let life: (f32, f32) = if spark { (0.3, 0.9) } else { (0.5, 1.4) };
    // Niagara's default fixed bounds (+-100 cm) say nothing about the emitter's size: then the particles stay
    // within 30 cm (HYPOTHESIS: the charge effects' 150-300 spark bursts at the first-person hand flew
    // through the camera as large soft discs).
    let half = e["fixed_bounds_cm"].as_array().map(|b| b.iter().flat_map(|c| c.as_array().into_iter().flatten().filter_map(|x| x.as_f64())).fold(0.0f64, |m, x| m.max(x.abs())) as f32).map_or(30.0, |h| if (h - 100.0).abs() < 1e-3 { 30.0 } else { h }).clamp(30.0, 400.0);
    let mean = (life.0 + life.1) * 0.5;
    e["lifetime"] = serde_json::json!([life.0, life.1]);
    e["velocity"] = serde_json::json!({"speed": [0.3 * half / mean, 1.2 * half / mean], "cone": {"axis": [0.0, 0.0, 1.0], "max_angle_deg": 180.0}});
    e["gravity"] = serde_json::json!(if has("gravity") || rock { -980.0 } else if spark { -200.0 } else { 0.0 });
    e["drag"] = serde_json::json!(if has("drag") { 2.0 } else { 0.0 });
    let size = if rock { [2.0, 5.0] } else if spark { [0.4, 1.2] } else { [8.0, 24.0] };
    let old = e["size"]["over_life"].clone();
    e["size"] = serde_json::json!({"initial": size, "over_life": old});
    // A colour curve module's curve (`ColorFromCurve*.ColorCurve`, rows [t, r, g, b, a]) is the particle colour
    // over life: Pyroblast's `spark_l` is HDR orange (100, 18.4, 0); left white, its sparks flew at the camera as
    // white discs.
    let curve = e["gpu_curves"].as_object().and_then(|c| c.iter().find(|(k, _)| k.ends_with(".ColorCurve")).map(|(_, v)| v.clone()));
    if let Some(rows) = curve.filter(|v| v.as_array().is_some_and(|r| r.iter().all(|x| x.as_array().is_some_and(|x| x.len() == 5)))) {
        e["color"]["over_life"] = rows;
    } else if e["color"]["over_life_multiplier"].is_null() {
        let alpha = e["color"]["alpha_multiplier"].as_array().cloned().unwrap_or_default();
        let rows: Vec<serde_json::Value> = if alpha.is_empty() {
            vec![serde_json::json!([0.0, 1.0, 1.0, 1.0, 1.0]), serde_json::json!([1.0, 1.0, 1.0, 1.0, 0.0])]
        } else {
            alpha.iter().filter_map(|r| Some(serde_json::json!([r[0].as_f64()?, 1.0, 1.0, 1.0, r[1].as_f64()?]))).collect()
        };
        e["color"]["over_life"] = serde_json::Value::Array(rows);
    }
}

/// GPU mesh emitters keep no particle numbers either (Blizzard's ice crystals). Stand-in (HYPOTHESIS): the rate
/// and bursts from the data; the mesh at its own size (the ice shard is 5 x 19 cm) times the first GPU curve that
/// rises from 0 and falls back (read as scale over life: grow, hold, shrink); the colour of a CPU emitter of the
/// same system drawing the same mesh (`Fountain005`, ice blue; its scale of 7 is one centre crystal from a single
/// burst, and on ~100 shards a second it filled the view); life 0.8-1.2 s; spawned within 150 cm of the system on
/// its floor plane; rising 50-150 cm/s within 30 degrees of up when the module list adds velocity.
fn gpu_mesh_standin(e: &mut serde_json::Value, siblings: &[serde_json::Value]) {
    if e["sim"] != "GPU" || e["renderer"]["type"] != "Mesh" || !e["lifetime"].is_null() {
        return;
    }
    let glb = e["renderer"]["mesh_glb"].clone();
    let twin = siblings.iter().find(|s| s["sim"] == "CPU" && s["renderer"]["type"] == "Mesh" && s["renderer"]["mesh_glb"] == glb && !s["lifetime"].is_null());
    let base = [1.0f64; 3];
    let curve: Vec<(f64, f64)> = e["gpu_curves"]
        .as_object()
        .into_iter()
        .flat_map(|c| c.values())
        .filter_map(|v| v.as_array())
        .map(|k| k.iter().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect::<Vec<_>>())
        .find(|k| k.len() >= 3 && k.first().is_some_and(|p| p.1.abs() < 1e-3) && k.last().is_some_and(|p| p.1.abs() < 1e-3) && k.iter().any(|p| p.1 > 0.5))
        .unwrap_or_else(|| vec![(0.0, 1.0), (1.0, 1.0)]);
    e["mesh_scale"] = serde_json::json!([base, base, base]);
    e["mesh_scale_over_life"] = serde_json::Value::Array(curve.iter().map(|(t, v)| serde_json::json!([t, base[0] * v, base[1] * v, base[2] * v])).collect());
    e["color"] = twin.map(|t| t["color"].clone()).unwrap_or_else(|| serde_json::json!({"initial": [1.0, 1.0, 1.0, 1.0], "over_life": [[0.0, 1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0, 1.0]]}));
    e["lifetime"] = serde_json::json!([0.8, 1.2]);
    e["spawn_radius_cm"] = serde_json::json!(150.0);
    let rises = e["gpu_modules"].as_array().is_some_and(|m| m.iter().any(|x| x.as_str().is_some_and(|x| x.contains("AddVelocity"))));
    e["velocity"] = serde_json::json!({"speed": if rises { [50.0, 150.0] } else { [0.0, 0.0] }, "cone": {"axis": [0.0, 0.0, 1.0], "max_angle_deg": 30.0}});
}

/// Startup: loads the effect numbers.
pub(crate) fn load_effects(mut commands: Commands, assets: Res<AssetServer>) {
    let mut lib = EffectLibrary::default();
    let table = crate::fx_material::read_fx_table(&assets);
    commands.init_resource::<ShowEffect>();
    let rel = "recovered/effects/effects.json";
    match crate::map_view::data_file(rel).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        Some(v) => {
            lib.effects = parse_library_with(&v, Some(&table));
            lib.lacking_mesh = lacking_meshes(&v);
            info!("{} effects", lib.effects.len());
        }
        None => warn!("no _data/{rel} (tools/recon/niagara_effects.py); no particle effects"),
    }
    commands.insert_resource(lib);
    commands.insert_resource(table);
}

/// One invisible draw per recovered formula, kept for [`PREWARM_SECONDS`]: Bevy builds a material's pipeline the
/// first time it draws, in the background, and skips the draw until then, so an effect's first play lost its opening
/// (the effect warm-up). These make every formula's pipeline before an effect needs it.
#[derive(Component)]
pub(crate) struct FxPrewarm(f32);

/// How long each prewarm draw stays (s, real time; ours: a debug build builds the ~100 pipelines in a few seconds).
const PREWARM_SECONDS: f32 = 30.0;

/// Prewarm draws started per batch, and the time between batches (s). All 641 formulas' pipelines queued in one
/// frame stalled the game for about 100 s while the arena's models loaded, and queued before the level they kept its
/// own materials (floor, monsters, arms) waiting to be built: grey with the UI for minutes. They start
/// [`PREWARM_AFTER`] s after a level's models are in, a few at a time.
const PREWARM_BATCH: usize = 2;
const PREWARM_EVERY: f32 = 0.25;
const PREWARM_AFTER: f32 = 3.0;

/// Pipelines the render world's cache is still building (queued, compiling or waiting on a shader), counted there
/// every frame. The prewarm adds its next batch only when no more than a batch is waiting: queued all at the rate
/// above, its draws piled up faster than they compiled, and an effect first played meanwhile waited behind the pile
/// and drew nothing for a minute or more.
#[derive(Resource, Clone, Default)]
pub(crate) struct PendingPipelines(std::sync::Arc<std::sync::atomic::AtomicUsize>);

/// Shares [`PendingPipelines`] with the render world and counts it there (after the cache's queue is processed).
pub(crate) fn watch_pipelines(app: &mut App) {
    let pending = PendingPipelines::default();
    app.insert_resource(pending.clone());
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render.insert_resource(pending).add_systems(bevy::render::Render, count_pending.in_set(bevy::render::RenderSystems::Cleanup));
    }
}

fn count_pending(cache: Res<bevy::render::render_resource::PipelineCache>, pending: Res<PendingPipelines>) {
    pending.0.store(cache.waiting_pipelines().count(), std::sync::atomic::Ordering::Relaxed);
}

/// The prewarm's progress: formulas still to start, and how many there are in all.
#[derive(Resource, Default)]
pub(crate) struct FxPrewarmQueue {
    pub(crate) left: Vec<(u32, bool)>,
    pub(crate) total: usize,
    wait: f32,
}

/// Starts the prewarm draws a batch at a time once the formulas are read; despawns each after [`PREWARM_SECONDS`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn prewarm_fx(mut commands: Commands, table: Option<Res<crate::fx_material::FxTable>>, assets: Res<AssetServer>, (mut meshes, mut mats): (ResMut<Assets<Mesh>>, ResMut<Assets<FxMaterial>>), time: Res<Time<Real>>, mut live: Query<(Entity, &mut FxPrewarm)>, mut queue: ResMut<FxPrewarmQueue>, mut mesh: Local<Option<Handle<Mesh>>>, mut done: Local<bool>, (screen, models, pending): (Res<State<crate::flow::Screen>>, Query<&crate::model_view::ModelBody>, Option<Res<PendingPipelines>>)) {
    for (e, mut p) in &mut live {
        p.0 += time.delta_secs();
        if p.0 >= PREWARM_SECONDS {
            commands.entity(e).despawn();
        }
    }
    let Some(table) = table else { return };
    if !*done {
        *done = true;
        // Dev: `DB_NO_PREWARM` leaves it out ('s before/after shots).
        if std::env::var("DB_NO_PREWARM").is_ok() {
            return;
        }
        let mut seen = std::collections::HashSet::new();
        queue.left = table.exact.values().filter(|x| seen.insert((x.perm, x.lit))).map(|x| (x.perm, x.lit)).collect();
        queue.left.reverse();
        queue.total = queue.left.len();
        info!("effect pipelines: {} to prewarm, {PREWARM_BATCH} every {PREWARM_EVERY} s", queue.total);
    }
    if queue.left.is_empty() {
        return;
    }
    // Not before a level is up and its models are in (their pipelines go first).
    use crate::flow::Screen;
    if matches!(screen.get(), Screen::Boot | Screen::Title | Screen::Stash | Screen::Result) || !models.iter().all(|m| m.loaded(&assets)) {
        queue.wait = queue.wait.max(PREWARM_AFTER);
        return;
    }
    queue.wait -= time.delta_secs();
    if queue.wait > 0.0 || pending.is_some_and(|p| p.0.load(std::sync::atomic::Ordering::Relaxed) > PREWARM_BATCH) {
        return;
    }
    queue.wait = PREWARM_EVERY;
    let mesh = mesh.get_or_insert_with(|| meshes.add(batch_mesh(Verts::default()))).clone();
    for _ in 0..PREWARM_BATCH {
        let Some((perm, lit)) = queue.left.pop() else { break };
        let Some(x) = table.exact.values().find(|x| x.perm == perm && x.lit == lit) else { continue };
        let textures = x.textures.iter().map(|t| crate::fx_material::load_fx_texture(&assets, t)).collect();
        let m = mats.add(FxMaterial { lit: x.lit, ..FxMaterial::new(x.blend, x.perm, 0, textures) });
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), NoFrustumCulling, bevy::light::NotShadowCaster, FxPrewarm(0.0)));
    }
    if queue.left.is_empty() {
        info!("effect pipelines: all {} prewarm draws started", queue.total);
    }
}

/// Every effect with at least one emitter the port draws (mesh emitters whose GLB isn't imported are
/// dropped).
#[cfg(test)]
pub(crate) fn parse_library(v: &serde_json::Value) -> HashMap<String, Vec<EmitterSpec>> {
    parse_library_with(v, None)
}

/// [`parse_library`] with the recovered material formulas.
pub(crate) fn parse_library_with(v: &serde_json::Value, table: Option<&crate::fx_material::FxTable>) -> HashMap<String, Vec<EmitterSpec>> {
    let mut out = HashMap::new();
    for (k, fx) in v.as_object().into_iter().flatten() {
        let mut standins = Vec::new();
        let siblings: Vec<serde_json::Value> = fx["emitters"].as_array().cloned().unwrap_or_default();
        // Emitters other emitters' event handlers name.
        let sources: std::collections::HashSet<&str> = siblings.iter().flat_map(|e| e["spawn"]["events"].as_array().into_iter().flatten().filter_map(|v| v["source_emitter"].as_str())).collect();
        let mut em: Vec<EmitterSpec> = fx["emitters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["enabled"].as_bool().unwrap_or(true))
            .filter_map(|e| {
                let mut e = e.clone();
                if e["name"].as_str().is_some_and(|n| sources.contains(n)) {
                    e["_event_source"] = serde_json::Value::Bool(true);
                }
                let gpu = e["sim"] == "GPU" && e["lifetime"].is_null();
                // A GPU emitter on the engine's default sprite material is all stand-in (colour, size, life): drawn,
                // white puffs at the staff grip while charging Pyroblast. The CPU ones carry their data.
                if gpu && e["renderer"]["material"].as_str().is_some_and(|m| m.ends_with("/DefaultSpriteMaterial")) {
                    return None;
                }
                gpu_standin(k, &mut e);
                gpu_mesh_standin(&mut e, &siblings);
                EmitterSpec::parse_with(&e, table).map(|s| (s, gpu))
            })
            .filter(|(s, _)| match &s.draw {
                Draw::Mesh { glb, .. } => crate::map_view::data_file(&format!("assets/{glb}")).is_some(),
                _ => true,
            })
            .enumerate()
            .map(|(i, (s, gpu))| {
                if gpu {
                    standins.push(i);
                }
                s
            })
            .collect();
        // A GPU stand-in whose colour curves carry no hue (white) takes the hue of the system's brightest
        // decoded emitter (sparks of a fire burst are orange; HYPOTHESIS).
        let hue = em.iter().enumerate().filter(|(i, s)| !standins.contains(i) && s.light.is_none()).map(|(_, s)| s.color_at(0.0)).filter(|c| c[0].max(c[1]).max(c[2]) > 0.05).max_by(|a, b| a[0].max(a[1]).max(a[2]).total_cmp(&b[0].max(b[1]).max(b[2])));
        if let Some(h) = hue {
            let m = h[0].max(h[1]).max(h[2]);
            for &i in &standins {
                let s = &mut em[i];
                if s.colors.iter().all(|c| (c.1[0] - c.1[1]).abs() < 0.05 && (c.1[1] - c.1[2]).abs() < 0.05) {
                    for c in &mut s.colors {
                        for (x, k) in c.1[..3].iter_mut().zip(h) {
                            *x *= k / m;
                        }
                    }
                }
            }
        }
        // Event handlers find their source emitter by name; the source learns what it sends.
        let names: Vec<String> = em.iter().map(|s| s.name.clone()).collect();
        let mut sends = vec![0u8; em.len()];
        for s in &mut em {
            for v in &mut s.events {
                if let Some(j) = names.iter().position(|n| *n == v.source_name) {
                    v.source = j;
                    sends[j] |= if v.death { 2 } else { 1 };
                }
            }
            s.events.retain(|v| v.source != usize::MAX);
        }
        for (s, b) in em.iter_mut().zip(sends) {
            s.sends = b;
        }
        if !em.is_empty() {
            out.insert(k.clone(), em);
        }
    }
    out
}

/// Starts effect `key` at `pos` (Bevy space) with its emitter frame turned by `rot` (UE space). Returns
/// the entity: parent it to follow something, despawn it to stop a looping one.
pub(crate) fn spawn_effect(commands: &mut Commands, lib: &EffectLibrary, key: &str, pos: Vec3, rot: Quat, until: Option<f32>) -> Option<Entity> {
    spawn_effect_scaled(commands, lib, key, pos, rot, until, 1.0)
}

/// [`spawn_effect`] at a uniform `scale`.
pub(crate) fn spawn_effect_scaled(commands: &mut Commands, lib: &EffectLibrary, key: &str, pos: Vec3, rot: Quat, until: Option<f32>, scale: f32) -> Option<Entity> {
    let em = lib.effects.get(key)?;
    let seed = (pos.x.to_bits() ^ pos.z.to_bits().rotate_left(13) ^ pos.y.to_bits().rotate_left(7) ^ (key.len() as u32).wrapping_mul(2654435761)) | 1;
    let fx = Effect { key: key.to_string(), age: 0.0, until, acc: vec![0.0; em.len()], dist: vec![0.0; em.len()], rot, seed, placed: false, last: None, ribbons: vec![None; em.len()], batches: vec![None; em.len()], scale };
    let e = commands.spawn((fx, Transform::from_translation(pos), Visibility::default())).id();
    commands.entity(e).with_children(|c| {
        for s in em.iter() {
            let Some(([r, g, b], _, radius)) = s.light else { continue };
            c.spawn((PointLight { color: Color::linear_rgb(r, g, b), intensity: LIGHT_LUMENS, range: (radius * 0.01 * scale).clamp(1.0, 15.0), shadow_maps_enabled: false, ..default() }, EffectLight { base: LIGHT_LUMENS, life: s.lifetime.1.max(0.1), loops: s.loops }));
        }
    });
    Some(e)
}

/// Scales an effect's lights by `k` (an effect in the first-person hand: a full light 10 cm from the
/// arm blows it out white).
pub(crate) fn dim_lights(commands: &mut Commands, e: Entity, k: f32) {
    commands.queue(move |w: &mut World| {
        let kids: Vec<Entity> = w.get::<Children>(e).map(|c| c.iter().collect()).unwrap_or_default();
        for c in kids {
            if let Some(mut l) = w.get_mut::<EffectLight>(c) {
                l.base *= k;
            }
            if let Some(mut l) = w.get_mut::<PointLight>(c) {
                l.intensity *= k;
            }
        }
    });
}

/// Stops an effect (or any entity) if it still exists.
pub(crate) fn stop(commands: &mut Commands, e: Entity) {
    if let Ok(mut c) = commands.get_entity(e) {
        c.try_despawn();
    }
}

/// Cheap deterministic per-effect random numbers (presentation only, not the sim RNG).
fn next(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32) / (u32::MAX as f32)
}

/// A unit vector within `cone` degrees of `dir`.
fn in_cone(dir: Vec3, cone: f32, seed: &mut u32) -> Vec3 {
    let cos_max = cone.clamp(0.0, 180.0).to_radians().cos();
    let z = cos_max + (1.0 - cos_max) * next(seed);
    let phi = next(seed) * std::f32::consts::TAU;
    let s = (1.0 - z * z).max(0.0).sqrt();
    let local = Vec3::new(s * phi.cos(), s * phi.sin(), z);
    Quat::from_rotation_arc(Vec3::Z, dir.normalize_or(Vec3::Z)) * local
}

/// A UE-space rotation as a Bevy-space one (`to_bevy` swaps Y and Z, a mirror: the axis is swapped and
/// the angle flips).
pub(crate) fn quat_to_bevy(q: Quat) -> Quat {
    Quat::from_xyzw(-q.x, -q.z, -q.y, q.w)
}

/// A sprite's right, up and normal (Bevy space) by its renderer's facing and alignment modes (Niagara's sprite vertex
/// factory: the custom facing vector is the quad's normal, the alignment vector its up, and the rotation turns both
/// about the normal). `rot` takes the emitter's frame to the world, `vel` is the world velocity, `to_eye` the
/// particle-to-camera vector, `cam` the camera's right, up and back. Modes 0, 1 and 4 face the camera plane, as does
/// a custom facing without its vector; a zero velocity, a missing alignment vector or one along the normal leaves the
/// sprite unaligned instead of collapsing it.
fn sprite_axes(spec: &EmitterSpec, rot: Quat, vel: Vec3, to_eye: Option<Vec3>, cam: (Vec3, Vec3, Vec3), spin: f32) -> (Vec3, Vec3, Vec3) {
    let (cam_right, cam_up, cam_back) = cam;
    let world = |v: Vec3| (rot * to_bevy(v)).normalize_or(Vec3::Y);
    let eye = to_eye.and_then(|d| d.try_normalize()).unwrap_or(cam_back);
    let custom = spec.facing_vec.filter(|_| spec.facing == 2).map(world);
    let (mut n, mut r, mut u) = match (spec.facing, custom) {
        (_, Some(n)) => {
            // The camera's right laid into the sprite's plane (its up when looking straight along the plane).
            let r = (cam_right - n * cam_right.dot(n)).try_normalize().or_else(|| (cam_up - n * cam_up.dot(n)).try_normalize()).unwrap_or(Vec3::X);
            (n, r, n.cross(r))
        }
        (3, _) => {
            let r = Vec3::Y.cross(eye).try_normalize().unwrap_or(cam_right);
            (eye, r, eye.cross(r))
        }
        _ => (cam_back, cam_right, cam_up),
    };
    let along = match spec.align {
        1 => vel.try_normalize(),
        2 => spec.align_vec.map(world),
        _ => None,
    };
    if let Some(a) = along {
        let face = if custom.is_some() { n } else { eye };
        if let Some(ar) = a.cross(face).try_normalize() {
            (r, u) = (ar, a);
            n = ar.cross(a);
        }
    }
    if spin != 0.0 {
        let (s, c) = spin.sin_cos();
        (r, u) = (r * c + u * s, u * c - r * s);
    }
    (r, u, n)
}

/// Particles an emitter spawns between effect ages `prev` and `age` (rate, bursts with loops repeating,
/// and `moved` cm of travel for per-distance emitters).
fn spawn_count(s: &EmitterSpec, acc: &mut f32, dist: &mut f32, prev: f32, age: f32, until: Option<f32>, moved: f32) -> u32 {
    let stop = until.unwrap_or(if s.loops { f32::INFINITY } else { s.duration.max(0.0) });
    if prev >= stop || s.light.is_some() {
        return 0;
    }
    let mut n = 0;
    if s.rate > 0.0 {
        *acc += s.rate * (age.min(stop) - prev);
        n += acc.floor() as u32;
        *acc = acc.fract();
    }
    if s.per_cm > 0.0 {
        *dist += s.per_cm * moved;
        n += dist.floor().min(64.0) as u32;
        *dist = dist.fract();
    }
    for &(count, t) in &s.bursts {
        let fired = if s.loops && s.duration > 0.0 {
            // Burst times repeat every loop: count the loop starts crossed in (prev, age].
            let k0 = ((prev - t) / s.duration).floor();
            let k1 = ((age - t) / s.duration).floor();
            if prev == 0.0 && t <= 0.0 { (k1 + 1.0).max(0.0) as u32 } else { (k1 - k0).max(0.0) as u32 }
        } else {
            u32::from((prev < t || (prev == 0.0 && t <= 0.0)) && t <= age)
        };
        n += fired * count;
    }
    n
}

/// When a playing effect has nothing left to spawn or show (None while it loops).
fn end_time(specs: &[EmitterSpec], until: Option<f32>) -> Option<f32> {
    let mut end: f32 = 0.0;
    for s in specs {
        let last = match until {
            Some(u) => u,
            None if s.loops => return None,
            None => s.duration.max(s.bursts.iter().map(|b| b.1).fold(0.0, f32::max)),
        };
        end = end.max(last + s.lifetime.1.max(0.0));
    }
    Some(end)
}

fn material_for(lib: &mut EffectLibrary, mats: &mut Assets<FxMaterial>, assets: &AssetServer, key: &str, i: usize) -> Handle<FxMaterial> {
    if let Some(m) = lib.materials.get(&(key.to_string(), i)) {
        return m.clone();
    }
    let spec = &lib.effects[key][i];
    if let Some(x) = &spec.exact {
        let textures = x.textures.iter().map(|t| crate::fx_material::load_fx_texture(assets, t)).collect();
        let m = mats.add(FxMaterial { lit: x.lit, ..FxMaterial::new(x.blend, x.perm, spec.dyn_valid(), textures) });
        lib.materials.insert((key.to_string(), i), m.clone());
        return m;
    }
    // The sprite copies (masked / faded), else the original texture. Meshes and ribbons map their
    // texture over their own UVs: the alpha copy without the radial fade (their shape comes from the
    // geometry).
    let name = spec.texture.as_ref().and_then(|t| t.rsplit('/').next());
    let copy = name.map(|n| match (spec.blend, &spec.draw) {
        (Blend::Alpha, Draw::Sprite) => format!("generated/effects/{n}"),
        (Blend::Add, Draw::Sprite) => format!("generated/effects/add_{n}"),
        (Blend::Alpha, _) => format!("generated/effects/geo_{n}"),
        (Blend::Add, _) => format!("generated/effects/geo_add_{n}"),
    });
    let path = copy.filter(|c| crate::map_view::data_file(&format!("assets/{c}")).is_some()).or_else(|| spec.texture.clone().filter(|t| crate::map_view::data_file(&format!("assets/{t}")).is_some()));
    let tex: Option<Handle<Image>> = path.map(|t| assets.load(t));
    let blend = if spec.blend == Blend::Add { crate::fx_material::FxBlend::Additive } else { crate::fx_material::FxBlend::Alpha };
    let m = mats.add(FxMaterial::new(blend, 0, 0, vec![tex]));
    lib.materials.insert((key.to_string(), i), m.clone());
    m
}

fn mesh_for(lib: &mut EffectLibrary, assets: &AssetServer, glb: &str) -> Handle<Mesh> {
    lib.meshes.entry(glb.to_string()).or_insert_with(|| assets.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(glb.to_string()))).clone()
}

/// Spawns particles for every playing effect; ends effects that are done.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn tick_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut lib: ResMut<EffectLibrary>,
    mut mats: ResMut<Assets<FxMaterial>>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    assets: Res<AssetServer>,
    mut effects: Query<(Entity, &mut Effect, &GlobalTransform, &InheritedVisibility, Option<&Beam>)>,
    mut batches: Query<&mut Batch>,
    mut ribbons: Query<&mut Ribbon>,
    mut lights: Query<(&mut PointLight, &EffectLight, &ChildOf)>,
    mut events: ResMut<FxEvents>,
    mut capped: Local<bool>,
) {
    let dt = time.delta_secs();
    let mut live: usize = batches.iter().map(|b| b.parts.len()).sum();
    let sent = std::mem::take(&mut events.0);
    for (e, mut fx, at, vis, beam) in &mut effects {
        let Some(specs) = lib.effects.get(&fx.key).cloned() else {
            commands.entity(e).despawn();
            continue;
        };
        // The first frame only lets transform propagation place it (else t = 0 bursts start at the origin).
        if !fx.placed {
            fx.placed = true;
            fx.last = Some(at.translation());
            continue;
        }
        let prev = fx.age;
        fx.age += dt;
        let age = fx.age;
        let origin = at.translation();
        let from = fx.last.unwrap_or(origin);
        fx.last = Some(origin);
        // Not started yet (timed to an attack window), or hidden (a closed portal): nothing spawns.
        if age <= 0.0 || !vis.get() {
            continue;
        }
        let jumped = from.distance(origin) * 100.0 > TELEPORT_CM;
        let (from, moved) = if jumped { (origin, 0.0) } else { (from, from.distance(origin) * 100.0) };
        let prev = prev.max(0.0);
        let rot = fx.rot;
        let ks = fx.scale;
        let brot = at.rotation();
        let until = fx.until;
        let mut seed = fx.seed;
        for (i, s) in specs.iter().enumerate() {
            let (mut acc, mut dist) = (fx.acc[i], fx.dist[i]);
            let n = spawn_count(s, &mut acc, &mut dist, prev, age, until, moved);
            (fx.acc[i], fx.dist[i]) = (acc, dist);
            // Particles at the events its sources sent last frame (world points).
            let mut at_events = Vec::new();
            for v in &s.events {
                for x in sent.iter().filter(|x| x.owner == e && x.emitter == v.source && x.death == v.death).take(v.per_frame as usize) {
                    let k = v.count.0 + ((next(&mut seed) * (v.count.1 - v.count.0 + 1) as f32) as u32).min(v.count.1 - v.count.0);
                    at_events.extend(std::iter::repeat_n(x.pos, k as usize));
                }
            }
            if n == 0 && at_events.is_empty() {
                continue;
            }
            let mat = material_for(&mut lib, &mut mats, &assets, &fx.key, i);
            if let Draw::Ribbon { .. } = s.draw {
                let mut pts = Vec::new();
                for k in 0..n {
                    let dir = in_cone(rot * s.direction, s.cone, &mut seed);
                    let speed = s.speed.0 + (s.speed.1 - s.speed.0) * next(&mut seed);
                    let life = (s.lifetime.0 + (s.lifetime.1 - s.lifetime.0) * next(&mut seed)).max(0.02);
                    match beam {
                        // A beam burst shares one age along the line (its points don't move on their own).
                        Some(b) => {
                            let f = if n > 1 { k as f32 / (n - 1) as f32 } else { 0.0 };
                            let off = Vec3::new(next(&mut seed) - 0.5, next(&mut seed) - 0.5, next(&mut seed) - 0.5) * (2.0 * b.jitter * (f * std::f32::consts::PI).sin());
                            pts.push(RibbonPoint { pos: origin.lerp(b.end, f) + off, vel: Vec3::ZERO, age: 0.0, life: s.lifetime.1.max(0.02), beam: Some((f, off)) });
                        }
                        None => pts.push(RibbonPoint { pos: from.lerp(origin, (k + 1) as f32 / n as f32), vel: to_bevy(dir * speed), age: 0.0, life, beam: None }),
                    }
                }
                match fx.ribbons[i].and_then(|r| ribbons.get_mut(r).ok()) {
                    Some(mut rb) => {
                        // A jump (a teleport, the viewer starting over) starts a new strip; a beam's new
                        // burst replaces its line.
                        if jumped || beam.is_some() {
                            rb.points.clear();
                        }
                        rb.points.extend(pts);
                        let over = rb.points.len().saturating_sub(MAX_RIBBON_POINTS);
                        rb.points.drain(..over);
                    }
                    None => {
                        let mesh = mesh_assets.add(batch_mesh(Verts::default()));
                        let r = commands.spawn((Ribbon { key: (fx.key.clone(), i), owner: e, points: pts, mesh: mesh.clone(), accel: to_bevy(s.gravity), drag: s.drag, beam: beam.map(|_| e) }, Mesh3d(mesh), MeshMaterial3d(mat), Transform::IDENTITY, Visibility::default(), NoFrustumCulling, bevy::light::NotShadowCaster)).id();
                        fx.ribbons[i] = Some(r);
                    }
                }
                continue;
            }
            if let Draw::Mesh { glb, .. } = &s.draw {
                mesh_for(&mut lib, &assets, glb);
            }
            let mut fresh = Vec::new();
            let to_local = at.affine().inverse();
            for k in 0..n + at_events.len() as u32 {
                if live >= MAX_PARTICLES {
                    if !*capped {
                        *capped = true;
                        warn!("effects: {MAX_PARTICLES} live particles, new ones dropped (first at {})", fx.key);
                    }
                    break;
                }
                live += 1;
                let dir = in_cone(rot * s.direction, s.cone, &mut seed);
                let speed = s.speed.0 + (s.speed.1 - s.speed.0) * next(&mut seed);
                let life = (s.lifetime.0 + (s.lifetime.1 - s.lifetime.0) * next(&mut seed)).max(0.02);
                let size = next(&mut seed);
                let rand = next(&mut seed);
                // A spawn radius scatters the particles over the effect's floor plane (GPU mesh stand-ins).
                let off = if s.radius > 0.0 {
                    let (a, r) = (next(&mut seed) * std::f32::consts::TAU, next(&mut seed).sqrt() * s.radius * ks);
                    to_bevy(rot * Vec3::new(r * a.cos(), r * a.sin(), 0.0))
                } else {
                    Vec3::ZERO
                };
                // Local space: rides along with the effect (its velocity in the effect's frame).
                let pos = match at_events.get((k as usize).wrapping_sub(n as usize)) {
                    Some(w) if k >= n => (if s.local { to_local.transform_point3(*w) } else { *w }) + off,
                    _ if s.local => off,
                    _ => from.lerp(origin, (k + 1) as f32 / n as f32) + off,
                };
                let spin = if s.spin.1 > s.spin.0 { s.spin.0 + (s.spin.1 - s.spin.0) * next(&mut seed) } else { s.spin.0 }.to_radians();
                fresh.push(Part { pos, vel: to_bevy(dir * speed * ks), accel: to_bevy(s.gravity * ks), drag: s.drag, age: 0.0, life, size, rand, rot: brot * quat_to_bevy(rot), scale: ks, spin });
            }
            match fx.batches[i].and_then(|b| batches.get_mut(b).ok()) {
                Some(mut b) => b.parts.extend(fresh),
                None => {
                    let mesh = mesh_assets.add(batch_mesh(Verts::default()));
                    let b = commands.spawn((Batch { key: (fx.key.clone(), i), parts: fresh, owner: e, local: s.local, mesh: mesh.clone() }, Mesh3d(mesh), MeshMaterial3d(mat), Transform::IDENTITY, Visibility::default(), NoFrustumCulling, bevy::light::NotShadowCaster)).id();
                    fx.batches[i] = Some(b);
                }
            }
        }
        fx.seed = seed;
        if end_time(&specs, until).is_some_and(|end| age > end) {
            commands.entity(e).despawn();
        }
    }
    for (mut l, el, parent) in &mut lights {
        if let Ok((_, fx, _, _, _)) = effects.get(parent.parent()) {
            l.intensity = if el.loops { el.base } else { el.base * (1.0 - fx.age / el.life).clamp(0.0, 1.0) };
        }
    }
}

/// Moves and ages every batch's particles and rebuilds its mesh: sprites face the camera, meshes keep their
/// effect's turn unless they face the camera; a batch whose effect is gone goes once its particles have died.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut lib: ResMut<EffectLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    owners: Query<(&GlobalTransform, Option<&Beam>), With<Effect>>,
    mut q: Query<(Entity, &mut Batch)>,
    mut events: ResMut<FxEvents>,
) {
    let dt = time.delta_secs();
    let cam = camera.iter().next().map(|c| c.rotation()).unwrap_or_default();
    let eye = camera.iter().next().map(|c| c.translation());
    let (right, up) = (cam * Vec3::X, cam * Vec3::Y);
    // Mesh particles' vertices, copied once their GLBs have loaded.
    let pending: Vec<(String, Handle<Mesh>)> = lib.meshes.iter().filter(|(g, _)| !lib.mesh_data.contains_key(*g)).map(|(g, h)| (g.clone(), h.clone())).collect();
    for (g, h) in pending {
        if let Some(d) = meshes.get(&h).and_then(MeshData::from_mesh) {
            lib.mesh_data.insert(g, std::sync::Arc::new(d));
        }
    }
    for (e, mut b) in &mut q {
        let owner = owners.get(b.owner).ok();
        if b.local && owner.is_none() {
            b.parts.clear();
        }
        for p in &mut b.parts {
            p.age += dt;
            p.vel += p.accel * dt;
            p.vel *= (1.0 - p.drag * dt).max(0.0);
            p.pos += p.vel * dt;
        }
        // Events for the effect's other emitters: every live particle's place, each one that died. Location events
        // at least [`EVENT_HZ`] a second, placed back along the frame's path: once per frame, a slow frame left a
        // trail of separate clumps (white discs 1.5 m apart behind Pyroblast's flying sources).
        let sends = lib.effects.get(&b.key.0).and_then(|v| v.get(b.key.1)).map_or(0, |s| s.sends);
        if sends != 0
            && let Some((g, _)) = owner
        {
            let steps = ((dt * EVENT_HZ).round() as usize).clamp(1, 32);
            for p in &b.parts {
                let death = p.age >= p.life;
                if sends & if death { 2 } else { 1 } == 0 {
                    continue;
                }
                for j in 0..if death { 1 } else { steps } {
                    let at = p.pos - p.vel * dt * (j as f32 / steps as f32);
                    events.0.push(FxEvent { owner: b.owner, emitter: b.key.1, death, pos: if b.local { g.transform_point(at) } else { at } });
                }
            }
        }
        b.parts.retain(|p| p.age < p.life);
        if b.parts.is_empty() && owner.is_none() {
            commands.entity(e).despawn();
            continue;
        }
        let Some(spec) = lib.effects.get(&b.key.0).and_then(|v| v.get(b.key.1)) else { continue };
        let data = match &spec.draw {
            Draw::Mesh { glb, .. } => match lib.mesh_data.get(glb) {
                Some(d) => Some(d.clone()),
                None => continue,
            },
            _ => None,
        };
        let (cols, rows) = if spec.draw == Draw::Sprite { spec.subuv } else { (1, 1) };
        let frames = (cols * rows).max(1);
        let cell = Vec2::new(1.0 / cols as f32, 1.0 / rows as f32);
        let frame_uv = |f: u32| Vec2::new((f % cols) as f32 / cols as f32, (f / cols) as f32 / rows as f32);
        let mut v = Verts::default();
        // Sprites face the camera: their normal points at it.
        let facing = cam * Vec3::Z;
        for p in &b.parts {
            let t = p.age / p.life;
            // World position: local particles ride their effect; a seeking one travels its beam.
            let at = match owner {
                Some((g, Some(beam))) if spec.seek_end => g.translation().lerp(beam.end, t),
                Some((g, _)) if b.local => g.transform_point(p.pos),
                _ => p.pos,
            };
            let c = spec.color_at(t);
            let dp = [0, 1, 2, 3].map(|k| spec.dyn_at(k, t));
            let f = t * frames as f32;
            let fa = (f as u32).min(frames - 1);
            let fb = (fa + 1).min(frames - 1);
            let blend = if spec.subuv_blend { f.fract() } else { 0.0 };
            let m = [blend, p.rand, t, 0.0];
            let base = v.pos.len() as u32;
            match (&spec.draw, &data) {
                (Draw::Hidden, _) => {}
                (Draw::Mesh { face_camera, .. }, Some(md)) => {
                    // Facing the camera: the mesh's flat axis (glTF +Y, UE +Z) towards the viewer.
                    let r = if *face_camera { cam * Quat::from_rotation_arc(Vec3::Y, Vec3::Z) } else { p.rot };
                    let sc = (spec.mesh_scale_at(t) * p.scale).max(Vec3::splat(1e-4));
                    // Not drawn while the eye is inside it: a thundercloud's 3.5 m lightning cylinder called next to
                    // the player covered the whole view from inside (HYPOTHESIS as the sprites' near rule: UE
                    // materials fade by camera distance). With the material's own formula its camera fade does that
                    // (the Thunder staff's aim column wraps the player when aimed close; skipped, the aim showed
                    // nothing).
                    if spec.exact.is_none() && eye.is_some_and(|eye| ((r.inverse() * (eye - at)) / sc - md.center).abs().cmple(md.half).all()) {
                        continue;
                    }
                    for ((p, n), u) in md.pos.iter().zip(&md.nor).zip(&md.uv) {
                        v.push(at + r * (*p * sc), *u, *u, c, r * (*n / sc).normalize_or(Vec3::Y), &dp, m);
                    }
                    v.idx.extend(md.idx.iter().map(|i| base + i));
                }
                _ => {
                    // A sprite no wider than its distance from the eye: a 4 m smoke or heal glow at the camera
                    // would be a full-screen veil (UE materials fade by camera distance; HYPOTHESIS).
                    let near = eye.map_or(f32::MAX, |eye| eye.distance(at) * NEAR_SPRITE);
                    let full = spec.size_at(t, p.size) * 0.01 * p.scale;
                    let half = (spec.aspect * full).min(Vec2::splat(near)).max(Vec2::splat(1e-4)) * 0.5;
                    let vel = match owner {
                        Some((g, _)) if b.local => g.affine().transform_vector3(p.vel),
                        _ => p.vel,
                    };
                    let (sr, su, sn) = sprite_axes(spec, p.rot, vel, eye.map(|eye| eye - at), (right, up, facing), p.spin);
                    let (a, bb) = (frame_uv(fa), frame_uv(fb));
                    for (x, y) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                        let corner = Vec2::new((x + 1.0) * 0.5, (1.0 - y) * 0.5) * cell;
                        v.push(at + sr * (x * half.x) + su * (y * half.y), (a + corner).to_array(), (bb + corner).to_array(), c, sn, &dp, m);
                    }
                    v.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&b.mesh) {
            *mesh = batch_mesh(v);
        }
    }
}

/// Ages ribbon points and rebuilds each ribbon's strip, facing the camera; a ribbon with no points left
/// goes.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn tick_ribbons(mut commands: Commands, time: Res<Time>, lib: Res<EffectLibrary>, camera: Query<&GlobalTransform, With<Camera3d>>, beams: Query<(&GlobalTransform, &Beam)>, mut meshes: ResMut<Assets<Mesh>>, mut q: Query<(Entity, &mut Ribbon)>, mut events: ResMut<FxEvents>) {
    let dt = time.delta_secs();
    let eye = camera.iter().next().map(|c| c.translation()).unwrap_or_default();
    for (e, mut rb) in &mut q {
        if let Some(owner) = rb.beam {
            let Ok((at, b)) = beams.get(owner) else {
                commands.entity(e).despawn();
                continue;
            };
            let (start, end) = (at.translation(), b.end);
            for p in &mut rb.points {
                if let Some((f, off)) = p.beam {
                    p.pos = start.lerp(end, f) + off;
                }
            }
        }
        let (accel, drag) = (rb.accel, rb.drag);
        for p in &mut rb.points {
            p.age += dt;
            p.vel += accel * dt;
            p.vel *= (1.0 - drag * dt).max(0.0);
            p.pos += p.vel * dt;
        }
        // Events for the effect's other emitters (a lightning ribbon's points place its bolt meshes).
        let sends = lib.effects.get(&rb.key.0).and_then(|v| v.get(rb.key.1)).map_or(0, |s| s.sends);
        if sends != 0 {
            for p in &rb.points {
                let death = p.age >= p.life;
                if sends & if death { 2 } else { 1 } != 0 {
                    events.0.push(FxEvent { owner: rb.owner, emitter: rb.key.1, death, pos: p.pos });
                }
            }
        }
        rb.points.retain(|p| p.age < p.life);
        if rb.points.is_empty() {
            commands.entity(e).despawn();
            continue;
        }
        let Some(spec) = lib.effects.get(&rb.key.0).and_then(|v| v.get(rb.key.1)) else { continue };
        let Some(mut mesh) = meshes.get_mut(&rb.mesh) else { continue };
        let n = rb.points.len();
        let mut v = Verts::default();
        if n >= 2 {
            for i in 0..n {
                let p = rb.points[i];
                let a = rb.points[i.saturating_sub(1)].pos;
                let b = rb.points[(i + 1).min(n - 1)].pos;
                let tangent = (b - a).normalize_or(Vec3::Y);
                let view = (eye - p.pos).normalize_or(Vec3::Z);
                let side = tangent.cross(view).normalize_or(Vec3::X);
                let t = p.age / p.life;
                // No wider than its distance from the eye, as sprites (a 1.5 m sword trail at the
                // first-person weapon would cover the screen).
                let half = (spec.ribbon_width_at(t) * 0.005).min(eye.distance(p.pos) * NEAR_SPRITE * 0.5);
                let mut c = spec.color_at(t);
                if let Some((f, _)) = p.beam {
                    let m = spec.along_at(f);
                    c = [0, 1, 2, 3].map(|k| c[k] * m[k]);
                }
                let dp = [0, 1, 2, 3].map(|k| spec.dyn_at(k, t));
                // UV along the ribbon from its head (newest point) to its tail.
                let u = (n - 1 - i) as f32 / (n - 1) as f32;
                for (s, w) in [(-1.0, 0.0), (1.0, 1.0)] {
                    v.push(p.pos + side * half * s, [u, w], [u, w], c, view, &dp, [0.0, 0.0, t, 0.0]);
                }
                if i + 1 < n {
                    let k = (2 * i) as u32;
                    v.idx.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
                }
            }
        }
        *mesh = batch_mesh(v);
    }
}

/// A batch's or ribbon's vertices in the effect material's layout (`fx_material.rs`).
#[derive(Default)]
struct Verts {
    pos: Vec<[f32; 3]>,
    uv0: Vec<[f32; 2]>,
    uv1: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    nor: Vec<[f32; 3]>,
    dyns: [Vec<[f32; 4]>; 4],
    misc: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Verts {
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, p: Vec3, uv0: [f32; 2], uv1: [f32; 2], col: [f32; 4], nor: Vec3, dyns: &[[f32; 4]; 4], misc: [f32; 4]) {
        self.pos.push(p.to_array());
        self.uv0.push(uv0);
        self.uv1.push(uv1);
        self.col.push(col);
        self.nor.push(nor.to_array());
        for (d, x) in self.dyns.iter_mut().zip(dyns) {
            d.push(*x);
        }
        self.misc.push(misc);
    }
}

/// The mesh of [`Verts`]; an empty one is a single invisible triangle (the renderer doesn't take meshes without
/// vertices).
fn batch_mesh(mut v: Verts) -> Mesh {
    if v.idx.is_empty() {
        for _ in 0..3 {
            v.push(Vec3::ZERO, [0.0; 2], [0.0; 2], [0.0; 4], Vec3::Y, &[[0.0; 4]; 4], [0.0; 4]);
        }
        v.idx = vec![0, 1, 2];
    }
    let [d0, d1, d2, d3] = v.dyns;
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, v.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, v.uv0)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, v.uv1)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, v.col)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, v.nor)
        .with_inserted_attribute(ATTRIBUTE_FX_DYN, d0)
        .with_inserted_attribute(ATTRIBUTE_FX_DYN1, d1)
        .with_inserted_attribute(ATTRIBUTE_FX_DYN2, d2)
        .with_inserted_attribute(ATTRIBUTE_FX_DYN3, d3)
        .with_inserted_attribute(ATTRIBUTE_FX_MISC, v.misc)
        .with_inserted_indices(Indices::U32(v.idx))
}

// ---- Where the game plays them (pairings marked HYPOTHESIS there) ----

/// Where a hit lands on a sim actor (Bevy space): its capsule centre (goblins are short; a chest offset
/// put the blood over their heads).
fn chest(p: Vec3) -> Vec3 {
    to_bevy(p)
}

/// The hit burst for a hit on `target` (UE space normal points from the target to the attacker).
fn hit_effect(blocked: bool, by: db_sim::HitSource, target_kit: Option<&db_sim::MonsterKit>) -> &'static str {
    use db_sim::HitSource;
    let undead = target_kit.is_some_and(|k| k.name.contains("Skeleton") || k.name.contains("Cyclop") || k.name.contains("Shadow_King"));
    let armour = target_kit.is_some_and(|k| k.name.starts_with("Armor"));
    match by {
        _ if blocked || armour => "hit_metal_sparks",
        _ if undead => "hit_bone_broken",
        HitSource::Swing(_) | HitSource::Skill(_) | HitSource::Bolt => "hit_flesh_slash",
        HitSource::Monster(_) => "hit_flesh_blunt",
        HitSource::Bleed(_) | HitSource::Trap(_) | HitSource::Staff(_) | HitSource::Zone | HitSource::Burn => "hit_generic",
    }
}

/// The trail and impact of a non-skill projectile (skill shots get theirs from `skill_vfx.rs`; a skill
/// shot without a table impact keeps the element burst here).
fn projectile_effects(p: &db_sim::Projectile) -> (Option<&'static str>, &'static str) {
    use db_sim::combat::Element;
    match (p.spec.by, p.spec.element) {
        (db_sim::HitSource::Monster(_), _) if p.spec.burst > 0.0 => (Some("goblin_shaman_missile"), "goblin_shaman_hit"),
        (db_sim::HitSource::Monster(_), _) if p.spec.radius >= 15.0 => (Some("skeleton_wizard_missile"), "projectile_ice_impact"),
        (_, Element::Fire) => (None, "projectile_fire_impact"),
        (_, Element::Ice) => (None, "projectile_ice_impact"),
        // the other elements' hits, which the target cue `GCNL_TGF_Character_Target_Slash_LongSword` imports
        // beside fire and ice (picked by the damage's element: HYPOTHESIS, by name).
        (_, Element::Thunder) => (None, "sys:FX_Common_Hit_Lightning"),
        (_, Element::Light) => (None, "sys:FX_Common_Hit_Light"),
        (_, Element::Dark) => (None, "sys:FX_Common_Hit_Dark"),
        _ => (None, "hit_generic"),
    }
}

/// Hits, casts, boss attacks, projectile impacts, from this frame's sim events.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn event_effects(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    montage_fx: Res<MonsterVfx>,
    table: Res<crate::skill_vfx::SkillVfx>,
    events: Res<crate::combat_view::FrameEvents>,
    index: Res<db_sim::SimIndex>,
    positions: Query<&db_sim::Position>,
    kits: Query<&db_sim::MonsterKit>,
    shots: Query<(&db_sim::SimId, &db_sim::Position, &db_sim::Projectile)>,
    mut flying: Local<HashMap<db_sim::SimId, (Vec3, Vec3, &'static str)>>,
) {
    use db_sim::SimEvent;
    if lib.effects.is_empty() {
        return;
    }
    let pos_of = |id: db_sim::SimId| index.get(id).and_then(|e| positions.get(e).ok()).map(|p| p.0);
    let kit_of = |id: db_sim::SimId| index.get(id).and_then(|e| kits.get(e).ok());
    for ev in &events.0 {
        match ev {
            SimEvent::Damaged { source, target, crit, blocked, by, .. } => {
                let Some(at) = pos_of(*target) else { continue };
                let normal = pos_of(*source).map_or(Vec3::X, |s| (s - at).with_z(0.0).normalize_or(Vec3::X));
                // Hit bursts point their local +X out of the surface, towards the attacker.
                let rot = Quat::from_rotation_arc(Vec3::X, normal);
                let key = match by {
                    db_sim::HitSource::Trap(db_sim::TrapKind::FireStatue) => "trap_fire_burn",
                    // A skill hit with its own table hit effect (a fire burst) doesn't also bleed.
                    db_sim::HitSource::Skill(s) if !*blocked && table.has_hit(*s) => "",
                    _ => hit_effect(*blocked, *by, kit_of(*target)),
                };
                if !key.is_empty() {
                    spawn_effect(&mut commands, &lib, key, chest(at), rot, Some(1.5));
                }
                if *crit {
                    spawn_effect(&mut commands, &lib, "hit_crit", chest(at), rot, None);
                }
            }
            SimEvent::MonsterAttackStarted { id, index: i } => {
                let (Some(kit), Some(at)) = (kit_of(*id), pos_of(*id)) else { continue };
                let Some(a) = kit.attacks.get(*i as usize) else { continue };
                let key = match a.name.as_str() {
                    n if n.contains("Goblin_Shaman") && a.shot.is_some() => "goblin_shaman_cast",
                    n if n.contains("Skeleton_Wizard") && a.shot.is_some() => "skeleton_wizard_cast",
                    // `GA_TGF_M_Totem_IceBolt` spawns `B_Totem_IceBolt_Cast`, the same cast at scale 2.
                    "AM_Totem02_IceBolt" => "totem_icebolt_cast",
                    // (a montage whose notifies the data has plays them in `monster_effects`.)
                    n if montage_fx.0.contains_key(n) => continue,
                    "AM_Shadow_King_Attack4" => "shadow_king_attack4",
                    n if n.contains("Cyclop") && n.contains("Attack3") => "cyclop_attack3_slam",
                    _ => continue,
                };
                // Timed to the attack's first window (the notify times aren't read; HYPOTHESIS).
                let delay = a.windows.first().map_or(0.0, |w| w.0);
                let (key, scale) = if key == "totem_icebolt_cast" { ("skeleton_wizard_cast", 2.0) } else { (key, 1.0) };
                if let Some(e) = spawn_effect_scaled(&mut commands, &lib, key, to_bevy(at), Quat::IDENTITY, None, scale) {
                    commands.queue(move |w: &mut World| {
                        if let Some(mut fx) = w.get_mut::<Effect>(e) {
                            fx.age = -delay;
                        }
                    });
                }
            }
            SimEvent::Despawned { id } => {
                if let Some((at, dir, key)) = flying.remove(id) {
                    spawn_effect(&mut commands, &lib, key, to_bevy(at), Quat::from_rotation_arc(Vec3::X, -dir), Some(1.0));
                }
            }
            _ => {}
        }
    }
    // Remember where every projectile is, for its impact when it goes (skill shots whose table has an
    // impact or burst get it from `skill_vfx.rs` instead).
    flying.retain(|id, _| index.get(*id).is_some());
    for (&id, p, shot) in &shots {
        if let db_sim::HitSource::Skill(s) = shot.spec.by
            && table.has_impact(s)
        {
            continue;
        }
        flying.insert(id, (p.0, shot.dir, projectile_effects(shot).1));
    }
}

/// Trails on monster projectiles, loops on portals and altars (they last as long as their views or markers).
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn attached_effects(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    index: Res<db_sim::SimIndex>,
    views: Query<(Entity, &crate::combat_view::ActorView), Added<crate::combat_view::ActorView>>,
    shots: Query<&db_sim::Projectile>,
    (actor_vfx, kits): (Res<MonsterActorVfx>, Query<&db_sim::MonsterKit>),
    portals: Query<(&db_sim::Position, &db_sim::Portal), Added<db_sim::Portal>>,
    altars: Query<&db_sim::Position, Added<db_sim::Altar>>,
    mut map_vfx: ResMut<crate::map_vfx::MapVfx>,
) {
    if lib.effects.is_empty() {
        return;
    }
    for (e, view) in &views {
        let Some(shot) = index.get(view.0).and_then(|s| shots.get(s).ok()) else { continue };
        // a monster shot's own actor's systems (its trail and glow), started with it, in its frame (X along the
        // flight).
        if let Some(a) = actor_vfx.shot(Some(&lib), &index, &kits, shot) {
            // A fuse (the Beholder's pillar) sits on the floor: turned about Z only, so its floor sigils stay flat.
            let dir = if shot.spec.fuse { shot.dir.with_z(0.0) } else { shot.dir };
            let frame = Quat::from_rotation_arc(Vec3::X, dir.normalize_or(Vec3::X));
            for s in a.systems.iter().filter(|s| s.auto) {
                spawn_actor_system(&mut commands, &lib, s, Vec3::ZERO, frame, Some(e));
            }
            continue;
        }
        let Some(key) = projectile_effects(shot).0 else { continue };
        // Its +X along the flight; local to the view so the trail follows it.
        if let Some(fx) = spawn_effect(&mut commands, &lib, key, Vec3::ZERO, Quat::from_rotation_arc(Vec3::X, shot.dir), None) {
            commands.entity(e).add_child(fx);
        }
    }
    for (p, portal) in &portals {
        // An escape point's own effects (its circle and portal, from the start, behind its escape door) when the map
        // places them.
        if map_vfx.start_escape(p.0 - Vec3::Z * db_sim::overrides::run::PORTAL_HEIGHT) {
            continue;
        }
        // Blue portals extract, red ones lead down (HYPOTHESIS).
        let key = match portal.kind {
            db_sim::dungeon::PortalKind::Extract => "portal_blue",
            db_sim::dungeon::PortalKind::Down => "portal_red",
        };
        if let Some(fx) = spawn_effect(&mut commands, &lib, key, to_bevy(p.0), Quat::IDENTITY, None) {
            commands.entity(fx).insert((crate::flow::LevelScoped, crate::dungeon_view::PortalMarker(p.0)));
        }
    }
    for p in &altars {
        if let Some(fx) = spawn_effect(&mut commands, &lib, "altar_relive", to_bevy(p.0), Quat::IDENTITY, None) {
            commands.entity(fx).insert(crate::flow::LevelScoped);
        }
    }
}

/// One effect a monster montage's notify plays: its library key, start and duration (s; `None`: once), the bones
/// it may ride with its offset in each one's frame (cm; its own bone first, then the ancestors the converted mesh
/// may keep instead) and scale.
#[derive(Debug, Clone)]
pub(crate) struct MontageFx {
    key: String,
    start: f32,
    duration: Option<f32>,
    bones: Vec<(String, Vec3)>,
    scale: f32,
}

/// The effects monster montages' notifies play, by montage (->
/// `_data/recovered/effects/monster_vfx.json`).
#[derive(Resource)]
pub(crate) struct MonsterVfx(HashMap<String, Vec<MontageFx>>);

impl Default for MonsterVfx {
    fn default() -> Self {
        let j = std::fs::read(crate::data_path("recovered/effects/monster_vfx.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let v3 = |v: &serde_json::Value| Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
        let table: HashMap<_, _> = j
            .as_ref()
            .and_then(|j| j.as_object())
            .into_iter()
            .flatten()
            .map(|(m, list)| {
                let fx = list
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| {
                        let system = e["system"].as_str()?;
                        Some(MontageFx {
                            key: format!("sys:{}", system.rsplit('/').next().unwrap_or(system)),
                            start: e["start"].as_f64().unwrap_or(0.0) as f32,
                            duration: e["duration"].as_f64().map(|d| d as f32),
                            bones: e["chain"].as_array().into_iter().flatten().filter_map(|c| Some((c[0].as_str()?.to_string(), v3(&c[1])))).collect(),
                            scale: e["scale"].as_f64().unwrap_or(1.0) as f32,
                        })
                    })
                    .collect();
                (m.clone(), fx)
            })
            .collect();
        info!("monster vfx: {} montages' effects", table.len());
        Self(table)
    }
}

/// One Niagara part of a monster ability's actor: its library key, offset (cm) and rotation in the actor's frame (UE
/// axes), uniform scale (the mean of its `RelativeScale3D`: HYPOTHESIS) and whether it starts with the actor.
#[derive(Debug, Clone)]
pub(crate) struct ActorSystem {
    pub key: String,
    pub offset: Vec3,
    pub rot: Quat,
    pub scale: f32,
    pub auto: bool,
}

/// A decal part of a monster ability's actor: its half size across (cm, `DecalSize` Y, Z), colour (linear) and mask
/// texture (asset path of its alpha copy).
#[derive(Debug, Clone)]
pub(crate) struct ActorDecal {
    pub half: Vec2,
    pub color: [f32; 3],
    pub mask: Option<String>,
}

/// A monster ability's actor's own effects.
#[derive(Debug, Clone, Default)]
pub(crate) struct ActorFx {
    pub systems: Vec<ActorSystem>,
    pub decals: Vec<ActorDecal>,
    /// Its sound properties' Wwise events (`LoopSound`, `LaunchSound`, `FlybySound`, `WooshSounds` ...).
    pub sounds: HashMap<String, Vec<String>>,
}

impl ActorFx {
    /// The first event of its sound property `key`.
    pub(crate) fn sound(&self, key: &str) -> Option<&str> {
        self.sounds.get(key)?.first().map(String::as_str)
    }
}

/// The actors monster abilities spawn, with their own effects (->
/// `_data/recovered/effects/monster_actor_vfx.json`).
#[derive(Resource, Default)]
pub(crate) struct MonsterActorVfx {
    /// Ability class -> (actor class, the ability property naming it).
    abilities: HashMap<String, Vec<(String, String)>>,
    actors: HashMap<String, ActorFx>,
}

/// The ability properties that name the projectile it shoots (not its hand cast, `GrabActor`, or its summons).
const PROJECTILE_VIA: [&str; 3] = ["SpawnActor", "SpawnConfig", "ProjectileTemplate"];

impl MonsterActorVfx {
    pub(crate) fn load() -> Self {
        let Some(j) = std::fs::read(crate::data_path("recovered/effects/monster_actor_vfx.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) else {
            warn!("no _data/recovered/effects/monster_actor_vfx.json (tools/recon/monster_actor_vfx.py); monster shots keep the stand-in look");
            return Self::default();
        };
        let v3 = |v: &serde_json::Value| Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
        let abilities = j["abilities"].as_object().into_iter().flatten().map(|(ga, list)| (ga.clone(), list.as_array().into_iter().flatten().filter_map(|e| Some((e["actor"].as_str()?.to_string(), e["via"].as_str()?.to_string()))).collect())).collect();
        let actors = j["actors"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, a)| {
                let systems = a["niagara"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|n| {
                        let system = n["system"].as_str()?;
                        let q = &n["quat"];
                        let scale = v3(&n["scale"]);
                        Some(ActorSystem {
                            key: format!("sys:{}", system.rsplit('/').next().unwrap_or(system)),
                            offset: v3(&n["location"]),
                            rot: Quat::from_xyzw(q[0].as_f64().unwrap_or(0.0) as f32, q[1].as_f64().unwrap_or(0.0) as f32, q[2].as_f64().unwrap_or(0.0) as f32, q[3].as_f64().unwrap_or(1.0) as f32).normalize(),
                            scale: (scale.x + scale.y + scale.z) / 3.0,
                            auto: n["auto"].as_bool().unwrap_or(true),
                        })
                    })
                    .collect();
                let sounds = a["sounds"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_array().into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect())).collect();
                let decals = a["decals"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|d| {
                        let size = v3(&d["size"]);
                        let c = v3(&d["color"]);
                        // `/Game/X/T` -> its alpha copy (`effect_textures.py`: `generated/effects/decal_T.png`).
                        let mask = d["mask"].as_str().and_then(|m| m.rsplit('/').next()).map(|m| format!("generated/effects/decal_{m}.png")).filter(|m| crate::map_view::data_file(&format!("assets/{m}")).is_some());
                        ActorDecal { half: Vec2::new(size.y, size.z), color: if d["color"].is_array() { [c.x, c.y, c.z] } else { [1.0; 3] }, mask }
                    })
                    .collect();
                (name.clone(), ActorFx { systems, decals, sounds })
            })
            .collect();
        let me = Self { abilities, actors };
        info!("monster actor vfx: {} abilities, {} actors", me.abilities.len(), me.actors.len());
        me
    }

    /// The actor `ability` names through one of `via`; with `lib`, the first whose effects the library has.
    pub(crate) fn actor(&self, ability: &str, via: &[&str], lib: Option<&EffectLibrary>) -> Option<&ActorFx> {
        self.abilities.get(ability)?.iter().filter(|(_, v)| via.contains(&v.as_str())).filter_map(|(a, _)| self.actors.get(a)).find(|a| lib.is_none_or(|lib| a.systems.iter().any(|s| lib.has(&s.key))))
    }

    /// The actor monster `owner`'s attack `by` spawns through one of `via` (see [`Self::actor`]).
    pub(crate) fn of_attack<'a>(&'a self, lib: Option<&EffectLibrary>, index: &db_sim::SimIndex, kits: &Query<&db_sim::MonsterKit>, owner: db_sim::SimId, by: db_sim::HitSource, via: &[&str]) -> Option<&'a ActorFx> {
        let db_sim::HitSource::Monster(i) = by else { return None };
        let kit = index.get(owner).and_then(|e| kits.get(e).ok())?;
        self.actor(&kit.attacks.get(usize::from(i))?.class, via, lib)
    }

    /// A monster projectile's actor (its shooter's ability's); with `lib`, only when its effects are in the library.
    pub(crate) fn shot<'a>(&'a self, lib: Option<&EffectLibrary>, index: &db_sim::SimIndex, kits: &Query<&db_sim::MonsterKit>, p: &db_sim::Projectile) -> Option<&'a ActorFx> {
        self.of_attack(lib, index, kits, p.owner, p.spec.by, &PROJECTILE_VIA)
    }
}

/// Spawns `s` on an actor at `at` (Bevy) whose frame is `frame` (UE axes); local to `parent` when given.
pub(crate) fn spawn_actor_system(commands: &mut Commands, lib: &EffectLibrary, s: &ActorSystem, at: Vec3, frame: Quat, parent: Option<Entity>) -> Option<Entity> {
    let offset = to_bevy(frame * s.offset);
    let e = spawn_effect_scaled(commands, lib, &s.key, if parent.is_some() { offset } else { at + offset }, frame * s.rot, None, s.scale)?;
    if let Some(p) = parent {
        commands.entity(p).add_child(e);
    }
    Some(e)
}

/// How long a once-played montage effect (`AnimNotify_PlayNiagaraEffect`) keeps spawning (s; ours: its system's own
/// loop isn't stopped by a notify end).
const MONTAGE_ONCE_S: f32 = 2.0;

/// Plays a monster's attack montage's effects when it starts the attack: each on its bone (found by name under the
/// monster's body; else the nearest ancestor the mesh keeps, else the body's chest height), at its notify time, for
/// its state's length. The offsets are in the bone's frame as UE has it (HYPOTHESIS that the converted bones keep
/// it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn monster_effects(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    table: Res<MonsterVfx>,
    events: Res<crate::combat_view::FrameEvents>,
    index: Res<db_sim::SimIndex>,
    kits: Query<&db_sim::MonsterKit>,
    bodies: Query<(Entity, &crate::model_view::ModelBody)>,
    (children, names, globals): (Query<&Children>, Query<&Name>, Query<&GlobalTransform>),
) {
    for ev in &events.0 {
        let db_sim::SimEvent::MonsterAttackStarted { id, index: i } = ev else { continue };
        let Some(a) = index.get(*id).and_then(|e| kits.get(e).ok()).and_then(|k| k.attacks.get(*i as usize)) else { continue };
        let Some(fx) = table.0.get(&a.name) else { continue };
        let Some(body) = bodies.iter().find(|(_, b)| b.id == *id).map(|(e, _)| e) else { continue };
        let mut on_bones = 0;
        for f in fx {
            let bone = f.bones.iter().find_map(|(name, off)| children.iter_descendants(body).find(|c| names.get(*c).is_ok_and(|n| n.as_str() == name)).map(|b| (b, *off)));
            on_bones += usize::from(bone.is_some());
            let (parent, local) = match bone {
                Some((b, offset)) => {
                    let s = globals.get(b).map_or(1.0, |g| g.compute_transform().scale.x.abs().max(1e-4));
                    (b, offset * 0.01 / s)
                }
                None => (body, Vec3::Y * 1.2),
            };
            if let Some(e) = spawn_effect_scaled(&mut commands, &lib, &f.key, local, Quat::IDENTITY, Some(f.duration.unwrap_or(MONTAGE_ONCE_S)), f.scale) {
                commands.entity(parent).add_child(e);
                delay_effect(&mut commands, e, f.start);
            }
        }
        info!("monster vfx: {} plays {} effects ({on_bones} on their bones)", a.name, fx.len());
    }
}

/// Starts effect `e`'s emitters `delay` seconds from now (a montage notify state's start).
fn delay_effect(commands: &mut Commands, e: Entity, delay: f32) {
    commands.queue(move |w: &mut World| {
        if let Some(mut fx) = w.get_mut::<Effect>(e) {
            fx.age = -delay;
        }
    });
}

/// What the Goblin Thief's view shows: its loot glow and gold trail, and the ability whose casts were started.
#[derive(Default)]
pub(crate) struct ThiefFx {
    woke: bool,
    trail: Option<Entity>,
    ability: Option<u32>,
}

/// Where the thief's effect sockets sit on its view (m up from its origin; `EP_Drop` (the loot bag) and `EP0` aren't
/// read from the skeleton: HYPOTHESIS, about its back).
const THIEF_SOCKET: Vec3 = Vec3::new(0.0, 0.9, 0.0);

/// The Goblin Thief's effects (names and times CONFIRMED): on its reveal the wake montage's
/// `FX_Goblin_Gold_Wakeup` (0-7 s) and `_Wakeup01` (4.84-6 s) and the loot glow its wake effect's cue loops for
/// the rest of its life (`FX_Common_Drop_Blue`: the maps place its `LootBlue` class); the gold trail while it runs
/// fast (`GE_TGF_M_GoblinThief_MoveFast`'s cue); OpenDoor's casts (`FX_Goblin_Gold_OpenDoor_Cast01` 2.46-10 s,
/// `_Cast` 9.19-10.69 s). All on its view, so they go with it.
pub(crate) fn thief_effects(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    tick: Res<db_sim::TickCount>,
    index: Res<db_sim::SimIndex>,
    bodies: Query<(Entity, &crate::model_view::ModelBody)>,
    thieves: Query<(&db_sim::ai::MonsterAi, &db_sim::MonsterKit)>,
    mut state: Local<HashMap<db_sim::SimId, ThiefFx>>,
) {
    let now = tick.0 as u32;
    let mut seen = Vec::new();
    for (view, body) in &bodies {
        let id = body.id;
        let Some((ai, kit)) = index.get(id).and_then(|e| thieves.get(e).ok()) else { continue };
        let Some(thief) = kit.thief.as_deref() else { continue };
        seen.push(id);
        let fx = state.entry(id).or_default();
        let start = |commands: &mut Commands, key: &str, delay: f32, until: Option<f32>| {
            let e = spawn_effect(commands, &lib, key, THIEF_SOCKET, Quat::IDENTITY, until)?;
            commands.entity(view).add_child(e);
            delay_effect(commands, e, delay);
            Some(e)
        };
        if ai.thief.revealed_at.is_some() && !fx.woke {
            fx.woke = true;
            info!("thief effects: {id:?} woke (wake bursts, loot glow)");
            start(&mut commands, "thief_wakeup", 0.0, Some(7.0));
            start(&mut commands, "thief_wakeup01", 4.84, Some(6.0 - 4.84));
            start(&mut commands, "thief_loot_blue", 0.0, None);
        }
        let fast_ticks = (thief.fast.1 / db_sim::DT).round() as u32;
        let fast = ai.thief.accelerated_at.is_some_and(|t| now < t + fast_ticks);
        match (fast, fx.trail) {
            (true, None) => {
                info!("thief effects: {id:?} runs fast (gold trail)");
                fx.trail = start(&mut commands, "thief_gold_trail", 0.0, None);
            }
            (false, Some(e)) => {
                stop(&mut commands, e);
                fx.trail = None;
            }
            _ => {}
        }
        if let Some(act) = ai.thief.active
            && fx.ability != Some(act.start)
        {
            fx.ability = Some(act.start);
            if thief.abilities.get(act.ability).is_some_and(|a| a.montage == "AM_Goblin_Thief_OpenDoor") {
                start(&mut commands, "thief_opendoor_cast01", 2.46, Some(10.0 - 2.46));
                start(&mut commands, "thief_opendoor_cast", 9.19, Some(10.69 - 9.19));
            }
        }
    }
    state.retain(|id, _| seen.contains(id));
}

/// A totem's line to the monster it summoned (`GE_TGF_M_Totem_Summon_LineEffect`'s looping cue,
/// `FX_Summon_LineEffect` at `EP_Chest`), drawn as a beam from the totem to its summon while both live (the line's
/// end is bound at run time: HYPOTHESIS that it is the summon).
#[derive(Component)]
pub(crate) struct SummonLine(db_sim::SimId);

pub(crate) fn summon_lines(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    index: Res<db_sim::SimIndex>,
    summons: Query<(&db_sim::SimId, &db_sim::Position, &db_sim::summon::Summoned)>,
    owners: Query<(&db_sim::Position, &db_sim::MonsterKit)>,
    mut lines: Query<(Entity, &SummonLine, &mut Transform, &mut Beam)>,
) {
    let ends = |id: db_sim::SimId| {
        let (_, pos, s) = summons.iter().find(|(i, ..)| **i == id)?;
        let (owner, kit) = index.get(s.by).and_then(|e| owners.get(e).ok())?;
        kit.name.starts_with("Totem").then(|| (chest(owner.0), chest(pos.0)))
    };
    let mut drawn = Vec::new();
    for (e, line, mut tf, mut beam) in &mut lines {
        match ends(line.0) {
            Some((from, to)) => {
                tf.translation = from;
                beam.end = to;
                drawn.push(line.0);
            }
            None => commands.entity(e).despawn(),
        }
    }
    for (id, ..) in &summons {
        if drawn.contains(id) {
            continue;
        }
        let Some((from, to)) = ends(*id) else { continue };
        if let Some(e) = spawn_effect(&mut commands, &lib, "totem_summon_line", from, Quat::IDENTITY, None) {
            info!("summon line: to {id:?}, {:.1} m", from.distance(to));
            commands.entity(e).insert((SummonLine(*id), Beam { end: to, jitter: 0.0 }, crate::flow::LevelScoped));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_emitter_and_samples_its_curves() {
        let v = serde_json::json!({"name": "Blood", "sim": "CPU", "spawn": {"rate": null, "bursts": [{"count": 30, "time": 0.0}], "loop_duration": 5.0, "lifecycle": {"final_state": 3}}, "lifetime": [0.3, 0.6],
            "velocity": {"speed": [300, 600, 450], "cone": {"axis": [1, 0, 0], "max_angle_deg": 45}}, "gravity": -980, "drag": 1.0,
            "color": {"initial": [1, 0, 0, 2], "over_life": [[0, 1, 0, 0, 2], [1, 0.2, 0, 0, 0]]}, "size": {"initial": [4, 10], "over_life": [[0, 1], [1, 0.5]]},
            "renderer": {"type": "Sprite", "texture": "textures/x.png", "texture_exists": true, "blend": "translucent", "subuv": [4, 2]}});
        let s = EmitterSpec::parse(&v).unwrap();
        assert_eq!(s.bursts, vec![(30, 0.0)]);
        assert!(!s.loops);
        assert_eq!(s.lifetime, (0.3, 0.6));
        assert_eq!(s.speed, (300.0, 600.0));
        assert_eq!((s.direction, s.cone), (Vec3::X, 45.0));
        assert_eq!(s.gravity, Vec3::new(0.0, 0.0, -980.0));
        assert_eq!(s.blend, Blend::Alpha);
        assert_eq!(s.subuv, (4, 2));
        // Alpha relative to its peak (2.0 here).
        assert_eq!(s.color_at(0.5), [0.6, 0.0, 0.0, 0.5]);
        // No absolute curve: spawn size (4..10 cm) times the relative curve.
        assert!((s.size_at(0.5, 0.5) - 7.0 * 0.75).abs() < 1e-5);
        let c = EmitterSpec::parse(&serde_json::json!({"lifetime": [1, 1], "renderer": {"type": "Sprite", "texture": "t.png"}, "size": {"initial": [1, 2], "over_life_cm": [[0, 10, 20], [1, 30, 60]]}})).unwrap();
        assert!((c.size_at(0.5, 0.0) - 20.0).abs() < 1e-5 && (c.size_at(1.0, 1.0) - 60.0).abs() < 1e-5);
        // HDR colours keep their hue under the brightness cap.
        let h = EmitterSpec::parse(&serde_json::json!({"lifetime": [1, 1], "renderer": {"type": "Sprite", "texture": "t.png"}, "color": {"over_life": [[0, 12, 3, 0, 1]]}})).unwrap();
        assert_eq!(h.color_at(0.0), [2.0, 0.5, 0.0, 1.0]);
        // A one-shot burst fires once at the start; the effect ends after the longest life.
        let (mut acc, mut dist) = (0.0, 0.0);
        assert_eq!(spawn_count(&s, &mut acc, &mut dist, 0.0, 0.016, None, 0.0), 30);
        assert_eq!(spawn_count(&s, &mut acc, &mut dist, 0.016, 0.032, None, 0.0), 0);
        assert_eq!(end_time(std::slice::from_ref(&s), None), Some(5.6));
        // A looping emitter repeats its first loop's bursts and keeps its rate.
        let l = EmitterSpec::parse(&serde_json::json!({"spawn": {"rate": {"per_second": 10.0}, "bursts": [{"count": 1, "time": 0.0}, {"count": 1, "time": 1.0}], "loop_duration": 1.0, "lifecycle": {"final_state": 0}}, "lifetime": [1, 1], "renderer": {"type": "Sprite", "texture": "t.png"}})).unwrap();
        assert!(l.loops);
        assert_eq!(l.bursts, vec![(1, 0.0)]);
        let (mut acc, mut dist) = (0.0, 0.0);
        let n: u32 = (0..300).map(|i| spawn_count(&l, &mut acc, &mut dist, i as f32 * 0.01, (i + 1) as f32 * 0.01, None, 0.0)).sum();
        assert_eq!(n, 30 + 3 + 1, "10/s for 3 s, a burst at 0, 1, 2 and 3 s");
        assert_eq!(end_time(&[l], None), None);
        // GPU emitters without a stand-in and distortion sprites are skipped; light emitters keep only the light.
        assert!(EmitterSpec::parse(&serde_json::json!({"sim": "GPU", "lifetime": null, "renderer": {"type": "Sprite"}})).is_none());
        assert!(EmitterSpec::parse(&serde_json::json!({"lifetime": [1, 1], "renderer": {"type": "Sprite", "texture": "t.png", "distortion": true}})).is_none());
        let light = EmitterSpec::parse(&serde_json::json!({"lifetime": [0.5, 0.5], "renderer": null, "light": {"color_initial": [10, 5, 0], "radius_cm": 300}})).unwrap();
        assert_eq!(light.light, Some(([1.0, 0.5, 0.0], 1.0, 300.0)));
    }

    #[test]
    fn meshes_ribbons_and_trails() {
        // A mesh emitter: its GLB, scale over life on Bevy axes (UE Z is Bevy Y).
        let m = EmitterSpec::parse(&serde_json::json!({"lifetime": [0.5, 0.5], "spawn": {"bursts": [{"count": 1, "time": 0.0}]},
            "renderer": {"type": "Mesh", "texture": "t.png", "mesh_glb": [null, "models/a.glb"], "facing": 0}, "color": {"over_life": [[0, 4, 1, 0, 1], [1, 4, 1, 0, 0]]},
            "mesh_scale_over_life": [[0, 1, 2, 3], [1, 2, 4, 6]]})).unwrap();
        assert_eq!(m.draw, Draw::Mesh { glb: "models/a.glb".into(), scale: vec![(0.0, Vec3::new(1.0, 3.0, 2.0)), (1.0, Vec3::new(2.0, 6.0, 4.0))], face_camera: false });
        assert_eq!(m.mesh_scale_at(0.5), Vec3::new(1.5, 4.5, 3.0));
        // No GLB, or a mesh that stays black (a refraction ring): not drawn.
        assert!(EmitterSpec::parse(&serde_json::json!({"lifetime": [1, 1], "renderer": {"type": "Mesh", "mesh_glb": [null]}})).is_none());
        assert!(EmitterSpec::parse(&serde_json::json!({"lifetime": [1, 1], "renderer": {"type": "Mesh", "texture": "t.png", "mesh_glb": ["models/a.glb"]}, "mesh_scale": [[1, 1, 1], [1, 1, 1], [1, 1, 1]], "color": {"over_life": [[0, 0, 0, 0, 0], [1, 0, 0, 0, 0]]}})).is_none());
        // A ribbon spawning per distance: 5 points per 100 cm.
        let r = EmitterSpec::parse(&serde_json::json!({"lifetime": [0.5, 0.5], "spawn": {"per_unit": {"per_100cm": 5.0, "per_second": 50.0}, "lifecycle": {"final_state": 0}},
            "renderer": {"type": "Ribbon", "texture": "t.png"}, "ribbon_width": [14, 14, 14], "ribbon_width_over_life": [[0, 14], [1, 0]]})).unwrap();
        assert!(matches!(r.draw, Draw::Ribbon { .. }) && r.rate == 0.0);
        assert!((r.ribbon_width_at(0.5) - 7.0).abs() < 1e-5);
        let (mut acc, mut dist) = (0.0, 0.0);
        assert_eq!(spawn_count(&r, &mut acc, &mut dist, 0.0, 0.1, None, 0.0), 0, "standing still spawns no trail");
        assert_eq!(spawn_count(&r, &mut acc, &mut dist, 0.1, 0.2, None, 150.0), 7);
        // A GPU sprite emitter with spawn data gets a stand-in.
        let mut g = serde_json::json!({"name": "Spark", "sim": "GPU", "lifetime": null, "spawn": {"bursts": [{"count": 300, "time": 0.0}]}, "renderer": {"type": "Sprite", "texture": "textures/TX_FX_Particle_101.png", "blend": "additive"},
            "fixed_bounds_cm": [[-200, -200, -200], [200, 200, 200]], "gpu_modules": ["Drag_Emitter"], "color": {"alpha_multiplier": [[0, 1], [1, 0]]}, "size": {"over_life": []}});
        gpu_standin("x", &mut g);
        let s = EmitterSpec::parse(&g).unwrap();
        assert_eq!(s.bursts, vec![(300, 0.0)]);
        assert!(s.lifetime.0 > 0.0 && s.speed.1 > s.speed.0 && s.drag > 0.0 && s.size.1 < 5.0);
        // The UE -> Bevy rotation agrees with `to_bevy` on a turned vector.
        let q = Quat::from_rotation_z(0.7) * Quat::from_rotation_x(0.3);
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!((quat_to_bevy(q) * to_bevy(v) - to_bevy(q * v)).length() < 1e-5);
    }

    #[test]
    fn monster_ability_actors_carry_their_own_effects_and_sounds() {
        // (`monster_actor_vfx.py`): the Wendigo's shots, thunder area and the Slow totem's area decal.
        if crate::map_view::data_file("recovered/effects/monster_actor_vfx.json").is_none() {
            return;
        }
        let vfx = MonsterActorVfx::load();
        let a4 = vfx.actor("GA_TGF_M_Wendigo_Attack4_C", &PROJECTILE_VIA, None).expect("Attack4's shard");
        assert_eq!((a4.systems[0].key.as_str(), a4.sound("WooshSounds")), ("sys:FX_Wendigo_Attack5_1_Missile", Some("Play_M_Wendigo_Attack4_Magic_Flying_Loop")));
        // Attack5's bolts are the skeleton wizard's missile at 2 x 1 x 3 (mean 2), rolled 90 degrees about their flight.
        let a5 = vfx.actor("GA_TGF_M_Wendigo_Attack5_C", &PROJECTILE_VIA, None).unwrap();
        assert_eq!((a5.systems[0].key.as_str(), a5.systems[0].scale, a5.sound("LaunchSound")), ("sys:FX_Skeleton_Wizard_Attack1_Missile", 2.0, Some("Play_M_Wendigo_Attack5_Ice_Release_Single")));
        assert!(a5.systems[0].rot.angle_between(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)) < 1e-3, "{:?}", a5.systems[0].rot);
        // His thunder area: the warning starts with it; the bolt and the ground burst wait for its strike.
        let t = vfx.actor("GA_TGF_M_Wendigo_Thunder_C", &crate::staff_view::AREA_VIA, None).unwrap();
        let mut keys: Vec<(&str, bool)> = t.systems.iter().map(|s| (s.key.as_str(), s.auto)).collect();
        keys.sort();
        assert_eq!(keys, [("sys:FX_Wendigo_Attack5_7_Range_03_01", true), ("sys:FX_Wendigo_Attack5_7_Range_03_02", false), ("sys:FX_Wendigo_Attack5_7_Range_03_03", false)]);
        assert_eq!(t.sound("LoopSound"), Some("Play_M_Wendigo_Attack8_Ground"));
        // The Lava totem's blobs; the Slow totem's ice decal (160 cm half size, its pale `Creack_Color`); a hand cast
        // (`GrabActor`) is no projectile.
        assert_eq!(vfx.actor("GA_TGF_M_Totem_Lava_C", &PROJECTILE_VIA, None).unwrap().systems[0].key, "sys:FX_Wendigo_Attack5_6_Missile");
        let d = &vfx.actor("GA_TGF_M_Totem_ReduceSpeed_C", &["SpawnActor"], None).unwrap().decals[0];
        assert_eq!((d.half, d.color[2]), (Vec2::splat(160.0), 1.0));
        assert!(vfx.actor("GA_TGF_M_Totem_IceBolt_C", &PROJECTILE_VIA, None).unwrap().systems.iter().all(|s| !s.key.contains("Cast")));
        // The Cyclops' rock lost its mesh emitter (its GLB isn't converted), so his shot keeps the stand-in ball; the
        // Wendigo's shots draw whole.
        let Some(p) = crate::map_view::data_file("recovered/effects/effects.json") else { return };
        let lacking = lacking_meshes(&serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap());
        assert!(lacking.contains("sys:Fx_Cyclop_Attack4_Lapidation"));
        for k in ["sys:FX_Wendigo_Attack5_1_Missile", "sys:FX_Skeleton_Wizard_Attack1_Missile", "sys:FX_Wendigo_Attack5_6_Missile", "sys:FX_Goblin_Shaman_Attack1_Missile"] {
            assert!(!lacking.contains(k), "{k}");
        }
    }

    #[test]
    fn event_handlers_find_their_source_emitter() {
        // a bolt mesh spawned at a ribbon's location events, a flash at a renderer-less source's deaths.
        let v = serde_json::json!({"sys:T": {"emitters": [
            {"name": "Arc", "lifetime": [0.25, 0.25], "spawn": {"bursts": [{"count": 20, "time": 0.0}]}, "renderer": {"type": "Ribbon", "texture": "t.png"}, "color": {"over_life": [[0.0, 1, 1, 1, 1]]}},
            {"name": "Source", "lifetime": [0.1, 0.1], "spawn": {"bursts": [{"count": 1, "time": 0.0}]}, "renderer": null},
            {"name": "Flash", "lifetime": [0.2, 0.2], "spawn": {"bursts": [], "events": [{"source_event": "DeathEvent", "source_emitter": "Source", "mode": 1, "spawn_number": 3, "random_spawn_number": true, "min_spawn_number": 2, "max_events_per_frame": 2}]},
             "renderer": {"type": "Sprite", "texture": "t.png"}, "color": {"over_life": [[0.0, 1, 1, 1, 1]]}},
            {"name": "Bolt", "lifetime": [0.2, 0.2], "spawn": {"bursts": [], "events": [{"source_event": "LocationEvent", "source_emitter": "Arc", "mode": 1, "spawn_number": 1, "max_events_per_frame": 15}]},
             "renderer": {"type": "Sprite", "texture": "t.png"}, "color": {"over_life": [[0.0, 1, 1, 1, 1]]}}
        ]}});
        let lib = parse_library(&v);
        let em = &lib["sys:T"];
        let names: Vec<&str> = em.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Arc", "Source", "Flash", "Bolt"]);
        assert_eq!(em[1].draw, Draw::Hidden);
        assert_eq!((em[0].sends, em[1].sends), (1, 2));
        assert_eq!(em[2].events, vec![EventSpawn { source_name: "Source".into(), source: 1, death: true, count: (2, 3), per_frame: 2 }]);
        assert_eq!((em[3].events[0].source, em[3].events[0].death, em[3].events[0].count), (0, false, (1, 1)));
    }

    #[test]
    fn cone_directions_stay_inside_the_cone() {
        let mut seed = 12345;
        for _ in 0..500 {
            let d = in_cone(Vec3::X, 30.0, &mut seed);
            assert!((d.length() - 1.0).abs() < 1e-4);
            assert!(d.angle_between(Vec3::X).to_degrees() <= 30.0 + 1e-3);
        }
    }

    #[test]
    /// `P4_PROBE=<key> cargo test -p db_client probe_effect -- --nocapture`: the parsed emitters of one effect.
    fn probe_effect() {
        let (Ok(key), Some(p)) = (std::env::var("P4_PROBE"), crate::map_view::data_file("recovered/effects/effects.json")) else { return };
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        for s in parse_library(&v).remove(&key).unwrap_or_default() {
            println!("{} {:?} {:?} tex {:?} life {:?} size {:?} col0 {:?} col.5 {:?} burst {:?} rate {:?}", s.name, s.draw, s.blend, s.texture, s.lifetime, s.size_cm.first(), s.color_at(0.0), s.color_at(0.5), s.bursts, s.rate);
        }
    }

    #[test]
    fn the_extracted_effects_parse() {
        let Some(p) = crate::map_view::data_file("recovered/effects/effects.json") else { return };
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        for (k, fx) in v.as_object().unwrap() {
            for e in fx["emitters"].as_array().into_iter().flatten() {
                let mut e = e.clone();
                gpu_standin(k, &mut e);
                let Some(s) = EmitterSpec::parse(&e) else { continue };
                assert!(s.lifetime.0 >= 0.0 && s.lifetime.1 >= s.lifetime.0, "{k}/{}: lifetime {:?}", s.name, s.lifetime);
                assert!(s.colors.iter().all(|c| (0.0..=1.0).contains(&c.1[3])), "{k}/{}: alpha", s.name);
            }
        }
    }

    #[test]
    fn sprites_face_and_align_by_their_renderer_modes() {
        let spec = |facing: u8, align: u8, f: [f32; 3]| {
            let e = serde_json::json!({"lifetime": [1, 1], "renderer": {"type": "Sprite", "texture": "t.png", "facing": facing, "alignment": align}, "sprite_facing": f, "size": {"initial_xy": [[10, 10, 10], [40, 40, 40]]}});
            EmitterSpec::parse(&e).unwrap()
        };
        // The camera looks along -Z from +Z (right +X, up +Y, back +Z); the particle is 3 m in front of it.
        let cam = (Vec3::X, Vec3::Y, Vec3::Z);
        let eye = Some(Vec3::Z * 3.0);
        let close = |a: Vec3, b: Vec3| a.distance(b) < 1e-4;
        // Default: the camera plane.
        let (r, u, n) = sprite_axes(&spec(0, 0, [1.0, 0.0, 0.0]), Quat::IDENTITY, Vec3::ZERO, eye, cam, 0.0);
        assert!(close(r, Vec3::X) && close(u, Vec3::Y) && close(n, Vec3::Z));
        // A custom facing of UE +Z lies flat on the floor (Bevy +Y up), its right still the camera's.
        let floor = spec(2, 0, [0.0, 0.0, 1.0]);
        let (r, u, n) = sprite_axes(&floor, Quat::IDENTITY, Vec3::ZERO, eye, cam, 0.0);
        assert!(close(n, Vec3::Y) && close(r, Vec3::X) && close(u, -Vec3::Z), "{r} {u} {n}");
        // ... and turns with the emitter's frame.
        let (_, _, n) = sprite_axes(&floor, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::ZERO, eye, cam, 0.0);
        assert!(close(n, Vec3::Z));
        // Velocity aligned: its up (the 40 cm length) runs along the motion, facing the camera as far as it can.
        let streak = spec(0, 1, [1.0, 0.0, 0.0]);
        assert_eq!(streak.aspect, Vec2::new(1.0, 4.0));
        let (r, u, n) = sprite_axes(&streak, Quat::IDENTITY, Vec3::X * 5.0, eye, cam, 0.0);
        assert!(close(u, Vec3::X) && close(r, -Vec3::Y) && close(n, Vec3::Z), "{r} {u} {n}");
        // Not moving, or moving at the camera: unaligned, not collapsed.
        let (r, u, _) = sprite_axes(&streak, Quat::IDENTITY, Vec3::ZERO, eye, cam, 0.0);
        assert!(close(r, Vec3::X) && close(u, Vec3::Y));
        let (r, _, _) = sprite_axes(&streak, Quat::IDENTITY, Vec3::Z, eye, cam, 0.0);
        assert!(close(r, Vec3::X));
        // A custom facing the emitter never writes faces the camera.
        let (_, _, n) = sprite_axes(&EmitterSpec { facing_vec: None, ..floor.clone() }, Quat::IDENTITY, Vec3::ZERO, eye, cam, 0.0);
        assert!(close(n, Vec3::Z));
        // The rotation turns right and up about the normal.
        let (r, u, n) = sprite_axes(&spec(0, 0, [1.0, 0.0, 0.0]), Quat::IDENTITY, Vec3::ZERO, eye, cam, std::f32::consts::FRAC_PI_2);
        assert!(close(r, Vec3::Y) && close(u, -Vec3::X) && close(n, Vec3::Z));
    }
}

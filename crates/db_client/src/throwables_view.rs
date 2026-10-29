//! The alchemical flasks in the client: their keys, and their blasts and areas where they land.

use bevy::prelude::*;
use db_sim::throwables::FlaskDb;
use db_sim::{CommandQueue, Inventory, SimCommand, SimEvent, SimIndex};

use crate::LocalPlayer;
use crate::effects::{EffectLibrary, spawn_effect};
use crate::settings::Bind;

/// The flask keys in order, by the flask kind they throw (the data's names: `ThunderPotion_0101` is a
/// `ThunderPotion`). The keys are the port's (the game throws the flask held in a quick slot with the left button).
const KEYS: [(Bind, &str); 5] = [(Bind::Flask1, "BurningPotion"), (Bind::Flask2, "ThunderPotion"), (Bind::Flask3, "SmokePotion"), (Bind::Flask4, "EarthPotion"), (Bind::Torch, "Torch")];

/// A flask's effects by kind: its trail, and its blast or area (`FX_Bottle_<X>_Cast`, `_Range`).
pub(crate) fn effects_of(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "BurningPotion" => Some(("flask_fire_trail", "flask_fire_area")),
        "ThunderPotion" => Some(("flask_thunder_trail", "flask_thunder_blast")),
        "SmokePotion" => Some(("flask_smoke_trail", "flask_smoke_area")),
        // The walls are its landing (`stone_walls`); `FX_Bottle_Wall`'s smoke as they rise.
        "EarthPotion" => Some(("flask_earth_trail", "flask_wall")),
        // The torch burns in flight and where it lies (`lit_torches`): its pickup's `PS_Torch_Fire_FX_01`.
        "Torch" => Some((TORCH_FIRE, "")),
        _ => None,
    }
}

/// What flies: a flask's bottle, or the torch.
pub(crate) fn model_of(kind: &str) -> String {
    match kind {
        "Torch" => "models/Dungeonborne/Content/TGF/Equipment/Item/Torch/WP_Torch_0001/Mesh/SM_Torch_Handheld_01.glb".to_string(),
        _ => format!("models/Dungeonborne/Content/TGF/Equipment/Item/AlchemicalPotion/CSM_AlchemicalPotion/SM_AlchemicalPotion_{kind}.glb"),
    }
}

/// `--auto-flask <1-5>`: the bot throws that key's flask whenever it can (checks).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct AutoFlask(pub Option<usize>);

/// A flask key throws the first flask of its kind in the backpack.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn flask_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::settings::Keys>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    flasks: Option<Res<FlaskDb>>,
    inventories: Query<&Inventory>,
    mut queue: ResMut<CommandQueue>,
    (auto, state, tick): (Option<Res<AutoFlask>>, Query<(Option<&db_sim::throwables::Throwing>, Option<&db_sim::throwables::FlaskCooldown>)>, Res<db_sim::TickCount>),
) {
    let (Some(id), Some(flasks)) = (player.0, flasks) else { return };
    let bot = auto.and_then(|a| a.0).filter(|_| index.get(id).and_then(|e| state.get(e).ok()).is_some_and(|(t, c)| t.is_none() && c.is_none_or(|c| tick.0 >= c.0)));
    for (i, (bind, kind)) in KEYS.into_iter().enumerate() {
        if !keys.just_pressed(binds.get(bind)) && bot != Some(i + 1) {
            continue;
        }
        // The torch key takes a torch in hand or puts it away (it's thrown and raised from the hand).
        if kind == "Torch" {
            queue.push(SimCommand::TorchKey { id });
            continue;
        }
        let uid = index.get(id).and_then(|e| inventories.get(e).ok()).and_then(|inv| inv.bag.iter().find(|b| flasks.0.get(&b.item.item.item_id).is_some_and(|f| f.kind() == kind)).map(|b| b.item.uid));
        if let Some(uid) = uid {
            queue.push(SimCommand::ThrowItem { id, uid });
        }
    }
}

/// Dev (`--auto-torch <frame>`): from that frame the torch is taken in hand, raised 3 s later and held up 2 s, then
/// thrown at 7.5 s (shots of each, the first torch body loads its clips for a while in a hidden run).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoTorch(pub Option<u32>);

pub(crate) fn auto_torch(auto: Res<AutoTorch>, frames: Res<bevy::diagnostic::FrameCount>, player: Res<LocalPlayer>, tick: Res<db_sim::TickCount>, mut queue: ResMut<CommandQueue>, mut from: Local<Option<u64>>, mut done: Local<u8>) {
    let (Some(at), Some(id)) = (auto.0, player.0) else { return };
    if frames.0 < at {
        return;
    }
    let t0 = *from.get_or_insert(tick.0);
    let secs = (tick.0 - t0) as f32 * db_sim::DT;
    let steps: [(f32, SimCommand); 4] = [(0.0, SimCommand::TorchKey { id }), (3.0, SimCommand::SetBlock { id, on: true }), (5.0, SimCommand::SetBlock { id, on: false }), (7.5, SimCommand::Attack { id, kind: db_sim::AttackKind::Light })];
    for (i, (when, c)) in steps.into_iter().enumerate() {
        if usize::from(*done) == i && secs >= when {
            queue.push(c);
            *done += 1;
        }
    }
    // (The input system sends the block button's state every frame: held up here until let go.)
    if (1..3).contains(&*done) {
        queue.push(SimCommand::SetBlock { id, on: true });
    }
}

/// Dev (`--auto-drink <frame>`): a healing potion given at that frame and drunk the next (shots).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoDrink(pub Option<u32>);

pub(crate) fn auto_drink(auto: Res<AutoDrink>, frames: Res<bevy::diagnostic::FrameCount>, player: Res<LocalPlayer>, index: Res<SimIndex>, inventories: Query<&Inventory>, mut queue: ResMut<CommandQueue>, mut done: Local<u8>) {
    let (Some(at), Some(id)) = (auto.0, player.0) else { return };
    if frames.0 < at || *done >= 2 {
        return;
    }
    if *done == 0 {
        queue.push(SimCommand::Inventory(db_sim::InventoryCommand::Give { id, item_id: 100_013 }));
        *done = 1;
    } else if let Some(uid) = index.get(id).and_then(|e| inventories.get(e).ok()).and_then(|inv| inv.bag.iter().find(|b| b.item.item.item_id == 100_013)).map(|b| b.item.uid) {
        queue.push(SimCommand::UseItem { id, uid });
        *done = 2;
    }
}

/// In the arena the player starts with a stack of each flask (not their "Competitive" twins).
pub(crate) fn arena_flasks(arena: Res<crate::Arena>, player: Res<LocalPlayer>, flasks: Option<Res<FlaskDb>>, mut given: Local<Option<db_sim::SimId>>, mut queue: ResMut<CommandQueue>) {
    let (Some(id), Some(flasks)) = (player.0.filter(|_| arena.0), flasks) else { return };
    if *given == Some(id) {
        return;
    }
    *given = Some(id);
    // (100012 is the torch every class starts with, `ExcelRoleDefaultEquipment`.)
    for (&item_id, _) in flasks.0.iter().filter(|(id, s)| s.name.ends_with("_0101") || **id == 100_012) {
        queue.push(SimCommand::Inventory(db_sim::InventoryCommand::GiveStarting { id, item_id, count: 3 }));
    }
}

/// Where a flask lands: its blast, or its area for its time.
pub(crate) fn flask_effects(mut commands: Commands, lib: Res<EffectLibrary>, flasks: Option<Res<FlaskDb>>, events: Res<crate::combat_view::FrameEvents>) {
    let Some(flasks) = flasks else { return };
    for ev in &events.0 {
        if let SimEvent::FlaskLanded { item_id, at, secs, .. } = *ev
            && let Some((_, key)) = flasks.0.get(&item_id).and_then(|f| effects_of(f.kind())).filter(|(_, k)| !k.is_empty())
        {
            let fx = spawn_effect(&mut commands, &lib, key, crate::to_bevy(at), Quat::IDENTITY, (secs > 0.0).then_some(secs));
            debug!("flask {item_id} landed at {at:?}: {key} {}", if fx.is_some() { "playing" } else { "not in the effect library" });
        }
    }
}

/// The Stoneshaping wall's look (`FX_Bottle_Wall`): its mesh emitters draw `SM_WallMud_2x4_04_Lowpoly` in the
/// wall's frame, scaled from 0.5 to 0.8 over their 0.25 s life (CONFIRMED data); the decoder has no spawn data for
/// the system, so it is drawn as a static model, rising so (HYPOTHESIS: its frame and how it holds after).
pub(crate) fn wall_model(class: &str) -> Option<(&'static str, f32)> {
    (class == "BP_Destructible_Wall").then_some(("models/TgfMaps/SceneAssets/Wooden_Trench/Mud/Mesh/SM_WallMud_2x4_04_Lowpoly.glb", WALL_SCALE))
}

const WALL_SCALE: f32 = 0.8;
const WALL_FROM: f32 = 0.5;
const WALL_RISE: f32 = 0.25;

/// A wall's view rising to its full scale.
#[derive(Component)]
pub(crate) struct WallRise {
    pub(crate) age: f32,
    pub(crate) scale: f32,
}

/// Walls rise as they appear, and crumble (`FX_Bottle_Wall_End`) when broken or out of time.
pub(crate) fn stone_walls(
    mut commands: Commands,
    time: Res<Time>,
    lib: Res<EffectLibrary>,
    mut rising: Query<(Entity, &mut WallRise, &mut Transform)>,
    walls: Query<(&db_sim::SimId, &db_sim::Destroyable, &db_sim::Health), With<db_sim::destroyables::Expires>>,
    mut crumbled: Local<std::collections::BTreeSet<db_sim::SimId>>,
) {
    for (e, mut r, mut t) in &mut rising {
        r.age += time.delta_secs();
        let k = (r.age / WALL_RISE).min(1.0);
        t.scale = Vec3::splat(r.scale / WALL_SCALE * (WALL_FROM + (WALL_SCALE - WALL_FROM) * k));
        if k >= 1.0 {
            commands.entity(e).remove::<WallRise>();
        }
    }
    crumbled.retain(|id| walls.iter().any(|(w, _, _)| w == id));
    for (&id, d, h) in &walls {
        if h.current <= 0.0 && crumbled.insert(id) {
            spawn_effect(&mut commands, &lib, "flask_wall_end", crate::to_bevy(d.origin), Quat::from_rotation_z(d.yaw), None);
        }
    }
}

const TORCH_FIRE: &str = "sys:PS_Torch_Fire_FX_01";

/// A lit torch's light and fire: `BP_CollectableBase_Torch`'s `PointLight1` (radius 600, intensity 6, FFB667, no
/// shadows) and `PS_Torch_Fire_FX_01`, 33-35 cm over its pickup (CONFIRMED); its light rising 10 cm as it settles is
/// left out.
#[derive(Component)]
pub(crate) struct TorchGlow {
    id: db_sim::SimId,
    fire: Option<Entity>,
}

const TORCH_LIGHT_CM: f32 = 600.0;
const TORCH_INTENSITY: f32 = 6.0;
const TORCH_HEIGHT: f32 = 35.0;

pub(crate) fn lit_torches(
    mut commands: Commands,
    lib: Res<EffectLibrary>,
    index: Res<SimIndex>,
    lit: Query<(&db_sim::SimId, &db_sim::Position), With<db_sim::throwables::Lit>>,
    glows: Query<(Entity, &TorchGlow)>,
) {
    for (e, g) in &glows {
        if index.get(g.id).is_none_or(|s| !lit.contains(s)) {
            if let Some(f) = g.fire {
                crate::effects::stop(&mut commands, f);
            }
            commands.entity(e).despawn();
        }
    }
    for (&id, p) in &lit {
        if glows.iter().any(|(_, g)| g.id == id) {
            continue;
        }
        let at = crate::to_bevy(p.0 + Vec3::Z * TORCH_HEIGHT);
        let fire = spawn_effect(&mut commands, &lib, TORCH_FIRE, at, Quat::IDENTITY, None);
        let light = PointLight { color: Color::srgb_u8(0xFF, 0xB6, 0x67), intensity: TORCH_INTENSITY * crate::map_view::LUMENS_PER_UNIT, range: TORCH_LIGHT_CM * 0.01, shadow_maps_enabled: false, ..default() };
        commands.spawn((light, Transform::from_translation(at), TorchGlow { id, fire }, crate::flow::LevelScoped));
    }
}

/// The Incendiary's burning ground (`BP_Bottle_Fire_Decal`: `DecalSize` 333 cm across, `MI_FX_DecalFire_101T`'s mask
/// `TX_FX_Decal_104`, CONFIRMED) as a see-through quad on the floor for the area's life. Its material
/// (`M_FX_Common_Decal`: the mask over `TX_FX_Decal_105` with its glow curve) isn't rebuilt: the mask tinted as
/// embers, fading over the last seconds (HYPOTHESIS look).
#[derive(Component)]
pub(crate) struct FireGround(db_sim::SimId, u64);

const FIRE_DECAL: &str = "textures/Dungeonborne/Content/TGF/VFX/Textures/TX_FX_Decal_104.png";
const FIRE_DECAL_CM: f32 = 333.0;
/// Its tint and the seconds it fades out over (ours).
const FIRE_TINT: [f32; 3] = [1.0, 0.45, 0.18];
const FIRE_FADE: f32 = 2.0;
/// Height of the quad over the floor (cm; ours, against z-fighting).
const LIFT: f32 = 5.0;

#[allow(clippy::too_many_arguments)]
pub(crate) fn fire_ground(
    mut commands: Commands,
    areas: Option<Res<db_sim::throwables::FlaskAreas>>,
    tick: Option<Res<db_sim::TickCount>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    views: Query<(Entity, &FireGround, &MeshMaterial3d<StandardMaterial>)>,
) {
    let (Some(areas), Some(tick)) = (areas, tick) else { return };
    for (e, g, m) in &views {
        match areas.0.iter().find(|a| a.owner == g.0 && a.born == g.1) {
            Some(a) => {
                let left = a.until.saturating_sub(tick.0) as f32 * db_sim::DT;
                if left < FIRE_FADE
                    && let Some(mut mat) = materials.get_mut(&m.0)
                {
                    mat.base_color.set_alpha((left / FIRE_FADE).min(1.0));
                }
            }
            None => commands.entity(e).despawn(),
        }
    }
    for a in areas.0.iter().filter(|a| a.dot.is_some()) {
        if views.iter().any(|(_, g, _)| g.0 == a.owner && g.1 == a.born) {
            continue;
        }
        let [r, g, b] = FIRE_TINT;
        let material = materials.add(StandardMaterial { base_color: Color::linear_rgba(r, g, b, 1.0), base_color_texture: Some(assets.load(FIRE_DECAL)), alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
        let size = FIRE_DECAL_CM * 0.01;
        commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(size, size))), MeshMaterial3d(material), Transform::from_translation(crate::to_bevy(a.at + Vec3::Z * LIFT)), bevy::light::NotShadowCaster, FireGround(a.owner, a.born)));
    }
}

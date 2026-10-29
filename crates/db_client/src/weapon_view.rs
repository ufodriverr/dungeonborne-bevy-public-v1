//! First-person weapon from the equipped item. The right-hand item's converted static mesh comes from the generated table [`WEAPON_MESHES`] (the mesh its `ID_*` blueprint references,
//! else its weapon folder's `SM_*.glb`, else a same-type stand-in). It replaces the placeholder blade under [`ViewSword`], which `combat_view::animate_viewmodel` keeps posing; the
//! placeholder stays when the mesh isn't imported. Since the first-person body (`fp_body`) takes over whenever a clip set matches the worn weapons; this view remains the fallback.

use bevy::prelude::*;
use db_sim::{Inventory, SimIndex};
use db_formats::config::generated::EnItemWearingPart;

use crate::combat_view::{LocalPlayer, ViewSword};
use crate::weapon_meshes::WEAPON_MESHES;

/// Dev: show this item's weapon instead of the equipped one (`--weapon <item id>`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ViewWeapon(pub Option<i32>);

/// Dev: give the local player this item once and wear it in the right hand, and the second one in the left
/// (`--wield <item id>[,<item id>]`: the legendary staff in hidden runs: weapon pairs). Goes through the sim's
/// own inventory commands.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevWield(pub Option<i32>, pub Option<i32>);

/// Dev (shots): press interact on the current target at this render frame and hold (`--interact-at <frame>`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevInteractAt(pub Option<u32>);

pub(crate) fn dev_interact(at: Res<DevInteractAt>, frames: Res<bevy::diagnostic::FrameCount>, player: Res<LocalPlayer>, target: Res<crate::items_view::InteractTarget>, mut queue: ResMut<db_sim::CommandQueue>) {
    if let (Some(at), Some(id), Some(t)) = (at.0, player.0, target.0)
        && frames.0 == at
    {
        queue.push(db_sim::SimCommand::Interact { id, target: t });
    }
}

/// Dev (shots): turn every mechanism on at this render frame (`--trigger-at <frame>`): and set off every floor's
/// collapse.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevTriggerAt(pub Option<u32>);

pub(crate) fn dev_trigger(at: Res<DevTriggerAt>, frames: Res<bevy::diagnostic::FrameCount>, mechs: Query<&db_sim::mechanisms::Mechanism>, collapses: Query<&db_sim::floor_collapse::FloorCollapse>, mut queue: ResMut<db_sim::CommandQueue>) {
    if at.0 == Some(frames.0) {
        for m in &mechs {
            queue.push(db_sim::SimCommand::TriggerMechanism { index: m.placement.index, param: 1 });
        }
        for c in &collapses {
            queue.push(db_sim::SimCommand::TriggerFloorCollapse { floor: c.floor });
        }
    }
}

/// Dev (shots): every Shadow King casts his `Attack8` (the Shadow Dragon's summon) at this render frame
/// (`--summon-dragon-at <frame>`); the local player gets a billion health, so the woken King doesn't end the run before
/// the dragon's breath.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevSummonDragonAt(pub Option<u32>);

pub(crate) fn dev_summon_dragon(at: Res<DevSummonDragonAt>, frames: Res<bevy::diagnostic::FrameCount>, kings: Query<(&db_sim::SimId, &db_sim::MonsterKit)>, player: Res<LocalPlayer>, index: Res<SimIndex>, combatants: Query<&db_sim::Combatant>, mut queue: ResMut<db_sim::CommandQueue>) {
    if at.0 != Some(frames.0) {
        return;
    }
    if let Some(id) = player.0
        && let Some(c) = index.get(id).and_then(|e| combatants.get(e).ok())
    {
        queue.push(db_sim::SimCommand::SetCombatant { id, combatant: Box::new(*c), max_health: 1e9 });
        info!("dev: the player gets 1e9 health");
    }
    let mut n = 0;
    for (id, kit) in &kings {
        if let Some(index) = kit.king.is_some().then(|| kit.by_class("GA_TGF_M_ShadowKing_Attack8_C")).flatten() {
            queue.push(db_sim::SimCommand::MonsterAttack { id: *id, index });
            n += 1;
        }
    }
    info!("dev: {n} Shadow King(s) cast Attack8");
}

/// Dev (shots): the player swings once (a light attack) at this render frame (`--attack-at <frame>`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevAttackAt(pub Option<u32>);

pub(crate) fn dev_attack(at: Res<DevAttackAt>, frames: Res<bevy::diagnostic::FrameCount>, player: Res<LocalPlayer>, mut queue: ResMut<db_sim::CommandQueue>) {
    if at.0 == Some(frames.0)
        && let Some(id) = player.0
    {
        queue.push(db_sim::SimCommand::Attack { id, kind: db_sim::AttackKind::Light });
        info!("dev: the player swings");
    }
}

/// Dev (shots): switch weapon sets at this render frame (`--switch-at <frame>`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DevSwitchAt(pub Option<u32>);

pub(crate) fn dev_switch(at: Res<DevSwitchAt>, frames: Res<bevy::diagnostic::FrameCount>, player: Res<LocalPlayer>, mut queue: ResMut<db_sim::CommandQueue>) {
    if let (Some(at), Some(id)) = (at.0, player.0)
        && frames.0 == at
    {
        queue.push(db_sim::SimCommand::SwitchWeaponPair { id });
    }
}

pub(crate) fn dev_wield(wield: Res<DevWield>, player: Res<LocalPlayer>, index: Res<SimIndex>, inventories: Query<&Inventory>, combos: Option<Res<db_sim::weapons::WeaponCombos>>, mut queue: ResMut<db_sim::CommandQueue>, mut given: Local<[bool; 2]>) {
    let Some(id) = player.0 else { return };
    let Some(inv) = index.get(id).and_then(|e| inventories.get(e).ok()) else { return };
    for (k, (item, slot)) in [(wield.0, EnItemWearingPart::Pair1rightweapon), (wield.1, EnItemWearingPart::Pair1leftweapon)].into_iter().enumerate() {
        let Some(item_id) = item else { continue };
        if inv.worn.get(&slot).is_some_and(|w| w.item.item_id == item_id) {
            continue;
        }
        // The right hand first (a pair's left item may need it).
        if k == 1 && wield.0.is_some_and(|r| inv.worn.get(&EnItemWearingPart::Pair1rightweapon).is_none_or(|w| w.item.item_id != r)) {
            continue;
        }
        // A second item of the same kind: the one not worn in the right hand.
        let right_uid = inv.worn.get(&EnItemWearingPart::Pair1rightweapon).map(|w| w.uid);
        match inv.bag.iter().find(|b| b.item.item.item_id == item_id && Some(b.item.uid) != right_uid) {
            Some(b) => queue.push(db_sim::SimCommand::Inventory(db_sim::InventoryCommand::Equip { id, uid: b.item.uid, slot })),
            None if !given[k] => {
                given[k] = true;
                queue.push(db_sim::SimCommand::Inventory(db_sim::InventoryCommand::Give { id, item_id }));
                // A crossbow comes with arrows: the 15 the classes that start with one carry (`excel_role_default_equipment`).
                let ammo = crate::fp_body::weapon_type(item_id).and_then(|w| combos.as_ref()?.crossbows.get(w.0).map(|c| c.ammo_item));
                for _ in 0..ammo.map_or(0, |_| 15) {
                    queue.push(db_sim::SimCommand::Inventory(db_sim::InventoryCommand::Give { id, item_id: ammo.unwrap_or_default() }));
                }
            }
            None => {}
        }
    }
}

/// The placeholder blade, guard and grip (hidden while a real mesh shows).
#[derive(Component)]
pub(crate) struct PlaceholderSword;

/// A converted weapon mesh in view.
#[derive(Component)]
pub(crate) struct WeaponModel;

/// Keeps the view weapon on the local player's right-hand item (or the dev override).
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_view_weapon(
    mut commands: Commands,
    assets: Res<AssetServer>,
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    dev: Res<ViewWeapon>,
    inventories: Query<&Inventory>,
    sword: Query<Entity, With<ViewSword>>,
    mut placeholder: Query<&mut Visibility, With<PlaceholderSword>>,
    models: Query<Entity, With<WeaponModel>>,
    fp: Query<(), With<crate::fp_body::FpReady>>,
    mut shown: Local<Option<(Entity, Option<i32>, bool)>>,
) {
    let Ok(sword) = sword.single() else { return };
    let worn = player.0.and_then(|id| index.get(id)).and_then(|e| inventories.get(e).ok()).and_then(|inv| inv.worn.get(&EnItemWearingPart::Pair1rightweapon)).map(|i| i.item.item_id);
    let want = dev.0.or(worn);
    // The first-person body carries its own weapons.
    let body = !fp.is_empty();
    // A new view sword (each run builds one), a different item or the body coming or going: rebuild.
    if *shown == Some((sword, want, body)) {
        return;
    }
    *shown = Some((sword, want, body));
    for m in &models {
        commands.entity(m).despawn();
    }
    let path = want.and_then(|id| WEAPON_MESHES.iter().find(|w| w.0 == id)).map(|w| w.1).filter(|p| crate::model_view::imported(p));
    for mut v in &mut placeholder {
        *v = if path.is_some() || body { Visibility::Hidden } else { Visibility::Inherited };
    }
    if body {
        return;
    }
    let Some(path) = path else { return };
    // The meshes lie along +X with the grip at the origin; the placeholder's blade runs up its +Y with
    // the edge facing sideways, so +X turns to +Y and the flat of the blade faces the camera.
    let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    commands.entity(sword).with_children(|s| {
        s.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))), Transform::from_rotation(turn), Visibility::default(), WeaponModel));
    });
}

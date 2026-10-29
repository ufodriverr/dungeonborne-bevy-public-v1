//! Potion bottles: their glass (`MI_Glass`, on `M_Glass`: refraction, `AlphaAdd`) comes in with the bottle's
//! opaque texture and hid the liquid, and their liquids (`MI_vfx_blood_1xx_DynamicLiquid(s)`, on
//! `MA_vfx_ContainerDynamicLiquids`) come in untextured white. The glass is drawn clear (HYPOTHESIS look: no
//! refraction), each liquid in its material's `LiquidColor` (CONFIRMED, the materials' vector parameters).

use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;

/// `LiquidColor` (linear RGB) by the liquid material's number.
const LIQUIDS: [(&str, [f32; 3]); 9] = [
    ("114", [0.3, 0.001, 0.001]),
    ("116", [0.5, 0.427, 0.053]),
    ("117", [0.022, 0.067, 0.405]),
    ("118", [0.401, 0.401, 0.401]),
    ("121", [0.854, 0.227, 0.0]),
    ("122", [0.114, 0.735, 0.854]),
    ("123", [0.024, 0.854, 0.231]),
    ("124", [0.092, 0.132, 0.432]),
    ("125", [0.074, 0.136, 0.854]),
];

fn liquid(name: &str) -> Option<[f32; 3]> {
    let n = name.strip_prefix("MI_vfx_blood_")?.get(..3)?;
    LIQUIDS.iter().find(|l| l.0 == n).map(|l| l.1)
}

/// Swaps the glass and liquid materials of newly loaded meshes.
pub(crate) fn potion_materials(mut commands: Commands, meshes: Query<(Entity, &GltfMaterialName), Added<GltfMaterialName>>, mut materials: ResMut<Assets<StandardMaterial>>, mut cache: Local<Option<Handle<StandardMaterial>>>, mut liquids: Local<Vec<(&'static str, Handle<StandardMaterial>)>>) {
    for (e, name) in &meshes {
        let handle = if name.0 == "MI_Glass" {
            cache.get_or_insert_with(|| materials.add(StandardMaterial { base_color: Color::srgba(0.9, 0.95, 1.0, 0.18), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, metallic: 0.0, reflectance: 0.6, ..default() })).clone()
        } else if let Some(c) = liquid(&name.0) {
            let key = LIQUIDS.iter().find(|l| l.1 == c).map_or("", |l| l.0);
            match liquids.iter().find(|l| l.0 == key) {
                Some(l) => l.1.clone(),
                None => {
                    let h = materials.add(StandardMaterial { base_color: Color::linear_rgb(c[0], c[1], c[2]), perceptual_roughness: 0.1, metallic: 0.0, ..default() });
                    liquids.push((key, h.clone()));
                    h
                }
            }
        } else {
            continue;
        };
        commands.entity(e).insert(MeshMaterial3d(handle));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn liquids_by_their_material() {
        assert_eq!(super::liquid("MI_vfx_blood_114_DynamicLiquids"), Some([0.3, 0.001, 0.001]));
        assert_eq!(super::liquid("MI_vfx_blood_117_DynamicLiquid"), Some([0.022, 0.067, 0.405]));
        assert_eq!(super::liquid("MI_CSM_HealingPotion_01"), None);
    }
}

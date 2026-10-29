//! Combat presentation: actor meshes, the first-person sword, a crosshair, the dummy's health
//! label and floating damage numbers. Everything here only reads sim state and events.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::text::FontSize;
use db_formats::config::generated::{EnItemWearingPart, EnRoleCareerType};
use db_sim::attributes::{AttributeSheet, CLASSES, STARTING_SLOTS, class_sheet, class_type_id, starting_gear, starting_loadout};
use db_sim::combat::{AttackStats, DefenceStats};
use db_sim::items::ItemDb;
use db_sim::level::Surface;
use db_sim::melee::SwingPhase;
use db_sim::monsters::{BOSSES, MonsterKit, ROSTER, load_kit, load_monster_db};
use db_sim::skills::{FighterSkillData, load_fighter_skills};
use db_sim::{ActorKind, AiSenses, Combatant, CommandQueue, DT, GameData, GroundItem, Health, InventoryCommand, Melee, Position, SimCommand, SimEvent, SimEvents, SimId, SimIndex, SkillKind, SkillKit};

use crate::{PlayerCamera, surface_material, to_bevy};

/// The local player's sim id, once the sim has spawned it.
#[derive(Resource, Debug, Default)]
pub struct LocalPlayer(pub Option<SimId>);

/// Dev/demo: attack whenever idle (screenshot runs use it to show damage numbers).
#[derive(Resource, Debug, Default)]
pub struct AutoAttack(pub bool);

/// Dev/demo: use Whirlwind whenever it is ready.
/// The bot uses its Q skill (`.0`) and its E skill (`.1`) whenever they are ready, from render frame `.2` on.
#[derive(Resource, Debug, Default)]
pub struct AutoSkill(pub bool, pub bool, pub u32);

/// Dev/demo: strafe left and right (exercises footsteps); the second field sprints while doing it
/// (`--auto-sprint`, exercises the stamina bar).
#[derive(Resource, Debug, Default)]
pub struct AutoWalk(pub bool, pub bool);

/// Sim events drained this frame, for every presentation system to read.
#[derive(Resource, Debug, Default)]
pub struct FrameEvents(pub Vec<SimEvent>);

pub(crate) fn drain_sim_events(mut sim: ResMut<SimEvents>, mut frame: ResMut<FrameEvents>) {
    frame.0.clear();
    frame.0.extend(sim.drain());
}

/// Stats sent to the sim when an actor spawns: (attack, defence, max health, guard reduction).
type Stats = (AttackStats, DefenceStats, f64, f64);

/// What the sim needs to run one monster type.
#[derive(Debug, Clone)]
pub struct MonsterData {
    stats: Stats,
    pub kit: MonsterKit,
    senses: AiSenses,
    pub speed: f32,
}

/// A class sheet plus the items worn at the start.
pub(crate) type Character = (Box<AttributeSheet>, Vec<(EnItemWearingPart, i32)>);

/// Combat stats built from the extracted data (gitignored; missing data leaves actors without
/// combat stats and logs a warning).
#[derive(Resource, Debug, Clone)]
pub struct CombatData {
    pub fighter_speed: f32,
    fighter: Option<Stats>,
    fighter_skills: Option<FighterSkillData>,
    /// SwordMaster and Rogue skills.
    class_skills: Vec<db_sim::ClassData>,
    dummy: Option<Stats>,
    /// Each roster monster (curve row → stats, attack kit, senses, move speed in cm/s).
    monsters: BTreeMap<&'static str, MonsterData>,
    /// The Fighter's class sheet and starting worn items (sent as `InventoryCommand::SetCharacter`).
    fighter_character: Option<Character>,
    /// Every class's level-1 sheet and starting worn items (test room `--class`).
    characters: Vec<(EnRoleCareerType, Character)>,
    /// Run length: map 1's timeout (2400 s); 40 minutes if the tables are missing.
    pub run_seconds: u32,
    /// Walk speed of each class (`affix_speed_move_speed_base`, level 1 sheet).
    speeds: Vec<(EnRoleCareerType, f32)>,
}

impl CombatData {
    /// Also returns the item tables for the sim (`GameData`) when the config cache is there.
    pub fn load() -> (Self, Option<GameData>) {
        let mut data = CombatData { fighter_speed: 330.0, fighter: None, fighter_skills: None, class_skills: Vec::new(), dummy: None, monsters: BTreeMap::new(), fighter_character: None, characters: Vec::new(), speeds: Vec::new(), run_seconds: 2400 };
        let mut game = None;
        match load_fighter_skills() {
            Ok(s) => data.fighter_skills = Some(s),
            Err(e) => warn!("Fighter skills unavailable: {e:#}"),
        }
        match db_sim::load_config_cache() {
            Some(t) => {
                let loadout = ItemDb::new(&t).and_then(|items| starting_loadout(&t, class_type_id(&t, CLASSES[0])?, 1, &items));
                match loadout {
                    Ok(l) => {
                        data.fighter_speed = l.sheet.get("affix_speed_move_speed_base") as f32;
                        data.fighter = Some((l.sheet.attack_stats(), l.sheet.defence_stats(), db_sim::gear_triggers::max_health(&l.sheet, 0.0), l.guard_reduce));
                    }
                    Err(e) => warn!("Fighter loadout: {e:#}"),
                }
                let character = class_type_id(&t, CLASSES[0]).and_then(|ty| Ok((class_sheet(&t, ty, 1)?, starting_gear(&t, ty)?)));
                match character {
                    Ok((sheet, gear)) => data.fighter_character = Some((Box::new(sheet), gear.into_iter().filter(|g| STARTING_SLOTS.contains(&g.slot)).map(|g| (g.slot, g.item_id)).collect())),
                    Err(e) => warn!("Fighter sheet: {e:#}"),
                }
                for c in CLASSES {
                    if let Ok((sheet, gear)) = class_type_id(&t, c).and_then(|ty| Ok((class_sheet(&t, ty, 1)?, starting_gear(&t, ty)?))) {
                        data.characters.push((c, (Box::new(sheet), gear.into_iter().filter(|g| STARTING_SLOTS.contains(&g.slot)).map(|g| (g.slot, g.item_id)).collect())));
                    }
                }
                if let Some(s) = db_sim::dungeon::run_seconds(&t, db_sim::dungeon::DUNGEON_MAP_ID) {
                    data.run_seconds = s.max(1) as u32;
                }
                for c in [EnRoleCareerType::Swordmaster, EnRoleCareerType::Rogue, EnRoleCareerType::Wizard, EnRoleCareerType::Lich, EnRoleCareerType::Priest, EnRoleCareerType::Deathknight, EnRoleCareerType::DruidPanther] {
                    match db_sim::class_skills::load_class_skills(&t, c) {
                        Ok(d) => data.class_skills.push(d),
                        Err(e) => warn!("{c:?} skills unavailable: {e:#}"),
                    }
                }
                for c in CLASSES {
                    if let Ok(sheet) = class_type_id(&t, c).and_then(|ty| class_sheet(&t, ty, 1)) {
                        data.speeds.push((c, sheet.get("affix_speed_move_speed_base") as f32));
                    }
                }
                match GameData::new(t) {
                    Ok(g) => game = Some(g),
                    Err(e) => warn!("item tables: {e:#}"),
                }
            }
            None => warn!("config cache missing (run `db_pipeline config`); Fighter has no combat stats, speed 330 cm/s"),
        }
        let monsters = load_monster_db();
        match monsters.as_ref().and_then(|m| m.get("Dummy")) {
            Some(d) => data.dummy = Some((d.attack_stats(), d.defence_stats(), d.max_health(), d.guard_reduce())),
            None => warn!("MonsterAttributeCurve JSON missing; the dummy has no health"),
        }
        for spec in ROSTER.iter().chain(BOSSES) {
            match (monsters.as_ref().and_then(|m| m.get(spec.row)), load_kit(spec)) {
                (Some(g), Ok(kit)) => {
                    let stats = (g.attack_stats(), g.defence_stats(), g.max_health(), g.guard_reduce());
                    data.monsters.insert(spec.row, MonsterData { stats, kit, senses: g.senses(), speed: g.move_speed() as f32 });
                }
                (_, Err(e)) => warn!("{} montages unavailable: {e:#}", spec.name),
                (None, _) => warn!("{} curve row missing", spec.name),
            }
        }
        (data, game)
    }

    /// A class's sheet and starting worn items.
    pub(crate) fn character(&self, career: EnRoleCareerType) -> Option<&Character> {
        if career == EnRoleCareerType::Fighter { self.fighter_character.as_ref() } else { self.characters.iter().find(|c| c.0 == career).map(|c| &c.1) }
    }

    /// A class's walk speed (cm/s); the Fighter's if unknown.
    pub fn speed(&self, career: EnRoleCareerType) -> f32 {
        self.speeds.iter().find(|s| s.0 == career).map_or(self.fighter_speed, |s| s.1)
    }

    /// A roster monster's data by curve row.
    pub fn monster(&self, row: &str) -> Option<&MonsterData> {
        self.monsters.get(row)
    }

    /// A monster type's move speed (cm/s); 0 if its data is missing.
    pub fn monster_speed(&self, row: &str) -> f32 {
        self.monster(row).map_or(0.0, |m| m.speed)
    }

    fn stats(&self, kind: ActorKind) -> Option<Stats> {
        match kind {
            ActorKind::Player => self.fighter,
            ActorKind::Dummy => self.dummy,
            // Monsters get theirs per type, with the kit, when they spawn.
            ActorKind::Monster | ActorKind::Wanderer | ActorKind::Chest | ActorKind::Item | ActorKind::Portal | ActorKind::Projectile | ActorKind::Summon | ActorKind::Door | ActorKind::Trap | ActorKind::Altar | ActorKind::Mechanism | ActorKind::Lever | ActorKind::Destroyable => None,
        }
    }
}

/// A client entity showing sim actor `0`.
#[derive(Component)]
pub(crate) struct ActorView(pub(crate) SimId);

/// The first-person sword (child of the camera).
#[derive(Component)]
pub(crate) struct ViewSword;

/// Screen text anchored to a sim position (cm) or following an actor.
#[derive(Component)]
pub struct WorldLabel {
    anchor: Vec3,
    follow: Option<SimId>,
    age: f32,
    /// Seconds until removed; `None` = permanent.
    life: Option<f32>,
    /// Upward drift over the label's life, cm.
    rise: f32,
    font_px: f32,
    /// Hidden beyond this distance from the eye (cm).
    range: f32,
}

impl WorldLabel {
    /// A permanent label at a fixed sim position (cm).
    pub fn at(pos: Vec3, font_px: f32) -> Self {
        WorldLabel { anchor: pos, follow: None, age: 0.0, life: None, rise: 0.0, font_px, range: LABEL_RANGE }
    }

    /// A permanent label `height` cm above actor `id`.
    pub fn follow(id: SimId, height: f32, font_px: f32) -> Self {
        WorldLabel { anchor: Vec3::new(0.0, 0.0, height), follow: Some(id), age: 0.0, life: None, rise: 0.0, font_px, range: LABEL_RANGE }
    }

    /// Shown only within `range` cm of the eye (ground items: the test arena's weapon racks were a pile of names).
    pub fn within(self, range: f32) -> Self {
        WorldLabel { range, ..self }
    }
}

/// Name and health over an actor.
#[derive(Component)]
pub(crate) struct HealthLabel {
    id: SimId,
    name: &'static str,
}

/// Skill cooldown readout (bottom left).
#[derive(Component)]
pub(crate) struct SkillHud;

/// Labels farther than this from the camera are hidden (cm; presentation choice).
const LABEL_RANGE: f32 = 2500.0;

/// Scale of the posed view weapon.
const VIEW_SWORD_SCALE: f32 = 0.6;

/// Damage number lifetime, seconds.
const NUMBER_LIFE: f32 = 1.4;

pub(crate) fn setup_hud(mut commands: Commands, cam: Query<Entity, With<PlayerCamera>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, assets: Res<AssetServer>) {
    // Reticle: the game's centre dot.
    commands.spawn((
        ImageNode::new(assets.load("textures/Dungeonborne/Content/TGF/UI/Game/Reticles/T_UI_Reticle_Center.png")),
        Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(50.0), width: px(47.0), height: px(47.0), margin: UiRect::all(px(-23.5)), ..default() },
        crate::flow::GameHud,
    ));
    commands.spawn((
        Text::new(""),
        crate::fonts::sans(22.0),
        TextColor(Color::srgb(0.95, 0.92, 0.85)),
        TextShadow::default(),
        Node { position_type: PositionType::Absolute, left: px(24.0), bottom: px(24.0), ..default() },
        SkillHud,
        crate::flow::GameHud,
    ));
    // Placeholder sword in view: blade, guard and grip; the equipped weapon's converted mesh replaces it
    // when imported (`weapon_view`).
    let Ok(cam) = cam.single() else { return };
    let metal = materials.add(StandardMaterial { metallic: 0.35, perceptual_roughness: 0.6, ..surface_material(Surface::Metal, Vec2::new(0.2, 1.0), &assets) });
    let wood = materials.add(surface_material(Surface::Wood, Vec2::ONE, &assets));
    commands.entity(cam).with_children(|c| {
        c.spawn((ViewSword, Transform::from_xyz(0.32, -0.32, -0.55).with_scale(Vec3::splat(VIEW_SWORD_SCALE)), Visibility::default())).with_children(|s| {
            s.spawn((crate::weapon_view::PlaceholderSword, Transform::default(), Visibility::default())).with_children(|s| {
                s.spawn((Mesh3d(meshes.add(Cuboid::new(0.045, 0.75, 0.012))), MeshMaterial3d(metal.clone()), Transform::from_xyz(0.0, 0.45, 0.0)));
                s.spawn((Mesh3d(meshes.add(Cuboid::new(0.2, 0.03, 0.04))), MeshMaterial3d(metal), Transform::from_xyz(0.0, 0.07, 0.0)));
                s.spawn((Mesh3d(meshes.add(Cylinder::new(0.018, 0.14))), MeshMaterial3d(wood), Transform::default()));
            });
        });
    });
}

pub(crate) fn spawn_label(commands: &mut Commands, text: String, color: Color, label: WorldLabel) -> Entity {
    let font_px = label.font_px;
    commands
        .spawn((
            Text::new(text),
            TextFont { font_size: FontSize::Px(font_px), ..default() },
            TextColor(color),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, ..default() },
            Visibility::Hidden,
            label,
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_events(
    mut commands: Commands,
    events: Res<FrameEvents>,
    mut player: ResMut<LocalPlayer>,
    mut queue: ResMut<CommandQueue>,
    data: Option<Res<CombatData>>,
    game: Option<Res<GameData>>,
    mut pending: ResMut<crate::flow::PendingCharacter>,
    mut spawns: ResMut<crate::dungeon_view::MonsterSpawns>,
    loadout: Res<crate::SkillLoadout>,
    test_class: Res<crate::TestClass>,
    index: Res<SimIndex>,
    (positions, ground, summoned, respawned): (Query<&Position>, Query<&GroundItem>, Query<&db_sim::summon::Summoned>, Query<&db_sim::respawn::Respawn>),
    (arena, observe): (Res<crate::Arena>, Res<crate::Observe>),
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    for e in events.0.iter().cloned() {
        match e {
            SimEvent::Spawned { id, kind } => {
                // With the item tables, the player's stats come from the class sheet plus worn items. In a run the
                // profile's character (`PendingCharacter`); in the test room the Fighter's starting gear.
                let character = if kind == ActorKind::Player && game.is_some() && player.0.is_none() {
                    // Test room: the Fighter, or the `--class` class (dev).
                    let career = test_class.0.unwrap_or(EnRoleCareerType::Fighter);
                    let fallback = data.as_ref().and_then(|d| if career == EnRoleCareerType::Fighter { d.fighter_character.clone() } else { d.characters.iter().find(|c| c.0 == career).map(|c| c.1.clone()) });
                    pending.0.take().or_else(|| fallback.map(|(sheet, worn)| InventoryCommand::SetCharacter { id, career, sheet, worn }))
                } else {
                    None
                };
                if character.is_none()
                    && let Some((attack, defence, max_health, guard_reduce)) = data.as_ref().and_then(|d| d.stats(kind))
                {
                    queue.push(SimCommand::SetCombatant { id, combatant: Box::new(Combatant { attack, defence, guard_reduce }), max_health });
                }
                if kind == ActorKind::Player && player.0.is_none() {
                    player.0 = Some(id);
                    let career = match &character {
                        Some(InventoryCommand::SetCharacter { career, .. } | InventoryCommand::SetCharacterItems { career, .. }) => *career,
                        _ => EnRoleCareerType::Fighter,
                    };
                    match character {
                        Some(InventoryCommand::SetCharacter { career, sheet, worn, .. }) => {
                            queue.push(SimCommand::Inventory(InventoryCommand::SetCharacter { id, career, sheet, worn }));
                            // The class's starting backpack (the crossbow classes' crossbow and arrows).
                            let bag = game.as_ref().and_then(|g| db_sim::attributes::starting_bag(g.tables, db_sim::attributes::class_type_id(g.tables, career).ok()?).ok()).unwrap_or_default();
                            for (item_id, count) in bag {
                                queue.push(SimCommand::Inventory(InventoryCommand::GiveStarting { id, item_id, count }));
                            }
                        }
                        Some(InventoryCommand::SetCharacterItems { career, sheet, inventory, .. }) => queue.push(SimCommand::Inventory(InventoryCommand::SetCharacterItems { id, career, sheet, inventory })),
                        _ => {}
                    }
                    // Only the Fighter's skills are built so far; other classes get theirs.
                    let class = data.as_ref().and_then(|d| d.class_skills.iter().find(|c| c.career() == career).cloned());
                    let has_skills = if career == EnRoleCareerType::Fighter
                        && let Some(s) = data.as_ref().and_then(|d| d.fighter_skills.clone())
                    {
                        queue.push(SimCommand::SetSkills { id, data: Box::new(s) });
                        true
                    } else if let Some(c) = class {
                        queue.push(SimCommand::SetClassSkills { id, data: Box::new(c) });
                        true
                    } else {
                        false
                    };
                    // The picked loadout, if it is this class's (a stale one would be refused).
                    if let Some([q, e]) = loadout.0.filter(|[q, e]| has_skills && SkillKind::options(career)[0].contains(q) && SkillKind::options(career)[1].contains(e)) {
                        queue.push(SimCommand::SetSkillLoadout { id, q, e });
                    }
                    // Seen from the observer camera (AR2): the wooden stand-in other actors get.
                    if observe.0.is_some() {
                        let wood = materials.add(surface_material(Surface::Wood, Vec2::new(1.0, 2.0), &assets));
                        commands.spawn((ActorView(id), Transform::default(), Visibility::default())).with_children(|b| {
                            b.spawn((Mesh3d(meshes.add(Capsule3d::new(0.3, 1.2))), MeshMaterial3d(wood), Transform::default()));
                        });
                    }
                    continue;
                }
                if kind == ActorKind::Portal {
                    // Drawn from the layout by the dungeon view.
                    continue;
                }
                if matches!(kind, ActorKind::Door | ActorKind::Trap | ActorKind::Altar | ActorKind::Mechanism | ActorKind::Lever) {
                    // Bodies from their sim components (`features_view`, `swing_door_view`).
                    continue;
                }
                if kind == ActorKind::Summon {
                    // The Druid's treant: the Treant monster's model (`model_view` places and swings it).
                    if crate::model_view::spawn_model_body(&mut commands, &assets, id, "Treant") {
                        continue;
                    }
                    // Without the imported model: a trunk and a crown.
                    let bark = materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.24, 0.14), perceptual_roughness: 0.9, ..default() });
                    let leaves = materials.add(StandardMaterial { base_color: Color::srgb(0.22, 0.45, 0.18), perceptual_roughness: 0.9, ..default() });
                    commands
                        .spawn((ActorView(id), Transform::default(), Visibility::default(), crate::flow::LevelScoped))
                        .with_children(|c| {
                            c.spawn((Mesh3d(meshes.add(Cylinder::new(0.25, 2.0))), MeshMaterial3d(bark), Transform::from_xyz(0.0, 0.1, 0.0)));
                            c.spawn((Mesh3d(meshes.add(Sphere::new(0.9))), MeshMaterial3d(leaves), Transform::from_xyz(0.0, 1.4, 0.0)));
                        });
                    continue;
                }
                // A breakable prop's body comes from `destroyable_view`.
                if kind == ActorKind::Destroyable {
                    continue;
                }
                if kind == ActorKind::Projectile {
                    // A glowing bolt (placeholder; the projectile meshes and VFX are blocked).
                    let glow = Color::srgb(1.0, 0.6, 0.25);
                    let mat = materials.add(StandardMaterial { base_color: glow, emissive: LinearRgba::from(glow) * 4.0, ..default() });
                    commands.spawn((ActorView(id), Transform::default(), Visibility::default(), Mesh3d(meshes.add(Sphere::new(0.07))), MeshMaterial3d(mat), crate::flow::LevelScoped));
                    continue;
                }
                if matches!(kind, ActorKind::Chest | ActorKind::Item) {
                    let item = index.get(id).and_then(|e| ground.get(e).ok()).zip(game.as_ref()).map(|(g, game)| {
                        let name = crate::items_view::item_name(game, g.0.item.item_id);
                        (if g.0.count > 1 { format!("{name} x{}", g.0.count) } else { name }, crate::items_view::rarity_color(g.0.item.rarity))
                    });
                    crate::items_view::spawn_loot_body(&mut commands, id, kind, item, &mut meshes, &mut materials, &assets);
                    continue;
                }
                if kind == ActorKind::Monster {
                    // A monster the sim summoned (the totem's Moose Ghost) or brought back (the arena) has its stats
                    // and kit already; it takes nothing from the layout's spawn queue.
                    let sim_row = index.get(id).and_then(|e| summoned.get(e).ok().map(|s| s.row).or_else(|| respawned.get(e).ok().and_then(|r| db_sim::monsters::spec(&r.0.kit.name)).map(|s| s.row)));
                    if let Some(row) = sim_row {
                        if index.get(id).is_some_and(|e| respawned.contains(e)) {
                            info!("arena: {row} back (id {})", id.0);
                        }
                        if !crate::model_view::spawn_model_body(&mut commands, &assets, id, row) {
                            crate::monster_view::spawn_monster_body(&mut commands, id, row, &mut meshes, &mut materials, &assets);
                        }
                        let e = crate::monster_view::spawn_monster_label(&mut commands, id, row);
                        commands.entity(e).insert(HealthLabel { id, name: db_sim::monsters::spec(row).map_or("Monster", |m| m.name) });
                        continue;
                    }
                    let (yaw, loot, row, opts) = spawns.0.pop_front().unwrap_or((db_sim::level::TEST_ROOM_GOBLIN_YAW, db_sim::level::TEST_ROOM_GOBLIN_LOOT, "GoblinAxe", Default::default()));
                    if let Some(m) = data.as_ref().and_then(|d| d.monster(row)) {
                        let (attack, defence, max_health, guard_reduce) = m.stats;
                        queue.push(SimCommand::SetCombatant { id, combatant: Box::new(Combatant { attack, defence, guard_reduce }), max_health });
                        queue.push(SimCommand::SetAim { id, yaw, pitch: 0.0 });
                        queue.push(SimCommand::SetMonster { id, kit: Box::new(m.kit.clone()), senses: m.senses });
                        queue.push(SimCommand::SetSpawnOpts { id, opts: opts.clone() });
                    }
                    // Loot 0: no loot source (the totems).
                    if loot != 0 {
                        queue.push(SimCommand::SetLoot { id, source_id: loot });
                    }
                    // The arena's monsters come back after their death.
                    if let Some(after) = arena.respawn() {
                        queue.push(SimCommand::SetRespawn { id, after, opts });
                    }
                    if !crate::model_view::spawn_model_body(&mut commands, &assets, id, row) {
                        crate::monster_view::spawn_monster_body(&mut commands, id, row, &mut meshes, &mut materials, &assets);
                    }
                    let e = crate::monster_view::spawn_monster_label(&mut commands, id, row);
                    commands.entity(e).insert(HealthLabel { id, name: db_sim::monsters::spec(row).map_or("Monster", |s| s.name) });
                    continue;
                }
                // Placeholder bodies until the character meshes decode.
                let wood = materials.add(surface_material(Surface::Wood, Vec2::new(1.0, 2.0), &assets));
                commands.spawn((ActorView(id), Transform::default(), Visibility::default())).with_children(|b| {
                    b.spawn((Mesh3d(meshes.add(Capsule3d::new(0.3, 1.2))), MeshMaterial3d(wood.clone()), Transform::default()));
                    if kind == ActorKind::Dummy {
                        // Cross-beam arms.
                        b.spawn((Mesh3d(meshes.add(Cuboid::new(0.12, 0.12, 1.1))), MeshMaterial3d(wood), Transform::from_xyz(0.0, 0.45, 0.0)));
                    }
                });
                if kind == ActorKind::Dummy {
                    let e = spawn_label(&mut commands, String::new(), Color::srgb(0.9, 0.9, 0.85), WorldLabel::follow(id, 115.0, 20.0));
                    commands.entity(e).insert(HealthLabel { id, name: "Training Dummy" });
                }
            }
            SimEvent::Damaged { target, damage, crit, blocked, .. } => {
                let Some(pos) = index.get(target).and_then(|t| positions.get(t).ok()) else { continue };
                // Spread numbers a little so consecutive hits don't overlap exactly.
                let jitter = Vec3::new(((damage * 7.0) % 30.0) as f32 - 15.0, 0.0, 0.0);
                let (color, size) = match (crit, blocked) {
                    (_, true) => (Color::srgb(0.6, 0.7, 0.9), 26.0),
                    (true, _) => (Color::srgb(1.0, 0.82, 0.2), 40.0),
                    _ => (Color::WHITE, 32.0),
                };
                let text = if crit { format!("{damage:.0}!") } else { format!("{damage:.0}") };
                spawn_label(&mut commands, text, color, WorldLabel { anchor: pos.0 + Vec3::Z * 70.0 + jitter, follow: None, age: 0.0, life: Some(NUMBER_LIFE), rise: 60.0, font_px: size, range: LABEL_RANGE });
            }
            _ => {}
        }
    }
}

/// Views follow their actor; a view whose actor is gone is removed.
pub(crate) fn sync_actor_views(mut commands: Commands, index: Res<SimIndex>, positions: Query<(&Position, Option<&crate::PrevPosition>)>, fixed: Res<Time<Fixed>>, mut views: Query<(Entity, &ActorView, &mut Transform)>) {
    for (e, view, mut t) in &mut views {
        match index.get(view.0).and_then(|s| positions.get(s).ok()) {
            // Between the last two ticks.
            Some((p, prev)) => t.translation = to_bevy(crate::drawn(p, prev, &fixed)),
            None => commands.entity(e).despawn(),
        }
    }
}

/// Sword pose from the local player's swing: pulled back in the wind-up, sweeping right to left in
/// the active window, returning in recovery; lowered while blocking.
#[allow(clippy::too_many_arguments)]
pub(crate) fn animate_viewmodel(
    player: Res<LocalPlayer>,
    index: Res<SimIndex>,
    melee: Query<&Melee>,
    kits: Query<&SkillKit>,
    mut sword: Query<&mut Transform, With<ViewSword>>,
    fp: Query<(), With<crate::fp_body::FpReady>>,
    fov: Res<crate::FovSetting>,
    cams: Query<&Projection, With<crate::PlayerCamera>>,
) {
    let Ok(mut t) = sword.single_mut() else { return };
    // The first-person body animates itself under the view sword. A zoomed view (a crossbow's aim) doesn't
    // zoom the arms (the game draws them with their own field of view): across the view they are scaled by the
    // zoom's tangent ratio, which puts them where the setting's field of view would.
    if !fp.is_empty() {
        let k = cams.single().ok().and_then(|p| match p {
            Projection::Perspective(pp) => Some((pp.fov * 0.5).tan() / (crate::vertical_fov(fov.0, pp.aspect_ratio) * 0.5).tan()),
            _ => None,
        });
        *t = Transform::from_scale(Vec3::new(k.unwrap_or(1.0), k.unwrap_or(1.0), 1.0));
        return;
    }
    t.scale = Vec3::splat(VIEW_SWORD_SCALE);
    let e = player.0.and_then(|id| index.get(id));
    if let Some(spin) = e.and_then(|e| kits.get(e).ok()).and_then(|k| k.whirlwind) {
        // Whirlwind: the blade sweeps flat across the view, about 1.5 turns per second.
        let a = spin as f32 * DT * std::f32::consts::TAU * 1.5;
        t.translation = Vec3::new(0.45 * a.sin(), -0.22, -0.5 - 0.1 * a.cos());
        t.rotation = Quat::from_euler(EulerRot::YXZ, -a, -1.45, 0.0);
        return;
    }
    let m = e.and_then(|e| melee.get(e).ok());
    let idle = (Vec3::new(0.3, -0.3, -0.5), Quat::from_euler(EulerRot::YXZ, 0.25, -0.35, -0.3));
    let (pos, rot) = match m.and_then(|m| m.swing.as_ref()) {
        Some(s) => {
            let (w, end) = s.windows.first().copied().unwrap_or((0, 0));
            let (phase, f) = match s.phase() {
                SwingPhase::Windup => (0, s.tick as f32 / w.max(1) as f32),
                SwingPhase::Active => (1, (s.tick - w) as f32 / (end - w).max(1) as f32),
                SwingPhase::Recovery => (2, (s.tick - end) as f32 / (s.length - end).max(1) as f32),
            };
            let cocked = (Vec3::new(0.38, -0.12, -0.45), Quat::from_euler(EulerRot::YXZ, -0.6, -0.2, -1.3));
            let swept = (Vec3::new(-0.3, -0.3, -0.5), Quat::from_euler(EulerRot::YXZ, 0.9, -0.6, 1.2));
            let lerp = |a: (Vec3, Quat), b: (Vec3, Quat), f: f32| (a.0.lerp(b.0, f), a.1.slerp(b.1, f));
            match phase {
                0 => lerp(idle, cocked, f),
                1 => lerp(cocked, swept, f),
                _ => lerp(swept, idle, f),
            }
        }
        None if m.is_some_and(Melee::blocking) => (Vec3::new(0.05, -0.2, -0.5), Quat::from_euler(EulerRot::YXZ, 0.0, 0.0, 1.45)),
        None => idle,
    };
    t.translation = pos;
    t.rotation = rot;
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn place_labels(
    mut commands: Commands,
    time: Res<Time>,
    index: Res<SimIndex>,
    positions: Query<&Position>,
    health: Query<(&Health, Option<&db_sim::Buffs>, Option<&db_sim::ai::MonsterAi>)>,
    cam: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    top: Res<crate::TopDown>,
    spatial: db_sim::SpatialQuery,
    boxes: Query<(), With<db_sim::LevelBox>>,
    mut labels: Query<(Entity, &mut WorldLabel, &mut Node, &mut TextColor, &mut Visibility, &mut Text, Option<&HealthLabel>)>,
) {
    let Ok((camera, cam_t)) = cam.single() else { return };
    // Camera position in sim cm (inverse of `to_bevy`).
    let eye = cam_t.translation() * 100.0;
    let eye = Vec3::new(eye.x, eye.z, eye.y);
    for (e, mut l, mut node, mut color, mut vis, mut text, hp) in &mut labels {
        l.age += time.delta_secs();
        if l.life.is_some_and(|life| l.age >= life) {
            commands.entity(e).despawn();
            continue;
        }
        let base = match l.follow {
            Some(id) => match index.get(id).and_then(|s| positions.get(s).ok()) {
                Some(p) => p.0,
                None => {
                    commands.entity(e).despawn();
                    continue;
                }
            },
            None => Vec3::ZERO,
        };
        // A mimic still posing as a chest or a thief still hidden has no label.
        let mut disguised = false;
        if let Some(HealthLabel { id, name }) = hp
            && let Some((h, b, ai)) = index.get(*id).and_then(|s| health.get(s).ok())
        {
            disguised = ai.is_some_and(|a| a.dormant && (a.wake_on_open || a.invisible_dormant));
            let cursed = b.is_some_and(|b| b.0.iter().any(|x| matches!(x.kind, db_sim::BuffKind::FrostCurse { .. })));
            let state = if b.is_some_and(db_sim::Buffs::stunned) { "  petrified" } else if cursed { "  cursed" } else { "" };
            let s = if h.current > 0.0 { format!("{name}  {:.0} / {:.0}{state}", h.current.ceil(), h.max) } else { format!("{name}  dead") };
            // Only on change: writing `Text` every frame re-lays out every label's glyphs.
            if text.0 != s {
                text.0 = s;
            }
        }
        let frac = l.life.map_or(0.0, |life| l.age / life);
        let world = base + l.anchor + Vec3::Z * l.rise * frac;
        // First person: no labels through walls or from across the dungeon.
        let hidden = disguised || (top.0.is_none() && (world.distance(eye) > l.range || db_sim::ai::wall_between(&spatial, &boxes, eye, world)));
        if hidden {
            *vis = Visibility::Hidden;
            continue;
        }
        let Ok(screen) = camera.world_to_viewport(cam_t, to_bevy(world)) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        // Centre the text on the point (approximate width: 0.55 em per character).
        let width = text.0.chars().count() as f32 * l.font_px * 0.55;
        node.left = px(screen.x - width * 0.5);
        node.top = px(screen.y - l.font_px * 0.5);
        color.0.set_alpha(1.0 - frac * frac);
    }
}

/// Q/E readout (ready, seconds left, charges, armed) and the running buffs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn skill_hud(player: Res<LocalPlayer>, index: Res<SimIndex>, kits: Query<&SkillKit>, class_kits: Query<&db_sim::ClassKit>, powers: Query<&db_sim::Power>, buffs: Query<&db_sim::Buffs>, crossbows: Query<(&db_sim::crossbow::CrossbowState, &db_sim::Inventory)>, mut hud: Query<&mut Text, With<SkillHud>>) {
    let Ok(mut text) = hud.single_mut() else { return };
    let e = player.0.and_then(|id| index.get(id));
    let secs = |ticks: u32| ticks as f32 * DT;
    let buff_lines = |s: &mut String| {
        // A held crossbow: the loaded bolts and the arrows in the bag.
        if let Some((c, inv)) = e.and_then(|e| crossbows.get(e).ok())
            && let (Some((n, max)), Some((_, spec))) = (c.held_ammo(), c.held.as_ref())
        {
            let arrows: u32 = inv.bag.iter().filter(|b| b.item.item.item_id == spec.ammo_item).map(|b| u32::from(b.item.count)).sum();
            let doing = match c.action {
                Some(db_sim::crossbow::CrossbowAction::Reload { .. }) => "  reloading",
                _ if c.aiming => "  aiming",
                _ => "",
            };
            s.push_str(&format!("
Bolt {n}/{max}  arrows {arrows}  [R] reload{doing}"));
        }
        if let Some(b) = e.and_then(|e| buffs.get(e).ok()) {
            for buff in &b.0 {
                let what = match buff.kind {
                    // Altar blessings.
                    db_sim::BuffKind::Battle { .. } if buff.skill == SkillKind::Blessing => "Attack blessing".to_string(),
                    db_sim::BuffKind::DamageReduction(_) if buff.skill == SkillKind::Blessing => "Defence blessing".to_string(),
                    db_sim::BuffKind::Regen(_) if buff.skill == SkillKind::Blessing => "Healing blessing".to_string(),
                    db_sim::BuffKind::MoveSpeed(_) if buff.skill == SkillKind::Blessing => "Speed blessing".to_string(),
                    db_sim::BuffKind::Stealth { .. } => "Stealth".to_string(),
                    db_sim::BuffKind::DamageReduction(r) => format!("{:.0}% damage reduction", r * 100.0),
                    db_sim::BuffKind::Slow(r) => format!("slowed {:.0}%", r * 100.0),
                    db_sim::BuffKind::Petrified { .. } => "petrified".to_string(),
                    db_sim::BuffKind::FrostCurse { .. } => "Frost Curse".to_string(),
                    db_sim::BuffKind::IceArmor { .. } => "Ice Armor counter".to_string(),
                    db_sim::BuffKind::Rooted | db_sim::BuffKind::Pull(_) => continue,
                    db_sim::BuffKind::Invincible => "Holy Shield".to_string(),
                    db_sim::BuffKind::MoveSpeed(_) if buff.skill == SkillKind::WildAwakening => continue,
                    _ => buff.skill.name().to_string(),
                };
                s.push_str(&format!("\n  {what} {:.0} s", secs(buff.ticks_left).ceil()));
            }
        }
    };
    if let Some(k) = e.and_then(|e| class_kits.get(e).ok()) {
        let [s1, s2] = db_sim::overrides::class_skills::CHARGE_STAGES;
        let state = |skill: SkillKind| -> String {
            if let Some((_, held)) = k.charge.filter(|c| c.0 == skill) {
                // Force of Nature is aimed, then planted.
                if skill == SkillKind::ForceOfNature {
                    return "aiming (E or left click plants it, right click drops it)".to_string();
                }
                // Flying Swords' hold counts its swords.
                if skill == SkillKind::FlyingSwords {
                    let most = if let db_sim::ClassData::SwordMaster(d) = &k.data { d.q_swords } else { db_sim::class_skills::FLYING_SWORDS_MAX };
                    let n = db_sim::class_skills::swords_summoned(held, most);
                    return format!("{n} sword{} (let go to launch)", if n == 1 { "" } else { "s" });
                }
                let t = secs(held);
                return if t >= s2 { "charged: stage 2".to_string() } else if t >= s1 { "charging: stage 1".to_string() } else { "charging".to_string() };
            }
            if skill == SkillKind::WildAwakening && k.panther {
                return "panther (Q to return)".to_string();
            }
            if skill.slot() == 1 && k.panther {
                return match k.cooldown(SkillKind::ShadowPounce) {
                    0 => "Shadow Pounce ready".to_string(),
                    t => format!("Shadow Pounce {:.0} s", secs(t).ceil()),
                };
            }
            if let Some((n, max, next)) = k.charges_of(skill) {
                return if n < max { format!("{n}/{max}  next {:.0} s", secs(next).ceil()) } else { format!("{n}/{max}") };
            }
            if let Some((s, _, left)) = k.aura.filter(|a| a.0 == skill) {
                return if s == SkillKind::SoulStorm { "on (Q to stop)".to_string() } else { format!("circling {:.0} s", secs(left).ceil()) };
            }
            if let Some((_, t)) = k.oracle.filter(|_| skill == SkillKind::Oracle) {
                return format!("blessing in {:.1} s", secs(t));
            }
            if let Some(b) = k.blizzard.filter(|_| skill == SkillKind::Blizzard) {
                return if b.channel > 0 { format!("storm {:.0} s (Q to unfold)", secs(b.channel).ceil()) } else { format!("blizzard {:.0} s", secs(b.unfolded).ceil()) };
            }
            match (k.preparing, k.waltz, k.poison) {
                (Some((s, t)), _, _) if s == skill => format!("preparing {:.0} s", secs(t).ceil()),
                (_, Some((t, _)), _) if skill == SkillKind::BladeWaltz => format!("circling {:.0} s", secs(t).ceil()),
                (_, _, Some(t)) if skill == SkillKind::PetrifyingPoison => format!("coated {:.0} s", secs(t).ceil()),
                _ if k.cooldown(skill) == 0 => "ready".to_string(),
                _ => format!("{:.0} s", secs(k.cooldown(skill)).ceil()),
            }
        };
        let [q, e_skill] = k.slots;
        let mut s = format!("[Q] {}  {}\n[E] {}  {}", q.name(), state(q), e_skill.name(), state(e_skill));
        if let Some(p) = e.and_then(|e| powers.get(e).ok()) {
            let what = match p.kind {
                db_sim::PowerKind::Mana => "Mana",
                db_sim::PowerKind::SoulEnergy => "Soul energy",
                db_sim::PowerKind::NaturalEnergy => "Shadow energy",
            };
            s = format!("{what} {:.0} / {:.0}\n{s}", p.current.floor(), p.max);
        }
        buff_lines(&mut s);
        if text.0 != s {
            text.0 = s;
        }
        return;
    }
    let Some(k) = e.and_then(|e| kits.get(e).ok()) else {
        text.0 = String::new();
        return;
    };
    let state = |skill: SkillKind| -> String {
        // Plain text: the default font has no pip glyphs.
        if let Some((n, max)) = k.charges_of(skill) {
            let next = if skill == SkillKind::Charge { k.charge_recharge } else { k.inspire_recharge };
            return if n < max { format!("{n}/{max}  next {:.0} s", secs(next).ceil()) } else { format!("{n}/{max}") };
        }
        match skill {
            SkillKind::Whirlwind if k.whirlwind.is_some() => "spinning".to_string(),
            SkillKind::Rend if let Some(t) = k.rend_armed => format!("armed {:.0} s", secs(t).ceil()),
            _ if k.cooldown(skill) == 0 => "ready".to_string(),
            _ => format!("{:.0} s", secs(k.cooldown(skill)).ceil()),
        }
    };
    let [q, e_skill] = k.slots;
    let mut s = format!("[Q] {}  {}\n[E] {}  {}", q.name(), state(q), e_skill.name(), state(e_skill));
    buff_lines(&mut s);
    text.0 = s;
}

/// Projectile looks by what they are: fire orange, ice pale blue, sized by their collision radius.
#[allow(clippy::too_many_arguments)]
pub(crate) fn style_projectiles(mut views: Query<(Entity, &ActorView, &mut Transform), Added<ActorView>>, index: Res<SimIndex>, shots: Query<&db_sim::Projectile>, table: Res<crate::skill_vfx::SkillVfx>, (actor_vfx, lib, kits): (Res<crate::effects::MonsterActorVfx>, Res<crate::effects::EffectLibrary>, Query<&db_sim::MonsterKit>), mut materials: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>, mut commands: Commands) {
    for (e, view, mut t) in &mut views {
        let Some(p) = index.get(view.0).and_then(|s| shots.get(s).ok()) else { continue };
        // A monster shot whose actor's effects the library has shows only those, unscaled (their actors set no mesh:
        // the look is the Niagara), unless one lost its mesh emitter (the Cyclops' rock keeps the ball). The
        // Beholder's pillar too since its floor sigils lie flat (its fire disc below only without them).
        if actor_vfx.shot(Some(&lib), &index, &kits, p).is_some_and(|a| !a.systems.iter().any(|s| lib.lacks_mesh(&s.key))) {
            commands.entity(e).remove::<Mesh3d>();
            continue;
        }
        // A skill shot with its own projectile effect shows only that: no placeholder ball, and an
        // unscaled view so the effect keeps its size.
        if let db_sim::HitSource::Skill(s) = p.spec.by
            && table.has_projectile(s)
        {
            commands.entity(e).remove::<Mesh3d>();
            continue;
        }
        // A fuse (the Beholder's pillar): a glowing ring of fire on the floor until it bursts
        // (placeholder for `FX_Beholder_Attack2_1_Range`; Niagara isn't converted yet).
        if p.spec.fuse {
            let glow = Color::srgba(1.0, 0.35, 0.05, 0.55);
            let mat = materials.add(StandardMaterial { base_color: glow, emissive: LinearRgba::rgb(1.0, 0.35, 0.05) * 3.0, alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
            let r = p.spec.burst / 100.0;
            commands.entity(e).remove::<Mesh3d>().with_child((Mesh3d(meshes.add(Cylinder::new(r, 0.03))), MeshMaterial3d(mat), Transform::from_xyz(0.0, -r + 0.02, 0.0)));
            continue;
        }
        let glow = match p.spec.element {
            db_sim::combat::Element::Fire => Color::srgb(1.0, 0.45, 0.1),
            db_sim::combat::Element::Ice => Color::srgb(0.55, 0.85, 1.0),
            // Monster spells: the shaman's fireball, the skeleton wizard's bolt; arrows and bolts stay plain.
            _ if matches!(p.spec.by, db_sim::HitSource::Monster(_)) && p.spec.burst > 0.0 => Color::srgb(1.0, 0.5, 0.15),
            _ if matches!(p.spec.by, db_sim::HitSource::Monster(_)) && p.spec.radius >= 15.0 => Color::srgb(0.6, 0.35, 1.0),
            _ => continue,
        };
        let mat = materials.add(StandardMaterial { base_color: glow, emissive: LinearRgba::from(glow) * 4.0, ..default() });
        // The placeholder sphere is 7 cm across in radius; scale it to the shot's radius.
        t.scale = Vec3::splat((p.spec.radius / 7.0).max(1.0));
        commands.entity(e).insert(MeshMaterial3d(mat));
    }
}

/// Skill areas on the floor (placeholder discs from before skill effects drew): a Blizzard's storm, an Oracle about
/// to go off, a DeathKnight's damage ring. Only for a skill with no effect of its own to play (with the recovered
/// formulas Wraith Ring's ring loops on its own, and the disc lay over it as a flat purple floor).
#[derive(Component)]
pub(crate) struct StormView(SimId);

pub(crate) fn storm_view(kits: Query<(&SimId, &db_sim::ClassKit, &Position)>, mut views: Query<(Entity, &StormView, &mut Transform)>, (table, lib): (Option<Res<crate::skill_vfx::SkillVfx>>, Option<Res<crate::effects::EffectLibrary>>), mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut commands: Commands) {
    use db_sim::overrides::class_skills as tune;
    let real = |skill: db_sim::SkillKind| table.as_ref().zip(lib.as_ref()).is_some_and(|(t, l)| t.triggers(skill, |_| true).any(|x| l.has(&x.effect)));
    let storms: Vec<(SimId, Vec3, f32, Color)> = kits
        .iter()
        .filter_map(|(id, k, p)| match &k.data {
            db_sim::ClassData::DeathKnight(_) if real(db_sim::SkillKind::WraithRing) => None,
            // Blizzard's and Oracle's area effects lie on the floor now that sprites keep their facing.
            db_sim::ClassData::Lich(_) if real(db_sim::SkillKind::Blizzard) => None,
            db_sim::ClassData::Priest(_) if real(db_sim::SkillKind::Oracle) => None,
            db_sim::ClassData::Lich(d) => {
                let b = k.blizzard?;
                Some((*id, b.pos, if b.channel > 0 { d.storm_radius } else { tune::BLIZZARD_UNFOLDED_RADIUS }, Color::srgba(0.6, 0.85, 1.0, 0.45)))
            }
            db_sim::ClassData::Priest(_) => k.oracle.map(|(at, _)| (*id, at, tune::ORACLE_RADIUS, Color::srgba(1.0, 0.9, 0.5, 0.4))),
            db_sim::ClassData::DeathKnight(d) => k.aura.map(|_| (*id, p.0 - Vec3::Z * 90.0, d.ring_radius + db_sim::movement::CAPSULE_RADIUS, Color::srgba(0.5, 0.3, 0.7, 0.35))),
            _ => None,
        })
        .collect();
    for (e, view, mut t) in &mut views {
        match storms.iter().find(|s| s.0 == view.0) {
            Some(&(_, pos, radius, _)) => {
                t.translation = crate::to_bevy(pos + Vec3::Z * 3.0);
                t.scale = Vec3::new(radius * 0.01, 1.0, radius * 0.01);
            }
            None => commands.entity(e).despawn(),
        }
    }
    for &(id, pos, radius, color) in &storms {
        if views.iter().any(|(_, v, _)| v.0 == id) {
            continue;
        }
        let mat = materials.add(StandardMaterial { base_color: color, alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
        let t = Transform::from_translation(crate::to_bevy(pos + Vec3::Z * 3.0)).with_scale(Vec3::new(radius * 0.01, 1.0, radius * 0.01));
        commands.spawn((StormView(id), t, Visibility::default(), Mesh3d(meshes.add(Cylinder::new(1.0, 0.04))), MeshMaterial3d(mat), crate::flow::LevelScoped));
    }
}

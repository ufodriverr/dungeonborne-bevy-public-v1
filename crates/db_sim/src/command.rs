//! Commands in, events out. Clients (and later the network layer) never touch sim components
//! directly: they push [`SimCommand`]s, which are applied in order at the start of the next tick,
//! and read [`SimEvent`]s, which describe what happened.

use avian3d::prelude::{CollisionLayers, LayerMask, RigidBody};
use bevy_ecs::prelude::*;
use bevy_math::{Vec2, Vec3};

use crate::id::{SimIdAllocator, SimIndex};
use crate::melee::{Aim, AttackKind, Combatant, Health, Melee};
use crate::ai::{AiSenses, AiState, MonsterAi, SpawnOpts};
use crate::monsters::MonsterKit;
use crate::movement::{CharacterState, MoveInput, MoveSpeed, character_collider, static_box};
use crate::skills::{FighterSkillData, HitSource, SkillKind, SkillKit};
use crate::{Position, SimId, Wander};

/// Marks static level geometry: boxes from [`SimCommand::AddStaticBox`] (their half extents) and the real
/// map's mesh colliders from [`SimCommand::AddMapCollision`] (zero extents).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LevelBox {
    pub half_extents: Vec3,
}

/// The kind an actor was spawned as.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind(pub ActorKind);

/// What kind of actor to spawn. Grows with G-milestones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    Player,
    /// Placeholder monster that wanders randomly.
    Wanderer,
    /// Training dummy (`BP_TGF_M_Dummy`): a static target.
    Dummy,
    /// A monster moved by the AI; its attacks come with `SetMonster`.
    Monster,
    /// A loot container (spawned with `SpawnChest`).
    Chest,
    /// An item lying on the ground.
    Item,
    /// A portal (down a floor, or extraction), used with `Interact`.
    Portal,
    /// A flying projectile (`SpawnProjectile`).
    Projectile,
    /// A skill's summon (the Druid's treant).
    Summon,
    /// A door (`SpawnDoor`).
    Door,
    /// A loop trap (`SpawnTrap`).
    Trap,
    /// An altar (`SpawnAltar`).
    Altar,
    /// A lever mechanism (`SpawnMechanism`): a portcullis or stone gate.
    Mechanism,
    /// A lever working a mechanism, used with `Interact`.
    Lever,
    /// A breakable prop (`SpawnDestroyable`): a barrel, box or claypot.
    Destroyable,
}

/// Input to the simulation.
#[derive(Debug, Clone, PartialEq)]
pub enum SimCommand {
    /// Spawn a character; `pos` is the capsule centre (cm), `speed` its base move speed (cm/s).
    Spawn { kind: ActorKind, pos: Vec3, speed: f32 },
    /// Desired planar direction (x, y used; z ignored).
    SetMoveIntent { id: SimId, dir: Vec3 },
    SetMoveFlags { id: SimId, sprint: bool, crouch: bool },
    Jump { id: SimId },
    Despawn { id: SimId },
    /// Static level geometry: an axis-aligned box (centre and half extents, cm).
    AddStaticBox { center: Vec3, half_extents: Vec3 },
    /// The real crypt map's static geometry ([`crate::map`]).
    AddMapCollision(std::sync::Arc<crate::map::MapCollision>),
    /// A floor's walkable grid and where it sits in the world: monsters path on it.
    AddNav(std::sync::Arc<crate::nav::NavGrid>, Vec3),
    /// Give an actor combat stats and full health (the caller builds them from data).
    SetCombatant { id: SimId, combatant: Box<Combatant>, max_health: f64 },
    /// Where the actor is looking (radians; yaw 0 = +X, positive towards +Y).
    SetAim { id: SimId, yaw: f32, pitch: f32 },
    /// Start an attack (queued until the current swing ends).
    Attack { id: SimId, kind: AttackKind },
    SetBlock { id: SimId, on: bool },
    /// Let go of a charge attack (a weapon's `ChargeLeft` ability); started by `Attack { kind: Heavy }`.
    ReleaseCharge { id: SimId },
    /// The reload key: a held crossbow loads a bolt from the bag's arrows.
    Reload { id: SimId },
    /// Switch to the other weapon set.
    SwitchWeaponPair { id: SimId },
    /// Give an actor the Fighter skills (the caller reads them from data).
    SetSkills { id: SimId, data: Box<FighterSkillData> },
    UseSkill { id: SimId, skill: SkillKind },
    /// The skill key was let go: a charged skill (Pyroblast, Frost Torment) fires at its stage.
    ReleaseSkill { id: SimId, skill: SkillKind },
    /// Cancel a charged skill without firing it (Pyroblast's right button).
    CancelSkill { id: SimId, skill: SkillKind },
    /// Launch a projectile owned by `owner` from `pos` along `dir` (homing on `target` if it turns).
    SpawnProjectile { owner: SimId, spec: Box<crate::projectiles::ProjectileSpec>, pos: Vec3, dir: Vec3, target: Option<SimId> },
    /// Give an actor a non-Fighter class's skills (SwordMaster, Rogue; the caller loads the data).
    SetClassSkills { id: SimId, data: Box<crate::class_skills::ClassData> },
    /// Choose the Q and E skills of an actor that has skills.
    SetSkillLoadout { id: SimId, q: SkillKind, e: SkillKind },
    /// Give a monster its attacks (montage data the caller loads) and its AI, homed where it stands.
    SetMonster { id: SimId, kit: Box<MonsterKit>, senses: AiSenses },
    /// What a monster's spawn point says about it (dormancy); after `SetMonster`.
    SetSpawnOpts { id: SimId, opts: SpawnOpts },
    /// Dev arena: the monster comes back at its spawn point, set up as it is now, `after` seconds after it dies;
    /// after its other setup commands.
    SetRespawn { id: SimId, after: f32, opts: SpawnOpts },
    /// Start monster attack `index` (queued until the current swing ends).
    MonsterAttack { id: SimId, index: u8 },
    /// Inventory and equipment (needs the `GameData` resource).
    Inventory(crate::inventory::InventoryCommand),
    /// A chest rolling loot source `source_id` when opened; `pos` is the floor point under it.
    SpawnChest { pos: Vec3, source_id: i32 },
    /// Items on the floor around `pos` (the test room's class switch: what the backpack can't hold).
    SpawnItems { pos: Vec3, items: Vec<crate::ItemInstance> },
    /// A freshly rolled item `item_id` lying at floor point `pos` (the test arena's weapon racks).
    PlaceItem { pos: Vec3, item_id: i32 },
    /// A portal at a floor point; a portal down takes the user to `dest` (capsule centre).
    /// A portal; it can be used `opens_in` seconds after it spawns.: once open it opens
    /// the mechanisms `opens` (floor indices: its escape doors).
    SpawnPortal { pos: Vec3, kind: crate::dungeon::PortalKind, dest: Option<Vec3>, opens_in: f32, opens: Vec<u32> },
    /// A closed door at a floor point; its leaf runs along X (`along_x`) or Y.
    SpawnDoor { pos: Vec3, along_x: bool },
    /// A map's swing door: its blueprint's package path, the actor's origin, yaw (degrees) and scale.
    SpawnSwingDoor { package: String, pos: Vec3, yaw: f32, scale: Vec3 },
    /// Let go of a held interaction.
    InteractCancel { id: SimId },
    /// Places a shut lever mechanism and its levers.
    SpawnMechanism(Box<crate::mechanisms::MechanismPlacement>),
    /// `OnTriggered(param)` on the mechanism with dungeon index `index` and those bound to it: 0 off, 1 on, 2
    /// toggle. (What a spawn room's start or a script would send; dev shots use it.)
    TriggerMechanism { index: u32, param: u8 },
    /// A loop trap; `pos` is the floor point under its area's centre, `yaw` its facing, `phase` (s)
    /// how far into its loop it starts.
    SpawnTrap { pos: Vec3, kind: crate::features::TrapKind, yaw: f32, phase: f32 },
    /// A breakable prop: its class (e.g. `BP_ClayPot_4`), origin, yaw (degrees) and scale.
    SpawnDestroyable { class: String, pos: Vec3, yaw: f32, scale: Vec3 },
    /// A floor's death zone: it starts when the first player in a run reaches floor `floor`.
    SpawnDeathZone { floor: u32, plan: Box<crate::death_zone::ZonePlan> },
    /// A floor's collapse and fall trap, at its floor offset.
    SpawnFloorCollapse { floor: u32, spec: std::sync::Arc<crate::floor_collapse::CollapseSpec>, off: Vec3 },
    /// Sets off a floor's collapse (dev; in play the Shadow King's floor break does).
    TriggerFloorCollapse { floor: u32 },
    /// An altar at a floor point.
    SpawnAltar { pos: Vec3, kind: crate::features::AltarKind },
    /// Starts the run clock (seconds) and puts every player into the run.
    StartRun { seconds: u32 },
    /// Dev (shots, `--start-floor`): every player in the run counts as on floor `floor` (0-based).
    DevOnFloor { floor: u32 },
    /// Removes every actor and all level geometry and stops the run clock (between runs).
    ClearWorld,
    /// The player gives up the run (the pause menu): they die where they stand and the run ends as
    /// `Died` on the next run check, dropping everything carried (as the clock running out does).
    Abandon { id: SimId },
    /// The loot source an actor drops when it dies.
    SetLoot { id: SimId, source_id: i32 },
    /// Open a chest, pick up a ground item, use a portal, open or close a door, pray at an altar
    /// (within reach).
    Interact { id: SimId, target: SimId },
    /// Use one item of a backpack stack (potions).
    UseItem { id: SimId, uid: u32 },
    /// Throws one of a backpack stack of flasks.
    ThrowItem { id: SimId, uid: u32 },
    /// The torch key: a torch from the backpack in hand, or the held one put away.
    TorchKey { id: SimId },
    /// The staff spell input: the left button held past the hold threshold (`on`) or let go.
    StaffCast { id: SimId, on: bool },
    /// The staff charge input (the right button) held or not: recharges; its press cancels a thunder aim.
    StaffRecharge { id: SimId, on: bool },
}

/// Output of the simulation.
#[derive(Debug, Clone, PartialEq)]
pub enum SimEvent {
    Spawned { id: SimId, kind: ActorKind },
    Despawned { id: SimId },
    /// A command referred to an id that does not exist (ignored).
    Rejected { reason: &'static str },
    SwingStarted { id: SimId, kind: AttackKind },
    MonsterAttackStarted { id: SimId, index: u8 },
    AiStateChanged { id: SimId, state: AiState, target: Option<SimId> },
    /// An actor's backpack or worn items changed (its combat stats were recomputed).
    InventoryChanged { id: SimId },
    /// A death zone's message to the players: a `GeneralStrings` key (`Shadowking_ClosingAlert`...).
    ZoneMessage { floor: u32, key: String },
    /// The floor collapse's warning effects on or off.
    FloorCollapseWarning { floor: u32, on: bool },
    /// A floor collapse stage broke its rocks; its collision goes `settle` s later.
    FloorCollapsed { floor: u32, stage: u8 },
    ChestOpened { id: SimId, chest: SimId, items: Vec<SimId> },
    /// A dead actor's loot fell to the ground.
    LootDropped { id: SimId, items: Vec<SimId> },
    PickedUp { id: SimId, item: SimId, uid: u32 },
    ItemUsed { id: SimId, item_id: i32 },
    /// A potion's drink started (its effect comes at the drink's `UseItem`, [`crate::world_items::Drinking`]).
    DrinkStarted { id: SimId, item_id: i32 },
    Healed { id: SimId, amount: f64 },
    /// A shield took `amount` of a hit; `remaining` is what it can still absorb.
    ShieldAbsorbed { id: SimId, amount: f64, remaining: f64 },
    /// `damage` is what was subtracted; `health` is what the target has left.
    Damaged { source: SimId, target: SimId, damage: f64, crit: bool, blocked: bool, health: f64, by: HitSource },
    Died { id: SimId },
    /// A player went through a portal down.
    FloorChanged { id: SimId, floor: u32 },
    /// A player's run is over; `kept` is what an extracted player carried out.
    RunEnded { id: SimId, outcome: crate::run::RunOutcome, kept: Option<Box<crate::Inventory>> },
    SkillStarted { id: SimId, skill: SkillKind },
    SkillEnded { id: SimId, skill: SkillKind },
    /// A charged or channelled skill went off at `stage` (1 or 2).
    SkillCast { id: SimId, skill: SkillKind, stage: u8 },
    /// A held charged skill reached charge `stage` (1 or 2; presentation).
    SkillCharged { id: SimId, skill: SkillKind, stage: u8 },
    /// A bursting projectile went off (Pyroblast stage 2).
    Burst { owner: SimId, at: Vec3, radius: f32, by: HitSource },
    /// On cooldown, out of charges, busy, or the actor has no such skill.
    SkillRejected { id: SimId, skill: SkillKind },
    /// `id` opened or closed a door.
    DoorToggled { id: SimId, door: SimId, open: bool },
    /// A held interaction started: it completes after `secs` unless let go.
    InteractHold { id: SimId, target: SimId, secs: f32 },
    InteractCancelled { id: SimId },
    /// Player `id` pulled `lever`, now `on`.
    LeverPulled { id: SimId, lever: SimId, on: bool },
    /// A mechanism started moving: on (opening) or off.
    MechanismMoved { id: SimId, on: bool },
    /// A loop trap came up or went down.
    TrapCycled { id: SimId, up: bool },
    /// `id` took an altar's blessing.
    AltarUsed { id: SimId, altar: SimId, kind: crate::features::AltarKind },
    /// A staff spell began: the Fire beam or the Thunder aim.
    StaffCastStarted { id: SimId, element: crate::staffs::StaffElement },
    /// A beam trigger: the trace from the eye `from` to `to`, hitting `target`; `extra` is the last trigger.
    StaffBeamTick { id: SimId, from: Vec3, to: Vec3, target: Option<SimId>, extra: bool },
    /// The legendary fire staff's burst at the beam's last trigger: at `at`, `hits` characters inside.
    StaffExplode { id: SimId, at: Vec3, radius: f32, hits: u32 },
    /// The beam stopped: let go early (`cancelled`) or ran its course (the montage's `End` notify).
    StaffBeamEnded { id: SimId, cancelled: bool },
    /// A thunder aim was released at a ground point (the cloud comes at the release montage's `CastCharge`).
    StaffThunderReleased { id: SimId, at: Vec3 },
    /// `id` threw its shield 6002: it leaves the hand shortly.
    ShieldThrown { id: SimId },
    /// A flask throw started; it leaves the hand at the montage's `ThrowItem`.
    ThrowStarted { id: SimId, item_id: i32 },
    /// A torch taken in hand (`on`) or put away.
    TorchHeld { id: SimId, on: bool },
    /// A thrown flask landed at `at`: its blast (`secs` 0) or its area for `secs`, `radius` cm.
    FlaskLanded { owner: SimId, item_id: i32, at: Vec3, radius: f32, secs: f32 },
    /// A charge's ground hit landed at `at` (the mace 6002 with a shield).
    GroundHit { id: SimId, at: Vec3 },
    /// A thunder aim failed (released too early or without a point) or was cancelled.
    StaffThunderFailed { id: SimId },
    /// A thundercloud appeared at a ground point, for `life` seconds.
    ThunderAreaSpawned { owner: SimId, at: Vec3, radius: f32, life: f32 },
    /// A Shadow King died: his throne's `Reset` (it vanishes; its walls are gone).
    ThroneReset { king: SimId },
    /// The Shadow Dragon's breath laid a fire patch at a floor point.
    FirePatchSpawned { owner: SimId, at: Vec3, burn: crate::shadow_dragon::Burn },
    /// A thundercloud struck; `hits` characters were inside.
    ThunderStruck { owner: SimId, at: Vec3, radius: f32, hits: u32 },
    StaffRechargeStarted { id: SimId },
    StaffRechargeEnded { id: SimId },
    /// The held staff's energy changed (cost, recovery).
    StaffEnergy { id: SimId, current: u32, max: u32 },
    /// The held relic's energy changed (recovery; the enchant's cost).
    RelicEnergy { id: SimId, current: u32, max: u32 },
    /// A relic enchant was cast: the ice projectile thrown and the enchant on the caster.
    RelicEnchantCast { id: SimId },
    /// A relic enchant was refused (not enough relic energy) or cancelled (right button while charging).
    RelicEnchantFailed { id: SimId, cancelled: bool },
    /// A crossbow bolt left, aimed down the sights or not.
    CrossbowFired { id: SimId, aimed: bool },
    /// A crossbow reload started; at its `Reload` notify the bolts went in.
    CrossbowReloadStarted { id: SimId },
    CrossbowLoaded { id: SimId, loaded: u32 },
    /// A press with nothing loaded and no arrows in the bag (`Ability.ActivateFail.Cost.Ammo.Bow`).
    CrossbowEmpty { id: SimId },
    /// A hit landed on the target's head (follows its `Damaged`).
    HeadHit { source: SimId, target: SimId },
    /// The weapon sets were switched: `second` is now held.
    WeaponPairSwitched { id: SimId, second: bool },
    /// A staff spell was refused: not enough staff energy.
    StaffRejected { id: SimId },
}

/// Commands waiting for the next tick, applied in push order.
#[derive(Resource, Debug, Default)]
pub struct CommandQueue(Vec<SimCommand>);

impl CommandQueue {
    pub fn push(&mut self, c: SimCommand) {
        self.0.push(c);
    }
}

/// Events produced since the client last drained them; (target, attacker) of every hit since the AI last took them
/// (the monsters' `TGF.GameplayEvent.Damage`); (killer, victim) of every death since the last
/// [`SimEvents::take_kills`] (the killer is the last actor to damage the victim); and each damaged actor's last
/// attacker; and (target, attacker) of every hit since [`SimEvents::take_damage`] (who is in combat).
#[derive(Resource, Debug, Default)]
pub struct SimEvents(Vec<SimEvent>, Vec<(SimId, SimId)>, Vec<(SimId, SimId)>, std::collections::BTreeMap<SimId, SimId>, Vec<(SimId, SimId)>);

impl SimEvents {
    pub fn drain(&mut self) -> impl Iterator<Item = SimEvent> + '_ {
        self.0.drain(..)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn push(&mut self, e: SimEvent) {
        if let SimEvent::Damaged { source, target, .. } = e
            && source != target
        {
            self.1.push((target, source));
            self.3.insert(target, source);
            self.4.push((target, source));
        }
        if let SimEvent::Died { id } = e
            && let Some(killer) = self.3.remove(&id)
        {
            self.2.push((killer, id));
        }
        self.0.push(e);
    }

    /// The hits since the last call (the AI takes them every tick).
    pub(crate) fn take_hits(&mut self) -> Vec<(SimId, SimId)> {
        std::mem::take(&mut self.1)
    }

    /// The (target, attacker) pairs of the hits since the last call.
    pub(crate) fn take_damage(&mut self) -> Vec<(SimId, SimId)> {
        std::mem::take(&mut self.4)
    }

    /// The (killer, victim) pairs since the last call.
    pub(crate) fn take_kills(&mut self) -> Vec<(SimId, SimId)> {
        std::mem::take(&mut self.2)
    }
}

/// A new actor (the `Spawn` command, and the sim's own spawns: summons). Returns its id.
pub(crate) fn spawn_actor(world: &mut World, kind: ActorKind, pos: Vec3, speed: f32) -> SimId {
    let id = world.resource_mut::<SimIdAllocator>().alloc();
    let mut e = world.spawn((id, Kind(kind), Position(pos), character_collider(), Aim::default()));
    if kind == ActorKind::Dummy {
        e.insert(RigidBody::Static);
    } else {
        // Characters move by sweeps (`move_and_slide`), never by contacts, so they make no contact
        // pairs (filters NONE). Spatial queries still see them (default membership).
        e.insert((MoveInput::default(), MoveSpeed(speed), CharacterState::default(), RigidBody::Kinematic, CollisionLayers::new(LayerMask::DEFAULT, LayerMask::NONE)));
    }
    if kind == ActorKind::Wanderer {
        e.insert(Wander { period: 45 });
    }
    let e = e.id();
    world.resource_mut::<SimIndex>().insert(id, e);
    world.resource_mut::<SimEvents>().push(SimEvent::Spawned { id, kind });
    id
}

pub(crate) fn apply_commands(world: &mut World) {
    let cmds = std::mem::take(&mut world.resource_mut::<CommandQueue>().0);
    for c in cmds {
        apply(world, c);
    }
}

/// One command (also the sim's own: an arena respawn sets its monster up as the client's commands do).
pub(crate) fn apply(world: &mut World, c: SimCommand) {
    match c {
        SimCommand::Spawn { kind, pos, speed } => {
            spawn_actor(world, kind, pos, speed);
        }
        SimCommand::SetMoveIntent { id, dir } => with_input(world, id, |m| m.dir = Vec2::new(dir.x, dir.y).clamp_length_max(1.0)),
        SimCommand::SetMoveFlags { id, sprint, crouch } => with_input(world, id, |m| {
            m.sprint = sprint;
            m.crouch = crouch;
        }),
        SimCommand::Jump { id } => with_input(world, id, |m| m.jump = true),
        SimCommand::AddStaticBox { center, half_extents } => {
            world.spawn((static_box(center, half_extents), LevelBox { half_extents }));
        }
        SimCommand::AddMapCollision(map) => {
            crate::map::spawn_map(world, &map);
        }
        SimCommand::AddNav(grid, offset) => world.get_resource_or_insert_with(crate::nav::NavMaps::default).0.push((grid, offset)),
        SimCommand::SetCombatant { id, combatant, max_health } => match world.resource::<SimIndex>().get(id) {
            Some(e) => {
                world.entity_mut(e).insert((*combatant, Health { current: max_health, max: max_health }, Melee::default()));
            }
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        SimCommand::SetAim { id, yaw, pitch } => with(world, id, |a: &mut Aim| *a = Aim { yaw, pitch }),
        // (A tap while guarding with the shield 6002 throws it instead.)
        SimCommand::Attack { id, kind } => {
            // (Nothing while drinking. A held torch takes the button: thrown.)
            if !crate::world_items::drinking(world, id) && !crate::throwables::torch_attack(world, id) && !(kind == AttackKind::Light && crate::affixes::try_shield_throw(world, id)) {
                with(world, id, |m: &mut Melee| m.queued = Some(kind));
            }
        }
        SimCommand::ReleaseCharge { id } => with(world, id, |m: &mut Melee| m.charge_release = true),
        SimCommand::SwitchWeaponPair { id } => match world.resource::<SimIndex>().get(id) {
            Some(e) => crate::weapon_slots::switch(world, id, e),
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        SimCommand::Reload { id } => match world.resource::<SimIndex>().get(id) {
            Some(e) => crate::crossbow::want_reload(world, e),
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        // (A held torch takes the button: raised.)
        SimCommand::SetBlock { id, on } => {
            if !crate::throwables::torch_block(world, id, on) {
                with(world, id, |m: &mut Melee| {
                    m.block_held = on;
                    m.block_input = on;
                });
            }
        }
        SimCommand::SetSkills { id, data } => match world.resource::<SimIndex>().get(id) {
            Some(e) => {
                world.entity_mut(e).insert(SkillKit::new(*data));
            }
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        SimCommand::UseSkill { id, .. } if crate::world_items::drinking(world, id) => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "busy" }),
        SimCommand::UseSkill { id, skill } => crate::skills::use_skill(world, id, skill),
        SimCommand::ReleaseSkill { id, skill } => crate::class_skills::release_skill(world, id, skill),
        SimCommand::CancelSkill { id, skill } => crate::class_skills::cancel_skill(world, id, skill),
        SimCommand::SetSkillLoadout { id, q, e } => crate::skills::set_loadout(world, id, q, e),
        SimCommand::SetClassSkills { id, data } => match world.resource::<SimIndex>().get(id) {
            Some(e) => {
                let power = crate::class_skills::Power::of(&data);
                let mut ent = world.entity_mut(e);
                ent.remove::<SkillKit>().remove::<crate::class_skills::Power>().insert(crate::class_skills::ClassKit::new(*data));
                if let Some(power) = power {
                    ent.insert(power);
                }
            }
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        SimCommand::SpawnProjectile { owner, spec, pos, dir, target } => crate::projectiles::spawn(world, owner, *spec, pos, dir, target),
        SimCommand::SetMonster { id, kit, senses } => match world.resource::<SimIndex>().get(id) {
            Some(e) => {
                let home = world.get::<Position>(e).map_or(Vec3::ZERO, |p| p.0);
                world.entity_mut(e).insert((*kit, MonsterAi::new(home, senses)));
            }
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        SimCommand::SetSpawnOpts { id, opts } => {
            if let Some(e) = world.resource::<SimIndex>().get(id)
                && let Some(kit) = world.get::<MonsterKit>(e).cloned()
                && let Some(mut ai) = world.get_mut::<MonsterAi>(e)
            {
                let trigger = opts.death_trigger;
                ai.apply_spawn(&kit, opts);
                if let Some((index, param)) = trigger {
                    world.entity_mut(e).insert(crate::mechanisms::DeathTrigger { index, param });
                }
            }
        }
        SimCommand::MonsterAttack { id, index } => with(world, id, |m: &mut Melee| m.queued_montage = Some(index)),
        SimCommand::SpawnChest { pos, source_id } => crate::world_items::spawn_chest(world, pos, source_id),
        SimCommand::SpawnItems { pos, items } => {
            let mut items = items;
            for item in &mut items {
                item.uid = world.resource_mut::<crate::inventory::ItemUids>().alloc();
            }
            crate::world_items::scatter(world, pos, items);
        }
        SimCommand::PlaceItem { pos, item_id } => crate::world_items::place_item(world, pos, item_id),
        SimCommand::SpawnPortal { pos, kind, dest, opens_in, opens } => {
            let portal = crate::run::spawn_portal(world, pos, kind, dest, opens_in);
            if !opens.is_empty()
                && let Some(e) = world.resource::<SimIndex>().get(portal)
            {
                world.entity_mut(e).insert(crate::run::OpensMechanisms(opens));
            }
        }
        SimCommand::SpawnDoor { pos, along_x } => crate::features::spawn_door(world, pos, along_x),
        SimCommand::SpawnSwingDoor { package, pos, yaw, scale } => crate::swing_doors::spawn(world, &package, pos, yaw, scale),
        SimCommand::InteractCancel { id } => crate::swing_doors::cancel(world, id),
        SimCommand::SpawnMechanism(placement) => crate::mechanisms::spawn(world, *placement),
        SimCommand::TriggerMechanism { index, param } => crate::mechanisms::trigger(world, index, param, 0),
        SimCommand::SpawnTrap { pos, kind, yaw, phase } => crate::features::spawn_trap(world, pos, kind, yaw, phase),
        SimCommand::SpawnDestroyable { class, pos, yaw, scale } => crate::destroyables::spawn(world, &class, pos, yaw, scale),
        SimCommand::SpawnDeathZone { floor, plan } => crate::death_zone::spawn(world, floor, *plan),
        SimCommand::SpawnFloorCollapse { floor, spec, off } => crate::floor_collapse::spawn(world, floor, spec, off),
        SimCommand::TriggerFloorCollapse { floor } => crate::floor_collapse::trigger(world, floor),
        SimCommand::SpawnAltar { pos, kind } => crate::features::spawn_altar(world, pos, kind),
        SimCommand::StartRun { seconds } => crate::run::start_run(world, seconds),
        SimCommand::DevOnFloor { floor } => crate::run::dev_on_floor(world, floor),
        SimCommand::ClearWorld => crate::run::clear_world(world),
        SimCommand::Abandon { id } => crate::run::abandon(world, id),
        SimCommand::SetRespawn { id, after, opts } => crate::respawn::arm(world, id, after, opts),
        SimCommand::SetLoot { id, source_id } => match world.resource::<SimIndex>().get(id) {
            Some(e) => {
                world.entity_mut(e).insert(crate::world_items::LootSource { source_id, dropped: false });
            }
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
        SimCommand::Interact { id, target } if crate::swing_doors::handles(world, target) => crate::swing_doors::interact(world, id, target),
        SimCommand::Interact { id, target } if crate::mechanisms::handles(world, target) => crate::mechanisms::interact(world, id, target),
        SimCommand::Interact { id, target } if crate::features::handles(world, target) => crate::features::interact(world, id, target),
        SimCommand::Interact { id, target } if world.contains_resource::<crate::inventory::GameData>() => crate::world_items::interact(world, id, target),
        SimCommand::UseItem { id, uid } => crate::world_items::use_item(world, id, uid),
        SimCommand::ThrowItem { id, .. } | SimCommand::TorchKey { id } if crate::world_items::drinking(world, id) => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "busy" }),
        SimCommand::ThrowItem { id, uid } => crate::throwables::throw(world, id, uid),
        SimCommand::TorchKey { id } => crate::throwables::torch_key(world, id),
        SimCommand::StaffCast { id, on } => crate::staffs::set_input(world, id, Some(on), None),
        SimCommand::StaffRecharge { id, on } => crate::staffs::set_input(world, id, None, Some(on)),
        SimCommand::Interact { .. } => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "no game data loaded" }),
        SimCommand::Inventory(cmd) => {
            if world.contains_resource::<crate::inventory::GameData>() {
                crate::inventory::apply(world, cmd);
            } else {
                world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "no game data loaded" });
            }
        }
        SimCommand::Despawn { id } => match world.resource_mut::<SimIndex>().remove(id) {
            Some(e) => {
                world.despawn(e);
                world.resource_mut::<SimEvents>().push(SimEvent::Despawned { id });
            }
            None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id" }),
        },
    }
}

fn with_input(world: &mut World, id: SimId, f: impl FnOnce(&mut MoveInput)) {
    with(world, id, f);
}

/// Apply `f` to component `C` of actor `id`, or report the command as rejected.
fn with<C: Component<Mutability = bevy_ecs::component::Mutable>>(world: &mut World, id: SimId, f: impl FnOnce(&mut C)) {
    match world.resource::<SimIndex>().get(id).and_then(|e| world.get_mut::<C>(e)) {
        Some(mut c) => f(&mut c),
        None => world.resource_mut::<SimEvents>().push(SimEvent::Rejected { reason: "unknown id or component" }),
    }
}

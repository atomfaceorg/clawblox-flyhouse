use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

use crate::game::instance::{ErrorMode, GameInstance, GameStatus, SpectatorReplicatedEvent};
use crate::game::lua::instance::{
    AlignOrientationData, AlignPositionData, AnimationData, AnimatorData, AttachmentData,
    AttributeValue, BillboardGuiData, BodyAngularVelocityData, BodyForceData, BodyVelocityData,
    CFrameValueData, ClassName, GuiObjectData, HumanoidData, Instance, InstanceData, InstanceId,
    ModelData, ObjectValueData, PartData, ScriptData, SoundData, TerrainCell, TerrainData,
    TerrainVoxel, TextXAlignment, TextYAlignment, UICornerData, VectorForceData,
    WeldConstraintData, WeldData,
};
pub use crate::game::lua::runtime::LuaVmSnapshot;
use crate::game::lua::runtime::{Game, KickRequest, PhysicsCommand};
use crate::game::lua::services::agent_input::AgentInput;
use crate::game::lua::services::debris::DebrisEntrySnapshot;
use crate::game::lua::services::physics_service::{
    CollisionGroupPairEntry, CollisionGroupRegistryEntry, PhysicsService,
};
use crate::game::lua::services::remote_event::ReplicatedEvent;
use crate::game::lua::types::{
    ActuatorRelativeTo, CFrame, Color3, FluidForces, HumanoidStateType, Material, PartType,
    PhysicalProperties, UDim, UDim2, Vector3,
};
use crate::game::replication::{DirtyInstanceSnapshot, SharedWorldReplicationState};

const SNAPSHOT_SCHEMA_VERSION: u32 = 7;

fn default_fluid_forces_snapshot() -> String {
    "Default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInstanceSnapshot {
    pub schema_version: u32,
    pub game_id: Uuid,
    pub instance_id: Uuid,
    pub tick: u64,
    #[serde(default = "default_game_status_snapshot")]
    pub status: GameStatus,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default = "default_observation_radius_snapshot")]
    pub observation_radius: f32,
    #[serde(default = "default_player_object_physics_snapshot")]
    pub player_object_physics: bool,
    #[serde(default = "default_error_mode_snapshot")]
    pub error_mode: ErrorMode,
    #[serde(default)]
    pub halted_error: Option<String>,
    #[serde(default = "default_afk_timeout_ms_snapshot")]
    pub afk_timeout_ms: u64,
    #[serde(default)]
    pub empty_since_elapsed_ms: Option<u64>,
    #[serde(default = "default_max_players_snapshot")]
    pub max_players: u32,
    #[serde(default = "default_players_character_auto_loads_snapshot")]
    pub players_character_auto_loads: bool,
    pub workspace_gravity: f32,
    #[serde(default = "default_fluid_forces_snapshot")]
    pub workspace_fluid_forces: String,
    #[serde(default)]
    pub physics_collision_groups: Vec<PhysicsCollisionGroupSnapshot>,
    #[serde(default)]
    pub physics_collision_group_pairs: Vec<PhysicsCollisionGroupPairSnapshot>,
    pub workspace_children: Vec<InstanceSnapshot>,
    #[serde(default)]
    pub server_script_service_children: Vec<InstanceSnapshot>,
    pub server_storage_children: Vec<InstanceSnapshot>,
    pub replicated_storage_children: Vec<InstanceSnapshot>,
    pub players: Vec<PlayerSnapshot>,
    #[serde(default)]
    pub pending_inputs: Vec<PendingInputSnapshot>,
    #[serde(default)]
    pub remote_events: Vec<ReplicatedEventSnapshot>,
    #[serde(default)]
    pub debris_queue: Vec<DebrisQueueSnapshot>,
    #[serde(default)]
    pub recent_replicated_events: Vec<SpectatorReplicatedEvent>,
    #[serde(default = "default_next_replicated_event_id")]
    pub next_replicated_event_id: u64,
    #[serde(default)]
    pub replication_revision: u64,
    #[serde(default)]
    pub replication_dirty_instances: Vec<ReplicationDirtyInstanceSnapshot>,
    #[serde(default)]
    pub orphan_instances: Vec<InstanceSnapshot>,
    #[serde(default)]
    pub prev_touches: Vec<TouchPairSnapshot>,
    #[serde(default)]
    pub pending_bots: Vec<String>,
    #[serde(default)]
    pub kick_requests: Vec<KickRequestSnapshot>,
    #[serde(default)]
    pub physics_commands: Vec<PhysicsCommandSnapshot>,
    pub lua_vm: LuaVmSnapshot,
}

fn default_next_replicated_event_id() -> u64 {
    1
}

fn default_max_players_snapshot() -> u32 {
    crate::game::instance::DEFAULT_MAX_PLAYERS
}

fn default_observation_radius_snapshot() -> f32 {
    crate::game::instance::DEFAULT_OBSERVATION_RADIUS
}

fn default_player_object_physics_snapshot() -> bool {
    true
}

fn default_game_status_snapshot() -> GameStatus {
    GameStatus::Playing
}

fn default_error_mode_snapshot() -> ErrorMode {
    ErrorMode::Continue
}

fn default_afk_timeout_ms_snapshot() -> u64 {
    300_000
}

fn default_players_character_auto_loads_snapshot() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsCollisionGroupSnapshot {
    pub name: String,
    pub id: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsCollisionGroupPairSnapshot {
    pub group1_id: u8,
    pub group2_id: u8,
    pub collidable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    #[serde(default)]
    pub player_instance_id: Option<u64>,
    #[serde(default)]
    pub player_gui_id: Option<u64>,
    pub agent_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub player_gui_children: Vec<InstanceSnapshot>,
    #[serde(default)]
    pub idle_ms: u64,
    #[serde(default)]
    pub character_id: Option<u64>,
    #[serde(default)]
    pub humanoid_root_part_id: Option<u64>,
    #[serde(default)]
    pub attributes: HashMap<String, SnapshotAttributeValue>,
    #[serde(default)]
    pub character_attributes: HashMap<String, SnapshotAttributeValue>,
    #[serde(default)]
    pub humanoid_root_part: Option<PlayerPartSnapshot>,
    #[serde(default)]
    pub humanoid: Option<PlayerHumanoidSnapshot>,
    #[serde(default)]
    pub character_controller: Option<PlayerCharacterControllerSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPartSnapshot {
    #[serde(default)]
    pub attributes: HashMap<String, SnapshotAttributeValue>,
    pub position: [f32; 3],
    pub cframe: SnapshotCFrame,
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHumanoidSnapshot {
    pub health: f32,
    pub max_health: f32,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_height: f32,
    pub auto_rotate: bool,
    pub move_direction: [f32; 3],
    pub move_input_direction: [f32; 3],
    pub running_speed: f32,
    pub state: String,
    #[serde(default)]
    pub move_to_target: Option<[f32; 3]>,
    pub cancel_move_to: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCharacterControllerSnapshot {
    pub vertical_velocity: f32,
    #[serde(default)]
    pub target_position: Option<[f32; 3]>,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_requested: bool,
    pub jump_buffer_remaining: f32,
    pub jump_cooldown_remaining: f32,
    pub move_to_elapsed: f32,
    pub grounded: bool,
    pub horizontal_velocity: [f32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingInputSnapshot {
    pub user_id: u64,
    pub input_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicatedEventSnapshot {
    pub name: String,
    pub payload: serde_json::Value,
    pub reliable: bool,
    #[serde(default)]
    pub target_user_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebrisQueueSnapshot {
    pub instance_id: u64,
    pub destroy_at_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchPairSnapshot {
    pub a_id: u64,
    pub b_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationDirtyInstanceSnapshot {
    pub instance_id: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickRequestSnapshot {
    pub user_id: u64,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PhysicsCommandSnapshot {
    ApplyImpulse {
        part_id: u64,
        impulse: [f32; 3],
    },
    ApplyImpulseAtPosition {
        part_id: u64,
        impulse: [f32; 3],
        position: [f32; 3],
    },
    ApplyAngularImpulse {
        part_id: u64,
        impulse: [f32; 3],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSnapshot {
    pub id: u64,
    pub class_name: String,
    pub name: String,
    #[serde(default)]
    pub destroyed: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attributes: HashMap<String, SnapshotAttributeValue>,
    #[serde(default)]
    pub part: Option<PartSnapshot>,
    #[serde(default)]
    pub model: Option<ModelSnapshot>,
    #[serde(default)]
    pub terrain: Option<TerrainSnapshot>,
    #[serde(default)]
    pub humanoid: Option<HumanoidSnapshot>,
    #[serde(default)]
    pub script: Option<ScriptSnapshot>,
    #[serde(default)]
    pub animation: Option<AnimationSnapshot>,
    #[serde(default)]
    pub animator: Option<AnimatorSnapshot>,
    #[serde(default)]
    pub sound: Option<SoundSnapshot>,
    #[serde(default)]
    pub int_value: Option<i64>,
    #[serde(default)]
    pub number_value: Option<f64>,
    #[serde(default)]
    pub bool_value: Option<bool>,
    #[serde(default)]
    pub string_value: Option<String>,
    #[serde(default)]
    pub object_value: Option<ObjectRefSnapshot>,
    #[serde(default)]
    pub cframe_value: Option<SnapshotCFrame>,
    #[serde(default)]
    pub body_velocity: Option<BodyVelocitySnapshot>,
    #[serde(default)]
    pub body_force: Option<BodyForceSnapshot>,
    #[serde(default)]
    pub body_angular_velocity: Option<BodyAngularVelocitySnapshot>,
    #[serde(default)]
    pub attachment: Option<AttachmentSnapshot>,
    #[serde(default)]
    pub vector_force: Option<VectorForceSnapshot>,
    #[serde(default)]
    pub align_position: Option<AlignPositionSnapshot>,
    #[serde(default)]
    pub align_orientation: Option<AlignOrientationSnapshot>,
    #[serde(default)]
    pub weld: Option<WeldSnapshot>,
    #[serde(default)]
    pub weld_constraint: Option<WeldConstraintSnapshot>,
    #[serde(default)]
    pub billboard_gui: Option<BillboardGuiSnapshot>,
    #[serde(default)]
    pub ui_corner: Option<UICornerSnapshot>,
    #[serde(default)]
    pub gui: Option<GuiSnapshot>,
    #[serde(default)]
    pub children: Vec<InstanceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartSnapshot {
    pub position: [f32; 3],
    pub cframe: SnapshotCFrame,
    pub size: [f32; 3],
    pub anchored: bool,
    pub can_collide: bool,
    pub can_query: bool,
    pub collision_group: String,
    #[serde(default)]
    pub collision_group_id: u8,
    pub can_touch: bool,
    pub transparency: f32,
    pub color: [f32; 3],
    pub material: String,
    pub top_surface: String,
    pub bottom_surface: String,
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub massless: bool,
    #[serde(default)]
    pub enable_fluid_forces: bool,
    #[serde(default)]
    pub custom_physical_properties: Option<PhysicalPropertiesSnapshot>,
    pub shape: String,
    #[serde(default)]
    pub mesh_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSnapshot {
    #[serde(default)]
    pub primary_part_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanoidSnapshot {
    pub health: f32,
    pub max_health: f32,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_height: f32,
    pub auto_rotate: bool,
    pub health_display_distance: f32,
    pub name_display_distance: f32,
    pub hip_height: f32,
    pub move_direction: [f32; 3],
    pub move_input_direction: [f32; 3],
    pub running_speed: f32,
    pub state: String,
    pub jump_requested: bool,
    #[serde(default)]
    pub move_to_target: Option<[f32; 3]>,
    pub cancel_move_to: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSnapshot {
    pub source: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationSnapshot {
    pub animation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatorSnapshot {
    pub loaded_tracks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundSnapshot {
    pub stream_id: String,
    pub volume: f32,
    pub playback_speed: f32,
    pub is_playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRefSnapshot {
    #[serde(default)]
    pub target_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainSnapshot {
    #[serde(default)]
    pub voxels: Vec<TerrainVoxelSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainVoxelSnapshot {
    pub cell: [i32; 3],
    pub solid_material: String,
    pub solid_occupancy: f32,
    pub liquid_occupancy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyVelocitySnapshot {
    pub velocity: [f32; 3],
    pub max_force: [f32; 3],
    #[serde(default = "default_body_mover_p")]
    pub p: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyForceSnapshot {
    pub force: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyAngularVelocitySnapshot {
    pub angular_velocity: [f32; 3],
    pub max_torque: [f32; 3],
    #[serde(default = "default_body_mover_p")]
    pub p: f32,
}

fn default_body_mover_p() -> f32 {
    1250.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeldSnapshot {
    #[serde(default)]
    pub part0_id: Option<u64>,
    #[serde(default)]
    pub part1_id: Option<u64>,
    pub c0: SnapshotCFrame,
    pub c1: SnapshotCFrame,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeldConstraintSnapshot {
    #[serde(default)]
    pub part0_id: Option<u64>,
    #[serde(default)]
    pub part1_id: Option<u64>,
    pub enabled: bool,
    #[serde(default)]
    pub offset: Option<[f32; 3]>,
    #[serde(default)]
    pub relative_cframe: Option<SnapshotCFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentSnapshot {
    pub cframe: SnapshotCFrame,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorForceSnapshot {
    #[serde(default)]
    pub attachment0_id: Option<u64>,
    #[serde(default)]
    pub attachment1_id: Option<u64>,
    pub force: [f32; 3],
    pub relative_to: String,
    pub apply_at_center_of_mass: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignPositionSnapshot {
    #[serde(default)]
    pub attachment0_id: Option<u64>,
    #[serde(default)]
    pub attachment1_id: Option<u64>,
    pub position: [f32; 3],
    pub responsiveness: f32,
    pub rigidity_enabled: bool,
    pub max_force: f32,
    #[serde(default = "default_align_position_max_velocity")]
    pub max_velocity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignOrientationSnapshot {
    #[serde(default)]
    pub attachment0_id: Option<u64>,
    #[serde(default)]
    pub attachment1_id: Option<u64>,
    pub responsiveness: f32,
    pub rigidity_enabled: bool,
    #[serde(default = "default_align_orientation_max_torque")]
    pub max_torque: [f32; 3],
}

fn default_align_position_max_velocity() -> f32 {
    f32::INFINITY
}

fn default_align_orientation_max_torque() -> [f32; 3] {
    [10000.0, 10000.0, 10000.0]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillboardGuiSnapshot {
    pub size: SnapshotUDim2,
    pub studs_offset: [f32; 3],
    pub always_on_top: bool,
    pub enabled: bool,
    #[serde(default)]
    pub adornee_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UICornerSnapshot {
    pub corner_radius: SnapshotUDim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiSnapshot {
    pub position: SnapshotUDim2,
    pub size: SnapshotUDim2,
    pub anchor_point: [f32; 2],
    pub rotation: f32,
    pub z_index: i32,
    pub layout_order: i32,
    pub visible: bool,
    pub background_color: [f32; 3],
    pub background_transparency: f32,
    pub border_color: [f32; 3],
    pub border_size_pixel: i32,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub text_color: Option<[f32; 3]>,
    #[serde(default)]
    pub text_size: Option<f32>,
    #[serde(default)]
    pub text_transparency: Option<f32>,
    #[serde(default)]
    pub text_stroke_transparency: Option<f32>,
    #[serde(default)]
    pub font: Option<String>,
    pub text_scaled: bool,
    pub text_x_alignment: String,
    pub text_y_alignment: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub image_color: Option<[f32; 3]>,
    #[serde(default)]
    pub image_transparency: Option<f32>,
    pub display_order: i32,
    pub ignore_gui_inset: bool,
    pub enabled: bool,
    pub reset_on_spawn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCFrame {
    pub position: [f32; 3],
    pub rotation: [[f32; 3]; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotUDim {
    pub scale: f32,
    pub offset: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotUDim2 {
    pub x: SnapshotUDim,
    pub y: SnapshotUDim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalPropertiesSnapshot {
    pub density: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub friction_weight: f32,
    pub elasticity_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SnapshotAttributeValue {
    String(String),
    Number(f64),
    Bool(bool),
    Vector3([f32; 3]),
    Color3([f32; 3]),
    Table(Vec<SnapshotAttributeValue>),
    Nil,
}

enum RefFixupKind {
    ObjectValue(Option<u64>),
    ModelPrimaryPart(Option<u64>),
    VectorForceAttachment0(Option<u64>),
    VectorForceAttachment1(Option<u64>),
    AlignPositionAttachment0(Option<u64>),
    AlignPositionAttachment1(Option<u64>),
    AlignOrientationAttachment0(Option<u64>),
    AlignOrientationAttachment1(Option<u64>),
    WeldPart0(Option<u64>),
    WeldPart1(Option<u64>),
    WeldConstraintPart0(Option<u64>),
    WeldConstraintPart1(Option<u64>),
    BillboardAdornee(Option<u64>),
}

struct RefFixup {
    instance: Instance,
    kind: RefFixupKind,
}

pub fn capture_game_instance(instance: &GameInstance) -> Result<GameInstanceSnapshot, String> {
    let runtime = instance
        .lua_runtime
        .as_ref()
        .ok_or_else(|| "cannot snapshot instance without an initialized Lua runtime".to_string())?;

    let workspace = runtime.workspace();
    let (workspace_gravity, workspace_fluid_forces) = {
        let data = workspace.data.lock().unwrap();
        (data.gravity, snapshot_fluid_forces(data.fluid_forces))
    };
    let players_character_auto_loads = runtime.players().data.lock().unwrap().character_auto_loads;
    let physics_service = runtime.game().physics_service();
    let physics_collision_groups = physics_service
        .snapshot_registry()
        .into_iter()
        .map(|group| PhysicsCollisionGroupSnapshot {
            name: group.name,
            id: group.id,
        })
        .collect();
    let physics_collision_group_pairs = physics_service
        .snapshot_pair_collidability()
        .into_iter()
        .map(|pair| PhysicsCollisionGroupPairSnapshot {
            group1_id: pair.group1_id,
            group2_id: pair.group2_id,
            collidable: pair.collidable,
        })
        .collect();
    let workspace_children = workspace
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;
    let server_script_service_children = runtime
        .game()
        .server_script_service()
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;
    let server_storage_children = runtime
        .game()
        .server_storage()
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;
    let replicated_storage_children = runtime
        .game()
        .replicated_storage()
        .get_children()
        .into_iter()
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;

    let players = instance
        .players
        .iter()
        .map(|(agent_id, user_id)| capture_player_snapshot(*agent_id, *user_id, instance, runtime))
        .collect::<Result<Vec<_>, _>>()?;

    let pending_inputs = runtime
        .agent_input_service()
        .data
        .lock()
        .unwrap()
        .pending_inputs
        .iter()
        .flat_map(|(user_id, queued)| {
            queued.iter().map(|input| PendingInputSnapshot {
                user_id: *user_id,
                input_type: input.input_type.clone(),
                data: input.data.clone(),
            })
        })
        .collect();

    let remote_events = runtime
        .game()
        .remote_event_service()
        .data
        .lock()
        .unwrap()
        .queued_events
        .iter()
        .map(|evt| ReplicatedEventSnapshot {
            name: evt.name.clone(),
            payload: evt.payload.clone(),
            reliable: evt.reliable,
            target_user_id: evt.target_user_id,
        })
        .collect();
    let (next_replicated_event_id, recent_replicated_events) =
        instance.snapshot_replicated_events();
    let (replication_revision, replication_dirty_instances) = runtime
        .lua()
        .app_data_ref::<SharedWorldReplicationState>()
        .map(|shared| {
            let state = shared.lock().unwrap();
            (
                state.latest_revision(),
                state
                    .snapshot_dirty_instances()
                    .into_iter()
                    .map(|entry| ReplicationDirtyInstanceSnapshot {
                        instance_id: entry.id,
                        revision: entry.revision,
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    let prev_touches = instance
        .snapshot_prev_touches()
        .into_iter()
        .map(|(a_id, b_id)| TouchPairSnapshot { a_id, b_id })
        .collect();
    let pending_bots = runtime
        .players()
        .data
        .lock()
        .unwrap()
        .pending_bots
        .iter()
        .cloned()
        .collect();
    let kick_requests = runtime
        .game()
        .snapshot_kick_requests()
        .into_iter()
        .map(|request| KickRequestSnapshot {
            user_id: request.user_id,
            message: request.message,
        })
        .collect();
    let physics_commands = runtime
        .game()
        .snapshot_physics_commands()
        .into_iter()
        .map(snapshot_physics_command)
        .collect();
    let debris_queue = runtime
        .game()
        .debris_service()
        .snapshot_queue()
        .into_iter()
        .map(|entry| DebrisQueueSnapshot {
            instance_id: entry.instance.id().0,
            destroy_at_secs: entry.destroy_at_secs,
        })
        .collect();
    let lua_vm_capture = runtime.capture_vm_snapshot_with_references()?;
    let mut player_root_ids = players
        .iter()
        .flat_map(|player| [player.player_instance_id, player.player_gui_id])
        .flatten()
        .collect::<Vec<_>>();
    let mut player_gui_child_ids = HashSet::new();
    for player in &players {
        collect_instance_snapshot_ids(&player.player_gui_children, &mut player_gui_child_ids);
    }
    player_root_ids.extend(player_gui_child_ids);
    let orphan_instances = capture_orphan_instance_roots(
        &lua_vm_capture.referenced_instances,
        [
            workspace_children.as_slice(),
            server_script_service_children.as_slice(),
            server_storage_children.as_slice(),
            replicated_storage_children.as_slice(),
        ],
        player_root_ids,
    )?;

    Ok(GameInstanceSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        game_id: instance.game_id,
        instance_id: instance.instance_id,
        tick: instance.tick,
        status: instance.status,
        elapsed_ms: instance.elapsed_ms(),
        observation_radius: instance.observation_radius,
        player_object_physics: instance.player_object_physics(),
        error_mode: instance.error_mode,
        halted_error: instance.halted_error.clone(),
        afk_timeout_ms: instance.snapshot_afk_timeout_ms(),
        empty_since_elapsed_ms: instance.snapshot_empty_since_elapsed_ms(),
        max_players: instance.max_players,
        players_character_auto_loads,
        workspace_gravity,
        workspace_fluid_forces,
        physics_collision_groups,
        physics_collision_group_pairs,
        workspace_children,
        server_script_service_children,
        server_storage_children,
        replicated_storage_children,
        players,
        pending_inputs,
        remote_events,
        debris_queue,
        recent_replicated_events,
        next_replicated_event_id,
        replication_revision,
        replication_dirty_instances,
        orphan_instances,
        prev_touches,
        pending_bots,
        kick_requests,
        physics_commands,
        lua_vm: lua_vm_capture.snapshot,
    })
}

pub fn restore_game_instance(
    instance: &mut GameInstance,
    snapshot: &GameInstanceSnapshot,
) -> Result<(), String> {
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported snapshot schema version {}",
            snapshot.schema_version
        ));
    }

    let (
        workspace,
        server_script_service,
        server_storage,
        replicated_storage,
        players_service,
        agent_input_service,
        remote_event_service,
        physics_service,
        debris_service,
    ) = {
        let runtime = instance.lua_runtime.as_ref().ok_or_else(|| {
            "cannot restore instance without an initialized Lua runtime".to_string()
        })?;
        (
            runtime.workspace(),
            runtime.game().server_script_service(),
            runtime.game().server_storage(),
            runtime.game().replicated_storage(),
            runtime.players(),
            runtime.agent_input_service(),
            runtime.game().remote_event_service(),
            runtime.game().physics_service(),
            runtime.game().debris_service(),
        )
    };

    instance.game_id = snapshot.game_id;
    instance.instance_id = snapshot.instance_id;
    instance.tick = snapshot.tick;
    instance.status = snapshot.status;
    instance.max_players = snapshot.max_players;
    instance.observation_radius = snapshot.observation_radius;
    instance.set_player_object_physics(snapshot.player_object_physics);
    instance.error_mode = snapshot.error_mode;
    instance.halted_error = snapshot.halted_error.clone();
    {
        let runtime = instance.lua_runtime.as_ref().ok_or_else(|| {
            "cannot restore runtime error mode without an initialized Lua runtime".to_string()
        })?;
        runtime.lua().set_app_data(instance.error_mode);
    }
    instance.restore_elapsed_ms(snapshot.elapsed_ms);
    instance.restore_afk_timeout_ms(snapshot.afk_timeout_ms);
    instance.restore_replicated_events(
        snapshot.next_replicated_event_id,
        snapshot.recent_replicated_events.clone(),
    );
    instance.restore_prev_touches(
        snapshot
            .prev_touches
            .iter()
            .map(|pair| (pair.a_id, pair.b_id))
            .collect(),
    );

    {
        let mut data = workspace.data.lock().unwrap();
        data.gravity = snapshot.workspace_gravity;
        data.fluid_forces = restore_fluid_forces(&snapshot.workspace_fluid_forces)?;
    }
    restore_physics_service(
        &physics_service,
        &snapshot.physics_collision_groups,
        &snapshot.physics_collision_group_pairs,
    )?;

    let mut instance_map = HashMap::new();
    let mut fixups = Vec::new();

    clear_workspace(&workspace);
    clear_instance_children(&server_script_service);
    clear_instance_children(&server_storage);
    clear_instance_children(&replicated_storage);

    restore_root_children(
        &snapshot.workspace_children,
        &workspace.instance,
        Some(&workspace),
        &mut instance_map,
        &mut fixups,
    )?;
    restore_root_children(
        &snapshot.server_script_service_children,
        &server_script_service,
        None,
        &mut instance_map,
        &mut fixups,
    )?;
    restore_root_children(
        &snapshot.server_storage_children,
        &server_storage,
        None,
        &mut instance_map,
        &mut fixups,
    )?;
    restore_root_children(
        &snapshot.replicated_storage_children,
        &replicated_storage,
        None,
        &mut instance_map,
        &mut fixups,
    )?;
    restore_orphan_instance_roots(&snapshot.orphan_instances, &mut instance_map, &mut fixups)?;

    apply_ref_fixups(&instance_map, fixups);

    restore_players(instance, snapshot, &players_service, &mut instance_map)?;
    instance.restore_empty_since_elapsed_ms(snapshot.empty_since_elapsed_ms);
    restore_replication_state(instance, snapshot, &instance_map)?;

    {
        let mut pending = agent_input_service.data.lock().unwrap();
        pending.pending_inputs.clear();
        for input in &snapshot.pending_inputs {
            pending
                .pending_inputs
                .entry(input.user_id)
                .or_insert_with(Vec::new)
                .push(AgentInput::new(
                    input.input_type.clone(),
                    input.data.clone(),
                ));
        }
    }

    {
        let mut queued = remote_event_service.data.lock().unwrap();
        queued.queued_events = snapshot
            .remote_events
            .iter()
            .map(|evt| ReplicatedEvent {
                name: evt.name.clone(),
                payload: evt.payload.clone(),
                reliable: evt.reliable,
                target_user_id: evt.target_user_id,
            })
            .collect();
    }
    let debris_entries = snapshot
        .debris_queue
        .iter()
        .map(|entry| {
            let instance = instance_map
                .get(&entry.instance_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "missing restored debris target instance {}",
                        entry.instance_id
                    )
                })?;
            Ok(DebrisEntrySnapshot {
                instance,
                destroy_at_secs: entry.destroy_at_secs,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    debris_service.restore_queue(debris_entries);
    {
        let mut players_data = players_service.data.lock().unwrap();
        players_data.max_players = snapshot.max_players;
        players_data.character_auto_loads = snapshot.players_character_auto_loads;
        players_data.pending_bots = snapshot.pending_bots.iter().cloned().collect();
    }
    let game = {
        let runtime = instance.lua_runtime.as_ref().ok_or_else(|| {
            "cannot restore host queues without an initialized Lua runtime".to_string()
        })?;
        runtime.game()
    };
    restore_runtime_queues(
        &instance_map,
        &snapshot.kick_requests,
        &snapshot.physics_commands,
        &game,
    )?;

    let runtime = instance.lua_runtime.as_mut().ok_or_else(|| {
        "cannot restore Lua VM snapshot without an initialized Lua runtime".to_string()
    })?;
    runtime.restore_vm_snapshot_with_instances(
        &snapshot.lua_vm,
        instance_map.values().cloned().collect::<Vec<_>>(),
    )?;

    Ok(())
}

fn snapshot_instance(instance: Instance) -> Result<InstanceSnapshot, String> {
    let data = instance.data.lock().unwrap();
    let children_refs = data.children.clone();
    let snapshot = InstanceSnapshot {
        id: data.id.0,
        class_name: data.class_name.as_str().to_string(),
        name: data.name.clone(),
        destroyed: data.is_destroyed(),
        tags: data.tags.iter().cloned().collect(),
        attributes: data
            .attributes
            .iter()
            .map(|(key, value)| (key.clone(), snapshot_attribute_value(value)))
            .collect(),
        part: data.part_data.as_ref().map(snapshot_part_data),
        model: data.model_data.as_ref().map(snapshot_model_data),
        terrain: data.terrain_data.as_ref().map(snapshot_terrain_data),
        humanoid: data.humanoid_data.as_ref().map(snapshot_humanoid_data),
        script: data.script_data.as_ref().map(snapshot_script_data),
        animation: data.animation_data.as_ref().map(snapshot_animation_data),
        animator: data.animator_data.as_ref().map(snapshot_animator_data),
        sound: data.sound_data.as_ref().map(snapshot_sound_data),
        int_value: data.int_value_data.as_ref().map(|v| v.value),
        number_value: data.number_value_data.as_ref().map(|v| v.value),
        bool_value: data.bool_value_data.as_ref().map(|v| v.value),
        string_value: data.string_value_data.as_ref().map(|v| v.value.clone()),
        object_value: data
            .object_value_data
            .as_ref()
            .map(snapshot_object_value_data),
        cframe_value: data
            .cframe_value_data
            .as_ref()
            .map(snapshot_cframe_value_data),
        body_velocity: data
            .body_velocity_data
            .as_ref()
            .map(snapshot_body_velocity_data),
        body_force: data.body_force_data.as_ref().map(snapshot_body_force_data),
        body_angular_velocity: data
            .body_angular_velocity_data
            .as_ref()
            .map(snapshot_body_angular_velocity_data),
        attachment: data.attachment_data.as_ref().map(snapshot_attachment_data),
        vector_force: data
            .vector_force_data
            .as_ref()
            .map(snapshot_vector_force_data),
        align_position: data
            .align_position_data
            .as_ref()
            .map(snapshot_align_position_data),
        align_orientation: data
            .align_orientation_data
            .as_ref()
            .map(snapshot_align_orientation_data),
        weld: data.weld_data.as_ref().map(snapshot_weld_data),
        weld_constraint: data
            .weld_constraint_data
            .as_ref()
            .map(snapshot_weld_constraint_data),
        billboard_gui: data
            .billboard_gui_data
            .as_ref()
            .map(snapshot_billboard_gui_data),
        ui_corner: data.ui_corner_data.as_ref().map(snapshot_ui_corner_data),
        gui: data.gui_data.as_ref().map(snapshot_gui_data),
        children: Vec::new(),
    };
    drop(data);

    let children = children_refs
        .into_iter()
        .map(Instance::from_ref)
        .map(snapshot_instance)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(InstanceSnapshot {
        children,
        ..snapshot
    })
}

fn capture_orphan_instance_roots<'a>(
    referenced_instances: &[Instance],
    rooted_groups: impl IntoIterator<Item = &'a [InstanceSnapshot]>,
    additional_rooted_ids: impl IntoIterator<Item = u64>,
) -> Result<Vec<InstanceSnapshot>, String> {
    let mut rooted_ids = HashSet::new();
    for group in rooted_groups {
        collect_instance_snapshot_ids(group, &mut rooted_ids);
    }
    rooted_ids.extend(additional_rooted_ids);

    let mut roots_by_id = HashMap::new();
    for instance in referenced_instances {
        let Some(root) = topmost_unrooted_ancestor(instance, &rooted_ids) else {
            continue;
        };
        roots_by_id.entry(root.id().0).or_insert(root);
    }

    let mut roots = roots_by_id.into_values().collect::<Vec<_>>();
    roots.sort_by_key(|instance| instance.id().0);
    roots.into_iter().map(snapshot_instance).collect()
}

fn collect_instance_snapshot_ids(snapshots: &[InstanceSnapshot], ids: &mut HashSet<u64>) {
    for snapshot in snapshots {
        ids.insert(snapshot.id);
        collect_instance_snapshot_ids(&snapshot.children, ids);
    }
}

fn topmost_unrooted_ancestor(instance: &Instance, rooted_ids: &HashSet<u64>) -> Option<Instance> {
    if rooted_ids.contains(&instance.id().0) {
        return None;
    }

    let mut root = instance.clone();
    while let Some(parent) = root.parent() {
        if rooted_ids.contains(&parent.id().0) {
            return None;
        }
        root = parent;
    }
    Some(root)
}

fn capture_player_snapshot(
    agent_id: Uuid,
    user_id: u64,
    instance: &GameInstance,
    runtime: &crate::game::lua::LuaRuntime,
) -> Result<PlayerSnapshot, String> {
    let player = runtime
        .players()
        .get_player_by_user_id(user_id)
        .ok_or_else(|| format!("missing player instance for user_id {}", user_id))?;
    let character = player
        .data
        .lock()
        .unwrap()
        .player_data
        .as_ref()
        .and_then(|pdata| pdata.character.as_ref())
        .and_then(|weak| weak.upgrade())
        .map(Instance::from_ref);

    let mut humanoid_root_part = None;
    let mut humanoid = None;
    let mut character_controller = None;
    let mut character_attributes = HashMap::new();
    let mut character_id = None;
    let mut humanoid_root_part_id = None;

    if let Some(character) = character {
        character_id = Some(character.id().0);
        {
            let char_data = character.data.lock().unwrap();
            character_attributes = char_data
                .attributes
                .iter()
                .map(|(key, value)| (key.clone(), snapshot_attribute_value(value)))
                .collect();
        }

        for child in character.get_children() {
            let data = child.data.lock().unwrap();
            if let Some(part) = &data.part_data {
                if data.name == "HumanoidRootPart" {
                    humanoid_root_part_id = Some(data.id.0);
                    character_controller =
                        instance
                            .physics
                            .get_character_state(data.id.0)
                            .map(|state| PlayerCharacterControllerSnapshot {
                                vertical_velocity: state.vertical_velocity,
                                target_position: state.target_position,
                                walk_speed: state.walk_speed,
                                jump_power: state.jump_power,
                                jump_requested: state.jump_requested,
                                jump_buffer_remaining: state.jump_buffer_remaining,
                                jump_cooldown_remaining: state.jump_cooldown_remaining,
                                move_to_elapsed: state.move_to_elapsed,
                                grounded: state.grounded,
                                horizontal_velocity: state.horizontal_velocity,
                            });
                    humanoid_root_part = Some(PlayerPartSnapshot {
                        attributes: data
                            .attributes
                            .iter()
                            .map(|(key, value)| (key.clone(), snapshot_attribute_value(value)))
                            .collect(),
                        position: snapshot_vector3(part.position),
                        cframe: snapshot_cframe(part.cframe),
                        velocity: snapshot_vector3(part.velocity),
                        angular_velocity: snapshot_vector3(part.angular_velocity),
                    });
                }
            }
            if let Some(h) = &data.humanoid_data {
                humanoid = Some(PlayerHumanoidSnapshot {
                    health: h.health,
                    max_health: h.max_health,
                    walk_speed: h.walk_speed,
                    jump_power: h.jump_power,
                    jump_height: h.jump_height,
                    auto_rotate: h.auto_rotate,
                    move_direction: snapshot_vector3(h.move_direction),
                    move_input_direction: snapshot_vector3(h.move_input_direction),
                    running_speed: h.running_speed,
                    state: snapshot_humanoid_state(h.state),
                    move_to_target: h.move_to_target.map(snapshot_vector3),
                    cancel_move_to: h.cancel_move_to,
                });
            }
        }
    }

    let (player_instance_id, player_gui) = {
        let data = player.data.lock().unwrap();
        let player_gui = data
            .player_data
            .as_ref()
            .and_then(|pdata| pdata.player_gui.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(Instance::from_ref);
        (data.id.0, player_gui)
    };
    let player_gui_id = player_gui.as_ref().map(|gui| gui.id().0);
    let player_gui_children = player_gui
        .map(|gui| {
            gui.get_children()
                .into_iter()
                .map(snapshot_instance)
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(PlayerSnapshot {
        player_instance_id: Some(player_instance_id),
        player_gui_id,
        agent_id,
        name: instance
            .player_names
            .get(&agent_id)
            .cloned()
            .unwrap_or_else(|| format!("Player-{}", user_id)),
        player_gui_children,
        idle_ms: instance
            .snapshot_player_idle_ms(agent_id)
            .unwrap_or_default(),
        character_id,
        humanoid_root_part_id,
        attributes: player
            .get_attributes()
            .into_iter()
            .map(|(key, value)| (key, snapshot_attribute_value(&value)))
            .collect(),
        character_attributes,
        humanoid_root_part,
        humanoid,
        character_controller,
    })
}

fn infer_restored_character_grounded(instance: &GameInstance, hrp_id: u64, height: f32) -> bool {
    let Some(state) = instance.physics.get_character_state(hrp_id) else {
        return false;
    };
    let Some(position) = instance.physics.get_character_position(hrp_id) else {
        return false;
    };

    let half_height = height * 0.5;
    let max_distance = half_height + crate::game::constants::physics::SNAP_TO_GROUND + 0.1;
    let Some((toi, _)) =
        instance
            .physics
            .raycast_down(position, max_distance, Some(state.body_handle))
    else {
        return false;
    };

    let support_gap = (toi - half_height).max(0.0);
    support_gap <= crate::game::constants::physics::SNAP_TO_GROUND + 0.1
}

fn restore_players(
    instance: &mut GameInstance,
    snapshot: &GameInstanceSnapshot,
    players_service: &crate::game::lua::services::PlayersService,
    instance_map: &mut HashMap<u64, Instance>,
) -> Result<(), String> {
    instance.players.clear();
    instance.player_hrp_ids.clear();
    instance.player_names.clear();
    players_service.data.lock().unwrap().players.clear();

    let mut player_fixups = Vec::new();
    for player in &snapshot.players {
        let user_id = GameInstance::user_id_from_agent_id(player.agent_id);
        instance.players.insert(player.agent_id, user_id);
        instance
            .player_names
            .insert(player.agent_id, player.name.clone());
        instance.restore_player_idle_ms(player.agent_id, player.idle_ms);

        let player_instance = Instance::from_data(InstanceData::new_player(user_id, &player.name));
        let player_gui = Instance::from_data(InstanceData::new_player_gui("PlayerGui"));
        {
            let mut data = player_instance.data.lock().unwrap();
            if let Some(player_instance_id) = player.player_instance_id {
                data.id = InstanceId(player_instance_id);
            }
        }
        {
            let mut data = player_gui.data.lock().unwrap();
            if let Some(player_gui_id) = player.player_gui_id {
                data.id = InstanceId(player_gui_id);
            }
        }
        player_gui.set_parent(Some(&player_instance));
        {
            let mut player_data = player_instance.data.lock().unwrap();
            let pdata = player_data
                .player_data
                .as_mut()
                .ok_or_else(|| format!("restored player {} missing player data", player.name))?;
            pdata.player_gui = Some(std::sync::Arc::downgrade(&player_gui.data));
        }
        players_service.add_player(player_instance.clone());
        instance_map.insert(player_instance.id().0, player_instance.clone());
        instance_map.insert(player_gui.id().0, player_gui.clone());
        restore_root_children(
            &player.player_gui_children,
            &player_gui,
            None,
            instance_map,
            &mut player_fixups,
        )?;

        apply_attributes(&player_instance, &player.attributes);

        let character = match player.character_id {
            Some(character_id) => {
                let character = instance_map.get(&character_id).cloned().ok_or_else(|| {
                    format!(
                        "missing restored character {} for player {}",
                        character_id, player.name
                    )
                })?;
                {
                    let mut player_data = player_instance.data.lock().unwrap();
                    let pdata = player_data.player_data.as_mut().ok_or_else(|| {
                        format!("restored player {} missing player data", player.name)
                    })?;
                    pdata.character = Some(std::sync::Arc::downgrade(&character.data));
                }
                Some(character)
            }
            None => {
                if !player.character_attributes.is_empty()
                    || player.humanoid_root_part.is_some()
                    || player.humanoid.is_some()
                {
                    return Err(format!(
                        "snapshot for player {} is missing character ids; resnapshot with the current engine before resuming",
                        player.name
                    ));
                }
                None
            }
        };

        if let Some(character) = character {
            apply_attributes(&character, &player.character_attributes);

            let mut restored_humanoid = None;
            for child in character.get_children() {
                let name = child.name();
                if name == "Humanoid" {
                    restored_humanoid = Some(child.clone());
                }
            }

            if let Some(expected_hrp_id) = player.humanoid_root_part_id {
                let hrp = instance_map.get(&expected_hrp_id).cloned().ok_or_else(|| {
                    format!(
                        "missing restored HumanoidRootPart {} for player {}",
                        expected_hrp_id, player.name
                    )
                })?;
                if !hrp.is_descendant_of(&character) {
                    return Err(format!(
                        "restored HumanoidRootPart {} is not under character {}",
                        expected_hrp_id, player.name
                    ));
                }
                let hrp_snapshot = player.humanoid_root_part.as_ref().ok_or_else(|| {
                    format!(
                        "missing HumanoidRootPart snapshot data for restored player {}",
                        player.name
                    )
                })?;

                apply_attributes(&hrp, &hrp_snapshot.attributes);
                let hrp_id = hrp.id().0;
                let (radius, height, hrp_id) = {
                    let mut data = hrp.data.lock().unwrap();
                    let part = data.part_data.as_mut().ok_or_else(|| {
                        format!(
                            "restored HumanoidRootPart missing part data for {}",
                            player.name
                        )
                    })?;
                    let radius = (part.size.x.max(part.size.z)) * 0.5;
                    let height = part.size.y;
                    part.position = restore_vector3(hrp_snapshot.position);
                    part.cframe = restore_cframe(&hrp_snapshot.cframe);
                    part.velocity = restore_vector3(hrp_snapshot.velocity);
                    part.angular_velocity = restore_vector3(hrp_snapshot.angular_velocity);
                    part.position_dirty = false;
                    part.velocity_dirty = false;
                    part.angular_velocity_dirty = false;
                    (radius, height, hrp_id)
                };

                instance
                    .physics
                    .add_character(hrp_id, hrp_snapshot.position, radius, height);
                instance.player_hrp_ids.insert(player.agent_id, hrp_id);
                instance
                    .physics
                    .set_character_position(hrp_id, hrp_snapshot.position);
                instance
                    .physics
                    .set_character_rotation(hrp_id, &hrp_snapshot.cframe.rotation);
                let inferred_grounded = infer_restored_character_grounded(instance, hrp_id, height);
                if let Some(state) = instance.physics.get_character_state_mut(hrp_id) {
                    if let Some(controller_snapshot) = player.character_controller.as_ref() {
                        state.vertical_velocity = controller_snapshot.vertical_velocity;
                        state.target_position = controller_snapshot.target_position;
                        state.walk_speed = controller_snapshot.walk_speed;
                        state.jump_power = controller_snapshot.jump_power;
                        state.jump_requested = controller_snapshot.jump_requested;
                        state.jump_buffer_remaining = controller_snapshot.jump_buffer_remaining;
                        state.jump_cooldown_remaining = controller_snapshot.jump_cooldown_remaining;
                        state.move_to_elapsed = controller_snapshot.move_to_elapsed;
                        state.grounded = controller_snapshot.grounded;
                        state.horizontal_velocity = controller_snapshot.horizontal_velocity;
                    } else {
                        state.grounded = inferred_grounded;
                    }
                }
            }

            if let (Some(humanoid_inst), Some(h_snapshot)) =
                (restored_humanoid, player.humanoid.as_ref())
            {
                let mut data = humanoid_inst.data.lock().unwrap();
                if let Some(humanoid) = data.humanoid_data.as_mut() {
                    humanoid.health = h_snapshot.health;
                    humanoid.max_health = h_snapshot.max_health;
                    humanoid.walk_speed = h_snapshot.walk_speed;
                    humanoid.jump_power = h_snapshot.jump_power;
                    humanoid.jump_height = h_snapshot.jump_height;
                    humanoid.auto_rotate = h_snapshot.auto_rotate;
                    humanoid.move_direction = restore_vector3(h_snapshot.move_direction);
                    humanoid.move_input_direction =
                        restore_vector3(h_snapshot.move_input_direction);
                    humanoid.running_speed = h_snapshot.running_speed;
                    humanoid.state = restore_humanoid_state(&h_snapshot.state)?;
                    humanoid.move_to_target = h_snapshot.move_to_target.map(restore_vector3);
                    humanoid.cancel_move_to = h_snapshot.cancel_move_to;
                }
            }
        }
    }

    apply_ref_fixups(instance_map, player_fixups);

    instance.empty_since = if instance.players.is_empty() {
        Some(Instant::now())
    } else {
        None
    };

    Ok(())
}

fn restore_physics_service(
    physics_service: &PhysicsService,
    groups: &[PhysicsCollisionGroupSnapshot],
    pairs: &[PhysicsCollisionGroupPairSnapshot],
) -> Result<(), String> {
    let groups = if groups.is_empty() {
        vec![CollisionGroupRegistryEntry {
            name: "Default".to_string(),
            id: 0,
        }]
    } else {
        groups
            .iter()
            .map(|group| CollisionGroupRegistryEntry {
                name: group.name.clone(),
                id: group.id,
            })
            .collect()
    };
    let pairs = pairs
        .iter()
        .map(|pair| CollisionGroupPairEntry {
            group1_id: pair.group1_id,
            group2_id: pair.group2_id,
            collidable: pair.collidable,
        })
        .collect::<Vec<_>>();
    physics_service
        .restore_registry(&groups, &pairs)
        .map_err(|err| err.to_string())
}

fn snapshot_physics_command(command: PhysicsCommand) -> PhysicsCommandSnapshot {
    match command {
        PhysicsCommand::ApplyImpulse { part_id, impulse } => PhysicsCommandSnapshot::ApplyImpulse {
            part_id,
            impulse: snapshot_vector3(impulse),
        },
        PhysicsCommand::ApplyImpulseAtPosition {
            part_id,
            impulse,
            position,
        } => PhysicsCommandSnapshot::ApplyImpulseAtPosition {
            part_id,
            impulse: snapshot_vector3(impulse),
            position: snapshot_vector3(position),
        },
        PhysicsCommand::ApplyAngularImpulse { part_id, impulse } => {
            PhysicsCommandSnapshot::ApplyAngularImpulse {
                part_id,
                impulse: snapshot_vector3(impulse),
            }
        }
    }
}

fn restore_physics_command(
    command: &PhysicsCommandSnapshot,
    instance_map: &HashMap<u64, Instance>,
) -> Result<PhysicsCommand, String> {
    let part_id = match command {
        PhysicsCommandSnapshot::ApplyImpulse { part_id, .. }
        | PhysicsCommandSnapshot::ApplyImpulseAtPosition { part_id, .. }
        | PhysicsCommandSnapshot::ApplyAngularImpulse { part_id, .. } => *part_id,
    };
    let target = instance_map.get(&part_id).ok_or_else(|| {
        format!(
            "missing restored physics command target instance {}",
            part_id
        )
    })?;
    if target.data.lock().unwrap().part_data.is_none() {
        return Err(format!(
            "restored physics command target {} is not a BasePart",
            part_id
        ));
    }

    Ok(match command {
        PhysicsCommandSnapshot::ApplyImpulse { part_id, impulse } => PhysicsCommand::ApplyImpulse {
            part_id: *part_id,
            impulse: restore_vector3(*impulse),
        },
        PhysicsCommandSnapshot::ApplyImpulseAtPosition {
            part_id,
            impulse,
            position,
        } => PhysicsCommand::ApplyImpulseAtPosition {
            part_id: *part_id,
            impulse: restore_vector3(*impulse),
            position: restore_vector3(*position),
        },
        PhysicsCommandSnapshot::ApplyAngularImpulse { part_id, impulse } => {
            PhysicsCommand::ApplyAngularImpulse {
                part_id: *part_id,
                impulse: restore_vector3(*impulse),
            }
        }
    })
}

fn restore_runtime_queues(
    instance_map: &HashMap<u64, Instance>,
    kick_requests: &[KickRequestSnapshot],
    physics_commands: &[PhysicsCommandSnapshot],
    game: &Game,
) -> Result<(), String> {
    game.restore_kick_requests(
        kick_requests
            .iter()
            .map(|request| KickRequest {
                user_id: request.user_id,
                message: request.message.clone(),
            })
            .collect(),
    );
    game.restore_physics_commands(
        physics_commands
            .iter()
            .map(|command| restore_physics_command(command, instance_map))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(())
}

fn restore_replication_state(
    instance: &GameInstance,
    snapshot: &GameInstanceSnapshot,
    instance_map: &HashMap<u64, Instance>,
) -> Result<(), String> {
    let runtime = instance.lua_runtime.as_ref().ok_or_else(|| {
        "cannot restore replication state without an initialized Lua runtime".to_string()
    })?;
    let Some(shared) = runtime.lua().app_data_ref::<SharedWorldReplicationState>() else {
        return Ok(());
    };
    let entries = snapshot
        .replication_dirty_instances
        .iter()
        .map(|entry| {
            let weak = instance_map
                .get(&entry.instance_id)
                .map(|instance| std::sync::Arc::downgrade(&instance.data))
                .unwrap_or_else(std::sync::Weak::new);
            (
                DirtyInstanceSnapshot {
                    id: entry.instance_id,
                    revision: entry.revision,
                },
                weak,
            )
        })
        .collect::<Vec<_>>();
    shared
        .lock()
        .unwrap()
        .restore_dirty_instances(snapshot.replication_revision, entries);
    Ok(())
}

fn restore_root_children(
    children: &[InstanceSnapshot],
    root: &Instance,
    workspace: Option<&crate::game::lua::services::WorkspaceService>,
    map: &mut HashMap<u64, Instance>,
    fixups: &mut Vec<RefFixup>,
) -> Result<(), String> {
    for child in children {
        let instance = restore_instance_node(child, map, fixups)?;
        if let Some(workspace) = workspace {
            workspace.add_child(instance);
        } else {
            instance.set_parent(Some(root));
        }
    }
    Ok(())
}

fn restore_orphan_instance_roots(
    roots: &[InstanceSnapshot],
    map: &mut HashMap<u64, Instance>,
    fixups: &mut Vec<RefFixup>,
) -> Result<(), String> {
    for root in roots {
        restore_instance_node(root, map, fixups)?;
    }
    Ok(())
}

fn restore_instance_node(
    snapshot: &InstanceSnapshot,
    map: &mut HashMap<u64, Instance>,
    fixups: &mut Vec<RefFixup>,
) -> Result<Instance, String> {
    let instance = create_instance_from_class_name(&snapshot.class_name, &snapshot.name)?;
    map.insert(snapshot.id, instance.clone());
    apply_instance_snapshot(&instance, snapshot, fixups)?;
    for child in &snapshot.children {
        let child_instance = restore_instance_node(child, map, fixups)?;
        child_instance.set_parent(Some(&instance));
    }
    Ok(instance)
}

fn apply_instance_snapshot(
    instance: &Instance,
    snapshot: &InstanceSnapshot,
    fixups: &mut Vec<RefFixup>,
) -> Result<(), String> {
    {
        let mut data = instance.data.lock().unwrap();
        data.id = InstanceId(snapshot.id);
        data.name = snapshot.name.clone();
        data.set_destroyed_for_restore(snapshot.destroyed);
        data.tags = snapshot.tags.iter().cloned().collect();
        data.attributes = snapshot
            .attributes
            .iter()
            .map(|(key, value)| (key.clone(), restore_attribute_value(value)))
            .collect();

        if let (Some(part), Some(part_snapshot)) = (data.part_data.as_mut(), snapshot.part.as_ref())
        {
            apply_part_snapshot(part, part_snapshot)?;
        }
        if let (Some(model), Some(model_snapshot)) =
            (data.model_data.as_mut(), snapshot.model.as_ref())
        {
            model.primary_part = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::ModelPrimaryPart(model_snapshot.primary_part_id),
            });
        }
        if let (Some(humanoid), Some(h_snapshot)) =
            (data.humanoid_data.as_mut(), snapshot.humanoid.as_ref())
        {
            apply_humanoid_snapshot(humanoid, h_snapshot)?;
        }
        if let (Some(script), Some(script_snapshot)) =
            (data.script_data.as_mut(), snapshot.script.as_ref())
        {
            script.source = script_snapshot.source.clone();
            script.disabled = script_snapshot.disabled;
        }
        if let (Some(animation), Some(saved)) =
            (data.animation_data.as_mut(), snapshot.animation.as_ref())
        {
            animation.animation_id = saved.animation_id.clone();
        }
        if let (Some(animator), Some(saved)) =
            (data.animator_data.as_mut(), snapshot.animator.as_ref())
        {
            animator.loaded_tracks = saved.loaded_tracks;
        }
        if let (Some(sound), Some(saved)) = (data.sound_data.as_mut(), snapshot.sound.as_ref()) {
            sound.stream_id = saved.stream_id.clone();
            sound.volume = saved.volume;
            sound.playback_speed = saved.playback_speed;
            sound.is_playing = saved.is_playing;
        }
        if let (Some(value), Some(saved)) = (data.int_value_data.as_mut(), snapshot.int_value) {
            value.value = saved;
        }
        if let (Some(value), Some(saved)) = (data.number_value_data.as_mut(), snapshot.number_value)
        {
            value.value = saved;
        }
        if let (Some(value), Some(saved)) = (data.bool_value_data.as_mut(), snapshot.bool_value) {
            value.value = saved;
        }
        if let (Some(value), Some(saved)) = (
            data.string_value_data.as_mut(),
            snapshot.string_value.as_ref(),
        ) {
            value.value = saved.clone();
        }
        if data.object_value_data.is_some() {
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::ObjectValue(
                    snapshot
                        .object_value
                        .as_ref()
                        .and_then(|value| value.target_id),
                ),
            });
        }
        if let (Some(value), Some(saved)) = (
            data.cframe_value_data.as_mut(),
            snapshot.cframe_value.as_ref(),
        ) {
            value.value = restore_cframe(saved);
        }
        if let (Some(terrain), Some(saved)) =
            (data.terrain_data.as_mut(), snapshot.terrain.as_ref())
        {
            *terrain = restore_terrain_data(saved)?;
        }
        if let (Some(body_velocity), Some(saved)) = (
            data.body_velocity_data.as_mut(),
            snapshot.body_velocity.as_ref(),
        ) {
            body_velocity.velocity = restore_vector3(saved.velocity);
            body_velocity.max_force = restore_vector3(saved.max_force);
            body_velocity.p = saved.p;
        }
        if let (Some(body_force), Some(saved)) =
            (data.body_force_data.as_mut(), snapshot.body_force.as_ref())
        {
            body_force.force = restore_vector3(saved.force);
        }
        if let (Some(body_angular_velocity), Some(saved)) = (
            data.body_angular_velocity_data.as_mut(),
            snapshot.body_angular_velocity.as_ref(),
        ) {
            body_angular_velocity.angular_velocity = restore_vector3(saved.angular_velocity);
            body_angular_velocity.max_torque = restore_vector3(saved.max_torque);
            body_angular_velocity.p = saved.p;
        }
        if let (Some(attachment), Some(saved)) =
            (data.attachment_data.as_mut(), snapshot.attachment.as_ref())
        {
            attachment.cframe = restore_cframe(&saved.cframe);
        }
        if let (Some(vector_force), Some(saved)) = (
            data.vector_force_data.as_mut(),
            snapshot.vector_force.as_ref(),
        ) {
            vector_force.force = restore_vector3(saved.force);
            vector_force.relative_to = restore_actuator_relative_to(&saved.relative_to)?;
            vector_force.apply_at_center_of_mass = saved.apply_at_center_of_mass;
            vector_force.enabled = saved.enabled;
            vector_force.attachment0 = None;
            vector_force.attachment1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::VectorForceAttachment0(saved.attachment0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::VectorForceAttachment1(saved.attachment1_id),
            });
        }
        if let (Some(align), Some(saved)) = (
            data.align_position_data.as_mut(),
            snapshot.align_position.as_ref(),
        ) {
            align.position = restore_vector3(saved.position);
            align.responsiveness = saved.responsiveness;
            align.rigidity_enabled = saved.rigidity_enabled;
            align.max_force = saved.max_force;
            align.max_velocity = saved.max_velocity;
            align.attachment0 = None;
            align.attachment1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::AlignPositionAttachment0(saved.attachment0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::AlignPositionAttachment1(saved.attachment1_id),
            });
        }
        if let (Some(align), Some(saved)) = (
            data.align_orientation_data.as_mut(),
            snapshot.align_orientation.as_ref(),
        ) {
            align.responsiveness = saved.responsiveness;
            align.rigidity_enabled = saved.rigidity_enabled;
            align.max_torque = restore_vector3(saved.max_torque);
            align.attachment0 = None;
            align.attachment1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::AlignOrientationAttachment0(saved.attachment0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::AlignOrientationAttachment1(saved.attachment1_id),
            });
        }
        if let (Some(weld), Some(saved)) = (data.weld_data.as_mut(), snapshot.weld.as_ref()) {
            weld.c0 = restore_cframe(&saved.c0);
            weld.c1 = restore_cframe(&saved.c1);
            weld.enabled = saved.enabled;
            weld.part0 = None;
            weld.part1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldPart0(saved.part0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldPart1(saved.part1_id),
            });
        }
        if let (Some(wc), Some(saved)) = (
            data.weld_constraint_data.as_mut(),
            snapshot.weld_constraint.as_ref(),
        ) {
            wc.enabled = saved.enabled;
            wc.relative_cframe = saved
                .relative_cframe
                .as_ref()
                .map(restore_cframe)
                .or_else(|| {
                    saved.offset.map(|o| {
                        CFrame::from_position(crate::game::lua::types::Vector3::new(
                            o[0], o[1], o[2],
                        ))
                    })
                });
            wc.part0 = None;
            wc.part1 = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldConstraintPart0(saved.part0_id),
            });
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::WeldConstraintPart1(saved.part1_id),
            });
        }
        if let (Some(billboard), Some(saved)) = (
            data.billboard_gui_data.as_mut(),
            snapshot.billboard_gui.as_ref(),
        ) {
            billboard.size = restore_udim2(&saved.size);
            billboard.studs_offset = restore_vector3(saved.studs_offset);
            billboard.always_on_top = saved.always_on_top;
            billboard.enabled = saved.enabled;
            billboard.adornee = None;
            fixups.push(RefFixup {
                instance: instance.clone(),
                kind: RefFixupKind::BillboardAdornee(saved.adornee_id),
            });
        }
        if let (Some(ui_corner), Some(saved)) =
            (data.ui_corner_data.as_mut(), snapshot.ui_corner.as_ref())
        {
            ui_corner.corner_radius = restore_udim(&saved.corner_radius);
        }
        if let (Some(gui), Some(saved)) = (data.gui_data.as_mut(), snapshot.gui.as_ref()) {
            apply_gui_snapshot(gui, saved)?;
        }
    }
    Ok(())
}

fn apply_ref_fixups(map: &HashMap<u64, Instance>, fixups: Vec<RefFixup>) {
    for fixup in fixups {
        let target = match &fixup.kind {
            RefFixupKind::ObjectValue(id)
            | RefFixupKind::ModelPrimaryPart(id)
            | RefFixupKind::VectorForceAttachment0(id)
            | RefFixupKind::VectorForceAttachment1(id)
            | RefFixupKind::AlignPositionAttachment0(id)
            | RefFixupKind::AlignPositionAttachment1(id)
            | RefFixupKind::AlignOrientationAttachment0(id)
            | RefFixupKind::AlignOrientationAttachment1(id)
            | RefFixupKind::WeldPart0(id)
            | RefFixupKind::WeldPart1(id)
            | RefFixupKind::WeldConstraintPart0(id)
            | RefFixupKind::WeldConstraintPart1(id)
            | RefFixupKind::BillboardAdornee(id) => id.and_then(|id| map.get(&id).cloned()),
        };

        let mut data = fixup.instance.data.lock().unwrap();
        match fixup.kind {
            RefFixupKind::ObjectValue(_) => {
                if let Some(object_value) = data.object_value_data.as_mut() {
                    object_value.value =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::ModelPrimaryPart(_) => {
                if let Some(model) = data.model_data.as_mut() {
                    model.primary_part =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::VectorForceAttachment0(_) => {
                if let Some(vector_force) = data.vector_force_data.as_mut() {
                    vector_force.attachment0 =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::VectorForceAttachment1(_) => {
                if let Some(vector_force) = data.vector_force_data.as_mut() {
                    vector_force.attachment1 =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::AlignPositionAttachment0(_) => {
                if let Some(align) = data.align_position_data.as_mut() {
                    align.attachment0 =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::AlignPositionAttachment1(_) => {
                if let Some(align) = data.align_position_data.as_mut() {
                    align.attachment1 =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::AlignOrientationAttachment0(_) => {
                if let Some(align) = data.align_orientation_data.as_mut() {
                    align.attachment0 =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::AlignOrientationAttachment1(_) => {
                if let Some(align) = data.align_orientation_data.as_mut() {
                    align.attachment1 =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldPart0(_) => {
                if let Some(weld) = data.weld_data.as_mut() {
                    weld.part0 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldPart1(_) => {
                if let Some(weld) = data.weld_data.as_mut() {
                    weld.part1 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldConstraintPart0(_) => {
                if let Some(wc) = data.weld_constraint_data.as_mut() {
                    wc.part0 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::WeldConstraintPart1(_) => {
                if let Some(wc) = data.weld_constraint_data.as_mut() {
                    wc.part1 = target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
            RefFixupKind::BillboardAdornee(_) => {
                if let Some(billboard) = data.billboard_gui_data.as_mut() {
                    billboard.adornee =
                        target.map(|instance| std::sync::Arc::downgrade(&instance.data));
                }
            }
        }
    }
}

fn clear_workspace(workspace: &crate::game::lua::services::WorkspaceService) {
    for child in workspace.get_children() {
        child.set_parent(None);
    }
    let mut data = workspace.data.lock().unwrap();
    data.current_camera = None;
}

fn clear_instance_children(root: &Instance) {
    for child in root.get_children() {
        child.set_parent(None);
    }
}

fn create_instance_from_class_name(class_name: &str, name: &str) -> Result<Instance, String> {
    let instance = match class_name {
        "Part" => Instance::from_data(crate::game::lua::instance::InstanceData::new_part(name)),
        "Model" => Instance::from_data(crate::game::lua::instance::InstanceData::new_model(name)),
        "Humanoid" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_humanoid(name))
        }
        "Folder" => Instance::new(ClassName::Folder, name),
        "Script" => Instance::from_data(crate::game::lua::instance::InstanceData::new_script(name)),
        "ModuleScript" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_module_script(name),
        ),
        "Animation" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_animation(name),
        ),
        "Animator" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_animator(name))
        }
        "RemoteEvent" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_remote_event(name),
        ),
        "IntValue" => Instance::from_data(crate::game::lua::instance::InstanceData::new_int_value(
            name,
        )),
        "NumberValue" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_number_value(name),
        ),
        "BoolValue" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_bool_value(name),
        ),
        "StringValue" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_string_value(name),
        ),
        "ObjectValue" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_object_value(name),
        ),
        "CFrameValue" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_cframe_value(name),
        ),
        "Terrain" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_terrain(name))
        }
        "BodyVelocity" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_body_velocity(name),
        ),
        "BodyForce" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_body_force(name),
        ),
        "BodyAngularVelocity" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_body_angular_velocity(name),
        ),
        "Attachment" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_attachment(name),
        ),
        "VectorForce" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_vector_force(name),
        ),
        "AlignPosition" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_align_position(name))
        }
        "AlignOrientation" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_align_orientation(name),
        ),
        "Weld" => Instance::from_data(crate::game::lua::instance::InstanceData::new_weld(name)),
        "WeldConstraint" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_weld_constraint(name))
        }
        "BillboardGui" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_billboard_gui(name),
        ),
        "PlayerGui" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_player_gui(name),
        ),
        "ScreenGui" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_screen_gui(name),
        ),
        "Frame" => Instance::from_data(crate::game::lua::instance::InstanceData::new_frame(name)),
        "TextLabel" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_text_label(name),
        ),
        "TextButton" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_text_button(name),
        ),
        "ImageLabel" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_image_label(name),
        ),
        "ImageButton" => Instance::from_data(
            crate::game::lua::instance::InstanceData::new_image_button(name),
        ),
        "UICorner" => {
            Instance::from_data(crate::game::lua::instance::InstanceData::new_uicorner(name))
        }
        "Sound" => Instance::from_data(crate::game::lua::instance::InstanceData::new_sound(name)),
        other => return Err(format!("unsupported snapshot class '{}'", other)),
    };
    {
        let mut data = instance.data.lock().unwrap();
        data.name = name.to_string();
    }
    Ok(instance)
}

fn apply_part_snapshot(part: &mut PartData, snapshot: &PartSnapshot) -> Result<(), String> {
    part.position = restore_vector3(snapshot.position);
    part.cframe = restore_cframe(&snapshot.cframe);
    part.size = restore_vector3(snapshot.size);
    part.anchored = snapshot.anchored;
    part.can_collide = snapshot.can_collide;
    part.can_query = snapshot.can_query;
    part.collision_group = snapshot.collision_group.clone();
    part.collision_group_id = snapshot.collision_group_id;
    part.can_touch = snapshot.can_touch;
    part.transparency = snapshot.transparency;
    part.color = restore_color3(snapshot.color);
    part.material = restore_material(&snapshot.material)?;
    part.top_surface = snapshot.top_surface.clone();
    part.bottom_surface = snapshot.bottom_surface.clone();
    part.velocity = restore_vector3(snapshot.velocity);
    part.angular_velocity = restore_vector3(snapshot.angular_velocity);
    part.massless = snapshot.massless;
    part.enable_fluid_forces = snapshot.enable_fluid_forces;
    part.custom_physical_properties = snapshot
        .custom_physical_properties
        .as_ref()
        .map(restore_physical_properties);
    part.shape = restore_part_type(&snapshot.shape)?;
    part.mesh_id = snapshot.mesh_id.clone();
    part.position_dirty = false;
    part.size_dirty = false;
    part.anchored_dirty = false;
    part.can_collide_dirty = false;
    part.velocity_dirty = false;
    part.angular_velocity_dirty = false;
    part.shape_dirty = false;
    part.custom_physical_properties_dirty = false;
    Ok(())
}

fn apply_humanoid_snapshot(
    humanoid: &mut HumanoidData,
    snapshot: &HumanoidSnapshot,
) -> Result<(), String> {
    humanoid.health = snapshot.health;
    humanoid.max_health = snapshot.max_health;
    humanoid.walk_speed = snapshot.walk_speed;
    humanoid.jump_power = snapshot.jump_power;
    humanoid.jump_height = snapshot.jump_height;
    humanoid.auto_rotate = snapshot.auto_rotate;
    humanoid.health_display_distance = snapshot.health_display_distance;
    humanoid.name_display_distance = snapshot.name_display_distance;
    humanoid.hip_height = snapshot.hip_height;
    humanoid.move_direction = restore_vector3(snapshot.move_direction);
    humanoid.move_input_direction = restore_vector3(snapshot.move_input_direction);
    humanoid.running_speed = snapshot.running_speed;
    humanoid.state = restore_humanoid_state(&snapshot.state)?;
    humanoid.jump_requested = snapshot.jump_requested;
    humanoid.move_to_target = snapshot.move_to_target.map(restore_vector3);
    humanoid.cancel_move_to = snapshot.cancel_move_to;
    Ok(())
}

fn apply_gui_snapshot(gui: &mut GuiObjectData, snapshot: &GuiSnapshot) -> Result<(), String> {
    gui.position = restore_udim2(&snapshot.position);
    gui.size = restore_udim2(&snapshot.size);
    gui.anchor_point = (snapshot.anchor_point[0], snapshot.anchor_point[1]);
    gui.rotation = snapshot.rotation;
    gui.z_index = snapshot.z_index;
    gui.layout_order = snapshot.layout_order;
    gui.visible = snapshot.visible;
    gui.background_color = restore_color3(snapshot.background_color);
    gui.background_transparency = snapshot.background_transparency;
    gui.border_color = restore_color3(snapshot.border_color);
    gui.border_size_pixel = snapshot.border_size_pixel;
    gui.text = snapshot.text.clone();
    gui.text_color = snapshot.text_color.map(restore_color3);
    gui.text_size = snapshot.text_size;
    gui.text_transparency = snapshot.text_transparency;
    gui.text_stroke_transparency = snapshot.text_stroke_transparency;
    gui.font = snapshot.font.clone();
    gui.text_scaled = snapshot.text_scaled;
    gui.text_x_alignment = restore_text_x_alignment(&snapshot.text_x_alignment)?;
    gui.text_y_alignment = restore_text_y_alignment(&snapshot.text_y_alignment)?;
    gui.image = snapshot.image.clone();
    gui.image_color = snapshot.image_color.map(restore_color3);
    gui.image_transparency = snapshot.image_transparency;
    gui.display_order = snapshot.display_order;
    gui.ignore_gui_inset = snapshot.ignore_gui_inset;
    gui.enabled = snapshot.enabled;
    gui.reset_on_spawn = snapshot.reset_on_spawn;
    Ok(())
}

fn apply_attributes(instance: &Instance, attributes: &HashMap<String, SnapshotAttributeValue>) {
    let mut data = instance.data.lock().unwrap();
    data.attributes = attributes
        .iter()
        .map(|(key, value)| (key.clone(), restore_attribute_value(value)))
        .collect();
}

fn snapshot_part_data(part: &PartData) -> PartSnapshot {
    PartSnapshot {
        position: snapshot_vector3(part.position),
        cframe: snapshot_cframe(part.cframe),
        size: snapshot_vector3(part.size),
        anchored: part.anchored,
        can_collide: part.can_collide,
        can_query: part.can_query,
        collision_group: part.collision_group.clone(),
        collision_group_id: part.collision_group_id,
        can_touch: part.can_touch,
        transparency: part.transparency,
        color: snapshot_color3(part.color),
        material: snapshot_material(part.material),
        top_surface: part.top_surface.clone(),
        bottom_surface: part.bottom_surface.clone(),
        velocity: snapshot_vector3(part.velocity),
        angular_velocity: snapshot_vector3(part.angular_velocity),
        massless: part.massless,
        enable_fluid_forces: part.enable_fluid_forces,
        custom_physical_properties: part
            .custom_physical_properties
            .map(snapshot_physical_properties),
        shape: snapshot_part_type(part.shape),
        mesh_id: part.mesh_id.clone(),
    }
}

fn snapshot_model_data(model: &ModelData) -> ModelSnapshot {
    ModelSnapshot {
        primary_part_id: model
            .primary_part
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
    }
}

fn snapshot_terrain_data(terrain: &TerrainData) -> TerrainSnapshot {
    TerrainSnapshot {
        voxels: terrain
            .voxels
            .iter()
            .map(|(cell, voxel)| TerrainVoxelSnapshot {
                cell: [cell.x, cell.y, cell.z],
                solid_material: snapshot_material(voxel.solid_material),
                solid_occupancy: voxel.solid_occupancy,
                liquid_occupancy: voxel.liquid_occupancy,
            })
            .collect(),
    }
}

fn restore_terrain_data(snapshot: &TerrainSnapshot) -> Result<TerrainData, String> {
    let mut terrain = TerrainData::default();
    for saved in &snapshot.voxels {
        terrain.voxels.insert(
            TerrainCell {
                x: saved.cell[0],
                y: saved.cell[1],
                z: saved.cell[2],
            },
            TerrainVoxel {
                solid_material: restore_material(&saved.solid_material)?,
                solid_occupancy: saved.solid_occupancy.clamp(0.0, 1.0),
                liquid_occupancy: saved.liquid_occupancy.clamp(0.0, 1.0),
            },
        );
    }
    Ok(terrain)
}

fn snapshot_humanoid_data(humanoid: &HumanoidData) -> HumanoidSnapshot {
    HumanoidSnapshot {
        health: humanoid.health,
        max_health: humanoid.max_health,
        walk_speed: humanoid.walk_speed,
        jump_power: humanoid.jump_power,
        jump_height: humanoid.jump_height,
        auto_rotate: humanoid.auto_rotate,
        health_display_distance: humanoid.health_display_distance,
        name_display_distance: humanoid.name_display_distance,
        hip_height: humanoid.hip_height,
        move_direction: snapshot_vector3(humanoid.move_direction),
        move_input_direction: snapshot_vector3(humanoid.move_input_direction),
        running_speed: humanoid.running_speed,
        state: snapshot_humanoid_state(humanoid.state),
        jump_requested: humanoid.jump_requested,
        move_to_target: humanoid.move_to_target.map(snapshot_vector3),
        cancel_move_to: humanoid.cancel_move_to,
    }
}

fn snapshot_script_data(script: &ScriptData) -> ScriptSnapshot {
    ScriptSnapshot {
        source: script.source.clone(),
        disabled: script.disabled,
    }
}

fn snapshot_animation_data(animation: &AnimationData) -> AnimationSnapshot {
    AnimationSnapshot {
        animation_id: animation.animation_id.clone(),
    }
}

fn snapshot_animator_data(animator: &AnimatorData) -> AnimatorSnapshot {
    AnimatorSnapshot {
        loaded_tracks: animator.loaded_tracks,
    }
}

fn snapshot_sound_data(sound: &SoundData) -> SoundSnapshot {
    SoundSnapshot {
        stream_id: sound.stream_id.clone(),
        volume: sound.volume,
        playback_speed: sound.playback_speed,
        is_playing: sound.is_playing,
    }
}

fn snapshot_object_value_data(value: &ObjectValueData) -> ObjectRefSnapshot {
    ObjectRefSnapshot {
        target_id: value
            .value
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
    }
}

fn snapshot_cframe_value_data(value: &CFrameValueData) -> SnapshotCFrame {
    snapshot_cframe(value.value)
}

fn snapshot_body_velocity_data(value: &BodyVelocityData) -> BodyVelocitySnapshot {
    BodyVelocitySnapshot {
        velocity: snapshot_vector3(value.velocity),
        max_force: snapshot_vector3(value.max_force),
        p: value.p,
    }
}

fn snapshot_body_force_data(value: &BodyForceData) -> BodyForceSnapshot {
    BodyForceSnapshot {
        force: snapshot_vector3(value.force),
    }
}

fn snapshot_body_angular_velocity_data(
    value: &BodyAngularVelocityData,
) -> BodyAngularVelocitySnapshot {
    BodyAngularVelocitySnapshot {
        angular_velocity: snapshot_vector3(value.angular_velocity),
        max_torque: snapshot_vector3(value.max_torque),
        p: value.p,
    }
}

fn snapshot_attachment_data(data: &AttachmentData) -> AttachmentSnapshot {
    AttachmentSnapshot {
        cframe: snapshot_cframe(data.cframe),
    }
}

fn snapshot_vector_force_data(data: &VectorForceData) -> VectorForceSnapshot {
    VectorForceSnapshot {
        attachment0_id: data
            .attachment0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        attachment1_id: data
            .attachment1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        force: snapshot_vector3(data.force),
        relative_to: snapshot_actuator_relative_to(data.relative_to),
        apply_at_center_of_mass: data.apply_at_center_of_mass,
        enabled: data.enabled,
    }
}

fn snapshot_align_position_data(data: &AlignPositionData) -> AlignPositionSnapshot {
    AlignPositionSnapshot {
        attachment0_id: data
            .attachment0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        attachment1_id: data
            .attachment1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        position: snapshot_vector3(data.position),
        responsiveness: data.responsiveness,
        rigidity_enabled: data.rigidity_enabled,
        max_force: data.max_force,
        max_velocity: data.max_velocity,
    }
}

fn snapshot_align_orientation_data(data: &AlignOrientationData) -> AlignOrientationSnapshot {
    AlignOrientationSnapshot {
        attachment0_id: data
            .attachment0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        attachment1_id: data
            .attachment1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        responsiveness: data.responsiveness,
        rigidity_enabled: data.rigidity_enabled,
        max_torque: snapshot_vector3(data.max_torque),
    }
}

fn snapshot_weld_data(weld: &WeldData) -> WeldSnapshot {
    WeldSnapshot {
        part0_id: weld
            .part0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        part1_id: weld
            .part1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        c0: snapshot_cframe(weld.c0),
        c1: snapshot_cframe(weld.c1),
        enabled: weld.enabled,
    }
}

fn snapshot_weld_constraint_data(wc: &WeldConstraintData) -> WeldConstraintSnapshot {
    WeldConstraintSnapshot {
        part0_id: wc
            .part0
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        part1_id: wc
            .part1
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
        enabled: wc.enabled,
        offset: wc
            .relative_cframe
            .map(|cf| [cf.position.x, cf.position.y, cf.position.z]),
        relative_cframe: wc.relative_cframe.map(snapshot_cframe),
    }
}

fn snapshot_billboard_gui_data(data: &BillboardGuiData) -> BillboardGuiSnapshot {
    BillboardGuiSnapshot {
        size: snapshot_udim2(data.size),
        studs_offset: snapshot_vector3(data.studs_offset),
        always_on_top: data.always_on_top,
        enabled: data.enabled,
        adornee_id: data
            .adornee
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().id.0),
    }
}

fn snapshot_ui_corner_data(data: &UICornerData) -> UICornerSnapshot {
    UICornerSnapshot {
        corner_radius: snapshot_udim(data.corner_radius),
    }
}

fn snapshot_gui_data(data: &GuiObjectData) -> GuiSnapshot {
    GuiSnapshot {
        position: snapshot_udim2(data.position),
        size: snapshot_udim2(data.size),
        anchor_point: [data.anchor_point.0, data.anchor_point.1],
        rotation: data.rotation,
        z_index: data.z_index,
        layout_order: data.layout_order,
        visible: data.visible,
        background_color: snapshot_color3(data.background_color),
        background_transparency: data.background_transparency,
        border_color: snapshot_color3(data.border_color),
        border_size_pixel: data.border_size_pixel,
        text: data.text.clone(),
        text_color: data.text_color.map(snapshot_color3),
        text_size: data.text_size,
        text_transparency: data.text_transparency,
        text_stroke_transparency: data.text_stroke_transparency,
        font: data.font.clone(),
        text_scaled: data.text_scaled,
        text_x_alignment: snapshot_text_x_alignment(data.text_x_alignment),
        text_y_alignment: snapshot_text_y_alignment(data.text_y_alignment),
        image: data.image.clone(),
        image_color: data.image_color.map(snapshot_color3),
        image_transparency: data.image_transparency,
        display_order: data.display_order,
        ignore_gui_inset: data.ignore_gui_inset,
        enabled: data.enabled,
        reset_on_spawn: data.reset_on_spawn,
    }
}

fn snapshot_attribute_value(value: &AttributeValue) -> SnapshotAttributeValue {
    match value {
        AttributeValue::String(v) => SnapshotAttributeValue::String(v.clone()),
        AttributeValue::Number(v) => SnapshotAttributeValue::Number(*v),
        AttributeValue::Bool(v) => SnapshotAttributeValue::Bool(*v),
        AttributeValue::Vector3(v) => SnapshotAttributeValue::Vector3(snapshot_vector3(*v)),
        AttributeValue::Color3(v) => SnapshotAttributeValue::Color3(snapshot_color3(*v)),
        AttributeValue::Table(items) => {
            SnapshotAttributeValue::Table(items.iter().map(snapshot_attribute_value).collect())
        }
        AttributeValue::Nil => SnapshotAttributeValue::Nil,
    }
}

fn restore_attribute_value(value: &SnapshotAttributeValue) -> AttributeValue {
    match value {
        SnapshotAttributeValue::String(v) => AttributeValue::String(v.clone()),
        SnapshotAttributeValue::Number(v) => AttributeValue::Number(*v),
        SnapshotAttributeValue::Bool(v) => AttributeValue::Bool(*v),
        SnapshotAttributeValue::Vector3(v) => AttributeValue::Vector3(restore_vector3(*v)),
        SnapshotAttributeValue::Color3(v) => AttributeValue::Color3(restore_color3(*v)),
        SnapshotAttributeValue::Table(items) => {
            AttributeValue::Table(items.iter().map(restore_attribute_value).collect())
        }
        SnapshotAttributeValue::Nil => AttributeValue::Nil,
    }
}

fn snapshot_vector3(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn restore_vector3(v: [f32; 3]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

fn snapshot_color3(v: Color3) -> [f32; 3] {
    [v.r, v.g, v.b]
}

fn restore_color3(v: [f32; 3]) -> Color3 {
    Color3::new(v[0], v[1], v[2])
}

fn snapshot_cframe(v: CFrame) -> SnapshotCFrame {
    SnapshotCFrame {
        position: snapshot_vector3(v.position),
        rotation: v.rotation,
    }
}

fn restore_cframe(v: &SnapshotCFrame) -> CFrame {
    CFrame {
        position: restore_vector3(v.position),
        rotation: v.rotation,
    }
}

fn snapshot_udim(v: UDim) -> SnapshotUDim {
    SnapshotUDim {
        scale: v.scale,
        offset: v.offset,
    }
}

fn restore_udim(v: &SnapshotUDim) -> UDim {
    UDim::new(v.scale, v.offset)
}

fn snapshot_udim2(v: UDim2) -> SnapshotUDim2 {
    SnapshotUDim2 {
        x: snapshot_udim(v.x),
        y: snapshot_udim(v.y),
    }
}

fn restore_udim2(v: &SnapshotUDim2) -> UDim2 {
    UDim2::from_udims(restore_udim(&v.x), restore_udim(&v.y))
}

fn snapshot_physical_properties(v: PhysicalProperties) -> PhysicalPropertiesSnapshot {
    PhysicalPropertiesSnapshot {
        density: v.density,
        friction: v.friction,
        elasticity: v.elasticity,
        friction_weight: v.friction_weight,
        elasticity_weight: v.elasticity_weight,
    }
}

fn restore_physical_properties(v: &PhysicalPropertiesSnapshot) -> PhysicalProperties {
    PhysicalProperties::new(
        v.density,
        v.friction,
        v.elasticity,
        v.friction_weight,
        v.elasticity_weight,
    )
}

fn snapshot_part_type(v: PartType) -> String {
    v.name().to_string()
}

fn restore_part_type(v: &str) -> Result<PartType, String> {
    match v {
        "Ball" => Ok(PartType::Ball),
        "Block" => Ok(PartType::Block),
        "Cylinder" => Ok(PartType::Cylinder),
        "Ring" => Ok(PartType::Ring),
        "Wedge" => Ok(PartType::Wedge),
        other => Err(format!("unsupported PartType '{}'", other)),
    }
}

fn snapshot_fluid_forces(v: FluidForces) -> String {
    v.name().to_string()
}

fn restore_fluid_forces(v: &str) -> Result<FluidForces, String> {
    match v {
        "Default" => Ok(FluidForces::Default),
        "Experimental" => Ok(FluidForces::Experimental),
        other => Err(format!("unsupported FluidForces '{}'", other)),
    }
}

fn snapshot_material(v: Material) -> String {
    v.name().to_string()
}

fn restore_material(v: &str) -> Result<Material, String> {
    match v {
        "Air" => Ok(Material::Air),
        "Plastic" => Ok(Material::Plastic),
        "Wood" => Ok(Material::Wood),
        "Metal" => Ok(Material::Metal),
        "Glass" => Ok(Material::Glass),
        "Neon" => Ok(Material::Neon),
        "Concrete" => Ok(Material::Concrete),
        "Brick" => Ok(Material::Brick),
        "Granite" => Ok(Material::Granite),
        "Grass" => Ok(Material::Grass),
        "Ice" => Ok(Material::Ice),
        "Sand" => Ok(Material::Sand),
        "Fabric" => Ok(Material::Fabric),
        "Marble" => Ok(Material::Marble),
        "Slate" => Ok(Material::Slate),
        "SmoothPlastic" => Ok(Material::SmoothPlastic),
        "ForceField" => Ok(Material::ForceField),
        "Water" => Ok(Material::Water),
        other => Err(format!("unsupported Material '{}'", other)),
    }
}

fn snapshot_actuator_relative_to(v: ActuatorRelativeTo) -> String {
    v.name().to_string()
}

fn restore_actuator_relative_to(v: &str) -> Result<ActuatorRelativeTo, String> {
    match v {
        "Attachment0" => Ok(ActuatorRelativeTo::Attachment0),
        "Attachment1" => Ok(ActuatorRelativeTo::Attachment1),
        "World" => Ok(ActuatorRelativeTo::World),
        other => Err(format!("unsupported ActuatorRelativeTo '{}'", other)),
    }
}

fn snapshot_humanoid_state(v: HumanoidStateType) -> String {
    v.name().to_string()
}

fn restore_humanoid_state(v: &str) -> Result<HumanoidStateType, String> {
    match v {
        "Running" => Ok(HumanoidStateType::Running),
        "Landed" => Ok(HumanoidStateType::Landed),
        "Jumping" => Ok(HumanoidStateType::Jumping),
        "Freefall" => Ok(HumanoidStateType::Freefall),
        "Dead" => Ok(HumanoidStateType::Dead),
        "Physics" => Ok(HumanoidStateType::Physics),
        "None" => Ok(HumanoidStateType::None),
        other => Err(format!("unsupported HumanoidStateType '{}'", other)),
    }
}

fn snapshot_text_x_alignment(v: TextXAlignment) -> String {
    match v {
        TextXAlignment::Left => "Left",
        TextXAlignment::Center => "Center",
        TextXAlignment::Right => "Right",
    }
    .to_string()
}

fn restore_text_x_alignment(v: &str) -> Result<TextXAlignment, String> {
    match v {
        "Left" => Ok(TextXAlignment::Left),
        "Center" => Ok(TextXAlignment::Center),
        "Right" => Ok(TextXAlignment::Right),
        other => Err(format!("unsupported TextXAlignment '{}'", other)),
    }
}

fn snapshot_text_y_alignment(v: TextYAlignment) -> String {
    match v {
        TextYAlignment::Top => "Top",
        TextYAlignment::Center => "Center",
        TextYAlignment::Bottom => "Bottom",
    }
    .to_string()
}

fn restore_text_y_alignment(v: &str) -> Result<TextYAlignment, String> {
    match v {
        "Top" => Ok(TextYAlignment::Top),
        "Center" => Ok(TextYAlignment::Center),
        "Bottom" => Ok(TextYAlignment::Bottom),
        other => Err(format!("unsupported TextYAlignment '{}'", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::instance::GameInstance;
    use crate::game::lua::instance::InstanceData;
    use mlua::ObjectLike;
    use std::collections::HashSet;

    fn assert_rotation_close(actual: [[f32; 3]; 3], expected: [[f32; 3]; 3], context: &str) {
        for row in 0..3 {
            for col in 0..3 {
                assert!(
                    (actual[row][col] - expected[row][col]).abs() < 1e-4,
                    "{context}: rotation[{row}][{col}] expected {} got {}",
                    expected[row][col],
                    actual[row][col]
                );
            }
        }
    }

    #[test]
    fn roundtrip_resumes_lua_closure_capturing_instance_and_value_userdata() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            _G.done = nil
            local part = Instance.new("Part")
            part.Name = "PersistTarget"
            part.Anchored = true
            part.Parent = workspace
            local position = Vector3.new(7, 8, 9)
            local color = Color3.new(0.25, 0.5, 0.75)
            local size = UDim2.new(0.5, 12, 0.25, 34)
            local debris = game:GetService("Debris")

            task.delay(0.05, function()
                part.Name = "RestoredByClosure"
                part.Position = position
                part.Color = color
                debris:AddItem(part, 10)
                _G.done = true
                _G.positionX = position.X
                _G.colorG = color.G
                _G.sizeOffset = size.X.Offset + size.Y.Offset
                _G.debrisType = typeof(debris)
            end)
            "#,
        );
        instance.tick();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert_eq!(globals.get::<Option<bool>>("done").unwrap(), None);

        for _ in 0..4 {
            restored.tick();
        }

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert!(globals.get::<bool>("done").unwrap());
        assert_eq!(globals.get::<f32>("positionX").unwrap(), 7.0);
        assert_eq!(globals.get::<f32>("colorG").unwrap(), 0.5);
        assert_eq!(globals.get::<i32>("sizeOffset").unwrap(), 46);
        assert_eq!(globals.get::<String>("debrisType").unwrap(), "userdata");

        let restored_part = restored_runtime
            .workspace()
            .instance
            .find_first_child("RestoredByClosure", true)
            .unwrap();
        let data = restored_part.data.lock().unwrap();
        let part_data = data.part_data.as_ref().unwrap();
        assert_eq!(snapshot_vector3(part_data.position), [7.0, 8.0, 9.0]);
        assert_eq!(snapshot_color3(part_data.color), [0.25, 0.5, 0.75]);
    }

    #[test]
    fn roundtrip_restores_instance_signal_connections() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local folder = Instance.new("Folder")
            folder.Name = "SignalParent"
            folder.Parent = workspace

            local count = 20
            folder.ChildAdded:Connect(function(child)
                count += 1
                _G.childSignalCount = count
            end)

            task.delay(0.05, function()
                local child = Instance.new("Part")
                child.Name = "AfterRestoreChild"
                child.Anchored = true
                child.Parent = folder
            end)
            "#,
        );
        instance.tick();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        for _ in 0..4 {
            restored.tick();
        }

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert_eq!(globals.get::<i64>("childSignalCount").unwrap(), 21);

        let parent = restored_runtime
            .workspace()
            .instance
            .find_first_child("SignalParent", true)
            .unwrap();
        assert!(parent
            .find_first_child("AfterRestoreChild", false)
            .is_some());
    }

    #[test]
    fn roundtrip_restores_once_signal_connections() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local folder = Instance.new("Folder")
            folder.Name = "OnceSignalParent"
            folder.Parent = workspace

            _G.onceSignalCount = 0
            folder.ChildAdded:Once(function(child)
                _G.onceSignalCount += 1
                _G.onceSignalLastChild = child.Name
            end)

            task.delay(0.05, function()
                local first = Instance.new("Folder")
                first.Name = "AfterRestoreFirstChild"
                first.Parent = folder

                local second = Instance.new("Folder")
                second.Name = "AfterRestoreSecondChild"
                second.Parent = folder
            end)
            "#,
        );
        instance.tick();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        for _ in 0..4 {
            restored.tick();
        }

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert_eq!(
            globals.get::<i64>("onceSignalCount").unwrap(),
            1,
            "restored :Once connection should fire exactly once"
        );
        assert_eq!(
            globals.get::<String>("onceSignalLastChild").unwrap(),
            "AfterRestoreFirstChild"
        );

        let parent = restored_runtime
            .workspace()
            .instance
            .find_first_child("OnceSignalParent", true)
            .unwrap();
        assert!(parent
            .find_first_child("AfterRestoreFirstChild", false)
            .is_some());
        assert!(parent
            .find_first_child("AfterRestoreSecondChild", false)
            .is_some());
    }

    #[test]
    fn roundtrip_restores_animation_track_scheduler_and_signals() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local humanoid = Instance.new("Humanoid")
            local animation = Instance.new("Animation")
            animation.AnimationId = "local://fire_rifle"
            local track = humanoid:LoadAnimation(animation)
            _G.track = track
            _G.ended = 0
            _G.trackId = track.TrackId
            track.Ended:Connect(function()
                _G.ended += 1
            end)
            track:Play(0, 1, 1)
            "#,
        );
        instance.tick();

        let runtime = instance.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        let before_track: mlua::AnyUserData = globals.get("track").unwrap();
        let before_position: f32 = before_track.get("TimePosition").unwrap();
        assert!(
            before_position > 0.0,
            "track should have advanced before snapshot"
        );
        assert!(before_track.get::<bool>("IsPlaying").unwrap());
        assert_eq!(globals.get::<i64>("ended").unwrap(), 0);

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        let restored_track: mlua::AnyUserData = globals.get("track").unwrap();
        assert_eq!(
            restored_track.get::<u64>("TrackId").unwrap(),
            globals.get::<u64>("trackId").unwrap()
        );
        assert!(restored_track.get::<bool>("IsPlaying").unwrap());
        let restored_position: f32 = restored_track.get("TimePosition").unwrap();
        assert!(
            restored_position >= before_position,
            "track time should be preserved across restore"
        );

        for _ in 0..20 {
            restored.tick();
        }

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        let restored_track: mlua::AnyUserData = globals.get("track").unwrap();
        assert!(!restored_track.get::<bool>("IsPlaying").unwrap());
        assert_eq!(globals.get::<i64>("ended").unwrap(), 1);
    }

    #[test]
    fn roundtrip_restores_workspace_server_storage_and_players() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();

        let workspace = instance.lua_runtime.as_ref().unwrap().workspace();
        let server_storage = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .game()
            .server_storage();

        let part = Instance::from_data(InstanceData::new_part("TrackedPart"));
        {
            let mut data = part.data.lock().unwrap();
            let part_data = data.part_data.as_mut().unwrap();
            part_data.position = Vector3::new(12.0, 3.0, -4.0);
            part_data.cframe = CFrame::new(12.0, 3.0, -4.0);
            part_data.color = Color3::from_rgb(10, 20, 30);
            data.attributes.insert(
                "Marker".to_string(),
                AttributeValue::String("alpha".to_string()),
            );
        }
        workspace.add_child(part.clone());
        {
            let runtime = instance.lua_runtime.as_ref().unwrap();
            runtime
                .lua()
                .globals()
                .set("__tracked_part", part.clone())
                .unwrap();
            runtime
                .lua()
                .load(
                    r#"
                    local PhysicsService = game:GetService("PhysicsService")
                    PhysicsService:RegisterCollisionGroup("Players")
                    PhysicsService:RegisterCollisionGroup("Ground")
                    PhysicsService:CollisionGroupSetCollidable("Players", "Ground", false)
                    PhysicsService:SetPartCollisionGroup(__tracked_part, "Players")
                    PhysicsService:RenameCollisionGroup("Players", "Characters")
                    "#,
                )
                .exec()
                .unwrap();
        }

        let target = Instance::from_data(InstanceData::new_part("ConstraintTarget"));
        {
            let mut data = target.data.lock().unwrap();
            let part_data = data.part_data.as_mut().unwrap();
            part_data.position = Vector3::new(20.0, 3.0, -4.0);
            part_data.cframe =
                CFrame::new(20.0, 3.0, -4.0).multiply(&CFrame::from_orientation(0.0, 1.57, 0.0));
            part_data.anchored = true;
        }
        workspace.add_child(target.clone());

        let attachment0 = Instance::from_data(InstanceData::new_attachment("TrackedAttachment"));
        {
            let mut data = attachment0.data.lock().unwrap();
            data.attachment_data.as_mut().unwrap().cframe =
                CFrame::from_position(Vector3::new(1.0, 0.0, 0.0));
        }
        attachment0.set_parent(Some(&part));

        let attachment1 = Instance::from_data(InstanceData::new_attachment("TargetAttachment"));
        attachment1.set_parent(Some(&target));

        let align_position =
            Instance::from_data(InstanceData::new_align_position("TrackedAlignPosition"));
        {
            let mut data = align_position.data.lock().unwrap();
            let align = data.align_position_data.as_mut().unwrap();
            align.attachment0 = Some(std::sync::Arc::downgrade(&attachment0.data));
            align.attachment1 = Some(std::sync::Arc::downgrade(&attachment1.data));
            align.rigidity_enabled = true;
        }
        workspace.add_child(align_position.clone());

        let align_orientation = Instance::from_data(InstanceData::new_align_orientation(
            "TrackedAlignOrientation",
        ));
        {
            let mut data = align_orientation.data.lock().unwrap();
            let align = data.align_orientation_data.as_mut().unwrap();
            align.attachment0 = Some(std::sync::Arc::downgrade(&attachment0.data));
            align.attachment1 = Some(std::sync::Arc::downgrade(&attachment1.data));
            align.rigidity_enabled = true;
        }
        workspace.add_child(align_orientation.clone());

        let folder = Instance::new(ClassName::Folder, "HiddenState");
        folder.set_parent(Some(&server_storage));
        let object_value = Instance::from_data(InstanceData::new_object_value("TrackedRef"));
        {
            let mut data = object_value.data.lock().unwrap();
            data.object_value_data.as_mut().unwrap().value =
                Some(std::sync::Arc::downgrade(&part.data));
        }
        object_value.set_parent(Some(&folder));
        let cframe_value = Instance::from_data(InstanceData::new_cframe_value("TrackedCFrame"));
        let expected_cframe =
            CFrame::new(1.0, 2.0, 3.0).multiply(&CFrame::from_orientation(0.2, 0.4, 0.1));
        {
            let mut data = cframe_value.data.lock().unwrap();
            data.cframe_value_data.as_mut().unwrap().value = expected_cframe;
        }
        cframe_value.set_parent(Some(&folder));
        folder.set_attribute("InternalCounter", AttributeValue::Number(42.0));

        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "Tester"));
        let user_id = *instance.players.get(&agent_id).unwrap();
        let player = instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .players()
            .get_player_by_user_id(user_id)
            .unwrap();
        player.set_attribute("PlayerMemo", AttributeValue::String("keep".to_string()));
        let character = player
            .data
            .lock()
            .unwrap()
            .player_data
            .as_ref()
            .and_then(|pdata| pdata.character.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(Instance::from_ref)
            .unwrap();
        let hrp = character
            .find_first_child("HumanoidRootPart", false)
            .unwrap();
        let expected_hrp_cframe =
            CFrame::new(5.0, 6.5, 7.5).multiply(&CFrame::from_orientation(0.0, 1.25, 0.0));
        {
            let mut data = hrp.data.lock().unwrap();
            let part_data = data.part_data.as_mut().unwrap();
            part_data.position = expected_hrp_cframe.position;
            part_data.cframe = expected_hrp_cframe;
            part_data.velocity = Vector3::new(1.0, 2.0, 3.0);
        }

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let restored_part = restored_runtime
            .workspace()
            .instance
            .find_first_child("TrackedPart", true)
            .unwrap();
        assert!(matches!(
            restored_part.get_attribute("Marker"),
            Some(AttributeValue::String(value)) if value == "alpha"
        ));
        let restored_part_data = restored_part.data.lock().unwrap();
        let restored_part_state = restored_part_data.part_data.as_ref().unwrap();
        let restored_part_pos = restored_part_state.position;
        let restored_part_collision_group_id = restored_part_state.collision_group_id;
        assert_eq!(snapshot_vector3(restored_part_pos), [12.0, 3.0, -4.0]);
        assert_eq!(restored_part_state.collision_group, "Players");
        assert_eq!(
            Some(restored_part_collision_group_id),
            restored_runtime
                .game()
                .physics_service()
                .resolve_collision_group_id("Characters")
                .ok()
        );
        drop(restored_part_data);
        let restored_physics_service = restored_runtime.game().physics_service();
        assert!(restored_physics_service
            .resolve_collision_group_id("Players")
            .is_err());
        assert_eq!(
            restored_physics_service
                .resolve_collision_group_id("Characters")
                .unwrap(),
            restored_part_collision_group_id
        );
        assert!(!restored_physics_service.groups_collidable("Characters", "Ground"));

        let restored_attachment = restored_runtime
            .workspace()
            .instance
            .find_first_child("TrackedAttachment", true)
            .unwrap();
        let restored_attachment_data = restored_attachment.data.lock().unwrap();
        let restored_attachment_cframe = restored_attachment_data
            .attachment_data
            .as_ref()
            .map(|attachment| attachment.cframe)
            .unwrap();
        assert_eq!(
            snapshot_cframe(restored_attachment_cframe).position,
            [1.0, 0.0, 0.0]
        );
        drop(restored_attachment_data);

        let restored_align_position = restored_runtime
            .workspace()
            .instance
            .find_first_child("TrackedAlignPosition", true)
            .unwrap();
        let restored_align_position_data = restored_align_position.data.lock().unwrap();
        let restored_align_position_state = restored_align_position_data
            .align_position_data
            .as_ref()
            .unwrap();
        assert!(restored_align_position_state
            .attachment0
            .as_ref()
            .and_then(|w| w.upgrade())
            .is_some());
        assert!(restored_align_position_state
            .attachment1
            .as_ref()
            .and_then(|w| w.upgrade())
            .is_some());
        assert!(restored_align_position_state.rigidity_enabled);
        drop(restored_align_position_data);

        let restored_align_orientation = restored_runtime
            .workspace()
            .instance
            .find_first_child("TrackedAlignOrientation", true)
            .unwrap();
        let restored_align_orientation_data = restored_align_orientation.data.lock().unwrap();
        let restored_align_orientation_state = restored_align_orientation_data
            .align_orientation_data
            .as_ref()
            .unwrap();
        assert!(restored_align_orientation_state
            .attachment0
            .as_ref()
            .and_then(|w| w.upgrade())
            .is_some());
        assert!(restored_align_orientation_state
            .attachment1
            .as_ref()
            .and_then(|w| w.upgrade())
            .is_some());
        assert!(restored_align_orientation_state.rigidity_enabled);
        drop(restored_align_orientation_data);

        let restored_folder = restored_runtime
            .game()
            .server_storage()
            .find_first_child("HiddenState", true)
            .unwrap();
        assert!(matches!(
            restored_folder.get_attribute("InternalCounter"),
            Some(AttributeValue::Number(value)) if (value - 42.0).abs() < f64::EPSILON
        ));
        let restored_ref = restored_folder
            .find_first_child("TrackedRef", false)
            .unwrap();
        let target_name = restored_ref
            .data
            .lock()
            .unwrap()
            .object_value_data
            .as_ref()
            .and_then(|value| value.value.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(|instance| instance.lock().unwrap().name.clone());
        assert_eq!(target_name.as_deref(), Some("TrackedPart"));
        let restored_cframe = restored_folder
            .find_first_child("TrackedCFrame", false)
            .unwrap();
        let restored_value = restored_cframe
            .data
            .lock()
            .unwrap()
            .cframe_value_data
            .as_ref()
            .map(|value| value.value)
            .unwrap();
        assert_eq!(snapshot_cframe(restored_value).position, [1.0, 2.0, 3.0]);
        assert_eq!(
            snapshot_cframe(restored_value).rotation,
            snapshot_cframe(expected_cframe).rotation
        );

        let restored_user_id = *restored.players.get(&agent_id).unwrap();
        let restored_player = restored_runtime
            .players()
            .get_player_by_user_id(restored_user_id)
            .unwrap();
        assert!(matches!(
            restored_player.get_attribute("PlayerMemo"),
            Some(AttributeValue::String(value)) if value == "keep"
        ));
        let restored_character = restored_player
            .data
            .lock()
            .unwrap()
            .player_data
            .as_ref()
            .and_then(|pdata| pdata.character.as_ref())
            .and_then(|weak| weak.upgrade())
            .map(Instance::from_ref)
            .unwrap();
        let restored_hrp = restored_character
            .find_first_child("HumanoidRootPart", false)
            .unwrap();
        let restored_hrp_data = restored_hrp.data.lock().unwrap();
        let restored_hrp_part = restored_hrp_data.part_data.as_ref().unwrap();
        assert_eq!(
            snapshot_vector3(restored_hrp_part.position),
            [5.0, 6.5, 7.5]
        );
        assert_eq!(
            snapshot_vector3(restored_hrp_part.velocity),
            [1.0, 2.0, 3.0]
        );
        assert_rotation_close(
            restored_hrp_part.cframe.rotation,
            expected_hrp_cframe.rotation,
            "restored HRP Lua CFrame",
        );
        drop(restored_hrp_data);
        let restored_hrp_id = *restored.player_hrp_ids.get(&agent_id).unwrap();
        let restored_physics_rotation = restored
            .physics
            .get_character_state(restored_hrp_id)
            .and_then(|state| restored.physics.get_rotation_matrix(state.body_handle))
            .unwrap();
        assert_rotation_close(
            restored_physics_rotation,
            expected_hrp_cframe.rotation,
            "restored HRP physics body",
        );

        let restored_character_models = restored_runtime
            .workspace()
            .get_children()
            .into_iter()
            .filter(|child| child.is_a("Model") && child.name() == "Tester")
            .count();
        assert_eq!(restored_character_models, 1);
    }

    #[test]
    fn roundtrip_restores_player_service_configuration() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new_with_config(
            game_id,
            2,
            64.0,
            None,
            crate::game::instance::ErrorMode::Continue,
        );
        instance.load_script(
            r#"
            Players.CharacterAutoLoads = false
            _G.beforeMaxPlayers = Players.MaxPlayers
            _G.beforeCharacterAutoLoads = Players.CharacterAutoLoads
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        assert_eq!(restored.max_players, 2);
        assert_eq!(restored.available_slots(), 2);
        assert!(restored.has_capacity());

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.afterMaxPlayers = Players.MaxPlayers
                _G.afterCharacterAutoLoads = Players.CharacterAutoLoads
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<u32>("beforeMaxPlayers").unwrap(), 2);
        assert!(!globals.get::<bool>("beforeCharacterAutoLoads").unwrap());
        assert_eq!(globals.get::<u32>("afterMaxPlayers").unwrap(), 2);
        assert!(!globals.get::<bool>("afterCharacterAutoLoads").unwrap());
    }

    #[test]
    fn roundtrip_restores_host_runtime_configuration() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new_with_config(game_id, 4, 5.0, None, ErrorMode::Halt);
        instance.init_runtime();
        instance.status = GameStatus::Waiting;
        instance.set_player_object_physics(false);

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        assert_eq!(restored.status, GameStatus::Waiting);
        assert_eq!(restored.max_players, 4);
        assert_eq!(restored.observation_radius, 5.0);
        assert!(!restored.player_object_physics());
        assert_eq!(restored.error_mode, ErrorMode::Halt);

        let runtime_error_mode = *restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .app_data_ref::<ErrorMode>()
            .unwrap();
        assert_eq!(runtime_error_mode, ErrorMode::Halt);
    }

    #[test]
    fn roundtrip_restores_halted_error_state() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new_with_config(game_id, 8, 100.0, None, ErrorMode::Halt);
        instance.load_script(r#"error("checkpoint halt")"#);

        let halted_error = instance
            .halted_error
            .clone()
            .expect("script error should halt the source instance");
        assert!(
            halted_error.contains("checkpoint halt"),
            "unexpected halt error: {halted_error}"
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        assert_eq!(restored.error_mode, ErrorMode::Halt);
        assert_eq!(restored.halted_error, Some(halted_error));

        let tick_before = restored.tick;
        restored.tick();
        assert_eq!(
            restored.tick, tick_before,
            "restored halted instance must not resume ticking"
        );
    }

    #[test]
    fn roundtrip_restores_lua_held_service_player_userdata() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();
        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "PersistedPlayer"));

        let runtime = instance.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local player = Players:GetPlayers()[1]
                _G.player = player
                _G.playerGui = player.PlayerGui

                local screen = Instance.new("ScreenGui")
                screen.Name = "PersistedPlayerScreen"
                screen.ResetOnSpawn = false
                screen.Parent = player.PlayerGui
                _G.playerScreenGui = screen
                "#,
            )
            .exec()
            .unwrap();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local player = Players:GetPlayers()[1]
                local child = player.PlayerGui:FindFirstChild("PersistedPlayerScreen")

                _G.samePlayer = _G.player == player
                _G.samePlayerGui = _G.playerGui == player.PlayerGui
                _G.samePlayerGuiChild = _G.playerScreenGui == child
                _G.restoredPlayerName = player.Name
                _G.restoredChildResetOnSpawn = child.ResetOnSpawn
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert!(globals.get::<bool>("samePlayer").unwrap());
        assert!(globals.get::<bool>("samePlayerGui").unwrap());
        assert!(globals.get::<bool>("samePlayerGuiChild").unwrap());
        assert_eq!(
            globals.get::<String>("restoredPlayerName").unwrap(),
            "PersistedPlayer"
        );
        assert!(!globals.get::<bool>("restoredChildResetOnSpawn").unwrap());
    }

    #[test]
    fn roundtrip_restores_lua_held_player_character_userdata() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();
        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "CharacterPlayer"));

        let runtime = instance.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local player = Players:GetPlayers()[1]
                local character = player.Character
                local humanoid = character and character:FindFirstChild("Humanoid")
                local hrp = character and character:FindFirstChild("HumanoidRootPart")

                assert(character ~= nil, "character missing before snapshot")
                assert(humanoid ~= nil, "humanoid missing before snapshot")
                assert(hrp ~= nil, "humanoid root part missing before snapshot")

                _G.player = player
                _G.character = character
                _G.humanoid = humanoid
                _G.hrp = hrp
                "#,
            )
            .exec()
            .unwrap();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local player = Players:GetPlayers()[1]
                local character = player.Character
                local humanoid = character and character:FindFirstChild("Humanoid")
                local hrp = character and character:FindFirstChild("HumanoidRootPart")

                _G.samePlayer = _G.player == player
                _G.sameCharacter = _G.character == character
                _G.sameHumanoid = _G.humanoid == humanoid
                _G.sameHrp = _G.hrp == hrp
                _G.restoredHumanoidName = humanoid and humanoid.Name or nil
                _G.restoredHrpName = hrp and hrp.Name or nil
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert!(globals.get::<bool>("samePlayer").unwrap());
        assert!(globals.get::<bool>("sameCharacter").unwrap());
        assert!(globals.get::<bool>("sameHumanoid").unwrap());
        assert!(globals.get::<bool>("sameHrp").unwrap());
        assert_eq!(
            globals.get::<String>("restoredHumanoidName").unwrap(),
            "Humanoid"
        );
        assert_eq!(
            globals.get::<String>("restoredHrpName").unwrap(),
            "HumanoidRootPart"
        );
    }

    #[test]
    fn roundtrip_restores_input_received_player_character_lookup() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();
        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "InputPlayer"));
        let user_id = GameInstance::user_id_from_agent_id(agent_id);

        let runtime = instance.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local AgentInputService = game:GetService("AgentInputService")

                _G.inputHumanoidFound = nil
                _G.inputMoveToAccepted = false

                AgentInputService.InputReceived:Connect(function(player, inputType, inputData)
                    local character = player.Character
                    local humanoid = character and character:FindFirstChild("Humanoid")
                    _G.inputPlayerName = player.Name
                    _G.inputType = inputType
                    _G.inputHumanoidFound = humanoid ~= nil
                    _G.inputHumanoidName = humanoid and humanoid.Name or nil

                    if humanoid and inputType == "MoveTo" then
                        humanoid:MoveTo(Vector3.new(12, 0, -4))
                        _G.inputMoveToAccepted = true
                    end
                end)
                "#,
            )
            .exec()
            .unwrap();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        restored.queue_agent_input(
            user_id,
            "MoveTo".to_string(),
            serde_json::json!({ "position": [12, 0, -4] }),
        );
        restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .process_agent_inputs()
            .unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(
            globals.get::<String>("inputPlayerName").unwrap(),
            "InputPlayer"
        );
        assert_eq!(globals.get::<String>("inputType").unwrap(), "MoveTo");
        assert!(globals.get::<bool>("inputHumanoidFound").unwrap());
        assert_eq!(
            globals.get::<String>("inputHumanoidName").unwrap(),
            "Humanoid"
        );
        assert!(globals.get::<bool>("inputMoveToAccepted").unwrap());

        restored.tick();

        let restored_hrp_id = restored
            .player_hrp_ids
            .get(&agent_id)
            .copied()
            .expect("missing restored humanoid root part");
        let state = restored
            .physics
            .get_character_state(restored_hrp_id)
            .expect("missing restored character controller state");
        assert_eq!(state.target_position, Some([12.0, 0.0, -4.0]));
    }

    #[test]
    fn roundtrip_restores_player_afk_timers() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();
        instance.set_afk_timeout(std::time::Duration::from_millis(40));
        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "IdlePlayer"));

        std::thread::sleep(std::time::Duration::from_millis(70));
        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();
        assert!(restored.players.contains_key(&agent_id));
        assert_eq!(restored.afk_timeout(), std::time::Duration::from_millis(40));

        restored.tick();

        assert!(
            !restored.players.contains_key(&agent_id),
            "restored player should keep pre-checkpoint idle age and be kicked by the next AFK check"
        );
    }

    #[test]
    fn roundtrip_restores_align_constraint_limits() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local alignPosition = Instance.new("AlignPosition")
            alignPosition.Name = "PersistedAlignPosition"
            alignPosition.MaxVelocity = 12.5
            alignPosition.MaxForce = 4567
            alignPosition.Parent = workspace
            _G.alignPosition = alignPosition

            local alignOrientation = Instance.new("AlignOrientation")
            alignOrientation.Name = "PersistedAlignOrientation"
            alignOrientation.MaxTorque = Vector3.new(9, 8, 7)
            alignOrientation.Parent = workspace
            _G.alignOrientation = alignOrientation
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.restoredMaxVelocity = _G.alignPosition.MaxVelocity
                _G.restoredMaxForce = _G.alignPosition.MaxForce
                _G.restoredTorqueX = _G.alignOrientation.MaxTorque.X
                _G.restoredTorqueY = _G.alignOrientation.MaxTorque.Y
                _G.restoredTorqueZ = _G.alignOrientation.MaxTorque.Z
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert!((globals.get::<f32>("restoredMaxVelocity").unwrap() - 12.5).abs() < 1e-6);
        assert!((globals.get::<f32>("restoredMaxForce").unwrap() - 4567.0).abs() < 1e-6);
        assert!((globals.get::<f32>("restoredTorqueX").unwrap() - 9.0).abs() < 1e-6);
        assert!((globals.get::<f32>("restoredTorqueY").unwrap() - 8.0).abs() < 1e-6);
        assert!((globals.get::<f32>("restoredTorqueZ").unwrap() - 7.0).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_restores_media_instance_payloads() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local animator = Instance.new("Animator")
            animator.Name = "PersistedAnimator"
            animator.Parent = workspace
            _G.animator = animator

            local animation = Instance.new("Animation")
            animation.Name = "PersistedAnimation"
            animation.AnimationId = "local://persisted-animation"
            animation.Parent = workspace
            _G.animation = animation

            animator:LoadAnimation(animation)

            local sound = Instance.new("Sound")
            sound.Name = "PersistedSound"
            sound.StreamId = "asset://persisted-sound"
            sound.Volume = 0.75
            sound.PlaybackSpeed = 1.25
            sound.Parent = workspace
            sound:Play()
            _G.sound = sound
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.restoredAnimationId = _G.animation.AnimationId
                _G.restoredSoundStreamId = _G.sound.StreamId
                _G.restoredSoundVolume = _G.sound.Volume
                _G.restoredSoundPlaybackSpeed = _G.sound.PlaybackSpeed
                _G.restoredSoundIsPlaying = _G.sound.IsPlaying
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(
            globals.get::<String>("restoredAnimationId").unwrap(),
            "local://persisted-animation"
        );
        assert_eq!(
            globals.get::<String>("restoredSoundStreamId").unwrap(),
            "asset://persisted-sound"
        );
        assert!((globals.get::<f32>("restoredSoundVolume").unwrap() - 0.75).abs() < 1e-6);
        assert!((globals.get::<f32>("restoredSoundPlaybackSpeed").unwrap() - 1.25).abs() < 1e-6);
        assert!(globals.get::<bool>("restoredSoundIsPlaying").unwrap());

        let restored_animator = runtime
            .workspace()
            .instance
            .find_first_child("PersistedAnimator", true)
            .unwrap();
        let restored_animator_data = restored_animator.data.lock().unwrap();
        assert_eq!(
            restored_animator_data
                .animator_data
                .as_ref()
                .map(|animator| animator.loaded_tracks),
            Some(1)
        );
    }

    #[test]
    fn roundtrip_resumes_lua_closure_capturing_instance_and_service() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            _G.starts = (_G.starts or 0) + 1

            local part = Instance.new("Part")
            part.Name = "CapturedBeforeResume"
            part.Anchored = true
            part.Parent = Workspace

            local offset = Vector3.new(1, 2, 3)
            local storage = game:GetService("ServerStorage")

            task.delay(0.05, function()
                part.Name = "CapturedAfterResume"
                part.Position = offset

                local marker = Instance.new("Folder")
                marker.Name = "ServiceCapturedAfterResume"
                marker.Parent = storage

                _G.done = (_G.done or 0) + 1
            end)
            "#,
        );

        instance.tick();
        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        for _ in 0..4 {
            restored.tick();
        }

        let runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<i64>("starts").unwrap(), 1);
        assert_eq!(globals.get::<i64>("done").unwrap(), 1);

        let part = runtime
            .workspace()
            .instance
            .find_first_child("CapturedAfterResume", true)
            .expect("captured Part should be mutated by restored closure");
        let position = part
            .data
            .lock()
            .unwrap()
            .part_data
            .as_ref()
            .unwrap()
            .position;
        assert_eq!(snapshot_vector3(position), [1.0, 2.0, 3.0]);

        let storage_marker = runtime
            .game()
            .server_storage()
            .find_first_child("ServiceCapturedAfterResume", false);
        assert!(
            storage_marker.is_some(),
            "captured ServerStorage service should resolve after restore"
        );
    }

    #[test]
    fn roundtrip_restores_mutable_global_delta_and_deletions() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            _G.persistedTable = { count = 12 }
            _G.persistedFunction = function(x)
                return x + persistedTable.count
            end
            _G.persistedConnection = RunService.Heartbeat:Connect(function()
                _G.connectionCallbackRan = true
            end)
            Workspace = { marker = 123.5 }
            warn = nil
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals();
        let table: mlua::Table = globals.get("persistedTable").unwrap();
        assert_eq!(table.get::<i64>("count").unwrap(), 12);
        let function: mlua::Function = globals.get("persistedFunction").unwrap();
        assert_eq!(function.call::<i64>(30).unwrap(), 42);
        runtime
            .lua()
            .load(
                r#"
                _G.connectionWasConnected = persistedConnection.Connected
                persistedConnection:Disconnect()
                _G.connectionIsConnected = persistedConnection.Connected
                "#,
            )
            .exec()
            .unwrap();
        assert!(globals.get::<bool>("connectionWasConnected").unwrap());
        assert!(!globals.get::<bool>("connectionIsConnected").unwrap());
        let workspace_replacement: mlua::Table = globals.get("Workspace").unwrap();
        assert_eq!(workspace_replacement.get::<f64>("marker").unwrap(), 123.5);
        assert!(
            matches!(
                globals.get::<mlua::Value>("warn").unwrap(),
                mlua::Value::Nil
            ),
            "deleted baseline global should remain deleted after restore"
        );
    }

    #[test]
    fn roundtrip_restores_non_string_global_keys() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local tableKey = { kind = "table-key" }
            local userdataKey = Vector3.new(1, 2, 3)

            _G.tableKey = tableKey
            _G.userdataKey = userdataKey
            _G[tableKey] = {
                value = 37,
                callback = function(x)
                    return x + 5
                end,
            }
            _G[userdataKey] = CFrame.new(4, 5, 6)
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local tableValue = _G[_G.tableKey]
                local userdataValue = _G[_G.userdataKey]
                _G.tableKeyValue = tableValue.value
                _G.tableKeyCallbackValue = tableValue.callback(10)
                _G.userdataKeyCFrameX = userdataValue.Position.X
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<i64>("tableKeyValue").unwrap(), 37);
        assert_eq!(globals.get::<i64>("tableKeyCallbackValue").unwrap(), 15);
        assert_eq!(globals.get::<f32>("userdataKeyCFrameX").unwrap(), 4.0);
    }

    #[test]
    fn roundtrip_restores_debris_queue() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local part = Instance.new("Part")
            part.Name = "DebrisTarget"
            part.Anchored = true
            part.Parent = workspace

            game:GetService("Debris"):AddItem(part, 0.05)
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        assert!(runtime
            .workspace()
            .instance
            .find_first_child("DebrisTarget", true)
            .is_some());

        restored.tick();
        assert!(restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .workspace()
            .instance
            .find_first_child("DebrisTarget", true)
            .is_some());

        for _ in 0..4 {
            restored.tick();
        }

        assert!(restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .workspace()
            .instance
            .find_first_child("DebrisTarget", true)
            .is_none());
    }

    #[test]
    fn roundtrip_restores_recent_replicated_events_and_next_id() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local remote = Instance.new("RemoteEvent")
            remote.Name = "RoundtripEvent"
            remote.Parent = game:GetService("ReplicatedStorage")
            remote:FireAllClients({ phase = "before" })
            "#,
        );
        instance.tick();

        let before_obs = instance.get_spectator_observation();
        assert_eq!(before_obs.recent_events.len(), 1);
        assert_eq!(before_obs.recent_events[0].id, 1);
        assert_eq!(before_obs.recent_events[0].name, "RoundtripEvent");

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let restored_obs = restored.get_spectator_observation();
        assert_eq!(restored_obs.recent_events.len(), 1);
        assert_eq!(restored_obs.recent_events[0].id, 1);
        assert_eq!(restored_obs.recent_events[0].name, "RoundtripEvent");
        assert_eq!(
            restored_obs.recent_events[0].payload["phase"],
            serde_json::Value::String("before".to_string())
        );

        restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .game()
            .remote_event_service()
            .fire_all_clients(
                "RoundtripEvent".to_string(),
                serde_json::json!({ "phase": "after" }),
                true,
            );
        restored.tick();

        let after_obs = restored.get_spectator_observation();
        assert_eq!(after_obs.recent_events.len(), 2);
        assert_eq!(after_obs.recent_events[0].id, 1);
        assert_eq!(after_obs.recent_events[1].id, 2);
        assert_eq!(
            after_obs.recent_events[1].payload["phase"],
            serde_json::Value::String("after".to_string())
        );
    }

    #[test]
    fn roundtrip_restores_replication_dirty_state() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local moving = Instance.new("Part")
            moving.Name = "MovingReplicationPart"
            moving.Anchored = true
            moving.Position = Vector3.new(0, 5, 0)
            moving.Parent = workspace
            _G.moving = moving

            local removed = Instance.new("Part")
            removed.Name = "RemovedReplicationPart"
            removed.Anchored = true
            removed.Position = Vector3.new(10, 5, 0)
            removed.Parent = workspace
            "#,
        );
        let known_revision = instance.replication_revision();
        let initial_observation = instance.get_spectator_observation();
        let moving_id = initial_observation
            .entities
            .iter()
            .find(|entity| entity.position == [0.0, 5.0, 0.0])
            .map(|entity| entity.id)
            .expect("moving part should be visible before checkpoint");
        let removed_id = initial_observation
            .entities
            .iter()
            .find(|entity| entity.position == [10.0, 5.0, 0.0])
            .map(|entity| entity.id)
            .expect("removed part should be visible before checkpoint");
        let known_ids = HashSet::from([moving_id, removed_id]);

        instance
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load(
                r#"
                _G.moving.Position = Vector3.new(4, 6, 8)
                workspace.RemovedReplicationPart:Destroy()
                "#,
            )
            .exec()
            .unwrap();
        let dirty_revision = instance.replication_revision();
        assert!(
            dirty_revision > known_revision,
            "test setup should create a newer replication revision"
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        assert_eq!(restored.replication_revision(), dirty_revision);
        let update = restored.get_replication_observation_update(&known_ids, known_revision);
        assert_eq!(update.latest_revision, dirty_revision);
        assert!(
            update.dirty_instance_count >= 2,
            "restored dirty table should contain the changed and removed parts"
        );
        assert!(
            update
                .observation
                .entities
                .iter()
                .any(|entity| entity.id == moving_id && entity.position == [4.0, 6.0, 8.0]),
            "restored dirty update should include the moved entity"
        );
        assert!(
            update.removed_entity_ids.contains(&removed_id),
            "restored dirty update should include removed entity id"
        );
    }

    #[test]
    fn roundtrip_restores_lua_held_orphan_instances() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local model = Instance.new("Model")
            model.Name = "OrphanModel"
            _G.orphanModel = model

            local child = Instance.new("Part")
            child.Name = "OrphanChild"
            child.Anchored = true
            child.Position = Vector3.new(3, 4, 5)
            child.Parent = model
            _G.orphanChild = child

            local doomed = Instance.new("Part")
            doomed.Name = "DestroyedButHeld"
            doomed.Anchored = true
            doomed.Position = Vector3.new(7, 8, 9)
            doomed.Parent = workspace
            _G.destroyedPart = doomed
            doomed:Destroy()
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        let orphan_child: mlua::AnyUserData = globals.get("orphanChild").unwrap();
        assert_eq!(orphan_child.get::<String>("Name").unwrap(), "OrphanChild");
        let orphan_parent: mlua::AnyUserData = orphan_child.get("Parent").unwrap();
        assert_eq!(orphan_parent.get::<String>("Name").unwrap(), "OrphanModel");
        let orphan_position: Vector3 = orphan_child.get("Position").unwrap();
        assert_eq!(snapshot_vector3(orphan_position), [3.0, 4.0, 5.0]);

        let destroyed_part: mlua::AnyUserData = globals.get("destroyedPart").unwrap();
        assert_eq!(
            destroyed_part.get::<String>("Name").unwrap(),
            "DestroyedButHeld"
        );
        assert!(matches!(
            destroyed_part.get::<mlua::Value>("Parent").unwrap(),
            mlua::Value::Nil
        ));
        let destroyed_position: Vector3 = destroyed_part.get("Position").unwrap();
        assert_eq!(snapshot_vector3(destroyed_position), [7.0, 8.0, 9.0]);
        let destroyed_instance = destroyed_part.borrow::<Instance>().unwrap();
        assert!(destroyed_instance.data.lock().unwrap().is_destroyed());
    }

    #[test]
    fn roundtrip_restores_lua_held_datastore_handles() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local service = game:GetService("DataStoreService")
            _G.store = service:GetDataStore("PlayerData")
            _G.ordered = service:GetOrderedDataStore("Leaderboard")
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.storeType = typeof(_G.store)
                _G.orderedType = typeof(_G.ordered)
                local okStore, errStore = pcall(function()
                    return _G.store:GetAsync("player-1")
                end)
                local okOrdered, errOrdered = pcall(function()
                    return _G.ordered:GetSortedAsync(false, 10)
                end)
                _G.storeCallOk = okStore
                _G.orderedCallOk = okOrdered
                _G.storeErr = tostring(errStore)
                _G.orderedErr = tostring(errOrdered)
                "#,
            )
            .exec()
            .unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<String>("storeType").unwrap(), "userdata");
        assert_eq!(globals.get::<String>("orderedType").unwrap(), "userdata");
        assert!(!globals.get::<bool>("storeCallOk").unwrap());
        assert!(!globals.get::<bool>("orderedCallOk").unwrap());
        assert!(globals
            .get::<String>("storeErr")
            .unwrap()
            .contains("DataStoreService not available"));
        assert!(globals
            .get::<String>("orderedErr")
            .unwrap()
            .contains("DataStoreService not available"));
    }

    #[test]
    fn roundtrip_restores_lua_held_workspace_query_userdata() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local target = Instance.new("Part")
            target.Name = "RaycastTarget"
            target.Anchored = true
            target.Size = Vector3.new(4, 2, 4)
            target.Position = Vector3.new(0, 5, 0)
            target.Parent = workspace
            _G.target = target

            local ignored = Instance.new("Part")
            ignored.Name = "IgnoredQueryPart"
            ignored.Anchored = true
            ignored.Size = Vector3.new(4, 2, 4)
            ignored.Position = Vector3.new(10, 5, 0)
            ignored.Parent = workspace
            _G.ignored = ignored

            local rayParams = RaycastParams.new()
            rayParams.FilterType = Enum.RaycastFilterType.Exclude
            rayParams.FilterDescendantsInstances = { ignored }
            rayParams.IgnoreWater = true
            rayParams.CollisionGroup = "Default"
            rayParams.RespectCanCollide = true
            _G.rayParams = rayParams
            _G.raycastHit = workspace:Raycast(Vector3.new(0, 20, 0), Vector3.new(0, -40, 0), rayParams)

            local overlapParams = OverlapParams.new()
            overlapParams.FilterType = Enum.RaycastFilterType.Include
            overlapParams.FilterDescendantsInstances = { target }
            overlapParams.MaxParts = 3
            overlapParams.CollisionGroup = "Default"
            overlapParams.RespectCanCollide = true
            _G.overlapParams = overlapParams
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.rayFilterType = _G.rayParams.FilterType.Name
                _G.rayFilterName = _G.rayParams.FilterDescendantsInstances[1].Name
                _G.rayIgnoreWater = _G.rayParams.IgnoreWater
                _G.rayRespectCanCollide = _G.rayParams.RespectCanCollide
                _G.rayHitName = _G.raycastHit.Instance.Name
                _G.rayHitY = _G.raycastHit.Position.Y

                _G.overlapFilterType = _G.overlapParams.FilterType.Name
                _G.overlapFilterName = _G.overlapParams.FilterDescendantsInstances[1].Name
                _G.overlapMaxParts = _G.overlapParams.MaxParts
                _G.overlapRespectCanCollide = _G.overlapParams.RespectCanCollide
                local parts = workspace:GetPartBoundsInBox(CFrame.new(0, 5, 0), Vector3.new(8, 8, 8), _G.overlapParams)
                _G.overlapCount = #parts
                _G.overlapName = parts[1] and parts[1].Name or ""
                "#,
            )
            .exec()
            .unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<String>("rayFilterType").unwrap(), "Exclude");
        assert_eq!(
            globals.get::<String>("rayFilterName").unwrap(),
            "IgnoredQueryPart"
        );
        assert!(globals.get::<bool>("rayIgnoreWater").unwrap());
        assert!(globals.get::<bool>("rayRespectCanCollide").unwrap());
        assert_eq!(
            globals.get::<String>("rayHitName").unwrap(),
            "RaycastTarget"
        );
        assert!(globals.get::<f32>("rayHitY").unwrap() > 5.0);

        assert_eq!(
            globals.get::<String>("overlapFilterType").unwrap(),
            "Include"
        );
        assert_eq!(
            globals.get::<String>("overlapFilterName").unwrap(),
            "RaycastTarget"
        );
        assert_eq!(globals.get::<i32>("overlapMaxParts").unwrap(), 3);
        assert!(globals.get::<bool>("overlapRespectCanCollide").unwrap());
        assert_eq!(globals.get::<i32>("overlapCount").unwrap(), 1);
        assert_eq!(
            globals.get::<String>("overlapName").unwrap(),
            "RaycastTarget"
        );
    }

    #[test]
    fn roundtrip_restores_lua_held_value_and_service_userdata() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            _G.vector2 = Vector2.new(3, 4)
            _G.ray = Ray.new(Vector3.new(1, 2, 3), Vector3.new(4, 5, 6))
            _G.region = Region3.new(Vector3.new(-1, -2, -3), Vector3.new(5, 6, 7))
            _G.dateTime = DateTime.fromUnixTimestampMillis(1700000000123)

            _G.gameService = game
            _G.workspaceService = game:GetService("Workspace")
            _G.playersService = game:GetService("Players")
            _G.runService = game:GetService("RunService")
            _G.agentInputService = game:GetService("AgentInputService")
            _G.dataStoreService = game:GetService("DataStoreService")
            _G.debrisService = game:GetService("Debris")
            _G.physicsService = game:GetService("PhysicsService")
            _G.remoteEventService = game:GetService("RemoteEventService")
            _G.httpService = game:GetService("HttpService")
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.vector2Type = typeof(_G.vector2)
                _G.vector2Magnitude = _G.vector2.Magnitude
                _G.vector2UnitX = _G.vector2.Unit.X

                _G.rayOriginX = _G.ray.Origin.X
                _G.rayDirectionZ = _G.ray.Direction.Z
                _G.regionSizeX = _G.region.Size.X
                _G.dateTimeMillis = _G.dateTime.UnixTimestampMillis

                _G.workspaceName = _G.workspaceService.Name
                _G.playersMaxPlayers = _G.playersService.MaxPlayers
                _G.hasHeartbeat = _G.runService.Heartbeat ~= nil
                _G.hasInputSignal = _G.agentInputService.InputReceived ~= nil
                _G.dataStoreType = typeof(_G.dataStoreService:GetDataStore("PersistedStore"))
                _G.debrisName = _G.debrisService.Name
                _G.physicsMaxGroups = _G.physicsService:GetMaxCollisionGroups()
                _G.remoteName = _G.remoteEventService.Name
                _G.httpJson = _G.httpService:JSONEncode({ ok = true })
                _G.gameWorkspaceSame = _G.gameService:GetService("Workspace") == _G.workspaceService
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<String>("vector2Type").unwrap(), "Vector2");
        assert!((globals.get::<f32>("vector2Magnitude").unwrap() - 5.0).abs() < 1e-5);
        assert!((globals.get::<f32>("vector2UnitX").unwrap() - 0.6).abs() < 1e-5);
        assert_eq!(globals.get::<f32>("rayOriginX").unwrap(), 1.0);
        assert_eq!(globals.get::<f32>("rayDirectionZ").unwrap(), 6.0);
        assert_eq!(globals.get::<f32>("regionSizeX").unwrap(), 6.0);
        assert_eq!(
            globals.get::<i64>("dateTimeMillis").unwrap(),
            1_700_000_000_123
        );
        assert_eq!(globals.get::<String>("workspaceName").unwrap(), "Workspace");
        assert_eq!(globals.get::<u32>("playersMaxPlayers").unwrap(), 8);
        assert!(globals.get::<bool>("hasHeartbeat").unwrap());
        assert!(globals.get::<bool>("hasInputSignal").unwrap());
        assert_eq!(globals.get::<String>("dataStoreType").unwrap(), "userdata");
        assert_eq!(globals.get::<String>("debrisName").unwrap(), "Debris");
        assert!(globals.get::<i32>("physicsMaxGroups").unwrap() > 0);
        assert_eq!(
            globals.get::<String>("remoteName").unwrap(),
            "RemoteEventService"
        );
        assert_eq!(globals.get::<String>("httpJson").unwrap(), r#"{"ok":true}"#);
        assert!(globals.get::<bool>("gameWorkspaceSame").unwrap());
    }

    #[test]
    fn roundtrip_restores_lua_held_enum_userdata() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            _G.partType = Enum.PartType.Ball
            _G.material = Enum.Material.Neon
            _G.relativeTo = Enum.ActuatorRelativeTo.World
            _G.fluidForces = Enum.FluidForces.Experimental
            _G.humanoidState = Enum.HumanoidStateType.Jumping
            _G.filterType = Enum.RaycastFilterType.Include
            _G.textXAlignment = Enum.TextXAlignment.Right
            _G.textYAlignment = Enum.TextYAlignment.Bottom
            _G.font = Enum.Font.SourceSansBold
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local part = Instance.new("Part")
                part.Shape = _G.partType
                part.Material = _G.material

                local vectorForce = Instance.new("VectorForce")
                vectorForce.RelativeTo = _G.relativeTo

                local humanoid = Instance.new("Humanoid")
                humanoid:ChangeState(_G.humanoidState)

                local params = RaycastParams.new()
                params.FilterType = _G.filterType

                local label = Instance.new("TextLabel")
                label.TextXAlignment = _G.textXAlignment
                label.TextYAlignment = _G.textYAlignment
                label.Font = _G.font

                Workspace.FluidForces = _G.fluidForces

                _G.partTypeName = part.Shape.Name
                _G.partTypeValue = part.Shape.Value
                _G.materialName = part.Material.Name
                _G.materialValue = part.Material.Value
                _G.relativeToName = vectorForce.RelativeTo.Name
                _G.fluidForcesName = Workspace.FluidForces.Name
                _G.fluidForcesValue = Workspace.FluidForces.Value
                _G.humanoidStateName = humanoid:GetState().Name
                _G.filterTypeName = params.FilterType.Name
                _G.filterTypeString = tostring(params.FilterType)
                _G.textXAlignmentName = label.TextXAlignment.Name
                _G.textYAlignmentName = label.TextYAlignment.Name
                _G.fontName = label.Font.Name
            "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<String>("partTypeName").unwrap(), "Ball");
        assert_eq!(globals.get::<i32>("partTypeValue").unwrap(), 0);
        assert_eq!(globals.get::<String>("materialName").unwrap(), "Neon");
        assert_eq!(globals.get::<i32>("materialValue").unwrap(), 288);
        assert_eq!(globals.get::<String>("relativeToName").unwrap(), "World");
        assert_eq!(
            globals.get::<String>("fluidForcesName").unwrap(),
            "Experimental"
        );
        assert_eq!(globals.get::<i32>("fluidForcesValue").unwrap(), 1);
        assert_eq!(
            globals.get::<String>("humanoidStateName").unwrap(),
            "Jumping"
        );
        assert_eq!(globals.get::<String>("filterTypeName").unwrap(), "Include");
        assert_eq!(
            globals.get::<String>("filterTypeString").unwrap(),
            "Enum.RaycastFilterType.Include"
        );
        assert_eq!(
            globals.get::<String>("textXAlignmentName").unwrap(),
            "Right"
        );
        assert_eq!(
            globals.get::<String>("textYAlignmentName").unwrap(),
            "Bottom"
        );
        assert_eq!(globals.get::<String>("fontName").unwrap(), "SourceSansBold");
    }

    #[test]
    fn roundtrip_restores_module_require_cache() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local module = Instance.new("ModuleScript")
            module.Name = "CachedModule"
            module.Source = [[
                _G.moduleRunCount = (_G.moduleRunCount or 0) + 1
                return { value = _G.moduleRunCount }
            ]]
            module.Parent = game:GetService("ServerStorage")

            _G.cachedModule = module
            _G.cachedBefore = require(module)
            _G.beforeValue = _G.cachedBefore.value
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let runtime = restored.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                _G.cachedAfter = require(_G.cachedModule)
                _G.afterValue = _G.cachedAfter.value
                _G.sameCachedTable = _G.cachedBefore == _G.cachedAfter
                "#,
            )
            .exec()
            .unwrap();

        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<i64>("moduleRunCount").unwrap(), 1);
        assert_eq!(globals.get::<i64>("beforeValue").unwrap(), 1);
        assert_eq!(globals.get::<i64>("afterValue").unwrap(), 1);
        assert!(globals.get::<bool>("sameCachedTable").unwrap());
    }

    #[test]
    fn roundtrip_restores_touch_transition_state() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local a = Instance.new("Part")
            a.Name = "TouchA"
            a.Anchored = true
            a.Size = Vector3.new(4, 4, 4)
            a.Position = Vector3.new(0, 5, 0)
            a.Parent = Workspace

            local b = Instance.new("Part")
            b.Name = "TouchB"
            b.Anchored = false
            b.Size = Vector3.new(4, 4, 4)
            b.Position = Vector3.new(0, 5, 0)
            b.Parent = Workspace
            _G.touchB = b

            _G.touched = 0
            _G.ended = 0
            a.Touched:Connect(function(other)
                if other.Name == "TouchB" then
                    _G.touched += 1
                end
            end)
            a.TouchEnded:Connect(function(other)
                if other.Name == "TouchB" then
                    _G.ended += 1
                end
            end)
            "#,
        );

        instance.tick();
        let runtime = instance.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert_eq!(globals.get::<i64>("touched").unwrap(), 1);
        assert_eq!(globals.get::<i64>("ended").unwrap(), 0);

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .lua()
            .load(
                r#"
                _G.touchB.Position = Vector3.new(20, 5, 0)
                "#,
            )
            .exec()
            .unwrap();
        restored.tick();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert_eq!(globals.get::<i64>("touched").unwrap(), 1);
        assert_eq!(globals.get::<i64>("ended").unwrap(), 1);
    }

    #[test]
    fn roundtrip_restores_signal_waiting_coroutine() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            _G.waitStarted = false
            _G.waitDone = false
            _G.waitedDt = nil

            task.spawn(function()
                _G.waitStarted = true
                local dt = RunService.Heartbeat:Wait()
                _G.waitDone = true
                _G.waitedDt = dt
            end)
            "#,
        );

        instance.tick();
        let runtime = instance.lua_runtime.as_ref().unwrap();
        let globals = runtime.lua().globals().get::<mlua::Table>("_G").unwrap();
        assert!(globals.get::<bool>("waitStarted").unwrap());
        assert!(!globals.get::<bool>("waitDone").unwrap());

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert!(globals.get::<bool>("waitStarted").unwrap());
        assert!(!globals.get::<bool>("waitDone").unwrap());

        restored.tick();

        let restored_runtime = restored.lua_runtime.as_ref().unwrap();
        let globals = restored_runtime
            .lua()
            .globals()
            .get::<mlua::Table>("_G")
            .unwrap();
        assert!(globals.get::<bool>("waitDone").unwrap());
        let waited_dt = globals.get::<f64>("waitedDt").unwrap();
        assert!(
            waited_dt > 0.0,
            "restored Heartbeat:Wait should resume with a positive delta"
        );
    }

    #[test]
    fn roundtrip_restores_pending_bot_queue() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            Players:AddBot("QueuedBot")
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();
        assert!(
            restored
                .player_names
                .values()
                .all(|name| name != "QueuedBot"),
            "pending bot should not be created until the restored tick drains the queue"
        );

        restored.tick();
        assert!(
            restored
                .player_names
                .values()
                .any(|name| name == "QueuedBot"),
            "restored pending bot queue should be drained on the next tick"
        );
    }

    #[test]
    fn roundtrip_restores_pending_kick_queue() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.init_runtime();
        let agent_id = Uuid::new_v4();
        assert!(instance.add_player(agent_id, "KickMe"));
        let runtime = instance.lua_runtime.as_ref().unwrap();
        runtime
            .lua()
            .load(
                r#"
                local player = Players:GetPlayers()[1]
                player:Kick("queued before checkpoint")
                "#,
            )
            .exec()
            .unwrap();

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();
        assert!(restored.players.contains_key(&agent_id));

        restored.tick();
        assert!(
            !restored.players.contains_key(&agent_id),
            "restored kick queue should remove the player on the next tick"
        );
    }

    #[test]
    fn roundtrip_restores_pending_physics_command_queue() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            Workspace.Gravity = 0

            local part = Instance.new("Part")
            part.Name = "ImpulseAfterRestore"
            part.Size = Vector3.new(2, 2, 2)
            part.Position = Vector3.new(0, 10, 0)
            part.Anchored = false
            part.CanCollide = false
            part.Parent = Workspace
            _G.impulsePart = part

            part:ApplyImpulse(Vector3.new(part:GetMass() * 12, 0, 0))
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        restored.tick();
        let runtime = restored.lua_runtime.as_ref().unwrap();
        let vx: f64 = runtime
            .lua()
            .load("return _G.impulsePart.AssemblyLinearVelocity.X")
            .eval()
            .unwrap();
        assert!(
            vx > 8.0,
            "restored pending impulse should affect the part, got vx={vx}"
        );
    }

    #[test]
    fn roundtrip_restores_bind_to_close_callbacks() {
        let game_id = Uuid::new_v4();
        let mut instance = GameInstance::new(game_id, None);
        instance.load_script(
            r#"
            local marker = Instance.new("Folder")
            marker.Name = "CloseMarker"
            marker.Parent = workspace

            game:BindToClose(function()
                marker:SetAttribute("Closed", true)
            end)
            "#,
        );

        let snapshot = capture_game_instance(&instance).unwrap();

        let mut restored = GameInstance::new(game_id, None);
        restored.init_runtime();
        restore_game_instance(&mut restored, &snapshot).unwrap();

        let marker = restored
            .lua_runtime
            .as_ref()
            .unwrap()
            .workspace()
            .instance
            .find_first_child("CloseMarker", true)
            .unwrap();
        assert!(marker.get_attribute("Closed").is_none());

        drop(restored.lua_runtime.take());

        assert!(matches!(
            marker.get_attribute("Closed"),
            Some(AttributeValue::Bool(true))
        ));
    }
}

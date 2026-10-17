pub mod async_bridge;
pub mod collision_mesh;
pub mod constants;
pub mod humanoid_movement;
pub mod instance;
pub mod lua;
mod manager;
pub mod physics;
pub mod profiling;
pub mod replication;
pub mod script_bundle;
pub mod world_root;

use dashmap::DashMap;
use parking_lot::RwLock;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use async_bridge::AsyncBridge;
use instance::{
    ErrorMode, GameInstance, MapInfo, PlayerObservation, ReplicationObservationUpdate,
    SpectatorObservation,
};

/// Handle to the game manager state
pub type GameManagerHandle = Arc<GameManagerState>;

/// Per-instance lock wrapper
pub type GameInstanceHandle = Arc<RwLock<GameInstance>>;

/// Default timeout before an empty instance is destroyed (60 seconds)
const EMPTY_INSTANCE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct GameManagerState {
    /// Running instances, keyed by instance_id
    pub instances: DashMap<Uuid, GameInstanceHandle>,
    /// Maps game_id to list of active instance_ids
    pub game_instances: DashMap<Uuid, Vec<Uuid>>,
    /// Maps (agent_id, game_id) to instance_id for routing
    pub player_instances: DashMap<(Uuid, Uuid), Uuid>,
    /// Cached observations, keyed by (instance_id, agent_id)
    pub observation_cache: DashMap<(Uuid, Uuid), PlayerObservation>,
    /// Cached spectator observations, keyed by instance_id
    pub spectator_cache: DashMap<Uuid, SpectatorObservation>,
    /// Cached static map geometry, keyed by game_id (same for all instances of a game)
    pub map_cache: DashMap<Uuid, MapInfo>,
    /// Shared async bridge for database operations
    pub async_bridge: Option<Arc<AsyncBridge>>,
    /// Error mode for new instances (Halt for CLI dev, Continue for production)
    pub error_mode: ErrorMode,
    /// When true, skip garbage collection of empty instances (used by CLI)
    pub disable_gc: bool,
}

impl GameManagerState {
    pub fn new(
        async_bridge: Option<Arc<AsyncBridge>>,
        error_mode: ErrorMode,
        disable_gc: bool,
    ) -> Self {
        Self {
            instances: DashMap::new(),
            game_instances: DashMap::new(),
            player_instances: DashMap::new(),
            observation_cache: DashMap::new(),
            spectator_cache: DashMap::new(),
            map_cache: DashMap::new(),
            async_bridge,
            error_mode,
            disable_gc,
        }
    }
}

pub struct GameManager {
    state: GameManagerHandle,
    tick_rate: u64,
}

impl GameManager {
    pub fn new(
        tick_rate: u64,
        pool: Arc<PgPool>,
        error_mode: ErrorMode,
    ) -> (Self, GameManagerHandle) {
        let async_bridge = Arc::new(AsyncBridge::new(pool));
        Self::new_with_bridge(tick_rate, Some(async_bridge), error_mode, false)
    }

    pub fn new_with_bridge(
        tick_rate: u64,
        async_bridge: Option<Arc<AsyncBridge>>,
        error_mode: ErrorMode,
        disable_gc: bool,
    ) -> (Self, GameManagerHandle) {
        let state = Arc::new(GameManagerState::new(async_bridge, error_mode, disable_gc));
        let handle = Arc::clone(&state);
        (Self { state, tick_rate }, handle)
    }

    pub fn new_without_db(tick_rate: u64, error_mode: ErrorMode) -> (Self, GameManagerHandle) {
        Self::new_with_bridge(tick_rate, None, error_mode, true)
    }
}

// =============================================================================
// Instance Management
// =============================================================================

#[derive(Debug, Clone)]
pub struct FindInstanceResult {
    pub instance_id: Uuid,
    pub created: bool,
}

/// Finds an instance with capacity or creates a new one
pub fn find_or_create_instance(
    state: &GameManagerHandle,
    game_id: Uuid,
    max_players: u32,
    observation_radius: f32,
    script: Option<&str>,
) -> FindInstanceResult {
    manager::instances::find_or_create_instance(
        state,
        game_id,
        max_players,
        observation_radius,
        script,
    )
}

/// Inserts a pre-built instance into the manager state.
pub fn insert_instance(state: &GameManagerHandle, instance: GameInstance) -> Uuid {
    manager::instances::insert_instance(state, instance)
}

/// Checks if any instance is running for this game
pub fn is_instance_running(state: &GameManagerHandle, game_id: Uuid) -> bool {
    manager::instances::is_instance_running(state, game_id)
}

/// Gets the instance_id a player is in for a specific game
pub fn get_player_instance(
    state: &GameManagerHandle,
    agent_id: Uuid,
    game_id: Uuid,
) -> Option<Uuid> {
    manager::instances::get_player_instance(state, agent_id, game_id)
}

// =============================================================================
// Player Management
// =============================================================================

/// Joins a player to an instance (with capacity check)
pub fn join_instance(
    state: &GameManagerHandle,
    instance_id: Uuid,
    game_id: Uuid,
    agent_id: Uuid,
    agent_name: &str,
) -> Result<(), String> {
    manager::players::join_instance(state, instance_id, game_id, agent_id, agent_name)
}

/// Leaves a player from an instance
pub fn leave_instance(
    state: &GameManagerHandle,
    instance_id: Uuid,
    agent_id: Uuid,
) -> Result<(), String> {
    manager::players::leave_instance(state, instance_id, agent_id)
}

/// Leaves a player from their instance in a game (lookup by game_id)
pub fn leave_game(state: &GameManagerHandle, game_id: Uuid, agent_id: Uuid) -> Result<(), String> {
    let instance_id = get_player_instance(state, agent_id, game_id)
        .ok_or_else(|| "Not in any instance of this game".to_string())?;
    leave_instance(state, instance_id, agent_id)
}

// =============================================================================
// Input
// =============================================================================

pub fn queue_input(
    state: &GameManagerHandle,
    game_id: Uuid,
    agent_id: Uuid,
    input_type: String,
    data: serde_json::Value,
) -> Result<(), String> {
    manager::players::queue_input(state, game_id, agent_id, input_type, data)
}

// =============================================================================
// Observations
// =============================================================================

pub fn get_observation(
    state: &GameManagerHandle,
    game_id: Uuid,
    agent_id: Uuid,
) -> Result<PlayerObservation, String> {
    manager::observations::get_observation(state, game_id, agent_id)
}

pub fn get_player_camera_pose(
    state: &GameManagerHandle,
    game_id: Uuid,
    agent_id: Uuid,
) -> Result<crate::game::instance::PlayerCameraPose, String> {
    manager::observations::get_player_camera_pose(state, game_id, agent_id)
}

pub fn get_spectator_observation(
    state: &GameManagerHandle,
    game_id: Uuid,
) -> Result<SpectatorObservation, String> {
    manager::observations::get_spectator_observation(state, game_id)
}

pub fn get_spectator_observation_for_instance(
    state: &GameManagerHandle,
    instance_id: Uuid,
) -> Result<SpectatorObservation, String> {
    manager::observations::get_spectator_observation_for_instance(state, instance_id)
}

pub fn replication_revision_for_instance(
    state: &GameManagerHandle,
    instance_id: Uuid,
) -> Result<u64, String> {
    manager::observations::replication_revision_for_instance(state, instance_id)
}

pub fn get_replication_observation_update_for_instance(
    state: &GameManagerHandle,
    instance_id: Uuid,
    known_entity_ids: &std::collections::HashSet<u32>,
    since_revision: u64,
) -> Result<ReplicationObservationUpdate, String> {
    manager::observations::get_replication_observation_update_for_instance(
        state,
        instance_id,
        known_entity_ids,
        since_revision,
    )
}

pub fn get_spectator_player_observation_for_instance(
    state: &GameManagerHandle,
    instance_id: Uuid,
) -> Result<SpectatorObservation, String> {
    manager::observations::get_spectator_player_observation_for_instance(state, instance_id)
}

/// Get static map geometry for a game (cached per game_id)
pub fn get_map(state: &GameManagerHandle, game_id: Uuid) -> Result<MapInfo, String> {
    manager::observations::get_map(state, game_id)
}

// =============================================================================
// Info & Listing
// =============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct GameInfo {
    pub id: Uuid,
    pub status: String,
    pub player_count: usize,
    pub tick: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstanceInfo {
    pub instance_id: Uuid,
    pub game_id: Uuid,
    pub status: String,
    pub player_count: usize,
    pub max_players: usize,
    pub tick: u64,
}

pub fn list_instances(state: &GameManagerHandle) -> Vec<InstanceInfo> {
    manager::listing::list_instances(state)
}

pub fn list_games(state: &GameManagerHandle) -> Vec<GameInfo> {
    manager::listing::list_games(state)
}

pub fn get_game_info(state: &GameManagerHandle, game_id: Uuid) -> Option<GameInfo> {
    manager::listing::get_game_info(state, game_id)
}

// =============================================================================
// Instance Lifecycle
// =============================================================================

pub fn destroy_instance(state: &GameManagerHandle, instance_id: Uuid) -> bool {
    manager::lifecycle::destroy_instance(state, instance_id)
}

pub fn cleanup_empty_instances(state: &GameManagerHandle) -> usize {
    cleanup_empty_instances_with_timeout(state, EMPTY_INSTANCE_TIMEOUT)
}

pub fn cleanup_empty_instances_with_timeout(state: &GameManagerHandle, timeout: Duration) -> usize {
    manager::lifecycle::cleanup_empty_instances_with_timeout(state, timeout)
}

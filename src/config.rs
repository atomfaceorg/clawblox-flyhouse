//! World configuration parsing from world.toml files

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Scripts configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct ScriptsConfig {
    /// Main Lua script file (relative to world directory)
    pub main: String,
    /// Optional script tree directory (relative to world directory).
    /// Expected layout uses service roots like:
    /// - scripts/ServerScriptService/**/*.lua
    /// - scripts/Workspace/**/*.lua
    #[serde(default)]
    pub tree: Option<String>,
    /// Agent API documentation file (relative to world directory).
    ///
    /// `API.md` is the preferred name. The legacy `skill` field name and
    /// `SKILL.md` file name remain supported as aliases.
    #[serde(default, alias = "skill")]
    pub api: Option<String>,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            main: "main.lua".to_string(),
            tree: None,
            api: Some("API.md".to_string()),
        }
    }
}

/// Runtime mode for local execution.
///
/// - `clawblox` (default): run the built-in Lua simulation.
/// - `external`: serve frontend/renderer only (no Lua runtime).
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_runtime_mode")]
    pub mode: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            mode: default_runtime_mode(),
        }
    }
}

/// Optional external run command configuration.
///
/// When configured, `clawblox run` can delegate process startup to this command.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RunConfig {
    /// Command to execute, argv-style.
    /// Example: ["node", "play.js"]
    #[serde(default)]
    pub command: Option<Vec<String>>,
}

/// Renderer configuration section.
#[derive(Debug, Clone, Deserialize)]
pub struct RendererConfig {
    /// Human-readable renderer name shown in local runtime UI.
    #[serde(default = "default_renderer_name")]
    pub name: String,
    /// Renderer host mode. Currently "module" or "default".
    #[serde(default = "default_renderer_mode")]
    pub mode: String,
    /// Renderer API contract version expected by this world.
    #[serde(default = "default_renderer_api_version")]
    pub api_version: u32,
    /// Optional renderer module entrypoint relative to world_dir/renderer/.
    #[serde(default)]
    pub entry: Option<String>,
    /// Optional renderer source entrypoint relative to world_dir/renderer/.
    /// If set, local CLI can bundle this into a runtime-friendly ESM module.
    #[serde(default)]
    pub source: Option<String>,
    /// Optional declared capabilities for docs and tooling.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            name: default_renderer_name(),
            mode: default_renderer_mode(),
            api_version: default_renderer_api_version(),
            entry: None,
            source: None,
            capabilities: vec![],
        }
    }
}

/// Observation configuration section.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservationConfig {
    /// Maximum distance at which other players and spatial dynamic entities are
    /// included in observations.
    #[serde(default = "default_observation_radius")]
    pub radius: f32,
    /// Send agents a compact observation: entities keep id, name, position,
    /// size, anchored and attributes, without rotation, bounding boxes, color,
    /// material or shape; view-cone attributes are dropped unless vision is on.
    /// Spectators and recordings are unaffected.
    #[serde(default)]
    pub compact: bool,
}

impl Default for ObservationConfig {
    fn default() -> Self {
        Self {
            radius: default_observation_radius(),
            compact: false,
        }
    }
}

/// Agent API configuration section.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentApiConfig {
    /// Whether engine reset endpoints are available to agent sessions.
    #[serde(default)]
    pub allow_reset: bool,
    /// Whether agent sessions can request first-person rendered frames via
    /// GET /observe/image (spawns a headless-browser capture sidecar).
    #[serde(default, alias = "vision")]
    pub vision_observation: bool,
    /// Whether agent sessions can request structured JSON state observations
    /// via GET /observe and action responses.
    #[serde(default = "default_state_observation")]
    pub state_observation: bool,
    /// Whether action responses may include a narrow, non-spatial feedback
    /// payload when state observations are disabled.
    #[serde(default = "default_action_feedback")]
    pub action_feedback: bool,
}

impl Default for AgentApiConfig {
    fn default() -> Self {
        Self {
            allow_reset: false,
            vision_observation: false,
            state_observation: default_state_observation(),
            action_feedback: default_action_feedback(),
        }
    }
}

fn default_state_observation() -> bool {
    true
}

fn default_action_feedback() -> bool {
    true
}

/// Built-in Clawblox runtime configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ClawbloxConfig {
    /// Whether player character controllers physically couple to dynamic world objects.
    #[serde(default = "default_player_object_physics")]
    pub player_object_physics: bool,
    /// Rapier physics tuning for the built-in runtime.
    #[serde(default)]
    pub physics: PhysicsConfig,
}

impl Default for ClawbloxConfig {
    fn default() -> Self {
        Self {
            player_object_physics: default_player_object_physics(),
            physics: PhysicsConfig::default(),
        }
    }
}

fn default_player_object_physics() -> bool {
    true
}

/// Built-in physics solver configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PhysicsConfig {
    /// Number of outer contact-solver iterations per physics step.
    ///
    /// Rapier's default is 4. Lower values reduce large awake-island cost but
    /// make stacked/penetrating contacts softer; higher values favor stability
    /// at the cost of CPU.
    #[serde(default = "default_physics_solver_iterations")]
    pub solver_iterations: usize,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            solver_iterations: default_physics_solver_iterations(),
        }
    }
}

fn default_physics_solver_iterations() -> usize {
    4
}

/// Speech delivery mode.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpeechMode {
    Global,
    Proximity,
}

impl Default for SpeechMode {
    fn default() -> Self {
        Self::Global
    }
}

/// Speech configuration section.
#[derive(Debug, Clone, Deserialize)]
pub struct SpeechConfig {
    /// How spoken messages are delivered.
    #[serde(default)]
    pub mode: SpeechMode,
    /// Maximum hearing distance for proximity speech.
    #[serde(default = "default_speech_hearing_radius")]
    pub hearing_radius: f32,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            mode: SpeechMode::default(),
            hearing_radius: default_speech_hearing_radius(),
        }
    }
}

/// Agent sandbox configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SandboxConfig {
    /// Enable the Docker-compatible nested sandbox mode.
    ///
    /// This is weaker than the standard bubblewrap sandbox and should only be
    /// enabled when the outer container is part of the isolation boundary.
    #[serde(default)]
    pub enable_weaker_nested_sandbox: bool,
    /// Optional host dependency prefix to mount read-only at /sandbox-deps.
    #[serde(default)]
    pub deps_root: Option<PathBuf>,
    /// Add /sandbox-deps/lib to LD_LIBRARY_PATH when deps_root is set.
    #[serde(default = "default_add_deps_lib_to_library_path")]
    pub add_deps_lib_to_library_path: bool,
}

fn default_add_deps_lib_to_library_path() -> bool {
    true
}

/// World configuration from world.toml
#[derive(Debug, Clone, Deserialize)]
pub struct WorldConfig {
    /// Display name of the world.
    pub name: String,
    /// Description of the world.
    #[serde(default)]
    pub description: Option<String>,
    /// Maximum number of players per instance
    #[serde(default = "default_max_players")]
    pub max_players: u32,
    /// Scripts configuration
    #[serde(default)]
    pub scripts: ScriptsConfig,
    /// Runtime mode selection
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// Frontend renderer configuration
    #[serde(default)]
    pub renderer: RendererConfig,
    /// Observation configuration
    #[serde(default)]
    pub observation: ObservationConfig,
    /// Agent API configuration
    #[serde(default)]
    pub agent_api: AgentApiConfig,
    /// Speech configuration
    #[serde(default)]
    pub speech: SpeechConfig,
    /// Agent sandbox configuration
    #[serde(default)]
    pub sandbox: SandboxConfig,
    /// Built-in Clawblox runtime configuration.
    #[serde(default)]
    pub clawblox: ClawbloxConfig,
    /// AFK timeout in seconds (0 = disabled). Players idle longer than this are kicked.
    #[serde(default = "default_afk_timeout")]
    pub afk_timeout: u64,
    /// Optional external runtime command
    #[serde(default)]
    pub run: RunConfig,
    /// Static triangle-mesh colliders loaded from .glb files (`[[collision_mesh]]`).
    #[serde(default, rename = "collision_mesh")]
    pub collision_meshes: Vec<crate::game::collision_mesh::CollisionMeshConfig>,
}

fn default_max_players() -> u32 {
    8
}

fn default_afk_timeout() -> u64 {
    300
}

fn default_renderer_name() -> String {
    "World Renderer".to_string()
}

fn default_renderer_mode() -> String {
    "module".to_string()
}

fn default_renderer_api_version() -> u32 {
    1
}

fn default_observation_radius() -> f32 {
    100.0
}

fn default_speech_hearing_radius() -> f32 {
    30.0
}

fn default_runtime_mode() -> String {
    "clawblox".to_string()
}

impl WorldConfig {
    /// Load world configuration from a TOML file
    pub fn from_file(path: &Path) -> Result<Self, WorldConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| WorldConfigError::IoError(path.to_path_buf(), e))?;

        let cfg: WorldConfig = toml::from_str(&content)
            .map_err(|e| WorldConfigError::ParseError(path.to_path_buf(), e))?;
        cfg.validate(path)?;
        Ok(cfg)
    }

    /// Load world configuration from a world directory.
    /// Looks for world.toml in the given directory
    pub fn from_game_dir(world_dir: &Path) -> Result<Self, WorldConfigError> {
        let config_path = world_dir.join("world.toml");
        Self::from_file(&config_path)
    }

    fn validate(&self, path: &Path) -> Result<(), WorldConfigError> {
        if !self.observation.radius.is_finite() || self.observation.radius <= 0.0 {
            return Err(WorldConfigError::ValidationError(
                path.to_path_buf(),
                "observation.radius must be a finite number > 0".to_string(),
            ));
        }
        if !self.speech.hearing_radius.is_finite() || self.speech.hearing_radius <= 0.0 {
            return Err(WorldConfigError::ValidationError(
                path.to_path_buf(),
                "speech.hearing_radius must be a finite number > 0".to_string(),
            ));
        }
        if let Some(cmd) = self.run.command.as_ref() {
            if cmd.is_empty() {
                return Err(WorldConfigError::ValidationError(
                    path.to_path_buf(),
                    "run.command must contain at least one token".to_string(),
                ));
            }
            if cmd[0].trim().is_empty() {
                return Err(WorldConfigError::ValidationError(
                    path.to_path_buf(),
                    "run.command[0] must be a non-empty executable".to_string(),
                ));
            }
        }
        if let Some(root) = self.sandbox.deps_root.as_ref() {
            if root.as_os_str().is_empty() {
                return Err(WorldConfigError::ValidationError(
                    path.to_path_buf(),
                    "sandbox.deps_root must be a non-empty path".to_string(),
                ));
            }
        }
        if !(1..=32).contains(&self.clawblox.physics.solver_iterations) {
            return Err(WorldConfigError::ValidationError(
                path.to_path_buf(),
                "clawblox.physics.solver_iterations must be between 1 and 32".to_string(),
            ));
        }
        if !self.agent_api.state_observation && !self.agent_api.vision_observation {
            return Err(WorldConfigError::ValidationError(
                path.to_path_buf(),
                "agent_api.state_observation=false requires agent_api.vision_observation=true"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// Errors that can occur when loading world configuration
#[derive(Debug)]
pub enum WorldConfigError {
    IoError(std::path::PathBuf, std::io::Error),
    ParseError(std::path::PathBuf, toml::de::Error),
    ValidationError(std::path::PathBuf, String),
}

impl std::fmt::Display for WorldConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldConfigError::IoError(path, e) => {
                write!(f, "Failed to read {}: {}", path.display(), e)
            }
            WorldConfigError::ParseError(path, e) => {
                write!(f, "Failed to parse {}: {}", path.display(), e)
            }
            WorldConfigError::ValidationError(path, msg) => {
                write!(f, "Invalid config {}: {}", path.display(), msg)
            }
        }
    }
}

impl std::error::Error for WorldConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
            name = "Test Game"
        "#;
        let config: WorldConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "Test Game");
        assert_eq!(config.max_players, 8);
        assert_eq!(config.runtime.mode, "clawblox");
        assert_eq!(config.scripts.api, Some("API.md".to_string()));
        assert_eq!(config.observation.radius, 100.0);
        assert_eq!(config.speech.mode, SpeechMode::Global);
        assert_eq!(config.speech.hearing_radius, 30.0);
        assert!(!config.agent_api.allow_reset);
        assert!(!config.agent_api.vision_observation);
        assert!(config.agent_api.state_observation);
        assert!(config.agent_api.action_feedback);
        assert!(config.clawblox.player_object_physics);
        assert_eq!(config.clawblox.physics.solver_iterations, 4);
        assert!(!config.sandbox.enable_weaker_nested_sandbox);
        assert_eq!(config.sandbox.deps_root, None);
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
            name = "Test Game"
            description = "A test game"
            max_players = 16
            [runtime]
            mode = "external"

            [scripts]
            main = "main.lua"
            tree = "scripts"
            skill = "SKILL.md"

            [renderer]
            name = "Test Renderer"
            mode = "module"
            api_version = 1
            entry = "index.js"
            source = "src/index.ts"
            capabilities = ["camera.follow"]

            [observation]
            radius = 64

            [speech]
            mode = "proximity"
            hearing_radius = 24

            [sandbox]
            enable_weaker_nested_sandbox = true
            deps_root = "/opt/conda"
            add_deps_lib_to_library_path = false

            [clawblox]
            player_object_physics = false

            [agent_api]
            allow_reset = true
            vision_observation = true
            state_observation = false
            action_feedback = true

            [clawblox.physics]
            solver_iterations = 2

            [run]
            command = ["custom-runner", "--flag"]
        "#;
        let config: WorldConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "Test Game");
        assert_eq!(config.description, Some("A test game".to_string()));
        assert_eq!(config.max_players, 16);
        assert_eq!(config.runtime.mode, "external");
        assert_eq!(config.scripts.main, "main.lua");
        assert_eq!(config.scripts.tree, Some("scripts".to_string()));
        assert_eq!(config.renderer.mode, "module");
        assert_eq!(config.renderer.entry.as_deref(), Some("index.js"));
        assert_eq!(config.renderer.source.as_deref(), Some("src/index.ts"));
        assert_eq!(config.renderer.api_version, 1);
        assert_eq!(config.observation.radius, 64.0);
        assert_eq!(config.speech.mode, SpeechMode::Proximity);
        assert_eq!(config.speech.hearing_radius, 24.0);
        assert!(config.agent_api.allow_reset);
        assert!(config.agent_api.vision_observation);
        assert!(!config.agent_api.state_observation);
        assert!(config.agent_api.action_feedback);
        assert!(!config.clawblox.player_object_physics);
        assert_eq!(config.clawblox.physics.solver_iterations, 2);
        assert!(config.sandbox.enable_weaker_nested_sandbox);
        assert_eq!(
            config.sandbox.deps_root.as_deref(),
            Some(Path::new("/opt/conda"))
        );
        assert!(!config.sandbox.add_deps_lib_to_library_path);
        assert_eq!(
            config.run.command.as_ref().map(|c| c.as_slice()),
            Some(vec!["custom-runner".to_string(), "--flag".to_string()].as_slice())
        );
    }

    #[test]
    fn test_invalid_observation_radius_fails_validation() {
        let toml = r#"
            name = "Bad Config"

            [observation]
            radius = 0
        "#;

        let config: WorldConfig = toml::from_str(toml).unwrap();
        let err = config
            .validate(std::path::Path::new("world.toml"))
            .unwrap_err();
        assert!(format!("{}", err).contains("observation.radius"));
    }

    #[test]
    fn test_invalid_speech_hearing_radius_fails_validation() {
        let toml = r#"
            name = "Bad Config"

            [speech]
            hearing_radius = 0
        "#;

        let config: WorldConfig = toml::from_str(toml).unwrap();
        let err = config
            .validate(std::path::Path::new("world.toml"))
            .unwrap_err();
        assert!(format!("{}", err).contains("speech.hearing_radius"));
    }

    #[test]
    fn test_legacy_vision_alias_parses() {
        let toml = r#"
            name = "Vision Alias"

            [agent_api]
            vision = true
        "#;

        let config: WorldConfig = toml::from_str(toml).unwrap();
        assert!(config.agent_api.vision_observation);
    }

    #[test]
    fn test_state_observation_disabled_requires_vision() {
        let toml = r#"
            name = "Bad Config"

            [agent_api]
            state_observation = false
        "#;

        let config: WorldConfig = toml::from_str(toml).unwrap();
        let err = config
            .validate(std::path::Path::new("world.toml"))
            .unwrap_err();
        assert!(format!("{}", err).contains("state_observation"));
    }
}

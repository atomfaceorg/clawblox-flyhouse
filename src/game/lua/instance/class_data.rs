use std::collections::HashMap;

use super::datamodel::WeakInstanceRef;
use crate::game::constants::humanoid as humanoid_consts;
pub use crate::game::lua::animation::{
    default_animation_length_seconds, AnimationScheduler, AnimationTrack, AnimationTrackSnapshot,
};
use crate::game::lua::events::{create_signal, RBXScriptSignal};
use crate::game::lua::types::{
    ActuatorRelativeTo, CFrame, Color3, HumanoidStateType, Material, PartType, PhysicalProperties,
    UDim, UDim2, Vector3,
};

#[derive(Debug, Clone)]
pub struct PartData {
    pub position: Vector3,
    pub cframe: CFrame,
    pub size: Vector3,
    pub anchored: bool,
    pub can_collide: bool,
    pub can_query: bool,
    pub collision_group: String,
    pub collision_group_id: u8,
    pub can_touch: bool,
    pub transparency: f32,
    pub color: Color3,
    pub material: Material,
    pub top_surface: String,
    pub bottom_surface: String,
    pub velocity: Vector3,
    pub angular_velocity: Vector3,
    pub massless: bool,
    pub enable_fluid_forces: bool,
    pub custom_physical_properties: Option<PhysicalProperties>,
    pub custom_physical_properties_dirty: bool,
    pub shape: PartType,
    /// Static triangle-mesh collider id from `collision_mesh`; overrides `shape` for physics and raycasts.
    pub mesh_id: Option<String>,
    pub position_dirty: bool,
    /// Distinguishes Roblox's welded-part transform semantics:
    /// Position changes recompute the weld offset, while CFrame changes move the
    /// whole rigid assembly through the existing offset.
    pub cframe_dirty: bool,
    pub size_dirty: bool,
    pub anchored_dirty: bool,
    pub can_collide_dirty: bool,
    pub interaction_dirty: bool,
    pub velocity_dirty: bool,
    pub angular_velocity_dirty: bool,
    pub shape_dirty: bool,

    pub touched: RBXScriptSignal,
    pub touch_ended: RBXScriptSignal,
}

impl Default for PartData {
    fn default() -> Self {
        Self {
            position: Vector3::zero(),
            cframe: CFrame::identity(),
            size: Vector3::new(4.0, 1.0, 2.0),
            anchored: false,
            can_collide: true,
            can_query: true,
            collision_group: "Default".to_string(),
            collision_group_id: 0,
            can_touch: true,
            transparency: 0.0,
            color: Color3::new(0.6, 0.6, 0.6),
            material: Material::Plastic,
            top_surface: "Smooth".to_string(),
            bottom_surface: "Smooth".to_string(),
            velocity: Vector3::zero(),
            angular_velocity: Vector3::zero(),
            massless: false,
            enable_fluid_forces: false,
            custom_physical_properties: None,
            custom_physical_properties_dirty: false,
            shape: PartType::Block,
            mesh_id: None,
            position_dirty: false,
            cframe_dirty: false,
            size_dirty: false,
            anchored_dirty: false,
            can_collide_dirty: false,
            interaction_dirty: false,
            velocity_dirty: false,
            angular_velocity_dirty: false,
            shape_dirty: false,
            touched: create_signal("Touched"),
            touch_ended: create_signal("TouchEnded"),
        }
    }
}

impl PartData {
    pub fn has_physics_sync_dirty(&self) -> bool {
        self.position_dirty
            || self.cframe_dirty
            || self.size_dirty
            || self.anchored_dirty
            || self.interaction_dirty
            || self.velocity_dirty
            || self.angular_velocity_dirty
            || self.shape_dirty
            || self.custom_physical_properties_dirty
    }

    pub fn clear_physics_sync_dirty(&mut self) {
        self.size_dirty = false;
        self.anchored_dirty = false;
        self.can_collide_dirty = false;
        self.interaction_dirty = false;
        self.velocity_dirty = false;
        self.angular_velocity_dirty = false;
        self.shape_dirty = false;
        self.custom_physical_properties_dirty = false;
        self.position_dirty = false;
        self.cframe_dirty = false;
    }

    fn default_density_for_material(material: Material) -> f32 {
        match material {
            Material::Air => 0.0,
            Material::Wood => 0.35,
            Material::Metal => 7.85,
            Material::Glass => 2.5,
            Material::Neon => 0.7,
            Material::Concrete => 2.403,
            Material::Brick => 1.922,
            Material::Granite => 2.691,
            Material::Grass => 0.7,
            Material::Ice => 0.919,
            Material::Sand => 1.602,
            Material::Fabric => 0.3,
            Material::Marble => 2.563,
            Material::Slate => 2.691,
            Material::SmoothPlastic => 0.7,
            Material::ForceField => 0.0,
            Material::Water => 1.0,
            Material::Plastic => 0.7,
        }
    }

    pub fn current_physical_properties(&self) -> PhysicalProperties {
        self.custom_physical_properties.unwrap_or_else(|| {
            PhysicalProperties::new(
                Self::default_density_for_material(self.material),
                0.3,
                0.5,
                1.0,
                1.0,
            )
        })
    }

    pub fn effective_physical_properties(&self) -> PhysicalProperties {
        let mut props = self.current_physical_properties();
        if self.massless {
            props.density = 0.0;
        }
        props
    }

    pub fn specific_gravity(&self) -> f32 {
        self.current_physical_properties().density
    }

    pub fn volume(&self) -> f32 {
        let sx = self.size.x.max(0.0);
        let sy = self.size.y.max(0.0);
        let sz = self.size.z.max(0.0);
        match self.shape {
            PartType::Block => sx * sy * sz,
            PartType::Wedge => 0.5 * sx * sy * sz,
            PartType::Ball => {
                let r = sx * 0.5;
                (4.0 / 3.0) * std::f32::consts::PI * r * r * r
            }
            PartType::Cylinder => {
                let r = sx * 0.5;
                std::f32::consts::PI * r * r * sy
            }
            PartType::Ring => {
                let outer_radius = 0.5 * sx.min(sz);
                let tube_radius = (sy * 0.5).min(outer_radius * 0.45);
                let major_radius = (outer_radius - tube_radius).max(0.0);
                2.0 * std::f32::consts::PI
                    * std::f32::consts::PI
                    * major_radius
                    * tube_radius
                    * tube_radius
            }
        }
    }

    pub fn mass(&self) -> f32 {
        self.volume() * self.effective_physical_properties().density
    }
}

pub const TERRAIN_VOXEL_SIZE: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainCell {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl TerrainCell {
    pub fn center(self) -> Vector3 {
        Vector3::new(
            (self.x as f32 + 0.5) * TERRAIN_VOXEL_SIZE,
            (self.y as f32 + 0.5) * TERRAIN_VOXEL_SIZE,
            (self.z as f32 + 0.5) * TERRAIN_VOXEL_SIZE,
        )
    }

    pub fn min(self) -> Vector3 {
        Vector3::new(
            self.x as f32 * TERRAIN_VOXEL_SIZE,
            self.y as f32 * TERRAIN_VOXEL_SIZE,
            self.z as f32 * TERRAIN_VOXEL_SIZE,
        )
    }

    pub fn max(self) -> Vector3 {
        Vector3::new(
            (self.x as f32 + 1.0) * TERRAIN_VOXEL_SIZE,
            (self.y as f32 + 1.0) * TERRAIN_VOXEL_SIZE,
            (self.z as f32 + 1.0) * TERRAIN_VOXEL_SIZE,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TerrainVoxel {
    pub solid_material: Material,
    pub solid_occupancy: f32,
    pub liquid_occupancy: f32,
}

impl Default for TerrainVoxel {
    fn default() -> Self {
        Self {
            solid_material: Material::Air,
            solid_occupancy: 0.0,
            liquid_occupancy: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TerrainData {
    pub voxels: HashMap<TerrainCell, TerrainVoxel>,
}

impl TerrainData {
    pub fn fill_block(&mut self, cframe: CFrame, size: Vector3, material: Material) {
        let size = Vector3::new(size.x.abs(), size.y.abs(), size.z.abs());
        if size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0 {
            return;
        }

        let half = Vector3::new(size.x / 2.0, size.y / 2.0, size.z / 2.0);
        let bounds = terrain_fill_bounds(cframe, size);
        for x in bounds[0].0..=bounds[0].1 {
            for y in bounds[1].0..=bounds[1].1 {
                for z in bounds[2].0..=bounds[2].1 {
                    let cell = TerrainCell { x, y, z };
                    let occupancy = if terrain_rotation_is_axis_aligned(cframe.rotation) {
                        terrain_axis_aligned_overlap_fraction(cframe.position, size, cell)
                    } else {
                        // Parity note: Roblox supports rotated FillBlock. This sampled
                        // integration preserves channel semantics, but it is not the final
                        // analytic terrain voxelizer needed for exact rotated FillBlock parity.
                        terrain_sampled_overlap_fraction(cframe, half, cell)
                    };
                    if occupancy <= 0.0 {
                        continue;
                    }

                    if material == Material::Air {
                        self.voxels.remove(&cell);
                    } else if material == Material::Water {
                        self.voxels.insert(
                            cell,
                            TerrainVoxel {
                                solid_material: Material::Air,
                                solid_occupancy: 0.0,
                                liquid_occupancy: occupancy,
                            },
                        );
                    } else {
                        self.voxels.insert(
                            cell,
                            TerrainVoxel {
                                solid_material: material,
                                solid_occupancy: occupancy,
                                liquid_occupancy: 0.0,
                            },
                        );
                    }
                }
            }
        }
    }

    pub fn water_voxels(&self) -> impl Iterator<Item = (TerrainCell, TerrainVoxel)> + '_ {
        self.voxels
            .iter()
            .filter(|(_, voxel)| voxel.liquid_occupancy > 0.0)
            .map(|(cell, voxel)| (*cell, *voxel))
    }
}

fn terrain_fill_bounds(cframe: CFrame, size: Vector3) -> [(i32, i32); 3] {
    let radius = 0.5 * (size.x * size.x + size.y * size.y + size.z * size.z).sqrt();
    let min = Vector3::new(
        cframe.position.x - radius,
        cframe.position.y - radius,
        cframe.position.z - radius,
    );
    let max = Vector3::new(
        cframe.position.x + radius,
        cframe.position.y + radius,
        cframe.position.z + radius,
    );
    [
        terrain_axis_bounds(min.x, max.x),
        terrain_axis_bounds(min.y, max.y),
        terrain_axis_bounds(min.z, max.z),
    ]
}

fn terrain_axis_bounds(min: f32, max: f32) -> (i32, i32) {
    (
        (min / TERRAIN_VOXEL_SIZE).floor() as i32,
        (max / TERRAIN_VOXEL_SIZE).ceil() as i32 - 1,
    )
}

fn terrain_mat_t_mul_vec(m: &[[f32; 3]; 3], v: Vector3) -> Vector3 {
    Vector3::new(
        m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
        m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
        m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
    )
}

fn terrain_rotation_is_axis_aligned(rotation: [[f32; 3]; 3]) -> bool {
    let identity = CFrame::identity().rotation;
    rotation
        .iter()
        .flatten()
        .zip(identity.iter().flatten())
        .all(|(a, b)| (*a - *b).abs() < 1e-5)
}

fn terrain_axis_aligned_overlap_fraction(
    position: Vector3,
    size: Vector3,
    cell: TerrainCell,
) -> f32 {
    let block_min = Vector3::new(
        position.x - size.x / 2.0,
        position.y - size.y / 2.0,
        position.z - size.z / 2.0,
    );
    let block_max = Vector3::new(
        position.x + size.x / 2.0,
        position.y + size.y / 2.0,
        position.z + size.z / 2.0,
    );
    let cell_min = cell.min();
    let cell_max = cell.max();
    let overlap_x = (block_max.x.min(cell_max.x) - block_min.x.max(cell_min.x)).max(0.0);
    let overlap_y = (block_max.y.min(cell_max.y) - block_min.y.max(cell_min.y)).max(0.0);
    let overlap_z = (block_max.z.min(cell_max.z) - block_min.z.max(cell_min.z)).max(0.0);
    let voxel_volume = TERRAIN_VOXEL_SIZE * TERRAIN_VOXEL_SIZE * TERRAIN_VOXEL_SIZE;
    (overlap_x * overlap_y * overlap_z / voxel_volume).clamp(0.0, 1.0)
}

fn terrain_sampled_overlap_fraction(cframe: CFrame, half: Vector3, cell: TerrainCell) -> f32 {
    const SAMPLES_PER_AXIS: usize = 4;
    let cell_min = cell.min();
    let step = TERRAIN_VOXEL_SIZE / SAMPLES_PER_AXIS as f32;
    let mut inside = 0usize;
    for x in 0..SAMPLES_PER_AXIS {
        for y in 0..SAMPLES_PER_AXIS {
            for z in 0..SAMPLES_PER_AXIS {
                let world = Vector3::new(
                    cell_min.x + (x as f32 + 0.5) * step,
                    cell_min.y + (y as f32 + 0.5) * step,
                    cell_min.z + (z as f32 + 0.5) * step,
                );
                let relative = Vector3::new(
                    world.x - cframe.position.x,
                    world.y - cframe.position.y,
                    world.z - cframe.position.z,
                );
                let local = terrain_mat_t_mul_vec(&cframe.rotation, relative);
                if local.x.abs() <= half.x && local.y.abs() <= half.y && local.z.abs() <= half.z {
                    inside += 1;
                }
            }
        }
    }
    inside as f32 / (SAMPLES_PER_AXIS * SAMPLES_PER_AXIS * SAMPLES_PER_AXIS) as f32
}

#[derive(Debug, Clone)]
pub struct HumanoidData {
    pub health: f32,
    pub max_health: f32,
    pub walk_speed: f32,
    pub jump_power: f32,
    pub jump_height: f32,
    pub auto_rotate: bool,
    pub health_display_distance: f32,
    pub name_display_distance: f32,
    pub hip_height: f32,
    pub move_direction: Vector3,
    /// Input movement direction set by Humanoid:Move().
    pub move_input_direction: Vector3,
    pub running_speed: f32,
    pub state: HumanoidStateType,
    /// One-shot jump request consumed by game movement update
    pub jump_requested: bool,
    /// Movement target set by MoveTo()
    pub move_to_target: Option<Vector3>,
    /// Flag to cancel movement (set by CancelMoveTo)
    pub cancel_move_to: bool,

    pub died: RBXScriptSignal,
    pub health_changed: RBXScriptSignal,
    pub running: RBXScriptSignal,
    pub state_changed: RBXScriptSignal,
    pub move_to_finished: RBXScriptSignal,
}

impl Default for HumanoidData {
    fn default() -> Self {
        Self {
            health: humanoid_consts::DEFAULT_HEALTH,
            max_health: humanoid_consts::DEFAULT_MAX_HEALTH,
            walk_speed: humanoid_consts::DEFAULT_WALK_SPEED,
            jump_power: humanoid_consts::DEFAULT_JUMP_POWER,
            jump_height: humanoid_consts::DEFAULT_JUMP_HEIGHT,
            auto_rotate: true,
            health_display_distance: 100.0,
            name_display_distance: 100.0,
            hip_height: humanoid_consts::DEFAULT_HIP_HEIGHT,
            move_direction: Vector3::zero(),
            move_input_direction: Vector3::zero(),
            running_speed: 0.0,
            state: HumanoidStateType::None,
            jump_requested: false,
            move_to_target: None,
            cancel_move_to: false,
            died: create_signal("Died"),
            health_changed: create_signal("HealthChanged"),
            running: create_signal("Running"),
            state_changed: create_signal("StateChanged"),
            move_to_finished: create_signal("MoveToFinished"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerData {
    pub user_id: u64,
    pub display_name: String,
    pub character: Option<WeakInstanceRef>,
    pub player_gui: Option<WeakInstanceRef>,

    pub character_added: RBXScriptSignal,
    pub character_removing: RBXScriptSignal,
}

impl PlayerData {
    pub fn new(user_id: u64, name: &str) -> Self {
        Self {
            user_id,
            display_name: name.to_string(),
            character: None,
            player_gui: None,
            character_added: create_signal("CharacterAdded"),
            character_removing: create_signal("CharacterRemoving"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelData {
    pub primary_part: Option<WeakInstanceRef>,
    pub world_pivot: Option<CFrame>,
}

#[derive(Debug, Clone)]
pub struct AnimationData {
    pub animation_id: String,
}

impl Default for AnimationData {
    fn default() -> Self {
        Self {
            animation_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnimatorData {
    pub loaded_tracks: u32,
}

impl Default for ModelData {
    fn default() -> Self {
        Self {
            primary_part: None,
            world_pivot: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScriptData {
    pub source: String,
    pub disabled: bool,
}

impl Default for ScriptData {
    fn default() -> Self {
        Self {
            source: String::new(),
            disabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntValueData {
    pub value: i64,
    pub changed: RBXScriptSignal,
}

impl Default for IntValueData {
    fn default() -> Self {
        Self {
            value: 0,
            changed: create_signal("Changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NumberValueData {
    pub value: f64,
    pub changed: RBXScriptSignal,
}

impl Default for NumberValueData {
    fn default() -> Self {
        Self {
            value: 0.0,
            changed: create_signal("Changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoolValueData {
    pub value: bool,
    pub changed: RBXScriptSignal,
}

impl Default for BoolValueData {
    fn default() -> Self {
        Self {
            value: false,
            changed: create_signal("Changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StringValueData {
    pub value: String,
    pub changed: RBXScriptSignal,
}

impl Default for StringValueData {
    fn default() -> Self {
        Self {
            value: String::new(),
            changed: create_signal("Changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectValueData {
    pub value: Option<WeakInstanceRef>,
    pub changed: RBXScriptSignal,
}

impl Default for ObjectValueData {
    fn default() -> Self {
        Self {
            value: None,
            changed: create_signal("Changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CFrameValueData {
    pub value: CFrame,
    pub changed: RBXScriptSignal,
}

impl Default for CFrameValueData {
    fn default() -> Self {
        Self {
            value: CFrame::identity(),
            changed: create_signal("Changed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SoundData {
    pub stream_id: String,
    pub volume: f32,
    pub playback_speed: f32,
    pub is_playing: bool,
    pub ended: RBXScriptSignal,
    pub stopped: RBXScriptSignal,
}

impl Default for SoundData {
    fn default() -> Self {
        Self {
            stream_id: String::new(),
            volume: 0.5,
            playback_speed: 1.0,
            is_playing: false,
            ended: create_signal("Ended"),
            stopped: create_signal("Stopped"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteEventData {
    pub on_server_event: RBXScriptSignal,
}

#[derive(Debug, Clone)]
pub struct BodyVelocityData {
    pub velocity: Vector3,
    pub max_force: Vector3,
    pub p: f32,
}

impl Default for BodyVelocityData {
    fn default() -> Self {
        Self {
            velocity: Vector3::new(0.0, 2.0, 0.0),
            max_force: Vector3::new(4000.0, 4000.0, 4000.0),
            p: 1250.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BodyForceData {
    pub force: Vector3,
}

impl Default for BodyForceData {
    fn default() -> Self {
        Self {
            force: Vector3::new(0.0, 1.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BodyAngularVelocityData {
    pub angular_velocity: Vector3,
    pub max_torque: Vector3,
    pub p: f32,
}

impl Default for BodyAngularVelocityData {
    fn default() -> Self {
        Self {
            angular_velocity: Vector3::new(0.0, 2.0, 0.0),
            max_torque: Vector3::new(4000.0, 4000.0, 4000.0),
            p: 1250.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AttachmentData {
    pub cframe: CFrame,
}

impl Default for AttachmentData {
    fn default() -> Self {
        Self {
            cframe: CFrame::identity(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VectorForceData {
    pub attachment0: Option<WeakInstanceRef>,
    pub attachment1: Option<WeakInstanceRef>,
    pub force: Vector3,
    pub relative_to: ActuatorRelativeTo,
    pub apply_at_center_of_mass: bool,
    pub enabled: bool,
}

impl Default for VectorForceData {
    fn default() -> Self {
        Self {
            attachment0: None,
            attachment1: None,
            force: Vector3::new(1000.0, 0.0, 0.0),
            relative_to: ActuatorRelativeTo::Attachment0,
            apply_at_center_of_mass: false,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlignPositionData {
    pub attachment0: Option<WeakInstanceRef>,
    pub attachment1: Option<WeakInstanceRef>,
    pub position: Vector3,
    pub responsiveness: f32,
    pub rigidity_enabled: bool,
    pub max_force: f32,
    pub max_velocity: f32,
}

impl Default for AlignPositionData {
    fn default() -> Self {
        Self {
            attachment0: None,
            attachment1: None,
            position: Vector3::zero(),
            responsiveness: 10.0,
            rigidity_enabled: false,
            max_force: 10000.0,
            max_velocity: f32::INFINITY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlignOrientationData {
    pub attachment0: Option<WeakInstanceRef>,
    pub attachment1: Option<WeakInstanceRef>,
    pub responsiveness: f32,
    pub rigidity_enabled: bool,
    pub max_torque: Vector3,
}

impl Default for AlignOrientationData {
    fn default() -> Self {
        Self {
            attachment0: None,
            attachment1: None,
            responsiveness: 10.0,
            rigidity_enabled: false,
            max_torque: Vector3::new(10000.0, 10000.0, 10000.0),
        }
    }
}

impl Default for RemoteEventData {
    fn default() -> Self {
        Self {
            on_server_event: create_signal("OnServerEvent"),
        }
    }
}

/// Data for Weld constraints
#[derive(Debug, Clone)]
pub struct WeldData {
    pub part0: Option<WeakInstanceRef>,
    pub part1: Option<WeakInstanceRef>,
    pub c0: CFrame,
    pub c1: CFrame,
    pub enabled: bool,
}

impl Default for WeldData {
    fn default() -> Self {
        Self {
            part0: None,
            part1: None,
            c0: CFrame::identity(),
            c1: CFrame::identity(),
            enabled: true,
        }
    }
}

/// Data for WeldConstraint – locks Part0 and Part1 at their current relative transform.
#[derive(Debug, Clone)]
pub struct WeldConstraintData {
    pub part0: Option<WeakInstanceRef>,
    pub part1: Option<WeakInstanceRef>,
    pub enabled: bool,
    /// Cached relative transform (Part0:Inverse() * Part1) computed when both parts are set.
    pub relative_cframe: Option<CFrame>,
}

impl Default for WeldConstraintData {
    fn default() -> Self {
        Self {
            part0: None,
            part1: None,
            enabled: true,
            relative_cframe: None,
        }
    }
}

/// Data for BillboardGui (3D GUI that floats above parts)
#[derive(Debug, Clone)]
pub struct BillboardGuiData {
    pub size: UDim2,
    pub studs_offset: Vector3,
    pub always_on_top: bool,
    pub enabled: bool,
    pub adornee: Option<WeakInstanceRef>,
}

impl Default for BillboardGuiData {
    fn default() -> Self {
        Self {
            size: UDim2::new(0.0, 100, 0.0, 50),
            studs_offset: Vector3::new(0.0, 0.0, 0.0),
            always_on_top: false,
            enabled: true,
            adornee: None,
        }
    }
}

/// Data for UICorner
#[derive(Debug, Clone, Copy)]
pub struct UICornerData {
    pub corner_radius: UDim,
}

impl Default for UICornerData {
    fn default() -> Self {
        Self {
            corner_radius: UDim::new(0.0, 0),
        }
    }
}

/// Data for GUI objects (Frame, TextLabel, TextButton, etc.)
#[derive(Debug, Clone)]
pub struct GuiObjectData {
    // Layout properties
    pub position: UDim2,
    pub size: UDim2,
    pub anchor_point: (f32, f32), // Vector2 equivalent: 0-1 for X and Y
    pub rotation: f32,
    pub z_index: i32,
    pub layout_order: i32,
    pub visible: bool,

    // Appearance
    pub background_color: Color3,
    pub background_transparency: f32,
    pub border_color: Color3,
    pub border_size_pixel: i32,

    // Text properties (for TextLabel, TextButton)
    pub text: Option<String>,
    pub text_color: Option<Color3>,
    pub text_size: Option<f32>,
    pub text_transparency: Option<f32>,
    pub text_stroke_transparency: Option<f32>,
    pub font: Option<String>,
    pub text_scaled: bool,
    pub text_x_alignment: TextXAlignment,
    pub text_y_alignment: TextYAlignment,

    // Image properties (for ImageLabel, ImageButton)
    pub image: Option<String>,
    pub image_color: Option<Color3>,
    pub image_transparency: Option<f32>,

    // ScreenGui-specific
    pub display_order: i32,
    pub ignore_gui_inset: bool,
    pub enabled: bool,
    pub reset_on_spawn: bool,

    // Events (for GuiButton types)
    pub mouse_button1_click: Option<RBXScriptSignal>,
    pub mouse_button1_down: Option<RBXScriptSignal>,
    pub mouse_button1_up: Option<RBXScriptSignal>,
    pub mouse_enter: Option<RBXScriptSignal>,
    pub mouse_leave: Option<RBXScriptSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextXAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextYAlignment {
    Top,
    #[default]
    Center,
    Bottom,
}

impl Default for GuiObjectData {
    fn default() -> Self {
        Self {
            position: UDim2::new(0.0, 0, 0.0, 0),
            size: UDim2::new(0.0, 100, 0.0, 100),
            anchor_point: (0.0, 0.0),
            rotation: 0.0,
            z_index: 1,
            layout_order: 0,
            visible: true,
            background_color: Color3::new(1.0, 1.0, 1.0),
            background_transparency: 0.0,
            border_color: Color3::new(0.1, 0.1, 0.1),
            border_size_pixel: 1,
            text: None,
            text_color: None,
            text_size: None,
            text_transparency: None,
            text_stroke_transparency: None,
            font: None,
            text_scaled: false,
            text_x_alignment: TextXAlignment::default(),
            text_y_alignment: TextYAlignment::default(),
            image: None,
            image_color: None,
            image_transparency: None,
            display_order: 0,
            ignore_gui_inset: false,
            enabled: true,
            reset_on_spawn: true,
            mouse_button1_click: None,
            mouse_button1_down: None,
            mouse_button1_up: None,
            mouse_enter: None,
            mouse_leave: None,
        }
    }
}

impl GuiObjectData {
    /// Create data for a Frame
    pub fn new_frame() -> Self {
        Self::default()
    }

    /// Create data for a TextLabel
    pub fn new_text_label() -> Self {
        Self {
            text: Some(String::new()),
            text_color: Some(Color3::new(0.0, 0.0, 0.0)),
            text_size: Some(14.0),
            text_transparency: Some(0.0),
            text_stroke_transparency: Some(1.0),
            font: Some("SourceSans".to_string()),
            background_transparency: 1.0, // TextLabels default to transparent background
            ..Self::default()
        }
    }

    /// Create data for a TextButton
    pub fn new_text_button() -> Self {
        Self {
            text: Some(String::new()),
            text_color: Some(Color3::new(0.0, 0.0, 0.0)),
            text_size: Some(14.0),
            text_transparency: Some(0.0),
            text_stroke_transparency: Some(1.0),
            font: Some("SourceSans".to_string()),
            mouse_button1_click: Some(create_signal("MouseButton1Click")),
            mouse_button1_down: Some(create_signal("MouseButton1Down")),
            mouse_button1_up: Some(create_signal("MouseButton1Up")),
            mouse_enter: Some(create_signal("MouseEnter")),
            mouse_leave: Some(create_signal("MouseLeave")),
            ..Self::default()
        }
    }

    /// Create data for an ImageLabel
    pub fn new_image_label() -> Self {
        Self {
            image: Some(String::new()),
            image_color: Some(Color3::new(1.0, 1.0, 1.0)),
            image_transparency: Some(0.0),
            background_transparency: 1.0,
            ..Self::default()
        }
    }

    /// Create data for an ImageButton
    pub fn new_image_button() -> Self {
        Self {
            image: Some(String::new()),
            image_color: Some(Color3::new(1.0, 1.0, 1.0)),
            image_transparency: Some(0.0),
            mouse_button1_click: Some(create_signal("MouseButton1Click")),
            mouse_button1_down: Some(create_signal("MouseButton1Down")),
            mouse_button1_up: Some(create_signal("MouseButton1Up")),
            mouse_enter: Some(create_signal("MouseEnter")),
            mouse_leave: Some(create_signal("MouseLeave")),
            ..Self::default()
        }
    }

    /// Create data for a ScreenGui
    pub fn new_screen_gui() -> Self {
        Self {
            enabled: true,
            display_order: 0,
            ignore_gui_inset: false,
            reset_on_spawn: true,
            background_transparency: 1.0, // ScreenGui is fully transparent
            ..Self::default()
        }
    }
}

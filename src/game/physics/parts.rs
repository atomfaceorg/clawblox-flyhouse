use nalgebra::{Translation3, UnitQuaternion};
use rapier3d::prelude::*;

use super::{PartInteractionState, PhysicsWorld};
use crate::game::lua::services::physics_service::CollisionGroupMask;
use crate::game::lua::types::{PartType, PhysicalProperties};

fn interaction_groups(state: PartInteractionState) -> InteractionGroups {
    InteractionGroups::new(
        Group::from(state.collision_group.membership),
        Group::from(state.collision_group.filter),
    )
}

fn build_ring_shared_shape(size: [f32; 3]) -> SharedShape {
    let [sx, sy, sz] = size;
    let outer_radius = 0.5 * sx.min(sz);
    let tube_radius = (sy * 0.5).min(outer_radius * 0.45).max(0.05);
    let major_radius = (outer_radius - tube_radius).max(0.05);
    let segments = 12usize;
    let segment_length = (2.0 * major_radius * (std::f32::consts::PI / segments as f32).tan())
        .max(tube_radius * 1.5)
        * 1.08;
    let segment_half = [segment_length * 0.5, tube_radius, tube_radius];
    let cuboid = SharedShape::cuboid(segment_half[0], segment_half[1], segment_half[2]);
    let mut pieces = Vec::with_capacity(segments);
    for index in 0..segments {
        let angle = (index as f32 / segments as f32) * std::f32::consts::TAU;
        let center_x = angle.cos() * major_radius;
        let center_z = angle.sin() * major_radius;
        let rotation =
            UnitQuaternion::from_euler_angles(0.0, angle + std::f32::consts::FRAC_PI_2, 0.0);
        let transform = Isometry::from_parts(Translation3::new(center_x, 0.0, center_z), rotation);
        pieces.push((transform, cuboid.clone()));
    }
    SharedShape::compound(pieces)
}

fn build_shared_shape(size: [f32; 3], shape: PartType) -> SharedShape {
    let [sx, sy, sz] = size;
    match shape {
        PartType::Block => SharedShape::cuboid(sx / 2.0, sy / 2.0, sz / 2.0),
        PartType::Ball => SharedShape::ball(sx / 2.0),
        PartType::Cylinder => SharedShape::cylinder(sy / 2.0, sx / 2.0),
        PartType::Ring => build_ring_shared_shape(size),
        PartType::Wedge => {
            // Triangular prism: flat bottom, slope rises from +X to -X
            // 6 vertices matching Roblox wedge convention
            let hx = sx / 2.0;
            let hy = sy / 2.0;
            let hz = sz / 2.0;
            let points = [
                point![-hx, -hy, -hz], // bottom-left-back
                point![hx, -hy, -hz],  // bottom-right-back
                point![-hx, -hy, hz],  // bottom-left-front
                point![hx, -hy, hz],   // bottom-right-front
                point![-hx, hy, -hz],  // top-left-back
                point![-hx, hy, hz],   // top-left-front
            ];
            SharedShape::convex_hull(&points)
                .expect("Wedge convex hull should always succeed with 6 valid vertices")
        }
    }
}

fn build_collider(
    shared_shape: SharedShape,
    can_collide: bool,
    interaction_state: PartInteractionState,
) -> Collider {
    ColliderBuilder::new(shared_shape)
        .enabled(interaction_state.collider_enabled())
        .sensor(!can_collide)
        .active_events(if interaction_state.can_touch {
            ActiveEvents::COLLISION_EVENTS
        } else {
            ActiveEvents::empty()
        })
        .active_collision_types(if interaction_state.can_touch {
            ActiveCollisionTypes::all()
        } else {
            ActiveCollisionTypes::default()
        })
        .collision_groups(interaction_groups(interaction_state))
        .solver_groups(interaction_groups(interaction_state))
        .build()
}

pub(super) fn build_collider_with_physical_properties(
    size: [f32; 3],
    shape: PartType,
    interaction_state: PartInteractionState,
    physical_properties: Option<PhysicalProperties>,
) -> Collider {
    let props = physical_properties.unwrap_or(PhysicalProperties::new(0.7, 0.3, 0.5, 1.0, 1.0));
    let mut collider = build_collider(
        build_shared_shape(size, shape),
        interaction_state.can_collide,
        interaction_state,
    );
    collider.set_friction(props.friction.max(0.0));
    collider.set_restitution(props.elasticity.max(0.0));
    collider.set_density(props.density.max(0.0));
    collider
}

/// Builds a collider from a static triangle mesh (see `collision_mesh`).
/// Triangle meshes are only valid on anchored (kinematic) parts.
pub(super) fn build_mesh_collider_with_physical_properties(
    mesh_shape: SharedShape,
    interaction_state: PartInteractionState,
    physical_properties: Option<PhysicalProperties>,
) -> Collider {
    let props = physical_properties.unwrap_or(PhysicalProperties::new(0.7, 0.3, 0.5, 1.0, 1.0));
    let mut collider = build_collider(mesh_shape, interaction_state.can_collide, interaction_state);
    collider.set_friction(props.friction.max(0.0));
    collider.set_restitution(props.elasticity.max(0.0));
    collider
}

impl PhysicsWorld {
    /// Updates the size of a part's collider
    pub fn set_size(&mut self, lua_id: u64, size: [f32; 3]) {
        let shape = self
            .lua_to_shape
            .get(&lua_id)
            .copied()
            .unwrap_or(PartType::Block);
        let physical_properties = self
            .lua_to_physical_properties
            .get(&lua_id)
            .copied()
            .unwrap_or(None);
        let interaction_state =
            self.lua_to_interactions
                .get(&lua_id)
                .copied()
                .unwrap_or(PartInteractionState {
                    can_collide: true,
                    can_touch: true,
                    can_query: true,
                    collision_group: CollisionGroupMask::default(),
                });
        if let (Some(&handle), Some(&old_collider)) = (
            self.lua_to_body.get(&lua_id),
            self.lua_to_collider.get(&lua_id),
        ) {
            let local_frame = self
                .lua_to_local_frame
                .get(&lua_id)
                .copied()
                .unwrap_or_else(Isometry::identity);
            self.collider_to_lua.remove(&old_collider);
            self.collider_set.remove(
                old_collider,
                &mut self.island_manager,
                &mut self.rigid_body_set,
                true,
            );
            let mut collider = build_collider_with_physical_properties(
                size,
                shape,
                interaction_state,
                physical_properties,
            );
            collider.set_position(local_frame);
            let new_ch =
                self.collider_set
                    .insert_with_parent(collider, handle, &mut self.rigid_body_set);
            self.collider_to_lua.insert(new_ch, lua_id);
            self.lua_to_collider.insert(lua_id, new_ch);
        }
    }

    /// Toggles collider sensor mode (can_collide=false -> sensor, can_collide=true -> solid)
    pub fn set_can_collide(&mut self, lua_id: u64, can_collide: bool) {
        let mut state =
            self.lua_to_interactions
                .get(&lua_id)
                .copied()
                .unwrap_or(PartInteractionState {
                    can_collide,
                    can_touch: true,
                    can_query: true,
                    collision_group: CollisionGroupMask::default(),
                });
        state.can_collide = can_collide;
        self.set_interaction_state(lua_id, state);
    }

    pub fn set_can_touch(&mut self, lua_id: u64, can_touch: bool) {
        let mut state =
            self.lua_to_interactions
                .get(&lua_id)
                .copied()
                .unwrap_or(PartInteractionState {
                    can_collide: true,
                    can_touch,
                    can_query: true,
                    collision_group: CollisionGroupMask::default(),
                });
        state.can_touch = can_touch;
        self.set_interaction_state(lua_id, state);
    }

    pub fn set_can_query(&mut self, lua_id: u64, can_query: bool) {
        let mut state =
            self.lua_to_interactions
                .get(&lua_id)
                .copied()
                .unwrap_or(PartInteractionState {
                    can_collide: true,
                    can_touch: true,
                    can_query,
                    collision_group: CollisionGroupMask::default(),
                });
        state.can_query = can_query;
        self.set_interaction_state(lua_id, state);
    }

    pub(super) fn set_interaction_state(&mut self, lua_id: u64, state: PartInteractionState) {
        self.lua_to_interactions.insert(lua_id, state);
        if let Some(&collider_handle) = self.lua_to_collider.get(&lua_id) {
            if let Some(collider) = self.collider_set.get_mut(collider_handle) {
                collider.set_sensor(!state.can_collide);
                collider.set_enabled(state.collider_enabled());
                collider.set_active_events(if state.can_touch {
                    ActiveEvents::COLLISION_EVENTS
                } else {
                    ActiveEvents::empty()
                });
                collider.set_active_collision_types(if state.can_touch {
                    ActiveCollisionTypes::all()
                } else {
                    ActiveCollisionTypes::default()
                });
                let groups = interaction_groups(state);
                collider.set_collision_groups(groups);
                collider.set_solver_groups(groups);
            }
        }
    }

    /// Updates the shape of a part's collider, rebuilding it with the current size
    pub fn set_shape(&mut self, lua_id: u64, shape: PartType, size: [f32; 3]) {
        self.lua_to_shape.insert(lua_id, shape);
        let physical_properties = self
            .lua_to_physical_properties
            .get(&lua_id)
            .copied()
            .unwrap_or(None);
        let interaction_state =
            self.lua_to_interactions
                .get(&lua_id)
                .copied()
                .unwrap_or(PartInteractionState {
                    can_collide: true,
                    can_touch: true,
                    can_query: true,
                    collision_group: CollisionGroupMask::default(),
                });
        if let (Some(&handle), Some(&old_collider)) = (
            self.lua_to_body.get(&lua_id),
            self.lua_to_collider.get(&lua_id),
        ) {
            let local_frame = self
                .lua_to_local_frame
                .get(&lua_id)
                .copied()
                .unwrap_or_else(Isometry::identity);
            self.collider_to_lua.remove(&old_collider);
            self.collider_set.remove(
                old_collider,
                &mut self.island_manager,
                &mut self.rigid_body_set,
                true,
            );
            let mut collider = build_collider_with_physical_properties(
                size,
                shape,
                interaction_state,
                physical_properties,
            );
            collider.set_position(local_frame);
            let new_ch =
                self.collider_set
                    .insert_with_parent(collider, handle, &mut self.rigid_body_set);
            self.collider_to_lua.insert(new_ch, lua_id);
            self.lua_to_collider.insert(lua_id, new_ch);
        }
    }

    /// Updates collider physical properties for a Lua part.
    pub fn set_physical_properties(
        &mut self,
        lua_id: u64,
        physical_properties: Option<PhysicalProperties>,
    ) {
        self.lua_to_physical_properties
            .insert(lua_id, physical_properties);
        if let Some(&collider_handle) = self.lua_to_collider.get(&lua_id) {
            if let Some(collider) = self.collider_set.get_mut(collider_handle) {
                let props =
                    physical_properties.unwrap_or(PhysicalProperties::new(0.7, 0.3, 0.5, 1.0, 1.0));
                collider.set_friction(props.friction.max(0.0));
                collider.set_restitution(props.elasticity.max(0.0));
                collider.set_density(props.density.max(0.0));
            }
        }
    }

    #[cfg(test)]
    pub(super) fn collider_enabled_for_lua(&self, lua_id: u64) -> Option<bool> {
        let handle = *self.lua_to_body.get(&lua_id)?;
        let _body = self.rigid_body_set.get(handle)?;
        let collider_handle = *self.lua_to_collider.get(&lua_id)?;
        Some(self.collider_set.get(collider_handle)?.is_enabled())
    }
}

use mlua::{AnyUserData, UserDataFields};

use crate::game::lua::runtime::Game;
use crate::game::lua::types::{Color3, Material, PartType, PhysicalProperties, Vector3};

use super::super::super::*;

pub(super) fn add_fields<F: UserDataFields<Instance>>(fields: &mut F) {
    fields.add_field_method_get("Anchored", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.anchored))
    });
    fields.add_field_method_set("Anchored", |lua, this, anchored: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.anchored = anchored;
            part.anchored_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("CanCollide", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.can_collide))
    });
    fields.add_field_method_set("CanCollide", |lua, this, can_collide: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.can_collide = can_collide;
            part.can_collide_dirty = true;
            part.interaction_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("Massless", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.massless))
    });
    fields.add_field_method_set("Massless", |lua, this, massless: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.massless = massless;
            part.custom_physical_properties_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("EnableFluidForces", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.enable_fluid_forces))
    });
    fields.add_field_method_set(
        "EnableFluidForces",
        |lua, this, enable_fluid_forces: bool| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.enable_fluid_forces = enable_fluid_forces;
                drop(data);
                this.mark_replication_dirty(lua);
            }
            Ok(())
        },
    );

    fields.add_field_method_get("CanQuery", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.can_query))
    });
    fields.add_field_method_set("CanQuery", |lua, this, can_query: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.can_query = can_query;
            part.interaction_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("CollisionGroup", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.collision_group.clone()))
    });
    fields.add_field_method_set("CollisionGroup", |lua, this, collision_group: String| {
        let group_id = {
            let game_ud: AnyUserData = lua.globals().get("__clawblox_game")?;
            let game = game_ud.borrow::<Game>()?;
            game.physics_service()
                .resolve_collision_group_id(&collision_group)?
        };
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.collision_group = collision_group;
            part.collision_group_id = group_id;
            part.interaction_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("CanTouch", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.can_touch))
    });
    fields.add_field_method_set("CanTouch", |lua, this, can_touch: bool| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.can_touch = can_touch;
            part.interaction_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("Transparency", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.transparency))
    });
    fields.add_field_method_set("Transparency", |lua, this, transparency: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.transparency = transparency.clamp(0.0, 1.0);
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("Color", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.color))
    });
    fields.add_field_method_set("Color", |lua, this, color: Color3| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.color = color;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("Material", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.material))
    });
    fields.add_field_method_set("Material", |lua, this, material: Material| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.material = material;
            // Keep CurrentPhysicalProperties in sync with material defaults when not overridden.
            if part.custom_physical_properties.is_none() {
                part.custom_physical_properties_dirty = true;
            }
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("TopSurface", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.top_surface.clone()))
    });
    fields.add_field_method_set("TopSurface", |lua, this, surface: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.top_surface = surface;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("BottomSurface", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.bottom_surface.clone()))
    });
    fields.add_field_method_set("BottomSurface", |lua, this, surface: String| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.bottom_surface = surface;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("BrickColor", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.color))
    });
    fields.add_field_method_set("BrickColor", |lua, this, color: Color3| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.color = color;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("Velocity", |_, this| {
        let data = this.data.lock().unwrap();
        if let Some(bv) = &data.body_velocity_data {
            Ok(Some(bv.velocity))
        } else {
            Ok(data.part_data.as_ref().map(|p| p.velocity))
        }
    });
    fields.add_field_method_set("Velocity", |lua, this, velocity: Vector3| {
        let mut data = this.data.lock().unwrap();
        if let Some(bv) = &mut data.body_velocity_data {
            bv.velocity = velocity;
            drop(data);
            this.mark_replication_dirty(lua);
        } else if let Some(part) = &mut data.part_data {
            part.velocity = velocity;
            part.velocity_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("CustomPhysicalProperties", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .and_then(|p| p.custom_physical_properties))
    });
    fields.add_field_method_get("CurrentPhysicalProperties", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.current_physical_properties()))
    });
    fields.add_field_method_set(
        "CustomPhysicalProperties",
        |lua, this, value: Option<PhysicalProperties>| {
            let mut data = this.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.custom_physical_properties = value;
                part.custom_physical_properties_dirty = true;
                drop(data);
                this.mark_replication_dirty(lua);
            }
            Ok(())
        },
    );
    fields.add_field_method_get("Density", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.current_physical_properties().density))
    });
    fields.add_field_method_get("SpecificGravity", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.specific_gravity()))
    });
    fields.add_field_method_set("Density", |lua, this, density: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            let props = part.current_physical_properties();
            part.custom_physical_properties = Some(PhysicalProperties::new(
                density,
                props.friction,
                props.elasticity,
                props.friction_weight,
                props.elasticity_weight,
            ));
            part.custom_physical_properties_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("Friction", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.current_physical_properties().friction))
    });
    fields.add_field_method_set("Friction", |lua, this, friction: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            let props = part.current_physical_properties();
            part.custom_physical_properties = Some(PhysicalProperties::new(
                props.density,
                friction,
                props.elasticity,
                props.friction_weight,
                props.elasticity_weight,
            ));
            part.custom_physical_properties_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("Elasticity", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.current_physical_properties().elasticity))
    });
    fields.add_field_method_set("Elasticity", |lua, this, elasticity: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            let props = part.current_physical_properties();
            part.custom_physical_properties = Some(PhysicalProperties::new(
                props.density,
                props.friction,
                elasticity,
                props.friction_weight,
                props.elasticity_weight,
            ));
            part.custom_physical_properties_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("FrictionWeight", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.current_physical_properties().friction_weight))
    });
    fields.add_field_method_set("FrictionWeight", |lua, this, friction_weight: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            let props = part.current_physical_properties();
            part.custom_physical_properties = Some(PhysicalProperties::new(
                props.density,
                props.friction,
                props.elasticity,
                friction_weight,
                props.elasticity_weight,
            ));
            part.custom_physical_properties_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });
    fields.add_field_method_get("ElasticityWeight", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.current_physical_properties().elasticity_weight))
    });
    fields.add_field_method_set("ElasticityWeight", |lua, this, elasticity_weight: f32| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            let props = part.current_physical_properties();
            part.custom_physical_properties = Some(PhysicalProperties::new(
                props.density,
                props.friction,
                props.elasticity,
                props.friction_weight,
                elasticity_weight,
            ));
            part.custom_physical_properties_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("AssemblyLinearVelocity", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.velocity))
    });
    fields.add_field_method_set("AssemblyLinearVelocity", |lua, this, velocity: Vector3| {
        for assembly_part in this.assembly_parts_from_graph() {
            let mut data = assembly_part.data.lock().unwrap();
            if let Some(part) = &mut data.part_data {
                part.velocity = velocity;
                part.velocity_dirty = true;
            }
        }
        this.mark_replication_dirty(lua);
        Ok(())
    });
    fields.add_field_method_get("AssemblyAngularVelocity", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.angular_velocity))
    });
    fields.add_field_method_set(
        "AssemblyAngularVelocity",
        |lua, this, angular_velocity: Vector3| {
            for assembly_part in this.assembly_parts_from_graph() {
                let mut data = assembly_part.data.lock().unwrap();
                if let Some(part) = &mut data.part_data {
                    part.angular_velocity = angular_velocity;
                    part.angular_velocity_dirty = true;
                }
            }
            this.mark_replication_dirty(lua);
            Ok(())
        },
    );

    fields.add_field_method_get("Shape", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.shape))
    });
    fields.add_field_method_set("Shape", |lua, this, shape: PartType| {
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.shape = shape;
            part.shape_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("MeshId", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data
            .part_data
            .as_ref()
            .map(|p| p.mesh_id.clone().unwrap_or_default()))
    });
    fields.add_field_method_set("MeshId", |lua, this, mesh_id: String| {
        let mesh_id = if mesh_id.is_empty() {
            None
        } else {
            if crate::game::collision_mesh::get(&mesh_id).is_none() {
                return Err(mlua::Error::RuntimeError(format!(
                    "MeshId '{mesh_id}' is not a loaded collision mesh"
                )));
            }
            Some(mesh_id)
        };
        let mut data = this.data.lock().unwrap();
        if let Some(part) = &mut data.part_data {
            part.mesh_id = mesh_id;
            part.shape_dirty = true;
            drop(data);
            this.mark_replication_dirty(lua);
        }
        Ok(())
    });

    fields.add_field_method_get("Touched", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.touched.clone()))
    });

    fields.add_field_method_get("TouchEnded", |_, this| {
        let data = this.data.lock().unwrap();
        Ok(data.part_data.as_ref().map(|p| p.touch_ended.clone()))
    });
}

use mlua::{UserData, UserDataFields, UserDataMethods};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::game::lua::instance::Instance;

pub const MAX_COLLISION_GROUPS: usize = 32;
const DEFAULT_COLLISION_GROUP: &str = "Default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionGroupMask {
    pub membership: u32,
    pub filter: u32,
}

impl Default for CollisionGroupMask {
    fn default() -> Self {
        Self {
            membership: 1,
            filter: u32::MAX,
        }
    }
}

/// Roblox-compatible PhysicsService surface API.
///
/// Clawblox tracks Roblox's 32 collision groups and pairwise collidable flags.
/// The physics sync path applies these masks to Rapier collider interaction and
/// solver groups so the matrix affects real contacts, not only scripts.
#[derive(Default)]
struct PhysicsServiceData {
    registered_groups: HashSet<String>,
    group_indices: HashMap<String, u8>,
    explicit_pair_collidable: HashMap<(u8, u8), bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionGroupRegistryEntry {
    pub name: String,
    pub id: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionGroupPairEntry {
    pub group1_id: u8,
    pub group2_id: u8,
    pub collidable: bool,
}

#[derive(Clone, Default)]
pub struct PhysicsService {
    data: Arc<Mutex<PhysicsServiceData>>,
}

impl PhysicsService {
    pub fn new() -> Self {
        let mut data = PhysicsServiceData::default();
        data.registered_groups
            .insert(DEFAULT_COLLISION_GROUP.to_string());
        data.group_indices
            .insert(DEFAULT_COLLISION_GROUP.to_string(), 0);
        Self {
            data: Arc::new(Mutex::new(data)),
        }
    }

    fn canonical_pair(group1: u8, group2: u8) -> (u8, u8) {
        if group1 <= group2 {
            (group1, group2)
        } else {
            (group2, group1)
        }
    }

    fn register_group(data: &mut PhysicsServiceData, group: &str) -> mlua::Result<()> {
        if group.is_empty() || group == DEFAULT_COLLISION_GROUP {
            return Err(mlua::Error::runtime(format!(
                "invalid collision group name '{group}'"
            )));
        }
        if data.registered_groups.contains(group) {
            return Ok(());
        }
        if data.group_indices.len() >= MAX_COLLISION_GROUPS {
            return Err(mlua::Error::runtime(format!(
                "cannot register more than {MAX_COLLISION_GROUPS} collision groups"
            )));
        }
        let next_index = (0..MAX_COLLISION_GROUPS)
            .find(|index| !data.group_indices.values().any(|&v| v as usize == *index))
            .expect("available collision group index");
        data.registered_groups.insert(group.to_string());
        data.group_indices
            .insert(group.to_string(), next_index as u8);
        Ok(())
    }

    fn ensure_registered_group(&self, group: &str) -> mlua::Result<()> {
        let mut data = self.data.lock().unwrap();
        Self::register_group(&mut data, group)
    }

    fn require_registered_group(data: &PhysicsServiceData, group: &str) -> mlua::Result<()> {
        if data.registered_groups.contains(group) {
            Ok(())
        } else {
            Err(mlua::Error::runtime(format!(
                "collision group '{group}' is not registered"
            )))
        }
    }

    pub fn is_collision_group_registered(&self, group: &str) -> bool {
        self.data.lock().unwrap().registered_groups.contains(group)
    }

    pub fn resolve_collision_group_id(&self, group: &str) -> mlua::Result<u8> {
        let data = self.data.lock().unwrap();
        Self::group_id(&data, group)
    }

    fn group_id(data: &PhysicsServiceData, group: &str) -> mlua::Result<u8> {
        data.group_indices.get(group).copied().ok_or_else(|| {
            mlua::Error::runtime(format!("collision group '{group}' is not registered"))
        })
    }

    fn group_name(data: &PhysicsServiceData, id: u8) -> Option<String> {
        data.group_indices
            .iter()
            .find_map(|(name, &index)| (index == id).then(|| name.clone()))
    }

    fn registered_group_snapshot(&self) -> Vec<(String, u8, u32)> {
        let data = self.data.lock().unwrap();
        let mut groups = data
            .group_indices
            .iter()
            .map(|(name, &index)| {
                let mut filter = 0u32;
                for &other_index in data.group_indices.values() {
                    let key = Self::canonical_pair(index, other_index);
                    if data
                        .explicit_pair_collidable
                        .get(&key)
                        .copied()
                        .unwrap_or(true)
                    {
                        filter |= 1u32 << other_index;
                    }
                }
                (name.clone(), index, filter)
            })
            .collect::<Vec<_>>();
        groups.sort_by_key(|(_, index, _)| *index);
        groups
    }

    fn unregister_group(data: &mut PhysicsServiceData, group: &str) -> mlua::Result<()> {
        if group == DEFAULT_COLLISION_GROUP {
            return Err(mlua::Error::runtime(
                "cannot unregister collision group 'Default'",
            ));
        }
        let Some(removed_id) = data.group_indices.get(group).copied() else {
            return Ok(());
        };
        data.registered_groups.remove(group);
        data.group_indices.remove(group);
        data.explicit_pair_collidable
            .retain(|(a, b), _| *a != removed_id && *b != removed_id);
        Ok(())
    }

    fn rename_group(data: &mut PhysicsServiceData, from: &str, to: &str) -> mlua::Result<()> {
        if from.is_empty()
            || to.is_empty()
            || from == DEFAULT_COLLISION_GROUP
            || to == DEFAULT_COLLISION_GROUP
        {
            return Err(mlua::Error::runtime("invalid collision group name"));
        }
        if !data.registered_groups.contains(from) {
            return Ok(());
        }
        if data.registered_groups.contains(to) {
            return Err(mlua::Error::runtime(format!(
                "collision group '{to}' is already registered"
            )));
        }

        let index = data
            .group_indices
            .remove(from)
            .expect("registered group should have an index");
        data.registered_groups.remove(from);
        data.registered_groups.insert(to.to_string());
        data.group_indices.insert(to.to_string(), index);

        Ok(())
    }

    pub fn groups_collidable(&self, group1: &str, group2: &str) -> bool {
        let data = self.data.lock().unwrap();
        let (Ok(group1), Ok(group2)) =
            (Self::group_id(&data, group1), Self::group_id(&data, group2))
        else {
            return true;
        };
        Self::groups_collidable_ids_locked(&data, group1, group2)
    }

    fn groups_collidable_ids_locked(data: &PhysicsServiceData, group1: u8, group2: u8) -> bool {
        let key = Self::canonical_pair(group1, group2);
        data.explicit_pair_collidable
            .get(&key)
            .copied()
            .unwrap_or(true)
    }

    pub fn groups_collidable_ids(&self, group1: u8, group2: u8) -> bool {
        let data = self.data.lock().unwrap();
        Self::groups_collidable_ids_locked(&data, group1, group2)
    }

    pub fn collision_group_mask(&self, group: &str) -> CollisionGroupMask {
        let data = self.data.lock().unwrap();
        let Some(&index) = data.group_indices.get(group) else {
            return CollisionGroupMask::default();
        };
        Self::collision_group_mask_locked(&data, index)
    }

    fn collision_group_mask_locked(data: &PhysicsServiceData, index: u8) -> CollisionGroupMask {
        let membership = 1u32 << index;
        let mut filter = 0u32;
        if !data.group_indices.values().any(|&id| id == index) {
            return CollisionGroupMask::default();
        }
        for &other_index in data.group_indices.values() {
            if Self::groups_collidable_ids_locked(data, index, other_index) {
                filter |= 1u32 << other_index;
            }
        }
        CollisionGroupMask { membership, filter }
    }

    pub fn collision_group_mask_by_id(&self, group_id: u8) -> CollisionGroupMask {
        let data = self.data.lock().unwrap();
        Self::collision_group_mask_locked(&data, group_id)
    }

    pub fn snapshot_registry(&self) -> Vec<CollisionGroupRegistryEntry> {
        let data = self.data.lock().unwrap();
        let mut groups = data
            .group_indices
            .iter()
            .map(|(name, &id)| CollisionGroupRegistryEntry {
                name: name.clone(),
                id,
            })
            .collect::<Vec<_>>();
        groups.sort_by_key(|group| group.id);
        groups
    }

    pub fn snapshot_pair_collidability(&self) -> Vec<CollisionGroupPairEntry> {
        let data = self.data.lock().unwrap();
        let mut pairs = data
            .explicit_pair_collidable
            .iter()
            .map(
                |(&(group1_id, group2_id), &collidable)| CollisionGroupPairEntry {
                    group1_id,
                    group2_id,
                    collidable,
                },
            )
            .collect::<Vec<_>>();
        pairs.sort_by_key(|pair| (pair.group1_id, pair.group2_id));
        pairs
    }

    pub fn restore_registry(
        &self,
        groups: &[CollisionGroupRegistryEntry],
        pair_collidability: &[CollisionGroupPairEntry],
    ) -> mlua::Result<()> {
        let mut group_indices = HashMap::new();
        let mut registered_groups = HashSet::new();
        for group in groups {
            if group.name.is_empty() || group.id as usize >= MAX_COLLISION_GROUPS {
                return Err(mlua::Error::runtime("invalid collision group snapshot"));
            }
            if registered_groups.contains(&group.name)
                || group_indices.values().any(|&id| id == group.id)
            {
                return Err(mlua::Error::runtime(
                    "duplicate collision group in snapshot",
                ));
            }
            registered_groups.insert(group.name.clone());
            group_indices.insert(group.name.clone(), group.id);
        }
        if group_indices.get(DEFAULT_COLLISION_GROUP).copied() != Some(0) {
            return Err(mlua::Error::runtime(
                "collision group snapshot is missing Default group id 0",
            ));
        }

        let mut explicit_pair_collidable = HashMap::new();
        for pair in pair_collidability {
            if !group_indices.values().any(|&id| id == pair.group1_id)
                || !group_indices.values().any(|&id| id == pair.group2_id)
            {
                return Err(mlua::Error::runtime(
                    "collision group pair references an unregistered group id",
                ));
            }
            let key = Self::canonical_pair(pair.group1_id, pair.group2_id);
            explicit_pair_collidable.insert(key, pair.collidable);
        }

        let mut data = self.data.lock().unwrap();
        data.registered_groups = registered_groups;
        data.group_indices = group_indices;
        data.explicit_pair_collidable = explicit_pair_collidable;
        Ok(())
    }
}

impl UserData for PhysicsService {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Name", |_, _| Ok("PhysicsService".to_string()));
        fields.add_field_method_get("ClassName", |_, _| Ok("PhysicsService".to_string()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("RegisterCollisionGroup", |_, this, name: String| {
            this.ensure_registered_group(&name)?;
            Ok(())
        });
        methods.add_method("CreateCollisionGroup", |_, this, name: String| {
            this.ensure_registered_group(&name)?;
            let data = this.data.lock().unwrap();
            Ok(PhysicsService::group_id(&data, &name)? as i32)
        });
        methods.add_method("GetMaxCollisionGroups", |_, _, ()| {
            Ok(MAX_COLLISION_GROUPS as i32)
        });
        methods.add_method("IsCollisionGroupRegistered", |_, this, name: String| {
            Ok(this.is_collision_group_registered(&name))
        });
        methods.add_method("GetCollisionGroupId", |_, this, name: String| {
            let data = this.data.lock().unwrap();
            Ok(PhysicsService::group_id(&data, &name)? as i32)
        });
        methods.add_method("GetCollisionGroupName", |_, this, id: i32| {
            if !(0..MAX_COLLISION_GROUPS as i32).contains(&id) {
                return Ok(None::<String>);
            }
            let data = this.data.lock().unwrap();
            Ok(PhysicsService::group_name(&data, id as u8))
        });
        // Clawblox extension: lists static triangle meshes loaded from world.toml
        // `[[collision_mesh]]` entries, for creating anchored Parts with `MeshId`.
        methods.add_method("GetCollisionMeshes", |lua, _, name: String| {
            use crate::game::lua::types::Vector3;
            let table = lua.create_table()?;
            for (offset, mesh) in crate::game::collision_mesh::list(&name)
                .into_iter()
                .enumerate()
            {
                let entry = lua.create_table()?;
                entry.set("Name", mesh.node_name.clone())?;
                entry.set("MeshId", mesh.id.clone())?;
                entry.set(
                    "Position",
                    Vector3::new(mesh.center[0], mesh.center[1], mesh.center[2]),
                )?;
                entry.set("Size", Vector3::new(mesh.size[0], mesh.size[1], mesh.size[2]))?;
                entry.set("Triangles", mesh.triangle_count as i64)?;
                table.set(offset + 1, entry)?;
            }
            Ok(table)
        });
        methods.add_method("GetRegisteredCollisionGroups", |lua, this, ()| {
            let table = lua.create_table()?;
            for (offset, (name, _id, mask)) in
                this.registered_group_snapshot().into_iter().enumerate()
            {
                let entry = lua.create_table()?;
                entry.set("name", name)?;
                entry.set("mask", mask as i64)?;
                table.set(offset + 1, entry)?;
            }
            Ok(table)
        });
        methods.add_method("GetCollisionGroups", |lua, this, ()| {
            let table = lua.create_table()?;
            for (offset, (name, id, mask)) in
                this.registered_group_snapshot().into_iter().enumerate()
            {
                let entry = lua.create_table()?;
                entry.set("name", name)?;
                entry.set("id", id as i32)?;
                entry.set("mask", mask as i64)?;
                table.set(offset + 1, entry)?;
            }
            Ok(table)
        });
        methods.add_method(
            "CollisionGroupsAreCollidable",
            |_, this, (group1, group2): (String, String)| {
                Ok(this.groups_collidable(&group1, &group2))
            },
        );
        methods.add_method(
            "CollisionGroupSetCollidable",
            |_, this, (group1, group2, collidable): (String, String, bool)| {
                let mut data = this.data.lock().unwrap();
                PhysicsService::require_registered_group(&data, &group1)?;
                PhysicsService::require_registered_group(&data, &group2)?;
                let group1 = PhysicsService::group_id(&data, &group1)?;
                let group2 = PhysicsService::group_id(&data, &group2)?;
                let key = PhysicsService::canonical_pair(group1, group2);
                data.explicit_pair_collidable.insert(key, collidable);
                Ok(())
            },
        );
        methods.add_method("UnregisterCollisionGroup", |_, this, name: String| {
            let mut data = this.data.lock().unwrap();
            PhysicsService::unregister_group(&mut data, &name)
        });
        methods.add_method("RemoveCollisionGroup", |_, this, name: String| {
            let mut data = this.data.lock().unwrap();
            PhysicsService::unregister_group(&mut data, &name)
        });
        methods.add_method(
            "RenameCollisionGroup",
            |_, this, (from, to): (String, String)| {
                let mut data = this.data.lock().unwrap();
                PhysicsService::rename_group(&mut data, &from, &to)
            },
        );
        methods.add_method(
            "SetPartCollisionGroup",
            |_, this, (part, group): (Instance, String)| {
                let group_id = {
                    let data = this.data.lock().unwrap();
                    PhysicsService::require_registered_group(&data, &group)?;
                    PhysicsService::group_id(&data, &group)?
                };
                let mut data = part.data.lock().unwrap();
                if let Some(part_data) = &mut data.part_data {
                    part_data.collision_group = group;
                    part_data.collision_group_id = group_id;
                    part_data.interaction_dirty = true;
                }
                Ok(())
            },
        );
    }
}

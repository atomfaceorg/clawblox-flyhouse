//! Static triangle-mesh colliders loaded from glTF binary (.glb) files.
//!
//! A world lists its meshes in `world.toml`:
//!
//! ```toml
//! [[collision_mesh]]
//! name = "house"
//! path = "assets/house_collision.glb"
//! scale = 100.0
//! ```
//!
//! Every mesh node in the file becomes one entry in a process-wide registry,
//! keyed `"<name>/<node name>"`. Vertices are baked into world space (node
//! transforms and `scale` applied), then stored relative to the node's bounding
//! box center. Luau reads the entries with `PhysicsService:GetCollisionMeshes`
//! and creates one anchored Part per entry with `Position` at that center, an
//! identity rotation, and `MeshId` set; the Part's collider and raycasts then
//! use the triangle mesh instead of its `Shape`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

use nalgebra::Matrix4;
use rapier3d::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CollisionMeshConfig {
    /// Prefix for mesh ids from this file.
    pub name: String,
    /// .glb path, relative to the world directory.
    pub path: String,
    /// Uniform scale applied after node transforms (e.g. 100 for meters to centimeters).
    #[serde(default = "default_scale")]
    pub scale: f32,
}

fn default_scale() -> f32 {
    1.0
}

#[derive(Clone)]
pub struct CollisionMesh {
    pub id: String,
    /// glTF node name (the Blender object name).
    pub node_name: String,
    /// World-space bounding box center; vertices are relative to this.
    pub center: [f32; 3],
    /// World-space bounding box size.
    pub size: [f32; 3],
    pub triangle_count: usize,
    pub shape: SharedShape,
}

impl std::fmt::Debug for CollisionMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionMesh")
            .field("id", &self.id)
            .field("center", &self.center)
            .field("size", &self.size)
            .field("triangle_count", &self.triangle_count)
            .finish()
    }
}

type Registry = RwLock<HashMap<String, Arc<CollisionMesh>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Looks up a loaded mesh by id (`"<name>/<node name>"`).
pub fn get(id: &str) -> Option<Arc<CollisionMesh>> {
    registry().read().unwrap().get(id).cloned()
}

/// All loaded meshes whose id starts with `"<name>/"`, sorted by id.
pub fn list(name: &str) -> Vec<Arc<CollisionMesh>> {
    let prefix = format!("{name}/");
    let mut meshes: Vec<_> = registry()
        .read()
        .unwrap()
        .values()
        .filter(|mesh| mesh.id.starts_with(&prefix))
        .cloned()
        .collect();
    meshes.sort_by(|a, b| a.id.cmp(&b.id));
    meshes
}

/// Loads every configured mesh file into the registry.
pub fn load_from_config(game_dir: &Path, configs: &[CollisionMeshConfig]) -> Result<(), String> {
    for config in configs {
        let path = game_dir.join(&config.path);
        let meshes = load_glb(&path, &config.name, config.scale)?;
        let triangles: usize = meshes.iter().map(|mesh| mesh.triangle_count).sum();
        eprintln!(
            "Collision mesh '{}': {} objects, {} triangles from {}",
            config.name,
            meshes.len(),
            triangles,
            config.path
        );
        let mut registry = registry().write().unwrap();
        for mesh in meshes {
            registry.insert(mesh.id.clone(), Arc::new(mesh));
        }
    }
    Ok(())
}

/// Parses a .glb and returns one collision mesh per named mesh node.
pub fn load_glb(path: &Path, name: &str, scale: f32) -> Result<Vec<CollisionMesh>, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read collision mesh {}: {err}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes)
        .map_err(|err| format!("failed to parse collision mesh {}: {err}", path.display()))?;
    let blob = gltf.blob.as_deref();

    // Merge primitives per node name, so an object split by material stays one collider.
    let mut by_node: Vec<(String, Vec<Point<f32>>, Vec<[u32; 3]>)> = Vec::new();
    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or_else(|| format!("collision mesh {} has no scene", path.display()))?;

    let mut stack: Vec<(gltf::Node, Matrix4<f32>)> = scene
        .nodes()
        .map(|node| (node, Matrix4::identity()))
        .collect();
    let mut unnamed = 0usize;
    while let Some((node, parent)) = stack.pop() {
        let local = Matrix4::from(node.transform().matrix());
        let world = parent * local;
        for child in node.children() {
            stack.push((child, world));
        }
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let node_name = node.name().map(str::to_string).unwrap_or_else(|| {
            unnamed += 1;
            format!("Mesh_{unnamed}")
        });
        let entry = match by_node.iter().position(|(n, _, _)| *n == node_name) {
            Some(index) => &mut by_node[index],
            None => {
                by_node.push((node_name, Vec::new(), Vec::new()));
                by_node.last_mut().unwrap()
            }
        };
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(|buffer| match buffer.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            });
            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let base = entry.1.len() as u32;
            for p in positions {
                let v = world.transform_point(&nalgebra::Point3::new(p[0], p[1], p[2]));
                entry.1.push(point![v.x * scale, v.y * scale, v.z * scale]);
            }
            let count = entry.1.len() as u32 - base;
            match reader.read_indices() {
                Some(indices) => {
                    let flat: Vec<u32> = indices.into_u32().collect();
                    for tri in flat.chunks_exact(3) {
                        entry.2.push([base + tri[0], base + tri[1], base + tri[2]]);
                    }
                }
                None => {
                    for i in (0..count.saturating_sub(2)).step_by(3) {
                        entry.2.push([base + i, base + i + 1, base + i + 2]);
                    }
                }
            }
        }
    }

    let mut meshes = Vec::with_capacity(by_node.len());
    for (node_name, vertices, indices) in by_node {
        if indices.is_empty() {
            continue;
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(v[axis]);
                max[axis] = max[axis].max(v[axis]);
            }
        }
        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        let size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        let local: Vec<Point<f32>> = vertices
            .iter()
            .map(|v| point![v.x - center[0], v.y - center[1], v.z - center[2]])
            .collect();
        let triangle_count = indices.len();
        let shape = SharedShape::trimesh_with_flags(
            local,
            indices,
            TriMeshFlags::FIX_INTERNAL_EDGES | TriMeshFlags::MERGE_DUPLICATE_VERTICES,
        )
        .map_err(|err| format!("collision mesh {name}/{node_name}: {err:?}"))?;
        meshes.push(CollisionMesh {
            id: format!("{name}/{node_name}"),
            node_name,
            center,
            size,
            triangle_count,
            shape,
        });
    }
    Ok(meshes)
}

/// Inserts a mesh directly; used by tests.
#[cfg(test)]
pub fn insert_for_test(mesh: CollisionMesh) {
    registry()
        .write()
        .unwrap()
        .insert(mesh.id.clone(), Arc::new(mesh));
}

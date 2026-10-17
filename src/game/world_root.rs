use nalgebra::{Isometry3, Matrix3, Rotation3, Translation3, UnitQuaternion};
use rapier3d::parry::query::intersection_test;
use rapier3d::prelude::{point, SharedShape};

use crate::game::lua::instance::{Instance, TerrainCell, TerrainVoxel, TERRAIN_VOXEL_SIZE};
use crate::game::lua::services::PhysicsService;
use crate::game::lua::types::{CFrame, Material, PartType, RaycastFilterType, Vector3};

#[derive(Clone)]
pub struct RaycastQueryParams {
    pub filter_type: RaycastFilterType,
    pub filter_instances: Vec<Instance>,
    pub collision_group: String,
    pub respect_can_collide: bool,
    pub ignore_water: bool,
}

#[derive(Clone)]
pub struct OverlapQueryParams {
    pub filter_type: RaycastFilterType,
    pub filter_instances: Vec<Instance>,
    pub max_parts: usize,
    pub collision_group: String,
    pub respect_can_collide: bool,
}

#[derive(Clone)]
pub struct RaycastHit {
    pub instance: Instance,
    pub position: Vector3,
    pub normal: Vector3,
    pub distance: f32,
    pub material: Material,
}

pub struct WorldRoot {
    root: Instance,
}

impl WorldRoot {
    pub fn new(root: Instance) -> Self {
        Self { root }
    }

    fn descendants(&self) -> Vec<Instance> {
        self.root.get_descendants()
    }

    pub fn raycast(
        &self,
        origin: Vector3,
        direction: Vector3,
        params: RaycastQueryParams,
        physics_service: &PhysicsService,
    ) -> mlua::Result<Option<RaycastHit>> {
        let query_group_id =
            resolve_query_collision_group(physics_service, &params.collision_group)?;
        let ray_length = direction.magnitude();
        let ray_dir = direction.unit();

        let mut closest: Option<(f32, Instance, Vector3, Material)> = None;

        for descendant in self.descendants() {
            let part_info = {
                let data = descendant.data.lock().unwrap();
                data.part_data.as_ref().map(|part| {
                    (
                        part.can_query,
                        part.can_collide,
                        part.collision_group_id,
                        part.size,
                        part.cframe.position,
                        part.cframe.rotation,
                        part.shape,
                        part.material,
                        part.mesh_id.clone(),
                    )
                })
            };

            let Some((
                can_query,
                can_collide,
                collision_group_id,
                size,
                position,
                rotation,
                shape,
                material,
                mesh_id,
            )) = part_info
            else {
                continue;
            };

            if !can_query {
                continue;
            }
            if params.respect_can_collide && !can_collide {
                continue;
            }
            if !physics_service.groups_collidable_ids(query_group_id, collision_group_id) {
                continue;
            }
            if query_filter_skips(&descendant, &params.filter_instances, params.filter_type) {
                continue;
            }

            let center = [position.x, position.y, position.z];
            let half = [size.x / 2.0, size.y / 2.0, size.z / 2.0];
            let ray_origin = [origin.x, origin.y, origin.z];
            let ray_direction = [ray_dir.x, ray_dir.y, ray_dir.z];

            let local_origin = mat_t_mul_vec(
                &rotation,
                [
                    ray_origin[0] - center[0],
                    ray_origin[1] - center[1],
                    ray_origin[2] - center[2],
                ],
            );
            let local_dir = mat_t_mul_vec(&rotation, ray_direction);

            let mesh = mesh_id.as_deref().and_then(crate::game::collision_mesh::get);
            let hit = match (mesh, shape) {
                (Some(mesh), _) => {
                    use rapier3d::parry::query::{Ray, RayCast};
                    let ray = Ray::new(point![local_origin[0], local_origin[1], local_origin[2]], local_dir.into());
                    mesh.shape
                        .cast_local_ray_and_get_normal(&ray, ray_length, true)
                        .map(|hit| (hit.time_of_impact, [hit.normal.x, hit.normal.y, hit.normal.z]))
                }
                (None, PartType::Block) => intersect_box_local(local_origin, local_dir, half),
                (None, PartType::Ball) => intersect_sphere_local(local_origin, local_dir, half[0]),
                (None, PartType::Cylinder) => {
                    intersect_cylinder_local(local_origin, local_dir, half[0], half[1])
                }
                (None, PartType::Ring) => intersect_ring_local(local_origin, local_dir, size),
                (None, PartType::Wedge) => intersect_wedge_local(local_origin, local_dir, half),
            };

            if let Some((t, local_normal)) = hit {
                if t < 0.0 || t > ray_length {
                    continue;
                }
                let world_normal = normalize(mat_mul_vec(&rotation, local_normal));
                let normal = Vector3::new(world_normal[0], world_normal[1], world_normal[2]);

                if closest.is_none() || t < closest.as_ref().unwrap().0 {
                    closest = Some((t, descendant.clone(), normal, material));
                }
            }
        }

        if !params.ignore_water {
            for terrain in self.descendants() {
                let water_blocks = {
                    let data = terrain.data.lock().unwrap();
                    data.terrain_data
                        .as_ref()
                        .map(|terrain| terrain.water_voxels().collect::<Vec<_>>())
                };
                let Some(water_voxels) = water_blocks else {
                    continue;
                };
                if query_filter_skips(&terrain, &params.filter_instances, params.filter_type) {
                    continue;
                }

                for (cell, voxel) in water_voxels {
                    if let Some((t, normal)) =
                        intersect_terrain_water_voxel(cell, voxel, origin, ray_dir, ray_length)
                    {
                        if closest.is_none() || t < closest.as_ref().unwrap().0 {
                            closest = Some((t, terrain.clone(), normal, Material::Water));
                        }
                    }
                }
            }
        }

        Ok(closest.map(|(dist, instance, normal, material)| {
            let position = Vector3::new(
                origin.x + ray_dir.x * dist,
                origin.y + ray_dir.y * dist,
                origin.z + ray_dir.z * dist,
            );
            RaycastHit {
                instance,
                position,
                normal,
                distance: dist,
                material,
            }
        }))
    }

    pub fn get_part_bounds_in_box(
        &self,
        cframe: CFrame,
        size: Vector3,
        params: OverlapQueryParams,
        physics_service: &PhysicsService,
    ) -> mlua::Result<Vec<Instance>> {
        let query_group_id =
            resolve_query_collision_group(physics_service, &params.collision_group)?;
        let query_center = [cframe.position.x, cframe.position.y, cframe.position.z];
        let query_rot = cframe.rotation;
        let query_half = [size.x / 2.0, size.y / 2.0, size.z / 2.0];

        let mut out = Vec::new();
        for inst in self.descendants() {
            if query_filter_skips(&inst, &params.filter_instances, params.filter_type) {
                continue;
            }

            let intersects = {
                let data = inst.data.lock().unwrap();
                if let Some(part) = &data.part_data {
                    if !part.can_query {
                        false
                    } else if params.respect_can_collide && !part.can_collide {
                        false
                    } else if !physics_service
                        .groups_collidable_ids(query_group_id, part.collision_group_id)
                    {
                        false
                    } else {
                        let part_center = [part.position.x, part.position.y, part.position.z];
                        let part_rot = part.cframe.rotation;
                        let part_half = [part.size.x / 2.0, part.size.y / 2.0, part.size.z / 2.0];

                        obb_intersects_obb(
                            query_center,
                            query_rot,
                            query_half,
                            part_center,
                            part_rot,
                            part_half,
                        )
                    }
                } else {
                    false
                }
            };

            if intersects {
                out.push(inst);
                if params.max_parts > 0 && out.len() >= params.max_parts {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn get_part_bounds_in_radius(
        &self,
        position: Vector3,
        radius: f32,
        params: OverlapQueryParams,
        physics_service: &PhysicsService,
    ) -> mlua::Result<Vec<Instance>> {
        let query_group_id =
            resolve_query_collision_group(physics_service, &params.collision_group)?;
        let sphere_center = [position.x, position.y, position.z];
        let mut out = Vec::new();
        for inst in self.descendants() {
            if query_filter_skips(&inst, &params.filter_instances, params.filter_type) {
                continue;
            }

            let intersects = {
                let data = inst.data.lock().unwrap();
                if let Some(part) = &data.part_data {
                    if !part.can_query {
                        false
                    } else if params.respect_can_collide && !part.can_collide {
                        false
                    } else if !physics_service
                        .groups_collidable_ids(query_group_id, part.collision_group_id)
                    {
                        false
                    } else {
                        let part_center = [part.position.x, part.position.y, part.position.z];
                        let part_rot = part.cframe.rotation;
                        let part_half = [part.size.x / 2.0, part.size.y / 2.0, part.size.z / 2.0];
                        sphere_intersects_obb(
                            sphere_center,
                            radius,
                            part_center,
                            part_rot,
                            part_half,
                        )
                    }
                } else {
                    false
                }
            };

            if intersects {
                out.push(inst);
                if params.max_parts > 0 && out.len() >= params.max_parts {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn get_parts_in_part(
        &self,
        query_part: Instance,
        params: OverlapQueryParams,
        physics_service: &PhysicsService,
    ) -> mlua::Result<Vec<Instance>> {
        let query_group_id =
            resolve_query_collision_group(physics_service, &params.collision_group)?;
        let query_data = {
            let data = query_part.data.lock().unwrap();
            let Some(part) = &data.part_data else {
                return Ok(Vec::new());
            };
            (data.id.0, part.size, part.cframe, part.shape, part.mesh_id.clone())
        };
        let (query_id, query_size, query_cframe, query_shape_type, query_mesh_id) = query_data;
        let Some(query_shape) =
            part_collision_shape(query_size, query_shape_type, query_mesh_id.as_deref())
        else {
            return Ok(Vec::new());
        };
        let query_iso = cframe_to_isometry(query_cframe);

        let mut out = Vec::new();
        for inst in self.descendants() {
            if query_filter_skips(&inst, &params.filter_instances, params.filter_type) {
                continue;
            }

            let part_data = {
                let data = inst.data.lock().unwrap();
                if data.id.0 == query_id {
                    None
                } else {
                    data.part_data.as_ref().map(|p| {
                        (
                            p.can_query,
                            p.can_collide,
                            p.collision_group_id,
                            p.size,
                            p.cframe,
                            p.shape,
                            p.mesh_id.clone(),
                        )
                    })
                }
            };

            let Some((
                can_query,
                can_collide,
                collision_group_id,
                size,
                cframe,
                shape_type,
                mesh_id,
            )) = part_data
            else {
                continue;
            };

            if !can_query {
                continue;
            }
            if params.respect_can_collide && !can_collide {
                continue;
            }
            if !physics_service.groups_collidable_ids(query_group_id, collision_group_id) {
                continue;
            }

            let Some(shape) = part_collision_shape(size, shape_type, mesh_id.as_deref()) else {
                continue;
            };
            let iso = cframe_to_isometry(cframe);
            let intersects =
                intersection_test(&query_iso, query_shape.as_ref(), &iso, shape.as_ref())
                    .unwrap_or(false);
            if intersects {
                out.push(inst);
                if params.max_parts > 0 && out.len() >= params.max_parts {
                    break;
                }
            }
        }
        Ok(out)
    }
}

fn resolve_query_collision_group(
    physics_service: &PhysicsService,
    group: &str,
) -> mlua::Result<u8> {
    if group.is_empty() {
        Ok(0)
    } else {
        physics_service.resolve_collision_group_id(group)
    }
}

fn query_filter_skips(
    instance: &Instance,
    filter_instances: &[Instance],
    filter_type: RaycastFilterType,
) -> bool {
    let should_filter = filter_instances
        .iter()
        .any(|i| i.id() == instance.id() || instance.is_descendant_of(i));
    match filter_type {
        RaycastFilterType::Exclude => should_filter,
        RaycastFilterType::Include => !should_filter,
    }
}

#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

#[inline]
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = length(v);
    if l <= 1e-8 {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

#[inline]
fn mat_mul_vec(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

#[inline]
fn mat_t_mul_vec(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

fn intersect_terrain_water_voxel(
    cell: TerrainCell,
    voxel: TerrainVoxel,
    origin: Vector3,
    ray_dir: Vector3,
    ray_length: f32,
) -> Option<(f32, Vector3)> {
    if voxel.liquid_occupancy <= 0.0 {
        return None;
    }
    let center = cell.center();
    let center = [center.x, center.y, center.z];
    let water_height = TERRAIN_VOXEL_SIZE * voxel.liquid_occupancy.clamp(0.0, 1.0);
    let half = [
        TERRAIN_VOXEL_SIZE / 2.0,
        water_height / 2.0,
        TERRAIN_VOXEL_SIZE / 2.0,
    ];
    let water_center_y = center[1] - TERRAIN_VOXEL_SIZE / 2.0 + water_height / 2.0;
    let local_origin = mat_t_mul_vec(
        &CFrame::identity().rotation,
        [
            origin.x - center[0],
            origin.y - water_center_y,
            origin.z - center[2],
        ],
    );
    let local_dir = [ray_dir.x, ray_dir.y, ray_dir.z];
    let (t, local_normal) = intersect_box_local(local_origin, local_dir, half)?;
    if t < 0.0 || t > ray_length {
        return None;
    }

    Some((
        t,
        Vector3::new(local_normal[0], local_normal[1], local_normal[2]),
    ))
}

fn intersect_box_local(origin: [f32; 3], dir: [f32; 3], half: [f32; 3]) -> Option<(f32, [f32; 3])> {
    let eps = 1e-8;
    let mut t_near = f32::NEG_INFINITY;
    let mut t_far = f32::INFINITY;
    let mut near_normal = [0.0, 0.0, 0.0];
    let mut far_normal = [0.0, 0.0, 0.0];

    for axis in 0..3 {
        if dir[axis].abs() < eps {
            if origin[axis] < -half[axis] || origin[axis] > half[axis] {
                return None;
            }
            continue;
        }

        let inv = 1.0 / dir[axis];
        let mut t1 = (-half[axis] - origin[axis]) * inv;
        let mut t2 = (half[axis] - origin[axis]) * inv;

        let mut n1 = [0.0, 0.0, 0.0];
        let mut n2 = [0.0, 0.0, 0.0];
        n1[axis] = -1.0;
        n2[axis] = 1.0;

        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            std::mem::swap(&mut n1, &mut n2);
        }

        if t1 > t_near {
            t_near = t1;
            near_normal = n1;
        }
        if t2 < t_far {
            t_far = t2;
            far_normal = n2;
        }

        if t_near > t_far {
            return None;
        }
    }

    if t_near >= 0.0 {
        Some((t_near, near_normal))
    } else if t_far >= 0.0 {
        Some((t_far, far_normal))
    } else {
        None
    }
}

fn intersect_sphere_local(origin: [f32; 3], dir: [f32; 3], radius: f32) -> Option<(f32, [f32; 3])> {
    let a = dot(dir, dir);
    let b = 2.0 * dot(origin, dir);
    let c = dot(origin, origin) - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }

    let sqrt_disc = disc.sqrt();
    let inv_2a = 1.0 / (2.0 * a);
    let t1 = (-b - sqrt_disc) * inv_2a;
    let t2 = (-b + sqrt_disc) * inv_2a;
    let t = if t1 >= 0.0 {
        t1
    } else if t2 >= 0.0 {
        t2
    } else {
        return None;
    };

    let p = [
        origin[0] + dir[0] * t,
        origin[1] + dir[1] * t,
        origin[2] + dir[2] * t,
    ];
    Some((t, normalize(p)))
}

fn intersect_cylinder_local(
    origin: [f32; 3],
    dir: [f32; 3],
    radius: f32,
    half_height: f32,
) -> Option<(f32, [f32; 3])> {
    let eps = 1e-8;
    let mut best_t = f32::INFINITY;
    let mut best_normal = [0.0, 0.0, 0.0];

    let a = dir[0] * dir[0] + dir[2] * dir[2];
    let b = 2.0 * (origin[0] * dir[0] + origin[2] * dir[2]);
    let c = origin[0] * origin[0] + origin[2] * origin[2] - radius * radius;

    if a > eps {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let sqrt_disc = disc.sqrt();
            let inv_2a = 1.0 / (2.0 * a);
            for t in [(-b - sqrt_disc) * inv_2a, (-b + sqrt_disc) * inv_2a] {
                if t < 0.0 || t >= best_t {
                    continue;
                }
                let y = origin[1] + dir[1] * t;
                if y >= -half_height && y <= half_height {
                    let x = origin[0] + dir[0] * t;
                    let z = origin[2] + dir[2] * t;
                    best_t = t;
                    best_normal = normalize([x, 0.0, z]);
                }
            }
        }
    }

    if dir[1].abs() > eps {
        for y_cap in [-half_height, half_height] {
            let t = (y_cap - origin[1]) / dir[1];
            if t < 0.0 || t >= best_t {
                continue;
            }
            let x = origin[0] + dir[0] * t;
            let z = origin[2] + dir[2] * t;
            if x * x + z * z <= radius * radius {
                best_t = t;
                best_normal = if y_cap < 0.0 {
                    [0.0, -1.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
            }
        }
    }

    if best_t.is_finite() {
        Some((best_t, best_normal))
    } else {
        None
    }
}

fn intersect_wedge_local(
    origin: [f32; 3],
    dir: [f32; 3],
    half: [f32; 3],
) -> Option<(f32, [f32; 3])> {
    let eps = 1e-8;
    let hx = half[0].max(eps);
    let hy = half[1];
    let hz = half[2];

    let planes: [([f32; 3], f32); 6] = [
        ([1.0, 0.0, 0.0], hx),
        ([-1.0, 0.0, 0.0], hx),
        ([0.0, 0.0, 1.0], hz),
        ([0.0, 0.0, -1.0], hz),
        ([0.0, -1.0, 0.0], hy),
        ([hy / hx, 1.0, 0.0], 0.0),
    ];

    let mut t_near = f32::NEG_INFINITY;
    let mut t_far = f32::INFINITY;
    let mut near_normal = [0.0, 0.0, 0.0];
    let mut far_normal = [0.0, 0.0, 0.0];

    for (normal, d) in planes {
        let denom = dot(normal, dir);
        let dist = d - dot(normal, origin);
        if denom.abs() < eps {
            if dist < 0.0 {
                return None;
            }
            continue;
        }

        let t = dist / denom;
        if denom < 0.0 {
            if t > t_near {
                t_near = t;
                near_normal = normal;
            }
        } else if t < t_far {
            t_far = t;
            far_normal = normal;
        }

        if t_near > t_far {
            return None;
        }
    }

    if t_near >= 0.0 {
        Some((t_near, near_normal))
    } else if t_far >= 0.0 {
        Some((t_far, far_normal))
    } else {
        None
    }
}

fn obb_intersects_obb(
    center_a: [f32; 3],
    rot_a: [[f32; 3]; 3],
    half_a: [f32; 3],
    center_b: [f32; 3],
    rot_b: [[f32; 3]; 3],
    half_b: [f32; 3],
) -> bool {
    let eps = 1e-6;
    let axes_a = [
        [rot_a[0][0], rot_a[1][0], rot_a[2][0]],
        [rot_a[0][1], rot_a[1][1], rot_a[2][1]],
        [rot_a[0][2], rot_a[1][2], rot_a[2][2]],
    ];
    let axes_b = [
        [rot_b[0][0], rot_b[1][0], rot_b[2][0]],
        [rot_b[0][1], rot_b[1][1], rot_b[2][1]],
        [rot_b[0][2], rot_b[1][2], rot_b[2][2]],
    ];

    let mut r = [[0.0f32; 3]; 3];
    let mut abs_r = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = dot(axes_a[i], axes_b[j]);
            abs_r[i][j] = r[i][j].abs() + eps;
        }
    }

    let t_world = [
        center_b[0] - center_a[0],
        center_b[1] - center_a[1],
        center_b[2] - center_a[2],
    ];
    let t = [
        dot(t_world, axes_a[0]),
        dot(t_world, axes_a[1]),
        dot(t_world, axes_a[2]),
    ];

    for i in 0..3 {
        let ra = half_a[i];
        let rb = half_b[0] * abs_r[i][0] + half_b[1] * abs_r[i][1] + half_b[2] * abs_r[i][2];
        if t[i].abs() > ra + rb {
            return false;
        }
    }

    for j in 0..3 {
        let ra = half_a[0] * abs_r[0][j] + half_a[1] * abs_r[1][j] + half_a[2] * abs_r[2][j];
        let rb = half_b[j];
        let t_proj = (t[0] * r[0][j] + t[1] * r[1][j] + t[2] * r[2][j]).abs();
        if t_proj > ra + rb {
            return false;
        }
    }

    for i in 0..3 {
        for j in 0..3 {
            let ra = half_a[(i + 1) % 3] * abs_r[(i + 2) % 3][j]
                + half_a[(i + 2) % 3] * abs_r[(i + 1) % 3][j];
            let rb = half_b[(j + 1) % 3] * abs_r[i][(j + 2) % 3]
                + half_b[(j + 2) % 3] * abs_r[i][(j + 1) % 3];
            let t_proj =
                (t[(i + 2) % 3] * r[(i + 1) % 3][j] - t[(i + 1) % 3] * r[(i + 2) % 3][j]).abs();
            if t_proj > ra + rb {
                return false;
            }
        }
    }

    true
}

fn sphere_intersects_obb(
    sphere_center: [f32; 3],
    sphere_radius: f32,
    obb_center: [f32; 3],
    obb_rotation: [[f32; 3]; 3],
    obb_half: [f32; 3],
) -> bool {
    let relative = [
        sphere_center[0] - obb_center[0],
        sphere_center[1] - obb_center[1],
        sphere_center[2] - obb_center[2],
    ];
    let local = mat_t_mul_vec(&obb_rotation, relative);

    let clamped = [
        local[0].max(-obb_half[0]).min(obb_half[0]),
        local[1].max(-obb_half[1]).min(obb_half[1]),
        local[2].max(-obb_half[2]).min(obb_half[2]),
    ];

    let dx = local[0] - clamped[0];
    let dy = local[1] - clamped[1];
    let dz = local[2] - clamped[2];
    (dx * dx + dy * dy + dz * dz) <= sphere_radius * sphere_radius
}

fn ring_segments(size: Vector3) -> Vec<([f32; 3], f32, [f32; 3])> {
    let outer_radius = 0.5 * size.x.min(size.z);
    let tube_radius = (size.y * 0.5).min(outer_radius * 0.45).max(0.05);
    let major_radius = (outer_radius - tube_radius).max(0.05);
    let segments = 12usize;
    let segment_length = (2.0 * major_radius * (std::f32::consts::PI / segments as f32).tan())
        .max(tube_radius * 1.5)
        * 1.08;
    let half = [segment_length * 0.5, tube_radius, tube_radius];

    let mut out = Vec::with_capacity(segments);
    for index in 0..segments {
        let angle = (index as f32 / segments as f32) * std::f32::consts::TAU;
        out.push((
            [angle.cos() * major_radius, 0.0, angle.sin() * major_radius],
            angle + std::f32::consts::FRAC_PI_2,
            half,
        ));
    }
    out
}

fn build_ring_shared_shape(size: Vector3) -> SharedShape {
    let cuboid_half = {
        let segments = ring_segments(size);
        let half = segments
            .first()
            .map(|(_, _, half)| *half)
            .unwrap_or([0.05, 0.05, 0.05]);
        SharedShape::cuboid(half[0], half[1], half[2])
    };

    let pieces = ring_segments(size)
        .into_iter()
        .map(|(center, angle, _half)| {
            let transform = Isometry3::from_parts(
                Translation3::new(center[0], center[1], center[2]),
                UnitQuaternion::from_euler_angles(0.0, angle, 0.0),
            );
            (transform, cuboid_half.clone())
        })
        .collect();

    SharedShape::compound(pieces)
}

fn intersect_ring_local(origin: [f32; 3], dir: [f32; 3], size: Vector3) -> Option<(f32, [f32; 3])> {
    let mut best: Option<(f32, [f32; 3])> = None;

    for (center, angle, half) in ring_segments(size) {
        let cos_y = angle.cos();
        let sin_y = angle.sin();
        let relative_origin = [
            origin[0] - center[0],
            origin[1] - center[1],
            origin[2] - center[2],
        ];
        let segment_origin = [
            cos_y * relative_origin[0] - sin_y * relative_origin[2],
            relative_origin[1],
            sin_y * relative_origin[0] + cos_y * relative_origin[2],
        ];
        let segment_dir = [
            cos_y * dir[0] - sin_y * dir[2],
            dir[1],
            sin_y * dir[0] + cos_y * dir[2],
        ];

        if let Some((t, local_normal)) = intersect_box_local(segment_origin, segment_dir, half) {
            let world_normal = [
                cos_y * local_normal[0] + sin_y * local_normal[2],
                local_normal[1],
                -sin_y * local_normal[0] + cos_y * local_normal[2],
            ];
            match best {
                Some((best_t, _)) if t >= best_t => {}
                _ => best = Some((t, normalize(world_normal))),
            }
        }
    }

    best
}

fn part_collision_shape(size: Vector3, shape: PartType, mesh_id: Option<&str>) -> Option<SharedShape> {
    match mesh_id.and_then(crate::game::collision_mesh::get) {
        Some(mesh) => Some(mesh.shape.clone()),
        None => shape_from_part(size, shape),
    }
}

fn shape_from_part(size: Vector3, shape: PartType) -> Option<SharedShape> {
    let [sx, sy, sz] = [size.x, size.y, size.z];
    Some(match shape {
        PartType::Block => SharedShape::cuboid(sx / 2.0, sy / 2.0, sz / 2.0),
        PartType::Ball => SharedShape::ball(sx / 2.0),
        PartType::Cylinder => SharedShape::cylinder(sy / 2.0, sx / 2.0),
        PartType::Ring => build_ring_shared_shape(size),
        PartType::Wedge => {
            let hx = sx / 2.0;
            let hy = sy / 2.0;
            let hz = sz / 2.0;
            let points = [
                point![-hx, -hy, -hz],
                point![hx, -hy, -hz],
                point![-hx, -hy, hz],
                point![hx, -hy, hz],
                point![-hx, hy, -hz],
                point![-hx, hy, hz],
            ];
            SharedShape::convex_hull(&points)?
        }
    })
}

fn cframe_to_isometry(cframe: CFrame) -> Isometry3<f32> {
    let cframe = cframe.orthonormalized();
    let m = Matrix3::new(
        cframe.rotation[0][0],
        cframe.rotation[0][1],
        cframe.rotation[0][2],
        cframe.rotation[1][0],
        cframe.rotation[1][1],
        cframe.rotation[1][2],
        cframe.rotation[2][0],
        cframe.rotation[2][1],
        cframe.rotation[2][2],
    );
    let rot = Rotation3::from_matrix_unchecked(m);
    Isometry3::from_parts(
        Translation3::new(cframe.position.x, cframe.position.y, cframe.position.z),
        UnitQuaternion::from_rotation_matrix(&rot),
    )
}

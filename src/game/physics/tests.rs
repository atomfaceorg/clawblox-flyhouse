use super::*;

#[test]
fn test_physics_world_creation() {
    let world = PhysicsWorld::new();
    assert_eq!(world.gravity.y, -consts::DEFAULT_GRAVITY);
}

#[test]
fn test_add_anchored_part() {
    let mut world = PhysicsWorld::new();

    let handle = world.add_part(
        1,
        [0.0, 10.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [4.0, 1.0, 2.0],
        true, // anchored
        true, // can_collide
        true,
        true,
        PartType::Block,
        None,
    );

    assert!(world.has_part(1));
    let pos = world.get_position(handle).unwrap();
    assert_eq!(pos, [0.0, 10.0, 0.0]);
}

#[test]
fn test_dynamic_part_falls() {
    let mut world = PhysicsWorld::new();

    let handle = world.add_part(
        1,
        [0.0, 10.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 1.0, 1.0],
        false, // not anchored - should fall
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let initial_pos = world.get_position(handle).unwrap();

    // Step physics a few times
    for _ in 0..10 {
        world.step(1.0 / 60.0);
    }

    let final_pos = world.get_position(handle).unwrap();

    // Y position should be lower due to gravity
    assert!(final_pos[1] < initial_pos[1]);
}

fn assembly_part(
    lua_id: u64,
    position: [f32; 3],
    size: [f32; 3],
    anchored: bool,
) -> PhysicsAssemblyPart {
    PhysicsAssemblyPart {
        lua_id,
        position,
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        size,
        anchored,
        can_collide: true,
        can_touch: true,
        can_query: true,
        collision_group: CollisionGroupMask::default(),
        shape: PartType::Block,
        mesh_id: None,
        physical_properties: None,
        velocity: [0.0, 0.0, 0.0],
        angular_velocity: [0.0, 0.0, 0.0],
        position_dirty: true,
        cframe_dirty: false,
        size_dirty: true,
        anchored_dirty: true,
        interaction_dirty: true,
        velocity_dirty: false,
        angular_velocity_dirty: false,
        shape_dirty: true,
        custom_physical_properties_dirty: true,
    }
}

fn single_part_spec(part: PhysicsAssemblyPart) -> PhysicsAssemblySpec {
    PhysicsAssemblySpec {
        root_id: part.lua_id,
        parts: vec![part],
    }
}

fn register_test_mesh_floor(id: &str) {
    // A 20 x 20 quad at y = 0, as two triangles, centered on the origin.
    let vertices = vec![
        point![-10.0, 0.0, -10.0],
        point![10.0, 0.0, -10.0],
        point![10.0, 0.0, 10.0],
        point![-10.0, 0.0, 10.0],
    ];
    let indices = vec![[0, 2, 1], [0, 3, 2]];
    crate::game::collision_mesh::insert_for_test(crate::game::collision_mesh::CollisionMesh {
        id: id.to_string(),
        node_name: "Floor".to_string(),
        center: [0.0, 0.0, 0.0],
        size: [20.0, 0.0, 20.0],
        triangle_count: 2,
        shape: SharedShape::trimesh(vertices, indices).unwrap(),
    });
}

#[test]
fn test_dynamic_part_lands_on_mesh_collider() {
    register_test_mesh_floor("test/Floor");
    let mut floor = assembly_part(1, [0.0, 0.0, 0.0], [20.0, 0.1, 20.0], true);
    floor.mesh_id = Some("test/Floor".to_string());
    let floor = single_part_spec(floor);

    let mut world = PhysicsWorld::new();
    world.sync_assemblies(
        vec![
            floor.clone(),
            single_part_spec(assembly_part(2, [0.0, 3.0, 0.0], [1.0, 1.0, 1.0], false)),
        ],
        1.0 / 60.0,
    );
    for _ in 0..180 {
        world.step(1.0 / 60.0);
    }
    let pos = world.get_part_position(2).unwrap();
    assert!(
        (pos[1] - 0.5).abs() < 0.1,
        "block should rest on the mesh floor at y=0.5, got {pos:?}"
    );
}

fn floor_spec() -> PhysicsAssemblySpec {
    single_part_spec(assembly_part(1, [0.0, 0.0, 0.0], [20.0, 1.0, 20.0], true))
}

#[test]
fn test_supported_zero_velocity_unanchor_starts_awake() {
    let mut world = PhysicsWorld::new();

    world.sync_assemblies(vec![floor_spec()], 1.0 / 60.0);
    world.query_pipeline.update(&world.collider_set);

    world.sync_assemblies(
        vec![
            floor_spec(),
            single_part_spec(assembly_part(2, [0.0, 1.0, 0.0], [1.0, 1.0, 1.0], true)),
        ],
        1.0 / 60.0,
    );
    world.query_pipeline.update(&world.collider_set);

    world.sync_assemblies(
        vec![
            floor_spec(),
            single_part_spec(assembly_part(2, [0.0, 1.0, 0.0], [1.0, 1.0, 1.0], false)),
        ],
        1.0 / 60.0,
    );

    assert!(!world.is_part_sleeping(2));
    let before = world.get_part_position(2).unwrap();
    world.step(1.0 / 60.0);
    let after = world.get_part_position(2).unwrap();
    assert!(
        after[1] <= before[1],
        "newly unanchored supported body should simulate naturally, before={before:?}, after={after:?}"
    );
}

#[test]
fn test_zero_velocity_unanchor_with_small_air_gap_falls() {
    let mut world = PhysicsWorld::new();

    world.sync_assemblies(vec![floor_spec()], 1.0 / 60.0);
    world.query_pipeline.update(&world.collider_set);

    // Floor top is y=0.5. This block's bottom is y=0.55: close to the
    // floor, but not actually touching.
    world.sync_assemblies(
        vec![
            floor_spec(),
            single_part_spec(assembly_part(2, [0.0, 1.05, 0.0], [1.0, 1.0, 1.0], true)),
        ],
        1.0 / 60.0,
    );
    world.query_pipeline.update(&world.collider_set);

    world.sync_assemblies(
        vec![
            floor_spec(),
            single_part_spec(assembly_part(2, [0.0, 1.05, 0.0], [1.0, 1.0, 1.0], false)),
        ],
        1.0 / 60.0,
    );

    assert!(!world.is_part_sleeping(2));
    let before = world.get_part_position(2).unwrap();
    for _ in 0..120 {
        world.step(1.0 / 60.0);
    }

    assert!(world.get_part_position(2).unwrap()[1] < before[1]);
}

#[test]
fn test_unsupported_zero_velocity_unanchor_falls() {
    let mut world = PhysicsWorld::new();

    world.sync_assemblies(
        vec![single_part_spec(assembly_part(
            2,
            [0.0, 10.0, 0.0],
            [1.0, 1.0, 1.0],
            true,
        ))],
        1.0 / 60.0,
    );
    world.query_pipeline.update(&world.collider_set);

    world.sync_assemblies(
        vec![single_part_spec(assembly_part(
            2,
            [0.0, 10.0, 0.0],
            [1.0, 1.0, 1.0],
            false,
        ))],
        1.0 / 60.0,
    );

    assert!(!world.is_part_sleeping(2));
    let before = world.get_part_position(2).unwrap();
    for _ in 0..10 {
        world.step(1.0 / 60.0);
    }
    assert!(world.get_part_position(2).unwrap()[1] < before[1]);
}

#[test]
fn test_velocity_unanchor_wakes_even_when_supported() {
    let mut world = PhysicsWorld::new();

    world.sync_assemblies(vec![floor_spec()], 1.0 / 60.0);
    world.query_pipeline.update(&world.collider_set);

    world.sync_assemblies(
        vec![
            floor_spec(),
            single_part_spec(assembly_part(2, [0.0, 1.0, 0.0], [1.0, 1.0, 1.0], true)),
        ],
        1.0 / 60.0,
    );
    world.query_pipeline.update(&world.collider_set);

    let mut moving = assembly_part(2, [0.0, 1.0, 0.0], [1.0, 1.0, 1.0], false);
    moving.velocity = [3.0, 0.0, 0.0];
    moving.velocity_dirty = true;
    world.sync_assemblies(vec![floor_spec(), single_part_spec(moving)], 1.0 / 60.0);

    assert!(!world.is_part_sleeping(2));
    assert_eq!(world.get_part_velocity(2), Some([3.0, 0.0, 0.0]));
}

#[test]
fn test_character_raycast_and_move() {
    let mut world = PhysicsWorld::new();

    // Add a floor
    world.add_part(
        1,
        [0.0, 0.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 1.0, 100.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Add character above floor
    let char_id = 100;
    world.add_character(char_id, [0.0, 6.0, 0.0], 1.0, 5.0);

    world.step(1.0 / 60.0);
    world.query_pipeline.update(&world.collider_set);

    // Test raycast finds ground
    let pos = world.get_character_position(char_id).unwrap();
    let state = world.get_character_state(char_id).unwrap();
    let hit = world.raycast_down(pos, 10.0, Some(state.body_handle));
    assert!(hit.is_some(), "Should detect floor");

    let (distance, ground_y) = hit.unwrap();
    // Character at Y=6, floor top at Y=0.5, distance = 5.5
    assert!(
        distance > 5.0 && distance < 6.0,
        "Distance to floor should be ~5.5, got {}",
        distance
    );
    assert!(
        (ground_y - 0.5).abs() < 0.1,
        "Ground Y should be ~0.5 (floor top), got {}",
        ground_y
    );

    // Test movement with controller translation (should not suppress horizontal)
    if let Some(state) = world.get_character_state_mut(char_id) {
        state.grounded = true;
    }
    let movement = world
        .move_character(char_id, [1.0, -0.05, 0.0], 1.0 / 60.0)
        .unwrap();
    world.step(1.0 / 60.0);

    let final_pos = world.get_character_position(char_id).unwrap();
    assert!(
        movement.translation.x.abs() > 0.0,
        "Horizontal movement should be applied"
    );
    assert!(final_pos[0] > 0.5, "Should have moved in X");
}

#[test]
fn test_character_horizontal_movement_when_grounded() {
    let mut world = PhysicsWorld::new();

    world.add_part(
        1,
        [0.0, 0.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 1.0, 100.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 200;
    world.add_character(char_id, [0.0, 3.0, 0.0], 1.0, 5.0);
    if let Some(state) = world.get_character_state_mut(char_id) {
        state.grounded = true;
    }

    let movement = world
        .move_character(char_id, [1.0, -0.05, 0.0], 1.0 / 60.0)
        .unwrap();
    assert!(
        movement.translation.x.abs() > 0.0 || movement.translation.z.abs() > 0.0,
        "Horizontal movement should not be suppressed when grounded"
    );
}

#[test]
fn test_characters_block_each_other() {
    let mut world = PhysicsWorld::new();

    world.add_part(
        1,
        [0.0, -0.5, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 1.0, 100.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_a = 300;
    let char_b = 301;
    world.add_character(char_a, [0.0, 2.6, 0.0], 1.0, 5.0);
    world.add_character(char_b, [2.6, 2.6, 0.0], 1.0, 5.0);

    if let Some(state) = world.get_character_state_mut(char_a) {
        state.grounded = true;
    }
    if let Some(state) = world.get_character_state_mut(char_b) {
        state.grounded = true;
    }

    let dt = 1.0 / 60.0;
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let movement = world.move_character(char_a, [1.0, 0.0, 0.0], dt).unwrap();
    world.step(dt);

    let final_pos = world.get_character_position(char_a).unwrap();
    assert!(
        movement.translation.x < 0.55,
        "Character should be blocked by another character, got {:?}",
        movement.translation
    );
    assert!(
        final_pos[0] < 0.55,
        "Character should not move through another character, final_pos={:?}",
        final_pos
    );
}

#[test]
fn test_character_ignores_sensor_colliders() {
    let mut world = PhysicsWorld::new();

    // Floor so the character can move while grounded.
    world.add_part(
        1,
        [0.0, -0.5, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 1.0, 100.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Non-collidable "trigger" directly in the movement path.
    world.add_part(
        2,
        [0.0, 1.0, -8.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [8.0, 2.0, 8.0],
        true,
        false, // sensor
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 300;
    world.add_character(char_id, [0.0, 2.6, 8.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;
    for _ in 0..120 {
        world.query_pipeline.update(&world.collider_set);
        world.move_character(char_id, [0.0, 0.0, -0.3], dt).unwrap();
        world.step(dt);
    }

    let final_pos = world.get_character_position(char_id).unwrap();
    assert!(
        final_pos[2] < -6.0,
        "Character should pass through CanCollide=false sensor. Final z={}",
        final_pos[2]
    );
}

#[test]
fn test_get_ground_kinematic_velocity() {
    let mut world = PhysicsWorld::new();

    // Moving platform under the character.
    let platform = world.add_part(
        1,
        [0.0, 0.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [10.0, 1.0, 10.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 301;
    world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    // Move the kinematic platform upward one step.
    world.set_kinematic_position(platform, [0.0, 0.2, 0.0]);
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let v = world
        .get_ground_kinematic_velocity(char_id, 4.0)
        .expect("Expected supporting kinematic velocity");
    assert!(
        v[1] > 0.0,
        "Platform upward velocity should be positive, got {:?}",
        v
    );
}

#[test]
fn test_get_ground_dynamic_support_reports_off_center_contact() {
    let mut world = PhysicsWorld::new();

    world.add_part(
        1,
        [2.0, 0.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [8.0, 1.0, 4.0],
        false,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 100;
    world.add_character(char_id, [4.0, 3.0, 0.0], 1.0, 5.0);
    world.step(1.0 / 60.0);
    world.query_pipeline.update(&world.collider_set);

    let support = world
        .get_ground_dynamic_support(char_id, 4.0)
        .expect("expected dynamic support under character");

    assert_eq!(support.lua_id, 1);
    assert!(
        support.point[0] > 3.9 && support.point[0] < 4.1,
        "support point should preserve off-center x, got {:?}",
        support.point
    );
    assert!(
        support.point[1] > 0.4 && support.point[1] < 0.6,
        "support point should be on plank top surface, got {:?}",
        support.point
    );
}

#[test]
fn test_off_center_support_load_creates_torque_on_dynamic_body() {
    let mut world = PhysicsWorld::new();

    let plank = world.add_part(
        1,
        [0.0, 0.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [8.0, 1.0, 4.0],
        false,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 100;
    world.add_character(char_id, [3.0, 3.0, 0.0], 1.0, 5.0);
    world.step(1.0 / 60.0);
    world.query_pipeline.update(&world.collider_set);

    let support = world
        .get_ground_dynamic_support(char_id, 4.0)
        .expect("expected dynamic support under character");
    world.add_force_at_position_to_part(1, [0.0, -1_000.0, 0.0], support.point);
    world.step(1.0 / 60.0);

    let angular_velocity = world
        .get_angular_velocity(plank)
        .expect("plank should still have a physics body");
    assert!(
        angular_velocity[2].abs() > 0.01,
        "off-center vertical load should create roll torque, got {:?}",
        angular_velocity
    );
}

#[test]
fn test_get_character_contact_kinematic_velocity_for_rotating_body() {
    let mut world = PhysicsWorld::new();

    let spinner = world.add_part(
        1,
        [0.0, 2.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [16.0, 4.0, 3.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 302;
    world.add_character(char_id, [0.0, 2.0, 2.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let theta = 0.5_f32;
    let c = theta.cos();
    let s = theta.sin();
    let rot_y = [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]];
    world.set_kinematic_rotation(spinner, &rot_y);
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let v = world
        .get_character_contact_kinematic_velocity(char_id)
        .expect("Expected contact velocity from rotating kinematic body");
    let horiz = (v[0] * v[0] + v[2] * v[2]).sqrt();
    assert!(
        horiz > 0.1,
        "Expected meaningful horizontal contact velocity, got {:?}",
        v
    );
}

#[test]
fn test_character_movement_after_landing_on_thin_platform() {
    let mut world = PhysicsWorld::new();

    // Floor at Y=0 (like the game)
    world.add_part(
        1,
        [0.0, -1.0, 0.0], // center at Y=-1
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 2.0, 100.0], // top at Y=0
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Thin platform at Y=0.1 (like base platform in tsunami game)
    world.add_part(
        2,
        [0.0, 0.1, 0.0], // center at Y=0.1
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [30.0, 0.2, 30.0], // top at Y=0.2
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Character spawns at Y=3 (above platform)
    let char_id = 100;
    world.add_character(char_id, [0.0, 3.0, 0.0], 1.0, 5.0);

    // Simulate falling (like the game does)
    let dt = 1.0 / 60.0;
    for _ in 0..30 {
        // About 0.5 seconds of falling
        world.query_pipeline.update(&world.collider_set);
        let gravity_movement: f32 = -196.2 * dt * dt; // Gravity acceleration
        world.move_character(char_id, [0.0, gravity_movement.max(-0.2), 0.0], dt);
        world.step(dt);
    }

    // Character should now be grounded
    let pos_after_landing = world.get_character_position(char_id).unwrap();
    println!("Position after landing: {:?}", pos_after_landing);

    // Try horizontal movement (like the game does)
    world.query_pipeline.update(&world.collider_set);
    let movement = world
        .move_character(char_id, [0.26, -0.02, 0.0], dt)
        .unwrap();

    println!("Movement result: {:?}", movement.translation);

    // This should NOT be blocked
    assert!(
        movement.translation.x.abs() > 0.1,
        "Horizontal movement should work after landing. Got: {:?}",
        movement.translation
    );
}

#[test]
fn test_character_direction_reversal() {
    let mut world = PhysicsWorld::new();

    // Floor at Y=0 (top surface)
    world.add_part(
        1,
        [0.0, -1.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [200.0, 2.0, 200.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Character with radius=1.0, height=5.0 (same as game)
    // Capsule: half_height=1.5, radius=1.0
    // Bottom of capsule is at Y = center - 1.5 - 1.0 = center - 2.5
    // For bottom to be at Y=0 (floor top), center should be at Y=2.5
    // Add small margin: Y=2.6
    let char_id = 100;
    world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;

    // Just update the query pipeline, don't fall
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let pos_after_landing = world.get_character_position(char_id).unwrap();
    println!("After landing: {:?}", pos_after_landing);

    // Move +X for 20 frames
    // When grounded, desired_y should be 0 (game logic zeros it)
    println!("\n=== Moving +X ===");
    let mut grounded = false;
    for i in 0..20 {
        world.query_pipeline.update(&world.collider_set);
        // When grounded, zero vertical like the game does
        let desired_y = if grounded { 0.0 } else { -0.008 };
        let movement = world
            .move_character(char_id, [0.2, desired_y, 0.0], dt)
            .unwrap();
        grounded = movement.grounded;
        world.step(dt);
        if i < 5 || movement.translation.x.abs() < 0.01 {
            let pos = world.get_character_position(char_id).unwrap();
            println!(
                "  Frame {}: pos={:.2} applied_x={:.3} grounded={}",
                i, pos[0], movement.translation.x, movement.grounded
            );
        }
    }

    let pos_after_plus_x = world.get_character_position(char_id).unwrap();
    println!("After +X: {:?}", pos_after_plus_x);
    assert!(pos_after_plus_x[0] > 2.0, "+X movement should work");

    // Now reverse: move -X for 20 frames
    println!("\n=== Moving -X (reversal) ===");
    let mut stuck_count = 0;
    for i in 0..20 {
        world.query_pipeline.update(&world.collider_set);
        // When grounded, zero vertical like the game does
        let desired_y = if grounded { 0.0 } else { -0.008 };
        let movement = world
            .move_character(char_id, [-0.2, desired_y, 0.0], dt)
            .unwrap();
        grounded = movement.grounded;
        world.step(dt);
        let pos = world.get_character_position(char_id).unwrap();
        println!(
            "  Frame {}: pos={:.2} applied_x={:.3} grounded={}",
            i, pos[0], movement.translation.x, movement.grounded
        );
        if movement.translation.x.abs() < 0.01 {
            stuck_count += 1;
        }
    }

    let pos_after_minus_x = world.get_character_position(char_id).unwrap();
    println!("After -X: {:?}", pos_after_minus_x);

    // Should have moved back toward 0
    assert!(
        pos_after_minus_x[0] < pos_after_plus_x[0] - 1.0,
        "Direction reversal should work! pos_after_plus_x={} pos_after_minus_x={} stuck_frames={}",
        pos_after_plus_x[0],
        pos_after_minus_x[0],
        stuck_count
    );
}

#[test]
fn test_character_autostep_over_small_obstacle() {
    let mut world = PhysicsWorld::new();

    // Floor
    world.add_part(
        1,
        [0.0, -0.5, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 1.0, 100.0], // top at Y=0
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Small obstacle (0.3 studs tall, should be steppable with max_height=0.5)
    world.add_part(
        2,
        [5.0, 0.15, 0.0], // center at Y=0.15
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 0.3, 4.0], // top at Y=0.3
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Character starts at X=0, grounded
    let char_id = 100;
    world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let start_pos = world.get_character_position(char_id).unwrap();
    println!("Start: {:?}", start_pos);

    // Move toward and over the obstacle
    for i in 0..60 {
        world.query_pipeline.update(&world.collider_set);
        let movement = world.move_character(char_id, [0.2, 0.0, 0.0], dt).unwrap();
        world.step(dt);

        let pos = world.get_character_position(char_id).unwrap();
        if i % 10 == 0 || (pos[0] > 4.0 && pos[0] < 6.0) {
            println!(
                "Frame {}: pos=({:.2}, {:.2}, {:.2}) grounded={}",
                i, pos[0], pos[1], pos[2], movement.grounded
            );
        }
    }

    let final_pos = world.get_character_position(char_id).unwrap();
    println!("Final: {:?}", final_pos);

    // Character should have moved past the obstacle (X > 6)
    assert!(
        final_pos[0] > 6.0,
        "Character should autostep over 0.3 stud obstacle. Final X={:.2}",
        final_pos[0]
    );
}

#[test]
fn test_character_blocked_by_tall_obstacle() {
    let mut world = PhysicsWorld::new();

    // Floor
    world.add_part(
        1,
        [0.0, -0.5, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 1.0, 100.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Tall obstacle (1.0 stud tall, should block with max_height=0.5)
    world.add_part(
        2,
        [5.0, 0.5, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 1.0, 4.0], // top at Y=1.0
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    let char_id = 100;
    world.add_character(char_id, [0.0, 2.6, 0.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    // Move toward the obstacle
    for _ in 0..60 {
        world.query_pipeline.update(&world.collider_set);
        world.move_character(char_id, [0.2, 0.0, 0.0], dt);
        world.step(dt);
    }

    let final_pos = world.get_character_position(char_id).unwrap();
    println!("Final pos with tall obstacle: {:?}", final_pos);

    // Character should be blocked (X < 5, can't step over 1.0 stud obstacle)
    assert!(
        final_pos[0] < 5.0,
        "Character should be blocked by 1.0 stud obstacle. Final X={:.2}",
        final_pos[0]
    );
}

#[test]
fn test_character_step_up_onto_platform() {
    // Mimics tsunami scenario: ground + raised platform
    let mut world = PhysicsWorld::new();

    // Ground (lower level) - extends UNDER the platform to avoid seam
    world.add_part(
        1,
        [0.0, -0.5, 0.0], // center
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [200.0, 1.0, 100.0], // top at Y=0, extends from X=-100 to X=100
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Raised platform (like tsunami base) - sits ON TOP of ground
    world.add_part(
        2,
        [50.0, 0.1, 0.0], // center at Y=0.1
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 0.2, 100.0], // top at Y=0.2, bottom at Y=0, X from 0 to 100
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Character starts on ground (left side), will try to step up onto platform
    let char_id = 100;
    world.add_character(char_id, [-5.0, 2.6, 0.0], 1.0, 5.0);

    let dt = 1.0 / 60.0;
    world.step(dt);
    world.query_pipeline.update(&world.collider_set);

    let start_pos = world.get_character_position(char_id).unwrap();
    println!("Start (on ground): {:?}", start_pos);

    // Move +X toward and onto the platform
    for i in 0..40 {
        world.query_pipeline.update(&world.collider_set);
        let movement = world.move_character(char_id, [0.2, 0.0, 0.0], dt).unwrap();
        world.step(dt);

        let pos = world.get_character_position(char_id).unwrap();
        if i % 5 == 0 || (pos[0] > -2.0 && pos[0] < 5.0) {
            println!(
                "Frame {}: pos=({:.2}, {:.2}, {:.2}) grounded={}",
                i, pos[0], pos[1], pos[2], movement.grounded
            );
        }
    }

    let final_pos = world.get_character_position(char_id).unwrap();
    println!("Final: {:?}", final_pos);

    // Character should step up onto platform (X > 2, past the edge at X=0)
    assert!(
        final_pos[0] > 2.0,
        "Character should step UP onto 0.2 stud platform. Final X={:.2}",
        final_pos[0]
    );
}

/// Reproduces the crash from run 20260331T191258Z:
/// When a character's HRP is removed via `remove_part()` (orphan cleanup),
/// the body is destroyed in rapier but `character_controllers` still holds
/// the stale handle. The next `move_character` or `step` then panics with
/// "No element at index".
#[test]
fn test_remove_part_on_character_hrp_causes_stale_handle() {
    let mut world = PhysicsWorld::new();

    // Ground
    world.add_part(
        1,
        [0.0, -0.5, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [200.0, 1.0, 200.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    // Add a character (like a player HRP)
    let char_id: u64 = 100;
    world.add_character(char_id, [0.0, 1.0, 0.0], 0.5, 2.0);
    assert!(world.has_character(char_id));

    // Simulate orphan cleanup: remove_part is called on the character's lua_id
    // This is what happens when sync_lua_to_physics finds the HRP gone from Lua
    let removed = world.remove_part(char_id);
    assert!(removed, "remove_part should find and remove the body");

    // BUG: character_controllers still has the entry with the stale handle
    assert!(
        !world.has_character(char_id),
        "character_controllers should be cleaned up when remove_part destroys a character body"
    );

    // If the assertion above is removed, the following would panic:
    // world.move_character(char_id, [1.0, 0.0, 0.0], 1.0 / 60.0);
    // world.step(1.0 / 60.0);
}

#[test]
fn test_remove_part_after_same_frame_mutation_does_not_panic_on_step() {
    let mut world = PhysicsWorld::new();

    let lua_id = 1;
    let handle = world.add_part(
        lua_id,
        [0.0, 1.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 1.0, 1.0],
        false,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    world.set_dynamic_position(handle, [1.0, 2.0, 3.0]);
    assert!(world.remove_part(lua_id));

    world.step(1.0 / 60.0);
    world.flush_pending_removals();

    assert!(!world.has_part(lua_id));
    assert!(world.get_handle(lua_id).is_none());
}

#[test]
fn test_remove_character_after_same_frame_mutation_does_not_panic_on_step() {
    let mut world = PhysicsWorld::new();

    let lua_id = 100;
    world.add_character(lua_id, [0.0, 3.0, 0.0], 1.0, 5.0);
    world.set_character_position(lua_id, [2.0, 4.0, 0.0]);
    assert!(world.remove_character(lua_id));

    world.step(1.0 / 60.0);
    world.flush_pending_removals();

    assert!(!world.has_character(lua_id));
    assert!(world.get_handle(lua_id).is_none());
}

#[test]
fn test_remove_part_after_same_frame_collider_rebuild_does_not_panic_on_step() {
    let mut world = PhysicsWorld::new();

    let lua_id = 7;
    world.add_part(
        lua_id,
        [0.0, 1.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 1.0, 1.0],
        false,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    world.set_size(lua_id, [2.0, 2.0, 2.0]);
    world.set_shape(lua_id, PartType::Ball, [2.0, 2.0, 2.0]);
    assert!(world.remove_part(lua_id));

    world.step(1.0 / 60.0);
    world.flush_pending_removals();

    assert!(!world.has_part(lua_id));
    assert!(world.get_handle(lua_id).is_none());
}

#[test]
fn test_scaled_rotation_matrix_is_normalized_before_physics() {
    let scaled_identity = [
        [0.111_069_754, 0.0, 0.0],
        [0.0, 0.111_069_77, 0.0],
        [0.0, 0.0, 0.111_069_78],
    ];

    let quat = rotation_matrix_to_quaternion(&scaled_identity);
    let rot = quaternion_to_rotation_matrix(&quat);

    assert!((rot[0][0] - 1.0).abs() < 1e-5);
    assert!((rot[1][1] - 1.0).abs() < 1e-5);
    assert!((rot[2][2] - 1.0).abs() < 1e-5);
    assert!(rot[0][1].abs() < 1e-5);
    assert!(rot[0][2].abs() < 1e-5);
    assert!(rot[1][0].abs() < 1e-5);
    assert!(rot[1][2].abs() < 1e-5);
    assert!(rot[2][0].abs() < 1e-5);
    assert!(rot[2][1].abs() < 1e-5);
}

#[test]
fn test_purge_invalid_body_removes_far_dynamic_part_before_step() {
    let mut world = PhysicsWorld::new();
    let lua_id = 42;
    world.add_part(
        lua_id,
        [PHYSICS_WORLD_ENVELOPE * 2.0, 0.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 1.0, 1.0],
        false,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    assert_eq!(world.purge_invalid_bodies(), 1);
    world.flush_pending_removals();
    assert!(world.get_handle(lua_id).is_none());
}

#[test]
fn test_invalid_velocity_write_removes_dynamic_part() {
    let mut world = PhysicsWorld::new();
    let lua_id = 43;
    let handle = world.add_part(
        lua_id,
        [0.0, 1.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 1.0, 1.0],
        false,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    world.set_velocity(handle, [f32::NAN, 0.0, 0.0]);
    world.flush_pending_removals();
    assert!(world.get_handle(lua_id).is_none());
}

#[test]
fn test_partial_single_part_sync_preserves_omitted_assemblies() {
    let mut world = PhysicsWorld::new();
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    let part_spec = |lua_id, position, position_dirty| PhysicsAssemblySpec {
        root_id: lua_id,
        parts: vec![PhysicsAssemblyPart {
            lua_id,
            position,
            rotation: identity,
            size: [1.0, 1.0, 1.0],
            anchored: false,
            can_collide: true,
            can_touch: true,
            can_query: true,
            collision_group: CollisionGroupMask::default(),
            shape: PartType::Block,
            mesh_id: None,
            physical_properties: None,
            velocity: [0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            position_dirty,
            cframe_dirty: false,
            size_dirty: false,
            anchored_dirty: false,
            interaction_dirty: false,
            velocity_dirty: false,
            angular_velocity_dirty: false,
            shape_dirty: false,
            custom_physical_properties_dirty: false,
        }],
    };

    world.sync_assemblies(
        vec![
            part_spec(1, [0.0, 1.0, 0.0], true),
            part_spec(2, [10.0, 1.0, 0.0], true),
        ],
        1.0 / 60.0,
    );

    world
        .sync_single_part_assemblies_partial(vec![part_spec(1, [2.0, 1.0, 0.0], true)], 1.0 / 60.0);

    assert_eq!(world.get_part_position(1), Some([2.0, 1.0, 0.0]));
    assert_eq!(world.get_part_position(2), Some([10.0, 1.0, 0.0]));
}

#[test]
fn test_inert_part_disables_collider_participation() {
    let mut world = PhysicsWorld::new();

    world.add_part(
        1,
        [0.0, 2.0, 0.0],
        &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [4.0, 4.0, 4.0],
        true,
        true,
        true,
        true,
        PartType::Block,
        None,
    );

    assert_eq!(world.collider_enabled_for_lua(1), Some(true));

    world.set_can_collide(1, false);
    assert_eq!(world.collider_enabled_for_lua(1), Some(true));

    world.set_can_touch(1, false);
    assert_eq!(world.collider_enabled_for_lua(1), Some(true));

    world.set_can_query(1, false);
    assert_eq!(world.collider_enabled_for_lua(1), Some(false));
}

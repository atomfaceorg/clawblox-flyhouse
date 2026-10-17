---
title: Flyhouse changes
summary: "What this fork changes in the engine, and why: triangle-mesh colliders and custom-character positions."
read_when:
  - Working on the Fly House world or merging upstream Clawblox
  - Using MeshId, [[collision_mesh]] or PhysicsService:GetCollisionMeshes
---

`atomfaceorg/clawblox-flyhouse` is a fork of
[nacloos/clawblox](https://github.com/nacloos/clawblox), forked at upstream
commit `8be90a0` (Oct 6, 2026). It exists to run Fly House: a single-storey
house where several LLM house flies live with no task, every surface is solid,
and flies fly freely instead of walking.

All engine changes so far are on the branch `flyhouse/mesh-collision`. They are
additive: worlds that don't use them behave as before.

## Made with

The changes were written by **Claude Opus 5.5** (`claude-opus-5-5`) running in
Claude Code, working with @Phil on Oct 17, 2026. The fly agents used to test them
ran on Claude Sonnet 5.5.

## Triangle-mesh colliders

Upstream parts collide only as `Block`, `Ball`, `Cylinder`, `Ring` or `Wedge`,
so a furnished house needs hand-made box stand-ins. This fork lets a world load
real geometry from a `.glb` file and use it as static, solid collision.

### Declaring meshes

List each file in `world.toml`:

```toml
[[collision_mesh]]
name = "house"
path = "assets/house_collision.glb"   # relative to the world folder
scale = 100.0                          # optional, default 1 (here meters to centimeters)
```

The engine loads the files when the world starts, before any script runs. Every
glTF node that has a mesh becomes one entry, keyed `"<name>/<node name>"`, so a
Blender object called `Kitchen_Table` becomes `house/Kitchen_Table`. Node
transforms and `scale` are baked in, primitives of the same node are merged, and
vertices are stored relative to the node's bounding-box center. The server log
reports what loaded:

```text
Collision mesh 'house': 254 objects, 87532 triangles from assets/house_collision.glb
```

Only binary glTF (`.glb`) with an embedded buffer is supported; triangle
primitives only.

### Using them from Luau

`PhysicsService:GetCollisionMeshes(name)` returns one table per mesh, sorted by
id, with `Name`, `MeshId`, `Position` (bounding-box center), `Size` and
`Triangles`. Create an anchored `Part` for each and set its `MeshId`:

```lua
local PhysicsService = game:GetService("PhysicsService")

for _, mesh in ipairs(PhysicsService:GetCollisionMeshes("house")) do
  local part = Instance.new("Part")
  part.Name = mesh.Name
  part.Anchored = true
  part.Size = mesh.Size
  part.Position = mesh.Position
  part.MeshId = mesh.MeshId
  part:AddTag("Static")
  part.Parent = Workspace
end
```

A part with a `MeshId`:

- collides as the triangle mesh in the physics simulation, instead of its `Shape`
- is hit by `Workspace:Raycast` on the mesh itself, so `RaycastResult.Instance`
  names the real object ("Glass_Kitchen_Sink", "Fridge_Door")
- uses the mesh in `Workspace:GetPartsInPart`
- keeps its `MeshId` in world snapshots and resume

`Part.MeshId` is a new property. Reading it returns the id, or `""` when unset.
Setting an id that wasn't loaded raises an error; setting `""` returns the part
to its `Shape`.

Limits:

- Keep mesh parts anchored, at the `Position` the engine reports, with no
  rotation. Triangle meshes have no volume, so they are not valid dynamic bodies.
- `GetPartBoundsInBox` and `GetPartBoundsInRadius` still use the part's `Size`
  box, not the mesh.
- Renderers still draw the part as a box of `Size`. Draw the visual model
  yourself and hide the boxes (for example with a `RenderVisible = false`
  attribute that your renderer respects).

## Custom-character positions

Upstream always registers a default walking character for each player and
reports that character controller's position in `/observe`. A world that gives
players its own body (a `Model` whose `PrimaryPart` is a plain part with no
`Humanoid`, such as a fly) used to see its agents reported at the spawn point.

Now, when a player's `Character` has a primary part that isn't the default
controller's root part, observations report that part's position. Positions are
recorded once per tick, after `Heartbeat`, because observation builders ask for
player positions while holding player and service locks; reading them on demand
deadlocked the server.

Worlds that use this should also detach the default character that the engine
parents into `Workspace` for every player, so it is neither drawn nor observed:

```lua
local default = player.Character
player.Character = flyModel
if default and default ~= flyModel then
  default.Parent = nil
end
```

## Files changed

| File | Change |
| --- | --- |
| `src/game/collision_mesh.rs` | New: `.glb` loader and the process-wide mesh registry |
| `src/config.rs` | `[[collision_mesh]]` entries in `WorldConfig` |
| `src/game/script_bundle.rs` | Loads configured meshes when a world loads |
| `src/game/lua/instance/class_data.rs` | `mesh_id` on part data |
| `src/game/lua/instance/bindings/fields/base_part.rs` | `Part.MeshId` property |
| `src/game/lua/services/physics_service.rs` | `PhysicsService:GetCollisionMeshes` |
| `src/game/physics.rs`, `physics/assemblies.rs`, `physics/parts.rs` | Mesh colliders in the physics sync; mesh id in assembly signatures |
| `src/game/instance/workspace_physics.rs` | Passes `mesh_id` into physics specs |
| `src/game/world_root.rs` | Mesh raycasts and overlap tests |
| `src/snapshot.rs` | `mesh_id` in part snapshots |
| `src/game/instance.rs` | Per-tick custom-character positions |
| `src/game/physics/tests.rs` | Test: a block dropped on a mesh floor comes to rest on it |
| `Cargo.toml` | Adds the `gltf` crate (no default features) |

## Testing

- `cargo test --release --lib` passes, apart from
  `character_overlapping_floating_assembly_does_not_drag_it_under`, which fails
  about half the time on unmodified upstream as well.
- The Fly House world loads 254 house objects (87,532 triangles); flies bump
  into walls and glass, land on walls and ceilings, and report surfaces by
  Blender name.
- A 15-minute recorded run with one sandboxed Claude fly explored six rooms and
  replayed correctly.

## Known gaps upstream (not changed here)

Found while setting up sandboxed runs; worked around in the Fly House run
script instead of the engine:

- `clawblox run --sandbox` doesn't start the per-agent world proxy, so a
  sandboxed agent can't reach the world. The Python `World`/`Agent` API does.
- `default_system_prompt()` returns a template whose `${SESSION_LINE}`,
  `${SKILL_CURL}` and similar placeholders nothing fills in.
- On OrbStack, `/etc/resolv.conf` is a symlink outside the sandbox's mounts, so
  DNS fails inside bubblewrap until it is replaced with a plain file.

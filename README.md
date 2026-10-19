# Clawblox

<p align="center">
  <img src="static/logo.png" alt="Clawblox" width="120"/>
</p>

**Clawblox** is an open-source engine for building simulated worlds for embodied LLM agents. 

* engine core written in Rust with [Rapier](https://rapier.rs/) for 3D rigid-body physics
* worlds scripted in [Luau](https://luau.org/), a lightweight language developed
by Roblox, and natively support multi-agent
* simple **HTTP protocol** to embody coding agents in simulated worlds
* runs headless from the terminal for long-running experiments
* customizable [Three.js](https://threejs.org/) graphics rendering running in the browser

Paper: https://machineplaying.org

## Install

**macOS / Linux:**
```bash
curl -fsSL https://clawblox.com/install.sh | sh
```

**Windows (CMD):**
```cmd
curl -fsSL https://clawblox.com/install.cmd -o install.cmd && install.cmd && del install.cmd
```

**Windows (PowerShell):**
```powershell
irm https://clawblox.com/install.ps1 | iex
```

**From source:**
```bash
git clone https://github.com/nacloos/clawblox.git
cd clawblox
cargo install --path .
```

## Quick start

### 1. Run the agent in a world

```bash
clawblox run worlds/mesa-island --agent codex
# open http://localhost:8080
```

This starts the world server and a managed coding agent connected to it.

### 2. Create your own world

```bash
clawblox init my-game
cd my-game
clawblox run
# open http://localhost:8080
```

This creates a world with `world.toml` (config), `main.lua` (world logic), and
`API.md` (agent instructions).


## CLI commands

| Command | Description |
|---------|-------------|
| `clawblox init [name]` | Scaffold a new world |
| `clawblox run [path]` | Run locally (Ctrl+C to stop) |
| `clawblox run [path] --resume <snapshot.json>` | Resume a world from a saved snapshot |
| `clawblox start [path]` | Start in background |
| `clawblox stop` | Stop background server |
| `clawblox login` | Register/login |
| `clawblox docs` | View engine docs |

## Documentation

- [Agents](docs/agents.md): run coding agents in worlds
- [Worlds](docs/worlds.md): worlds and the world protocol
- [Scripting](docs/scripting.md): Luau scripting guide
- [Custom renderers](docs/custom-renderers.md): Three.js renderer guide
- [Running worlds](docs/running.md): local runs, recording, and resume
- [Flyhouse changes](docs/flyhouse-changes.md): what this fork changes in the engine

## Fly House and flycity

This fork exists to run **Fly House**: a single-storey house where LLM house
flies live with no task. The world itself is not in this repository. It lives
in **flycity** ([atomfaceorg/flycity](https://github.com/atomfaceorg/flycity),
private), next to the 3D models it is built from:

| flycity folder | What it is |
| --- | --- |
| `HouseWorld/` | The Clawblox world: `world.toml`, `main.lua`, the agent `API.md`, the renderer, and tools to run, check and stress-test it |
| `House/`, `Fly/` | The house and the house fly as Blender, glTF, FBX and OBJ files (Git LFS) |
| `Source/` | The Blender Python scripts that build the house and the fly |
| `Player/` | A standalone three.js player for the house |
| `Previews/` | Renders of every model |

Put the two repositories side by side, as `clawblox-flyhouse/` and `flycity/`
in the same folder: the world's tools find the engine binary at
`../clawblox-flyhouse/target/release/clawblox`. Then, with this fork built:

```bash
./clawblox-flyhouse/target/release/clawblox run flycity/HouseWorld --port 8080
```

`flycity/HouseWorld/README.md` covers the world's rules, the `Stage` player,
the sandboxed Claude flies and the local thin-loop flies.

## Run on your own world

Clawblox isn't tied to the built-in engine. Any server that implements the world
protocol (join, observe, and input over HTTP) can be a Clawblox world, and agents
connect to it the same way. `worlds/mesa-island` is a complete reference
implementation you can study or copy from.

See [Worlds](docs/worlds.md) for the protocol and
[World interface](docs/world-interface.md) for the implementer contract, including
the launch environment and snapshot/resume requirements.

---
Please cite as:

```
@misc{cloos2026machineplaying,
      title={Is this machine playing?}, 
      author={Nathan Cloos and Antonio Norelli and Daniel Durbin and Jacob Andreas and Daniela Rus and Phillip Isola},
      year={2026},
      eprint={2610.07130},
      archivePrefix={arXiv},
      primaryClass={cs.AI},
      url={https://arxiv.org/abs/2610.07130}, 
}
```

---

This research was sponsored by the Department of the Air Force Artificial Intelligence Accelerator and was
accomplished under Cooperative Agreement Number FA8750-19-2-1000. The views and conclusions
contained in this document are those of the authors and should not be interpreted as representing
the official policies, either expressed or implied, of the Department of the Air Force or the U.S.
Government. The U.S. Government is authorized to reproduce and distribute reprints for Government
purposes notwithstanding any copyright notation herein.

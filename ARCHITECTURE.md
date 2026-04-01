# Architecture

## Bird's Eye View

`magicrate` is a small Godot 4 puzzle-platformer whose gameplay runtime is split between Godot scenes and a Rust GDExtension. Godot owns the project shell, imported assets, and editor-time pipelines. Rust owns the custom node classes, live stage orchestration, and most gameplay rules.

At boot, `game.tscn` instantiates `Game` and `LevelRuntime`. `LevelRuntime` discovers LDtk-imported room scenes, loads the current room, spawns `PlayerController`, and manages snapshot-based undo. During play, `PlayerController` handles movement and crate pushes, `LevelRuntime` updates crate falling plus bridge/goal/death state, and `Game` turns those runtime signals into restart or next-stage transitions. Engine-agnostic rules live in the `gameplay_core` crate and are adapted to Godot nodes by the root Rust crate.

## Code Map

### Godot Project

- `godot/`
  Purpose: Godot project root and runtime entrypoint.
  Key files: `project.godot`, `game.tscn`, `rust.gdextension`.
  Relationships: Loads the compiled Rust library, enables the editor plugins, and instantiates the top-level `Game` and `LevelRuntime` nodes.

- `godot/player/` and `godot/entity/`
  Purpose: Authored scene templates for Rust-backed gameplay nodes.
  Key files: `player.tscn`, `pushable_crate.tscn`, `bridge_switch.tscn`, `bridge_tile.tscn`, `goal_petal.tscn`.
  Relationships: Referenced by the LDtk post-import scripts and by `LevelRuntime::spawn_player_for_stage`; their node types and groups must match the Rust `SceneContract`.

- `godot/pipeline/ldtk/`
  Purpose: Level source, post-import customization, and exported stage ordering.
  Key files: `levels.ldtk`, `entities_post_import.gd`, `level_post_import.gd`, `stage_manifest.txt`, `levels/Room_*.scn`.
  Relationships: Uses the vendored LDtk importer to generate room scenes, replaces LDtk entity placeholders with project scenes, creates collision and marker nodes from `IG_Rules`, and feeds `LevelRuntime` the scenes it loads at runtime.

- `godot/pipeline/aseprite/`
  Purpose: Art source files and generated animation resources.
  Key files: `src/player.aseprite`, `wizard/player.res`, `scripts/player.lua`.
  Relationships: Uses the Aseprite plugin to generate the `SpriteFrames` consumed by `player.tscn`.

- `godot/addons/`
  Purpose: Vendored editor plugins.
  Key files: `ldtk-importer`, `AsepriteWizard`.
  Relationships: Enabled by `project.godot`; project-specific import behavior is layered on top in `godot/pipeline/*`, not by changing plugin internals.

### Rust Workspace

- `rust/`
  Purpose: Workspace root and GDExtension crate.
  Key files: `Cargo.toml`, `src/lib.rs`.
  Relationships: Builds the Godot-facing Rust library, depends on `gameplay_core`, and exports the classes used by the Godot scenes.

- `rust/src/game/`
  Purpose: Top-level session controller.
  Key files: `Game` in `game/mod.rs`, `gameplay_core::game_flow::GameState`.
  Relationships: Listens to `LevelRuntime` signals, reads global restart/undo/jump input, and delegates stage loading or rewinding back to `LevelRuntime`.

- `rust/src/level/`
  Purpose: Stage loading, entity discovery, environment updates, and undo application.
  Key files: `LevelRuntime` in `level/mod.rs`, `catalog.rs`, `scene_ops.rs`, `snapshot_io.rs`, `crate_updates.rs`, `stage_state.rs`.
  Relationships: Loads room scenes through `rooms`, spawns the player scene, queries tile rules and entity groups from Godot, and applies `gameplay_core` helpers for stage ordering, crate runtime, and snapshots.

- `rust/src/player/`
  Purpose: Player movement, pushing, collision checks, and animation state.
  Key files: `PlayerController` in `player/mod.rs`, `collision.rs`, `movement.rs`, `push.rs`, `visual.rs`.
  Relationships: Reads Godot input actions and imported rule layers, consults crate and bridge geometry from the scene tree, delegates push math to `gameplay_core::player_logic`, and emits `crate_pushed` so `LevelRuntime` can snapshot after a settled move.

- `rust/src/entity/`
  Purpose: Rust-backed node classes for interactive stage objects.
  Key files: `BridgeSwitch`, `GoalPetal`, `BridgeTile`, `PushableCrate`.
  Relationships: Attached to authored Godot scenes, use `gameplay_core::activation` where applicable, and are polled by `LevelRuntime` through group membership.

- `rust/src/config.rs`, `rust/src/core_bridge.rs`, `rust/src/rooms/`
  Purpose: Shared adapter layer between engine-agnostic rules and Godot resources.
  Key files: `SCENE_CONTRACT`, `core_vec`, `godot_vec`, `load_scene`, `instantiate_scene`.
  Relationships: Centralizes resource paths, group names, input action names, and vector conversion so the rest of the root crate can talk to Godot consistently.

- `rust/gameplay_core/`
  Purpose: Deterministic gameplay rules and data structures with no engine dependency.
  Key files: `game_flow.rs`, `player_logic.rs`, `crate_runtime.rs`, `snapshot.rs`, `stage_paths.rs`, `activation.rs`, `undo_history.rs`, `vec2.rs`.
  Relationships: Consumed by the root crate for transition state, crate falling, push-chain resolution, activation counting, undo history, and room-file ordering.

### Tooling

- `scripts/`
  Purpose: Local automation for build, run, export, and dependency updates.
  Key files: `run.ps1`, `export.ps1`, `update_gdext.ps1`.
  Relationships: Build the Rust extension, launch the Godot project, refresh `stage_manifest.txt` before export, normalize extension metadata, and update the pinned `godot-rust` revision.

## Architectural Invariants

- `gameplay_core` stays Godot-free. All node lookup, signal wiring, scene mutation, and resource loading live in the root `rust` crate.
- `SCENE_CONTRACT` is the handshake between runtime code and imported/authored Godot content. Group names, node paths, layer names, scene paths, and input action names must stay aligned across Rust, `project.godot`, and the LDtk post-import scripts.
- Stage content comes from imported room scenes in `godot/pipeline/ldtk/levels`, not from hand-built runtime construction. `LevelRuntime` prefers `stage_manifest.txt` for ordering and only falls back to filename sorting when the manifest is missing.
- Project-specific importer behavior belongs in `godot/pipeline/*`. `godot/addons/*` is treated as vendored dependency code.
- Undo is snapshot-based. The runtime captures `StageSnapshot` data for the player and crates and restores from that history instead of replaying inputs or rebuilding stages from scratch.

## Cross-Cutting Concerns

- Runtime coordination relies on Godot signals and groups rather than direct references serialized into every scene. `Game` listens to `LevelRuntime`; `LevelRuntime` and `PlayerController` discover crates, switches, bridge tiles, goals, and hazard markers through scene groups.
- The import pipeline is part of the architecture, not just content tooling. LDtk post-import scripts generate collision and marker nodes that runtime Rust code queries by name and group; Aseprite output feeds the player scene's animation resources.
- Testing is Rust-first. Unit tests live inline next to the relevant gameplay modules, especially in `gameplay_core`; scene, import-pipeline, and export changes still need a manual smoke run through `scripts/run.ps1`.
- `scripts/export.ps1` is the release path. It regenerates the stage manifest, checks export metadata, runs Godot headless export, and ensures the release `rust.dll` ships next to the exported executable.

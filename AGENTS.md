# Repository Guidelines

## Project Structure & Module Organization
`godot/` houses the Godot 4 project. `game.tscn` is the entry scene; `entity/` and `player/` contain authored scenes; `pipeline/ldtk/` and `pipeline/aseprite/` hold imported level and art sources plus post-import scripts. Treat `godot/addons/` as vendored plugin code; only edit it when intentionally updating a dependency.

`rust/` is a workspace. `rust/src/` contains the GDExtension bridge and Godot-facing systems, while `rust/gameplay_core/src/` keeps engine-agnostic rules, snapshots, stage paths, and undo logic. `xtask/` contains the Rust workflow CLI used by `cargo xtask`; `scripts/` contains PowerShell compatibility wrappers and small local helpers. `export/` is generated output and ignored.

## Build, Test, and Development Commands
Use PowerShell from the repo root.

`cargo xtask run` builds the debug Rust extension and launches the game. `./scripts/run.ps1 -Build Debug` remains as a PowerShell compatibility wrapper.

`cargo xtask run --build none --editor` opens the Godot editor without rebuilding Rust.

`cd rust; cargo test --workspace` runs inline Rust unit tests across both crates.

`cd rust; cargo fmt --all` formats Rust sources with `rustfmt`.

`cargo xtask export` creates a Windows build in `export/`, refreshes the stage manifest, and copies the release DLL. `./scripts/export.ps1` forwards to the same command.

`cargo xtask update-gdext` updates the pinned `godot-rust` revision, and `cargo xtask update-godot-addons` refreshes vendored Godot add-ons.

## Coding Style & Naming Conventions
Rust uses `rustfmt` defaults: 4-space indentation, `snake_case` modules and functions, `PascalCase` types. Keep deterministic gameplay logic in `gameplay_core`; keep scene orchestration, node classes, and Godot bridge code in the root crate.

GDScript follows the existing tab-indented style, uppercase `const` names, and `_helper_name` internals. Match existing asset names such as `bridge_switch.tscn`, `pushable_crate.rs`, and `Room_0_0.scn`.

## Testing Guidelines
Place Rust tests beside the code they cover with `#[cfg(test)] mod tests`. There is no numeric coverage gate, but new gameplay rules should include focused unit tests in `gameplay_core`. For scene, import-pipeline, or export changes, also do a manual smoke run with `./scripts/run.ps1`.

## Commit & Pull Request Guidelines
Recent commits use scoped, imperative subjects like `player: simplify collision` and `export: support packaged Windows builds`. Follow `<area>: <summary>`.

Pull requests should explain gameplay or tooling impact, list the commands run, link related issues, and include screenshots or short clips for visible Godot changes.

# Repository Guidelines

## Project Structure & Module Organization
`godot/` is the Godot 4.6 project. `game.tscn` is the entry scene, `entity/` and `player/` hold scene files, and `pipeline/ldtk/` contains LDtk data, post-import scripts, and generated room scenes. `godot/addons/` contains vendored plugins (`ldtk-importer`, `AsepriteWizard`); only edit these when intentionally updating vendor code.

`rust/` is the Rust workspace. The root crate builds the GDExtension bridge in `rust/src/`, while `rust/gameplay_core/src/` holds engine-agnostic gameplay logic such as movement, snapshots, undo history, and stage path handling. `scripts/` contains PowerShell entry points for local run, export, and `gdext` revision updates.

## Build, Test, and Development Commands
`./scripts/run.ps1 -Build Debug` builds the Rust extension and launches Godot from `godot/`.

`./scripts/run.ps1 -Build None -Editor` opens the editor without rebuilding Rust.

`cd rust; cargo test --workspace` runs the Rust unit tests, including the inline tests in `gameplay_core`.

`cd rust; cargo fmt --all` formats Rust code with standard `rustfmt`.

`./scripts/export.ps1` produces a Windows export in `export/` and refreshes the LDtk stage manifest before packaging.

## Coding Style & Naming Conventions
Rust follows `rustfmt` defaults: 4-space indentation, `snake_case` functions/modules, and `PascalCase` types. Keep pure rules and state transitions in `gameplay_core`; keep Godot-facing node registration, scene orchestration, and bridge code in the root crate.

GDScript uses tabs, uppercase `const` names, and `_prefixed` helpers for internal functions. Match existing scene and asset naming such as `bridge_switch.tscn` and `Room_0_0.scn`.

## Testing Guidelines
Prefer inline Rust tests with `#[cfg(test)] mod tests` next to the code they cover. New gameplay logic should land in `gameplay_core` with behavior-focused test names such as `push_dedup_applies_capacity`. For scene or import-pipeline changes, also run a local Godot launch check with `./scripts/run.ps1`.

## Commit & Pull Request Guidelines
Current history uses short scoped subjects like `core: extract gameplay_core crate` and `export: support packaged Windows builds`. Follow `<area>: <imperative summary>`.

Pull requests should state the gameplay or tooling change, list the commands run, link related issues, and include screenshots or short clips for visible Godot changes.

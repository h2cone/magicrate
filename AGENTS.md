# Repository Guidelines

## Project Structure & Module Organization
- `rust/` is the Godot GDExtension crate.
  - `src/core/`: pure gameplay logic (activation, crate runtime, stage sorting, undo helpers) with unit tests.
  - `src/entity/`: Godot node classes such as `BridgeSwitch`, `BridgeTile`, `GoalPetal`, and `PushableCrate`.
  - `src/game/`, `src/level/`, `src/player/`, `src/rooms/`, `src/undo/`: runtime orchestration and scene integration.
- `godot/` is the Godot 4.6 project.
  - `game.tscn` is the main scene; `entity/` and `player/` hold scene files.
  - `pipeline/ldtk/` contains LDtk post-import scripts and generated room assets.
  - `addons/` contains third-party plugins (`ldtk-importer`, `AsepriteWizard`); avoid editing unless intentionally upgrading vendor code.
- `docs/p8/` stores prototype references.

## Build, Test, and Development Commands
- `cd rust && cargo build` — builds `target/debug/librust.*` used by `godot/rust.gdextension`.
- `cd rust && cargo test` — runs Rust unit tests.
- `cd rust && cargo fmt --check` — verifies Rust formatting.
- `/Applications/Godot.app/Contents/MacOS/godot --path godot` — run the project locally.
- `/Applications/Godot.app/Contents/MacOS/godot --path godot --headless --quit` — quick headless startup check.

## Coding Style & Naming Conventions
- Rust: follow `rustfmt` defaults (4-space indentation, snake_case functions/tests, PascalCase types).
- GDScript: match existing style (`const` in uppercase, internal helpers prefixed with `_`, tabs in `.gd` files).
- Keep deterministic logic in `rust/src/core/*`; keep Godot node lifecycle logic in `entity/`, `level/`, and `player/` modules.

## Testing Guidelines
- Use inline Rust tests (`#[cfg(test)] mod tests`) near the code under test.
- Prefer behavior-focused snake_case names, e.g. `transition_tick_requests_next_stage_when_timer_ends`.
- For gameplay or scene changes, run `cargo test` and at least one Godot startup/playthrough check before submitting.
- No formal coverage target exists; add tests for all new non-trivial logic.

## Commit & Pull Request Guidelines
- Git history is currently minimal (`Initial commit`), so use short, imperative commit subjects.
- Suggested commit format: `<area>: <change>` (example: `level: fix bridge activation state sync`).
- PRs should include: purpose, key file paths changed, test commands run, linked issue (if any), and screenshot/GIF for visible gameplay updates.

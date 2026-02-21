use godot::prelude::*;

mod core;
mod entity;
mod game;
mod level;
mod player;
mod rooms;
mod undo;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

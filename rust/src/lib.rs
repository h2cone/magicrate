use godot::prelude::*;

mod config;
mod core_bridge;
mod entity;
mod game;
mod level;
mod player;
mod rooms;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

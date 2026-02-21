use godot::{
    classes::{Node2D, PackedScene},
    prelude::*,
};

pub struct StageLoader;

impl StageLoader {
    pub fn load_scene(path: &str) -> Option<Gd<PackedScene>> {
        try_load::<PackedScene>(path).ok()
    }

    pub fn instantiate_scene(path: &str) -> Option<Gd<Node2D>> {
        let scene = Self::load_scene(path)?;
        let instance = scene.instantiate()?;
        instance.try_cast::<Node2D>().ok()
    }
}

use gameplay_core::Vec2;
use godot::builtin::Vector2;

pub fn core_vec(value: Vector2) -> Vec2 {
    Vec2::new(value.x, value.y)
}

pub fn godot_vec(value: Vec2) -> Vector2 {
    Vector2::new(value.x, value.y)
}

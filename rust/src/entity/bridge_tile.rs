use godot::classes::{IStaticBody2D, StaticBody2D};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=StaticBody2D)]
pub struct BridgeTile {
    #[base]
    base: Base<StaticBody2D>,
    active: bool,
}

#[godot_api]
impl IStaticBody2D for BridgeTile {
    fn init(base: Base<StaticBody2D>) -> Self {
        Self {
            base,
            active: false,
        }
    }

    fn ready(&mut self) {
        self.apply_state();
    }
}

#[godot_api]
impl BridgeTile {
    #[func]
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        self.apply_state();
    }

    #[func]
    pub fn is_active(&self) -> bool {
        self.active
    }

    fn apply_state(&mut self) {
        let active = self.active;
        self.base_mut()
            .set_collision_layer(if active { 1 } else { 0 });
        self.base_mut().set_visible(active);
    }
}

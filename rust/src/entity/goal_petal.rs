use godot::classes::{Area2D, IArea2D};
use godot::prelude::*;

use crate::core::activation::{ActivationChange, ActivationCounter};

#[derive(GodotClass)]
#[class(base=Area2D)]
pub struct GoalPetal {
    #[base]
    base: Base<Area2D>,
    counter: ActivationCounter,
}

#[godot_api]
impl IArea2D for GoalPetal {
    fn init(base: Base<Area2D>) -> Self {
        Self {
            base,
            counter: ActivationCounter::default(),
        }
    }

    fn ready(&mut self) {
        self.signals()
            .body_entered()
            .connect_self(Self::on_body_entered);
        self.signals()
            .body_exited()
            .connect_self(Self::on_body_exited);
    }
}

#[godot_api]
impl GoalPetal {
    #[signal]
    fn activated(active: bool);

    #[func]
    pub fn is_active(&self) -> bool {
        self.counter.is_active()
    }

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node2D>) {
        let relevant = Self::is_trigger_body(&body);
        if self.counter.on_enter(relevant) == ActivationChange::Activated {
            self.signals().activated().emit(true);
        }
    }

    #[func]
    fn on_body_exited(&mut self, body: Gd<Node2D>) {
        let relevant = Self::is_trigger_body(&body);
        if self.counter.on_exit(relevant) == ActivationChange::Deactivated {
            self.signals().activated().emit(false);
        }
    }

    fn is_trigger_body(body: &Gd<Node2D>) -> bool {
        body.is_in_group("crate")
    }
}

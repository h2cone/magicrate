use godot::{
    classes::{INode, Input, Node},
    prelude::*,
};

use crate::{core::game_flow::GameState, level::LevelRuntime};

#[derive(GodotClass)]
#[class(base=Node)]
pub struct Game {
    base: Base<Node>,
    level_runtime: OnReady<Gd<LevelRuntime>>,
    state: GameState,
}

#[godot_api]
impl INode for Game {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            level_runtime: OnReady::from_node("LevelRuntime"),
            state: GameState::default(),
        }
    }

    fn ready(&mut self) {
        let game = self.to_gd();

        self.level_runtime
            .signals()
            .stage_cleared()
            .connect_other(&game, Self::on_stage_cleared);

        self.level_runtime
            .signals()
            .player_died()
            .connect_other(&game, Self::on_player_died);

        self.level_runtime
            .signals()
            .stage_loaded()
            .connect_other(&game, Self::on_stage_loaded);
    }

    fn process(&mut self, delta: f64) {
        let input = Input::singleton();

        let restart_pressed = input.is_action_just_pressed("act_restart");
        let undo_pressed = input.is_action_just_pressed("act_undo");
        let jump_pressed = input.is_action_just_pressed("act_jump");

        if self.state.restart_requested(restart_pressed, jump_pressed)
            && self.level_runtime.bind_mut().restart_current_stage()
        {
            self.state = self.state.on_restart_succeeded();
        }

        if self.state.undo_requested(undo_pressed) {
            let _ = self.level_runtime.bind_mut().request_undo();
        }

        let tick = self.state.tick_transition(delta);
        if tick.should_load_next_stage {
            let _ = self.level_runtime.bind_mut().load_next_stage();
        }
        self.state = tick.state;
    }
}

#[godot_api]
impl Game {
    #[func]
    fn on_stage_cleared(&mut self) {
        self.state = self.state.on_stage_cleared();
    }

    #[func]
    fn on_player_died(&mut self) {
        self.state = self.state.on_player_died();
    }

    #[func]
    fn on_stage_loaded(&mut self, _index: i64) {
        self.state = self.state.on_stage_loaded();
    }
}

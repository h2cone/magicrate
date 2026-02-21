#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Playing,
    Over,
    Transition,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameState {
    pub mode: GameMode,
    pub transition_timer: f64,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            mode: GameMode::Playing,
            transition_timer: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionTick {
    pub state: GameState,
    pub should_load_next_stage: bool,
}

impl GameState {
    pub const STAGE_TRANSITION_SECONDS: f64 = 0.8;

    pub fn on_stage_cleared(self) -> Self {
        Self {
            mode: GameMode::Transition,
            transition_timer: Self::STAGE_TRANSITION_SECONDS,
        }
    }

    pub fn on_player_died(self) -> Self {
        Self {
            mode: GameMode::Over,
            transition_timer: 0.0,
        }
    }

    pub fn on_stage_loaded(self) -> Self {
        Self {
            mode: GameMode::Playing,
            transition_timer: 0.0,
        }
    }

    pub fn restart_requested(&self, restart_pressed: bool, jump_pressed: bool) -> bool {
        restart_pressed || (self.mode == GameMode::Over && jump_pressed)
    }

    pub fn undo_requested(&self, undo_pressed: bool) -> bool {
        undo_pressed && self.mode == GameMode::Playing
    }

    pub fn on_restart_succeeded(self) -> Self {
        Self {
            mode: GameMode::Playing,
            transition_timer: 0.0,
        }
    }

    pub fn tick_transition(self, delta: f64) -> TransitionTick {
        if self.mode != GameMode::Transition {
            return TransitionTick {
                state: self,
                should_load_next_stage: false,
            };
        }

        let timer = self.transition_timer - delta;
        if timer <= 0.0 {
            return TransitionTick {
                state: self.on_stage_loaded(),
                should_load_next_stage: true,
            };
        }

        TransitionTick {
            state: Self {
                transition_timer: timer,
                ..self
            },
            should_load_next_stage: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GameMode, GameState};

    #[test]
    fn stage_cleared_enters_transition_mode() {
        let state = GameState::default().on_stage_cleared();

        assert_eq!(state.mode, GameMode::Transition);
        assert!((state.transition_timer - GameState::STAGE_TRANSITION_SECONDS).abs() < 1e-9);
    }

    #[test]
    fn player_died_enters_over_mode() {
        let state = GameState::default().on_player_died();

        assert_eq!(state.mode, GameMode::Over);
        assert_eq!(state.transition_timer, 0.0);
    }

    #[test]
    fn restart_requested_by_restart_button_in_any_mode() {
        let over = GameState {
            mode: GameMode::Over,
            transition_timer: 0.0,
        };
        let transition = GameState {
            mode: GameMode::Transition,
            transition_timer: 0.4,
        };

        assert!(over.restart_requested(true, false));
        assert!(transition.restart_requested(true, false));
    }

    #[test]
    fn restart_requested_by_jump_only_in_over_mode() {
        let playing = GameState::default();
        let over = GameState {
            mode: GameMode::Over,
            transition_timer: 0.0,
        };

        assert!(!playing.restart_requested(false, true));
        assert!(over.restart_requested(false, true));
    }

    #[test]
    fn undo_only_allowed_while_playing() {
        let playing = GameState::default();
        let over = GameState {
            mode: GameMode::Over,
            transition_timer: 0.0,
        };

        assert!(playing.undo_requested(true));
        assert!(!over.undo_requested(true));
    }

    #[test]
    fn transition_tick_counts_down_without_loading() {
        let state = GameState {
            mode: GameMode::Transition,
            transition_timer: 0.8,
        };

        let tick = state.tick_transition(0.2);

        assert!(!tick.should_load_next_stage);
        assert_eq!(tick.state.mode, GameMode::Transition);
        assert!((tick.state.transition_timer - 0.6).abs() < 1e-9);
    }

    #[test]
    fn transition_tick_requests_next_stage_when_timer_ends() {
        let state = GameState {
            mode: GameMode::Transition,
            transition_timer: 0.2,
        };

        let tick = state.tick_transition(0.3);

        assert!(tick.should_load_next_stage);
        assert_eq!(tick.state.mode, GameMode::Playing);
        assert_eq!(tick.state.transition_timer, 0.0);
    }

    #[test]
    fn non_transition_tick_is_noop() {
        let state = GameState::default();

        let tick = state.tick_transition(100.0);

        assert!(!tick.should_load_next_stage);
        assert_eq!(tick.state, state);
    }
}

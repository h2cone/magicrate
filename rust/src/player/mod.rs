mod collision;
mod movement;
mod push;
mod visual;

use godot::{
    classes::{CharacterBody2D, ICharacterBody2D, Input},
    prelude::*,
};

use crate::config::SCENE_CONTRACT;

const PUSH_COOLDOWN_FRAMES: i32 = 8;
const PUSH_RESIST_FRAMES: i32 = 6;
const WALK_STEP: f32 = 1.0;
const VERTICAL_STEP: f32 = 2.0;
const JUMP_COUNTER_START: i32 = 49;
const JUMP_ASCEND_THRESHOLD: i32 = 44;

// Template contract for a new player.tscn:
// keep the PlayerController root, add an AnimatedSprite2D child named "AnimatedSprite2D" or "Visual",
// and provide "idle", "move", and "jump" animations in its SpriteFrames.
const PLAYER_TEMPLATE_ANIM_IDLE: &str = "idle";
const PLAYER_TEMPLATE_ANIM_MOVE: &str = "move";
const PLAYER_TEMPLATE_ANIM_JUMP: &str = "jump";
const PLAYER_TEMPLATE_VISUAL_CANDIDATES: [&str; 2] = ["AnimatedSprite2D", "Visual"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerState {
    Idle,
    Move,
    Jump,
}

#[derive(GodotClass)]
#[class(base=CharacterBody2D)]
pub struct PlayerController {
    base: Base<CharacterBody2D>,
    input_enabled: bool,
    state: PlayerState,
    facing: i32,
    push_cooldown: i32,
    push_intent_dir: i32,
    push_intent_timer: i32,
    jump_counter: i32,
}

#[godot_api]
impl ICharacterBody2D for PlayerController {
    fn init(base: Base<CharacterBody2D>) -> Self {
        Self {
            base,
            input_enabled: true,
            state: PlayerState::Idle,
            facing: 1,
            push_cooldown: 0,
            push_intent_dir: 0,
            push_intent_timer: 0,
            jump_counter: 0,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(SCENE_CONTRACT.group_player);
        self.sync_visual_template();
    }

    fn physics_process(&mut self, _delta: f64) {
        if self.push_cooldown > 0 {
            self.push_cooldown -= 1;
        }

        let context = self.build_collision_context();
        self.apply_vertical_motion(&context);
        self.align_to_grid_when_grounded(&context);

        let input = Input::singleton();
        let mut axis = 0.0;
        let mut moved_horizontally = false;

        if self.input_enabled {
            axis = input.get_axis(SCENE_CONTRACT.actions.left, SCENE_CONTRACT.actions.right);

            if axis.abs() > 0.01 {
                self.facing = if axis > 0.0 { 1 } else { -1 };
            }

            if input.is_action_just_pressed(SCENE_CONTRACT.actions.jump)
                && self.jump_counter == 0
                && self.has_floor_support_at(self.base().get_position(), &context)
            {
                self.reset_push_intent();
                self.jump_counter = JUMP_COUNTER_START;
            }
        }

        if axis.abs() > 0.01 {
            moved_horizontally = if self.try_push_crates(axis, &context) {
                true
            } else {
                let dir = if axis > 0.0 { 1.0 } else { -1.0 };
                self.try_step(Vector2::new(dir * WALK_STEP, 0.0), &context)
            };
        } else {
            self.reset_push_intent();
        }

        self.align_to_adjacent_crate_row(&context);

        self.base_mut().set_velocity(Vector2::ZERO);
        let state_context = self.build_collision_context();
        self.update_state(moved_horizontally, &state_context);
        self.sync_visual_template();
    }
}

#[godot_api]
impl PlayerController {
    #[signal]
    pub(crate) fn crate_pushed();

    #[func]
    pub fn set_input_enabled(&mut self, enabled: bool) {
        self.input_enabled = enabled;
        if !enabled {
            self.base_mut().set_velocity(Vector2::ZERO);
            self.push_cooldown = 0;
            self.reset_push_intent();
            self.jump_counter = 0;
            self.state = PlayerState::Idle;
        }
        self.sync_visual_template();
    }

    #[func]
    pub fn is_input_enabled(&self) -> bool {
        self.input_enabled
    }

    #[func]
    pub fn get_facing(&self) -> i64 {
        self.facing as i64
    }

    #[func]
    pub fn set_facing(&mut self, facing: i64) {
        self.facing = if facing < 0 { -1 } else { 1 };
        self.sync_visual_template();
    }

    #[func]
    pub fn get_visual_state_name(&self) -> GString {
        GString::from(Self::animation_name_for_state(self.state))
    }

    #[func]
    pub fn refresh_visual_template(&mut self) {
        self.sync_visual_template();
    }

    #[func]
    pub fn is_jump_active(&self) -> bool {
        self.jump_counter > 0 || self.state == PlayerState::Jump
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_template_animation_names_match_states() {
        assert_eq!(
            PlayerController::animation_name_for_state(PlayerState::Idle),
            "idle"
        );
        assert_eq!(
            PlayerController::animation_name_for_state(PlayerState::Move),
            "move"
        );
        assert_eq!(
            PlayerController::animation_name_for_state(PlayerState::Jump),
            "jump"
        );
    }
}

use godot::prelude::*;

use super::{
    JUMP_ASCEND_THRESHOLD, PlayerController, PlayerState, VERTICAL_STEP,
    collision::CollisionContext,
};

impl PlayerController {
    pub(super) fn update_state(&mut self, moved_horizontally: bool, context: &CollisionContext) {
        if self.jump_counter > 0 || !self.has_floor_support_at(self.base().get_position(), context)
        {
            self.state = PlayerState::Jump;
            return;
        }

        self.state = if moved_horizontally {
            PlayerState::Move
        } else {
            PlayerState::Idle
        };
    }

    pub(super) fn apply_vertical_motion(&mut self, context: &CollisionContext) {
        if self.jump_counter > 0 {
            self.jump_counter -= 1;
            if self.jump_counter > JUMP_ASCEND_THRESHOLD {
                let _ = self.try_step(Vector2::new(0.0, -VERTICAL_STEP), context);
            }
        }

        if self.jump_counter > JUMP_ASCEND_THRESHOLD
            && self.has_ceiling_block_at(self.base().get_position(), context)
        {
            self.jump_counter = JUMP_ASCEND_THRESHOLD;
        }

        if !self.has_floor_support_at(self.base().get_position(), context)
            && self.jump_counter < JUMP_ASCEND_THRESHOLD
        {
            self.jump_counter = 1;
            let _ = self.try_step(Vector2::new(0.0, VERTICAL_STEP), context);
        }

        if self.has_floor_support_at(self.base().get_position(), context) {
            if self.jump_counter > 0 {
                self.jump_counter = 0;
            }

            let mut pos = self.base().get_position();
            pos.y = Self::snap_coord(pos.y);
            self.base_mut().set_position(pos);
        }
    }

    pub(super) fn align_to_grid_when_grounded(&mut self, context: &CollisionContext) {
        if self.jump_counter > 0 || !self.has_floor_support_at(self.base().get_position(), context)
        {
            return;
        }

        let current = self.base().get_position();
        let snapped_y = Self::snap_y(current.y);
        if (current.y - snapped_y).abs() <= 0.01 {
            return;
        }

        let aligned = Vector2::new(current.x, snapped_y);
        if !Self::is_collision_at(aligned, context) {
            self.base_mut().set_position(aligned);
        }
    }
}

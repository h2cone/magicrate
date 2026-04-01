use gameplay_core::{
    Vec2,
    player_logic::{self, PushIntentProgress},
};
use godot::{
    classes::{Node, RigidBody2D, TileMapLayer},
    prelude::*,
};

use crate::{
    config::SCENE_CONTRACT,
    core_bridge::{core_vec, godot_vec},
};

use super::{
    PUSH_COOLDOWN_FRAMES, PUSH_RESIST_FRAMES, PlayerController, collision::CollisionContext,
};

impl PlayerController {
    pub(super) fn reset_push_intent(&mut self) {
        self.push_intent_dir = 0;
        self.push_intent_timer = 0;
    }

    pub(super) fn align_to_adjacent_crate_row(&mut self, context: &CollisionContext) {
        if self.jump_counter > 0 || !self.has_floor_support_at(self.base().get_position(), context)
        {
            return;
        }

        let player_pos = self.base().get_position();
        let tree = self.base().get_tree();
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_crate);
        let crate_positions: Vec<Vec2> = crates
            .iter_shared()
            .filter_map(|node| {
                node.try_cast::<RigidBody2D>()
                    .ok()
                    .map(|body| core_vec(Self::crate_top_left(&body)))
            })
            .collect();

        let Some(target_y) = player_logic::find_adjacent_row_target_y(
            core_vec(player_pos),
            &crate_positions,
            SCENE_CONTRACT.cell_size,
            2.0,
            SCENE_CONTRACT.cell_size,
        ) else {
            return;
        };

        if (player_pos.y - target_y).abs() <= 0.01 {
            return;
        }

        let aligned = Vector2::new(player_pos.x, target_y);
        if !Self::is_collision_at(aligned, context) {
            self.base_mut().set_position(aligned);
        }
    }

    pub(super) fn try_push_crates(&mut self, axis: f32, context: &CollisionContext) -> bool {
        let Some(dir_sign) = player_logic::direction_from_axis(axis) else {
            self.reset_push_intent();
            return false;
        };

        if self.push_cooldown > 0 || !self.has_floor_support_at(self.base().get_position(), context)
        {
            self.reset_push_intent();
            return false;
        }

        let (next_dir, next_timer, progress) = player_logic::update_push_intent(
            self.push_intent_dir,
            self.push_intent_timer,
            dir_sign,
            PUSH_RESIST_FRAMES,
        );
        self.push_intent_dir = next_dir;
        self.push_intent_timer = next_timer;

        match progress {
            PushIntentProgress::DirectionChanged => return false,
            PushIntentProgress::Waiting => {
                if self.resolve_push_chain(dir_sign, context).is_none() {
                    self.reset_push_intent();
                }
                return false;
            }
            PushIntentProgress::Ready => {}
        }

        let Some((chain, push_y)) = self.resolve_push_chain(dir_sign, context) else {
            self.reset_push_intent();
            return false;
        };

        let dir = dir_sign as f32;
        for mut body in chain.into_iter().rev() {
            let top_left = Self::crate_top_left(&body);
            let next_top_left =
                Vector2::new(top_left.x + dir * SCENE_CONTRACT.cell_size, top_left.y);
            body.set_position(next_top_left);
            body.set_linear_velocity(Vector2::ZERO);
            body.set_angular_velocity(0.0);
            body.set_sleeping(true);
        }

        let mut position = self.base().get_position();
        position.x = Self::snap_coord(position.x + dir * SCENE_CONTRACT.cell_size);
        position.y = push_y;
        self.base_mut().set_position(position);

        self.push_cooldown = PUSH_COOLDOWN_FRAMES;
        self.reset_push_intent();
        self.signals().crate_pushed().emit();
        true
    }

    fn resolve_push_chain(
        &self,
        dir_sign: i32,
        context: &CollisionContext,
    ) -> Option<(Vec<Gd<RigidBody2D>>, f32)> {
        let tree = self.base().get_tree();
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_crate);
        if crates.is_empty() {
            return None;
        }

        let crate_cells: Vec<Vec2> = crates
            .iter_shared()
            .filter_map(|node| {
                node.try_cast::<RigidBody2D>()
                    .ok()
                    .map(|body| core_vec(Self::crate_top_left(&body)))
            })
            .collect();

        let plan = player_logic::resolve_push_chain_plan(
            core_vec(self.base().get_position()),
            dir_sign,
            &crate_cells,
            SCENE_CONTRACT.cell_size,
            |target_top_left| {
                Self::is_rule_blocking_for_crate(&context.rules_tilemap, target_top_left)
                    || Self::is_bridge_blocking_for_crate(&context.bridge_solids, target_top_left)
            },
        )?;

        let chain = Self::chain_cells_to_bodies(&crates, &plan.chain_cells)?;
        Some((chain, plan.push_y))
    }

    fn chain_cells_to_bodies(
        crates: &Array<Gd<Node>>,
        chain_cells: &[Vec2],
    ) -> Option<Vec<Gd<RigidBody2D>>> {
        let mut chain = Vec::with_capacity(chain_cells.len());
        for &cell in chain_cells {
            chain.push(Self::find_crate_at_cell(crates, cell)?);
        }
        Some(chain)
    }

    fn find_crate_at_cell(
        crates: &Array<Gd<Node>>,
        target_top_left: Vec2,
    ) -> Option<Gd<RigidBody2D>> {
        for node in crates.iter_shared() {
            let Ok(body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };

            let crate_top_left = core_vec(Self::crate_top_left(&body));
            if (crate_top_left.x - target_top_left.x).abs() <= 0.5
                && (crate_top_left.y - target_top_left.y).abs() <= 0.5
            {
                return Some(body);
            }
        }

        None
    }

    fn is_rule_blocking_for_crate(
        rules_tilemap: &Option<Gd<TileMapLayer>>,
        target_top_left: Vec2,
    ) -> bool {
        let Some(tilemap) = rules_tilemap else {
            return false;
        };

        let target_top_left = godot_vec(target_top_left);
        let rule_value = Self::rule_at_point(tilemap, target_top_left + Vector2::new(0.1, 0.1));
        SCENE_CONTRACT.crate_rule_is_solid(rule_value)
    }

    fn is_bridge_blocking_for_crate(bridge_solids: &[Rect2], target_top_left: Vec2) -> bool {
        Self::any_corner_hit(godot_vec(target_top_left), |point| {
            Self::is_point_inside_rects(point, bridge_solids)
        })
    }

    pub(super) fn crate_top_left(body: &Gd<RigidBody2D>) -> Vector2 {
        let pos = body.get_position();
        Vector2::new(Self::snap_coord(pos.x), Self::snap_y(pos.y))
    }

    pub(super) fn snap_coord(value: f32) -> f32 {
        player_logic::snap_coord(value, SCENE_CONTRACT.cell_size)
    }

    pub(super) fn snap_y(value: f32) -> f32 {
        player_logic::snap_y(value, SCENE_CONTRACT.cell_size)
    }
}

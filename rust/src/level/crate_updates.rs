use std::collections::HashSet;

use gameplay_core::{Vec2, crate_runtime};
use godot::{
    classes::{Node, RigidBody2D, TileMapLayer},
    prelude::*,
};

use crate::{
    config::SCENE_CONTRACT,
    core_bridge::{core_vec, godot_vec},
};

use super::{LevelRuntime, scene_ops};

impl LevelRuntime {
    pub(super) fn update_crate_runtime(&mut self) -> bool {
        let Some(stage) = self.current_stage.as_ref() else {
            return false;
        };

        let Some(tile_node) = stage.get_node_or_null(SCENE_CONTRACT.rules_layer_path) else {
            return false;
        };
        let Ok(tilemap) = tile_node.try_cast::<TileMapLayer>() else {
            return false;
        };

        let tree = self.base().get_tree();
        let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_crate);
        if crate_nodes.is_empty() {
            return false;
        }

        let mut bodies = Vec::new();
        let mut positions = Vec::new();

        for node in crate_nodes.iter_shared() {
            let Ok(mut body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };

            let pos = Self::stabilize_crate_body(&mut body);
            bodies.push(body);
            positions.push(core_vec(pos));
        }

        if bodies.is_empty() {
            return false;
        }

        let plan = crate_runtime::compute_plan(
            &positions,
            SCENE_CONTRACT.cell_size,
            SCENE_CONTRACT.box_fall_speed,
            |pos, occupancy| Self::crate_has_support(&tilemap, occupancy, pos),
        );

        for (mut body, next_pos) in bodies.into_iter().zip(plan.next_positions.into_iter()) {
            body.set_position(godot_vec(next_pos));
        }

        plan.moved
    }

    fn stabilize_crate_body(body: &mut Gd<RigidBody2D>) -> Vector2 {
        let mut pos = body.get_position();
        pos.x = crate_runtime::snap_grid(pos.x, SCENE_CONTRACT.cell_size);
        body.set_position(pos);
        body.set_linear_velocity(Vector2::ZERO);
        body.set_angular_velocity(0.0);
        body.set_gravity_scale(0.0);
        body.set_sleeping(true);
        body.set_freeze_enabled(true);
        pos
    }

    fn crate_has_support(
        tilemap: &Gd<TileMapLayer>,
        occupancy: &HashSet<(i32, i32)>,
        pos: Vec2,
    ) -> bool {
        let below_cell = (
            crate_runtime::snap_grid(pos.x, SCENE_CONTRACT.cell_size) as i32,
            crate_runtime::snap_grid(pos.y, SCENE_CONTRACT.cell_size) as i32
                + SCENE_CONTRACT.cell_size as i32,
        );
        if occupancy.contains(&below_cell) {
            return true;
        }

        let below_left = Vector2::new(pos.x, pos.y + SCENE_CONTRACT.cell_size);
        let below_right = Vector2::new(
            pos.x + SCENE_CONTRACT.cell_size - 1.0,
            pos.y + SCENE_CONTRACT.cell_size,
        );

        Self::is_solid_for_crate(tilemap, below_left)
            || Self::is_solid_for_crate(tilemap, below_right)
    }

    fn is_solid_for_crate(tilemap: &Gd<TileMapLayer>, world_point: Vector2) -> bool {
        let rule = scene_ops::rule_at_point(tilemap, world_point, SCENE_CONTRACT.cell_size);
        SCENE_CONTRACT.crate_rule_is_solid(rule)
    }
}

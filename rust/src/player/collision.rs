use godot::{
    classes::{
        CharacterBody2D, CollisionShape2D, Node, Node2D, RectangleShape2D, RigidBody2D,
        StaticBody2D, TileMapLayer,
    },
    prelude::*,
};

use crate::{config::SCENE_CONTRACT, level::scene_ops};

use super::PlayerController;

pub(super) struct CollisionContext {
    pub(super) rules_tilemap: Option<Gd<TileMapLayer>>,
    pub(super) crate_cells: Vec<Rect2>,
    pub(super) bridge_solids: Vec<Rect2>,
}

impl PlayerController {
    pub(super) fn build_collision_context(&self) -> CollisionContext {
        let rules_tilemap = Self::find_rules_tilemap(&self.base());
        let tree = self.base().get_tree();

        let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_crate);
        let mut crate_cells = Vec::new();
        for node in crate_nodes.iter_shared() {
            let Ok(body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };

            crate_cells.push(Rect2::new(
                Self::crate_top_left(&body),
                Vector2::new(SCENE_CONTRACT.cell_size, SCENE_CONTRACT.cell_size),
            ));
        }

        let bridge_nodes: Array<Gd<Node>> =
            tree.get_nodes_in_group(SCENE_CONTRACT.group_bridge_tile);
        let mut bridge_solids = Vec::new();
        for node in bridge_nodes.iter_shared() {
            let Ok(body) = node.try_cast::<StaticBody2D>() else {
                continue;
            };
            if body.get_collision_layer() & 1 == 0 {
                continue;
            }

            let Some(shape_node) =
                body.get_node_or_null(SCENE_CONTRACT.bridge_collision_shape_path)
            else {
                continue;
            };
            let Ok(shape_node) = shape_node.try_cast::<CollisionShape2D>() else {
                continue;
            };
            let Some(shape) = shape_node.get_shape() else {
                continue;
            };
            let Ok(rect_shape) = shape.try_cast::<RectangleShape2D>() else {
                continue;
            };
            let size = rect_shape.get_size();
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }

            let top_left = body.get_position() + shape_node.get_position() - (size * 0.5);
            bridge_solids.push(Rect2::new(top_left, size));
        }

        CollisionContext {
            rules_tilemap,
            crate_cells,
            bridge_solids,
        }
    }

    fn find_rules_tilemap(player: &CharacterBody2D) -> Option<Gd<TileMapLayer>> {
        let parent = player.get_parent()?;
        let stage = parent.try_cast::<Node2D>().ok()?;
        let tile_node = stage.get_node_or_null(SCENE_CONTRACT.rules_layer_path)?;
        tile_node.try_cast::<TileMapLayer>().ok()
    }

    pub(super) fn has_floor_support_at(
        &self,
        position: Vector2,
        context: &CollisionContext,
    ) -> bool {
        Self::is_solid_point_for_player(
            Vector2::new(position.x + 1.0, position.y + SCENE_CONTRACT.cell_size),
            context,
        ) || Self::is_solid_point_for_player(
            Vector2::new(
                position.x + SCENE_CONTRACT.cell_size - 2.0,
                position.y + SCENE_CONTRACT.cell_size,
            ),
            context,
        )
    }

    pub(super) fn has_ceiling_block_at(
        &self,
        position: Vector2,
        context: &CollisionContext,
    ) -> bool {
        Self::is_solid_point_for_player(Vector2::new(position.x + 1.0, position.y), context)
            || Self::is_solid_point_for_player(
                Vector2::new(position.x + SCENE_CONTRACT.cell_size - 2.0, position.y),
                context,
            )
    }

    pub(super) fn try_step(&mut self, motion: Vector2, context: &CollisionContext) -> bool {
        let target = self.base().get_position() + motion;
        if Self::is_collision_at(target, context) {
            return false;
        }

        self.base_mut().set_position(target);
        true
    }

    pub(super) fn any_corner_hit(top_left: Vector2, predicate: impl Fn(Vector2) -> bool) -> bool {
        let s = SCENE_CONTRACT.cell_size - 1.0;
        predicate(top_left)
            || predicate(top_left + Vector2::new(s, 0.0))
            || predicate(top_left + Vector2::new(0.0, s))
            || predicate(top_left + Vector2::new(s, s))
    }

    pub(super) fn is_collision_at(position: Vector2, context: &CollisionContext) -> bool {
        Self::any_corner_hit(position, |point| {
            Self::is_solid_point_for_player(point, context)
        })
    }

    fn is_solid_point_for_player(point: Vector2, context: &CollisionContext) -> bool {
        Self::is_rule_solid_for_player(&context.rules_tilemap, point)
            || Self::is_point_inside_rects(point, &context.crate_cells)
            || Self::is_point_inside_rects(point, &context.bridge_solids)
    }

    fn is_rule_solid_for_player(rules_tilemap: &Option<Gd<TileMapLayer>>, point: Vector2) -> bool {
        let Some(tilemap) = rules_tilemap else {
            return false;
        };

        SCENE_CONTRACT.player_rule_is_solid(Self::rule_at_point(tilemap, point))
    }

    pub(super) fn rule_at_point(tilemap: &Gd<TileMapLayer>, point: Vector2) -> i32 {
        scene_ops::rule_at_point(tilemap, point, SCENE_CONTRACT.cell_size)
    }

    pub(super) fn is_point_inside_rects(point: Vector2, rects: &[Rect2]) -> bool {
        for rect in rects {
            if point.x >= rect.position.x
                && point.x < rect.position.x + rect.size.x
                && point.y >= rect.position.y
                && point.y < rect.position.y + rect.size.y
            {
                return true;
            }
        }

        false
    }
}

use godot::{
    classes::{
        CharacterBody2D, CollisionShape2D, ICharacterBody2D, Input, Node, Node2D, RectangleShape2D,
        RigidBody2D, StaticBody2D, TileMapLayer,
    },
    prelude::*,
};

use crate::core::player_logic::{self, PushIntentProgress};

const PLAYER_CELL_SIZE: f32 = 8.0;
const PUSH_COOLDOWN_FRAMES: i32 = 8;
const PUSH_RESIST_FRAMES: i32 = 6;
const IG_RULES_LAYER_PATH: &str = "IG_Rules-values";
const WALK_STEP: f32 = 1.0;
const VERTICAL_STEP: f32 = 2.0;
const JUMP_COUNTER_START: i32 = 49;
const JUMP_ASCEND_THRESHOLD: i32 = 44;

struct CollisionContext {
    rules_tilemap: Option<Gd<TileMapLayer>>,
    crate_cells: Vec<Vector2>,
    bridge_solids: Vec<Rect2>,
}

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
        self.base_mut().add_to_group("player");
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
            axis = input.get_axis("act_left", "act_right");

            if axis.abs() > 0.01 {
                self.facing = if axis > 0.0 { 1 } else { -1 };
            }

            if input.is_action_just_pressed("act_jump")
                && self.jump_counter == 0
                && self.has_floor_support_at(self.base().get_position(), &context)
            {
                self.reset_push_intent();
                self.jump_counter = JUMP_COUNTER_START;
            }
        }

        if axis.abs() > 0.01 {
            if self.try_push_crates(axis, &context) {
                moved_horizontally = true;
            } else {
                let dir = if axis > 0.0 { 1.0 } else { -1.0 };
                moved_horizontally = self.try_step(Vector2::new(dir * WALK_STEP, 0.0), &context);
            }
        } else {
            self.reset_push_intent();
        }

        self.align_to_adjacent_crate_row(&context);

        self.base_mut().set_velocity(Vector2::ZERO);
        let state_context = self.build_collision_context();
        self.update_state(moved_horizontally, &state_context);
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
        }
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
    }

    #[func]
    pub fn is_jump_active(&self) -> bool {
        self.jump_counter > 0 || self.state == PlayerState::Jump
    }

    fn reset_push_intent(&mut self) {
        self.push_intent_dir = 0;
        self.push_intent_timer = 0;
    }

    fn update_state(&mut self, moved_horizontally: bool, context: &CollisionContext) {
        if self.jump_counter > 0 || !self.has_floor_support_at(self.base().get_position(), context)
        {
            self.state = PlayerState::Jump;
            return;
        }

        if moved_horizontally {
            self.state = PlayerState::Move;
        } else {
            self.state = PlayerState::Idle;
        }
    }

    fn apply_vertical_motion(&mut self, context: &CollisionContext) {
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

    fn align_to_grid_when_grounded(&mut self, context: &CollisionContext) {
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

    fn align_to_adjacent_crate_row(&mut self, context: &CollisionContext) {
        if self.jump_counter > 0 || !self.has_floor_support_at(self.base().get_position(), context)
        {
            return;
        }

        let player_pos = self.base().get_position();

        let tree = self.base().get_tree();
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        let crate_positions: Vec<Vector2> = crates
            .iter_shared()
            .filter_map(|node| {
                node.try_cast::<RigidBody2D>()
                    .ok()
                    .map(|body| Self::crate_top_left(&body))
            })
            .collect();

        let Some(target_y) = player_logic::find_adjacent_row_target_y(
            player_pos,
            &crate_positions,
            PLAYER_CELL_SIZE,
            2.0,
            PLAYER_CELL_SIZE,
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

    fn build_collision_context(&self) -> CollisionContext {
        let rules_tilemap = Self::find_rules_tilemap(&self.base());
        let tree = self.base().get_tree();

        let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        let mut crate_cells = Vec::new();
        for node in crate_nodes.iter_shared() {
            let Ok(body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };
            crate_cells.push(Self::crate_top_left(&body));
        }

        let bridge_nodes: Array<Gd<Node>> = tree.get_nodes_in_group("bridge_tile");
        let mut bridge_solids = Vec::new();
        for node in bridge_nodes.iter_shared() {
            let Ok(body) = node.try_cast::<StaticBody2D>() else {
                continue;
            };
            if body.get_collision_layer() & 1 == 0 {
                continue;
            }

            let Some(shape_node) = body.get_node_or_null("CollisionShape2D") else {
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
        let tile_node = stage.get_node_or_null(IG_RULES_LAYER_PATH)?;
        tile_node.try_cast::<TileMapLayer>().ok()
    }

    fn has_floor_support_at(&self, position: Vector2, context: &CollisionContext) -> bool {
        Self::is_solid_point_for_player(
            Vector2::new(position.x + 1.0, position.y + PLAYER_CELL_SIZE),
            context,
        ) || Self::is_solid_point_for_player(
            Vector2::new(
                position.x + PLAYER_CELL_SIZE - 2.0,
                position.y + PLAYER_CELL_SIZE,
            ),
            context,
        )
    }

    fn has_ceiling_block_at(&self, position: Vector2, context: &CollisionContext) -> bool {
        Self::is_solid_point_for_player(Vector2::new(position.x + 1.0, position.y), context)
            || Self::is_solid_point_for_player(
                Vector2::new(position.x + PLAYER_CELL_SIZE - 2.0, position.y),
                context,
            )
    }

    fn try_step(&mut self, motion: Vector2, context: &CollisionContext) -> bool {
        let target = self.base().get_position() + motion;
        if Self::is_collision_at(target, context) {
            return false;
        }

        self.base_mut().set_position(target);
        true
    }

    fn is_collision_at(position: Vector2, context: &CollisionContext) -> bool {
        Self::is_solid_point_for_player(Vector2::new(position.x, position.y), context)
            || Self::is_solid_point_for_player(
                Vector2::new(position.x + PLAYER_CELL_SIZE - 1.0, position.y),
                context,
            )
            || Self::is_solid_point_for_player(
                Vector2::new(position.x, position.y + PLAYER_CELL_SIZE - 1.0),
                context,
            )
            || Self::is_solid_point_for_player(
                Vector2::new(
                    position.x + PLAYER_CELL_SIZE - 1.0,
                    position.y + PLAYER_CELL_SIZE - 1.0,
                ),
                context,
            )
    }

    fn is_solid_point_for_player(point: Vector2, context: &CollisionContext) -> bool {
        Self::is_rule_solid_for_player(&context.rules_tilemap, point)
            || Self::is_point_inside_cells(point, &context.crate_cells)
            || Self::is_point_inside_rects(point, &context.bridge_solids)
    }

    fn is_rule_solid_for_player(rules_tilemap: &Option<Gd<TileMapLayer>>, point: Vector2) -> bool {
        let Some(tilemap) = rules_tilemap else {
            return false;
        };

        Self::rule_at_point(tilemap, point) == 1
    }

    fn rule_at_point(tilemap: &Gd<TileMapLayer>, point: Vector2) -> i32 {
        let local = point - tilemap.get_position();
        let cell = Vector2i::new(
            (local.x / PLAYER_CELL_SIZE).floor() as i32,
            (local.y / PLAYER_CELL_SIZE).floor() as i32,
        );
        let atlas_coords = tilemap.get_cell_atlas_coords(cell);
        if atlas_coords.x < 0 {
            return 0;
        }

        atlas_coords.x + 1
    }

    fn is_point_inside_cells(point: Vector2, cells: &[Vector2]) -> bool {
        for top_left in cells {
            if point.x >= top_left.x
                && point.x < top_left.x + PLAYER_CELL_SIZE
                && point.y >= top_left.y
                && point.y < top_left.y + PLAYER_CELL_SIZE
            {
                return true;
            }
        }

        false
    }

    fn is_point_inside_rects(point: Vector2, rects: &[Rect2]) -> bool {
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

    fn try_push_crates(&mut self, axis: f32, context: &CollisionContext) -> bool {
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
            PushIntentProgress::DirectionChanged => {
                return false;
            }
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
            let next_top_left = Vector2::new(top_left.x + dir * PLAYER_CELL_SIZE, top_left.y);
            body.set_position(Self::crate_center_from_top_left(next_top_left));
            body.set_linear_velocity(Vector2::ZERO);
            body.set_angular_velocity(0.0);
            body.set_sleeping(true);
        }

        let mut position = self.base().get_position();
        position.x = Self::snap_coord(position.x + dir * PLAYER_CELL_SIZE);
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
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        if crates.is_empty() {
            return None;
        }

        let crate_cells: Vec<Vector2> = crates
            .iter_shared()
            .filter_map(|node| {
                node.try_cast::<RigidBody2D>()
                    .ok()
                    .map(|body| Self::crate_top_left(&body))
            })
            .collect();

        let plan = player_logic::resolve_push_chain_plan(
            self.base().get_position(),
            dir_sign,
            &crate_cells,
            PLAYER_CELL_SIZE,
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
        chain_cells: &[Vector2],
    ) -> Option<Vec<Gd<RigidBody2D>>> {
        let mut chain = Vec::with_capacity(chain_cells.len());
        for &cell in chain_cells {
            chain.push(Self::find_crate_at_cell(crates, cell)?);
        }
        Some(chain)
    }

    fn find_crate_at_cell(
        crates: &Array<Gd<Node>>,
        target_top_left: Vector2,
    ) -> Option<Gd<RigidBody2D>> {
        for node in crates.iter_shared() {
            let Ok(body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };

            let crate_top_left = Self::crate_top_left(&body);
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
        target_top_left: Vector2,
    ) -> bool {
        let Some(tilemap) = rules_tilemap else {
            return false;
        };

        let rule_value = Self::rule_at_point(tilemap, target_top_left + Vector2::new(0.1, 0.1));
        rule_value == 1 || rule_value == 2
    }

    fn is_bridge_blocking_for_crate(bridge_solids: &[Rect2], target_top_left: Vector2) -> bool {
        Self::is_point_inside_rects(target_top_left, bridge_solids)
            || Self::is_point_inside_rects(
                target_top_left + Vector2::new(PLAYER_CELL_SIZE - 1.0, 0.0),
                bridge_solids,
            )
            || Self::is_point_inside_rects(
                target_top_left + Vector2::new(0.0, PLAYER_CELL_SIZE - 1.0),
                bridge_solids,
            )
            || Self::is_point_inside_rects(
                target_top_left + Vector2::new(PLAYER_CELL_SIZE - 1.0, PLAYER_CELL_SIZE - 1.0),
                bridge_solids,
            )
    }

    fn crate_top_left(body: &Gd<RigidBody2D>) -> Vector2 {
        let pos = body.get_position();
        Vector2::new(Self::snap_coord(pos.x), Self::snap_y(pos.y))
    }

    fn crate_center_from_top_left(top_left: Vector2) -> Vector2 {
        top_left
    }

    fn snap_coord(value: f32) -> f32 {
        player_logic::snap_coord(value, PLAYER_CELL_SIZE)
    }

    fn snap_y(value: f32) -> f32 {
        player_logic::snap_y(value, PLAYER_CELL_SIZE)
    }
}

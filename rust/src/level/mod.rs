mod catalog;
pub(crate) mod scene_ops;
mod snapshot_io;

use std::collections::HashSet;

use gameplay_core::{
    crate_runtime,
    snapshot::{SnapshotHistory, StageSnapshot},
};
use godot::{
    classes::{CharacterBody2D, INode2D, Node, Node2D, RigidBody2D, TileMapLayer},
    prelude::*,
};

use crate::{
    config::SCENE_CONTRACT,
    core_bridge::{core_vec, godot_vec},
    entity::{bridge_switch::BridgeSwitch, bridge_tile::BridgeTile, goal_petal::GoalPetal},
    player::PlayerController,
    rooms,
};

use self::{
    catalog::discover_stage_paths,
    scene_ops::{
        center_stage_in_viewport, cleanup_entity_placeholders, find_spawn_position,
        normalize_crate_spawn_positions, snap_to_grid, target_viewport_size,
    },
    snapshot_io::{apply_stage_snapshot, build_stage_snapshot},
};

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct LevelRuntime {
    base: Base<Node2D>,

    stage_paths: Vec<String>,

    #[export]
    debug_alignment: bool,

    stage_index: i32,
    pending_push_snapshot: bool,
    debug_alignment_tick: i32,
    debug_alignment_last_sig: i64,
    bridge_active: bool,
    stage_cleared_emitted: bool,
    player_died_emitted: bool,

    current_stage: Option<Gd<Node2D>>,
    player: Option<Gd<CharacterBody2D>>,
    undo_history: SnapshotHistory,
}

#[godot_api]
impl INode2D for LevelRuntime {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            stage_paths: Vec::new(),
            debug_alignment: false,
            stage_index: 0,
            pending_push_snapshot: false,
            debug_alignment_tick: 0,
            debug_alignment_last_sig: i64::MIN,
            bridge_active: false,
            stage_cleared_emitted: false,
            player_died_emitted: false,
            current_stage: None,
            player: None,
            undo_history: SnapshotHistory::default(),
        }
    }

    fn ready(&mut self) {
        self.stage_paths = discover_stage_paths(&SCENE_CONTRACT);
        if self.stage_paths.is_empty() {
            godot_warn!(
                "[LevelRuntime] no Room_*.scn/tscn found in {}.",
                SCENE_CONTRACT.stage_dir
            );
            return;
        }

        if !self.load_stage(0) {
            godot_error!("[LevelRuntime] failed to load initial stage index=0");
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        if self.current_stage.is_none() {
            return;
        }

        let crates_moved = self.update_crate_runtime();
        self.update_bridge_state();
        self.check_goal_state();
        self.check_player_death();
        self.debug_player_crate_alignment();

        if self.pending_push_snapshot && !crates_moved {
            self.capture_snapshot();
            self.pending_push_snapshot = false;
        }
    }
}

#[godot_api]
impl LevelRuntime {
    #[signal]
    pub(crate) fn stage_loaded(index: i64);

    #[signal]
    pub(crate) fn stage_cleared();

    #[signal]
    pub(crate) fn player_died();

    #[func]
    pub fn load_stage(&mut self, index: i64) -> bool {
        let total = self.stage_paths.len() as i64;
        if total == 0 || index < 0 || index >= total {
            godot_warn!("[LevelRuntime] invalid stage index: {}", index);
            return false;
        }

        self.unload_current_stage();

        let Some(stage_path) = self.stage_paths.get(index as usize).cloned() else {
            return false;
        };
        let Some(mut stage_node) = rooms::instantiate_scene(&stage_path) else {
            godot_error!("[LevelRuntime] failed to instantiate stage: {}", stage_path);
            return false;
        };

        stage_node.set_name(&format!("Stage{}", index + 1));
        self.base_mut().add_child(&stage_node);
        center_stage_in_viewport(&mut stage_node, target_viewport_size(&SCENE_CONTRACT));

        self.current_stage = Some(stage_node.clone());
        self.stage_index = index as i32;
        self.pending_push_snapshot = false;
        self.debug_alignment_tick = 0;
        self.debug_alignment_last_sig = i64::MIN;
        self.bridge_active = false;
        self.stage_cleared_emitted = false;
        self.player_died_emitted = false;

        if !self.spawn_player_for_stage(&mut stage_node) {
            godot_error!("[LevelRuntime] player spawn failed in stage {}", index + 1);
            return false;
        }
        cleanup_entity_placeholders(&mut stage_node, &SCENE_CONTRACT);
        normalize_crate_spawn_positions(&self.base(), &SCENE_CONTRACT);

        self.undo_history.clear();
        self.capture_snapshot();

        self.signals().stage_loaded().emit(index + 1);
        true
    }

    #[func]
    pub fn restart_current_stage(&mut self) -> bool {
        self.load_stage(self.stage_index as i64)
    }

    #[func]
    pub fn load_next_stage(&mut self) -> bool {
        let total = self.stage_paths.len();
        if total == 0 {
            return false;
        }

        let next = ((self.stage_index + 1) as usize) % total;
        self.load_stage(next as i64)
    }

    #[func]
    pub fn request_undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_history.pop_previous_snapshot() else {
            return false;
        };

        self.apply_snapshot(snapshot);
        true
    }

    #[func]
    pub fn get_stage_number(&self) -> i64 {
        (self.stage_index + 1) as i64
    }

    #[func]
    pub fn get_stage_count(&self) -> i64 {
        self.stage_paths.len() as i64
    }

    #[func]
    fn on_player_crate_pushed(&mut self) {
        self.pending_push_snapshot = true;
    }

    fn unload_current_stage(&mut self) {
        if let Some(mut player) = self.player.take() {
            if let Some(mut parent) = player.get_parent() {
                let player_node = player.clone().upcast::<Node>();
                parent.remove_child(&player_node);
            }
            player.queue_free();
        }

        if let Some(mut stage) = self.current_stage.take() {
            self.base_mut().remove_child(&stage);
            stage.queue_free();
        }
    }

    fn spawn_player_for_stage(&mut self, stage: &mut Gd<Node2D>) -> bool {
        let Some(scene) = rooms::load_scene(SCENE_CONTRACT.player_scene) else {
            godot_error!(
                "[LevelRuntime] missing player scene: {}",
                SCENE_CONTRACT.player_scene
            );
            return false;
        };

        let Some(instance) = scene.instantiate() else {
            return false;
        };

        let Ok(mut player) = instance.try_cast::<CharacterBody2D>() else {
            godot_error!("[LevelRuntime] player scene root must be CharacterBody2D");
            return false;
        };

        let Some(spawn_pos) = find_spawn_position(stage, &SCENE_CONTRACT) else {
            godot_error!(
                "[LevelRuntime] missing required Node2D `{}` in stage {}",
                SCENE_CONTRACT.player_spawn_path,
                stage.get_name()
            );
            return false;
        };
        player.set_position(snap_to_grid(spawn_pos, SCENE_CONTRACT.cell_size));
        stage.add_child(&player);

        if let Ok(player_script) = player.clone().try_cast::<PlayerController>() {
            let runtime = self.to_gd();
            player_script
                .signals()
                .crate_pushed()
                .connect_other(&runtime, Self::on_player_crate_pushed);
        }

        self.player = Some(player);
        true
    }

    fn update_bridge_state(&mut self) {
        let tree = self.base().get_tree();

        let switches: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_bridge_switch);
        let mut any_active = false;
        for node in switches.iter_shared() {
            if let Ok(switch) = node.try_cast::<BridgeSwitch>() {
                if switch.bind().is_active() {
                    any_active = true;
                    break;
                }
            }
        }

        if any_active == self.bridge_active {
            return;
        }

        self.bridge_active = any_active;

        let tiles: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_bridge_tile);
        for node in tiles.iter_shared() {
            if let Ok(mut tile) = node.try_cast::<BridgeTile>() {
                tile.bind_mut().set_active(any_active);
            }
        }
    }

    fn update_crate_runtime(&mut self) -> bool {
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

        let mut bodies: Vec<Gd<RigidBody2D>> = Vec::new();
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
        pos: gameplay_core::Vec2,
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

    fn check_goal_state(&mut self) {
        if self.stage_cleared_emitted {
            return;
        }

        let tree = self.base().get_tree();

        let goals: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_goal_petal);
        let mut found_goal = false;
        let mut all_active = true;

        for node in goals.iter_shared() {
            if let Ok(goal) = node.try_cast::<GoalPetal>() {
                found_goal = true;
                if !goal.bind().is_active() {
                    all_active = false;
                    break;
                }
            }
        }

        if found_goal && all_active {
            self.stage_cleared_emitted = true;
            self.set_player_input_enabled(false);
            self.signals().stage_cleared().emit();
        }
    }

    fn check_player_death(&mut self) {
        if self.player_died_emitted {
            return;
        }

        let Some(ref player) = self.player else {
            return;
        };

        let fell_out = player.get_position().y > SCENE_CONTRACT.player_fall_death_y;
        let touched_hazard = self.player_touches_hazard(player.get_global_position());
        if fell_out || touched_hazard {
            self.player_died_emitted = true;
            self.set_player_input_enabled(false);
            self.signals().player_died().emit();
        }
    }

    fn player_touches_hazard(&self, player_global_pos: Vector2) -> bool {
        let tree = self.base().get_tree();
        let markers: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_hazard_marker);

        for node in markers.iter_shared() {
            let Ok(marker) = node.try_cast::<Node2D>() else {
                continue;
            };

            let marker_pos = marker.get_global_position();
            let half = Vector2::new(
                SCENE_CONTRACT.cell_size * 0.5,
                SCENE_CONTRACT.cell_size * 0.5,
            );
            if (player_global_pos.x - marker_pos.x).abs() <= half.x
                && (player_global_pos.y - marker_pos.y).abs() <= half.y
            {
                return true;
            }
        }

        false
    }

    fn set_player_input_enabled(&mut self, enabled: bool) {
        let Some(ref mut player) = self.player else {
            return;
        };

        if let Ok(mut script) = player.clone().try_cast::<PlayerController>() {
            script.bind_mut().set_input_enabled(enabled);
        }
    }

    fn debug_player_crate_alignment(&mut self) {
        if !self.debug_alignment {
            return;
        }

        self.debug_alignment_tick += 1;
        if self.debug_alignment_tick % 30 != 0 {
            return;
        }

        let Some(player) = self.player.as_ref() else {
            return;
        };

        let player_local = player.get_position();
        let player_global = player.get_global_position();

        let tree = self.base().get_tree();
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_crate);

        let mut nearest: Option<Gd<RigidBody2D>> = None;
        let mut nearest_dx = f32::INFINITY;

        for node in crates.iter_shared() {
            let Ok(body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };
            let dx = (body.get_global_position().x - player_global.x).abs();
            if dx < nearest_dx {
                nearest_dx = dx;
                nearest = Some(body);
            }
        }

        let Some(body) = nearest else {
            return;
        };

        if nearest_dx > SCENE_CONTRACT.cell_size * 2.0 {
            return;
        }

        let crate_local = body.get_position();
        let crate_global = body.get_global_position();

        let player_parent_y = player
            .get_parent()
            .and_then(|node| node.try_cast::<Node2D>().ok())
            .map(|n| n.get_global_position().y)
            .unwrap_or(0.0);
        let crate_parent_y = body
            .get_parent()
            .and_then(|node| node.try_cast::<Node2D>().ok())
            .map(|n| n.get_global_position().y)
            .unwrap_or(0.0);

        let sig = ((player_global.y * 100.0).round() as i64)
            ^ (((crate_global.y * 100.0).round() as i64) << 1)
            ^ (((player_parent_y * 100.0).round() as i64) << 2)
            ^ (((crate_parent_y * 100.0).round() as i64) << 3);
        if sig == self.debug_alignment_last_sig {
            return;
        }
        self.debug_alignment_last_sig = sig;

        godot_print!(
            "[AlignDebug] p_local=({:.2},{:.2}) p_global=({:.2},{:.2}) p_parent_y={:.2} | c_local=({:.2},{:.2}) c_global=({:.2},{:.2}) c_parent_y={:.2} | dy_local={:.2} dy_global={:.2}",
            player_local.x,
            player_local.y,
            player_global.x,
            player_global.y,
            player_parent_y,
            crate_local.x,
            crate_local.y,
            crate_global.x,
            crate_global.y,
            crate_parent_y,
            player_local.y - crate_local.y,
            player_global.y - crate_global.y,
        );
    }

    fn capture_snapshot(&mut self) {
        let Some(ref player) = self.player else {
            return;
        };

        let Some(snapshot) = build_stage_snapshot(&self.base(), player, &SCENE_CONTRACT) else {
            return;
        };

        self.undo_history.push_snapshot(snapshot);
    }

    fn apply_snapshot(&mut self, snapshot: StageSnapshot) {
        self.stage_cleared_emitted = false;
        self.player_died_emitted = false;

        let mut player = self.player.take();
        let root = self.base();
        apply_stage_snapshot(&root, player.as_mut(), &snapshot, &SCENE_CONTRACT);
        self.player = player;
        self.set_player_input_enabled(true);
    }
}

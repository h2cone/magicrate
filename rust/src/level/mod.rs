use std::collections::{HashMap, HashSet};

use godot::{
    classes::{CharacterBody2D, INode2D, Node, Node2D, ProjectSettings, RigidBody2D, TileMapLayer},
    prelude::*,
};

use crate::{
    core::{crate_runtime, player_logic, stage_paths},
    entity::{bridge_switch::BridgeSwitch, bridge_tile::BridgeTile, goal_petal::GoalPetal},
    player::PlayerController,
    rooms::StageLoader,
    undo::{BodySnapshot, StageSnapshot, UndoService},
};

const DEFAULT_PLAYER_SCENE: &str = "res://player/player.tscn";
const DEFAULT_STAGE_DIR: &str = "res://pipeline/ldtk/levels";
const ENTITY_LAYER_NAME: &str = "Entities";
const PLAYER_SPAWN_PATH: &str = "Entities/PlayerSpawn";
const PLACEHOLDER_PLAYER_SPAWN: &str = "PlayerSpawn";
const PLACEHOLDER_PUSHABLE_CRATE: &str = "PushableCrate";
const PLACEHOLDER_GOAL_PETAL: &str = "GoalPetal";
const PLACEHOLDER_BRIDGE_SWITCH: &str = "BridgeSwitch";
const PLACEHOLDER_BRIDGE_TILE: &str = "BridgeTile";
const IG_RULES_LAYER_PATH: &str = "IG_Rules-values";
const GRID_SIZE: f32 = 8.0;
const BOX_FALL_SPEED: f32 = 2.0;

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
    undo_service: Option<Gd<UndoService>>,
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
            undo_service: None,
        }
    }

    fn ready(&mut self) {
        let undo_service = Gd::<UndoService>::from_init_fn(UndoService::init);
        self.base_mut().add_child(&undo_service);
        self.undo_service = Some(undo_service);

        self.stage_paths = Self::discover_stage_paths(DEFAULT_STAGE_DIR);
        if self.stage_paths.is_empty() {
            godot_warn!(
                "[LevelRuntime] no Room_*.scn/tscn found in {}.",
                DEFAULT_STAGE_DIR
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
        self.align_player_with_adjacent_crate_row();
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
        let Some(mut stage_node) = StageLoader::instantiate_scene(&stage_path) else {
            godot_error!("[LevelRuntime] failed to instantiate stage: {}", stage_path);
            return false;
        };

        stage_node.set_name(&format!("Stage{}", index + 1));
        self.base_mut().add_child(&stage_node);
        let viewport_size = Self::target_viewport_size();
        Self::center_stage_in_viewport(&mut stage_node, viewport_size);

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
        Self::cleanup_entity_placeholders(&mut stage_node);
        Self::normalize_crate_spawn_positions(&self.base());

        if let Some(ref mut undo_service) = self.undo_service {
            undo_service.bind_mut().clear();
        }
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
        let Some(ref mut undo_service) = self.undo_service else {
            return false;
        };

        let Some(snapshot) = undo_service.bind_mut().pop_previous_snapshot() else {
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
        let Some(scene) = StageLoader::load_scene(DEFAULT_PLAYER_SCENE) else {
            godot_error!(
                "[LevelRuntime] missing player scene: {}",
                DEFAULT_PLAYER_SCENE
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

        let Some(spawn_pos) = Self::find_spawn_position(stage) else {
            godot_error!(
                "[LevelRuntime] missing required Node2D `Entities/PlayerSpawn` in stage {}",
                stage.get_name()
            );
            return false;
        };
        player.set_position(Self::snap_to_grid(spawn_pos));
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

    fn find_spawn_position(stage: &Gd<Node2D>) -> Option<Vector2> {
        let _ = stage.get_node_or_null(ENTITY_LAYER_NAME)?;
        let spawn_node = stage.get_node_or_null(PLAYER_SPAWN_PATH)?;
        let spawn = spawn_node.try_cast::<Node2D>().ok()?;
        Some(spawn.get_position())
    }

    fn cleanup_entity_placeholders(stage: &mut Gd<Node2D>) {
        let Some(entities_node) = stage.get_node_or_null(ENTITY_LAYER_NAME) else {
            return;
        };
        let Ok(mut entities) = entities_node.try_cast::<Node>() else {
            return;
        };

        let children: Array<Gd<Node>> = entities.get_children();
        let mut to_remove: Vec<Gd<Node2D>> = Vec::new();
        for node in children.iter_shared() {
            let Ok(node2d) = node.try_cast::<Node2D>() else {
                continue;
            };

            let identifier = node2d
                .get("identifier")
                .try_to::<GString>()
                .ok()
                .map(|value| value.to_string())
                .unwrap_or_default();
            let should_hide = identifier == PLACEHOLDER_PLAYER_SPAWN
                || identifier == PLACEHOLDER_PUSHABLE_CRATE
                || identifier == PLACEHOLDER_GOAL_PETAL
                || identifier == PLACEHOLDER_BRIDGE_SWITCH
                || identifier == PLACEHOLDER_BRIDGE_TILE
                || node2d
                    .get_script()
                    .map(|script| script.get_path().to_string())
                    .is_some_and(|path| {
                        path.ends_with("addons/ldtk-importer/src/components/ldtk-entity.gd")
                    });
            if should_hide {
                to_remove.push(node2d);
            }
        }

        for mut node2d in to_remove {
            node2d.set_visible(false);
            let mut node = node2d.clone().upcast::<Node>();
            entities.remove_child(&node);
            node.queue_free();
        }
    }

    fn normalize_crate_spawn_positions(root: &Node2D) {
        let tree = root.get_tree();
        let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        for node in crate_nodes.iter_shared() {
            let Ok(mut body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };

            let pos = body.get_position();
            body.set_position(Self::snap_to_grid(pos));
            body.set_linear_velocity(Vector2::ZERO);
            body.set_angular_velocity(0.0);
            body.set_sleeping(true);
        }
    }

    fn update_bridge_state(&mut self) {
        let tree = self.base().get_tree();

        let switches: Array<Gd<Node>> = tree.get_nodes_in_group("bridge_switch");
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

        let tiles: Array<Gd<Node>> = tree.get_nodes_in_group("bridge_tile");
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

        let Some(tile_node) = stage.get_node_or_null(IG_RULES_LAYER_PATH) else {
            return false;
        };
        let Ok(tilemap) = tile_node.try_cast::<TileMapLayer>() else {
            return false;
        };

        let tree = self.base().get_tree();
        let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        if crate_nodes.is_empty() {
            return false;
        }

        let mut bodies: Vec<Gd<RigidBody2D>> = Vec::new();
        let mut positions: Vec<Vector2> = Vec::new();

        for node in crate_nodes.iter_shared() {
            let Ok(mut body) = node.try_cast::<RigidBody2D>() else {
                continue;
            };

            let pos = Self::stabilize_crate_body(&mut body);
            bodies.push(body);
            positions.push(pos);
        }

        if bodies.is_empty() {
            return false;
        }

        let plan =
            crate_runtime::compute_plan(&positions, GRID_SIZE, BOX_FALL_SPEED, |pos, occupancy| {
                Self::crate_has_support(&tilemap, occupancy, pos)
            });

        for (mut body, next_pos) in bodies.into_iter().zip(plan.next_positions.into_iter()) {
            body.set_position(next_pos);
        }

        plan.moved
    }

    fn stabilize_crate_body(body: &mut Gd<RigidBody2D>) -> Vector2 {
        let mut pos = body.get_position();
        pos.x = Self::snap_grid(pos.x);
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
        pos: Vector2,
    ) -> bool {
        let below_cell = (
            Self::snap_grid(pos.x) as i32,
            Self::snap_grid(pos.y) as i32 + GRID_SIZE as i32,
        );
        if occupancy.contains(&below_cell) {
            return true;
        }

        let below_left = Vector2::new(pos.x, pos.y + GRID_SIZE);
        let below_right = Vector2::new(pos.x + GRID_SIZE - 1.0, pos.y + GRID_SIZE);

        Self::is_solid_for_crate(tilemap, below_left)
            || Self::is_solid_for_crate(tilemap, below_right)
    }

    fn is_solid_for_crate(tilemap: &Gd<TileMapLayer>, world_point: Vector2) -> bool {
        let local = world_point - tilemap.get_position();
        let cell = tilemap.local_to_map(local + Vector2::new(0.1, 0.1));
        let atlas_coords = tilemap.get_cell_atlas_coords(cell);
        if atlas_coords.x < 0 {
            return false;
        }

        let rule = atlas_coords.x + 1;
        rule == 1 || rule == 2
    }

    fn snap_grid(value: f32) -> f32 {
        crate_runtime::snap_grid(value, GRID_SIZE)
    }

    fn check_goal_state(&mut self) {
        if self.stage_cleared_emitted {
            return;
        }

        let tree = self.base().get_tree();

        let goals: Array<Gd<Node>> = tree.get_nodes_in_group("goal_petal");
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

        let fell_out = player.get_position().y > 300.0;
        let touched_hazard = self.player_touches_hazard(player.get_global_position());
        if fell_out || touched_hazard {
            self.player_died_emitted = true;
            self.set_player_input_enabled(false);
            self.signals().player_died().emit();
        }
    }

    fn player_touches_hazard(&self, player_global_pos: Vector2) -> bool {
        let tree = self.base().get_tree();
        let markers: Array<Gd<Node>> = tree.get_nodes_in_group("ig_hazard_marker");

        for node in markers.iter_shared() {
            let Ok(marker) = node.try_cast::<Node2D>() else {
                continue;
            };

            let marker_pos = marker.get_global_position();
            let half = Vector2::new(4.0, 4.0);
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

    fn align_player_with_adjacent_crate_row(&mut self) {
        let Some(player) = self.player.as_ref() else {
            return;
        };

        if let Ok(script) = player.clone().try_cast::<PlayerController>() {
            if script.bind().is_jump_active() {
                return;
            }
        }

        let player_pos = player.get_position();

        let tree = self.base().get_tree();
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        let crate_positions: Vec<Vector2> = crates
            .iter_shared()
            .filter_map(|node| {
                node.try_cast::<RigidBody2D>()
                    .ok()
                    .map(|body| body.get_position())
            })
            .collect();

        let Some(target_y) = player_logic::find_adjacent_row_target_y(
            player_pos,
            &crate_positions,
            GRID_SIZE,
            2.0,
            2.0,
        ) else {
            return;
        };

        if (player_pos.y - target_y).abs() <= 0.01 {
            return;
        }

        if let Some(ref mut player) = self.player {
            let mut aligned = player_pos;
            aligned.y = target_y;
            player.set_position(aligned);
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
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group("crate");

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

        if nearest_dx > GRID_SIZE * 2.0 {
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
        let Some(snapshot) = self.build_snapshot() else {
            return;
        };

        if let Some(ref mut undo_service) = self.undo_service {
            undo_service.bind_mut().push_snapshot(snapshot);
        }
    }

    fn build_snapshot(&self) -> Option<StageSnapshot> {
        let player = self.player.as_ref()?;

        let facing = player
            .clone()
            .try_cast::<PlayerController>()
            .ok()
            .map(|controller| controller.bind().get_facing() as i32)
            .unwrap_or(1);

        let mut bodies: Vec<BodySnapshot> = Vec::new();

        let tree = self.base().get_tree();
        let crates: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        for node in crates.iter_shared() {
            if let Ok(body) = node.try_cast::<RigidBody2D>() {
                bodies.push(BodySnapshot {
                    name: body.get_name().to_string(),
                    position: body.get_position(),
                    linear_velocity: body.get_linear_velocity(),
                });
            }
        }

        bodies.sort_by(|a, b| a.name.cmp(&b.name));

        Some(StageSnapshot {
            player_position: player.get_position(),
            player_velocity: player.get_velocity(),
            player_facing: facing,
            bodies,
        })
    }

    fn apply_snapshot(&mut self, snapshot: StageSnapshot) {
        self.stage_cleared_emitted = false;
        self.player_died_emitted = false;

        if let Some(ref mut player) = self.player {
            player.set_position(snapshot.player_position);
            player.set_velocity(snapshot.player_velocity);

            if let Ok(mut script) = player.clone().try_cast::<PlayerController>() {
                script.bind_mut().set_facing(snapshot.player_facing as i64);
                script.bind_mut().set_input_enabled(true);
            }
        }

        let tree = self.base().get_tree();

        let mut existing: HashMap<String, Gd<RigidBody2D>> = HashMap::new();
        let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group("crate");
        for node in crate_nodes.iter_shared() {
            if let Ok(body) = node.try_cast::<RigidBody2D>() {
                existing.insert(body.get_name().to_string(), body);
            }
        }

        let mut snapshot_names = HashSet::new();

        for body_snapshot in &snapshot.bodies {
            snapshot_names.insert(body_snapshot.name.clone());

            if let Some(mut body) = existing.remove(&body_snapshot.name) {
                body.set_position(body_snapshot.position);
                body.set_linear_velocity(body_snapshot.linear_velocity);
            }
        }

        for (_, mut body) in existing {
            if !snapshot_names.contains(&body.get_name().to_string()) {
                body.set_linear_velocity(Vector2::ZERO);
            }
        }
    }

    fn snap_to_grid(pos: Vector2) -> Vector2 {
        Vector2::new(Self::snap_grid(pos.x), Self::snap_grid(pos.y))
    }

    fn discover_stage_paths(stage_dir: &str) -> Vec<String> {
        let global_dir = ProjectSettings::singleton()
            .globalize_path(stage_dir)
            .to_string();

        let Ok(entries) = std::fs::read_dir(&global_dir) else {
            return Vec::new();
        };

        let file_names = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok());
        let room_files = stage_paths::collect_sorted_room_files(file_names);

        room_files
            .into_iter()
            .map(|file_name| format!("{}/{}", stage_dir, file_name))
            .collect()
    }

    fn center_stage_in_viewport(stage: &mut Gd<Node2D>, viewport_size: Vector2) {
        let stage_size_var = stage.get("size");
        let stage_size = stage_size_var
            .try_to::<Vector2>()
            .ok()
            .or_else(|| {
                stage_size_var
                    .try_to::<Vector2i>()
                    .ok()
                    .map(|v| Vector2::new(v.x as f32, v.y as f32))
            })
            .unwrap_or(Vector2::ZERO);

        if stage_size.x <= 0.0 || stage_size.y <= 0.0 {
            stage.set_position(Vector2::ZERO);
            return;
        }

        let offset = Vector2::new(
            ((viewport_size.x - stage_size.x) * 0.5).floor(),
            ((viewport_size.y - stage_size.y) * 0.5).floor(),
        );
        stage.set_position(offset);
    }

    fn target_viewport_size() -> Vector2 {
        let settings = ProjectSettings::singleton();

        let width = settings
            .get("display/window/size/viewport_width")
            .to::<i64>() as f32;
        let height = settings
            .get("display/window/size/viewport_height")
            .to::<i64>() as f32;

        if width <= 0.0 || height <= 0.0 {
            return Vector2::new(136.0, 136.0);
        }

        Vector2::new(width, height)
    }
}

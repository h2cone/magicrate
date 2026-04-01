mod catalog;
mod crate_updates;
mod debug;
pub(crate) mod scene_ops;
mod snapshot_io;
mod stage_state;

use gameplay_core::snapshot::{SnapshotHistory, StageSnapshot};
use godot::{
    classes::{CharacterBody2D, INode2D, Node, Node2D},
    prelude::*,
};

use crate::{config::SCENE_CONTRACT, player::PlayerController, rooms};

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

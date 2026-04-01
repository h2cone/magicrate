use godot::{
    classes::{Node, Node2D},
    prelude::*,
};

use crate::{
    config::SCENE_CONTRACT,
    entity::{bridge_switch::BridgeSwitch, bridge_tile::BridgeTile, goal_petal::GoalPetal},
    player::PlayerController,
};

use super::LevelRuntime;

impl LevelRuntime {
    pub(super) fn update_bridge_state(&mut self) {
        let tree = self.base().get_tree();
        let switches: Array<Gd<Node>> = tree.get_nodes_in_group(SCENE_CONTRACT.group_bridge_switch);

        let mut any_active = false;
        for node in switches.iter_shared() {
            if let Ok(switch) = node.try_cast::<BridgeSwitch>()
                && switch.bind().is_active()
            {
                any_active = true;
                break;
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

    pub(super) fn check_goal_state(&mut self) {
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

    pub(super) fn check_player_death(&mut self) {
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

    pub(super) fn set_player_input_enabled(&mut self, enabled: bool) {
        let Some(ref mut player) = self.player else {
            return;
        };

        if let Ok(mut script) = player.clone().try_cast::<PlayerController>() {
            script.bind_mut().set_input_enabled(enabled);
        }
    }
}

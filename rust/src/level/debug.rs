use godot::{
    classes::{Node, Node2D, RigidBody2D},
    prelude::*,
};

use crate::config::SCENE_CONTRACT;

use super::LevelRuntime;

impl LevelRuntime {
    pub(super) fn debug_player_crate_alignment(&mut self) {
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
            .map(|node| node.get_global_position().y)
            .unwrap_or(0.0);
        let crate_parent_y = body
            .get_parent()
            .and_then(|node| node.try_cast::<Node2D>().ok())
            .map(|node| node.get_global_position().y)
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
}

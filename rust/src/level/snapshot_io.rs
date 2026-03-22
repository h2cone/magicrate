use std::collections::{HashMap, HashSet};

use gameplay_core::snapshot::{BodySnapshot, StageSnapshot};
use godot::{
    classes::{CharacterBody2D, Node, Node2D, RigidBody2D},
    prelude::*,
};

use crate::{
    config::SceneContract,
    core_bridge::{core_vec, godot_vec},
    player::PlayerController,
};

pub fn build_stage_snapshot(
    root: &Gd<Node2D>,
    player: &Gd<CharacterBody2D>,
    contract: &SceneContract,
) -> Option<StageSnapshot> {
    let facing = player
        .clone()
        .try_cast::<PlayerController>()
        .ok()
        .map(|controller| controller.bind().get_facing() as i32)
        .unwrap_or(1);

    let mut bodies = Vec::new();

    let tree = root.get_tree();
    let crates: Array<Gd<Node>> = tree.get_nodes_in_group(contract.group_crate);
    for node in crates.iter_shared() {
        if let Ok(body) = node.try_cast::<RigidBody2D>() {
            bodies.push(BodySnapshot {
                name: body.get_name().to_string(),
                position: core_vec(body.get_position()),
                linear_velocity: core_vec(body.get_linear_velocity()),
            });
        }
    }

    bodies.sort_by(|a, b| a.name.cmp(&b.name));

    Some(StageSnapshot {
        player_position: core_vec(player.get_position()),
        player_velocity: core_vec(player.get_velocity()),
        player_facing: facing,
        bodies,
    })
}

pub fn apply_stage_snapshot(
    root: &Gd<Node2D>,
    player: Option<&mut Gd<CharacterBody2D>>,
    snapshot: &StageSnapshot,
    contract: &SceneContract,
) {
    if let Some(player) = player {
        player.set_position(godot_vec(snapshot.player_position));
        player.set_velocity(godot_vec(snapshot.player_velocity));

        if let Ok(mut script) = player.clone().try_cast::<PlayerController>() {
            script.bind_mut().set_facing(snapshot.player_facing as i64);
        }
    }

    let tree = root.get_tree();

    let mut existing: HashMap<String, Gd<RigidBody2D>> = HashMap::new();
    let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group(contract.group_crate);
    for node in crate_nodes.iter_shared() {
        if let Ok(body) = node.try_cast::<RigidBody2D>() {
            existing.insert(body.get_name().to_string(), body);
        }
    }

    let mut snapshot_names = HashSet::new();

    for body_snapshot in &snapshot.bodies {
        snapshot_names.insert(body_snapshot.name.clone());

        if let Some(mut body) = existing.remove(&body_snapshot.name) {
            body.set_position(godot_vec(body_snapshot.position));
            body.set_linear_velocity(godot_vec(body_snapshot.linear_velocity));
        }
    }

    for (_, mut body) in existing {
        if !snapshot_names.contains(&body.get_name().to_string()) {
            body.set_linear_velocity(Vector2::ZERO);
        }
    }
}

use gameplay_core::crate_runtime;
use godot::{
    classes::{Node, Node2D, ProjectSettings, RigidBody2D, TileMapLayer},
    prelude::*,
};

use crate::config::SceneContract;

pub fn find_spawn_position(stage: &Gd<Node2D>, contract: &SceneContract) -> Option<Vector2> {
    let _ = stage.get_node_or_null(contract.entity_layer_name)?;
    let spawn_node = stage.get_node_or_null(contract.player_spawn_path)?;
    let spawn = spawn_node.try_cast::<Node2D>().ok()?;
    Some(spawn.get_position())
}

pub fn cleanup_entity_placeholders(stage: &mut Gd<Node2D>, contract: &SceneContract) {
    let Some(entities_node) = stage.get_node_or_null(contract.entity_layer_name) else {
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
        let should_remove = contract.is_placeholder_identifier(&identifier)
            || node2d
                .get_script()
                .map(|script| script.get_path().to_string())
                .is_some_and(|path| path.ends_with(contract.placeholder_script_suffix));
        if should_remove {
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

pub fn normalize_crate_spawn_positions(root: &Gd<Node2D>, contract: &SceneContract) {
    let tree = root.get_tree();
    let crate_nodes: Array<Gd<Node>> = tree.get_nodes_in_group(contract.group_crate);
    for node in crate_nodes.iter_shared() {
        let Ok(mut body) = node.try_cast::<RigidBody2D>() else {
            continue;
        };

        let pos = body.get_position();
        body.set_position(snap_to_grid(pos, contract.cell_size));
        body.set_linear_velocity(Vector2::ZERO);
        body.set_angular_velocity(0.0);
        body.set_sleeping(true);
    }
}

pub fn snap_to_grid(pos: Vector2, cell_size: f32) -> Vector2 {
    Vector2::new(
        crate_runtime::snap_grid(pos.x, cell_size),
        crate_runtime::snap_grid(pos.y, cell_size),
    )
}

pub fn rule_at_point(tilemap: &Gd<TileMapLayer>, point: Vector2, cell_size: f32) -> i32 {
    let local = point - tilemap.get_position();
    let cell = Vector2i::new(
        (local.x / cell_size).floor() as i32,
        (local.y / cell_size).floor() as i32,
    );
    let atlas_coords = tilemap.get_cell_atlas_coords(cell);
    if atlas_coords.x < 0 {
        return 0;
    }

    atlas_coords.x + 1
}

pub fn center_stage_in_viewport(stage: &mut Gd<Node2D>, viewport_size: Vector2) {
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

pub fn target_viewport_size(contract: &SceneContract) -> Vector2 {
    let settings = ProjectSettings::singleton();

    let width = settings
        .get("display/window/size/viewport_width")
        .to::<i64>() as f32;
    let height = settings
        .get("display/window/size/viewport_height")
        .to::<i64>() as f32;

    if width <= 0.0 || height <= 0.0 {
        return Vector2::new(contract.viewport_fallback.0, contract.viewport_fallback.1);
    }

    Vector2::new(width, height)
}

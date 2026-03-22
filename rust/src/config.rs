#[derive(Debug, Clone, Copy)]
pub struct InputActions {
    pub left: &'static str,
    pub right: &'static str,
    pub jump: &'static str,
    pub restart: &'static str,
    pub undo: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneContract {
    pub actions: InputActions,
    pub player_scene: &'static str,
    pub stage_dir: &'static str,
    pub stage_manifest: &'static str,
    pub entity_layer_name: &'static str,
    pub player_spawn_path: &'static str,
    pub rules_layer_path: &'static str,
    pub bridge_collision_shape_path: &'static str,
    pub placeholder_identifiers: &'static [&'static str],
    pub placeholder_script_suffix: &'static str,
    pub group_player: &'static str,
    pub group_crate: &'static str,
    pub group_bridge_switch: &'static str,
    pub group_bridge_tile: &'static str,
    pub group_goal_petal: &'static str,
    pub group_hazard_marker: &'static str,
    pub cell_size: f32,
    pub box_fall_speed: f32,
    pub player_fall_death_y: f32,
    pub rule_solid_all: i32,
    pub rule_solid_box_only: i32,
    pub viewport_fallback: (f32, f32),
}

impl SceneContract {
    pub fn is_placeholder_identifier(&self, identifier: &str) -> bool {
        self.placeholder_identifiers.contains(&identifier)
    }

    pub fn player_rule_is_solid(&self, rule: i32) -> bool {
        rule == self.rule_solid_all
    }

    pub fn crate_rule_is_solid(&self, rule: i32) -> bool {
        rule == self.rule_solid_all || rule == self.rule_solid_box_only
    }
}

const PLACEHOLDER_IDENTIFIERS: [&str; 5] = [
    "PlayerSpawn",
    "PushableCrate",
    "GoalPetal",
    "BridgeSwitch",
    "BridgeTile",
];

pub const SCENE_CONTRACT: SceneContract = SceneContract {
    actions: InputActions {
        left: "act_left",
        right: "act_right",
        jump: "act_jump",
        restart: "act_restart",
        undo: "act_undo",
    },
    player_scene: "res://player/player.tscn",
    stage_dir: "res://pipeline/ldtk/levels",
    stage_manifest: "res://pipeline/ldtk/stage_manifest.txt",
    entity_layer_name: "Entities",
    player_spawn_path: "Entities/PlayerSpawn",
    rules_layer_path: "IG_Rules-values",
    bridge_collision_shape_path: "CollisionShape2D",
    placeholder_identifiers: &PLACEHOLDER_IDENTIFIERS,
    placeholder_script_suffix: "addons/ldtk-importer/src/components/ldtk-entity.gd",
    group_player: "player",
    group_crate: "crate",
    group_bridge_switch: "bridge_switch",
    group_bridge_tile: "bridge_tile",
    group_goal_petal: "goal_petal",
    group_hazard_marker: "ig_hazard_marker",
    cell_size: 8.0,
    box_fall_speed: 2.0,
    player_fall_death_y: 300.0,
    rule_solid_all: 1,
    rule_solid_box_only: 2,
    viewport_fallback: (136.0, 136.0),
};

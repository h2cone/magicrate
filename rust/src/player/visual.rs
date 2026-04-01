use godot::{
    classes::{AnimatedSprite2D, Node},
    prelude::*,
};

use super::{
    PLAYER_TEMPLATE_ANIM_IDLE, PLAYER_TEMPLATE_ANIM_JUMP, PLAYER_TEMPLATE_ANIM_MOVE,
    PLAYER_TEMPLATE_VISUAL_CANDIDATES, PlayerController, PlayerState,
};

impl PlayerController {
    pub(super) fn animation_name_for_state(state: PlayerState) -> &'static str {
        match state {
            PlayerState::Idle => PLAYER_TEMPLATE_ANIM_IDLE,
            PlayerState::Move => PLAYER_TEMPLATE_ANIM_MOVE,
            PlayerState::Jump => PLAYER_TEMPLATE_ANIM_JUMP,
        }
    }

    pub(super) fn sync_visual_template(&self) {
        let Some(mut sprite) = self.find_template_sprite() else {
            return;
        };

        let should_flip = self.facing < 0;
        sprite.set("flip_h", &should_flip.to_variant());

        let animation = StringName::from(Self::animation_name_for_state(self.state));
        let current_animation = sprite
            .get("animation")
            .try_to::<StringName>()
            .ok()
            .unwrap_or_else(|| StringName::from(""));
        let is_playing = sprite
            .call("is_playing", &[])
            .try_to::<bool>()
            .ok()
            .unwrap_or(false);

        if current_animation != animation || !is_playing {
            sprite.call("play", &[animation.to_variant()]);
        }
    }

    fn find_template_sprite(&self) -> Option<Gd<AnimatedSprite2D>> {
        let base = self.base();

        for path in PLAYER_TEMPLATE_VISUAL_CANDIDATES {
            let Some(node) = base.get_node_or_null(path) else {
                continue;
            };

            if let Ok(sprite) = node.try_cast::<AnimatedSprite2D>() {
                return Some(sprite);
            }
        }

        let children: Array<Gd<Node>> = base.get_children();
        for node in children.iter_shared() {
            if let Ok(sprite) = node.try_cast::<AnimatedSprite2D>() {
                return Some(sprite);
            }
        }

        None
    }
}

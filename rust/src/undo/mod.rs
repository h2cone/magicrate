use godot::classes::{INode, Node};
use godot::prelude::*;

use crate::core::undo_history;

const DEFAULT_MAX_HISTORY: usize = 240;

#[derive(Clone)]
pub struct BodySnapshot {
    pub name: String,
    pub position: Vector2,
    pub linear_velocity: Vector2,
}

#[derive(Clone)]
pub struct StageSnapshot {
    pub player_position: Vector2,
    pub player_velocity: Vector2,
    pub player_facing: i32,
    pub bodies: Vec<BodySnapshot>,
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct UndoService {
    #[base]
    base: Base<Node>,
    history: Vec<StageSnapshot>,
}

#[godot_api]
impl INode for UndoService {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            history: Vec::new(),
        }
    }
}

#[godot_api]
impl UndoService {
    #[func]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    #[func]
    pub fn snapshot_count(&self) -> i64 {
        self.history.len() as i64
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn push_snapshot(&mut self, snapshot: StageSnapshot) {
        undo_history::push_dedup_with_cap(
            &mut self.history,
            snapshot,
            DEFAULT_MAX_HISTORY,
            snapshots_are_close,
        );
    }

    pub fn pop_previous_snapshot(&mut self) -> Option<StageSnapshot> {
        undo_history::pop_previous(&mut self.history)
    }
}

fn snapshots_are_close(left: &StageSnapshot, right: &StageSnapshot) -> bool {
    const EPS: f32 = 0.01;

    if left
        .player_position
        .distance_squared_to(right.player_position)
        > EPS
    {
        return false;
    }

    if left
        .player_velocity
        .distance_squared_to(right.player_velocity)
        > EPS
    {
        return false;
    }

    if left.player_facing != right.player_facing {
        return false;
    }

    if left.bodies.len() != right.bodies.len() {
        return false;
    }

    left.bodies.iter().zip(right.bodies.iter()).all(|(a, b)| {
        a.name == b.name
            && a.position.distance_squared_to(b.position) <= EPS
            && a.linear_velocity.distance_squared_to(b.linear_velocity) <= EPS
    })
}

#[cfg(test)]
mod tests {
    use godot::builtin::Vector2;

    use super::{BodySnapshot, StageSnapshot, snapshots_are_close};

    fn sample_snapshot() -> StageSnapshot {
        StageSnapshot {
            player_position: Vector2::new(8.0, 16.0),
            player_velocity: Vector2::new(0.0, 0.0),
            player_facing: 1,
            bodies: vec![BodySnapshot {
                name: "crate_a".to_string(),
                position: Vector2::new(24.0, 16.0),
                linear_velocity: Vector2::ZERO,
            }],
        }
    }

    #[test]
    fn snapshots_close_for_small_noise() {
        let left = sample_snapshot();
        let mut right = sample_snapshot();
        right.player_position.x += 0.05;

        assert!(snapshots_are_close(&left, &right));
    }

    #[test]
    fn snapshots_not_close_when_body_changes() {
        let left = sample_snapshot();
        let mut right = sample_snapshot();
        right.bodies[0].position.x += 1.0;

        assert!(!snapshots_are_close(&left, &right));
    }

    #[test]
    fn snapshots_not_close_when_facing_changes() {
        let left = sample_snapshot();
        let mut right = sample_snapshot();
        right.player_facing = -1;

        assert!(!snapshots_are_close(&left, &right));
    }
}

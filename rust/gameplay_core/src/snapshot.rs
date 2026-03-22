use crate::{
    Vec2,
    undo_history::{pop_previous, push_dedup_with_cap},
};

pub const DEFAULT_MAX_HISTORY: usize = 240;

#[derive(Clone, Debug, PartialEq)]
pub struct BodySnapshot {
    pub name: String,
    pub position: Vec2,
    pub linear_velocity: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageSnapshot {
    pub player_position: Vec2,
    pub player_velocity: Vec2,
    pub player_facing: i32,
    pub bodies: Vec<BodySnapshot>,
}

#[derive(Debug)]
pub struct SnapshotHistory {
    history: Vec<StageSnapshot>,
    max_len: usize,
}

impl SnapshotHistory {
    pub fn new(max_len: usize) -> Self {
        Self {
            history: Vec::new(),
            max_len: max_len.max(1),
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn snapshot_count(&self) -> usize {
        self.history.len()
    }

    pub fn push_snapshot(&mut self, snapshot: StageSnapshot) {
        let max_len = self.max_len.max(1);
        push_dedup_with_cap(&mut self.history, snapshot, max_len, snapshots_are_close);
    }

    pub fn pop_previous_snapshot(&mut self) -> Option<StageSnapshot> {
        pop_previous(&mut self.history)
    }
}

impl Default for SnapshotHistory {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HISTORY)
    }
}

pub fn snapshots_are_close(left: &StageSnapshot, right: &StageSnapshot) -> bool {
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
    use super::{BodySnapshot, SnapshotHistory, StageSnapshot, snapshots_are_close};
    use crate::Vec2;

    fn sample_snapshot() -> StageSnapshot {
        StageSnapshot {
            player_position: Vec2::new(8.0, 16.0),
            player_velocity: Vec2::ZERO,
            player_facing: 1,
            bodies: vec![BodySnapshot {
                name: "crate_a".to_string(),
                position: Vec2::new(24.0, 16.0),
                linear_velocity: Vec2::ZERO,
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

    #[test]
    fn history_deduplicates_and_rewinds() {
        let mut history = SnapshotHistory::default();

        history.push_snapshot(sample_snapshot());
        history.push_snapshot(sample_snapshot());

        let mut moved = sample_snapshot();
        moved.player_position.x += 8.0;
        history.push_snapshot(moved.clone());

        assert_eq!(history.snapshot_count(), 2);
        assert_eq!(history.pop_previous_snapshot(), Some(sample_snapshot()));
        assert_eq!(history.pop_previous_snapshot(), None);
    }
}

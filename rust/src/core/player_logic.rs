use std::collections::HashSet;

use godot::builtin::Vector2;

#[derive(Debug, Clone, PartialEq)]
pub struct PushChainPlan {
    pub chain_cells: Vec<Vector2>,
    pub push_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushIntentProgress {
    DirectionChanged,
    Waiting,
    Ready,
}

pub fn direction_from_axis(axis: f32) -> Option<i32> {
    if axis > 0.01 {
        Some(1)
    } else if axis < -0.01 {
        Some(-1)
    } else {
        None
    }
}

pub fn update_push_intent(
    current_dir: i32,
    current_timer: i32,
    dir_sign: i32,
    resist_frames: i32,
) -> (i32, i32, PushIntentProgress) {
    if current_dir != dir_sign {
        return (
            dir_sign,
            resist_frames.max(0),
            PushIntentProgress::DirectionChanged,
        );
    }

    if current_timer > 0 {
        return (dir_sign, current_timer - 1, PushIntentProgress::Waiting);
    }

    (dir_sign, 0, PushIntentProgress::Ready)
}

pub fn find_adjacent_row_target_y(
    player_pos: Vector2,
    crate_positions: &[Vector2],
    cell_size: f32,
    max_gap: f32,
    max_dy: f32,
) -> Option<f32> {
    let player_left = player_pos.x;
    let player_right = player_pos.x + cell_size;

    let mut target_y: Option<f32> = None;
    let mut best_score = f32::INFINITY;

    for &crate_pos in crate_positions {
        let crate_left = crate_pos.x;
        let crate_right = crate_pos.x + cell_size;

        let gap = if player_right < crate_left {
            crate_left - player_right
        } else if crate_right < player_left {
            player_left - crate_right
        } else {
            0.0
        };

        if gap > max_gap {
            continue;
        }

        let dy = (player_pos.y - crate_pos.y).abs();
        if dy > max_dy {
            continue;
        }

        let score = gap * 100.0 + dy;
        if score < best_score {
            best_score = score;
            target_y = Some(crate_pos.y);
        }
    }

    target_y
}

pub fn resolve_push_chain_plan<F>(
    player_pos: Vector2,
    dir_sign: i32,
    crate_cells: &[Vector2],
    cell_size: f32,
    mut is_blocked: F,
) -> Option<PushChainPlan>
where
    F: FnMut(Vector2) -> bool,
{
    if dir_sign != 1 && dir_sign != -1 {
        return None;
    }

    let push_y = snap_y(player_pos.y, cell_size);
    let dir = dir_sign as f32;

    let first_target_x = if dir_sign > 0 {
        ((player_pos.x + cell_size) / cell_size).floor() * cell_size
    } else {
        ((player_pos.x - 1.0) / cell_size).floor() * cell_size
    };

    let occupancy = crate_cells_set(crate_cells, cell_size);
    let mut target = Vector2::new(first_target_x, push_y);
    let mut chain_cells = Vec::new();

    while occupancy.contains(&cell_key(target, cell_size)) {
        chain_cells.push(target);
        target.x += dir * cell_size;
    }

    if chain_cells.is_empty() || is_blocked(target) {
        return None;
    }

    Some(PushChainPlan {
        chain_cells,
        push_y,
    })
}

pub fn snap_coord(value: f32, cell_size: f32) -> f32 {
    (value / cell_size).round() * cell_size
}

pub fn snap_y(value: f32, cell_size: f32) -> f32 {
    (value / cell_size).floor() * cell_size
}

fn crate_cells_set(crate_cells: &[Vector2], cell_size: f32) -> HashSet<(i32, i32)> {
    crate_cells
        .iter()
        .map(|cell| cell_key(*cell, cell_size))
        .collect()
}

fn cell_key(cell: Vector2, cell_size: f32) -> (i32, i32) {
    (
        snap_coord(cell.x, cell_size) as i32,
        snap_y(cell.y, cell_size) as i32,
    )
}

#[cfg(test)]
mod tests {
    use godot::builtin::Vector2;

    use super::{
        PushIntentProgress, direction_from_axis, find_adjacent_row_target_y,
        resolve_push_chain_plan, snap_coord, snap_y, update_push_intent,
    };

    #[test]
    fn direction_from_axis_uses_deadzone() {
        assert_eq!(direction_from_axis(0.0), None);
        assert_eq!(direction_from_axis(0.01), None);
        assert_eq!(direction_from_axis(0.02), Some(1));
        assert_eq!(direction_from_axis(-0.02), Some(-1));
    }

    #[test]
    fn update_push_intent_handles_direction_change_and_cooldown() {
        let (dir, timer, phase) = update_push_intent(0, 0, 1, 6);
        assert_eq!(
            (dir, timer, phase),
            (1, 6, PushIntentProgress::DirectionChanged)
        );

        let (dir, timer, phase) = update_push_intent(dir, timer, 1, 6);
        assert_eq!((dir, timer, phase), (1, 5, PushIntentProgress::Waiting));

        let (dir, timer, phase) = update_push_intent(dir, 0, 1, 6);
        assert_eq!((dir, timer, phase), (1, 0, PushIntentProgress::Ready));
    }

    #[test]
    fn find_adjacent_row_target_prefers_smallest_gap_then_vertical_distance() {
        let player = Vector2::new(8.0, 16.0);
        let crates = vec![
            Vector2::new(20.0, 16.0),
            Vector2::new(16.0, 17.0),
            Vector2::new(0.0, 16.0),
        ];

        let target = find_adjacent_row_target_y(player, &crates, 8.0, 2.0, 8.0);

        assert_eq!(target, Some(16.0));
    }

    #[test]
    fn find_adjacent_row_target_returns_none_for_distant_crates() {
        let player = Vector2::new(8.0, 16.0);
        let crates = vec![Vector2::new(40.0, 40.0)];

        let target = find_adjacent_row_target_y(player, &crates, 8.0, 2.0, 2.0);

        assert_eq!(target, None);
    }

    #[test]
    fn resolve_push_chain_plan_collects_contiguous_cells() {
        let player = Vector2::new(8.0, 16.0);
        let crates = vec![Vector2::new(16.0, 16.0), Vector2::new(24.0, 16.0)];

        let plan = resolve_push_chain_plan(player, 1, &crates, 8.0, |_target| false).unwrap();

        assert_eq!(plan.push_y, 16.0);
        assert_eq!(plan.chain_cells, crates);
    }

    #[test]
    fn resolve_push_chain_plan_returns_none_if_front_is_blocked() {
        let player = Vector2::new(8.0, 16.0);
        let crates = vec![Vector2::new(16.0, 16.0)];

        let plan = resolve_push_chain_plan(player, 1, &crates, 8.0, |target| {
            (target.x - 24.0).abs() <= 0.01 && (target.y - 16.0).abs() <= 0.01
        });

        assert!(plan.is_none());
    }

    #[test]
    fn resolve_push_chain_plan_returns_none_without_adjacent_crate() {
        let player = Vector2::new(8.0, 16.0);
        let crates = vec![Vector2::new(40.0, 16.0)];

        let plan = resolve_push_chain_plan(player, 1, &crates, 8.0, |_target| false);

        assert!(plan.is_none());
    }

    #[test]
    fn snap_helpers_match_grid_behavior() {
        assert_eq!(snap_coord(7.9, 8.0), 8.0);
        assert_eq!(snap_coord(3.9, 8.0), 0.0);
        assert_eq!(snap_y(7.9, 8.0), 0.0);
        assert_eq!(snap_y(8.0, 8.0), 8.0);
    }
}

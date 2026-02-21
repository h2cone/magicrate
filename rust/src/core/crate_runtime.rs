use std::collections::HashSet;

use godot::builtin::Vector2;

#[derive(Debug, Clone, PartialEq)]
pub struct CrateRuntimePlan {
    pub next_positions: Vec<Vector2>,
    pub moved: bool,
}

pub fn snap_grid(value: f32, grid_size: f32) -> f32 {
    (value / grid_size).round() * grid_size
}

pub fn crate_occupancy(positions: &[Vector2], grid_size: f32) -> HashSet<(i32, i32)> {
    positions
        .iter()
        .map(|pos| {
            (
                snap_grid(pos.x, grid_size) as i32,
                snap_grid(pos.y, grid_size) as i32,
            )
        })
        .collect()
}

pub fn compute_plan<F>(
    positions: &[Vector2],
    grid_size: f32,
    fall_speed: f32,
    mut has_support: F,
) -> CrateRuntimePlan
where
    F: FnMut(Vector2, &HashSet<(i32, i32)>) -> bool,
{
    let occupancy = crate_occupancy(positions, grid_size);
    let mut moved = false;
    let mut next_positions = Vec::with_capacity(positions.len());

    for &pos in positions {
        let mut next = pos;
        if !has_support(pos, &occupancy) {
            next.y += fall_speed;
            moved = true;
        } else {
            next.y = snap_grid(next.y, grid_size);
        }
        next_positions.push(next);
    }

    CrateRuntimePlan {
        next_positions,
        moved,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use godot::builtin::Vector2;

    use super::{compute_plan, crate_occupancy, snap_grid};

    #[test]
    fn snap_grid_rounds_to_nearest_cell() {
        assert_eq!(snap_grid(7.9, 8.0), 8.0);
        assert_eq!(snap_grid(4.0, 8.0), 8.0);
        assert_eq!(snap_grid(3.9, 8.0), 0.0);
    }

    #[test]
    fn occupancy_uses_snapped_positions() {
        let positions = vec![Vector2::new(0.2, 7.6), Vector2::new(8.1, 15.9)];

        let occupancy = crate_occupancy(&positions, 8.0);

        assert!(occupancy.contains(&(0, 8)));
        assert!(occupancy.contains(&(8, 16)));
        assert_eq!(occupancy.len(), 2);
    }

    #[test]
    fn compute_plan_falls_when_no_support() {
        let positions = vec![Vector2::new(0.0, 0.0)];

        let plan = compute_plan(&positions, 8.0, 2.0, |_pos, _occupancy| false);

        assert!(plan.moved);
        assert_eq!(plan.next_positions, vec![Vector2::new(0.0, 2.0)]);
    }

    #[test]
    fn compute_plan_snaps_when_supported() {
        let positions = vec![Vector2::new(1.2, 7.1)];

        let plan = compute_plan(&positions, 8.0, 2.0, |_pos, _occupancy| true);

        assert!(!plan.moved);
        assert_eq!(plan.next_positions, vec![Vector2::new(1.2, 8.0)]);
    }

    #[test]
    fn support_closure_receives_full_occupancy() {
        let positions = vec![Vector2::new(0.0, 0.0), Vector2::new(8.0, 0.0)];
        let mut seen = HashSet::new();

        let _plan = compute_plan(&positions, 8.0, 2.0, |_pos, occupancy| {
            seen = occupancy.clone();
            true
        });

        assert_eq!(seen.len(), 2);
        assert!(seen.contains(&(0, 0)));
        assert!(seen.contains(&(8, 0)));
    }
}

pub fn push_dedup_with_cap<T, F>(history: &mut Vec<T>, snapshot: T, max_len: usize, is_close: F)
where
    F: Fn(&T, &T) -> bool,
{
    if history
        .last()
        .map(|last| is_close(last, &snapshot))
        .unwrap_or(false)
    {
        return;
    }

    history.push(snapshot);
    let max = max_len.max(1);
    if history.len() > max {
        let overflow = history.len() - max;
        history.drain(0..overflow);
    }
}

pub fn pop_previous<T: Clone>(history: &mut Vec<T>) -> Option<T> {
    if history.len() < 2 {
        return None;
    }

    history.pop();
    history.last().cloned()
}

#[cfg(test)]
mod tests {
    use super::{pop_previous, push_dedup_with_cap};

    #[test]
    fn push_dedup_skips_close_snapshot() {
        let mut history: Vec<i32> = vec![10];

        push_dedup_with_cap(&mut history, 11, 10, |left, right| {
            (*left - *right).abs() <= 1
        });

        assert_eq!(history, vec![10]);
    }

    #[test]
    fn push_dedup_appends_distinct_snapshot() {
        let mut history: Vec<i32> = vec![10];

        push_dedup_with_cap(&mut history, 12, 10, |left, right| {
            (*left - *right).abs() <= 1
        });

        assert_eq!(history, vec![10, 12]);
    }

    #[test]
    fn push_dedup_applies_capacity() {
        let mut history = vec![1, 2, 3];

        push_dedup_with_cap(&mut history, 4, 2, |left, right| left == right);

        assert_eq!(history, vec![3, 4]);
    }

    #[test]
    fn pop_previous_requires_two_snapshots() {
        let mut history = vec![1];

        let popped = pop_previous(&mut history);

        assert_eq!(popped, None);
        assert_eq!(history, vec![1]);
    }

    #[test]
    fn pop_previous_discards_latest_and_returns_new_latest() {
        let mut history = vec![1, 2, 3];

        let popped = pop_previous(&mut history);

        assert_eq!(popped, Some(2));
        assert_eq!(history, vec![1, 2]);
    }
}

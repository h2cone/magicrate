#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationChange {
    Activated,
    Deactivated,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActivationCounter {
    active_count: u32,
}

impl ActivationCounter {
    pub fn is_active(&self) -> bool {
        self.active_count > 0
    }

    pub fn on_enter(&mut self, relevant: bool) -> ActivationChange {
        if !relevant {
            return ActivationChange::Unchanged;
        }

        self.active_count = self.active_count.saturating_add(1);
        if self.active_count == 1 {
            ActivationChange::Activated
        } else {
            ActivationChange::Unchanged
        }
    }

    pub fn on_exit(&mut self, relevant: bool) -> ActivationChange {
        if !relevant {
            return ActivationChange::Unchanged;
        }

        let was_active = self.is_active();
        self.active_count = self.active_count.saturating_sub(1);
        if was_active && !self.is_active() {
            ActivationChange::Deactivated
        } else {
            ActivationChange::Unchanged
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ActivationChange, ActivationCounter};

    #[test]
    fn first_enter_activates() {
        let mut counter = ActivationCounter::default();

        let change = counter.on_enter(true);

        assert_eq!(change, ActivationChange::Activated);
        assert!(counter.is_active());
    }

    #[test]
    fn multiple_enters_only_activate_once() {
        let mut counter = ActivationCounter::default();

        assert_eq!(counter.on_enter(true), ActivationChange::Activated);
        assert_eq!(counter.on_enter(true), ActivationChange::Unchanged);
        assert!(counter.is_active());
    }

    #[test]
    fn irrelevant_body_never_changes_state() {
        let mut counter = ActivationCounter::default();

        assert_eq!(counter.on_enter(false), ActivationChange::Unchanged);
        assert_eq!(counter.on_exit(false), ActivationChange::Unchanged);
        assert!(!counter.is_active());
    }

    #[test]
    fn leaving_last_body_deactivates() {
        let mut counter = ActivationCounter::default();
        counter.on_enter(true);

        let change = counter.on_exit(true);

        assert_eq!(change, ActivationChange::Deactivated);
        assert!(!counter.is_active());
    }

    #[test]
    fn extra_exit_is_safely_ignored() {
        let mut counter = ActivationCounter::default();

        let change = counter.on_exit(true);

        assert_eq!(change, ActivationChange::Unchanged);
        assert!(!counter.is_active());
    }
}

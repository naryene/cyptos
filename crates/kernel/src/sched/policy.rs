//! Pure task-selection policy shared by the scheduler and host tests.

pub(crate) const IDLE_TASK_INDEX: usize = 0;

/// Select the next ready task.
///
/// Non-idle tasks retain round-robin ordering and idle is considered only when
/// no normal task is ready, including during bootstrap.
pub(crate) fn select_next_ready<const N: usize>(
    current: Option<usize>,
    mut is_ready: impl FnMut(usize) -> bool,
) -> Option<usize> {
    if N == 0 {
        return None;
    }

    let start = current.map_or((IDLE_TASK_INDEX + 1) % N, |current| (current + 1) % N);

    for offset in 0..N {
        let idx = (start + offset) % N;
        if idx != IDLE_TASK_INDEX && is_ready(idx) {
            return Some(idx);
        }
    }

    is_ready(IDLE_TASK_INDEX).then_some(IDLE_TASK_INDEX)
}

#[cfg(test)]
mod tests {
    use super::{IDLE_TASK_INDEX, select_next_ready};

    #[test]
    fn bootstrap_dispatches_first_normal_task() {
        let current = None;
        let ready = [true, true, false, false];

        assert_eq!(current, None);
        assert_eq!(select_next_ready::<4>(current, |idx| ready[idx]), Some(1));
    }

    #[test]
    fn bootstrap_uses_idle_only_as_fallback() {
        let ready = [true, false, false, false];

        assert_eq!(
            select_next_ready::<4>(None, |idx| ready[idx]),
            Some(IDLE_TASK_INDEX)
        );
    }

    #[test]
    fn idle_is_skipped_during_normal_round_robin() {
        let ready = [true, true, false, true];

        assert_eq!(select_next_ready::<4>(Some(3), |idx| ready[idx]), Some(1));
    }

    #[test]
    fn idle_is_selected_only_as_fallback() {
        let ready = [true, false, false, false];

        assert_eq!(
            select_next_ready::<4>(Some(2), |idx| ready[idx]),
            Some(IDLE_TASK_INDEX)
        );
    }

    #[test]
    fn no_ready_task_returns_none() {
        assert_eq!(select_next_ready::<4>(Some(1), |_| false), None);
    }
}

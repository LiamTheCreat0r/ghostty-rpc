// src/process.rs

use sysinfo::{ProcessRefreshKind, System};

/// Name of the Ghostty process to look for.
///
/// Keeping the name in one place makes it easy to adjust for platform or
/// packaging differences. Exact matching is intentional: `ghostty-rpc` must
/// never be mistaken for Ghostty itself.
const GHOSTTY_PROCESS_NAME: &str = "ghostty";

/// Action the daemon should take after comparing the previous and current
/// Ghostty process state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceAction {
    /// Nothing changed; keep the current behavior.
    None,
    /// Ghostty just exited; clear the Discord activity once.
    Clear,
    /// Ghostty just appeared; resume normal presence updates.
    Resume,
}

/// Compares the previous and current Ghostty process state.
///
/// The daemon starts with `was_running = true` so that a stale presence from a
/// previous daemon instance is cleared on the first check if Ghostty is closed.
pub fn presence_action(running: bool, was_running: bool) -> PresenceAction {
    match (was_running, running) {
        (true, false) => PresenceAction::Clear,
        (false, true) => PresenceAction::Resume,
        _ => PresenceAction::None,
    }
}

/// Returns whether a process named exactly [`GHOSTTY_PROCESS_NAME`] is running.
pub fn is_ghostty_running() -> bool {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessRefreshKind::new());
    system
        .processes()
        .values()
        .any(|process| matches_ghostty_process_name(process.name()))
}

/// Exact process-name match so `ghostty-rpc` is never matched.
fn matches_ghostty_process_name(name: &str) -> bool {
    name == GHOSTTY_PROCESS_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_when_ghostty_exits() {
        assert_eq!(presence_action(false, true), PresenceAction::Clear);
    }

    #[test]
    fn resumes_when_ghostty_starts() {
        assert_eq!(presence_action(true, false), PresenceAction::Resume);
    }

    #[test]
    fn does_nothing_while_state_is_unchanged() {
        assert_eq!(presence_action(true, true), PresenceAction::None);
        assert_eq!(presence_action(false, false), PresenceAction::None);
    }

    #[test]
    fn matches_ghostty_but_not_ghostty_rpc() {
        assert!(matches_ghostty_process_name("ghostty"));
        assert!(!matches_ghostty_process_name("ghostty-rpc"));
    }
}

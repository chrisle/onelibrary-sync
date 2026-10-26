//! Whether rekordbox is running. It writes to the sticks it sees, without
//! the write lock RBXport and this app share, so a sync waits for it to quit.
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

fn is_rekordbox(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    // The app and its agent, not their Electron helpers.
    matches!(name, "rekordbox" | "rekordboxagent")
}

pub fn rekordbox_running() -> bool {
    let system = System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
    system.processes().values().any(|process| is_rekordbox(&process.name().to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rekordbox_and_its_agent_but_not_their_helpers() {
        for name in ["rekordbox Helper", "onelibrary-sync", "RBXport", "Finder"] {
            assert!(!is_rekordbox(name), "{name}");
        }
        assert!(is_rekordbox("rekordbox"));
        assert!(is_rekordbox("rekordboxAgent.exe"));
    }
}

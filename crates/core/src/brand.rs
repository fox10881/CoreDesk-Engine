//! Product identity.

use std::env::VarError;
use std::ffi::OsString;

pub const NAME: &str = "CoreDesk Engine";
/// Internal identifier (directories, pipes, mutexes) — the display name is `NAME`.
pub const ID: &str = "CoreDeskEngine";
/// Microsoft Store product id (Partner Center, packaging/msix/identity.json).
pub const STORE_PRODUCT_ID: &str = "9MV6WG3XNWSX";
pub const REPOSITORY_URL: &str = "https://github.com/fox10881/CoreDesk-Engine";

/// `COREDESK_*` environment overrides.
pub fn var_os(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

pub fn var(name: &str) -> Result<String, VarError> {
    var_os(name)
        .ok_or(VarError::NotPresent)?
        .into_string()
        .map_err(VarError::NotUnicode)
}

/// The single-instance lock this build holds. Named test instances remain
/// independent of the main app.
pub fn instance_mutex_name(instance: Option<&str>) -> String {
    format!(r"Local\{ID}.SingleInstance{}", instance_suffix(instance))
}

/// Named pipe the CLI (`coredesk-cli`) talks to: `CoreDeskEngine.<user SID>[.<instance>]`.
/// Pipe names are machine-wide, so the SID keeps two signed-in accounts apart, while the
/// same user in another session (ssh) still reaches their own instance.
pub fn ipc_pipe_name(user_sid: &str, instance: Option<&str>) -> String {
    format!(r"\\.\pipe\{ID}.{user_sid}{}", instance_suffix(instance))
}

/// [`log_file_name`] suffixes: this run's log and the rotated-out part before it (the
/// previous run's, or the previous 1 MiB when a run outgrows the file).
pub const LOG: &str = "";
pub const PREVIOUS_LOG: &str = ".prev";

/// Log file in `%LOCALAPPDATA%\CoreDeskEngine`: `coredesk[.name]<suffix>.log`, one set per
/// instance.
pub fn log_file_name(instance: Option<&str>, suffix: &str) -> String {
    format!("coredesk{}{suffix}.log", instance_suffix(instance))
}

/// `.name` for a named (test) instance, empty for the main one; whitespace-only = main.
fn instance_suffix(instance: Option<&str>) -> String {
    instance
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!(".{value}"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_use_the_normalized_suffix() {
        assert_eq!(
            instance_mutex_name(None),
            r"Local\CoreDeskEngine.SingleInstance"
        );
        assert_eq!(instance_mutex_name(None), instance_mutex_name(Some("  ")));
        assert_eq!(
            instance_mutex_name(Some(" test ")),
            r"Local\CoreDeskEngine.SingleInstance.test"
        );
        assert_ne!(instance_mutex_name(None), instance_mutex_name(Some("x")));
    }

    #[test]
    fn pipe_name_carries_the_user_sid_and_the_instance_suffix() {
        let sid = "S-1-5-21-1-2-3-1001";
        assert_eq!(
            ipc_pipe_name(sid, None),
            r"\\.\pipe\CoreDeskEngine.S-1-5-21-1-2-3-1001"
        );
        assert_eq!(ipc_pipe_name(sid, Some(" ")), ipc_pipe_name(sid, None));
        assert_eq!(
            ipc_pipe_name(sid, Some(" test ")),
            r"\\.\pipe\CoreDeskEngine.S-1-5-21-1-2-3-1001.test"
        );
        assert_ne!(
            ipc_pipe_name(sid, None),
            ipc_pipe_name("S-1-5-21-1-2-3-1002", None)
        );
    }

    #[test]
    fn log_names_follow_the_instance_suffix() {
        assert_eq!(log_file_name(None, LOG), "coredesk.log");
        assert_eq!(log_file_name(Some(" "), PREVIOUS_LOG), "coredesk.prev.log");
        assert_eq!(
            log_file_name(Some("test"), PREVIOUS_LOG),
            "coredesk.test.prev.log"
        );
    }
}

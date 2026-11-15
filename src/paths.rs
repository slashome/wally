//! Where wally keeps its files: the configuration under `~/.config/wally`, the
//! rotation state and the daemon lock under `~/.local/state/wally`, the daemon
//! log under `~/Library/Logs/wally`. `XDG_CONFIG_HOME` and `XDG_STATE_HOME`
//! are honoured.

use std::env;
use std::path::{Path, PathBuf};

/// The home directory, from `HOME`.
pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The directory named by `var`, or `fallback` under the home directory.
fn xdg(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(fallback))
}

/// The configuration directory, `~/.config/wally`.
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("wally")
}

/// The configuration file, `~/.config/wally/config.yaml`.
pub fn config_file() -> PathBuf {
    config_dir().join("config.yaml")
}

/// The state directory, `~/.local/state/wally`.
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("wally")
}

/// The rotation state: which images are left to show and which one each
/// screen has.
pub fn state_file() -> PathBuf {
    state_dir().join("state.json")
}

/// The lock the running daemon holds, with its pid inside.
pub fn lock_file() -> PathBuf {
    state_dir().join("daemon.lock")
}

/// The directory the daemon logs into.
pub fn log_dir() -> PathBuf {
    home().join("Library/Logs/wally")
}

/// `path` with a leading `~` replaced by the home directory.
pub fn expand(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some("") => home(),
        Some(rest) if rest.starts_with('/') => home().join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

/// `path` with the home directory written `~`, for display.
pub fn tilde(path: &Path) -> String {
    match path.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_tilde_is_the_home_directory() {
        assert_eq!(expand("~/Pictures"), home().join("Pictures"));
        assert_eq!(expand("~"), home());
        assert_eq!(expand("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(expand("~other/x"), PathBuf::from("~other/x"));
    }

    #[test]
    fn the_home_directory_is_displayed_as_a_tilde() {
        assert_eq!(tilde(&home().join("Pictures")), "~/Pictures");
        assert_eq!(tilde(Path::new("/tmp/x")), "/tmp/x");
    }
}

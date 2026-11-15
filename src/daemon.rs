//! The daemon: one instance at a time, changing the wallpapers at every
//! interval, dressing the screens plugged in meanwhile, and following the
//! configuration as it is edited.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::desktop;
use crate::engine::{self, Targets};
use crate::paths;
use crate::rotation::State;

/// How often the daemon wakes up to look at the clock, the screens and the
/// configuration.
const TICK: Duration = Duration::from_secs(5);

/// Prints `message` with the time, for the daemon log.
fn log(message: &str) {
    println!(
        "{} {message}",
        humantime::format_rfc3339_seconds(SystemTime::now())
    );
}

/// The pid of the running daemon, if one holds the lock.
pub fn running_pid() -> Option<u32> {
    let mut file = File::open(paths::lock_file()).ok()?;
    if file.try_lock().is_ok() {
        return None;
    }
    let mut pid = String::new();
    file.read_to_string(&mut pid).ok()?;
    pid.trim().parse().ok()
}

/// Takes the daemon lock and writes the pid in it, or fails when another
/// daemon holds it. The lock lasts as long as the returned file.
fn lock() -> Result<File> {
    let path = paths::lock_file();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    if file.try_lock().is_err() {
        bail!(
            "the daemon is already running (pid {})",
            running_pid().unwrap_or(0)
        );
    }
    file.set_len(0)?;
    file.rewind()?;
    write!(file, "{}", std::process::id())?;
    file.flush()?;
    Ok(file)
}

/// When `path` was last modified.
fn modified(path: &PathBuf) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// When the next scheduled change is due, given the last one; none while
/// paused.
pub fn next_change(state: &State, config: &Config) -> Option<SystemTime> {
    if state.paused {
        return None;
    }
    state.last_change.map(|t| t + config.interval())
}

/// Whether a scheduled change is due at `now`: never while paused, at once
/// when nothing changed yet.
pub fn due(state: &State, config: &Config, now: SystemTime) -> bool {
    !state.paused && next_change(state, config).is_none_or(|t| now >= t)
}

/// Runs the daemon until it is killed.
pub fn run() -> Result<()> {
    let _lock = lock()?;
    let config_path = paths::config_file();
    let mut config = Config::load_or_init()?;
    let mut stamp = modified(&config_path);
    log(&format!("started, pid {}", std::process::id()));
    let mut last_error = String::new();
    loop {
        let now_stamp = modified(&config_path);
        if now_stamp != stamp {
            stamp = now_stamp;
            match Config::read(&config_path) {
                Ok(fresh) => {
                    config = fresh;
                    log("configuration reloaded");
                }
                Err(e) => log(&format!(
                    "configuration kept, the new one is invalid: {e:#}"
                )),
            }
        }
        let state = State::load(&paths::state_file());
        let due = due(&state, &config, SystemTime::now());
        let targets = if due {
            Targets::All
        } else {
            Targets::Undressed
        };
        let changed = engine::change(&config, &targets).and_then(|mut changes| {
            if !due {
                changes.extend(engine::follow()?);
            }
            Ok(changes)
        });
        match changed {
            Ok(changes) => {
                last_error.clear();
                for c in changes {
                    log(&format!("{} ← {}", c.screen.name, c.image.display()));
                }
            }
            Err(e) => {
                let error = format!("{e:#}");
                if error != last_error {
                    log(&format!("no change: {error}"));
                    last_error = error;
                }
            }
        }
        desktop::pump(TICK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_due_once_the_interval_is_over() {
        let config = Config::default();
        let now = SystemTime::now();
        let mut state = State::default();
        assert!(due(&state, &config, now));
        state.last_change = Some(now);
        assert!(!due(&state, &config, now + Duration::from_secs(60)));
        assert!(due(&state, &config, now + config.interval()));
    }

    #[test]
    fn nothing_is_due_while_paused() {
        let config = Config::default();
        let now = SystemTime::now();
        let mut state = State::default();
        state.pause();
        assert!(!due(&state, &config, now + Duration::from_secs(86_400)));
        assert_eq!(next_change(&state, &config), None);
        state.resume(now);
        assert_eq!(next_change(&state, &config), Some(now + config.interval()));
        assert!(!due(&state, &config, now));
    }
}

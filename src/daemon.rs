//! The daemon: one instance at a time, changing the wallpapers at every
//! interval, dressing the screens plugged in meanwhile, following the
//! configuration and the marks as they are edited, and serving the commands
//! of the socket on the main thread.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::config::{Config, Key};
use crate::desktop;
use crate::engine::{self, Change, Targets};
use crate::history;
use crate::library;
use crate::marks::{Mark, Marks};
use crate::paths;
use crate::protocol::{self, Command, LibraryItem, Snapshot, Status, Target};
use crate::rotation::State;
use crate::server::{self, Job};

/// How often the daemon looks at the clock, the screens and the
/// configuration.
const TICK: Duration = Duration::from_secs(5);

/// How long the daemon lets AppKit run between two looks at the socket
/// requests.
const SLICE: Duration = Duration::from_millis(150);

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

/// The state of the running daemon between two wake-ups.
struct Daemon {
    /// The configuration in use.
    config: Config,
    /// When the configuration file was modified, as last seen.
    config_stamp: Option<SystemTime>,
    /// When the marks file was modified, as last seen.
    marks_stamp: Option<SystemTime>,
    /// The last error of the loop, cleared by the next success.
    error: Option<String>,
    /// Where the status events go, one per subscribed connection.
    subscribers: Vec<Sender<String>>,
    /// The status last pushed to the subscribers.
    published: Option<Status>,
}

impl Daemon {
    /// The daemon, with the configuration at the usual place.
    fn new() -> Result<Self> {
        Ok(Daemon {
            config: Config::load_or_init()?,
            config_stamp: modified(&paths::config_file()),
            marks_stamp: modified(&paths::marks_file()),
            error: None,
            subscribers: Vec::new(),
            published: None,
        })
    }

    /// Reads the configuration again if its file changed, and notes a change
    /// of the marks, read afresh wherever they are used.
    fn reload(&mut self) {
        let config_path = paths::config_file();
        let stamp = modified(&config_path);
        if stamp != self.config_stamp {
            self.config_stamp = stamp;
            match Config::read(&config_path) {
                Ok(fresh) => {
                    self.config = fresh;
                    log("configuration reloaded");
                }
                Err(e) => log(&format!(
                    "configuration kept, the new one is invalid: {e:#}"
                )),
            }
        }
        let stamp = modified(&paths::marks_file());
        if stamp != self.marks_stamp {
            self.marks_stamp = stamp;
            log("marks reloaded");
        }
    }

    /// Changes the images when due, dresses the screens plugged in meanwhile
    /// and makes the Spaces follow.
    fn tick(&mut self) {
        self.reload();
        let state = State::load(&paths::state_file());
        let due = due(&state, &self.config, SystemTime::now());
        let targets = if due {
            Targets::All
        } else {
            Targets::Undressed
        };
        let changed = engine::change(&self.config, &targets).and_then(|mut changes| {
            if !due {
                changes.extend(engine::follow()?);
            }
            Ok(changes)
        });
        match changed {
            Ok(changes) => {
                self.error = None;
                logged(&changes);
            }
            Err(e) => {
                let error = format!("{e:#}");
                if self.error.as_ref() != Some(&error) {
                    log(&format!("no change: {error}"));
                    self.error = Some(error);
                }
            }
        }
    }

    /// The status as it stands.
    fn status(&self) -> Result<Status> {
        let state = State::load(&paths::state_file());
        let marks = Marks::load(&paths::marks_file());
        let images = engine::library(&self.config, false).len();
        let screens = engine::screens()?;
        Ok(Status::new(&Snapshot {
            config: &self.config,
            state: &state,
            marks: &marks,
            images,
            screens: &screens,
            pid: std::process::id(),
            error: self.error.as_deref(),
        }))
    }

    /// The status as a protocol value.
    fn status_value(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.status()?)?)
    }

    /// Answers `job`, and subscribes its connection when it asks to.
    fn serve(&mut self, job: Job) {
        let outcome = self.answer(job.command).map_err(|e| format!("{e:#}"));
        if let (Ok(status), Some(events)) = (&outcome, job.events) {
            self.subscribers.push(events);
            if self.published.is_none() {
                self.published = serde_json::from_value(status.clone()).ok();
            }
        }
        let _ = job.reply.send(outcome);
    }

    /// Runs `command`.
    fn answer(&mut self, command: Command) -> Result<Value> {
        match command {
            Command::Hello => Ok(protocol::hello()),
            Command::Status | Command::Subscribe => self.status_value(),
            Command::Next { screen } => {
                let targets = match screen {
                    Some(id) => Targets::Only(vec![engine::screen(id)?.id]),
                    None => Targets::All,
                };
                logged(&engine::change(&self.config, &targets)?);
                self.status_value()
            }
            Command::Set { screen, image } => {
                let change = engine::set(screen, &paths::expand(&image))?;
                logged(std::slice::from_ref(&change));
                self.status_value()
            }
            Command::Pause => {
                engine::pause()?;
                log("paused");
                self.status_value()
            }
            Command::Resume => {
                engine::resume()?;
                log("resumed");
                self.status_value()
            }
            Command::Favorite(target) => self.mark(Mark::Favorite, target),
            Command::Ban(target) => self.mark(Mark::Ban, target),
            Command::History { limit } => Ok(serde_json::to_value(history::read(
                &paths::history_file(),
                limit,
            ))?),
            Command::Library { query } => {
                let images = engine::library(&self.config, false);
                let words: Vec<String> = query
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect();
                let marks = Marks::load(&paths::marks_file());
                let items: Vec<LibraryItem> = library::search(&images, &words, false)?
                    .iter()
                    .map(|p| LibraryItem::new(p, &marks))
                    .collect();
                Ok(serde_json::to_value(items)?)
            }
            Command::ConfigGet => Ok(serde_json::to_value(&self.config)?),
            Command::ConfigSet { key, value } => {
                let key: Key = key.parse()?;
                let mut config = self.config.clone();
                config.set(key, &value)?;
                self.configure(config)
            }
            Command::SourceAdd { folder } => {
                let mut config = self.config.clone();
                config.add_source(&folder)?;
                self.configure(config)
            }
            Command::SourceRemove { folder } => {
                let mut config = self.config.clone();
                config.remove_source(&folder)?;
                self.configure(config)
            }
        }
    }

    /// Marks or unmarks the image `target` names.
    fn mark(&mut self, mark: Mark, target: Target) -> Result<Value> {
        let image = target.image.as_deref().map(paths::expand);
        let image = engine::marked_image(image.as_deref(), target.screen)?;
        let changes = engine::mark(&self.config, mark, &image, target.on)?;
        self.marks_stamp = modified(&paths::marks_file());
        let verb = if target.on { "on" } else { "off" };
        log(&format!("{mark} {verb}: {}", image.display()));
        logged(&changes);
        self.status_value()
    }

    /// Writes `config` and uses it from now on.
    fn configure(&mut self, config: Config) -> Result<Value> {
        let path = paths::config_file();
        config.write(&path)?;
        self.config_stamp = modified(&path);
        self.config = config;
        log("configuration changed");
        Ok(serde_json::to_value(&self.config)?)
    }

    /// Pushes the status to the subscribers when it changed since the last
    /// push, and forgets the subscribers gone.
    fn publish(&mut self) {
        if self.subscribers.is_empty() {
            self.published = None;
            return;
        }
        let Ok(status) = self.status() else {
            return;
        };
        if self.published.as_ref() == Some(&status) {
            return;
        }
        let line = protocol::event(&status);
        self.subscribers.retain(|s| s.send(line.clone()).is_ok());
        self.published = Some(status);
    }
}

/// Logs every change of `changes`.
fn logged(changes: &[Change]) {
    for c in changes {
        log(&format!("{} ← {}", c.screen.name, c.image.display()));
    }
}

/// Runs the daemon until it is killed: serves the socket, and every
/// [`TICK`] changes the images when due and makes the Spaces follow.
pub fn run() -> Result<()> {
    let _lock = lock()?;
    let (jobs, requests) = mpsc::channel();
    let socket = paths::socket_file();
    server::listen(&socket, jobs)?;
    let mut daemon = Daemon::new()?;
    log(&format!(
        "started, pid {}, listening on {}",
        std::process::id(),
        socket.display()
    ));
    let mut next_tick = Instant::now();
    loop {
        let mut woke = false;
        if Instant::now() >= next_tick {
            daemon.tick();
            next_tick = Instant::now() + TICK;
            woke = true;
        }
        while let Ok(job) = requests.try_recv() {
            daemon.serve(job);
            woke = true;
        }
        if woke {
            daemon.publish();
        }
        desktop::pump(SLICE);
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

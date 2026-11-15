//! `wally init`: looks at the system first — screens, what they show, the
//! folders of images around, the configuration and the daemon already there —
//! then asks only the questions that matter on this machine.

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::config::{Config, Order, Spread, normalize_interval, parse_interval};
use crate::desktop::{self, Screen};
use crate::{daemon, launchd, library, paths};

/// Why a folder is offered as a source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// Already a source of the configuration.
    Configured,
    /// A folder macOS rotates on a screen right now.
    Rotating,
    /// The folder of the image a screen shows right now.
    OnScreen,
    /// A folder whose name says it holds wallpapers.
    NamedLikeWallpapers,
}

impl Reason {
    /// How the questionnaire explains the reason.
    fn describe(self) -> &'static str {
        match self {
            Reason::Configured => "in your configuration",
            Reason::Rotating => "rotated by macOS right now",
            Reason::OnScreen => "holds the image on screen",
            Reason::NamedLikeWallpapers => "found on this Mac",
        }
    }
}

/// A folder offered as a source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The folder.
    pub path: PathBuf,
    /// Images right in the folder.
    pub images: usize,
    /// Images in the folder and its subfolders.
    pub deep_images: usize,
    /// Why it is offered.
    pub reason: Reason,
}

/// What `wally init` found on the machine.
#[derive(Clone, Debug)]
pub struct System {
    /// The operating system and its version, such as `macOS 26.4`.
    pub os: String,
    /// The processor architecture, such as `aarch64`.
    pub arch: &'static str,
    /// The connected screens, left to right.
    pub screens: Vec<Screen>,
    /// The configuration already written, if any.
    pub config: Option<Config>,
    /// Whether the daemon is declared to launchd.
    pub daemon_installed: bool,
    /// The pid of the running daemon.
    pub daemon_pid: Option<u32>,
    /// The folders worth offering as sources.
    pub candidates: Vec<Candidate>,
}

/// The name and version of the operating system.
fn os_name() -> String {
    if cfg!(target_os = "macos") {
        let version = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        format!("macOS {version}").trim_end().to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

/// Folders under `root`, `depth` levels down at most, whose name contains
/// "wallpaper", case ignored.
fn named_like_wallpapers(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.starts_with('.') {
            continue;
        }
        if name.contains("wallpaper") {
            found.push(path.clone());
        }
        if depth > 1 {
            found.extend(named_like_wallpapers(&path, depth - 1));
        }
    }
    found
}

/// Folders worth offering, best first, each once: the configured sources,
/// what the screens show, then folders named like wallpapers in the usual
/// places under `home`. Empty folders are dropped, unless configured.
pub fn candidates(config: Option<&Config>, screens: &[Screen], home: &Path) -> Vec<Candidate> {
    let mut offered: Vec<(PathBuf, Reason)> = Vec::new();
    if let Some(config) = config {
        offered.extend(
            config
                .source_paths()
                .into_iter()
                .map(|p| (p, Reason::Configured)),
        );
    }
    for screen in screens {
        match screen.wallpaper.as_deref() {
            Some(p) if p.is_dir() => offered.push((p.to_path_buf(), Reason::Rotating)),
            Some(p) => {
                if let Some(dir) = p.parent().filter(|d| !d.starts_with("/System")) {
                    offered.push((dir.to_path_buf(), Reason::OnScreen));
                }
            }
            None => {}
        }
    }
    for (root, depth) in [
        (home.join("Pictures"), 2),
        (home.join("Library/Mobile Documents/com~apple~CloudDocs"), 2),
        (home.join("Documents"), 1),
    ] {
        offered.extend(
            named_like_wallpapers(&root, depth)
                .into_iter()
                .map(|p| (p, Reason::NamedLikeWallpapers)),
        );
    }
    let mut seen = BTreeSet::new();
    offered
        .into_iter()
        .filter(|(p, _)| seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone())))
        .map(|(path, reason)| {
            let images = library::scan(std::slice::from_ref(&path), false).len();
            let deep_images = library::scan(std::slice::from_ref(&path), true).len();
            Candidate {
                path,
                images,
                deep_images,
                reason,
            }
        })
        .filter(|c| c.reason == Reason::Configured || c.deep_images > 0)
        .collect()
}

/// Looks at the machine.
pub fn detect() -> System {
    let screens = desktop::screens().unwrap_or_default();
    let config_path = paths::config_file();
    let config = config_path
        .exists()
        .then(|| Config::read(&config_path).ok())
        .flatten();
    let candidates = candidates(config.as_ref(), &screens, &paths::home());
    System {
        os: os_name(),
        arch: std::env::consts::ARCH,
        screens,
        config,
        daemon_installed: launchd::installed(),
        daemon_pid: daemon::running_pid(),
        candidates,
    }
}

/// The questions and answers of a run, on any input and output.
pub struct Prompt<R, W> {
    input: R,
    output: W,
    assume_defaults: bool,
}

impl<R: BufRead, W: Write> Prompt<R, W> {
    /// A prompt reading `input` and writing `output`; with `assume_defaults`,
    /// every question takes its default without being asked.
    pub fn new(input: R, output: W, assume_defaults: bool) -> Self {
        Prompt {
            input,
            output,
            assume_defaults,
        }
    }

    /// Writes a line.
    pub fn say(&mut self, line: &str) -> Result<()> {
        writeln!(self.output, "{line}")?;
        Ok(())
    }

    /// Asks `question` until `parse` accepts the answer; an empty answer, or
    /// the end of the input, is `default`.
    pub fn ask<T>(
        &mut self,
        question: &str,
        default: &str,
        parse: impl Fn(&str) -> Result<T>,
    ) -> Result<T> {
        if self.assume_defaults {
            return parse(default);
        }
        loop {
            write!(self.output, "{question} [{default}]: ")?;
            self.output.flush()?;
            let mut line = String::new();
            let read = self.input.read_line(&mut line)?;
            let answer = match line.trim() {
                "" => default,
                answer => answer,
            };
            match parse(answer) {
                Ok(value) => return Ok(value),
                Err(_) if read == 0 => return parse(default),
                Err(e) => writeln!(self.output, "  {e:#}")?,
            }
        }
    }

    /// Asks a yes or no question.
    pub fn confirm(&mut self, question: &str, default: bool) -> Result<bool> {
        let shown = if default { "Y/n" } else { "y/N" };
        self.ask(question, shown, |a| match a.to_lowercase().as_str() {
            _ if a == shown => Ok(default),
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => bail!("answer y or n"),
        })
    }
}

/// What the command line already answers.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Source folders, instead of asking.
    pub sources: Vec<String>,
    /// Never install the daemon.
    pub no_daemon: bool,
}

/// What the questionnaire settled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answers {
    /// The new configuration.
    pub config: Config,
    /// Whether to install the daemon.
    pub install_daemon: bool,
}

/// Prints what was detected.
fn report<R: BufRead, W: Write>(prompt: &mut Prompt<R, W>, system: &System) -> Result<()> {
    let row = |key: &str, value: &str| format!("  {key:<10} {value}");
    prompt.say("Detected")?;
    prompt.say(&row("system", &format!("{}, {}", system.os, system.arch)))?;
    let names: Vec<&str> = system.screens.iter().map(|s| s.name.as_str()).collect();
    prompt.say(&row(
        "screens",
        &format!("{}: {}", names.len(), names.join(", ")),
    ))?;
    for screen in &system.screens {
        let shown = match screen.wallpaper.as_deref() {
            Some(p) if p.is_dir() => format!("{}/ (a folder macOS rotates)", paths::tilde(p)),
            Some(p) => paths::tilde(p),
            None => "unknown".into(),
        };
        prompt.say(&row("wallpaper", &shown))?;
    }
    let config = match &system.config {
        Some(_) => format!(
            "{} — its values are the defaults",
            paths::tilde(&paths::config_file())
        ),
        None => "none yet".into(),
    };
    prompt.say(&row("config", &config))?;
    let daemon = match (system.daemon_pid, system.daemon_installed) {
        (Some(pid), _) => format!("running (pid {pid})"),
        (None, true) => "installed, stopped".into(),
        (None, false) => "not installed".into(),
    };
    prompt.say(&row("daemon", &daemon))?;
    prompt.say("")
}

/// Parses a pick among `count` numbered folders: numbers separated by spaces
/// or commas, `0` for another folder.
fn parse_picks(answer: &str, count: usize) -> Result<Vec<usize>> {
    let picks = answer
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<usize>()
                .ok()
                .filter(|&n| n <= count)
                .with_context(|| format!("`{s}` is not one of 0 to {count}"))
        })
        .collect::<Result<Vec<_>>>()?;
    if picks.is_empty() {
        bail!("pick at least one folder");
    }
    Ok(picks)
}

/// Asks for the source folders.
fn ask_sources<R: BufRead, W: Write>(
    prompt: &mut Prompt<R, W>,
    candidates: &[Candidate],
) -> Result<Vec<PathBuf>> {
    prompt.say("Where are your wallpapers?")?;
    for (i, c) in candidates.iter().enumerate() {
        let deeper = c.deep_images - c.images;
        let deeper = if deeper > 0 {
            format!(" (+{deeper} in subfolders)")
        } else {
            String::new()
        };
        prompt.say(&format!(
            "  {}) {}  {} images{deeper}, {}",
            i + 1,
            paths::tilde(&c.path),
            c.images,
            c.reason.describe()
        ))?;
    }
    prompt.say("  0) another folder")?;
    let default = if candidates.is_empty() { "0" } else { "1" };
    let picks = prompt.ask("Folders, by number", default, |a| {
        parse_picks(a, candidates.len())
    })?;
    let mut sources: Vec<PathBuf> = Vec::new();
    for pick in picks {
        let path = if pick == 0 {
            prompt.ask("Folder", "~/Pictures/Wallpapers", |a| {
                let path = paths::expand(a);
                if path.is_dir() {
                    Ok(path)
                } else {
                    bail!("{a} is not a folder")
                }
            })?
        } else {
            candidates[pick - 1].path.clone()
        };
        if !sources.contains(&path) {
            sources.push(path);
        }
    }
    Ok(sources)
}

/// Asks the questions this system calls for, starting from the current
/// configuration or the default one.
pub fn questionnaire<R: BufRead, W: Write>(
    prompt: &mut Prompt<R, W>,
    system: &System,
    options: &Options,
) -> Result<Answers> {
    report(prompt, system)?;
    let base = system.config.clone().unwrap_or_default();
    let sources: Vec<PathBuf> = if options.sources.is_empty() {
        ask_sources(prompt, &system.candidates)?
    } else {
        options
            .sources
            .iter()
            .map(|s| {
                let path = paths::expand(s);
                if path.is_dir() {
                    Ok(path)
                } else {
                    bail!("{s} is not a folder")
                }
            })
            .collect::<Result<_>>()?
    };
    let flat = library::scan(&sources, false).len();
    let deep = library::scan(&sources, true).len();
    let recursive = if deep > flat {
        prompt.confirm(
            &format!("Include the {} images of their subfolders?", deep - flat),
            base.recursive,
        )?
    } else {
        base.recursive
    };
    let interval = prompt.ask(
        "Change every — 45s, 15m, 2h, 1h 30m",
        &base.interval,
        |a| parse_interval(a).map(|_| normalize_interval(a)),
    )?;
    let order: Order = prompt.ask(
        "Order: (r)andom or (s)equential",
        &base.order.to_string(),
        |a| a.parse(),
    )?;
    let screens: Spread = if system.screens.len() > 1 {
        prompt.ask(
            "Screens: (d)istinct images or the (s)ame on all",
            &base.screens.to_string(),
            |a| a.parse(),
        )?
    } else {
        base.screens
    };
    let install_daemon = !options.no_daemon
        && !system.daemon_installed
        && prompt.confirm("Change them on schedule, starting at login?", true)?;
    let config = Config {
        sources: sources.iter().map(|p| paths::tilde(p)).collect(),
        recursive,
        interval,
        order,
        screens,
    };
    config.check()?;
    Ok(Answers {
        config,
        install_daemon,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    fn system(candidates: Vec<Candidate>, screens: usize) -> System {
        System {
            os: "macOS 26.4".into(),
            arch: "aarch64",
            screens: (0..screens)
                .map(|i| Screen {
                    id: i as u32,
                    name: format!("Screen {i}"),
                    x: i as f64,
                    wallpaper: None,
                })
                .collect(),
            config: None,
            daemon_installed: false,
            daemon_pid: None,
            candidates,
        }
    }

    fn run(answers: &str, system: &System, options: &Options) -> (Result<Answers>, String) {
        let mut out = Vec::new();
        let result = questionnaire(
            &mut Prompt::new(Cursor::new(answers.to_string()), &mut out, false),
            system,
            options,
        );
        (result, String::from_utf8(out).unwrap())
    }

    #[test]
    fn folders_are_offered_once_with_their_image_counts() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        touch(&home.join("Pictures/My Wallpapers/a.jpg"));
        touch(&home.join("Pictures/My Wallpapers/old/b.jpg"));
        touch(&home.join("Pictures/empty-wallpapers/notes.txt"));
        let screens = vec![Screen {
            id: 1,
            name: "Built-in".into(),
            x: 0.0,
            wallpaper: Some(home.join("Pictures/My Wallpapers/a.jpg")),
        }];
        let found = candidates(None, &screens, home);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].reason, Reason::OnScreen);
        assert_eq!((found[0].images, found[0].deep_images), (1, 2));
    }

    #[test]
    fn a_single_screen_is_never_asked_about_screens() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.jpg"));
        let candidate = Candidate {
            path: dir.path().into(),
            images: 1,
            deep_images: 1,
            reason: Reason::NamedLikeWallpapers,
        };
        let (answers, out) = run(
            "1\n15\ns\nn\n",
            &system(vec![candidate.clone()], 1),
            &Options::default(),
        );
        let answers = answers.unwrap();
        assert!(!out.contains("Screens:"));
        assert!(!out.contains("subfolders?"));
        assert_eq!(answers.config.interval, "15m");
        assert_eq!(answers.config.order, Order::Sequential);
        assert!(!answers.install_daemon);
        let (_, out) = run(
            "1\n\n\nsame\n\n",
            &system(vec![candidate], 2),
            &Options::default(),
        );
        assert!(out.contains("Screens:"));
    }

    #[test]
    fn subfolders_are_asked_about_only_when_they_hold_images() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.jpg"));
        touch(&dir.path().join("sub/b.jpg"));
        let options = Options {
            sources: vec![dir.path().to_string_lossy().into()],
            no_daemon: true,
        };
        let (answers, out) = run("y\n\n\n", &system(Vec::new(), 1), &options);
        assert!(out.contains("Include the 1 images of their subfolders?"));
        assert!(answers.unwrap().config.recursive);
    }

    #[test]
    fn wrong_answers_are_asked_again_and_defaults_fill_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.jpg"));
        let candidate = Candidate {
            path: dir.path().into(),
            images: 1,
            deep_images: 1,
            reason: Reason::Configured,
        };
        let (answers, out) = run(
            "7\n1\n2s\n\nbackwards\n",
            &system(vec![candidate], 1),
            &Options::default(),
        );
        let answers = answers.unwrap();
        assert!(out.contains("`7` is not one of 0 to 1"));
        assert!(out.contains("too short"));
        assert_eq!(answers.config.interval, "30m");
        assert_eq!(answers.config.order, Order::Random);
        assert!(answers.install_daemon);
    }

    #[test]
    fn yes_or_no_falls_back_on_its_default() {
        let mut out = Vec::new();
        let mut prompt = Prompt::new(Cursor::new("\nmaybe\nyes\n".to_string()), &mut out, false);
        assert!(!prompt.confirm("Sure?", false).unwrap());
        assert!(prompt.confirm("Sure?", false).unwrap());
        let mut quiet = Prompt::new(Cursor::new(String::new()), Vec::new(), true);
        assert!(quiet.confirm("Sure?", true).unwrap());
    }

    #[test]
    fn a_source_given_on_the_command_line_has_to_exist() {
        let options = Options {
            sources: vec!["/nowhere/at/all".into()],
            no_daemon: true,
        };
        assert!(run("", &system(Vec::new(), 1), &options).0.is_err());
    }

    #[test]
    fn picks_are_numbers_within_the_list() {
        assert_eq!(parse_picks("1 3,2", 3).unwrap(), vec![1, 3, 2]);
        assert!(parse_picks("4", 3).is_err());
        assert!(parse_picks("", 3).is_err());
    }
}

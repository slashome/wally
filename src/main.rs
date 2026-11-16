//! The `wally` command line.

use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as Process, ExitCode};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::json;

use wally::client::{self, Route};
use wally::config::{Config, Key};
use wally::desktop::{self, Screen};
use wally::engine::{self, Targets};
use wally::marks::{Mark, Marks};
use wally::protocol::{self, Status, Target};
use wally::rotation::State;
use wally::{banner, daemon, launchd, library, paths};

/// What the bare `wally help` lists after the commands.
const AFTER_HELP: &str = "\
Files
  ~/.config/wally/config.yaml         the configuration (wally settings)
  ~/.config/wally/marks.yaml          favorite and banned images
  ~/.local/state/wally/state.json     what each screen shows, what comes next
  ~/.local/state/wally/history.jsonl  the images shown, newest last
  ~/Library/Logs/wally/wally.log      the daemon log

Environment
  WALLY_NO_BANNER   hide the banner
  NO_COLOR          no colour

Examples
  wally init                     set wally up, questions adapted to this Mac
  wally settings source add ~/Pictures/Wallpapers
  wally settings set interval 15m
  wally daemon install           start at login, change every interval
  wally show                     which image is on which screen
  wally next                     change now
  wally search forest            images whose name contains \"forest\"";

/// Wally rotates your wallpapers, screen by screen.
#[derive(Parser)]
#[command(name = "wally", version, after_help = AFTER_HELP)]
struct Cli {
    /// Hide the banner.
    #[arg(long, global = true)]
    no_banner: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Set wally up: looks at this Mac, then asks what it cannot guess.
    ///
    /// Detects the screens, what they show (a folder macOS rotates, or an
    /// image and its folder), the folders of wallpapers around, and the
    /// configuration and daemon already there. Then it asks for the source
    /// folders, the interval, the order — and, only where it matters, the
    /// subfolders, one image per screen, and starting at login. An existing
    /// configuration gives the default answers.
    Init {
        /// Keep every default answer, ask nothing.
        #[arg(long, short)]
        yes: bool,
        /// Source folder, instead of asking (repeatable).
        #[arg(long = "source", value_name = "FOLDER")]
        sources: Vec<String>,
        /// Do not install the daemon.
        #[arg(long)]
        no_daemon: bool,
    },
    /// Whether the daemon runs, where images come from, when they change next.
    Status {
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Which image is on which screen, left to right.
    ///
    /// Read from macOS itself, so it is exact even when the image was not put
    /// there by wally.
    #[command(visible_alias = "current")]
    Show {
        /// Full paths instead of file names.
        #[arg(long, conflicts_with = "json")]
        paths: bool,
        /// JSON output (screen, id, name, path).
        #[arg(long)]
        json: bool,
    },
    /// Change the wallpapers now, and restart the interval.
    #[command(visible_alias = "change")]
    Next {
        /// Only this screen: its number in `wally show`, or part of its name.
        #[arg(long, short)]
        screen: Option<String>,
        /// Say nothing.
        #[arg(long, short)]
        quiet: bool,
    },
    /// Put an image on a screen now, and restart the interval.
    Set {
        /// The image.
        image: PathBuf,
        /// The screen: its number in `wally show`, or part of its name; the
        /// first one by default.
        #[arg(long, short)]
        screen: Option<String>,
    },
    /// Stop changing the wallpapers on schedule, until `wally resume`.
    ///
    /// The Spaces still follow, and `wally next` still changes.
    Pause,
    /// Change the wallpapers on schedule again, the interval counted from now.
    Resume,
    /// The images shown lately, newest first.
    History {
        /// How many entries.
        #[arg(long, short, default_value_t = 20)]
        limit: usize,
        /// Full paths instead of file names.
        #[arg(long, conflicts_with = "json")]
        paths: bool,
        /// JSON output (at, screen_id, screen_name, image).
        #[arg(long)]
        json: bool,
    },
    /// Mark an image as a favorite: it comes three times as often.
    ///
    /// The image on the first screen by default, or on the one given. In
    /// sequential order a favorite still comes once per lap.
    Favorite {
        /// The image, instead of the one on the screen.
        image: Option<PathBuf>,
        /// The screen whose image to mark: its number in `wally show`, or
        /// part of its name.
        #[arg(long, short)]
        screen: Option<String>,
        /// Remove the mark instead.
        #[arg(long)]
        off: bool,
    },
    /// Ban an image: it never comes again, and leaves the screens showing it.
    ///
    /// The image on the first screen by default, or on the one given.
    Ban {
        /// The image, instead of the one on the screen.
        image: Option<PathBuf>,
        /// The screen whose image to ban: its number in `wally show`, or part
        /// of its name.
        #[arg(long, short)]
        screen: Option<String>,
        /// Lift the ban instead.
        #[arg(long)]
        off: bool,
    },
    /// Find images of the sources by file name.
    ///
    /// Case is ignored. Several words add up: a name has to contain them all,
    /// in any order. With no word, the whole library is listed.
    Search {
        /// Words the file name has to contain.
        words: Vec<String>,
        /// Full paths instead of file names.
        #[arg(long, conflicts_with = "json")]
        paths: bool,
        /// JSON output (name, path).
        #[arg(long)]
        json: bool,
        /// Read the words as regular expressions.
        #[arg(long)]
        regex: bool,
        /// Walk the subfolders of the sources even when `recursive` is off.
        #[arg(long)]
        deep: bool,
    },
    /// Show or change the configuration
    ///
    /// The configuration lives in ~/.config/wally/config.yaml:
    ///
    ///   sources     folders the images come from (`settings source add|remove`)
    ///   recursive   walk their subfolders too: true or false
    ///   interval    how long an image stays: 45s, 15m, 2h, 1h 30m; a bare number
    ///               counts minutes (10s at least)
    ///   order       r or random (each image once before any twice), s or sequential
    ///   screens     d or distinct (an image per screen), s or same
    ///
    /// The daemon follows every change, no restart needed.
    #[command(verbatim_doc_comment, about = "Show or change the configuration")]
    Settings {
        #[command(subcommand)]
        action: Option<Settings>,
    },
    /// Install, start or stop the daemon that changes the wallpapers.
    Daemon {
        #[command(subcommand)]
        action: Daemon,
    },
    /// Print the banner on standard output.
    #[command(hide = true)]
    Banner,
}

#[derive(Subcommand)]
enum Settings {
    /// Print the configuration (the default).
    Show,
    /// Print the path of the configuration file.
    Path,
    /// Open the configuration in $EDITOR, and check it once saved.
    Edit,
    /// Print one setting.
    Get {
        /// interval, order, screens or recursive.
        key: Key,
    },
    /// Change one setting.
    Set {
        /// interval, order, screens or recursive.
        key: Key,
        /// The new value.
        value: String,
    },
    /// Add or remove a source folder.
    Source {
        #[command(subcommand)]
        action: Source,
    },
}

#[derive(Subcommand)]
enum Source {
    /// Add a folder of images.
    Add {
        /// The folder.
        folder: String,
    },
    /// Remove a folder.
    Remove {
        /// The folder.
        folder: String,
    },
}

#[derive(Subcommand)]
enum Daemon {
    /// Start the daemon at login, and now.
    Install,
    /// Stop the daemon and stop starting it at login.
    Uninstall,
    /// Start the daemon.
    Start,
    /// Stop the daemon until the next start or login.
    Stop,
    /// Restart the daemon.
    Restart,
    /// Run the daemon in this terminal (launchd does it for you).
    Run,
}

/// Whether the command line asks for help, so that the banner goes first.
fn asks_for_help(args: &[String]) -> bool {
    args.len() <= 1
        || args.get(1).is_some_and(|a| a == "help")
        || args.iter().any(|a| a == "-h" || a == "--help")
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if asks_for_help(&args) && !args.iter().any(|a| a == "--no-banner") {
        banner::print();
    }
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("wally: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the command, returning the exit code.
fn run(cli: Cli) -> Result<ExitCode> {
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        return Ok(ExitCode::SUCCESS);
    };
    match command {
        Command::Init {
            yes,
            sources,
            no_daemon,
        } => init(yes, sources, no_daemon),
        Command::Status { json } => status(json),
        Command::Show { paths, json } => show(paths, json),
        Command::Next { screen, quiet } => next(screen.as_deref(), quiet),
        Command::Set { image, screen } => set(&image, screen.as_deref()),
        Command::Pause => pause(),
        Command::Resume => resume(),
        Command::History { limit, paths, json } => history(limit, paths, json),
        Command::Favorite { image, screen, off } => {
            mark(Mark::Favorite, image.as_deref(), screen.as_deref(), off)
        }
        Command::Ban { image, screen, off } => {
            mark(Mark::Ban, image.as_deref(), screen.as_deref(), off)
        }
        Command::Search {
            words,
            paths,
            json,
            regex,
            deep,
        } => search(&words, paths, json, regex, deep),
        Command::Settings { action } => settings(action.unwrap_or(Settings::Show)),
        Command::Daemon { action } => daemon_command(action),
        Command::Banner => {
            print!("{}", banner::render(banner::Palette::from_env()));
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The file name of `path`, or the whole path when asked or when it has none.
fn label(path: &Path, full: bool) -> String {
    match path.file_name() {
        Some(name) if !full => name.to_string_lossy().into_owned(),
        _ => path.display().to_string(),
    }
}

/// `duration` rounded to the minute past an hour, to the second below.
fn rough(duration: Duration) -> String {
    let secs = duration.as_secs();
    let secs = if secs >= 3600 { secs / 60 * 60 } else { secs };
    humantime::format_duration(Duration::from_secs(secs)).to_string()
}

/// `wally init`.
fn init(yes: bool, sources: Vec<String>, no_daemon: bool) -> Result<ExitCode> {
    let system = wally::init::detect();
    if !cfg!(target_os = "macos") {
        bail!(
            "wally only drives wallpapers on macOS for now ({})",
            system.os
        );
    }
    let assume = yes || !io::stdin().is_terminal();
    let options = wally::init::Options { sources, no_daemon };
    let mut prompt = wally::init::Prompt::new(io::stdin().lock(), io::stdout(), assume);
    let answers = wally::init::questionnaire(&mut prompt, &system, &options)?;
    let path = paths::config_file();
    answers.config.write(&path)?;
    println!("\nWrote {}", paths::tilde(&path));
    if answers.install_daemon {
        launchd::install(&launchd::program()?)?;
        println!(
            "Daemon installed: it starts at login, log in {}",
            paths::tilde(&launchd::log_path())
        );
    } else if system.daemon_installed {
        println!("The daemon picks the new configuration up by itself.");
    } else {
        println!("Start the daemon later with `wally daemon install`.");
    }
    Ok(ExitCode::SUCCESS)
}

/// `wally status`.
fn status(json: bool) -> Result<ExitCode> {
    let config = Config::load_or_init()?;
    let state = State::load(&paths::state_file());
    let images = engine::library(&config, false);
    let marks = Marks::load(&paths::marks_file());
    let pid = daemon::running_pid();
    let installed = launchd::installed();
    let next = daemon::next_change(&state, &config);
    let screens = engine::screens().unwrap_or_default();
    if json {
        let value = json!({
            "daemon": { "installed": installed, "running": pid.is_some(), "pid": pid },
            "config": paths::config_file(),
            "sources": config.source_paths(),
            "images": images.len(),
            "paused": state.paused,
            "favorites": marks.favorites.len(),
            "banned": marks.banned.len(),
            "interval": config.interval,
            "order": config.order.to_string(),
            "screens": config.screens.to_string(),
            "last_change": state.last_change.map(humantime::format_rfc3339_seconds).map(|t| t.to_string()),
            "next_change": next.filter(|_| pid.is_some()).map(humantime::format_rfc3339_seconds).map(|t| t.to_string()),
            "displays": screens.len(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(ExitCode::SUCCESS);
    }
    let daemon = match (pid, installed) {
        (Some(pid), true) => format!("running (pid {pid}), starts at login"),
        (Some(pid), false) => format!("running (pid {pid}), not started at login"),
        (None, true) => "stopped — `wally daemon start`".into(),
        (None, false) => "not installed — `wally daemon install`".into(),
    };
    let now = SystemTime::now();
    let row = |key: &str, value: &str| println!("{key:<10} {value}");
    row("daemon", &daemon);
    row("config", &paths::tilde(&paths::config_file()));
    for (i, source) in config.source_paths().iter().enumerate() {
        let missing = if source.is_dir() { "" } else { "  (missing)" };
        row(
            if i == 0 { "sources" } else { "" },
            &format!("{}{missing}", paths::tilde(source)),
        );
    }
    row(
        "images",
        &format!(
            "{}, {} favorite{}, {} banned",
            images.len(),
            marks.favorites.len(),
            if marks.favorites.len() == 1 { "" } else { "s" },
            marks.banned.len()
        ),
    );
    row(
        "rotation",
        &format!(
            "every {}, {}, {} per screen",
            config.interval, config.order, config.screens
        )
        .replace("same per screen", "same on every screen"),
    );
    if let Some(last) = state.last_change {
        row(
            "changed",
            &format!(
                "{} ago",
                rough(now.duration_since(last).unwrap_or_default())
            ),
        );
    }
    match (next, pid) {
        _ if state.paused => row("next", "paused — `wally resume`"),
        (Some(next), Some(_)) => {
            let wait = next.duration_since(now).unwrap_or_default();
            row(
                "next",
                &if wait.is_zero() {
                    "now".into()
                } else {
                    format!("in {}", rough(wait))
                },
            );
        }
        (None, Some(_)) => row("next", "now"),
        _ => {}
    }
    row("screens", &screens.len().to_string());
    Ok(ExitCode::SUCCESS)
}

/// `wally show`.
fn show(full: bool, json: bool) -> Result<ExitCode> {
    let screens = engine::screens()?;
    if json {
        let value: Vec<_> = screens
            .iter()
            .enumerate()
            .map(|(i, s)| json!({ "screen": i + 1, "id": s.id, "name": s.name, "path": s.wallpaper }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(ExitCode::SUCCESS);
    }
    let width = screens
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(0);
    for (i, s) in screens.iter().enumerate() {
        let image = match s.wallpaper.as_deref() {
            Some(p) if p.is_dir() => {
                format!("{}/  (a folder macOS rotates itself)", label(p, full))
            }
            Some(p) => label(p, full),
            None => "?".into(),
        };
        println!("{}  {:<width$}  {image}", i + 1, s.name);
    }
    Ok(ExitCode::SUCCESS)
}

/// The display id of the screen `which` names: its number in `wally show`,
/// or part of its name.
fn pick_screen(screens: &[Screen], which: &str) -> Result<u32> {
    if let Ok(n) = which.parse::<usize>() {
        return screens
            .get(n.wrapping_sub(1))
            .map(|s| s.id)
            .with_context(|| format!("no screen {n}: there are {}", screens.len()));
    }
    let needle = which.to_lowercase();
    let found: Vec<&Screen> = screens
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&needle))
        .collect();
    match found.as_slice() {
        [one] => Ok(one.id),
        [] => bail!("no screen named like `{which}`"),
        _ => bail!("several screens are named like `{which}`: use their number"),
    }
}

/// Whether the running daemon carries out the commands that change
/// something.
fn via_daemon() -> bool {
    client::current_route() == Route::Daemon
}

/// Runs `command` on the daemon and reads the status it answers.
fn daemon_status(command: &protocol::Command) -> Result<Status> {
    serde_json::from_value(client::call(command)?).context("the daemon answered no status")
}

/// `wally next`.
fn next(screen: Option<&str>, quiet: bool) -> Result<ExitCode> {
    let screen = match screen {
        Some(which) => Some(pick_screen(&desktop::screens()?, which)?),
        None => None,
    };
    if via_daemon() {
        let status = daemon_status(&protocol::Command::Next { screen })?;
        if !quiet {
            for s in status
                .screens
                .iter()
                .filter(|s| screen.is_none_or(|id| id == s.id))
            {
                let image = s.image.as_deref().map_or("?".into(), |p| label(p, false));
                println!("{}  {image}", s.name);
            }
        }
        return Ok(ExitCode::SUCCESS);
    }
    let config = Config::load_or_init()?;
    let targets = match screen {
        Some(id) => Targets::Only(vec![id]),
        None => Targets::All,
    };
    let changes = engine::change(&config, &targets)?;
    if !quiet {
        for c in changes {
            println!("{}  {}", c.screen.name, label(&c.image, false));
        }
        if daemon::running_pid().is_none() {
            eprintln!(
                "note: macOS only changed the current Space; start the daemon \
                 (`wally daemon install`) so the other Spaces follow"
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// The display id of the screen `which` names, or of the first screen.
fn screen_or_first(which: Option<&str>) -> Result<u32> {
    let screens = desktop::screens()?;
    match which {
        Some(which) => pick_screen(&screens, which),
        None => screens
            .first()
            .map(|s| s.id)
            .context("no screen is connected"),
    }
}

/// `wally set`.
fn set(image: &Path, which: Option<&str>) -> Result<ExitCode> {
    let image = absolute(image)?;
    let screen = screen_or_first(which)?;
    if via_daemon() {
        let command = protocol::Command::Set {
            screen,
            image: image.display().to_string(),
        };
        let status = daemon_status(&command)?;
        let name = status
            .screens
            .iter()
            .find(|s| s.id == screen)
            .map_or(String::new(), |s| s.name.clone());
        println!("{name}  {}", label(&image, false));
        return Ok(ExitCode::SUCCESS);
    }
    let change = engine::set(screen, &image)?;
    println!("{}  {}", change.screen.name, label(&change.image, false));
    Ok(ExitCode::SUCCESS)
}

/// `wally pause`.
fn pause() -> Result<ExitCode> {
    if via_daemon() {
        client::call(&protocol::Command::Pause)?;
    } else {
        engine::pause()?;
    }
    println!("paused: the wallpapers stay until `wally resume`");
    Ok(ExitCode::SUCCESS)
}

/// `wally resume`.
fn resume() -> Result<ExitCode> {
    if via_daemon() {
        client::call(&protocol::Command::Resume)?;
    } else {
        engine::resume()?;
    }
    let config = Config::load_or_init()?;
    println!("resumed: next change in {}", config.interval);
    Ok(ExitCode::SUCCESS)
}

/// `wally history`.
fn history(limit: usize, full: bool, json: bool) -> Result<ExitCode> {
    let entries = wally::history::read(&paths::history_file(), limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(ExitCode::SUCCESS);
    }
    let width = entries
        .iter()
        .map(|e| e.screen_name.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = io::stdout().lock();
    for e in &entries {
        writeln!(
            out,
            "{}  {:<width$}  {}",
            e.at.replace('T', " ").trim_end_matches('Z'),
            e.screen_name,
            label(&e.image, full)
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

/// `image` as an absolute path, read from the current directory.
fn absolute(image: &Path) -> Result<PathBuf> {
    if image.is_absolute() {
        return Ok(image.to_path_buf());
    }
    let full = env::current_dir()?.join(image);
    Ok(full.canonicalize().unwrap_or(full))
}

/// The image `wally favorite` or `wally ban` marks: the one named, or the one
/// on the screen `which` names, or on the first screen.
fn image_to_mark(image: Option<&Path>, which: Option<&str>) -> Result<PathBuf> {
    if let Some(image) = image {
        return absolute(image);
    }
    let screen = match which {
        Some(which) => Some(pick_screen(&desktop::screens()?, which)?),
        None => None,
    };
    engine::marked_image(None, screen)
}

/// What `wally favorite` or `wally ban` says once done.
fn marked(mark: Mark, image: &Path, on: bool) -> String {
    let name = label(image, false);
    match (mark, on) {
        (Mark::Favorite, true) => format!("{name} is a favorite"),
        (Mark::Favorite, false) => format!("{name} is no longer a favorite"),
        (Mark::Ban, true) => format!("{name} is banned"),
        (Mark::Ban, false) => format!("{name} is no longer banned"),
    }
}

/// `wally favorite` and `wally ban`.
fn mark(mark: Mark, image: Option<&Path>, which: Option<&str>, off: bool) -> Result<ExitCode> {
    let image = image_to_mark(image, which)?;
    if via_daemon() {
        let target = Target {
            image: Some(image.display().to_string()),
            screen: None,
            on: !off,
        };
        client::call(&match mark {
            Mark::Favorite => protocol::Command::Favorite(target),
            Mark::Ban => protocol::Command::Ban(target),
        })?;
        println!("{}", marked(mark, &image, !off));
        return Ok(ExitCode::SUCCESS);
    }
    let config = Config::load_or_init()?;
    let changes = engine::mark(&config, mark, &image, !off)?;
    println!("{}", marked(mark, &image, !off));
    for c in changes {
        println!("{}  {}", c.screen.name, label(&c.image, false));
    }
    Ok(ExitCode::SUCCESS)
}

/// `wally search`.
fn search(words: &[String], full: bool, json: bool, regex: bool, deep: bool) -> Result<ExitCode> {
    let config = Config::load_or_init()?;
    let images = engine::library(&config, deep);
    let found = library::search(&images, words, regex)?;
    if json {
        let value: Vec<_> = found
            .iter()
            .map(|p| json!({ "name": label(p, false), "path": p }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        let mut out = io::stdout().lock();
        for path in &found {
            writeln!(out, "{}", label(path, full))?;
        }
    }
    Ok(if found.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// `folder` as the daemon reads it: relative to the current directory
/// unless it starts with `/` or `~`.
fn folder_argument(folder: &str) -> Result<String> {
    if folder.starts_with('/') || folder.starts_with('~') {
        return Ok(folder.to_string());
    }
    Ok(env::current_dir()?.join(folder).display().to_string())
}

/// `wally settings`.
fn settings(action: Settings) -> Result<ExitCode> {
    let path = paths::config_file();
    let mut config = Config::load_or_init()?;
    match action {
        Settings::Show => {
            let text = std::fs::read_to_string(&path)?;
            if io::stdout().is_terminal() {
                println!("# {}\n", paths::tilde(&path));
            }
            print!("{text}");
        }
        Settings::Path => println!("{}", path.display()),
        Settings::Get { key } => println!("{}", config.get(key)),
        Settings::Set { key, value } => {
            if via_daemon() {
                let command = protocol::Command::ConfigSet {
                    key: key.to_string(),
                    value,
                };
                config = serde_json::from_value(client::call(&command)?)?;
            } else {
                config.set(key, &value)?;
                config.write(&path)?;
            }
            println!("{key}: {}", config.get(key));
        }
        Settings::Source { action } => {
            if via_daemon() {
                let command = match action {
                    Source::Add { folder } => protocol::Command::SourceAdd {
                        folder: folder_argument(&folder)?,
                    },
                    Source::Remove { folder } => protocol::Command::SourceRemove {
                        folder: folder_argument(&folder)?,
                    },
                };
                config = serde_json::from_value(client::call(&command)?)?;
            } else {
                match action {
                    Source::Add { folder } => config.add_source(&folder)?,
                    Source::Remove { folder } => config.remove_source(&folder)?,
                }
                config.write(&path)?;
            }
            for source in &config.sources {
                println!("{source}");
            }
        }
        Settings::Edit => edit(&path)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// Opens the configuration in `$VISUAL` or `$EDITOR` on a copy, and puts the
/// copy in place only once it reads as a valid configuration.
fn edit(path: &Path) -> Result<()> {
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let draft: PathBuf = path.with_extension("draft.yaml");
    std::fs::copy(path, &draft)?;
    loop {
        let status = Process::new("sh")
            .arg("-c")
            .arg(format!("{editor} \"$1\""))
            .arg("sh")
            .arg(&draft)
            .status()
            .with_context(|| format!("cannot run {editor}"))?;
        if !status.success() {
            std::fs::remove_file(&draft)?;
            bail!("{editor} failed, the configuration is unchanged");
        }
        match Config::read(&draft) {
            Ok(_) => {
                std::fs::rename(&draft, path)?;
                return Ok(());
            }
            Err(e) => {
                eprintln!("wally: {e:#}");
                eprint!("Edit again? [Y/n] ");
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("n") {
                    std::fs::remove_file(&draft)?;
                    bail!("the configuration is unchanged");
                }
            }
        }
    }
}

/// `wally daemon`.
fn daemon_command(action: Daemon) -> Result<ExitCode> {
    match action {
        Daemon::Run => daemon::run()?,
        Daemon::Install => {
            launchd::install(&launchd::program()?)?;
            println!(
                "daemon installed: it starts at login, log in {}",
                paths::tilde(&launchd::log_path())
            );
        }
        Daemon::Uninstall => {
            launchd::uninstall()?;
            println!("daemon uninstalled");
        }
        Daemon::Start => {
            launchd::start()?;
            println!("daemon started");
        }
        Daemon::Stop => {
            launchd::stop()?;
            println!("daemon stopped");
        }
        Daemon::Restart => {
            launchd::restart()?;
            println!("daemon restarted");
        }
    }
    Ok(ExitCode::SUCCESS)
}

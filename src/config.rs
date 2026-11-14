//! The configuration, `~/.config/wally/config.yaml`: where the images come
//! from, how often they change, in which order, and whether every screen gets
//! its own.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths;

/// The shortest interval accepted, so that a typo never makes the screens
/// flicker.
pub const MIN_INTERVAL: Duration = Duration::from_secs(10);

/// The first lines of a configuration file wally writes.
const HEADER: &str = "# wally configuration — see `wally help settings`.\n";

/// The order images are drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    /// Shuffled, and every image comes once before any comes again.
    #[default]
    Random,
    /// By path, alphabetically.
    Sequential,
}

/// Whether the screens share an image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Spread {
    /// Every screen gets an image of its own.
    #[default]
    Distinct,
    /// Every screen shows the same image.
    Same,
}

/// The configuration, as written in `config.yaml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Folders the images are taken from; `~` stands for the home directory.
    pub sources: Vec<String>,
    /// Whether subfolders of the sources are walked too.
    pub recursive: bool,
    /// How long an image stays, such as `30m`, `1h 30m` or `45s`.
    pub interval: String,
    /// The order images are drawn in.
    pub order: Order,
    /// Whether the screens share an image.
    pub screens: Spread,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sources: vec!["~/Pictures/Wallpapers".into()],
            recursive: false,
            interval: "30m".into(),
            order: Order::Random,
            screens: Spread::Distinct,
        }
    }
}

/// A key `wally settings set` can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// `interval`.
    Interval,
    /// `order`.
    Order,
    /// `screens`.
    Screens,
    /// `recursive`.
    Recursive,
}

impl Key {
    /// Every key, in the order the help lists them.
    pub const ALL: [Key; 4] = [Key::Interval, Key::Order, Key::Screens, Key::Recursive];

    /// The key as written in the file.
    pub fn name(self) -> &'static str {
        match self {
            Key::Interval => "interval",
            Key::Order => "order",
            Key::Screens => "screens",
            Key::Recursive => "recursive",
        }
    }
}

impl FromStr for Key {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Key::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .with_context(|| {
                let names: Vec<_> = Key::ALL.iter().map(|k| k.name()).collect();
                format!("unknown setting `{s}`: one of {}", names.join(", "))
            })
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `interval` read as a duration, refused below [`MIN_INTERVAL`].
pub fn parse_interval(text: &str) -> Result<Duration> {
    let interval = humantime::parse_duration(text)
        .with_context(|| format!("`{text}` is not a duration (try 30m, 1h, 1h 30m)"))?;
    if interval < MIN_INTERVAL {
        bail!(
            "interval `{text}` is too short: at least {}",
            humantime::format_duration(MIN_INTERVAL)
        );
    }
    Ok(interval)
}

impl Config {
    /// Reads and checks the configuration at `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        Config::parse(&text).with_context(|| format!("invalid configuration {}", path.display()))
    }

    /// Parses and checks a configuration; an empty text is the default one.
    pub fn parse(text: &str) -> Result<Self> {
        let config: Config = if text.trim().lines().all(|l| l.trim_start().starts_with('#')) {
            Config::default()
        } else {
            serde_saphyr::from_str(text).map_err(|e| anyhow::anyhow!("{e}"))?
        };
        config.check()?;
        Ok(config)
    }

    /// Reads the configuration at the usual place, writing the default one
    /// first when there is none.
    pub fn load_or_init() -> Result<Self> {
        let path = paths::config_file();
        if !path.exists() {
            Config::default().write(&path)?;
        }
        Config::read(&path)
    }

    /// Checks the values serde cannot: the interval, the sources.
    pub fn check(&self) -> Result<()> {
        parse_interval(&self.interval)?;
        if self.sources.is_empty() {
            bail!("no source: add a folder with `wally settings source add <folder>`");
        }
        Ok(())
    }

    /// Writes the configuration at `path`, creating its directory.
    pub fn write(&self, path: &Path) -> Result<()> {
        self.check()?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let body = serde_saphyr::to_string(self).map_err(|e| anyhow::anyhow!("{e}"))?;
        fs::write(path, format!("{HEADER}{body}"))
            .with_context(|| format!("cannot write {}", path.display()))
    }

    /// How long an image stays.
    pub fn interval(&self) -> Duration {
        parse_interval(&self.interval).unwrap_or(Duration::from_secs(1800))
    }

    /// The sources, `~` expanded.
    pub fn source_paths(&self) -> Vec<PathBuf> {
        self.sources.iter().map(|s| paths::expand(s)).collect()
    }

    /// The value of `key`, as written in the file.
    pub fn get(&self, key: Key) -> String {
        match key {
            Key::Interval => self.interval.clone(),
            Key::Order => self.order.to_string(),
            Key::Screens => self.screens.to_string(),
            Key::Recursive => self.recursive.to_string(),
        }
    }

    /// Sets `key` to `value`, checking it.
    pub fn set(&mut self, key: Key, value: &str) -> Result<()> {
        match key {
            Key::Interval => {
                parse_interval(value)?;
                self.interval = value.to_string();
            }
            Key::Order => self.order = value.parse()?,
            Key::Screens => self.screens = value.parse()?,
            Key::Recursive => {
                self.recursive = value
                    .parse()
                    .with_context(|| format!("`{value}` is not true or false"))?
            }
        }
        Ok(())
    }

    /// Adds a source folder, refusing one that does not exist or is already
    /// there.
    pub fn add_source(&mut self, folder: &str) -> Result<()> {
        let path = paths::expand(folder);
        if !path.is_dir() {
            bail!("{folder} is not a folder");
        }
        let written = paths::tilde(&path.canonicalize().unwrap_or(path));
        if self.source_paths().contains(&paths::expand(&written)) {
            bail!("{written} is already a source");
        }
        self.sources.push(written);
        Ok(())
    }

    /// Removes a source folder, refusing the last one.
    pub fn remove_source(&mut self, folder: &str) -> Result<()> {
        let path = paths::expand(folder);
        let path = path.canonicalize().unwrap_or(path);
        let before = self.sources.len();
        self.sources.retain(|s| {
            paths::expand(s)
                .canonicalize()
                .unwrap_or_else(|_| paths::expand(s))
                != path
        });
        if self.sources.len() == before {
            bail!("{folder} is not a source");
        }
        if self.sources.is_empty() {
            bail!("{folder} is the last source: add another one first");
        }
        Ok(())
    }
}

impl FromStr for Order {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "random" => Ok(Order::Random),
            "sequential" => Ok(Order::Sequential),
            _ => bail!("`{s}` is not an order: random or sequential"),
        }
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Order::Random => "random",
            Order::Sequential => "sequential",
        })
    }
}

impl FromStr for Spread {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "distinct" => Ok(Spread::Distinct),
            "same" => Ok(Spread::Same),
            _ => bail!("`{s}` is not a screen mode: distinct or same"),
        }
    }
}

impl fmt::Display for Spread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Spread::Distinct => "distinct",
            Spread::Same => "same",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_file_is_read() {
        let config = Config::parse(
            "sources:\n  - ~/a\n  - /b\nrecursive: true\ninterval: 1h 30m\norder: sequential\nscreens: same\n",
        )
        .unwrap();
        assert_eq!(config.sources, vec!["~/a", "/b"]);
        assert!(config.recursive);
        assert_eq!(config.interval(), Duration::from_secs(5400));
        assert_eq!(config.order, Order::Sequential);
        assert_eq!(config.screens, Spread::Same);
    }

    #[test]
    fn missing_keys_take_their_default() {
        let config = Config::parse("interval: 5m\n").unwrap();
        assert_eq!(config.sources, Config::default().sources);
        assert_eq!(config.order, Order::Random);
    }

    #[test]
    fn an_empty_file_is_the_default() {
        assert_eq!(Config::parse("# nothing\n").unwrap(), Config::default());
    }

    #[test]
    fn unknown_keys_and_bad_values_are_refused() {
        assert!(Config::parse("colour: red\n").is_err());
        assert!(Config::parse("order: backwards\n").is_err());
        assert!(Config::parse("interval: soon\n").is_err());
        assert!(Config::parse("interval: 2s\n").is_err());
        assert!(Config::parse("sources: []\n").is_err());
    }

    #[test]
    fn a_written_file_reads_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut config = Config::default();
        config.set(Key::Order, "sequential").unwrap();
        config.set(Key::Interval, "2h").unwrap();
        config.write(&path).unwrap();
        assert_eq!(Config::read(&path).unwrap(), config);
    }

    #[test]
    fn settings_are_checked() {
        let mut config = Config::default();
        assert!(config.set(Key::Interval, "1s").is_err());
        assert!(config.set(Key::Recursive, "maybe").is_err());
        assert!("colour".parse::<Key>().is_err());
        config.set(Key::Screens, "same").unwrap();
        assert_eq!(config.get(Key::Screens), "same");
    }

    #[test]
    fn sources_are_added_once_and_removed_but_never_all() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_str().unwrap();
        let mut config = Config::default();
        config.add_source(folder).unwrap();
        assert!(config.add_source(folder).is_err());
        assert!(config.add_source("/nowhere/at/all").is_err());
        config.remove_source(folder).unwrap();
        assert!(config.remove_source(folder).is_err());
        let mut lonely = Config {
            sources: vec![folder.into()],
            ..Config::default()
        };
        assert!(lonely.remove_source(folder).is_err());
    }
}

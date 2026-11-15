//! The daemon protocol, as `docs/protocol.md` describes it: the requests a
//! client sends on the socket, one JSON object per line, and the responses,
//! status and events the daemon sends back.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::daemon;
use crate::desktop;
use crate::marks::Marks;
use crate::rotation::State;

/// The protocol version a client and the daemon have to share.
pub const PROTOCOL: u32 = 1;

/// The version of wally.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How many history entries `history` returns when no limit is given.
pub const HISTORY_LIMIT: usize = 50;

/// Every command name, as sent in `cmd`.
pub const COMMANDS: [&str; 15] = [
    "hello",
    "status",
    "next",
    "set",
    "pause",
    "resume",
    "favorite",
    "ban",
    "history",
    "library",
    "config_get",
    "config_set",
    "source_add",
    "source_remove",
    "subscribe",
];

/// The arguments of `favorite` and `ban`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// The image to mark; the image of `screen` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// The display id of the screen whose image to mark; the first screen
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<u32>,
    /// Whether to mark, or to unmark.
    #[serde(default = "yes")]
    pub on: bool,
}

/// `true`, the default of [`Target::on`].
fn yes() -> bool {
    true
}

/// [`HISTORY_LIMIT`], the default of `history`'s `limit`.
fn history_limit() -> usize {
    HISTORY_LIMIT
}

/// A string, or a number or boolean written as one, so that `config_set`
/// takes `"value": true` as well as `"value": "true"`.
fn stringish<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::String(text) => Ok(text),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "`{other}` is not a string"
        ))),
    }
}

/// A request, without its `id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// The version and the protocol of the daemon.
    Hello,
    /// The status.
    Status,
    /// A change now, on one screen or all.
    Next {
        /// The display id of the screen; every screen when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screen: Option<u32>,
    },
    /// An image put on a screen.
    Set {
        /// The display id of the screen.
        screen: u32,
        /// The image.
        image: String,
    },
    /// A pause of the scheduled changes.
    Pause,
    /// The end of the pause.
    Resume,
    /// An image marked as a favorite, or unmarked.
    Favorite(Target),
    /// An image banned, or allowed again.
    Ban(Target),
    /// The last images shown.
    History {
        /// How many entries, newest first.
        #[serde(default = "history_limit")]
        limit: usize,
    },
    /// The images of the sources, banned ones left out.
    Library {
        /// Words the file names have to contain, as in `wally search`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
    },
    /// The configuration.
    ConfigGet,
    /// One setting changed.
    ConfigSet {
        /// `interval`, `order`, `screens` or `recursive`.
        key: String,
        /// The new value.
        #[serde(deserialize_with = "stringish")]
        value: String,
    },
    /// A source folder added.
    SourceAdd {
        /// The folder, `~` allowed.
        folder: String,
    },
    /// A source folder removed.
    SourceRemove {
        /// The folder.
        folder: String,
    },
    /// The status, then an event at every change of it.
    Subscribe,
}

/// A request as sent on the socket.
#[derive(Clone, Debug, Serialize)]
pub struct Request<'a> {
    /// The number the response echoes.
    pub id: u64,
    /// The command and its arguments.
    #[serde(flatten)]
    pub command: &'a Command,
}

/// A response, as sent on the socket.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The `id` of the request, `null` when it had none.
    #[serde(default)]
    pub id: Value,
    /// Whether the command succeeded.
    pub ok: bool,
    /// What the command gives, when it succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Why the command failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    /// The response to the request `id`, from what the command gave.
    pub fn new(id: Value, outcome: Result<Value, String>) -> Self {
        match outcome {
            Ok(result) => Response {
                id,
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => Response {
                id,
                ok: false,
                result: None,
                error: Some(error),
            },
        }
    }

    /// The response as a line of the socket, `\n` included.
    pub fn line(&self) -> String {
        line(self)
    }
}

/// `value` as a line of the socket, `\n` included.
fn line<T: Serialize>(value: &T) -> String {
    let mut text = serde_json::to_string(value).unwrap_or_else(|_| "{}".into());
    text.push('\n');
    text
}

/// The `id` of a request line and its command, or why the line is not one.
pub fn parse(text: &str) -> (Value, Result<Command, String>) {
    let value: Value = match serde_json::from_str(text) {
        Ok(value @ Value::Object(_)) => value,
        Ok(_) => return (Value::Null, Err("a request is a JSON object".into())),
        Err(e) => return (Value::Null, Err(format!("invalid JSON: {e}"))),
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    match value.get("cmd") {
        Some(Value::String(cmd)) if !COMMANDS.contains(&cmd.as_str()) => {
            return (id, Err(format!("unknown command `{cmd}`")));
        }
        Some(Value::String(_)) => {}
        _ => return (id, Err("missing command: `cmd` names one".into())),
    }
    let command = Command::deserialize(value).map_err(|e| e.to_string());
    (id, command)
}

/// The result of `hello`.
pub fn hello() -> Value {
    serde_json::json!({ "version": VERSION, "protocol": PROTOCOL })
}

/// The status event pushed to subscribers, as a line of the socket.
pub fn event<T: Serialize>(status: &T) -> String {
    line(&serde_json::json!({ "event": "status", "status": status }))
}

/// A screen, in the status.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    /// The 1-based position, left to right, as in `wally show`.
    pub index: usize,
    /// The display id.
    pub id: u32,
    /// The name macOS gives the screen.
    pub name: String,
    /// The image macOS reports for the screen.
    pub image: Option<PathBuf>,
    /// Whether that image is a favorite.
    pub favorite: bool,
}

/// The status of the daemon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// The version of wally.
    pub version: String,
    /// The protocol version.
    pub protocol: u32,
    /// The pid of the daemon.
    pub pid: u32,
    /// Whether the scheduled changes are paused.
    pub paused: bool,
    /// The interval, as written in the configuration.
    pub interval: String,
    /// The interval, in seconds.
    pub interval_secs: u64,
    /// `random` or `sequential`.
    pub order: String,
    /// `distinct` or `same`.
    pub screens_mode: String,
    /// Whether the subfolders of the sources are walked.
    pub recursive: bool,
    /// The sources, as written in the configuration.
    pub sources: Vec<String>,
    /// How many images the sources hold, banned ones left out.
    pub images: usize,
    /// How many favorites.
    pub favorites: usize,
    /// How many banned images.
    pub banned: usize,
    /// When the images last changed on schedule, by `next` or by `set`.
    pub last_change: Option<String>,
    /// When the next scheduled change comes; `null` while paused.
    pub next_change: Option<String>,
    /// The screens, left to right.
    pub screens: Vec<Screen>,
    /// The last error of the daemon loop.
    pub error: Option<String>,
}

/// What the status is made of.
pub struct Snapshot<'a> {
    /// The configuration in use.
    pub config: &'a Config,
    /// The rotation state.
    pub state: &'a State,
    /// The marks.
    pub marks: &'a Marks,
    /// How many images the sources hold, banned ones left out.
    pub images: usize,
    /// The screens, left to right.
    pub screens: &'a [desktop::Screen],
    /// The pid of the daemon.
    pub pid: u32,
    /// The last error of the daemon loop.
    pub error: Option<&'a str>,
}

/// `time` in RFC 3339, UTC, to the second.
fn timestamp(time: std::time::SystemTime) -> String {
    humantime::format_rfc3339_seconds(time).to_string()
}

impl Status {
    /// The status `snapshot` tells.
    pub fn new(snapshot: &Snapshot) -> Self {
        let Snapshot {
            config,
            state,
            marks,
            ..
        } = snapshot;
        Status {
            version: VERSION.into(),
            protocol: PROTOCOL,
            pid: snapshot.pid,
            paused: state.paused,
            interval: config.interval.clone(),
            interval_secs: config.interval().as_secs(),
            order: config.order.to_string(),
            screens_mode: config.screens.to_string(),
            recursive: config.recursive,
            sources: config.sources.clone(),
            images: snapshot.images,
            favorites: marks.favorites.len(),
            banned: marks.banned.len(),
            last_change: state.last_change.map(timestamp),
            next_change: daemon::next_change(state, config).map(timestamp),
            screens: snapshot
                .screens
                .iter()
                .enumerate()
                .map(|(i, s)| Screen {
                    index: i + 1,
                    id: s.id,
                    name: s.name.clone(),
                    image: s.wallpaper.clone(),
                    favorite: s.wallpaper.as_deref().is_some_and(|p| marks.is_favorite(p)),
                })
                .collect(),
            error: snapshot.error.map(str::to_string),
        }
    }
}

/// An image of the library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryItem {
    /// The path.
    pub path: PathBuf,
    /// The file name.
    pub name: String,
    /// Whether it is a favorite.
    pub favorite: bool,
}

impl LibraryItem {
    /// The item for `path`, given the marks.
    pub fn new(path: &Path, marks: &Marks) -> Self {
        LibraryItem {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            favorite: marks.is_favorite(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marks::Mark;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn requests_parse_with_their_arguments_and_defaults() {
        let (id, command) = parse(r#"{"id": 1, "cmd": "next", "screen": 2, "extra": true}"#);
        assert_eq!(id, json!(1));
        assert_eq!(command, Ok(Command::Next { screen: Some(2) }));
        assert_eq!(
            parse(r#"{"id": 2, "cmd": "next"}"#).1,
            Ok(Command::Next { screen: None })
        );
        assert_eq!(
            parse(r#"{"id": 3, "cmd": "ban", "screen": 5}"#).1,
            Ok(Command::Ban(Target {
                image: None,
                screen: Some(5),
                on: true
            }))
        );
        assert_eq!(
            parse(r#"{"id": 4, "cmd": "favorite", "image": "/w/a.jpg", "on": false}"#).1,
            Ok(Command::Favorite(Target {
                image: Some("/w/a.jpg".into()),
                screen: None,
                on: false
            }))
        );
        assert_eq!(
            parse(r#"{"id": 5, "cmd": "history"}"#).1,
            Ok(Command::History {
                limit: HISTORY_LIMIT
            })
        );
        assert_eq!(
            parse(r#"{"id": 6, "cmd": "config_set", "key": "recursive", "value": true}"#).1,
            Ok(Command::ConfigSet {
                key: "recursive".into(),
                value: "true".into()
            })
        );
        assert_eq!(parse(r#"{"id": 7, "cmd": "pause"}"#).1, Ok(Command::Pause));
    }

    #[test]
    fn every_command_name_parses() {
        let arguments = json!({"screen": 1, "image": "/a.jpg", "key": "order",
            "value": "random", "folder": "~/w", "query": "sea", "limit": 3});
        for name in COMMANDS {
            let mut request = arguments.clone();
            request["id"] = json!(1);
            request["cmd"] = json!(name);
            let (_, command) = parse(&request.to_string());
            let command = command.unwrap_or_else(|e| panic!("{name}: {e}"));
            let written = serde_json::to_value(&command).unwrap();
            assert_eq!(written["cmd"], json!(name));
        }
    }

    #[test]
    fn bad_requests_say_why() {
        assert_eq!(
            parse(r#"{"id": 9, "cmd": "dance"}"#),
            (json!(9), Err("unknown command `dance`".into()))
        );
        assert!(parse(r#"{"id": 9}"#).1.is_err());
        assert!(parse("not json").1.unwrap_err().starts_with("invalid JSON"));
        assert!(parse("[1]").1.is_err());
        let (id, missing) = parse(r#"{"id": 3, "cmd": "set", "screen": 1}"#);
        assert_eq!(id, json!(3));
        assert!(missing.unwrap_err().contains("image"));
    }

    #[test]
    fn requests_are_written_with_their_id() {
        let command = Command::Set {
            screen: 4,
            image: "/w/a.jpg".into(),
        };
        let request = Request {
            id: 7,
            command: &command,
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({"id": 7, "cmd": "set", "screen": 4, "image": "/w/a.jpg"})
        );
    }

    #[test]
    fn responses_are_written_as_the_protocol_says() {
        let ok = Response::new(json!(1), Ok(json!({"a": 1}))).line();
        assert_eq!(ok, "{\"id\":1,\"ok\":true,\"result\":{\"a\":1}}\n");
        let failed = Response::new(json!(2), Err("no screen with id 9".into())).line();
        assert_eq!(
            failed,
            "{\"id\":2,\"ok\":false,\"error\":\"no screen with id 9\"}\n"
        );
        let read: Response = serde_json::from_str(&failed).unwrap();
        assert_eq!(read.error.as_deref(), Some("no screen with id 9"));
    }

    #[test]
    fn the_status_tells_the_screens_marks_and_schedule() {
        let config = Config::default();
        let last = humantime::parse_rfc3339("2026-11-15T10:00:00Z").unwrap();
        let state = State {
            last_change: Some(last),
            ..State::default()
        };
        let mut marks = Marks::default();
        marks.set(Mark::Favorite, Path::new("/w/a.jpg"), true);
        marks.set(Mark::Ban, Path::new("/w/b.jpg"), true);
        let screens = vec![desktop::Screen {
            id: 42,
            name: "DELL".into(),
            x: 0.0,
            wallpaper: Some(PathBuf::from("/w/a.jpg")),
        }];
        let snapshot = Snapshot {
            config: &config,
            state: &state,
            marks: &marks,
            images: 10,
            screens: &screens,
            pid: 1234,
            error: None,
        };
        let value = serde_json::to_value(Status::new(&snapshot)).unwrap();
        assert_eq!(
            value,
            json!({"version": VERSION, "protocol": 1, "pid": 1234, "paused": false,
                "interval": "30m", "interval_secs": 1800, "order": "random",
                "screens_mode": "distinct", "recursive": false,
                "sources": ["~/Pictures/Wallpapers"], "images": 10, "favorites": 1,
                "banned": 1, "last_change": "2026-11-15T10:00:00Z",
                "next_change": "2026-11-15T10:30:00Z",
                "screens": [{"index": 1, "id": 42, "name": "DELL",
                    "image": "/w/a.jpg", "favorite": true}],
                "error": null})
        );
        let mut paused_state = state.clone();
        paused_state.pause();
        let paused = Status::new(&Snapshot {
            state: &paused_state,
            error: Some("no image in ~/w"),
            ..snapshot
        });
        assert_eq!(paused.next_change, None);
        assert_eq!(paused.error.as_deref(), Some("no image in ~/w"));
        assert_eq!(
            event(&paused),
            format!("{}\n", json!({"event": "status", "status": paused}))
        );
        assert!(Duration::from_secs(paused.interval_secs) == config.interval());
    }

    #[test]
    fn library_items_tell_their_name_and_mark() {
        let mut marks = Marks::default();
        marks.set(Mark::Favorite, Path::new("/w/a.jpg"), true);
        assert_eq!(
            serde_json::to_value(LibraryItem::new(Path::new("/w/a.jpg"), &marks)).unwrap(),
            json!({"path": "/w/a.jpg", "name": "a.jpg", "favorite": true})
        );
    }
}

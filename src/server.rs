//! The daemon socket: a Unix socket the menu bar app and the CLI talk to.
//! Connections are served on background threads; every command but `hello`
//! is handed to the main loop as a [`Job`], since AppKit only works on the
//! main thread.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::protocol::{self, Command, Response};

/// What a command gives back: its result, or why it failed.
pub type Outcome = Result<Value, String>;

/// A command for the main loop, with where to send its outcome.
pub struct Job {
    /// The command.
    pub command: Command,
    /// Where the outcome goes.
    pub reply: Sender<Outcome>,
    /// For `subscribe`, where the status events go, one socket line each.
    pub events: Option<Sender<String>>,
}

/// Binds the socket at `path`, mode `0600`, replacing any socket left there,
/// and serves its connections on background threads, handing their commands
/// to `jobs`.
pub fn listen(path: &Path, jobs: Sender<Job>) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    if fs::symlink_metadata(path).is_ok() {
        fs::remove_file(path).with_context(|| format!("cannot remove {}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("cannot listen on {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("cannot protect {}", path.display()))?;
    thread::Builder::new()
        .name("wally-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let jobs = jobs.clone();
                let _ = thread::Builder::new()
                    .name("wally-connection".into())
                    .spawn(move || converse(stream, jobs));
            }
        })
        .context("cannot start the socket thread")?;
    Ok(())
}

/// Hands `command` to the main loop and waits for its outcome.
fn ask(jobs: &Sender<Job>, command: Command, events: Option<Sender<String>>) -> Outcome {
    let (reply, outcome) = mpsc::channel();
    let job = Job {
        command,
        reply,
        events,
    };
    jobs.send(job)
        .map_err(|_| "the daemon is stopping".to_string())?;
    outcome
        .recv()
        .map_err(|_| "the daemon is stopping".to_string())?
}

/// Writes `line` on `stream`, telling whether it went through.
fn send(stream: &mut UnixStream, line: &str) -> bool {
    stream.write_all(line.as_bytes()).is_ok() && stream.flush().is_ok()
}

/// Serves one connection: answers its requests in order until it closes, or
/// pushes the status events once it subscribes.
fn converse(stream: UnixStream, jobs: Sender<Job>) {
    let Ok(reading) = stream.try_clone() else {
        return;
    };
    let mut writing = stream;
    for text in BufReader::new(reading).lines() {
        let Ok(text) = text else {
            return;
        };
        if text.trim().is_empty() {
            continue;
        }
        let (id, command) = protocol::parse(&text);
        let outcome = match command {
            Err(e) => Err(e),
            Ok(Command::Hello) => Ok(protocol::hello()),
            Ok(Command::Subscribe) => {
                let (events, pushed) = mpsc::channel();
                let outcome = ask(&jobs, Command::Subscribe, Some(events));
                let subscribed = outcome.is_ok();
                if send(&mut writing, &Response::new(id, outcome).line()) && subscribed {
                    push(&mut writing, pushed);
                }
                return;
            }
            Ok(command) => ask(&jobs, command, None),
        };
        if !send(&mut writing, &Response::new(id, outcome).line()) {
            return;
        }
    }
}

/// Writes every event of `pushed` on `stream` until either side closes.
fn push(stream: &mut UnixStream, pushed: Receiver<String>) {
    for line in pushed {
        if !send(stream, &line) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::BufRead;

    fn serve_fake(path: &Path) -> Sender<String> {
        let (jobs, received) = mpsc::channel::<Job>();
        listen(path, jobs).unwrap();
        let (broadcast, broadcasts) = mpsc::channel::<String>();
        thread::spawn(move || {
            let mut subscribers: Vec<Sender<String>> = Vec::new();
            loop {
                if let Ok(line) = broadcasts.try_recv() {
                    subscribers.retain(|s| s.send(line.clone()).is_ok());
                }
                match received.recv_timeout(std::time::Duration::from_millis(10)) {
                    Ok(job) => {
                        let outcome = match &job.command {
                            Command::Next { screen: Some(9) } => {
                                Err("no screen with id 9".to_string())
                            }
                            Command::Subscribe => Ok(json!({"paused": false})),
                            other => Ok(serde_json::to_value(other).unwrap()),
                        };
                        if let Some(events) = job.events {
                            subscribers.push(events);
                        }
                        let _ = job.reply.send(outcome);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        });
        broadcast
    }

    fn exchange(stream: &mut UnixStream, reader: &mut impl BufRead, line: &str) -> Value {
        stream.write_all(format!("{line}\n").as_bytes()).unwrap();
        read(reader)
    }

    fn read(reader: &mut impl BufRead) -> Value {
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        serde_json::from_str(&answer).unwrap()
    }

    #[test]
    fn the_socket_is_private_and_answers_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wally/wally.sock");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"stale").unwrap();
        let _broadcast = serve_fake(&path);
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let mut stream = UnixStream::connect(&path).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let hello = exchange(&mut stream, &mut reader, r#"{"id": 1, "cmd": "hello"}"#);
        assert_eq!(
            hello,
            json!({"id": 1, "ok": true, "result": protocol::hello()})
        );
        stream
            .write_all(b"{\"id\": 2, \"cmd\": \"pause\"}\n\n{\"id\": 3, \"cmd\": \"next\", \"screen\": 9}\n")
            .unwrap();
        assert_eq!(
            read(&mut reader),
            json!({"id": 2, "ok": true, "result": {"cmd": "pause"}})
        );
        assert_eq!(
            read(&mut reader),
            json!({"id": 3, "ok": false, "error": "no screen with id 9"})
        );
        let unknown = exchange(&mut stream, &mut reader, r#"{"id": 4, "cmd": "dance"}"#);
        assert_eq!(
            unknown,
            json!({"id": 4, "ok": false, "error": "unknown command `dance`"})
        );
        let broken = exchange(&mut stream, &mut reader, "{oops");
        assert_eq!(broken["id"], Value::Null);
        assert_eq!(broken["ok"], json!(false));
    }

    #[test]
    fn a_subscriber_gets_the_status_then_the_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wally.sock");
        let broadcast = serve_fake(&path);
        let mut stream = UnixStream::connect(&path).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let first = exchange(&mut stream, &mut reader, r#"{"id": 1, "cmd": "subscribe"}"#);
        assert_eq!(
            first,
            json!({"id": 1, "ok": true, "result": {"paused": false}})
        );
        let event = protocol::event(&json!({"paused": true}));
        broadcast.send(event).unwrap();
        assert_eq!(
            read(&mut reader),
            json!({"event": "status", "status": {"paused": true}})
        );
    }
}

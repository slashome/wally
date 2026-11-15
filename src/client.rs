//! Talking to the running daemon over its socket, so that the CLI leaves the
//! daemon the only writer of the wallpapers, the configuration and the marks
//! while it runs.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::daemon;
use crate::paths;
use crate::protocol::{self, Command, Request, Response};

/// How long the CLI waits for an answer from the daemon.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Who carries out a command that changes something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The running daemon, through its socket.
    Daemon,
    /// The CLI itself, the daemon being off.
    Direct,
}

/// The route for a command, given the pid of the running daemon and whether
/// its socket exists: a daemon too old to listen is left alone.
pub fn route(pid: Option<u32>, socket_exists: bool) -> Route {
    match (pid, socket_exists) {
        (Some(_), true) => Route::Daemon,
        _ => Route::Direct,
    }
}

/// The route for a command now.
pub fn current_route() -> Route {
    route(daemon::running_pid(), paths::socket_file().exists())
}

/// A connection to the daemon, past `hello`.
struct Connection {
    /// Where requests are written.
    stream: UnixStream,
    /// Where responses are read.
    reader: BufReader<UnixStream>,
    /// The id of the next request.
    next_id: u64,
}

impl Connection {
    /// Connects to the socket at `path` and checks the daemon speaks the same
    /// protocol.
    fn open(path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(path)
            .with_context(|| format!("cannot reach the daemon at {}", path.display()))?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let reader = BufReader::new(stream.try_clone()?);
        let mut connection = Connection {
            stream,
            reader,
            next_id: 1,
        };
        let hello = connection.send(&Command::Hello)?;
        let protocol = hello.get("protocol").and_then(Value::as_u64);
        if protocol != Some(u64::from(protocol::PROTOCOL)) {
            bail!(
                "the daemon speaks protocol {}, this wally {}: restart it with `wally daemon restart`",
                protocol.map_or("?".into(), |p| p.to_string()),
                protocol::PROTOCOL
            );
        }
        Ok(connection)
    }

    /// Sends `command` and returns its result, or its error.
    fn send(&mut self, command: &Command) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let mut text = serde_json::to_string(&Request { id, command })?;
        text.push('\n');
        self.stream
            .write_all(text.as_bytes())
            .context("cannot write to the daemon")?;
        let mut answer = String::new();
        self.reader
            .read_line(&mut answer)
            .context("no answer from the daemon")?;
        if answer.is_empty() {
            bail!("the daemon closed the connection");
        }
        let response: Response =
            serde_json::from_str(&answer).context("the daemon answered nonsense")?;
        if response.id != id {
            bail!("the daemon answered another request");
        }
        match (response.ok, response.result, response.error) {
            (true, result, _) => Ok(result.unwrap_or(Value::Null)),
            (false, _, error) => bail!("{}", error.unwrap_or_else(|| "the daemon failed".into())),
        }
    }
}

/// Runs `command` on the daemon listening at `path`.
pub fn call_at(path: &Path, command: &Command) -> Result<Value> {
    Connection::open(path)?.send(command)
}

/// Runs `command` on the running daemon.
pub fn call(command: &Command) -> Result<Value> {
    call_at(&paths::socket_file(), command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{self, Job};
    use serde_json::json;
    use std::sync::mpsc;
    use std::thread;

    fn serve_echo(path: &Path) {
        let (jobs, received) = mpsc::channel::<Job>();
        server::listen(path, jobs).unwrap();
        thread::spawn(move || {
            for job in received {
                let outcome = match &job.command {
                    Command::Pause => Err("already paused".to_string()),
                    other => Ok(serde_json::to_value(other).unwrap()),
                };
                let _ = job.reply.send(outcome);
            }
        });
    }

    #[test]
    fn mutations_go_through_the_daemon_only_when_it_listens() {
        assert_eq!(route(Some(12), true), Route::Daemon);
        assert_eq!(route(Some(12), false), Route::Direct);
        assert_eq!(route(None, true), Route::Direct);
        assert_eq!(route(None, false), Route::Direct);
    }

    #[test]
    fn a_call_returns_the_result_or_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wally.sock");
        serve_echo(&path);
        let command = Command::Next { screen: Some(3) };
        assert_eq!(
            call_at(&path, &command).unwrap(),
            json!({"cmd": "next", "screen": 3})
        );
        let error = call_at(&path, &Command::Pause).unwrap_err();
        assert_eq!(error.to_string(), "already paused");
    }

    #[test]
    fn no_daemon_means_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = call_at(&dir.path().join("none.sock"), &Command::Status).unwrap_err();
        assert!(error.to_string().contains("cannot reach the daemon"));
    }
}

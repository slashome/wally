//! The LaunchAgent that starts the daemon at login and keeps it running:
//! `~/Library/LaunchAgents/me.slashome.wally.plist`, driven by `launchctl`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use crate::paths;

/// The label of the agent.
pub const LABEL: &str = "me.slashome.wally";

/// Where the agent is declared.
pub fn plist_path() -> PathBuf {
    paths::home()
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// The agent declaration: runs `wally daemon run` from `program` in the
/// graphical session, at login, restarting it if it stops on an error.
pub fn plist(program: &Path, log: &Path) -> String {
    let escape = |p: &Path| {
        p.display()
            .to_string()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{program}</string>
		<string>daemon</string>
		<string>run</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>StandardOutPath</key>
	<string>{log}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
</dict>
</plist>
"#,
        program = escape(program),
        log = escape(log),
    )
}

/// The daemon log file.
pub fn log_path() -> PathBuf {
    paths::log_dir().join("wally.log")
}

/// The user id, for the `gui/<uid>` launchd domain.
fn uid() -> Result<String> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("cannot run `id -u`")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The launchd domain of the graphical session, `gui/<uid>`.
fn domain() -> Result<String> {
    Ok(format!("gui/{}", uid()?))
}

/// Runs `launchctl` with `args`.
fn launchctl(args: &[&str]) -> Result<Output> {
    Command::new("launchctl")
        .args(args)
        .output()
        .context("cannot run launchctl")
}

/// Fails with what `launchctl` said when `out` reports a failure.
fn check(out: Output, what: &str) -> Result<()> {
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    bail!("{what} failed: {}", said.trim())
}

/// Whether the agent is declared.
pub fn installed() -> bool {
    plist_path().exists()
}

/// Whether launchd has the agent loaded.
pub fn loaded() -> bool {
    domain()
        .and_then(|d| launchctl(&["print", &format!("{d}/{LABEL}")]))
        .is_ok_and(|out| out.status.success())
}

/// Writes the agent for `program` and loads it, replacing a previous one.
pub fn install(program: &Path) -> Result<()> {
    let path = plist_path();
    fs::create_dir_all(paths::log_dir())?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if loaded() {
        stop()?;
    }
    fs::write(&path, plist(program, &log_path()))
        .with_context(|| format!("cannot write {}", path.display()))?;
    start()
}

/// Unloads the agent and removes its declaration.
pub fn uninstall() -> Result<()> {
    if loaded() {
        stop()?;
    }
    let path = plist_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
    }
    Ok(())
}

/// Loads the agent, which starts the daemon.
pub fn start() -> Result<()> {
    if !installed() {
        bail!("the daemon is not installed: run `wally daemon install`");
    }
    if loaded() {
        return Ok(());
    }
    let path = plist_path();
    check(
        launchctl(&["bootstrap", &domain()?, &path.to_string_lossy()])?,
        "launchctl bootstrap",
    )
}

/// Unloads the agent, which stops the daemon until the next `start` or login.
pub fn stop() -> Result<()> {
    if !loaded() {
        return Ok(());
    }
    check(
        launchctl(&["bootout", &format!("{}/{LABEL}", domain()?)])?,
        "launchctl bootout",
    )
}

/// Restarts the daemon.
pub fn restart() -> Result<()> {
    if !loaded() {
        return start();
    }
    check(
        launchctl(&["kickstart", "-k", &format!("{}/{LABEL}", domain()?)])?,
        "launchctl kickstart",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_runs_the_daemon_in_the_graphical_session() {
        let text = plist(Path::new("/opt/w&y/wally"), Path::new("/tmp/wally.log"));
        assert!(text.contains("<string>/opt/w&amp;y/wally</string>"));
        assert!(text.contains("<string>daemon</string>\n\t\t<string>run</string>"));
        assert!(text.contains("<string>Aqua</string>"));
        assert!(text.contains(&format!("<string>{LABEL}</string>")));
    }
}

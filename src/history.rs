//! The history, `~/.local/state/wally/history.jsonl`: one line per image put
//! on a screen by a change or by `wally set`, the last [`KEEP`] kept.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::engine::Change;

/// How many entries the history keeps.
pub const KEEP: usize = 500;

/// One image put on one screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// When, RFC 3339 in UTC, to the second.
    pub at: String,
    /// The display id of the screen.
    pub screen_id: u32,
    /// The name of the screen.
    pub screen_name: String,
    /// The image.
    pub image: PathBuf,
}

impl HistoryEntry {
    /// The entry for `change`, made at `at`.
    pub fn new(change: &Change, at: SystemTime) -> Self {
        HistoryEntry {
            at: humantime::format_rfc3339_seconds(at).to_string(),
            screen_id: change.screen.id,
            screen_name: change.screen.name.clone(),
            image: change.image.clone(),
        }
    }
}

/// The history `text` with `entries` added at the end, only the last `keep`
/// lines kept.
fn appended(text: &str, entries: &[HistoryEntry], keep: usize) -> Result<String> {
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect();
    for entry in entries {
        lines.push(serde_json::to_string(entry)?);
    }
    let skip = lines.len().saturating_sub(keep);
    Ok(lines[skip..].iter().map(|l| format!("{l}\n")).collect())
}

/// The last `limit` entries of the history `text`, newest first; lines that
/// do not read as an entry are skipped.
fn newest(text: &str, limit: usize) -> Vec<HistoryEntry> {
    text.lines()
        .rev()
        .filter_map(|l| serde_json::from_str(l).ok())
        .take(limit)
        .collect()
}

/// Adds the entries for `changes`, made now, to the history at `path`.
pub fn record(path: &Path, changes: &[Change]) -> Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    let now = SystemTime::now();
    let entries: Vec<HistoryEntry> = changes.iter().map(|c| HistoryEntry::new(c, now)).collect();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let text = fs::read_to_string(path).unwrap_or_default();
    let partial = path.with_extension("jsonl.partial");
    fs::write(&partial, appended(&text, &entries, KEEP)?)
        .with_context(|| format!("cannot write {}", partial.display()))?;
    fs::rename(&partial, path).with_context(|| format!("cannot write {}", path.display()))
}

/// The last `limit` entries of the history at `path`, newest first.
pub fn read(path: &Path, limit: usize) -> Vec<HistoryEntry> {
    newest(&fs::read_to_string(path).unwrap_or_default(), limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::Screen;

    fn change(id: u32, image: &str) -> Change {
        Change {
            screen: Screen {
                id,
                name: format!("Screen {id}"),
                x: 0.0,
                wallpaper: None,
            },
            image: PathBuf::from(image),
        }
    }

    fn entry(n: usize) -> HistoryEntry {
        HistoryEntry {
            at: "2026-11-15T10:00:00Z".into(),
            screen_id: 1,
            screen_name: "Screen 1".into(),
            image: PathBuf::from(format!("/w/{n}.jpg")),
        }
    }

    #[test]
    fn an_entry_reads_as_the_protocol_says() {
        let at = humantime::parse_rfc3339("2026-11-15T10:00:00Z").unwrap();
        let entry = HistoryEntry::new(&change(7, "/w/a.jpg"), at);
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            serde_json::json!({"at": "2026-11-15T10:00:00Z", "screen_id": 7,
                "screen_name": "Screen 7", "image": "/w/a.jpg"})
        );
    }

    #[test]
    fn only_the_last_lines_are_kept() {
        let first: Vec<HistoryEntry> = (0..4).map(entry).collect();
        let text = appended("", &first, 3).unwrap();
        assert_eq!(text.lines().count(), 3);
        let text = appended(&text, &[entry(9)], 3).unwrap();
        let kept: Vec<PathBuf> = newest(&text, 10).into_iter().map(|e| e.image).collect();
        assert_eq!(
            kept,
            vec![
                PathBuf::from("/w/9.jpg"),
                PathBuf::from("/w/3.jpg"),
                PathBuf::from("/w/2.jpg")
            ]
        );
    }

    #[test]
    fn the_newest_come_first_and_broken_lines_are_skipped() {
        let text = appended("not json\n", &[entry(1), entry(2), entry(3)], 10).unwrap();
        let newest = newest(&text, 2);
        assert_eq!(newest, vec![entry(3), entry(2)]);
    }

    #[test]
    fn changes_are_recorded_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wally/history.jsonl");
        record(&path, &[change(1, "/w/a.jpg"), change(2, "/w/b.jpg")]).unwrap();
        record(&path, &[]).unwrap();
        let read = read(&path, 50);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].screen_id, 2);
        assert!(super::read(&dir.path().join("none"), 5).is_empty());
    }
}

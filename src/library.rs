//! The images of the sources: walking the folders, and searching them by name.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::RegexBuilder;

/// The extensions macOS can show as a wallpaper, lowercase.
pub const EXTENSIONS: [&str; 11] = [
    "jpg", "jpeg", "png", "heic", "heif", "tif", "tiff", "webp", "avif", "gif", "bmp",
];

/// Whether `path` names an image, by its extension.
pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Every image of `sources`, sorted by path, each once. Hidden files are
/// skipped; subfolders are walked when `recursive` is set. A source that
/// cannot be read is skipped.
pub fn scan(sources: &[PathBuf], recursive: bool) -> Vec<PathBuf> {
    let mut images = BTreeSet::new();
    for source in sources {
        walk(source, recursive, &mut images);
    }
    images.into_iter().collect()
}

/// Adds the images of `dir` to `images`.
fn walk(dir: &Path, recursive: bool, images: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() && recursive => walk(&path, recursive, images),
            Ok(t) if !t.is_dir() && is_image(&path) => {
                images.insert(path);
            }
            _ => {}
        }
    }
}

/// The images whose file name matches every one of `words`, case ignored.
/// With `regex`, each word is an extended regular expression.
pub fn search(images: &[PathBuf], words: &[String], regex: bool) -> Result<Vec<PathBuf>> {
    let matchers = words
        .iter()
        .map(|w| {
            let pattern = if regex { w.clone() } else { regex::escape(w) };
            RegexBuilder::new(&pattern)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("`{w}` is not a regular expression"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(images
        .iter()
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            matchers.iter().all(|m| m.is_match(&name))
        })
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn only_visible_images_are_found_and_subfolders_on_demand() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("b.JPG"));
        touch(&root.join("a.webp"));
        touch(&root.join("notes.txt"));
        touch(&root.join(".hidden.png"));
        touch(&root.join("deep/c.png"));
        let flat = scan(&[root.to_path_buf()], false);
        assert_eq!(flat, vec![root.join("a.webp"), root.join("b.JPG")]);
        let deep = scan(&[root.to_path_buf(), root.join("deep")], true);
        assert_eq!(deep.len(), 3);
    }

    #[test]
    fn a_missing_source_is_skipped() {
        assert!(scan(&[PathBuf::from("/nowhere/at/all")], false).is_empty());
    }

    #[test]
    fn every_word_has_to_match_in_any_order_and_case() {
        let images: Vec<PathBuf> = [
            "/w/Dark-Forest.jpg",
            "/w/forest-light.png",
            "/w/dark/sea.png",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let words = vec!["forest".to_string(), "DARK".to_string()];
        assert_eq!(
            search(&images, &words, false).unwrap(),
            vec![images[0].clone()]
        );
        let regex = vec![r"^forest.*\.png$".to_string()];
        assert_eq!(
            search(&images, &regex, true).unwrap(),
            vec![images[1].clone()]
        );
        assert_eq!(search(&images, &[], false).unwrap().len(), 3);
        assert!(search(&images, &["(".to_string()], true).is_err());
    }
}

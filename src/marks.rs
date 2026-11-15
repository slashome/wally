//! The marks, `~/.config/wally/marks.yaml`: the favorite images, drawn more
//! often, and the banned ones, never drawn again.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The first lines of a marks file wally writes.
const HEADER: &str = "# wally marks — see `wally help favorite` and `wally help ban`.\n";

/// How an image can be marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A favorite: drawn three times as often in random order.
    Favorite,
    /// Banned: never drawn, never listed.
    Ban,
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Mark::Favorite => "favorite",
            Mark::Ban => "ban",
        })
    }
}

/// The marked images, as written in `marks.yaml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marks {
    /// The favorite images, absolute paths.
    pub favorites: Vec<PathBuf>,
    /// The banned images, absolute paths.
    pub banned: Vec<PathBuf>,
}

impl Marks {
    /// Reads and parses the marks at `path`; a missing file has none.
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Marks::default());
        }
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        Marks::parse(&text).with_context(|| format!("invalid marks {}", path.display()))
    }

    /// Parses marks; an empty text has none.
    pub fn parse(text: &str) -> Result<Self> {
        if text.trim().lines().all(|l| l.trim_start().starts_with('#')) {
            return Ok(Marks::default());
        }
        serde_saphyr::from_str(text).map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// The marks at `path`, or none when they cannot be read.
    pub fn load(path: &Path) -> Self {
        Marks::read(path).unwrap_or_default()
    }

    /// Writes the marks at `path`, creating its directory.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let body = serde_saphyr::to_string(self).map_err(|e| anyhow::anyhow!("{e}"))?;
        fs::write(path, format!("{HEADER}{body}"))
            .with_context(|| format!("cannot write {}", path.display()))
    }

    /// Whether `image` is a favorite.
    pub fn is_favorite(&self, image: &Path) -> bool {
        self.favorites.iter().any(|p| p == image)
    }

    /// Whether `image` is banned.
    pub fn is_banned(&self, image: &Path) -> bool {
        self.banned.iter().any(|p| p == image)
    }

    /// Marks `image` as `mark`, or unmarks it when `on` is false. An image is
    /// never a favorite and banned at once: one mark clears the other.
    pub fn set(&mut self, mark: Mark, image: &Path, on: bool) {
        let (list, other) = match mark {
            Mark::Favorite => (&mut self.favorites, &mut self.banned),
            Mark::Ban => (&mut self.banned, &mut self.favorites),
        };
        list.retain(|p| p != image);
        if on {
            other.retain(|p| p != image);
            list.push(image.to_path_buf());
            list.sort();
        }
    }

    /// `images` without the banned ones.
    pub fn keep(&self, images: Vec<PathBuf>) -> Vec<PathBuf> {
        if self.banned.is_empty() {
            return images;
        }
        let banned: HashSet<&PathBuf> = self.banned.iter().collect();
        images.into_iter().filter(|p| !banned.contains(p)).collect()
    }

    /// The favorites, for a quick lookup.
    pub fn favorite_set(&self) -> HashSet<PathBuf> {
        self.favorites.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_empty_file_has_no_marks() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Marks::read(&dir.path().join("none.yaml")).unwrap(),
            Marks::default()
        );
        assert_eq!(Marks::parse("# nothing\n").unwrap(), Marks::default());
        assert!(Marks::parse("favorites: 3\n").is_err());
    }

    #[test]
    fn marks_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wally/marks.yaml");
        let mut marks = Marks::default();
        marks.set(Mark::Favorite, Path::new("/w/b.jpg"), true);
        marks.set(Mark::Favorite, Path::new("/w/a.jpg"), true);
        marks.set(Mark::Ban, Path::new("/w/c.jpg"), true);
        marks.write(&path).unwrap();
        let read = Marks::read(&path).unwrap();
        assert_eq!(read, marks);
        assert_eq!(
            read.favorites,
            vec![PathBuf::from("/w/a.jpg"), PathBuf::from("/w/b.jpg")]
        );
    }

    #[test]
    fn one_mark_clears_the_other_and_off_clears_it() {
        let image = Path::new("/w/a.jpg");
        let mut marks = Marks::default();
        marks.set(Mark::Favorite, image, true);
        marks.set(Mark::Favorite, image, true);
        assert_eq!(marks.favorites.len(), 1);
        marks.set(Mark::Ban, image, true);
        assert!(marks.is_banned(image) && !marks.is_favorite(image));
        marks.set(Mark::Ban, image, false);
        assert_eq!(marks, Marks::default());
    }

    #[test]
    fn banned_images_are_left_out() {
        let mut marks = Marks::default();
        marks.set(Mark::Ban, Path::new("/w/b.jpg"), true);
        let images = vec![PathBuf::from("/w/a.jpg"), PathBuf::from("/w/b.jpg")];
        assert_eq!(marks.keep(images), vec![PathBuf::from("/w/a.jpg")]);
    }
}

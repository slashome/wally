//! The rotation state, `~/.local/state/wally/state.json`, and the drawing of
//! the next images from it.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Order;

/// What the rotation remembers between two changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// The shuffled images still to come, next one last.
    pub deck: Vec<PathBuf>,
    /// The position of the next image, in sequential order.
    pub cursor: usize,
    /// When the images last changed on schedule or by `wally next`.
    pub last_change: Option<SystemTime>,
    /// The image wally put on each screen, by screen id.
    pub shown: BTreeMap<u32, PathBuf>,
}

impl State {
    /// Reads the state at `path`; a missing or unreadable one starts afresh.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the state at `path` in one step, so that a reader never sees
    /// half of it.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let partial = path.with_extension("json.partial");
        fs::write(&partial, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("cannot write {}", partial.display()))?;
        fs::rename(&partial, path).with_context(|| format!("cannot write {}", path.display()))
    }

    /// The next `count` images of `library`, distinct whenever the library is
    /// large enough. `library` is sorted; an empty one gives nothing.
    pub fn draw(
        &mut self,
        library: &[PathBuf],
        count: usize,
        order: Order,
        rng: &mut fastrand::Rng,
    ) -> Vec<PathBuf> {
        if library.is_empty() {
            return Vec::new();
        }
        match order {
            Order::Sequential => (0..count)
                .map(|_| {
                    let image = library[self.cursor % library.len()].clone();
                    self.cursor = (self.cursor + 1) % library.len();
                    image
                })
                .collect(),
            Order::Random => {
                let known: HashSet<&PathBuf> = library.iter().collect();
                self.deck.retain(|p| known.contains(p));
                let mut drawn: Vec<PathBuf> = Vec::with_capacity(count);
                while drawn.len() < count {
                    if self.deck.is_empty() {
                        self.deck = library.to_vec();
                        rng.shuffle(&mut self.deck);
                    }
                    let image = self.deck.pop().expect("the deck was just filled");
                    if drawn.contains(&image) && library.len() > drawn.len() {
                        continue;
                    }
                    drawn.push(image);
                }
                drawn
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library(n: usize) -> Vec<PathBuf> {
        (0..n)
            .map(|i| PathBuf::from(format!("/w/{i:03}.jpg")))
            .collect()
    }

    #[test]
    fn sequential_order_walks_the_library_and_wraps() {
        let mut state = State::default();
        let lib = library(3);
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(state.draw(&lib, 2, Order::Sequential, &mut rng), lib[0..2]);
        assert_eq!(
            state.draw(&lib, 2, Order::Sequential, &mut rng),
            vec![lib[2].clone(), lib[0].clone()]
        );
    }

    #[test]
    fn random_order_shows_every_image_before_any_twice() {
        let mut state = State::default();
        let lib = library(10);
        let mut rng = fastrand::Rng::with_seed(7);
        let mut seen: Vec<PathBuf> = (0..5)
            .flat_map(|_| state.draw(&lib, 2, Order::Random, &mut rng))
            .collect();
        seen.sort();
        assert_eq!(seen, lib);
    }

    #[test]
    fn one_draw_never_repeats_an_image_when_the_library_allows() {
        let lib = library(3);
        let mut rng = fastrand::Rng::with_seed(3);
        for _ in 0..50 {
            let mut state = State {
                deck: vec![lib[0].clone()],
                ..State::default()
            };
            let mut drawn = state.draw(&lib, 3, Order::Random, &mut rng);
            drawn.sort();
            drawn.dedup();
            assert_eq!(drawn.len(), 3);
        }
    }

    #[test]
    fn a_tiny_library_still_fills_every_screen() {
        let mut state = State::default();
        let lib = library(1);
        let mut rng = fastrand::Rng::with_seed(3);
        assert_eq!(
            state.draw(&lib, 3, Order::Random, &mut rng),
            vec![lib[0].clone(); 3]
        );
        assert!(state.draw(&[], 2, Order::Random, &mut rng).is_empty());
    }

    #[test]
    fn images_gone_from_the_library_leave_the_deck() {
        let lib = library(4);
        let mut state = State {
            deck: vec![PathBuf::from("/gone.jpg")],
            ..State::default()
        };
        let mut rng = fastrand::Rng::with_seed(5);
        let drawn = state.draw(&lib, 1, Order::Random, &mut rng);
        assert!(lib.contains(&drawn[0]));
    }

    #[test]
    fn the_state_survives_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut state = State {
            cursor: 4,
            last_change: Some(SystemTime::now()),
            ..State::default()
        };
        state.shown.insert(2, PathBuf::from("/w/a.jpg"));
        state.save(&path).unwrap();
        assert_eq!(State::load(&path), state);
        assert_eq!(State::load(&dir.path().join("none.json")), State::default());
    }
}

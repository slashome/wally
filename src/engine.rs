//! Changing wallpapers: drawing the next images and putting them on the
//! screens, for `wally next` and the daemon alike.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};

use crate::config::{Config, Spread};
use crate::desktop::{self, Screen};
use crate::library;
use crate::paths;
use crate::rotation::State;

/// How long a change waits for macOS to report the new images.
const SETTLE: Duration = Duration::from_secs(5);

/// How long after a change the Spaces are left alone, while macOS still
/// reports its default picture on the screens just changed.
const QUIET: Duration = Duration::from_secs(10);

/// How often a change asks macOS whether the new images are in place.
const SETTLE_POLL: Duration = Duration::from_millis(100);

/// Which screens a change touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Targets {
    /// Every screen.
    All,
    /// The screens with these display ids.
    Only(Vec<u32>),
    /// The screens wally has not dressed yet, such as one just plugged in.
    Undressed,
}

/// One screen and the image it just received.
#[derive(Clone, Debug)]
pub struct Change {
    /// The screen.
    pub screen: Screen,
    /// The image now on it.
    pub image: PathBuf,
}

/// The screens of `screens` that `targets` names.
fn chosen<'a>(screens: &'a [Screen], targets: &Targets, state: &State) -> Vec<&'a Screen> {
    screens
        .iter()
        .filter(|s| match targets {
            Targets::All => true,
            Targets::Only(ids) => ids.contains(&s.id),
            Targets::Undressed => !state.shown.contains_key(&s.id),
        })
        .collect()
}

/// Draws the next images and puts them on the `targets` screens. A change of
/// [`Targets::Undressed`] screens leaves the schedule as it is; any other
/// restarts the interval.
pub fn change(config: &Config, targets: &Targets) -> Result<Vec<Change>> {
    let state_path = paths::state_file();
    let mut state = State::load(&state_path);
    let screens = desktop::screens()?;
    let chosen = chosen(&screens, targets, &state);
    if chosen.is_empty() {
        return Ok(Vec::new());
    }
    let images = library::scan(&config.source_paths(), config.recursive);
    if images.is_empty() {
        bail!(
            "no image in {}: add a folder with `wally settings source add <folder>`",
            config.sources.join(", ")
        );
    }
    let reused = match (config.screens, targets) {
        (Spread::Same, Targets::Undressed) => state.shown.values().next().cloned(),
        _ => None,
    };
    let count = match config.screens {
        Spread::Distinct => chosen.len(),
        Spread::Same => 1,
    };
    let mut drawn = match reused {
        Some(image) => vec![image],
        None => state.draw(&images, count, config.order, &mut fastrand::Rng::new()),
    };
    if config.screens == Spread::Same {
        drawn = vec![drawn[0].clone(); chosen.len()];
    }
    let mut changes = Vec::with_capacity(chosen.len());
    for (screen, image) in chosen.into_iter().zip(drawn) {
        desktop::set_wallpaper(screen.id, &image)?;
        state.shown.insert(screen.id, image.clone());
        changes.push(Change {
            screen: screen.clone(),
            image,
        });
    }
    let present: Vec<u32> = screens.iter().map(|s| s.id).collect();
    state.shown.retain(|id, _| present.contains(id));
    if *targets != Targets::Undressed {
        state.last_change = Some(SystemTime::now());
    }
    state.save(&state_path)?;
    settle(&changes);
    Ok(changes)
}

/// The screens whose current Space shows something other than the image
/// wally put on them, with that image, as long as it still exists.
fn strays(screens: &[Screen], state: &State) -> Vec<(Screen, PathBuf)> {
    screens
        .iter()
        .filter_map(|screen| {
            let image = state.shown.get(&screen.id)?;
            let stray = screen.wallpaper.as_ref() != Some(image) && image.is_file();
            stray.then(|| (screen.clone(), image.clone()))
        })
        .collect()
}

/// Puts back the image wally chose on every screen whose current Space shows
/// another one. macOS only sets the wallpaper of the current Space, so this is
/// how every Space — the desktop left for a fullscreen app included — catches
/// up with the rotation once it is shown.
pub fn follow() -> Result<Vec<Change>> {
    let state_path = paths::state_file();
    let recent = fs::metadata(&state_path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().unwrap_or_default() < QUIET);
    if recent {
        return Ok(Vec::new());
    }
    let state = State::load(&state_path);
    let screens = desktop::screens()?;
    let mut changes = Vec::new();
    for (screen, image) in strays(&screens, &state) {
        desktop::set_wallpaper(screen.id, &image)?;
        changes.push(Change { screen, image });
    }
    settle(&changes);
    Ok(changes)
}

/// Waits until macOS reports the image of every change, for [`SETTLE`] at
/// most: right after a change it still reports its default desktop picture
/// for a moment, and a `wally show` run then would say so.
fn settle(changes: &[Change]) {
    let started = Instant::now();
    while started.elapsed() < SETTLE {
        let Ok(screens) = desktop::screens() else {
            return;
        };
        let settled = changes.iter().all(|c| {
            screens
                .iter()
                .find(|s| s.id == c.screen.id)
                .is_none_or(|s| s.wallpaper.as_ref() == Some(&c.image))
        });
        if settled {
            return;
        }
        desktop::pump(SETTLE_POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn screen(id: u32, wallpaper: Option<&Path>) -> Screen {
        Screen {
            id,
            name: format!("Screen {id}"),
            x: id as f64,
            wallpaper: wallpaper.map(Path::to_path_buf),
        }
    }

    #[test]
    fn only_screens_showing_another_image_stray() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = dir.path().join("chosen.jpg");
        std::fs::write(&chosen, b"").unwrap();
        let other = dir.path().join("other.jpg");
        let mut state = State::default();
        state.shown.insert(1, chosen.clone());
        state.shown.insert(2, chosen.clone());
        state.shown.insert(3, dir.path().join("gone.jpg"));
        let screens = vec![
            screen(1, Some(&chosen)),
            screen(2, Some(&other)),
            screen(3, Some(&other)),
            screen(4, Some(&other)),
        ];
        let strays = strays(&screens, &state);
        assert_eq!(strays.len(), 1);
        assert_eq!((strays[0].0.id, &strays[0].1), (2, &chosen));
    }
}

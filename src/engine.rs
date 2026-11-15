//! Changing wallpapers: drawing the next images and putting them on the
//! screens, for `wally next` and the daemon alike.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};

use crate::config::{Config, Spread};
use crate::desktop::{self, Screen};
use crate::history;
use crate::library;
use crate::marks::{Mark, Marks};
use crate::paths;
use crate::rotation::State;

/// How long after a change macOS may still report its default picture on the
/// screens just changed, while WallpaperAgent decodes and fades the new image
/// in: within it, wally trusts the image it set over what macOS reports, and
/// leaves the Spaces alone.
const QUIET: Duration = Duration::from_secs(10);

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
    let marks = Marks::load(&paths::marks_file());
    let images = marks.keep(library::scan(&config.source_paths(), config.recursive));
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
        None => state.draw(
            &images,
            &marks.favorite_set(),
            count,
            config.order,
            &mut fastrand::Rng::new(),
        ),
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
    history::record(&paths::history_file(), &changes)?;
    Ok(changes)
}

/// Puts `image` on the screen with display id `screen`, and restarts the
/// interval as a change does.
pub fn set(screen: u32, image: &Path) -> Result<Change> {
    if !image.is_absolute() {
        bail!("{} is not an absolute path", image.display());
    }
    if !image.is_file() || !library::is_image(image) {
        bail!("{} is not an image", image.display());
    }
    let screen = self::screen(screen)?;
    desktop::set_wallpaper(screen.id, image)?;
    let state_path = paths::state_file();
    let mut state = State::load(&state_path);
    state.shown.insert(screen.id, image.to_path_buf());
    state.last_change = Some(SystemTime::now());
    state.save(&state_path)?;
    let change = Change {
        screen,
        image: image.to_path_buf(),
    };
    history::record(&paths::history_file(), std::slice::from_ref(&change))?;
    Ok(change)
}

/// Pauses the scheduled changes, in the state file.
pub fn pause() -> Result<()> {
    let path = paths::state_file();
    let mut state = State::load(&path);
    state.pause();
    state.save(&path)
}

/// Resumes the scheduled changes, the interval counted from now.
pub fn resume() -> Result<()> {
    let path = paths::state_file();
    let mut state = State::load(&path);
    state.resume(SystemTime::now());
    state.save(&path)
}

/// The images of the sources, banned ones left out; `deep` walks the
/// subfolders even when the configuration does not.
pub fn library(config: &Config, deep: bool) -> Vec<PathBuf> {
    let marks = Marks::load(&paths::marks_file());
    marks.keep(library::scan(
        &config.source_paths(),
        config.recursive || deep,
    ))
}

/// The screen with display id `id`, among the connected ones.
pub fn screen(id: u32) -> Result<Screen> {
    desktop::screens()?
        .into_iter()
        .find(|s| s.id == id)
        .with_context(|| format!("no screen with id {id}"))
}

/// The image a mark applies to: `image` when given, otherwise the image on
/// the screen with display id `screen`, or on the first screen.
pub fn marked_image(image: Option<&Path>, screen: Option<u32>) -> Result<PathBuf> {
    if let Some(image) = image {
        if !image.is_absolute() {
            bail!("{} is not an absolute path", image.display());
        }
        return Ok(image.to_path_buf());
    }
    let shown = match screen {
        Some(id) => self::screen(id)?,
        None => desktop::screens()?
            .into_iter()
            .next()
            .context("no screen is connected")?,
    };
    match shown.wallpaper {
        Some(path) if path.is_file() => Ok(path),
        _ => bail!("{} shows no image wally can mark", shown.name),
    }
}

/// Marks `image` as `mark`, or unmarks it when `on` is false, in the marks
/// file. Banning an image some screens show replaces it there at once; the
/// changes made are returned.
pub fn mark(config: &Config, mark: Mark, image: &Path, on: bool) -> Result<Vec<Change>> {
    let path = paths::marks_file();
    let mut marks = Marks::read(&path)?;
    if on && mark == Mark::Favorite && !image.is_file() {
        bail!("{} is not an image", image.display());
    }
    marks.set(mark, image, on);
    marks.write(&path)?;
    if mark != Mark::Ban || !on {
        return Ok(Vec::new());
    }
    let state = State::load(&paths::state_file());
    let showing: Vec<u32> = desktop::screens()?
        .iter()
        .filter(|s| {
            s.wallpaper.as_deref() == Some(image)
                || state.shown.get(&s.id).map(PathBuf::as_path) == Some(image)
        })
        .map(|s| s.id)
        .collect();
    if showing.is_empty() {
        return Ok(Vec::new());
    }
    change(config, &Targets::Only(showing))
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
    if changed_lately() {
        return Ok(Vec::new());
    }
    let state = State::load(&paths::state_file());
    let screens = desktop::screens()?;
    let mut changes = Vec::new();
    for (screen, image) in strays(&screens, &state) {
        desktop::set_wallpaper(screen.id, &image)?;
        changes.push(Change { screen, image });
    }
    Ok(changes)
}

/// Whether the state file was written within [`QUIET`], so a change may still
/// be on its way to the screen.
fn changed_lately() -> bool {
    fs::metadata(paths::state_file())
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().unwrap_or_default() < QUIET)
}

/// `screens` with the image wally just set in place of what macOS reports,
/// when a change happened lately and macOS does not report it yet.
fn with_pending(mut screens: Vec<Screen>, state: &State, lately: bool) -> Vec<Screen> {
    if lately {
        for screen in &mut screens {
            if let Some(image) = state.shown.get(&screen.id) {
                screen.wallpaper = Some(image.clone());
            }
        }
    }
    screens
}

/// The screens and what they show: what macOS reports, except right after a
/// change, where the image wally just set is reported without waiting for
/// macOS to catch up.
pub fn screens() -> Result<Vec<Screen>> {
    let state = State::load(&paths::state_file());
    Ok(with_pending(desktop::screens()?, &state, changed_lately()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_recent_change_is_reported_before_macos_catches_up() {
        let chosen = PathBuf::from("/w/chosen.jpg");
        let default = PathBuf::from("/System/Library/CoreServices/DefaultDesktop.heic");
        let mut state = State::default();
        state.shown.insert(1, chosen.clone());
        let reported = vec![screen(1, Some(&default)), screen(2, Some(&default))];
        let lately = with_pending(reported.clone(), &state, true);
        assert_eq!(lately[0].wallpaper, Some(chosen));
        assert_eq!(lately[1].wallpaper, Some(default.clone()));
        let later = with_pending(reported, &state, false);
        assert_eq!(later[0].wallpaper, Some(default));
    }
}

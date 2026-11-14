//! Changing wallpapers: drawing the next images and putting them on the
//! screens, for `wally next` and the daemon alike.

use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{Result, bail};

use crate::config::{Config, Spread};
use crate::desktop::{self, Screen};
use crate::library;
use crate::paths;
use crate::rotation::State;

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
    Ok(changes)
}

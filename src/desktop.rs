//! The screens and their wallpapers, through AppKit: `NSScreen` lists them,
//! `NSWorkspace` reads and sets the image of each.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

/// A screen, as AppKit sees it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Screen {
    /// The display id, stable while the screen stays connected.
    pub id: u32,
    /// The name macOS gives the screen, such as `DELL U2723QE`.
    pub name: String,
    /// The left edge in the global screen space, to order screens left to
    /// right.
    pub x: f64,
    /// The image the screen shows, if macOS tells.
    pub wallpaper: Option<PathBuf>,
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;

    use anyhow::{Context, bail};
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSScreen, NSWorkspace};
    use objc2_foundation::{NSDate, NSDictionary, NSNumber, NSRunLoop, NSString, NSURL};

    /// The main-thread token AppKit asks for.
    fn main_thread() -> Result<MainThreadMarker> {
        MainThreadMarker::new().context("wally has to run on the main thread")
    }

    /// The display id of `screen`, from its `NSScreenNumber`.
    fn display_id(screen: &NSScreen) -> Option<u32> {
        let description = screen.deviceDescription();
        let number = description.objectForKey(&NSString::from_str("NSScreenNumber"))?;
        let number = number.downcast::<NSNumber>().ok()?;
        Some(number.unsignedIntValue())
    }

    /// Every connected screen, left to right.
    pub fn screens() -> Result<Vec<Screen>> {
        let mtm = main_thread()?;
        let workspace = NSWorkspace::sharedWorkspace();
        let mut screens: Vec<Screen> = NSScreen::screens(mtm)
            .iter()
            .filter_map(|screen| {
                let id = display_id(&screen)?;
                let wallpaper = workspace
                    .desktopImageURLForScreen(&screen)
                    .and_then(|url| url.path())
                    .map(|path| PathBuf::from(path.to_string()));
                Some(Screen {
                    id,
                    name: screen.localizedName().to_string(),
                    x: screen.frame().origin.x,
                    wallpaper,
                })
            })
            .collect();
        screens.sort_by(|a, b| a.x.total_cmp(&b.x));
        Ok(screens)
    }

    /// The `NSScreen` with display id `id`.
    fn find(mtm: MainThreadMarker, id: u32) -> Result<Retained<NSScreen>> {
        NSScreen::screens(mtm)
            .iter()
            .find(|screen| display_id(screen) == Some(id))
            .context("this screen is no longer connected")
    }

    /// Puts `image` on the screen with display id `id`.
    pub fn set_wallpaper(id: u32, image: &Path) -> Result<()> {
        let mtm = main_thread()?;
        let screen = find(mtm, id)?;
        let Some(text) = image.to_str() else {
            bail!("{} is not a UTF-8 path", image.display());
        };
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        let options = NSDictionary::new();
        let workspace = NSWorkspace::sharedWorkspace();
        unsafe { workspace.setDesktopImageURL_forScreen_options_error(&url, &screen, &options) }
            .map_err(|e| anyhow::anyhow!("{}", e.localizedDescription()))
            .with_context(|| format!("cannot set {} as wallpaper", image.display()))
    }

    /// Lets AppKit process its pending events for `duration`, so that the
    /// list of screens follows the ones plugged or unplugged meanwhile.
    pub fn pump(duration: Duration) {
        let until = NSDate::dateWithTimeIntervalSinceNow(duration.as_secs_f64());
        NSRunLoop::currentRunLoop().runUntilDate(&until);
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;

    use anyhow::bail;

    /// Every connected screen: macOS only for now.
    pub fn screens() -> Result<Vec<Screen>> {
        bail!("wally only drives wallpapers on macOS for now")
    }

    /// Puts an image on a screen: macOS only for now.
    pub fn set_wallpaper(_id: u32, _image: &Path) -> Result<()> {
        bail!("wally only drives wallpapers on macOS for now")
    }

    /// Waits for `duration`.
    pub fn pump(duration: Duration) {
        std::thread::sleep(duration);
    }
}

pub use native::{pump, screens, set_wallpaper};

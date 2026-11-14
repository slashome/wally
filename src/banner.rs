//! The WALLY banner: the name in block letters and a small trash-compacting
//! robot beside it, painted in the yellow of its body and rusting towards the
//! bottom.

use std::env;
use std::io::{IsTerminal, Write, stderr};

/// The name, six rows of block letters.
const LETTERS: [&str; 6] = [
    "██╗    ██╗ █████╗ ██╗     ██╗     ██╗   ██╗",
    "██║    ██║██╔══██╗██║     ██║     ╚██╗ ██╔╝",
    "██║ █╗ ██║███████║██║     ██║      ╚████╔╝ ",
    "██║███╗██║██╔══██║██║     ██║       ╚██╔╝  ",
    "╚███╔███╔╝██║  ██║███████╗███████╗   ██║   ",
    " ╚══╝╚══╝ ╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ",
];

/// The robot: binocular eyes, a box body with its solar gauge, treads.
const ROBOT: [&str; 9] = [
    "   .---.   .---.  ",
    "  ( (o) )=( (o) ) ",
    "   '---' | '---'  ",
    "     ____|____    ",
    "  __|  _____  |__ ",
    " |__| |▮▮▮▯▯| |__|",
    "    |_________|   ",
    "   _/=========\\_  ",
    "  (_o_O_o_O_o_O_) ",
];

/// The robot row the letters start on, so that they sit on its treads.
const LETTERS_TOP: usize = 2;

/// Columns between the letters and the robot.
const GUTTER: usize = 4;

/// The yellow of the robot's body, at the top of the banner.
const BODY: Rgb = Rgb(242, 179, 61);

/// The rust the body fades into at the bottom of the banner.
const RUST: Rgb = Rgb(168, 84, 32);

/// The darker rust of the stains.
const STAIN: Rgb = Rgb(110, 46, 18);

/// The grey of the robot's eyes.
const EYES: Rgb = Rgb(170, 170, 160);

/// A colour, red, green and blue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// How the terminal is able to draw colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    /// No colour at all: `NO_COLOR` is set.
    Plain,
    /// The 256 colours of xterm.
    Indexed,
    /// 24-bit colour.
    TrueColor,
}

impl Palette {
    /// The palette the environment allows: `NO_COLOR` turns colour off,
    /// `COLORTERM` announces 24-bit colour.
    pub fn from_env() -> Self {
        if env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Palette::Plain;
        }
        match env::var("COLORTERM").as_deref() {
            Ok("truecolor") | Ok("24bit") => Palette::TrueColor,
            _ => Palette::Indexed,
        }
    }

    /// The escape sequence that sets the foreground to `rgb`.
    fn paint(self, rgb: Rgb) -> String {
        match self {
            Palette::Plain => String::new(),
            Palette::TrueColor => format!("\x1b[38;2;{};{};{}m", rgb.0, rgb.1, rgb.2),
            Palette::Indexed => format!("\x1b[38;5;{}m", xterm_index(rgb)),
        }
    }

    /// The escape sequence that restores the default colour.
    fn reset(self) -> &'static str {
        match self {
            Palette::Plain => "",
            _ => "\x1b[0m",
        }
    }
}

/// The nearest colour of the xterm 6×6×6 cube.
fn xterm_index(rgb: Rgb) -> u8 {
    let level = |c: u8| ((c as u16 * 5 + 127) / 255) as u8;
    16 + 36 * level(rgb.0) + 6 * level(rgb.1) + level(rgb.2)
}

/// The colour `t` of the way from `from` to `to`, `t` between 0 and 1.
fn blend(from: Rgb, to: Rgb, t: f32) -> Rgb {
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Rgb(mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}

/// The colour of the body on `row`: plain yellow on the upper part, then
/// fading into rust.
fn body_colour(row: usize) -> Rgb {
    let rows = ROBOT.len() as f32;
    let t = ((row as f32 - rows * 0.4) / (rows * 0.6 - 1.0)).clamp(0.0, 1.0);
    blend(BODY, RUST, t)
}

/// Whether the cell at `row`, `column` carries a rust stain: none on the upper
/// half, more and more towards the bottom. Deterministic, so the banner is the
/// same on every run.
fn stained(row: usize, column: usize) -> bool {
    let rows = ROBOT.len();
    if row < rows / 2 {
        return false;
    }
    let hash = (row as u32).wrapping_mul(73_856_093) ^ (column as u32).wrapping_mul(19_349_663);
    let density = (row + 1 - rows / 2) as u32 * 7;
    hash % 100 < density
}

/// The colour of one visible character of the banner.
fn colour_of(row: usize, column: usize, in_robot: bool) -> Rgb {
    if in_robot && row < 2 {
        return EYES;
    }
    if stained(row, column) {
        return STAIN;
    }
    body_colour(row)
}

/// The banner as a grid of characters, each visible one with its colour.
pub fn cells() -> Vec<Vec<(char, Option<Rgb>)>> {
    let letters_width = LETTERS[0].chars().count();
    ROBOT
        .iter()
        .enumerate()
        .map(|(row, robot)| {
            let letters = row
                .checked_sub(LETTERS_TOP)
                .and_then(|i| LETTERS.get(i))
                .copied()
                .unwrap_or("");
            let left = format!("{letters:<letters_width$}{:GUTTER$}", "");
            left.chars()
                .map(|c| (c, false))
                .chain(robot.trim_end().chars().map(|c| (c, true)))
                .enumerate()
                .map(|(column, (c, in_robot))| {
                    let colour = (!c.is_whitespace()).then(|| colour_of(row, column, in_robot));
                    (c, colour)
                })
                .collect()
        })
        .collect()
}

/// The banner, one string with its line breaks, painted with `palette`.
pub fn render(palette: Palette) -> String {
    let mut out = String::from("\n");
    for row in cells() {
        let mut current = None;
        for (c, colour) in row {
            if let Some(colour) = colour
                && current != Some(colour)
            {
                out.push_str(&palette.paint(colour));
                current = Some(colour);
            }
            out.push(c);
        }
        out.push_str(palette.reset());
        out.push('\n');
    }
    out.push('\n');
    out
}

/// The banner as an SVG image on a dark background, for the README.
pub fn svg() -> String {
    let rows = cells();
    let (cell_w, cell_h, pad) = (9.6_f32, 19.0_f32, 24.0_f32);
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0) as f32;
    let width = columns * cell_w + 2.0 * pad;
    let height = rows.len() as f32 * cell_h + 2.0 * pad;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"#1d1b19\"/>\n\
         <g font-family=\"Menlo, Consolas, monospace\" font-size=\"16\" xml:space=\"preserve\">\n"
    );
    for (row, line) in rows.iter().enumerate() {
        let y = pad + (row as f32 + 0.8) * cell_h;
        for (column, (c, colour)) in line.iter().enumerate() {
            let Some(Rgb(r, g, b)) = colour else {
                continue;
            };
            let x = pad + column as f32 * cell_w;
            let glyph = match c {
                '<' => "&lt;".to_string(),
                '>' => "&gt;".to_string(),
                '&' => "&amp;".to_string(),
                c => c.to_string(),
            };
            out.push_str(&format!(
                "<text x=\"{x:.1}\" y=\"{y:.1}\" fill=\"#{r:02x}{g:02x}{b:02x}\">{glyph}</text>\n"
            ));
        }
    }
    out.push_str("</g>\n</svg>\n");
    out
}

/// Prints the banner on standard error, only when it is a terminal and
/// `WALLY_NO_BANNER` is unset: piped output never carries the drawing.
pub fn print() {
    if env::var_os("WALLY_NO_BANNER").is_some_and(|v| !v.is_empty()) {
        return;
    }
    let mut err = stderr();
    if !err.is_terminal() {
        return;
    }
    let _ = err.write_all(render(Palette::from_env()).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_letter_row_has_the_same_width() {
        let width = LETTERS[0].chars().count();
        assert!(LETTERS.iter().all(|l| l.chars().count() == width));
    }

    #[test]
    fn every_robot_row_has_the_same_width() {
        let width = ROBOT[0].chars().count();
        assert!(ROBOT.iter().all(|l| l.chars().count() == width));
    }

    #[test]
    fn the_letters_fit_beside_the_robot() {
        assert!(LETTERS_TOP + LETTERS.len() <= ROBOT.len());
    }

    #[test]
    fn plain_rendering_carries_no_escape() {
        assert!(!render(Palette::Plain).contains('\x1b'));
    }

    #[test]
    fn the_top_is_yellow_and_the_bottom_rusty() {
        assert_eq!(body_colour(0), BODY);
        assert_eq!(body_colour(ROBOT.len() - 1), RUST);
    }

    #[test]
    fn stains_only_appear_on_the_lower_half() {
        assert!((0..ROBOT.len() / 2).all(|row| (0..80).all(|col| !stained(row, col))));
        assert!((0..80).any(|col| stained(ROBOT.len() - 1, col)));
    }
}

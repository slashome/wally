//! Writes the banner as an SVG image on standard output, for the README:
//! `cargo run --example banner_svg > assets/banner.svg`.

fn main() {
    print!("{}", wally::banner::svg());
}

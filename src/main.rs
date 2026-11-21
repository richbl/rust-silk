//! Main application entry point (get it, main?)

mod app;
mod preferences;
mod render;
mod settings;

use relm4::RelmApp;

/// Application constants
pub const APP_ID: &str = "io.github.richbl.Silk";
pub const APP_VER: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub const DEFAULT_WIDTH: i32 = 1000;
pub const DEFAULT_HEIGHT: i32 = 700;

fn main() {
    let app = RelmApp::new(APP_ID);
    app.run::<app::App>(());
}

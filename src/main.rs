//! Main application entry point (get it, main?)

mod app;
mod constants;
mod preferences;
mod render;
mod settings;

use crate::constants::APP_ID;
use relm4::RelmApp;

fn main() {
    let app = RelmApp::new(APP_ID);
    app.run::<app::App>(());
}

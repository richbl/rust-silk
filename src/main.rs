//! Main application entry point (get it, main?)

mod app;
mod constants;
mod preferences;
mod render;
mod settings;

use crate::constants::APP_ID;
use relm4::RelmApp;

fn main() {
    relm4::gtk::gio::resources_register_include!("silk.gresource")
        .expect("unable to register application resources");

    let app = RelmApp::new(APP_ID);
    app.run::<app::App>(());
}

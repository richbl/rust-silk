//! Main application entry point (get it, main?)

mod app;
mod constants;
mod installer;
mod preferences;
mod render;
mod settings;

use crate::constants::APP_ID;
use crate::installer::CommandLineAction;
use relm4::RelmApp;
use std::process::ExitCode;

fn run_gui() {
    relm4::gtk::gio::resources_register_include!("silk.gresource")
        .expect("unable to register application resources");

    let app = RelmApp::new(APP_ID);
    app.run::<app::App>(());
}

fn main() -> ExitCode {
    match installer::parse_command_line(std::env::args_os().skip(1)) {
        Ok(CommandLineAction::Launch) => run_gui(),
        Ok(CommandLineAction::Help) => installer::print_help(),
        Ok(CommandLineAction::Install) => {
            if let Err(error) = installer::install() {
                eprintln!("installation failed: {error:#}");
                return ExitCode::FAILURE;
            }
        }
        Ok(CommandLineAction::Uninstall) => {
            if let Err(error) = installer::uninstall() {
                eprintln!("uninstallation failed: {error:#}");
                return ExitCode::FAILURE;
            }
        }
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

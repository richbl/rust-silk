//! Appearance: settings changes, editor CSS and color scheme

use relm4::adw;
use relm4::gtk::{self, gdk};
use relm4::prelude::*;

use super::{App, Msg};
use crate::preferences::PrefsMsg;
use crate::settings::{Settings, parse_color, readable_foreground};

impl App {
    // Apply the editor CSS styling based on current settings
    pub(super) fn apply_editor_css(&self) {
        let css = self
            .settings
            .editor_background
            .as_deref()
            .and_then(|c| parse_color(c).map(|rgba| (c, rgba)))
            .map(|(bg, rgba)| {
                format!(
                    "textview.silk-editor, textview.silk-editor text \
                     {{ background-color: {bg}; color: {}; }}",
                    readable_foreground(&rgba)
                )
            })
            .unwrap_or_default();
        self.editor_css.load_from_string(&css);
    }

    // Handle settings change event or update window size/state depending on the application context
    fn apply_settings(&mut self, new: Settings, from_prefs: bool) {
        if new == self.settings {
            return;
        }
        let old = std::mem::replace(&mut self.settings, new);

        if let Err(e) = self.settings.save() {
            eprintln!("silk: could not save settings: {e}");
        }
        if old.color_scheme != self.settings.color_scheme {
            adw::StyleManager::default().set_color_scheme(self.settings.color_scheme.to_adw());
        }
        if old.editor_background != self.settings.editor_background {
            self.apply_editor_css();
        }
        if old.color_scheme != self.settings.color_scheme
            || old.preview_background != self.settings.preview_background
        {
            self.reload_shell();
        }
        if !from_prefs {
            self.preferences.emit(PrefsMsg::Sync(self.settings.clone()));
        }
    }

    // Handle the settings-related messages (view mode, color scheme, preferences dialog)
    pub(super) fn update_settings(&mut self, msg: Msg) {
        match msg {
            Msg::SetViewMode(view_mode) => {
                let mut next = self.settings.clone();
                next.view_mode = view_mode;
                self.apply_settings(next, false);
            }
            Msg::SetColorScheme(color_scheme) => {
                let mut next = self.settings.clone();
                next.color_scheme = color_scheme;
                self.apply_settings(next, false);
            }
            Msg::SettingsChanged(settings) => {
                self.apply_settings(settings, true);
            }
            _ => unreachable!("settings handler called with unexpected message"),
        }
    }
}

// Create the CSS provider for the editor styling and inject it into the default display
pub(super) fn build_editor_css() -> gtk::CssProvider {
    let editor_css = gtk::CssProvider::new();

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &editor_css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    editor_css
}

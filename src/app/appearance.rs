//! Appearance: settings changes, editor CSS and color scheme

use relm4::adw;
use relm4::gtk::{self, gdk};
use relm4::prelude::*;

use super::{App, Msg};
use crate::preferences::PrefsMsg;
use crate::settings::{Settings, parse_color, readable_foreground, scheme_palette};

// Which side effects a settings change requires
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Effects {
    /// Push the color scheme to libadwaita
    color_scheme: bool,
    /// Regenerate the editor style-sheet
    editor_css: bool,
    /// Reload the preview shell page
    reload_shell: bool,
}

impl Effects {
    fn between(old: &Settings, new: &Settings) -> Self {
        let color_scheme = old.color_scheme != new.color_scheme;
        Self {
            color_scheme,
            editor_css: old.editor_background != new.editor_background,
            reload_shell: color_scheme || old.preview_background != new.preview_background,
        }
    }
}

// The editor style-sheet: a custom background wins (with a readable foreground), otherwise set
// the palette for the resolved light/dark style
fn editor_css_text(background: Option<&str>, dark: bool) -> String {
    let (bg, fg) = match background.and_then(|c| parse_color(c).map(|rgba| (c, rgba))) {
        Some((bg, rgba)) => (bg, readable_foreground(&rgba)),
        None => scheme_palette(dark),
    };

    format!(
        "textview.silk-editor, textview.silk-editor text {{ background-color: {bg}; color: {fg}; }}"
    )
}

impl App {
    // Apply the editor CSS: custom background if set, otherwise the resolved scheme palette
    pub(super) fn apply_editor_css(&self) {
        let css = editor_css_text(
            self.settings.editor_background.as_deref(),
            adw::StyleManager::default().is_dark(),
        );
        self.editor_css.load_from_string(&css);
    }

    // Handle settings change event or update window size/state depending on the application
    // context
    fn apply_settings(&mut self, new: Settings, from_prefs: bool) {
        if new == self.settings {
            return;
        }
        let old = std::mem::replace(&mut self.settings, new);

        if let Err(e) = self.settings.save() {
            eprintln!("silk: could not save settings: {e}");
        }

        let effects = Effects::between(&old, &self.settings);
        if effects.color_scheme {
            adw::StyleManager::default().set_color_scheme(self.settings.color_scheme.to_adw());
        }
        if effects.editor_css {
            self.apply_editor_css();
        }
        if effects.reload_shell {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ColorScheme, ViewMode, WindowSettings};

    // ---------------------------------------------------------------------
    // Effects::between
    // ---------------------------------------------------------------------

    #[test]
    fn identical_settings_need_no_effects() {
        let s = Settings::default();
        assert_eq!(Effects::between(&s, &s), Effects::default());
    }

    #[test]
    fn view_mode_change_needs_no_effects() {
        let old = Settings::default();
        let new = Settings {
            view_mode: ViewMode::EditorOnly,
            ..old.clone()
        };
        assert_eq!(Effects::between(&old, &new), Effects::default());
    }

    #[test]
    fn window_change_needs_no_effects() {
        let old = Settings::default();
        let new = Settings {
            window: WindowSettings {
                width: 640,
                height: 480,
                maximized: true,
                fullscreened: true,
            },
            ..old.clone()
        };
        assert_eq!(Effects::between(&old, &new), Effects::default());
    }

    #[test]
    fn color_scheme_change_updates_scheme_and_reloads_shell_only() {
        let old = Settings::default();
        let new = Settings {
            color_scheme: ColorScheme::Dark,
            ..old.clone()
        };
        assert_eq!(
            Effects::between(&old, &new),
            Effects {
                color_scheme: true,
                editor_css: false,
                reload_shell: true,
            }
        );
    }

    #[test]
    fn editor_background_change_regenerates_editor_css_only() {
        let old = Settings::default();
        let new = Settings {
            editor_background: Some("#123456".to_owned()),
            ..old.clone()
        };
        assert_eq!(
            Effects::between(&old, &new),
            Effects {
                color_scheme: false,
                editor_css: true,
                reload_shell: false,
            }
        );
    }

    #[test]
    fn preview_background_change_reloads_shell_only() {
        let old = Settings::default();
        let new = Settings {
            preview_background: Some("#123456".to_owned()),
            ..old.clone()
        };
        assert_eq!(
            Effects::between(&old, &new),
            Effects {
                color_scheme: false,
                editor_css: false,
                reload_shell: true,
            }
        );
    }

    #[test]
    fn clearing_a_background_counts_as_a_change() {
        let old = Settings {
            editor_background: Some("#123456".to_owned()),
            preview_background: Some("#654321".to_owned()),
            ..Settings::default()
        };
        let new = Settings::default();
        assert_eq!(
            Effects::between(&old, &new),
            Effects {
                color_scheme: false,
                editor_css: true,
                reload_shell: true,
            }
        );
    }

    #[test]
    fn everything_changing_triggers_every_effect() {
        let old = Settings::default();
        let new = Settings {
            color_scheme: ColorScheme::Light,
            editor_background: Some("#111111".to_owned()),
            preview_background: Some("#222222".to_owned()),
            ..old.clone()
        };
        assert_eq!(
            Effects::between(&old, &new),
            Effects {
                color_scheme: true,
                editor_css: true,
                reload_shell: true,
            }
        );
    }

    // ---------------------------------------------------------------------
    // editor_css_text
    // ---------------------------------------------------------------------

    #[test]
    fn editor_css_without_background_uses_scheme_palette() {
        let light = editor_css_text(None, false);
        assert!(
            light.contains("background-color: #ffffff; color: #241f31;"),
            "got: {light}"
        );

        let dark = editor_css_text(None, true);
        assert!(
            dark.contains("background-color: #242424; color: #ffffff;"),
            "got: {dark}"
        );
    }

    #[test]
    fn editor_css_custom_light_background_gets_dark_text() {
        let css = editor_css_text(Some("#f0f0f0"), false);
        assert!(
            css.contains("background-color: #f0f0f0; color: #241f31;"),
            "got: {css}"
        );
    }

    #[test]
    fn editor_css_custom_dark_background_gets_light_text() {
        let css = editor_css_text(Some("#101010"), false);
        assert!(
            css.contains("background-color: #101010; color: #ffffff;"),
            "got: {css}"
        );
    }

    #[test]
    fn editor_css_custom_background_ignores_the_system_style() {
        for bg in ["#f0f0f0", "#101010", "rgb(10,20,30)"] {
            assert_eq!(
                editor_css_text(Some(bg), true),
                editor_css_text(Some(bg), false),
                "{bg}"
            );
        }
    }

    #[test]
    fn editor_css_invalid_background_falls_back_to_palette() {
        for bad in ["", "not-a-color", "#12"] {
            assert_eq!(
                editor_css_text(Some(bad), true),
                editor_css_text(None, true),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn editor_css_rejects_injection_via_background() {
        let css = editor_css_text(Some("red; } body { display: none"), false);
        assert!(!css.contains("display: none"), "got: {css}");
    }

    #[test]
    fn editor_css_targets_the_editor_and_is_well_formed() {
        let css = editor_css_text(None, false);
        assert!(
            css.starts_with("textview.silk-editor, textview.silk-editor text {"),
            "got: {css}"
        );
        assert_eq!(css.matches('{').count(), 1, "got: {css}");
        assert_eq!(css.matches('}').count(), 1, "got: {css}");
        assert!(css.trim_end().ends_with('}'), "got: {css}");
    }
}

//! Preferences dialog (`adw::PreferencesDialog`, libadwaita ≥ 1.5)
//!
//! The dialog keeps its own copy of `Settings`
//!
//! User edits are emitted to the parent as `Output`;
//! changes made elsewhere (the header-bar View popover) arrive as `PrefsMsg::Sync`. Signal
//! handlers are connected *after* the initial widget state is set so construction never emits
//! spurious changes

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk};
use relm4::prelude::*;

use crate::settings::{ColorScheme, Settings, ViewMode, default_custom_color, parse_color};

pub struct Preferences {
    settings: Settings,
}

#[derive(Debug)]
pub enum PrefsMsg {
    /// Replace local state with the application's current state
    Sync(Settings),
    SyncScroll(bool),
    ViewMode(ViewMode),
    Scheme(ColorScheme),
    EditorCustom(bool),
    EditorColor(gdk::RGBA),
    PreviewCustom(bool),
    PreviewColor(gdk::RGBA),
}

// Color parsing in custom settings
fn color_or_default(css: Option<&str>) -> gdk::RGBA {
    css.and_then(parse_color)
        .or_else(|| parse_color(&default_custom_color()))
        .unwrap_or(gdk::RGBA::WHITE)
}

// Handle toggling custom color settings
fn toggle(current: Option<String>, on: bool) -> Option<String> {
    match (current, on) {
        (Some(c), true) => Some(c),
        (None, true) => Some(default_custom_color()),
        (_, false) => None,
    }
}

#[relm4::component(pub)]
impl SimpleComponent for Preferences {
    type Init = Settings;
    type Input = PrefsMsg;
    type Output = Settings;

    view! {
        adw::PreferencesDialog {
            set_title: "Preferences",

            add = &adw::PreferencesPage {

                add = &adw::PreferencesGroup {
                    set_title: "Layout",

                    #[name = "layout_row"]
                    add = &adw::ComboRow {
                        set_title: "Visible Panes",
                        set_model: Some(&gtk::StringList::new(&ViewMode::LABELS)),
                        #[watch]
                        set_selected: model.settings.view_mode.index(),
                    },

                    #[name = "sync_row"]
                    add = &adw::SwitchRow {
                        set_title: "Synchronized Scrolling",
                        set_subtitle: "Scroll the editor and preview together.",
                        #[watch]
                        set_active: model.settings.sync_scroll,
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Appearance",
                    set_description: Some("By default, Silk follows the system light or dark style."),

                    #[name = "scheme_row"]
                    add = &adw::ComboRow {
                        set_title: "Color Scheme",
                        set_model: Some(&gtk::StringList::new(&ColorScheme::LABELS)),
                        #[watch]
                        set_selected: model.settings.color_scheme.index(),
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Custom Backgrounds",
                    set_description: Some("Override the background of each pane. Text color adjusts automatically."),

                    #[name = "editor_row"]
                    add = &adw::SwitchRow {
                        set_title: "Editor Background",
                        #[watch]
                        set_active: model.settings.editor_background.is_some(),

                        #[name = "editor_color"]
                        add_suffix = &gtk::ColorDialogButton {
                            set_valign: gtk::Align::Center,
                            set_dialog: &gtk::ColorDialog::builder()
                                .title("Editor Background")
                                .with_alpha(false)
                                .build(),
                            #[watch]
                            set_sensitive: model.settings.editor_background.is_some(),
                            #[watch]
                            set_rgba: &color_or_default(model.settings.editor_background.as_deref()),
                        },
                    },

                    #[name = "preview_row"]
                    add = &adw::SwitchRow {
                        set_title: "Preview Background",
                        #[watch]
                        set_active: model.settings.preview_background.is_some(),

                        #[name = "preview_color"]
                        add_suffix = &gtk::ColorDialogButton {
                            set_valign: gtk::Align::Center,
                            set_dialog: &gtk::ColorDialog::builder()
                                .title("Preview Background")
                                .with_alpha(false)
                                .build(),
                            #[watch]
                            set_sensitive: model.settings.preview_background.is_some(),
                            #[watch]
                            set_rgba: &color_or_default(model.settings.preview_background.as_deref()),
                        },
                    },
                },
            },
        }
    }

    fn init(
        settings: Settings,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Preferences { settings };
        let widgets = view_output!();

        // Connect handlers only now that the initial state has been applied
        let s = sender.clone();
        widgets.layout_row.connect_selected_notify(move |row| {
            s.input(PrefsMsg::ViewMode(ViewMode::from_index(row.selected())));
        });
        let s = sender.clone();
        widgets.scheme_row.connect_selected_notify(move |row| {
            s.input(PrefsMsg::Scheme(ColorScheme::from_index(row.selected())));
        });
        let s = sender.clone();
        widgets.editor_row.connect_active_notify(move |row| {
            s.input(PrefsMsg::EditorCustom(row.is_active()));
        });
        let s = sender.clone();
        widgets.editor_color.connect_rgba_notify(move |b| {
            s.input(PrefsMsg::EditorColor(b.rgba()));
        });
        let s = sender.clone();
        widgets.preview_row.connect_active_notify(move |row| {
            s.input(PrefsMsg::PreviewCustom(row.is_active()));
        });
        let s = sender.clone();
        widgets.sync_row.connect_active_notify(move |row| {
            s.input(PrefsMsg::SyncScroll(row.is_active()));
        });
        let s = sender;
        widgets.preview_color.connect_rgba_notify(move |b| {
            s.input(PrefsMsg::PreviewColor(b.rgba()));
        });

        ComponentParts { model, widgets }
    }

    // Update the preferences struct implementation and update handler
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        let mut next = self.settings.clone();

        match msg {
            PrefsMsg::Sync(settings) => {
                self.settings = settings;
                return;
            }
            PrefsMsg::SyncScroll(on) => next.sync_scroll = on,
            PrefsMsg::ViewMode(mode) => next.view_mode = mode,
            PrefsMsg::Scheme(scheme) => next.color_scheme = scheme,
            PrefsMsg::EditorCustom(on) => {
                next.editor_background = toggle(next.editor_background.take(), on);
            }
            PrefsMsg::PreviewCustom(on) => {
                next.preview_background = toggle(next.preview_background.take(), on);
            }
            // Ignore color notifications while the override is off: they are
            // produced by the `#[watch]` resetting the button, not by the user
            PrefsMsg::EditorColor(rgba) => {
                if next.editor_background.is_some() {
                    next.editor_background = Some(rgba.to_str().to_string());
                }
            }
            PrefsMsg::PreviewColor(rgba) => {
                if next.preview_background.is_some() {
                    next.preview_background = Some(rgba.to_str().to_string());
                }
            }
        }

        if next != self.settings {
            self.settings = next.clone();
            let _ = sender.output(next);
        }
    }
}

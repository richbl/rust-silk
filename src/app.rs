//! Main GUI window: editor, preview, file handling and view options

mod actions;
mod appearance;
mod file;
mod preview;
mod scroll;
mod view_menu;

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib};
use relm4::prelude::*;
use scroll::ScrollSync;
use std::path::Path;
use webkit6::WebView;

use crate::constants::{ABOUT, APP_ID, APP_NAME, APP_VER};
use crate::preferences::Preferences;
use crate::settings::{ColorScheme, Settings, ViewMode, WindowSettings};

// The menu! macro below refers to these action types by name
use actions::{AboutAction, PreferencesAction, SaveAsAction, register_window_actions};
use view_menu::{ViewRadios, build_view_popover};

/// Issue tracker used by the "Report an issue" row
const ISSUE_URL: &str = "https://github.com/richbl/rust-silk/issues/new?template=issue-report.md";

/// Actions that discard the current document (and so must first respect unsaved changes)
#[derive(Debug, Clone, Copy)]
pub enum Action {
    New,
    Open,
    Close,
}

/// Actions that can be performed without asking about unsaved changes
#[derive(Debug)]
pub enum Msg {
    BufferChanged,
    PreviewLoaded,
    /// Run `Action`, asking first about unsaved changes...
    Request(Action),
    /// Run `Action` unconditionally. Just do it...
    Perform(Action),
    Save(Option<Action>),
    SaveAs(Option<Action>),
    SaveTo(gio::File, Option<Action>),
    Saved(gio::File, Option<Action>),
    SaveFailed(String),
    Loaded(gio::File, String),
    SetViewMode(ViewMode),
    SetColorScheme(ColorScheme),
    SettingsChanged(Settings),
    EditorScrolled(f64),
    PreviewScrolled(f64),
    StyleChanged,
    ShowPreferences,
    ShowAbout,
    Toast(String),
}

/// The main window of the application
pub struct App {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    buffer: gtk::TextBuffer,
    web_view: WebView,
    editor_css: gtk::CssProvider,
    editor_vert_adj: gtk::Adjustment,
    sync: ScrollSync,
    preferences: Controller<Preferences>,
    radios: ViewRadios,
    settings: Settings,
    file: Option<gio::File>,
    dirty: bool,
    preview_ready: bool,
}

/// Title shown for a document that has not been saved to a file yet
const UNTITLED: &str = "Untitled Document";

// The name to show for a document, given its file's base name (if any)
fn display_name(basename: Option<&Path>) -> String {
    basename.map_or_else(|| UNTITLED.to_owned(), |p| p.to_string_lossy().into_owned())
}

// The window title: the name, prefixed with a bullet when there are unsaved changes
fn window_title(name: &str, dirty: bool) -> String {
    format!("{}{name}", if dirty { "• " } else { "" })
}

// `Save` is only meaningful when there are unsaved changes or no file yet, while `Save As` must
// stay available at all times (GNOME HIG)
fn save_enabled(dirty: bool, has_file: bool) -> bool {
    dirty || !has_file
}

// The Markdown shown in a fresh editor buffer
fn welcome_text() -> String {
    format!(
        "# Welcome to {APP_NAME} {APP_VER}\n\n## {ABOUT}\n\nStart typing **Markdown** on the left, \
        and it'll get rendered on the right.\n\n- Open, edit and save files\n\
        - Choose which panes to show\n- Customize color themes/background colors in *Preferences*\n"
    )
}

impl App {
    // Returns the file name for the window title
    fn name(&self) -> String {
        let basename = self
            .file
            .as_ref()
            .and_then(webkit6::prelude::FileExt::basename);
        display_name(basename.as_deref())
    }

    // Returns the window title based on the file name and dirty state
    fn title(&self) -> String {
        window_title(&self.name(), self.dirty)
    }

    fn sync_save_action(&self) {
        self.window
            .action_set_enabled("win.save", save_enabled(self.dirty, self.file.is_some()));
    }
    // Returns the text content of the text buffer
    fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), false)
            .to_string()
    }

    // Replace the buffer content without it being undo-able or counted as an edit
    fn set_document_text(&mut self, text: &str) {
        self.buffer.begin_irreversible_action();
        self.buffer.set_text(text);
        self.buffer.end_irreversible_action();
        self.buffer.set_modified(false);
        self.dirty = false;
        self.push_preview();
    }

    // Run an action that replaces or closes the current document
    fn perform(&mut self, action: Action, sender: &ComponentSender<Self>) {
        match action {
            Action::New => {
                self.file = None;
                self.set_document_text("");
            }
            Action::Open => self.show_open_dialog(sender),
            Action::Close => {
                self.buffer.set_modified(false);
                self.window.close();
            }
        }
    }

    // Shows the about dialog for the application
    fn show_about(&self) {
        adw::AboutDialog::builder()
            .application_name(APP_NAME)
            .application_icon(APP_ID)
            .version(env!("CARGO_PKG_VERSION"))
            .comments(ABOUT)
            .copyright("Copyright ©2026 Rich Bloch")
            .issue_url(ISSUE_URL)
            .developers(vec!["Rich Bloch".to_owned()])
            .website("https://github.com/richbl/rust-silk")
            .license_type(gtk::License::MitX11)
            .build()
            .present(Some(&self.window));
    }

    // Updates UI state or general messages depending on the component
    fn update_ui(&mut self, msg: Msg) {
        match msg {
            Msg::ShowPreferences => {
                self.preferences.widget().present(Some(&self.window));
            }
            Msg::ShowAbout => self.show_about(),
            Msg::Toast(message) => self.toasts.add_toast(adw::Toast::new(&message)),
            _ => unreachable!("UI handler called with unexpected message"),
        }
    }
}

// Prompt before closing when the document has unsaved changes
fn connect_close_request(
    root: &adw::ApplicationWindow,
    buffer: &gtk::TextBuffer,
    sender: &ComponentSender<App>,
) {
    let sender = sender.clone();
    let buffer = buffer.clone();

    root.connect_close_request(move |window| {
        if buffer.is_modified() {
            sender.input(Msg::Request(Action::Close));
            return glib::Propagation::Stop;
        }

        // `default_size()` tracks normal resizes and ignores maximized/fullscreen forced sizes
        let (width, height) = window.default_size();

        // Re-read from disk: the file already holds the latest preferences because
        // `apply_settings()` saves on every change
        let mut settings = Settings::load();
        settings.window = WindowSettings {
            width,
            height,
            maximized: window.is_maximized(),
            fullscreened: window.is_fullscreen(),
        };

        if let Err(e) = settings.save() {
            eprintln!("silk: could not save window state: {e}");
        }

        glib::Propagation::Proceed
    });
}

// Create the text buffer pre-filled with the welcome text, and notify the app when it changes
fn build_buffer(sender: &ComponentSender<App>) -> gtk::TextBuffer {
    let welcome = welcome_text();

    let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    buffer.set_text(&welcome);
    buffer.set_modified(false);

    let sender = sender.clone();
    buffer.connect_changed(move |_| sender.input(Msg::BufferChanged));

    buffer
}

// Re-apply the editor styling whenever the system light/dark preference changes
fn connect_style_changes(sender: &ComponentSender<App>) {
    let sender = sender.clone();
    adw::StyleManager::default().connect_dark_notify(move |_| sender.input(Msg::StyleChanged));
}

// Split the editor and preview 50/50 once the paned has a real width, then stop
fn center_paned(paned: &gtk::Paned) {
    paned.add_tick_callback(|paned, _| {
        let width = paned.width();
        if width > 0 {
            paned.set_position(width / 2);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    menu! {
        save_menu: {
            "Save As…" => SaveAsAction,
        },
        primary_menu: {
            section! {
                "Preferences" => PreferencesAction,
                "About Silk" => AboutAction,
            },
        }
    }

    view! {
        adw::ApplicationWindow {
            set_default_size: (model.settings.window.width, model.settings.window.height),
            set_maximized: model.settings.window.maximized,
            set_fullscreened: model.settings.window.fullscreened,
            #[watch]
            set_title: Some(&model.title()),

            #[local_ref]
            toasts -> adw::ToastOverlay {
                #[wrap(Some)]
                set_child = &adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            #[watch]
                            set_title: &model.title(),
                        },

                        pack_start = &gtk::Button {
                            #[wrap(Some)]
                            set_child = &adw::ButtonContent {
                                    set_icon_name: "document-new-symbolic",
                                    set_label: "New",
                                },
                                set_tooltip_text: Some("New Document (Ctrl+N)"),
                                set_action_name: Some("win.new"),
                        },

                        pack_start = &gtk::Button {
                            #[wrap(Some)]
                            set_child = &adw::ButtonContent {
                                set_icon_name: "document-open-symbolic",
                                set_label: "Open",
                            },
                            set_tooltip_text: Some("Open… (Ctrl+O)"),
                            set_action_name: Some("win.open"),
                    },

                        pack_start = &adw::SplitButton {
                            #[wrap(Some)]
                            set_child = &adw::ButtonContent {
                                set_icon_name: "document-save-symbolic",
                                set_label: "Save",
                            },
                            set_tooltip_text: Some("Save (Ctrl+S)"),
                            set_dropdown_tooltip: "More Save Options",
                            set_menu_model: Some(&save_menu),
                            set_action_name: Some("win.save"),
                        },

                        pack_end = &gtk::MenuButton {
                            set_icon_name: "open-menu-symbolic",
                            set_tooltip_text: Some("Main Menu"),
                            set_primary: true,
                            set_menu_model: Some(&primary_menu),
                        },
                        pack_end = &gtk::MenuButton {
                            #[wrap(Some)]
                            set_child = &adw::ButtonContent {
                                set_icon_name: "view-dual-symbolic",
                                set_label: "View",
                            },
                            set_tooltip_text: Some("View Options"),
                            set_popover: Some(&view_popover),
                        },
                    },

                    #[wrap(Some)]
                    #[name = "paned"]
                    set_content = &gtk::Paned {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_wide_handle: true,
                        set_resize_start_child: true,
                        set_resize_end_child: true,
                        set_shrink_start_child: false,
                        set_shrink_end_child: false,
                        set_position: model.settings.window.width / 2,

                        set_end_child: Some(&web_view),

                        #[wrap(Some)]
                        set_start_child = &gtk::ScrolledWindow {
                            set_vadjustment: Some(&model.editor_vert_adj),
                            set_hexpand: true,
                            set_vexpand: true,
                            #[watch]
                            set_visible: model.settings.view_mode.shows_editor(),

                            gtk::TextView {
                                set_buffer: Some(&model.buffer),
                                set_monospace: true,
                                set_wrap_mode: gtk::WrapMode::WordChar,
                                set_top_margin: 16,
                                set_bottom_margin: 16,
                                set_left_margin: 16,
                                set_right_margin: 16,
                                add_css_class: "silk-editor",
                            },
                        },
                    },
                },
            },
        }
    }

    // Widgets the macro does not manage are synced after every update
    fn post_view() {
        model
            .web_view
            .set_visible(model.settings.view_mode.shows_preview());
        model.radios.sync(&model.settings);
        model.sync_save_action();
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let settings = Settings::load();

        adw::StyleManager::default().set_color_scheme(settings.color_scheme.to_adw());

        let buffer = build_buffer(&sender);
        connect_style_changes(&sender);

        let web_view = preview::build_preview(&sender);
        let editor_css = appearance::build_editor_css();
        let toasts = adw::ToastOverlay::new();
        let (view_popover, radios) = build_view_popover(&sender, &settings);
        let editor_vert_adj = scroll::build_editor_adjustment(&sender);
        let preferences = Preferences::builder()
            .launch(settings.clone())
            .forward(sender.input_sender(), Msg::SettingsChanged);

        let mut model = App {
            window: root.clone(),
            toasts: toasts.clone(),
            buffer: buffer.clone(),
            web_view: web_view.clone(),
            editor_css,
            editor_vert_adj,
            sync: ScrollSync::default(),
            preferences,
            radios,
            settings,
            file: None,
            dirty: false,
            preview_ready: false,
        };

        if let Some(display) = gtk::gdk::Display::default() {
            let path = format!("/{}/icons", APP_ID.replace('.', "/"));
            gtk::IconTheme::for_display(&display).add_resource_path(&path);
        }

        let widgets = view_output!();

        widgets.paned.set_end_child(Some(&web_view));
        center_paned(&widgets.paned);

        model
            .web_view
            .set_visible(model.settings.view_mode.shows_preview());
        model.apply_editor_css();
        model.reload_shell();

        connect_close_request(&root, &buffer, &sender);
        register_window_actions(&root, &sender);
        model.sync_save_action();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Msg::BufferChanged => {
                self.dirty = self.buffer.is_modified();
                self.push_preview();
            }
            Msg::PreviewLoaded => {
                self.preview_ready = true;
                self.push_preview();
            }
            Msg::Request(action) => {
                if self.dirty {
                    self.confirm_discard(action, &sender);
                } else {
                    self.perform(action, &sender);
                }
            }

            Msg::StyleChanged => self.apply_editor_css(),
            Msg::Perform(action) => self.perform(action, &sender),

            msg @ (Msg::Save(_)
            | Msg::SaveAs(_)
            | Msg::SaveTo(_, _)
            | Msg::Saved(_, _)
            | Msg::SaveFailed(_)
            | Msg::Loaded(_, _)) => {
                self.update_file(msg, &sender);
            }

            msg @ (Msg::EditorScrolled(_) | Msg::PreviewScrolled(_)) => self.update_scroll(&msg),

            msg @ (Msg::SetViewMode(_) | Msg::SetColorScheme(_) | Msg::SettingsChanged(_)) => {
                self.update_settings(msg);
            }

            msg @ (Msg::ShowPreferences | Msg::ShowAbout | Msg::Toast(_)) => {
                self.update_ui(msg);
            }
        }
    }
}

#[cfg(test)]
mod tests;

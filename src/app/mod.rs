//! Main GUI window: editor, preview, file handling and view options

mod actions;
mod appearance;
mod file;
mod preview;
mod view_menu;

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib};
use relm4::prelude::*;
use webkit6::WebView;

use crate::preferences::Preferences;
use crate::settings::{ColorScheme, Settings, ViewMode};
use crate::{APP_VER, DEFAULT_HEIGHT, DEFAULT_WIDTH};

// The menu! macro below refers to these action types by name
use actions::{AboutAction, PreferencesAction, SaveAsAction, register_window_actions};
use view_menu::{ViewRadios, build_view_popover};

/// Actions that discard the current document and therefore must first respect unsaved changes
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
    /// Run `Action`, asking about unsaved changes first...
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
    preferences: Controller<Preferences>,
    radios: ViewRadios,
    settings: Settings,
    file: Option<gio::File>,
    dirty: bool,
    preview_ready: bool,
}

impl App {
    // Returns the file name for the window title
    fn name(&self) -> String {
        self.file
            .as_ref()
            .and_then(webkit6::prelude::FileExt::basename)
            .map_or_else(
                || "Untitled Document".into(),
                |p| p.to_string_lossy().into_owned(),
            )
    }

    // Returns the window title based on the file name and dirty state
    fn title(&self) -> String {
        format!("{}{}", if self.dirty { "• " } else { "" }, self.name())
    }

    // Returns the text content of the text buffer
    fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), false)
            .to_string()
    }

    // Save is only meaningful when there are unsaved changes; Save As must stay available
    // at all times (GNOME HIG), so only `win.save` is toggled.
    fn sync_save_action(&self) {
        self.window
            .action_set_enabled("win.save", self.dirty || self.file.is_none());
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
            .application_name("Silk")
            .application_icon("accessories-text-editor")
            .developer_name("The Silk Authors")
            .version(env!("CARGO_PKG_VERSION"))
            .comments("A minimal Markdown editor for GNOME")
            .license_type(gtk::License::Gpl30)
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

    root.connect_close_request(move |_| {
        if buffer.is_modified() {
            sender.input(Msg::Request(Action::Close));
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
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
            set_default_size: (DEFAULT_WIDTH, DEFAULT_HEIGHT),
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
                            set_icon_name: "view-dual-symbolic",
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
                        set_position: 500,

                        set_end_child: Some(&web_view),

                        #[wrap(Some)]
                        set_start_child = &gtk::ScrolledWindow {
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

        let welcome = format!(
            "# Welcome to Silk {APP_VER}\n\nStart typing **Markdown** on the left, \
        and it'll get rendered on the right.\n\n- [x] Open, edit and save files\n\
        - [x] Choose which panes to show\n- [x] Customize backgrounds in *Preferences*\n"
        );

        let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
        buffer.set_text(&welcome);
        buffer.set_modified(false);

        {
            let sender = sender.clone();
            buffer.connect_changed(move |_| {
                sender.input(Msg::BufferChanged);
            });
        }

        let web_view = preview::build_preview(&sender);
        let editor_css = appearance::build_editor_css();
        let toasts = adw::ToastOverlay::new();
        let (view_popover, radios) = build_view_popover(&sender, &settings);
        let preferences = Preferences::builder()
            .launch(settings.clone())
            .forward(sender.input_sender(), Msg::SettingsChanged);

        let mut model = App {
            window: root.clone(),
            toasts: toasts.clone(),
            buffer: buffer.clone(),
            web_view: web_view.clone(),
            editor_css,
            preferences,
            radios,
            settings,
            file: None,
            dirty: false,
            preview_ready: false,
        };

        let widgets = view_output!();

        widgets.paned.set_end_child(Some(&web_view));
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
            Msg::Perform(action) => self.perform(action, &sender),

            msg @ (Msg::Save(_)
            | Msg::SaveAs(_)
            | Msg::SaveTo(_, _)
            | Msg::Saved(_, _)
            | Msg::SaveFailed(_)
            | Msg::Loaded(_, _)) => {
                self.update_file(msg, &sender);
            }

            msg @ (Msg::SetViewMode(_) | Msg::SetColorScheme(_) | Msg::SettingsChanged(_)) => {
                self.update_settings(msg);
            }

            msg @ (Msg::ShowPreferences | Msg::ShowAbout | Msg::Toast(_)) => {
                self.update_ui(msg);
            }
        }
    }
}

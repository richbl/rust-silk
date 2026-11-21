//! Main GUI window: editor, preview, file handling and view options

use std::rc::Rc;

use relm4::actions::{AccelsPlus, RelmAction, RelmActionGroup};
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk, gio, glib};
use relm4::prelude::*;
use webkit6::WebView;
use webkit6::prelude::*;

use crate::preferences::{Preferences, PrefsMsg};
use crate::render::{markdown_to_html, shell_html};
use crate::settings::{ColorScheme, Settings, ViewMode, parse_color, readable_foreground};
use crate::{APP_VER, DEFAULT_HEIGHT, DEFAULT_WIDTH};

// Window actions (menu items and keyboard shortcuts)
relm4::new_action_group!(WinActions, "win");
relm4::new_stateless_action!(NewAction, WinActions, "new");
relm4::new_stateless_action!(OpenAction, WinActions, "open");
relm4::new_stateless_action!(SaveAction, WinActions, "save");
relm4::new_stateless_action!(SaveAsAction, WinActions, "save-as");
relm4::new_stateless_action!(PreferencesAction, WinActions, "preferences");
relm4::new_stateless_action!(AboutAction, WinActions, "about");

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

// Radio buttons living in the View popover; kept so they can be re-synced when settings change
// from the Preferences dialog
struct ViewRadios {
    layout: Vec<gtk::CheckButton>,
    scheme: Vec<gtk::CheckButton>,
}

impl ViewRadios {
    fn sync(&self, settings: &Settings) {
        self.layout[settings.view_mode.index() as usize].set_active(true);
        self.scheme[settings.color_scheme.index() as usize].set_active(true);
    }
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

    // Replace the buffer content without it being undo-able or counted as an edit
    fn set_document_text(&mut self, text: &str) {
        self.buffer.begin_irreversible_action();
        self.buffer.set_text(text);
        self.buffer.end_irreversible_action();
        self.buffer.set_modified(false);
        self.dirty = false;
        self.push_preview();
    }

    // Update the preview with the current editor text when ready
    fn push_preview(&self) {
        if !self.preview_ready {
            return;
        }
        let html = markdown_to_html(&self.text());
        if let Ok(literal) = serde_json::to_string(&html) {
            let script = format!("document.getElementById('content').innerHTML = {literal};");
            self.web_view.evaluate_javascript(
                &script,
                None,
                None,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }
    }

    // Reload the preview shell with the current appearance settings
    fn reload_shell(&mut self) {
        self.preview_ready = false;
        let background = self.settings.preview_background.as_deref();
        let rgba = background
            .and_then(parse_color)
            .unwrap_or(gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));
        self.web_view.set_background_color(&rgba);
        self.web_view
            .load_html(&shell_html(self.settings.color_scheme, background), None);
    }

    // Apply the editor CSS styling based on current settings
    fn apply_editor_css(&self) {
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

    // Update the window title or path based on the current file
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

    // Confirm whether the user wants to discard unsaved changes before proceeding with an action
    fn confirm_discard(&self, action: Action, sender: &ComponentSender<Self>) {
        let dialog = adw::AlertDialog::builder()
            .heading("Save Changes?")
            .body(format!(
                "“{}” has unsaved changes. If you don’t save, they will be lost.",
                self.name()
            ))
            .close_response("cancel")
            .default_response("save")
            .build();

        dialog.add_responses(&[
            ("cancel", "_Cancel"),
            ("discard", "_Discard"),
            ("save", "_Save"),
        ]);

        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);

        let window = self.window.clone();
        let sender = sender.clone();
        glib::spawn_future_local(async move {
            match dialog.choose_future(Some(&window)).await.as_str() {
                "save" => sender.input(Msg::Save(Some(action))),
                "discard" => sender.input(Msg::Perform(action)),
                _ => {}
            }
        });
    }

    // Show the file save dialog before performing an action that requires a saved file
    fn show_open_dialog(&self, sender: &ComponentSender<Self>) {
        let dialog = markdown_dialog("Open Markdown File");
        let window = self.window.clone();
        let sender = sender.clone();

        glib::spawn_future_local(async move {
            let file = match dialog.open_future(Some(&window)).await {
                Ok(file) => file,
                Err(e) => {
                    report_dialog_error(&sender, &e, "Could not open file");
                    return;
                }
            };
            match file.load_contents_future().await {
                Ok((bytes, _)) => match String::from_utf8(bytes.to_vec()) {
                    Ok(text) => sender.input(Msg::Loaded(file, text)),
                    Err(_) => sender.input(Msg::Toast("That file is not valid UTF-8 text".into())),
                },
                Err(e) => sender.input(Msg::Toast(format!("Could not open file: {}", e.message()))),
            }
        });
    }

    // Helper function or method to handle export dialogs
    fn show_save_dialog(&self, then: Option<Action>, sender: &ComponentSender<Self>) {
        let dialog = markdown_dialog("Save Markdown File");
        match &self.file {
            Some(file) => dialog.set_initial_file(Some(file)),
            None => dialog.set_initial_name(Some("Untitled.md")),
        }
        let window = self.window.clone();
        let sender = sender.clone();

        glib::spawn_future_local(async move {
            match dialog.save_future(Some(&window)).await {
                Ok(file) => sender.input(Msg::SaveTo(file, then)),
                Err(e) => report_dialog_error(&sender, &e, "Could not save file"),
            }
        });
    }

    // Helper function to write content to a file asynchronously
    fn write_file(
        &mut self,
        file: gio::File,
        then: Option<Action>,
        sender: &ComponentSender<Self>,
    ) {
        let bytes = self.text().into_bytes();

        // Mark clean up front so edits made during the async write stay dirty
        self.buffer.set_modified(false);
        self.dirty = false;

        // Replace contents future call and spawn the write operation
        let sender = sender.clone();
        glib::spawn_future_local(async move {
            match file
                .replace_contents_future(bytes, None, false, gio::FileCreateFlags::NONE)
                .await
            {
                Ok(_) => sender.input(Msg::Saved(file, then)),
                Err((_, e)) => sender.input(Msg::SaveFailed(e.message().to_string())),
            }
        });
    }

    // Update helper function to check if the file has unsaved changes
    fn update_file(&mut self, msg: Msg, sender: &ComponentSender<Self>) {
        match msg {
            Msg::Save(then) => match self.file.clone() {
                Some(file) => self.write_file(file, then, sender),
                None => self.show_save_dialog(then, sender),
            },
            Msg::SaveAs(then) => self.show_save_dialog(then, sender),
            Msg::SaveTo(file, then) => self.write_file(file, then, sender),
            Msg::Saved(file, then) => {
                self.file = Some(file);
                if let Some(action) = then {
                    self.perform(action, sender);
                }
            }
            Msg::SaveFailed(error) => {
                self.buffer.set_modified(true);
                self.dirty = true;
                self.toasts
                    .add_toast(adw::Toast::new(&format!("Could not save file: {error}")));
            }
            Msg::Loaded(file, text) => {
                self.file = Some(file);
                self.set_document_text(&text);
            }
            _ => unreachable!("file message handler called with unexpected message"),
        }
    }

    // Updates UI state or general messages depending on the component
    fn update_settings(&mut self, msg: Msg) {
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

    //  Updates UI state or general messages depending on the component
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

// Create a file dialog filtered for Markdown files
fn markdown_dialog(title: &str) -> gtk::FileDialog {
    let markdown = gtk::FileFilter::new();
    markdown.set_name(Some("Markdown"));
    markdown.add_mime_type("text/markdown");
    markdown.add_suffix("md");
    markdown.add_suffix("markdown");

    let all = gtk::FileFilter::new();
    all.set_name(Some("All Files"));
    all.add_pattern("*");

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&markdown);
    filters.append(&all);

    gtk::FileDialog::builder()
        .title(title)
        .filters(&filters)
        .default_filter(&markdown)
        .build()
}

// Handle errors from asynchronous file dialogs or other operations
fn report_dialog_error(sender: &ComponentSender<App>, error: &glib::Error, context: &str) {
    if !(error.matches(gtk::DialogError::Dismissed) || error.matches(gtk::DialogError::Cancelled)) {
        sender.input(Msg::Toast(format!("{context}: {}", error.message())));
    }
}

// Create a group of radio buttons (a heading plus one `CheckButton` per option)
fn radio_group(
    parent: &gtk::Box,
    heading: &str,
    labels: &[&str],
    active: usize,
    on_select: impl Fn(usize) + 'static,
) -> Vec<gtk::CheckButton> {
    let heading = gtk::Label::new(Some(heading));
    heading.set_halign(gtk::Align::Start);
    heading.add_css_class("heading");
    parent.append(&heading);

    let mut buttons: Vec<gtk::CheckButton> = Vec::with_capacity(labels.len());
    for (i, label) in labels.iter().enumerate() {
        let button = gtk::CheckButton::with_label(label);
        if let Some(first) = buttons.first() {
            button.set_group(Some(first));
        }
        button.set_active(i == active);
        parent.append(&button);
        buttons.push(button);
    }

    // Connect last so building the group emits nothing
    let on_select = Rc::new(on_select);
    for (i, button) in buttons.iter().enumerate() {
        let on_select = Rc::clone(&on_select);
        button.connect_toggled(move |b| {
            if b.is_active() {
                on_select(i);
            }
        });
    }
    buttons
}

// Convert settings into view radio groups
fn build_view_popover(
    sender: &ComponentSender<App>,
    settings: &Settings,
) -> (gtk::Popover, ViewRadios) {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let s = sender.clone();
    let layout = radio_group(
        &content,
        "Layout",
        &ViewMode::LABELS,
        settings.view_mode.index() as usize,
        move |i| {
            if let Ok(i) = u32::try_from(i) {
                s.input(Msg::SetViewMode(ViewMode::from_index(i)));
            }
        },
    );

    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let s = sender.clone();
    let scheme = radio_group(
        &content,
        "Appearance",
        &ColorScheme::LABELS,
        settings.color_scheme.index() as usize,
        move |i| {
            if let Ok(i) = u32::try_from(i) {
                s.input(Msg::SetColorScheme(ColorScheme::from_index(i)));
            }
        },
    );

    let popover = gtk::Popover::new();
    popover.set_child(Some(&content));
    (popover, ViewRadios { layout, scheme })
}

// Build the preview web view and handle loading and external links
fn build_preview(sender: &ComponentSender<App>) -> WebView {
    let web_view = WebView::new();
    web_view.set_hexpand(true);
    web_view.set_vexpand(true);

    let sender = sender.clone();
    web_view.connect_load_changed(move |_, event| {
        if event == webkit6::LoadEvent::Finished {
            sender.input(Msg::PreviewLoaded);
        }
    });

    web_view.connect_decide_policy(|_, decision, kind| {
        use webkit6::{NavigationPolicyDecision, NavigationType, PolicyDecisionType};

        if !matches!(
            kind,
            PolicyDecisionType::NavigationAction | PolicyDecisionType::NewWindowAction
        ) {
            return false;
        }

        let Some(nav) = decision.downcast_ref::<NavigationPolicyDecision>() else {
            return false;
        };
        let Some(action) = nav.navigation_action() else {
            return false;
        };
        if action.navigation_type() != NavigationType::LinkClicked {
            return false;
        }

        if let Some(uri) = action.request().and_then(|r| r.uri()) {
            gtk::UriLauncher::new(&uri).launch(
                None::<&gtk::Window>,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }

        decision.ignore();
        true
    });

    web_view
}

// Create the CSS provider for the editor styling and inject it into the default display
fn build_editor_css() -> gtk::CssProvider {
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

// Register window actions and keyboard shortcuts
fn register_window_actions(root: &adw::ApplicationWindow, sender: &ComponentSender<App>) {
    let mut actions = RelmActionGroup::<WinActions>::new();

    macro_rules! action {
        ($ty:ty, $msg:expr) => {{
            let sender = sender.clone();
            actions.add_action(RelmAction::<$ty>::new_stateless(move |_| {
                sender.input($msg)
            }));
        }};
    }

    action!(NewAction, Msg::Request(Action::New));
    action!(OpenAction, Msg::Request(Action::Open));
    action!(SaveAction, Msg::Save(None));
    action!(SaveAsAction, Msg::SaveAs(None));
    action!(PreferencesAction, Msg::ShowPreferences);
    action!(AboutAction, Msg::ShowAbout);
    actions.register_for_widget(root);

    let app = relm4::main_application();
    app.set_accelerators_for_action::<NewAction>(&["<Primary>n"]);
    app.set_accelerators_for_action::<OpenAction>(&["<Primary>o"]);
    app.set_accelerators_for_action::<SaveAction>(&["<Primary>s"]);
    app.set_accelerators_for_action::<SaveAsAction>(&["<Primary><Shift>s"]);
    app.set_accelerators_for_action::<PreferencesAction>(&["<Primary>comma"]);
    app.set_accels_for_action("window.close", &["<Primary>w", "<Primary>q"]);
}

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    menu! {
        primary_menu: {
            "Save As…" => SaveAsAction,
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
                            set_icon_name: "document-new-symbolic",
                            set_tooltip_text: Some("New Document (Ctrl+N)"),
                            set_action_name: Some("win.new"),
                        },
                        pack_start = &gtk::Button {
                            set_icon_name: "document-open-symbolic",
                            set_tooltip_text: Some("Open… (Ctrl+O)"),
                            set_action_name: Some("win.open"),
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
                        pack_end = &gtk::Button {
                            set_label: "Save",
                            set_tooltip_text: Some("Save (Ctrl+S)"),
                            add_css_class: "suggested-action",
                            set_action_name: Some("win.save"),
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

        let web_view = build_preview(&sender);
        let editor_css = build_editor_css();
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

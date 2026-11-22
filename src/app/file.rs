//! File handling: open, save, save-as and the unsaved-changes prompt

use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio, glib};
use relm4::prelude::*;

use super::{Action, App, Msg};

impl App {
    // Handle the file-related messages (save, save as, loaded, ...)
    pub(super) fn update_file(&mut self, msg: Msg, sender: &ComponentSender<Self>) {
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

    // Confirm whether the user wants to discard unsaved changes before proceeding with an action
    pub(super) fn confirm_discard(&self, action: Action, sender: &ComponentSender<Self>) {
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

    // Show the file open dialog and load the chosen file
    pub(super) fn show_open_dialog(&self, sender: &ComponentSender<Self>) {
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

    // Show the file save dialog, optionally continuing with an action afterwards
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

    // Write the buffer content to a file asynchronously
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

use std::fs;
use std::path::PathBuf;

use pulldown_cmark::{Options, Parser, html};
use relm4::adw;
use relm4::gtk;
use relm4::prelude::*;
use webkit6::WebView;
use webkit6::prelude::*;

struct AppModel {
    markdown: String,
    current_file: Option<PathBuf>,
    web_view: WebView,
}

#[derive(Debug)]
enum AppMsg {
    TextChanged(String),
    OpenClicked,
    FileOpened(PathBuf, String),
    SaveClicked,
    FileSaved(PathBuf),
}

fn render_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut body = String::new();
    html::push_html(&mut body, parser);

    // Basic CSS supporting GNOME light/dark mode automatically
    format!(
        r#"<!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <style>
                :root {{ color-scheme: light dark; }}
                body {{
                    font-family: system-ui, -apple-system, sans-serif;
                    line-height: 1.6;
                    padding: 20px;
                    max-width: 800px;
                    margin: 0 auto;
                }}
                pre, code {{
                    font-family: monospace;
                    background: rgba(127, 127, 127, 0.15);
                    border-radius: 4px;
                }}
                pre {{ padding: 12px; overflow-x: auto; }}
                code {{ padding: 2px 4px; }}
                blockquote {{
                    border-left: 4px solid rgba(127, 127, 127, 0.4);
                    margin-left: 0;
                    padding-left: 16px;
                }}
            </style>
        </head>
        <body>{body}</body>
        </html>"#
    )
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[name = "main_window"]
        adw::ApplicationWindow {
            set_title: Some("Silk Markdown Editor"),
            set_default_size: (1000, 700),

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_start = &gtk::Button {
                        set_label: "Open",
                        set_icon_name: "document-open-symbolic",
                        connect_clicked => AppMsg::OpenClicked,
                    },
                    pack_end = &gtk::Button {
                        set_label: "Save",
                        add_css_class: "suggested-action",
                        connect_clicked => AppMsg::SaveClicked,
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Paned {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_wide_handle: true,
                    set_position: 500,

                    // Left Pane: Raw Markdown Editor
                    #[wrap(Some)]
                    set_start_child = &gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,

                        #[name = "text_view"]
                        gtk::TextView {
                            set_monospace: true,
                            set_wrap_mode: gtk::WrapMode::WordChar,
                            set_top_margin: 16,
                            set_bottom_margin: 16,
                            set_left_margin: 16,
                            set_right_margin: 16,
                        }
                    },

                    // Right Pane: Rendered Markdown Preview
                    #[wrap(Some)]
                    set_end_child = &gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        #[local_ref]
                        web_view -> WebView {
                            set_hexpand: true,
                            set_vexpand: true,
                        }
                    },
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let initial_md =
            String::from("# Hello World\n\nStart typing **Markdown** syntax on the left!");
        let web_view = WebView::new();
        web_view.load_html(&render_html(&initial_md), None);

        let model = AppModel {
            markdown: initial_md.clone(),
            current_file: None,
            web_view,
        };

        let web_view = &model.web_view;
        let widgets = view_output!();

        // Initialize buffer text and connect change listener
        let buffer = widgets.text_view.buffer();
        buffer.set_text(&initial_md);

        let input_sender = sender.input_sender().clone();
        buffer.connect_changed(move |buf| {
            let text = buf
                .text(&buf.start_iter(), &buf.end_iter(), false)
                .to_string();
            let _ = input_sender.send(AppMsg::TextChanged(text));
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::TextChanged(new_text) => {
                self.markdown = new_text;
                self.web_view.load_html(&render_html(&self.markdown), None);
            }
            AppMsg::OpenClicked => {
                let dialog = gtk::FileDialog::builder()
                    .title("Open Markdown File")
                    .build();

                let input_sender = sender.input_sender().clone();
                dialog.open(
                    None::<&gtk::Window>,
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        if let Ok(file) = result
                            && let Some(path) = file.path()
                            && let Ok(content) = fs::read_to_string(&path)
                        {
                            let _ = input_sender.send(AppMsg::FileOpened(path, content));
                        }
                    },
                );
            }
            AppMsg::FileOpened(path, content) => {
                self.current_file = Some(path);
                self.markdown = content;
                self.web_view.load_html(&render_html(&self.markdown), None);
                // Note: To update TextView without triggering an infinite loop,
                // store the TextBuffer in AppModel and call buffer.set_text(&self.markdown)
            }
            AppMsg::SaveClicked => {
                if let Some(ref path) = self.current_file {
                    let _ = fs::write(path, &self.markdown);
                } else {
                    let dialog = gtk::FileDialog::builder()
                        .title("Save Markdown File")
                        .initial_name("document.md")
                        .build();

                    let content = self.markdown.clone();
                    let input_sender = sender.input_sender().clone();
                    dialog.save(
                        None::<&gtk::Window>,
                        None::<&gtk::gio::Cancellable>,
                        move |result| {
                            if let Ok(file) = result
                                && let Some(path) = file.path()
                                && fs::write(&path, &content).is_ok()
                            {
                                let _ = input_sender.send(AppMsg::FileSaved(path));
                            }
                        },
                    );
                }
            }
            AppMsg::FileSaved(path) => {
                self.current_file = Some(path);
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("org.example.MarkdownEditor");
    app.run::<AppModel>(());
}

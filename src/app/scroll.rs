//! Synchronized scrolling between the editor and the preview windows

use relm4::gtk::{self, gio};
use relm4::prelude::*;
use webkit6::prelude::*;
use webkit6::{UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime};

use super::{App, Msg};

// Ignore echoes smaller than this (fraction of the scroll range)
const EPSILON: f64 = 0.002;

// Reports the page's scroll fraction (0.0..=1.0) back to the app
const REPORT_SCRIPT: &str = r"
window.addEventListener('scroll', () => {
  const range = document.documentElement.scrollHeight - window.innerHeight;
  window.webkit.messageHandlers.scroll.postMessage(range > 0 ? window.scrollY / range : 0);
}, { passive: true });
";

/// The last fraction each pane was told to scroll to, used to suppress echo events
#[derive(Debug, Default)]
pub(super) struct ScrollSync {
    editor: f64,
    preview: f64,
}

/// Current scroll position of an adjustment as a fraction of its range
pub(super) fn fraction(adj: &gtk::Adjustment) -> f64 {
    let range = adj.upper() - adj.page_size();
    if range > 0.0 {
        (adj.value() / range).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Create the editor's vertical adjustment and report user scrolling to the app
pub(super) fn build_editor_adjustment(sender: &ComponentSender<App>) -> gtk::Adjustment {
    let adj = gtk::Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let sender = sender.clone();
    adj.connect_value_changed(move |adj| sender.input(Msg::EditorScrolled(fraction(adj))));
    adj
}

/// Create the content manager that injects the scroll reporter into the preview page
pub(super) fn build_content_manager(sender: &ComponentSender<App>) -> UserContentManager {
    let manager = UserContentManager::new();
    manager.register_script_message_handler("scroll", None);

    let sender = sender.clone();
    manager.connect_script_message_received(Some("scroll"), move |_, value| {
        sender.input(Msg::PreviewScrolled(value.to_double()));
    });

    manager.add_script(&UserScript::new(
        REPORT_SCRIPT,
        UserContentInjectedFrames::TopFrame,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    ));
    manager
}

impl App {
    // Handle scroll events from either pane, ignoring echoes of our own scrolling
    pub(super) fn update_scroll(&mut self, msg: &Msg) {
        if !self.settings.sync_scroll {
            return;
        }
        match *msg {
            Msg::EditorScrolled(f) if (f - self.sync.editor).abs() > EPSILON => {
                self.sync.preview = f;
                let script = format!(
                    "window.scrollTo(0, {f} * \
                     (document.documentElement.scrollHeight - window.innerHeight));"
                );
                self.web_view.evaluate_javascript(
                    &script,
                    None,
                    None,
                    None::<&gio::Cancellable>,
                    |_| {},
                );
            }
            Msg::PreviewScrolled(f) if (f - self.sync.preview).abs() > EPSILON => {
                self.sync.editor = f;
                let adj = &self.editor_vert_adj;
                adj.set_value(f * (adj.upper() - adj.page_size()).max(0.0));
            }
            _ => {}
        }
    }
}

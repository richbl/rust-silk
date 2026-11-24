//! Markdown preview: the web view, its shell page and live updates

use relm4::adw::prelude::*;
use relm4::gtk::{self, gdk, gio};
use relm4::prelude::*;
use webkit6::WebView;
use webkit6::prelude::*;

use super::{App, Msg};
use crate::render::{markdown_to_html, shell_html};
use crate::settings::parse_color;

impl App {
    // Update the preview with the current editor text when ready
    pub(super) fn push_preview(&self) {
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
    pub(super) fn reload_shell(&mut self) {
        self.preview_ready = false;
        let background = self.settings.preview_background.as_deref();
        let rgba = background
            .and_then(parse_color)
            .unwrap_or(gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));
        self.web_view.set_background_color(&rgba);
        self.web_view
            .load_html(&shell_html(self.settings.color_scheme, background), None);
    }
}

// Build the preview web view and handle loading and external links
pub(super) fn build_preview(sender: &ComponentSender<App>) -> WebView {
    let manager = super::scroll::build_content_manager(sender);
    let web_view = WebView::builder().user_content_manager(&manager).build();
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

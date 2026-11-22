//! The View popover: layout and appearance radio buttons

use std::rc::Rc;

use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

use super::{App, Msg};
use crate::settings::{ColorScheme, Settings, ViewMode};

// Radio buttons living in the View popover; kept so they can be re-synced when settings change
// from the Preferences dialog
pub(super) struct ViewRadios {
    layout: Vec<gtk::CheckButton>,
    scheme: Vec<gtk::CheckButton>,
}

impl ViewRadios {
    pub(super) fn sync(&self, settings: &Settings) {
        self.layout[settings.view_mode.index() as usize].set_active(true);
        self.scheme[settings.color_scheme.index() as usize].set_active(true);
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
pub(super) fn build_view_popover(
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

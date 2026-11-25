//! Synchronized scrolling between the editor and the preview windows

use relm4::gtk::{self, gio};
use relm4::prelude::*;
use webkit6::prelude::*;
use webkit6::{UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime};

use super::{App, Msg};

// Ignore echoes smaller than this (fraction of the scroll range)
const EPSILON: f64 = 0.002;

// Name of the script message handler: registered on the content manager, listened to on the
// Rust side, and posted to by `REPORT_SCRIPT`
const MESSAGE_HANDLER: &str = "scroll";

// Reports the page's scroll fraction (0.0..=1.0) back to the app
const REPORT_SCRIPT: &str = r"
window.addEventListener('scroll', () => {
  const range = document.documentElement.scrollHeight - window.innerHeight;
  window.webkit.messageHandlers.scroll.postMessage(range > 0 ? window.scrollY / range : 0);
}, { passive: true });
";

/// Where a pane must be scrolled to, as a fraction (0.0..=1.0) of its scroll range
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ScrollTarget {
    Editor(f64),
    Preview(f64),
}

/// The position both panes were last synchronized to, used to suppress echo events
#[derive(Debug, Default)]
pub(super) struct ScrollSync {
    position: f64,
}

impl ScrollSync {
    pub(super) fn handle(&mut self, enabled: bool, msg: &Msg) -> Option<ScrollTarget> {
        if !enabled {
            return None;
        }
        match *msg {
            Msg::EditorScrolled(f) if self.moved_to(f) => Some(ScrollTarget::Preview(f)),
            Msg::PreviewScrolled(f) if self.moved_to(f) => Some(ScrollTarget::Editor(f)),
            _ => None,
        }
    }

    fn moved_to(&mut self, f: f64) -> bool {
        let moved = (f - self.position).abs() > EPSILON;
        if moved {
            self.position = f;
        }
        moved
    }
}

// Scroll position as a fraction of the scrollable range
fn fraction_of(value: f64, upper: f64, page_size: f64) -> f64 {
    let range = upper - page_size;
    if range > 0.0 {
        (value / range).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

// Adjustment value for a fraction of the scrollable range
fn value_for_fraction(fraction: f64, upper: f64, page_size: f64) -> f64 {
    fraction.clamp(0.0, 1.0) * (upper - page_size).max(0.0)
}

// JavaScript that scrolls the preview page to a fraction of its scrollable range
fn preview_scroll_script(fraction: f64) -> String {
    format!(
        "window.scrollTo(0, {fraction} * \
         (document.documentElement.scrollHeight - window.innerHeight));"
    )
}

/// Current scroll position of an adjustment as a fraction of its range
pub(super) fn fraction(adj: &gtk::Adjustment) -> f64 {
    fraction_of(adj.value(), adj.upper(), adj.page_size())
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
    manager.register_script_message_handler(MESSAGE_HANDLER, None);

    let sender = sender.clone();
    manager.connect_script_message_received(Some(MESSAGE_HANDLER), move |_, value| {
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
        match self.sync.handle(self.settings.sync_scroll, msg) {
            Some(ScrollTarget::Preview(f)) => {
                self.web_view.evaluate_javascript(
                    &preview_scroll_script(f),
                    None,
                    None,
                    None::<&gio::Cancellable>,
                    |_| {},
                );
            }
            Some(ScrollTarget::Editor(f)) => {
                let adj = &self.editor_vert_adj;
                adj.set_value(value_for_fraction(f, adj.upper(), adj.page_size()));
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // Enabled-sync shorthands
    fn editor(sync: &mut ScrollSync, f: f64) -> Option<ScrollTarget> {
        sync.handle(true, &Msg::EditorScrolled(f))
    }

    fn preview(sync: &mut ScrollSync, f: f64) -> Option<ScrollTarget> {
        sync.handle(true, &Msg::PreviewScrolled(f))
    }

    // ---------------------------------------------------------------------
    // fraction_of
    // ---------------------------------------------------------------------

    #[test]
    fn fraction_of_top_middle_and_bottom() {
        // upper 2000, page 500 => scrollable range 1500
        assert!(approx(fraction_of(0.0, 2000.0, 500.0), 0.0));
        assert!(approx(fraction_of(750.0, 2000.0, 500.0), 0.5));
        assert!(approx(fraction_of(1500.0, 2000.0, 500.0), 1.0));
    }

    #[test]
    fn fraction_of_is_zero_when_nothing_can_scroll() {
        // Content fits the viewport (an empty or short document)
        assert!(approx(fraction_of(0.0, 500.0, 500.0), 0.0));
        assert!(approx(fraction_of(10.0, 500.0, 500.0), 0.0));
        // Page larger than content (negative range)
        assert!(approx(fraction_of(0.0, 300.0, 500.0), 0.0));
        // Initial `Adjustment::new(0, 0, 0, ...)`
        assert!(approx(fraction_of(0.0, 0.0, 0.0), 0.0));
    }

    #[test]
    fn fraction_of_clamps_out_of_range_values() {
        assert!(approx(fraction_of(9999.0, 2000.0, 500.0), 1.0));
        assert!(approx(fraction_of(-50.0, 2000.0, 500.0), 0.0));
    }

    // ---------------------------------------------------------------------
    // value_for_fraction
    // ---------------------------------------------------------------------

    #[test]
    fn value_for_fraction_top_middle_and_bottom() {
        assert!(approx(value_for_fraction(0.0, 2000.0, 500.0), 0.0));
        assert!(approx(value_for_fraction(0.5, 2000.0, 500.0), 750.0));
        assert!(approx(value_for_fraction(1.0, 2000.0, 500.0), 1500.0));
    }

    #[test]
    fn value_for_fraction_is_zero_when_nothing_can_scroll() {
        assert!(approx(value_for_fraction(0.7, 500.0, 500.0), 0.0));
        assert!(approx(value_for_fraction(0.7, 300.0, 500.0), 0.0));
        assert!(approx(value_for_fraction(0.7, 0.0, 0.0), 0.0));
    }

    #[test]
    fn value_for_fraction_clamps_the_fraction() {
        // The preview can report over-scroll (e.g. rubber-banding) outside 0.0..=1.0
        assert!(approx(value_for_fraction(1.5, 2000.0, 500.0), 1500.0));
        assert!(approx(value_for_fraction(-0.5, 2000.0, 500.0), 0.0));
    }

    #[test]
    fn fraction_and_value_round_trip() {
        for f in [0.0, 0.001, 0.25, 0.5, 0.999, 1.0] {
            let value = value_for_fraction(f, 4321.0, 612.0);
            assert!(
                approx(fraction_of(value, 4321.0, 612.0), f),
                "fraction {f} did not round-trip"
            );
        }
    }

    // ---------------------------------------------------------------------
    // preview_scroll_script / REPORT_SCRIPT
    // ---------------------------------------------------------------------

    #[test]
    fn preview_scroll_script_embeds_the_fraction() {
        assert_eq!(
            preview_scroll_script(0.5),
            "window.scrollTo(0, 0.5 * (document.documentElement.scrollHeight - window.innerHeight));"
        );
    }

    #[test]
    fn preview_scroll_script_formats_whole_numbers_as_valid_js() {
        assert!(preview_scroll_script(0.0).contains("scrollTo(0, 0 * ("));
        assert!(preview_scroll_script(1.0).contains("scrollTo(0, 1 * ("));
    }

    #[test]
    fn preview_scroll_script_never_uses_exponent_notation() {
        assert!(preview_scroll_script(1e-10).contains("scrollTo(0, 0.0000000001 * ("));
    }

    #[test]
    fn scripts_agree_on_the_scrollable_range_formula() {
        let range = "document.documentElement.scrollHeight - window.innerHeight";
        assert!(REPORT_SCRIPT.contains(range));
        assert!(preview_scroll_script(0.5).contains(range));
    }

    #[test]
    fn report_script_posts_to_the_registered_handler() {
        assert!(
            REPORT_SCRIPT.contains(&format!("messageHandlers.{MESSAGE_HANDLER}.postMessage(")),
            "script and handler name disagree"
        );
    }

    #[test]
    fn report_script_guards_against_zero_range_and_is_passive() {
        assert!(REPORT_SCRIPT.contains("range > 0 ?"));
        assert!(REPORT_SCRIPT.contains("passive: true"));
        assert!(REPORT_SCRIPT.contains("addEventListener('scroll'"));
    }

    // ---------------------------------------------------------------------
    // ScrollSync::handle: enabled / disabled / unrelated messages
    // ---------------------------------------------------------------------

    #[test]
    fn disabled_sync_ignores_scroll_messages_and_keeps_state() {
        let mut sync = ScrollSync::default();
        assert_eq!(sync.handle(false, &Msg::EditorScrolled(0.5)), None);
        assert_eq!(sync.handle(false, &Msg::PreviewScrolled(0.9)), None);
        assert!(approx(sync.position, 0.0));
    }

    #[test]
    fn sync_works_after_being_re_enabled() {
        let mut sync = ScrollSync::default();
        assert_eq!(sync.handle(false, &Msg::EditorScrolled(0.5)), None);
        assert_eq!(editor(&mut sync, 0.5), Some(ScrollTarget::Preview(0.5)));
    }

    #[test]
    fn unrelated_messages_are_ignored() {
        let mut sync = ScrollSync::default();
        for msg in [
            Msg::BufferChanged,
            Msg::PreviewLoaded,
            Msg::StyleChanged,
            Msg::Toast("hello".to_owned()),
        ] {
            assert_eq!(sync.handle(true, &msg), None, "{msg:?}");
        }
        assert!(approx(sync.position, 0.0));
    }

    // ---------------------------------------------------------------------
    // ScrollSync::handle: mirroring
    // ---------------------------------------------------------------------

    #[test]
    fn editor_scroll_moves_the_preview() {
        let mut sync = ScrollSync::default();
        assert_eq!(editor(&mut sync, 0.4), Some(ScrollTarget::Preview(0.4)));
    }

    #[test]
    fn preview_scroll_moves_the_editor() {
        let mut sync = ScrollSync::default();
        assert_eq!(preview(&mut sync, 0.4), Some(ScrollTarget::Editor(0.4)));
    }

    // ---------------------------------------------------------------------
    // ScrollSync::handle: echo suppression
    // ---------------------------------------------------------------------

    #[test]
    fn preview_echo_of_an_editor_scroll_is_ignored() {
        let mut sync = ScrollSync::default();
        assert!(editor(&mut sync, 0.5).is_some());
        assert_eq!(preview(&mut sync, 0.5), None);
    }

    #[test]
    fn editor_echo_of_a_preview_scroll_is_ignored() {
        let mut sync = ScrollSync::default();
        assert!(preview(&mut sync, 0.5).is_some());
        assert_eq!(editor(&mut sync, 0.5), None);
    }

    #[test]
    fn echoes_within_epsilon_are_ignored_but_larger_moves_are_not() {
        let mut sync = ScrollSync::default();
        assert!(editor(&mut sync, 0.5).is_some());

        // Rounding differences between GTK and WebKit are expected
        assert_eq!(preview(&mut sync, 0.5 + EPSILON / 2.0), None);
        assert_eq!(preview(&mut sync, 0.5 - EPSILON / 2.0), None);

        // A genuinely different position is a real scroll
        assert_eq!(
            preview(&mut sync, 0.5 + EPSILON * 2.0),
            Some(ScrollTarget::Editor(0.5 + EPSILON * 2.0))
        );
    }

    #[test]
    fn a_move_of_exactly_epsilon_is_still_an_echo() {
        // The threshold is strict (`>`), so only larger moves count
        let mut sync = ScrollSync::default();
        assert_eq!(editor(&mut sync, EPSILON), None);
    }

    #[test]
    fn tiny_steps_accumulate_until_they_exceed_epsilon() {
        // Ignored events do not move the baseline, so slow drift is eventually noticed
        let mut sync = ScrollSync::default();
        assert_eq!(editor(&mut sync, 0.001), None);
        assert_eq!(editor(&mut sync, 0.002), None);
        assert_eq!(editor(&mut sync, 0.003), Some(ScrollTarget::Preview(0.003)));
    }

    #[test]
    fn nan_is_ignored_and_does_not_poison_state() {
        let mut sync = ScrollSync::default();
        assert_eq!(editor(&mut sync, f64::NAN), None);
        assert_eq!(preview(&mut sync, f64::NAN), None);
        assert!(approx(sync.position, 0.0));
        assert_eq!(editor(&mut sync, 0.5), Some(ScrollTarget::Preview(0.5)));
    }

    #[test]
    fn editor_returning_to_the_top_moves_the_preview_back() {
        let mut sync = ScrollSync::default();
        assert_eq!(editor(&mut sync, 0.5), Some(ScrollTarget::Preview(0.5)));
        assert_eq!(preview(&mut sync, 0.5), None); // echo

        // e.g. Ctrl+Home in the editor
        assert_eq!(editor(&mut sync, 0.0), Some(ScrollTarget::Preview(0.0)));
    }

    #[test]
    fn preview_returning_to_the_top_moves_the_editor_back() {
        let mut sync = ScrollSync::default();
        assert_eq!(preview(&mut sync, 0.5), Some(ScrollTarget::Editor(0.5)));
        assert_eq!(editor(&mut sync, 0.5), None); // echo

        assert_eq!(preview(&mut sync, 0.0), Some(ScrollTarget::Editor(0.0)));
    }

    #[test]
    fn jumping_to_the_bottom_and_back_to_the_top_stays_in_sync() {
        let mut sync = ScrollSync::default();
        assert_eq!(editor(&mut sync, 1.0), Some(ScrollTarget::Preview(1.0)));
        assert_eq!(preview(&mut sync, 1.0), None);
        assert_eq!(editor(&mut sync, 0.0), Some(ScrollTarget::Preview(0.0)));
        assert_eq!(preview(&mut sync, 0.0), None);
    }

    #[test]
    fn smooth_scrolling_down_and_back_up_mirrors_every_step() {
        let mut sync = ScrollSync::default();
        let steps: Vec<f64> = (1..=10)
            .chain((0..10).rev())
            .map(|i| f64::from(i) / 10.0)
            .collect();

        for f in steps {
            assert_eq!(
                editor(&mut sync, f),
                Some(ScrollTarget::Preview(f)),
                "editor step to {f}"
            );
            assert_eq!(preview(&mut sync, f), None, "echo at {f}");
        }
    }

    #[test]
    fn alternating_panes_keeps_both_directions_working() {
        let mut sync = ScrollSync::default();

        assert_eq!(editor(&mut sync, 0.3), Some(ScrollTarget::Preview(0.3)));
        assert_eq!(preview(&mut sync, 0.3), None);

        assert_eq!(preview(&mut sync, 0.6), Some(ScrollTarget::Editor(0.6)));
        assert_eq!(editor(&mut sync, 0.6), None);

        // Back to where the editor last was driven by the user
        assert_eq!(editor(&mut sync, 0.3), Some(ScrollTarget::Preview(0.3)));
        assert_eq!(preview(&mut sync, 0.3), None);

        assert_eq!(preview(&mut sync, 0.0), Some(ScrollTarget::Editor(0.0)));
    }
}

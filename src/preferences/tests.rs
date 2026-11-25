//! Tests for the pure logic behind the Preferences dialog

use super::*;
use crate::settings::WindowSettings;

type TestResult = Result<(), Box<dyn std::error::Error>>;

// A "default custom color", deliberately unlike the real defaults, so tests prove the
// *parameter* is used rather than some hard-coded value
const DEFAULT: &str = "rgb(1,2,3)";

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

fn assert_rgb(color: &gdk::RGBA, (r, g, b): (f32, f32, f32)) {
    assert!(
        approx(color.red(), r) && approx(color.green(), g) && approx(color.blue(), b),
        "got {color:?}, expected rgb({r}, {g}, {b})"
    );
}

fn with_backgrounds(editor: Option<&str>, preview: Option<&str>) -> Settings {
    Settings {
        editor_background: editor.map(str::to_owned),
        preview_background: preview.map(str::to_owned),
        ..Settings::default()
    }
}

// -------------------------------------------------------------------------
// toggle
// -------------------------------------------------------------------------

#[test]
fn toggle_truth_table() {
    // (current, on, expected)
    let cases = [
        (None, true, Some(DEFAULT)),
        (Some("#abcdef"), true, Some("#abcdef")),
        (None, false, None),
        (Some("#abcdef"), false, None),
    ];
    for (current, on, expected) in cases {
        let result = toggle(current.map(str::to_owned), on, DEFAULT);
        assert_eq!(
            result.as_deref(),
            expected,
            "current = {current:?}, on = {on}"
        );
    }
}

#[test]
fn toggle_on_keeps_existing_color_even_if_it_differs_from_default() {
    let result = toggle(Some("#123456".to_owned()), true, DEFAULT);
    assert_eq!(result.as_deref(), Some("#123456"));
}

// -------------------------------------------------------------------------
// color_or_default
// -------------------------------------------------------------------------

#[test]
fn color_or_default_uses_valid_css() {
    let color = color_or_default(Some("#ff0000"), "#00ff00");
    assert_rgb(&color, (1.0, 0.0, 0.0));
}

#[test]
fn color_or_default_falls_back_to_default_for_none() {
    let color = color_or_default(None, "#00ff00");
    assert_rgb(&color, (0.0, 1.0, 0.0));
}

#[test]
fn color_or_default_falls_back_to_default_for_invalid_css() {
    for bad in ["", "not-a-color", "#12"] {
        let color = color_or_default(Some(bad), "#00ff00");
        assert_rgb(&color, (0.0, 1.0, 0.0));
    }
}

#[test]
fn color_or_default_falls_back_to_white_when_everything_is_invalid() {
    assert_eq!(color_or_default(None, "garbage"), gdk::RGBA::WHITE);
    assert_eq!(
        color_or_default(Some("garbage"), "also garbage"),
        gdk::RGBA::WHITE
    );
}

// -------------------------------------------------------------------------
// apply: simple fields
// -------------------------------------------------------------------------

#[test]
fn apply_view_mode_changes_only_view_mode() {
    let current = Settings::default();
    let next = apply(&current, PrefsMsg::ViewMode(ViewMode::EditorOnly), DEFAULT);

    assert_eq!(next.view_mode, ViewMode::EditorOnly);
    assert_eq!(
        next,
        Settings {
            view_mode: ViewMode::EditorOnly,
            ..current
        }
    );
}

#[test]
fn apply_scheme_changes_only_color_scheme() {
    let current = Settings::default();
    let next = apply(&current, PrefsMsg::Scheme(ColorScheme::Dark), DEFAULT);

    assert_eq!(next.color_scheme, ColorScheme::Dark);
    assert_eq!(
        next,
        Settings {
            color_scheme: ColorScheme::Dark,
            ..current
        }
    );
}

#[test]
fn apply_preserves_window_settings() {
    let current = Settings {
        window: WindowSettings {
            width: 1234,
            height: 567,
            maximized: true,
            fullscreened: false,
        },
        ..Settings::default()
    };

    for msg in [
        PrefsMsg::ViewMode(ViewMode::PreviewOnly),
        PrefsMsg::Scheme(ColorScheme::Light),
        PrefsMsg::EditorCustom(true),
        PrefsMsg::PreviewCustom(true),
    ] {
        let next = apply(&current, msg, DEFAULT);
        assert_eq!(next.window, current.window);
    }
}

#[test]
fn apply_same_value_produces_equal_settings() {
    let current = Settings {
        view_mode: ViewMode::PreviewOnly,
        color_scheme: ColorScheme::Dark,
        ..Settings::default()
    };

    assert_eq!(
        apply(&current, PrefsMsg::ViewMode(ViewMode::PreviewOnly), DEFAULT),
        current
    );
    assert_eq!(
        apply(&current, PrefsMsg::Scheme(ColorScheme::Dark), DEFAULT),
        current
    );
    assert_eq!(
        apply(&current, PrefsMsg::EditorCustom(false), DEFAULT),
        current
    );
    assert_eq!(
        apply(&current, PrefsMsg::PreviewCustom(false), DEFAULT),
        current
    );
}

// -------------------------------------------------------------------------
// apply: custom backgrounds
// -------------------------------------------------------------------------

#[test]
fn apply_editor_custom_on_starts_from_default_color() {
    let next = apply(&Settings::default(), PrefsMsg::EditorCustom(true), DEFAULT);
    assert_eq!(next.editor_background.as_deref(), Some(DEFAULT));
    assert_eq!(next.preview_background, None);
}

#[test]
fn apply_preview_custom_on_starts_from_default_color() {
    let next = apply(&Settings::default(), PrefsMsg::PreviewCustom(true), DEFAULT);
    assert_eq!(next.preview_background.as_deref(), Some(DEFAULT));
    assert_eq!(next.editor_background, None);
}

#[test]
fn apply_custom_off_clears_only_that_background() {
    let current = with_backgrounds(Some("#111111"), Some("#222222"));

    let next = apply(&current, PrefsMsg::EditorCustom(false), DEFAULT);
    assert_eq!(next.editor_background, None);
    assert_eq!(next.preview_background.as_deref(), Some("#222222"));

    let next = apply(&current, PrefsMsg::PreviewCustom(false), DEFAULT);
    assert_eq!(next.editor_background.as_deref(), Some("#111111"));
    assert_eq!(next.preview_background, None);
}

#[test]
fn apply_custom_on_when_already_on_keeps_current_color() {
    let current = with_backgrounds(Some("#111111"), Some("#222222"));
    assert_eq!(
        apply(&current, PrefsMsg::EditorCustom(true), DEFAULT),
        current
    );
    assert_eq!(
        apply(&current, PrefsMsg::PreviewCustom(true), DEFAULT),
        current
    );
}

#[test]
fn apply_editor_color_updates_only_when_override_is_on() -> TestResult {
    let picked = gdk::RGBA::new(0.2, 0.4, 0.6, 1.0);
    let current = with_backgrounds(Some(DEFAULT), Some("#222222"));

    let next = apply(&current, PrefsMsg::EditorColor(picked), DEFAULT);
    let stored = next
        .editor_background
        .as_deref()
        .ok_or("override must stay on")?;
    let parsed = parse_color(stored).ok_or("stored color must be parseable")?;

    // `to_str()` rounds to 8 bits per channel, so allow half a step of error
    assert!((parsed.red() - 0.2).abs() < 0.005, "got {stored}");
    assert!((parsed.green() - 0.4).abs() < 0.005, "got {stored}");
    assert!((parsed.blue() - 0.6).abs() < 0.005, "got {stored}");
    assert_eq!(next.preview_background.as_deref(), Some("#222222"));
    Ok(())
}

#[test]
fn apply_preview_color_updates_only_when_override_is_on() -> TestResult {
    let picked = gdk::RGBA::new(1.0, 0.0, 0.0, 1.0);
    let current = with_backgrounds(Some("#111111"), Some(DEFAULT));

    let next = apply(&current, PrefsMsg::PreviewColor(picked), DEFAULT);
    let stored = next
        .preview_background
        .as_deref()
        .ok_or("override must stay on")?;
    let parsed = parse_color(stored).ok_or("stored color must be parseable")?;

    assert_rgb(&parsed, (1.0, 0.0, 0.0));
    assert_eq!(next.editor_background.as_deref(), Some("#111111"));
    Ok(())
}

#[test]
fn apply_color_is_ignored_while_override_is_off() {
    let current = Settings::default();
    let picked = gdk::RGBA::new(0.1, 0.2, 0.3, 1.0);

    assert_eq!(
        apply(&current, PrefsMsg::EditorColor(picked), DEFAULT),
        current
    );
    assert_eq!(
        apply(&current, PrefsMsg::PreviewColor(picked), DEFAULT),
        current
    );
}

#[test]
fn apply_color_notification_for_unchanged_color_does_not_change_settings() -> TestResult {
    for default in ["rgb(36,36,36)", "rgb(255,255,255)"] {
        let enabled = apply(&Settings::default(), PrefsMsg::EditorCustom(true), default);
        let echoed = parse_color(default).ok_or("default must parse")?;

        let after = apply(&enabled, PrefsMsg::EditorColor(echoed), default);
        assert_eq!(after, enabled, "echo of {default} changed the settings");
    }
    Ok(())
}

// -------------------------------------------------------------------------
// apply: Sync
// -------------------------------------------------------------------------

#[test]
fn apply_sync_replaces_all_settings() {
    let current = Settings::default();
    let incoming = Settings {
        view_mode: ViewMode::EditorOnly,
        color_scheme: ColorScheme::Light,
        editor_background: Some("#abcdef".to_owned()),
        preview_background: Some("#fedcba".to_owned()),
        sync_scroll: true,
        window: WindowSettings {
            width: 900,
            height: 700,
            maximized: true,
            fullscreened: true,
        },
    };

    assert_eq!(
        apply(&current, PrefsMsg::Sync(incoming.clone()), DEFAULT),
        incoming
    );
}

#[test]
fn apply_does_not_modify_its_input() {
    let current = with_backgrounds(Some("#111111"), None);
    let snapshot = current.clone();

    let _ = apply(&current, PrefsMsg::EditorCustom(false), DEFAULT);
    assert_eq!(current, snapshot);
}

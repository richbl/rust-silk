//! Tests for the pure helpers in `app.rs`

use super::*;
use crate::render::markdown_to_html;

// -------------------------------------------------------------------------
// display_name
// -------------------------------------------------------------------------

#[test]
fn display_name_without_a_file_is_untitled() {
    assert_eq!(display_name(None), "Untitled Document");
    assert_eq!(display_name(None), UNTITLED);
}

#[test]
fn display_name_uses_the_base_name() {
    assert_eq!(display_name(Some(Path::new("notes.md"))), "notes.md");
}

#[test]
fn display_name_keeps_spaces_and_unicode() {
    assert_eq!(
        display_name(Some(Path::new("my notes – été.md"))),
        "my notes – été.md"
    );
}

#[cfg(unix)]
#[test]
fn display_name_is_lossy_for_non_utf8_names() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let path = Path::new(OsStr::from_bytes(b"bad\xff.md"));
    let name = display_name(Some(path));
    assert!(name.starts_with("bad"), "got: {name}");
    assert!(name.contains('\u{FFFD}'), "got: {name}");
    assert!(
        std::path::Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md")),
        "got: {name}"
    );
}

// -------------------------------------------------------------------------
// window_title
// -------------------------------------------------------------------------

#[test]
fn window_title_is_the_bare_name_when_clean() {
    assert_eq!(window_title("notes.md", false), "notes.md");
}

#[test]
fn window_title_gets_a_bullet_when_dirty() {
    assert_eq!(window_title("notes.md", true), "• notes.md");
}

#[test]
fn window_title_works_for_untitled_documents() {
    assert_eq!(window_title(UNTITLED, false), "Untitled Document");
    assert_eq!(window_title(UNTITLED, true), "• Untitled Document");
}

// -------------------------------------------------------------------------
// save_enabled
// -------------------------------------------------------------------------

#[test]
fn save_enabled_truth_table() {
    // (dirty, has_file, expected)
    let cases = [
        (false, false, true), // brand new document: Save opens the Save As dialog
        (true, false, true),
        (true, true, true),
        (false, true, false), // nothing to save
    ];
    for (dirty, has_file, expected) in cases {
        assert_eq!(
            save_enabled(dirty, has_file),
            expected,
            "dirty = {dirty}, has_file = {has_file}"
        );
    }
}

// -------------------------------------------------------------------------
// welcome_text
// -------------------------------------------------------------------------

#[test]
fn welcome_text_mentions_app_name_version_and_tagline() {
    let text = welcome_text();
    assert!(text.contains(APP_NAME), "got: {text}");
    assert!(text.contains(APP_VER), "got: {text}");
    assert!(text.contains(ABOUT), "got: {text}");
}

#[test]
fn welcome_text_starts_with_a_title_and_ends_with_a_newline() {
    let text = welcome_text();
    assert!(
        text.starts_with(&format!("# Welcome to {APP_NAME} {APP_VER}\n")),
        "got: {text}"
    );
    assert!(text.ends_with('\n'));
}

#[test]
fn welcome_text_renders_as_expected_markdown() {
    let html = markdown_to_html(&welcome_text());

    assert!(
        html.contains(&format!("<h1>Welcome to {APP_NAME} {APP_VER}</h1>")),
        "got: {html}"
    );
    assert!(html.contains(&format!("<h2>{ABOUT}</h2>")), "got: {html}");
    assert!(html.contains("<strong>Markdown</strong>"), "got: {html}");
    assert!(html.contains("<em>Preferences</em>"), "got: {html}");
    assert_eq!(html.matches("<li>").count(), 3, "got: {html}");
}

#[test]
fn welcome_text_is_stable_across_calls() {
    assert_eq!(welcome_text(), welcome_text());
}

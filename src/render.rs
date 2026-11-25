//! Markdown → HTML, and the static shell page the preview `WebView` hosts
//!
//! The shell is loaded once (and again only when appearance settings change).
//! Subsequent edits only replace `#content`'s innerHTML, which avoids the flash
//! and scroll reset caused by calling `load_html` on every keystroke.

use pulldown_cmark::{Options, Parser, html};

use crate::settings::{ColorScheme, is_light, parse_color};

/// Converts a markdown string into an HTML string with common extensions enabled
pub fn markdown_to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;

    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut out, Parser::new_ext(markdown, options));
    out
}

// The CSP blocks scripts, remote styles, frames and network access inside the previewed document
// (raw HTML in Markdown is passed through by the pulldown-cmark crate logic)
const SHELL: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy"
      content="default-src 'none'; img-src https: data:; style-src 'unsafe-inline'">
<style>
  :root { color-scheme: __SCHEME__; }
  body {
    __THEME__
    font-family: system-ui, sans-serif;
    line-height: 1.6;
    margin: 0;
    padding: 24px;
  }
  main { max-width: 80ch; margin: 0 auto; }
  img { max-width: 100%; }
  pre, code { font-family: ui-monospace, monospace; background: rgba(127,127,127,.15); border-radius: 4px; }
  pre { padding: 12px; overflow-x: auto; }
  code { padding: 2px 4px; }
  pre code { padding: 0; background: none; }
  blockquote { border-left: 4px solid rgba(127,127,127,.4); margin-left: 0; padding-left: 16px; }
  table { border-collapse: collapse; }
  th, td { border: 1px solid rgba(127,127,127,.4); padding: 4px 10px; }
  hr { border: 0; border-top: 1px solid rgba(127,127,127,.4); }
</style>
</head>
<body><main id="content"></main></body>
</html>"#;

/// Generates the full HTML shell for the markdown previewer, injecting color schemes and background
/// styles. Note that `background == None` follows the system style (or the forced light/dark scheme)
pub fn shell_html(scheme: ColorScheme, background: Option<&str>) -> String {
    let custom = background.and_then(|css| parse_color(css).map(|rgba| (css, rgba)));

    let (color_scheme, theme) = match custom {
        Some((css, rgba)) => {
            let light = is_light(&rgba);
            (
                if light { "light" } else { "dark" },
                format!(
                    "background: {css}; color: {};",
                    if light { "#241f31" } else { "#ffffff" }
                ),
            )
        }
        None => (
            match scheme {
                ColorScheme::System => "light dark",
                ColorScheme::Light => "light",
                ColorScheme::Dark => "dark",
            },
            "background: Canvas; color: CanvasText;".to_owned(),
        ),
    };

    SHELL
        .replace("__SCHEME__", color_scheme)
        .replace("__THEME__", &theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::readable_foreground;

    // ---------------------------------------------------------------------
    // markdown_to_html
    // ---------------------------------------------------------------------

    #[test]
    fn markdown_empty_input_yields_empty_output() {
        assert_eq!(markdown_to_html(""), "");
    }

    #[test]
    fn markdown_renders_heading() {
        assert_eq!(markdown_to_html("# Title"), "<h1>Title</h1>\n");
    }

    #[test]
    fn markdown_renders_heading_levels() {
        for level in 1..=6 {
            let html = markdown_to_html(&format!("{} Title", "#".repeat(level)));
            assert_eq!(html, format!("<h{level}>Title</h{level}>\n"));
        }
    }

    #[test]
    fn markdown_renders_paragraphs_and_inline_emphasis() {
        assert_eq!(
            markdown_to_html("**bold** and *italic*"),
            "<p><strong>bold</strong> and <em>italic</em></p>\n"
        );
        assert_eq!(markdown_to_html("a\n\nb").matches("<p>").count(), 2);
    }

    #[test]
    fn markdown_renders_links() {
        let html = markdown_to_html("[Silk](https://example.com)");
        assert!(
            html.contains(r#"<a href="https://example.com">Silk</a>"#),
            "got: {html}"
        );
    }

    #[test]
    fn markdown_renders_fenced_code_block_with_language() {
        let html = markdown_to_html("```rust\nfn main() {}\n```");
        assert!(
            html.contains(r#"<pre><code class="language-rust">fn main() {}"#),
            "got: {html}"
        );
    }

    #[test]
    fn markdown_escapes_special_characters_in_text() {
        let html = markdown_to_html("a < b & c");
        assert!(html.contains("&lt;"), "got: {html}");
        assert!(html.contains("&amp;"), "got: {html}");
    }

    #[test]
    fn markdown_preserves_unicode() {
        let html = markdown_to_html("héllo wörld");
        assert!(html.contains("héllo wörld"), "got: {html}");
    }

    #[test]
    fn markdown_extension_tables() {
        let html = markdown_to_html("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(html.contains("<table>"), "got: {html}");
        assert!(html.contains("<th>a</th>"), "got: {html}");
        assert!(html.contains("<td>2</td>"), "got: {html}");
    }

    #[test]
    fn markdown_extension_strikethrough() {
        assert_eq!(markdown_to_html("~~gone~~"), "<p><del>gone</del></p>\n");
    }

    #[test]
    fn markdown_extension_task_lists() {
        let html = markdown_to_html("- [x] done\n- [ ] todo\n");
        assert_eq!(html.matches(r#"type="checkbox""#).count(), 2, "got: {html}");
        assert_eq!(html.matches("checked").count(), 1, "got: {html}");
    }

    #[test]
    fn markdown_extension_footnotes() {
        let html = markdown_to_html("Text[^1]\n\n[^1]: The note\n");
        assert!(html.contains("footnote-reference"), "got: {html}");
        assert!(html.contains("footnote-definition"), "got: {html}");
    }

    #[test]
    fn markdown_passes_raw_html_through_unescaped() {
        let html = markdown_to_html("<script>alert(1)</script>");
        assert!(html.contains("<script>alert(1)</script>"), "got: {html}");
    }

    // ---------------------------------------------------------------------
    // shell_html: structure and safety
    // ---------------------------------------------------------------------

    #[test]
    fn shell_is_a_complete_html_document() {
        let html = shell_html(ColorScheme::System, None);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.ends_with("</html>"));
    }

    #[test]
    fn shell_has_the_content_container_the_preview_targets() {
        let html = shell_html(ColorScheme::System, None);
        assert!(html.contains(r#"<main id="content">"#), "got: {html}");
    }

    #[test]
    fn shell_has_no_unreplaced_placeholders() {
        let backgrounds = [None, Some("#ffffff"), Some("#000000"), Some("not-a-color")];
        for scheme in ColorScheme::ALL {
            for bg in backgrounds {
                let html = shell_html(scheme, bg);
                assert!(!html.contains("__SCHEME__"), "{scheme:?} / {bg:?}");
                assert!(!html.contains("__THEME__"), "{scheme:?} / {bg:?}");
            }
        }
    }

    #[test]
    fn shell_csp_blocks_everything_by_default() {
        let html = shell_html(ColorScheme::System, None);
        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("default-src 'none'"));
        assert!(!html.contains("script-src"));
        assert!(!html.contains("unsafe-eval"));
    }

    #[test]
    fn shell_csp_is_identical_for_every_appearance() {
        let csp = |html: &str| -> String {
            let meta = html
                .find("Content-Security-Policy")
                .expect("CSP meta tag present");
            let attr = html[meta..]
                .find("content=\"")
                .map(|i| meta + i + "content=\"".len())
                .expect("CSP content attribute present");
            let len = html[attr..]
                .find('"')
                .expect("CSP content attribute closed");
            html[attr..attr + len].to_owned()
        };

        let baseline = csp(&shell_html(ColorScheme::System, None));
        assert!(baseline.contains("default-src 'none'"), "got: {baseline}");

        for scheme in ColorScheme::ALL {
            for bg in [None, Some("#123456"), Some("junk")] {
                assert_eq!(
                    csp(&shell_html(scheme, bg)),
                    baseline,
                    "{scheme:?} / {bg:?}"
                );
            }
        }
    }

    #[test]
    fn shell_is_deterministic() {
        assert_eq!(
            shell_html(ColorScheme::Dark, Some("#336699")),
            shell_html(ColorScheme::Dark, Some("#336699"))
        );
    }

    // ---------------------------------------------------------------------
    // shell_html: no custom background (follows the scheme)
    // ---------------------------------------------------------------------

    #[test]
    fn shell_without_background_follows_color_scheme() {
        let cases = [
            (ColorScheme::System, "color-scheme: light dark;"),
            (ColorScheme::Light, "color-scheme: light;"),
            (ColorScheme::Dark, "color-scheme: dark;"),
        ];
        for (scheme, expected) in cases {
            let html = shell_html(scheme, None);
            assert!(html.contains(expected), "{scheme:?}: {html}");
            assert!(
                html.contains("background: Canvas; color: CanvasText;"),
                "{scheme:?}: {html}"
            );
        }
    }

    // ---------------------------------------------------------------------
    // shell_html: custom background
    // ---------------------------------------------------------------------

    #[test]
    fn shell_light_custom_background_uses_dark_text() {
        let html = shell_html(ColorScheme::System, Some("#ffffff"));
        assert!(html.contains("color-scheme: light;"), "got: {html}");
        assert!(
            html.contains("background: #ffffff; color: #241f31;"),
            "got: {html}"
        );
    }

    #[test]
    fn shell_dark_custom_background_uses_light_text() {
        let html = shell_html(ColorScheme::System, Some("#000000"));
        assert!(html.contains("color-scheme: dark;"), "got: {html}");
        assert!(
            html.contains("background: #000000; color: #ffffff;"),
            "got: {html}"
        );
    }

    #[test]
    fn shell_custom_background_overrides_scheme() {
        // The background's brightness wins over a forced scheme
        let html = shell_html(ColorScheme::Dark, Some("#ffffff"));
        assert!(html.contains("color-scheme: light;"), "got: {html}");

        let html = shell_html(ColorScheme::Light, Some("#000000"));
        assert!(html.contains("color-scheme: dark;"), "got: {html}");
    }

    #[test]
    fn shell_custom_background_is_inserted_verbatim() {
        let html = shell_html(ColorScheme::System, Some("rgb(10,20,30)"));
        assert!(html.contains("background: rgb(10,20,30);"), "got: {html}");
    }

    #[test]
    fn shell_foreground_agrees_with_readable_foreground() -> Result<(), Box<dyn std::error::Error>>
    {
        // The shell hardcodes its foregrounds; this guards against drifting from the helper
        for css in [
            "#ffffff",
            "#000000",
            "#808080",
            "#336699",
            "#ffcc00",
            "rgb(36,36,36)",
        ] {
            let rgba = parse_color(css).ok_or("test color must parse")?;
            let html = shell_html(ColorScheme::System, Some(css));
            let expected = format!("background: {css}; color: {};", readable_foreground(&rgba));
            assert!(html.contains(&expected), "{css}: {html}");
        }
        Ok(())
    }

    #[test]
    fn shell_invalid_background_falls_back_to_scheme() {
        for bad in ["", "not-a-color", "#12"] {
            let html = shell_html(ColorScheme::Dark, Some(bad));
            assert!(html.contains("color-scheme: dark;"), "{bad:?}: {html}");
            assert!(
                html.contains("background: Canvas; color: CanvasText;"),
                "{bad:?}: {html}"
            );
        }
    }

    #[test]
    fn shell_rejects_css_injection_via_background() {
        // Anything that isn't a valid color must never reach the style-sheet
        let evil = "red; } body { display: none";
        let html = shell_html(ColorScheme::System, Some(evil));
        assert!(!html.contains("display: none"), "got: {html}");
        assert!(html.contains("background: Canvas; color: CanvasText;"));
    }
}

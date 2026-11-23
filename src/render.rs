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

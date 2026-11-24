//! User preferences: persistence and color helpers
//!
//! Stored as TOML under `$XDG_CONFIG_HOME/silk/config.toml`

use std::{fs, io, path::PathBuf};

use crate::constants::{APP_VER, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use relm4::adw;
use relm4::gtk::{gdk, glib};
use serde::{Deserialize, Serialize};

const CONFIG_HEADER: &str = "# Silk Configuration (TOML)\n# ";

/// Persisted window geometry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    pub fullscreened: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            maximized: false,
            fullscreened: false,
        }
    }
}

impl WindowSettings {
    const MIN_WIDTH: i32 = 400;
    const MIN_HEIGHT: i32 = 300;

    // Guard against hand-edited or corrupt values
    fn sanitized(mut self) -> Self {
        if self.width < Self::MIN_WIDTH {
            self.width = DEFAULT_WIDTH;
        }
        if self.height < Self::MIN_HEIGHT {
            self.height = DEFAULT_HEIGHT;
        }
        self
    }
}

/// User preferences view configuration structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewMode {
    EditorOnly,
    PreviewOnly,
    #[default]
    Both,
}

impl ViewMode {
    pub const ALL: [Self; 3] = [Self::EditorOnly, Self::PreviewOnly, Self::Both];
    pub const LABELS: [&'static str; 3] = ["Editor Only", "Preview Only", "Editor and Preview"];

    /// Get the index of the current view mode
    pub fn index(self) -> u32 {
        match self {
            Self::EditorOnly => 0,
            Self::PreviewOnly => 1,
            Self::Both => 2,
        }
    }

    /// Get `ViewMode` from index
    pub fn from_index(index: u32) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }

    /// Are we in editor mode?
    pub fn shows_editor(self) -> bool {
        self != Self::PreviewOnly
    }

    /// Are we in preview mode?
    pub fn shows_preview(self) -> bool {
        self != Self::EditorOnly
    }
}

/// Color scheme preference configuration structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorScheme {
    #[default]
    System,
    Light,
    Dark,
}

impl ColorScheme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];
    pub const LABELS: [&'static str; 3] = ["Follow System", "Light", "Dark"];

    /// Get the index of the current color scheme
    pub fn index(self) -> u32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }
    /// Get `ColorScheme` from index
    pub fn from_index(index: u32) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }

    /// Set the color scheme
    pub fn to_adw(self) -> adw::ColorScheme {
        match self {
            Self::System => adw::ColorScheme::Default,
            Self::Light => adw::ColorScheme::ForceLight,
            Self::Dark => adw::ColorScheme::ForceDark,
        }
    }
}

/// Settings elements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub view_mode: ViewMode,
    pub color_scheme: ColorScheme,
    pub editor_background: Option<String>,
    pub preview_background: Option<String>,
    pub sync_scroll: bool,
    pub window: WindowSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::default(),
            color_scheme: ColorScheme::default(),
            editor_background: None,
            preview_background: None,
            sync_scroll: true,
            window: WindowSettings::default(),
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        glib::user_config_dir().join("silk").join("config.toml")
    }

    /// Load settings from file or return default values
    pub fn load() -> Self {
        let mut settings: Self = match fs::read_to_string(Self::path()) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                eprintln!("silk: ignoring malformed settings: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        };
        settings.window = settings.window.sanitized();
        settings
    }

    /// Save settings atomically: write a sibling temp file, then rename over the config
    pub fn save(&self) -> io::Result<()> {
        use std::io::Write;

        let path = Self::path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let text = format!(
            "{CONFIG_HEADER}{APP_VER}\n\n{}",
            toml::to_string_pretty(self).map_err(io::Error::other)?
        );

        let tmp = path.with_extension("toml.tmp");
        let result = (|| {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&tmp, &path)
        })();

        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

/// Parse a color string into a GDK RGBA color
pub fn parse_color(css: &str) -> Option<gdk::RGBA> {
    gdk::RGBA::parse(css).ok()
}

/// Sensible starting color when the user enables a custom background
pub fn default_custom_color() -> String {
    if adw::StyleManager::default().is_dark() {
        "rgb(36,36,36)".into()
    } else {
        "rgb(255,255,255)".into()
    }
}

fn linearize(c: f32) -> f32 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance used to determine the perceived brightness of a color
pub fn is_light(color: &gdk::RGBA) -> bool {
    let l = 0.2126 * linearize(color.red())
        + 0.7152 * linearize(color.green())
        + 0.0722 * linearize(color.blue());
    l > 0.179
}

/// Pick a foreground that stays readable on an arbitrary custom background
pub fn readable_foreground(background: &gdk::RGBA) -> &'static str {
    if is_light(background) {
        "#241f31"
    } else {
        "#ffffff"
    }
}

/// Editor/preview base colors for the resolved light or dark style: (background, foreground)
pub fn scheme_palette(dark: bool) -> (&'static str, &'static str) {
    if dark {
        ("#242424", "#ffffff")
    } else {
        ("#ffffff", "#241f31")
    }
}

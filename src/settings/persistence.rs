//! Settings persistence: the `Settings` struct and its TOML load/save

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use relm4::gtk::glib;
use serde::{Deserialize, Serialize};

use super::{ColorScheme, ViewMode, WindowSettings};
use crate::constants::APP_VER;

const CONFIG_HEADER: &str = "# Silk Configuration (TOML)\n# ";

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
    /// Location of the config file on this system
    fn path() -> PathBuf {
        glib::user_config_dir().join("silk").join("config.toml")
    }

    /// Load settings from the config file or return default values
    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    /// Save settings atomically to the config file
    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Self::path())
    }

    // Load settings from a given path
    fn load_from(path: &Path) -> Self {
        let mut settings: Self = match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                eprintln!("silk: ignoring malformed settings: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        };
        settings.window = settings.window.sanitized();
        settings
    }

    // Text formatting (header + TOML) separated from the file I/O around it
    fn to_config_text(&self) -> io::Result<String> {
        let body = toml::to_string_pretty(self).map_err(io::Error::other)?;
        Ok(format!("{CONFIG_HEADER}{APP_VER}\n\n{body}"))
    }

    // Write a sibling temp file, then rename it over `path` so users never see a partial file
    fn save_to(&self, path: &Path) -> io::Result<()> {
        use std::io::Write;

        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let text = self.to_config_text()?;

        let tmp = path.with_extension("toml.tmp");
        let result = (|| {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&tmp, path)
        })();

        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

#[cfg(test)]
mod tests;

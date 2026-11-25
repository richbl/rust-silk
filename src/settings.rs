//! User preferences: persistence and color helpers
//!
//! Stored as TOML under `$XDG_CONFIG_HOME/silk/config.toml`
//!
//! Split by responsibility, with everything callers need re-exported here so existing
//! `crate::settings::...` paths keep working:
//!
//! - `window`: persisted window geometry
//! - `view`: pane layout and color scheme choices
//! - `persistence`: the `Settings` struct and its TOML load/save
//! - `color`: color parsing, luminance and palette helpers

mod color;
mod persistence;
mod view;
mod window;

pub use color::{default_custom_color, is_light, parse_color, readable_foreground, scheme_palette};
pub use persistence::Settings;
pub use view::{ColorScheme, ViewMode};
pub use window::WindowSettings;

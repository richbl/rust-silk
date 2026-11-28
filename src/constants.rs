//! Application-wide constants
//!

/// The name of the application
pub const APP_NAME: &str = "Silk";

/// One-line description of the application
pub const ABOUT: &str = "A minimal Markdown editor for GNOME";

/// The application ID used for GTK/GLib registration
pub const APP_ID: &str = "io.github.richbl.Silk";

/// The application version string (derived from `Cargo.toml`)
pub const APP_VER: &str = concat!("v", env!("CARGO_PKG_VERSION"));

/// The CLI name of the Silk executable
pub const APP_EXECUTABLE: &str = "silk-markdown-editor";

/// Default window width in pixels
pub const DEFAULT_WIDTH: i32 = 1200;

/// Default window height in pixels
pub const DEFAULT_HEIGHT: i32 = 800;

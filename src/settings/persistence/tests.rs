//! Tests for `Settings` serialization, loading and saving

use std::ffi::OsString;

use tempfile::TempDir;

use super::*;
use crate::constants::{DEFAULT_HEIGHT, DEFAULT_WIDTH};

type TestResult = Result<(), Box<dyn std::error::Error>>;

// A fresh temp dir plus a `config.toml` path inside it
fn temp_config() -> io::Result<(TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("config.toml");
    Ok((dir, path))
}

// Names of the entries directly inside `dir`
fn file_names(dir: &Path) -> io::Result<Vec<OsString>> {
    fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect()
}

// -------------------------------------------------------------------------
// Defaults and serde
// -------------------------------------------------------------------------

#[test]
fn default_values() {
    let s = Settings::default();
    assert_eq!(s.view_mode, ViewMode::Both);
    assert_eq!(s.color_scheme, ColorScheme::System);
    assert_eq!(s.editor_background, None);
    assert_eq!(s.preview_background, None);
    assert_eq!(s.window, WindowSettings::default());
}

#[test]
fn toml_round_trip_defaults() -> TestResult {
    let original = Settings::default();
    let text = toml::to_string_pretty(&original)?;
    let parsed: Settings = toml::from_str(&text)?;
    assert_eq!(parsed, original);
    Ok(())
}

#[test]
fn toml_round_trip_fully_populated() -> TestResult {
    let original = Settings {
        view_mode: ViewMode::PreviewOnly,
        color_scheme: ColorScheme::Dark,
        editor_background: Some("rgb(10,20,30)".to_owned()),
        preview_background: Some("#abcdef".to_owned()),
        sync_scroll: true,
        window: WindowSettings {
            width: 1024,
            height: 768,
            maximized: true,
            fullscreened: false,
        },
    };
    let text = toml::to_string_pretty(&original)?;
    let parsed: Settings = toml::from_str(&text)?;
    assert_eq!(parsed, original);
    Ok(())
}

#[test]
fn empty_toml_yields_defaults() -> TestResult {
    let parsed: Settings = toml::from_str("")?;
    assert_eq!(parsed, Settings::default());
    Ok(())
}

#[test]
fn partial_toml_fills_missing_fields() -> TestResult {
    let parsed: Settings = toml::from_str("color_scheme = \"dark\"")?;
    assert_eq!(parsed.color_scheme, ColorScheme::Dark);
    assert_eq!(parsed.view_mode, ViewMode::default());
    assert_eq!(parsed.window, WindowSettings::default());
    Ok(())
}

#[test]
fn partial_window_table_fills_missing_fields() -> TestResult {
    let parsed: Settings = toml::from_str("[window]\nwidth = 900\n")?;
    assert_eq!(parsed.window.width, 900);
    assert_eq!(parsed.window.height, DEFAULT_HEIGHT);
    assert!(!parsed.window.maximized);
    Ok(())
}

#[test]
fn enums_use_kebab_case() -> TestResult {
    let parsed: Settings = toml::from_str("view_mode = \"editor-only\"")?;
    assert_eq!(parsed.view_mode, ViewMode::EditorOnly);

    let parsed: Settings = toml::from_str("view_mode = \"preview-only\"")?;
    assert_eq!(parsed.view_mode, ViewMode::PreviewOnly);

    let text = toml::to_string_pretty(&Settings {
        view_mode: ViewMode::EditorOnly,
        ..Settings::default()
    })?;
    assert!(text.contains("view_mode = \"editor-only\""), "got: {text}");
    Ok(())
}

#[test]
fn unknown_fields_are_ignored() -> TestResult {
    let parsed: Settings = toml::from_str("future_option = true\nview_mode = \"both\"")?;
    assert_eq!(parsed, Settings::default());
    Ok(())
}

#[test]
fn invalid_enum_value_is_an_error() {
    assert!(toml::from_str::<Settings>("view_mode = \"sideways\"").is_err());
    assert!(toml::from_str::<Settings>("color_scheme = \"purple\"").is_err());
}

#[test]
fn wrong_type_is_an_error() {
    assert!(toml::from_str::<Settings>("window = 5").is_err());
    assert!(toml::from_str::<Settings>("[window]\nwidth = \"wide\"").is_err());
}

#[test]
fn malformed_toml_is_an_error() {
    assert!(toml::from_str::<Settings>("this is = = not toml").is_err());
}

// -------------------------------------------------------------------------
// to_config_text (pure)
// -------------------------------------------------------------------------

#[test]
fn to_config_text_starts_with_header_and_version() -> TestResult {
    let text = Settings::default().to_config_text()?;
    assert!(
        text.starts_with(&format!("{CONFIG_HEADER}{APP_VER}\n")),
        "got: {text}"
    );
    Ok(())
}

#[test]
fn to_config_text_is_valid_toml_that_round_trips() -> TestResult {
    let original = Settings {
        view_mode: ViewMode::EditorOnly,
        color_scheme: ColorScheme::Light,
        editor_background: Some("#123456".to_owned()),
        preview_background: None,
        sync_scroll: true,
        window: WindowSettings {
            width: 800,
            height: 600,
            maximized: true,
            fullscreened: false,
        },
    };
    let parsed: Settings = toml::from_str(&original.to_config_text()?)?;
    assert_eq!(parsed, original);
    Ok(())
}

// -------------------------------------------------------------------------
// load_from
// -------------------------------------------------------------------------

#[test]
fn load_from_missing_file_returns_defaults() -> TestResult {
    let (_dir, path) = temp_config()?;
    assert_eq!(Settings::load_from(&path), Settings::default());
    Ok(())
}

#[test]
fn load_from_empty_file_returns_defaults() -> TestResult {
    let (_dir, path) = temp_config()?;
    fs::write(&path, "")?;
    assert_eq!(Settings::load_from(&path), Settings::default());
    Ok(())
}

#[test]
fn load_from_valid_file_returns_stored_values() -> TestResult {
    let (_dir, path) = temp_config()?;
    fs::write(
        &path,
        "view_mode = \"preview-only\"\ncolor_scheme = \"dark\"\n\n[window]\nwidth = 900\nheight = 700\n",
    )?;

    let settings = Settings::load_from(&path);
    assert_eq!(settings.view_mode, ViewMode::PreviewOnly);
    assert_eq!(settings.color_scheme, ColorScheme::Dark);
    assert_eq!(settings.window.width, 900);
    assert_eq!(settings.window.height, 700);
    Ok(())
}

#[test]
fn load_from_malformed_file_returns_defaults() -> TestResult {
    let (_dir, path) = temp_config()?;
    fs::write(&path, "this is = = not toml")?;
    assert_eq!(Settings::load_from(&path), Settings::default());
    Ok(())
}

#[test]
fn load_from_invalid_enum_value_returns_defaults() -> TestResult {
    // One bad field discards the whole file (documents current behavior)
    let (_dir, path) = temp_config()?;
    fs::write(&path, "color_scheme = \"dark\"\nview_mode = \"sideways\"\n")?;
    assert_eq!(Settings::load_from(&path), Settings::default());
    Ok(())
}

#[test]
fn load_from_unreadable_path_returns_defaults() -> TestResult {
    // Reading a directory as a file is an I/O error, not a parse error
    let dir = tempfile::tempdir()?;
    assert_eq!(Settings::load_from(dir.path()), Settings::default());
    Ok(())
}

#[test]
fn load_from_sanitizes_window_geometry() -> TestResult {
    let (_dir, path) = temp_config()?;
    fs::write(
        &path,
        "[window]\nwidth = 10\nheight = -5\nmaximized = true\n",
    )?;

    let settings = Settings::load_from(&path);
    assert_eq!(settings.window.width, DEFAULT_WIDTH);
    assert_eq!(settings.window.height, DEFAULT_HEIGHT);
    assert!(settings.window.maximized);
    Ok(())
}

// -------------------------------------------------------------------------
// save_to
// -------------------------------------------------------------------------

#[test]
fn save_to_creates_missing_parent_directories() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("a").join("b").join("config.toml");

    Settings::default().save_to(&path)?;
    assert!(path.is_file());
    Ok(())
}

#[test]
fn save_to_writes_header_and_valid_toml() -> TestResult {
    let (_dir, path) = temp_config()?;

    Settings::default().save_to(&path)?;
    let text = fs::read_to_string(&path)?;
    assert!(text.starts_with(CONFIG_HEADER), "got: {text}");
    assert!(text.contains(APP_VER));
    toml::from_str::<Settings>(&text)?;
    Ok(())
}

#[test]
fn save_then_load_round_trips() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("silk").join("config.toml");
    let original = Settings {
        view_mode: ViewMode::PreviewOnly,
        color_scheme: ColorScheme::Dark,
        editor_background: Some("rgb(1,2,3)".to_owned()),
        preview_background: Some("#fedcba".to_owned()),
        sync_scroll: true,
        window: WindowSettings {
            width: 1111,
            height: 777,
            maximized: false,
            fullscreened: true,
        },
    };

    original.save_to(&path)?;
    assert_eq!(Settings::load_from(&path), original);
    Ok(())
}

#[test]
fn save_to_overwrites_existing_file() -> TestResult {
    let (_dir, path) = temp_config()?;

    Settings::default().save_to(&path)?;
    let updated = Settings {
        color_scheme: ColorScheme::Light,
        ..Settings::default()
    };
    updated.save_to(&path)?;

    assert_eq!(Settings::load_from(&path), updated);
    Ok(())
}

#[test]
fn save_to_leaves_no_temp_file_behind() -> TestResult {
    let (dir, path) = temp_config()?;
    Settings::default().save_to(&path)?;

    assert_eq!(file_names(dir.path())?, ["config.toml"]);
    Ok(())
}

#[test]
fn save_to_fails_when_parent_is_a_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "I am a file, not a directory")?;

    let result = Settings::default().save_to(&blocker.join("config.toml"));
    assert!(result.is_err());
    Ok(())
}

// Since renaming a file over an existing directory fails on Unix, we can test the error path and
// verify the temp file is cleaned up
#[cfg(unix)]
#[test]
fn save_to_cleans_up_temp_file_when_rename_fails() -> TestResult {
    let (dir, path) = temp_config()?;
    fs::create_dir(&path)?; // the "config file" is actually a directory

    let result = Settings::default().save_to(&path);
    assert!(result.is_err());
    assert!(path.is_dir(), "original directory must be untouched");
    assert_eq!(
        file_names(dir.path())?,
        ["config.toml"],
        "temp file should have been removed"
    );
    Ok(())
}

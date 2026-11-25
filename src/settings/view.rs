//! View preferences: which panes are visible and which color scheme is used

use relm4::adw;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // ViewMode
    // ---------------------------------------------------------------------

    #[test]
    fn view_mode_default_is_both() {
        assert_eq!(ViewMode::default(), ViewMode::Both);
    }

    #[test]
    fn view_mode_parallel_arrays_are_consistent() {
        assert_eq!(ViewMode::ALL.len(), ViewMode::LABELS.len());
        for (i, mode) in ViewMode::ALL.iter().enumerate() {
            assert_eq!(mode.index() as usize, i, "{mode:?} index mismatch");
        }
    }

    #[test]
    fn view_mode_index_round_trip() {
        for mode in ViewMode::ALL {
            assert_eq!(ViewMode::from_index(mode.index()), mode);
        }
    }

    #[test]
    fn view_mode_from_out_of_range_index_falls_back_to_default() {
        for bad in [3, 4, 100, u32::MAX] {
            assert_eq!(ViewMode::from_index(bad), ViewMode::default());
        }
    }

    #[test]
    fn view_mode_visibility_truth_table() {
        // (mode, shows_editor, shows_preview)
        let cases = [
            (ViewMode::EditorOnly, true, false),
            (ViewMode::PreviewOnly, false, true),
            (ViewMode::Both, true, true),
        ];
        for (mode, editor, preview) in cases {
            assert_eq!(mode.shows_editor(), editor, "{mode:?} editor");
            assert_eq!(mode.shows_preview(), preview, "{mode:?} preview");
        }
    }

    #[test]
    fn view_mode_every_variant_shows_something() {
        for mode in ViewMode::ALL {
            assert!(mode.shows_editor() || mode.shows_preview(), "{mode:?}");
        }
    }

    // ---------------------------------------------------------------------
    // ColorScheme
    // ---------------------------------------------------------------------

    #[test]
    fn color_scheme_default_is_system() {
        assert_eq!(ColorScheme::default(), ColorScheme::System);
    }

    #[test]
    fn color_scheme_parallel_arrays_are_consistent() {
        assert_eq!(ColorScheme::ALL.len(), ColorScheme::LABELS.len());
        for (i, scheme) in ColorScheme::ALL.iter().enumerate() {
            assert_eq!(scheme.index() as usize, i, "{scheme:?} index mismatch");
        }
    }

    #[test]
    fn color_scheme_index_round_trip() {
        for scheme in ColorScheme::ALL {
            assert_eq!(ColorScheme::from_index(scheme.index()), scheme);
        }
    }

    #[test]
    fn color_scheme_from_out_of_range_index_falls_back_to_default() {
        for bad in [3, 4, 100, u32::MAX] {
            assert_eq!(ColorScheme::from_index(bad), ColorScheme::default());
        }
    }

    #[test]
    fn color_scheme_maps_to_adw() {
        assert_eq!(ColorScheme::System.to_adw(), adw::ColorScheme::Default);
        assert_eq!(ColorScheme::Light.to_adw(), adw::ColorScheme::ForceLight);
        assert_eq!(ColorScheme::Dark.to_adw(), adw::ColorScheme::ForceDark);
    }
}

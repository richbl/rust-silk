//! Persisted window geometry

use serde::{Deserialize, Serialize};

use crate::constants::{DEFAULT_HEIGHT, DEFAULT_WIDTH};

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
    pub(super) fn sanitized(mut self) -> Self {
        if self.width < Self::MIN_WIDTH {
            self.width = DEFAULT_WIDTH;
        }
        if self.height < Self::MIN_HEIGHT {
            self.height = DEFAULT_HEIGHT;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_constants() {
        let w = WindowSettings::default();
        assert_eq!(w.width, DEFAULT_WIDTH);
        assert_eq!(w.height, DEFAULT_HEIGHT);
        assert!(!w.maximized);
        assert!(!w.fullscreened);
    }

    #[test]
    fn sanitized_keeps_valid_values() {
        let w = WindowSettings {
            width: 1600,
            height: 900,
            maximized: true,
            fullscreened: true,
        };
        assert_eq!(w.sanitized(), w);
    }

    #[test]
    fn sanitized_keeps_values_exactly_at_minimum() {
        let w = WindowSettings {
            width: WindowSettings::MIN_WIDTH,
            height: WindowSettings::MIN_HEIGHT,
            ..WindowSettings::default()
        };
        assert_eq!(w.sanitized(), w);
    }

    #[test]
    fn sanitized_resets_width_below_minimum() {
        for bad in [WindowSettings::MIN_WIDTH - 1, 1, 0, -1, i32::MIN] {
            let w = WindowSettings {
                width: bad,
                ..WindowSettings::default()
            }
            .sanitized();
            assert_eq!(w.width, DEFAULT_WIDTH, "width {bad} should be reset");
            assert_eq!(w.height, DEFAULT_HEIGHT);
        }
    }

    #[test]
    fn sanitized_resets_height_below_minimum() {
        for bad in [WindowSettings::MIN_HEIGHT - 1, 1, 0, -1, i32::MIN] {
            let w = WindowSettings {
                height: bad,
                ..WindowSettings::default()
            }
            .sanitized();
            assert_eq!(w.height, DEFAULT_HEIGHT, "height {bad} should be reset");
            assert_eq!(w.width, DEFAULT_WIDTH);
        }
    }

    #[test]
    fn sanitized_resets_each_dimension_independently() {
        let w = WindowSettings {
            width: 10,
            height: 700,
            maximized: false,
            fullscreened: false,
        }
        .sanitized();
        assert_eq!(w.width, DEFAULT_WIDTH);
        assert_eq!(w.height, 700);
    }

    #[test]
    fn sanitized_preserves_state_flags() {
        let w = WindowSettings {
            width: 0,
            height: 0,
            maximized: true,
            fullscreened: true,
        }
        .sanitized();
        assert!(w.maximized);
        assert!(w.fullscreened);
    }

    #[test]
    fn default_size_is_not_reset_by_sanitized() {
        let w = WindowSettings::default();
        assert_eq!(w.sanitized(), w);
    }
}

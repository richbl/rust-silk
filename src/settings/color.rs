//! Color helpers: parsing, luminance, and the light/dark palettes

use relm4::adw;
use relm4::gtk::gdk;

const CUSTOM_COLOR_DARK: &str = "rgb(36,36,36)";
const CUSTOM_COLOR_LIGHT: &str = "rgb(255,255,255)";

/// Parse a color string into a GDK RGBA color
pub fn parse_color(css: &str) -> Option<gdk::RGBA> {
    gdk::RGBA::parse(css).ok()
}

/// Starting color for a custom background, for the given light/dark style
pub fn custom_color_for(dark: bool) -> &'static str {
    if dark {
        CUSTOM_COLOR_DARK
    } else {
        CUSTOM_COLOR_LIGHT
    }
}

/// Sensible starting color when the user enables a custom background
pub fn default_custom_color() -> String {
    custom_color_for(adw::StyleManager::default().is_dark()).to_owned()
}

// Calculate the relative luminance of a color and linearize sRGB channels
fn linearize(c: f32) -> f32 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG-relative luminance used to determine the perceived brightness of a color
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

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const EPSILON: f32 = 1e-5;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn gray(level: f32) -> gdk::RGBA {
        gdk::RGBA::new(level, level, level, 1.0)
    }

    // ---------------------------------------------------------------------
    // parse_color
    // ---------------------------------------------------------------------

    #[test]
    fn parse_color_accepts_valid_css_colors() {
        let red = parse_color("#ff0000").expect("hex should parse");
        assert!(approx(red.red(), 1.0));
        assert!(approx(red.green(), 0.0));
        assert!(approx(red.blue(), 0.0));

        let dark = parse_color("rgb(36,36,36)").expect("rgb() should parse");
        let expected = 36.0 / 255.0;
        assert!(approx(dark.red(), expected));
        assert!(approx(dark.green(), expected));
        assert!(approx(dark.blue(), expected));
    }

    #[test]
    fn parse_color_rejects_invalid_input() {
        for bad in ["", "not-a-color", "#12", "rgb(1,2)"] {
            assert!(parse_color(bad).is_none(), "{bad:?} should not parse");
        }
    }

    // ---------------------------------------------------------------------
    // custom_color_for
    // ---------------------------------------------------------------------

    #[test]
    fn custom_color_for_returns_expected_values() {
        assert_eq!(custom_color_for(true), "rgb(36,36,36)");
        assert_eq!(custom_color_for(false), "rgb(255,255,255)");
    }

    #[test]
    fn custom_colors_are_parseable() {
        for dark in [false, true] {
            assert!(
                parse_color(custom_color_for(dark)).is_some(),
                "dark = {dark}"
            );
        }
    }

    #[test]
    fn custom_colors_match_scheme_palette_backgrounds() -> TestResult {
        // The "starting" custom color should look identical to the stock editor background
        for dark in [false, true] {
            let custom = parse_color(custom_color_for(dark)).ok_or("custom color must parse")?;
            let (bg, _) = scheme_palette(dark);
            let palette = parse_color(bg).ok_or("palette color must parse")?;

            assert!(approx(custom.red(), palette.red()), "red, dark = {dark}");
            assert!(
                approx(custom.green(), palette.green()),
                "green, dark = {dark}"
            );
            assert!(approx(custom.blue(), palette.blue()), "blue, dark = {dark}");
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // linearize
    // ---------------------------------------------------------------------

    #[test]
    fn linearize_endpoints() {
        assert!(approx(linearize(0.0), 0.0));
        assert!(approx(linearize(1.0), 1.0));
    }

    #[test]
    fn linearize_uses_linear_segment_below_cutoff() {
        let c = 0.02;
        assert!(approx(linearize(c), c / 12.92));
    }

    #[test]
    fn linearize_uses_power_segment_above_cutoff() {
        let c = 0.5;
        let expected = ((c + 0.055) / 1.055_f32).powf(2.4);
        assert!(approx(linearize(c), expected));
    }

    #[test]
    fn linearize_is_continuous_at_cutoff() {
        let below = linearize(0.03928);
        let above = linearize(0.03929);
        assert!((above - below).abs() < 1e-4, "{below} vs {above}");
    }

    #[test]
    fn linearize_is_monotonic() {
        let mut prev = linearize(0.0);
        for step in 1..=100_u8 {
            let current = linearize(f32::from(step) / 100.0);
            assert!(current >= prev, "not monotonic at step {step}");
            prev = current;
        }
    }

    // ---------------------------------------------------------------------
    // is_light / readable_foreground
    // ---------------------------------------------------------------------

    #[test]
    fn is_light_extremes() {
        assert!(is_light(&gray(1.0)));
        assert!(!is_light(&gray(0.0)));
    }

    #[test]
    fn is_light_primary_colors() {
        // Relative luminance weights: R 0.2126, G 0.7152, B 0.0722 (threshold 0.179)
        assert!(is_light(&gdk::RGBA::new(1.0, 0.0, 0.0, 1.0)));
        assert!(is_light(&gdk::RGBA::new(0.0, 1.0, 0.0, 1.0)));
        assert!(!is_light(&gdk::RGBA::new(0.0, 0.0, 1.0, 1.0)));
    }

    #[test]
    fn is_light_threshold_for_neutral_gray() {
        // A neutral gray crosses the 0.179 luminance threshold at roughly sRGB 0.46
        assert!(!is_light(&gray(0.45)));
        assert!(is_light(&gray(0.47)));
    }

    #[test]
    fn is_light_ignores_alpha() {
        let transparent_white = gdk::RGBA::new(1.0, 1.0, 1.0, 0.0);
        assert!(is_light(&transparent_white));
    }

    #[test]
    fn readable_foreground_contrasts_with_background() {
        assert_eq!(readable_foreground(&gray(1.0)), "#241f31");
        assert_eq!(readable_foreground(&gray(0.0)), "#ffffff");
    }

    #[test]
    fn readable_foreground_matches_is_light() {
        for level in [0.0, 0.2, 0.45, 0.47, 0.8, 1.0] {
            let bg = gray(level);
            let expected = if is_light(&bg) { "#241f31" } else { "#ffffff" };
            assert_eq!(readable_foreground(&bg), expected, "level {level}");
        }
    }

    // ---------------------------------------------------------------------
    // scheme_palette
    // ---------------------------------------------------------------------

    #[test]
    fn scheme_palette_light_and_dark() {
        assert_eq!(scheme_palette(false), ("#ffffff", "#241f31"));
        assert_eq!(scheme_palette(true), ("#242424", "#ffffff"));
    }

    #[test]
    fn scheme_palette_foreground_agrees_with_readable_foreground() -> TestResult {
        // The palette and the dynamic helper should never disagree on contrast
        for dark in [false, true] {
            let (bg, fg) = scheme_palette(dark);
            let rgba = parse_color(bg).ok_or("palette background must be parseable")?;
            assert_eq!(readable_foreground(&rgba), fg, "dark = {dark}");
        }
        Ok(())
    }
}

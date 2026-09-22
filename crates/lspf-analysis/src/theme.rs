//! The colour a grade letter is drawn in, fitted to the reader's theme.
//!
//! A grade letter has to be legible against whatever surface the editor draws
//! its hover on, and there is no one colour that manages it: a green that
//! reads on near-black washes out on white. The server used to sidestep this
//! by picking four colours that clear a weak contrast ratio against both ends,
//! which is a compromise that serves neither end well, and each client that
//! could do better then did its own thing — three palettes, three answers to
//! the same question.
//!
//! So the client reports its theme and this module answers once. It is told
//! two different things and uses them for two different purposes, which is why
//! they can never disagree:
//!
//! - [`ThemeKind`] is the *intent*: light, dark, or high contrast. No colour
//!   value implies a reader's accessibility choice, so the client has to say
//!   it. It picks which base palette is used.
//! - [`Theme::background`] is the *fact*: the real surface the letter lands
//!   on. It is used only to fit the base colour to it, which is what makes a
//!   custom theme work as well as a stock one.
//!
//! The fit runs in OKLab and moves only lightness, leaving hue and chroma
//! where they were. Walking a colour towards white in sRGB, which is the
//! obvious way and the way a client did it, desaturates as it goes: a red
//! headed for white turns pink long before it is legible. Holding chroma
//! fixed is the whole reason this is worth doing on the server rather than
//! three times over on the clients.

use lspf_analysis_core::health::Grade;

/// The contrast a grade letter has to clear against its surface.
///
/// WCAG 2 at the ratio it asks of normal-sized text. APCA models perception
/// better, especially on dark surfaces, but its thresholds are chosen per font
/// size and weight — which the server does not know, since each editor draws
/// the hover in its own type. One testable number beats a more accurate one
/// that cannot be applied here.
const TARGET_CONTRAST: f64 = 4.5;

/// How finely the fit walks lightness towards the surface's opposite.
///
/// 256 steps lands within a hair of the minimum that clears the target, and
/// makes the walk deterministic, which the property tests depend on.
const STEPS: u32 = 256;

/// The luminance below which a surface counts as dark.
///
/// The midpoint of the WCAG scale: the luminance at which white and black
/// contrast equally against it.
const DARK_SURFACE: f64 = 0.179;

/// An sRGB colour, as it travels and as it is written back out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const fn new(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
        }
    }

    /// Reads a colour a client sent.
    ///
    /// `#RGB`, `#RRGGBB` and `#RRGGBBAA` are all accepted, and the alpha of
    /// the last is dropped: a hover's surface is whatever the editor finally
    /// painted, and a client that reports a translucent one has told us the
    /// colour of the glass rather than of the wall. Anything else reads as
    /// nothing at all, which falls back to the kind's stand-in surface —
    /// never to an error, because a colour this server could not parse must
    /// not be able to take the reader's thresholds down with it.
    pub fn parse(value: &str) -> Option<Self> {
        let digits = value.strip_prefix('#')?;
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let pair = |index: usize| u8::from_str_radix(&digits[index..index + 2], 16).ok();
        match digits.len() {
            3 => {
                let single = |index: usize| {
                    u8::from_str_radix(&digits[index..=index], 16)
                        .ok()
                        .map(|v| v * 17)
                };
                Some(Self {
                    r: single(0)?,
                    g: single(1)?,
                    b: single(2)?,
                })
            }
            6 | 8 => Some(Self {
                r: pair(0)?,
                g: pair(2)?,
                b: pair(4)?,
            }),
            _ => None,
        }
    }

    /// Writes the colour as the six-digit hex a `style` attribute takes.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// What kind of theme the reader has on.
///
/// Three values, not the four VS Code names: it distinguishes a light high
/// contrast theme from a dark one, but under high contrast no colour is sent
/// at all, so the distinction has nothing left to decide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeKind {
    #[default]
    Dark,
    Light,
    /// The reader asked not to be told things by hue.
    HighContrast,
}

impl ThemeKind {
    /// Reads a kind from what a client sent.
    ///
    /// Spelling is not the client's problem: `highContrast`, `high-contrast`
    /// and `HIGH_CONTRAST` all arrive from somewhere. An unrecognized word is
    /// dark, which is the more common editor default and the safer guess —
    /// the dark palette on a light surface is merely dull, while the light
    /// palette on a dark one is dim.
    pub fn of(tag: &str) -> Self {
        let tag = tag.to_ascii_lowercase().replace(['-', '_', ' '], "");
        match tag.as_str() {
            "light" => Self::Light,
            "highcontrast" => Self::HighContrast,
            _ => Self::Dark,
        }
    }

    /// The kind a surface implies, for a client that reported one without
    /// naming its intent.
    ///
    /// This is the one place a colour is allowed to decide a kind, and it can
    /// only ever answer light or dark: high contrast is an accessibility
    /// choice, and a surface that happens to be black says nothing about
    /// whether its reader made one.
    pub fn of_surface(surface: Rgb) -> Self {
        if luminance(surface) < DARK_SURFACE {
            Self::Dark
        } else {
            Self::Light
        }
    }

    /// The surface to fit against when the client could not report one.
    ///
    /// VS Code is the client this exists for: its extension API exposes a
    /// theme's `kind` and nothing else, with no way to resolve a `ThemeColor`
    /// to a value, so it can report the intent but never the fact. These are
    /// the editor background of its two stock themes — a stand-in for the
    /// reader's real surface, not a claim about it.
    const fn stand_in_surface(self) -> Rgb {
        match self {
            Self::Light => Rgb::new(0xffffff),
            Self::Dark | Self::HighContrast => Rgb::new(0x1f1f1f),
        }
    }
}

/// What a client reported about the theme its reader has on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Theme {
    pub kind: ThemeKind,
    /// The hover surface's real colour, when the client can see it.
    pub background: Option<Rgb>,
}

/// The four colours a grade letter is drawn in, already fitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    excellent: Rgb,
    good: Rgb,
    fair: Rgb,
    poor: Rgb,
}

/// The base colours for a dark surface, before fitting.
const DARK: Palette = Palette {
    excellent: Rgb::new(0x30d158),
    good: Rgb::new(0x64d2ff),
    fair: Rgb::new(0xffd60a),
    poor: Rgb::new(0xff6961),
};

/// The base colours for a light surface, before fitting.
const LIGHT: Palette = Palette {
    excellent: Rgb::new(0x248a3d),
    good: Rgb::new(0x0055cc),
    fair: Rgb::new(0x895400),
    poor: Rgb::new(0xd70015),
};

/// The one palette stretched across both ends, for a client that reports no
/// theme at all.
///
/// This was the server's only palette, and its four colours are chosen to
/// clear 3:1 against white and against near-black alike — the compromise that
/// motivated all of the above. It stays, unchanged, because an editor that
/// speaks plain LSP and says nothing about its appearance is still owed a
/// readable letter, and this is the best that can be done without being told.
const UNTOLD: Palette = Palette {
    excellent: Rgb::new(0x2f9e44),
    good: Rgb::new(0x5a9216),
    fair: Rgb::new(0xc08a00),
    poor: Rgb::new(0xd1242f),
};

impl Palette {
    /// The palette for a theme, or `None` when no colour should be sent.
    ///
    /// High contrast returns `None`: a reader on such a theme has asked that
    /// meaning not ride on hue, and the grade is already carried in full by
    /// the letter itself. Dropping the colour is the answer, not finding a
    /// louder one.
    pub fn for_theme(theme: &Theme) -> Option<Self> {
        if theme.kind == ThemeKind::HighContrast {
            return None;
        }
        let base = match theme.kind {
            ThemeKind::Light => &LIGHT,
            ThemeKind::Dark | ThemeKind::HighContrast => &DARK,
        };
        let surface = theme
            .background
            .unwrap_or_else(|| theme.kind.stand_in_surface());
        Some(Self {
            excellent: fit(base.excellent, surface),
            good: fit(base.good, surface),
            fair: fit(base.fair, surface),
            poor: fit(base.poor, surface),
        })
    }

    /// The palette for a client that reported no theme.
    pub const fn untold() -> Self {
        UNTOLD
    }

    /// The colour one grade is drawn in.
    pub const fn of(&self, grade: Grade) -> Rgb {
        match grade {
            Grade::Excellent => self.excellent,
            Grade::Good => self.good,
            Grade::Fair => self.fair,
            Grade::Poor => self.poor,
        }
    }
}

/// Moves a colour's lightness until it is legible on `surface`.
///
/// Only lightness moves. Hue and chroma are held where the base palette put
/// them, so a fitted red is still that red, just light enough or dark enough
/// to read — which is what walking towards white or black in sRGB cannot do.
///
/// The walk runs away from the surface first, since that is where the room
/// is. If that direction cannot reach the target — a mid-grey surface leaves
/// less than 4.5:1 in either direction, for any colour — the other is tried,
/// and failing both, whichever got further wins. A letter that falls short is
/// still better read than one left at a ratio of 1.2.
fn fit(base: Rgb, surface: Rgb) -> Rgb {
    if contrast(base, surface) >= TARGET_CONTRAST {
        return base;
    }
    let away = if luminance(surface) < DARK_SURFACE {
        1.0
    } else {
        0.0
    };
    let first = walk(base, surface, away);
    if contrast(first, surface) >= TARGET_CONTRAST {
        return first;
    }
    let second = walk(base, surface, 1.0 - away);
    if contrast(second, surface) >= TARGET_CONTRAST
        || contrast(second, surface) > contrast(first, surface)
    {
        second
    } else {
        first
    }
}

/// Walks lightness towards `target`, stopping as soon as the letter reads.
fn walk(base: Rgb, surface: Rgb, target: f64) -> Rgb {
    let (lightness, a, b) = oklab(base);
    let mut last = base;
    for step in 1..=STEPS {
        let moved = lightness + (target - lightness) * f64::from(step) / f64::from(STEPS);
        last = from_oklab(moved, a, b);
        if contrast(last, surface) >= TARGET_CONTRAST {
            break;
        }
    }
    last
}

/// The WCAG contrast ratio between two colours.
fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (high, low) = (luminance(a), luminance(b));
    let (high, low) = if high > low { (high, low) } else { (low, high) };
    (high + 0.05) / (low + 0.05)
}

/// The WCAG relative luminance of a colour.
fn luminance(colour: Rgb) -> f64 {
    0.2126 * to_linear(colour.r) + 0.7152 * to_linear(colour.g) + 0.0722 * to_linear(colour.b)
}

/// One sRGB channel, undone back to light.
fn to_linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// One linear channel, redone as sRGB and clamped into range.
///
/// The clamp is where a fitted colour can lose a little of its hue: pushing
/// lightness while holding chroma eventually asks for a colour sRGB cannot
/// make, and the nearest one it can make is slightly off. Contrast is
/// measured after the clamp, so the loop never believes a colour it did not
/// actually produce.
fn from_linear(value: f64) -> u8 {
    let encoded = if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// sRGB to OKLab, by Björn Ottosson's matrices.
fn oklab(colour: Rgb) -> (f64, f64, f64) {
    let r = to_linear(colour.r);
    let g = to_linear(colour.g);
    let b = to_linear(colour.b);

    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();

    (
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    )
}

/// OKLab back to sRGB.
fn from_oklab(lightness: f64, a: f64, b: f64) -> Rgb {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);

    Rgb {
        r: from_linear(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        g: from_linear(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        b: from_linear(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRADES: [Grade; 4] = [Grade::Excellent, Grade::Good, Grade::Fair, Grade::Poor];

    /// The hue angle a colour sits at, in degrees, for comparing before and
    /// after a fit.
    fn hue(colour: Rgb) -> f64 {
        let (_, a, b) = oklab(colour);
        b.atan2(a).to_degrees()
    }

    /// How far apart two hue angles are, the short way round.
    fn hue_gap(a: Rgb, b: Rgb) -> f64 {
        let gap = (hue(a) - hue(b)).abs() % 360.0;
        if gap > 180.0 { 360.0 - gap } else { gap }
    }

    #[test]
    fn a_colour_reads_against_the_surface_it_was_fitted_to() {
        // The whole point of the exercise, stated as the test that would fail
        // if the fit were dropped.
        for (kind, surface) in [
            (ThemeKind::Dark, Rgb::new(0x1f1f1f)),
            (ThemeKind::Light, Rgb::new(0xffffff)),
            // Surfaces no stock theme uses, which is what the fit is for.
            (ThemeKind::Dark, Rgb::new(0x2b2b2b)),
            (ThemeKind::Dark, Rgb::new(0x3c3f41)),
            (ThemeKind::Light, Rgb::new(0xf5f5f5)),
            (ThemeKind::Light, Rgb::new(0xfdf6e3)),
        ] {
            let theme = Theme {
                kind,
                background: Some(surface),
            };
            let palette = Palette::for_theme(&theme).unwrap();
            for grade in GRADES {
                let ratio = contrast(palette.of(grade), surface);
                assert!(
                    ratio >= TARGET_CONTRAST,
                    "{kind:?} {grade:?} on {} reads at {ratio:.2}:1",
                    surface.hex()
                );
            }
        }
    }

    #[test]
    fn fitting_moves_lightness_and_leaves_the_hue_alone() {
        // A colour walked towards white in sRGB desaturates; this one must
        // not. The tolerance covers the gamut clamp and nothing more.
        for (kind, base, surface) in [
            (ThemeKind::Dark, &DARK, Rgb::new(0x1f1f1f)),
            (ThemeKind::Light, &LIGHT, Rgb::new(0xffffff)),
            (ThemeKind::Dark, &DARK, Rgb::new(0x3c3f41)),
            (ThemeKind::Light, &LIGHT, Rgb::new(0xf5f5f5)),
        ] {
            let palette = Palette::for_theme(&Theme {
                kind,
                background: Some(surface),
            })
            .unwrap();
            for grade in GRADES {
                let gap = hue_gap(base.of(grade), palette.of(grade));
                assert!(
                    gap < 10.0,
                    "{kind:?} {grade:?} on {} drifted {gap:.1}° in hue",
                    surface.hex()
                );
            }
        }
    }

    #[test]
    fn the_four_grades_stay_distinguishable_from_each_other() {
        for (kind, surface) in [
            (ThemeKind::Dark, Rgb::new(0x1f1f1f)),
            (ThemeKind::Light, Rgb::new(0xffffff)),
            (ThemeKind::Dark, Rgb::new(0x2b2b2b)),
            (ThemeKind::Light, Rgb::new(0xf5f5f5)),
        ] {
            let palette = Palette::for_theme(&Theme {
                kind,
                background: Some(surface),
            })
            .unwrap();
            for (index, one) in GRADES.iter().enumerate() {
                for other in &GRADES[index + 1..] {
                    let gap = hue_gap(palette.of(*one), palette.of(*other));
                    assert!(
                        gap > 20.0,
                        "{kind:?}: {one:?} and {other:?} are {gap:.1}° apart on {}",
                        surface.hex()
                    );
                }
            }
        }
    }

    #[test]
    fn a_colour_that_already_reads_is_left_exactly_as_it_was() {
        // The stock palettes were picked to work on the stock surfaces, so
        // most of them come through the fit untouched. A fit that nudged
        // every colour would mean the base palette had drifted.
        let palette = Palette::for_theme(&Theme {
            kind: ThemeKind::Dark,
            background: Some(Rgb::new(0x1f1f1f)),
        })
        .unwrap();
        assert_eq!(palette.of(Grade::Excellent), DARK.excellent);
        assert_eq!(palette.of(Grade::Good), DARK.good);
    }

    #[test]
    fn high_contrast_is_told_to_send_no_colour_at_all() {
        for background in [None, Some(Rgb::new(0x000000)), Some(Rgb::new(0xffffff))] {
            assert_eq!(
                Palette::for_theme(&Theme {
                    kind: ThemeKind::HighContrast,
                    background,
                }),
                None
            );
        }
    }

    #[test]
    fn a_client_that_reports_no_surface_is_fitted_to_a_stand_in() {
        // VS Code, which can name its theme's kind but never read a colour
        // out of it.
        for kind in [ThemeKind::Light, ThemeKind::Dark] {
            let palette = Palette::for_theme(&Theme {
                kind,
                background: None,
            })
            .unwrap();
            let surface = kind.stand_in_surface();
            for grade in GRADES {
                assert!(
                    contrast(palette.of(grade), surface) >= TARGET_CONTRAST,
                    "{kind:?} {grade:?} against the stand-in surface"
                );
            }
        }
    }

    #[test]
    fn a_surface_no_colour_can_read_on_still_gets_the_best_there_is() {
        // Mid-grey leaves under 4.5:1 in either direction for any hue. The
        // fit cannot win, and must not give up at the base colour either.
        let surface = Rgb::new(0x808080);
        let palette = Palette::for_theme(&Theme {
            kind: ThemeKind::Dark,
            background: Some(surface),
        })
        .unwrap();
        for grade in GRADES {
            let fitted = contrast(palette.of(grade), surface);
            let base = contrast(DARK.of(grade), surface);
            assert!(
                fitted >= base,
                "{grade:?} came out worse than it went in: {fitted:.2} < {base:.2}"
            );
        }
    }

    #[test]
    fn the_untold_palette_is_the_one_the_server_always_had() {
        // A plain LSP client that says nothing about its appearance must see
        // exactly what it saw before any of this existed.
        let palette = Palette::untold();
        assert_eq!(palette.of(Grade::Excellent).hex(), "#2f9e44");
        assert_eq!(palette.of(Grade::Good).hex(), "#5a9216");
        assert_eq!(palette.of(Grade::Fair).hex(), "#c08a00");
        assert_eq!(palette.of(Grade::Poor).hex(), "#d1242f");
    }

    #[test]
    fn a_kind_is_read_however_the_client_spells_it() {
        for tag in ["light", "Light", "LIGHT"] {
            assert_eq!(ThemeKind::of(tag), ThemeKind::Light, "{tag}");
        }
        for tag in [
            "highContrast",
            "high-contrast",
            "HIGH_CONTRAST",
            "high contrast",
        ] {
            assert_eq!(ThemeKind::of(tag), ThemeKind::HighContrast, "{tag}");
        }
        for tag in ["dark", "Dark", "vivid-purple", ""] {
            assert_eq!(ThemeKind::of(tag), ThemeKind::Dark, "{tag}");
        }
    }

    #[test]
    fn a_colour_is_read_in_every_spelling_a_client_might_send() {
        assert_eq!(Rgb::parse("#1f1f1f"), Some(Rgb::new(0x1f1f1f)));
        assert_eq!(Rgb::parse("#FFFFFF"), Some(Rgb::new(0xffffff)));
        assert_eq!(Rgb::parse("#fff"), Some(Rgb::new(0xffffff)));
        assert_eq!(Rgb::parse("#1a2"), Some(Rgb::new(0x11aa22)));
        // The alpha is dropped: the surface is what was painted, not the
        // glass in front of it.
        assert_eq!(Rgb::parse("#1f1f1fcc"), Some(Rgb::new(0x1f1f1f)));
    }

    #[test]
    fn an_unreadable_colour_is_nothing_rather_than_an_error() {
        // Every one of these has to come back `None` so the caller can fall
        // back to the kind's stand-in. A parse that could fail loudly would
        // take the reader's thresholds down with it.
        for value in [
            "rgb(31, 31, 31)",
            "1f1f1f",
            "#12345",
            "#1f1f1f1f1f",
            "#gggggg",
            "#",
            "",
            "transparent",
        ] {
            assert_eq!(Rgb::parse(value), None, "{value}");
        }
    }

    #[test]
    fn a_colour_survives_a_round_trip_through_oklab() {
        for colour in [
            Rgb::new(0x30d158),
            Rgb::new(0x0055cc),
            Rgb::new(0x000000),
            Rgb::new(0xffffff),
            Rgb::new(0x808080),
        ] {
            let (l, a, b) = oklab(colour);
            let back = from_oklab(l, a, b);
            for (one, other) in [(colour.r, back.r), (colour.g, back.g), (colour.b, back.b)] {
                assert!(
                    one.abs_diff(other) <= 1,
                    "{} came back as {}",
                    colour.hex(),
                    back.hex()
                );
            }
        }
    }
}

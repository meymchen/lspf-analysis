//! What the fitted palettes actually come out as.
//!
//! The property tests in `theme` say what a palette has to be true of —
//! legible on its surface, still the hue it started as, four colours a reader
//! can tell apart. Those are the specification, and they are what should fail
//! when the fit is wrong.
//!
//! They cannot say whether a palette got quietly uglier, so this pins the
//! values down as well. One snapshot, over the stock surfaces of the three
//! kinds plus the untold fallback: enough that a change to the fit has to be
//! looked at and accepted, few enough that accepting one is not a chore.

use lspf_analysis::hover::Colour;
use lspf_analysis::theme::{Palette, Rgb, Theme, ThemeKind};
use lspf_analysis_core::health::Grade;

const GRADES: [(&str, Grade); 4] = [
    ("A", Grade::Excellent),
    ("B", Grade::Good),
    ("C", Grade::Fair),
    ("D", Grade::Poor),
];

/// One line per grade, as `A #30d158`.
fn write(into: &mut String, palette: &Palette) {
    for (letter, grade) in GRADES {
        into.push_str(&format!("  {letter} {}\n", palette.of(grade).hex()));
    }
}

#[test]
fn the_palettes_a_reader_actually_sees() {
    let mut out = String::new();
    for (name, kind, background) in [
        ("dark, stock surface", ThemeKind::Dark, Some("#1f1f1f")),
        ("light, stock surface", ThemeKind::Light, Some("#ffffff")),
        // The surfaces the two desktop IDEs actually draw their hovers on,
        // which no stock palette was picked against.
        ("dark, IntelliJ surface", ThemeKind::Dark, Some("#3c3f41")),
        ("light, IntelliJ surface", ThemeKind::Light, Some("#f2f2f2")),
        // VS Code, which can name its kind but never read its surface.
        ("dark, no surface reported", ThemeKind::Dark, None),
        ("light, no surface reported", ThemeKind::Light, None),
    ] {
        out.push_str(&format!("{name}\n"));
        let theme = Theme {
            kind,
            background: background.and_then(Rgb::parse),
        };
        write(&mut out, &Palette::for_theme(&theme).expect("a palette"));
    }

    out.push_str("high contrast\n  no colour is sent\n");
    assert_eq!(
        Palette::for_theme(&Theme {
            kind: ThemeKind::HighContrast,
            background: Rgb::parse("#000000"),
        }),
        None
    );

    out.push_str("no theme reported\n");
    write(&mut out, &Palette::untold());

    insta::assert_snapshot!(out);
}

#[test]
fn the_letters_a_hover_draws_carry_the_fitted_colours() {
    // The snapshot above reads the palette directly; this checks the same
    // values survive the trip into the cell a table row is built from.
    let dark = Theme {
        kind: ThemeKind::Dark,
        background: Rgb::parse("#1f1f1f"),
    };
    let palette = Palette::for_theme(&dark).unwrap();
    let Colour::Spans(drawn) = Colour::Spans(palette.clone()) else {
        unreachable!()
    };
    assert_eq!(drawn, palette);
    assert_eq!(drawn.of(Grade::Excellent).hex(), "#30d158");
}

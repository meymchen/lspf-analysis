/**
 * What to tell the server about the theme the reader has on.
 *
 * The server colours the grade letters in a hover, and it can only do that
 * legibly if it knows what surface they land on. Other clients report both
 * the kind of theme and its actual background colour; this one can only
 * report the kind, because the extension API exposes `ColorTheme.kind` and
 * nothing else — there is no way to resolve a `ThemeColor` to a value. The
 * server falls back to a stand-in surface for the kind, which is why a
 * custom VS Code theme gets a slightly less exact fit than a custom IntelliJ
 * one.
 */

/**
 * The numeric values of `vscode.ColorThemeKind`.
 *
 * Spelled out rather than imported so this module stays free of `vscode` and
 * can be tested on its own, the way the rest of the pure logic here is.
 */
export const enum ColorThemeKind {
  Light = 1,
  Dark = 2,
  HighContrast = 3,
  HighContrastLight = 4,
}

/** The three kinds the server distinguishes. */
export type ThemeKind = 'light' | 'dark' | 'highContrast';

/**
 * Names a theme kind for the server.
 *
 * The editor's two high contrast kinds collapse into one: under high contrast
 * the server sends no colour at all, so whether the theme is a light or a
 * dark one has nothing left to decide. An unrecognized value — a kind added
 * to the editor after this was written — reads as dark, which is what the
 * server would have assumed anyway.
 */
export function themeKind(kind: number): ThemeKind {
  switch (kind) {
    case ColorThemeKind.Light:
      return 'light';
    case ColorThemeKind.HighContrast:
    case ColorThemeKind.HighContrastLight:
      return 'highContrast';
    default:
      return 'dark';
  }
}

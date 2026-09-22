using System;
using System.Globalization;
using System.Text.RegularExpressions;
using System.Windows.Media;

namespace LspfAnalysis
{
    // What to tell the server about the appearance it is drawing for, and how
    // to read back the colours it chose.
    //
    // The grade letters in a hover used to be coloured here, from a palette
    // kept in the view, fitted to the popup's background by walking each
    // colour towards white or black until it cleared 4.5:1. The server now
    // does that, for every client at once, and does it in a colour space that
    // keeps the hue while it moves the lightness — which walking towards
    // white cannot. What is left here is reporting the two facts the server
    // needs, and recognizing the one markup shape it answers with.
    internal static class HealthTheme
    {
        // The one form the server emits, and nothing else.
        //
        // This renderer's safety comes entirely from understanding a small
        // fixed subset and treating everything else as inert text. The hover
        // Markdown carries function names taken from the user's own source,
        // so a general-purpose HTML parser here would turn this path into one
        // that renders whatever markup happens to appear in a name. Anchoring
        // the pattern, and accepting only six hex digits and a single grade
        // letter, keeps the set of inputs this can act on small enough to
        // reason about exhaustively.
        // The timeout is belt and braces rather than a fix for a known cost:
        // the pattern is anchored at both ends and has no nested quantifier to
        // backtrack through, so it is linear in the length of the cell. But
        // the cell is derived from a document the reader opened, and a regex
        // that can run on that should have a bound it cannot exceed however
        // the pattern is later edited.
        private static readonly Regex GradeSpan = new(
            @"^<span style=""color:#(?<hex>[0-9a-fA-F]{6});"">(?<grade>[A-D])</span>$",
            RegexOptions.Compiled | RegexOptions.CultureInvariant,
            TimeSpan.FromMilliseconds(100));

        // Reads a leading table cell the server coloured.
        //
        // Returns false for anything else, including a bare letter from a
        // server that was never told about themes, which the caller then
        // draws in the ordinary text colour.
        internal static bool TryReadGrade(string cell, out string grade, out Color color)
        {
            grade = null;
            color = default;
            if (string.IsNullOrEmpty(cell)) return false;
            Match match;
            try
            {
                match = GradeSpan.Match(cell.Trim());
            }
            catch (RegexMatchTimeoutException)
            {
                // The same answer as any cell this cannot read: draw it as
                // text. A hover missing one colour beats a hover that threw.
                return false;
            }
            if (!match.Success) return false;
            grade = match.Groups["grade"].Value;
            var hex = match.Groups["hex"].Value;
            color = Color.FromRgb(
                byte.Parse(hex.Substring(0, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture),
                byte.Parse(hex.Substring(2, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture),
                byte.Parse(hex.Substring(4, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture));
            return true;
        }

        // Names the kind of theme in force.
        //
        // High contrast is the reader's accessibility choice and no colour
        // value implies it, so it is reported rather than inferred; the server
        // answers it by sending no colour at all. Light and dark come from the
        // surface's own luminance, which is what the palette used to be picked
        // by here.
        internal static string Kind(Color surface, bool highContrast) =>
            highContrast ? "highContrast" : Luminance(surface) < 0.179 ? "dark" : "light";

        // A colour as the `#rrggbb` the server reads.
        internal static string Hex(Color color) =>
            string.Format(CultureInfo.InvariantCulture, "#{0:x2}{1:x2}{2:x2}", color.R, color.G, color.B);

        // The WCAG relative luminance of a colour.
        internal static double Luminance(Color color) =>
            0.2126 * Linear(color.R) + 0.7152 * Linear(color.G) + 0.0722 * Linear(color.B);

        private static double Linear(byte channel)
        {
            var value = channel / 255.0;
            return value <= 0.04045 ? value / 12.92 : Math.Pow((value + 0.055) / 1.055, 2.4);
        }
    }
}

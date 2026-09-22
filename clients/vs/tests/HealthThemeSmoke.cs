using System;
using System.Windows.Media;
using LspfAnalysis;

// Checks what this client reports about its appearance, and what it will
// accept back from the server.
//
// The parser is the part worth pinning down. This renderer's safety rests on
// understanding a small fixed subset of Markdown and treating everything else
// as inert text, and the hover Markdown carries function names taken from the
// user's own source. So the negative cases below matter more than the
// positive one: each is a shape that must not be acted on.
internal static class HealthThemeSmoke
{
    private static int failures;

    private static void Check(bool condition, string what)
    {
        if (condition) return;
        Console.Error.WriteLine("FAIL " + what);
        failures++;
    }

    private static void Accepts(string cell, string expectedGrade, string expectedHex)
    {
        var read = HealthTheme.TryReadGrade(cell, out var grade, out var color);
        Check(read, "accepted: " + cell);
        if (!read) return;
        Check(grade == expectedGrade, "grade " + grade + " == " + expectedGrade);
        Check(HealthTheme.Hex(color) == expectedHex, "colour " + HealthTheme.Hex(color) + " == " + expectedHex);
    }

    private static void Rejects(string cell, string why)
    {
        Check(!HealthTheme.TryReadGrade(cell, out _, out _), "rejected (" + why + "): " + cell);
    }

    private static int Main()
    {
        Accepts("<span style=\"color:#30d158;\">A</span>", "A", "#30d158");
        Accepts(" <span style=\"color:#FF6961;\">D</span> ", "D", "#ff6961");

        // A server that was never told a theme sends the letter on its own.
        // It has to be drawn, but not as a colour this client invented.
        Rejects("A", "a bare letter is not a coloured one");
        Rejects("", "an empty cell");
        Rejects("  ", "a blank cell");

        // Everything below is a shape a general-purpose HTML parser would act
        // on and this one must not.
        Rejects("<span style=\"color:#30d158;\">A</span><script>x</script>", "trailing markup");
        Rejects("prefix <span style=\"color:#30d158;\">A</span>", "leading text");
        Rejects("<span onclick=\"x()\" style=\"color:#30d158;\">A</span>", "an extra attribute");
        Rejects("<span style=\"color:#30d158;background:url(x)\">A</span>", "an extra declaration");
        Rejects("<span style=\"color:red;\">A</span>", "a named colour");
        Rejects("<span style=\"color:#30d15;\">A</span>", "five hex digits");
        Rejects("<span style=\"color:#30d158;\">AB</span>", "two letters");
        Rejects("<span style=\"color:#30d158;\">E</span>", "a letter that is not a grade");
        Rejects("<span style=\"color:#30d158;\">control flow</span>", "a pillar name");
        Rejects("<a href=\"x\">A</a>", "a different tag");

        // The kind names the reader's intent. High contrast is reported
        // rather than inferred, because no colour value implies it.
        Check(HealthTheme.Kind(Color.FromRgb(0x1f, 0x1f, 0x1f), false) == "dark", "a dark surface");
        Check(HealthTheme.Kind(Color.FromRgb(0xff, 0xff, 0xff), false) == "light", "a light surface");
        Check(HealthTheme.Kind(Color.FromRgb(0x3c, 0x3f, 0x41), false) == "dark", "a dim grey surface");
        Check(HealthTheme.Kind(Color.FromRgb(0xf2, 0xf2, 0xf2), false) == "light", "an off-white surface");
        Check(HealthTheme.Kind(Color.FromRgb(0x00, 0x00, 0x00), true) == "highContrast", "high contrast, dark");
        Check(HealthTheme.Kind(Color.FromRgb(0xff, 0xff, 0xff), true) == "highContrast", "high contrast, light");

        Check(HealthTheme.Hex(Color.FromRgb(0, 0, 0)) == "#000000", "black as hex");
        Check(HealthTheme.Hex(Color.FromRgb(0xff, 0xff, 0xff)) == "#ffffff", "white as hex");

        Console.WriteLine(failures == 0 ? "HealthTheme: all checks passed." : failures + " check(s) failed.");
        return failures == 0 ? 0 : 1;
    }
}

# The server colours grade letters; clients colour their own icons

A hover's grade letters (A–D) are coloured by the server, which is told the
reader's theme and fits one palette to it. The icons the clients draw in their
own views — status bar, tree, gutter — keep using each editor's semantic colour
tokens. Two mechanisms, deliberately.

## Context

The server could only send one palette, so it sent four colours chosen to clear
a weak contrast ratio against both white and near-black — a compromise that
served neither end well. Every client that could do better then did its own
thing: IntelliJ re-coloured the letters from a `JBColor` light/dark pair, and
Visual Studio parsed the Markdown itself and applied a third palette, fitted to
the popup background by walking each colour towards white or black. Three
palettes, three answers to one question, and they disagreed: the same grade B
was yellow-green in VS Code and blue in Visual Studio.

## Decision

Clients report their theme in the `lspfAnalysis` settings section, as two
fields that answer different questions and therefore cannot contradict each
other:

- `kind` (`light` | `dark` | `highContrast`) is the reader's **intent**. No
  colour value implies an accessibility choice, so a client has to say it. It
  picks the base palette.
- `background` is the **fact**: the real surface the letters land on. It is
  used only to fit the base colours to it.

The fit runs in OKLab and moves only lightness, holding hue and chroma. This is
the part that makes doing it centrally worth the trouble: walking a colour
towards white in sRGB, which is what the Visual Studio client did, desaturates
as it goes — a red headed for white turns pink long before it is legible.

Under `highContrast` the server sends no colour at all. The reader asked that
meaning not ride on hue, and the letter already carries the grade in full.

A client must separately declare that its Markdown renderer keeps a `<span>`,
or it is sent bare letters — markup a client strips is worse than a letter on
its own. LSP's place for that is `general.markdown.allowedTags` in
`initialize`, and it is where the VS Code and IntelliJ clients say it. **The
Visual Studio client says it in `initializationOptions` instead**, under
`lspfAnalysis.markdown.allowedTags`. Its `ILanguageClient` exposes no hook for
shaping client capabilities, and the platform sends `initialize` itself without
consulting the middle layer — which was tried, and produced hovers in plain
text with nothing to say why. The server unions the two sources; they are the
same claim in two places, so no precedence rule is needed.

## Considered options

**Send a theme kind only, and keep two fixed palettes per kind.** Rejected
because it would not have removed the client-side work it was meant to remove:
Visual Studio fits against the *actual* popup background, so custom themes
would still have needed a client-side pass. The result would have been two
server palettes plus one client fitter — no convergence.

**APCA instead of WCAG 2 for the contrast target.** APCA models perception
better, especially on dark surfaces. Rejected because its thresholds are chosen
per font size and weight, and the server does not know either — each editor
draws the hover in its own type. WCAG 2's single 4.5:1 is a number that can
actually be applied here and tested.

**Move the icons to server-supplied hex as well, for consistency.** Rejected.
`charts.blue`, `JBColor`, and `EnvironmentColors` follow the reader's theme
live, follow their accessibility settings, and update the moment a theme
changes. A hex from the server can do none of that. Consistency is not worth
downgrading the mechanism that is already better.

## Consequences

**Grade colours have two sources of truth and will not match exactly.** A
hover's A is a fitted hex; the tree's A is `charts.green`. This is the accepted
cost of the rejection above, not an oversight.

**A hover already on screen keeps the colours it was drawn with.** Visual
Studio previously repainted open popups through `DynamicResource`. Now the
server has to be asked again, which happens on the next hover. The ordinary
text around the letters still follows the theme live.

**VS Code gets a slightly less exact fit than the other two.** Its extension
API exposes `ColorTheme.kind` and nothing else — there is no way to resolve a
`ThemeColor` to a value — so it reports the intent but never the fact, and the
server fits against a stand-in surface for the kind.

**An old server with a new client degrades on its own; no version negotiation
is needed.** A new client advertises `general.markdown.allowedTags: ["span"]`,
an old server sees that and sends its one fixed palette, and the client renders
it. The reader sees the old compromise colours rather than broken markup or
bare letters. The same palette is what a client that reports no theme at all
gets — a plain LSP client such as nvim, Helix, or Emacs.

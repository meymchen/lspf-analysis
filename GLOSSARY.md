# lspf-analysis

Code health analysis for source files, functions, and classes.

## Language

**Code Health**:
The assessment of source code through measurements, quality scores, and grades,
covering functions, classes, and source files.
_Avoid_: Function Health (only the function-specific assessment), Code Metrics
(measurements without the assessment).

**Function Health**:
The function-specific part of Code Health: each function's quality score,
pillars, and measurements.
_Avoid_: File health (the document-wide summary).

**Class Health**:
The class-specific part of Code Health: each scored class or interface's quality
score, class-design pillar, and measurements.
_Avoid_: Function Health (functions have their own assessment).

**Grade**:
The band a quality score falls in, named by the letter A to D. The letter is
the name: the words behind it are translated and their initials are not.
_Avoid_: Rating, level, severity.

**Palette**:
The four colours a grade is drawn in inside a hover, fitted by the server to
the theme a client reported. There is one per reader, and it reaches the hover
text and nothing else.
_Avoid_: Theme (what a reader's editor looks like, which a palette is fitted
_to_, not a synonym for it).

**Grade icon**:
The icon a client tints a grade with in its own views — status bar, tree,
gutter. Coloured from the editor's own semantic tokens rather than from the
palette, so it follows the reader's theme live. See
[ADR-0001](./docs/adr/0001-the-server-colours-grade-letters.md) for why these
are two things and not one.
_Avoid_: Badge, marker.

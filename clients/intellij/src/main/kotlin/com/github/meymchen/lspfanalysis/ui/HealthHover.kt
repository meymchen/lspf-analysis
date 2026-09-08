package com.github.meymchen.lspfanalysis.ui

import com.intellij.markdown.utils.doc.DocMarkdownToHtmlConverter
import com.intellij.openapi.project.Project
import com.intellij.ui.ColorUtil
import com.intellij.ui.JBColor

/**
 * A cell holding nothing but whitespace.
 *
 * The lookahead rather than matching the closing pipe, so two empty cells in
 * a row are both found: every metric row that is not the first of its pillar
 * leaves the pillar's cell empty.
 */
private val EMPTY_CELL = Regex("""\|[ \t]*(?=\|)""")

/** A row's leading grade cell, which the server sends as a bare letter. */
private val GRADE_CELL = Regex("""^\|[ \t]*([A-D])[ \t]*\|""")

/**
 * What each grade is drawn in, light theme first.
 *
 * The server picks its own hexadecimal for clients that take a `<span>` in
 * Markdown, because a sanitizer will not accept a colour asked for by name.
 * The IDE is not one of those clients — it advertises no `allowedTags`, so
 * the letter arrives bare — and here a theme-aware pair is available, which
 * is better than one compromise palette stretched across both themes.
 */
private val GRADE_COLOURS = mapOf(
    'A' to JBColor(0x2F9E44, 0x4CAF50),
    'B' to JBColor(0x5A9216, 0x8BC34A),
    'C' to JBColor(0xC08A00, 0xE2A336),
    'D' to JBColor(0xD1242F, 0xF14C4C),
)

/** Colour the grade letters and keep empty cells. */
internal fun healthHoverHtml(project: Project, markdown: String): String =
    DocMarkdownToHtmlConverter.convert(project, markdown.lineSequence().joinToString("\n") { line ->
        // A letter, not a span: a row the server already coloured is left
        // alone rather than wrapped twice.
        val graded = GRADE_CELL.replace(line) { match ->
            val grade = match.groupValues[1].first()
            val hex = ColorUtil.toHex(GRADE_COLOURS.getValue(grade))
            """| <span style="color:#$hex;">$grade</span> |"""
        }
        EMPTY_CELL.replace(graded, "| &nbsp; ")
    })

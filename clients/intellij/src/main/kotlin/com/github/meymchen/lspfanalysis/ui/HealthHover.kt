package com.github.meymchen.lspfanalysis.ui

import com.intellij.markdown.utils.doc.DocMarkdownToHtmlConverter
import com.intellij.openapi.project.Project

/**
 * A cell holding nothing but whitespace.
 *
 * The lookahead rather than matching the closing pipe, so two empty cells in
 * a row are both found: every metric row that is not the first of its pillar
 * leaves the pillar's cell empty.
 */
private val EMPTY_CELL = Regex("""\|[ \t]*(?=\|)""")

/**
 * Renders the server's Markdown as the HTML a gutter tooltip shows.
 *
 * The grade letters arrive already coloured. This plugin used to colour them
 * here, from a `JBColor` pair, because the server had only one palette to
 * offer and it was a compromise between a light theme and a dark one. It now
 * gets told which theme is on and what the tooltip's background actually is,
 * and fits the colour to that — see the `theme` module in the server — so
 * there is nothing left to do here but keep the table's empty cells from
 * collapsing.
 */
internal fun healthHoverHtml(project: Project, markdown: String): String = DocMarkdownToHtmlConverter.convert(
    project,
    markdown.lineSequence().joinToString("\n") { line -> EMPTY_CELL.replace(line, "| &nbsp; ") },
)

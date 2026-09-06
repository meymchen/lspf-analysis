package com.github.meymchen.lspfanalysis.ui

import com.intellij.markdown.utils.doc.DocMarkdownToHtmlConverter
import com.intellij.openapi.project.Project

/** Keep empty leading cells and make each bar an indivisible Swing preformatted block. */
internal fun healthHoverHtml(project: Project, markdown: String): String =
    DocMarkdownToHtmlConverter.convert(project, markdown.lineSequence().joinToString("\n") { line ->
        val cellsPreserved = line.replace(Regex("^\\|\\s*\\|"), "| &nbsp; |")
        cellsPreserved.replace(Regex("[█░]{10}(?: +[0-9]+%)?")) {
            "<pre style=\"margin: 0;\">${it.value}</pre>"
        }
    })

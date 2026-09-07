package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClient
import com.intellij.openapi.fileEditor.OpenFileDescriptor
import com.intellij.openapi.project.Project

/**
 * Opens the document behind `uri` with the caret on a 1-based `line`.
 *
 * The URI is resolved through the running client's descriptor, which is the
 * same mapping that produced it, so a Windows drive letter or a percent-escape
 * cannot round-trip into a file that is not found.
 */
fun navigateToFunction(project: Project, uri: String, line: Int) {
    val file = LspfAnalysisClient.findFile(project, uri) ?: return
    OpenFileDescriptor(project, file, (line - 1).coerceAtLeast(0), 0).navigate(true)
}

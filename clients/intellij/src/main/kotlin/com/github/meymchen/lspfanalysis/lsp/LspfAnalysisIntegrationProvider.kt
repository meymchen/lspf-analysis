package com.github.meymchen.lspfanalysis.lsp

import com.github.meymchen.lspfanalysis.LspfAnalysisIcons
import com.github.meymchen.lspfanalysis.settings.LspfAnalysisConfigurable
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspClient
import com.intellij.platform.lsp.api.LspIntegrationProvider
import com.intellij.platform.lsp.api.lsWidget.LspClientWidgetItem

/**
 * Starts the server the first time a file it can analyze is opened.
 *
 * One client per project: the server scores a document at a time and holds no
 * per-module state, so there is nothing for a second one to do.
 */
internal class LspfAnalysisIntegrationProvider : LspIntegrationProvider {

    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        clientStarter: LspIntegrationProvider.LspClientStarter,
    ) {
        if (isAnalyzable(file)) {
            clientStarter.ensureClientStarted(LspfAnalysisClientDescriptor(project))
        }
    }

    /** Puts this server in the platform's own `Language Services` widget. */
    override fun createWidgetItem(lspClient: LspClient, currentFile: VirtualFile?): LspClientWidgetItem =
        LspClientWidgetItem(
            lspClient,
            currentFile,
            LspfAnalysisIcons.Logo,
            LspfAnalysisConfigurable::class.java,
        )
}

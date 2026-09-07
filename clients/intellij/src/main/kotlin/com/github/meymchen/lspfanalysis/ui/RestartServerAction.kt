package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClient
import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.project.DumbAwareAction

/**
 * Starts the server again.
 *
 * Also the way back from a first start that gave up on a missing binary: the
 * platform has no client to restart, and this asks it to try again, which is
 * what someone who has just installed one wants.
 */
internal class RestartServerAction : DumbAwareAction() {

    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(event: AnActionEvent) {
        event.presentation.isEnabled = event.project != null
    }

    override fun actionPerformed(event: AnActionEvent) {
        event.project?.let { LspfAnalysisClient.restart(it) }
    }
}

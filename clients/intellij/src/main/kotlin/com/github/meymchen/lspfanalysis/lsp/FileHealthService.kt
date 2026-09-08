package com.github.meymchen.lspfanalysis.lsp

import com.github.meymchen.lspfanalysis.model.FileHealth
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.project.Project
import com.intellij.psi.PsiManager
import com.intellij.util.messages.Topic
import java.util.concurrent.ConcurrentHashMap

/**
 * The latest file summary per document, and who to tell when one arrives.
 *
 * The server re-analyzes on every keystroke and announces it here, so both the
 * status bar and the Function Health view can follow one source rather than
 * each listening to the connection.
 */
@Service(Service.Level.PROJECT)
class FileHealthService(private val project: Project) {

    private val reports = ConcurrentHashMap<String, FileHealth>()

    /** The summary for a document, or `null` if the server has not sent one. */
    fun report(uri: String): FileHealth? = reports[uri]

    fun publish(health: FileHealth) {
        reports[health.uri] = health
        project.messageBus.syncPublisher(TOPIC).fileHealthChanged(health.uri)
        // The analysis may finish after the editor's first line-marker pass.
        ApplicationManager.getApplication().invokeLater({
            if (!project.isDisposed) {
                val file = LspfAnalysisClient.findFile(project, health.uri)
                val psiFile = file?.let { PsiManager.getInstance(project).findFile(it) }
                if (psiFile != null) DaemonCodeAnalyzer.getInstance(project).restart(psiFile)
            }
        }, project.disposed)
    }

    /** Drops a closed document's summary, so the status bar stops offering it. */
    fun forget(uri: String) {
        if (reports.remove(uri) != null) {
            project.messageBus.syncPublisher(TOPIC).fileHealthChanged(uri)
        }
    }

    /** Forgets everything, for a server that stopped or restarted. */
    fun clear() {
        if (reports.isNotEmpty()) {
            reports.clear()
            project.messageBus.syncPublisher(TOPIC).fileHealthChanged(null)
            ApplicationManager.getApplication().invokeLater({
                if (!project.isDisposed) DaemonCodeAnalyzer.getInstance(project).restart()
            }, project.disposed)
        }
    }

    /** Told which document changed, or `null` when several did at once. */
    fun interface Listener {
        fun fileHealthChanged(uri: String?)
    }

    companion object {
        @Topic.ProjectLevel
        val TOPIC: Topic<Listener> = Topic.create("LSPF Analysis file health", Listener::class.java)

        fun getInstance(project: Project): FileHealthService = project.service()
    }
}

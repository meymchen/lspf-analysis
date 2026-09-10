package com.github.meymchen.lspfanalysis.lsp

import com.github.meymchen.lspfanalysis.model.FunctionHealth
import com.github.meymchen.lspfanalysis.model.parseFunctionHealth
import com.github.meymchen.lspfanalysis.settings.SECTION
import com.github.meymchen.lspfanalysis.settings.settingsPayload
import com.intellij.openapi.diagnostic.thisLogger
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspClient
import com.intellij.platform.lsp.api.LspClientManager
import org.eclipse.lsp4j.DidChangeConfigurationParams
import org.eclipse.lsp4j.HoverParams
import org.eclipse.lsp4j.MarkupKind
import org.eclipse.lsp4j.Position

/**
 * Everything this plugin asks of its running LSP client.
 *
 * Kept in one place so the views and actions never reach into the platform's
 * client registry themselves, and so "there is no server yet" is answered the
 * same way everywhere: with nothing, rather than an error.
 */
object LspfAnalysisClient {

    /** The line-marker pass only reads cached results and schedules missing ones. */
    fun healthHover(project: Project, file: VirtualFile, line: Int, character: Int): String? =
        HealthHoverService.getInstance(project).hover(file, line, character)

    internal suspend fun requestHealthHover(project: Project, file: VirtualFile, line: Int, character: Int): String? {
        val client = client(project) ?: return null
        val params = HoverParams(client.getDocumentIdentifier(file), Position(line, character))
        val hover = client.sendRequest { it.textDocumentService.hover(params) }
            ?: return null
        return hover.contents?.right?.takeIf { it.kind == MarkupKind.MARKDOWN }?.value
    }

    private fun client(project: Project): LspClient? = LspClientManager.getInstance(project)
        .getClients(LspfAnalysisIntegrationProvider::class.java)
        .firstOrNull()

    /** The URI the server knows `file` by, or `null` before the server starts. */
    fun fileUri(project: Project, file: VirtualFile): String? = client(project)?.descriptor?.getFileUri(file)

    /** The file behind a URI the server sent us. */
    fun findFile(project: Project, uri: String): VirtualFile? = client(project)?.descriptor?.findFileByUri(uri)

    /**
     * Asks the server for one document's per-function breakdown.
     *
     * An empty answer means no report. Request errors propagate so the Function
     * Health session can distinguish an unavailable server from an empty result.
     */
    suspend fun functionHealth(project: Project, uri: String): FunctionHealth? {
        val client = client(project) ?: return null
        val answer = client.sendRequest { server ->
            (server as LspfAnalysisLsp4jServer).functionHealth(FunctionHealthParams(uri))
        }
        return parseFunctionHealth(answer)
    }

    /**
     * Pushes the current settings.
     *
     * The server only accepts pushed configuration, and the payload carries the
     * IDE's display language along with the thresholds, so a push that dropped
     * it would leave the server rendering English from here on.
     */
    fun pushConfiguration(project: Project) {
        HealthHoverService.getInstance(project).clear()
        val client = client(project) ?: return
        val payload = settingsPayload(project)
        thisLogger().debug("pushing $SECTION configuration")
        client.sendNotification { server ->
            server.workspaceService.didChangeConfiguration(DidChangeConfigurationParams(payload))
        }
    }

    /**
     * Starts the server again with whatever the settings now say.
     *
     * Also the way back from a first start that gave up on a missing binary:
     * there is no client to restart, and this asks the platform to try again.
     */
    fun restart(project: Project) {
        LspClientManager.getInstance(project)
            .stopAndRestartClientsIfNeeded(LspfAnalysisIntegrationProvider::class.java)
    }
}

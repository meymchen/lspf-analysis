package com.github.meymchen.lspfanalysis.lsp

import com.github.meymchen.lspfanalysis.model.FILE_HEALTH_METHOD
import com.github.meymchen.lspfanalysis.model.FUNCTION_HEALTH_METHOD
import com.google.gson.JsonElement
import com.intellij.platform.lsp.api.Lsp4jClient
import com.intellij.platform.lsp.api.Lsp4jServer
import com.intellij.platform.lsp.api.LspServerNotificationsHandler
import org.eclipse.lsp4j.jsonrpc.services.JsonNotification
import org.eclipse.lsp4j.jsonrpc.services.JsonRequest
import java.util.concurrent.CompletableFuture

/** The argument of `lspfAnalysis/functionHealth`. */
data class FunctionHealthParams(val uri: String)

/**
 * The two methods this server adds to the protocol.
 *
 * There is no implementation to write: lsp4j generates one by reflection from
 * these annotations. The answers come back as raw JSON because the server is
 * versioned separately from this plugin, and the parsers in `model` decide
 * whether what arrived is something worth drawing.
 */
interface LspfAnalysisLsp4jServer : Lsp4jServer {
    @JsonRequest(FUNCTION_HEALTH_METHOD)
    fun functionHealth(params: FunctionHealthParams): CompletableFuture<JsonElement?>
}

/**
 * Listens for the file summary the server pushes after every analysis.
 *
 * A client that does not listen for the method ignores it, so this is the only
 * thing standing between the notification and the status bar.
 */
class LspfAnalysisLsp4jClient(
    handler: LspServerNotificationsHandler,
    private val onFileHealth: (JsonElement) -> Unit,
) : Lsp4jClient(handler) {

    @JsonNotification(FILE_HEALTH_METHOD)
    fun fileHealth(params: JsonElement) {
        onFileHealth(params)
    }
}

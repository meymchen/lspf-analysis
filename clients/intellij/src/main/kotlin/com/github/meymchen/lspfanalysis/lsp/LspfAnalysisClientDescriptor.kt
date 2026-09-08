package com.github.meymchen.lspfanalysis.lsp

import com.github.meymchen.lspfanalysis.model.ResolvedServer
import com.github.meymchen.lspfanalysis.model.ServerLocation
import com.github.meymchen.lspfanalysis.model.debugServerPort
import com.github.meymchen.lspfanalysis.model.describeMissingServer
import com.github.meymchen.lspfanalysis.model.parseFileHealth
import com.github.meymchen.lspfanalysis.model.resolveServerBinary
import com.github.meymchen.lspfanalysis.settings.LspfAnalysisServerSettings
import com.github.meymchen.lspfanalysis.settings.SECTION
import com.github.meymchen.lspfanalysis.settings.settingsPayload
import com.intellij.execution.ExecutionException
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.application.PluginPathManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.Lsp4jClient
import com.intellij.platform.lsp.api.Lsp4jServer
import com.intellij.platform.lsp.api.LspCommunicationChannel
import com.intellij.platform.lsp.api.LspServerListener
import com.intellij.platform.lsp.api.LspServerNotificationsHandler
import com.intellij.platform.lsp.api.ProjectWideLspClientDescriptor
import com.intellij.platform.lsp.api.customization.LspCustomization
import com.intellij.platform.lsp.api.customization.LspHoverDisabled
import org.eclipse.lsp4j.ConfigurationItem
import org.eclipse.lsp4j.InitializeResult
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/** The notification group declared in `plugin.xml`. */
const val NOTIFICATION_GROUP: String = "LSPF Analysis"

/**
 * What the server can analyze, keyed by extension, valued by the LSP language
 * id to announce it under.
 *
 * Extensions match the upstream grammars' `file-types` declarations and the
 * server's extension table. The ids are accepted by the server's `language_for`.
 */
private val LANGUAGE_IDS: Map<String, String> = buildMap {
    put("java", "java")
    for (extension in listOf("js", "mjs", "cjs")) put(extension, "javascript")
    put("jsx", "javascriptreact")
    put("py", "python")
    put("rs", "rust")
    put("ts", "typescript")
    put("tsx", "typescriptreact")
    for (extension in listOf(
        "cc", "cpp", "cxx", "hpp", "hxx", "h",
    )) {
        put(extension, "cpp")
    }
}

/** True when `file` is one this server can analyze. */
fun isAnalyzable(file: VirtualFile): Boolean = file.extension?.lowercase() in LANGUAGE_IDS

class LspfAnalysisClientDescriptor(project: Project) : ProjectWideLspClientDescriptor(project, "LSPF Analysis") {

    // A platform LSP documentation target suppresses the native PSI fallback.
    // Health belongs to a separate gutter tooltip so docstrings remain available.
    override val lspCustomization = object : LspCustomization() {
        override val hoverCustomizer = LspHoverDisabled
    }

    override fun isSupportedFile(file: VirtualFile): Boolean = isAnalyzable(file)

    /**
     * The server's own fallback would resolve `.rs` from the extension anyway,
     * but saying `rust` outright means both clients send the same id and the
     * server never has to guess.
     */
    override fun getLanguageId(file: VirtualFile): String =
        LANGUAGE_IDS[file.extension?.lowercase()] ?: super.getLanguageId(file)

    /**
     * How to reach the server: a socket to one already running under a
     * debugger, or a process of our own over stdio.
     */
    override val lspCommunicationChannel: LspCommunicationChannel
        get() = debugPort()?.let { LspCommunicationChannel.Socket(it, startProcess = false) }
            ?: LspCommunicationChannel.StdIO

    /**
     * Returns nothing usable when the binary is missing, having said so --
     * starting anyway would leave the platform retrying a spawn that cannot
     * succeed.
     */
    override fun createCommandLine(): GeneralCommandLine {
        val resolved = resolveServer()
        if (!Files.exists(resolved.binary)) {
            val explanation = describeMissingServer(resolved)
            NotificationGroupManager.getInstance()
                .getNotificationGroup(NOTIFICATION_GROUP)
                .createNotification(explanation, NotificationType.ERROR)
                .notify(project)
            throw ExecutionException(explanation)
        }

        return GeneralCommandLine(resolved.binary.toString(), "serve", "--stdio").apply {
            // `LSPF_ANALYSIS_LOG_FILE` is deliberately left alone. Unset, the
            // server logs to stderr, which the platform captures into the IDE
            // log -- which is what we want. Setting it to an empty string to
            // mean "no log file" does not work: the server reads it with
            // `var_os`, so an empty value is still a value, and it exits 1
            // trying to create a file at that path.
            environment["RUST_LOG"] =
                System.getenv("RUST_LOG") ?: if (isDevelopment()) "debug" else "info"
        }
    }

    /** Settings the server can use before the first document arrives. */
    override fun createInitializationOptions(): Any = settingsPayload(project)

    /**
     * Answers a server that pulls its configuration instead of taking the push.
     * Today's server only takes the push; this costs nothing and means a later
     * one needs no change here.
     */
    override fun getWorkspaceConfiguration(item: ConfigurationItem): Any? = if (item.section == SECTION) {
        settingsPayload(project)
    } else {
        super.getWorkspaceConfiguration(item)
    }

    override val lsp4jServerClass: Class<out Lsp4jServer> = LspfAnalysisLsp4jServer::class.java

    override fun createLsp4jClient(handler: LspServerNotificationsHandler): Lsp4jClient =
        LspfAnalysisLsp4jClient(handler) { payload ->
            parseFileHealth(payload)?.let { FileHealthService.getInstance(project).publish(it) }
        }

    override val lspServerListener: LspServerListener = object : LspServerListener {
        override fun serverInitialized(params: InitializeResult) {
            // A restart re-analyzes everything, so what was on screen belongs to
            // the previous connection until the summaries come back.
            FileHealthService.getInstance(project).clear()
        }

        override fun serverStopped(shutdownNormally: Boolean) {
            FileHealthService.getInstance(project).clear()
        }
    }

    /** Where this run would look for the server, whether or not it is there. */
    fun resolveServer(): ResolvedServer = resolveServerBinary(
        ServerLocation(
            bundledDirectory = bundledDirectory(),
            development = isDevelopment(),
            repositoryRoot = repositoryRoot(),
            configuredPath = LspfAnalysisServerSettings.getInstance().state.serverPath,
        ),
    )

    companion object {
        /** True when running out of a Gradle sandbox rather than an installation. */
        fun isDevelopment(): Boolean = System.getProperty("lspfAnalysis.development") == "true"

        /** The repository root, which only a sandbox run is told about. */
        fun repositoryRoot(): Path? = System.getProperty("lspfAnalysis.repositoryRoot")?.let { Paths.get(it) }

        /** The port a TCP debug session is listening on, if there is one. */
        fun debugPort(): Int? = debugServerPort(System.getenv())

        /**
         * Where `buildPlugin` puts the binary, inside the plugin's own
         * distribution directory.
         *
         * Resolved from a class of ours rather than from the plugin id: that is
         * the supported way to reach a plugin's own files, and it keeps working
         * if the id is ever changed in one place only.
         */
        private fun bundledDirectory(): Path? = PluginPathManager
            .getPluginResource(LspfAnalysisClientDescriptor::class.java, SERVER_DIRECTORY)
            ?.toPath()
    }
}

/** The directory `prepareSandbox` copies the server binary into. */
private const val SERVER_DIRECTORY = "server"

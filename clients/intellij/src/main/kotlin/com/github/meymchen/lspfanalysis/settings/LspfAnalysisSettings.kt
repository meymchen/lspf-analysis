package com.github.meymchen.lspfanalysis.settings

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service
import com.intellij.openapi.project.Project
import com.intellij.util.xmlb.XmlSerializerUtil

/**
 * Everything the analysis is judged by, per project.
 *
 * Project-level rather than global because the LSP client is project-wide and
 * because thresholds are a property of a codebase: a parser and a controller
 * are not held to the same complexity. The defaults are the ones the VS Code
 * client contributes, so a reader who moves between the two editors sees the
 * same numbers.
 */
class HealthState {
    var complexityThreshold: Double = 15.0
    var cyclomaticThreshold: Double = 10.0
    var lengthThreshold: Double = 30.0
    var workingMemoryThreshold: Double = 8.0
    var halsteadDifficultyThreshold: Double = 12.0
    var parametersThreshold: Double = 4.0
    var wmcThreshold: Double = 34.0
    var publicMethodsThreshold: Double = 14.0
    var publicAttributesThreshold: Double = 8.0

    var weightComplexity: Double = 1.0
    var weightLength: Double = 1.0
    var weightWorkingMemory: Double = 1.0
    var weightInterface: Double = 0.5
    var weightClassDesign: Double = 0.5

    var qualityWarn: Double = 25.0
    var qualityError: Double = 10.0

    var diagnosticsEnabled: Boolean = true
    var diagnosticsPerMetric: Boolean = false
    var diagnosticsFile: Boolean = false

    var statusBarEnabled: Boolean = true
    var gutterIconsEnabled: Boolean = true
}

@Service(Service.Level.PROJECT)
@State(name = "LspfAnalysis", storages = [Storage("lspfAnalysis.xml")])
class LspfAnalysisSettings : PersistentStateComponent<HealthState> {
    private var state = HealthState()

    override fun getState(): HealthState = state

    override fun loadState(state: HealthState) {
        XmlSerializerUtil.copyBean(state, this.state)
    }

    companion object {
        fun getInstance(project: Project): LspfAnalysisSettings = project.service()
    }
}

/**
 * Settings that belong to this machine rather than to a project.
 *
 * The path to a binary follows the installation, not the codebase, which is why
 * the VS Code client marks the same setting `machine-overridable`.
 *
 * There is no counterpart to that client's `trace.server`: the platform owns
 * the LSP conversation and logs it under its own `LSP log: info, trace`
 * notification category, so a plugin-side switch would have nothing to switch.
 */
class ServerState {
    var serverPath: String = ""
}

@Service(Service.Level.APP)
@State(name = "LspfAnalysisServer", storages = [Storage("lspfAnalysis.xml")])
class LspfAnalysisServerSettings : PersistentStateComponent<ServerState> {
    private var state = ServerState()

    override fun getState(): ServerState = state

    override fun loadState(state: ServerState) {
        XmlSerializerUtil.copyBean(state, this.state)
    }

    companion object {
        fun getInstance(): LspfAnalysisServerSettings = service()
    }
}

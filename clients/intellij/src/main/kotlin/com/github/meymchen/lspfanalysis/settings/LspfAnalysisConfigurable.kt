package com.github.meymchen.lspfanalysis.settings

import com.github.meymchen.lspfanalysis.LspfAnalysisBundle
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClient
import com.github.meymchen.lspfanalysis.ui.refreshFileHealthWidget
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer
import com.intellij.openapi.options.BoundSearchableConfigurable
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.DialogPanel
import com.intellij.openapi.ui.ValidationInfo
import com.intellij.ui.components.JBTextField
import com.intellij.ui.dsl.builder.Cell
import com.intellij.ui.dsl.builder.Row
import com.intellij.ui.dsl.builder.bindSelected
import com.intellij.ui.dsl.builder.bindText
import com.intellij.ui.dsl.builder.columns
import com.intellij.ui.dsl.builder.panel
import kotlin.math.abs
import kotlin.math.floor

/**
 * One page, two halves: what this project is judged by, and where this machine
 * keeps the binary.
 *
 * The split is the same one the VS Code client draws with `machine-overridable`
 * -- thresholds belong to a codebase, a path belongs to an installation.
 */
class LspfAnalysisConfigurable(private val project: Project) :
    BoundSearchableConfigurable(
        LspfAnalysisBundle.message("settings.title"),
        "settings.lspfAnalysis",
        "lspfAnalysis.settings",
    ) {

    private val health get() = LspfAnalysisSettings.getInstance(project).state
    private val server get() = LspfAnalysisServerSettings.getInstance().state

    override fun createPanel(): DialogPanel = panel {
        group(LspfAnalysisBundle.message("settings.group.thresholds")) {
            row(LspfAnalysisBundle.message("settings.complexityThreshold")) {
                positive({ health.complexityThreshold }, { health.complexityThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.complexityThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.cyclomaticThreshold")) {
                positive({ health.cyclomaticThreshold }, { health.cyclomaticThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.cyclomaticThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.lengthThreshold")) {
                positive({ health.lengthThreshold }, { health.lengthThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.lengthThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.workingMemoryThreshold")) {
                positive({ health.workingMemoryThreshold }, { health.workingMemoryThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.workingMemoryThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.halsteadDifficultyThreshold")) {
                positive(
                    { health.halsteadDifficultyThreshold },
                    { health.halsteadDifficultyThreshold = it },
                )
            }.rowComment(LspfAnalysisBundle.message("settings.halsteadDifficultyThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.parametersThreshold")) {
                positive({ health.parametersThreshold }, { health.parametersThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.parametersThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.qualityWarn")) {
                percentage({ health.qualityWarn }, { health.qualityWarn = it })
            }.rowComment(LspfAnalysisBundle.message("settings.qualityWarn.comment"))

            row(LspfAnalysisBundle.message("settings.qualityError")) {
                percentage({ health.qualityError }, { health.qualityError = it })
            }.rowComment(LspfAnalysisBundle.message("settings.qualityError.comment"))
        }

        group(LspfAnalysisBundle.message("settings.group.classes")) {
            row(LspfAnalysisBundle.message("settings.wmcThreshold")) {
                positive({ health.wmcThreshold }, { health.wmcThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.wmcThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.publicMethodsThreshold")) {
                positive({ health.publicMethodsThreshold }, { health.publicMethodsThreshold = it })
            }.rowComment(LspfAnalysisBundle.message("settings.publicMethodsThreshold.comment"))

            row(LspfAnalysisBundle.message("settings.publicAttributesThreshold")) {
                positive(
                    { health.publicAttributesThreshold },
                    { health.publicAttributesThreshold = it },
                )
            }.rowComment(LspfAnalysisBundle.message("settings.publicAttributesThreshold.comment"))
        }

        group(LspfAnalysisBundle.message("settings.group.weights")) {
            row(LspfAnalysisBundle.message("settings.weights.complexity")) {
                weight({ health.weightComplexity }, { health.weightComplexity = it })
            }
            row(LspfAnalysisBundle.message("settings.weights.length")) {
                weight({ health.weightLength }, { health.weightLength = it })
            }
            row(LspfAnalysisBundle.message("settings.weights.workingMemory")) {
                weight({ health.weightWorkingMemory }, { health.weightWorkingMemory = it })
            }
            row(LspfAnalysisBundle.message("settings.weights.interface")) {
                weight({ health.weightInterface }, { health.weightInterface = it })
            }
            row(LspfAnalysisBundle.message("settings.weights.classDesign")) {
                weight({ health.weightClassDesign }, { health.weightClassDesign = it })
            }.rowComment(LspfAnalysisBundle.message("settings.weights.comment"))
        }

        group(LspfAnalysisBundle.message("settings.group.diagnostics")) {
            row {
                checkBox(LspfAnalysisBundle.message("settings.diagnostics.enabled"))
                    .bindSelected({ health.diagnosticsEnabled }, { health.diagnosticsEnabled = it })
            }.rowComment(LspfAnalysisBundle.message("settings.diagnostics.enabled.comment"))

            row {
                checkBox(LspfAnalysisBundle.message("settings.diagnostics.perMetric"))
                    .bindSelected({ health.diagnosticsPerMetric }, { health.diagnosticsPerMetric = it })
            }.rowComment(LspfAnalysisBundle.message("settings.diagnostics.perMetric.comment"))

            row {
                checkBox(LspfAnalysisBundle.message("settings.diagnostics.file"))
                    .bindSelected({ health.diagnosticsFile }, { health.diagnosticsFile = it })
            }.rowComment(LspfAnalysisBundle.message("settings.diagnostics.file.comment"))

            row {
                checkBox(LspfAnalysisBundle.message("settings.statusBar.enabled"))
                    .bindSelected({ health.statusBarEnabled }, { health.statusBarEnabled = it })
            }
            row {
                checkBox(LspfAnalysisBundle.message("settings.gutterIcons.enabled"))
                    .bindSelected({ health.gutterIconsEnabled }, { health.gutterIconsEnabled = it })
            }
        }

        group(LspfAnalysisBundle.message("settings.group.server")) {
            row(LspfAnalysisBundle.message("settings.serverPath")) {
                textField()
                    .columns(40)
                    .bindText({ server.serverPath }, { server.serverPath = it.trim() })
            }.rowComment(LspfAnalysisBundle.message("settings.serverPath.comment"))

            row {
                comment(LspfAnalysisBundle.message("settings.serverLog.comment"))
            }
        }
    }

    override fun apply() {
        val previousPath = server.serverPath
        val previousGutterIcons = health.gutterIconsEnabled
        super.apply()

        if (previousPath != server.serverPath) {
            // A different binary is a different server, not a new configuration.
            LspfAnalysisClient.restart(project)
        } else {
            LspfAnalysisClient.pushConfiguration(project)
        }
        // The status bar can be switched off without the server knowing.
        refreshFileHealthWidget(project)
        if (previousGutterIcons != health.gutterIconsEnabled) {
            DaemonCodeAnalyzer.getInstance(project).restart()
        }
    }
}

/** A threshold: the value that would score 50%, which has to be above zero. */
private fun Row.positive(get: () -> Double, set: (Double) -> Unit): Cell<JBTextField> =
    numberField(get, set) { if (it > 0) null else "settings.error.positive" }

/** A weight, which may be zero to drop a pillar out of the blend entirely. */
private fun Row.weight(get: () -> Double, set: (Double) -> Unit): Cell<JBTextField> =
    numberField(get, set) { if (it >= 0) null else "settings.error.nonNegative" }

/** A quality score, on the same 0-100 scale the server reports. */
private fun Row.percentage(get: () -> Double, set: (Double) -> Unit): Cell<JBTextField> =
    numberField(get, set) { if (it in 0.0..100.0) null else "settings.error.percentage" }

private fun Row.numberField(
    get: () -> Double,
    set: (Double) -> Unit,
    validate: (Double) -> String?,
): Cell<JBTextField> = textField()
    .columns(8)
    .bindText({ formatNumber(get()) }, { set(it.trim().toDoubleOrNull() ?: get()) })
    .validationOnInput { field ->
        val value = field.text.trim().toDoubleOrNull()
        val problem = if (value == null) "settings.error.number" else validate(value)
        problem?.let { ValidationInfo(LspfAnalysisBundle.dynamic(it, emptyArray()), field) }
    }

/**
 * Prints a threshold the way it was typed.
 *
 * The settings are doubles because the weights are fractional, but a threshold
 * of 15 should not show up as `15.0` in a text field.
 */
private fun formatNumber(value: Double): String =
    if (value == floor(value) && abs(value) < 1e9) value.toLong().toString() else value.toString()

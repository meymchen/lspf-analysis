package com.github.meymchen.lspfanalysis.settings

import com.google.gson.JsonObject
import com.intellij.DynamicBundle
import com.intellij.openapi.project.Project
import com.intellij.util.ui.StartupUiUtil
import com.intellij.util.ui.UIUtil
import java.awt.Color
import java.util.Locale

/** The settings section the server reads, and this plugin's own id for it. */
const val SECTION: String = "lspfAnalysis"

/**
 * The settings to send the server, with the IDE's display language added.
 *
 * The language is not one of the plugin's settings -- it belongs to the IDE --
 * but the server renders hovers and diagnostic messages, so it has to be told.
 * It rides in the same section so that both the startup options and every later
 * push carry it; a push without it would leave the server rendering English
 * from the next configuration change onward.
 *
 * Only what the server actually reads is sent. Everything else here -- whether
 * the status bar is shown, where the binary is -- is this plugin's own business
 * and never travels.
 */
fun settingsPayload(project: Project): JsonObject {
    val state = LspfAnalysisSettings.getInstance(project).state

    val weights = JsonObject().apply {
        addProperty("complexity", state.weightComplexity)
        addProperty("length", state.weightLength)
        addProperty("workingMemory", state.weightWorkingMemory)
        addProperty("interface", state.weightInterface)
        addProperty("classDesign", state.weightClassDesign)
    }

    val health = JsonObject().apply {
        addProperty("complexityThreshold", state.complexityThreshold)
        addProperty("cyclomaticThreshold", state.cyclomaticThreshold)
        addProperty("lengthThreshold", state.lengthThreshold)
        addProperty("workingMemoryThreshold", state.workingMemoryThreshold)
        addProperty("halsteadDifficultyThreshold", state.halsteadDifficultyThreshold)
        addProperty("parametersThreshold", state.parametersThreshold)
        addProperty("wmcThreshold", state.wmcThreshold)
        addProperty("publicMethodsThreshold", state.publicMethodsThreshold)
        addProperty("publicAttributesThreshold", state.publicAttributesThreshold)
        addProperty("qualityWarn", state.qualityWarn)
        addProperty("qualityError", state.qualityError)
        add("weights", weights)
    }

    val diagnostics = JsonObject().apply {
        addProperty("enabled", state.diagnosticsEnabled)
        addProperty("perMetric", state.diagnosticsPerMetric)
        addProperty("file", state.diagnosticsFile)
    }

    val section = JsonObject().apply {
        add("health", health)
        add("diagnostics", diagnostics)
        addProperty("locale", displayLocale())
        add("theme", displayTheme())
    }

    return JsonObject().apply { add(SECTION, section) }
}

/**
 * The appearance the server should colour grade letters for.
 *
 * Two things, because the server uses them for two different purposes and so
 * they can never contradict each other. The `kind` is the intent, which picks
 * which palette is used. The `background` is the fact -- the colour the
 * tooltip is actually painted on, which the palette's colours are then fitted
 * to, so a hand-rolled theme reads as well as a stock one. This plugin can
 * report both; the VS Code one can only report the kind.
 *
 * High contrast is missing, and is the one gap in what this reports: the IDE
 * has no supported way to ask whether a high contrast theme is on, only
 * theme-name matching that breaks the first time a theme is renamed. A reader
 * on one gets the dark palette rather than the uncoloured letters they should.
 */
private fun displayTheme(): JsonObject = JsonObject().apply {
    addProperty("kind", if (StartupUiUtil.isDarkTheme) "dark" else "light")
    addProperty("background", hex(UIUtil.getToolTipBackground()))
}

/**
 * A colour as the `#rrggbb` the server reads.
 *
 * Formatted against [Locale.ROOT] rather than the IDE's own locale. The
 * server parses this, so it is a wire value and not display text: a locale
 * whose digits or letter casing differ would produce a colour the server
 * reads as nothing, and the letters would silently lose their fitted palette.
 */
private fun hex(color: Color): String = String.format(Locale.ROOT, "#%02x%02x%02x", color.red, color.green, color.blue)

/**
 * The IDE's display language as a tag the server understands.
 *
 * `zh-cn` and `en` are the two it has translations for; anything else renders
 * English rather than a guess, which is the server's own rule.
 */
private fun displayLocale(): String = DynamicBundle.getLocale().toLanguageTag().lowercase()

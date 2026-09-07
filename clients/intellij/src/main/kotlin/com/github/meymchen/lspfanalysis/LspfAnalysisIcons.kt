package com.github.meymchen.lspfanalysis

import com.intellij.icons.AllIcons
import com.intellij.openapi.util.IconLoader
import javax.swing.Icon

object LspfAnalysisIcons {
    /** The plugin's own mark, used by the tool window and the widgets. */
    @JvmField
    val Logo: Icon = IconLoader.getIcon("/icons/lspfAnalysis.svg", LspfAnalysisIcons::class.java)

    /**
     * The icon standing in for a band.
     *
     * The platform's own four severities rather than tinted glyphs of our own:
     * they already read as green, blue, yellow and red under every theme, which
     * is what the VS Code client gets out of `charts.*`. A fair function is
     * worth looking at, not a problem the IDE is reporting, so nothing here
     * borrows the inspection colours beyond that.
     */
    fun badge(grade: String): Icon = when (grade) {
        "excellent" -> AllIcons.General.InspectionsOK
        "good" -> AllIcons.General.Information
        "fair" -> AllIcons.General.Warning
        else -> AllIcons.General.Error
    }
}

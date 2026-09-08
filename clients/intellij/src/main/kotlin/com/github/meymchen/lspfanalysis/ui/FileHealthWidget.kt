package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.LspfAnalysisBundle
import com.github.meymchen.lspfanalysis.LspfAnalysisIcons
import com.github.meymchen.lspfanalysis.lsp.FileHealthService
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClient
import com.github.meymchen.lspfanalysis.model.FileHealth
import com.github.meymchen.lspfanalysis.model.StatusPresentation
import com.github.meymchen.lspfanalysis.model.WorstFunction
import com.github.meymchen.lspfanalysis.model.renderStatus
import com.github.meymchen.lspfanalysis.model.worstLabel
import com.github.meymchen.lspfanalysis.settings.LspfAnalysisConfigurable
import com.github.meymchen.lspfanalysis.settings.LspfAnalysisSettings
import com.intellij.icons.AllIcons
import com.intellij.openapi.actionSystem.ActionManager
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.DataContext
import com.intellij.openapi.actionSystem.DefaultActionGroup
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerEvent
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.project.DumbAwareAction
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.popup.JBPopup
import com.intellij.openapi.ui.popup.JBPopupFactory
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.openapi.wm.StatusBar
import com.intellij.openapi.wm.StatusBarWidget
import com.intellij.openapi.wm.StatusBarWidgetFactory
import com.intellij.openapi.wm.ToolWindowId
import com.intellij.openapi.wm.ToolWindowManager
import com.intellij.openapi.wm.WindowManager
import javax.swing.Icon

internal const val FILE_HEALTH_WIDGET_ID = "LspfAnalysis.fileHealth"

/** Repaints the widget, for a setting that changed under it. */
fun refreshFileHealthWidget(project: Project) {
    WindowManager.getInstance().getStatusBar(project)?.updateWidget(FILE_HEALTH_WIDGET_ID)
}

internal class FileHealthWidgetFactory : StatusBarWidgetFactory {
    override fun getId(): String = FILE_HEALTH_WIDGET_ID

    override fun getDisplayName(): String = LspfAnalysisBundle.message("status.widgetName")

    override fun createWidget(project: Project): StatusBarWidget = FileHealthWidget(project)
}

/**
 * The active file's health, as one entry in the status bar.
 *
 * It follows the editor rather than the last analysis: a summary for a file in
 * another tab would be worse than no summary at all.
 *
 * The tooltip carries the shape of the file -- the quality bar, the band
 * spread, the counts. Everything a reader would act on is in the popup instead,
 * because a Swing tooltip cannot be clicked the way the VS Code client's
 * Markdown hover can.
 */
internal class FileHealthWidget(private val project: Project) :
    StatusBarWidget,
    StatusBarWidget.MultipleTextValuesPresentation {

    private var statusBar: StatusBar? = null

    /** What the widget was last set to, so it is not set to it again. */
    private var shown: StatusPresentation? = null

    override fun ID(): String = FILE_HEALTH_WIDGET_ID

    override fun getPresentation(): StatusBarWidget.WidgetPresentation = this

    override fun install(statusBar: StatusBar) {
        this.statusBar = statusBar

        val connection = project.messageBus.connect(this)
        connection.subscribe(FileHealthService.TOPIC, FileHealthService.Listener { refresh() })
        connection.subscribe(
            FileEditorManagerListener.FILE_EDITOR_MANAGER,
            object : FileEditorManagerListener {
                override fun selectionChanged(event: FileEditorManagerEvent) = refresh()

                override fun fileClosed(source: FileEditorManager, file: VirtualFile) {
                    LspfAnalysisClient.fileUri(project, file)?.let {
                        FileHealthService.getInstance(project).forget(it)
                    }
                }
            },
        )
        refresh()
    }

    override fun getSelectedValue(): String? = shown?.text

    override fun getIcon(): Icon? = shown?.let { LspfAnalysisIcons.badge(it.grade) }

    override fun getTooltipText(): String? = shown?.tooltip

    override fun getPopup(): JBPopup? {
        val health = currentHealth() ?: return null
        val group = DefaultActionGroup()

        for (worst in health.worst) {
            group.add(GoToWorstFunction(project, health.uri, worst))
        }
        if (health.worst.isNotEmpty()) {
            group.addSeparator()
        }
        group.add(ShowProblems(project))
        group.add(ShowSettings(project))
        // Borrowed from the registered action so the row reads the same as it
        // does in Find Action, without a second copy of the string.
        ActionManager.getInstance().getAction(RESTART_ACTION_ID)?.templatePresentation?.let {
            group.add(RestartServer(project, it.text, it.icon))
        }

        return JBPopupFactory.getInstance().createActionGroupPopup(
            LspfAnalysisBundle.message("status.worstHeader"),
            group,
            DataContext.EMPTY_CONTEXT,
            JBPopupFactory.ActionSelectionAid.SPEEDSEARCH,
            false,
        )
    }

    override fun dispose() {
        statusBar = null
        shown = null
    }

    /**
     * Recomputes what to show, and repaints only if it actually changed.
     *
     * The server republishes on every keystroke, and a widget that repainted
     * each time would flicker for no new information.
     */
    private fun refresh() {
        ApplicationManager.getApplication().invokeLater({
            val next = currentHealth()?.let { renderStatus(it) }
            if (next != shown) {
                shown = next
                statusBar?.updateWidget(ID())
            }
        }, project.disposed)
    }

    /** The active document's summary, when there is one worth showing. */
    private fun currentHealth(): FileHealth? {
        if (!LspfAnalysisSettings.getInstance(project).state.statusBarEnabled) {
            return null
        }
        val file = FileEditorManager.getInstance(project).selectedEditor?.file ?: return null
        val uri = LspfAnalysisClient.fileUri(project, file) ?: return null
        return FileHealthService.getInstance(project).report(uri)
    }

    private companion object {
        const val RESTART_ACTION_ID = "LspfAnalysis.RestartServer"
    }
}

private class GoToWorstFunction(
    private val project: Project,
    private val uri: String,
    private val worst: WorstFunction,
) : DumbAwareAction(worstLabel(worst), null, LspfAnalysisIcons.badge(worst.grade)) {

    override fun actionPerformed(event: AnActionEvent) {
        navigateToFunction(project, uri, worst.line)
    }
}

// The popup is opened with an empty data context -- a status bar widget has no
// editor or project to hand out -- so these carry the project themselves.
private class ShowProblems(private val project: Project) :
    DumbAwareAction(
        LspfAnalysisBundle.message("action.problems"),
        null,
        AllIcons.Toolwindows.ToolWindowProblems,
    ) {
    override fun actionPerformed(event: AnActionEvent) {
        ToolWindowManager.getInstance(project).getToolWindow(ToolWindowId.PROBLEMS_VIEW)?.activate(null)
    }
}

private class RestartServer(private val project: Project, text: String?, icon: Icon?) :
    DumbAwareAction(text, null, icon) {
    override fun actionPerformed(event: AnActionEvent) {
        LspfAnalysisClient.restart(project)
    }
}

private class ShowSettings(private val project: Project) :
    DumbAwareAction(
        LspfAnalysisBundle.message("action.settings"),
        null,
        AllIcons.General.Settings,
    ) {
    override fun actionPerformed(event: AnActionEvent) {
        ShowSettingsUtil.getInstance().showSettingsDialog(project, LspfAnalysisConfigurable::class.java)
    }
}

package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.LspfAnalysisBundle
import com.github.meymchen.lspfanalysis.LspfAnalysisIcons
import com.github.meymchen.lspfanalysis.lsp.FileHealthService
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClient
import com.github.meymchen.lspfanalysis.lsp.isAnalyzable
import com.github.meymchen.lspfanalysis.model.FunctionDetail
import com.github.meymchen.lspfanalysis.model.FunctionHealth
import com.github.meymchen.lspfanalysis.model.Measure
import com.github.meymchen.lspfanalysis.model.Pillar
import com.github.meymchen.lspfanalysis.model.functionDescription
import com.github.meymchen.lspfanalysis.model.functionLabel
import com.github.meymchen.lspfanalysis.model.functionTooltip
import com.github.meymchen.lspfanalysis.model.gradeOf
import com.github.meymchen.lspfanalysis.model.measureDescription
import com.github.meymchen.lspfanalysis.model.measureLabel
import com.github.meymchen.lspfanalysis.model.measureTooltip
import com.github.meymchen.lspfanalysis.model.pillarDescription
import com.github.meymchen.lspfanalysis.model.pillarLabel
import com.github.meymchen.lspfanalysis.model.sortFunctions
import com.intellij.ide.util.treeView.TreeState
import com.intellij.openapi.Disposable
import com.intellij.openapi.actionSystem.ActionManager
import com.intellij.openapi.actionSystem.ActionPlaces
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonShortcuts
import com.intellij.openapi.actionSystem.DefaultActionGroup
import com.intellij.openapi.application.EDT
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerEvent
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.project.DumbAwareAction
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.SimpleToolWindowPanel
import com.intellij.ui.ColoredTreeCellRenderer
import com.intellij.ui.SimpleTextAttributes
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.treeStructure.Tree
import com.intellij.util.Alarm
import com.intellij.util.ui.tree.TreeUtil
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import java.util.concurrent.atomic.AtomicInteger
import javax.swing.JComponent
import javax.swing.JTree
import javax.swing.SwingUtilities
import javax.swing.tree.DefaultMutableTreeNode
import javax.swing.tree.DefaultTreeModel
import javax.swing.tree.TreeSelectionModel

/**
 * How long to wait before asking again after a republish.
 *
 * The server re-analyzes on every keystroke and announces it with a file
 * summary; refetching the whole breakdown that often would be a request per
 * character typed. Long enough to cover typing, short enough that the view is
 * never visibly stale.
 */
private const val REFRESH_DELAY_MS = 1000

/** The coroutine scope the view's requests run on. */
@Service(Service.Level.PROJECT)
internal class FunctionHealthScope(val scope: CoroutineScope) {
    companion object {
        fun of(project: Project): CoroutineScope = project.service<FunctionHealthScope>().scope
    }
}

/** One row of the tree. */
private sealed interface Row {
    data class Function(val uri: String, val detail: FunctionDetail) : Row
    data class OfPillar(val pillar: Pillar) : Row
    data class OfMeasure(val measure: Measure) : Row
}

/**
 * The file's functions, worst first, expandable down to the individual measure
 * that set each pillar.
 *
 * A hover answers about the function under the pointer, one at a time, and
 * shares its popup with every other provider. This view answers about the whole
 * file at once and keeps its own space, which is what makes it worth having
 * alongside.
 */
internal class FunctionHealthPanel(private val project: Project, parent: Disposable) :
    SimpleToolWindowPanel(true, true) {

    private val root = DefaultMutableTreeNode()
    private val model = DefaultTreeModel(root)
    private val tree = object : Tree(model) {
        override fun getToolTipText(event: MouseEvent): String? = rowAt(event)?.let(::tooltipFor)
    }

    private val alarm = Alarm(Alarm.ThreadToUse.SWING_THREAD, parent)

    /** The answer on screen, so expanding a row costs no round trip. */
    private var loaded: FunctionHealth? = null

    /**
     * Which fetch is current. An answer that arrives after the reader moved to
     * another file is dropped rather than drawn under its name.
     */
    private val generation = AtomicInteger()

    init {
        tree.isRootVisible = false
        tree.showsRootHandles = true
        tree.selectionModel.selectionMode = TreeSelectionModel.SINGLE_TREE_SELECTION
        tree.cellRenderer = RowRenderer()
        tree.emptyText.text = LspfAnalysisBundle.message("toolWindow.empty")
        TreeUtil.installActions(tree)

        // On click and on Enter, but deliberately not on every selection
        // change: navigating moves focus to the editor, and doing that from a
        // selection listener would take the keyboard away mid-arrow-key.
        tree.addMouseListener(object : MouseAdapter() {
            override fun mouseClicked(event: MouseEvent) {
                if (SwingUtilities.isLeftMouseButton(event)) {
                    (rowAt(event) as? Row.Function)?.let(::navigateTo)
                }
            }
        })
        object : DumbAwareAction() {
            override fun actionPerformed(event: AnActionEvent) {
                (selectedRow() as? Row.Function)?.let(::navigateTo)
            }
        }.registerCustomShortcutSet(CommonShortcuts.ENTER, tree, parent)

        setContent(JBScrollPane(tree))
        toolbar = buildToolbar()

        val connection = project.messageBus.connect(parent)
        connection.subscribe(
            FileEditorManagerListener.FILE_EDITOR_MANAGER,
            object : FileEditorManagerListener {
                override fun selectionChanged(event: FileEditorManagerEvent) = refreshNow()
            },
        )
        connection.subscribe(FileHealthService.TOPIC, FileHealthService.Listener { uri -> republished(uri) })
        // Reordering what is on screen costs no round trip.
        connection.subscribe(FunctionHealthSort.TOPIC, FunctionHealthSort.Listener { redraw() })

        refreshNow()
    }

    private fun buildToolbar(): JComponent {
        val group = DefaultActionGroup().apply {
            add(SortByQualityAction())
            add(SortByPositionAction())
        }
        val toolbar = ActionManager.getInstance()
            .createActionToolbar(ActionPlaces.TOOLWINDOW_TITLE, group, true)
        toolbar.targetComponent = tree
        return toolbar.component
    }

    /** Redraws now: the document or the configuration changed. */
    private fun refreshNow() {
        alarm.cancelAllRequests()
        fetchActiveDocument()
    }

    /**
     * Redraws shortly, coalescing the republishes that arrive while typing.
     *
     * What is on screen is left standing until the new answer arrives, so the
     * rows do not blink empty between keystrokes.
     *
     * Called from the connection's own thread, so which document is active is
     * not asked here: the alarm runs on the EDT, and the republishes for other
     * documents are dropped there.
     */
    private fun republished(uri: String?) {
        SwingUtilities.invokeLater {
            if (project.isDisposed || alarm.isDisposed) return@invokeLater
            if (uri != null && uri != activeUri()) return@invokeLater
            alarm.cancelAllRequests()
            alarm.addRequest({
                if (uri == null || uri == activeUri()) {
                    fetchActiveDocument()
                }
            }, REFRESH_DELAY_MS)
        }
    }

    /** Asks the server about whatever document is in front of the reader. */
    private fun fetchActiveDocument() {
        val uri = activeUri()
        if (uri == null) {
            loaded = null
            generation.incrementAndGet()
            redraw()
            return
        }

        val fetch = generation.incrementAndGet()
        FunctionHealthScope.of(project).launch {
            val health = LspfAnalysisClient.functionHealth(project, uri)
            withContext(kotlinx.coroutines.Dispatchers.EDT) {
                if (fetch == generation.get() && health?.uri == uri) {
                    loaded = health
                    redraw()
                }
            }
        }
    }

    /** Rebuilds the rows from what is already loaded, keeping what was open. */
    private fun redraw() {
        val state = TreeState.createOn(tree, root)
        root.removeAllChildren()

        val health = loaded
        if (health != null) {
            val sort = FunctionHealthSort.getInstance(project).sort
            for (detail in sortFunctions(health.functions, sort)) {
                val functionNode = DefaultMutableTreeNode(Row.Function(health.uri, detail))
                for (pillar in detail.pillars) {
                    val pillarNode = DefaultMutableTreeNode(Row.OfPillar(pillar))
                    for (measure in pillar.measures) {
                        pillarNode.add(DefaultMutableTreeNode(Row.OfMeasure(measure)))
                    }
                    functionNode.add(pillarNode)
                }
                root.add(functionNode)
            }
        }

        model.reload()
        state.applyTo(tree, root)
    }

    /** The active document's URI, when it is one the server can analyze. */
    private fun activeUri(): String? {
        val file = FileEditorManager.getInstance(project).selectedEditor?.file ?: return null
        if (!isAnalyzable(file)) {
            return null
        }
        return LspfAnalysisClient.fileUri(project, file)
    }

    private fun navigateTo(row: Row.Function) {
        navigateToFunction(project, row.uri, row.detail.startLine)
    }

    private fun rowAt(event: MouseEvent): Row? {
        val path = tree.getPathForLocation(event.x, event.y) ?: return null
        return (path.lastPathComponent as? DefaultMutableTreeNode)?.userObject as? Row
    }

    private fun selectedRow(): Row? =
        (tree.selectionPath?.lastPathComponent as? DefaultMutableTreeNode)?.userObject as? Row

    private fun tooltipFor(row: Row): String? = when (row) {
        is Row.Function -> functionTooltip(row.detail)
        is Row.OfMeasure -> measureTooltip(row.measure)
        // A pillar's score is the worst of its measures and nothing more, which
        // the row itself already says.
        is Row.OfPillar -> null
    }
}

private class RowRenderer : ColoredTreeCellRenderer() {
    override fun customizeCellRenderer(
        tree: JTree,
        value: Any?,
        selected: Boolean,
        expanded: Boolean,
        leaf: Boolean,
        row: Int,
        hasFocus: Boolean,
    ) {
        when (val node = (value as? DefaultMutableTreeNode)?.userObject) {
            is Row.Function -> {
                icon = LspfAnalysisIcons.badge(node.detail.grade)
                append(functionLabel(node.detail.name))
                append("  ${functionDescription(node.detail)}", SimpleTextAttributes.GRAYED_ATTRIBUTES)
            }

            is Row.OfPillar -> {
                icon = LspfAnalysisIcons.badge(gradeOf(node.pillar.score))
                append(pillarLabel(node.pillar.name))
                append("  ${pillarDescription(node.pillar)}", SimpleTextAttributes.GRAYED_ATTRIBUTES)
            }

            is Row.OfMeasure -> {
                icon = LspfAnalysisIcons.badge(gradeOf(node.measure.score))
                append(measureLabel(node.measure.name))
                append("  ${measureDescription(node.measure)}", SimpleTextAttributes.GRAYED_ATTRIBUTES)
            }
        }
    }
}

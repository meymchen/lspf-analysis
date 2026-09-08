package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.model.Sort
import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.project.DumbAwareAction
import com.intellij.openapi.project.Project
import com.intellij.util.messages.Topic

/**
 * Which order the Function Health view is in.
 *
 * A project service rather than panel state, so the two actions that change it
 * can be ordinary registered actions -- reachable from Find Action, the way the
 * VS Code client's two commands are reachable from the command palette.
 */
@Service(Service.Level.PROJECT)
class FunctionHealthSort(private val project: Project) {

    var sort: Sort = Sort.QUALITY
        set(value) {
            if (field != value) {
                field = value
                project.messageBus.syncPublisher(TOPIC).sortChanged(value)
            }
        }

    fun interface Listener {
        fun sortChanged(sort: Sort)
    }

    companion object {
        @Topic.ProjectLevel
        val TOPIC: Topic<Listener> = Topic.create("LSPF Analysis sort", Listener::class.java)

        fun getInstance(project: Project): FunctionHealthSort = project.service()
    }
}

/**
 * Text, description and icon come from `plugin.xml` and the bundle.
 *
 * Only shown when it would change something, the way the VS Code client's view
 * title shows whichever of the two orders is not the current one.
 */
internal abstract class SortAction(private val target: Sort) : DumbAwareAction() {

    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(event: AnActionEvent) {
        val project = event.project
        event.presentation.isEnabledAndVisible =
            project != null &&
            FunctionHealthSort.getInstance(project).sort != target
    }

    override fun actionPerformed(event: AnActionEvent) {
        event.project?.let { FunctionHealthSort.getInstance(it).sort = target }
    }
}

internal class SortByQualityAction : SortAction(Sort.QUALITY)

internal class SortByPositionAction : SortAction(Sort.POSITION)

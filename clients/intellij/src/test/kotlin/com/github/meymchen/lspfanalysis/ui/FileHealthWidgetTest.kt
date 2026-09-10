package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.lsp.FileHealthService
import com.github.meymchen.lspfanalysis.model.Bands
import com.github.meymchen.lspfanalysis.model.FileHealth
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerEvent
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.util.Disposer
import com.intellij.openapi.wm.StatusBar
import com.intellij.testFramework.LightPlatformTestCase
import com.intellij.util.ui.UIUtil
import java.lang.reflect.Proxy

class FileHealthWidgetTest : LightPlatformTestCase() {
    fun testPublishedSummaryAppearsWithoutSwitchingFiles() {
        withWidget { widget, service ->
            service.publish(report("file:///a.py", 77.0))
            UIUtil.dispatchAllInvocationEvents()
            assertEquals("77%", widget.getSelectedValue())
            assertNotNull(widget.getIcon())
            service.forget("file:///a.py")
            UIUtil.dispatchAllInvocationEvents()
            assertNull(widget.getSelectedValue())
            assertNull(widget.getIcon())
        }
    }

    fun testBackgroundSummaryRefreshesTheSelectedFile() {
        withWidget { widget, service ->
            val publication = Thread { service.publish(report("file:///a.py", 88.0)) }
            publication.start()
            publication.join(5000)
            assertFalse("Publication did not finish", publication.isAlive)
            UIUtil.dispatchAllInvocationEvents()
            assertEquals("88%", widget.getSelectedValue())
        }
    }

    fun testSwitchingFilesUsesTheirOwnCachedSummaries() {
        var selectedUri = "file:///a.py"
        withWidget({ selectedUri }) { widget, service ->
            service.publish(report(selectedUri, 77.0))
            service.publish(report("file:///b.py", 88.0))
            UIUtil.dispatchAllInvocationEvents()
            assertEquals("77%", widget.getSelectedValue())
            selectedUri = "file:///b.py"
            project.messageBus.syncPublisher(FileEditorManagerListener.FILE_EDITOR_MANAGER)
                .selectionChanged(FileEditorManagerEvent(FileEditorManager.getInstance(project), null, null))
            UIUtil.dispatchAllInvocationEvents()
            assertEquals("88%", widget.getSelectedValue())
        }
    }

    private fun withWidget(
        documentUri: () -> String? = { "file:///a.py" },
        check: (FileHealthWidget, FileHealthService) -> Unit,
    ) {
        val widget = FileHealthWidget(project, documentUri)
        val bar = Proxy.newProxyInstance(StatusBar::class.java.classLoader, arrayOf(StatusBar::class.java)) { _, _, _ ->
            null
        } as StatusBar
        try {
            widget.install(bar)
            UIUtil.dispatchAllInvocationEvents()
            assertNull(widget.getSelectedValue())
            check(widget, FileHealthService.getInstance(project))
        } finally {
            Disposer.dispose(widget)
            FileHealthService.getInstance(project).serverInitialized()
        }
    }

    private fun report(uri: String, quality: Double) =
        FileHealth(uri, quality, "good", 1, 0, Bands(0, 1, 0, 0), emptyList())
}

package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.LspfAnalysisBundle
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClientDescriptor
import com.github.meymchen.lspfanalysis.model.FunctionDetail
import com.github.meymchen.lspfanalysis.model.FunctionHealth
import com.intellij.openapi.util.Disposer
import com.intellij.testFramework.LightPlatformTestCase
import com.intellij.ui.treeStructure.Tree
import com.intellij.util.ui.UIUtil
import kotlinx.coroutines.CompletableDeferred
import org.eclipse.lsp4j.InitializeResult

class FunctionHealthPanelTest : LightPlatformTestCase() {
    fun testPanelOpenedAfterServerStopRemembersUnavailableState() {
        val parent = Disposer.newDisposable()
        val listener = LspfAnalysisClientDescriptor(project).lspServerListener
        try {
            listener.serverStopped(true)
            // The platform may not resolve a document URI until its client starts.
            var uri: String? = null
            var calls = 0
            val panel = FunctionHealthPanel(project, parent, load = {
                calls++
                FunctionHealth(it, emptyList())
            }, documentUri = { uri })
            val tree = UIUtil.findComponentOfType(panel, Tree::class.java)!!
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(LspfAnalysisBundle.message("toolWindow.unavailable"), tree.emptyText.text)
            assertEquals(0, calls)
            uri = "file:///a.rs"
            listener.serverInitialized(InitializeResult())
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(1, calls)
            assertEquals(LspfAnalysisBundle.message("toolWindow.noResults"), tree.emptyText.text)
        } finally {
            Disposer.dispose(parent)
            listener.serverInitialized(InitializeResult())
        }
    }

    fun testRequestFailureShowsUnavailableAndEmptyResponseShowsNoResults() {
        val parent = Disposer.newDisposable()
        try {
            var failing = true
            val panel = FunctionHealthPanel(project, parent, load = {
                check(!failing) { "server failed" }
                null
            }, documentUri = { "file:///a.rs" })
            val tree = UIUtil.findComponentOfType(panel, Tree::class.java)!!
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(LspfAnalysisBundle.message("toolWindow.unavailable"), tree.emptyText.text)
            failing = false
            LspfAnalysisClientDescriptor(project).lspServerListener.serverInitialized(InitializeResult())
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(LspfAnalysisBundle.message("toolWindow.noResults"), tree.emptyText.text)
            assertEquals(0, tree.model.getChildCount(tree.model.root))
        } finally {
            Disposer.dispose(parent)
        }
    }

    fun testDisposingPanelCancelsItsRequestAndDisconnectsLifecycleEvents() {
        val parent = Disposer.newDisposable()
        val response = CompletableDeferred<FunctionHealth?>()
        var cancelled = false
        var calls = 0
        try {
            val panel = FunctionHealthPanel(project, parent, load = {
                calls++
                try {
                    response.await()
                } finally {
                    cancelled = true
                }
            }, documentUri = { "file:///a.rs" })
            val tree = UIUtil.findComponentOfType(panel, Tree::class.java)!!
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(1, calls)
            Disposer.dispose(parent)
            UIUtil.dispatchAllInvocationEvents()
            assertTrue(cancelled)
            LspfAnalysisClientDescriptor(project).lspServerListener.serverInitialized(InitializeResult())
            response.complete(FunctionHealth("file:///a.rs", emptyList()))
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(1, calls)
            assertEquals(0, tree.model.getChildCount(tree.model.root))
        } finally {
            Disposer.dispose(parent)
        }
    }

    fun testRestartRestoresRowsEvenWithoutCachedFileSummaries() {
        val parent = Disposer.newDisposable()
        try {
            var calls = 0
            val panel = FunctionHealthPanel(project, parent, load = { uri ->
                calls++
                FunctionHealth(uri, listOf(FunctionDetail("add", 1, 2, 90.0, "excellent", "", "", emptyList())))
            }, documentUri = { "file:///a.rs" })
            val tree = UIUtil.findComponentOfType(panel, Tree::class.java)!!
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(1, tree.model.getChildCount(tree.model.root))

            val listener = LspfAnalysisClientDescriptor(project).lspServerListener
            listener.serverStopped(true)
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(0, tree.model.getChildCount(tree.model.root))
            assertEquals(LspfAnalysisBundle.message("toolWindow.unavailable"), tree.emptyText.text)

            listener.serverInitialized(InitializeResult())
            UIUtil.dispatchAllInvocationEvents()
            assertEquals(2, calls)
            assertEquals(1, tree.model.getChildCount(tree.model.root))
        } finally {
            Disposer.dispose(parent)
        }
    }
}

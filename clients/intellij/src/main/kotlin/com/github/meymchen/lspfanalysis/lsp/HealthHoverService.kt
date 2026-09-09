package com.github.meymchen.lspfanalysis.lsp

import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.diagnostic.thisLogger
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.psi.PsiManager
import kotlinx.coroutines.CoroutineScope

@Service(Service.Level.PROJECT)
internal class HealthHoverService(private val project: Project, scope: CoroutineScope) {
    private val cache = HealthHoverCache(scope)

    init {
        project.messageBus.connect().subscribe(
            FileEditorManagerListener.FILE_EDITOR_MANAGER,
            object : FileEditorManagerListener {
                override fun fileClosed(source: com.intellij.openapi.fileEditor.FileEditorManager, file: VirtualFile) {
                    cache.forget(file.url)
                }
            },
        )
    }

    fun hover(file: VirtualFile, line: Int, character: Int): String? {
        val document = FileDocumentManager.getInstance().getCachedDocument(file) ?: return null
        val stamp = document.modificationStamp
        return cache.get(
            file.url,
            stamp,
            HealthPosition(line, character),
            { position -> LspfAnalysisClient.requestHealthHover(project, file, position.line, position.character) },
            {
                ApplicationManager.getApplication().invokeLater({
                    if (!project.isDisposed && file.isValid && document.modificationStamp == stamp) {
                        val psiFile = PsiManager.getInstance(project).findFile(file)
                        if (psiFile != null) DaemonCodeAnalyzer.getInstance(project).restart(psiFile)
                    }
                }, project.disposed)
            },
            { error -> thisLogger().warn("Could not load health gutter hover", error) },
        )
    }

    fun forget(uri: String) {
        LspfAnalysisClient.findFile(project, uri)?.let { cache.forget(it.url) }
    }

    fun clear() = cache.clear()

    companion object {
        fun getInstance(project: Project): HealthHoverService = project.service()
    }
}

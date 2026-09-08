package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.LspfAnalysisIcons
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClient
import com.github.meymchen.lspfanalysis.lsp.isAnalyzable
import com.github.meymchen.lspfanalysis.settings.LspfAnalysisSettings
import com.intellij.codeInsight.daemon.LineMarkerInfo
import com.intellij.codeInsight.daemon.LineMarkerProvider
import com.intellij.openapi.editor.markup.GutterIconRenderer
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.psi.PsiDocumentManager
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiNameIdentifierOwner

/** Health tooltips do not participate in documentation target selection. */
class HealthLineMarkerProvider internal constructor(private val hover: (Project, VirtualFile, Int, Int) -> String?) :
    LineMarkerProvider {
    constructor() : this(LspfAnalysisClient::healthHover)

    override fun getLineMarkerInfo(element: PsiElement): LineMarkerInfo<*>? = null

    override fun collectSlowLineMarkers(elements: List<PsiElement>, result: MutableCollection<in LineMarkerInfo<*>>) {
        val project = elements.firstOrNull()?.project ?: return
        if (!LspfAnalysisSettings.getInstance(project).state.gutterIconsEnabled) return

        for (element in elements) {
            ProgressManager.checkCanceled()
            if (element.firstChild != null) continue
            val owner = element.parent as? PsiNameIdentifierOwner ?: continue
            if (owner.nameIdentifier != element) continue
            val psiFile = element.containingFile ?: continue
            val file = psiFile.virtualFile ?: continue
            if (!isAnalyzable(file)) continue
            val document = PsiDocumentManager.getInstance(element.project).getDocument(psiFile) ?: continue
            val offset = element.textRange.startOffset
            val line = document.getLineNumber(offset)
            val markdown = hover(
                element.project,
                file,
                line,
                offset - document.getLineStartOffset(line),
            ) ?: continue
            val html = "<html><body>${healthHoverHtml(element.project, markdown)}</body></html>"
            result.add(
                LineMarkerInfo(
                    element,
                    element.textRange,
                    LspfAnalysisIcons.Logo,
                    { html },
                    null,
                    GutterIconRenderer.Alignment.RIGHT,
                    { "LSPF Analysis" },
                ),
            )
        }
    }
}

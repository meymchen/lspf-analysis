package com.github.meymchen.lspfanalysis.ui

import com.intellij.testFramework.LightPlatformTestCase
import javax.swing.JEditorPane
import javax.swing.text.html.HTMLDocument
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClientDescriptor
import com.intellij.platform.lsp.api.customization.LspHoverDisabled
import com.intellij.codeInsight.daemon.LineMarkerInfo
import com.intellij.codeInsight.documentation.DocumentationManager
import com.intellij.lang.Language
import com.intellij.psi.PsiFileFactory
import com.intellij.psi.util.PsiTreeUtil

class HealthHoverTest : LightPlatformTestCase() {
    fun testAnalysisDoesNotRegisterAnLspDocumentationTarget() {
        assertSame(LspHoverDisabled, LspfAnalysisClientDescriptor(project).lspCustomization.hoverCustomizer)
    }

    fun testClassAndFunctionHealthUseGutterAndKeepJavaDoc() {
        val source = """
            /** Class documentation. */
            class Box {
                /** Function documentation. */
                int area() { return 1; }
            }
        """.trimIndent()
        val file = PsiFileFactory.getInstance(project).createFileFromText(
            "Box.java", Language.findLanguageByID("JAVA")!!, source,
        )
        val provider = HealthLineMarkerProvider { _, _, line, character ->
            val text = source.lines()[line].substring(character)
            if (text.startsWith("Box") || text.startsWith("area")) MARKDOWN else null
        }
        val result = mutableListOf<LineMarkerInfo<*>>()
        provider.collectSlowLineMarkers(PsiTreeUtil.collectElements(file) { true }.toList(), result)
        assertEquals(listOf("Box", "area"), result.map { it.element!!.text })
        for ((marker, expected) in result.zip(listOf("Class documentation.", "Function documentation."))) {
            val name = marker.element!!
            val native = DocumentationManager.getProviderFromElement(name.parent)
                .generateDoc(name.parent, name)
            assertTrue(native.orEmpty(), native?.contains(expected) == true)
            assertTrue(marker.lineMarkerTooltip.orEmpty().contains("100%"))
        }
    }

    fun testEmptyPillarKeepsFourColumns() {
        val html = healthHoverHtml(project, MARKDOWN)
        val rows = Regex("<tr\\b[^>]*>(.*?)</tr>", RegexOption.DOT_MATCHES_ALL).findAll(html).toList()
        assertEquals(3, rows.size)
        for (row in rows) {
            assertEquals(html, 4, Regex("<t[dh]\\b").findAll(row.value).count())
        }
    }

    fun testScoreStaysOnOneLineInNarrowSwingPopup() {
        val html = healthHoverHtml(project, MARKDOWN)
        run {
            val pane = JEditorPane("text/html", html)
            pane.setSize(435, 1000)
            val doc = pane.document as HTMLDocument
            val text = doc.getText(0, doc.length)
            val start = text.indexOf("██████████")
            val end = text.indexOf("100%", start) + 3
            assertTrue(text, start >= 0 && end > start)
            assertEquals(html, pane.modelToView2D(start).y, pane.modelToView2D(end).y)
        }
    }

    companion object {
        private val MARKDOWN = """
            | pillar | metric | value | score |
            | :-- | :-- | --: | :-- |
            | control flow | cognitive complexity | 0 / 15 | ██████████ 100% |
            |  | cyclomatic complexity | 1 / 10 | ██████████ 99% |
        """.trimIndent()
    }
}

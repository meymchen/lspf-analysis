package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClientDescriptor
import com.intellij.codeInsight.daemon.LineMarkerInfo
import com.intellij.codeInsight.documentation.DocumentationManager
import com.intellij.lang.Language
import com.intellij.platform.lsp.api.customization.LspHoverDisabled
import com.intellij.psi.PsiFileFactory
import com.intellij.psi.util.PsiTreeUtil
import com.intellij.testFramework.LightPlatformTestCase

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
            "Box.java",
            Language.findLanguageByID("JAVA")!!,
            source,
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
            assertTrue(marker.lineMarkerTooltip.orEmpty().contains("33%"))
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

    /** The server sends bare letters; the colour is the IDE's to choose. */
    fun testGradeLettersAreColoured() {
        val html = healthHoverHtml(project, MARKDOWN)
        val coloured = Regex("""<span style="color:#([0-9a-f]{6});">([A-D])</span>""")
            .findAll(html)
            .map { it.groupValues[2] to it.groupValues[1] }
            .toList()
        assertEquals(listOf("A", "D"), coloured.map { it.first })
        assertEquals(2, coloured.map { it.second }.toSet().size)
        // The word in the header is not a grade cell, and nor is a value.
        assertFalse(html, html.contains(""">grade</span>"""))
    }

    companion object {
        /** The shape the server renders, for a client that takes no HTML. */
        private val MARKDOWN = """
            **`area`**  ·  quality **33%**  ·  fair

            | grade | pillar | metric | value |
            | :-: | :-- | :-- | --: |
            | A | control flow | cognitive complexity | 0 / 15 |
            | D |  | cyclomatic complexity | 22 / 10 |
        """.trimIndent()
    }
}

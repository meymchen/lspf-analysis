package com.github.meymchen.lspfanalysis.ui

import com.github.meymchen.lspfanalysis.lsp.HealthHoverCache
import com.github.meymchen.lspfanalysis.lsp.HealthPosition
import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClientDescriptor
import com.intellij.codeInsight.daemon.LineMarkerInfo
import com.intellij.codeInsight.documentation.DocumentationManager
import com.intellij.lang.Language
import com.intellij.platform.lsp.api.customization.LspHoverDisabled
import com.intellij.psi.PsiFileFactory
import com.intellij.psi.util.PsiTreeUtil
import com.intellij.testFramework.LightPlatformTestCase
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout

class HealthHoverTest : LightPlatformTestCase() {
    fun testSlowHealthResponseAddsGutterMarkersOnTheRefreshPass() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val source = "class Box { int area() { return 1; } }"
            val file = PsiFileFactory.getInstance(project).createFileFromText(
                "Box.java",
                Language.findLanguageByID("JAVA")!!,
                source,
            )
            val cache = HealthHoverCache(scope)
            val release = CompletableDeferred<Unit>()
            val refresh = CompletableDeferred<Unit>()
            val provider = HealthLineMarkerProvider { _, _, line, character ->
                cache.get(
                    "Box.java",
                    1,
                    HealthPosition(line, character),
                    {
                        release.await()
                        delay(150)
                        MARKDOWN
                    },
                    { refresh.complete(Unit) },
                    { throw it },
                )
            }
            val elements = PsiTreeUtil.collectElements(file) { true }.toList()
            val markers = mutableListOf<LineMarkerInfo<*>>()
            provider.collectSlowLineMarkers(elements, markers)
            assertTrue(markers.isEmpty())
            release.complete(Unit)
            withTimeout(5000) { refresh.await() }
            provider.collectSlowLineMarkers(elements, markers)
            assertEquals(listOf("Box", "area"), markers.map { it.element!!.text })
        } finally {
            scope.cancel()
        }
    }

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

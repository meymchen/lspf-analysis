package com.github.meymchen.lspfanalysis.model

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

class MessagesTest {

    /**
     * The translator is global, and a platform test that runs first boots the
     * IDE and lets `LspfAnalysisStartup` install one. Clearing it before each
     * test as well as after makes "with no translator installed" a state this
     * test establishes rather than one it inherits from whatever ran before.
     */
    @Before
    @After
    fun restoreEnglish() {
        Messages.useTranslator(null)
    }

    @Test
    fun `with no translator installed, strings are the English of the bundle`() {
        assertEquals("Worth opening first", t("status.worstHeader"))
        assertEquals("Go to line 84", t("status.goToLine", 84))
    }

    @Test
    fun `an installed translator is used instead`() {
        Messages.useTranslator { key, args -> "[$key:${args.joinToString()}]" }
        assertEquals("[status.goToLine:84]", t("status.goToLine", 84))
    }

    @Test
    fun `a key with no entry shows itself rather than throwing`() {
        assertEquals("pillar.somethingNew", t("pillar.somethingNew"))
    }

    @Test
    fun `the vocabulary the server sends is translated at the point it is drawn`() {
        assertEquals("control flow", pillarLabel("control flow"))
        assertEquals("Halstead difficulty", measureLabel("Halstead difficulty"))
        assertEquals("excellent", gradeLabel("excellent"))
    }

    @Test
    fun `vocabulary from a newer server is shown as it arrived, not dropped`() {
        assertEquals("coupling", pillarLabel("coupling"))
        assertEquals("fan-out", measureLabel("fan-out"))
        assertEquals("stellar", gradeLabel("stellar"))
    }

    @Test
    fun `the server's stand-in for an unnamed function is ours to say`() {
        assertEquals("<anonymous>", functionLabel("<anonymous>"))
        assertEquals("parse", functionLabel("parse"))
    }
}

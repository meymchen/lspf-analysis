package com.github.meymchen.lspfanalysis.model

import com.google.gson.JsonParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class StatusTest {

    private fun health(
        quality: Double = 63.4,
        grade: String = "good",
        functions: Int = 7,
        below: Int = 1,
        bands: Bands = Bands(excellent = 4, good = 2, fair = 0, poor = 1),
        worst: List<WorstFunction> = emptyList(),
    ) = FileHealth("file:///src/lib.rs", quality, grade, functions, below, bands, worst)

    @Test
    fun `the widget shows the score and how many want attention`() {
        assertEquals("63%  ⚠1", renderStatus(health()).text)
    }

    @Test
    fun `nothing below the threshold means nothing to flag`() {
        assertEquals("63%", renderStatus(health(below = 0)).text)
    }

    @Test
    fun `only a file bad enough to act on is coloured`() {
        assertFalse(renderStatus(health(grade = "excellent")).warning)
        assertFalse(renderStatus(health(grade = "good")).warning)
        assertTrue(renderStatus(health(grade = "fair")).warning)
        assertTrue(renderStatus(health(grade = "poor")).warning)
    }

    @Test
    fun `the tooltip counts the functions and spells out the bands`() {
        val tooltip = renderStatus(health()).tooltip
        assertTrue(tooltip, tooltip.contains("7 functions, 1 below the warning threshold."))
        assertTrue(tooltip, tooltip.contains("excellent"))
        assertTrue(tooltip, tooltip.contains("poor"))
    }

    @Test
    fun `one function is said in the singular`() {
        val tooltip = renderStatus(health(functions = 1, bands = Bands(0, 1, 0, 0))).tooltip
        assertTrue(tooltip, tooltip.contains("1 function, 1 below the warning threshold."))
    }

    @Test
    fun `a file with nothing to analyze says so instead of counting`() {
        val tooltip = renderStatus(health(functions = 0, below = 0, bands = Bands(0, 0, 0, 0))).tooltip
        assertTrue(tooltip, tooltip.contains("No functions to analyze in this file."))
    }

    @Test
    fun `a popup row names the function and what dragged it`() {
        val worst = WorstFunction(
            name = "tangled",
            quality = 22.1,
            grade = "poor",
            line = 84,
            weakestPillar = "control flow",
            weakestMetric = "cognitive complexity",
        )
        assertEquals(
            "tangled — 22%, weakest control flow (cognitive complexity)",
            worstLabel(worst),
        )
    }

    @Test
    fun `a pillar with one measure names only the pillar`() {
        val worst = WorstFunction("wide", 40.0, "fair", 12, "interface", "")
        assertEquals("wide — 40%, weakest interface", worstLabel(worst))
    }

    private val payload = """
        {
          "uri": "file:///src/lib.rs",
          "quality": 63.4,
          "grade": "good",
          "functions": 7,
          "below": 1,
          "bands": { "excellent": 4, "good": 2, "fair": 0, "poor": 1 },
          "worst": [
            {
              "name": "tangled",
              "quality": 22.1,
              "grade": "poor",
              "line": 84,
              "weakestPillar": "control flow",
              "weakestMetric": "cognitive complexity"
            }
          ]
        }
    """.trimIndent()

    @Test
    fun `the documented notification parses`() {
        val parsed = parseFileHealth(JsonParser.parseString(payload))
        assertNotNull(parsed)
        assertEquals(7, parsed!!.functions)
        assertEquals(4, parsed.bands.excellent)
        assertEquals(84, parsed.worst.single().line)
    }

    @Test
    fun `an unrecognizable notification is ignored rather than rendered`() {
        val cases = listOf(
            "null",
            "\"nonsense\"",
            """{"uri": "file:///a.rs", "quality": "high", "grade": "good", "functions": 1,
               "below": 0, "bands": {"excellent":1,"good":0,"fair":0,"poor":0}, "worst": []}""",
            // A bands object missing one of its four counts.
            """{"uri": "file:///a.rs", "quality": 50.0, "grade": "good", "functions": 1,
               "below": 0, "bands": {"excellent":1,"good":0,"fair":0}, "worst": []}""",
            // A worst entry without the line to jump to.
            """{"uri": "file:///a.rs", "quality": 50.0, "grade": "good", "functions": 1,
               "below": 0, "bands": {"excellent":1,"good":0,"fair":0,"poor":0},
               "worst": [{"name": "a", "quality": 1.0, "grade": "poor"}]}""",
        )
        for (case in cases) {
            assertNull(case, parseFileHealth(JsonParser.parseString(case)))
        }
    }
}

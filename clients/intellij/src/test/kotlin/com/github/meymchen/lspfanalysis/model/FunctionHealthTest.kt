package com.github.meymchen.lspfanalysis.model

import com.google.gson.JsonParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FunctionHealthTest {

    private fun detail(name: String, quality: Double, startLine: Int) = FunctionDetail(
        name = name,
        startLine = startLine,
        endLine = startLine + 10,
        quality = quality,
        grade = gradeOf(quality),
        weakestPillar = "control flow",
        weakestMetric = "cognitive complexity",
        pillars = emptyList(),
    )

    @Test
    fun `worst first by default`() {
        val functions = listOf(detail("a", 80.0, 1), detail("b", 20.0, 40), detail("c", 50.0, 20))
        assertEquals(
            listOf("b", "c", "a"),
            sortFunctions(functions, Sort.QUALITY).map { it.name },
        )
    }

    @Test
    fun `ties break toward the earlier function, so the order is stable`() {
        val functions = listOf(detail("late", 30.0, 90), detail("early", 30.0, 12))
        assertEquals(
            listOf("early", "late"),
            sortFunctions(functions, Sort.QUALITY).map { it.name },
        )
    }

    @Test
    fun `source order ignores the score`() {
        val functions = listOf(detail("a", 10.0, 40), detail("b", 90.0, 5))
        assertEquals(listOf("b", "a"), sortFunctions(functions, Sort.POSITION).map { it.name })
    }

    @Test
    fun `sorting leaves the server's answer alone`() {
        val functions = listOf(detail("a", 80.0, 1), detail("b", 20.0, 40))
        sortFunctions(functions, Sort.QUALITY)
        assertEquals(listOf("a", "b"), functions.map { it.name })
    }

    @Test
    fun `a measure is shown against the threshold it was judged by`() {
        val measure = Measure(name = "statements", value = 41.4, threshold = 30.0, score = 11.9)
        assertEquals("41 / 30  ·  12%", measureDescription(measure))
    }

    @Test
    fun `a function is shown with its score and what dragged it`() {
        assertEquals("22%  ·  control flow", functionDescription(detail("tangled", 22.1, 84)))
    }

    private val payload = """
        {
          "uri": "file:///src/lib.rs",
          "functions": [
            {
              "name": "tangled",
              "startLine": 84,
              "endLine": 131,
              "quality": 22.1,
              "grade": "poor",
              "weakestPillar": "control flow",
              "weakestMetric": "cognitive complexity",
              "pillars": [
                {
                  "name": "control flow",
                  "score": 11.9,
                  "measures": [
                    { "name": "cognitive complexity", "value": 41, "threshold": 15, "score": 11.9 }
                  ]
                }
              ]
            }
          ]
        }
    """.trimIndent()

    @Test
    fun `the documented payload parses down to the measure`() {
        val health = parseFunctionHealth(JsonParser.parseString(payload))
        assertNotNull(health)
        assertEquals("file:///src/lib.rs", health!!.uri)
        assertEquals(1, health.functions.size)

        val function = health.functions.single()
        assertEquals("tangled", function.name)
        assertEquals(84, function.startLine)
        assertEquals(131, function.endLine)

        val measure = function.pillars.single().measures.single()
        assertEquals("cognitive complexity", measure.name)
        assertEquals(41.0, measure.value, 0.0)
        assertEquals(15.0, measure.threshold, 0.0)
    }

    @Test
    fun `a document the server has not analyzed answers with nothing`() {
        assertNull(parseFunctionHealth(null))
        assertNull(parseFunctionHealth(JsonParser.parseString("null")))
    }

    @Test
    fun `an answer of the wrong shape draws nothing rather than NaN`() {
        val cases = listOf(
            """{"functions": []}""",
            """{"uri": 7, "functions": []}""",
            """{"uri": "file:///a.rs"}""",
            """{"uri": "file:///a.rs", "functions": [{"name": "a"}]}""",
            """{"uri": "file:///a.rs", "functions": [{"name": "a", "startLine": 1, "endLine": 2,
               "quality": "bad", "grade": "poor", "pillars": []}]}""",
        )
        for (case in cases) {
            assertNull(case, parseFunctionHealth(JsonParser.parseString(case)))
        }
    }

    @Test
    fun `an empty weakest metric is a value, not a malformation`() {
        val single = """
            {"uri": "file:///a.rs", "functions": [{"name": "wide", "startLine": 1, "endLine": 3,
             "quality": 40.0, "grade": "fair", "weakestPillar": "interface",
             "weakestMetric": "", "pillars": []}]}
        """.trimIndent()
        val health = parseFunctionHealth(JsonParser.parseString(single))
        assertEquals("", health!!.functions.single().weakestMetric)
    }

    @Test
    fun `a tooltip escapes the name it was given`() {
        val tooltip = functionTooltip(detail("<anonymous>", 22.0, 84))
        assertTrue(tooltip, tooltip.contains("&lt;anonymous&gt;"))
        assertTrue(tooltip, tooltip.contains("Lines 84–94."))
    }
}

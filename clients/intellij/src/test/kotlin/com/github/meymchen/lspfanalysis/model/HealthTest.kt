package com.github.meymchen.lspfanalysis.model

import org.junit.Assert.assertEquals
import org.junit.Test

class HealthTest {

    @Test
    fun `a bar is ten cells wide whatever the score`() {
        for (score in listOf(0.0, 12.5, 50.0, 99.9, 100.0)) {
            assertEquals(10, bar(score).length)
        }
    }

    @Test
    fun `a bar fills in proportion, rounded, and never past its ends`() {
        assertEquals("░░░░░░░░░░", bar(0.0))
        assertEquals("█████░░░░░", bar(50.0))
        assertEquals("██████████", bar(100.0))
        // Out of range on either side clamps rather than throwing or overflowing.
        assertEquals("░░░░░░░░░░", bar(-20.0))
        assertEquals("██████████", bar(140.0))
    }

    @Test
    fun `grades fall on the same cuts the server scores with`() {
        assertEquals("excellent", gradeOf(80.0))
        assertEquals("good", gradeOf(79.9))
        assertEquals("good", gradeOf(50.0))
        assertEquals("fair", gradeOf(49.9))
        assertEquals("fair", gradeOf(25.0))
        assertEquals("poor", gradeOf(24.9))
        assertEquals("poor", gradeOf(0.0))
    }

    @Test
    fun `the band line is twenty cells, worst band last`() {
        val bands = Bands(excellent = 2, good = 1, fair = 0, poor = 1)
        val line = bandLine(bands, total = 4)
        assertEquals(20, line.length)
        assertEquals("██████████▓▓▓▓▓░░░░░", line)
    }

    @Test
    fun `a file with no functions has no band line to draw`() {
        assertEquals("", bandLine(Bands(0, 0, 0, 0), total = 0))
    }

    @Test
    fun `a band line is padded out when rounding leaves it short`() {
        // Three equal bands round to 7 + 7 + 7 = 21 cells before the cut, and
        // seven thirds of nothing for the fourth; either way it ends at twenty.
        val line = bandLine(Bands(excellent = 1, good = 1, fair = 1, poor = 0), total = 3)
        assertEquals(20, line.length)
    }
}

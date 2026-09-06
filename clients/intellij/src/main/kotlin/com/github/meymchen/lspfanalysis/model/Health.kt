package com.github.meymchen.lspfanalysis.model

import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** How many cells a score bar is drawn with. */
private const val BAR_CELLS = 10

/** How many cells the band spread is drawn with. */
private const val BAND_CELLS = 20

/** Rounds a metric the way the server's hover does, so the two never disagree. */
internal fun rounded(value: Double): Int = value.roundToInt()

/**
 * Draws a 0-100 score as a bar, the way the server draws them in hovers.
 *
 * Never wrapped in a code span: the IDE draws inline code with a background and
 * horizontal padding, which puts a gap on either side of the bar.
 */
fun bar(score: Double): String {
    val filled = min(BAR_CELLS, max(0, ((score / 100) * BAR_CELLS).roundToInt()))
    return "█".repeat(filled) + "░".repeat(BAR_CELLS - filled)
}

/** The band a score falls in, by the same cuts the server scores with. */
fun gradeOf(score: Double): String = when {
    score >= 80 -> "excellent"
    score >= 50 -> "good"
    score >= 25 -> "fair"
    else -> "poor"
}

/**
 * Renders the band spread as one line of bars.
 *
 * A single percentage says how the file scores; this says how it is shaped,
 * which is what tells a reader whether one bad function is dragging an
 * otherwise healthy file down.
 */
fun bandLine(bands: Bands, total: Int): String {
    if (total == 0) {
        return ""
    }
    val order = listOf(
        bands.excellent to "█",
        bands.good to "▓",
        bands.fair to "▒",
        bands.poor to "░",
    )
    val drawn = buildString {
        for ((count, glyph) in order) {
            append(glyph.repeat(((count.toDouble() / total) * BAND_CELLS).roundToInt()))
        }
    }
    return drawn.take(BAND_CELLS).padEnd(BAND_CELLS, '░')
}

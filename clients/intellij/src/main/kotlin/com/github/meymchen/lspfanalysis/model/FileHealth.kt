package com.github.meymchen.lspfanalysis.model

import com.google.gson.JsonElement

/** The `lspfAnalysis/fileHealth` notification the server pushes per file. */
const val FILE_HEALTH_METHOD: String = "lspfAnalysis/fileHealth"

data class Bands(
    val excellent: Int,
    val good: Int,
    val fair: Int,
    val poor: Int,
)

data class WorstFunction(
    val name: String,
    val quality: Double,
    val grade: String,
    /** Where it starts, 1-based. */
    val line: Int,
    val weakestPillar: String,
    val weakestMetric: String,
)

data class FileHealth(
    val uri: String,
    val quality: Double,
    val grade: String,
    val functions: Int,
    /** How many functions already carry a diagnostic. */
    val below: Int,
    val bands: Bands,
    /** Worst first, capped by the server's `worstFunctions` setting. */
    val worst: List<WorstFunction>,
)

/**
 * Reads a `lspfAnalysis/fileHealth` payload, or `null` when it is not the shape
 * this client expects.
 *
 * An unrecognizable payload is ignored rather than rendered as `NaN%`.
 */
fun parseFileHealth(payload: JsonElement?): FileHealth? {
    val root = payload.objectOrNull() ?: return null
    val bands = root.obj("bands")?.let { band ->
        Bands(
            excellent = band.int("excellent") ?: return null,
            good = band.int("good") ?: return null,
            fair = band.int("fair") ?: return null,
            poor = band.int("poor") ?: return null,
        )
    } ?: return null

    val worst = root.array("worst") ?: return null
    // A malformed entry rejects the whole payload rather than being skipped:
    // half a "worth opening first" list is worse than none.
    val worstFunctions = worst.map { entry ->
        val item = entry.objectOrNull() ?: return null
        WorstFunction(
            name = item.string("name") ?: return null,
            quality = item.number("quality") ?: return null,
            grade = item.string("grade") ?: return null,
            line = item.int("line") ?: return null,
            // The server leaves the metric empty for a pillar with a single
            // measure, so an absent one is a value rather than a malformation.
            weakestPillar = item.string("weakestPillar").orEmpty(),
            weakestMetric = item.string("weakestMetric").orEmpty(),
        )
    }

    return FileHealth(
        uri = root.string("uri") ?: return null,
        quality = root.number("quality") ?: return null,
        grade = root.string("grade") ?: return null,
        functions = root.int("functions") ?: return null,
        below = root.int("below") ?: return null,
        bands = bands,
        worst = worstFunctions,
    )
}

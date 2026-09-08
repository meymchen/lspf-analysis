package com.github.meymchen.lspfanalysis.model

import com.google.gson.JsonElement

/** The `lspfAnalysis/functionHealth` request the server answers per file. */
const val FUNCTION_HEALTH_METHOD: String = "lspfAnalysis/functionHealth"

data class Measure(
    val name: String,
    /** The raw value the engine computed. */
    val value: Double,
    /** The value that would score 50%. */
    val threshold: Double,
    val score: Double,
)

data class Pillar(
    val name: String,
    /** The worst of [measures], never their average. */
    val score: Double,
    val measures: List<Measure>,
)

data class FunctionDetail(
    val name: String,
    /** Where it starts, 1-based. */
    val startLine: Int,
    /** Where it ends, 1-based. */
    val endLine: Int,
    val quality: Double,
    val grade: String,
    val weakestPillar: String,
    val weakestMetric: String,
    val pillars: List<Pillar>,
)

data class FunctionHealth(
    val uri: String,
    /** Every function of the file, in source order. */
    val functions: List<FunctionDetail>,
)

/** How the view orders the file's functions. */
enum class Sort { QUALITY, POSITION }

/**
 * Orders the functions for display, without disturbing the server's answer.
 *
 * Worst first by default, because the plugin's whole claim is that it tells you
 * which functions are getting hard to work with; source order is there for
 * reading alongside the file.
 */
fun sortFunctions(functions: List<FunctionDetail>, sort: Sort): List<FunctionDetail> = when (sort) {
    // Ties break toward the earlier function, so the order is stable between
    // keystrokes rather than shuffling on every republish.
    Sort.QUALITY -> functions.sortedWith(compareBy({ it.quality }, { it.startLine }))
    Sort.POSITION -> functions.sortedBy { it.startLine }
}

/** The line beside a function's name: what it scored, and what dragged it. */
fun functionDescription(detail: FunctionDetail): String =
    "${rounded(detail.quality)}%  ·  ${pillarLabel(detail.weakestPillar)}"

/** The line beside a pillar's name. */
fun pillarDescription(pillar: Pillar): String = "${rounded(pillar.score)}%"

/**
 * The line beside a measure's name.
 *
 * The raw value against its threshold, then what that scored -- a reader can
 * see both what was counted and how it was judged.
 */
fun measureDescription(measure: Measure): String =
    "${rounded(measure.value)} / ${rounded(measure.threshold)}  ·  ${rounded(measure.score)}%"

/** The HTML shown when the pointer rests on a function. */
fun functionTooltip(detail: FunctionDetail): String = html {
    line(
        "<b><code>${escape(functionLabel(detail.name))}</code></b>  ·  " +
            "${escape(t("label.quality"))} <b>${rounded(detail.quality)}%</b>  ·  " +
            escape(gradeLabel(detail.grade)),
    )
    line("<code>${bar(detail.quality)}</code>")
    line(escape(t("function.lines", detail.startLine, detail.endLine)))
    if (detail.weakestMetric.isNotEmpty()) {
        // Already carries markup, so only the substituted names are escaped.
        line(
            t(
                "function.weakest",
                escape(pillarLabel(detail.weakestPillar)),
                escape(measureLabel(detail.weakestMetric)),
            ),
        )
    }
}

/** The HTML shown when the pointer rests on a measure. */
fun measureTooltip(measure: Measure): String = html {
    line("<b>${escape(measureLabel(measure.name))}</b>  ·  ${rounded(measure.score)}%")
    line("<code>${bar(measure.score)}</code>")
    line(escape(t("measure.detail", rounded(measure.value), rounded(measure.threshold))))
}

/**
 * Reads a `lspfAnalysis/functionHealth` response, or `null` when it is not the
 * shape this client expects.
 *
 * The answer is `null` for a document the server has not analyzed, which lands
 * here as the same "nothing to draw".
 */
fun parseFunctionHealth(payload: JsonElement?): FunctionHealth? {
    val root = payload.objectOrNull() ?: return null
    val functions = root.array("functions") ?: return null

    return FunctionHealth(
        uri = root.string("uri") ?: return null,
        functions = functions.map { entry ->
            val detail = entry.objectOrNull() ?: return null
            val pillars = detail.array("pillars") ?: return null
            FunctionDetail(
                name = detail.string("name") ?: return null,
                startLine = detail.int("startLine") ?: return null,
                endLine = detail.int("endLine") ?: return null,
                quality = detail.number("quality") ?: return null,
                grade = detail.string("grade") ?: return null,
                weakestPillar = detail.string("weakestPillar").orEmpty(),
                weakestMetric = detail.string("weakestMetric").orEmpty(),
                pillars = pillars.map { pillarEntry ->
                    val pillar = pillarEntry.objectOrNull() ?: return null
                    val measures = pillar.array("measures") ?: return null
                    Pillar(
                        name = pillar.string("name") ?: return null,
                        score = pillar.number("score") ?: return null,
                        measures = measures.map { measureEntry ->
                            val measure = measureEntry.objectOrNull() ?: return null
                            Measure(
                                name = measure.string("name") ?: return null,
                                value = measure.number("value") ?: return null,
                                threshold = measure.number("threshold") ?: return null,
                                score = measure.number("score") ?: return null,
                            )
                        },
                    )
                },
            )
        },
    )
}

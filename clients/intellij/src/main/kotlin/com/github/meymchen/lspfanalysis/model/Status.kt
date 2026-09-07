package com.github.meymchen.lspfanalysis.model

/** What the status bar draws for one file. */
data class StatusPresentation(
    /** The widget's own text: the score, and how many functions want attention. */
    val text: String,
    /** The band, which decides the widget's icon. */
    val grade: String,
    /** HTML, which the widget hands to Swing as its tooltip. */
    val tooltip: String,
    /** Set only when the file is bad enough to be worth colouring. */
    val warning: Boolean,
)

/**
 * Renders a file's health for the status bar.
 *
 * The widget itself has room for the band and the number; everything a reader
 * would act on goes in the tooltip, or -- where it has to be clickable -- in
 * the popup the widget opens, since a Swing tooltip cannot be clicked through
 * the way a VS Code Markdown hover can.
 */
fun renderStatus(health: FileHealth): StatusPresentation {
    val quality = rounded(health.quality)
    val attention = if (health.below > 0) "  ⚠${health.below}" else ""

    val tooltip = html {
        line(
            "<b>${escape(t("plugin.name"))}</b>  ·  <b>$quality%</b>  ·  " +
                escape(gradeLabel(health.grade))
        )
        line("<code>${bar(health.quality)}</code>")

        if (health.functions == 0) {
            line(escape(t("status.noFunctions")))
        } else {
            line(
                escape(
                    if (health.functions == 1) {
                        t("status.oneFunction", 1, health.below)
                    } else {
                        t("status.manyFunctions", health.functions, health.below)
                    }
                )
            )
            line("<code>${bandLine(health.bands, health.functions)}</code>")
            line(bandTable(health.bands))
        }
    }

    return StatusPresentation(
        text = "$quality%$attention",
        grade = health.grade,
        tooltip = tooltip,
        warning = health.grade == "poor" || health.grade == "fair",
    )
}

/**
 * The line a worst-function entry is listed under in the widget's popup.
 *
 * Plain text, not HTML: a popup row is an action's presentation text, which the
 * platform draws literally.
 */
fun worstLabel(worst: WorstFunction): String {
    val verdict = if (worst.weakestMetric.isNotEmpty()) {
        t(
            "status.verdictWithMetric",
            rounded(worst.quality),
            pillarLabel(worst.weakestPillar),
            measureLabel(worst.weakestMetric),
        )
    } else {
        t("status.verdict", rounded(worst.quality), pillarLabel(worst.weakestPillar))
    }
    return "${functionLabel(worst.name)} — $verdict"
}

private fun bandTable(bands: Bands): String = buildString {
    append("<table cellpadding=0 cellspacing=0>")
    append("<tr><th align=left>${escape(t("label.band"))}</th>")
    append("<th align=right>&nbsp;&nbsp;${escape(t("label.functions"))}</th></tr>")
    for ((grade, count) in listOf(
        "excellent" to bands.excellent,
        "good" to bands.good,
        "fair" to bands.fair,
        "poor" to bands.poor,
    )) {
        append("<tr><td>${escape(gradeLabel(grade))}</td>")
        append("<td align=right>&nbsp;&nbsp;$count</td></tr>")
    }
    append("</table>")
}

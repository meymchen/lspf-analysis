package com.github.meymchen.lspfanalysis.model

/**
 * The little HTML the IDE renders in tooltips and popups.
 *
 * The server writes Markdown, which the health gutter tooltip converts to
 * HTML with the platform's renderer. These are the strings this plugin
 * draws itself, where Swing wants HTML rather than Markdown.
 */
class HtmlBuilder internal constructor() {
    private val parts = mutableListOf<String>()

    /** Appends one line. The caller has already escaped whatever needed it. */
    fun line(html: String) {
        parts += html
    }

    /** Appends a horizontal rule, which does not take a line break of its own. */
    fun rule() {
        parts += RULE
    }

    internal fun render(): String = buildString {
        append("<html><body>")
        parts.forEachIndexed { index, part ->
            if (index > 0 && part != RULE && parts[index - 1] != RULE) {
                append("<br>")
            }
            append(part)
        }
        append("</body></html>")
    }

    private companion object {
        const val RULE = "<hr>"
    }
}

fun html(build: HtmlBuilder.() -> Unit): String = HtmlBuilder().apply(build).render()

/**
 * Escapes text for an HTML body.
 *
 * Load-bearing rather than hygiene: the server's stand-in for a function with
 * no name is literally `<anonymous>`, which Swing would swallow as a tag.
 */
fun escape(text: String): String = text
    .replace("&", "&amp;")
    .replace("<", "&lt;")
    .replace(">", "&gt;")

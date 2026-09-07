package com.github.meymchen.lspfanalysis.model

import java.text.MessageFormat
import java.util.Properties

/**
 * Display text in the reader's language.
 *
 * The IDE's own translator lives behind `DynamicBundle`, which the modules that
 * build these strings deliberately do not import -- that is what lets them be
 * tested without starting a platform fixture. So the translator is installed at
 * startup instead, and until it is, every string renders as the English of
 * `messages/LspfAnalysisBundle.properties`.
 *
 * Only text a person reads goes through here. The vocabulary the server sends
 * in `lspfAnalysis/fileHealth` and `lspfAnalysis/functionHealth` -- pillar
 * names, metric names, grade words -- stays English on the wire and is
 * translated at the moment it is drawn, by [pillarLabel] and its neighbours.
 * The server translates the same vocabulary the same way for the text it
 * renders itself.
 */
object Messages {

    /** Translates a message by key, filling its `{0}`-style placeholders. */
    fun interface Translate {
        fun message(key: String, args: Array<out Any>): String
    }

    private var translate: Translate? = null

    /**
     * The English source strings, read straight out of the bundle file.
     *
     * Not `ResourceBundle`: asked for English on a machine whose default locale
     * is Simplified Chinese, `ResourceBundle` prefers the `_zh_CN` file over the
     * base one, and the fallback that produces is exactly what this property is
     * for ruling out.
     */
    private val english: Properties by lazy {
        Properties().apply {
            val stream = Messages::class.java.getResourceAsStream(BUNDLE)
                ?: error("$BUNDLE is missing from the plugin resources")
            stream.reader(Charsets.UTF_8).use { load(it) }
        }
    }

    /**
     * Installs the IDE's translator, normally `LspfAnalysisBundle::message`.
     *
     * Passing `null` restores the English default, which is what the tests use
     * to read the source strings back.
     */
    fun useTranslator(translator: Translate?) {
        translate = translator
    }

    /** Translates one message. */
    fun t(key: String, vararg args: Any): String =
        translate?.message(key, args) ?: fill(key, args)

    /**
     * Formats the English source string for `key`.
     *
     * A key with no entry renders as the key itself: a `pillar.somethingNew` in
     * the UI is a bug that shows itself, which beats an exception thrown while
     * painting a tooltip.
     */
    private fun fill(key: String, args: Array<out Any>): String {
        val pattern = english.getProperty(key) ?: return key
        return if (args.isEmpty()) pattern else MessageFormat.format(pattern, *args)
    }

    private const val BUNDLE = "/messages/LspfAnalysisBundle.properties"
}

/** Translates one message by key. */
fun t(key: String, vararg args: Any): String = Messages.t(key, *args)

/**
 * The display name of a pillar the server named.
 *
 * A pillar this client has never heard of -- a newer server -- is shown under
 * the name it arrived with rather than dropped.
 */
fun pillarLabel(name: String): String = when (name) {
    "control flow" -> t("pillar.controlFlow")
    "size" -> t("pillar.size")
    "vocabulary load" -> t("pillar.vocabularyLoad")
    "interface" -> t("pillar.interface")
    "class design" -> t("pillar.classDesign")
    else -> name
}

/** The display name of a metric the server named. */
fun measureLabel(name: String): String = when (name) {
    "cognitive complexity" -> t("measure.cognitiveComplexity")
    "cyclomatic complexity" -> t("measure.cyclomaticComplexity")
    "statements" -> t("measure.statements")
    "working memory" -> t("measure.workingMemory")
    "Halstead difficulty" -> t("measure.halsteadDifficulty")
    "parameters" -> t("measure.parameters")
    else -> name
}

/**
 * The name a function is shown under.
 *
 * `<anonymous>` is the server's stand-in for a function with no name, and is
 * the one name here that is ours to say rather than the reader's.
 */
fun functionLabel(name: String): String =
    if (name == "<anonymous>") t("label.anonymous") else name

/** The display name of a band. */
fun gradeLabel(grade: String): String = when (grade) {
    "excellent" -> t("grade.excellent")
    "good" -> t("grade.good")
    "fair" -> t("grade.fair")
    "poor" -> t("grade.poor")
    else -> grade
}

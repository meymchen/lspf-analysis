package com.github.meymchen.lspfanalysis

import com.github.meymchen.lspfanalysis.model.Messages
import com.intellij.DynamicBundle
import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.ProjectActivity
import org.jetbrains.annotations.Nls
import org.jetbrains.annotations.NonNls
import org.jetbrains.annotations.PropertyKey

@NonNls
private const val BUNDLE = "messages.LspfAnalysisBundle"

object LspfAnalysisBundle : DynamicBundle(BUNDLE) {

    @Nls
    fun message(
        key:
        @PropertyKey(resourceBundle = BUNDLE)
        String,
        vararg params: Any,
    ): String = getMessage(key, *params)

    /**
     * The same lookup for a key that is not a literal at the call site.
     *
     * The model layer builds its keys from the vocabulary the server sent, so it
     * cannot satisfy `@PropertyKey`; it reaches the bundle through here.
     */
    @Nls
    fun dynamic(key: String, params: Array<out Any>): String = getMessage(key, *params)
}

/**
 * Hands the model layer the IDE's translator.
 *
 * Until this runs, every string renders as the English of the bundle file,
 * which is exactly what the model's own tests rely on.
 */
internal class LspfAnalysisStartup : ProjectActivity {
    override suspend fun execute(project: Project) {
        Messages.useTranslator { key, params -> LspfAnalysisBundle.dynamic(key, params) }
    }
}

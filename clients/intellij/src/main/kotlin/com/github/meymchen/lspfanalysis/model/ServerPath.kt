package com.github.meymchen.lspfanalysis.model

import java.nio.file.Path
import java.nio.file.Paths

/** Where the binary came from, which decides what to say when it is missing. */
enum class ServerSource { CONFIGURED, DEVELOPMENT, BUNDLED }

data class ResolvedServer(val binary: Path, val source: ServerSource)

/** The server binary's name, which only Windows spells differently. */
fun executableName(osName: String = System.getProperty("os.name").orEmpty()): String =
    if (osName.startsWith("Windows", ignoreCase = true)) "lspf-analysis.exe" else "lspf-analysis"

/** Everything needed to decide which binary to launch. */
data class ServerLocation(
    /**
     * The directory the packaged binary lives in, or `null` when the plugin
     * cannot work out where it was installed.
     */
    val bundledDirectory: Path?,
    /** True when running out of a Gradle sandbox rather than an installation. */
    val development: Boolean,
    /** The repository root, known only in development. */
    val repositoryRoot: Path? = null,
    /** What the language server path setting is set to, if anything. */
    val configuredPath: String? = null,
    val osName: String = System.getProperty("os.name").orEmpty(),
    val homeDirectory: String = System.getProperty("user.home").orEmpty(),
)

/**
 * Decides which binary to launch, most specific source first.
 *
 * A configured path always wins, since someone who set it means it. Failing
 * that, a sandbox IDE runs the debug build from the repository, and an
 * installed plugin runs the one packaged beside it.
 */
fun resolveServerBinary(location: ServerLocation): ResolvedServer {
    val configured = location.configuredPath?.trim()
    if (!configured.isNullOrEmpty()) {
        return ResolvedServer(
            Paths.get(expandHome(configured, location.homeDirectory)).toAbsolutePath().normalize(),
            ServerSource.CONFIGURED,
        )
    }

    val executable = executableName(location.osName)
    val repositoryRoot = location.repositoryRoot
    if (location.development && repositoryRoot != null) {
        return ResolvedServer(
            repositoryRoot.resolve("target").resolve("debug").resolve(executable).normalize(),
            ServerSource.DEVELOPMENT,
        )
    }
    // A bare name when the install directory could not be worked out: it will
    // not exist either, and the reader gets the same explanation of why.
    val bundled = location.bundledDirectory?.resolve(executable)?.normalize()
        ?: Paths.get(executable)
    return ResolvedServer(bundled, ServerSource.BUNDLED)
}

/** Expands a leading `~`, which a hand-written setting is likely to contain. */
fun expandHome(candidate: String, homeDirectory: String): String {
    if (candidate == "~") {
        return homeDirectory
    }
    if (candidate.startsWith("~/") || candidate.startsWith("~\\")) {
        return Paths.get(homeDirectory, candidate.substring(2)).toString()
    }
    return candidate
}

/**
 * Explains a binary that is not there.
 *
 * For a packaged plugin the likely cause is a ZIP built for another platform:
 * only Windows is published today, so anyone else installing it lands here, and
 * saying so is more useful than reporting a bare path.
 */
fun describeMissingServer(resolved: ResolvedServer): String {
    val found = t("server.missing", resolved.binary.toString())
    val advice = when (resolved.source) {
        ServerSource.CONFIGURED -> t("server.missing.configured")
        ServerSource.DEVELOPMENT -> t("server.missing.development")
        ServerSource.BUNDLED -> t("server.missing.bundled")
    }
    return "$found $advice"
}

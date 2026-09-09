import org.jetbrains.intellij.platform.gradle.IntelliJPlatformType
import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.intellij.platform.gradle.tasks.RunIdeTask
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("org.jetbrains.kotlin.jvm")
    id("org.jetbrains.intellij.platform")
    // Formats and checks the Kotlin sources. Everything it applies -- indent,
    // line length, code style -- comes from the repository's .editorconfig.
    id("org.jlleitschuh.gradle.ktlint")
}

/**
 * Which Rust target builds the server for which platform name.
 *
 * The names are the ones the VS Code client uses (`clients/vscode/scripts/targets.mjs`),
 * so the two clients speak of a platform the same way. Only Windows is wired up for
 * now; adding a platform is one line here plus a ZIP to publish.
 */
val serverTargets = mapOf(
    "win32-x64" to "x86_64-pc-windows-msvc",
)

val serverTarget: String = providers.gradleProperty("target").getOrElse("win32-x64")

val serverTriple: String = serverTargets[serverTarget]
    ?: throw GradleException(
        "unsupported target $serverTarget; expected one of ${serverTargets.keys.joinToString(", ")}",
    )

/** The server binary's name, which only Windows spells differently. */
val serverExecutable = if (serverTarget.startsWith("win32-")) "lspf-analysis.exe" else "lspf-analysis"

/** The same, for a build for this machine rather than for the packaged target. */
val hostExecutable =
    if (System.getProperty("os.name").startsWith("Windows")) "lspf-analysis.exe" else "lspf-analysis"

/** The repository root: this build lives two levels below it. */
val repositoryRoot: File = rootDir.parentFile.parentFile
val ideVersion = "2026.1.4"

dependencies {
    testImplementation("junit:junit:4.13.2")

    intellijPlatform {
        // The earliest version this plugin supports, so an API added in a later
        // patch cannot be used by accident.
        intellijIdea(ideVersion)
        testFramework(TestFrameworkType.Platform)
    }
}

intellijPlatformTesting.runIde.register("runPyCharm") {
    type = IntelliJPlatformType.PyCharm
    version = ideVersion
}

intellijPlatform {
    pluginConfiguration {
        ideaVersion {
            // 2026.1.4, the release that open-sourced the LSP client API and so
            // let this plugin use it outside the commercial IDEs. Spelled as its
            // build number rather than "261.4", which would also admit every
            // earlier 2026.1 build -- none of which has com.intellij.modules.lsp.
            sinceBuild = "261.26222"
            // Left open: nothing here is likely to break on a later platform, and
            // a pinned upper bound would need a release per IDE version.
            untilBuild = provider { null }
        }
    }
}

kotlin {
    jvmToolchain(21)
    compilerOptions {
        jvmTarget = JvmTarget.JVM_21
    }
}

/**
 * Whether this invocation is producing something to install.
 *
 * `runIde` shares `prepareSandbox` with `buildPlugin`, and a sandbox IDE runs
 * the debug build from the repository rather than a packaged binary. Without
 * this, every `runIde` would wait on a full release cargo build it then ignores.
 */
val packaging = gradle.startParameter.taskNames.any {
    it.substringAfterLast(':') in setOf(
        "buildPlugin",
        "verifyPlugin",
        "signPlugin",
        "publishPlugin",
        "buildServer",
        "prepareServer",
    )
}

/**
 * Builds the language server for the packaged target.
 *
 * Always given an explicit `--target`, even for the host, so that the artifact
 * path is the same whether or not this is a cross build and a stale host binary
 * can never be mistaken for a cross-built one.
 */
val buildServer = tasks.register<Exec>("buildServer") {
    // Read into locals so the lambdas below close over values rather than over
    // this build script, which the configuration cache cannot serialize.
    val enabled = packaging
    val triple = serverTriple
    val binary = File(repositoryRoot, "target/$serverTriple/release/$serverExecutable")

    group = "build"
    description = "Builds the lspf-analysis language server for $serverTarget."

    onlyIf { enabled }
    workingDir = repositoryRoot
    commandLine("cargo", "build", "--release", "--package", "lspf-analysis", "--target", triple)

    inputs.dir(File(repositoryRoot, "crates")).withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.file(File(repositoryRoot, "Cargo.lock"))
    outputs.file(binary)

    // Both usual causes of a failing cross build are invisible in cargo's own
    // output when the target is simply not set up on this machine.
    doLast {
        if (!binary.exists()) {
            throw GradleException(
                "no server binary at $binary.\n" +
                    "Install the target with: rustup target add $triple\n" +
                    "A cross build also needs a C toolchain and linker for the target, " +
                    "because the tree-sitter grammars are C.",
            )
        }
    }
}

/**
 * Builds the debug server that a sandbox IDE runs.
 *
 * Deliberately without `--target`: a host build lands in `target/debug/`, which
 * is where the plugin looks when it is running out of a sandbox. This is the
 * counterpart of the VS Code client's `build language server` prelaunch task --
 * without it, every fresh clone's first `runIde` comes up reporting a server
 * that was never built.
 */
val buildServerDebug = tasks.register<Exec>("buildServerDebug") {
    group = "build"
    description = "Builds the lspf-analysis language server for a sandbox run."

    workingDir = repositoryRoot
    commandLine("cargo", "build", "--package", "lspf-analysis")

    inputs.dir(File(repositoryRoot, "crates")).withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.file(File(repositoryRoot, "Cargo.lock"))
    outputs.file(File(repositoryRoot, "target/debug/$hostExecutable"))
}

/**
 * Runs the server on a TCP port for a sandbox IDE to attach to.
 *
 * A Gradle task rather than a shell script so that it behaves the same on every
 * OS: `Exec` puts `RUST_LOG` into the child's environment directly, with no
 * shell in between to disagree about how an environment variable is spelled.
 *
 * Start this before `runIde -PdebugPort=...`. The server takes one client and
 * exits when it disconnects, and the IDE tries the socket only once.
 */
val runServerTcp = tasks.register<Exec>("runServerTcp") {
    val address = providers.gradleProperty("tcpAddress").getOrElse("127.0.0.1:9257")
    val level = providers.gradleProperty("rustLog").getOrElse("debug")

    group = "application"
    description = "Runs the language server on $address for a sandbox IDE to attach to."

    dependsOn(buildServerDebug)
    workingDir = repositoryRoot
    commandLine(
        File(repositoryRoot, "target/debug/$hostExecutable").absolutePath,
        "serve",
        "--tcp",
        address,
    )
    environment("RUST_LOG", level)
}

/** Stages the binary where `prepareSandbox` picks it up. */
val prepareServer = tasks.register<Copy>("prepareServer") {
    val enabled = packaging

    group = "build"
    description = "Stages the server binary for packaging."

    onlyIf { enabled }
    dependsOn(buildServer)
    from(File(repositoryRoot, "target/$serverTriple/release/$serverExecutable"))
    into(layout.buildDirectory.dir("server"))
}

tasks {
    prepareSandbox {
        dependsOn(prepareServer)
        from(layout.buildDirectory.dir("server")) {
            into(pluginName.map { "$it/server" })
        }
    }

    buildPlugin {
        archiveBaseName = "lspf-analysis-intellij-$serverTarget"
    }

    withType<RunIdeTask>().configureEach {
        dependsOn(buildServerDebug)

        // What tells the plugin it is running out of a sandbox rather than an
        // installation, so it runs the debug build from the repository instead
        // of looking for a binary it was packaged with.
        systemProperty("lspfAnalysis.development", "true")
        systemProperty("lspfAnalysis.repositoryRoot", repositoryRoot.absolutePath)

        // `-PdebugPort=9257` attaches to a server already running under a
        // debugger instead of spawning one over stdio. Passed as a property
        // rather than read from the environment, because this forked JVM
        // inherits the Gradle daemon's environment and not the caller's.
        providers.gradleProperty("debugPort").orNull?.let {
            environment("LSPF_ANALYSIS_DEBUG_PORT", it)
        }
    }

    test {
        useJUnit()
    }
}

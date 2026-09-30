import groovy.json.JsonSlurper
import org.jetbrains.intellij.platform.gradle.IntelliJPlatformType
import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.intellij.platform.gradle.tasks.RunIdeTask
import org.jetbrains.intellij.platform.gradle.tasks.VerifyPluginTask
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("org.jetbrains.kotlin.jvm")
    id("org.jetbrains.intellij.platform")
    // Formats and checks the Kotlin sources. Everything it applies -- indent,
    // line length, code style -- comes from the repository's .editorconfig.
    id("org.jlleitschuh.gradle.ktlint")
}

@Suppress("UNCHECKED_CAST")
val releaseConfig = JsonSlurper().parseText(
    providers.fileContents(layout.projectDirectory.file("release.json")).asText.get(),
)
    as Map<String, Any>

@Suppress("UNCHECKED_CAST")
val platforms = releaseConfig["platforms"] as List<Map<String, String>>
val serverTargets = platforms.associate { it.getValue("target") to it.getValue("rust") }
val hostOs = when {
    System.getProperty("os.name").startsWith("Windows") -> "win32"
    System.getProperty("os.name").startsWith("Mac") -> "darwin"
    else -> "linux"
}
val hostArch = when (System.getProperty("os.arch")) {
    "aarch64", "arm64" -> "arm64"
    "amd64", "x86_64" -> "x64"
    else -> throw GradleException("Unsupported host architecture")
}
val serverTarget = providers.gradleProperty("target").getOrElse("$hostOs-$hostArch")

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
val nativeDirectory = layout.buildDirectory.dir("native")
val releaseArchive = providers.gradleProperty("releaseArchive")
val changelog = providers.fileContents(layout.projectDirectory.file("CHANGELOG.md")).asText.get().replace("\r\n", "\n")
val releaseNotes = changelog.substringAfter("## $version\n", "").substringBefore("\n## ").trim()

val releaseSmokeSources = sourceSets.create("releaseSmoke")
configurations[releaseSmokeSources.implementationConfigurationName].extendsFrom(configurations.testImplementation.get())
configurations[releaseSmokeSources.runtimeOnlyConfigurationName].extendsFrom(configurations.testRuntimeOnly.get())
releaseSmokeSources.compileClasspath += sourceSets.main.get().output + sourceSets.test.get().compileClasspath

// This test installs the downloaded ZIP. Its runtime excludes main's build output.
intellijPlatformTesting.testIde.register("releaseSmoke") {
    type = IntelliJPlatformType.valueOf(providers.gradleProperty("smokeIde").getOrElse("IntellijIdea"))
    version = ideVersion
    testFramework(TestFrameworkType.Platform)
    val installed = layout.buildDirectory.dir("release-smoke-plugin/lspf-analysis")
    prepareSandboxTask {
        pluginJar = installed.map { it.file("lib/lspf-analysis-${project.version}.jar") }
        runtimeClasspath.setFrom(
            fileTree(installed.map { it.dir("lib") }) {
                include("*.jar")
                exclude("lspf-analysis-${project.version}.jar")
            },
        )
        from(installed.map { it.dir("server") }) {
            into(pluginName.map { "$it/server" })
            filePermissions { unix("755") }
        }
    }
    task {
        testClassesDirs = releaseSmokeSources.output.classesDirs
        classpath += releaseSmokeSources.runtimeClasspath
        filter { includeTestsMatching("*ReleaseInstallationTest") }
        systemProperty("lspfAnalysis.development", "false")
        useJUnit()
    }
}

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
        changeNotes = "<pre>" + releaseNotes.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;") + "</pre>"
        ideaVersion {
            // 2026.1.4, the release that open-sourced the LSP client API and so
            // let this plugin use it outside the commercial IDEs. Spelled as its
            // build number rather than "261.4", which would also admit every
            // earlier 2026.1 build -- none of which has com.intellij.modules.lsp.
            sinceBuild = releaseConfig.getValue("sinceBuild") as String
            untilBuild = releaseConfig.getValue("untilBuild") as String
        }
    }
    nativeVariants {
        enabled = true
        windows {
            x86_64.from(nativeDirectory.map { it.dir("windows-x86_64") })
            arm64.from(nativeDirectory.map { it.dir("windows-arm64") })
        }
        mac {
            x86_64.from(nativeDirectory.map { it.dir("mac-x86_64") })
            arm64.from(nativeDirectory.map { it.dir("mac-arm64") })
        }
        linux {
            x86_64.from(nativeDirectory.map { it.dir("linux-x86_64") })
            arm64.from(nativeDirectory.map { it.dir("linux-arm64") })
        }
    }
    signing {
        privateKeyFile = providers.environmentVariable("INTELLIJ_PRIVATE_KEY_FILE").map { file(it) }
        certificateChainFile = providers.environmentVariable("INTELLIJ_CERTIFICATE_CHAIN_FILE").map { file(it) }
        password = providers.environmentVariable("INTELLIJ_PRIVATE_KEY_PASSWORD")
    }
    pluginVerification {
        // These synthetic selectors are checked by release.py and native smoke
        // tests. Verifier 1.409+ can check JVM compatibility independently.
        freeArgs = listOf("-ignore-os-arch")
        failureLevel = listOf(
            VerifyPluginTask.FailureLevel.COMPATIBILITY_PROBLEMS,
            VerifyPluginTask.FailureLevel.INVALID_PLUGIN,
            VerifyPluginTask.FailureLevel.MISSING_DEPENDENCIES,
        )
        ides {
            val selected = providers.gradleProperty("verificationIde").orNull
            if (selected != null) {
                val (type, targetVersion) = selected.split(":", limit = 2)
                create(IntelliJPlatformType.valueOf(type), targetVersion)
            } else {
                current()
            }
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
 * Builds the language server for the packaged target.
 *
 * Always given an explicit `--target`, even for the host, so that the artifact
 * path is the same whether or not this is a cross build and a stale host binary
 * can never be mistaken for a cross-built one.
 */
val buildServer = tasks.register<Exec>("buildServer") {
    // Read into locals so the lambdas below close over values rather than over
    // this build script, which the configuration cache cannot serialize.
    val triple = serverTriple
    val binary = File(repositoryRoot, "target/$serverTriple/release/$serverExecutable")

    group = "build"
    description = "Builds the lspf-analysis language server for $serverTarget."

    workingDir = repositoryRoot
    commandLine("cargo", "build", "--locked", "--release", "--package", "lspf-analysis", "--target", triple)

    inputs.dir(File(repositoryRoot, "crates")).withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.file(File(repositoryRoot, "Cargo.toml"))
    inputs.file(File(repositoryRoot, "Cargo.lock"))
    inputs.file(File(repositoryRoot, "rust-toolchain.toml"))
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
    commandLine("cargo", "build", "--locked", "--package", "lspf-analysis")

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

/** Stages the binary in the selected native variant's input directory. */
val prepareServer = tasks.register<Copy>("prepareServer") {
    group = "build"
    description = "Stages the server binary for packaging."

    dependsOn(buildServer)
    from(File(repositoryRoot, "target/$serverTriple/release/$serverExecutable"))
    into(
        nativeDirectory.map {
            it.dir("${platforms.single { it.getValue("target") == serverTarget }.getValue("variant")}/server")
        },
    )
    filePermissions { unix("755") }
}

// The native-variants DSL accepts empty inputs. A distributable must not.
platforms.forEach { platform ->
    val variant = platform.getValue("variant")
    val executable = if (variant.startsWith("windows-")) "lspf-analysis.exe" else "lspf-analysis"
    val binary = nativeDirectory.map { it.file("$variant/server/$executable") }
    tasks.named<Zip>("buildPluginVariants_${variant.replace('-', '_')}") {
        mustRunAfter(prepareServer)
        inputs.file(binary)
        // Gradle 9 archives default to 0644 even when the source is executable.
        // Set the ZIP entry explicitly, including when packaging on Windows.
        filesMatching("**/server/$executable") {
            permissions { unix("755") }
        }
        doFirst {
            require(binary.get().asFile.length() > 0) { "Missing server for $variant; stage all native servers first" }
        }
    }
}

tasks {
    buildPlugin {
        archiveBaseName = "lspf-analysis-intellij"
    }

    // These tasks consume the already-tested ZIP, without building another one.
    if (releaseArchive.isPresent) {
        signPlugin {
            setDependsOn(emptyList<Any>())
            archiveFile = layout.projectDirectory.file(releaseArchive.get())
            signedArchiveFile =
                layout.projectDirectory.file(
                    providers.gradleProperty("signedArchive").getOrElse("build/signed/plugin.zip"),
                )
            doFirst {
                require(privateKeyFile.isPresent && certificateChainFile.isPresent) {
                    "Signing credentials are required"
                }
            }
        }
        verifyPluginSignature {
            setDependsOn(emptyList<Any>())
            inputArchiveFile = layout.projectDirectory.file(releaseArchive.get())
        }
        verifyPlugin {
            setDependsOn(emptyList<Any>())
            archiveFile = layout.projectDirectory.file(releaseArchive.get())
        }
    }
    publishPlugin {
        setDependsOn(emptyList<Any>())
        doFirst {
            throw GradleException("Publish the verified, signed release bundle with scripts/publish.py")
        }
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

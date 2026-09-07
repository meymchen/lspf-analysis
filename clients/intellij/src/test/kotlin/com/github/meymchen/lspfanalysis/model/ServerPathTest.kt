package com.github.meymchen.lspfanalysis.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Paths

class ServerPathTest {

    private val bundledDirectory = Paths.get("/plugins/lspf-analysis/server")
    private val repositoryRoot = Paths.get("/work/lspf-analysis")

    private fun location(
        development: Boolean = false,
        configuredPath: String? = null,
        osName: String = "Linux",
    ) = ServerLocation(
        bundledDirectory = bundledDirectory,
        development = development,
        repositoryRoot = repositoryRoot,
        configuredPath = configuredPath,
        osName = osName,
        homeDirectory = "/home/reader",
    )

    @Test
    fun `only Windows spells the binary differently`() {
        assertEquals("lspf-analysis.exe", executableName("Windows 11"))
        assertEquals("lspf-analysis", executableName("Linux"))
        assertEquals("lspf-analysis", executableName("Mac OS X"))
    }

    @Test
    fun `an installed plugin runs the binary packaged beside it`() {
        val resolved = resolveServerBinary(location())
        assertEquals(ServerSource.BUNDLED, resolved.source)
        assertEquals(Paths.get("/plugins/lspf-analysis/server/lspf-analysis"), resolved.binary)
    }

    @Test
    fun `an install directory we could not work out still explains itself`() {
        val resolved = resolveServerBinary(location().copy(bundledDirectory = null))
        assertEquals(ServerSource.BUNDLED, resolved.source)
        assertEquals(Paths.get("lspf-analysis"), resolved.binary)
    }

    @Test
    fun `a sandbox run uses the debug build from the repository`() {
        val resolved = resolveServerBinary(location(development = true))
        assertEquals(ServerSource.DEVELOPMENT, resolved.source)
        assertEquals(Paths.get("/work/lspf-analysis/target/debug/lspf-analysis"), resolved.binary)
    }

    @Test
    fun `a configured path wins over both, since someone who set it means it`() {
        val resolved = resolveServerBinary(
            location(development = true, configuredPath = "/opt/lspf/lspf-analysis")
        )
        assertEquals(ServerSource.CONFIGURED, resolved.source)
        // Absolute, because a relative setting would resolve against whatever
        // directory the IDE happens to have been started from. What that
        // absolute path looks like is the platform's business: on Windows the
        // configured path picks up the current drive.
        assertTrue(resolved.binary.toString(), resolved.binary.isAbsolute)
        assertTrue(resolved.binary.toString(), resolved.binary.endsWith(Paths.get("opt/lspf/lspf-analysis")))
    }

    @Test
    fun `a blank setting is no setting`() {
        assertEquals(ServerSource.BUNDLED, resolveServerBinary(location(configuredPath = "  ")).source)
        assertEquals(ServerSource.BUNDLED, resolveServerBinary(location(configuredPath = "")).source)
    }

    @Test
    fun `a leading tilde is expanded, since a hand-written path is likely to have one`() {
        assertEquals("/home/reader", expandHome("~", "/home/reader"))
        assertEquals(
            Paths.get("/home/reader", "bin/lspf-analysis").toString(),
            expandHome("~/bin/lspf-analysis", "/home/reader"),
        )
        // Only leading, and only as a path segment of its own.
        assertEquals("/opt/~/x", expandHome("/opt/~/x", "/home/reader"))
        assertEquals("~x/y", expandHome("~x/y", "/home/reader"))
    }

    @Test
    fun `a missing binary is explained by where it was looked for`() {
        val bundled = describeMissingServer(resolveServerBinary(location()))
        assertTrue(bundled, bundled.contains("built for a different platform"))

        val development = describeMissingServer(resolveServerBinary(location(development = true)))
        assertTrue(development, development.contains("cargo build"))

        val configured =
            describeMissingServer(resolveServerBinary(location(configuredPath = "/opt/x")))
        assertTrue(configured, configured.contains("path setting"))

        // Whichever it is, it names the path it tried.
        assertTrue(bundled, bundled.contains("server"))
    }
}

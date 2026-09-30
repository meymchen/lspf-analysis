package com.github.meymchen.lspfanalysis

import com.github.meymchen.lspfanalysis.lsp.LspfAnalysisClientDescriptor
import com.intellij.ide.plugins.PluginManagerCore
import com.intellij.openapi.extensions.PluginId
import com.intellij.testFramework.LightPlatformTestCase
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** Loads the installed distribution and starts the executable its descriptor resolves. */
class ReleaseInstallationTest : LightPlatformTestCase() {
    @org.junit.Test
    fun testInstalledPluginStartsItsBundledServer() {
        val plugin = requireNotNull(PluginManagerCore.getPlugin(PluginId.getId("com.github.meymchen.lspfanalysis")))
        assertTrue("Installed plugin is disabled", plugin.isEnabled)
        val source =
            requireNotNull(LspfAnalysisClientDescriptor::class.java.getResource("LspfAnalysisClientDescriptor.class"))
        assertEquals("Smoke must load the installed JAR: $source", "jar", source.protocol)
        val jar = java.nio.file.Path.of(
            java.net.URI(source.toExternalForm().substringAfter("jar:").substringBefore("!/")),
        )
        assertTrue(
            "Class must come from the installed plugin",
            jar.toRealPath().startsWith(plugin.pluginPath.toRealPath()),
        )
        val command = LspfAnalysisClientDescriptor(project).createCommandLine()
        val binary = java.nio.file.Path.of(command.exePath).toRealPath()
        assertTrue("Descriptor must resolve the installed server", binary.startsWith(plugin.pluginPath.toRealPath()))
        assertTrue(Files.isExecutable(binary))

        val process = command.createProcess()
        val executor = Executors.newSingleThreadExecutor()
        val logs = StringBuilder()
        val logReader = Thread {
            process.errorStream.bufferedReader().useLines { lines -> lines.forEach { logs.appendLine(it) } }
        }.apply {
            isDaemon = true
            start()
        }
        try {
            val result = executor.submit<Boolean> {
                val input = process.inputStream.buffered()
                fun send(body: String) {
                    val bytes = body.toByteArray(StandardCharsets.UTF_8)
                    process.outputStream.write(
                        "Content-Length: ${bytes.size}\r\n\r\n".toByteArray(StandardCharsets.US_ASCII),
                    )
                    process.outputStream.write(bytes)
                    process.outputStream.flush()
                }
                fun receive(): com.google.gson.JsonObject {
                    val header = StringBuilder()
                    while (!header.endsWith("\r\n\r\n")) {
                        val next = input.read()
                        check(next >= 0) { "Server closed stdout" }
                        header.append(next.toChar())
                        check(header.length < 8192)
                    }
                    val length = Regex("(?i)Content-Length: (\\d+)").find(header)!!.groupValues[1].toInt()
                    check(length in 1..8_388_608)
                    return com.google.gson.JsonParser.parseString(
                        String(input.readNBytes(length), StandardCharsets.UTF_8),
                    ).asJsonObject
                }
                fun response(id: Int): com.google.gson.JsonObject {
                    while (true) {
                        val message = receive()
                        if (message.get("id")?.asInt == id) {
                            check(!message.has("error")) { message.toString() }
                            return message
                        }
                    }
                }
                send(
                    """{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}""",
                )
                check(response(1).getAsJsonObject("result").has("capabilities"))
                send("""{"jsonrpc":"2.0","method":"initialized","params":{}}""")
                send(
                    """{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///release-smoke.rs","languageId":"rust","version":1,"text":"fn wide(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 { a + b + c + d + e }"}}}""",
                )
                while (true) {
                    val notification = receive()
                    if (notification.get("method")?.asString == "lspfAnalysis/fileHealth") {
                        check(notification.getAsJsonObject("params").get("functions").asInt == 1)
                        break
                    }
                }
                send(
                    """{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///release-smoke.rs"},"position":{"line":0,"character":4}}}""",
                )
                check(response(2).get("result").toString().contains("wide"))
                send("""{"jsonrpc":"2.0","id":3,"method":"shutdown"}""")
                check(response(3).get("result").isJsonNull)
                send("""{"jsonrpc":"2.0","method":"exit"}""")
                true
            }
            assertTrue(result.get(30, TimeUnit.SECONDS))
            assertTrue("Server did not exit", process.waitFor(5, TimeUnit.SECONDS))
            assertEquals(logs.toString(), 0, process.exitValue())
        } finally {
            process.destroyForcibly()
            executor.shutdownNow()
            logReader.join(1000)
        }
    }
}

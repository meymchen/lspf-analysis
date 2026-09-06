package com.github.meymchen.lspfanalysis.model

/**
 * Set to a port by the TCP run configuration.
 *
 * When it is present the plugin talks to a server already started in a separate
 * debug session, rather than spawning one of its own. Both sides can be stepped
 * through independently. Nothing sets it in a released plugin.
 */
const val DEBUG_PORT_VARIABLE: String = "LSPF_ANALYSIS_DEBUG_PORT"

/**
 * Reads the port to attach to, or `null` outside a debug session.
 *
 * A value that is not a port is an error rather than a silent fall-back to
 * spawning a process: someone who set the variable meant to attach, and quietly
 * starting a second server would leave them stepping through the wrong one.
 */
fun debugServerPort(environment: Map<String, String>): Int? {
    val raw = environment[DEBUG_PORT_VARIABLE]?.trim()
    if (raw.isNullOrEmpty()) {
        return null
    }
    val port = raw.toIntOrNull()
    require(port != null && port in 1..65535) {
        "$DEBUG_PORT_VARIABLE is $raw, which is not a port number"
    }
    return port
}

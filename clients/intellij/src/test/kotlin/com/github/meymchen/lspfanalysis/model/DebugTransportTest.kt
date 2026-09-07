package com.github.meymchen.lspfanalysis.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class DebugTransportTest {

    @Test
    fun `no variable means spawn a server of our own`() {
        assertNull(debugServerPort(emptyMap()))
        assertNull(debugServerPort(mapOf(DEBUG_PORT_VARIABLE to "")))
        assertNull(debugServerPort(mapOf(DEBUG_PORT_VARIABLE to "   ")))
    }

    @Test
    fun `a port is read, whitespace and all`() {
        assertEquals(9257, debugServerPort(mapOf(DEBUG_PORT_VARIABLE to "9257")))
        assertEquals(9257, debugServerPort(mapOf(DEBUG_PORT_VARIABLE to " 9257 ")))
    }

    @Test
    fun `something that is not a port is an error, not a silent second server`() {
        for (value in listOf("nine", "0", "65536", "-1", "9257.5")) {
            assertThrows(IllegalArgumentException::class.java) {
                debugServerPort(mapOf(DEBUG_PORT_VARIABLE to value))
            }
        }
    }
}

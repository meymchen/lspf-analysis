package com.github.meymchen.lspfanalysis.lsp

import com.github.meymchen.lspfanalysis.model.FILE_HEALTH_METHOD
import com.github.meymchen.lspfanalysis.model.FUNCTION_HEALTH_METHOD
import org.eclipse.lsp4j.jsonrpc.services.ServiceEndpoints
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

/**
 * The two custom methods are wired by lsp4j from annotations, by reflection, at
 * runtime -- nothing about them is checked by the compiler. A method name that
 * drifted from the server's would fail silently: the request would time out and
 * the view would just be empty. So the wiring is asserted here.
 */
class ProtocolTest {

    @Test
    fun `the breakdown is requested under the method name the server answers`() {
        val methods = ServiceEndpoints.getSupportedMethods(LspfAnalysisLsp4jServer::class.java)
        val method = methods[FUNCTION_HEALTH_METHOD]
        assertNotNull("$FUNCTION_HEALTH_METHOD is not registered: ${methods.keys}", method)
        assertEquals(FunctionHealthParams::class.java, method!!.parameterTypes.single())
    }

    @Test
    fun `the file summary is listened for under the method name the server pushes`() {
        val methods = ServiceEndpoints.getSupportedMethods(LspfAnalysisLsp4jClient::class.java)
        val method = methods[FILE_HEALTH_METHOD]
        assertNotNull("$FILE_HEALTH_METHOD is not registered: ${methods.keys}", method)
        assertEquals(true, method!!.isNotification)
    }

    @Test
    fun `the method names are the ones the protocol documents`() {
        assertEquals("lspfAnalysis/functionHealth", FUNCTION_HEALTH_METHOD)
        assertEquals("lspfAnalysis/fileHealth", FILE_HEALTH_METHOD)
    }
}

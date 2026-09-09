package com.github.meymchen.lspfanalysis.lsp

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Test
import java.util.concurrent.atomic.AtomicInteger

class HealthHoverCacheTest {
    @Test
    fun slowHoverReturnsImmediatelyThenRefreshesAndDeduplicates() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val cache = HealthHoverCache(scope)
            val release = CompletableDeferred<Unit>()
            val ready = CompletableDeferred<Unit>()
            val calls = AtomicInteger()
            val position = HealthPosition(90, 38)
            val load: suspend (HealthPosition) -> String? = {
                calls.incrementAndGet()
                release.await()
                delay(150)
                "health"
            }
            repeat(10) {
                assertNull(cache.get("file", 1, position, load, { ready.complete(Unit) }, { throw it }))
            }
            assertFalse(ready.isCompleted)
            release.complete(Unit)
            withTimeout(5000) { ready.await() }
            assertEquals("health", cache.get("file", 1, position, load, {}, { throw it }))
            assertEquals(1, calls.get())
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun editsAndInvalidationDiscardOldResultsAndCacheEmptyHovers() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        try {
            val cache = HealthHoverCache(scope)
            val position = HealthPosition(1, 1)
            val started = CompletableDeferred<Unit>()
            val old = CompletableDeferred<String?>()
            cache.get("file", 1, position, {
                started.complete(Unit)
                old.await()
            }, { fail("stale refresh") }, { throw it })
            withTimeout(5000) { started.await() }
            val ready = CompletableDeferred<Unit>()
            cache.get("file", 2, position, { "new" }, { ready.complete(Unit) }, { throw it })
            withTimeout(5000) { ready.await() }
            old.complete("old")
            assertEquals(
                "new",
                cache.get("file", 2, position, {
                    fail("cached")
                    null
                }, {}, { throw it }),
            )
            cache.forget("file")
            val emptyReady = CompletableDeferred<Unit>()
            cache.get("file", 2, position, { null }, { emptyReady.complete(Unit) }, { throw it })
            withTimeout(5000) { emptyReady.await() }
            assertNull(
                cache.get("file", 2, position, {
                    fail("empty cached")
                    null
                }, {}, { throw it }),
            )
            cache.clear()
            val reset = CompletableDeferred<Unit>()
            cache.get("file", 2, position, { "restart" }, { reset.complete(Unit) }, { throw it })
            withTimeout(5000) { reset.await() }
            assertEquals("restart", cache.get("file", 2, position, { null }, {}, { throw it }))
        } finally {
            scope.cancel()
        }
    }
}

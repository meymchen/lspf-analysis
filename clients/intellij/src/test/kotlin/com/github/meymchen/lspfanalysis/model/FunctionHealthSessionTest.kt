package com.github.meymchen.lspfanalysis.model

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.cancel
import kotlinx.coroutines.isActive
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FunctionHealthSessionTest {
    @Test
    fun selectingAnotherEditorForTheSameFileKeepsRowsAndThePendingRefresh() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            val ready = CompletableDeferred<Unit>()
            var calls = 0
            val session = FunctionHealthSession(scope, { uri ->
                calls++
                FunctionHealth(uri, emptyList())
            }, {}, pause = { ready.await() })
            session.select("a")
            session.republished("a")
            session.select("a")
            assertEquals("a", session.state.health?.uri)
            assertEquals(1, calls)
            ready.complete(Unit)
            assertEquals(2, calls)
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun aRequestThatNeverAnswersBecomesUnavailable() = runBlocking {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            val unavailable = CompletableDeferred<Unit>()
            var cancelled = false
            val session = FunctionHealthSession(scope, {
                try {
                    awaitCancellation()
                } finally {
                    cancelled = true
                }
            }, { if (it.unavailable) unavailable.complete(Unit) })
            session.select("a")
            withTimeout(15000) { unavailable.await() }
            assertTrue(cancelled)
            assertNull(session.state.health)
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun disposalCancelsWorkAndPreventsFurtherRenderingOrRequests() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val late = CompletableDeferred<FunctionHealth?>()
        try {
            var cancelled = false
            val shown = mutableListOf<FunctionHealthState>()
            val session = FunctionHealthSession(scope, {
                try {
                    awaitCancellation()
                } catch (_: CancellationException) {
                    cancelled = true
                    withContext(NonCancellable) { late.await() }
                }
            }, shown::add)
            session.select("a")
            val before = shown.toList()
            session.close()
            assertTrue(cancelled)
            late.complete(FunctionHealth("a", emptyList()))
            session.select("b")
            session.republished("b")
            session.serverInitialized()
            session.serverStopped()
            assertEquals(before, shown)
            assertTrue("the panel does not cancel its parent's scope", scope.isActive)
        } finally {
            late.complete(null)
            scope.cancel()
        }
    }

    @Test
    fun serverStopClearsRowsAndInitializationFetchesImmediately() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            var calls = 0
            val session = FunctionHealthSession(scope, { uri ->
                calls++
                FunctionHealth(uri, emptyList())
            }, {}, pause = { error("initialization must not debounce") })
            session.select("a")
            session.serverStopped()
            assertNull(session.state.health)
            assertTrue(session.state.unavailable)
            session.republished("a")
            session.select("b")
            assertEquals(1, calls)
            session.serverInitialized()
            assertEquals(2, calls)
            assertEquals("b", session.state.health?.uri)
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun failedRefreshClearsRowsAndRecoversOnTheNextPublication() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            var failing = false
            val session = FunctionHealthSession(scope, { uri ->
                check(!failing) { "request failed" }
                FunctionHealth(uri, emptyList())
            }, {}, pause = {})
            session.select("a")
            failing = true
            session.republished("a")
            assertNull(session.state.health)
            assertTrue(session.state.unavailable)
            failing = false
            session.republished("a")
            assertEquals("a", session.state.health?.uri)
            assertFalse(session.state.unavailable)
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun republishesKeepRowsAndFetchOnlyAfterTheLastQuietPeriod() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            val waits = mutableListOf<CompletableDeferred<Unit>>()
            val refreshed = CompletableDeferred<FunctionHealth?>()
            val original = FunctionHealth("a", emptyList())
            var calls = 0
            val session = FunctionHealthSession(scope, {
                if (++calls == 1) original else refreshed.await()
            }, {}, pause = { milliseconds ->
                assertEquals(1000L, milliseconds)
                CompletableDeferred<Unit>().also { waits.add(it) }.await()
            })
            session.select("a")
            session.republished("other")
            assertTrue(waits.isEmpty())
            session.republished("a")
            session.republished("a")
            assertEquals(original, session.state.health)
            waits[0].complete(Unit)
            assertEquals(1, calls)
            waits[1].complete(Unit)
            assertEquals(2, calls)
            assertEquals(original, session.state.health)
            refreshed.complete(null)
            assertNull(session.state.health)
        } finally {
            scope.cancel()
        }
    }

    @Test
    fun switchingCancelsTheOldRequestAndRejectsItsLateAnswer() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val late = CompletableDeferred<FunctionHealth?>()
        try {
            var cancelled = false
            val session = FunctionHealthSession(scope, { uri ->
                if (uri == "a") {
                    try {
                        awaitCancellation()
                    } catch (_: CancellationException) {
                        cancelled = true
                        withContext(NonCancellable) { late.await() }
                    }
                } else {
                    FunctionHealth(uri, emptyList())
                }
            }, {})
            session.select("a")
            session.select("b")
            assertTrue("switching cancels A", cancelled)
            late.complete(FunctionHealth("a", emptyList()))
            assertEquals("b", session.state.health?.uri)
        } finally {
            late.complete(null)
            scope.cancel()
        }
    }

    @Test
    fun switchingFilesClearsPreviousRowsBeforeTheNewAnswerArrives() {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        try {
            val answer = CompletableDeferred<FunctionHealth?>()
            val shown = mutableListOf<FunctionHealthState>()
            val session = FunctionHealthSession(scope, { uri ->
                if (uri == "a") FunctionHealth("a", emptyList()) else answer.await()
            }, shown::add)

            session.select("a")
            assertEquals("a", shown.last().health?.uri)
            session.select("b")
            assertEquals("b", shown.last().uri)
            assertNull(shown.last().health)
            answer.complete(null)
            assertNull(shown.last().health)
        } finally {
            scope.cancel()
        }
    }
}

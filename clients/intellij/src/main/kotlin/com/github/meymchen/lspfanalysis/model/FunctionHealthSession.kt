package com.github.meymchen.lspfanalysis.model

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout

/** The Function Health information the view can currently display. */
internal data class FunctionHealthState(
    val uri: String? = null,
    val health: FunctionHealth? = null,
    val unavailable: Boolean = false,
)

/** Owns the active document's Function Health while requests complete. */
internal class FunctionHealthSession(
    private val scope: CoroutineScope,
    private val load: suspend (String) -> FunctionHealth?,
    private val changed: (FunctionHealthState) -> Unit,
    private val pause: suspend (Long) -> Unit = { delay(it) },
) {
    private var request: Job? = null
    private var generation = 0L
    private var running = true
    private var closed = false
    var state = FunctionHealthState()
        private set

    fun select(uri: String?) {
        if (closed || uri == state.uri) return
        reset(uri)
    }

    private fun reset(uri: String?, unavailable: Boolean = uri != null && !running) {
        state = FunctionHealthState(uri, unavailable = unavailable)
        changed(state)
        refresh(uri, false)
    }

    fun republished(uri: String) {
        if (closed) return
        if (uri == state.uri) refresh(uri, true)
    }

    fun serverStopped() {
        if (closed) return
        running = false
        reset(state.uri, unavailable = true)
    }

    fun serverInitialized(uri: String? = state.uri) {
        if (closed) return
        running = true
        reset(uri)
    }

    fun close() {
        closed = true
        generation++
        request?.cancel()
        request = null
    }

    private fun refresh(uri: String?, debounced: Boolean) {
        val fetch = ++generation
        request?.cancel()
        if (uri != null && running) {
            request = scope.launch {
                if (debounced) pause(1000)
                val next = try {
                    // The asynchronous platform request has no timeout of its own.
                    // Match the platform's normal ten-second request allowance.
                    val health = withTimeout(10_000) { load(uri) }
                    FunctionHealthState(uri, health?.takeIf { it.uri == uri })
                } catch (_: Exception) {
                    // An obsolete request is cancellation, not a visible failure.
                    currentCoroutineContext().ensureActive()
                    FunctionHealthState(uri, unavailable = true)
                }
                if (fetch == generation) {
                    state = next
                    changed(state)
                }
            }
        }
    }
}

package com.github.meymchen.lspfanalysis.lsp

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

internal data class HealthPosition(val line: Int, val character: Int)

/** Nonblocking reads; one worker drains each document's missing positions. */
internal class HealthHoverCache(private val scope: CoroutineScope) {
    private class Entry(val stamp: Long) {
        val values = mutableMapOf<HealthPosition, String?>()
        val pending = linkedSetOf<HealthPosition>()
        var running = false
        var job: Job? = null
    }

    private val entries = mutableMapOf<String, Entry>()

    @Synchronized
    fun get(
        uri: String,
        stamp: Long,
        position: HealthPosition,
        load: suspend (HealthPosition) -> String?,
        ready: () -> Unit,
        failed: (Exception) -> Unit,
    ): String? {
        var entry = entries[uri]
        if (entry == null || entry.stamp != stamp) {
            entry?.job?.cancel()
            entry = Entry(stamp)
            entries[uri] = entry
        }
        if (entry.values.containsKey(position)) return entry.values[position]
        entry.pending.add(position)
        if (!entry.running) {
            entry.running = true
            val current = entry
            current.job = scope.launch {
                try {
                    while (true) {
                        val next = synchronized(this@HealthHoverCache) {
                            if (entries[uri] !== current) return@launch
                            current.pending.firstOrNull().also {
                                if (it == null) current.running = false
                            }
                        } ?: break
                        val value = load(next)
                        synchronized(this@HealthHoverCache) {
                            if (entries[uri] !== current) return@launch
                            current.values[next] = value
                            current.pending.remove(next)
                        }
                    }
                    ready()
                } catch (error: Exception) {
                    synchronized(this@HealthHoverCache) {
                        if (entries[uri] === current) entries.remove(uri)
                    }
                    if (error is CancellationException) throw error
                    failed(error)
                }
            }
        }
        return null
    }

    @Synchronized
    fun forget(uri: String) {
        entries.remove(uri)?.job?.cancel()
    }

    @Synchronized
    fun clear() {
        val old = entries.values.toList()
        entries.clear()
        old.forEach { it.job?.cancel() }
    }
}

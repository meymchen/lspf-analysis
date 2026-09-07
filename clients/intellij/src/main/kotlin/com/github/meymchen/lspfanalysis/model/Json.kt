package com.github.meymchen.lspfanalysis.model

import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject

/**
 * Reading the server's answers defensively.
 *
 * The server is versioned separately from this plugin -- a user can point the
 * language server path setting at any build -- so every accessor here answers
 * `null` for a value that is missing or of the wrong type, and the parsers
 * above turn that into "nothing to draw" rather than a tree of `NaN%`.
 */
internal fun JsonElement?.objectOrNull(): JsonObject? = this as? JsonObject

internal fun JsonObject.obj(key: String): JsonObject? = get(key) as? JsonObject

internal fun JsonObject.array(key: String): JsonArray? = get(key) as? JsonArray

internal fun JsonObject.string(key: String): String? {
    val value = get(key)
    return if (value != null && value.isJsonPrimitive && value.asJsonPrimitive.isString) {
        value.asString
    } else {
        null
    }
}

internal fun JsonObject.number(key: String): Double? {
    val value = get(key)
    if (value == null || !value.isJsonPrimitive || !value.asJsonPrimitive.isNumber) {
        return null
    }
    return value.asDouble.takeIf { it.isFinite() }
}

internal fun JsonObject.int(key: String): Int? = number(key)?.toInt()

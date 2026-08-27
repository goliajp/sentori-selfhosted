package com.sentori

import android.content.Context
import android.os.Build

/**
 * What the device is, as `payload.device` on every event.
 *
 * Everything here is read once and cached: this runs on the calling
 * thread inside every verb, and a verb that asks the window manager
 * for a fresh value each time is not the O(1) the contract promises.
 */
object SentoriDevice {

    private val lock = Any()
    private var screen: Map<String, Any?>? = null

    /**
     * Optional. Without a [Context] the snapshot omits the screen and
     * carries everything else — an event with a thinner device beats
     * no event, and a host that starts the SDK before it has a context
     * is making a reasonable choice.
     */
    @JvmStatic
    fun bind(context: Context?) {
        val ctx = context ?: return
        synchronized(lock) {
            val m = ctx.resources.displayMetrics
            screen =
                mapOf(
                    "width" to (m.widthPixels / m.density),
                    "height" to (m.heightPixels / m.density),
                    "scale" to m.density,
                )
        }
    }

    @JvmStatic
    fun snapshot(): Map<String, Any?> {
        val out =
            mutableMapOf<String, Any?>(
                "os" to "android",
                "osVersion" to Build.VERSION.RELEASE,
                // The hardware identifier, not a marketing name.
                // Mapping it would need a table that goes stale with
                // every launch; the dashboard does that lookup
                // server-side where it can be updated without a client
                // release.
                "model" to "${Build.MANUFACTURER} ${Build.MODEL}",
            )
        synchronized(lock) { screen }?.let { out["screen"] = it }
        return out
    }

    internal fun resetForTests() {
        synchronized(lock) { screen = null }
    }
}

// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriConfig.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import java.util.concurrent.atomic.AtomicReference

/**
 * Set once by [Sentori.start]; everything else reads it and treats
 * absence as "not initialised, so every verb is a no-op".
 *
 * That default is the failure-isolation half of the client zero-cost
 * rule: an app that mis-wires its token gets a silent SDK, never an
 * exception on a path it did not know it had.
 */
class SentoriConfig
@JvmOverloads
constructor(
    val token: String,
    ingestUrl: String,
    val release: String,
    val environment: String,
    /**
     * The integrator's own health endpoint, carried on batches and
     * probed server-side. The app never pings it — a monitoring SDK
     * that adds traffic to the thing it monitors has picked the wrong
     * side of the bargain.
     */
    val backendHealthUrl: String? = null,
) {
    /**
     * A trailing slash here becomes `//v1/events:batch`, which some
     * proxies answer and some 404.
     */
    val ingestUrl: String = ingestUrl.trimEnd('/')

    companion object {
        private val ref = AtomicReference<SentoriConfig?>(null)

        @JvmStatic val current: SentoriConfig? get() = ref.get()

        @JvmStatic val isInitialised: Boolean get() = ref.get() != null

        internal fun set(config: SentoriConfig?) = ref.set(config)

        internal fun resetForTests() = ref.set(null)
    }
}

/**
 * Reported in the `Sentori-Sdk` header so a server-side problem can be
 * attributed to a client version.
 *
 * Mirrors the constant in `sdk/react-native/src/transport.ts` and the
 * Swift one, and is written by `scripts/sync-sdk-version.mjs` — a
 * version string nothing writes goes stale, and only ever in the
 * direction of a lie.
 */
object SentoriVersion {
    const val CURRENT = "2.0.1"
}

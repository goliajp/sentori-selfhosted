// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriMask.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

/**
 * The privacy half of visual replay.
 *
 * A host registers a lambda returning the identifiers of views that
 * must never appear in a captured frame — a camera preview, a card
 * number, anything identifying. The capture paints over those
 * subtrees in the same render pass, so the pixels never leave the
 * device.
 *
 * There was no way for a native host to say any of this. The capture
 * has taken a list of identifiers all along; only React Native had
 * somewhere to put one, so an app using the Kotlin SDK directly could
 * enable replay and had no means of excluding anything from it.
 *
 * Identifiers are matched against a view's tag, which is what React
 * Native's `nativeID` becomes — so a mixed app registering one string
 * masks the same view from either side.
 */
object SentoriMask {
    /**
     * Returns the identifiers to mask. Called once per captured frame,
     * so keep it cheap: return a cached list rather than walking a
     * view tree.
     */
    fun interface Query {
        fun ids(): List<String>
    }

    @Volatile private var query: Query? = null

    /** Register, or with null, clear. */
    @JvmStatic
    fun register(query: Query?) {
        this.query = query
    }

    /**
     * The identifiers to mask right now.
     *
     * A query that throws masks nothing this frame rather than taking
     * the capture path down with it — the five verbs never throw into
     * the host, and neither does anything reached from a capture tick.
     * The empty-on-failure rule matches the TypeScript registry,
     * because a privacy rule that differs by platform is one nobody
     * can state.
     */
    @JvmStatic
    fun ids(): List<String> {
        val current = query ?: return emptyList()
        return try {
            current.ids()
        } catch (t: Throwable) {
            // `warn`, never `error`: red text in a host's console makes
            // their team think their own app is broken, and the first
            // thing they do about it is remove us.
            android.util.Log.w("sentori", "mask query threw; this frame masks nothing: $t")
            emptyList()
        }
    }

    @JvmStatic
    fun __resetForTests() {
        query = null
    }
}

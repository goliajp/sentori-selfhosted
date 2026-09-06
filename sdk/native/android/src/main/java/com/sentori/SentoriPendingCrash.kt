package com.sentori

import org.json.JSONArray
import org.json.JSONObject

/**
 * The crash that killed the app, sent on the next launch.
 *
 * [SentoriCrashHandler] writes one JSON file per crash from inside an
 * uncaught-exception handler, where almost nothing is safe to do — so
 * it writes a flat, older shape and gets out. Something has to read
 * those files, convert them to the wire format and enqueue them, and
 * until now nothing in this package did: the crash handler filled a
 * directory nobody emptied.
 *
 * React Native has had the missing half since the beginning, in
 * `sdk/react-native/src/native-pending.ts`. This is the same
 * conversion as that and as the Swift one, so a crash arrives
 * identical whichever SDK sent it — the dashboard cannot tell and
 * should not have to.
 *
 * Attachments captured before death are carried in the file and are
 * **not** uploaded yet: that needs the attachment endpoint this
 * package does not speak. They are left in place rather than silently
 * dropped, and the event ships without them — a crash report with no
 * screenshot beats no crash report.
 */
internal object SentoriPendingCrash {

    /** Drain, convert, enqueue. Called once by [Sentori.start]. */
    fun ship() {
        val files = SentoriCrashHandler.consumePending()
        if (files.isEmpty()) return

        for (text in files) {
            val raw =
                try {
                    JSONObject(text)
                } catch (_: Throwable) {
                    // One corrupt file must not cost the others.
                    continue
                }
            // The screenshot and view tree the handler captured as the
            // app died. They travel in the file and never on the wire —
            // the server takes them separately, keyed on an event it
            // must already have.
            val blobs = raw.optJSONArray("_pendingAttachments")
            raw.remove("_pendingAttachments")
            val wire = toWire(raw)
            val id = wire["id"] as? String

            // Registered before this event is queued, and keyed on its
            // id. Two races close here, and one of them was real: the
            // first version registered after `flush`, which hands the
            // send to a worker and returns — so on a fast network the
            // batch was accepted before the block existed and the
            // attachments never uploaded at all. Registering before the
            // loop's flush is not enough either, since `enqueue` sends
            // of its own accord once ten events are queued, and a run
            // with ten crash files would flush mid-loop.
            if (blobs != null && blobs.length() > 0 && id != null) {
                SentoriTransport.afterDelivery(setOf(id)) {
                    for (i in 0 until blobs.length()) {
                        val blob = blobs.optJSONObject(i) ?: continue
                        val kind = blob.optString("kind", "")
                        val base64 = blob.optString("base64", "")
                        if (kind.isEmpty() || base64.isEmpty()) continue
                        SentoriAttachment.upload(
                            eventId = id,
                            kind = kind,
                            base64 = base64,
                            mediaType =
                                blob.optString("mediaType", "").ifEmpty {
                                    "application/octet-stream"
                                },
                            source = blob.optString("source", "").ifEmpty { "android" },
                        )
                    }
                }
            }
            SentoriTransport.enqueue(wire)
        }
        SentoriTransport.flush()
    }

    /**
     * The flat on-disk shape into the nested wire one. Mirrors
     * `toWire` in `native-pending.ts` and `SentoriPendingCrash.swift`
     * field for field.
     */
    fun toWire(raw: JSONObject): Map<String, Any?> {
        val rawError = raw.optJSONObject("error") ?: JSONObject()
        val rawStack = rawError.optJSONArray("stack") ?: JSONArray()
        val frames =
            (0 until rawStack.length()).mapNotNull { i ->
                val f = rawStack.optJSONObject(i) ?: return@mapNotNull null
                buildMap<String, Any?> {
                    f.optString("file", "").takeIf { it.isNotEmpty() }?.let { put("file", it) }
                    f.optString("function", "").takeIf { it.isNotEmpty() }
                        ?.let { put("function", it) }
                    if (f.has("line")) put("line", f.optInt("line"))
                    if (f.has("inApp")) put("inApp", f.optBoolean("inApp"))
                }
            }

        val payload =
            mutableMapOf<String, Any?>(
                "error" to
                    mapOf(
                        // A crash with no type is still a crash. The
                        // default names what it is rather than leaving
                        // the issue title empty.
                        "type" to
                            rawError.optString("type", "").ifEmpty { "NativeCrash" },
                        "message" to rawError.optString("message", ""),
                        "stack" to frames,
                    ),
                // Tells the dashboard this arrived from the grave
                // rather than from a caught error — the difference
                // between "the app died" and "the app noticed".
                "nativeCrash" to true,
            )
        raw.optJSONObject("device")?.let { payload["device"] = it.toMap() }
        raw.optJSONObject("app")?.let { payload["app"] = it.toMap() }

        return mapOf(
            "id" to raw.optString("id", "").ifEmpty { Sentori.newEventId() },
            "kind" to "error",
            // The file says `timestamp`; the wire says `occurredAt`.
            // Keeping the crash's own time matters — it is the moment
            // the app died, not the moment the next launch noticed.
            "occurredAt" to
                raw.optString("timestamp", "").ifEmpty {
                    Sentori.iso8601(java.util.Date())
                },
            "platform" to if (raw.optString("platform") == "ios") "ios" else "android",
            "release" to raw.optString("release", ""),
            "environment" to raw.optString("environment", ""),
            "payload" to payload,
        )
    }

    private fun JSONObject.toMap(): Map<String, Any?> =
        keys().asSequence().associateWith { k ->
            when (val v = get(k)) {
                is JSONObject -> v.toMap()
                JSONObject.NULL -> null
                else -> v
            }
        }
}

package com.sentori

import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/**
 * Blobs that belong to an event: the screenshot taken as the app died,
 * the view tree behind it.
 *
 * They upload *after* the event, never before. The server keys an
 * attachment on an event id it must already know, so an upload that
 * races the batch 404s — and it always wins that race, because the
 * batch waits for a flush and the upload does not.
 *
 * The multipart body is built by hand. React Native learned why the
 * expensive way: its `FormData` file part wants a `uri`, the `data:`
 * form throws a bare network error on iOS, and every JS attachment
 * died silently for a release. Doing it literally is both shorter and
 * checkable.
 *
 * Mirrors `SentoriAttachment.swift` part for part.
 */
internal object SentoriAttachment {

    /**
     * The kinds the server's CHECK constraint accepts, from
     * `handlers/sdk/events_attachments.rs`. Anything else is a 400,
     * so it is dropped here with the round trip saved.
     *
     * Worth keeping honest: the first version of this list had three
     * entries, and the crash handler's view tree — written on both
     * platforms, kind `viewTree` — was not one of them. It would have
     * been dropped on the way out by the very code meant to deliver
     * it, silently, with the event arriving intact and the evidence
     * missing.
     */
    val KNOWN =
        setOf(
            "logTail",
            "replay",
            "screens",
            "screenshot",
            "sessionTrail",
            "stateSnapshot",
            "viewTree",
        )

    /** `android` | `ios` | `js`, likewise CHECK-constrained. */
    val KNOWN_SOURCES = setOf("android", "ios", "js")

    /**
     * Test seam: capture instead of sending.
     *
     * What is worth asserting is *what* would be uploaded and *when* —
     * the kind, the event it is keyed on, the body, and that none of
     * it moves before the batch lands. None of that is about TCP; the
     * wire has its own gate in `android-live-ingest`.
     */
    internal var recorderForTests: ((String, String, String) -> Unit)? = null

    private val worker = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r, "sentori-attachment").apply { isDaemon = true }
    }

    /**
     * Upload one blob against an event the server already has.
     *
     * Fire and forget: nothing here reaches the caller, and a failure
     * costs the attachment rather than the crash report it belongs to.
     */
    fun upload(
        eventId: String,
        kind: String,
        base64: String,
        mediaType: String,
        source: String = "android",
        completion: ((Boolean) -> Unit)? = null,
    ) {
        val config = SentoriConfig.current
        if (config == null || kind !in KNOWN || source !in KNOWN_SOURCES) {
            completion?.invoke(false)
            return
        }

        val boundary = "----sentori-$eventId"
        val body = multipartBody(boundary, kind, mediaType, base64, source)

        recorderForTests?.let {
            it(eventId, kind, body)
            completion?.invoke(true)
            return
        }

        worker.execute {
            var conn: HttpURLConnection? = null
            var ok = false
            try {
                val url =
                    "${config.ingestUrl}/v1/events/" +
                        URLEncoder.encode(eventId, "UTF-8") +
                        "/attachments/" +
                        URLEncoder.encode(kind, "UTF-8")
                conn = URL(url).openConnection() as HttpURLConnection
                conn.requestMethod = "POST"
                conn.setRequestProperty("Content-Type", "multipart/form-data; boundary=$boundary")
                conn.setRequestProperty("Authorization", "Bearer ${config.token}")
                conn.setRequestProperty("Sentori-Sdk", "kotlin/${SentoriVersion.CURRENT}")
                conn.connectTimeout = 15_000
                conn.readTimeout = 30_000
                conn.doOutput = true
                conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
                ok = conn.responseCode in 200..299
            } catch (_: Throwable) {
                // An attachment that will not go is an attachment lost,
                // and never the host app's problem.
            } finally {
                conn?.disconnect()
                completion?.invoke(ok)
            }
        }
    }

    /**
     * The multipart body, visible so a test can read what would go on
     * the wire. Building it correctly is the whole job here, and the
     * failure mode is a 2xx that stored the wrong bytes.
     */
    internal fun multipartBody(
        boundary: String,
        kind: String,
        mediaType: String,
        base64: String,
        source: String,
    ): String =
        buildString {
            append("--$boundary\r\n")
            append("Content-Disposition: form-data; name=\"file\"; filename=\"$kind.bin\"\r\n")
            append("Content-Type: $mediaType\r\n")
            // The server decodes when told to. Without this header it
            // stores the base64 text as the image, which renders as
            // nothing and looks like a capture problem.
            append("Content-Transfer-Encoding: base64\r\n")
            append("\r\n$base64\r\n")
            append("--$boundary\r\n")
            append("Content-Disposition: form-data; name=\"source\"\r\n")
            append("\r\n$source\r\n")
            append("--$boundary--\r\n")
        }
}

package com.sentori

import android.app.Application
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.util.Base64
import android.view.PixelCopy
import android.view.View
import android.view.ViewGroup
import android.view.Window
import java.io.ByteArrayOutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject

/**
 * Phase 42 sub-F.01/02/08 — capture the current activity's screen +
 * view tree at native crash / ANR time.
 *
 * Lives separately from `SentoriCrashHandler` so we can also invoke
 * it from `SentoriAnrWatchdog` (sub-F.07: ANR detector fires →
 * snapshot main thread state → enqueue with the ANR event).
 *
 * The Android side of this story is harder than iOS:
 *   - iOS NSException fires on the main thread before tear-down → we
 *     can drive UIKit synchronously.
 *   - On Android, `Thread.UncaughtExceptionHandler` is on whatever
 *     thread crashed, *not* always the main one. Even on main, the
 *     activity might be partially torn down by the time we run.
 *   - `View.draw(Canvas)` works on the main thread (needs the view's
 *     RenderNode to be live); we use `PixelCopy.request` (API 24+)
 *     instead because it's GPU-driven, non-blocking on main, and
 *     produces a Bitmap even if the main thread is wedged (sub-F.07
 *     ANR path needs this).
 *   - Bitmap.compress(WEBP_LOSSY, ...) is Android 11+ only. We pick
 *     it when available, fall back to JPEG q=70 below 30.
 *
 * Output mirrors the iOS Swift helper + the sub-G dashboard schema:
 *
 *     {
 *       "screenshot": { "base64": "...", "mediaType": "image/webp|jpeg" },
 *       "viewTree":   { "rootId": "n1", "nodes": { ... } }
 *     }
 */
object SentoriScreenshotCapture {

    private const val MAX_LONG_EDGE_PX = 480
    private const val WEBP_QUALITY = 70
    private const val JPEG_QUALITY = 70
    private const val MAX_TREE_DEPTH = 10
    private const val MAX_NODES = 1500
    private const val PIXEL_COPY_TIMEOUT_MS = 200L

    // v1.0.0-rc.2 — diagnostic readout so the JS side can ask
    // "why did screenshot return null" without parsing logcat. Mirrors
    // SentoriReplayCapture.probe() and the iOS Swift probe.
    @Volatile private var lastDiagPath: String = "none(not-yet-called)"
    @Volatile private var lastDiagW: Int = 0
    @Volatile private var lastDiagH: Int = 0

    /**
     * Snapshot of the most recent capture attempt — what code path
     * resolved an Activity, what the decor view's dimensions were,
     * and what call source the foreground tracker last saw the
     * Activity from. Used by `probeNativeScreenshot()` on the JS
     * side so Insight can ship raw diagnostic state back without
     * needing logcat access.
     */
    @JvmStatic
    fun probe(): Map<String, Any> {
        val tracked = SentoriForegroundActivity.current()
        return mapOf(
            "lastPath" to lastDiagPath,
            "lastWidth" to lastDiagW,
            "lastHeight" to lastDiagH,
            "trackedSource" to SentoriForegroundActivity.lastPath,
            "trackedActivity" to (tracked?.javaClass?.name ?: "null"),
            "decorViewFound" to (tracked?.window?.decorView != null),
        )
    }

    /**
     * Idempotent. Wires the screenshot helper into the shared
     * foreground-activity tracker; kept as a public entrypoint for
     * backwards compat with existing call sites (the crash handler
     * still calls this), but the actual lifecycle subscription lives
     * in [SentoriForegroundActivity].
     */
    @JvmStatic
    fun register(application: Application) {
        SentoriForegroundActivity.install(application)
    }

    /**
     * Top-level entry. Returns a JSON-shape `{screenshot, viewTree}`
     * map, or `null` if the activity is gone / API < 24 / capture
     * timed out. Safe to call from any thread; the PixelCopy request
     * itself runs on its own HandlerThread so the calling thread
     * (main during NSException-equivalent / ANR detector) doesn't
     * block on the GPU.
     */
    @JvmStatic
    fun captureKeyWindow(): Map<String, Any>? {
        val activity = SentoriForegroundActivity.current()
        if (activity == null) {
            lastDiagPath = "activity.null"
            return null
        }
        val window = activity.window
        if (window == null) {
            lastDiagPath = "window.null"
            return null
        }
        val out = mutableMapOf<String, Any>()
        captureScreen(window, emptySet())?.let { (base64, mediaType) ->
            out["screenshot"] = mapOf("base64" to base64, "mediaType" to mediaType)
        }
        out["viewTree"] = walkTree(window.decorView)
        return if (out.isEmpty()) null else out
    }

    /// v0.7.3 — JS-triggered screenshot path with consumer-supplied
    /// mask IDs. Returns `{ base64, mediaType }` or `null`; matches
    /// the iOS bridge contract. Native walks the view tree by
    /// `View.tag` (RN bridges JS `nativeID` to the default String tag
    /// on the underlying View) and paints black rectangles over each
    /// masked subview's frame on the captured bitmap.
    @JvmStatic
    fun captureScreenshotWithMask(maskedIds: List<String>): Map<String, String>? {
        val activity = SentoriForegroundActivity.current()
        if (activity == null) {
            lastDiagPath = "activity.null"
            return null
        }
        val window = activity.window
        if (window == null) {
            lastDiagPath = "window.null"
            return null
        }
        val (base64, mediaType) = captureScreen(window, maskedIds.toHashSet()) ?: return null
        return mapOf("base64" to base64, "mediaType" to mediaType)
    }

    // ── screenshot ────────────────────────────────────────────────

    private fun captureScreen(window: Window, maskedIds: Set<String>): Pair<String, String>? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.N) {
            // PixelCopy is API 24+. Older Android: fall back to a
            // `View.draw(Canvas)` path that *must* run on main and
            // requires the activity not to be torn down. Skip for
            // now; v0.6.1 SDK can add the fallback if real-world
            // data shows we have users below API 24.
            lastDiagPath = "api.unsupported"
            return null
        }
        val decor = window.decorView
        if (decor == null) {
            lastDiagPath = "decorView.null"
            return null
        }
        val w = decor.width
        val h = decor.height
        lastDiagW = w
        lastDiagH = h
        if (w <= 0 || h <= 0) {
            lastDiagPath = "decorView.zero-size"
            return null
        }

        // Long-edge scale.
        val longEdge = maxOf(w, h).toFloat()
        val scale = if (longEdge > MAX_LONG_EDGE_PX) MAX_LONG_EDGE_PX / longEdge else 1f
        val outW = (w * scale).toInt().coerceAtLeast(1)
        val outH = (h * scale).toInt().coerceAtLeast(1)
        val bitmap = Bitmap.createBitmap(outW, outH, Bitmap.Config.ARGB_8888)

        val latch = CountDownLatch(1)
        var success = false
        val handlerThread = HandlerThread("sentori-pixel-copy").apply { start() }
        val handler = Handler(handlerThread.looper)
        try {
            // Render the live window into our smaller Bitmap. PixelCopy
            // does the scale internally (`request(Window, Rect, Bitmap, ...)`
            // signature on API 26+; on 24/25 we use the rectless variant
            // and accept the unscaled bitmap, then downscale ourselves).
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                PixelCopy.request(
                    window,
                    bitmap,
                    { result -> success = result == PixelCopy.SUCCESS; latch.countDown() },
                    handler,
                )
            } else {
                @Suppress("DEPRECATION")
                PixelCopy.request(
                    window,
                    bitmap,
                    { result -> success = result == PixelCopy.SUCCESS; latch.countDown() },
                    handler,
                )
            }
            latch.await(PIXEL_COPY_TIMEOUT_MS, TimeUnit.MILLISECONDS)
        } catch (t: Throwable) {
            lastDiagPath = "pixelCopy.threw:${t.javaClass.simpleName}"
            return null
        } finally {
            handlerThread.quitSafely()
        }
        if (!success) {
            lastDiagPath = "pixelCopy.notSuccess"
            return null
        }
        lastDiagPath = "ok"

        // v0.7.3 — paint black rectangles over masked subviews on the
        // already-captured bitmap. We get window-relative coordinates
        // from `getLocationInWindow` (respects parent transforms) and
        // scale them down to the output bitmap size.
        if (maskedIds.isNotEmpty()) {
            val regions = findMaskedViews(decor, maskedIds)
            if (regions.isNotEmpty()) {
                val canvas = Canvas(bitmap)
                val paint = Paint().apply { color = Color.BLACK }
                val rootLoc = IntArray(2).also { decor.getLocationInWindow(it) }
                val tmp = IntArray(2)
                for (v in regions) {
                    v.getLocationInWindow(tmp)
                    val x = (tmp[0] - rootLoc[0]) * scale
                    val y = (tmp[1] - rootLoc[1]) * scale
                    val rw = v.width * scale
                    val rh = v.height * scale
                    canvas.drawRect(x, y, x + rw, y + rh, paint)
                }
            }
        }

        val baos = ByteArrayOutputStream(64 * 1024)
        val mediaType: String
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: native WEBP_LOSSY ~30% smaller than JPEG q=70.
            bitmap.compress(Bitmap.CompressFormat.WEBP_LOSSY, WEBP_QUALITY, baos)
            mediaType = "image/webp"
        } else {
            bitmap.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, baos)
            mediaType = "image/jpeg"
        }
        bitmap.recycle()
        val base64 = Base64.encodeToString(baos.toByteArray(), Base64.NO_WRAP)
        return Pair(base64, mediaType)
    }

    /// Depth-first walk that stops descending once a masked subtree
    /// is hit. RN bridges JS `nativeID` to `View.setTag(Object)` with
    /// a String value — that's why we cast to `String` rather than
    /// looking at the int resource-id tag space.
    private fun findMaskedViews(root: View, ids: Set<String>): List<View> {
        val out = mutableListOf<View>()
        fun walk(v: View) {
            val tag = v.tag as? String
            if (tag != null && ids.contains(tag)) {
                out.add(v)
                return
            }
            if (v is ViewGroup) {
                for (i in 0 until v.childCount) walk(v.getChildAt(i))
            }
        }
        walk(root)
        return out
    }

    // ── view tree ─────────────────────────────────────────────────

    /** Synchronously walk the view hierarchy from `root`. Safe to call
     *  from any thread *as long as no concurrent layout pass is
     *  invalidating subview lists* — at crash time the main thread is
     *  paused on the exception handler, so the read is race-free. */
    private fun walkTree(root: View): Map<String, Any> {
        val nodes = mutableMapOf<String, Any>()
        var counter = 0
        var nodeCount = 0

        fun nextId(): String {
            counter += 1
            return "n$counter"
        }

        fun walk(view: View, depth: Int): String {
            val id = nextId()
            nodeCount += 1
            val children = mutableListOf<String>()
            if (depth < MAX_TREE_DEPTH && nodeCount < MAX_NODES && view is ViewGroup) {
                for (i in 0 until view.childCount) {
                    if (nodeCount >= MAX_NODES) break
                    children.add(walk(view.getChildAt(i), depth + 1))
                }
            }
            val rect = "${view.left},${view.top},${view.width},${view.height}"
            val propsSummary = mutableMapOf(
                "frame" to rect,
                "alpha" to String.format("%.2f", view.alpha),
                "hidden" to (view.visibility != View.VISIBLE).toString(),
            )
            view.contentDescription?.toString()?.takeIf { it.isNotEmpty() }?.let {
                propsSummary["contentDescription"] = if (it.length > 200) it.substring(0, 200) else it
            }
            nodes[id] = mapOf(
                "type" to "View",
                "name" to view.javaClass.simpleName,
                "props_summary" to propsSummary,
                "children" to children,
            )
            return id
        }

        val rootId = walk(root, 0)
        return mapOf("rootId" to rootId, "nodes" to nodes)
    }

    // ── helpers for the crash-handler JSON path ───────────────────

    /**
     * Convert a Kotlin Map-tree into a `JSONObject`-tree suitable for
     * embedding inside the event JSON written by `SentoriCrashHandler`.
     * Public so the crash handler can use it.
     */
    @JvmStatic
    fun toJson(value: Any?): Any {
        return when (value) {
            null -> JSONObject.NULL
            is Map<*, *> -> JSONObject().apply {
                for ((k, v) in value) put(k.toString(), toJson(v))
            }
            is List<*> -> JSONArray().apply {
                for (v in value) put(toJson(v))
            }
            else -> value
        }
    }

    /**
     * Convenience for `Canvas.draw` benchmarking in instrumentation
     * tests (sub-F.10). Renders the input `view` onto a Bitmap on the
     * caller's thread — *do not* use this at crash time; it only
     * exists for test latency measurements.
     */
    @JvmStatic
    fun benchmarkRenderToBitmapBlocking(view: View): Long {
        val w = view.width.coerceAtLeast(1)
        val h = view.height.coerceAtLeast(1)
        val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        val started = System.nanoTime()
        view.draw(canvas)
        val elapsed = System.nanoTime() - started
        bmp.recycle()
        return elapsed
    }
}

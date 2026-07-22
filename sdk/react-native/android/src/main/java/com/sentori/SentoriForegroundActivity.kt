package com.sentori

import android.app.Activity
import android.app.Application
import android.os.Bundle
import java.lang.ref.WeakReference

/**
 * v1.0.0-rc.2 — process-wide foreground-Activity tracker.
 *
 * Both [SentoriScreenshotCapture] and [SentoriReplayCapture] need a
 * pointer to the currently-foregrounded Activity to drive their
 * native captures. The previous implementation registered each
 * helper's own `ActivityLifecycleCallbacks` inside
 * [SentoriCrashHandler.register], which runs from the Expo module's
 * `OnCreate` lifecycle — **after** the MainActivity has already been
 * resumed in the typical Insight / Expo dev-launcher topology.
 * `ActivityLifecycleCallbacks` does not back-fill the current state,
 * it only forwards future events, so `lastActivity` would stay null
 * for the entire session if the user never backgrounded the app.
 *
 * Fix mirrors the iOS keyWindow 4-layer fallback:
 *
 *   1. Lifecycle callbacks track resumed/started activities going
 *      forward (the same approach as before, kept).
 *   2. **On first install** we also probe `ActivityThread` reflection
 *      to back-fill whatever Activity is currently foreground, so the
 *      already-resumed window is seen by the SDK from boot.
 *   3. If reflection fails (e.g. on a future Android release that
 *      removes `ActivityThread` access), the lifecycle callbacks
 *      still catch the next foreground transition.
 *
 * `lastPath` carries the diagnostic provenance for `probeWireframe` /
 * `probeScreenshot`, so the JS side (and Insight) can tell whether
 * the SDK was working off a live lifecycle event or fell back to
 * reflection.
 */
object SentoriForegroundActivity {

    @Volatile private var lastActivity: WeakReference<Activity>? = null
    @Volatile var lastPath: String = "none(not-yet-resolved)"
        private set

    /** Idempotent. Call from [SentoriCrashHandler.register]; subsequent
     *  calls are no-ops because the same callbacks object would be
     *  registered twice otherwise. */
    @Volatile private var registered = false

    @Synchronized
    fun install(application: Application) {
        if (registered) {
            // Even on second install attempt, try the reflection
            // back-fill again — process state may have advanced.
            backfillFromActivityThread()
            return
        }
        registered = true
        application.registerActivityLifecycleCallbacks(object :
            Application.ActivityLifecycleCallbacks {
            override fun onActivityCreated(a: Activity, b: Bundle?) {
                set(a, "lifecycle.created")
            }
            override fun onActivityStarted(a: Activity) {
                set(a, "lifecycle.started")
            }
            override fun onActivityResumed(a: Activity) {
                set(a, "lifecycle.resumed")
            }
            override fun onActivityPaused(a: Activity) {}
            override fun onActivityStopped(a: Activity) {}
            override fun onActivitySaveInstanceState(a: Activity, b: Bundle) {}
            override fun onActivityDestroyed(a: Activity) {}
        })
        // Back-fill in case the Activity was already resumed before
        // we got installed (the dev-launcher → MainActivity transition
        // happens before the Expo module's OnCreate fires).
        backfillFromActivityThread()
    }

    /** Public for tests + the helpers' own explicit `setActivity`
     *  paths (kept for backwards-compat). */
    fun set(activity: Activity, source: String) {
        lastActivity = WeakReference(activity)
        lastPath = source
    }

    fun current(): Activity? {
        val live = lastActivity?.get()
        if (live != null) return live
        // Last-ditch: try reflection again in case install() ran before
        // any Activity existed. Cheap if it fails.
        backfillFromActivityThread()
        return lastActivity?.get()
    }

    /**
     * Best-effort foreground-Activity lookup using non-SDK reflection.
     * Walks `ActivityThread.sCurrentActivityThread.mActivities` (an
     * `ArrayMap<IBinder, ActivityClientRecord>`) and finds the record
     * whose `paused` field is false. This is what Stetho / LeakCanary
     * / Firebase Performance do, with the same caveat that future
     * Android versions may break it — failures are silent and the
     * lifecycle-callbacks path still wins.
     *
     * Tagged via `lastPath = "reflection.activityThread"` so the JS
     * probe can see when reflection had to step in.
     */
    @Suppress("PrivateApi", "UNCHECKED_CAST")
    private fun backfillFromActivityThread() {
        try {
            val activityThreadClass = Class.forName("android.app.ActivityThread")
            val currentActivityThread = activityThreadClass
                .getDeclaredMethod("currentActivityThread")
                .also { it.isAccessible = true }
                .invoke(null) ?: return
            val activitiesField = activityThreadClass
                .getDeclaredField("mActivities")
                .also { it.isAccessible = true }
            val activities = activitiesField.get(currentActivityThread) ?: return

            // ArrayMap<IBinder, ActivityClientRecord> — iterate via reflection
            // so we don't have to depend on the (also private) ArrayMap shape.
            val valuesIter = (activities as? Map<*, *>)?.values?.iterator() ?: return
            while (valuesIter.hasNext()) {
                val record = valuesIter.next() ?: continue
                val recordClass = record.javaClass
                val pausedField = try {
                    recordClass.getDeclaredField("paused").also { it.isAccessible = true }
                } catch (_: NoSuchFieldException) { null }
                val activityField = try {
                    recordClass.getDeclaredField("activity").also { it.isAccessible = true }
                } catch (_: NoSuchFieldException) { null } ?: continue
                val paused = pausedField?.getBoolean(record) ?: false
                val candidate = activityField.get(record) as? Activity ?: continue
                if (!paused && !candidate.isFinishing && !candidate.isDestroyed) {
                    set(candidate, "reflection.activityThread")
                    return
                }
            }
        } catch (_: Throwable) {
            // Reflection unavailable; the lifecycle-callbacks path
            // will still catch the next foreground transition.
        }
    }
}

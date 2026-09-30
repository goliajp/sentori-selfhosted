package com.sentori

import android.app.Activity
import android.app.Application
import android.content.ComponentCallbacks2
import android.content.Context
import android.content.res.Configuration
import android.os.Bundle

/**
 * What the app was doing when something went wrong.
 *
 * The signal ring — the sixty seconds of context that ride an error —
 * could not say whether the app was in the foreground, had just come
 * back to it, or was under memory pressure, which is the first
 * question anyone asks about a crash they cannot reproduce. And an
 * event queued in the seconds before the user left went with the
 * process: the flush timer had not fired yet.
 *
 * `ActivityLifecycleCallbacks` rather than `ProcessLifecycleOwner`:
 * the latter lives in `androidx.lifecycle`, and an SDK that drags a
 * dependency into a host app has spent some of the host's budget to
 * save its own. The foreground count here is what that library does.
 */
object SentoriLifecycle {
    private var registered = false
    private var started = 0

    @Synchronized
    fun register(context: Context) {
        if (registered) return
        val app = context.applicationContext as? Application ?: return
        registered = true

        app.registerActivityLifecycleCallbacks(
            object : Application.ActivityLifecycleCallbacks {
                override fun onActivityStarted(activity: Activity) {
                    // Zero to one is the app coming forward. Any other
                    // increment is one screen replacing another, which
                    // the ring does not need a signal for.
                    if (started == 0) signal("app.foreground")
                    started += 1
                }

                override fun onActivityStopped(activity: Activity) {
                    started -= 1
                    if (started <= 0) {
                        started = 0
                        signal("app.background")
                        // The last moment the process is reliably
                        // alive and ours to use.
                        SentoriTransport.flush()
                    }
                }

                override fun onActivityCreated(activity: Activity, state: Bundle?) = Unit

                override fun onActivityResumed(activity: Activity) = Unit

                override fun onActivityPaused(activity: Activity) = Unit

                override fun onActivitySaveInstanceState(activity: Activity, out: Bundle) = Unit

                override fun onActivityDestroyed(activity: Activity) = Unit
            }
        )

        app.registerComponentCallbacks(
            object : ComponentCallbacks2 {
                override fun onTrimMemory(level: Int) {
                    // A crash a minute after this is very often this.
                    signal("app.trimMemory", level)
                }

                override fun onLowMemory() = signal("app.lowMemory", null)

                override fun onConfigurationChanged(config: Configuration) = Unit
            }
        )
    }

    /**
     * Iron rule, dimension 3: this runs inside the host's own
     * lifecycle callback, so anything thrown here surfaces as their
     * bug in their stack.
     */
    private fun signal(name: String, level: Int? = null) {
        try {
            val data = mutableMapOf<String, Any?>("name" to name)
            if (level != null) data["level"] = level
            SentoriSignalRing.push("lifecycle", data)
        } catch (_: Throwable) {
        }
    }

    @Synchronized
    fun __resetForTests() {
        registered = false
        started = 0
    }
}

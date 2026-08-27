package com.sentori

/**
 * v0.9.5 #7 — Android NDK origin detection (stub).
 *
 * Pure-Kotlin heuristics to tag a Throwable as "originated in native
 * code" so the dashboard can show NDK crashes separately from JVM
 * crashes. Real breakpad/crashpad integration (with minidump +
 * dump_syms symbolicator) is queued for v1.1 — see
 * `docs/design/v1-roadmap.md` #7.
 */
object SentoriNativeOrigin {

    /** Returns true iff this throwable likely originated in native
     *  (NDK / .so / JNI) code. Used by `SentoriCrashHandler.write` to
     *  flip the `native_signal` tag on the event. */
    @JvmStatic
    fun looksNative(t: Throwable): Boolean {
        val name = t.javaClass.simpleName
        if (name == "UnsatisfiedLinkError") return true
        // OutOfMemoryError can be either JVM or native allocator;
        // bias toward native since pure-JVM OOM is rare on modern
        // Android with heap auto-growth.
        if (name == "OutOfMemoryError") return true
        return t.stackTrace.any { f ->
            val cls = f.className
            cls.contains("jni", ignoreCase = true) ||
                cls.contains("native", ignoreCase = true) ||
                f.fileName?.endsWith(".so") == true
        }
    }
}

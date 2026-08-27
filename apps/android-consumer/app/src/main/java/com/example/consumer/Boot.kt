package com.example.consumer

import android.app.Activity
import android.content.Context
import com.sentori.Sentori
import com.sentori.SentoriConfig
import com.sentori.SentoriPush

/**
 * Every call an integrator makes, written the way the docs say to
 * write it.
 *
 * Nothing here runs — this file exists to be *compiled* against the
 * published artifact. That is the claim `android-artifact` cannot
 * make: it proves the AAR assembles and its POM is complete, not that
 * an app can resolve it, see the symbols, and satisfy the types.
 *
 * The Swift side learned the difference the hard way. Its package
 * built inside the monorepo for a week and failed the first time it
 * was built from a clean checkout of what would actually be
 * published.
 *
 * If a signature changes in a way that breaks callers, this stops
 * compiling — which is the point, and is a nicer place to find out
 * than an integrator's build.
 */
object Boot {

    fun start(context: Context) {
        Sentori.start(
            SentoriConfig(
                token = "st_example",
                ingestUrl = "https://sentori.golia.jp",
                release = "com.example.consumer@1.0.0+1",
                environment = "production",
            ),
            context,
        )
        Sentori.user(id = "usr_1", email = null)
        Sentori.context(mapOf("tenant" to "acme"))
    }

    fun verbs(e: Throwable) {
        Sentori.error(e, mapOf("cartId" to "c_1"))
        Sentori.error("something went wrong", "PaymentError")
        Sentori.warn("checkout.slow", mapOf("ms" to 3200))
        Sentori.trace("cart.opened")
        Sentori.trace("tick", quiet = true)
        Sentori.assert("total.positive", true)
        Sentori.probe("SEN-482")
        Sentori.pushSignal("nav", mapOf("to" to "/checkout"))
    }

    fun push(context: Context, activity: Activity) {
        Sentori.push.register(
            context = context,
            activity = activity,
            onMessage = { _ -> },
            onTap = { _ -> },
        ) { result ->
            when (result) {
                is SentoriPush.Result.Success -> println(result.handle)
                is SentoriPush.Result.Failure ->
                    // Every reason an integrator has to branch on. If a
                    // case is added or renamed, this `when` stops being
                    // exhaustive and the build says so.
                    when (result.reason) {
                        SentoriPush.Failure.NOT_INITIALISED,
                        SentoriPush.Failure.PERMISSION_DENIED,
                        SentoriPush.Failure.NO_TRANSPORT,
                        SentoriPush.Failure.TOKEN_TIMEOUT,
                        SentoriPush.Failure.SERVER_REJECTED -> Unit
                    }
            }
        }
        SentoriPush.cachedDeviceHandle(context)
        SentoriPush.permissionStatus(context)
        SentoriPush.unregister(context)
    }
}

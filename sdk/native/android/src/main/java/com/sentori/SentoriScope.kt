package com.sentori

/**
 * Ambient scope: the current user and the context patch that ride
 * every outgoing event. Two verbs own this state; everything else
 * reads it.
 */
object SentoriScope {

    private val lock = Any()
    private var _userKey: String? = null
    private var _traits: Map<String, Any?>? = null
    private val _context = mutableMapOf<String, Any?>()

    /**
     * What to tell when the person changes.
     *
     * The push device row carries the identity, and nothing updated it
     * after registration: an app that registers at launch and signs in
     * ten seconds later — which is every app with a login screen —
     * held a row with no user on it for the life of the install. A
     * send aimed at that person reached nobody and said it had worked.
     *
     * A callback rather than a call into [SentoriPush], so this file
     * keeps knowing nothing about push.
     */
    @Volatile private var onIdentityChange: (() -> Unit)? = null

    /** An identity change announced with no listener installed. */
    @Volatile private var missedAnnounce = false

    /** Register interest in identity changes. Only push does. */
    @JvmStatic
    fun setIdentityListener(listener: (() -> Unit)?) {
        onIdentityChange = listener
        // A change announced while nobody was listening was dropped,
        // and nothing announces it again. Push installs this listener
        // only once a registration has landed, so a host that signs
        // someone in while that request is in flight reached nobody.
        //
        // A flag, not a queue: identity is a current value rather than
        // a stream, so replaying the latest is the whole of it.
        if (listener != null && missedAnnounce) {
            missedAnnounce = false
            runCatching { listener.invoke() }
        }
    }

    /**
     * Identify the person using the app. Only the hash goes on the
     * wire; the id and email stay on the device.
     *
     * Unlike the JavaScript version this is genuinely synchronous —
     * WebCrypto's digest is a promise, so `scope.ts` sets the key a
     * tick later and events sent in that gap carry none.
     * `MessageDigest` has no such gap, so the first event after this
     * call is already addressable.
     *
     * Pass null for both to forget the user on sign-out.
     */
    @JvmStatic
    @JvmOverloads
    fun setUser(id: String?, email: String?, traits: Map<String, Any?>? = null) {
        val key = SentoriIdentity.userKey(id, email)
        synchronized(lock) {
            _userKey = key
            // A call describes the person completely, so one made
            // without traits means they have none rather than "leave
            // the last ones". Absent and empty differ on the wire —
            // absent keeps what the row has, and a signed-out device
            // that kept them would still be selectable as whoever
            // just left.
            _traits = traits?.toMap() ?: emptyMap()
        }
        // Outside the lock: a listener that registers a device must
        // not be holding this while it makes a request.
        val listener = onIdentityChange
        if (listener == null) missedAnnounce = true else runCatching { listener.invoke() }
    }

    /** Merge keys into the ambient context. Later calls win per key. */
    @JvmStatic
    fun patchContext(patch: Map<String, Any?>) {
        synchronized(lock) { _context.putAll(patch) }
    }

    @JvmStatic
    val userKey: String?
        get() = synchronized(lock) { _userKey }

    /**
     * The person's attributes, for the push device row.
     *
     * Null until the host has called [setUser] at all, which is
     * different from an empty map: null leaves the row's traits alone,
     * empty clears them.
     */
    @JvmStatic
    val traits: Map<String, Any?>?
        get() = synchronized(lock) { _traits }

    /**
     * Null rather than an empty map, so an event with no context omits
     * the field instead of carrying `{}`.
     */
    @JvmStatic
    val context: Map<String, Any?>?
        get() = synchronized(lock) { if (_context.isEmpty()) null else _context.toMap() }

    @JvmStatic
    fun clear() {
        synchronized(lock) {
            _userKey = null
            _traits = null
            _context.clear()
        }
        onIdentityChange = null
        missedAnnounce = false
    }
}

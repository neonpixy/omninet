package com.omnidea.core

/** Callback interface for Email subscribers. */
fun interface EmailHandler {
    /** Receives event bytes. Fire-and-forget. */
    fun handle(data: ByteArray)
}

/** Pub/sub event hub. */
class Email : AutoCloseable {
    private var ptr: Long

    init {
        ptr = nativeNew()
        check(ptr != 0L) { "Failed to create Email" }
    }

    /** Subscribe to an email ID. Returns the 16-byte subscriber UUID. */
    fun subscribeRaw(emailId: String, handler: EmailHandler): ByteArray {
        checkOpen()
        return nativeSubscribeRaw(ptr, emailId, handler)
            ?: throw OmnideaError.last()
    }

    /** Send raw bytes to all subscribers of an email ID. */
    fun sendRaw(emailId: String, data: ByteArray = byteArrayOf()) {
        checkOpen()
        nativeSendRaw(ptr, emailId, data)
    }

    /** Unsubscribe by 16-byte UUID. */
    fun unsubscribe(subscriberId: ByteArray) {
        checkOpen()
        nativeUnsubscribe(ptr, subscriberId)
    }

    /** Unsubscribe all subscribers for an email ID. */
    fun unsubscribeAll(emailId: String) {
        checkOpen()
        nativeUnsubscribeAll(ptr, emailId)
    }

    /** Check if any subscribers exist for an email ID. */
    fun hasSubscribers(emailId: String): Boolean {
        checkOpen()
        return nativeHasSubscribers(ptr, emailId)
    }

    /** Get all active email IDs as a JSON array string. */
    fun activeEmailIdsJson(): String? {
        checkOpen()
        return nativeActiveEmailIds(ptr)
    }

    override fun close() {
        if (ptr != 0L) {
            nativeFree(ptr)
            ptr = 0L
        }
    }

    private fun checkOpen() = check(ptr != 0L) { "Email already closed" }

    companion object {
        init { NativeLoader.ensureLoaded() }

        @JvmStatic private external fun nativeNew(): Long
        @JvmStatic private external fun nativeFree(ptr: Long)
        @JvmStatic private external fun nativeSubscribeRaw(ptr: Long, emailId: String, handler: EmailHandler): ByteArray?
        @JvmStatic private external fun nativeSendRaw(ptr: Long, emailId: String, data: ByteArray)
        @JvmStatic private external fun nativeUnsubscribe(ptr: Long, uuid: ByteArray)
        @JvmStatic private external fun nativeUnsubscribeAll(ptr: Long, emailId: String)
        @JvmStatic private external fun nativeHasSubscribers(ptr: Long, emailId: String): Boolean
        @JvmStatic private external fun nativeActiveEmailIds(ptr: Long): String?
    }
}

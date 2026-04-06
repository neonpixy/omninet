package com.omnidea.core

/** Callback interface for Phone handlers. */
fun interface PhoneHandler {
    /** Receives request bytes, returns response bytes (or null on error). */
    fun handle(request: ByteArray): ByteArray?
}

/** RPC switchboard — register handlers, make calls. */
class Phone : AutoCloseable {
    private var ptr: Long

    init {
        ptr = nativeNew()
        check(ptr != 0L) { "Failed to create Phone" }
    }

    /** Register a raw handler for a call ID. */
    fun registerRaw(callId: String, handler: PhoneHandler) {
        checkOpen()
        nativeRegisterRaw(ptr, callId, handler)
    }

    /** Make a raw call. Returns response bytes. Throws on error. */
    fun callRaw(callId: String, data: ByteArray = byteArrayOf()): ByteArray {
        checkOpen()
        return nativeCallRaw(ptr, callId, data)
            ?: throw OmnideaError.last()
    }

    /** Check if a handler is registered for a call ID. */
    fun hasHandler(callId: String): Boolean {
        checkOpen()
        return nativeHasHandler(ptr, callId)
    }

    /** Get all registered call IDs as a JSON array string. */
    fun registeredCallIdsJson(): String? {
        checkOpen()
        return nativeRegisteredCallIds(ptr)
    }

    /** Unregister a handler by call ID. */
    fun unregister(callId: String) {
        checkOpen()
        nativeUnregister(ptr, callId)
    }

    override fun close() {
        if (ptr != 0L) {
            nativeFree(ptr)
            ptr = 0L
        }
    }

    private fun checkOpen() = check(ptr != 0L) { "Phone already closed" }

    companion object {
        init { NativeLoader.ensureLoaded() }

        @JvmStatic private external fun nativeNew(): Long
        @JvmStatic private external fun nativeFree(ptr: Long)
        @JvmStatic private external fun nativeRegisterRaw(ptr: Long, callId: String, handler: PhoneHandler)
        @JvmStatic private external fun nativeCallRaw(ptr: Long, callId: String, data: ByteArray): ByteArray?
        @JvmStatic private external fun nativeHasHandler(ptr: Long, callId: String): Boolean
        @JvmStatic private external fun nativeRegisteredCallIds(ptr: Long): String?
        @JvmStatic private external fun nativeUnregister(ptr: Long, callId: String)
    }
}

package com.omnidea.core

class OmnideaError(message: String) : Exception(message) {
    companion object {
        init { NativeLoader.ensureLoaded() }

        fun last(): OmnideaError {
            val msg = nativeLastError() ?: "unknown error"
            return OmnideaError(msg)
        }

        fun check(result: Int) {
            if (result != 0) throw last()
        }

        @JvmStatic private external fun nativeLastError(): String?
    }
}

/** Loads the native library once. */
internal object NativeLoader {
    private var loaded = false

    @Synchronized
    fun ensureLoaded() {
        if (!loaded) {
            System.loadLibrary("omnidea_jni")
            loaded = true
        }
    }
}

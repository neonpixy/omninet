package com.omnidea.core

/** Callback interface for module shutdown. */
fun interface ShutdownCallback {
    fun onShutdown()
}

/** Module registry with dependency tracking. */
class Contacts : AutoCloseable {
    private var ptr: Long

    init {
        ptr = nativeNew()
        check(ptr != 0L) { "Failed to create Contacts" }
    }

    /** Register a module from a JSON string. Throws on error. */
    fun register(json: String) {
        checkOpen()
        OmnideaError.check(nativeRegister(ptr, json))
    }

    /** Register a module with a shutdown callback. */
    fun registerWithShutdown(json: String, callback: ShutdownCallback) {
        checkOpen()
        OmnideaError.check(nativeRegisterWithShutdown(ptr, json, callback))
    }

    /** Unregister a module by ID. Throws on error. */
    fun unregister(moduleId: String) {
        checkOpen()
        OmnideaError.check(nativeUnregister(ptr, moduleId))
    }

    /** Shut down all modules in dependency order. */
    fun shutdownAll() {
        checkOpen()
        nativeShutdownAll(ptr)
    }

    /** Look up a module by ID. Returns JSON string or null. */
    fun lookup(moduleId: String): String? {
        checkOpen()
        return nativeLookup(ptr, moduleId)
    }

    /** Get all registered modules as a JSON array string. */
    fun allModulesJson(): String? {
        checkOpen()
        return nativeAllModules(ptr)
    }

    /** Get all registered module IDs as a JSON array string. */
    fun registeredModuleIdsJson(): String? {
        checkOpen()
        return nativeRegisteredModuleIds(ptr)
    }

    /** Get all dependents of a module as a JSON array string. */
    fun dependentsOf(moduleId: String): String? {
        checkOpen()
        return nativeDependentsOf(ptr, moduleId)
    }

    override fun close() {
        if (ptr != 0L) {
            nativeFree(ptr)
            ptr = 0L
        }
    }

    private fun checkOpen() = check(ptr != 0L) { "Contacts already closed" }

    companion object {
        init { NativeLoader.ensureLoaded() }

        @JvmStatic private external fun nativeNew(): Long
        @JvmStatic private external fun nativeFree(ptr: Long)
        @JvmStatic private external fun nativeRegister(ptr: Long, json: String): Int
        @JvmStatic private external fun nativeRegisterWithShutdown(ptr: Long, json: String, callback: ShutdownCallback): Int
        @JvmStatic private external fun nativeUnregister(ptr: Long, moduleId: String): Int
        @JvmStatic private external fun nativeShutdownAll(ptr: Long)
        @JvmStatic private external fun nativeLookup(ptr: Long, moduleId: String): String?
        @JvmStatic private external fun nativeAllModules(ptr: Long): String?
        @JvmStatic private external fun nativeRegisteredModuleIds(ptr: Long): String?
        @JvmStatic private external fun nativeDependentsOf(ptr: Long, moduleId: String): String?
    }
}

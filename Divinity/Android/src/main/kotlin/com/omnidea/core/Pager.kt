package com.omnidea.core

/** Notification queue — no callbacks, pure query/mutation. */
class Pager : AutoCloseable {
    private var ptr: Long

    init {
        ptr = nativeNew()
        check(ptr != 0L) { "Failed to create Pager" }
    }

    /** Push a notification from a JSON string. Returns UUID string or null. */
    fun notify(json: String): String? {
        checkOpen()
        return nativeNotify(ptr, json)
    }

    /** Get pending notifications as a JSON array. */
    fun getPendingJson(priorityJson: String? = null): String? {
        checkOpen()
        return nativeGetPending(ptr, priorityJson)
    }

    /** Get unread notifications as a JSON array. */
    fun getUnreadJson(): String? {
        checkOpen()
        return nativeGetUnread(ptr)
    }

    /** Mark a notification as read. Returns true if found. */
    fun markRead(uuidStr: String): Boolean {
        checkOpen()
        return nativeMarkRead(ptr, uuidStr)
    }

    /** Dismiss a notification. Returns true if found. */
    fun dismiss(uuidStr: String): Boolean {
        checkOpen()
        return nativeDismiss(ptr, uuidStr)
    }

    /** Get the count of unread notifications. */
    val badgeCount: Long
        get() {
            checkOpen()
            return nativeBadgeCount(ptr)
        }

    /** Prune expired notifications. */
    fun pruneExpired() {
        checkOpen()
        nativePruneExpired(ptr)
    }

    /** Export pager state as a JSON array. */
    fun exportStateJson(): String? {
        checkOpen()
        return nativeExportState(ptr)
    }

    /** Restore pager state from a JSON array. */
    fun restoreState(json: String) {
        checkOpen()
        nativeRestoreState(ptr, json)
    }

    override fun close() {
        if (ptr != 0L) {
            nativeFree(ptr)
            ptr = 0L
        }
    }

    private fun checkOpen() = check(ptr != 0L) { "Pager already closed" }

    companion object {
        init { NativeLoader.ensureLoaded() }

        @JvmStatic private external fun nativeNew(): Long
        @JvmStatic private external fun nativeFree(ptr: Long)
        @JvmStatic private external fun nativeNotify(ptr: Long, json: String): String?
        @JvmStatic private external fun nativeGetPending(ptr: Long, priorityJson: String?): String?
        @JvmStatic private external fun nativeGetUnread(ptr: Long): String?
        @JvmStatic private external fun nativeMarkRead(ptr: Long, uuidStr: String): Boolean
        @JvmStatic private external fun nativeDismiss(ptr: Long, uuidStr: String): Boolean
        @JvmStatic private external fun nativeBadgeCount(ptr: Long): Long
        @JvmStatic private external fun nativePruneExpired(ptr: Long)
        @JvmStatic private external fun nativeExportState(ptr: Long): String?
        @JvmStatic private external fun nativeRestoreState(ptr: Long, json: String)
    }
}

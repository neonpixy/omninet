import COmnideaFFI
import Foundation

/// Swift wrapper around the Rust Pager (notification queue).
public final class Pager: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_pager_new()!
    }

    deinit {
        divi_pager_free(ptr)
    }

    /// Push a notification from a JSON string. Returns the UUID or nil on error.
    public func notify(_ notificationJSON: String) -> UUID? {
        guard let uuidStr = divi_pager_notify(ptr, notificationJSON) else { return nil }
        defer { divi_free_string(uuidStr) }
        return UUID(uuidString: String(cString: uuidStr))
    }

    /// Get pending notifications as a JSON array.
    public func getPendingJSON(priority priorityJSON: String? = nil) -> String {
        guard let json = divi_pager_get_pending(ptr, priorityJSON) else { return "[]" }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get unread notifications as a JSON array.
    public func getUnreadJSON() -> String {
        guard let json = divi_pager_get_unread(ptr) else { return "[]" }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Mark a notification as read.
    public func markRead(_ id: UUID) -> Bool {
        divi_pager_mark_read(ptr, id.uuidString.lowercased())
    }

    /// Dismiss a notification.
    public func dismiss(_ id: UUID) -> Bool {
        divi_pager_dismiss(ptr, id.uuidString.lowercased())
    }

    /// Count of unread notifications.
    public var badgeCount: Int {
        Int(divi_pager_badge_count(ptr))
    }

    /// Prune expired notifications.
    public func pruneExpired() {
        divi_pager_prune_expired(ptr)
    }

    /// Export pager state as a JSON string.
    public func exportStateJSON() -> String {
        guard let json = divi_pager_export_state(ptr) else { return "[]" }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Restore pager state from a JSON string.
    public func restoreState(json: String) {
        divi_pager_restore_state(ptr, json)
    }
}

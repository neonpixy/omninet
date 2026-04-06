import Foundation
import Testing
@testable import OmnideaCore

@Suite("Equipment FFI Bridge")
struct EquipmentTests {

    // MARK: - Phone

    @Test func phoneCallRaw() throws {
        let phone = Phone()

        // Register a handler that doubles the input number.
        phone.registerRaw("test.double") { request in
            guard let n = try? JSONDecoder().decode(Int.self, from: request) else {
                return nil
            }
            return try? JSONEncoder().encode(n * 2)
        }

        let request = try JSONEncoder().encode(21)
        let response = try phone.callRaw("test.double", data: request)
        let result = try JSONDecoder().decode(Int.self, from: response)
        #expect(result == 42)
    }

    @Test func phoneHasHandler() {
        let phone = Phone()
        #expect(!phone.hasHandler("test.nope"))

        phone.registerRaw("test.echo") { $0 }
        #expect(phone.hasHandler("test.echo"))
    }

    @Test func phoneRegisteredCallIds() {
        let phone = Phone()
        phone.registerRaw("a.one") { $0 }
        phone.registerRaw("b.two") { $0 }

        let ids = phone.registeredCallIds().sorted()
        #expect(ids == ["a.one", "b.two"])
    }

    @Test func phoneUnregister() {
        let phone = Phone()
        phone.registerRaw("test.remove") { $0 }
        #expect(phone.hasHandler("test.remove"))

        phone.unregister("test.remove")
        #expect(!phone.hasHandler("test.remove"))
    }

    @Test func phoneCallNoHandler() {
        let phone = Phone()
        #expect(throws: OmnideaError.self) {
            _ = try phone.callRaw("nonexistent")
        }
    }

    // MARK: - Email

    @Test func emailSendAndSubscribe() throws {
        let email = Email()
        var received: Data?

        let subId = email.subscribeRaw("test.event") { data in
            received = data
        }
        #expect(subId != UUID())

        let payload = try JSONEncoder().encode("hello")
        email.sendRaw("test.event", data: payload)

        #expect(received == payload)
    }

    @Test func emailUnsubscribe() throws {
        let email = Email()
        var callCount = 0

        let subId = email.subscribeRaw("test.count") { _ in
            callCount += 1
        }

        email.sendRaw("test.count", data: Data([1]))
        #expect(callCount == 1)

        email.unsubscribe(subId)
        email.sendRaw("test.count", data: Data([2]))
        #expect(callCount == 1) // No change.
    }

    @Test func emailHasSubscribers() {
        let email = Email()
        #expect(!email.hasSubscribers("test.x"))

        let subId = email.subscribeRaw("test.x") { _ in }
        #expect(email.hasSubscribers("test.x"))

        email.unsubscribe(subId)
        #expect(!email.hasSubscribers("test.x"))
    }

    @Test func emailActiveIds() {
        let email = Email()
        _ = email.subscribeRaw("alpha") { _ in }
        _ = email.subscribeRaw("beta") { _ in }

        let ids = email.activeEmailIds().sorted()
        #expect(ids == ["alpha", "beta"])
    }

    // MARK: - Contacts

    @Test func contactsRegisterAndLookup() throws {
        let contacts = Contacts()

        let info = ModuleInfoDTO(id: "vault", name: "Vault", moduleType: "source", dependsOn: [])
        try contacts.register(info)

        let json = contacts.lookup("vault")
        #expect(json != nil)
        #expect(json!.contains("vault"))
    }

    @Test func contactsRegisteredModuleIds() throws {
        let contacts = Contacts()
        try contacts.register(ModuleInfoDTO(id: "a", name: "A", moduleType: "source", dependsOn: []))
        try contacts.register(ModuleInfoDTO(id: "b", name: "B", moduleType: "plugin", dependsOn: []))

        let ids = contacts.registeredModuleIds()
        #expect(ids.contains("a"))
        #expect(ids.contains("b"))
    }

    @Test func contactsUnregister() throws {
        let contacts = Contacts()
        try contacts.register(ModuleInfoDTO(id: "temp", name: "Temp", moduleType: "app", dependsOn: []))

        #expect(contacts.lookup("temp") != nil)
        try contacts.unregister("temp")
        #expect(contacts.lookup("temp") == nil)
    }

    // MARK: - Pager

    /// Build a full Notification JSON with all required fields.
    private func makeNotificationJSON(title: String, source: String = "vault") -> String {
        let id = UUID().uuidString.lowercased()
        let now = ISO8601DateFormatter().string(from: Date())
        return """
        {"id":"\(id)","title":"\(title)","body":null,"priority":"normal","delivery":"toast","source_module":"\(source)","created":"\(now)","expires":null,"read":false,"dismissed":false}
        """
    }

    @Test func pagerNotifyAndPending() throws {
        let pager = Pager()
        let json = makeNotificationJSON(title: "Test")

        let id = pager.notify(json)
        #expect(id != nil)

        let pending = pager.getPendingJSON()
        #expect(pending.contains("Test"))
    }

    @Test func pagerMarkReadAndBadge() throws {
        let pager = Pager()
        let json = makeNotificationJSON(title: "Badge Test")

        guard let id = pager.notify(json) else {
            Issue.record("Failed to create notification")
            return
        }

        #expect(pager.badgeCount == 1)
        #expect(pager.markRead(id))
        #expect(pager.badgeCount == 0)
    }

    @Test func pagerDismiss() throws {
        let pager = Pager()
        let json = makeNotificationJSON(title: "Dismiss Me")

        guard let id = pager.notify(json) else {
            Issue.record("Failed to create notification")
            return
        }

        #expect(pager.dismiss(id))
        let pending = pager.getPendingJSON()
        #expect(!pending.contains("Dismiss Me"))
    }
}

// MARK: - DTO for Contacts

private struct ModuleInfoDTO: Codable {
    let id: String
    let name: String
    let moduleType: String
    let dependsOn: [String]

    enum CodingKeys: String, CodingKey {
        case id, name
        case moduleType = "module_type"
        case dependsOn = "depends_on"
    }
}

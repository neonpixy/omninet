#include <divinity/divinity.hpp>

#include <algorithm>
#include <cassert>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>
#include <chrono>
#include <iomanip>
#include <sstream>

// Fully qualify divinity:: to avoid collision with C typedefs (Phone, Email, etc.).

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static int tests_passed = 0;
static int tests_failed = 0;

#define RUN_TEST(name) do {                                        \
    std::cout << "  " << #name << "... ";                          \
    try {                                                          \
        name();                                                    \
        std::cout << "PASS" << std::endl;                          \
        ++tests_passed;                                            \
    } catch (const std::exception& e) {                            \
        std::cout << "FAIL: " << e.what() << std::endl;            \
        ++tests_failed;                                            \
    }                                                              \
} while (0)

/// Encode an int as JSON bytes (e.g., 21 → "21").
static std::vector<uint8_t> json_encode_int(int n) {
    auto s = std::to_string(n);
    return std::vector<uint8_t>(s.begin(), s.end());
}

/// Decode JSON bytes as int.
static int json_decode_int(const std::vector<uint8_t>& data) {
    std::string s(data.begin(), data.end());
    return std::stoi(s);
}

/// Simple counter for generating unique pseudo-UUIDs in tests.
static int notification_counter = 0;

/// Build a full Notification JSON with all required fields.
static std::string make_notification_json(const std::string& title,
                                           const std::string& source = "vault") {
    // Generate a UUID-formatted string (not cryptographically random, just unique).
    ++notification_counter;
    char uuid_buf[37];
    std::snprintf(uuid_buf, sizeof(uuid_buf),
        "00000000-0000-4000-8000-%012x", notification_counter);

    // ISO 8601 timestamp.
    auto now = std::chrono::system_clock::now();
    auto tt = std::chrono::system_clock::to_time_t(now);
    std::ostringstream ts;
    ts << std::put_time(std::gmtime(&tt), "%FT%TZ");

    return "{\"id\":\"" + std::string(uuid_buf) +
           "\",\"title\":\"" + title +
           "\",\"body\":null,\"priority\":\"normal\",\"delivery\":\"toast\""
           ",\"source_module\":\"" + source +
           "\",\"created\":\"" + ts.str() +
           "\",\"expires\":null,\"read\":false,\"dismissed\":false}";
}

// ---------------------------------------------------------------------------
// Phone Tests
// ---------------------------------------------------------------------------

static void phone_call_raw() {
    divinity::Phone phone;

    // Register a handler that doubles the input number.
    phone.register_raw("test.double", [](const uint8_t* data, size_t len) {
        std::string s(reinterpret_cast<const char*>(data), len);
        int n = std::stoi(s);
        return json_encode_int(n * 2);
    });

    auto request = json_encode_int(21);
    auto response = phone.call_raw("test.double", request);
    assert(json_decode_int(response) == 42);
}

static void phone_has_handler() {
    divinity::Phone phone;
    assert(!phone.has_handler("test.nope"));

    phone.register_raw("test.echo", [](const uint8_t* data, size_t len) {
        return std::vector<uint8_t>(data, data + len);
    });
    assert(phone.has_handler("test.echo"));
}

static void phone_registered_call_ids() {
    divinity::Phone phone;
    phone.register_raw("a.one", [](const uint8_t* d, size_t l) {
        return std::vector<uint8_t>(d, d + l);
    });
    phone.register_raw("b.two", [](const uint8_t* d, size_t l) {
        return std::vector<uint8_t>(d, d + l);
    });

    auto ids = phone.registered_call_ids();
    std::sort(ids.begin(), ids.end());
    assert(ids.size() == 2);
    assert(ids[0] == "a.one");
    assert(ids[1] == "b.two");
}

static void phone_unregister() {
    divinity::Phone phone;
    phone.register_raw("test.remove", [](const uint8_t* d, size_t l) {
        return std::vector<uint8_t>(d, d + l);
    });
    assert(phone.has_handler("test.remove"));

    phone.unregister("test.remove");
    assert(!phone.has_handler("test.remove"));
}

static void phone_call_no_handler() {
    divinity::Phone phone;
    bool threw = false;
    try {
        phone.call_raw("nonexistent");
    } catch (const divinity::OmnideaError&) {
        threw = true;
    }
    assert(threw);
}

// ---------------------------------------------------------------------------
// Email Tests
// ---------------------------------------------------------------------------

static void email_send_and_subscribe() {
    divinity::Email email;
    std::vector<uint8_t> received;

    auto sub_id = email.subscribe_raw("test.event",
        [&received](const uint8_t* data, size_t len) {
            received.assign(data, data + len);
        });

    assert(!sub_id.empty());

    std::string payload = "\"hello\"";
    std::vector<uint8_t> data(payload.begin(), payload.end());
    email.send_raw("test.event", data);

    assert(received == data);
}

static void email_unsubscribe() {
    divinity::Email email;
    int call_count = 0;

    auto sub_id = email.subscribe_raw("test.count",
        [&call_count](const uint8_t*, size_t) {
            ++call_count;
        });

    email.send_raw("test.count", {1});
    assert(call_count == 1);

    email.unsubscribe(sub_id);
    email.send_raw("test.count", {2});
    assert(call_count == 1);  // No change.
}

static void email_has_subscribers() {
    divinity::Email email;
    assert(!email.has_subscribers("test.x"));

    auto sub_id = email.subscribe_raw("test.x",
        [](const uint8_t*, size_t) {});
    assert(email.has_subscribers("test.x"));

    email.unsubscribe(sub_id);
    assert(!email.has_subscribers("test.x"));
}

static void email_active_ids() {
    divinity::Email email;
    email.subscribe_raw("alpha", [](const uint8_t*, size_t) {});
    email.subscribe_raw("beta", [](const uint8_t*, size_t) {});

    auto ids = email.active_email_ids();
    std::sort(ids.begin(), ids.end());
    assert(ids.size() == 2);
    assert(ids[0] == "alpha");
    assert(ids[1] == "beta");
}

// ---------------------------------------------------------------------------
// Contacts Tests
// ---------------------------------------------------------------------------

static void contacts_register_and_lookup() {
    divinity::Contacts contacts;
    std::string json = R"({"id":"vault","name":"Vault","module_type":"source","depends_on":[]})";
    contacts.register_module(json);

    auto result = contacts.lookup("vault");
    assert(result.has_value());
    assert(result->find("vault") != std::string::npos);
}

static void contacts_registered_module_ids() {
    divinity::Contacts contacts;
    contacts.register_module(R"({"id":"a","name":"A","module_type":"source","depends_on":[]})");
    contacts.register_module(R"({"id":"b","name":"B","module_type":"plugin","depends_on":[]})");

    auto ids = contacts.registered_module_ids();
    assert(std::find(ids.begin(), ids.end(), "a") != ids.end());
    assert(std::find(ids.begin(), ids.end(), "b") != ids.end());
}

static void contacts_unregister() {
    divinity::Contacts contacts;
    contacts.register_module(R"({"id":"temp","name":"Temp","module_type":"app","depends_on":[]})");

    assert(contacts.lookup("temp").has_value());
    contacts.unregister("temp");
    assert(!contacts.lookup("temp").has_value());
}

// ---------------------------------------------------------------------------
// Pager Tests
// ---------------------------------------------------------------------------

static void pager_notify_and_pending() {
    divinity::Pager pager;
    auto json = make_notification_json("Test");

    auto id = pager.notify(json);
    assert(id.has_value());

    auto pending = pager.get_pending_json();
    assert(pending.find("Test") != std::string::npos);
}

static void pager_mark_read_and_badge() {
    divinity::Pager pager;
    auto json = make_notification_json("Badge Test");

    auto id = pager.notify(json);
    assert(id.has_value());

    assert(pager.badge_count() == 1);
    assert(pager.mark_read(*id));
    assert(pager.badge_count() == 0);
}

static void pager_dismiss() {
    divinity::Pager pager;
    auto json = make_notification_json("Dismiss Me");

    auto id = pager.notify(json);
    assert(id.has_value());

    assert(pager.dismiss(*id));
    auto pending = pager.get_pending_json();
    assert(pending.find("Dismiss Me") == std::string::npos);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

int main() {
    std::cout << "Equipment FFI Bridge (C++)" << std::endl;

    std::cout << "\nPhone:" << std::endl;
    RUN_TEST(phone_call_raw);
    RUN_TEST(phone_has_handler);
    RUN_TEST(phone_registered_call_ids);
    RUN_TEST(phone_unregister);
    RUN_TEST(phone_call_no_handler);

    std::cout << "\nEmail:" << std::endl;
    RUN_TEST(email_send_and_subscribe);
    RUN_TEST(email_unsubscribe);
    RUN_TEST(email_has_subscribers);
    RUN_TEST(email_active_ids);

    std::cout << "\nContacts:" << std::endl;
    RUN_TEST(contacts_register_and_lookup);
    RUN_TEST(contacts_registered_module_ids);
    RUN_TEST(contacts_unregister);

    std::cout << "\nPager:" << std::endl;
    RUN_TEST(pager_notify_and_pending);
    RUN_TEST(pager_mark_read_and_badge);
    RUN_TEST(pager_dismiss);

    std::cout << "\n" << tests_passed << " passed, "
              << tests_failed << " failed." << std::endl;

    return tests_failed > 0 ? 1 : 0;
}

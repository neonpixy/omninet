use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use equipment::{
    CallDescriptor, Contacts, EdgeType, Email, EmailEvent, EventDescriptor, ModuleCatalog,
    ModuleInfo, ModuleType, Notification, NotificationPriority, Pager, Phone, PhoneCall,
};

// --- Test call/event types ---

#[derive(Serialize, Deserialize)]
struct GreetCall {
    name: String,
}

impl PhoneCall for GreetCall {
    const CALL_ID: &'static str = "oracle.greet";
    type Response = String;
}

#[derive(Serialize, Deserialize)]
struct StoreCall {
    key: String,
    value: String,
}

impl PhoneCall for StoreCall {
    const CALL_ID: &'static str = "vault.store";
    type Response = bool;
}

#[derive(Serialize, Deserialize)]
struct ModuleStartedEvent {
    module_id: String,
}

impl EmailEvent for ModuleStartedEvent {
    const EMAIL_ID: &'static str = "contacts.moduleStarted";
}

// --- Integration tests ---

#[test]
fn full_module_lifecycle() {
    let phone = Phone::new();
    let email = Email::new();
    let contacts = Contacts::new();

    // Register a module.
    contacts
        .register(ModuleInfo::new("oracle", "Oracle", ModuleType::Source))
        .unwrap();

    // Module registers a Phone handler on "startup".
    phone.register(|call: GreetCall| Ok(format!("Hello, {}!", call.name)));

    // Another module subscribes to an Email event.
    let started_modules = Arc::new(Mutex::new(Vec::new()));
    let sm_clone = started_modules.clone();
    email.subscribe(move |event: ModuleStartedEvent| {
        sm_clone.lock().unwrap().push(event.module_id);
    });

    // Broadcast that oracle started.
    email.send(&ModuleStartedEvent {
        module_id: "oracle".to_string(),
    });

    // Another module calls oracle via Phone.
    let greeting = phone
        .call(&GreetCall {
            name: "Sam".to_string(),
        })
        .unwrap();
    assert_eq!(greeting, "Hello, Sam!");

    // Verify the email was received.
    assert_eq!(*started_modules.lock().unwrap(), vec!["oracle"]);

    // Shutdown.
    contacts.shutdown_all();
    assert!(contacts.all_modules().is_empty());
}

#[test]
fn phone_cross_module_communication() {
    let phone = Arc::new(Phone::new());

    // Module A: Oracle
    phone.register(|call: GreetCall| Ok(format!("Hello, {}!", call.name)));

    // Module B: Vault — its handler calls Oracle via Phone (reentrant).
    let phone_clone = phone.clone();
    phone.register(move |call: StoreCall| {
        // Vault calls Oracle as part of its handler.
        let greeting = phone_clone
            .call(&GreetCall {
                name: call.key.clone(),
            })
            .unwrap();
        Ok(!greeting.is_empty())
    });

    // External caller invokes Vault, which internally calls Oracle.
    let stored = phone
        .call(&StoreCall {
            key: "test".to_string(),
            value: "data".to_string(),
        })
        .unwrap();
    assert!(stored);
}

#[test]
fn email_broadcast_to_multiple_modules() {
    let email = Email::new();
    let total = Arc::new(AtomicI64::new(0));

    // Three "modules" subscribe to the same event.
    for _ in 0..3 {
        let total_clone = total.clone();
        email.subscribe(move |_: ModuleStartedEvent| {
            total_clone.fetch_add(1, Ordering::SeqCst);
        });
    }

    // One broadcast reaches all three.
    email.send(&ModuleStartedEvent {
        module_id: "crown".to_string(),
    });
    assert_eq!(total.load(Ordering::SeqCst), 3);
}

#[test]
fn pager_notification_lifecycle() {
    let pager = Pager::new();

    // Queue notifications.
    let id1 = pager.notify(Notification::new("System update", "vault"));
    let id2 = pager.notify(
        Notification::new("Security alert", "sentinal")
            .with_priority(NotificationPriority::Urgent),
    );
    let _id3 = pager.notify(Notification::new("Welcome", "oracle"));

    assert_eq!(pager.badge_count(), 3);

    // Mark one as read.
    pager.mark_read(id1);
    assert_eq!(pager.badge_count(), 2);
    assert_eq!(pager.get_unread().len(), 2);

    // Dismiss one.
    pager.dismiss(id2);
    assert_eq!(pager.badge_count(), 1);
    assert_eq!(pager.get_pending(None).len(), 2); // read but not dismissed still pending

    // Export state.
    let state = pager.export_state();
    assert_eq!(state.notifications.len(), 3);

    // Restore into fresh Pager.
    let pager2 = Pager::new();
    pager2.restore_state(state);
    assert_eq!(pager2.badge_count(), 1);
    assert_eq!(pager2.get_pending(None).len(), 2);
}

#[test]
fn phone_and_email_independent() {
    let phone = Phone::new();
    let email = Email::new();

    // Register a Phone handler.
    phone.register(|call: GreetCall| Ok(format!("Hi, {}", call.name)));

    // Email should know nothing about Phone handlers.
    assert!(!email.has_subscribers("oracle.greet"));
    assert!(email.active_email_ids().is_empty());

    // Subscribe to an Email event.
    email.subscribe(move |_: ModuleStartedEvent| {});

    // Phone should know nothing about Email subscribers.
    assert!(!phone.has_handler("contacts.moduleStarted"));
}

#[test]
fn contacts_shutdown_with_phone_cleanup() {
    let phone = Arc::new(Phone::new());
    let contacts = Contacts::new();

    // Module registers a Phone handler on startup, unregisters on shutdown.
    phone.register(|call: GreetCall| Ok(format!("Hello, {}", call.name)));
    assert!(phone.has_handler("oracle.greet"));

    let phone_clone = phone.clone();
    contacts
        .register_with_shutdown(
            ModuleInfo::new("oracle", "Oracle", ModuleType::Source),
            move || {
                phone_clone.unregister("oracle.greet");
            },
        )
        .unwrap();

    // Shutdown all — the callback should unregister the Phone handler.
    contacts.shutdown_all();
    assert!(!phone.has_handler("oracle.greet"));
}

#[test]
fn contacts_dependency_shutdown_with_callbacks() {
    let phone = Arc::new(Phone::new());
    let contacts = Contacts::new();
    let shutdown_order = Arc::new(Mutex::new(Vec::new()));

    // Sentinal has no deps, registers a Phone handler.
    phone.register_raw("sentinal.status", |_| Ok(b"ok".to_vec()));

    let so = shutdown_order.clone();
    let pc = phone.clone();
    contacts
        .register_with_shutdown(
            ModuleInfo::new("sentinal", "Sentinal", ModuleType::Source),
            move || {
                pc.unregister("sentinal.status");
                so.lock().unwrap().push("sentinal".to_string());
            },
        )
        .unwrap();

    // Vault depends on sentinal.
    let so = shutdown_order.clone();
    contacts
        .register_with_shutdown(
            ModuleInfo::new("vault", "Vault", ModuleType::Source)
                .with_dependencies(vec!["sentinal".to_string()]),
            move || {
                so.lock().unwrap().push("vault".to_string());
            },
        )
        .unwrap();

    // Verify both exist.
    assert!(contacts.lookup("sentinal").is_some());
    assert!(contacts.lookup("vault").is_some());
    assert!(phone.has_handler("sentinal.status"));

    // Shutdown all.
    contacts.shutdown_all();

    // Vault (dependent) shut down before sentinal (dependency).
    let order = shutdown_order.lock().unwrap();
    assert_eq!(*order, vec!["vault", "sentinal"]);

    // Phone handler cleaned up.
    assert!(!phone.has_handler("sentinal.status"));
}

// --- Catalog integration tests ---

#[test]
fn catalog_lifecycle_with_contacts() {
    let contacts = Contacts::new();

    // Register with catalog.
    contacts
        .register(
            ModuleInfo::new("crown", "Crown", ModuleType::Source).with_catalog(
                ModuleCatalog::new()
                    .with_call(CallDescriptor::new("crown.getProfile", "Get profile"))
                    .with_emitted_event(EventDescriptor::new(
                        "crown.profileChanged",
                        "Profile updated",
                    )),
            ),
        )
        .unwrap();

    // Query works.
    assert_eq!(contacts.who_handles("crown.getProfile"), Some("crown".to_string()));
    assert_eq!(contacts.all_calls().len(), 1);

    // Shutdown removes everything.
    contacts.shutdown_all();
    assert!(contacts.who_handles("crown.getProfile").is_none());
    assert!(contacts.all_calls().is_empty());
}

#[test]
fn catalog_phone_consistency() {
    let phone = Phone::new();
    let contacts = Contacts::new();

    // Register catalog claiming "vault.lock" is handled.
    contacts
        .register(
            ModuleInfo::new("vault", "Vault", ModuleType::Source).with_catalog(
                ModuleCatalog::new()
                    .with_call(CallDescriptor::new("vault.lock", "Lock the vault")),
            ),
        )
        .unwrap();

    // Register actual Phone handler.
    phone.register(|_call: StoreCall| Ok(true));
    phone.register_raw("vault.lock", |_| Ok(b"true".to_vec()));

    // Catalog says vault handles "vault.lock".
    assert_eq!(contacts.who_handles("vault.lock"), Some("vault".to_string()));
    // Phone confirms it has the handler.
    assert!(phone.has_handler("vault.lock"));
}

#[test]
fn catalog_multi_module_topology() {
    let contacts = Contacts::new();

    contacts
        .register(
            ModuleInfo::new("crown", "Crown", ModuleType::Source).with_catalog(
                ModuleCatalog::new()
                    .with_call(CallDescriptor::new("crown.getProfile", "Get profile"))
                    .with_emitted_event(EventDescriptor::new(
                        "crown.profileChanged",
                        "Profile updated",
                    )),
            ),
        )
        .unwrap();

    contacts
        .register(
            ModuleInfo::new("vault", "Vault", ModuleType::Source).with_catalog(
                ModuleCatalog::new()
                    .with_call(CallDescriptor::new("vault.lock", "Lock vault"))
                    .with_subscribed_event(EventDescriptor::new(
                        "crown.profileChanged",
                        "Re-key on identity change",
                    )),
            ),
        )
        .unwrap();

    contacts
        .register(
            ModuleInfo::new("advisor", "Advisor", ModuleType::Source).with_catalog(
                ModuleCatalog::new()
                    .with_call(CallDescriptor::new("advisor.tick", "Tick cognitive loop"))
                    .with_subscribed_event(EventDescriptor::new(
                        "crown.profileChanged",
                        "Update user context",
                    )),
            ),
        )
        .unwrap();

    let topo = contacts.topology();

    // Event edges: Crown emits profileChanged, both Vault and Advisor subscribe.
    let event_edges: Vec<_> = topo
        .edges
        .iter()
        .filter(|e| e.edge_type == EdgeType::Event)
        .collect();
    assert_eq!(event_edges.len(), 2);
    assert!(event_edges.iter().any(|e| e.from_module == "crown" && e.to_module == "vault"));
    assert!(event_edges.iter().any(|e| e.from_module == "crown" && e.to_module == "advisor"));

    // Call edges: 3 modules × 1 call each = 3 call edges.
    let call_edges: Vec<_> = topo
        .edges
        .iter()
        .filter(|e| e.edge_type == EdgeType::Call)
        .collect();
    assert_eq!(call_edges.len(), 3);
}

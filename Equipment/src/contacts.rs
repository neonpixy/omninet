use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::catalog::{CallDescriptor, EventDescriptor, EdgeType, MessageEdge, MessageTopology, ModuleCatalog};
use crate::error::ContactsError;
use crate::federation_scope::FederationScope;

/// What kind of module this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModuleType {
    /// A core system module (e.g., Sentinal, Crown, Vault).
    Source,
    /// A third-party plugin extending the system.
    Plugin,
    /// A user-facing application or program.
    App,
}

/// Information about a registered module.
///
/// All modules are sovereign peers. They can declare dependencies
/// ("I need Sentinal running") but no module owns another.
///
/// Modules may optionally be associated with a community via `community_id`.
/// System-level modules (Sentinal, Crown, etc.) have no community -- they
/// are always visible regardless of federation scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleInfo {
    id: String,
    name: String,
    module_type: ModuleType,
    depends_on: Vec<String>,
    catalog: ModuleCatalog,
    /// Optional community this module belongs to. `None` for system-level modules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    community_id: Option<String>,
}

impl ModuleInfo {
    /// Create a module.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        module_type: ModuleType,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            module_type,
            depends_on: Vec::new(),
            catalog: ModuleCatalog::new(),
            community_id: None,
        }
    }

    /// Declare dependencies on other modules.
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.depends_on = deps;
        self
    }

    /// Associate this module with a community.
    ///
    /// Modules with a community_id are subject to federation scoping --
    /// they become invisible when their community is defederated.
    /// System-level modules should not set this.
    pub fn with_community(mut self, community_id: impl Into<String>) -> Self {
        self.community_id = Some(community_id.into());
        self
    }

    /// The unique identifier for this module.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The human-readable name of this module.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What kind of module this is (Source, Plugin, or App).
    pub fn module_type(&self) -> ModuleType {
        self.module_type
    }

    /// IDs of modules this one depends on.
    pub fn depends_on(&self) -> &[String] {
        &self.depends_on
    }

    /// The community this module belongs to, if any.
    pub fn community_id(&self) -> Option<&str> {
        self.community_id.as_deref()
    }

    /// Attach a catalog describing this module's message-passing capabilities.
    pub fn with_catalog(mut self, catalog: ModuleCatalog) -> Self {
        self.catalog = catalog;
        self
    }

    /// This module's catalog.
    pub fn catalog(&self) -> &ModuleCatalog {
        &self.catalog
    }
}

/// Shutdown callback — called once when a module is unregistered or shut down.
type ShutdownCallback = Box<dyn FnOnce() + Send>;

/// Communal registry of sovereign modules.
///
/// Every module is a peer. Modules declare dependencies ("I need X to be running"),
/// and shutdown respects the dependency graph — dependents shut down before their
/// dependencies. No module owns another.
pub struct Contacts {
    inner: Mutex<ContactsInner>,
}

struct ContactsInner {
    modules: HashMap<String, ModuleInfo>,
    registration_order: Vec<String>,
    shutdown_callbacks: HashMap<String, ShutdownCallback>,
}

impl Contacts {
    /// Create an empty module registry.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ContactsInner {
                modules: HashMap::new(),
                registration_order: Vec::new(),
                shutdown_callbacks: HashMap::new(),
            }),
        }
    }

    /// Register a module. Dependencies must already be registered.
    pub fn register(&self, info: ModuleInfo) -> Result<(), ContactsError> {
        let mut inner = self.inner.lock().expect("contacts mutex poisoned");

        if inner.modules.contains_key(&info.id) {
            return Err(ContactsError::AlreadyRegistered(info.id.clone()));
        }

        for dep in &info.depends_on {
            if !inner.modules.contains_key(dep) {
                return Err(ContactsError::DependencyNotFound(dep.clone()));
            }
        }

        inner.registration_order.push(info.id.clone());
        inner.modules.insert(info.id.clone(), info);
        Ok(())
    }

    /// Register a module with a shutdown callback.
    pub fn register_with_shutdown(
        &self,
        info: ModuleInfo,
        on_shutdown: impl FnOnce() + Send + 'static,
    ) -> Result<(), ContactsError> {
        let id = info.id.clone();
        self.register(info)?;
        self.inner
            .lock()
            .expect("contacts mutex poisoned")
            .shutdown_callbacks
            .insert(id, Box::new(on_shutdown));
        Ok(())
    }

    /// Unregister a module. Dependents are shut down first (reverse dependency order).
    pub fn unregister(&self, module_id: &str) -> Result<(), ContactsError> {
        let dependents = {
            let inner = self.inner.lock().expect("contacts mutex poisoned");
            if !inner.modules.contains_key(module_id) {
                return Err(ContactsError::NotFound(module_id.to_string()));
            }
            self.transitive_dependents(&inner, module_id)
        };

        // Shut down dependents first (deepest dependents first).
        for dep_id in &dependents {
            self.remove_single(dep_id);
        }

        // Shut down the module itself.
        self.remove_single(module_id);
        Ok(())
    }

    /// Shutdown all modules respecting the dependency graph.
    ///
    /// Dependents always shut down before their dependencies. Callbacks are consumed.
    pub fn shutdown_all(&self) {
        let order = {
            let inner = self.inner.lock().expect("contacts mutex poisoned");
            self.topological_shutdown_order(&inner)
        };

        for module_id in &order {
            self.remove_single(module_id);
        }
    }

    /// Look up a module by ID.
    pub fn lookup(&self, module_id: &str) -> Option<ModuleInfo> {
        self.inner.lock().expect("contacts mutex poisoned").modules.get(module_id).cloned()
    }

    /// All registered module IDs in registration order.
    pub fn registered_module_ids(&self) -> Vec<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .registration_order
            .iter()
            .filter(|id| inner.modules.contains_key(*id))
            .cloned()
            .collect()
    }

    /// All modules in registration order.
    pub fn all_modules(&self) -> Vec<ModuleInfo> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .registration_order
            .iter()
            .filter_map(|id| inner.modules.get(id).cloned())
            .collect()
    }

    /// All modules that depend on the given module (directly or transitively).
    pub fn dependents_of(&self, module_id: &str) -> Vec<ModuleInfo> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let dep_ids = self.transitive_dependents(&inner, module_id);
        dep_ids
            .iter()
            .filter_map(|id| inner.modules.get(id).cloned())
            .collect()
    }

    // ── Catalog queries ──────────────────────────────────────────────

    /// Replace a module's catalog at runtime. The module must already be registered.
    pub fn update_catalog(
        &self,
        module_id: &str,
        catalog: ModuleCatalog,
    ) -> Result<(), ContactsError> {
        let mut inner = self.inner.lock().expect("contacts mutex poisoned");
        let info = inner
            .modules
            .get_mut(module_id)
            .ok_or_else(|| ContactsError::NotFound(module_id.to_string()))?;
        info.catalog = catalog;
        Ok(())
    }

    /// Phone calls this module handles. Empty if unknown module.
    pub fn calls_for(&self, module_id: &str) -> Vec<CallDescriptor> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .get(module_id)
            .map(|m| m.catalog.calls_handled().to_vec())
            .unwrap_or_default()
    }

    /// Email events this module emits. Empty if unknown module.
    pub fn events_emitted_by(&self, module_id: &str) -> Vec<EventDescriptor> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .get(module_id)
            .map(|m| m.catalog.events_emitted().to_vec())
            .unwrap_or_default()
    }

    /// Email events this module subscribes to. Empty if unknown module.
    pub fn events_subscribed_by(&self, module_id: &str) -> Vec<EventDescriptor> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .get(module_id)
            .map(|m| m.catalog.events_subscribed().to_vec())
            .unwrap_or_default()
    }

    /// Which module handles this call ID? At most one (Phone is single-handler).
    pub fn who_handles(&self, call_id: &str) -> Option<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        for (id, info) in &inner.modules {
            if info.catalog.calls_handled().iter().any(|c| c.call_id() == call_id) {
                return Some(id.clone());
            }
        }
        None
    }

    /// Which modules emit this email event?
    pub fn who_emits(&self, email_id: &str) -> Vec<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .iter()
            .filter(|(_, info)| {
                info.catalog
                    .events_emitted()
                    .iter()
                    .any(|e| e.email_id() == email_id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Which modules subscribe to this email event?
    pub fn who_subscribes(&self, email_id: &str) -> Vec<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .iter()
            .filter(|(_, info)| {
                info.catalog
                    .events_subscribed()
                    .iter()
                    .any(|e| e.email_id() == email_id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// All registered Phone calls across all modules: (module_id, descriptor).
    pub fn all_calls(&self) -> Vec<(String, CallDescriptor)> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let mut result = Vec::new();
        for id in &inner.registration_order {
            if let Some(info) = inner.modules.get(id) {
                for call in info.catalog.calls_handled() {
                    result.push((id.clone(), call.clone()));
                }
            }
        }
        result
    }

    /// All emitted Email events across all modules: (module_id, descriptor).
    pub fn all_events(&self) -> Vec<(String, EventDescriptor)> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let mut result = Vec::new();
        for id in &inner.registration_order {
            if let Some(info) = inner.modules.get(id) {
                for event in info.catalog.events_emitted() {
                    result.push((id.clone(), event.clone()));
                }
            }
        }
        result
    }

    /// Compute the full message-passing topology from registered catalogs.
    ///
    /// Event edges: emitter → subscriber (complete graph).
    /// Call edges: "" → handler (caller unknown, handler known).
    pub fn topology(&self) -> MessageTopology {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let mut edges = Vec::new();

        let modules: Vec<(&String, &ModuleCatalog)> = inner
            .modules
            .iter()
            .map(|(id, info)| (id, &info.catalog))
            .collect();

        // Event edges: for each emitted event, find all subscribers.
        for (emitter_id, emitter_catalog) in &modules {
            for emitted in emitter_catalog.events_emitted() {
                for (subscriber_id, subscriber_catalog) in &modules {
                    if subscriber_catalog
                        .events_subscribed()
                        .iter()
                        .any(|s| s.email_id() == emitted.email_id())
                    {
                        edges.push(MessageEdge {
                            from_module: (*emitter_id).clone(),
                            to_module: (*subscriber_id).clone(),
                            message_id: emitted.email_id().to_string(),
                            edge_type: EdgeType::Event,
                        });
                    }
                }
            }
        }

        // Call edges: handler side only (caller unknown).
        for (handler_id, handler_catalog) in &modules {
            for call in handler_catalog.calls_handled() {
                edges.push(MessageEdge {
                    from_module: String::new(),
                    to_module: (*handler_id).clone(),
                    message_id: call.call_id().to_string(),
                    edge_type: EdgeType::Call,
                });
            }
        }

        MessageTopology { edges }
    }

    // ── Federation-scoped queries ─────────────────────────────────

    /// All modules visible under the given federation scope, in registration order.
    ///
    /// Modules without a `community_id` are always visible (system-level).
    /// Modules with a `community_id` are only visible if that community is
    /// in the scope.
    pub fn all_modules_scoped(&self, scope: &FederationScope) -> Vec<ModuleInfo> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .registration_order
            .iter()
            .filter_map(|id| inner.modules.get(id))
            .filter(|m| scope.is_visible_opt(m.community_id.as_deref()))
            .cloned()
            .collect()
    }

    /// Look up a module by ID, subject to federation scope.
    ///
    /// Returns `None` if the module doesn't exist or belongs to a
    /// defederated community.
    pub fn lookup_scoped(&self, module_id: &str, scope: &FederationScope) -> Option<ModuleInfo> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .get(module_id)
            .filter(|m| scope.is_visible_opt(m.community_id.as_deref()))
            .cloned()
    }

    /// Which module handles this call ID, subject to federation scope?
    ///
    /// Returns `None` if no visible module handles the call.
    pub fn who_handles_scoped(&self, call_id: &str, scope: &FederationScope) -> Option<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        for (id, info) in &inner.modules {
            if scope.is_visible_opt(info.community_id.as_deref())
                && info.catalog.calls_handled().iter().any(|c| c.call_id() == call_id)
            {
                return Some(id.clone());
            }
        }
        None
    }

    /// Which visible modules emit this email event?
    pub fn who_emits_scoped(&self, email_id: &str, scope: &FederationScope) -> Vec<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .iter()
            .filter(|(_, info)| {
                scope.is_visible_opt(info.community_id.as_deref())
                    && info
                        .catalog
                        .events_emitted()
                        .iter()
                        .any(|e| e.email_id() == email_id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Which visible modules subscribe to this email event?
    pub fn who_subscribes_scoped(&self, email_id: &str, scope: &FederationScope) -> Vec<String> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .iter()
            .filter(|(_, info)| {
                scope.is_visible_opt(info.community_id.as_deref())
                    && info
                        .catalog
                        .events_subscribed()
                        .iter()
                        .any(|e| e.email_id() == email_id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// All registered Phone calls across visible modules: (module_id, descriptor).
    pub fn all_calls_scoped(&self, scope: &FederationScope) -> Vec<(String, CallDescriptor)> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let mut result = Vec::new();
        for id in &inner.registration_order {
            if let Some(info) = inner.modules.get(id)
                && scope.is_visible_opt(info.community_id.as_deref())
            {
                for call in info.catalog.calls_handled() {
                    result.push((id.clone(), call.clone()));
                }
            }
        }
        result
    }

    /// All emitted Email events across visible modules: (module_id, descriptor).
    pub fn all_events_scoped(&self, scope: &FederationScope) -> Vec<(String, EventDescriptor)> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let mut result = Vec::new();
        for id in &inner.registration_order {
            if let Some(info) = inner.modules.get(id)
                && scope.is_visible_opt(info.community_id.as_deref())
            {
                for event in info.catalog.events_emitted() {
                    result.push((id.clone(), event.clone()));
                }
            }
        }
        result
    }

    /// Compute the message-passing topology from visible modules only.
    ///
    /// Same as `topology()` but filters out modules from defederated communities.
    pub fn topology_scoped(&self, scope: &FederationScope) -> MessageTopology {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        let mut edges = Vec::new();

        let modules: Vec<(&String, &ModuleCatalog)> = inner
            .modules
            .iter()
            .filter(|(_, info)| scope.is_visible_opt(info.community_id.as_deref()))
            .map(|(id, info)| (id, &info.catalog))
            .collect();

        // Event edges: for each emitted event, find all subscribers.
        for (emitter_id, emitter_catalog) in &modules {
            for emitted in emitter_catalog.events_emitted() {
                for (subscriber_id, subscriber_catalog) in &modules {
                    if subscriber_catalog
                        .events_subscribed()
                        .iter()
                        .any(|s| s.email_id() == emitted.email_id())
                    {
                        edges.push(MessageEdge {
                            from_module: (*emitter_id).clone(),
                            to_module: (*subscriber_id).clone(),
                            message_id: emitted.email_id().to_string(),
                            edge_type: EdgeType::Event,
                        });
                    }
                }
            }
        }

        // Call edges: handler side only (caller unknown).
        for (handler_id, handler_catalog) in &modules {
            for call in handler_catalog.calls_handled() {
                edges.push(MessageEdge {
                    from_module: String::new(),
                    to_module: (*handler_id).clone(),
                    message_id: call.call_id().to_string(),
                    edge_type: EdgeType::Call,
                });
            }
        }

        MessageTopology { edges }
    }

    // ── Dependency queries ──────────────────────────────────────────

    /// Modules that the given module directly depends on.
    pub fn dependencies_of(&self, module_id: &str) -> Vec<ModuleInfo> {
        let inner = self.inner.lock().expect("contacts mutex poisoned");
        inner
            .modules
            .get(module_id)
            .map(|info| {
                info.depends_on
                    .iter()
                    .filter_map(|dep| inner.modules.get(dep).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all transitive dependents in reverse dependency order
    /// (deepest dependents first, so they shut down before shallower ones).
    fn transitive_dependents(&self, inner: &ContactsInner, module_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        self.collect_dependents(inner, module_id, &mut result, &mut visited);
        result.reverse();
        result
    }

    /// Recursively collect modules that depend on the given module.
    fn collect_dependents(
        &self,
        inner: &ContactsInner,
        module_id: &str,
        result: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) {
        for (id, info) in &inner.modules {
            if !visited.contains(id) && info.depends_on.iter().any(|d| d == module_id) {
                visited.insert(id.clone());
                self.collect_dependents(inner, id, result, visited);
                result.push(id.clone());
            }
        }
    }

    /// Topological sort for shutdown: dependents before dependencies.
    /// Within the same dependency level, reverse registration order.
    fn topological_shutdown_order(&self, inner: &ContactsInner) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();

        // Process in reverse registration order so that later-registered
        // modules at the same level shut down first.
        for id in inner.registration_order.iter().rev() {
            if !visited.contains(id) {
                self.topo_visit(inner, id, &mut result, &mut visited);
            }
        }

        result
    }

    /// DFS visit for topological sort. Dependents are emitted before dependencies.
    fn topo_visit(
        &self,
        inner: &ContactsInner,
        module_id: &str,
        result: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(module_id) {
            return;
        }
        visited.insert(module_id.to_string());

        // Visit all modules that depend on this one first (dependents before deps).
        for (id, info) in &inner.modules {
            if !visited.contains(id) && info.depends_on.iter().any(|d| d == module_id) {
                self.topo_visit(inner, id, result, visited);
            }
        }

        result.push(module_id.to_string());
    }

    /// Remove a single module and call its shutdown callback.
    fn remove_single(&self, module_id: &str) {
        let callback = {
            let mut inner = self.inner.lock().expect("contacts mutex poisoned");
            inner.modules.remove(module_id);
            inner.registration_order.retain(|id| id != module_id);
            inner.shutdown_callbacks.remove(module_id)
        };

        if let Some(cb) = callback {
            cb();
        }
    }
}

impl Default for Contacts {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[test]
    fn register_and_lookup() {
        let contacts = Contacts::new();
        let info = ModuleInfo::new("vault", "Vault", ModuleType::Source);
        contacts.register(info).unwrap();

        let found = contacts.lookup("vault").unwrap();
        assert_eq!(found.id(), "vault");
        assert_eq!(found.name(), "Vault");
        assert_eq!(found.module_type(), ModuleType::Source);
        assert!(found.depends_on().is_empty());
    }

    #[test]
    fn register_duplicate_rejected() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("vault", "Vault", ModuleType::Source))
            .unwrap();

        let result = contacts.register(ModuleInfo::new("vault", "Vault 2", ModuleType::Source));
        assert!(matches!(result, Err(ContactsError::AlreadyRegistered(_))));
    }

    #[test]
    fn register_with_dependencies() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("sentinal", "Sentinal", ModuleType::Source))
            .unwrap();

        let vault = ModuleInfo::new("vault", "Vault", ModuleType::Source)
            .with_dependencies(vec!["sentinal".to_string()]);
        contacts.register(vault).unwrap();

        let found = contacts.lookup("vault").unwrap();
        assert_eq!(found.depends_on(), &["sentinal".to_string()]);
    }

    #[test]
    fn register_dependency_not_found() {
        let contacts = Contacts::new();
        let vault = ModuleInfo::new("vault", "Vault", ModuleType::Source)
            .with_dependencies(vec!["sentinal".to_string()]);

        let result = contacts.register(vault);
        assert!(matches!(result, Err(ContactsError::DependencyNotFound(_))));
    }

    #[test]
    fn unregister_removes_module() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("vault", "Vault", ModuleType::Source))
            .unwrap();

        contacts.unregister("vault").unwrap();
        assert!(contacts.lookup("vault").is_none());
    }

    #[test]
    fn unregister_not_found() {
        let contacts = Contacts::new();
        let result = contacts.unregister("nonexistent");
        assert!(matches!(result, Err(ContactsError::NotFound(_))));
    }

    #[test]
    fn unregister_shuts_down_dependents() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("sentinal", "Sentinal", ModuleType::Source))
            .unwrap();
        contacts
            .register(
                ModuleInfo::new("vault", "Vault", ModuleType::Source)
                    .with_dependencies(vec!["sentinal".to_string()]),
            )
            .unwrap();
        contacts
            .register(
                ModuleInfo::new("hall", "Hall", ModuleType::Source)
                    .with_dependencies(vec!["sentinal".to_string()]),
            )
            .unwrap();

        // Shutting down sentinal should take vault and hall with it.
        contacts.unregister("sentinal").unwrap();

        assert!(contacts.lookup("sentinal").is_none());
        assert!(contacts.lookup("vault").is_none());
        assert!(contacts.lookup("hall").is_none());
    }

    #[test]
    fn shutdown_callback_called() {
        let contacts = Contacts::new();
        let called = Arc::new(StdMutex::new(false));

        let called_clone = called.clone();
        contacts
            .register_with_shutdown(
                ModuleInfo::new("vault", "Vault", ModuleType::Source),
                move || {
                    *called_clone.lock().unwrap() = true;
                },
            )
            .unwrap();

        contacts.unregister("vault").unwrap();
        assert!(*called.lock().unwrap());
    }

    #[test]
    fn shutdown_all_reverse_order() {
        let contacts = Contacts::new();
        let order = Arc::new(StdMutex::new(Vec::new()));

        // Three independent modules — should shut down in reverse registration order.
        for name in ["alpha", "beta", "gamma"] {
            let order_clone = order.clone();
            let name_owned = name.to_string();
            contacts
                .register_with_shutdown(
                    ModuleInfo::new(name, name, ModuleType::Source),
                    move || {
                        order_clone.lock().unwrap().push(name_owned);
                    },
                )
                .unwrap();
        }

        contacts.shutdown_all();
        assert_eq!(*order.lock().unwrap(), vec!["gamma", "beta", "alpha"]);
    }

    #[test]
    fn shutdown_all_dependents_before_dependencies() {
        let contacts = Contacts::new();
        let order = Arc::new(StdMutex::new(Vec::new()));

        // sentinal has no deps, vault depends on sentinal, hall depends on vault.
        let o = order.clone();
        contacts
            .register_with_shutdown(
                ModuleInfo::new("sentinal", "Sentinal", ModuleType::Source),
                move || o.lock().unwrap().push("sentinal".to_string()),
            )
            .unwrap();

        let o = order.clone();
        contacts
            .register_with_shutdown(
                ModuleInfo::new("vault", "Vault", ModuleType::Source)
                    .with_dependencies(vec!["sentinal".to_string()]),
                move || o.lock().unwrap().push("vault".to_string()),
            )
            .unwrap();

        let o = order.clone();
        contacts
            .register_with_shutdown(
                ModuleInfo::new("hall", "Hall", ModuleType::Source)
                    .with_dependencies(vec!["vault".to_string()]),
                move || o.lock().unwrap().push("hall".to_string()),
            )
            .unwrap();

        contacts.shutdown_all();

        let shutdown_order = order.lock().unwrap();
        // hall depends on vault depends on sentinal.
        // So: hall first, then vault, then sentinal.
        let sentinal_pos = shutdown_order
            .iter()
            .position(|s| s == "sentinal")
            .unwrap();
        let vault_pos = shutdown_order.iter().position(|s| s == "vault").unwrap();
        let hall_pos = shutdown_order.iter().position(|s| s == "hall").unwrap();
        assert!(hall_pos < vault_pos);
        assert!(vault_pos < sentinal_pos);
    }

    #[test]
    fn registered_module_ids_in_order() {
        let contacts = Contacts::new();
        for name in ["alpha", "beta", "gamma"] {
            contacts
                .register(ModuleInfo::new(name, name, ModuleType::Source))
                .unwrap();
        }

        assert_eq!(
            contacts.registered_module_ids(),
            vec!["alpha", "beta", "gamma"]
        );
    }

    #[test]
    fn dependents_of() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("sentinal", "Sentinal", ModuleType::Source))
            .unwrap();
        contacts
            .register(
                ModuleInfo::new("vault", "Vault", ModuleType::Source)
                    .with_dependencies(vec!["sentinal".to_string()]),
            )
            .unwrap();
        contacts
            .register(
                ModuleInfo::new("hall", "Hall", ModuleType::Source)
                    .with_dependencies(vec!["sentinal".to_string()]),
            )
            .unwrap();
        contacts
            .register(ModuleInfo::new("crown", "Crown", ModuleType::Source))
            .unwrap();

        let dependents: Vec<String> = contacts
            .dependents_of("sentinal")
            .iter()
            .map(|m| m.id().to_string())
            .collect();
        assert!(dependents.contains(&"vault".to_string()));
        assert!(dependents.contains(&"hall".to_string()));
        assert!(!dependents.contains(&"crown".to_string()));
    }

    #[test]
    fn dependencies_of() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("sentinal", "Sentinal", ModuleType::Source))
            .unwrap();
        contacts
            .register(ModuleInfo::new("x", "X", ModuleType::Source))
            .unwrap();
        contacts
            .register(
                ModuleInfo::new("vault", "Vault", ModuleType::Source)
                    .with_dependencies(vec!["sentinal".to_string(), "x".to_string()]),
            )
            .unwrap();

        let deps: Vec<String> = contacts
            .dependencies_of("vault")
            .iter()
            .map(|m| m.id().to_string())
            .collect();
        assert!(deps.contains(&"sentinal".to_string()));
        assert!(deps.contains(&"x".to_string()));
    }

    #[test]
    fn module_info_serde() {
        let info = ModuleInfo::new("vault", "Vault", ModuleType::Plugin)
            .with_dependencies(vec!["sentinal".to_string()]);

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModuleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
    }

    #[test]
    fn module_type_serde() {
        let json = serde_json::to_string(&ModuleType::Plugin).unwrap();
        assert_eq!(json, "\"plugin\"");

        let deserialized: ModuleType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ModuleType::Plugin);
    }

    // ── Catalog tests ───────────────────────────────────────────────

    use crate::catalog::{CallDescriptor, EventDescriptor, EdgeType, ModuleCatalog};

    fn crown_info() -> ModuleInfo {
        ModuleInfo::new("crown", "Crown", ModuleType::Source).with_catalog(
            ModuleCatalog::new()
                .with_call(CallDescriptor::new("crown.getProfile", "Get profile"))
                .with_call(CallDescriptor::new("crown.sign", "Sign data"))
                .with_emitted_event(EventDescriptor::new(
                    "crown.profileChanged",
                    "Profile updated",
                ))
                .with_subscribed_event(EventDescriptor::new(
                    "globe.eventReceived",
                    "Relay events",
                )),
        )
    }

    fn vault_info() -> ModuleInfo {
        ModuleInfo::new("vault", "Vault", ModuleType::Source).with_catalog(
            ModuleCatalog::new()
                .with_call(CallDescriptor::new("vault.lock", "Lock vault"))
                .with_emitted_event(EventDescriptor::new("vault.locked", "Vault was locked"))
                .with_subscribed_event(EventDescriptor::new(
                    "crown.profileChanged",
                    "Identity changed",
                )),
        )
    }

    #[test]
    fn module_info_with_catalog() {
        let info = crown_info();
        assert_eq!(info.catalog().calls_handled().len(), 2);
    }

    #[test]
    fn module_info_default_catalog_empty() {
        let info = ModuleInfo::new("x", "X", ModuleType::Source);
        assert!(info.catalog().calls_handled().is_empty());
        assert!(info.catalog().events_emitted().is_empty());
        assert!(info.catalog().events_subscribed().is_empty());
    }

    #[test]
    fn module_info_catalog_serde() {
        let info = crown_info();
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModuleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
    }

    #[test]
    fn module_info_empty_catalog_serde() {
        let info = ModuleInfo::new("x", "X", ModuleType::Source);
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModuleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
    }

    #[test]
    fn update_catalog_success() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("crown", "Crown", ModuleType::Source))
            .unwrap();

        let catalog = ModuleCatalog::new()
            .with_call(CallDescriptor::new("crown.getProfile", "Get profile"));
        contacts.update_catalog("crown", catalog).unwrap();

        assert_eq!(contacts.calls_for("crown").len(), 1);
        assert_eq!(contacts.calls_for("crown")[0].call_id(), "crown.getProfile");
    }

    #[test]
    fn update_catalog_not_found() {
        let contacts = Contacts::new();
        let result = contacts.update_catalog(
            "nonexistent",
            ModuleCatalog::new(),
        );
        assert!(matches!(result, Err(ContactsError::NotFound(_))));
    }

    #[test]
    fn update_catalog_replaces_previous() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        assert_eq!(contacts.calls_for("crown").len(), 2);

        let new_catalog = ModuleCatalog::new()
            .with_call(CallDescriptor::new("crown.newCall", "New call"));
        contacts.update_catalog("crown", new_catalog).unwrap();

        assert_eq!(contacts.calls_for("crown").len(), 1);
        assert_eq!(contacts.calls_for("crown")[0].call_id(), "crown.newCall");
    }

    #[test]
    fn calls_for_with_catalog() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();

        let calls = contacts.calls_for("crown");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].call_id(), "crown.getProfile");
        assert_eq!(calls[1].call_id(), "crown.sign");
    }

    #[test]
    fn calls_for_without_catalog() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("x", "X", ModuleType::Source))
            .unwrap();
        assert!(contacts.calls_for("x").is_empty());
    }

    #[test]
    fn calls_for_unknown_module() {
        let contacts = Contacts::new();
        assert!(contacts.calls_for("nonexistent").is_empty());
    }

    #[test]
    fn events_emitted_by_module() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();

        let events = contacts.events_emitted_by("crown");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].email_id(), "crown.profileChanged");
    }

    #[test]
    fn events_subscribed_by_module() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();

        let events = contacts.events_subscribed_by("crown");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].email_id(), "globe.eventReceived");
    }

    #[test]
    fn who_handles_found() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();

        assert_eq!(
            contacts.who_handles("crown.getProfile"),
            Some("crown".to_string())
        );
    }

    #[test]
    fn who_handles_not_found() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        assert!(contacts.who_handles("nonexistent.call").is_none());
    }

    #[test]
    fn who_emits_single() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();

        let emitters = contacts.who_emits("crown.profileChanged");
        assert_eq!(emitters, vec!["crown"]);
    }

    #[test]
    fn who_emits_multiple() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        // Both emit different events, check each.
        let crown_emitters = contacts.who_emits("crown.profileChanged");
        assert_eq!(crown_emitters.len(), 1);
        assert!(crown_emitters.contains(&"crown".to_string()));

        let vault_emitters = contacts.who_emits("vault.locked");
        assert_eq!(vault_emitters.len(), 1);
        assert!(vault_emitters.contains(&"vault".to_string()));
    }

    #[test]
    fn who_subscribes_found() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        // Vault subscribes to "crown.profileChanged".
        let subscribers = contacts.who_subscribes("crown.profileChanged");
        assert!(subscribers.contains(&"vault".to_string()));
    }

    #[test]
    fn who_subscribes_not_found() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        assert!(contacts.who_subscribes("nonexistent.event").is_empty());
    }

    #[test]
    fn all_calls_in_registration_order() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        let calls = contacts.all_calls();
        // Crown has 2, Vault has 1 = 3 total.
        assert_eq!(calls.len(), 3);
        // Crown registered first.
        assert_eq!(calls[0].0, "crown");
        assert_eq!(calls[0].1.call_id(), "crown.getProfile");
        assert_eq!(calls[1].0, "crown");
        assert_eq!(calls[1].1.call_id(), "crown.sign");
        assert_eq!(calls[2].0, "vault");
        assert_eq!(calls[2].1.call_id(), "vault.lock");
    }

    #[test]
    fn all_events_in_registration_order() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        let events = contacts.all_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "crown");
        assert_eq!(events[0].1.email_id(), "crown.profileChanged");
        assert_eq!(events[1].0, "vault");
        assert_eq!(events[1].1.email_id(), "vault.locked");
    }

    #[test]
    fn topology_event_edges() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        let topo = contacts.topology();

        // Crown emits "crown.profileChanged", Vault subscribes to it.
        let event_edges: Vec<_> = topo
            .edges
            .iter()
            .filter(|e| e.edge_type == EdgeType::Event)
            .collect();

        assert!(event_edges.iter().any(|e| e.from_module == "crown"
            && e.to_module == "vault"
            && e.message_id == "crown.profileChanged"));
    }

    #[test]
    fn topology_call_edges() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();

        let topo = contacts.topology();

        let call_edges: Vec<_> = topo
            .edges
            .iter()
            .filter(|e| e.edge_type == EdgeType::Call)
            .collect();

        // Crown handles 2 calls.
        assert_eq!(call_edges.len(), 2);
        assert!(call_edges
            .iter()
            .all(|e| e.from_module.is_empty() && e.to_module == "crown"));
    }

    #[test]
    fn topology_mixed() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        let topo = contacts.topology();

        let event_count = topo.edges.iter().filter(|e| e.edge_type == EdgeType::Event).count();
        let call_count = topo.edges.iter().filter(|e| e.edge_type == EdgeType::Call).count();

        // Crown emits profileChanged → Vault subscribes = 1 event edge.
        // Crown: 2 calls, Vault: 1 call = 3 call edges.
        assert_eq!(event_count, 1);
        assert_eq!(call_count, 3);
    }

    // ── Federation scope tests ──────────────────────────────────────

    use crate::federation_scope::FederationScope;

    /// Module with community_id for federation tests.
    fn community_module(id: &str, name: &str, community: &str) -> ModuleInfo {
        ModuleInfo::new(id, name, ModuleType::App).with_community(community)
    }

    /// Module with community + catalog for federation tests.
    fn community_module_with_catalog(
        id: &str,
        community: &str,
        call_id: &str,
        event_id: &str,
    ) -> ModuleInfo {
        ModuleInfo::new(id, id, ModuleType::App)
            .with_community(community)
            .with_catalog(
                ModuleCatalog::new()
                    .with_call(CallDescriptor::new(call_id, "desc"))
                    .with_emitted_event(EventDescriptor::new(event_id, "desc")),
            )
    }

    #[test]
    fn module_info_community_id() {
        let system = ModuleInfo::new("crown", "Crown", ModuleType::Source);
        assert!(system.community_id().is_none());

        let community = community_module("tome_alpha", "Tome Alpha", "alpha");
        assert_eq!(community.community_id(), Some("alpha"));
    }

    #[test]
    fn module_info_community_serde() {
        let info = community_module("tome_alpha", "Tome Alpha", "alpha");
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModuleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
        assert_eq!(deserialized.community_id(), Some("alpha"));
    }

    #[test]
    fn module_info_no_community_backward_compat() {
        // JSON without community_id field should deserialize fine.
        let json = r#"{
            "id": "crown",
            "name": "Crown",
            "module_type": "source",
            "depends_on": [],
            "catalog": {
                "calls_handled": [],
                "events_emitted": [],
                "events_subscribed": [],
                "channels_supported": []
            }
        }"#;
        let info: ModuleInfo = serde_json::from_str(json).unwrap();
        assert!(info.community_id().is_none());
    }

    #[test]
    fn all_modules_scoped_system_always_visible() {
        let contacts = Contacts::new();
        // System module (no community).
        contacts
            .register(ModuleInfo::new("crown", "Crown", ModuleType::Source))
            .unwrap();
        // Community module.
        contacts
            .register(community_module("tome_alpha", "Tome Alpha", "alpha"))
            .unwrap();

        // Scope to beta -- crown visible (system), tome_alpha not.
        let scope = FederationScope::from_communities(["beta"]);
        let visible = contacts.all_modules_scoped(&scope);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id(), "crown");
    }

    #[test]
    fn all_modules_scoped_unrestricted() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("crown", "Crown", ModuleType::Source))
            .unwrap();
        contacts
            .register(community_module("tome_alpha", "Tome Alpha", "alpha"))
            .unwrap();

        let scope = FederationScope::new();
        let visible = contacts.all_modules_scoped(&scope);
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn all_modules_scoped_filters_defederated() {
        let contacts = Contacts::new();
        contacts
            .register(community_module("tome_alpha", "Tome Alpha", "alpha"))
            .unwrap();
        contacts
            .register(community_module("tome_beta", "Tome Beta", "beta"))
            .unwrap();
        contacts
            .register(community_module("tome_gamma", "Tome Gamma", "gamma"))
            .unwrap();

        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        let visible = contacts.all_modules_scoped(&scope);
        assert_eq!(visible.len(), 2);
        let ids: Vec<&str> = visible.iter().map(|m| m.id()).collect();
        assert!(ids.contains(&"tome_alpha"));
        assert!(ids.contains(&"tome_gamma"));
        assert!(!ids.contains(&"tome_beta"));
    }

    #[test]
    fn lookup_scoped_visible() {
        let contacts = Contacts::new();
        contacts
            .register(community_module("tome_alpha", "Tome Alpha", "alpha"))
            .unwrap();

        let scope = FederationScope::from_communities(["alpha"]);
        assert!(contacts.lookup_scoped("tome_alpha", &scope).is_some());
    }

    #[test]
    fn lookup_scoped_defederated() {
        let contacts = Contacts::new();
        contacts
            .register(community_module("tome_alpha", "Tome Alpha", "alpha"))
            .unwrap();

        let scope = FederationScope::from_communities(["beta"]);
        assert!(contacts.lookup_scoped("tome_alpha", &scope).is_none());
    }

    #[test]
    fn lookup_scoped_system_module_always_visible() {
        let contacts = Contacts::new();
        contacts
            .register(ModuleInfo::new("crown", "Crown", ModuleType::Source))
            .unwrap();

        let scope = FederationScope::from_communities(["beta"]);
        assert!(contacts.lookup_scoped("crown", &scope).is_some());
    }

    #[test]
    fn who_handles_scoped_visible() {
        let contacts = Contacts::new();
        contacts
            .register(community_module_with_catalog(
                "tome_alpha",
                "alpha",
                "tome.create",
                "tome.changed",
            ))
            .unwrap();

        let scope = FederationScope::from_communities(["alpha"]);
        assert_eq!(
            contacts.who_handles_scoped("tome.create", &scope),
            Some("tome_alpha".to_string())
        );
    }

    #[test]
    fn who_handles_scoped_defederated() {
        let contacts = Contacts::new();
        contacts
            .register(community_module_with_catalog(
                "tome_alpha",
                "alpha",
                "tome.create",
                "tome.changed",
            ))
            .unwrap();

        let scope = FederationScope::from_communities(["beta"]);
        assert!(contacts.who_handles_scoped("tome.create", &scope).is_none());
    }

    #[test]
    fn who_emits_scoped_filters_defederated() {
        let contacts = Contacts::new();
        contacts
            .register(community_module_with_catalog(
                "tome_alpha",
                "alpha",
                "tome.create",
                "content.changed",
            ))
            .unwrap();
        contacts
            .register(community_module_with_catalog(
                "tome_beta",
                "beta",
                "tome.search",
                "content.changed",
            ))
            .unwrap();

        let scope = FederationScope::from_communities(["alpha"]);
        let emitters = contacts.who_emits_scoped("content.changed", &scope);
        assert_eq!(emitters.len(), 1);
        assert!(emitters.contains(&"tome_alpha".to_string()));
    }

    #[test]
    fn who_subscribes_scoped_filters_defederated() {
        let contacts = Contacts::new();

        // System module emits event.
        contacts
            .register(crown_info())
            .unwrap();

        // Two community modules subscribe.
        contacts
            .register(
                ModuleInfo::new("vault_alpha", "Vault Alpha", ModuleType::App)
                    .with_community("alpha")
                    .with_catalog(
                        ModuleCatalog::new()
                            .with_subscribed_event(EventDescriptor::new(
                                "crown.profileChanged",
                                "Re-key",
                            )),
                    ),
            )
            .unwrap();
        contacts
            .register(
                ModuleInfo::new("vault_beta", "Vault Beta", ModuleType::App)
                    .with_community("beta")
                    .with_catalog(
                        ModuleCatalog::new()
                            .with_subscribed_event(EventDescriptor::new(
                                "crown.profileChanged",
                                "Re-key",
                            )),
                    ),
            )
            .unwrap();

        let scope = FederationScope::from_communities(["alpha"]);
        let subscribers = contacts.who_subscribes_scoped("crown.profileChanged", &scope);
        assert_eq!(subscribers.len(), 1);
        assert!(subscribers.contains(&"vault_alpha".to_string()));
    }

    #[test]
    fn all_calls_scoped_filters_defederated() {
        let contacts = Contacts::new();
        // System module.
        contacts.register(crown_info()).unwrap();
        // Community module.
        contacts
            .register(community_module_with_catalog(
                "tome_alpha",
                "alpha",
                "tome.create",
                "tome.changed",
            ))
            .unwrap();

        let scope = FederationScope::from_communities(["beta"]);
        let calls = contacts.all_calls_scoped(&scope);
        // Only crown's 2 calls (system), not tome_alpha's.
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|(id, _)| id == "crown"));
    }

    #[test]
    fn all_events_scoped_filters_defederated() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts
            .register(community_module_with_catalog(
                "tome_alpha",
                "alpha",
                "tome.create",
                "tome.changed",
            ))
            .unwrap();

        let scope = FederationScope::from_communities(["beta"]);
        let events = contacts.all_events_scoped(&scope);
        // Only crown's event (system), not tome_alpha's.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "crown");
    }

    #[test]
    fn topology_scoped_excludes_defederated() {
        let contacts = Contacts::new();

        // System module emits profileChanged.
        contacts.register(crown_info()).unwrap();

        // Alpha subscribes to profileChanged -- visible.
        contacts
            .register(
                ModuleInfo::new("vault_alpha", "Vault Alpha", ModuleType::App)
                    .with_community("alpha")
                    .with_catalog(
                        ModuleCatalog::new()
                            .with_subscribed_event(EventDescriptor::new(
                                "crown.profileChanged",
                                "Re-key",
                            )),
                    ),
            )
            .unwrap();

        // Beta subscribes to profileChanged -- defederated.
        contacts
            .register(
                ModuleInfo::new("vault_beta", "Vault Beta", ModuleType::App)
                    .with_community("beta")
                    .with_catalog(
                        ModuleCatalog::new()
                            .with_subscribed_event(EventDescriptor::new(
                                "crown.profileChanged",
                                "Re-key",
                            )),
                    ),
            )
            .unwrap();

        let scope = FederationScope::from_communities(["alpha"]);
        let topo = contacts.topology_scoped(&scope);

        let event_edges: Vec<_> = topo
            .edges
            .iter()
            .filter(|e| e.edge_type == EdgeType::Event)
            .collect();

        // Only crown -> vault_alpha, not crown -> vault_beta.
        assert_eq!(event_edges.len(), 1);
        assert_eq!(event_edges[0].from_module, "crown");
        assert_eq!(event_edges[0].to_module, "vault_alpha");
    }

    #[test]
    fn topology_scoped_unrestricted_matches_topology() {
        let contacts = Contacts::new();
        contacts.register(crown_info()).unwrap();
        contacts.register(vault_info()).unwrap();

        let full = contacts.topology();
        let scoped = contacts.topology_scoped(&FederationScope::new());

        assert_eq!(full.edges.len(), scoped.edges.len());
    }
}

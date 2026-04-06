use serde::{Deserialize, Serialize};

use super::permission::Permission;
use crate::trust::trust_layer::TrustLayer;

/// The 4 Collective default roles — baseline permissions that apps refine.
///
/// These map to Vault Collective roles. Apps can define additional roles
/// beyond these 4, but every Collective member has exactly one of these.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CollectiveRole {
    /// Read-only access.
    Readonly,
    /// Standard member: can create and interact.
    Member,
    /// Administrative capabilities: can manage members and settings.
    Admin,
    /// Full control: can delete, transfer ownership, manage admins.
    Owner,
}

impl CollectiveRole {
    /// Whether this role has at least as much authority as another.
    pub fn has_authority_over(&self, other: &CollectiveRole) -> bool {
        *self >= *other
    }
}

impl std::fmt::Display for CollectiveRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollectiveRole::Readonly => write!(f, "Readonly"),
            CollectiveRole::Member => write!(f, "Member"),
            CollectiveRole::Admin => write!(f, "Admin"),
            CollectiveRole::Owner => write!(f, "Owner"),
        }
    }
}

/// An app-defined role — a named set of permissions with optional trust requirements.
///
/// Apps register custom roles beyond the 4 Collective defaults. Each role
/// is a named bundle of permissions, optionally requiring a minimum trust
/// layer and/or minimum collective role.
///
/// Example roles: "Designer" (can upload brand assets), "Reviewer" (can
/// approve assets), "External" (can view watermarked only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Role {
    /// Unique name within the app (e.g., "Designer", "Reviewer").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Permissions this role grants.
    pub permissions: Vec<Permission>,
    /// Minimum trust layer required to hold this role.
    /// `None` means any trust layer is sufficient.
    pub minimum_trust_layer: Option<TrustLayer>,
    /// Minimum collective role required.
    /// `None` means this role can be assigned independently of collective role.
    pub minimum_collective_role: Option<CollectiveRole>,
}

impl Role {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            permissions: Vec::new(),
            minimum_trust_layer: None,
            minimum_collective_role: None,
        }
    }

    /// Add a permission to this role.
    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permissions.push(permission);
        self
    }

    /// Add multiple permissions.
    pub fn with_permissions(mut self, permissions: Vec<Permission>) -> Self {
        self.permissions.extend(permissions);
        self
    }

    /// Require a minimum trust layer to hold this role.
    pub fn requiring_trust_layer(mut self, layer: TrustLayer) -> Self {
        self.minimum_trust_layer = Some(layer);
        self
    }

    /// Require a minimum collective role to hold this role.
    pub fn requiring_collective_role(mut self, role: CollectiveRole) -> Self {
        self.minimum_collective_role = Some(role);
        self
    }

    /// Whether an actor with the given trust layer and collective role meets
    /// this role's prerequisites.
    pub fn actor_qualifies(&self, trust_layer: TrustLayer, collective_role: CollectiveRole) -> bool {
        let trust_ok = self
            .minimum_trust_layer
            .is_none_or(|min| trust_layer >= min);
        let role_ok = self
            .minimum_collective_role
            .is_none_or(|min| collective_role >= min);
        trust_ok && role_ok
    }

    /// Whether this role grants a specific action on a resource.
    pub fn has_permission_for(
        &self,
        action: &super::permission::Action,
        resource: &super::permission::ResourceScope,
    ) -> bool {
        self.permissions.iter().any(|p| p.covers(action, resource))
    }
}

/// A registry of app-defined roles.
///
/// Apps register their custom roles here. The registry validates
/// that role names are unique and allows lookup by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RoleRegistry {
    pub roles: Vec<Role>,
}

impl RoleRegistry {
    pub fn new() -> Self {
        Self { roles: Vec::new() }
    }

    /// Register a new role. Returns an error if the name is already taken.
    pub fn register(&mut self, role: Role) -> Result<(), crate::BulwarkError> {
        if self.roles.iter().any(|r| r.name == role.name) {
            return Err(crate::BulwarkError::ConfigError(format!(
                "role '{}' already registered",
                role.name
            )));
        }
        self.roles.push(role);
        Ok(())
    }

    /// Look up a role by name.
    pub fn get(&self, name: &str) -> Option<&Role> {
        self.roles.iter().find(|r| r.name == name)
    }

    /// Remove a role by name.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.roles.len();
        self.roles.retain(|r| r.name != name);
        self.roles.len() < before
    }

    /// All registered role names.
    pub fn role_names(&self) -> Vec<&str> {
        self.roles.iter().map(|r| r.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::permission::{Action, ResourceScope};

    #[test]
    fn collective_role_ordering() {
        assert!(CollectiveRole::Readonly < CollectiveRole::Member);
        assert!(CollectiveRole::Member < CollectiveRole::Admin);
        assert!(CollectiveRole::Admin < CollectiveRole::Owner);
    }

    #[test]
    fn collective_role_authority() {
        assert!(CollectiveRole::Owner.has_authority_over(&CollectiveRole::Admin));
        assert!(CollectiveRole::Admin.has_authority_over(&CollectiveRole::Member));
        assert!(CollectiveRole::Member.has_authority_over(&CollectiveRole::Readonly));
        assert!(!CollectiveRole::Readonly.has_authority_over(&CollectiveRole::Member));
    }

    #[test]
    fn role_creation_builder() {
        let role = Role::new("Designer", "Can upload and manage brand assets")
            .with_permission(Permission::from_strings("upload", "brand"))
            .with_permission(Permission::from_strings("view", "brand"))
            .requiring_trust_layer(TrustLayer::Verified)
            .requiring_collective_role(CollectiveRole::Member);

        assert_eq!(role.name, "Designer");
        assert_eq!(role.permissions.len(), 2);
        assert_eq!(role.minimum_trust_layer, Some(TrustLayer::Verified));
        assert_eq!(role.minimum_collective_role, Some(CollectiveRole::Member));
    }

    #[test]
    fn role_with_multiple_permissions() {
        let perms = vec![
            Permission::from_strings("view", "brand"),
            Permission::from_strings("upload", "brand"),
            Permission::from_strings("edit", "brand.logo"),
        ];
        let role = Role::new("Designer", "Brand designer").with_permissions(perms);
        assert_eq!(role.permissions.len(), 3);
    }

    #[test]
    fn actor_qualifies_no_requirements() {
        let role = Role::new("Viewer", "Can view things");
        // No minimums — anyone qualifies
        assert!(role.actor_qualifies(TrustLayer::Connected, CollectiveRole::Readonly));
    }

    #[test]
    fn actor_qualifies_trust_layer_check() {
        let role = Role::new("Contributor", "Needs verification")
            .requiring_trust_layer(TrustLayer::Verified);

        assert!(!role.actor_qualifies(TrustLayer::Connected, CollectiveRole::Member));
        assert!(role.actor_qualifies(TrustLayer::Verified, CollectiveRole::Member));
        assert!(role.actor_qualifies(TrustLayer::Vouched, CollectiveRole::Member));
    }

    #[test]
    fn actor_qualifies_collective_role_check() {
        let role = Role::new("Moderator", "Needs admin")
            .requiring_collective_role(CollectiveRole::Admin);

        assert!(!role.actor_qualifies(TrustLayer::Vouched, CollectiveRole::Member));
        assert!(role.actor_qualifies(TrustLayer::Vouched, CollectiveRole::Admin));
        assert!(role.actor_qualifies(TrustLayer::Vouched, CollectiveRole::Owner));
    }

    #[test]
    fn actor_qualifies_both_requirements() {
        let role = Role::new("Safety Officer", "Highly trusted admin")
            .requiring_trust_layer(TrustLayer::Shielded)
            .requiring_collective_role(CollectiveRole::Admin);

        // Fails trust
        assert!(!role.actor_qualifies(TrustLayer::Vouched, CollectiveRole::Admin));
        // Fails role
        assert!(!role.actor_qualifies(TrustLayer::Shielded, CollectiveRole::Member));
        // Both pass
        assert!(role.actor_qualifies(TrustLayer::Shielded, CollectiveRole::Admin));
    }

    #[test]
    fn role_has_permission() {
        let role = Role::new("Designer", "Brand designer")
            .with_permission(Permission::from_strings("upload", "brand"))
            .with_permission(Permission::from_strings("view", "brand"));

        assert!(role.has_permission_for(&Action::upload(), &ResourceScope::new("brand")));
        assert!(role.has_permission_for(&Action::view(), &ResourceScope::new("brand.logo")));
        assert!(!role.has_permission_for(&Action::delete(), &ResourceScope::new("brand")));
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut registry = RoleRegistry::new();
        registry
            .register(
                Role::new("Designer", "Brand designer")
                    .with_permission(Permission::from_strings("upload", "brand")),
            )
            .unwrap();

        assert!(registry.get("Designer").is_some());
        assert!(registry.get("Reviewer").is_none());
        assert_eq!(registry.role_names(), vec!["Designer"]);
    }

    #[test]
    fn registry_rejects_duplicate_names() {
        let mut registry = RoleRegistry::new();
        registry
            .register(Role::new("Designer", "First"))
            .unwrap();
        let result = registry.register(Role::new("Designer", "Second"));
        assert!(result.is_err());
    }

    #[test]
    fn registry_unregister() {
        let mut registry = RoleRegistry::new();
        registry
            .register(Role::new("Designer", "Brand designer"))
            .unwrap();
        assert!(registry.unregister("Designer"));
        assert!(registry.get("Designer").is_none());
        assert!(!registry.unregister("Designer")); // already removed
    }

    #[test]
    fn collective_role_display() {
        assert_eq!(CollectiveRole::Owner.to_string(), "Owner");
        assert_eq!(CollectiveRole::Admin.to_string(), "Admin");
        assert_eq!(CollectiveRole::Member.to_string(), "Member");
        assert_eq!(CollectiveRole::Readonly.to_string(), "Readonly");
    }
}

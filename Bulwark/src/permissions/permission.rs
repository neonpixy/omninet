use serde::{Deserialize, Serialize};

/// An action that can be performed on a resource.
///
/// Actions are app-defined strings, but common ones are provided as constants
/// for consistency across the ecosystem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Action(pub String);

impl Action {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Common actions — apps can define their own beyond these.
impl Action {
    pub fn view() -> Self { Self::new("view") }
    pub fn create() -> Self { Self::new("create") }
    pub fn edit() -> Self { Self::new("edit") }
    pub fn delete() -> Self { Self::new("delete") }
    pub fn upload() -> Self { Self::new("upload") }
    pub fn download() -> Self { Self::new("download") }
    pub fn approve() -> Self { Self::new("approve") }
    pub fn publish() -> Self { Self::new("publish") }
    pub fn invite() -> Self { Self::new("invite") }
    pub fn manage() -> Self { Self::new("manage") }
}

/// A resource scope — what the action applies to.
///
/// Resource scopes are hierarchical dot-separated paths.
/// `"brand.logo"` is a child of `"brand"`. A permission on `"brand"`
/// covers `"brand.logo"` unless the permission is exact-only.
///
/// The wildcard `"*"` matches everything.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ResourceScope(pub String);

impl ResourceScope {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Wildcard scope — matches any resource.
    pub fn wildcard() -> Self {
        Self::new("*")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this scope is a wildcard.
    pub fn is_wildcard(&self) -> bool {
        self.0 == "*"
    }

    /// Whether `self` covers `other` in the hierarchy.
    ///
    /// `"brand"` covers `"brand.logo"` and `"brand.logo.dark"`.
    /// `"brand.logo"` does NOT cover `"brand"`.
    /// `"*"` covers everything.
    pub fn covers(&self, other: &ResourceScope) -> bool {
        if self.is_wildcard() {
            return true;
        }
        if self.0 == other.0 {
            return true;
        }
        // Check hierarchical prefix: "brand" covers "brand.logo"
        other.0.starts_with(&self.0) && other.0.as_bytes().get(self.0.len()) == Some(&b'.')
    }
}

impl std::fmt::Display for ResourceScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single permission — an action on a resource scope, optionally conditional.
///
/// Permissions are local enforcement. The app checks them, not the protocol.
/// They compose ON TOP of trust layers, not replace them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Permission {
    pub action: Action,
    pub resource: ResourceScope,
}

impl Permission {
    pub fn new(action: Action, resource: ResourceScope) -> Self {
        Self { action, resource }
    }

    /// Convenience: create from string action and resource.
    pub fn from_strings(action: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            action: Action::new(action),
            resource: ResourceScope::new(resource),
        }
    }

    /// Whether this permission covers the requested action + resource.
    ///
    /// A broader permission covers a narrower request:
    /// `Permission("view", "brand")` covers a check for `("view", "brand.logo")`.
    pub fn covers(&self, action: &Action, resource: &ResourceScope) -> bool {
        self.action == *action && self.resource.covers(resource)
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.action, self.resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_creation() {
        let a = Action::new("upload");
        assert_eq!(a.as_str(), "upload");
        assert_eq!(a.to_string(), "upload");
    }

    #[test]
    fn common_actions() {
        assert_eq!(Action::view().as_str(), "view");
        assert_eq!(Action::create().as_str(), "create");
        assert_eq!(Action::edit().as_str(), "edit");
        assert_eq!(Action::delete().as_str(), "delete");
        assert_eq!(Action::upload().as_str(), "upload");
        assert_eq!(Action::download().as_str(), "download");
        assert_eq!(Action::approve().as_str(), "approve");
        assert_eq!(Action::publish().as_str(), "publish");
        assert_eq!(Action::invite().as_str(), "invite");
        assert_eq!(Action::manage().as_str(), "manage");
    }

    #[test]
    fn resource_scope_exact_match() {
        let scope = ResourceScope::new("brand.logo");
        assert!(scope.covers(&ResourceScope::new("brand.logo")));
        assert!(!scope.covers(&ResourceScope::new("brand")));
        // Hierarchical: "brand.logo" covers "brand.logo.dark"
        assert!(scope.covers(&ResourceScope::new("brand.logo.dark")));
    }

    #[test]
    fn resource_scope_hierarchical() {
        let parent = ResourceScope::new("brand");
        assert!(parent.covers(&ResourceScope::new("brand")));
        assert!(parent.covers(&ResourceScope::new("brand.logo")));
        assert!(parent.covers(&ResourceScope::new("brand.logo.dark")));
        assert!(!parent.covers(&ResourceScope::new("settings")));
        assert!(!parent.covers(&ResourceScope::new("branding"))); // not a child
    }

    #[test]
    fn resource_scope_wildcard() {
        let wild = ResourceScope::wildcard();
        assert!(wild.is_wildcard());
        assert!(wild.covers(&ResourceScope::new("anything")));
        assert!(wild.covers(&ResourceScope::new("brand.logo")));
        assert!(wild.covers(&ResourceScope::wildcard()));
    }

    #[test]
    fn permission_covers() {
        let perm = Permission::from_strings("view", "brand");
        assert!(perm.covers(&Action::view(), &ResourceScope::new("brand")));
        assert!(perm.covers(&Action::view(), &ResourceScope::new("brand.logo")));
        assert!(!perm.covers(&Action::edit(), &ResourceScope::new("brand")));
        assert!(!perm.covers(&Action::view(), &ResourceScope::new("settings")));
    }

    #[test]
    fn permission_display() {
        let perm = Permission::from_strings("upload", "brand.logo");
        assert_eq!(perm.to_string(), "upload:brand.logo");
    }

    #[test]
    fn permission_equality() {
        let a = Permission::from_strings("view", "brand");
        let b = Permission::from_strings("view", "brand");
        let c = Permission::from_strings("edit", "brand");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn permission_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Permission::from_strings("view", "brand"));
        set.insert(Permission::from_strings("view", "brand")); // duplicate
        set.insert(Permission::from_strings("edit", "brand"));
        assert_eq!(set.len(), 2);
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::permission::Permission;

/// A delegation — granting a subset of your permissions to another actor.
///
/// Delegations are time-limited and revocable. You can only delegate
/// permissions you actually hold (enforced at check time, not at creation
/// time, since permission sets can change).
///
/// Design: Delegation is local data, not a protocol-level event. The app
/// tracks who delegated what to whom. Sovereignty means the delegator can
/// revoke at any time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Delegation {
    pub id: Uuid,
    /// Who is granting the permissions.
    pub delegator: String,
    /// Who is receiving the permissions.
    pub delegate: String,
    /// Which permissions are being delegated (must be a subset of what
    /// the delegator holds).
    pub permissions: Vec<Permission>,
    /// When this delegation was created.
    pub granted_at: DateTime<Utc>,
    /// When this delegation expires (if ever).
    pub expires_at: Option<DateTime<Utc>>,
    /// When this delegation was revoked (if ever).
    pub revoked_at: Option<DateTime<Utc>>,
    /// Optional reason or note.
    pub reason: Option<String>,
    /// Whether the delegate can further delegate these permissions.
    pub can_redelegate: bool,
}

impl Delegation {
    pub fn new(
        delegator: impl Into<String>,
        delegate: impl Into<String>,
        permissions: Vec<Permission>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            delegator: delegator.into(),
            delegate: delegate.into(),
            permissions,
            granted_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
            reason: None,
            can_redelegate: false,
        }
    }

    /// Set an expiration time.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Attach a reason/note.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Allow the delegate to further delegate these permissions.
    pub fn with_redelegation(mut self) -> Self {
        self.can_redelegate = true;
        self
    }

    /// Whether this delegation is currently active.
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
            && self
                .expires_at
                .is_none_or(|exp| Utc::now() < exp)
    }

    /// Revoke this delegation. Always available (consent is revocable).
    pub fn revoke(&mut self) {
        self.revoked_at = Some(Utc::now());
    }

    /// Whether this delegation covers a specific action + resource.
    pub fn covers(
        &self,
        action: &super::permission::Action,
        resource: &super::permission::ResourceScope,
    ) -> bool {
        self.is_active() && self.permissions.iter().any(|p| p.covers(action, resource))
    }
}

/// A store for tracking active delegations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DelegationStore {
    pub delegations: Vec<Delegation>,
}

impl DelegationStore {
    pub fn new() -> Self {
        Self {
            delegations: Vec::new(),
        }
    }

    /// Record a new delegation.
    pub fn grant(&mut self, delegation: Delegation) {
        self.delegations.push(delegation);
    }

    /// Get all active delegations TO a specific delegate.
    pub fn delegations_for(&self, delegate: &str) -> Vec<&Delegation> {
        self.delegations
            .iter()
            .filter(|d| d.delegate == delegate && d.is_active())
            .collect()
    }

    /// Get all active delegations FROM a specific delegator.
    pub fn delegations_from(&self, delegator: &str) -> Vec<&Delegation> {
        self.delegations
            .iter()
            .filter(|d| d.delegator == delegator && d.is_active())
            .collect()
    }

    /// Revoke all delegations from a specific delegator to a specific delegate.
    pub fn revoke_all(&mut self, delegator: &str, delegate: &str) {
        for d in &mut self.delegations {
            if d.delegator == delegator && d.delegate == delegate {
                d.revoke();
            }
        }
    }

    /// Revoke a specific delegation by ID.
    pub fn revoke_by_id(&mut self, id: Uuid) -> bool {
        if let Some(d) = self.delegations.iter_mut().find(|d| d.id == id) {
            d.revoke();
            true
        } else {
            false
        }
    }

    /// Whether the delegate has any active delegation covering an action + resource.
    pub fn delegate_can(
        &self,
        delegate: &str,
        action: &super::permission::Action,
        resource: &super::permission::ResourceScope,
    ) -> bool {
        self.delegations_for(delegate)
            .iter()
            .any(|d| d.covers(action, resource))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::permission::{Action, Permission, ResourceScope};

    #[test]
    fn delegation_lifecycle() {
        let mut d = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand.logo")],
        );
        assert!(d.is_active());

        d.revoke();
        assert!(!d.is_active());
    }

    #[test]
    fn delegation_with_expiry() {
        let d = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("view", "brand")],
        )
        .with_expiry(Utc::now() + chrono::Duration::days(30));
        assert!(d.is_active());

        let expired = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("view", "brand")],
        )
        .with_expiry(Utc::now() - chrono::Duration::days(1));
        assert!(!expired.is_active());
    }

    #[test]
    fn delegation_covers_check() {
        let d = Delegation::new(
            "alice",
            "bob",
            vec![
                Permission::from_strings("upload", "brand"),
                Permission::from_strings("view", "brand"),
            ],
        );

        assert!(d.covers(&Action::upload(), &ResourceScope::new("brand")));
        assert!(d.covers(&Action::upload(), &ResourceScope::new("brand.logo")));
        assert!(d.covers(&Action::view(), &ResourceScope::new("brand")));
        assert!(!d.covers(&Action::delete(), &ResourceScope::new("brand")));
        assert!(!d.covers(&Action::upload(), &ResourceScope::new("settings")));
    }

    #[test]
    fn revoked_delegation_does_not_cover() {
        let mut d = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand")],
        );
        d.revoke();
        assert!(!d.covers(&Action::upload(), &ResourceScope::new("brand")));
    }

    #[test]
    fn delegation_store_grant_and_lookup() {
        let mut store = DelegationStore::new();
        store.grant(Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand")],
        ));
        store.grant(Delegation::new(
            "alice",
            "charlie",
            vec![Permission::from_strings("view", "brand")],
        ));

        assert_eq!(store.delegations_for("bob").len(), 1);
        assert_eq!(store.delegations_for("charlie").len(), 1);
        assert_eq!(store.delegations_from("alice").len(), 2);
    }

    #[test]
    fn delegation_store_delegate_can() {
        let mut store = DelegationStore::new();
        store.grant(Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand")],
        ));

        assert!(store.delegate_can("bob", &Action::upload(), &ResourceScope::new("brand")));
        assert!(store.delegate_can("bob", &Action::upload(), &ResourceScope::new("brand.logo")));
        assert!(!store.delegate_can("bob", &Action::delete(), &ResourceScope::new("brand")));
        assert!(!store.delegate_can("charlie", &Action::upload(), &ResourceScope::new("brand")));
    }

    #[test]
    fn delegation_store_revoke_all() {
        let mut store = DelegationStore::new();
        store.grant(Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand")],
        ));
        store.grant(Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("view", "brand")],
        ));

        assert_eq!(store.delegations_for("bob").len(), 2);
        store.revoke_all("alice", "bob");
        assert_eq!(store.delegations_for("bob").len(), 0);
    }

    #[test]
    fn delegation_store_revoke_by_id() {
        let mut store = DelegationStore::new();
        let d = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand")],
        );
        let id = d.id;
        store.grant(d);

        assert!(store.revoke_by_id(id));
        assert_eq!(store.delegations_for("bob").len(), 0);
        assert!(!store.revoke_by_id(Uuid::new_v4())); // not found
    }

    #[test]
    fn delegation_with_reason() {
        let d = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("upload", "brand")],
        )
        .with_reason("Covering while Alice is on vacation");
        assert_eq!(
            d.reason.as_deref(),
            Some("Covering while Alice is on vacation")
        );
    }

    #[test]
    fn delegation_redelegation_flag() {
        let d = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("view", "brand")],
        )
        .with_redelegation();
        assert!(d.can_redelegate);

        let d2 = Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("view", "brand")],
        );
        assert!(!d2.can_redelegate);
    }
}

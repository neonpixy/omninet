use serde::{Deserialize, Serialize};

use super::condition::{ConditionalPermission, PermissionContext};
use super::delegation::DelegationStore;
use super::permission::{Action, Permission, ResourceScope};
use super::role::{CollectiveRole, Role, RoleRegistry};
use crate::trust::trust_layer::TrustLayer;

/// An actor's identity in the permission system.
///
/// Ties together the actor's pubkey, trust layer, collective role,
/// and assigned app roles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActorContext {
    /// The actor's public key.
    pub pubkey: String,
    /// Current trust layer (from Bulwark's trust system).
    pub trust_layer: TrustLayer,
    /// Current collective role (from Vault).
    pub collective_role: CollectiveRole,
    /// App-defined role names assigned to this actor.
    pub assigned_roles: Vec<String>,
}

impl ActorContext {
    pub fn new(
        pubkey: impl Into<String>,
        trust_layer: TrustLayer,
        collective_role: CollectiveRole,
    ) -> Self {
        Self {
            pubkey: pubkey.into(),
            trust_layer,
            collective_role,
            assigned_roles: Vec::new(),
        }
    }

    /// Assign an app-defined role to this actor.
    pub fn with_role(mut self, role_name: impl Into<String>) -> Self {
        let name = role_name.into();
        if !self.assigned_roles.contains(&name) {
            self.assigned_roles.push(name);
        }
        self
    }

    /// Assign multiple app-defined roles.
    pub fn with_roles(mut self, role_names: Vec<String>) -> Self {
        for name in role_names {
            if !self.assigned_roles.contains(&name) {
                self.assigned_roles.push(name);
            }
        }
        self
    }
}

/// The permission checker — the central `can(actor, action, resource)` function.
///
/// Integrates:
/// - Trust layers (network-level primitive from Bulwark)
/// - Collective roles (baseline from Vault)
/// - App-defined roles (from the RoleRegistry)
/// - Conditional permissions
/// - Delegations
///
/// Permissions compose ON TOP of trust layers, not replace them.
/// The checker is pure data — zero async.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PermissionChecker {
    /// Registry of app-defined roles.
    pub role_registry: RoleRegistry,
    /// Conditional permissions (applied globally, not role-specific).
    pub conditional_permissions: Vec<ActorConditionalPermission>,
    /// Delegation store.
    pub delegations: DelegationStore,
}

/// A conditional permission assigned to a specific actor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActorConditionalPermission {
    pub actor_pubkey: String,
    pub conditional: ConditionalPermission,
}

/// The result of a permission check — explains why access was granted or denied.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    /// Access granted, with the reason.
    Allowed(PermissionSource),
    /// Access denied, with the reason.
    Denied(DenialReason),
}

impl PermissionDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionDecision::Allowed(_))
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, PermissionDecision::Denied(_))
    }
}

/// Why access was granted.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionSource {
    /// Granted through an app-defined role.
    Role(String),
    /// Granted through a conditional permission.
    Conditional,
    /// Granted through a delegation.
    Delegation { delegator: String },
}

/// Why access was denied.
#[derive(Debug, Clone, PartialEq)]
pub enum DenialReason {
    /// No role, conditional permission, or delegation grants this action.
    NoPermission,
    /// Actor has the role, but doesn't meet the role's prerequisites.
    RolePrerequisitesNotMet {
        role: String,
        reason: String,
    },
    /// Conditional permission exists but conditions not met.
    ConditionsNotMet,
}

impl PermissionChecker {
    pub fn new() -> Self {
        Self {
            role_registry: RoleRegistry::new(),
            conditional_permissions: Vec::new(),
            delegations: DelegationStore::new(),
        }
    }

    /// Register an app-defined role.
    pub fn register_role(&mut self, role: Role) -> Result<(), crate::BulwarkError> {
        self.role_registry.register(role)
    }

    /// Add a conditional permission for a specific actor.
    pub fn add_conditional_permission(
        &mut self,
        actor_pubkey: impl Into<String>,
        conditional: ConditionalPermission,
    ) {
        self.conditional_permissions.push(ActorConditionalPermission {
            actor_pubkey: actor_pubkey.into(),
            conditional,
        });
    }

    /// The core permission check: can this actor perform this action on this resource?
    ///
    /// Checks in order:
    /// 1. App-defined roles assigned to the actor
    /// 2. Conditional permissions for the actor
    /// 3. Delegations to the actor
    ///
    /// Returns `true` if any source grants the permission.
    pub fn can(
        &self,
        actor: &ActorContext,
        action: &Action,
        resource: &ResourceScope,
    ) -> bool {
        self.check(actor, action, resource, &PermissionContext::new())
            .is_allowed()
    }

    /// Permission check with context (for conditional permissions).
    pub fn check(
        &self,
        actor: &ActorContext,
        action: &Action,
        resource: &ResourceScope,
        context: &PermissionContext,
    ) -> PermissionDecision {
        // 1. Check app-defined roles
        for role_name in &actor.assigned_roles {
            if let Some(role) = self.role_registry.get(role_name) {
                if role.actor_qualifies(actor.trust_layer, actor.collective_role)
                    && role.has_permission_for(action, resource)
                {
                    return PermissionDecision::Allowed(PermissionSource::Role(
                        role_name.clone(),
                    ));
                }
            }
        }

        // 2. Check conditional permissions
        for acp in &self.conditional_permissions {
            if acp.actor_pubkey == actor.pubkey
                && acp.conditional.allows(action, resource, context)
            {
                return PermissionDecision::Allowed(PermissionSource::Conditional);
            }
        }

        // 3. Check delegations
        for delegation in self.delegations.delegations_for(&actor.pubkey) {
            if delegation.covers(action, resource) {
                return PermissionDecision::Allowed(PermissionSource::Delegation {
                    delegator: delegation.delegator.clone(),
                });
            }
        }

        PermissionDecision::Denied(DenialReason::NoPermission)
    }

    /// Check whether a specific role would grant an action on a resource,
    /// regardless of whether any actor is assigned to it.
    pub fn role_would_allow(
        &self,
        role_name: &str,
        action: &Action,
        resource: &ResourceScope,
    ) -> bool {
        self.role_registry
            .get(role_name)
            .is_some_and(|role| role.has_permission_for(action, resource))
    }

    /// List all permissions an actor effectively has (from all sources).
    pub fn effective_permissions(&self, actor: &ActorContext) -> Vec<EffectivePermission> {
        let mut result = Vec::new();

        // From roles
        for role_name in &actor.assigned_roles {
            if let Some(role) = self.role_registry.get(role_name) {
                if role.actor_qualifies(actor.trust_layer, actor.collective_role) {
                    for perm in &role.permissions {
                        result.push(EffectivePermission {
                            permission: perm.clone(),
                            source: PermissionSource::Role(role_name.clone()),
                        });
                    }
                }
            }
        }

        // From delegations
        for delegation in self.delegations.delegations_for(&actor.pubkey) {
            for perm in &delegation.permissions {
                result.push(EffectivePermission {
                    permission: perm.clone(),
                    source: PermissionSource::Delegation {
                        delegator: delegation.delegator.clone(),
                    },
                });
            }
        }

        result
    }
}

/// A permission with its source — for listing effective permissions.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectivePermission {
    pub permission: Permission,
    pub source: PermissionSource,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::condition::{Condition, ConditionalPermission};
    use crate::permissions::delegation::Delegation;

    fn setup_checker() -> PermissionChecker {
        let mut checker = PermissionChecker::new();

        // Register roles
        checker
            .register_role(
                Role::new("Designer", "Can upload and view brand assets")
                    .with_permission(Permission::from_strings("upload", "brand"))
                    .with_permission(Permission::from_strings("view", "brand"))
                    .requiring_trust_layer(TrustLayer::Verified)
                    .requiring_collective_role(CollectiveRole::Member),
            )
            .unwrap();

        checker
            .register_role(
                Role::new("Reviewer", "Can approve brand assets")
                    .with_permission(Permission::from_strings("view", "brand"))
                    .with_permission(Permission::from_strings("approve", "brand"))
                    .requiring_trust_layer(TrustLayer::Vouched)
                    .requiring_collective_role(CollectiveRole::Admin),
            )
            .unwrap();

        checker
            .register_role(
                Role::new("External", "Can view watermarked only")
                    .with_permission(Permission::from_strings("view", "brand.public")),
            )
            .unwrap();

        checker
    }

    #[test]
    fn can_basic_role_check() {
        let checker = setup_checker();

        let designer = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Member)
            .with_role("Designer");

        assert!(checker.can(&designer, &Action::upload(), &ResourceScope::new("brand")));
        assert!(checker.can(&designer, &Action::view(), &ResourceScope::new("brand")));
        assert!(checker.can(&designer, &Action::view(), &ResourceScope::new("brand.logo")));
        assert!(!checker.can(&designer, &Action::delete(), &ResourceScope::new("brand")));
        assert!(!checker.can(&designer, &Action::approve(), &ResourceScope::new("brand")));
    }

    #[test]
    fn can_denied_insufficient_trust() {
        let checker = setup_checker();

        // Alice has Designer role but only Connected trust (needs Verified)
        let alice = ActorContext::new("alice", TrustLayer::Connected, CollectiveRole::Member)
            .with_role("Designer");

        assert!(!checker.can(&alice, &Action::upload(), &ResourceScope::new("brand")));
    }

    #[test]
    fn can_denied_insufficient_collective_role() {
        let checker = setup_checker();

        // Alice has Designer role but is Readonly (needs Member)
        let alice = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Readonly)
            .with_role("Designer");

        assert!(!checker.can(&alice, &Action::upload(), &ResourceScope::new("brand")));
    }

    #[test]
    fn reviewer_role_requires_vouched_admin() {
        let checker = setup_checker();

        let reviewer = ActorContext::new("bob", TrustLayer::Vouched, CollectiveRole::Admin)
            .with_role("Reviewer");
        assert!(checker.can(&reviewer, &Action::approve(), &ResourceScope::new("brand")));

        // Not enough trust
        let bad_reviewer = ActorContext::new("bob", TrustLayer::Verified, CollectiveRole::Admin)
            .with_role("Reviewer");
        assert!(!checker.can(&bad_reviewer, &Action::approve(), &ResourceScope::new("brand")));
    }

    #[test]
    fn external_role_limited_scope() {
        let checker = setup_checker();

        let external = ActorContext::new("vendor", TrustLayer::Connected, CollectiveRole::Readonly)
            .with_role("External");

        // Can view brand.public
        assert!(checker.can(&external, &Action::view(), &ResourceScope::new("brand.public")));
        // Cannot view brand (broader scope than their permission)
        assert!(!checker.can(&external, &Action::view(), &ResourceScope::new("brand")));
        // Cannot upload anything
        assert!(!checker.can(&external, &Action::upload(), &ResourceScope::new("brand.public")));
    }

    #[test]
    fn multiple_roles_stack() {
        let checker = setup_checker();

        // Actor has both Designer and Reviewer roles
        let power_user = ActorContext::new("alice", TrustLayer::Vouched, CollectiveRole::Admin)
            .with_role("Designer")
            .with_role("Reviewer");

        assert!(checker.can(&power_user, &Action::upload(), &ResourceScope::new("brand")));
        assert!(checker.can(&power_user, &Action::approve(), &ResourceScope::new("brand")));
    }

    #[test]
    fn check_returns_decision_with_source() {
        let checker = setup_checker();

        let designer = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Member)
            .with_role("Designer");

        let decision = checker.check(
            &designer,
            &Action::upload(),
            &ResourceScope::new("brand"),
            &PermissionContext::new(),
        );
        assert!(decision.is_allowed());
        assert_eq!(
            decision,
            PermissionDecision::Allowed(PermissionSource::Role("Designer".into()))
        );
    }

    #[test]
    fn conditional_permission_check() {
        let mut checker = setup_checker();

        // "Bob can download brand assets IF asset_status == approved"
        checker.add_conditional_permission(
            "bob",
            ConditionalPermission::new(Permission::from_strings("download", "brand"))
                .with_condition(Condition::equals("asset_status", "approved")),
        );

        let bob = ActorContext::new("bob", TrustLayer::Verified, CollectiveRole::Member);

        // Without context or wrong context: denied
        assert!(!checker.can(&bob, &Action::download(), &ResourceScope::new("brand")));

        let ctx_approved = PermissionContext::new().set("asset_status", "approved");
        let decision = checker.check(
            &bob,
            &Action::download(),
            &ResourceScope::new("brand"),
            &ctx_approved,
        );
        assert!(decision.is_allowed());
        assert_eq!(
            decision,
            PermissionDecision::Allowed(PermissionSource::Conditional)
        );

        let ctx_draft = PermissionContext::new().set("asset_status", "draft");
        let decision = checker.check(
            &bob,
            &Action::download(),
            &ResourceScope::new("brand"),
            &ctx_draft,
        );
        assert!(decision.is_denied());
    }

    #[test]
    fn delegation_check() {
        let mut checker = setup_checker();

        // Alice delegates upload:brand to Charlie
        checker.delegations.grant(Delegation::new(
            "alice",
            "charlie",
            vec![Permission::from_strings("upload", "brand")],
        ));

        let charlie = ActorContext::new("charlie", TrustLayer::Connected, CollectiveRole::Readonly);

        let decision = checker.check(
            &charlie,
            &Action::upload(),
            &ResourceScope::new("brand"),
            &PermissionContext::new(),
        );
        assert!(decision.is_allowed());
        match decision {
            PermissionDecision::Allowed(PermissionSource::Delegation { delegator }) => {
                assert_eq!(delegator, "alice");
            }
            _ => panic!("expected delegation source"),
        }
    }

    #[test]
    fn check_priority_role_before_delegation() {
        let mut checker = setup_checker();

        // Alice delegates view:brand to bob
        checker.delegations.grant(Delegation::new(
            "alice",
            "bob",
            vec![Permission::from_strings("view", "brand")],
        ));

        // Bob also has Designer role
        let bob = ActorContext::new("bob", TrustLayer::Verified, CollectiveRole::Member)
            .with_role("Designer");

        // Role should be checked first
        let decision = checker.check(
            &bob,
            &Action::view(),
            &ResourceScope::new("brand"),
            &PermissionContext::new(),
        );
        assert_eq!(
            decision,
            PermissionDecision::Allowed(PermissionSource::Role("Designer".into()))
        );
    }

    #[test]
    fn no_permissions_denied() {
        let checker = setup_checker();

        let nobody = ActorContext::new("nobody", TrustLayer::Connected, CollectiveRole::Readonly);

        let decision = checker.check(
            &nobody,
            &Action::delete(),
            &ResourceScope::new("everything"),
            &PermissionContext::new(),
        );
        assert!(decision.is_denied());
        assert_eq!(
            decision,
            PermissionDecision::Denied(DenialReason::NoPermission)
        );
    }

    #[test]
    fn effective_permissions_from_roles() {
        let checker = setup_checker();

        let designer = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Member)
            .with_role("Designer");

        let effective = checker.effective_permissions(&designer);
        assert_eq!(effective.len(), 2); // upload:brand, view:brand
    }

    #[test]
    fn effective_permissions_from_delegations() {
        let mut checker = setup_checker();

        checker.delegations.grant(Delegation::new(
            "alice",
            "bob",
            vec![
                Permission::from_strings("view", "brand"),
                Permission::from_strings("download", "brand"),
            ],
        ));

        let bob = ActorContext::new("bob", TrustLayer::Connected, CollectiveRole::Readonly);
        let effective = checker.effective_permissions(&bob);
        assert_eq!(effective.len(), 2);
        assert!(effective.iter().all(|e| matches!(
            &e.source,
            PermissionSource::Delegation { delegator } if delegator == "alice"
        )));
    }

    #[test]
    fn effective_permissions_unqualified_role_excluded() {
        let checker = setup_checker();

        // Alice has Designer role but insufficient trust
        let alice = ActorContext::new("alice", TrustLayer::Connected, CollectiveRole::Member)
            .with_role("Designer");

        let effective = checker.effective_permissions(&alice);
        assert!(effective.is_empty()); // role prerequisites not met
    }

    #[test]
    fn role_would_allow_check() {
        let checker = setup_checker();

        assert!(checker.role_would_allow("Designer", &Action::upload(), &ResourceScope::new("brand")));
        assert!(!checker.role_would_allow("Designer", &Action::delete(), &ResourceScope::new("brand")));
        assert!(!checker.role_would_allow("Nonexistent", &Action::view(), &ResourceScope::new("brand")));
    }

    #[test]
    fn actor_context_builder() {
        let actor = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Member)
            .with_role("Designer")
            .with_role("Reviewer")
            .with_role("Designer"); // duplicate ignored

        assert_eq!(actor.assigned_roles, vec!["Designer", "Reviewer"]);
    }

    #[test]
    fn actor_context_with_multiple_roles() {
        let actor = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Member)
            .with_roles(vec!["Designer".into(), "Reviewer".into()]);

        assert_eq!(actor.assigned_roles.len(), 2);
    }

    #[test]
    fn real_world_scenario_brand_management() {
        let mut checker = PermissionChecker::new();

        // Register roles for a brand management app
        checker
            .register_role(
                Role::new("Designer", "Creates and uploads brand assets")
                    .with_permissions(vec![
                        Permission::from_strings("upload", "brand"),
                        Permission::from_strings("view", "brand"),
                        Permission::from_strings("edit", "brand.drafts"),
                    ])
                    .requiring_trust_layer(TrustLayer::Verified)
                    .requiring_collective_role(CollectiveRole::Member),
            )
            .unwrap();

        checker
            .register_role(
                Role::new("Reviewer", "Approves brand assets")
                    .with_permissions(vec![
                        Permission::from_strings("view", "brand"),
                        Permission::from_strings("approve", "brand"),
                    ])
                    .requiring_trust_layer(TrustLayer::Vouched)
                    .requiring_collective_role(CollectiveRole::Admin),
            )
            .unwrap();

        checker
            .register_role(
                Role::new("External Vendor", "Can view approved watermarked only")
                    .with_permission(Permission::from_strings("view", "brand.approved")),
            )
            .unwrap();

        // Conditional: external vendors can download IF watermark == true
        checker.add_conditional_permission(
            "vendor_acme",
            ConditionalPermission::new(Permission::from_strings("download", "brand.approved"))
                .with_condition(Condition::equals("watermark", "true")),
        );

        // Actors
        let designer = ActorContext::new("alice", TrustLayer::Verified, CollectiveRole::Member)
            .with_role("Designer");
        let reviewer = ActorContext::new("bob", TrustLayer::Vouched, CollectiveRole::Admin)
            .with_role("Reviewer");
        let vendor = ActorContext::new("vendor_acme", TrustLayer::Connected, CollectiveRole::Readonly)
            .with_role("External Vendor");

        // Designer can upload and view, but not approve
        assert!(checker.can(&designer, &Action::upload(), &ResourceScope::new("brand")));
        assert!(checker.can(&designer, &Action::view(), &ResourceScope::new("brand.logo")));
        assert!(!checker.can(&designer, &Action::approve(), &ResourceScope::new("brand")));

        // Reviewer can approve, but not upload
        assert!(checker.can(&reviewer, &Action::approve(), &ResourceScope::new("brand")));
        assert!(!checker.can(&reviewer, &Action::upload(), &ResourceScope::new("brand")));

        // Vendor can view approved, not the rest
        assert!(checker.can(&vendor, &Action::view(), &ResourceScope::new("brand.approved")));
        assert!(!checker.can(&vendor, &Action::view(), &ResourceScope::new("brand.drafts")));

        // Vendor can download approved IF watermarked
        let ctx_watermarked = PermissionContext::new().set("watermark", "true");
        assert!(checker.check(
            &vendor,
            &Action::download(),
            &ResourceScope::new("brand.approved"),
            &ctx_watermarked,
        ).is_allowed());

        let ctx_no_watermark = PermissionContext::new().set("watermark", "false");
        assert!(checker.check(
            &vendor,
            &Action::download(),
            &ResourceScope::new("brand.approved"),
            &ctx_no_watermark,
        ).is_denied());
    }
}

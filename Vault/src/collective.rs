use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VaultError;

/// Role within a collective. Higher values = more permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollectiveRole {
    /// Read-only access (level 1).
    Readonly = 1,
    /// Read/write access (level 2).
    Member = 2,
    /// Can add members (level 3).
    Admin = 3,
    /// Full control including removal and deletion (level 4).
    Owner = 4,
}

impl CollectiveRole {
    /// Check if this role has at least the given permission level.
    pub fn has_permission(&self, required: CollectiveRole) -> bool {
        *self >= required
    }
}

impl std::fmt::Display for CollectiveRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Readonly => write!(f, "readonly"),
            Self::Member => write!(f, "member"),
            Self::Admin => write!(f, "admin"),
            Self::Owner => write!(f, "owner"),
        }
    }
}

/// A member of a collective.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectiveMember {
    /// Member's public key (crown_id).
    pub public_key: String,
    /// When this member joined.
    pub joined_at: DateTime<Utc>,
    /// Member's role.
    pub role: CollectiveRole,
}

/// A shared space among multiple members.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collective {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub members: Vec<CollectiveMember>,
    /// Our role in this collective.
    pub our_role: CollectiveRole,
}

impl Collective {
    /// Create a new collective with the given owner.
    pub fn create(name: String, owner_public_key: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            created_at: now,
            members: vec![CollectiveMember {
                public_key: owner_public_key,
                joined_at: now,
                role: CollectiveRole::Owner,
            }],
            our_role: CollectiveRole::Owner,
        }
    }

    /// Add a member to this collective. Requires Admin or higher.
    pub fn add_member(
        &mut self,
        public_key: String,
        role: CollectiveRole,
    ) -> Result<(), VaultError> {
        if self.our_role < CollectiveRole::Admin {
            return Err(VaultError::InsufficientPermissions {
                current: self.our_role.to_string(),
                required: CollectiveRole::Admin.to_string(),
            });
        }
        self.members.push(CollectiveMember {
            public_key,
            joined_at: Utc::now(),
            role,
        });
        Ok(())
    }

    /// Remove a member by public key. Requires Owner.
    pub fn remove_member(&mut self, public_key: &str) -> Result<(), VaultError> {
        if self.our_role < CollectiveRole::Owner {
            return Err(VaultError::InsufficientPermissions {
                current: self.our_role.to_string(),
                required: CollectiveRole::Owner.to_string(),
            });
        }
        self.members.retain(|m| m.public_key != public_key);
        Ok(())
    }

    /// Check if a public key is a member.
    pub fn is_member(&self, public_key: &str) -> bool {
        self.members.iter().any(|m| m.public_key == public_key)
    }

    /// Get a member's role. Returns None if not a member.
    pub fn member_role(&self, public_key: &str) -> Option<CollectiveRole> {
        self.members
            .iter()
            .find(|m| m.public_key == public_key)
            .map(|m| m.role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_ordering() {
        assert!(CollectiveRole::Readonly < CollectiveRole::Member);
        assert!(CollectiveRole::Member < CollectiveRole::Admin);
        assert!(CollectiveRole::Admin < CollectiveRole::Owner);
    }

    #[test]
    fn has_permission_check() {
        assert!(CollectiveRole::Owner.has_permission(CollectiveRole::Admin));
        assert!(CollectiveRole::Admin.has_permission(CollectiveRole::Admin));
        assert!(!CollectiveRole::Member.has_permission(CollectiveRole::Admin));
        assert!(!CollectiveRole::Readonly.has_permission(CollectiveRole::Member));
    }

    #[test]
    fn create_collective() {
        let coll = Collective::create("Test Group".to_string(), "cpub1owner".to_string());
        assert_eq!(coll.name, "Test Group");
        assert_eq!(coll.our_role, CollectiveRole::Owner);
        assert_eq!(coll.members.len(), 1);
        assert_eq!(coll.members[0].public_key, "cpub1owner");
        assert_eq!(coll.members[0].role, CollectiveRole::Owner);
    }

    #[test]
    fn add_member_as_admin() {
        let mut coll = Collective::create("Test".to_string(), "cpub1owner".to_string());

        // Owner can add (owner >= admin).
        coll.add_member("cpub1friend".to_string(), CollectiveRole::Member)
            .unwrap();
        assert_eq!(coll.members.len(), 2);

        // Simulate being a member (not admin) — should fail.
        coll.our_role = CollectiveRole::Member;
        let result = coll.add_member("cpub1other".to_string(), CollectiveRole::Readonly);
        assert!(matches!(result, Err(VaultError::InsufficientPermissions { .. })));
    }

    #[test]
    fn remove_member_as_owner() {
        let mut coll = Collective::create("Test".to_string(), "cpub1owner".to_string());
        coll.add_member("cpub1friend".to_string(), CollectiveRole::Member)
            .unwrap();
        assert_eq!(coll.members.len(), 2);

        coll.remove_member("cpub1friend").unwrap();
        assert_eq!(coll.members.len(), 1);
    }

    #[test]
    fn remove_member_as_admin_fails() {
        let mut coll = Collective::create("Test".to_string(), "cpub1owner".to_string());
        coll.add_member("cpub1friend".to_string(), CollectiveRole::Member)
            .unwrap();

        coll.our_role = CollectiveRole::Admin;
        let result = coll.remove_member("cpub1friend");
        assert!(matches!(result, Err(VaultError::InsufficientPermissions { .. })));
    }

    #[test]
    fn collective_serde_round_trip() {
        let coll = Collective::create("Serde Test".to_string(), "cpub1test".to_string());
        let json = serde_json::to_string(&coll).unwrap();
        let restored: Collective = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, coll.id);
        assert_eq!(restored.name, coll.name);
        assert_eq!(restored.members.len(), 1);
    }
}

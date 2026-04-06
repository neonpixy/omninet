use serde::{Deserialize, Serialize};

/// Configuration for the Kids Sphere — what children can see and who they can contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KidsSphereConfig {
    pub allowed_contact_types: Vec<AllowedContactType>,
    pub allowed_content_types: Vec<AllowedContentType>,
}

impl Default for KidsSphereConfig {
    fn default() -> Self {
        Self {
            allowed_contact_types: vec![
                AllowedContactType::Family,
                AllowedContactType::Vouchers,
                AllowedContactType::VerifiedKids,
            ],
            allowed_content_types: vec![
                AllowedContentType::KidCollectives,
                AllowedContentType::FamilyContent,
            ],
        }
    }
}

/// Who children can contact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AllowedContactType {
    /// Parents, siblings — always allowed.
    Family,
    /// Adults who vouched for the child.
    Vouchers,
    /// Other verified children in Kids Sphere.
    VerifiedKids,
    /// Adults explicitly approved by parent.
    ApprovedAdults,
}

/// What content children can access.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AllowedContentType {
    /// Collectives marked as kid-safe.
    KidCollectives,
    /// Content shared by family members.
    FamilyContent,
    /// Content from vouchers (filtered).
    VoucherContent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = KidsSphereConfig::default();
        assert_eq!(config.allowed_contact_types.len(), 3);
        assert_eq!(config.allowed_content_types.len(), 2);
        assert!(config.allowed_contact_types.contains(&AllowedContactType::Family));
        assert!(!config.allowed_contact_types.contains(&AllowedContactType::ApprovedAdults));
    }
}

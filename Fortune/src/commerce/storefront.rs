use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A seller's storefront — the business entity behind product listings.
///
/// From Consortium Art. 2 §2: "Fair compensation, consent-based agreements,
/// and transparency in value flows shall be required."
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Storefront {
    pub id: Uuid,
    pub owner_pubkey: String,
    pub name: String,
    pub description: String,
    /// Regalia Reign theme reference.
    pub theme_ref: Option<String>,
    /// .idea reference to the product catalog.
    pub catalog_ref: Option<String>,
    pub policies: StorefrontPolicies,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub active: bool,
}

impl Storefront {
    /// Create a new storefront.
    pub fn new(
        owner_pubkey: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            owner_pubkey: owner_pubkey.into(),
            name: name.into(),
            description: description.into(),
            theme_ref: None,
            catalog_ref: None,
            policies: StorefrontPolicies::default(),
            created_at: now,
            updated_at: now,
            active: true,
        }
    }
}

/// Storefront policies — shipping, returns, privacy, terms.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StorefrontPolicies {
    pub shipping_policy: Option<String>,
    pub return_policy: Option<String>,
    pub privacy_policy: Option<String>,
    pub terms_of_service: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_storefront() {
        let store = Storefront::new("cpub1owner", "Artisan Goods", "Handcrafted items");
        assert_eq!(store.owner_pubkey, "cpub1owner");
        assert_eq!(store.name, "Artisan Goods");
        assert_eq!(store.description, "Handcrafted items");
        assert!(store.active);
        assert!(store.theme_ref.is_none());
        assert!(store.catalog_ref.is_none());
        assert!(store.policies.shipping_policy.is_none());
    }

    #[test]
    fn storefront_with_policies() {
        let mut store = Storefront::new("cpub1owner", "My Shop", "A shop");
        store.policies = StorefrontPolicies {
            shipping_policy: Some("Free shipping over 50 Cool".into()),
            return_policy: Some("30-day returns".into()),
            privacy_policy: None,
            terms_of_service: None,
        };

        assert_eq!(
            store.policies.shipping_policy.as_deref(),
            Some("Free shipping over 50 Cool")
        );
        assert_eq!(
            store.policies.return_policy.as_deref(),
            Some("30-day returns")
        );
    }

    #[test]
    fn storefront_serde_round_trip() {
        let mut store = Storefront::new("cpub1owner", "Artisan Goods", "Handcrafted");
        store.theme_ref = Some("reign-dark".into());
        store.catalog_ref = Some("idea-catalog-001".into());

        let json = serde_json::to_string(&store).unwrap();
        let deserialized: Storefront = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, store.id);
        assert_eq!(deserialized.name, "Artisan Goods");
        assert_eq!(deserialized.theme_ref.as_deref(), Some("reign-dark"));
        assert_eq!(
            deserialized.catalog_ref.as_deref(),
            Some("idea-catalog-001")
        );
    }
}

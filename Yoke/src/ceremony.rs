use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Types of ceremonies the Covenant recognizes.
///
/// These are the moments that matter — not just data, but meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CeremonyType {
    /// Voluntary entry into the Covenant
    CovenantOath,
    /// New community formed
    CommunityFormation,
    /// Union formed (marriage, partnership, guild, etc.)
    UnionFormation,
    /// Charter amended
    CharterAmendment,
    /// Community or union dissolved
    Dissolution,
    /// Leadership transition
    LeadershipTransition,
    /// Constitutional review completed
    ConstitutionalReview,
    /// Two communities formally entering federation
    FederationCeremony,
    /// A community formally withdrawing from federation
    DefederationCeremony,
    /// App-defined ceremony
    Custom(String),
}

/// A participant in a ceremony.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CeremonyParticipant {
    pub crown_id: String,
    pub role: ParticipantRole,
}

/// Role in a ceremony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantRole {
    /// The person taking the oath or initiating
    Principal,
    /// Someone witnessing the ceremony
    Witness,
    /// Someone officiating (community elder, etc.)
    Officiant,
    /// App-defined role
    Custom(String),
}

/// A recorded ceremony — a moment that matters.
///
/// Ceremonies are not governance (that's Kingdom). They're the human moments:
/// oaths taken, communities born, unions formed, transitions honored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CeremonyRecord {
    pub id: Uuid,
    pub ceremony_type: CeremonyType,
    pub participants: Vec<CeremonyParticipant>,
    pub community_id: Option<String>,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub related_events: Vec<String>,
}

impl CeremonyRecord {
    pub fn new(ceremony_type: CeremonyType) -> Self {
        Self {
            id: Uuid::new_v4(),
            ceremony_type,
            participants: Vec::new(),
            community_id: None,
            content: None,
            created_at: Utc::now(),
            related_events: Vec::new(),
        }
    }

    pub fn with_principal(mut self, crown_id: impl Into<String>) -> Self {
        self.participants.push(CeremonyParticipant {
            crown_id: crown_id.into(),
            role: ParticipantRole::Principal,
        });
        self
    }

    pub fn with_witness(mut self, crown_id: impl Into<String>) -> Self {
        self.participants.push(CeremonyParticipant {
            crown_id: crown_id.into(),
            role: ParticipantRole::Witness,
        });
        self
    }

    pub fn with_officiant(mut self, crown_id: impl Into<String>) -> Self {
        self.participants.push(CeremonyParticipant {
            crown_id: crown_id.into(),
            role: ParticipantRole::Officiant,
        });
        self
    }

    pub fn with_participant(mut self, crown_id: impl Into<String>, role: ParticipantRole) -> Self {
        self.participants.push(CeremonyParticipant {
            crown_id: crown_id.into(),
            role,
        });
        self
    }

    pub fn in_community(mut self, community_id: impl Into<String>) -> Self {
        self.community_id = Some(community_id.into());
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_related_event(mut self, event_id: impl Into<String>) -> Self {
        self.related_events.push(event_id.into());
        self
    }

    /// Validate that the ceremony has the required structure.
    ///
    /// Rules:
    /// - CovenantOath: at least one principal required
    /// - CommunityFormation: at least one principal + community_id required
    /// - UnionFormation: at least two principals required
    /// - Dissolution: at least one principal + community_id required
    /// - LeadershipTransition: at least one principal + community_id required
    /// - CharterAmendment: community_id required
    /// - ConstitutionalReview: community_id required
    /// - Custom: no structural requirements
    pub fn validate(&self) -> Result<(), crate::error::YokeError> {
        let principals = self.principals();
        match &self.ceremony_type {
            CeremonyType::CovenantOath => {
                if principals.is_empty() {
                    return Err(crate::error::YokeError::Validation(
                        "CovenantOath requires at least one principal".into(),
                    ));
                }
            }
            CeremonyType::CommunityFormation => {
                if principals.is_empty() {
                    return Err(crate::error::YokeError::Validation(
                        "CommunityFormation requires at least one principal (founder)".into(),
                    ));
                }
                if self.community_id.is_none() {
                    return Err(crate::error::YokeError::Validation(
                        "CommunityFormation requires a community_id".into(),
                    ));
                }
            }
            CeremonyType::UnionFormation => {
                if principals.len() < 2 {
                    return Err(crate::error::YokeError::Validation(
                        "UnionFormation requires at least two principals".into(),
                    ));
                }
            }
            CeremonyType::Dissolution => {
                if principals.is_empty() {
                    return Err(crate::error::YokeError::Validation(
                        "Dissolution requires at least one principal".into(),
                    ));
                }
                if self.community_id.is_none() {
                    return Err(crate::error::YokeError::Validation(
                        "Dissolution requires a community_id".into(),
                    ));
                }
            }
            CeremonyType::LeadershipTransition => {
                if principals.is_empty() {
                    return Err(crate::error::YokeError::Validation(
                        "LeadershipTransition requires at least one principal".into(),
                    ));
                }
                if self.community_id.is_none() {
                    return Err(crate::error::YokeError::Validation(
                        "LeadershipTransition requires a community_id".into(),
                    ));
                }
            }
            CeremonyType::CharterAmendment | CeremonyType::ConstitutionalReview => {
                if self.community_id.is_none() {
                    return Err(crate::error::YokeError::Validation(
                        "CharterAmendment/ConstitutionalReview requires a community_id".into(),
                    ));
                }
            }
            CeremonyType::FederationCeremony => {
                if principals.len() < 2 {
                    return Err(crate::error::YokeError::Validation(
                        "FederationCeremony requires at least two principals (one from each community)".into(),
                    ));
                }
                if self.community_id.is_none() {
                    return Err(crate::error::YokeError::Validation(
                        "FederationCeremony requires a community_id".into(),
                    ));
                }
            }
            CeremonyType::DefederationCeremony => {
                if principals.is_empty() {
                    return Err(crate::error::YokeError::Validation(
                        "DefederationCeremony requires at least one principal".into(),
                    ));
                }
                if self.community_id.is_none() {
                    return Err(crate::error::YokeError::Validation(
                        "DefederationCeremony requires a community_id".into(),
                    ));
                }
            }
            CeremonyType::Custom(_) => {}
        }
        Ok(())
    }

    /// Get all principals in this ceremony.
    pub fn principals(&self) -> Vec<&str> {
        self.participants
            .iter()
            .filter(|p| p.role == ParticipantRole::Principal)
            .map(|p| p.crown_id.as_str())
            .collect()
    }

    /// Get all witnesses in this ceremony.
    pub fn witnesses(&self) -> Vec<&str> {
        self.participants
            .iter()
            .filter(|p| p.role == ParticipantRole::Witness)
            .map(|p| p.crown_id.as_str())
            .collect()
    }

    /// Get all officiants in this ceremony.
    pub fn officiants(&self) -> Vec<&str> {
        self.participants
            .iter()
            .filter(|p| p.role == ParticipantRole::Officiant)
            .map(|p| p.crown_id.as_str())
            .collect()
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covenant_oath() {
        let ceremony = CeremonyRecord::new(CeremonyType::CovenantOath)
            .with_principal("cpub1alice")
            .with_witness("cpub1bob")
            .with_witness("cpub1carol")
            .with_content("I enter the Covenant freely and with understanding.");

        assert_eq!(ceremony.ceremony_type, CeremonyType::CovenantOath);
        assert_eq!(ceremony.principals(), vec!["cpub1alice"]);
        assert_eq!(ceremony.witnesses().len(), 2);
        assert_eq!(ceremony.participant_count(), 3);
        assert!(ceremony.content.is_some());
    }

    #[test]
    fn community_formation() {
        let ceremony = CeremonyRecord::new(CeremonyType::CommunityFormation)
            .with_principal("cpub1founder")
            .with_officiant("cpub1elder")
            .with_witness("cpub1witness1")
            .with_witness("cpub1witness2")
            .in_community("design-guild")
            .with_related_event("charter-event-id");

        assert_eq!(ceremony.ceremony_type, CeremonyType::CommunityFormation);
        assert_eq!(ceremony.community_id.as_deref(), Some("design-guild"));
        assert_eq!(ceremony.officiants(), vec!["cpub1elder"]);
        assert_eq!(ceremony.related_events.len(), 1);
    }

    #[test]
    fn union_formation() {
        let ceremony = CeremonyRecord::new(CeremonyType::UnionFormation)
            .with_principal("cpub1partner1")
            .with_principal("cpub1partner2")
            .with_officiant("cpub1officiant")
            .with_witness("cpub1family1")
            .with_witness("cpub1family2")
            .with_witness("cpub1friend");

        assert_eq!(ceremony.principals().len(), 2);
        assert_eq!(ceremony.witnesses().len(), 3);
        assert_eq!(ceremony.officiants().len(), 1);
        assert_eq!(ceremony.participant_count(), 6);
    }

    #[test]
    fn dissolution() {
        let ceremony = CeremonyRecord::new(CeremonyType::Dissolution)
            .with_principal("cpub1leader")
            .in_community("old-guild")
            .with_content("The guild dissolves with gratitude for what was shared.");

        assert_eq!(ceremony.ceremony_type, CeremonyType::Dissolution);
        assert_eq!(ceremony.community_id.as_deref(), Some("old-guild"));
    }

    #[test]
    fn custom_ceremony() {
        let ceremony = CeremonyRecord::new(CeremonyType::Custom("graduation".into()))
            .with_principal("cpub1student")
            .with_participant("cpub1mentor", ParticipantRole::Custom("mentor".into()));

        assert_eq!(
            ceremony.ceremony_type,
            CeremonyType::Custom("graduation".into())
        );
        assert_eq!(ceremony.participant_count(), 2);
    }

    #[test]
    fn leadership_transition() {
        let ceremony = CeremonyRecord::new(CeremonyType::LeadershipTransition)
            .with_participant("cpub1outgoing", ParticipantRole::Custom("outgoing".into()))
            .with_participant("cpub1incoming", ParticipantRole::Principal)
            .with_witness("cpub1community-member")
            .in_community("the-guild");

        assert_eq!(ceremony.participant_count(), 3);
        assert_eq!(ceremony.principals(), vec!["cpub1incoming"]);
    }

    #[test]
    fn serde_round_trip() {
        let ceremony = CeremonyRecord::new(CeremonyType::CovenantOath)
            .with_principal("cpub1alice")
            .with_witness("cpub1bob")
            .with_content("oath text")
            .with_related_event("event-1");

        let json = serde_json::to_string(&ceremony).unwrap();
        let restored: CeremonyRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.ceremony_type, CeremonyType::CovenantOath);
        assert_eq!(restored.participant_count(), 2);
        assert_eq!(restored.related_events.len(), 1);
    }

    #[test]
    fn validate_covenant_oath_requires_principal() {
        let c = CeremonyRecord::new(CeremonyType::CovenantOath);
        assert!(c.validate().is_err());
        let c = c.with_principal("cpub1alice");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_community_formation_requires_principal_and_community() {
        let c = CeremonyRecord::new(CeremonyType::CommunityFormation);
        assert!(c.validate().is_err());
        let c = c.with_principal("cpub1founder");
        assert!(c.validate().is_err()); // still no community
        let c = c.in_community("guild");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_union_requires_two_principals() {
        let c = CeremonyRecord::new(CeremonyType::UnionFormation)
            .with_principal("cpub1a");
        assert!(c.validate().is_err());
        let c = c.with_principal("cpub1b");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_dissolution_requires_principal_and_community() {
        let c = CeremonyRecord::new(CeremonyType::Dissolution)
            .with_principal("cpub1leader");
        assert!(c.validate().is_err());
        let c = c.in_community("old-guild");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_leadership_transition_requires_principal_and_community() {
        let c = CeremonyRecord::new(CeremonyType::LeadershipTransition);
        assert!(c.validate().is_err());
        let c = c.with_principal("cpub1incoming").in_community("guild");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_charter_amendment_requires_community() {
        let c = CeremonyRecord::new(CeremonyType::CharterAmendment);
        assert!(c.validate().is_err());
        let c = c.in_community("guild");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_constitutional_review_requires_community() {
        let c = CeremonyRecord::new(CeremonyType::ConstitutionalReview);
        assert!(c.validate().is_err());
        let c = c.in_community("polity");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_custom_always_valid() {
        let c = CeremonyRecord::new(CeremonyType::Custom("party".into()));
        assert!(c.validate().is_ok());
    }

    #[test]
    fn federation_ceremony() {
        let ceremony = CeremonyRecord::new(CeremonyType::FederationCeremony)
            .with_principal("cpub1community_a_rep")
            .with_principal("cpub1community_b_rep")
            .with_witness("cpub1observer")
            .in_community("community-a")
            .with_related_event("federation-agreement-id")
            .with_content("Communities A and B enter federation.");

        assert_eq!(ceremony.ceremony_type, CeremonyType::FederationCeremony);
        assert_eq!(ceremony.principals().len(), 2);
        assert!(ceremony.validate().is_ok());
    }

    #[test]
    fn defederation_ceremony() {
        let ceremony = CeremonyRecord::new(CeremonyType::DefederationCeremony)
            .with_principal("cpub1withdrawing_rep")
            .in_community("community-a")
            .with_content("Community A withdraws from federation with Community B.");

        assert_eq!(ceremony.ceremony_type, CeremonyType::DefederationCeremony);
        assert!(ceremony.validate().is_ok());
    }

    #[test]
    fn validate_federation_ceremony_requires_two_principals_and_community() {
        let c = CeremonyRecord::new(CeremonyType::FederationCeremony);
        assert!(c.validate().is_err());
        let c = c.with_principal("cpub1a");
        assert!(c.validate().is_err()); // only 1 principal
        let c = c.with_principal("cpub1b");
        assert!(c.validate().is_err()); // no community
        let c = c.in_community("community-a");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_defederation_ceremony_requires_principal_and_community() {
        let c = CeremonyRecord::new(CeremonyType::DefederationCeremony);
        assert!(c.validate().is_err());
        let c = c.with_principal("cpub1rep");
        assert!(c.validate().is_err()); // no community
        let c = c.in_community("community-a");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn federation_ceremony_serde() {
        let ceremony = CeremonyRecord::new(CeremonyType::FederationCeremony)
            .with_principal("cpub1a")
            .with_principal("cpub1b")
            .in_community("guild");
        let json = serde_json::to_string(&ceremony).unwrap();
        let restored: CeremonyRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.ceremony_type, CeremonyType::FederationCeremony);
        assert_eq!(restored.participant_count(), 2);
    }

    #[test]
    fn constitutional_review() {
        let ceremony = CeremonyRecord::new(CeremonyType::ConstitutionalReview)
            .with_officiant("cpub1reviewer")
            .in_community("polity-council")
            .with_related_event("review-proposal")
            .with_related_event("amendment-event");

        assert_eq!(ceremony.related_events.len(), 2);
        assert_eq!(ceremony.officiants(), vec!["cpub1reviewer"]);
    }
}

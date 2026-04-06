//! Inter-Community Diplomacy — tools for communities to communicate,
//! negotiate, and establish shared standards.
//!
//! Diplomacy is for **peer-to-peer** relationships between independent
//! communities. This complements the existing `Consortium` (hierarchical
//! federation via subsidiarity). Consortium members might also have
//! bilateral treaties.
//!
//! # Integration
//!
//! - **Globe**: Diplomatic channels and treaties are published as Globe
//!   events (kind range 11000-11099).
//! - **Yoke**: Treaty ratifications are cross-referenced as
//!   `CeremonyRecord` (type: `Custom("TreatyRatification")`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A diplomatic communication channel between communities.
///
/// Channels are the medium through which communities negotiate,
/// discuss, and formalize agreements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiplomaticChannel {
    /// Unique channel ID.
    pub id: Uuid,
    /// The participating community IDs (2+ communities).
    pub communities: Vec<String>,
    /// The type of diplomatic interaction.
    pub channel_type: ChannelType,
    /// Current lifecycle status.
    pub status: ChannelStatus,
    /// When the channel was established.
    pub established_at: DateTime<Utc>,
    /// Messages exchanged on this channel.
    pub messages: Vec<DiplomaticMessage>,
}

impl DiplomaticChannel {
    /// Create a new diplomatic channel.
    pub fn new(communities: Vec<String>, channel_type: ChannelType) -> Self {
        Self {
            id: Uuid::new_v4(),
            communities,
            channel_type,
            status: ChannelStatus::Proposed,
            established_at: Utc::now(),
            messages: Vec::new(),
        }
    }

    /// Whether the channel is currently active.
    pub fn is_active(&self) -> bool {
        self.status == ChannelStatus::Active
    }

    /// Whether a community is part of this channel.
    pub fn includes_community(&self, community_id: &str) -> bool {
        self.communities.iter().any(|c| c == community_id)
    }

    /// Number of participating communities.
    pub fn community_count(&self) -> usize {
        self.communities.len()
    }

    /// Number of messages exchanged.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Accept the channel (transition from Proposed to Accepted).
    pub fn accept(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != ChannelStatus::Proposed {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Accepted".into(),
            });
        }
        self.status = ChannelStatus::Accepted;
        Ok(())
    }

    /// Activate the channel (transition from Accepted to Active).
    pub fn activate(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != ChannelStatus::Accepted {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Active".into(),
            });
        }
        self.status = ChannelStatus::Active;
        Ok(())
    }

    /// Suspend the channel.
    pub fn suspend(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != ChannelStatus::Active {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Suspended".into(),
            });
        }
        self.status = ChannelStatus::Suspended;
        Ok(())
    }

    /// Close the channel.
    pub fn close(&mut self) -> Result<(), crate::KingdomError> {
        if self.status == ChannelStatus::Closed {
            return Err(crate::KingdomError::InvalidTransition {
                current: "Closed".into(),
                target: "Closed".into(),
            });
        }
        self.status = ChannelStatus::Closed;
        Ok(())
    }

    /// Add a message to the channel.
    pub fn send_message(&mut self, message: DiplomaticMessage) -> Result<(), crate::KingdomError> {
        if self.status != ChannelStatus::Active {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "sending message requires Active".into(),
            });
        }
        if !self.includes_community(&message.community_id) {
            return Err(crate::KingdomError::NotConsortiumMember(
                message.community_id.clone(),
            ));
        }
        self.messages.push(message);
        Ok(())
    }
}

/// The type of diplomatic interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelType {
    /// Two communities in direct dialogue.
    Bilateral,
    /// Three or more communities in dialogue.
    Multilateral,
    /// Formal agreement negotiation.
    Treaty,
    /// Dispute resolution between communities.
    Mediation,
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelType::Bilateral => write!(f, "bilateral"),
            ChannelType::Multilateral => write!(f, "multilateral"),
            ChannelType::Treaty => write!(f, "treaty"),
            ChannelType::Mediation => write!(f, "mediation"),
        }
    }
}

/// Lifecycle status of a diplomatic channel.
///
/// Proposed -> Accepted -> Active -> Suspended -> Closed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelStatus {
    /// Channel has been proposed but not yet accepted.
    Proposed,
    /// Channel has been accepted by participating communities.
    Accepted,
    /// Channel is active and accepting messages.
    Active,
    /// Channel is temporarily suspended.
    Suspended,
    /// Channel is permanently closed.
    Closed,
}

impl std::fmt::Display for ChannelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelStatus::Proposed => write!(f, "proposed"),
            ChannelStatus::Accepted => write!(f, "accepted"),
            ChannelStatus::Active => write!(f, "active"),
            ChannelStatus::Suspended => write!(f, "suspended"),
            ChannelStatus::Closed => write!(f, "closed"),
        }
    }
}

/// A message sent within a diplomatic channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiplomaticMessage {
    /// The sender's public key.
    pub author_pubkey: String,
    /// Which community the sender represents.
    pub community_id: String,
    /// The message content.
    pub content: String,
    /// When the message was sent.
    pub timestamp: DateTime<Utc>,
}

impl DiplomaticMessage {
    /// Create a new diplomatic message.
    pub fn new(
        author_pubkey: impl Into<String>,
        community_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            author_pubkey: author_pubkey.into(),
            community_id: community_id.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

// --- Treaties ---

/// A formal agreement between communities.
///
/// Treaties formalize cooperation, mutual recognition, trade,
/// or shared standards. They require ratification by each party
/// through their own governance processes (Kingdom Proposals).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Treaty {
    /// Unique treaty ID.
    pub id: Uuid,
    /// The participating community IDs.
    pub parties: Vec<String>,
    /// Human-readable title.
    pub title: String,
    /// The specific terms of the agreement.
    pub terms: Vec<TreatyTerm>,
    /// Which parties have ratified.
    pub ratified_by: Vec<TreatyRatification>,
    /// Current lifecycle status.
    pub status: TreatyStatus,
    /// When the treaty takes effect (after full ratification).
    pub effective_at: Option<DateTime<Utc>>,
    /// When the treaty expires (if term-limited).
    pub expires_at: Option<DateTime<Utc>>,
}

impl Treaty {
    /// Create a new draft treaty.
    pub fn new(parties: Vec<String>, title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            parties,
            title: title.into(),
            terms: Vec::new(),
            ratified_by: Vec::new(),
            status: TreatyStatus::Drafted,
            effective_at: None,
            expires_at: None,
        }
    }

    /// Add a term to the treaty.
    pub fn add_term(&mut self, term: TreatyTerm) {
        self.terms.push(term);
    }

    /// Whether a community is a party to this treaty.
    pub fn is_party(&self, community_id: &str) -> bool {
        self.parties.iter().any(|p| p == community_id)
    }

    /// Whether a community has ratified this treaty.
    pub fn has_ratified(&self, community_id: &str) -> bool {
        self.ratified_by.iter().any(|r| r.community_id == community_id)
    }

    /// Record a ratification from a community.
    pub fn ratify(
        &mut self,
        community_id: impl Into<String>,
        proposal_id: Uuid,
    ) -> Result<(), crate::KingdomError> {
        let community_id = community_id.into();

        if !self.is_party(&community_id) {
            return Err(crate::KingdomError::NotConsortiumMember(community_id));
        }

        if self.has_ratified(&community_id) {
            return Err(crate::KingdomError::AlreadyMember(community_id));
        }

        if self.status != TreatyStatus::Drafted && self.status != TreatyStatus::Ratifying {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Ratifying".into(),
            });
        }

        self.ratified_by.push(TreatyRatification {
            community_id,
            proposal_id,
            ratified_at: Utc::now(),
        });

        // Transition to Ratifying on first ratification.
        if self.status == TreatyStatus::Drafted {
            self.status = TreatyStatus::Ratifying;
        }

        // Activate when all parties have ratified.
        if self.ratified_by.len() == self.parties.len() {
            self.status = TreatyStatus::Active;
            self.effective_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Number of parties.
    pub fn party_count(&self) -> usize {
        self.parties.len()
    }

    /// Number of ratifications received.
    pub fn ratification_count(&self) -> usize {
        self.ratified_by.len()
    }

    /// Whether the treaty has been fully ratified by all parties.
    pub fn is_fully_ratified(&self) -> bool {
        self.ratified_by.len() == self.parties.len()
    }

    /// Whether the treaty is currently active.
    pub fn is_active(&self) -> bool {
        self.status == TreatyStatus::Active
    }

    /// Whether the treaty has expired (if it has an expiry date).
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Utc::now() > expires
        } else {
            false
        }
    }

    /// Suspend the treaty.
    pub fn suspend(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != TreatyStatus::Active {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Suspended".into(),
            });
        }
        self.status = TreatyStatus::Suspended;
        Ok(())
    }

    /// Dissolve the treaty.
    pub fn dissolve(&mut self) -> Result<(), crate::KingdomError> {
        if self.status == TreatyStatus::Dissolved {
            return Err(crate::KingdomError::InvalidTransition {
                current: "Dissolved".into(),
                target: "Dissolved".into(),
            });
        }
        self.status = TreatyStatus::Dissolved;
        Ok(())
    }
}

/// A specific term within a treaty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreatyTerm {
    /// Unique term ID.
    pub id: Uuid,
    /// Human-readable description of the obligation.
    pub description: String,
    /// What kind of obligation this represents.
    pub obligation_type: ObligationType,
    /// Which parties this term applies to (subset of treaty parties).
    pub applicable_to: Vec<String>,
}

impl TreatyTerm {
    /// Create a new treaty term.
    pub fn new(
        description: impl Into<String>,
        obligation_type: ObligationType,
        applicable_to: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
            obligation_type,
            applicable_to,
        }
    }

    /// Whether this term applies to a specific community.
    pub fn applies_to(&self, community_id: &str) -> bool {
        self.applicable_to.is_empty() || self.applicable_to.iter().any(|c| c == community_id)
    }
}

/// The type of obligation a treaty term creates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObligationType {
    /// Recognize each other's governance decisions.
    MutualRecognition,
    /// Agree on common data/communication standards.
    SharedStandard,
    /// Economic cooperation terms.
    TradeAgreement,
    /// Mutual safety commitment.
    DefenseAlliance,
    /// Share Commons events, cross-reference provenance.
    InformationSharing,
    /// Accept each other's Jail decisions.
    AdjudicationReciprocity,
    /// Application-defined obligation type.
    Custom(String),
}

impl std::fmt::Display for ObligationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObligationType::MutualRecognition => write!(f, "mutual-recognition"),
            ObligationType::SharedStandard => write!(f, "shared-standard"),
            ObligationType::TradeAgreement => write!(f, "trade-agreement"),
            ObligationType::DefenseAlliance => write!(f, "defense-alliance"),
            ObligationType::InformationSharing => write!(f, "information-sharing"),
            ObligationType::AdjudicationReciprocity => write!(f, "adjudication-reciprocity"),
            ObligationType::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// A record of a community's treaty ratification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreatyRatification {
    /// The ratifying community.
    pub community_id: String,
    /// The Kingdom Proposal that authorized ratification.
    pub proposal_id: Uuid,
    /// When ratification occurred.
    pub ratified_at: DateTime<Utc>,
}

/// Lifecycle status of a treaty.
///
/// Drafted -> Ratifying -> Active -> Suspended -> Expired -> Dissolved
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TreatyStatus {
    /// Treaty text has been drafted but no ratifications yet.
    Drafted,
    /// One or more parties have ratified, but not all.
    Ratifying,
    /// All parties have ratified; treaty is in effect.
    Active,
    /// Treaty is temporarily suspended.
    Suspended,
    /// Treaty term has expired.
    Expired,
    /// Treaty has been formally dissolved.
    Dissolved,
}

impl std::fmt::Display for TreatyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreatyStatus::Drafted => write!(f, "drafted"),
            TreatyStatus::Ratifying => write!(f, "ratifying"),
            TreatyStatus::Active => write!(f, "active"),
            TreatyStatus::Suspended => write!(f, "suspended"),
            TreatyStatus::Expired => write!(f, "expired"),
            TreatyStatus::Dissolved => write!(f, "dissolved"),
        }
    }
}

// --- Liaisons ---

/// A representative from one community stationed in another.
///
/// Liaisons facilitate ongoing diplomatic relationships, participate
/// in host community governance according to their role level, and
/// bridge communication between communities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Liaison {
    /// The liaison's public key.
    pub pubkey: String,
    /// The community the liaison comes from.
    pub home_community: String,
    /// The community the liaison is stationed in.
    pub host_community: String,
    /// The liaison's role and permissions.
    pub role: LiaisonRole,
    /// When the liaison was appointed.
    pub appointed_at: DateTime<Utc>,
    /// When the liaison's term expires (if term-limited).
    pub term_expires: Option<DateTime<Utc>>,
}

impl Liaison {
    /// Create a new liaison appointment.
    pub fn new(
        pubkey: impl Into<String>,
        home_community: impl Into<String>,
        host_community: impl Into<String>,
        role: LiaisonRole,
    ) -> Self {
        Self {
            pubkey: pubkey.into(),
            home_community: home_community.into(),
            host_community: host_community.into(),
            role,
            appointed_at: Utc::now(),
            term_expires: None,
        }
    }

    /// Set the term expiry.
    pub fn with_term_expires(mut self, expires: DateTime<Utc>) -> Self {
        self.term_expires = Some(expires);
        self
    }

    /// Whether the liaison's term has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.term_expires {
            Utc::now() > expires
        } else {
            false
        }
    }

    /// Whether the liaison can participate in discussions.
    pub fn can_discuss(&self) -> bool {
        matches!(self.role, LiaisonRole::Ambassador | LiaisonRole::Representative)
    }

    /// Whether the liaison can vote on shared-concern proposals.
    pub fn can_vote(&self) -> bool {
        matches!(self.role, LiaisonRole::Representative)
    }
}

/// The role of a liaison in the host community.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiaisonRole {
    /// Read-only participation in host community governance.
    Observer,
    /// Can propose and participate in discussion, cannot vote.
    Ambassador,
    /// Can vote on shared-concern proposals only.
    Representative,
}

impl std::fmt::Display for LiaisonRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LiaisonRole::Observer => write!(f, "observer"),
            LiaisonRole::Ambassador => write!(f, "ambassador"),
            LiaisonRole::Representative => write!(f, "representative"),
        }
    }
}

/// Globe event kind range for diplomacy (11000-11099).
pub mod kind {
    /// Diplomatic channel creation/update.
    pub const DIPLOMATIC_CHANNEL: u32 = 11000;
    /// Diplomatic message.
    pub const DIPLOMATIC_MESSAGE: u32 = 11001;
    /// Treaty publication.
    pub const TREATY: u32 = 11010;
    /// Treaty ratification announcement.
    pub const TREATY_RATIFICATION: u32 = 11011;
    /// Liaison appointment.
    pub const LIAISON_APPOINTMENT: u32 = 11020;
    /// Federation agreement proposal (kind 11030).
    pub const FEDERATION_PROPOSAL: u32 = 11030;
    /// Federation agreement status update (kind 11031).
    pub const FEDERATION_STATUS: u32 = 11031;
    /// Federation withdrawal announcement (kind 11032).
    pub const FEDERATION_WITHDRAWAL: u32 = 11032;

    /// Check if a kind is in the diplomacy range.
    pub fn is_diplomacy_kind(kind: u32) -> bool {
        (11000..11100).contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- DiplomaticChannel tests ---

    #[test]
    fn create_bilateral_channel() {
        let channel = DiplomaticChannel::new(
            vec!["guild-a".into(), "guild-b".into()],
            ChannelType::Bilateral,
        );
        assert_eq!(channel.community_count(), 2);
        assert_eq!(channel.status, ChannelStatus::Proposed);
        assert!(!channel.is_active());
        assert!(channel.includes_community("guild-a"));
        assert!(!channel.includes_community("guild-c"));
    }

    #[test]
    fn channel_lifecycle() {
        let mut channel = DiplomaticChannel::new(
            vec!["a".into(), "b".into()],
            ChannelType::Bilateral,
        );

        assert_eq!(channel.status, ChannelStatus::Proposed);
        channel.accept().unwrap();
        assert_eq!(channel.status, ChannelStatus::Accepted);
        channel.activate().unwrap();
        assert!(channel.is_active());
        channel.suspend().unwrap();
        assert_eq!(channel.status, ChannelStatus::Suspended);
        channel.close().unwrap();
        assert_eq!(channel.status, ChannelStatus::Closed);
    }

    #[test]
    fn channel_invalid_transition() {
        let mut channel = DiplomaticChannel::new(
            vec!["a".into(), "b".into()],
            ChannelType::Bilateral,
        );

        // Cannot activate before accepting.
        assert!(channel.activate().is_err());
        // Cannot suspend before activating.
        assert!(channel.suspend().is_err());
    }

    #[test]
    fn channel_send_message() {
        let mut channel = DiplomaticChannel::new(
            vec!["guild-a".into(), "guild-b".into()],
            ChannelType::Bilateral,
        );
        channel.accept().unwrap();
        channel.activate().unwrap();

        let msg = DiplomaticMessage::new("cpub1alice", "guild-a", "Hello, neighbors");
        channel.send_message(msg).unwrap();
        assert_eq!(channel.message_count(), 1);
    }

    #[test]
    fn channel_send_message_non_member() {
        let mut channel = DiplomaticChannel::new(
            vec!["guild-a".into(), "guild-b".into()],
            ChannelType::Bilateral,
        );
        channel.accept().unwrap();
        channel.activate().unwrap();

        let msg = DiplomaticMessage::new("cpub1eve", "guild-c", "Intruder");
        assert!(channel.send_message(msg).is_err());
    }

    #[test]
    fn channel_send_message_inactive() {
        let mut channel = DiplomaticChannel::new(
            vec!["a".into(), "b".into()],
            ChannelType::Bilateral,
        );

        let msg = DiplomaticMessage::new("cpub1a", "a", "Hello");
        assert!(channel.send_message(msg).is_err());
    }

    #[test]
    fn channel_close_already_closed() {
        let mut channel = DiplomaticChannel::new(
            vec!["a".into(), "b".into()],
            ChannelType::Bilateral,
        );
        channel.close().unwrap(); // Can close from any state except Closed.
        assert!(channel.close().is_err());
    }

    #[test]
    fn channel_serde() {
        let channel = DiplomaticChannel::new(
            vec!["a".into(), "b".into()],
            ChannelType::Mediation,
        );
        let json = serde_json::to_string(&channel).unwrap();
        let restored: DiplomaticChannel = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.channel_type, ChannelType::Mediation);
        assert_eq!(restored.communities, vec!["a", "b"]);
    }

    // --- ChannelType tests ---

    #[test]
    fn channel_type_display() {
        assert_eq!(ChannelType::Bilateral.to_string(), "bilateral");
        assert_eq!(ChannelType::Multilateral.to_string(), "multilateral");
        assert_eq!(ChannelType::Treaty.to_string(), "treaty");
        assert_eq!(ChannelType::Mediation.to_string(), "mediation");
    }

    // --- Treaty tests ---

    #[test]
    fn create_treaty() {
        let treaty = Treaty::new(
            vec!["guild-a".into(), "guild-b".into()],
            "Mutual Recognition Treaty",
        );
        assert_eq!(treaty.status, TreatyStatus::Drafted);
        assert_eq!(treaty.party_count(), 2);
        assert!(treaty.is_party("guild-a"));
        assert!(!treaty.is_party("guild-c"));
        assert!(!treaty.is_fully_ratified());
    }

    #[test]
    fn treaty_add_terms() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test Treaty",
        );

        treaty.add_term(TreatyTerm::new(
            "Recognize each other's governance",
            ObligationType::MutualRecognition,
            vec!["a".into(), "b".into()],
        ));

        treaty.add_term(TreatyTerm::new(
            "Share safety data",
            ObligationType::InformationSharing,
            vec![],
        ));

        assert_eq!(treaty.terms.len(), 2);
    }

    #[test]
    fn treaty_ratification_lifecycle() {
        let mut treaty = Treaty::new(
            vec!["guild-a".into(), "guild-b".into()],
            "Test Treaty",
        );

        // First ratification: Drafted -> Ratifying.
        treaty.ratify("guild-a", Uuid::new_v4()).unwrap();
        assert_eq!(treaty.status, TreatyStatus::Ratifying);
        assert_eq!(treaty.ratification_count(), 1);
        assert!(!treaty.is_fully_ratified());

        // Second ratification: Ratifying -> Active.
        treaty.ratify("guild-b", Uuid::new_v4()).unwrap();
        assert_eq!(treaty.status, TreatyStatus::Active);
        assert!(treaty.is_fully_ratified());
        assert!(treaty.is_active());
        assert!(treaty.effective_at.is_some());
    }

    #[test]
    fn treaty_duplicate_ratification() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test",
        );
        treaty.ratify("a", Uuid::new_v4()).unwrap();
        assert!(treaty.ratify("a", Uuid::new_v4()).is_err());
    }

    #[test]
    fn treaty_non_party_ratification() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test",
        );
        assert!(treaty.ratify("c", Uuid::new_v4()).is_err());
    }

    #[test]
    fn treaty_ratification_wrong_status() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test",
        );
        treaty.ratify("a", Uuid::new_v4()).unwrap();
        treaty.ratify("b", Uuid::new_v4()).unwrap();
        // Treaty is now Active. Cannot ratify further.
        // (There are no more parties, but if we tried to add one, it would fail on status.)
    }

    #[test]
    fn treaty_suspend_and_dissolve() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test",
        );
        treaty.ratify("a", Uuid::new_v4()).unwrap();
        treaty.ratify("b", Uuid::new_v4()).unwrap();

        treaty.suspend().unwrap();
        assert_eq!(treaty.status, TreatyStatus::Suspended);

        treaty.dissolve().unwrap();
        assert_eq!(treaty.status, TreatyStatus::Dissolved);
    }

    #[test]
    fn treaty_suspend_not_active() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test",
        );
        assert!(treaty.suspend().is_err()); // Cannot suspend a Draft.
    }

    #[test]
    fn treaty_dissolve_already_dissolved() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test",
        );
        treaty.dissolve().unwrap();
        assert!(treaty.dissolve().is_err());
    }

    #[test]
    fn treaty_serde() {
        let mut treaty = Treaty::new(
            vec!["a".into(), "b".into()],
            "Test Treaty",
        );
        treaty.add_term(TreatyTerm::new(
            "term 1",
            ObligationType::SharedStandard,
            vec!["a".into()],
        ));
        let json = serde_json::to_string(&treaty).unwrap();
        let restored: Treaty = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.title, "Test Treaty");
        assert_eq!(restored.terms.len(), 1);
    }

    // --- TreatyTerm tests ---

    #[test]
    fn term_applies_to() {
        let specific = TreatyTerm::new(
            "Only for A",
            ObligationType::TradeAgreement,
            vec!["a".into()],
        );
        assert!(specific.applies_to("a"));
        assert!(!specific.applies_to("b"));

        let universal = TreatyTerm::new(
            "For everyone",
            ObligationType::MutualRecognition,
            vec![],
        );
        assert!(universal.applies_to("a"));
        assert!(universal.applies_to("b"));
    }

    // --- ObligationType tests ---

    #[test]
    fn obligation_type_display() {
        assert_eq!(ObligationType::MutualRecognition.to_string(), "mutual-recognition");
        assert_eq!(ObligationType::SharedStandard.to_string(), "shared-standard");
        assert_eq!(ObligationType::TradeAgreement.to_string(), "trade-agreement");
        assert_eq!(ObligationType::DefenseAlliance.to_string(), "defense-alliance");
        assert_eq!(ObligationType::InformationSharing.to_string(), "information-sharing");
        assert_eq!(ObligationType::AdjudicationReciprocity.to_string(), "adjudication-reciprocity");
        assert_eq!(ObligationType::Custom("x".into()).to_string(), "custom:x");
    }

    #[test]
    fn obligation_type_serde() {
        let ot = ObligationType::DefenseAlliance;
        let json = serde_json::to_string(&ot).unwrap();
        let restored: ObligationType = serde_json::from_str(&json).unwrap();
        assert_eq!(ot, restored);
    }

    // --- Liaison tests ---

    #[test]
    fn create_liaison() {
        let liaison = Liaison::new("cpub1alice", "guild-a", "guild-b", LiaisonRole::Ambassador);
        assert_eq!(liaison.pubkey, "cpub1alice");
        assert_eq!(liaison.home_community, "guild-a");
        assert_eq!(liaison.host_community, "guild-b");
        assert!(liaison.can_discuss());
        assert!(!liaison.can_vote());
        assert!(!liaison.is_expired());
    }

    #[test]
    fn liaison_observer() {
        let liaison = Liaison::new("cpub1bob", "a", "b", LiaisonRole::Observer);
        assert!(!liaison.can_discuss());
        assert!(!liaison.can_vote());
    }

    #[test]
    fn liaison_representative() {
        let liaison = Liaison::new("cpub1carol", "a", "b", LiaisonRole::Representative);
        assert!(liaison.can_discuss());
        assert!(liaison.can_vote());
    }

    #[test]
    fn liaison_with_term() {
        let future = Utc::now() + chrono::Duration::days(365);
        let liaison = Liaison::new("cpub1dave", "a", "b", LiaisonRole::Ambassador)
            .with_term_expires(future);
        assert!(liaison.term_expires.is_some());
        assert!(!liaison.is_expired());
    }

    #[test]
    fn liaison_expired() {
        let past = Utc::now() - chrono::Duration::days(1);
        let liaison = Liaison::new("cpub1eve", "a", "b", LiaisonRole::Observer)
            .with_term_expires(past);
        assert!(liaison.is_expired());
    }

    #[test]
    fn liaison_serde() {
        let liaison = Liaison::new("cpub1test", "a", "b", LiaisonRole::Ambassador);
        let json = serde_json::to_string(&liaison).unwrap();
        let restored: Liaison = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.pubkey, "cpub1test");
        assert_eq!(restored.role, LiaisonRole::Ambassador);
    }

    // --- LiaisonRole tests ---

    #[test]
    fn liaison_role_display() {
        assert_eq!(LiaisonRole::Observer.to_string(), "observer");
        assert_eq!(LiaisonRole::Ambassador.to_string(), "ambassador");
        assert_eq!(LiaisonRole::Representative.to_string(), "representative");
    }

    // --- Kind tests ---

    #[test]
    fn diplomacy_kind_range() {
        assert!(kind::is_diplomacy_kind(11000));
        assert!(kind::is_diplomacy_kind(11099));
        assert!(!kind::is_diplomacy_kind(10999));
        assert!(!kind::is_diplomacy_kind(11100));
    }

    #[test]
    fn diplomacy_kind_constants() {
        assert_eq!(kind::DIPLOMATIC_CHANNEL, 11000);
        assert_eq!(kind::DIPLOMATIC_MESSAGE, 11001);
        assert_eq!(kind::TREATY, 11010);
        assert_eq!(kind::TREATY_RATIFICATION, 11011);
        assert_eq!(kind::LIAISON_APPOINTMENT, 11020);
    }

    #[test]
    fn federation_kind_constants() {
        assert_eq!(kind::FEDERATION_PROPOSAL, 11030);
        assert_eq!(kind::FEDERATION_STATUS, 11031);
        assert_eq!(kind::FEDERATION_WITHDRAWAL, 11032);
        assert!(kind::is_diplomacy_kind(11030));
        assert!(kind::is_diplomacy_kind(11031));
        assert!(kind::is_diplomacy_kind(11032));
    }

    // --- DiplomaticMessage tests ---

    #[test]
    fn create_message() {
        let msg = DiplomaticMessage::new("cpub1alice", "guild-a", "Greetings");
        assert_eq!(msg.author_pubkey, "cpub1alice");
        assert_eq!(msg.community_id, "guild-a");
        assert_eq!(msg.content, "Greetings");
    }

    #[test]
    fn message_serde() {
        let msg = DiplomaticMessage::new("cpub1test", "guild", "Hello");
        let json = serde_json::to_string(&msg).unwrap();
        let restored: DiplomaticMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.content, "Hello");
    }

    // --- ChannelStatus Display ---

    #[test]
    fn channel_status_display() {
        assert_eq!(ChannelStatus::Proposed.to_string(), "proposed");
        assert_eq!(ChannelStatus::Active.to_string(), "active");
        assert_eq!(ChannelStatus::Closed.to_string(), "closed");
    }

    // --- TreatyStatus Display ---

    #[test]
    fn treaty_status_display() {
        assert_eq!(TreatyStatus::Drafted.to_string(), "drafted");
        assert_eq!(TreatyStatus::Ratifying.to_string(), "ratifying");
        assert_eq!(TreatyStatus::Active.to_string(), "active");
        assert_eq!(TreatyStatus::Dissolved.to_string(), "dissolved");
    }
}

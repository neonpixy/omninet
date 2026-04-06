use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A convocation — a gathering of people for governance, deliberation, or ceremony.
///
/// From Convocation Art. 1 §1: "The right of convocation shall be vested in
/// all persons and communities without exception."
///
/// From Convocation Art. 3 §1: Purposes include "public deliberation, community
/// decision-making, legislative drafting, dispute resolution, healing, celebration."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assembly {
    pub id: Uuid,
    pub name: String,
    pub assembly_type: AssemblyType,
    pub convened_by: String,
    pub purpose: String,
    pub trigger: ConvocationTrigger,
    pub participants: Vec<String>,
    pub status: AssemblyStatus,
    pub records: Vec<AssemblyRecord>,
    pub convened_at: DateTime<Utc>,
    pub concluded_at: Option<DateTime<Utc>>,
}

impl Assembly {
    pub fn new(
        name: impl Into<String>,
        assembly_type: AssemblyType,
        convened_by: impl Into<String>,
        purpose: impl Into<String>,
        trigger: ConvocationTrigger,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            assembly_type,
            convened_by: convened_by.into(),
            purpose: purpose.into(),
            trigger,
            participants: Vec::new(),
            status: AssemblyStatus::Convened,
            records: Vec::new(),
            convened_at: Utc::now(),
            concluded_at: None,
        }
    }

    /// Add a participant to the assembly. Duplicates are ignored.
    pub fn add_participant(&mut self, pubkey: impl Into<String>) {
        let pubkey = pubkey.into();
        if !self.participants.contains(&pubkey) {
            self.participants.push(pubkey);
        }
    }

    /// Add a record to the assembly. Cannot add records after conclusion.
    pub fn add_record(&mut self, record: AssemblyRecord) -> Result<(), crate::KingdomError> {
        if self.status == AssemblyStatus::Concluded {
            return Err(crate::KingdomError::AssemblyConcluded);
        }
        self.records.push(record);
        Ok(())
    }

    /// Begin deliberation.
    pub fn begin(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != AssemblyStatus::Convened {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "InProgress".into(),
            });
        }
        self.status = AssemblyStatus::InProgress;
        Ok(())
    }

    /// Conclude the assembly.
    pub fn conclude(&mut self) -> Result<(), crate::KingdomError> {
        if self.status == AssemblyStatus::Concluded {
            return Err(crate::KingdomError::AssemblyConcluded);
        }
        self.status = AssemblyStatus::Concluded;
        self.concluded_at = Some(Utc::now());
        Ok(())
    }

    /// Pause for later resumption.
    pub fn pause(&mut self) {
        if self.status == AssemblyStatus::InProgress {
            self.status = AssemblyStatus::Paused;
        }
    }

    /// Resume from pause.
    pub fn resume(&mut self) {
        if self.status == AssemblyStatus::Paused {
            self.status = AssemblyStatus::InProgress;
        }
    }

    /// Number of participants who joined the assembly.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Whether the assembly has concluded.
    pub fn is_concluded(&self) -> bool {
        self.status == AssemblyStatus::Concluded
    }
}

/// What kind of assembly this is.
///
/// From Convocation Art. 5 §1: "Convocations may take any form suited to their
/// purpose — councils, assemblies, rituals, tribunals, healing circles, or otherwise."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssemblyType {
    /// General community assembly.
    CommunityAssembly,
    /// Federation/consortium coordination.
    ConsortiumAssembly,
    /// Emergency response coordination.
    EmergencyAssembly,
    /// Dispute resolution or tribunal.
    Tribunal,
    /// Healing or restorative process.
    HealingCircle,
    /// Cultural or ceremonial gathering.
    Ceremony,
    /// Open public forum.
    PublicForum,
}

/// What triggered the convocation.
///
/// From Convocation Art. 1 §3: "For a convocation to be lawful, it shall arise
/// from clarity of purpose, invitation by consent, and a declared intention
/// to uphold the Core and Commons."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConvocationTrigger {
    /// Regularly scheduled meeting.
    Scheduled,
    /// Called by a person or community.
    Called(String),
    /// Triggered by a proposal.
    Proposal(Uuid),
    /// Triggered by a dispute.
    Dispute(Uuid),
    /// Emergency requiring rapid coordination.
    Emergency(String),
    /// Seasonal or periodic renewal.
    SeasonalReview,
    /// Public challenge to governance.
    PublicChallenge(Uuid),
}

/// Lifecycle of an assembly.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssemblyStatus {
    /// Called but not yet started.
    Convened,
    /// Currently in session.
    InProgress,
    /// Temporarily paused.
    Paused,
    /// Formally concluded.
    Concluded,
}

/// A record from the assembly — what was discussed, proposed, or decided.
///
/// From Convocation Art. 2 §4: "Convocations shall maintain a living record
/// of who was called, who responded, what was proposed, and what was resolved."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyRecord {
    pub id: Uuid,
    pub record_type: RecordType,
    pub content: String,
    pub recorded_by: String,
    pub recorded_at: DateTime<Utc>,
}

impl AssemblyRecord {
    pub fn new(
        record_type: RecordType,
        content: impl Into<String>,
        recorded_by: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            record_type,
            content: content.into(),
            recorded_by: recorded_by.into(),
            recorded_at: Utc::now(),
        }
    }
}

/// Type of assembly record.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RecordType {
    /// Agenda item.
    Agenda,
    /// Discussion summary.
    Discussion,
    /// Proposal introduced.
    ProposalIntroduced,
    /// Decision made.
    Decision,
    /// Action item assigned.
    ActionItem,
    /// Note or observation.
    Note,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_assembly() {
        let a = Assembly::new(
            "Monthly Gathering",
            AssemblyType::CommunityAssembly,
            "alice",
            "Discuss community garden proposal",
            ConvocationTrigger::Scheduled,
        );
        assert_eq!(a.status, AssemblyStatus::Convened);
        assert!(!a.is_concluded());
    }

    #[test]
    fn assembly_lifecycle() {
        let mut a = Assembly::new(
            "Test",
            AssemblyType::PublicForum,
            "alice",
            "Test",
            ConvocationTrigger::Called("alice".into()),
        );

        a.add_participant("alice");
        a.add_participant("bob");
        a.add_participant("alice"); // duplicate ignored
        assert_eq!(a.participant_count(), 2);

        a.begin().unwrap();
        assert_eq!(a.status, AssemblyStatus::InProgress);

        a.add_record(AssemblyRecord::new(
            RecordType::Discussion,
            "Discussed the proposal",
            "alice",
        ))
        .unwrap();
        assert_eq!(a.records.len(), 1);

        a.pause();
        assert_eq!(a.status, AssemblyStatus::Paused);

        a.resume();
        assert_eq!(a.status, AssemblyStatus::InProgress);

        a.conclude().unwrap();
        assert!(a.is_concluded());
        assert!(a.concluded_at.is_some());
    }

    #[test]
    fn cannot_conclude_twice() {
        let mut a = Assembly::new(
            "Test",
            AssemblyType::Ceremony,
            "alice",
            "Test",
            ConvocationTrigger::SeasonalReview,
        );
        a.conclude().unwrap();
        assert!(a.conclude().is_err());
    }

    #[test]
    fn cannot_add_record_to_concluded_assembly() {
        let mut a = Assembly::new(
            "Test",
            AssemblyType::Tribunal,
            "alice",
            "Test",
            ConvocationTrigger::Dispute(Uuid::new_v4()),
        );
        a.conclude().unwrap();
        let record = AssemblyRecord::new(RecordType::Note, "Late note", "bob");
        assert!(a.add_record(record).is_err());
    }

    #[test]
    fn assembly_types_and_triggers() {
        let a = Assembly::new(
            "Emergency",
            AssemblyType::EmergencyAssembly,
            "steward",
            "Flood response",
            ConvocationTrigger::Emergency("River flooding".into()),
        );
        assert_eq!(a.assembly_type, AssemblyType::EmergencyAssembly);
    }
}

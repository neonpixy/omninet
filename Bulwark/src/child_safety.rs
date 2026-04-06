use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A child safety flag — bypasses normal governance entirely.
///
/// Child safety protocol bypasses normal governance.
/// Encrypted flags visible only to safety stewards, real-world resources
/// surfaced immediately, silent restriction on accused, reporter protected,
/// never adjudicated internally — always escalated to real-world services."
///
/// This is the most sensitive code in Omnidea. Every design choice here
/// is defensive — protecting the child, protecting the reporter, and
/// ensuring real-world professionals handle the situation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildSafetyFlag {
    pub id: Uuid,
    pub reporter_pubkey: String,
    pub concern: ChildSafetyConcern,
    pub affected_child_pubkey: Option<String>,
    pub accused_pubkey: Option<String>,
    pub description: String,
    pub real_world_resources_shown: bool,
    pub status: ChildSafetyStatus,
    pub created_at: DateTime<Utc>,
}

impl ChildSafetyFlag {
    /// File a child safety flag. Real-world resources are shown immediately.
    pub fn file(
        reporter_pubkey: impl Into<String>,
        concern: ChildSafetyConcern,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            reporter_pubkey: reporter_pubkey.into(),
            concern,
            affected_child_pubkey: None,
            accused_pubkey: None,
            description: description.into(),
            real_world_resources_shown: true, // ALWAYS shown
            status: ChildSafetyStatus::Filed,
            created_at: Utc::now(),
        }
    }

    /// Identify the affected child.
    pub fn with_affected_child(mut self, pubkey: impl Into<String>) -> Self {
        self.affected_child_pubkey = Some(pubkey.into());
        self
    }

    /// Identify the accused person.
    pub fn with_accused(mut self, pubkey: impl Into<String>) -> Self {
        self.accused_pubkey = Some(pubkey.into());
        self
    }
}

/// What kind of child safety concern was raised.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChildSafetyConcern {
    /// Physical abuse.
    PhysicalAbuse,
    /// Emotional/psychological abuse.
    EmotionalAbuse,
    /// Sexual abuse or exploitation.
    SexualAbuse,
    /// Neglect.
    Neglect,
    /// Online grooming or predatory behavior.
    Grooming,
    /// Child-to-child bullying or harm.
    Bullying,
    /// Self-harm or suicidal ideation disclosed.
    SelfHarm,
    /// Other safety concern.
    Other,
}

/// Lifecycle of a child safety flag.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChildSafetyStatus {
    /// Just filed. Resources shown. Silent restriction applied.
    Filed,
    /// Escalated to real-world services.
    Escalated,
    /// Resolved by real-world professionals.
    Resolved,
}

/// The child safety protocol — 5 steps, all automatic.
///
/// 1. Flag is encrypted, visible only to child safety stewards.
/// 2. Real-world resources surfaced immediately to reporter.
/// 3. Silent restriction on accused (no notification that they're restricted).
/// 4. Reporter protected from retaliation.
/// 5. Never adjudicated internally — always escalated to real-world services.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildSafetyProtocol {
    /// Flag is encrypted — only safety stewards can read it.
    pub encrypted_flag: bool,
    /// Real-world resources shown to reporter immediately.
    pub resources_shown: bool,
    /// Accused is silently restricted (no notification).
    pub silent_restriction: bool,
    /// Reporter is protected from retaliation.
    pub reporter_protected: bool,
    /// Never adjudicated by the platform — always escalated.
    pub always_escalate: bool,
}

impl Default for ChildSafetyProtocol {
    fn default() -> Self {
        Self {
            encrypted_flag: true,
            resources_shown: true,
            silent_restriction: true,
            reporter_protected: true,
            always_escalate: true,
        }
    }
}

impl ChildSafetyProtocol {
    /// The protocol is immutable — all steps must be true.
    pub fn is_valid(&self) -> bool {
        self.encrypted_flag
            && self.resources_shown
            && self.silent_restriction
            && self.reporter_protected
            && self.always_escalate
    }
}

/// Real-world resources to surface when a child safety flag is filed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RealWorldResources {
    pub emergency_number: String,
    pub crisis_hotline: String,
    pub local_services_url: Option<String>,
    pub description: String,
}

impl RealWorldResources {
    /// US defaults — communities configure their own.
    pub fn us_defaults() -> Self {
        Self {
            emergency_number: "911".into(),
            crisis_hotline: "988".into(),
            local_services_url: Some("https://www.childhelp.org".into()),
            description: "If a child is in immediate danger, call 911. For crisis support, call 988 (Suicide & Crisis Lifeline). For child abuse reporting, contact Childhelp National Child Abuse Hotline: 1-800-422-4453.".into(),
        }
    }
}

/// A silent restriction applied to an accused person.
/// They are restricted from interacting with children WITHOUT being notified.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SilentRestriction {
    pub restricted_pubkey: String,
    pub flag_id: Uuid,
    pub applied_at: DateTime<Utc>,
    /// The accused does NOT know they are restricted.
    pub notified: bool,
}

impl SilentRestriction {
    pub fn apply(restricted_pubkey: impl Into<String>, flag_id: Uuid) -> Self {
        Self {
            restricted_pubkey: restricted_pubkey.into(),
            flag_id,
            applied_at: Utc::now(),
            notified: false, // NEVER notified
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_safety_flag() {
        let flag = ChildSafetyFlag::file(
            "reporter_alice",
            ChildSafetyConcern::Grooming,
            "Suspicious messages from adult to child in community chat",
        )
        .with_affected_child("kid_bob")
        .with_accused("adult_charlie");

        assert!(flag.real_world_resources_shown);
        assert_eq!(flag.status, ChildSafetyStatus::Filed);
        assert_eq!(flag.affected_child_pubkey.as_deref(), Some("kid_bob"));
        assert_eq!(flag.accused_pubkey.as_deref(), Some("adult_charlie"));
    }

    #[test]
    fn protocol_is_immutable() {
        let protocol = ChildSafetyProtocol::default();
        assert!(protocol.is_valid());

        // If any step is disabled, protocol is invalid
        let broken = ChildSafetyProtocol {
            always_escalate: false,
            ..Default::default()
        };
        assert!(!broken.is_valid());
    }

    #[test]
    fn silent_restriction_not_notified() {
        let restriction = SilentRestriction::apply("accused", Uuid::new_v4());
        assert!(!restriction.notified); // NEVER notified
    }

    #[test]
    fn resources_always_shown() {
        let flag = ChildSafetyFlag::file(
            "reporter",
            ChildSafetyConcern::SelfHarm,
            "Child disclosed self-harm",
        );
        assert!(flag.real_world_resources_shown);
    }

    #[test]
    fn all_concern_types() {
        let types = [
            ChildSafetyConcern::PhysicalAbuse,
            ChildSafetyConcern::EmotionalAbuse,
            ChildSafetyConcern::SexualAbuse,
            ChildSafetyConcern::Neglect,
            ChildSafetyConcern::Grooming,
            ChildSafetyConcern::Bullying,
            ChildSafetyConcern::SelfHarm,
            ChildSafetyConcern::Other,
        ];
        assert_eq!(types.len(), 8);
    }

    #[test]
    fn us_default_resources() {
        let resources = RealWorldResources::us_defaults();
        assert_eq!(resources.emergency_number, "911");
        assert_eq!(resources.crisis_hotline, "988");
        assert!(resources.description.contains("child"));
    }
}

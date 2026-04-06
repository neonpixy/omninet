use serde::{Deserialize, Serialize};

/// Parent oversight settings — scales with child's age.
///
/// Gradual reduction of oversight as children grow.
/// Under 13: full visibility. 13-17: privacy for messages, no screen time limits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParentOversight {
    pub view_contacts: bool,
    pub view_messages: bool,
    pub approve_contacts: bool,
    pub activity_notifications: bool,
    pub screen_time_limits: bool,
    pub health_alerts: bool,
}

impl ParentOversight {
    /// Full oversight for young children (under 13).
    pub fn for_kid() -> Self {
        Self {
            view_contacts: true,
            view_messages: true,
            approve_contacts: true,
            activity_notifications: true,
            screen_time_limits: true,
            health_alerts: true,
        }
    }

    /// Reduced oversight for teens (13-17) — privacy for messages.
    pub fn for_teen() -> Self {
        Self {
            view_contacts: true,
            view_messages: false, // PRIVACY for teens
            approve_contacts: true,
            activity_notifications: true,
            screen_time_limits: false, // no screen time limits
            health_alerts: true,
        }
    }

    /// Appropriate oversight based on age.
    pub fn for_age(age: u8) -> Self {
        if age < 13 {
            Self::for_kid()
        } else {
            Self::for_teen()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kid_full_oversight() {
        let oversight = ParentOversight::for_kid();
        assert!(oversight.view_contacts);
        assert!(oversight.view_messages);
        assert!(oversight.approve_contacts);
        assert!(oversight.screen_time_limits);
    }

    #[test]
    fn teen_privacy_for_messages() {
        let oversight = ParentOversight::for_teen();
        assert!(oversight.view_contacts);
        assert!(!oversight.view_messages); // privacy
        assert!(oversight.approve_contacts);
        assert!(!oversight.screen_time_limits); // no screen time
        assert!(oversight.health_alerts); // still get alerts
    }

    #[test]
    fn age_based_selection() {
        let young = ParentOversight::for_age(8);
        assert!(young.view_messages); // kid = full oversight

        let teen = ParentOversight::for_age(15);
        assert!(!teen.view_messages); // teen = message privacy
    }
}

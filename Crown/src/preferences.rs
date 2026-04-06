use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// User preferences and settings.
///
/// Controls display, language, privacy, and notification behavior.
/// Defaults are sensible for a new user (system theme, 1x text scale,
/// English, all notifications on, private by default).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Preferences {
    // -- Display --

    /// Color theme preference.
    pub theme: Theme,
    /// Text scaling factor (1.0 = normal). Used for accessibility.
    pub text_scale: f64,
    /// Whether to minimize motion and animations.
    pub reduce_motion: bool,

    // -- Language --

    /// BCP 47 code for content authored by this user (default `"en"`).
    pub content_language: String,
    /// BCP 47 code for the UI. `None` means follow the OS setting.
    pub interface_language: Option<String>,
    /// Whether to auto-translate foreign-language content via Lingo.
    pub auto_translate: bool,

    // -- Privacy --

    /// Who can see new ideas by default.
    pub default_visibility: Visibility,
    /// Whether others can see when you're online.
    pub show_online_status: bool,
    /// Whether to send read receipts in conversations.
    pub send_read_receipts: bool,

    // -- Notifications --

    /// Master toggle for push notifications.
    pub push_enabled: bool,
    /// Which categories of notification are enabled.
    pub notification_categories: HashSet<NotificationCategory>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            text_scale: 1.0,
            reduce_motion: false,
            content_language: "en".into(),
            interface_language: None,
            auto_translate: true,
            default_visibility: Visibility::Private,
            show_online_status: true,
            send_read_receipts: true,
            push_enabled: true,
            notification_categories: NotificationCategory::all(),
        }
    }
}

/// Color theme preference.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the OS light/dark setting.
    System,
    /// Always light mode.
    Light,
    /// Always dark mode.
    Dark,
    /// Omnidea's signature theme -- deep, ambient, alive.
    Cosmic,
}

/// Default visibility for newly created ideas.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Only the creator can see it.
    Private,
    /// Visible to the creator's collectives (communities).
    Collective,
    /// Visible to everyone on the network.
    Public,
}

/// Categories of notifications a user can toggle independently.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum NotificationCategory {
    /// Someone mentioned you by crown ID.
    Mentions,
    /// Replies to your ideas or comments.
    Replies,
    /// Someone endorsed your work.
    Endorsements,
    /// Cool (currency) transfers to or from you.
    Transfers,
    /// Activity in your collectives (communities).
    CollectiveActivity,
    /// System-level updates (security alerts, version announcements).
    SystemUpdates,
}

impl NotificationCategory {
    /// Returns a set containing all six notification categories. Used as
    /// the default for new users (everything enabled).
    pub fn all() -> HashSet<Self> {
        HashSet::from([
            Self::Mentions,
            Self::Replies,
            Self::Endorsements,
            Self::Transfers,
            Self::CollectiveActivity,
            Self::SystemUpdates,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preferences() {
        let prefs = Preferences::default();
        assert_eq!(prefs.theme, Theme::System);
        assert!((prefs.text_scale - 1.0).abs() < f64::EPSILON);
        assert!(!prefs.reduce_motion);
        assert_eq!(prefs.content_language, "en");
        assert!(prefs.interface_language.is_none());
        assert!(prefs.auto_translate);
        assert_eq!(prefs.default_visibility, Visibility::Private);
        assert!(prefs.show_online_status);
        assert!(prefs.send_read_receipts);
        assert!(prefs.push_enabled);
        assert_eq!(prefs.notification_categories.len(), 6);
    }

    #[test]
    fn preferences_serde_round_trip() {
        let prefs = Preferences {
            theme: Theme::Cosmic,
            text_scale: 1.5,
            content_language: "ja".into(),
            default_visibility: Visibility::Public,
            ..Default::default()
        };

        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(prefs, loaded);
    }

    #[test]
    fn theme_serde_values() {
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(
            serde_json::to_string(&Theme::Cosmic).unwrap(),
            "\"cosmic\""
        );
    }

    #[test]
    fn visibility_serde_values() {
        assert_eq!(
            serde_json::to_string(&Visibility::Private).unwrap(),
            "\"private\""
        );
        assert_eq!(
            serde_json::to_string(&Visibility::Collective).unwrap(),
            "\"collective\""
        );
        assert_eq!(
            serde_json::to_string(&Visibility::Public).unwrap(),
            "\"public\""
        );
    }

    #[test]
    fn notification_category_all_variants() {
        let all = NotificationCategory::all();
        assert_eq!(all.len(), 6);
        assert!(all.contains(&NotificationCategory::Mentions));
        assert!(all.contains(&NotificationCategory::CollectiveActivity));

        // Serde camelCase
        assert_eq!(
            serde_json::to_string(&NotificationCategory::CollectiveActivity).unwrap(),
            "\"collectiveActivity\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationCategory::SystemUpdates).unwrap(),
            "\"systemUpdates\""
        );
    }
}

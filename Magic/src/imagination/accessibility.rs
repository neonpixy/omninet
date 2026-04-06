//! Accessibility specification for render output.
//!
//! Every RenderSpec carries a required AccessibilitySpec that describes how
//! the rendered element should be presented to assistive technology. This is
//! Magic's rendering-side complement to Ideas' AccessibilityMetadata (which
//! stores accessibility data *on* the digit). The renderer reads the digit's
//! a11y metadata and produces an AccessibilitySpec that platform layers
//! (Divinity) translate into native accessibility APIs.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — accessibility is not optional. Every rendered element declares
//! its semantic role, label, and traits. **Sovereignty** — users control their
//! experience via MotionPreference and focus order.

use serde::{Deserialize, Serialize};

/// The semantic role of a rendered element for assistive technology.
///
/// Mirrors `ideas::AccessibilityRole` but lives in the rendering layer.
/// Platform layers (Divinity) map these to native roles:
/// - SwiftUI: `.accessibilityAddTraits(.isButton)`, etc.
/// - HTML: `role="button"`, `<nav>`, `<dialog>`, etc.
/// - Android: `AccessibilityNodeInfo.setClassName()`
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessibilityRole {
    Button,
    Link,
    Image,
    Heading,
    List,
    ListItem,
    TextField,
    Checkbox,
    Slider,
    Tab,
    Table,
    Cell,
    Form,
    Navigation,
    Alert,
    Dialog,
    Custom(String),
}

impl Default for AccessibilityRole {
    fn default() -> Self {
        AccessibilityRole::Custom("none".into())
    }
}

impl From<&ideas::AccessibilityRole> for AccessibilityRole {
    fn from(role: &ideas::AccessibilityRole) -> Self {
        match role {
            ideas::AccessibilityRole::Button => AccessibilityRole::Button,
            ideas::AccessibilityRole::Link => AccessibilityRole::Link,
            ideas::AccessibilityRole::Image => AccessibilityRole::Image,
            ideas::AccessibilityRole::Heading => AccessibilityRole::Heading,
            ideas::AccessibilityRole::List => AccessibilityRole::List,
            ideas::AccessibilityRole::ListItem => AccessibilityRole::ListItem,
            ideas::AccessibilityRole::TextField => AccessibilityRole::TextField,
            ideas::AccessibilityRole::Checkbox => AccessibilityRole::Checkbox,
            ideas::AccessibilityRole::Slider => AccessibilityRole::Slider,
            ideas::AccessibilityRole::Tab => AccessibilityRole::Tab,
            ideas::AccessibilityRole::Table => AccessibilityRole::Table,
            ideas::AccessibilityRole::Cell => AccessibilityRole::Cell,
            ideas::AccessibilityRole::Form => AccessibilityRole::Form,
            ideas::AccessibilityRole::Navigation => AccessibilityRole::Navigation,
            ideas::AccessibilityRole::Alert => AccessibilityRole::Alert,
            ideas::AccessibilityRole::Dialog => AccessibilityRole::Dialog,
            ideas::AccessibilityRole::Custom(s) => AccessibilityRole::Custom(s.clone()),
        }
    }
}

/// Behavioral traits that modify how assistive technology presents an element.
///
/// Multiple traits can apply to a single element. Platform layers translate
/// these into native trait flags.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessibilityTrait {
    /// Element can be interacted with (tapped, clicked).
    Interactive,
    /// Element's value can be adjusted (e.g., slider, stepper).
    Adjustable,
    /// Element contains static text (not editable).
    StaticText,
    /// Element is a section header.
    Header,
    /// Element provides a summary of the current state.
    Summary,
    /// Element is a search field.
    SearchField,
    /// Element is currently selected.
    Selected,
    /// Element is disabled and cannot be interacted with.
    Disabled,
}

/// How a live region announces dynamic content changes to screen readers.
///
/// Mirrors `ideas::LiveRegion`. Platform layers translate:
/// - SwiftUI: `UIAccessibility.post(.announcement, ...)`
/// - HTML: `aria-live="polite"` / `aria-live="assertive"`
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveRegion {
    /// Changes are announced when the user is idle.
    Polite,
    /// Changes are announced immediately, interrupting current speech.
    Assertive,
    /// No live announcements.
    #[default]
    Off,
}

impl From<&ideas::LiveRegion> for LiveRegion {
    fn from(lr: &ideas::LiveRegion) -> Self {
        match lr {
            ideas::LiveRegion::Polite => LiveRegion::Polite,
            ideas::LiveRegion::Assertive => LiveRegion::Assertive,
            ideas::LiveRegion::Off => LiveRegion::Off,
        }
    }
}

/// A custom accessibility action that assistive technology can invoke.
///
/// Beyond the standard tap/double-tap, elements can declare custom actions
/// that appear in the VoiceOver/TalkBack rotor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomAccessibilityAction {
    /// Human-readable action name shown in the accessibility menu.
    pub name: String,
    /// Machine-readable action identifier for the platform layer to dispatch.
    pub action_id: String,
}

/// Required accessibility specification for every rendered element.
///
/// This is a **required** field on `RenderSpec`. Every digit renderer must
/// produce a meaningful `AccessibilitySpec`. Platform layers (Divinity)
/// translate this into native accessibility APIs.
///
/// ## Example
///
/// ```rust
/// use magic::imagination::accessibility::{AccessibilitySpec, AccessibilityRole, AccessibilityTrait};
///
/// let spec = AccessibilitySpec::new(AccessibilityRole::Button, "Submit form")
///     .with_hint("Double-tap to submit the form")
///     .with_trait(AccessibilityTrait::Interactive);
/// assert_eq!(spec.label, "Submit form");
/// assert_eq!(spec.role, AccessibilityRole::Button);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccessibilitySpec {
    /// The semantic role of this element.
    pub role: AccessibilityRole,
    /// Human-readable label for screen readers. Must not be empty.
    pub label: String,
    /// Current value (e.g., "50%" for a slider, "checked" for a checkbox).
    pub value: Option<String>,
    /// Usage hint (e.g., "Double-tap to activate").
    pub hint: Option<String>,
    /// Behavioral traits for this element.
    pub traits: Vec<AccessibilityTrait>,
    /// Tab/focus order. Lower values are focused first. `None` uses natural order.
    pub focus_order: Option<i32>,
    /// Custom actions beyond standard interaction.
    pub custom_actions: Vec<CustomAccessibilityAction>,
    /// Live region announcement behavior for dynamic content.
    pub live_region: LiveRegion,
    /// Whether this element is hidden from assistive technology.
    pub hidden: bool,
}

impl AccessibilitySpec {
    /// Create a new AccessibilitySpec with the required role and label.
    #[must_use]
    pub fn new(role: AccessibilityRole, label: impl Into<String>) -> Self {
        Self {
            role,
            label: label.into(),
            value: None,
            hint: None,
            traits: Vec::new(),
            focus_order: None,
            custom_actions: Vec::new(),
            live_region: LiveRegion::Off,
            hidden: false,
        }
    }

    /// Create a hidden AccessibilitySpec for decorative elements.
    ///
    /// Decorative elements (dividers, spacers) should still have an
    /// AccessibilitySpec, but with `hidden: true` so screen readers skip them.
    #[must_use]
    pub fn decorative() -> Self {
        Self {
            role: AccessibilityRole::Custom("none".into()),
            label: String::new(),
            value: None,
            hint: None,
            traits: Vec::new(),
            focus_order: None,
            custom_actions: Vec::new(),
            live_region: LiveRegion::Off,
            hidden: true,
        }
    }

    /// Build an AccessibilitySpec from a Digit's AccessibilityMetadata.
    ///
    /// Falls back to a default spec with the provided role and label if
    /// the digit has no accessibility metadata.
    #[must_use]
    pub fn from_digit(
        digit: &ideas::Digit,
        fallback_role: AccessibilityRole,
        fallback_label: impl Into<String>,
    ) -> Self {
        if let Some(meta) = digit.accessibility() {
            let mut spec = Self::new(
                AccessibilityRole::from(&meta.role),
                meta.label,
            );
            spec.value = meta.value;
            spec.hint = meta.hint;
            spec.focus_order = meta.focus_order;
            if let Some(lr) = &meta.live_region {
                spec.live_region = LiveRegion::from(lr);
            }
            spec
        } else {
            Self::new(fallback_role, fallback_label)
        }
    }

    /// Set the current value.
    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set the usage hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Add a behavioral trait.
    #[must_use]
    pub fn with_trait(mut self, t: AccessibilityTrait) -> Self {
        if !self.traits.contains(&t) {
            self.traits.push(t);
        }
        self
    }

    /// Set the focus order.
    #[must_use]
    pub fn with_focus_order(mut self, order: i32) -> Self {
        self.focus_order = Some(order);
        self
    }

    /// Add a custom action.
    #[must_use]
    pub fn with_custom_action(mut self, name: impl Into<String>, action_id: impl Into<String>) -> Self {
        self.custom_actions.push(CustomAccessibilityAction {
            name: name.into(),
            action_id: action_id.into(),
        });
        self
    }

    /// Set the live region behavior.
    #[must_use]
    pub fn with_live_region(mut self, live_region: LiveRegion) -> Self {
        self.live_region = live_region;
        self
    }

    /// Mark this element as hidden from assistive technology.
    #[must_use]
    pub fn as_hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_minimal_spec() {
        let spec = AccessibilitySpec::new(AccessibilityRole::Button, "Click me");
        assert_eq!(spec.role, AccessibilityRole::Button);
        assert_eq!(spec.label, "Click me");
        assert!(spec.value.is_none());
        assert!(spec.hint.is_none());
        assert!(spec.traits.is_empty());
        assert!(spec.focus_order.is_none());
        assert!(spec.custom_actions.is_empty());
        assert_eq!(spec.live_region, LiveRegion::Off);
        assert!(!spec.hidden);
    }

    #[test]
    fn decorative_is_hidden() {
        let spec = AccessibilitySpec::decorative();
        assert!(spec.hidden);
        assert!(spec.label.is_empty());
    }

    #[test]
    fn builder_chain() {
        let spec = AccessibilitySpec::new(AccessibilityRole::Slider, "Volume")
            .with_value("50%")
            .with_hint("Swipe up or down to adjust")
            .with_trait(AccessibilityTrait::Adjustable)
            .with_focus_order(3)
            .with_live_region(LiveRegion::Polite)
            .with_custom_action("Mute", "mute_action");

        assert_eq!(spec.role, AccessibilityRole::Slider);
        assert_eq!(spec.label, "Volume");
        assert_eq!(spec.value.as_deref(), Some("50%"));
        assert_eq!(spec.hint.as_deref(), Some("Swipe up or down to adjust"));
        assert_eq!(spec.traits, vec![AccessibilityTrait::Adjustable]);
        assert_eq!(spec.focus_order, Some(3));
        assert_eq!(spec.live_region, LiveRegion::Polite);
        assert_eq!(spec.custom_actions.len(), 1);
        assert_eq!(spec.custom_actions[0].name, "Mute");
        assert_eq!(spec.custom_actions[0].action_id, "mute_action");
    }

    #[test]
    fn duplicate_trait_not_added() {
        let spec = AccessibilitySpec::new(AccessibilityRole::Button, "OK")
            .with_trait(AccessibilityTrait::Interactive)
            .with_trait(AccessibilityTrait::Interactive);
        assert_eq!(spec.traits.len(), 1);
    }

    #[test]
    fn as_hidden_marks_hidden() {
        let spec = AccessibilitySpec::new(AccessibilityRole::Image, "Logo").as_hidden();
        assert!(spec.hidden);
    }

    #[test]
    fn from_digit_with_metadata() {
        let digit = ideas::Digit::new("text".into(), x::Value::Null, "alice".into()).unwrap();
        let meta = ideas::AccessibilityMetadata {
            role: ideas::AccessibilityRole::Button,
            label: "Submit".into(),
            value: Some("enabled".into()),
            hint: Some("Double-tap to submit".into()),
            heading_level: None,
            language: None,
            focus_order: Some(1),
            live_region: Some(ideas::LiveRegion::Assertive),
        };
        let digit = ideas::accessibility::with_accessibility(digit, &meta, "alice");
        let spec = AccessibilitySpec::from_digit(
            &digit,
            AccessibilityRole::Custom("fallback".into()),
            "Fallback label",
        );
        assert_eq!(spec.role, AccessibilityRole::Button);
        assert_eq!(spec.label, "Submit");
        assert_eq!(spec.value.as_deref(), Some("enabled"));
        assert_eq!(spec.hint.as_deref(), Some("Double-tap to submit"));
        assert_eq!(spec.focus_order, Some(1));
        assert_eq!(spec.live_region, LiveRegion::Assertive);
    }

    #[test]
    fn from_digit_without_metadata_uses_fallback() {
        let digit = ideas::Digit::new("text".into(), x::Value::Null, "alice".into()).unwrap();
        let spec = AccessibilitySpec::from_digit(
            &digit,
            AccessibilityRole::Heading,
            "Default heading",
        );
        assert_eq!(spec.role, AccessibilityRole::Heading);
        assert_eq!(spec.label, "Default heading");
    }

    #[test]
    fn role_from_ideas_conversion() {
        let ideas_role = ideas::AccessibilityRole::Dialog;
        let magic_role = AccessibilityRole::from(&ideas_role);
        assert_eq!(magic_role, AccessibilityRole::Dialog);
    }

    #[test]
    fn role_custom_from_ideas_conversion() {
        let ideas_role = ideas::AccessibilityRole::Custom("carousel".into());
        let magic_role = AccessibilityRole::from(&ideas_role);
        assert_eq!(magic_role, AccessibilityRole::Custom("carousel".into()));
    }

    #[test]
    fn live_region_from_ideas_conversion() {
        assert_eq!(LiveRegion::from(&ideas::LiveRegion::Polite), LiveRegion::Polite);
        assert_eq!(LiveRegion::from(&ideas::LiveRegion::Assertive), LiveRegion::Assertive);
        assert_eq!(LiveRegion::from(&ideas::LiveRegion::Off), LiveRegion::Off);
    }

    #[test]
    fn serde_roundtrip() {
        let spec = AccessibilitySpec::new(AccessibilityRole::Button, "Save")
            .with_value("active")
            .with_trait(AccessibilityTrait::Interactive)
            .with_custom_action("Delete", "delete_action");

        let json = serde_json::to_string(&spec).unwrap();
        let decoded: AccessibilitySpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, decoded);
    }

    #[test]
    fn default_role() {
        let role = AccessibilityRole::default();
        assert_eq!(role, AccessibilityRole::Custom("none".into()));
    }

    #[test]
    fn default_live_region() {
        assert_eq!(LiveRegion::default(), LiveRegion::Off);
    }

    #[test]
    fn all_roles_serde() {
        let roles = vec![
            AccessibilityRole::Button,
            AccessibilityRole::Link,
            AccessibilityRole::Image,
            AccessibilityRole::Heading,
            AccessibilityRole::List,
            AccessibilityRole::ListItem,
            AccessibilityRole::TextField,
            AccessibilityRole::Checkbox,
            AccessibilityRole::Slider,
            AccessibilityRole::Tab,
            AccessibilityRole::Table,
            AccessibilityRole::Cell,
            AccessibilityRole::Form,
            AccessibilityRole::Navigation,
            AccessibilityRole::Alert,
            AccessibilityRole::Dialog,
            AccessibilityRole::Custom("widget".into()),
        ];
        for role in roles {
            let json = serde_json::to_string(&role).unwrap();
            let decoded: AccessibilityRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, decoded);
        }
    }

    #[test]
    fn all_traits_serde() {
        let traits = vec![
            AccessibilityTrait::Interactive,
            AccessibilityTrait::Adjustable,
            AccessibilityTrait::StaticText,
            AccessibilityTrait::Header,
            AccessibilityTrait::Summary,
            AccessibilityTrait::SearchField,
            AccessibilityTrait::Selected,
            AccessibilityTrait::Disabled,
        ];
        for t in traits {
            let json = serde_json::to_string(&t).unwrap();
            let decoded: AccessibilityTrait = serde_json::from_str(&json).unwrap();
            assert_eq!(t, decoded);
        }
    }

    #[test]
    fn custom_action_serde() {
        let action = CustomAccessibilityAction {
            name: "Share".into(),
            action_id: "share_action".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let decoded: CustomAccessibilityAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, decoded);
    }
}

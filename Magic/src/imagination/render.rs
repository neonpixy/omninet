use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use ideas::Digit;

use super::accessibility::AccessibilitySpec;

/// How a digit should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RenderMode {
    /// Normal viewing mode.
    Display,
    /// Interactive editing mode with handles and selection.
    Editing,
    /// Small preview for navigation strips and galleries.
    Thumbnail,
    /// High-fidelity output for printing.
    Print,
}

/// Color scheme for rendering context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ColorScheme {
    /// Light appearance.
    #[default]
    Light,
    /// Dark appearance.
    Dark,
}


/// Environmental context passed to renderers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderContext {
    pub available_width: f64,
    pub available_height: f64,
    pub color_scheme: ColorScheme,
    pub text_scale: f64,
    pub reduce_motion: bool,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            available_width: 800.0,
            available_height: 600.0,
            color_scheme: ColorScheme::Light,
            text_scale: 1.0,
            reduce_motion: false,
        }
    }
}

/// Platform-independent rendering specification for a digit.
///
/// The Rust crate produces these. Platform layers (Swift, HTML, etc.)
/// consume them to create actual views. The Rust side never touches pixels.
///
/// `accessibility` is a **required** field. Every rendered element must
/// declare how it presents itself to assistive technology. Use
/// `AccessibilitySpec::decorative()` for purely visual elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderSpec {
    pub digit_id: Uuid,
    pub digit_type: String,
    pub mode: RenderMode,
    pub estimated_width: f64,
    pub estimated_height: f64,
    /// Required accessibility specification for this rendered element.
    pub accessibility: AccessibilitySpec,
    /// Opaque properties the platform renderer needs.
    pub properties: HashMap<String, serde_json::Value>,
}

impl RenderSpec {
    /// Create a new RenderSpec with a decorative (hidden) accessibility spec.
    ///
    /// Callers should replace the accessibility field with a meaningful spec
    /// via `with_accessibility()` for any non-decorative element.
    pub fn new(digit_id: Uuid, digit_type: impl Into<String>, mode: RenderMode) -> Self {
        Self {
            digit_id,
            digit_type: digit_type.into(),
            mode,
            estimated_width: 0.0,
            estimated_height: 0.0,
            accessibility: AccessibilitySpec::decorative(),
            properties: HashMap::new(),
        }
    }

    /// Set the estimated width and height for layout purposes.
    pub fn with_size(mut self, width: f64, height: f64) -> Self {
        self.estimated_width = width;
        self.estimated_height = height;
        self
    }

    /// Set the accessibility specification for this render spec.
    pub fn with_accessibility(mut self, accessibility: AccessibilitySpec) -> Self {
        self.accessibility = accessibility;
        self
    }

    /// Add an opaque property that the platform renderer will consume.
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

/// Trait for mapping digit types to render specifications.
///
/// Platform layers implement this. The Rust crate defines the trait and
/// infrastructure (registry, cache) only.
pub trait DigitRenderer: Send + Sync {
    /// Which digit type this renderer handles (e.g. "text", "image").
    fn digit_type(&self) -> &str;

    /// Supported render modes.
    fn supported_modes(&self) -> Vec<RenderMode>;

    /// Produce a render specification for the given digit + mode + context.
    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec;

    /// Estimated size without full rendering (for layout).
    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let _ = (digit, context);
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_mode_serde_roundtrip() {
        for mode in [
            RenderMode::Display,
            RenderMode::Editing,
            RenderMode::Thumbnail,
            RenderMode::Print,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let decoded: RenderMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, decoded);
        }
    }

    #[test]
    fn render_context_defaults() {
        let ctx = RenderContext::default();
        assert_eq!(ctx.available_width, 800.0);
        assert_eq!(ctx.color_scheme, ColorScheme::Light);
        assert_eq!(ctx.text_scale, 1.0);
        assert!(!ctx.reduce_motion);
    }

    #[test]
    fn color_scheme_default_is_light() {
        assert_eq!(ColorScheme::default(), ColorScheme::Light);
    }

    #[test]
    fn render_spec_builder() {
        use crate::imagination::accessibility::{AccessibilityRole, AccessibilitySpec};
        let id = Uuid::new_v4();
        let spec = RenderSpec::new(id, "text", RenderMode::Display)
            .with_size(200.0, 50.0)
            .with_accessibility(AccessibilitySpec::new(AccessibilityRole::Heading, "Title"))
            .with_property("font_size", serde_json::json!(16));
        assert_eq!(spec.digit_id, id);
        assert_eq!(spec.estimated_width, 200.0);
        assert_eq!(spec.properties.get("font_size"), Some(&serde_json::json!(16)));
        assert_eq!(spec.accessibility.role, AccessibilityRole::Heading);
        assert_eq!(spec.accessibility.label, "Title");
    }

    #[test]
    fn render_spec_default_accessibility_is_decorative() {
        let id = Uuid::new_v4();
        let spec = RenderSpec::new(id, "divider", RenderMode::Display);
        assert!(spec.accessibility.hidden);
    }

    #[test]
    fn digit_renderer_trait_is_object_safe() {
        fn _accepts(_: &dyn DigitRenderer) {}
    }

    #[test]
    fn render_mode_equality() {
        assert_eq!(RenderMode::Display, RenderMode::Display);
        assert_ne!(RenderMode::Display, RenderMode::Editing);
    }
}

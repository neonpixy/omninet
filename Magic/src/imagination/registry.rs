use std::collections::HashMap;

use ideas::Digit;

use super::accessibility::{AccessibilityRole, AccessibilitySpec};
use super::render::{DigitRenderer, RenderContext, RenderMode, RenderSpec};

/// Fallback renderer for unregistered digit types.
/// Renders any digit type with a placeholder specification.
pub struct FallbackRenderer;

impl DigitRenderer for FallbackRenderer {
    fn digit_type(&self) -> &str {
        "*"
    }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Thumbnail]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, _context: &RenderContext) -> RenderSpec {
        let a11y = AccessibilitySpec::from_digit(
            digit,
            AccessibilityRole::Custom("unknown".into()),
            format!("Unknown element: {}", digit.digit_type()),
        );
        RenderSpec::new(digit.id(), digit.digit_type(), mode)
            .with_size(100.0, 40.0)
            .with_accessibility(a11y)
            .with_property("fallback", serde_json::json!(true))
    }

    fn estimated_size(&self, _digit: &Digit, _context: &RenderContext) -> (f64, f64) {
        (100.0, 40.0)
    }
}

/// Registry mapping digit types to their renderers.
pub struct RendererRegistry {
    renderers: HashMap<String, Box<dyn DigitRenderer>>,
    fallback: Box<dyn DigitRenderer>,
}

impl RendererRegistry {
    /// Create an empty registry with only the fallback renderer.
    pub fn new() -> Self {
        Self {
            renderers: HashMap::new(),
            fallback: Box::new(FallbackRenderer),
        }
    }

    /// Register a renderer for its digit type.
    pub fn register(&mut self, renderer: Box<dyn DigitRenderer>) {
        self.renderers
            .insert(renderer.digit_type().to_string(), renderer);
    }

    /// Get the renderer for a type (falls back to FallbackRenderer).
    pub fn get(&self, digit_type: &str) -> &dyn DigitRenderer {
        self.renderers
            .get(digit_type)
            .map(|r| r.as_ref())
            .unwrap_or(self.fallback.as_ref())
    }

    /// Check if a specific (non-fallback) renderer exists.
    pub fn has_renderer(&self, digit_type: &str) -> bool {
        self.renderers.contains_key(digit_type)
    }

    /// Check if the type can be rendered (including fallback).
    pub fn can_render(&self, _digit_type: &str) -> bool {
        true // fallback always handles it
    }

    /// Render a digit using the appropriate renderer.
    pub fn render(
        &self,
        digit: &Digit,
        mode: RenderMode,
        context: &RenderContext,
    ) -> RenderSpec {
        let renderer = self.get(digit.digit_type());
        renderer.render(digit, mode, context)
    }

    /// Iterate over all registered digit type names (excluding the fallback).
    pub fn registered_types(&self) -> impl Iterator<Item = &String> {
        self.renderers.keys()
    }

    /// Number of explicitly registered renderers (excluding the fallback).
    pub fn count(&self) -> usize {
        self.renderers.len()
    }
}

impl Default for RendererRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x::Value;

    struct MockTextRenderer;

    impl DigitRenderer for MockTextRenderer {
        fn digit_type(&self) -> &str {
            "text"
        }
        fn supported_modes(&self) -> Vec<RenderMode> {
            vec![RenderMode::Display, RenderMode::Editing]
        }
        fn render(&self, digit: &Digit, mode: RenderMode, _ctx: &RenderContext) -> RenderSpec {
            RenderSpec::new(digit.id(), "text", mode)
                .with_size(300.0, 20.0)
                .with_accessibility(AccessibilitySpec::new(
                    AccessibilityRole::Custom("text".into()),
                    "Mock text",
                ))
                .with_property("mock", serde_json::json!(true))
        }
    }

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn fallback_renders_any_type() {
        let reg = RendererRegistry::new();
        let digit = make_digit("unknown");
        let spec = reg.render(&digit, RenderMode::Display, &RenderContext::default());
        assert_eq!(spec.digit_type, "unknown");
        assert_eq!(spec.properties.get("fallback"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn register_then_get() {
        let mut reg = RendererRegistry::new();
        reg.register(Box::new(MockTextRenderer));
        assert!(reg.has_renderer("text"));
        let digit = make_digit("text");
        let spec = reg.render(&digit, RenderMode::Display, &RenderContext::default());
        assert_eq!(spec.properties.get("mock"), Some(&serde_json::json!(true)));
        assert_eq!(spec.estimated_width, 300.0);
    }

    #[test]
    fn unregistered_falls_back() {
        let reg = RendererRegistry::new();
        assert!(!reg.has_renderer("image"));
        let digit = make_digit("image");
        let spec = reg.render(&digit, RenderMode::Display, &RenderContext::default());
        assert_eq!(spec.estimated_width, 100.0); // fallback size
    }

    #[test]
    fn can_render_always_true() {
        let reg = RendererRegistry::new();
        assert!(reg.can_render("anything"));
    }

    #[test]
    fn registered_types_iteration() {
        let mut reg = RendererRegistry::new();
        reg.register(Box::new(MockTextRenderer));
        let types: Vec<_> = reg.registered_types().collect();
        assert_eq!(types.len(), 1);
        assert!(types.contains(&&"text".to_string()));
    }

    #[test]
    fn count() {
        let mut reg = RendererRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(Box::new(MockTextRenderer));
        assert_eq!(reg.count(), 1);
    }
}

use serde::{Deserialize, Serialize};

/// How an image fills its container.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ImageFitMode {
    /// Scale to fill, may crop.
    Fill,
    /// Scale to fit entirely, may letterbox.
    #[default]
    Fit,
    /// Distort to fill exactly.
    Stretch,
    /// Repeat as a pattern.
    Tile,
}

/// Image styling tokens. All reference fields are string keys into Aura's token maps
/// (Arch for corner radius, Crest for border color, Umbra for shadow).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageStyle {
    pub fit_mode: ImageFitMode,
    /// Arch key reference for corner radius.
    pub corner_radius: Option<String>,
    /// Crest color reference for border color.
    pub border_color: Option<String>,
    pub border_width: Option<f64>,
    /// Umbra key reference for shadow.
    pub shadow: Option<String>,
    /// Opacity, 0.0 to 1.0.
    pub opacity: f64,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            fit_mode: ImageFitMode::Fit,
            corner_radius: None,
            border_color: None,
            border_width: None,
            shadow: None,
            opacity: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let style = ImageStyle::default();
        assert_eq!(style.fit_mode, ImageFitMode::Fit);
        assert_eq!(style.opacity, 1.0);
        assert!(style.corner_radius.is_none());
        assert!(style.border_color.is_none());
        assert!(style.border_width.is_none());
        assert!(style.shadow.is_none());
    }

    #[test]
    fn fit_mode_default() {
        assert_eq!(ImageFitMode::default(), ImageFitMode::Fit);
    }

    #[test]
    fn with_all_fields() {
        let style = ImageStyle {
            fit_mode: ImageFitMode::Fill,
            corner_radius: Some("md".into()),
            border_color: Some("accent".into()),
            border_width: Some(2.0),
            shadow: Some("subtle".into()),
            opacity: 0.8,
        };
        assert_eq!(style.fit_mode, ImageFitMode::Fill);
        assert_eq!(style.corner_radius.as_deref(), Some("md"));
        assert_eq!(style.border_color.as_deref(), Some("accent"));
        assert_eq!(style.border_width, Some(2.0));
        assert_eq!(style.shadow.as_deref(), Some("subtle"));
        assert!((style.opacity - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn serde_roundtrip_default() {
        let style = ImageStyle::default();
        let json = serde_json::to_string(&style).unwrap();
        let decoded: ImageStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.fit_mode, style.fit_mode);
        assert!((decoded.opacity - style.opacity).abs() < f64::EPSILON);
    }

    #[test]
    fn serde_roundtrip_full() {
        let style = ImageStyle {
            fit_mode: ImageFitMode::Tile,
            corner_radius: Some("lg".into()),
            border_color: Some("danger".into()),
            border_width: Some(3.0),
            shadow: Some("elevated".into()),
            opacity: 0.5,
        };
        let json = serde_json::to_string(&style).unwrap();
        let decoded: ImageStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.fit_mode, ImageFitMode::Tile);
        assert_eq!(decoded.corner_radius.as_deref(), Some("lg"));
        assert_eq!(decoded.border_color.as_deref(), Some("danger"));
        assert_eq!(decoded.border_width, Some(3.0));
        assert_eq!(decoded.shadow.as_deref(), Some("elevated"));
        assert!((decoded.opacity - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn serde_roundtrip_fit_modes() {
        for mode in [
            ImageFitMode::Fill,
            ImageFitMode::Fit,
            ImageFitMode::Stretch,
            ImageFitMode::Tile,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let decoded: ImageFitMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, decoded);
        }
    }

    #[test]
    fn stretch_mode() {
        let style = ImageStyle {
            fit_mode: ImageFitMode::Stretch,
            ..ImageStyle::default()
        };
        assert_eq!(style.fit_mode, ImageFitMode::Stretch);
        assert_eq!(style.opacity, 1.0);
    }
}

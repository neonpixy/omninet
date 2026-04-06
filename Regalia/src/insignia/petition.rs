use serde::{Deserialize, Serialize};

/// Size proposal: proposed width/height for a child to measure against.
///
/// `None` means "use your intrinsic size."
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Petition {
    pub width: Option<f64>,
    pub height: Option<f64>,
}

impl Petition {
    /// Create a petition with optional width and height proposals.
    pub fn new(width: Option<f64>, height: Option<f64>) -> Self {
        Self { width, height }
    }

    /// A petition with no size constraints -- the child decides.
    pub fn unspecified() -> Self {
        Self {
            width: None,
            height: None,
        }
    }

    /// A petition proposing a specific width and height.
    pub fn size(width: f64, height: f64) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
        }
    }
}

impl Default for Petition {
    fn default() -> Self {
        Self::unspecified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unspecified() {
        let p = Petition::unspecified();
        assert!(p.width.is_none());
        assert!(p.height.is_none());
    }

    #[test]
    fn size_constructor() {
        let p = Petition::size(100.0, 50.0);
        assert_eq!(p.width, Some(100.0));
        assert_eq!(p.height, Some(50.0));
    }
}

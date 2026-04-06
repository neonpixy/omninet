use serde::{Deserialize, Serialize};

/// Sanctum attachment edge. Determines which edge of the parent a sanctum claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Border {
    /// Attached to the top edge.
    Top,
    /// Attached to the bottom edge.
    Bottom,
    /// Attached to the leading (left in LTR) edge.
    Leading,
    /// Attached to the trailing (right in LTR) edge.
    Trailing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip() {
        let b = Border::Leading;
        let json = serde_json::to_string(&b).unwrap();
        let decoded: Border = serde_json::from_str(&json).unwrap();
        assert_eq!(b, decoded);
    }
}

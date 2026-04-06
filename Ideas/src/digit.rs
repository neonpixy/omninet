use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use x::{Value, VectorClock};

use crate::error::IdeasError;
use crate::validation;

/// A Digit is the atomic unit of content in Omnidea.
///
/// Everything is made of Digits — text, images, code, documents.
/// Immutable fields (id, type, created, author) cannot change after creation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Digit {
    id: Uuid,
    #[serde(rename = "type")]
    digit_type: String,
    pub content: Value,
    #[serde(default)]
    pub properties: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Uuid>>,
    created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    author: String,
    #[serde(default)]
    pub vector: VectorClock,
    #[serde(default)]
    pub tombstone: bool,
}

impl Digit {
    /// Maximum length for a digit type string.
    pub const MAX_TYPE_LENGTH: usize = 64;
    /// Maximum length for a property key string.
    pub const MAX_PROPERTY_KEY_LENGTH: usize = 64;

    /// Creates a new Digit with the given type, content, and author.
    ///
    /// The digit type is validated against the `[a-z][a-z0-9.-]*` pattern.
    /// Returns an error if the type string is invalid.
    pub fn new(
        digit_type: String,
        content: Value,
        author: String,
    ) -> Result<Self, IdeasError> {
        validation::validate_digit_type(&digit_type)?;
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            digit_type,
            content,
            properties: HashMap::new(),
            children: None,
            created: now,
            modified: now,
            author,
            vector: VectorClock::new(),
            tombstone: false,
        })
    }

    /// The unique identifier for this digit, assigned at creation.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The type string for this digit (e.g., `"text"`, `"media.image"`).
    pub fn digit_type(&self) -> &str {
        &self.digit_type
    }

    /// When this digit was first created.
    pub fn created(&self) -> DateTime<Utc> {
        self.created
    }

    /// The Crown public key of the person who created this digit.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Returns a new digit with updated content.
    pub fn with_content(&self, new_content: Value, by: &str) -> Self {
        let mut copy = self.clone();
        copy.content = new_content;
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a new digit with a property set.
    pub fn with_property(&self, key: String, value: Value, by: &str) -> Self {
        let mut copy = self.clone();
        copy.properties.insert(key, value);
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a new digit with a child added.
    pub fn with_child(&self, child_id: Uuid, by: &str) -> Self {
        let mut copy = self.clone();
        match &mut copy.children {
            Some(children) => children.push(child_id),
            None => copy.children = Some(vec![child_id]),
        }
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a new digit with a child removed.
    pub fn without_child(&self, child_id: Uuid, by: &str) -> Self {
        let mut copy = self.clone();
        if let Some(ref mut children) = copy.children {
            children.retain(|id| *id != child_id);
            if children.is_empty() {
                copy.children = None;
            }
        }
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a new digit with children reordered.
    /// `new_order` must contain exactly the same UUIDs as the current children.
    pub fn with_children_reordered(&self, new_order: Vec<Uuid>, by: &str) -> Self {
        let mut copy = self.clone();
        copy.children = if new_order.is_empty() {
            None
        } else {
            Some(new_order)
        };
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a new digit with a child inserted at a specific index.
    /// If index >= current length, appends to the end.
    pub fn with_child_at(&self, index: usize, child_id: Uuid, by: &str) -> Self {
        let mut copy = self.clone();
        match &mut copy.children {
            Some(children) => {
                let pos = index.min(children.len());
                children.insert(pos, child_id);
            }
            None => copy.children = Some(vec![child_id]),
        }
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a tombstoned copy of this digit.
    pub fn deleted(&self, by: &str) -> Self {
        let mut copy = self.clone();
        copy.tombstone = true;
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Returns a restored (un-tombstoned) copy of this digit.
    pub fn restored(&self, by: &str) -> Self {
        let mut copy = self.clone();
        copy.tombstone = false;
        copy.modified = Utc::now();
        copy.vector.increment(by);
        copy
    }

    /// Whether this digit has been soft-deleted (tombstoned).
    pub fn is_deleted(&self) -> bool {
        self.tombstone
    }

    /// Whether this digit has any child digit references.
    pub fn has_children(&self) -> bool {
        self.children.as_ref().is_some_and(|c| !c.is_empty())
    }

    /// Validates this digit, checking type and property keys.
    pub fn validate(&self) -> Result<(), IdeasError> {
        validation::validate_digit_type(&self.digit_type)?;
        for key in self.properties.keys() {
            validation::validate_property_key(key)?;
        }
        Ok(())
    }

    /// Returns accessibility metadata if present on this digit.
    pub fn accessibility(&self) -> Option<crate::accessibility::AccessibilityMetadata> {
        crate::accessibility::accessibility_metadata(self)
    }

    /// Extracts all text content for search indexing.
    pub fn extract_text(&self) -> String {
        let mut parts = Vec::new();
        if let Some(text) = self.content.as_str() {
            parts.push(text.to_string());
        }
        for value in self.properties.values() {
            if let Some(text) = value.as_str() {
                parts.push(text.to_string());
            }
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_digit() {
        let d = Digit::new("text".into(), Value::from("Hello"), "cpub1test".into()).unwrap();
        assert_eq!(d.digit_type(), "text");
        assert_eq!(d.author(), "cpub1test");
        assert!(!d.is_deleted());
        assert!(!d.has_children());
    }

    #[test]
    fn invalid_type_rejected() {
        assert!(Digit::new("Text".into(), Value::Null, "a".into()).is_err());
        assert!(Digit::new("1bad".into(), Value::Null, "a".into()).is_err());
    }

    #[test]
    fn immutable_fields_preserved() {
        let d = Digit::new("text".into(), Value::from("v1"), "alice".into()).unwrap();
        let orig_id = d.id();
        let orig_created = d.created();
        let orig_type = d.digit_type().to_string();

        let d2 = d.with_content(Value::from("v2"), "bob");
        assert_eq!(d2.id(), orig_id);
        assert_eq!(d2.created(), orig_created);
        assert_eq!(d2.digit_type(), &orig_type);
        assert_eq!(d2.author(), "alice"); // Author doesn't change
    }

    #[test]
    fn with_content_updates_modified_and_vector() {
        let d = Digit::new("text".into(), Value::from("v1"), "alice".into()).unwrap();
        let d2 = d.with_content(Value::from("v2"), "alice");
        assert!(d2.modified >= d.modified);
        assert_eq!(d2.vector.count_for("alice"), 1);
    }

    #[test]
    fn with_property() {
        let d = Digit::new("text".into(), Value::from("hello"), "alice".into()).unwrap();
        let d2 = d.with_property("font".into(), Value::from("mono"), "alice");
        assert_eq!(d2.properties.get("font"), Some(&Value::from("mono")));
    }

    #[test]
    fn with_child() {
        let d = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        assert!(!d.has_children());
        let child_id = Uuid::new_v4();
        let d2 = d.with_child(child_id, "alice");
        assert!(d2.has_children());
        assert_eq!(d2.children.as_ref().unwrap(), &[child_id]);
    }

    #[test]
    fn without_child() {
        let d = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        let d = d.with_child(c1, "alice").with_child(c2, "alice");
        assert_eq!(d.children.as_ref().unwrap().len(), 2);

        let d = d.without_child(c1, "alice");
        assert_eq!(d.children.as_ref().unwrap(), &[c2]);

        // Removing last child sets children to None
        let d = d.without_child(c2, "alice");
        assert!(d.children.is_none());
    }

    #[test]
    fn without_child_nonexistent() {
        let d = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        let c1 = Uuid::new_v4();
        let d = d.with_child(c1, "alice");
        let d = d.without_child(Uuid::new_v4(), "alice"); // remove non-existent
        assert_eq!(d.children.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn with_child_at() {
        let d = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        let c3 = Uuid::new_v4();
        let d = d.with_child(c1, "alice").with_child(c3, "alice");
        // Insert c2 at index 1 (between c1 and c3)
        let d = d.with_child_at(1, c2, "alice");
        assert_eq!(d.children.as_ref().unwrap(), &[c1, c2, c3]);
    }

    #[test]
    fn with_child_at_beyond_end() {
        let d = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        let d = d.with_child(c1, "alice");
        let d = d.with_child_at(100, c2, "alice"); // beyond end = append
        assert_eq!(d.children.as_ref().unwrap(), &[c1, c2]);
    }

    #[test]
    fn with_children_reordered() {
        let d = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        let c3 = Uuid::new_v4();
        let d = d
            .with_child(c1, "alice")
            .with_child(c2, "alice")
            .with_child(c3, "alice");
        let d = d.with_children_reordered(vec![c3, c1, c2], "alice");
        assert_eq!(d.children.as_ref().unwrap(), &[c3, c1, c2]);
    }

    #[test]
    fn tombstone_lifecycle() {
        let d = Digit::new("text".into(), Value::from("hi"), "alice".into()).unwrap();
        assert!(!d.is_deleted());
        let deleted = d.deleted("alice");
        assert!(deleted.is_deleted());
        let restored = deleted.restored("alice");
        assert!(!restored.is_deleted());
    }

    #[test]
    fn extract_text() {
        let d = Digit::new("text".into(), Value::from("hello"), "alice".into())
            .unwrap()
            .with_property("title".into(), Value::from("world"), "alice");
        let text = d.extract_text();
        assert!(text.contains("hello"));
        assert!(text.contains("world"));
    }

    #[test]
    fn serde_round_trip() {
        let d = Digit::new("text".into(), Value::from("hello"), "cpub1alice".into())
            .unwrap()
            .with_property("size".into(), Value::Double(16.0), "cpub1alice");

        let json = serde_json::to_string_pretty(&d).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.id(), d.id());
        assert_eq!(rt.digit_type(), d.digit_type());
        assert_eq!(rt.content, d.content);
        assert_eq!(rt.properties, d.properties);
        assert_eq!(rt.author(), d.author());
    }
}

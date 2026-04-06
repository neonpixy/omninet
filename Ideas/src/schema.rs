//! Content schema system for Digits.
//!
//! Schemas are optional, serializable definitions of what a Digit type should
//! contain. They live alongside Digits as data — never code — so they can be
//! shared, versioned, and discovered across the network via gospel events.
//!
//! Untyped Digits still work. Schemas are opt-in validation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use x::Value;

use crate::digit::Digit;
use crate::validation;

// ── Property Types ──

/// The type of a property value, mapping 1:1 to `Value` variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PropertyType {
    /// A text string.
    String,
    /// A 64-bit signed integer.
    Int,
    /// A 64-bit floating-point number.
    Double,
    /// A boolean (true/false).
    Bool,
    /// A date/time value.
    Date,
    /// Raw binary data.
    Data,
    /// An ordered list of values.
    Array,
    /// A key-value dictionary.
    Dict,
}

impl PropertyType {
    /// Returns `true` if the given `Value` matches this property type.
    pub fn matches(&self, value: &Value) -> bool {
        matches!(
            (self, value),
            (PropertyType::String, Value::String(_))
                | (PropertyType::Int, Value::Int(_))
                | (PropertyType::Double, Value::Double(_))
                | (PropertyType::Bool, Value::Bool(_))
                | (PropertyType::Date, Value::Date(_))
                | (PropertyType::Data, Value::Data(_))
                | (PropertyType::Array, Value::Array(_))
                | (PropertyType::Dict, Value::Dictionary(_))
        )
    }

    /// Human-readable name for error messages.
    fn display_name(&self) -> &'static str {
        match self {
            PropertyType::String => "string",
            PropertyType::Int => "int",
            PropertyType::Double => "double",
            PropertyType::Bool => "bool",
            PropertyType::Date => "date",
            PropertyType::Data => "data",
            PropertyType::Array => "array",
            PropertyType::Dict => "dict",
        }
    }
}

impl std::fmt::Display for PropertyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Returns the `PropertyType` of a `Value`, or `None` for `Value::Null`.
fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Double(_) => "double",
        Value::String(_) => "string",
        Value::Date(_) => "date",
        Value::Data(_) => "data",
        Value::Array(_) => "array",
        Value::Dictionary(_) => "dict",
    }
}

// ── Property Definition ──

/// Defines a single property within a schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyDef {
    /// The expected value type.
    pub property_type: PropertyType,

    /// Whether this property must be present on the Digit.
    #[serde(default)]
    pub required: bool,

    /// Default value applied when the property is absent. Must match
    /// `property_type`. Only meaningful for optional properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,

    /// Human-readable description of the property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ── Validation Errors ──

/// A single validation failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValidationError {
    /// A required property is missing from the Digit.
    MissingRequired {
        property: String,
    },

    /// A property's value does not match the expected type.
    TypeMismatch {
        property: String,
        expected: String,
        actual: String,
    },

    /// The Digit's type string does not match the schema's target type.
    WrongDigitType {
        expected: String,
        actual: String,
    },

    /// A default value's type does not match the property's declared type.
    InvalidDefault {
        property: String,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MissingRequired { property } => {
                write!(f, "required property '{property}' is missing")
            }
            ValidationError::TypeMismatch {
                property,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "property '{property}' has type '{actual}', expected '{expected}'"
                )
            }
            ValidationError::WrongDigitType { expected, actual } => {
                write!(f, "digit type is '{actual}', schema expects '{expected}'")
            }
            ValidationError::InvalidDefault {
                property,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "default for '{property}' has type '{actual}', expected '{expected}'"
                )
            }
        }
    }
}

// ── Migration Hint ──

/// A hint describing how to migrate from a previous schema version.
///
/// Migration hints are informational. They tell apps what changed so they can
/// handle upgrades — but the migration itself happens in app code, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MigrationHint {
    /// A new property was introduced in this version.
    PropertyAdded {
        property: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<Value>,
    },

    /// A property was removed.
    PropertyRemoved { property: String },

    /// A property was renamed.
    PropertyRenamed { from: String, to: String },

    /// A property's type changed.
    TypeChanged {
        property: String,
        from: String,
        to: String,
    },

    /// A property changed from optional to required.
    BecameRequired { property: String },

    /// A property changed from required to optional.
    BecameOptional { property: String },
}

// ── Schema Version ──

/// Version metadata for a schema.
///
/// Versions follow the `type.v{N}` convention: `brand.logo.v1`, `brand.logo.v2`.
/// The `from_version` field and `migration_hints` describe how to upgrade from
/// the immediately preceding version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaVersion {
    /// The version number (e.g. 1, 2, 3).
    pub version: u32,

    /// The previous version this one migrates from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_version: Option<u32>,

    /// Migration hints describing what changed since `from_version`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migration_hints: Vec<MigrationHint>,
}

// ── DigitSchema ──

/// A schema defining the expected structure of a Digit type.
///
/// Schemas are pure data — serializable, shareable, discoverable via gospel.
/// They never execute code; validation is a simple property-type check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DigitSchema {
    /// The Digit type this schema validates (e.g. `"brand.logo"`).
    pub digit_type: String,

    /// Version metadata.
    pub version: SchemaVersion,

    /// Property definitions, keyed by property name.
    #[serde(default)]
    pub properties: HashMap<String, PropertyDef>,

    /// Schema(s) this one extends. Properties from base schemas are inherited.
    /// If a property appears in both base and extension, the extension wins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<String>,

    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl DigitSchema {
    /// Creates a new schema for the given Digit type at version 1.
    pub fn new(digit_type: String) -> Self {
        DigitSchema {
            digit_type,
            version: SchemaVersion {
                version: 1,
                from_version: None,
                migration_hints: Vec::new(),
            },
            properties: HashMap::new(),
            extends: Vec::new(),
            description: None,
        }
    }

    /// Builder: add a required property.
    pub fn with_required(mut self, name: &str, property_type: PropertyType) -> Self {
        self.properties.insert(
            name.to_string(),
            PropertyDef {
                property_type,
                required: true,
                default: None,
                description: None,
            },
        );
        self
    }

    /// Builder: add an optional property.
    pub fn with_optional(mut self, name: &str, property_type: PropertyType) -> Self {
        self.properties.insert(
            name.to_string(),
            PropertyDef {
                property_type,
                required: false,
                default: None,
                description: None,
            },
        );
        self
    }

    /// Builder: add an optional property with a default value.
    pub fn with_default(
        mut self,
        name: &str,
        property_type: PropertyType,
        default: Value,
    ) -> Self {
        self.properties.insert(
            name.to_string(),
            PropertyDef {
                property_type,
                required: false,
                default: Some(default),
                description: None,
            },
        );
        self
    }

    /// Builder: set a base schema to extend.
    pub fn extending(mut self, base_type: &str) -> Self {
        self.extends.push(base_type.to_string());
        self
    }

    /// Builder: set the description.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// The versioned type string: `"{digit_type}.v{version}"`.
    pub fn versioned_type(&self) -> String {
        format!("{}.v{}", self.digit_type, self.version.version)
    }

    /// Validates the schema definition itself (not a Digit against it).
    ///
    /// Checks that defaults match their declared types and that property keys
    /// follow Digit property key rules.
    pub fn validate_self(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for (key, def) in &self.properties {
            // Validate property key format
            if validation::validate_property_key(key).is_err() {
                errors.push(ValidationError::TypeMismatch {
                    property: key.clone(),
                    expected: "valid property key".to_string(),
                    actual: key.clone(),
                });
            }

            // Validate default type matches declared type
            if let Some(ref default) = def.default
                && !def.property_type.matches(default)
            {
                errors.push(ValidationError::InvalidDefault {
                    property: key.clone(),
                    expected: def.property_type.display_name().to_string(),
                    actual: value_type_name(default).to_string(),
                });
            }
        }

        errors
    }
}

// ── Validation ──

/// Validates a Digit against a schema.
///
/// Returns `Ok(())` if the Digit satisfies the schema, or a list of every
/// violation found. Validation is purely local — no network call, no registry
/// lookup. Pass in resolved properties (after composition) if using `extends`.
pub fn validate(digit: &Digit, schema: &DigitSchema) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check digit type matches
    if digit.digit_type() != schema.digit_type {
        errors.push(ValidationError::WrongDigitType {
            expected: schema.digit_type.clone(),
            actual: digit.digit_type().to_string(),
        });
    }

    for (key, def) in &schema.properties {
        match digit.properties.get(key) {
            Some(value) => {
                // Property exists — check type
                if !def.property_type.matches(value) {
                    errors.push(ValidationError::TypeMismatch {
                        property: key.clone(),
                        expected: def.property_type.display_name().to_string(),
                        actual: value_type_name(value).to_string(),
                    });
                }
            }
            None => {
                // Property absent — error only if required
                if def.required {
                    errors.push(ValidationError::MissingRequired {
                        property: key.clone(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates a Digit against a composed schema (with base schema resolution).
///
/// Resolves `extends` chains using the provided registry, then validates
/// against the merged property set. Extension properties override base
/// properties of the same name.
pub fn validate_composed(
    digit: &Digit,
    schema: &DigitSchema,
    registry: &SchemaRegistry,
) -> Result<(), Vec<ValidationError>> {
    let resolved = registry.resolve(schema);
    validate(digit, &resolved)
}

// ── Schema Registry ──

/// A local registry of schemas, keyed by versioned type string.
///
/// The registry is a pure in-memory data structure. In production, schemas
/// are loaded from gospel events or local storage into the registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaRegistry {
    /// Schemas keyed by versioned type string (e.g. `"brand.logo.v1"`).
    schemas: HashMap<String, DigitSchema>,
}

impl SchemaRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a schema. Overwrites any existing schema with the same
    /// versioned type string.
    pub fn register(&mut self, schema: DigitSchema) {
        let key = schema.versioned_type();
        self.schemas.insert(key, schema);
    }

    /// Looks up a schema by its versioned type string.
    pub fn get(&self, versioned_type: &str) -> Option<&DigitSchema> {
        self.schemas.get(versioned_type)
    }

    /// Looks up the latest version of a schema for a given base type.
    ///
    /// Scans all registered schemas whose `digit_type` matches and returns
    /// the one with the highest version number.
    pub fn latest(&self, digit_type: &str) -> Option<&DigitSchema> {
        self.schemas
            .values()
            .filter(|s| s.digit_type == digit_type)
            .max_by_key(|s| s.version.version)
    }

    /// Returns all registered schemas.
    pub fn all(&self) -> impl Iterator<Item = &DigitSchema> {
        self.schemas.values()
    }

    /// Returns all versions of a given base type, ordered by version number.
    pub fn versions_of(&self, digit_type: &str) -> Vec<&DigitSchema> {
        let mut versions: Vec<&DigitSchema> = self
            .schemas
            .values()
            .filter(|s| s.digit_type == digit_type)
            .collect();
        versions.sort_by_key(|s| s.version.version);
        versions
    }

    /// Number of schemas in the registry.
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    /// Resolves a schema by flattening its `extends` chain.
    ///
    /// Walks each base type (by latest version), collects their properties,
    /// then overlays the extension's own properties. Extension wins ties.
    /// If a base type is not found in the registry, it is silently skipped.
    pub fn resolve(&self, schema: &DigitSchema) -> DigitSchema {
        let mut merged_properties: HashMap<String, PropertyDef> = HashMap::new();

        // Collect base properties (earlier extends are lower priority)
        for base_type in &schema.extends {
            if let Some(base_schema) = self.latest(base_type) {
                // Recursively resolve the base schema
                let resolved_base = self.resolve(base_schema);
                for (key, def) in resolved_base.properties {
                    merged_properties.insert(key, def);
                }
            }
        }

        // Overlay extension's own properties (highest priority)
        for (key, def) in &schema.properties {
            merged_properties.insert(key.clone(), def.clone());
        }

        DigitSchema {
            digit_type: schema.digit_type.clone(),
            version: schema.version.clone(),
            properties: merged_properties,
            // Resolved schema has no extends — it's fully flattened
            extends: Vec::new(),
            description: schema.description.clone(),
        }
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // -- PropertyType matching --

    #[test]
    fn property_type_matches_correct_value() {
        assert!(PropertyType::String.matches(&Value::String("hello".into())));
        assert!(PropertyType::Int.matches(&Value::Int(42)));
        assert!(PropertyType::Double.matches(&Value::Double(3.125)));
        assert!(PropertyType::Bool.matches(&Value::Bool(true)));
        assert!(PropertyType::Date.matches(&Value::Date(chrono::Utc::now())));
        assert!(PropertyType::Data.matches(&Value::Data(vec![1, 2, 3])));
        assert!(PropertyType::Array.matches(&Value::Array(vec![])));
        assert!(PropertyType::Dict.matches(&Value::Dictionary(HashMap::new())));
    }

    #[test]
    fn property_type_rejects_wrong_value() {
        assert!(!PropertyType::String.matches(&Value::Int(42)));
        assert!(!PropertyType::Int.matches(&Value::String("hi".into())));
        assert!(!PropertyType::Double.matches(&Value::Bool(false)));
        assert!(!PropertyType::Bool.matches(&Value::Null));
        assert!(!PropertyType::Array.matches(&Value::String("nope".into())));
        assert!(!PropertyType::Dict.matches(&Value::Array(vec![])));
    }

    #[test]
    fn property_type_never_matches_null() {
        // Null is not a schema-representable type; it's the absence of a value
        assert!(!PropertyType::String.matches(&Value::Null));
        assert!(!PropertyType::Int.matches(&Value::Null));
        assert!(!PropertyType::Bool.matches(&Value::Null));
    }

    // -- DigitSchema construction --

    #[test]
    fn schema_builder() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String)
            .with_optional("width", PropertyType::Int)
            .with_default("color", PropertyType::String, Value::from("#000000"))
            .with_description("A brand logo");

        assert_eq!(schema.digit_type, "brand.logo");
        assert_eq!(schema.version.version, 1);
        assert_eq!(schema.properties.len(), 3);
        assert!(schema.properties["name"].required);
        assert!(!schema.properties["width"].required);
        assert!(!schema.properties["color"].required);
        assert_eq!(
            schema.properties["color"].default,
            Some(Value::from("#000000"))
        );
        assert_eq!(schema.description.as_deref(), Some("A brand logo"));
    }

    #[test]
    fn versioned_type_string() {
        let schema = DigitSchema::new("brand.logo".into());
        assert_eq!(schema.versioned_type(), "brand.logo.v1");

        let v2 = DigitSchema {
            digit_type: "brand.logo".into(),
            version: SchemaVersion {
                version: 2,
                from_version: Some(1),
                migration_hints: vec![MigrationHint::PropertyAdded {
                    property: "alt".into(),
                    default: None,
                }],
            },
            properties: HashMap::new(),
            extends: Vec::new(),
            description: None,
        };
        assert_eq!(v2.versioned_type(), "brand.logo.v2");
    }

    // -- Schema self-validation --

    #[test]
    fn validate_self_clean_schema() {
        let schema = DigitSchema::new("text".into())
            .with_required("content", PropertyType::String)
            .with_default("fontSize", PropertyType::Double, Value::Double(16.0));

        let errors = schema.validate_self();
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn validate_self_catches_bad_default_type() {
        let schema = DigitSchema::new("text".into())
            .with_default("fontSize", PropertyType::Double, Value::from("sixteen"));

        let errors = schema.validate_self();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ValidationError::InvalidDefault { property, .. } if property == "fontSize"));
    }

    #[test]
    fn validate_self_catches_bad_property_key() {
        let mut schema = DigitSchema::new("text".into());
        schema.properties.insert(
            "1invalid".to_string(),
            PropertyDef {
                property_type: PropertyType::String,
                required: false,
                default: None,
                description: None,
            },
        );

        let errors = schema.validate_self();
        assert!(!errors.is_empty());
    }

    // -- Digit validation --

    #[test]
    fn validate_digit_passes_with_all_required_present() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String)
            .with_required("width", PropertyType::Int);

        let digit = Digit::new("brand.logo".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("name".into(), Value::from("Acme"), "alice")
            .with_property("width".into(), Value::Int(256), "alice");

        assert!(validate(&digit, &schema).is_ok());
    }

    #[test]
    fn validate_digit_fails_on_missing_required() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String)
            .with_required("width", PropertyType::Int);

        let digit = Digit::new("brand.logo".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("name".into(), Value::from("Acme"), "alice");
        // Missing "width"

        let result = validate(&digit, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::MissingRequired { property } if property == "width")
        );
    }

    #[test]
    fn validate_digit_fails_on_type_mismatch() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("width", PropertyType::Int);

        let digit = Digit::new("brand.logo".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("width".into(), Value::from("not a number"), "alice");

        let result = validate(&digit, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ValidationError::TypeMismatch { property, expected, actual }
            if property == "width" && expected == "int" && actual == "string"
        ));
    }

    #[test]
    fn validate_digit_allows_extra_properties() {
        let schema = DigitSchema::new("text".into())
            .with_required("content", PropertyType::String);

        let digit = Digit::new("text".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("content".into(), Value::from("hello"), "alice")
            .with_property("extraStuff".into(), Value::Int(99), "alice");

        // Extra properties are fine — schemas are not restrictive
        assert!(validate(&digit, &schema).is_ok());
    }

    #[test]
    fn validate_digit_optional_property_absent_is_ok() {
        let schema = DigitSchema::new("text".into())
            .with_optional("color", PropertyType::String);

        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(validate(&digit, &schema).is_ok());
    }

    #[test]
    fn validate_digit_optional_property_present_must_match_type() {
        let schema = DigitSchema::new("text".into())
            .with_optional("color", PropertyType::String);

        let digit = Digit::new("text".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("color".into(), Value::Int(255), "alice");

        let result = validate(&digit, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn validate_wrong_digit_type() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String);

        let digit = Digit::new("text".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("name".into(), Value::from("Acme"), "alice");

        let result = validate(&digit, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::WrongDigitType { .. })));
    }

    #[test]
    fn validate_collects_multiple_errors() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String)
            .with_required("width", PropertyType::Int)
            .with_required("height", PropertyType::Int);

        // All three properties missing
        let digit = Digit::new("brand.logo".into(), Value::Null, "alice".into()).unwrap();

        let result = validate(&digit, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn validate_with_default_values() {
        // Defaults don't affect validation — they're informational for apps.
        // If an optional property with a default is absent, that's still valid.
        let schema = DigitSchema::new("text".into())
            .with_default("fontSize", PropertyType::Double, Value::Double(16.0));

        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(validate(&digit, &schema).is_ok());
    }

    #[test]
    fn validate_all_property_types() {
        let schema = DigitSchema::new("test.all".into())
            .with_required("s", PropertyType::String)
            .with_required("i", PropertyType::Int)
            .with_required("d", PropertyType::Double)
            .with_required("b", PropertyType::Bool)
            .with_required("dt", PropertyType::Date)
            .with_required("data", PropertyType::Data)
            .with_required("arr", PropertyType::Array)
            .with_required("dict", PropertyType::Dict);

        let mut digit = Digit::new("test.all".into(), Value::Null, "alice".into()).unwrap();
        digit = digit.with_property("s".into(), Value::from("hello"), "alice");
        digit = digit.with_property("i".into(), Value::Int(42), "alice");
        digit = digit.with_property("d".into(), Value::Double(3.125), "alice");
        digit = digit.with_property("b".into(), Value::Bool(true), "alice");
        digit = digit.with_property("dt".into(), Value::Date(chrono::Utc::now()), "alice");
        digit = digit.with_property("data".into(), Value::Data(vec![1, 2, 3]), "alice");
        digit = digit.with_property("arr".into(), Value::Array(vec![Value::Int(1)]), "alice");
        digit = digit.with_property(
            "dict".into(),
            Value::Dictionary(HashMap::new()),
            "alice",
        );

        assert!(validate(&digit, &schema).is_ok());
    }

    // -- Schema Registry --

    #[test]
    fn registry_register_and_get() {
        let mut reg = SchemaRegistry::new();
        assert!(reg.is_empty());

        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String);

        reg.register(schema.clone());
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());

        let found = reg.get("brand.logo.v1").unwrap();
        assert_eq!(found.digit_type, "brand.logo");
        assert_eq!(found.version.version, 1);
    }

    #[test]
    fn registry_get_missing() {
        let reg = SchemaRegistry::new();
        assert!(reg.get("nonexistent.v1").is_none());
    }

    #[test]
    fn registry_latest_version() {
        let mut reg = SchemaRegistry::new();

        let v1 = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String);

        let v2 = DigitSchema {
            digit_type: "brand.logo".into(),
            version: SchemaVersion {
                version: 2,
                from_version: Some(1),
                migration_hints: vec![MigrationHint::PropertyAdded {
                    property: "alt".into(),
                    default: Some(Value::from("")),
                }],
            },
            properties: {
                let mut p = HashMap::new();
                p.insert(
                    "name".to_string(),
                    PropertyDef {
                        property_type: PropertyType::String,
                        required: true,
                        default: None,
                        description: None,
                    },
                );
                p.insert(
                    "alt".to_string(),
                    PropertyDef {
                        property_type: PropertyType::String,
                        required: false,
                        default: Some(Value::from("")),
                        description: Some("Alt text".into()),
                    },
                );
                p
            },
            extends: Vec::new(),
            description: None,
        };

        reg.register(v1);
        reg.register(v2);

        let latest = reg.latest("brand.logo").unwrap();
        assert_eq!(latest.version.version, 2);
        assert!(latest.properties.contains_key("alt"));
    }

    #[test]
    fn registry_versions_of() {
        let mut reg = SchemaRegistry::new();

        let v1 = DigitSchema::new("brand.logo".into());
        let v2 = DigitSchema {
            digit_type: "brand.logo".into(),
            version: SchemaVersion {
                version: 2,
                from_version: Some(1),
                migration_hints: Vec::new(),
            },
            properties: HashMap::new(),
            extends: Vec::new(),
            description: None,
        };
        let v3 = DigitSchema {
            digit_type: "brand.logo".into(),
            version: SchemaVersion {
                version: 3,
                from_version: Some(2),
                migration_hints: Vec::new(),
            },
            properties: HashMap::new(),
            extends: Vec::new(),
            description: None,
        };

        // Register in non-sequential order
        reg.register(v3);
        reg.register(v1);
        reg.register(v2);

        let versions = reg.versions_of("brand.logo");
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version.version, 1);
        assert_eq!(versions[1].version.version, 2);
        assert_eq!(versions[2].version.version, 3);
    }

    #[test]
    fn registry_latest_returns_none_for_unknown_type() {
        let reg = SchemaRegistry::new();
        assert!(reg.latest("nonexistent").is_none());
    }

    #[test]
    fn registry_overwrite() {
        let mut reg = SchemaRegistry::new();

        let v1a = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String);

        let v1b = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String)
            .with_optional("alt", PropertyType::String);

        reg.register(v1a);
        assert_eq!(reg.get("brand.logo.v1").unwrap().properties.len(), 1);

        reg.register(v1b);
        assert_eq!(reg.get("brand.logo.v1").unwrap().properties.len(), 2);

        // Still only one entry — overwritten, not duplicated
        assert_eq!(reg.len(), 1);
    }

    // -- Composable schemas --

    #[test]
    fn schema_composition_inherits_base_properties() {
        let mut reg = SchemaRegistry::new();

        let base = DigitSchema::new("brand.asset".into())
            .with_required("name", PropertyType::String)
            .with_required("owner", PropertyType::String);

        let extension = DigitSchema::new("brand.logo".into())
            .extending("brand.asset")
            .with_required("width", PropertyType::Int)
            .with_required("height", PropertyType::Int);

        reg.register(base);

        let resolved = reg.resolve(&extension);
        assert_eq!(resolved.properties.len(), 4);
        assert!(resolved.properties.contains_key("name"));
        assert!(resolved.properties.contains_key("owner"));
        assert!(resolved.properties.contains_key("width"));
        assert!(resolved.properties.contains_key("height"));
        // Extends should be flattened away
        assert!(resolved.extends.is_empty());
    }

    #[test]
    fn schema_composition_extension_overrides_base() {
        let mut reg = SchemaRegistry::new();

        let base = DigitSchema::new("brand.asset".into())
            .with_required("name", PropertyType::String)
            .with_optional("color", PropertyType::String);

        // Extension makes "color" required
        let extension = DigitSchema::new("brand.logo".into())
            .extending("brand.asset")
            .with_required("color", PropertyType::String);

        reg.register(base);

        let resolved = reg.resolve(&extension);
        assert!(resolved.properties["color"].required);
    }

    #[test]
    fn schema_composition_missing_base_is_graceful() {
        let reg = SchemaRegistry::new();

        let extension = DigitSchema::new("brand.logo".into())
            .extending("nonexistent.base")
            .with_required("width", PropertyType::Int);

        let resolved = reg.resolve(&extension);
        // Only the extension's own properties
        assert_eq!(resolved.properties.len(), 1);
        assert!(resolved.properties.contains_key("width"));
    }

    #[test]
    fn schema_composition_multi_level() {
        let mut reg = SchemaRegistry::new();

        let base = DigitSchema::new("thing".into())
            .with_required("id", PropertyType::String);

        let mid = DigitSchema::new("brand.asset".into())
            .extending("thing")
            .with_required("name", PropertyType::String);

        let leaf = DigitSchema::new("brand.logo".into())
            .extending("brand.asset")
            .with_required("width", PropertyType::Int);

        reg.register(base);
        reg.register(mid);

        let resolved = reg.resolve(&leaf);
        assert_eq!(resolved.properties.len(), 3);
        assert!(resolved.properties.contains_key("id"));
        assert!(resolved.properties.contains_key("name"));
        assert!(resolved.properties.contains_key("width"));
    }

    #[test]
    fn schema_composition_multiple_bases() {
        let mut reg = SchemaRegistry::new();

        let visual = DigitSchema::new("visual".into())
            .with_required("width", PropertyType::Int)
            .with_required("height", PropertyType::Int);

        let named = DigitSchema::new("named".into())
            .with_required("name", PropertyType::String);

        let logo = DigitSchema::new("brand.logo".into())
            .extending("visual")
            .extending("named")
            .with_optional("alt", PropertyType::String);

        reg.register(visual);
        reg.register(named);

        let resolved = reg.resolve(&logo);
        assert_eq!(resolved.properties.len(), 4);
        assert!(resolved.properties.contains_key("width"));
        assert!(resolved.properties.contains_key("height"));
        assert!(resolved.properties.contains_key("name"));
        assert!(resolved.properties.contains_key("alt"));
    }

    // -- validate_composed --

    #[test]
    fn validate_composed_with_inheritance() {
        let mut reg = SchemaRegistry::new();

        let base = DigitSchema::new("brand.asset".into())
            .with_required("name", PropertyType::String);

        let extension = DigitSchema::new("brand.logo".into())
            .extending("brand.asset")
            .with_required("width", PropertyType::Int);

        reg.register(base);
        reg.register(extension.clone());

        // Digit satisfies both base and extension
        let good = Digit::new("brand.logo".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("name".into(), Value::from("Acme"), "alice")
            .with_property("width".into(), Value::Int(256), "alice");

        assert!(validate_composed(&good, &extension, &reg).is_ok());

        // Missing base-required "name"
        let bad = Digit::new("brand.logo".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("width".into(), Value::Int(256), "alice");

        let result = validate_composed(&bad, &extension, &reg);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingRequired { property } if property == "name")));
    }

    // -- Schema versioning --

    #[test]
    fn migration_hints_serde() {
        let hints = vec![
            MigrationHint::PropertyAdded {
                property: "alt".into(),
                default: Some(Value::from("")),
            },
            MigrationHint::PropertyRemoved {
                property: "legacyId".into(),
            },
            MigrationHint::PropertyRenamed {
                from: "colour".into(),
                to: "color".into(),
            },
            MigrationHint::TypeChanged {
                property: "size".into(),
                from: "string".into(),
                to: "int".into(),
            },
            MigrationHint::BecameRequired {
                property: "name".into(),
            },
            MigrationHint::BecameOptional {
                property: "title".into(),
            },
        ];

        let json = serde_json::to_string_pretty(&hints).unwrap();
        let rt: Vec<MigrationHint> = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.len(), 6);
        assert_eq!(rt, hints);
    }

    // -- Serde round-trips --

    #[test]
    fn schema_serde_round_trip() {
        let schema = DigitSchema::new("brand.logo".into())
            .with_required("name", PropertyType::String)
            .with_optional("width", PropertyType::Int)
            .with_default("color", PropertyType::String, Value::from("#FF0000"))
            .with_description("A brand logo image");

        let json = serde_json::to_string_pretty(&schema).unwrap();
        let rt: DigitSchema = serde_json::from_str(&json).unwrap();

        assert_eq!(rt.digit_type, schema.digit_type);
        assert_eq!(rt.version.version, schema.version.version);
        assert_eq!(rt.properties.len(), schema.properties.len());
        assert!(rt.properties["name"].required);
        assert!(!rt.properties["width"].required);
        assert_eq!(
            rt.properties["color"].default,
            Some(Value::from("#FF0000"))
        );
        assert_eq!(rt.description, schema.description);
    }

    #[test]
    fn property_def_serde_round_trip() {
        let def = PropertyDef {
            property_type: PropertyType::Array,
            required: true,
            default: None,
            description: Some("A list of tags".into()),
        };

        let json = serde_json::to_string(&def).unwrap();
        let rt: PropertyDef = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.property_type, PropertyType::Array);
        assert!(rt.required);
        assert!(rt.default.is_none());
        assert_eq!(rt.description.as_deref(), Some("A list of tags"));
    }

    #[test]
    fn validation_error_serde_round_trip() {
        let errors = vec![
            ValidationError::MissingRequired {
                property: "name".into(),
            },
            ValidationError::TypeMismatch {
                property: "width".into(),
                expected: "int".into(),
                actual: "string".into(),
            },
            ValidationError::WrongDigitType {
                expected: "brand.logo".into(),
                actual: "text".into(),
            },
            ValidationError::InvalidDefault {
                property: "color".into(),
                expected: "string".into(),
                actual: "int".into(),
            },
        ];

        let json = serde_json::to_string_pretty(&errors).unwrap();
        let rt: Vec<ValidationError> = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.len(), 4);
        assert_eq!(rt, errors);
    }

    #[test]
    fn registry_serde_round_trip() {
        let mut reg = SchemaRegistry::new();
        reg.register(
            DigitSchema::new("brand.logo".into())
                .with_required("name", PropertyType::String),
        );
        reg.register(
            DigitSchema::new("text".into())
                .with_optional("fontSize", PropertyType::Double),
        );

        let json = serde_json::to_string_pretty(&reg).unwrap();
        let rt: SchemaRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.len(), 2);
        assert!(rt.get("brand.logo.v1").is_some());
        assert!(rt.get("text.v1").is_some());
    }

    #[test]
    fn property_type_serde_round_trip() {
        let types = vec![
            PropertyType::String,
            PropertyType::Int,
            PropertyType::Double,
            PropertyType::Bool,
            PropertyType::Date,
            PropertyType::Data,
            PropertyType::Array,
            PropertyType::Dict,
        ];

        let json = serde_json::to_string(&types).unwrap();
        let rt: Vec<PropertyType> = serde_json::from_str(&json).unwrap();
        assert_eq!(rt, types);
    }

    #[test]
    fn schema_version_with_migration_serde() {
        let version = SchemaVersion {
            version: 2,
            from_version: Some(1),
            migration_hints: vec![
                MigrationHint::PropertyAdded {
                    property: "alt".into(),
                    default: Some(Value::from("")),
                },
                MigrationHint::BecameRequired {
                    property: "name".into(),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&version).unwrap();
        let rt: SchemaVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.version, 2);
        assert_eq!(rt.from_version, Some(1));
        assert_eq!(rt.migration_hints.len(), 2);
    }

    // -- Display impls --

    #[test]
    fn validation_error_display() {
        let e = ValidationError::MissingRequired {
            property: "name".into(),
        };
        assert_eq!(e.to_string(), "required property 'name' is missing");

        let e = ValidationError::TypeMismatch {
            property: "width".into(),
            expected: "int".into(),
            actual: "string".into(),
        };
        assert_eq!(
            e.to_string(),
            "property 'width' has type 'string', expected 'int'"
        );

        let e = ValidationError::WrongDigitType {
            expected: "brand.logo".into(),
            actual: "text".into(),
        };
        assert_eq!(
            e.to_string(),
            "digit type is 'text', schema expects 'brand.logo'"
        );
    }

    #[test]
    fn property_type_display() {
        assert_eq!(PropertyType::String.to_string(), "string");
        assert_eq!(PropertyType::Int.to_string(), "int");
        assert_eq!(PropertyType::Double.to_string(), "double");
        assert_eq!(PropertyType::Bool.to_string(), "bool");
        assert_eq!(PropertyType::Date.to_string(), "date");
        assert_eq!(PropertyType::Data.to_string(), "data");
        assert_eq!(PropertyType::Array.to_string(), "array");
        assert_eq!(PropertyType::Dict.to_string(), "dict");
    }

    // -- Edge cases --

    #[test]
    fn empty_schema_validates_any_digit_of_matching_type() {
        let schema = DigitSchema::new("text".into());
        let digit = Digit::new("text".into(), Value::Null, "alice".into())
            .unwrap()
            .with_property("anything".into(), Value::Int(42), "alice");

        assert!(validate(&digit, &schema).is_ok());
    }

    #[test]
    fn empty_schema_still_checks_digit_type() {
        let schema = DigitSchema::new("brand.logo".into());
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();

        let result = validate(&digit, &schema);
        assert!(result.is_err());
    }

    #[test]
    fn registry_all_iterates_everything() {
        let mut reg = SchemaRegistry::new();
        reg.register(DigitSchema::new("a".into()));
        reg.register(DigitSchema::new("b".into()));
        reg.register(DigitSchema::new("c".into()));

        let all: Vec<_> = reg.all().collect();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn schema_extending_builder() {
        let schema = DigitSchema::new("brand.logo".into())
            .extending("brand.asset")
            .extending("visual");

        assert_eq!(schema.extends, vec!["brand.asset", "visual"]);
    }
}

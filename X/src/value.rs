//! Polymorphic content type for the Omninet protocol.
//!
//! `Value` is the universal container -- any field in an `.idea`, any parameter
//! in a pipeline step, any piece of user data flows through this type. Custom
//! serde encodes each variant as a single-key JSON object so the type tag is
//! never ambiguous across languages.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;

/// A polymorphic value that can represent any content type.
///
/// Values are encoded as single-key objects for type disambiguation:
/// - `{ "null": true }`
/// - `{ "string": "hello" }`
/// - `{ "int": 42 }`
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// The absence of a value. Serializes as `{"null": true}`.
    Null,
    /// A boolean value. Serializes as `{"bool": true}` or `{"bool": false}`.
    Bool(bool),
    /// A 64-bit signed integer. Serializes as `{"int": 42}`.
    Int(i64),
    /// A 64-bit floating-point number. Serializes as `{"double": 3.14}`.
    Double(f64),
    /// A UTF-8 string. Serializes as `{"string": "hello"}`.
    String(String),
    /// A UTC timestamp. Serializes as `{"date": "<RFC3339>"}`.
    Date(DateTime<Utc>),
    /// Raw binary data. Serializes as `{"data": "<base64>"}`.
    Data(Vec<u8>),
    /// An ordered list of values. Serializes as `{"array": [...]}`.
    Array(Vec<Value>),
    /// A string-keyed map of values. Serializes as `{"dictionary": {...}}`.
    Dictionary(HashMap<String, Value>),
}

// -- Accessors --

impl Value {
    /// Returns `true` if this value is `Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Extracts the boolean, or `None` if this isn't a `Bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Extracts the integer, or `None` if this isn't an `Int`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Extracts the double, or `None` if this isn't a `Double`.
    pub fn as_double(&self) -> Option<f64> {
        match self {
            Value::Double(v) => Some(*v),
            _ => None,
        }
    }

    /// Extracts a string slice, or `None` if this isn't a `String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(v) => Some(v),
            _ => None,
        }
    }

    /// Extracts the timestamp, or `None` if this isn't a `Date`.
    pub fn as_date(&self) -> Option<&DateTime<Utc>> {
        match self {
            Value::Date(v) => Some(v),
            _ => None,
        }
    }

    /// Extracts the raw bytes, or `None` if this isn't `Data`.
    pub fn as_data(&self) -> Option<&[u8]> {
        match self {
            Value::Data(v) => Some(v),
            _ => None,
        }
    }

    /// Extracts the array slice, or `None` if this isn't an `Array`.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Extracts the dictionary, or `None` if this isn't a `Dictionary`.
    pub fn as_dictionary(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Value::Dictionary(v) => Some(v),
            _ => None,
        }
    }
}

// -- From impls --

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Double(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Data(v)
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::Array(v)
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(v: DateTime<Utc>) -> Self {
        Value::Date(v)
    }
}

// -- Custom serde: single-key pattern --

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Value::Null => map.serialize_entry("null", &true)?,
            Value::Bool(v) => map.serialize_entry("bool", v)?,
            Value::Int(v) => map.serialize_entry("int", v)?,
            Value::Double(v) => map.serialize_entry("double", v)?,
            Value::String(v) => map.serialize_entry("string", v)?,
            Value::Date(v) => {
                map.serialize_entry("date", &v.to_rfc3339())?;
            }
            Value::Data(v) => {
                map.serialize_entry("data", &BASE64.encode(v))?;
            }
            Value::Array(v) => map.serialize_entry("array", v)?,
            Value::Dictionary(v) => map.serialize_entry("dictionary", v)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a single-key map representing a Value")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        let key: String = map
            .next_key()?
            .ok_or_else(|| de::Error::custom("empty map for Value"))?;

        let value = match key.as_str() {
            "null" => {
                let _: bool = map.next_value()?;
                Value::Null
            }
            "bool" => Value::Bool(map.next_value()?),
            "int" => Value::Int(map.next_value()?),
            "double" => Value::Double(map.next_value()?),
            "string" => Value::String(map.next_value()?),
            "date" => {
                let s: String = map.next_value()?;
                let dt = DateTime::parse_from_rfc3339(&s)
                    .map(|d| d.with_timezone(&Utc))
                    .or_else(|_| {
                        // Try without fractional seconds
                        s.parse::<DateTime<Utc>>()
                    })
                    .map_err(|_| de::Error::custom(format!("invalid ISO8601 date: {s}")))?;
                Value::Date(dt)
            }
            "data" => {
                let s: String = map.next_value()?;
                let bytes = BASE64
                    .decode(&s)
                    .map_err(|_| de::Error::custom("invalid base64 data"))?;
                Value::Data(bytes)
            }
            "array" => Value::Array(map.next_value()?),
            "dictionary" => Value::Dictionary(map.next_value()?),
            other => return Err(de::Error::unknown_field(other, FIELDS)),
        };

        Ok(value)
    }
}

const FIELDS: &[&str] = &[
    "null",
    "bool",
    "int",
    "double",
    "string",
    "date",
    "data",
    "array",
    "dictionary",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(value: &Value) -> Value {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn null_round_trip() {
        let v = Value::Null;
        assert_eq!(round_trip(&v), v);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"null":true}"#);
    }

    #[test]
    fn bool_round_trip() {
        let v = Value::Bool(true);
        assert_eq!(round_trip(&v), v);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"bool":true}"#);

        let v2 = Value::Bool(false);
        assert_eq!(round_trip(&v2), v2);
    }

    #[test]
    fn int_round_trip() {
        let v = Value::Int(42);
        assert_eq!(round_trip(&v), v);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"int":42}"#);

        // Negative
        let v2 = Value::Int(-999);
        assert_eq!(round_trip(&v2), v2);
    }

    #[test]
    fn double_round_trip() {
        let v = Value::Double(42.195);
        assert_eq!(round_trip(&v), v);

        // Zero
        let v2 = Value::Double(0.0);
        assert_eq!(round_trip(&v2), v2);
    }

    #[test]
    fn string_round_trip() {
        let v = Value::String("Hello, Omnidea!".into());
        assert_eq!(round_trip(&v), v);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"{"string":"Hello, Omnidea!"}"#);

        // Empty string
        let v2 = Value::String(String::new());
        assert_eq!(round_trip(&v2), v2);
    }

    #[test]
    fn date_round_trip() {
        let dt = Utc::now();
        let v = Value::Date(dt);
        let rt = round_trip(&v);
        // Dates may lose sub-nanosecond precision in round-trip
        if let Value::Date(rt_dt) = &rt {
            assert!((dt - *rt_dt).num_milliseconds().abs() < 1);
        } else {
            panic!("expected Date variant");
        }
    }

    #[test]
    fn data_round_trip() {
        let v = Value::Data(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(round_trip(&v), v);

        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"data\":"));

        // Empty data
        let v2 = Value::Data(vec![]);
        assert_eq!(round_trip(&v2), v2);
    }

    #[test]
    fn array_round_trip() {
        let v = Value::Array(vec![
            Value::Int(1),
            Value::String("two".into()),
            Value::Bool(true),
        ]);
        assert_eq!(round_trip(&v), v);

        // Empty array
        let v2 = Value::Array(vec![]);
        assert_eq!(round_trip(&v2), v2);
    }

    #[test]
    fn dictionary_round_trip() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), Value::String("test".into()));
        map.insert("count".to_string(), Value::Int(5));
        let v = Value::Dictionary(map);
        assert_eq!(round_trip(&v), v);
    }

    #[test]
    fn nested_values() {
        let v = Value::Array(vec![
            Value::Dictionary({
                let mut m = HashMap::new();
                m.insert("inner".to_string(), Value::Array(vec![Value::Null]));
                m
            }),
        ]);
        assert_eq!(round_trip(&v), v);
    }

    #[test]
    fn from_impls() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42_i64), Value::Int(42));
        assert_eq!(Value::from(42.195_f64), Value::Double(42.195));
        assert_eq!(
            Value::from("hello"),
            Value::String("hello".into())
        );
        assert_eq!(
            Value::from(vec![1_u8, 2, 3]),
            Value::Data(vec![1, 2, 3])
        );
    }
}

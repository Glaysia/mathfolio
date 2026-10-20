//! Immutable runtime text storage, not source validation or evaluator memoization.
//! Wire is exactly the pre-existing plain JSON string. Scope and Value cloning
//! may share ownership; independently evaluated equal values are not interned.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, ops::Deref, sync::Arc};
#[derive(Clone, Eq, PartialEq)]
pub struct StringValue(Arc<str>);
impl From<String> for StringValue {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}
impl From<&str> for StringValue {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}
impl Deref for StringValue {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for StringValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for StringValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for StringValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl Serialize for StringValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for StringValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum LegacyValue {
        String { value: String },
    }
    #[test]
    fn wire_and_numeric_json_are_exactly_legacy_strings() {
        for text in [
            String::new(),
            "  a<&>\"\\\r\n\t한글𝛼𝄞  ".into(),
            "-0.0".into(),
            "\u{0}".into(),
            "a".repeat(crate::strings::MAX_STRING_BYTES),
        ] {
            let expected = serde_json::to_vec(&LegacyValue::String {
                value: text.clone(),
            })
            .unwrap();
            let value = Value::String {
                value: text.clone().into(),
            };
            assert_eq!(serde_json::to_vec(&value).unwrap(), expected);
            assert_eq!(
                value.numeric_json(),
                serde_json::json!({"kind":"string","value":text})
            );
            let decoded: Value = serde_json::from_slice(&expected).unwrap();
            assert_eq!(serde_json::to_vec(&decoded).unwrap(), expected);
            assert_eq!(
                format!("{value:?}"),
                format!("String {{ value: {text:?} }}")
            );
            assert_eq!(
                value.as_string().is_ok(),
                crate::strings::validate(&text).is_ok()
            );
        }
    }
    #[test]
    fn value_field_deserialization_errors_and_unknown_metadata_match_legacy() {
        for json in [
            r#"{"kind":"string"}"#,
            r#"{"kind":"string","value":null}"#,
            r#"{"kind":"string","value":4}"#,
            r#"{"kind":"string","value":[]}"#,
            r#"{"kind":"string","value":{}}"#,
            r#"{"kind":"string","value":"a","value":"b"}"#,
        ] {
            let old = serde_json::from_str::<LegacyValue>(json)
                .err()
                .unwrap()
                .to_string();
            let new = serde_json::from_str::<Value>(json)
                .err()
                .unwrap()
                .to_string();
            assert_eq!(new, old, "{json}");
        }
        // Generic Value preserves its old grammar; strict inert cache admission
        // is a different caller and is not weakened by this storage change.
        let json = r#"{"kind":"string","value":"a","future":0}"#;
        let old = serde_json::from_str::<LegacyValue>(json).unwrap();
        let new = serde_json::from_str::<Value>(json).unwrap();
        assert_eq!(
            serde_json::to_vec(&old).unwrap(),
            serde_json::to_vec(&new).unwrap()
        );
    }
    #[test]
    fn clone_shares_immutable_storage_but_equality_is_by_value_and_is_send_sync() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Value>();
        let first = StringValue::from("abc");
        let clone = first.clone();
        let separate = StringValue::from("abc");
        assert!(Arc::ptr_eq(&first.0, &clone.0));
        assert!(!Arc::ptr_eq(&first.0, &separate.0));
        assert_eq!(first, separate);
        let weak = Arc::downgrade(&first.0);
        drop(first);
        assert!(weak.upgrade().is_some());
        drop(clone);
        assert!(weak.upgrade().is_none());
        assert_eq!(separate.as_ref(), "abc");
    }
}

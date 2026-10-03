//! Deterministic canonical serialization.
//!
//! Every protocol object is hashed and signed over its canonical bytes:
//!
//! * JSON with object keys sorted lexicographically by their UTF-8 bytes;
//! * every string (keys and values) normalized to Unicode NFC;
//! * integers in plain decimal, with no exponent or leading zeros;
//! * floating-point numbers rejected;
//! * no insignificant whitespace.
//!
//! The serializer is written by hand instead of relying on `serde_json`'s map
//! ordering, so that a dependency enabling `preserve_order` can never change
//! protocol hashes.

use crate::error::{CoreError, Result};
use serde::Serialize;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

/// Deterministic byte encoding used for hashing and signing.
pub trait CanonicalSerialize {
    fn canonical_bytes(&self) -> Result<Vec<u8>>;

    /// BLAKE3 of the canonical bytes.
    fn canonical_hash(&self) -> Result<crate::Hash> {
        Ok(crate::Hash::digest(&self.canonical_bytes()?))
    }
}

impl<T: Serialize + ?Sized> CanonicalSerialize for T {
    fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let value =
            serde_json::to_value(self).map_err(|e| CoreError::Serialization(e.to_string()))?;
        canonical_value_bytes(&value)
    }
}

/// Canonical bytes of an already-built JSON value.
pub fn canonical_value_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    write_value(value, &mut out)?;
    Ok(out)
}

fn write_value(value: &Value, out: &mut Vec<u8>) -> Result<()> {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.extend_from_slice(i.to_string().as_bytes());
            } else if let Some(u) = n.as_u64() {
                out.extend_from_slice(u.to_string().as_bytes());
            } else {
                return Err(CoreError::FloatNotAllowed);
            }
        }
        Value::String(s) => write_string(s, out)?,
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(item, out)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(String, &Value)> = map.iter().map(|(k, v)| (nfc(k), v)).collect();
            entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push(b'{');
            for (i, (key, item)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_string(key, out)?;
                out.push(b':');
                write_value(item, out)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

fn write_string(s: &str, out: &mut Vec<u8>) -> Result<()> {
    // serde_json's string escaping is deterministic: it escapes only `"`, `\`
    // and control characters, and emits all other code points as raw UTF-8.
    let encoded =
        serde_json::to_string(&nfc(s)).map_err(|e| CoreError::Serialization(e.to_string()))?;
    out.extend_from_slice(encoded.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted_and_whitespace_removed() {
        let v = json!({"b": 1, "a": [true, null, "x"], "c": {"z": 0, "y": -5}});
        let bytes = canonical_value_bytes(&v).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"a":[true,null,"x"],"b":1,"c":{"y":-5,"z":0}}"#
        );
    }

    #[test]
    fn floats_are_rejected() {
        assert_eq!(
            canonical_value_bytes(&json!({"x": 1.5})),
            Err(CoreError::FloatNotAllowed)
        );
    }

    #[test]
    fn unicode_is_nfc_normalized() {
        // "é" as a precomposed code point and as e + combining acute accent.
        let composed = json!({"k": "caf\u{00e9}"});
        let decomposed = json!({"k": "cafe\u{0301}"});
        assert_eq!(
            canonical_value_bytes(&composed).unwrap(),
            canonical_value_bytes(&decomposed).unwrap()
        );
    }

    #[test]
    fn large_unsigned_integers_are_exact() {
        let bytes = canonical_value_bytes(&json!(u64::MAX)).unwrap();
        assert_eq!(bytes, u64::MAX.to_string().into_bytes());
    }
}

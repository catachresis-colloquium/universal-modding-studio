// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Canonical bytes for integer-only JSON, following RFC 8785 (JCS).
//!
//! The documents that cross the preview seam are integer-only I-JSON
//! (Enaction completion plan, ruling Q1 arm A): every number is an integer
//! with magnitude at most 2^53-1. For such documents JCS reduces to: object
//! members sorted by the UTF-16 code units of their names, no insignificant
//! whitespace, integers in plain decimal, and strings escaped as ECMAScript
//! `JSON.stringify` does. A float or an out-of-range integer is refused
//! rather than approximated, because JCS number formatting for floats is not
//! implemented here.
//!
//! Guarantee: tested (`canonical` unit tests below), not checked against the
//! RFC 8785 test vectors.

use serde_json::Value;
use std::fmt;

/// The largest integer magnitude admitted on the wire (2^53-1).
pub const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalError {
    /// A number that is not an integer, at the given JSON pointer.
    NonInteger { path: String },
    /// An integer whose magnitude exceeds 2^53-1, at the given JSON pointer.
    IntegerOutOfRange { path: String },
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonInteger { path } => write!(f, "NonInteger at {path}"),
            Self::IntegerOutOfRange { path } => {
                write!(f, "IntegerOutOfRange at {path} (limit 2^53-1)")
            }
        }
    }
}

impl std::error::Error for CanonicalError {}

/// Refuse any float or any integer outside +-(2^53-1).
pub fn check_integers(value: &Value) -> Result<(), CanonicalError> {
    walk(value, &mut String::new())
}

fn walk(value: &Value, path: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Number(number) => {
            let in_range = if let Some(unsigned) = number.as_u64() {
                unsigned <= MAX_SAFE_INTEGER
            } else if let Some(signed) = number.as_i64() {
                signed.unsigned_abs() <= MAX_SAFE_INTEGER
            } else {
                return Err(CanonicalError::NonInteger {
                    path: pointer(path),
                });
            };
            if in_range {
                Ok(())
            } else {
                Err(CanonicalError::IntegerOutOfRange {
                    path: pointer(path),
                })
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let len = path.len();
                path.push('/');
                path.push_str(&index.to_string());
                walk(item, path)?;
                path.truncate(len);
            }
            Ok(())
        }
        Value::Object(members) => {
            for (name, member) in members {
                let len = path.len();
                path.push('/');
                path.push_str(&name.replace('~', "~0").replace('/', "~1"));
                walk(member, path)?;
                path.truncate(len);
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

fn pointer(path: &str) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    }
}

/// Serialise an integer-only document to its canonical form.
pub fn to_canonical_string(value: &Value) -> Result<String, CanonicalError> {
    check_integers(value)?;
    let mut out = String::new();
    write_value(value, &mut out);
    Ok(out)
}

/// [`to_canonical_string`] as UTF-8 bytes.
pub fn to_canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    to_canonical_string(value).map(String::into_bytes)
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        // check_integers has already admitted only in-range integers.
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::String(text) => write_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => {
            let mut names: Vec<&String> = members.keys().collect();
            names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (index, name) in names.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_string(name, out);
                out.push(':');
                write_value(&members[name], out);
            }
            out.push('}');
        }
    }
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn members_sort_by_utf16_code_units_not_utf8_bytes() {
        // In UTF-8 byte order U+FB01 (EF AC 81) precedes U+1F600 (F0 9F ..).
        // In UTF-16 code units U+1F600 is the surrogate 0xD83D, which
        // precedes 0xFB01. JCS uses UTF-16, so the emoji comes first.
        let value = json!({"\u{fb01}": 1, "\u{1f600}": 2, "a": 3});
        assert_eq!(
            to_canonical_string(&value).unwrap(),
            "{\"a\":3,\"\u{1f600}\":2,\"\u{fb01}\":1}"
        );
    }

    #[test]
    fn strings_escape_like_json_stringify() {
        let value = json!(["\u{1}\n\"\\/\u{7f}"]);
        assert_eq!(
            to_canonical_string(&value).unwrap(),
            "[\"\\u0001\\n\\\"\\\\/\u{7f}\"]"
        );
    }

    #[test]
    fn no_whitespace_and_nested_objects_are_sorted() {
        let value = json!({"b": [1, {"d": 0, "c": -4}], "a": null});
        assert_eq!(
            to_canonical_string(&value).unwrap(),
            "{\"a\":null,\"b\":[1,{\"c\":-4,\"d\":0}]}"
        );
    }

    #[test]
    fn integer_bounds_follow_q1() {
        assert!(to_canonical_string(&json!(MAX_SAFE_INTEGER)).is_ok());
        assert_eq!(
            to_canonical_string(&json!({"x": [MAX_SAFE_INTEGER + 1]})),
            Err(CanonicalError::IntegerOutOfRange {
                path: "/x/0".into()
            })
        );
        assert_eq!(
            to_canonical_string(&json!(u64::MAX)),
            Err(CanonicalError::IntegerOutOfRange { path: "/".into() })
        );
        assert!(to_canonical_string(&json!(-(MAX_SAFE_INTEGER as i64))).is_ok());
    }

    #[test]
    fn floats_are_refused() {
        assert_eq!(
            to_canonical_string(&json!({"a/b": 1.5})),
            Err(CanonicalError::NonInteger {
                path: "/a~1b".into()
            })
        );
    }
}

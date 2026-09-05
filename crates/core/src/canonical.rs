//! Canonical JSON writer, byte-identical to the TypeScript encoder in
//! `packages/protocol/src/canonical.ts` (06-format-and-export.md §项目文件).
//! Rules: lexicographically sorted object keys, id-bearing object arrays
//! sorted by stable `id`, finite ECMAScript-style decimals, 2-space indent,
//! LF endings, trailing LF.

use serde::Serialize;
use serde_json::{Map, Number, Value};

use crate::error::{codes, OxitoneError};

fn canonical_err(code: &'static str, message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(code, message)
}

/// Deep-apply the canonical ordering rules; returns a fresh `Value`.
pub fn canonicalize(value: &Value) -> Result<Value, OxitoneError> {
    match value {
        Value::Number(number) => {
            if let Some(f) = number.as_f64() {
                if !f.is_finite() {
                    return Err(canonical_err(
                        codes::AUTOMATION_NON_FINITE,
                        format!("non-finite number in canonical JSON: {f}"),
                    ));
                }
                if number.is_f64() && f.fract() == 0.0 && f.abs() > 9007199254740991.0 {
                    return Err(canonical_err(
                        codes::INVALID_PROJECT,
                        format!(
                            "integer {f} exceeds the JSON safe-integer range; use a decimal string"
                        ),
                    ));
                }
            }
            Ok(value.clone())
        }
        Value::Array(items) => {
            let mut out: Vec<Value> = items.iter().map(canonicalize).collect::<Result<_, _>>()?;
            let sortable = out.len() > 1
                && out.iter().all(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("id"))
                        .is_some_and(Value::is_string)
                });
            if sortable {
                out.sort_by(|a, b| {
                    let ai = a["id"].as_str().unwrap_or_default();
                    let bi = b["id"].as_str().unwrap_or_default();
                    ai.cmp(bi)
                });
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = Map::with_capacity(entries.len());
            for (key, item) in entries {
                out.insert(key.clone(), canonicalize(item)?);
            }
            Ok(Value::Object(out))
        }
        _ => Ok(value.clone()),
    }
}

/// ECMAScript `Number.prototype.toString` for finite f64, so float bytes
/// match `JSON.stringify` exactly. Rust's `Display` already yields the
/// shortest round-trip digits in plain decimal; this re-renders them with
/// the ECMA decimal/scientific thresholds.
pub fn format_f64_js(value: f64) -> Result<String, OxitoneError> {
    if !value.is_finite() {
        return Err(canonical_err(
            codes::AUTOMATION_NON_FINITE,
            format!("non-finite number in canonical JSON: {value}"),
        ));
    }
    if value == 0.0 {
        return Ok("0".to_owned());
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let plain = format!("{}", value.abs());
    let (int_part, frac_part) = match plain.split_once('.') {
        Some((i, f)) => (i.to_owned(), f.to_owned()),
        None => (plain, String::new()),
    };
    let (digits_raw, n) = if int_part != "0" {
        let n = int_part.len() as i32;
        (format!("{int_part}{frac_part}"), n)
    } else {
        let zeros = frac_part.bytes().take_while(|b| *b == b'0').count();
        (frac_part[zeros..].to_owned(), -(zeros as i32))
    };
    let digits = digits_raw.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let k = digits.len() as i32;

    let body = if k <= n && n <= 21 {
        format!("{}{}", digits, "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{}", "0".repeat((-n) as usize), digits)
    } else {
        let mantissa = if k > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits.to_owned()
        };
        let exponent = n - 1;
        format!(
            "{mantissa}e{}{}",
            if exponent >= 0 { "+" } else { "-" },
            exponent.abs()
        )
    };
    Ok(format!("{sign}{body}"))
}

fn write_number(number: &Number, out: &mut String) -> Result<(), OxitoneError> {
    if let Some(u) = number.as_u64() {
        out.push_str(&u.to_string());
    } else if let Some(i) = number.as_i64() {
        out.push_str(&i.to_string());
    } else if let Some(f) = number.as_f64() {
        out.push_str(&format_f64_js(f)?);
    }
    Ok(())
}

fn write_value(value: &Value, level: usize, out: &mut String) -> Result<(), OxitoneError> {
    const INDENT: &str = "  ";
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(number) => write_number(number, out)?,
        Value::String(s) => {
            out.push_str(&serde_json::to_string(s).map_err(|e| {
                canonical_err(codes::INVALID_PROJECT, format!("string escape failed: {e}"))
            })?);
        }
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
            } else {
                out.push_str("[\n");
                for (index, item) in items.iter().enumerate() {
                    out.push_str(&INDENT.repeat(level + 1));
                    write_value(item, level + 1, out)?;
                    out.push_str(if index + 1 == items.len() {
                        "\n"
                    } else {
                        ",\n"
                    });
                }
                out.push_str(&INDENT.repeat(level));
                out.push(']');
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
            } else {
                out.push_str("{\n");
                for (index, (key, item)) in map.iter().enumerate() {
                    out.push_str(&INDENT.repeat(level + 1));
                    out.push_str(&serde_json::to_string(key).map_err(|e| {
                        canonical_err(codes::INVALID_PROJECT, format!("key escape failed: {e}"))
                    })?);
                    out.push_str(": ");
                    write_value(item, level + 1, out)?;
                    out.push_str(if index + 1 == map.len() { "\n" } else { ",\n" });
                }
                out.push_str(&INDENT.repeat(level));
                out.push('}');
            }
        }
    }
    Ok(())
}

/// Serialize a `Value` to canonical JSON text (trailing LF).
pub fn write_canonical(value: &Value) -> Result<String, OxitoneError> {
    let canonical = canonicalize(value)?;
    let mut out = String::new();
    write_value(&canonical, 0, &mut out)?;
    out.push('\n');
    Ok(out)
}

/// Serialize any `Serialize` type to canonical JSON text (trailing LF).
pub fn to_canonical_json<T: Serialize>(value: &T) -> Result<String, OxitoneError> {
    let value = serde_json::to_value(value)
        .map_err(|e| canonical_err(codes::INVALID_PROJECT, format!("serialization failed: {e}")))?;
    write_canonical(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(v: f64) -> String {
        format_f64_js(v).unwrap()
    }

    #[test]
    fn floats_match_ecmascript_tostring() {
        // Expected strings are the exact ECMAScript Number::toString results.
        assert_eq!(fmt(0.0), "0");
        assert_eq!(fmt(-0.0), "0");
        assert_eq!(fmt(120.0), "120");
        assert_eq!(fmt(-1.0), "-1");
        assert_eq!(fmt(0.5), "0.5");
        assert_eq!(fmt(0.1), "0.1");
        assert_eq!(fmt(0.72), "0.72");
        assert_eq!(fmt(0.04), "0.04");
        assert_eq!(fmt(0.000025), "0.000025");
        assert_eq!(fmt(0.000001), "0.000001");
        assert_eq!(fmt(1e-7), "1e-7");
        assert_eq!(fmt(1.5e-7), "1.5e-7");
        assert_eq!(fmt(1e20), "100000000000000000000");
        assert_eq!(fmt(1e21), "1e+21");
        assert_eq!(fmt(123.456), "123.456");
        assert_eq!(fmt(960.0), "960");
        assert_eq!(fmt(-12.5), "-12.5");
        assert_eq!(fmt(0.14773134794086218), "0.14773134794086218");
    }

    #[test]
    fn rejects_non_finite() {
        assert!(format_f64_js(f64::NAN).is_err());
        assert!(format_f64_js(f64::INFINITY).is_err());
    }

    #[test]
    fn canonicalize_sorts_keys_and_id_arrays() {
        let value: Value = serde_json::json!({
            "b": 1,
            "a": [{"id": "z_9"}, {"id": "a_1"}, 7],
            "c": [{"id": "b_2"}, {"id": "a_1"}],
        });
        let text = write_canonical(&value).unwrap();
        let expected = "{\n  \"a\": [\n    {\n      \"id\": \"z_9\"\n    },\n    {\n      \"id\": \"a_1\"\n    },\n    7\n  ],\n  \"b\": 1,\n  \"c\": [\n    {\n      \"id\": \"a_1\"\n    },\n    {\n      \"id\": \"b_2\"\n    }\n  ]\n}\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn empty_containers_stay_inline() {
        let text = write_canonical(&serde_json::json!({"a": [], "o": {}})).unwrap();
        assert_eq!(text, "{\n  \"a\": [],\n  \"o\": {}\n}\n");
    }

    #[test]
    fn string_escapes_match_json_stringify() {
        let text = write_canonical(&serde_json::json!("\u{1}\n\"\\")).unwrap();
        assert_eq!(text, "\"\\u0001\\n\\\"\\\\\"\n");
    }
}

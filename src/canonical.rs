// SPDX-License-Identifier: Apache-2.0

//! The canonical wire form shared by all editions:
//!
//! - UTF-8 JSON, object keys sorted by code point, two-space indent, a trailing newline;
//! - the tag lists of every `or_set` (`elements` values and `tombstones`) sorted;
//! - numbers that print the same in Python, JavaScript and Rust: whole numbers are integers,
//!   and no float is whole or needs an exponent.

use serde_json::{Map, Value};

/// Copy a wire value, sorting the tag lists of every `or_set` it contains.
pub fn canonicalize(node: &Value) -> Value {
    match node {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), canonicalize(v));
            }
            if map.get("type").and_then(Value::as_str) == Some("or_set") {
                if let Some(Value::Object(elements)) = out.get_mut("elements") {
                    for tags in elements.values_mut() {
                        sort_strings(tags);
                    }
                }
                if let Some(tombstones) = out.get_mut("tombstones") {
                    sort_strings(tombstones);
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn sort_strings(v: &mut Value) {
    if let Value::Array(items) = v {
        items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    }
}

fn emit(node: &Value, indent: &str, out: &mut String) {
    let next = format!("{indent}  ");
    match node {
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&next);
                emit(item, &next, out);
            }
            out.push('\n');
            out.push_str(indent);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            // Rust compares strings byte by byte, which is code point order for UTF-8.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push_str("{\n");
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&next);
                out.push_str(&serde_json::to_string(key).expect("string"));
                out.push_str(": ");
                emit(&map[*key], &next, out);
            }
            out.push('\n');
            out.push_str(indent);
            out.push('}');
        }
        scalar => out.push_str(&serde_json::to_string(scalar).expect("scalar")),
    }
}

/// Write a wire value in the canonical layout, with a trailing newline.
pub fn emit_canonical(node: &Value) -> String {
    let mut out = String::new();
    emit(node, "", &mut out);
    out.push('\n');
    out
}

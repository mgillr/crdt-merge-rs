//! Tabular merge engine for JSON record arrays.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Merge strategy for conflicting fields.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Strategy {
    /// Last-Writer-Wins: values from `b` override `a`.
    LWW,
    /// Always keep `a`'s value.
    KeepA,
    /// Always keep `b`'s value.
    KeepB,
}

impl Default for Strategy {
    fn default() -> Self {
        Strategy::LWW
    }
}

/// Options for the tabular merge operation.
#[derive(Clone, Debug)]
pub struct MergeOptions {
    /// The field name to use as the primary key (default: "id").
    pub key: String,
    /// The strategy for resolving conflicts.
    pub strategy: Strategy,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            key: "id".to_string(),
            strategy: Strategy::LWW,
        }
    }
}

/// Merge two arrays of JSON records.
///
/// Records are matched by a key field (default: `"id"`). Records present in
/// only one side pass through unchanged. When both sides contain a record with
/// the same key, fields are merged according to the given strategy.
///
/// Returns a new `Vec<Value>` with the merged records, ordered: matched records
/// first (in order of `a`), then unmatched from `b`.
pub fn merge(
    a: &[Value],
    b: &[Value],
    options: Option<MergeOptions>,
) -> Vec<Value> {
    let opts = options.unwrap_or_default();
    let key = &opts.key;

    // Index b by key
    let mut b_map: BTreeMap<String, &Value> = BTreeMap::new();
    for item in b {
        if let Some(k) = extract_key(item, key) {
            b_map.insert(k, item);
        }
    }

    let mut result: Vec<Value> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Process all records from a
    for item_a in a {
        if let Some(k) = extract_key(item_a, key) {
            seen_keys.insert(k.clone());
            if let Some(item_b) = b_map.get(&k) {
                // Merge the two records
                result.push(merge_records(item_a, item_b, &opts.strategy));
            } else {
                result.push(item_a.clone());
            }
        } else {
            // No key – pass through
            result.push(item_a.clone());
        }
    }

    // Add records from b that are not in a
    for item_b in b {
        if let Some(k) = extract_key(item_b, key) {
            if !seen_keys.contains(&k) {
                result.push(item_b.clone());
            }
        } else {
            result.push(item_b.clone());
        }
    }

    result
}

/// Extract the string representation of a key field from a JSON value.
fn extract_key(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}

/// Merge two JSON objects field by field according to a strategy.
fn merge_records(a: &Value, b: &Value, strategy: &Strategy) -> Value {
    match (a, b) {
        (Value::Object(map_a), Value::Object(map_b)) => {
            let mut merged = serde_json::Map::new();
            // Start with all fields from a
            for (k, v) in map_a {
                merged.insert(k.clone(), v.clone());
            }
            // Merge in fields from b
            for (k, v_b) in map_b {
                if let Some(v_a) = map_a.get(k) {
                    // Conflict – resolve by strategy
                    let resolved = match strategy {
                        Strategy::LWW | Strategy::KeepB => v_b.clone(),
                        Strategy::KeepA => v_a.clone(),
                    };
                    merged.insert(k.clone(), resolved);
                } else {
                    // Only in b
                    merged.insert(k.clone(), v_b.clone());
                }
            }
            Value::Object(merged)
        }
        // Non-object values: apply strategy
        _ => match strategy {
            Strategy::LWW | Strategy::KeepB => b.clone(),
            Strategy::KeepA => a.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_basic() {
        let a = vec![json!({"id": "1", "name": "Alice"})];
        let b = vec![json!({"id": "2", "name": "Bob"})];
        let result = merge(&a, &b, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn merge_conflict_lww() {
        let a = vec![json!({"id": "1", "name": "Alice", "age": 30})];
        let b = vec![json!({"id": "1", "name": "Alicia", "age": 31})];
        let result = merge(&a, &b, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "Alicia"); // b wins (LWW)
    }

    #[test]
    fn merge_keep_a() {
        let a = vec![json!({"id": "1", "name": "Alice"})];
        let b = vec![json!({"id": "1", "name": "Alicia"})];
        let opts = MergeOptions {
            key: "id".to_string(),
            strategy: Strategy::KeepA,
        };
        let result = merge(&a, &b, Some(opts));
        assert_eq!(result[0]["name"], "Alice");
    }
}

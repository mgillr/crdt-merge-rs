//! Structural diff engine for JSON record arrays.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A change to a single field.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FieldChange {
    /// Field name.
    pub field: String,
    /// Old value (from `a`).
    pub old: Value,
    /// New value (from `b`).
    pub new: Value,
}

/// A row that exists in both inputs but has changed fields.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModifiedRow {
    /// The key value that identifies this row.
    pub key: Value,
    /// The list of changed fields.
    pub changes: Vec<FieldChange>,
    /// The full record from `a`.
    pub old_record: Value,
    /// The full record from `b`.
    pub new_record: Value,
}

/// The result of a structural diff between two record arrays.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffResult {
    /// Records present only in `b` (added).
    pub added: Vec<Value>,
    /// Records present only in `a` (removed).
    pub removed: Vec<Value>,
    /// Records present in both but with different values.
    pub modified: Vec<ModifiedRow>,
    /// Records identical in both inputs.
    pub unchanged: Vec<Value>,
    /// Human-readable summary string.
    pub summary: String,
}

/// Compute the structural diff between two arrays of JSON records.
///
/// Records are matched by a key field (default: `"id"`).
pub fn diff(
    a: &[Value],
    b: &[Value],
    key: Option<&str>,
) -> DiffResult {
    let key_field = key.unwrap_or("id");

    // Build maps: key → record
    let map_a = build_index(a, key_field);
    let map_b = build_index(b, key_field);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged = Vec::new();

    // Records in a
    for (k, rec_a) in &map_a {
        if let Some(rec_b) = map_b.get(k) {
            if rec_a == rec_b {
                unchanged.push((*rec_a).clone());
            } else {
                let changes = compute_field_changes(rec_a, rec_b);
                modified.push(ModifiedRow {
                    key: Value::String(k.clone()),
                    changes,
                    old_record: (*rec_a).clone(),
                    new_record: (*rec_b).clone(),
                });
            }
        } else {
            removed.push((*rec_a).clone());
        }
    }

    // Records only in b
    for (k, rec_b) in &map_b {
        if !map_a.contains_key(k) {
            added.push((*rec_b).clone());
        }
    }

    let summary = format!(
        "{} added, {} removed, {} modified, {} unchanged",
        added.len(),
        removed.len(),
        modified.len(),
        unchanged.len()
    );

    DiffResult {
        added,
        removed,
        modified,
        unchanged,
        summary,
    }
}

/// Build an ordered map from key → record reference.
fn build_index<'a>(
    records: &'a [Value],
    key: &str,
) -> BTreeMap<String, &'a Value> {
    let mut map = BTreeMap::new();
    for rec in records {
        if let Some(k) = extract_key(rec, key) {
            map.insert(k, rec);
        }
    }
    map
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

/// Compare two JSON objects field by field and return a list of changes.
fn compute_field_changes(a: &Value, b: &Value) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    match (a, b) {
        (Value::Object(map_a), Value::Object(map_b)) => {
            // Fields in a
            for (k, v_a) in map_a {
                match map_b.get(k) {
                    Some(v_b) if v_a != v_b => {
                        changes.push(FieldChange {
                            field: k.clone(),
                            old: v_a.clone(),
                            new: v_b.clone(),
                        });
                    }
                    None => {
                        changes.push(FieldChange {
                            field: k.clone(),
                            old: v_a.clone(),
                            new: Value::Null,
                        });
                    }
                    _ => {}
                }
            }
            // Fields only in b
            for (k, v_b) in map_b {
                if !map_a.contains_key(k) {
                    changes.push(FieldChange {
                        field: k.clone(),
                        old: Value::Null,
                        new: v_b.clone(),
                    });
                }
            }
        }
        _ => {
            if a != b {
                changes.push(FieldChange {
                    field: "<root>".to_string(),
                    old: a.clone(),
                    new: b.clone(),
                });
            }
        }
    }
    changes
}

/// Format a DiffResult as a human-readable table string.
pub fn format_diff_table(result: &DiffResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("Summary: {}\n", result.summary));
    out.push_str(&format!("{}\n", "=".repeat(60)));

    if !result.added.is_empty() {
        out.push_str("\n+ ADDED:\n");
        for rec in &result.added {
            out.push_str(&format!("  + {}\n", rec));
        }
    }

    if !result.removed.is_empty() {
        out.push_str("\n- REMOVED:\n");
        for rec in &result.removed {
            out.push_str(&format!("  - {}\n", rec));
        }
    }

    if !result.modified.is_empty() {
        out.push_str("\n~ MODIFIED:\n");
        for m in &result.modified {
            out.push_str(&format!("  Key: {}\n", m.key));
            for c in &m.changes {
                out.push_str(&format!(
                    "    {}: {} → {}\n",
                    c.field, c.old, c.new
                ));
            }
        }
    }

    if !result.unchanged.is_empty() {
        out.push_str(&format!("\n= UNCHANGED: {} records\n", result.unchanged.len()));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diff_basic() {
        let a = vec![
            json!({"id": "1", "name": "Alice"}),
            json!({"id": "2", "name": "Bob"}),
        ];
        let b = vec![
            json!({"id": "1", "name": "Alice"}),
            json!({"id": "3", "name": "Charlie"}),
        ];
        let result = diff(&a, &b, None);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.unchanged.len(), 1);
    }
}

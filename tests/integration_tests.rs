use crdt_merge::*;
use serde_json::json;

// ===========================================================================
// JSON Merge integration tests
// ===========================================================================

#[test]
fn json_merge_flat_objects() {
    let a = json!({"name": "Alice", "age": 30});
    let b = json!({"name": "Alicia", "email": "a@b.c"});
    let result = merge_json(&a, &b, None);
    assert_eq!(result["name"], "Alicia"); // b wins
    assert_eq!(result["age"], 30); // only in a
    assert_eq!(result["email"], "a@b.c"); // only in b
}

#[test]
fn json_merge_nested_objects() {
    let a = json!({
        "user": {
            "name": "Alice",
            "address": {
                "city": "NYC",
                "zip": "10001"
            }
        }
    });
    let b = json!({
        "user": {
            "name": "Alicia",
            "address": {
                "city": "LA",
                "state": "CA"
            }
        }
    });
    let result = merge_json(&a, &b, None);
    assert_eq!(result["user"]["name"], "Alicia");
    assert_eq!(result["user"]["address"]["city"], "LA");
    assert_eq!(result["user"]["address"]["zip"], "10001");
    assert_eq!(result["user"]["address"]["state"], "CA");
}

#[test]
fn json_merge_arrays() {
    let a = json!({"tags": ["rust", "crdt"]});
    let b = json!({"tags": ["crdt", "merge", "data"]});
    let result = merge_json(&a, &b, None);
    let tags = result["tags"].as_array().unwrap();
    assert!(tags.contains(&json!("rust")));
    assert!(tags.contains(&json!("crdt")));
    assert!(tags.contains(&json!("merge")));
    assert!(tags.contains(&json!("data")));
}

#[test]
fn json_merge_keep_a() {
    let a = json!({"val": "a"});
    let b = json!({"val": "b"});
    let result = merge_json(&a, &b, Some(Strategy::KeepA));
    assert_eq!(result["val"], "a");
}

#[test]
fn json_merge_primitives() {
    let a = json!(42);
    let b = json!(99);
    let result = merge_json(&a, &b, None);
    assert_eq!(result, json!(99)); // LWW: b wins
}

#[test]
fn json_merge_null_handling() {
    let a = json!({"x": null});
    let b = json!({"x": "value"});
    let result = merge_json(&a, &b, None);
    assert_eq!(result["x"], "value");
}

// ===========================================================================
// Diff integration tests
// ===========================================================================

#[test]
fn diff_all_added() {
    let a: Vec<serde_json::Value> = vec![];
    let b = vec![json!({"id": "1", "name": "Alice"})];
    let result = diff(&a, &b, None);
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.removed.len(), 0);
    assert_eq!(result.modified.len(), 0);
}

#[test]
fn diff_all_removed() {
    let a = vec![json!({"id": "1", "name": "Alice"})];
    let b: Vec<serde_json::Value> = vec![];
    let result = diff(&a, &b, None);
    assert_eq!(result.added.len(), 0);
    assert_eq!(result.removed.len(), 1);
}

#[test]
fn diff_modified_fields() {
    let a = vec![json!({"id": "1", "name": "Alice", "age": 30})];
    let b = vec![json!({"id": "1", "name": "Alicia", "age": 31})];
    let result = diff(&a, &b, None);
    assert_eq!(result.modified.len(), 1);
    let changes = &result.modified[0].changes;
    assert!(changes.iter().any(|c| c.field == "name"));
    assert!(changes.iter().any(|c| c.field == "age"));
}

#[test]
fn diff_unchanged() {
    let a = vec![json!({"id": "1", "name": "Alice"})];
    let b = vec![json!({"id": "1", "name": "Alice"})];
    let result = diff(&a, &b, None);
    assert_eq!(result.unchanged.len(), 1);
    assert_eq!(result.modified.len(), 0);
}

#[test]
fn diff_mixed_changes() {
    let a = vec![
        json!({"id": "1", "name": "Alice"}),
        json!({"id": "2", "name": "Bob"}),
        json!({"id": "3", "name": "Charlie"}),
    ];
    let b = vec![
        json!({"id": "1", "name": "Alice"}),   // unchanged
        json!({"id": "2", "name": "Bobby"}),    // modified
        json!({"id": "4", "name": "Dave"}),     // added
    ];
    let result = diff(&a, &b, None);
    assert_eq!(result.unchanged.len(), 1);
    assert_eq!(result.modified.len(), 1);
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.removed.len(), 1); // id=3
}

#[test]
fn diff_summary_format() {
    let a = vec![json!({"id": "1", "name": "A"})];
    let b = vec![json!({"id": "1", "name": "B"})];
    let result = diff(&a, &b, None);
    assert!(result.summary.contains("modified"));
}

#[test]
fn diff_new_field_in_b() {
    let a = vec![json!({"id": "1", "name": "Alice"})];
    let b = vec![json!({"id": "1", "name": "Alice", "email": "a@b.c"})];
    let result = diff(&a, &b, None);
    assert_eq!(result.modified.len(), 1);
    let change = result.modified[0]
        .changes
        .iter()
        .find(|c| c.field == "email")
        .unwrap();
    assert_eq!(change.old, serde_json::Value::Null);
    assert_eq!(change.new, "a@b.c");
}

// ===========================================================================
// End-to-end: merge then diff should show no changes
// ===========================================================================

#[test]
fn merge_then_diff_empty() {
    let a = vec![
        json!({"id": "1", "name": "Alice"}),
        json!({"id": "2", "name": "Bob"}),
    ];
    let b = vec![
        json!({"id": "2", "name": "Bobby"}),
        json!({"id": "3", "name": "Charlie"}),
    ];
    let merged = merge::merge(&a, &b, None);
    // Diff merged vs itself should show no changes
    let d = diff::diff(&merged, &merged, None);
    assert!(d.added.is_empty());
    assert!(d.removed.is_empty());
    assert!(d.modified.is_empty());
    assert_eq!(d.unchanged.len(), merged.len());
}

// ===========================================================================
// Dedup + merge pipeline
// ===========================================================================

#[test]
fn dedup_then_merge() {
    let data = vec![
        json!({"id": "1", "name": "Alice"}),
        json!({"id": "1", "name": "Alice dup"}),
        json!({"id": "2", "name": "Bob"}),
    ];
    let deduped = dedup::dedup(&data, Some("id"), None);
    assert_eq!(deduped.unique.len(), 2);

    let other = vec![json!({"id": "3", "name": "Charlie"})];
    let merged = merge::merge(&deduped.unique, &other, None);
    assert_eq!(merged.len(), 3);
}

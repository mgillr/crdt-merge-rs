use crdt_merge::merge::{merge, MergeOptions, Strategy};
use serde_json::json;

#[test]
fn merge_no_overlap() {
    let a = vec![json!({"id": "1", "name": "Alice"})];
    let b = vec![json!({"id": "2", "name": "Bob"})];
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 2);
}

#[test]
fn merge_full_overlap_lww() {
    let a = vec![json!({"id": "1", "name": "Alice", "score": 90})];
    let b = vec![json!({"id": "1", "name": "Alicia", "score": 95})];
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "Alicia");
    assert_eq!(result[0]["score"], 95);
}

#[test]
fn merge_partial_overlap() {
    let a = vec![
        json!({"id": "1", "name": "Alice"}),
        json!({"id": "2", "name": "Bob"}),
    ];
    let b = vec![
        json!({"id": "2", "name": "Bobby"}),
        json!({"id": "3", "name": "Charlie"}),
    ];
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 3);
    // id=2 should have merged
    let bob = result.iter().find(|r| r["id"] == "2").unwrap();
    assert_eq!(bob["name"], "Bobby"); // LWW: b wins
}

#[test]
fn merge_keep_a_strategy() {
    let a = vec![json!({"id": "1", "val": "a_val"})];
    let b = vec![json!({"id": "1", "val": "b_val"})];
    let opts = MergeOptions {
        key: "id".to_string(),
        strategy: Strategy::KeepA,
    };
    let result = merge(&a, &b, Some(opts));
    assert_eq!(result[0]["val"], "a_val");
}

#[test]
fn merge_keep_b_strategy() {
    let a = vec![json!({"id": "1", "val": "a_val"})];
    let b = vec![json!({"id": "1", "val": "b_val"})];
    let opts = MergeOptions {
        key: "id".to_string(),
        strategy: Strategy::KeepB,
    };
    let result = merge(&a, &b, Some(opts));
    assert_eq!(result[0]["val"], "b_val");
}

#[test]
fn merge_new_fields_from_b() {
    let a = vec![json!({"id": "1", "name": "Alice"})];
    let b = vec![json!({"id": "1", "name": "Alice", "email": "alice@example.com"})];
    let result = merge(&a, &b, None);
    assert_eq!(result[0]["email"], "alice@example.com");
}

#[test]
fn merge_custom_key() {
    let a = vec![json!({"uid": "x1", "val": 10})];
    let b = vec![json!({"uid": "x1", "val": 20})];
    let opts = MergeOptions {
        key: "uid".to_string(),
        strategy: Strategy::LWW,
    };
    let result = merge(&a, &b, Some(opts));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["val"], 20);
}

#[test]
fn merge_empty_a() {
    let a: Vec<serde_json::Value> = vec![];
    let b = vec![json!({"id": "1", "name": "Bob"})];
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "Bob");
}

#[test]
fn merge_empty_b() {
    let a = vec![json!({"id": "1", "name": "Alice"})];
    let b: Vec<serde_json::Value> = vec![];
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "Alice");
}

#[test]
fn merge_both_empty() {
    let a: Vec<serde_json::Value> = vec![];
    let b: Vec<serde_json::Value> = vec![];
    let result = merge(&a, &b, None);
    assert!(result.is_empty());
}

#[test]
fn merge_numeric_keys() {
    let a = vec![json!({"id": 1, "val": "a"})];
    let b = vec![json!({"id": 1, "val": "b"})];
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["val"], "b");
}

#[test]
fn merge_preserves_unmatched_fields_from_a() {
    let a = vec![json!({"id": "1", "name": "Alice", "extra_a": true})];
    let b = vec![json!({"id": "1", "name": "Alicia", "extra_b": false})];
    let result = merge(&a, &b, None);
    assert_eq!(result[0]["extra_a"], true);
    assert_eq!(result[0]["extra_b"], false);
}

#[test]
fn merge_many_records() {
    let a: Vec<serde_json::Value> = (0..100)
        .map(|i| json!({"id": i.to_string(), "source": "a"}))
        .collect();
    let b: Vec<serde_json::Value> = (50..150)
        .map(|i| json!({"id": i.to_string(), "source": "b"}))
        .collect();
    let result = merge(&a, &b, None);
    assert_eq!(result.len(), 150); // 100 from a (50 merged) + 50 only in b
}

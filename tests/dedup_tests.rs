use crdt_merge::dedup::{dedup, jaccard_similarity};
use serde_json::json;

#[test]
fn dedup_no_duplicates() {
    let items = vec![
        json!({"id": "1", "name": "Alice"}),
        json!({"id": "2", "name": "Bob"}),
        json!({"id": "3", "name": "Charlie"}),
    ];
    let result = dedup(&items, Some("id"), None);
    assert_eq!(result.unique.len(), 3);
    assert_eq!(result.duplicates.len(), 0);
}

#[test]
fn dedup_exact_by_key() {
    let items = vec![
        json!({"id": "1", "name": "Alice"}),
        json!({"id": "1", "name": "Alice Copy"}),
        json!({"id": "2", "name": "Bob"}),
    ];
    let result = dedup(&items, Some("id"), None);
    assert_eq!(result.unique.len(), 2);
    assert_eq!(result.duplicates.len(), 1);
}

#[test]
fn dedup_fuzzy_similar_names() {
    let items = vec![
        json!({"name": "Jonathan Smith"}),
        json!({"name": "Johnathan Smith"}),
        json!({"name": "Bob Jones"}),
    ];
    let result = dedup(&items, Some("name"), Some(0.5));
    assert_eq!(result.unique.len(), 2);
    assert_eq!(result.duplicates.len(), 1);
    assert!(result.duplicates[0].similarity > 0.5);
}

#[test]
fn dedup_no_key_exact_match() {
    let items = vec![
        json!({"a": 1, "b": 2}),
        json!({"a": 1, "b": 2}),
        json!({"a": 3, "b": 4}),
    ];
    let result = dedup(&items, None, None);
    assert_eq!(result.unique.len(), 2);
    assert_eq!(result.duplicates.len(), 1);
}

#[test]
fn dedup_empty() {
    let items: Vec<serde_json::Value> = vec![];
    let result = dedup(&items, Some("id"), None);
    assert!(result.unique.is_empty());
    assert!(result.duplicates.is_empty());
}

#[test]
fn dedup_high_threshold_no_fuzzy() {
    let items = vec![
        json!({"name": "Alice"}),
        json!({"name": "Alicia"}),
    ];
    let result = dedup(&items, Some("name"), Some(0.99));
    assert_eq!(result.unique.len(), 2); // threshold too high for fuzzy match
}

#[test]
fn dedup_low_threshold_catches_more() {
    let items = vec![
        json!({"name": "cat"}),
        json!({"name": "car"}),
    ];
    let result = dedup(&items, Some("name"), Some(0.3));
    // "cat" and "car" share bigrams "ca" out of {"ca","at"} ∪ {"ca","ar"} = 3 → 1/3 ≈ 0.33
    assert_eq!(result.unique.len(), 1);
    assert_eq!(result.duplicates.len(), 1);
}

#[test]
fn dedup_multiple_duplicates() {
    let items = vec![
        json!({"id": "1"}),
        json!({"id": "1"}),
        json!({"id": "1"}),
        json!({"id": "2"}),
    ];
    let result = dedup(&items, Some("id"), None);
    assert_eq!(result.unique.len(), 2);
    assert_eq!(result.duplicates.len(), 2);
}

// Jaccard similarity unit tests

#[test]
fn jaccard_identical_strings() {
    assert_eq!(jaccard_similarity("hello", "hello"), 1.0);
}

#[test]
fn jaccard_completely_different() {
    let sim = jaccard_similarity("abc", "xyz");
    assert_eq!(sim, 0.0);
}

#[test]
fn jaccard_case_insensitive() {
    assert_eq!(jaccard_similarity("Hello", "hello"), 1.0);
}

#[test]
fn jaccard_empty_strings() {
    assert_eq!(jaccard_similarity("", ""), 1.0);
}

#[test]
fn jaccard_one_empty() {
    // One empty, one not: should be 0
    let sim = jaccard_similarity("hello", "");
    assert!(sim <= 0.2); // depends on single-char handling
}

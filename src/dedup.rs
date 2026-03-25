//! Fuzzy & exact deduplication engine.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// A pair of duplicate records with their similarity score.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DuplicatePair {
    /// Index of the first record.
    pub index_a: usize,
    /// Index of the second record (kept in `unique`, a is removed).
    pub index_b: usize,
    /// Similarity score between the two records (0.0–1.0).
    pub similarity: f64,
    /// The first record.
    pub record_a: Value,
    /// The second record.
    pub record_b: Value,
}

/// Result of a deduplication operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DedupResult {
    /// The unique (deduplicated) records.
    pub unique: Vec<Value>,
    /// Pairs that were identified as duplicates.
    pub duplicates: Vec<DuplicatePair>,
}

/// Deduplicate a list of JSON records.
///
/// If `key` is provided, records with identical key values are considered
/// exact duplicates. Additionally, string values of the key field are compared
/// using Jaccard similarity; pairs exceeding `threshold` (default 0.85) are
/// flagged as fuzzy duplicates.
///
/// When no key is given, the full JSON serialisation is used for exact
/// matching, and no fuzzy matching is performed.
pub fn dedup(
    items: &[Value],
    key: Option<&str>,
    threshold: Option<f64>,
) -> DedupResult {
    let threshold = threshold.unwrap_or(0.85);
    let n = items.len();

    // Track which indices have been marked as duplicates
    let mut is_dup = vec![false; n];
    let mut duplicates: Vec<DuplicatePair> = Vec::new();

    if let Some(key_field) = key {
        // Key-based dedup
        for i in 0..n {
            if is_dup[i] {
                continue;
            }
            let val_i = extract_string(&items[i], key_field);
            for j in (i + 1)..n {
                if is_dup[j] {
                    continue;
                }
                let val_j = extract_string(&items[j], key_field);

                match (&val_i, &val_j) {
                    (Some(a), Some(b)) => {
                        if a == b {
                            // Exact duplicate
                            is_dup[j] = true;
                            duplicates.push(DuplicatePair {
                                index_a: j,
                                index_b: i,
                                similarity: 1.0,
                                record_a: items[j].clone(),
                                record_b: items[i].clone(),
                            });
                        } else {
                            let sim = jaccard_similarity(a, b);
                            if sim >= threshold {
                                is_dup[j] = true;
                                duplicates.push(DuplicatePair {
                                    index_a: j,
                                    index_b: i,
                                    similarity: sim,
                                    record_a: items[j].clone(),
                                    record_b: items[i].clone(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    } else {
        // Full-record exact dedup
        let mut seen: HashSet<String> = HashSet::new();
        for i in 0..n {
            let serialized = serde_json::to_string(&items[i]).unwrap_or_default();
            if seen.contains(&serialized) {
                is_dup[i] = true;
                // Find the first occurrence
                let first = (0..i)
                    .find(|&j| {
                        !is_dup[j]
                            && serde_json::to_string(&items[j]).unwrap_or_default() == serialized
                    })
                    .unwrap_or(0);
                duplicates.push(DuplicatePair {
                    index_a: i,
                    index_b: first,
                    similarity: 1.0,
                    record_a: items[i].clone(),
                    record_b: items[first].clone(),
                });
            } else {
                seen.insert(serialized);
            }
        }
    }

    let unique: Vec<Value> = items
        .iter()
        .enumerate()
        .filter(|(i, _)| !is_dup[*i])
        .map(|(_, v)| v.clone())
        .collect();

    DedupResult { unique, duplicates }
}

/// Extract a string representation of a field from a JSON value.
fn extract_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        Some(v) => Some(v.to_string()),
        None => None,
    }
}

/// Compute the Jaccard similarity of two strings based on character bigrams.
///
/// Returns a value in [0.0, 1.0]. Empty strings both → 1.0.
pub fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    if a_lower == b_lower {
        return 1.0;
    }

    let bigrams_a = char_bigrams(&a_lower);
    let bigrams_b = char_bigrams(&b_lower);

    if bigrams_a.is_empty() && bigrams_b.is_empty() {
        return 1.0; // both single-char or empty
    }
    if bigrams_a.is_empty() || bigrams_b.is_empty() {
        return 0.0;
    }

    let intersection = bigrams_a.intersection(&bigrams_b).count();
    let union = bigrams_a.union(&bigrams_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

/// Produce the set of character bigrams for a string.
fn char_bigrams(s: &str) -> HashSet<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        let mut set = HashSet::new();
        if !chars.is_empty() {
            set.insert(chars[0].to_string());
        }
        return set;
    }
    let mut set = HashSet::new();
    for w in chars.windows(2) {
        set.insert(format!("{}{}", w[0], w[1]));
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn jaccard_identical() {
        assert_eq!(jaccard_similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn jaccard_different() {
        let sim = jaccard_similarity("hello", "world");
        assert!(sim < 0.5);
    }

    #[test]
    fn jaccard_similar() {
        let sim = jaccard_similarity("Jonathan", "Johnathan");
        assert!(sim > 0.5);
    }

    #[test]
    fn dedup_exact() {
        let items = vec![
            json!({"id": "1", "name": "Alice"}),
            json!({"id": "1", "name": "Alice"}),
            json!({"id": "2", "name": "Bob"}),
        ];
        let result = dedup(&items, Some("id"), None);
        assert_eq!(result.unique.len(), 2);
        assert_eq!(result.duplicates.len(), 1);
    }
}

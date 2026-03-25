//! Deep JSON merge using LWW semantics.

use crate::merge::Strategy;
use serde_json::Value;
use std::collections::HashSet;

/// Deep-merge two JSON values.
///
/// - **Objects**: recursively merged. Fields unique to one side pass through.
/// - **Arrays**: merged via set-union (preserving order, `a` first then new from `b`).
/// - **Primitives**: resolved by `strategy` (default: `LWW`, i.e. `b` wins).
pub fn merge_json(
    a: &Value,
    b: &Value,
    strategy: Option<Strategy>,
) -> Value {
    let strategy = strategy.unwrap_or(Strategy::LWW);
    merge_recursive(a, b, &strategy)
}

fn merge_recursive(a: &Value, b: &Value, strategy: &Strategy) -> Value {
    match (a, b) {
        // Both objects → recursive merge
        (Value::Object(map_a), Value::Object(map_b)) => {
            let mut merged = serde_json::Map::new();

            // All keys from a
            for (k, v_a) in map_a {
                if let Some(v_b) = map_b.get(k) {
                    merged.insert(k.clone(), merge_recursive(v_a, v_b, strategy));
                } else {
                    merged.insert(k.clone(), v_a.clone());
                }
            }

            // Keys only in b
            for (k, v_b) in map_b {
                if !map_a.contains_key(k) {
                    merged.insert(k.clone(), v_b.clone());
                }
            }

            Value::Object(merged)
        }

        // Both arrays → union (preserve order, a then new items from b)
        (Value::Array(arr_a), Value::Array(arr_b)) => {
            let mut result = arr_a.clone();
            let existing: HashSet<String> = arr_a
                .iter()
                .map(|v| serde_json::to_string(v).unwrap_or_default())
                .collect();
            for v in arr_b {
                let s = serde_json::to_string(v).unwrap_or_default();
                if !existing.contains(&s) {
                    result.push(v.clone());
                }
            }
            Value::Array(result)
        }

        // One is object, other is not – strategy decides
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
    fn merge_objects() {
        let a = json!({"name": "Alice", "age": 30});
        let b = json!({"name": "Alicia", "email": "a@b.c"});
        let result = merge_json(&a, &b, None);
        assert_eq!(result["name"], "Alicia");
        assert_eq!(result["age"], 30);
        assert_eq!(result["email"], "a@b.c");
    }

    #[test]
    fn merge_arrays_union() {
        let a = json!([1, 2, 3]);
        let b = json!([3, 4, 5]);
        let result = merge_json(&a, &b, None);
        assert_eq!(result, json!([1, 2, 3, 4, 5]));
    }
}

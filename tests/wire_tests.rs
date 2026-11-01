use crdt_merge::canonical::{canonicalize, emit_canonical};
use crdt_merge::{Element, GCounter, LWWMap, LWWRegister, PNCounter, ORSet};
use serde_json::{json, Value};

fn wire<T: serde::Serialize>(v: &T) -> Value {
    canonicalize(&serde_json::to_value(v).unwrap())
}

// ---- ORSet -------------------------------------------------------------------------

#[test]
fn orset_wire_form_has_a_type_key_and_no_tombstones_until_a_remove() {
    let mut s: ORSet<String> = ORSet::new();
    let tag = s.add("x".to_string());
    assert_eq!(wire(&s), json!({"type": "or_set", "elements": {"x": [tag]}}));
}

#[test]
fn orset_remove_records_tombstones_and_keeps_the_empty_element() {
    let mut s: ORSet<String> = ORSet::new();
    let t1 = s.add("x".to_string());
    let t2 = s.add("x".to_string());
    s.remove(&"x".to_string());
    let mut tags = vec![t1, t2];
    tags.sort();
    assert_eq!(wire(&s), json!({"type": "or_set", "elements": {"x": []}, "tombstones": tags}));
    assert!(!s.contains(&"x".to_string()));
}

#[test]
fn orset_reads_a_wire_state_without_tombstones() {
    let s: ORSet<String> = serde_json::from_value(json!({"type": "or_set", "elements": {"x": ["t1"]}})).unwrap();
    assert!(s.contains(&"x".to_string()));
}

#[test]
fn orset_drops_tombstoned_tags_when_reading() {
    let s: ORSet<String> =
        serde_json::from_value(json!({"type": "or_set", "elements": {"x": ["t1"]}, "tombstones": ["t1"]})).unwrap();
    assert!(!s.contains(&"x".to_string()));
    assert_eq!(wire(&s), json!({"type": "or_set", "elements": {"x": []}, "tombstones": ["t1"]}));
}

#[test]
fn orset_a_stale_replica_does_not_resurrect_a_removed_element() {
    let mut a: ORSet<String> = ORSet::new();
    a.add("x".to_string());
    let stale = a.clone();
    a.remove(&"x".to_string());
    assert!(!a.merge(&stale).contains(&"x".to_string()));
    assert!(!stale.merge(&a).contains(&"x".to_string()));
}

#[test]
fn orset_merge_keeps_the_key_of_an_emptied_element() {
    let a: ORSet<String> =
        serde_json::from_value(json!({"type": "or_set", "elements": {"x": []}, "tombstones": ["t1"]})).unwrap();
    let b: ORSet<String> = serde_json::from_value(json!({"type": "or_set", "elements": {"y": ["t2"]}})).unwrap();
    assert_eq!(
        wire(&a.merge(&b)),
        json!({"type": "or_set", "elements": {"x": [], "y": ["t2"]}, "tombstones": ["t1"]})
    );
}

#[test]
fn orset_tags_and_tombstones_come_out_sorted() {
    let s: ORSet<String> = serde_json::from_value(
        json!({"type": "or_set", "elements": {"x": ["c", "a", "b"]}, "tombstones": ["z", "y"]}),
    )
    .unwrap();
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["elements"]["x"], json!(["a", "b", "c"]));
    assert_eq!(v["tombstones"], json!(["y", "z"]));
}

#[test]
fn orset_remove_tag_removes_one_tag() {
    let mut s: ORSet<String> = ORSet::new();
    let t1 = s.add("x".to_string());
    s.add("x".to_string());
    s.remove_tag(&"x".to_string(), &t1).unwrap();
    assert!(s.contains(&"x".to_string()));
    assert_eq!(serde_json::to_value(&s).unwrap()["tombstones"], json!([t1]));
    assert!(s.remove_tag(&"missing".to_string(), "t").is_err());
}

#[test]
fn orset_integer_element_records_its_type() {
    let mut s: ORSet<i64> = ORSet::new();
    let tag = s.add(7);
    assert_eq!(
        wire(&s),
        json!({"type": "or_set", "elements": {"7": [tag]}, "element_types": {"7": "int"}})
    );
    let back: ORSet<i64> = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
    assert!(back.contains(&7));
}

#[test]
fn orset_boolean_element_uses_the_reference_spelling() {
    let mut s: ORSet<bool> = ORSet::new();
    s.add(true);
    let v = serde_json::to_value(&s).unwrap();
    assert!(v["elements"].get("True").is_some());
    assert_eq!(v["element_types"], json!({"True": "bool"}));
    let back: ORSet<bool> = serde_json::from_value(v).unwrap();
    assert!(back.contains(&true));
}

#[test]
fn orset_string_set_rejects_a_typed_element() {
    let r: Result<ORSet<String>, _> = serde_json::from_value(
        json!({"type": "or_set", "elements": {"7": ["t1"]}, "element_types": {"7": "int"}}),
    );
    assert!(r.is_err());
}

#[test]
fn orset_dynamic_element_reads_every_type() {
    let s: ORSet<Element> = serde_json::from_value(json!({
        "type": "or_set",
        "elements": {"a": ["t1"], "7": ["t2"], "1.5": ["t3"], "True": ["t4"]},
        "element_types": {"7": "int", "1.5": "float", "True": "bool"}
    }))
    .unwrap();
    assert!(s.contains(&Element::Str("a".into())));
    assert!(s.contains(&Element::Int(7)));
    assert!(s.contains(&Element::Float(1.5.into())));
    assert!(s.contains(&Element::Bool(true)));
}

// ---- counters and register wire forms -----------------------------------------------

#[test]
fn counters_carry_a_type_key() {
    let mut c = GCounter::new();
    c.increment("a");
    assert_eq!(wire(&c), json!({"type": "g_counter", "counts": {"a": 1}}));
    let mut p = PNCounter::new();
    p.increment("a");
    p.decrement("b");
    assert_eq!(
        wire(&p),
        json!({
            "type": "pn_counter",
            "pos": {"type": "g_counter", "counts": {"a": 1}},
            "neg": {"type": "g_counter", "counts": {"b": 1}}
        })
    );
    let back: PNCounter = serde_json::from_value(wire(&p)).unwrap();
    assert_eq!(back.value(), 0);
}

#[test]
fn register_wire_form_has_type_timestamp_and_node_id() {
    let r = LWWRegister::with_node("v", 10.5, "n1");
    assert_eq!(
        wire(&r),
        json!({"type": "lww_register", "value": "v", "timestamp": 10.5, "node_id": "n1"})
    );
    assert_eq!(r.node_id(), "n1");
    let back: LWWRegister<String> = serde_json::from_value(json!({
        "type": "lww_register", "value": "v", "timestamp": 10.5, "node_id": "n1"
    }))
    .unwrap();
    assert_eq!(back.timestamp(), 10.5);
}

#[test]
fn register_new_keeps_the_integer_timestamp_api() {
    let r = LWWRegister::new("hello", 5);
    assert_eq!(r.timestamp(), 5);
    assert_eq!(r.node_id(), "");
}

#[test]
fn register_tie_is_settled_by_the_higher_node_id_in_either_order() {
    let a = LWWRegister::with_node("from-n1", 4.5, "n1");
    let b = LWWRegister::with_node("from-n2", 4.5, "n2");
    assert_eq!(*a.merge(&b).value(), "from-n2");
    assert_eq!(*b.merge(&a).value(), "from-n2");
}

// ---- LWWMap --------------------------------------------------------------------------

#[test]
fn lwwmap_set_get_delete() {
    let mut m = LWWMap::new();
    m.set("k", json!("v"), 1.5, "n1");
    assert_eq!(m.get("k"), Some(&json!("v")));
    m.delete("k", 2.5);
    assert_eq!(m.get("k"), None);
    m.set("k", json!("w"), 3.5, "n1");
    assert_eq!(m.get("k"), Some(&json!("w")));
}

#[test]
fn lwwmap_a_tombstone_as_new_as_the_register_hides_it() {
    let m: LWWMap = serde_json::from_value(json!({
        "type": "lww_map",
        "registers": {"k": {"type": "lww_register", "value": 1, "timestamp": 5.5, "node_id": "n"}},
        "tombstones": {"k": 5.5}
    }))
    .unwrap();
    assert_eq!(m.get("k"), None);
    assert!(m.value().is_empty());
}

#[test]
fn lwwmap_a_register_newer_than_a_merged_tombstone_is_live() {
    let mut a = LWWMap::new();
    a.set("k", json!("new"), 5.5, "n1");
    let mut b = LWWMap::new();
    b.delete("k", 3.5);
    let merged = a.merge(&b);
    assert_eq!(merged.get("k"), Some(&json!("new")));
    assert_eq!(merged.value().len(), 1);
}

#[test]
fn lwwmap_merge_keeps_a_tombstone_with_timestamp_zero() {
    let mut a = LWWMap::new();
    a.delete("k", 0.0);
    assert_eq!(wire(&a.merge(&LWWMap::new()))["tombstones"], json!({"k": 0.0}));
}

#[test]
fn lwwmap_wire_form_round_trips_in_canonical_form() {
    let mut m = LWWMap::new();
    m.set("a", json!({"z": [1, null], "b": true}), 10.5, "n1");
    m.delete("gone", 11.5);
    let v = wire(&m);
    let back: LWWMap = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(emit_canonical(&wire(&back)), emit_canonical(&v));
}

// ---- canonical form ------------------------------------------------------------------

#[test]
fn canonicalize_sorts_the_tag_lists_of_every_or_set_also_nested() {
    let out = canonicalize(&json!({
        "tags": {"type": "or_set", "elements": {"x": ["b", "a"]}, "tombstones": ["d", "c"]},
        "other": ["z", "y"]
    }));
    assert_eq!(out["tags"]["elements"]["x"], json!(["a", "b"]));
    assert_eq!(out["tags"]["tombstones"], json!(["c", "d"]));
    assert_eq!(out["other"], json!(["z", "y"]));
}

#[test]
fn emit_canonical_sorts_keys_by_code_point_and_ends_with_a_newline() {
    let s = emit_canonical(&json!({"b": 1, "a": [], "\u{e9}": {}, "Z": "x"}));
    assert_eq!(s, "{\n  \"Z\": \"x\",\n  \"a\": [],\n  \"b\": 1,\n  \"\u{e9}\": {}\n}\n");
}

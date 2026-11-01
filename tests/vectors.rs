//! Protocol vectors written by the reference implementation (see tests/vectors/SOURCE).
//!
//! For every vector this edition reads the wire form, re-emits it, and must produce the same
//! bytes as the file. Canonical form: UTF-8 JSON, keys sorted, two-space indent, trailing
//! newline, or_set tag lists sorted.

use crdt_merge::ORSet;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn read(name: &str) -> (String, Value) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/vectors");
    path.push(format!("{name}.json"));
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let doc: Value = serde_json::from_str(&raw).expect("vector file is JSON");
    (raw, doc)
}

/// serde_json keeps object keys sorted and pretty-prints with two spaces, which is the canonical layout.
fn emit(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("serialise");
    s.push('\n');
    s
}

/// Sort the tag lists of every or_set.
fn canonical(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        Value::Object(o) => {
            let mut out = serde_json::Map::new();
            for (k, x) in o {
                out.insert(k.clone(), canonical(x));
            }
            if o.get("type") == Some(&json!("or_set")) {
                if let Some(Value::Object(elements)) = out.get_mut("elements") {
                    for (_, tags) in elements.iter_mut() {
                        if let Value::Array(t) = tags {
                            t.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                        }
                    }
                }
                if let Some(Value::Array(t)) = out.get_mut("tombstones") {
                    t.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Read a wire state with this edition's type and write it back out.
fn re_emit(wire: &Value) -> Result<Value, String> {
    match wire["type"].as_str().unwrap_or("") {
        "or_set" => {
            let set: ORSet<String> = serde_json::from_value(wire.clone()).map_err(|e| e.to_string())?;
            serde_json::to_value(&set).map_err(|e| e.to_string())
        }
        other => Err(format!("this edition has no type for wire type {other}")),
    }
}

fn check(name: &str) {
    let (raw, doc) = read(name);
    assert_eq!(emit(&doc), raw, "{name}.json is not in canonical form");

    let mut failures: Vec<String> = Vec::new();
    let mut try_state = |label: String, state: &Value| match re_emit(state) {
        Ok(again) => {
            if emit(&canonical(&again)) != emit(state) {
                failures.push(format!("{label}: re-emitted differently"));
            }
        }
        Err(e) => failures.push(format!("{label}: {e}")),
    };
    for v in doc["vectors"].as_array().expect("vectors") {
        try_state(format!("state {}", v["name"].as_str().unwrap()), &v["state"]);
    }
    for m in doc["merges"].as_array().expect("merges") {
        for side in ["a", "b", "result"] {
            try_state(format!("merge {} {side}", m["name"].as_str().unwrap()), &m[side]);
        }
    }
    assert!(failures.is_empty(), "{} vectors do not round-trip:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn orset_vectors_re_emit_byte_identically() {
    check("orset");
}

#[test]
fn lwwmap_vectors_re_emit_byte_identically() {
    check("lwwmap");
}

#[test]
fn agentstate_vectors_re_emit_byte_identically() {
    check("agentstate");
}

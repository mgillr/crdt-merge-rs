//! Protocol vectors written by the reference implementation (see tests/vectors/SOURCE).
//!
//! For every vector this edition reads the wire form, re-emits it, and must produce the same
//! bytes as the file. Canonical form: UTF-8 JSON, keys sorted, two-space indent, trailing
//! newline, or_set tag lists sorted. Merge vectors are also merged here and compared.

use crdt_merge::canonical::{canonicalize, emit_canonical};
use crdt_merge::{AgentState, Element, LWWMap, ORSet};
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

/// A wire state read with this edition's type.
enum State {
    Set(ORSet<Element>),
    Map(LWWMap),
    Agent(Box<AgentState>),
}

fn load(wire: &Value) -> Result<State, String> {
    let e = |x: serde_json::Error| x.to_string();
    match wire["type"].as_str().unwrap_or("") {
        "or_set" => serde_json::from_value(wire.clone())
            .map(State::Set)
            .map_err(e),
        "lww_map" => serde_json::from_value(wire.clone())
            .map(State::Map)
            .map_err(e),
        "agent_state" => serde_json::from_value(wire.clone())
            .map(State::Agent)
            .map_err(e),
        other => Err(format!("unknown wire type {other}")),
    }
}

fn dump(state: &State) -> Value {
    let v = match state {
        State::Set(s) => serde_json::to_value(s),
        State::Map(s) => serde_json::to_value(s),
        State::Agent(s) => serde_json::to_value(s),
    };
    canonicalize(&v.expect("serialise"))
}

fn merge(a: &State, b: &State) -> State {
    match (a, b) {
        (State::Set(a), State::Set(b)) => State::Set(a.merge(b)),
        (State::Map(a), State::Map(b)) => State::Map(a.merge(b)),
        (State::Agent(a), State::Agent(b)) => State::Agent(Box::new(a.merge(b))),
        _ => panic!("mixed types"),
    }
}

fn observable(state: &State) -> Value {
    match state {
        State::Set(s) => {
            let mut keys: Vec<String> = s.value().iter().map(|e| e.to_string()).collect();
            keys.sort();
            json!(keys)
        }
        State::Map(m) => json!(m.value()),
        State::Agent(a) => {
            let facts: serde_json::Map<String, Value> = a
                .list_facts()
                .iter()
                .map(|(k, f)| (k.clone(), serde_json::to_value(f).unwrap()))
                .collect();
            let counters: serde_json::Map<String, Value> = a
                .counter_names()
                .iter()
                .map(|n| (n.clone(), json!(a.counter_value(n))))
                .collect();
            json!({
                "facts": facts,
                "tags": a.tags().iter().collect::<Vec<_>>(),
                "counters": counters,
                "messages": a.messages().len(),
            })
        }
    }
}

fn check(name: &str) {
    let (raw, doc) = read(name);
    assert_eq!(
        emit_canonical(&doc),
        raw,
        "{name}.json is not in canonical form"
    );

    let mut failures: Vec<String> = Vec::new();
    let mut round_trip = |label: String, wire: &Value| match load(wire) {
        Ok(state) => {
            if emit_canonical(&dump(&state)) != emit_canonical(wire) {
                failures.push(format!("{label}: re-emitted differently"));
            }
        }
        Err(e) => failures.push(format!("{label}: {e}")),
    };
    for v in doc["vectors"].as_array().expect("vectors") {
        round_trip(
            format!("state {}", v["name"].as_str().unwrap()),
            &v["state"],
        );
    }
    for m in doc["merges"].as_array().expect("merges") {
        for side in ["a", "b", "result"] {
            round_trip(
                format!("merge {} {side}", m["name"].as_str().unwrap()),
                &m[side],
            );
        }
    }
    assert!(
        failures.is_empty(),
        "{} vectors do not round-trip:\n{}",
        failures.len(),
        failures.join("\n")
    );

    let mut mismatches: Vec<String> = Vec::new();
    for v in doc["vectors"].as_array().unwrap() {
        let state = load(&v["state"]).unwrap();
        if emit_canonical(&observable(&state)) != emit_canonical(&v["value"]) {
            mismatches.push(format!(
                "state {}: observable value differs",
                v["name"].as_str().unwrap()
            ));
        }
    }
    for m in doc["merges"].as_array().unwrap() {
        let (a, b) = (load(&m["a"]).unwrap(), load(&m["b"]).unwrap());
        let want = emit_canonical(&m["result"]);
        let label = m["name"].as_str().unwrap();
        if emit_canonical(&dump(&merge(&a, &b))) != want {
            mismatches.push(format!("merge {label}: a with b differs"));
        }
        if emit_canonical(&dump(&merge(&b, &a))) != want {
            mismatches.push(format!("merge {label}: b with a differs"));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
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

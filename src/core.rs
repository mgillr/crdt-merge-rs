// Copyright 2026 Ryan Gillespie
// SPDX-License-Identifier: Apache-2.0
//
// Commercial licensing: data@optitransfer.ch, rgillespie83@icloud.com

//! Core CRDT types: GCounter, PNCounter, LWWRegister, ORSet, LWWMap.
//!
//! Every type reads and writes the wire form shared with the other editions (see `canonical`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use ordered_float::OrderedFloat;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// GCounter – Grow-only counter
// ---------------------------------------------------------------------------

/// A grow-only counter backed by a per-node map of `u64` values.
/// The counter value is the sum across all nodes.
/// Merge takes the max per node (least-upper-bound).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename = "g_counter")]
pub struct GCounter {
    counts: HashMap<String, u64>,
}

impl GCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increment the counter for the given node by 1.
    pub fn increment(&mut self, node: &str) {
        *self.counts.entry(node.to_string()).or_insert(0) += 1;
    }

    /// Increment the counter for the given node by an arbitrary amount.
    pub fn increment_by(&mut self, node: &str, amount: u64) {
        *self.counts.entry(node.to_string()).or_insert(0) += amount;
    }

    /// Merge two GCounters by taking the max per node.
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.counts.clone();
        for (node, &val) in &other.counts {
            let entry = result.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(val);
        }
        Self { counts: result }
    }

    /// The counter value: sum of all node counts.
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Get the count for a specific node.
    pub fn get(&self, node: &str) -> u64 {
        self.counts.get(node).copied().unwrap_or(0)
    }
}

impl Default for GCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PNCounter – Positive-Negative counter
// ---------------------------------------------------------------------------

/// A counter that supports both increment and decrement via two GCounters.
/// Value = positive.value() - negative.value().
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename = "pn_counter")]
pub struct PNCounter {
    #[serde(rename = "pos")]
    positive: GCounter,
    #[serde(rename = "neg")]
    negative: GCounter,
}

impl PNCounter {
    /// Create an empty PN counter.
    pub fn new() -> Self {
        Self {
            positive: GCounter::new(),
            negative: GCounter::new(),
        }
    }

    /// Increment the counter for the given node.
    pub fn increment(&mut self, node: &str) {
        self.positive.increment(node);
    }

    /// Increment by an arbitrary amount.
    pub fn increment_by(&mut self, node: &str, amount: u64) {
        self.positive.increment_by(node, amount);
    }

    /// Decrement the counter for the given node.
    pub fn decrement(&mut self, node: &str) {
        self.negative.increment(node);
    }

    /// Decrement by an arbitrary amount.
    pub fn decrement_by(&mut self, node: &str, amount: u64) {
        self.negative.increment_by(node, amount);
    }

    /// Merge two PNCounters.
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            positive: self.positive.merge(&other.positive),
            negative: self.negative.merge(&other.negative),
        }
    }

    /// The counter value: positive - negative. Can be negative.
    pub fn value(&self) -> i64 {
        self.positive.value() as i64 - self.negative.value() as i64
    }
}

impl Default for PNCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LWWRegister – Last-Writer-Wins Register
// ---------------------------------------------------------------------------

/// A timestamp in seconds since the epoch. It is a float on the wire, so a fractional time
/// survives a round trip. Integer literals and integer variables convert with `into()`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub f64);

// A timestamp is never NaN in practice; this keeps `Eq` on the register as before.
impl Eq for Timestamp {}

impl Timestamp {
    /// The timestamp as a float.
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

macro_rules! timestamp_from {
    ($($t:ty),*) => {$(
        impl From<$t> for Timestamp {
            fn from(v: $t) -> Self {
                Timestamp(v as f64)
            }
        }
        impl PartialEq<$t> for Timestamp {
            fn eq(&self, other: &$t) -> bool {
                self.0 == *other as f64
            }
        }
    )*};
}
timestamp_from!(f64, i32, i64, u32, u64);

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A register where the value with the highest timestamp wins on merge.
/// Equal timestamps are settled by the greater `node_id`. When the node ids are also equal,
/// the value from `other` is kept (right-bias).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename = "lww_register")]
pub struct LWWRegister<T: Clone + Debug> {
    value: T,
    timestamp: Timestamp,
    #[serde(default)]
    node_id: String,
}

impl<T: Clone + Debug> LWWRegister<T> {
    /// Create a new register with the given value and timestamp, and an empty node id.
    pub fn new(value: T, timestamp: impl Into<Timestamp>) -> Self {
        Self::with_node(value, timestamp, "")
    }

    /// Create a register that records which node wrote it.
    pub fn with_node(value: T, timestamp: impl Into<Timestamp>, node_id: &str) -> Self {
        Self {
            value,
            timestamp: timestamp.into(),
            node_id: node_id.to_string(),
        }
    }

    /// Update the value if the new timestamp is >= the current one.
    pub fn set(&mut self, value: T, timestamp: impl Into<Timestamp>) {
        let timestamp = timestamp.into();
        if timestamp >= self.timestamp {
            self.value = value;
            self.timestamp = timestamp;
        }
    }

    /// Merge two registers: the higher timestamp wins, then the greater node id.
    /// On equal timestamp and node id, `other` wins (right-bias).
    pub fn merge(&self, other: &Self) -> Self {
        let other_wins = other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.node_id >= self.node_id);
        if other_wins {
            other.clone()
        } else {
            self.clone()
        }
    }

    /// Get a reference to the current value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Get the current timestamp.
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// The node that wrote the current value (empty if none was recorded).
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

// ---------------------------------------------------------------------------
// ORSet – Observed-Remove Set
// ---------------------------------------------------------------------------

/// How an element is written on the wire: a string key plus, for non-strings, a type name.
pub trait WireElement: Sized {
    /// The wire key and, for a non-string element, its type name (`int`, `float` or `bool`).
    fn to_wire(&self) -> (String, Option<&'static str>);
    /// Rebuild an element from its wire key and type name.
    fn from_wire(key: &str, ty: Option<&str>) -> Result<Self, String>;
}

fn wrong_type<T>(key: &str, ty: Option<&str>, want: &str) -> Result<T, String> {
    Err(format!("element {key:?} has type {ty:?}, expected {want}"))
}

impl WireElement for String {
    fn to_wire(&self) -> (String, Option<&'static str>) {
        (self.clone(), None)
    }
    fn from_wire(key: &str, ty: Option<&str>) -> Result<Self, String> {
        match ty {
            None => Ok(key.to_string()),
            Some(_) => wrong_type(key, ty, "a string"),
        }
    }
}

macro_rules! wire_int {
    ($($t:ty),*) => {$(
        impl WireElement for $t {
            fn to_wire(&self) -> (String, Option<&'static str>) {
                (self.to_string(), Some("int"))
            }
            fn from_wire(key: &str, ty: Option<&str>) -> Result<Self, String> {
                match ty {
                    Some("int") => key.parse::<$t>().map_err(|e| format!("element {key:?}: {e}")),
                    _ => wrong_type(key, ty, "int"),
                }
            }
        }
    )*};
}
wire_int!(i32, i64, u32, u64);

impl WireElement for bool {
    fn to_wire(&self) -> (String, Option<&'static str>) {
        ((if *self { "True" } else { "False" }).to_string(), Some("bool"))
    }
    fn from_wire(key: &str, ty: Option<&str>) -> Result<Self, String> {
        match ty {
            Some("bool") => Ok(key.eq_ignore_ascii_case("true")),
            _ => wrong_type(key, ty, "bool"),
        }
    }
}

/// An element of any wire type. `ORSet<Element>` reads every set written by any edition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    /// A string element.
    Str(String),
    /// An integer element.
    Int(i64),
    /// A floating-point element.
    Float(OrderedFloat<f64>),
    /// A boolean element.
    Bool(bool),
}

impl WireElement for Element {
    fn to_wire(&self) -> (String, Option<&'static str>) {
        match self {
            Element::Str(s) => s.to_wire(),
            Element::Int(n) => n.to_wire(),
            Element::Float(f) => (format!("{:?}", f.0), Some("float")),
            Element::Bool(b) => b.to_wire(),
        }
    }
    fn from_wire(key: &str, ty: Option<&str>) -> Result<Self, String> {
        match ty {
            None => Ok(Element::Str(key.to_string())),
            Some("int") => i64::from_wire(key, ty).map(Element::Int),
            Some("float") => key
                .parse::<f64>()
                .map(|f| Element::Float(OrderedFloat(f)))
                .map_err(|e| format!("element {key:?}: {e}")),
            Some("bool") => bool::from_wire(key, ty).map(Element::Bool),
            // The reference keeps an unknown type name as a plain string.
            Some(_) => Ok(Element::Str(key.to_string())),
        }
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_wire().0)
    }
}

/// An add-wins observed-remove set.
///
/// Each element addition is tagged with a unique UUID.
/// Removes only affect tags that have been *observed* (i.e., present at
/// the time of the remove) and record them as tombstones, so a stale replica
/// cannot bring a removed element back. Concurrent adds therefore "win" over removes.
///
/// The wire form is `{"type": "or_set", "elements": {...}, "element_types": {...},
/// "tombstones": [...]}`. `element_types` and `tombstones` appear only when they are not empty,
/// and a state written without them still loads.
#[derive(Clone, Debug)]
pub struct ORSet<T: Clone + Debug + Eq + Hash> {
    /// Mapping from element → set of unique add-tags.
    elements: HashMap<T, HashSet<String>>,
    /// Set of all tombstoned (removed) tags.
    tombstones: HashSet<String>,
}

impl<T: Clone + Debug + Eq + Hash> ORSet<T> {
    /// Create an empty set.
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            tombstones: HashSet::new(),
        }
    }

    /// Add an element, returning the unique tag generated.
    pub fn add(&mut self, element: T) -> String {
        let tag = Uuid::new_v4().to_string();
        self.elements
            .entry(element)
            .or_default()
            .insert(tag.clone());
        tag
    }

    /// Remove an element by tombstoning all of its currently observed tags.
    /// The element stays in the map with no live tag, as in the reference.
    pub fn remove(&mut self, element: &T) {
        if let Some(tags) = self.elements.get_mut(element) {
            for tag in tags.drain() {
                self.tombstones.insert(tag);
            }
        }
    }

    /// Remove one tag of an element. The element stays live while another tag remains.
    pub fn remove_tag(&mut self, element: &T, tag: &str) -> Result<(), String> {
        match self.elements.get_mut(element) {
            Some(tags) => {
                self.tombstones.insert(tag.to_string());
                tags.remove(tag);
                Ok(())
            }
            None => Err(format!("{element:?} not found in ORSet")),
        }
    }

    /// Check if an element is in the set (has at least one live tag).
    pub fn contains(&self, element: &T) -> bool {
        if let Some(tags) = self.elements.get(element) {
            tags.iter().any(|t| !self.tombstones.contains(t))
        } else {
            false
        }
    }

    /// Return the set of currently live elements.
    pub fn value(&self) -> HashSet<T> {
        let mut result = HashSet::new();
        for (elem, tags) in &self.elements {
            if tags.iter().any(|t| !self.tombstones.contains(t)) {
                result.insert(elem.clone());
            }
        }
        result
    }

    /// Merge two ORSets.
    /// Union of elements (per-tag), union of tombstones.
    /// An element is live if it has at least one tag not in the combined tombstone set.
    pub fn merge(&self, other: &Self) -> Self {
        let mut elements: HashMap<T, HashSet<String>> = HashMap::new();
        let tombstones: HashSet<String> =
            self.tombstones.union(&other.tombstones).cloned().collect();

        // Union of all element→tag mappings
        for (elem, tags) in &self.elements {
            elements
                .entry(elem.clone())
                .or_default()
                .extend(tags.iter().cloned());
        }
        for (elem, tags) in &other.elements {
            elements
                .entry(elem.clone())
                .or_default()
                .extend(tags.iter().cloned());
        }

        // Remove tags that are tombstoned. An emptied element keeps its key.
        for tags in elements.values_mut() {
            tags.retain(|t| !tombstones.contains(t));
        }

        Self {
            elements,
            tombstones,
        }
    }

    /// Number of live elements.
    pub fn len(&self) -> usize {
        self.value().len()
    }

    /// Whether the set has no live elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Clone + Debug + Eq + Hash> Default for ORSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Debug + Eq + Hash + PartialEq> PartialEq for ORSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
    }
}

impl<T: Clone + Debug + Eq + Hash> Eq for ORSet<T> {}

fn or_set_kind() -> String {
    "or_set".to_string()
}

/// The wire shape of an ORSet. Keys and tag lists are held sorted, so output is stable.
#[derive(Serialize, Deserialize)]
struct ORSetWire {
    #[serde(rename = "type", default = "or_set_kind")]
    kind: String,
    #[serde(default)]
    elements: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    element_types: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tombstones: Vec<String>,
}

impl<T: Clone + Debug + Eq + Hash + WireElement> Serialize for ORSet<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut wire = ORSetWire {
            kind: or_set_kind(),
            elements: BTreeMap::new(),
            element_types: BTreeMap::new(),
            tombstones: self.tombstones.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect(),
        };
        for (element, tags) in &self.elements {
            let (key, ty) = element.to_wire();
            if let Some(ty) = ty {
                wire.element_types.insert(key.clone(), ty.to_string());
            }
            wire.elements.insert(key, tags.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect());
        }
        wire.serialize(serializer)
    }
}

impl<'de, T: Clone + Debug + Eq + Hash + WireElement> Deserialize<'de> for ORSet<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let wire = ORSetWire::deserialize(deserializer)?;
        if wire.kind != "or_set" {
            return Err(D::Error::custom(format!("expected an or_set, found {:?}", wire.kind)));
        }
        let tombstones: HashSet<String> = wire.tombstones.into_iter().collect();
        let mut elements = HashMap::new();
        for (key, tags) in wire.elements {
            let element = T::from_wire(&key, wire.element_types.get(&key).map(String::as_str)).map_err(D::Error::custom)?;
            let live: HashSet<String> = tags.into_iter().filter(|t| !tombstones.contains(t)).collect();
            elements.insert(element, live);
        }
        Ok(Self { elements, tombstones })
    }
}

// ---------------------------------------------------------------------------
// LWWMap - Last-Writer-Wins Map
// ---------------------------------------------------------------------------

/// A dictionary where each key is a last-writer-wins register of a JSON value.
///
/// A key is live unless a tombstone is at least as new as its register. Concurrent writes to
/// one key are settled by the later timestamp, then by the greater node id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename = "lww_map")]
pub struct LWWMap {
    #[serde(default)]
    registers: HashMap<String, LWWRegister<Value>>,
    #[serde(default)]
    tombstones: HashMap<String, f64>,
}

impl LWWMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Write a key. The write replaces the register, and clears the tombstone if it is newer.
    pub fn set(&mut self, key: &str, value: Value, timestamp: f64, node_id: &str) {
        self.registers
            .insert(key.to_string(), LWWRegister::with_node(value, timestamp, node_id));
        if let Some(&tomb) = self.tombstones.get(key) {
            if timestamp > tomb {
                self.tombstones.remove(key);
            }
        }
    }

    /// Delete a key by recording a tombstone.
    pub fn delete(&mut self, key: &str, timestamp: f64) {
        self.tombstones.insert(key.to_string(), timestamp);
    }

    fn is_live(&self, key: &str, reg: &LWWRegister<Value>) -> bool {
        match self.tombstones.get(key) {
            None => true,
            Some(&tomb) => reg.timestamp().as_f64() > tomb,
        }
    }

    /// The value of a live key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        let reg = self.registers.get(key)?;
        if self.is_live(key, reg) {
            Some(reg.value())
        } else {
            None
        }
    }

    /// All live keys and values.
    pub fn value(&self) -> BTreeMap<String, Value> {
        self.registers
            .iter()
            .filter(|(k, reg)| self.is_live(k, reg))
            .map(|(k, reg)| (k.clone(), reg.value().clone()))
            .collect()
    }

    /// Merge two maps: registers by `LWWRegister::merge`, tombstones by the later time.
    pub fn merge(&self, other: &Self) -> Self {
        let mut registers = HashMap::new();
        for key in self.registers.keys().chain(other.registers.keys()) {
            let merged = match (self.registers.get(key), other.registers.get(key)) {
                (Some(a), Some(b)) => a.merge(b),
                (Some(a), None) => a.clone(),
                (None, Some(b)) => b.clone(),
                (None, None) => continue,
            };
            registers.insert(key.clone(), merged);
        }
        let mut tombstones = self.tombstones.clone();
        for (key, &ts) in &other.tombstones {
            let entry = tombstones.entry(key.clone()).or_insert(ts);
            if ts > *entry {
                *entry = ts;
            }
        }
        Self { registers, tombstones }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcounter_basic() {
        let mut c = GCounter::new();
        c.increment("a");
        c.increment("a");
        c.increment("b");
        assert_eq!(c.value(), 3);
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
    }

    #[test]
    fn gcounter_merge() {
        let mut a = GCounter::new();
        a.increment("x");
        a.increment("x");
        let mut b = GCounter::new();
        b.increment("x");
        b.increment("y");
        let merged = a.merge(&b);
        assert_eq!(merged.get("x"), 2); // max(2, 1)
        assert_eq!(merged.get("y"), 1);
        assert_eq!(merged.value(), 3);
    }

    #[test]
    fn pncounter_basic() {
        let mut c = PNCounter::new();
        c.increment("a");
        c.increment("a");
        c.decrement("a");
        assert_eq!(c.value(), 1);
    }

    #[test]
    fn lww_merge() {
        let a = LWWRegister::new("hello", 1);
        let b = LWWRegister::new("world", 2);
        let merged = a.merge(&b);
        assert_eq!(*merged.value(), "world");
    }

    #[test]
    fn orset_add_remove() {
        let mut s = ORSet::new();
        s.add("a");
        s.add("b");
        assert!(s.contains(&"a"));
        s.remove(&"a");
        assert!(!s.contains(&"a"));
        assert!(s.contains(&"b"));
    }
}

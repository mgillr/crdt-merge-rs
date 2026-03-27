// Copyright 2026 Ryan Gillespie
// SPDX-License-Identifier: Apache-2.0
//
// Commercial licensing: data@optitransfer.ch, rgillespie83@icloud.com

//! Core CRDT types: GCounter, PNCounter, LWWRegister, ORSet.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// GCounter – Grow-only counter
// ---------------------------------------------------------------------------

/// A grow-only counter backed by a per-node map of `u64` values.
/// The counter value is the sum across all nodes.
/// Merge takes the max per node (least-upper-bound).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct PNCounter {
    positive: GCounter,
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

/// A register where the value with the highest timestamp wins on merge.
/// Ties are broken by keeping the value from `other` (right-bias).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LWWRegister<T: Clone + Debug> {
    value: T,
    timestamp: u64,
}

impl<T: Clone + Debug> LWWRegister<T> {
    /// Create a new register with the given value and timestamp.
    pub fn new(value: T, timestamp: u64) -> Self {
        Self { value, timestamp }
    }

    /// Update the value if the new timestamp is >= the current one.
    pub fn set(&mut self, value: T, timestamp: u64) {
        if timestamp >= self.timestamp {
            self.value = value;
            self.timestamp = timestamp;
        }
    }

    /// Merge two registers – keep the one with the higher timestamp.
    /// On equal timestamps, `other` wins (right-bias).
    pub fn merge(&self, other: &Self) -> Self {
        if other.timestamp >= self.timestamp {
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
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

// ---------------------------------------------------------------------------
// ORSet – Observed-Remove Set
// ---------------------------------------------------------------------------

/// An add-wins observed-remove set.
///
/// Each element addition is tagged with a unique UUID.  
/// Removes only affect tags that have been *observed* (i.e., present at
/// the time of the remove). Concurrent adds therefore "win" over removes.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
            .or_insert_with(HashSet::new)
            .insert(tag.clone());
        tag
    }

    /// Remove an element by tombstoning all of its currently observed tags.
    pub fn remove(&mut self, element: &T) {
        if let Some(tags) = self.elements.get(element) {
            for tag in tags.iter() {
                self.tombstones.insert(tag.clone());
            }
        }
        self.elements.remove(element);
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
                .or_insert_with(HashSet::new)
                .extend(tags.iter().cloned());
        }
        for (elem, tags) in &other.elements {
            elements
                .entry(elem.clone())
                .or_insert_with(HashSet::new)
                .extend(tags.iter().cloned());
        }

        // Remove tags that are tombstoned, and remove empty entries
        for tags in elements.values_mut() {
            tags.retain(|t| !tombstones.contains(t));
        }
        elements.retain(|_, tags| !tags.is_empty());

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

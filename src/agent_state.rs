// SPDX-License-Identifier: Apache-2.0

//! AgentState: a CRDT container for one agent's facts, tags, counters and messages.
//!
//! Wraps an `LWWMap` (facts), an `ORSet` (tags) and `PNCounter`s, and reads and writes the same
//! wire form as the reference implementation.

use crate::core::{LWWMap, ORSet, PNCounter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_MAX_MESSAGES: usize = 10_000;

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn default_max_messages() -> usize {
    DEFAULT_MAX_MESSAGES
}

fn default_dedup() -> String {
    "content_agent_time".to_string()
}

/// A single fact with confidence and provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    /// The fact's payload.
    pub value: Value,
    /// Confidence in [0, 1].
    #[serde(default = "one")]
    pub confidence: f64,
    /// The agent that recorded the fact.
    #[serde(default)]
    pub source_agent: String,
    /// When the fact was recorded.
    #[serde(default)]
    pub timestamp: f64,
}

fn one() -> f64 {
    1.0
}

/// CRDT state for one agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename = "agent_state")]
pub struct AgentState {
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    facts: LWWMap,
    #[serde(default)]
    tags: ORSet<String>,
    #[serde(default)]
    counters: HashMap<String, PNCounter>,
    #[serde(default)]
    messages: Vec<Value>,
    #[serde(default)]
    created_at: f64,
    #[serde(skip, default = "default_max_messages")]
    max_messages: usize,
    #[serde(skip, default = "default_dedup")]
    dedup_strategy: String,
}

impl AgentState {
    /// A new, empty state created now.
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            facts: LWWMap::new(),
            tags: ORSet::new(),
            counters: HashMap::new(),
            messages: Vec::new(),
            created_at: now(),
            max_messages: DEFAULT_MAX_MESSAGES,
            dedup_strategy: default_dedup(),
        }
    }

    /// Set the creation time, in seconds since the epoch.
    pub fn with_created_at(mut self, created_at: f64) -> Self {
        self.created_at = created_at;
        self
    }

    /// Keep only the newest `max` messages.
    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self.trim();
        self
    }

    /// `content_agent_time` (default) or `content_only`.
    pub fn with_dedup_strategy(mut self, strategy: &str) -> Self {
        self.dedup_strategy = strategy.to_string();
        self
    }

    /// The agent id.
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    // -- facts ------------------------------------------------------------------

    /// Record a fact. The write replaces any earlier fact under the key.
    pub fn add_fact(&mut self, key: &str, value: Value, confidence: f64, timestamp: f64) {
        let fact = Fact {
            value,
            confidence,
            source_agent: self.agent_id.clone(),
            timestamp,
        };
        let id = self.agent_id.clone();
        self.facts
            .set(key, serde_json::to_value(&fact).expect("fact"), timestamp, &id);
    }

    /// The live fact under a key.
    pub fn get_fact(&self, key: &str) -> Option<Fact> {
        self.facts.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// All live facts.
    pub fn list_facts(&self) -> BTreeMap<String, Fact> {
        self.facts
            .value()
            .into_iter()
            .filter_map(|(k, v)| serde_json::from_value(v).ok().map(|f| (k, f)))
            .collect()
    }

    // -- tags -------------------------------------------------------------------

    /// Add a tag.
    pub fn add_tag(&mut self, tag: &str) {
        self.tags.add(tag.to_string());
    }

    /// Remove a tag, recording the observed tags as removed.
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.remove(&tag.to_string());
    }

    /// Whether a tag is live.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_string())
    }

    /// The live tags.
    pub fn tags(&self) -> BTreeSet<String> {
        self.tags.value().into_iter().collect()
    }

    // -- counters ---------------------------------------------------------------

    /// Increase a counter.
    pub fn increment(&mut self, name: &str, amount: u64) {
        let id = self.agent_id.clone();
        self.counters
            .entry(name.to_string())
            .or_insert_with(PNCounter::new)
            .increment_by(&id, amount);
    }

    /// Decrease a counter.
    pub fn decrement(&mut self, name: &str, amount: u64) {
        let id = self.agent_id.clone();
        self.counters
            .entry(name.to_string())
            .or_insert_with(PNCounter::new)
            .decrement_by(&id, amount);
    }

    /// The value of a counter, 0 if it does not exist.
    pub fn counter_value(&self, name: &str) -> i64 {
        self.counters.get(name).map(PNCounter::value).unwrap_or(0)
    }

    /// The names of all counters, sorted.
    pub fn counter_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.counters.keys().cloned().collect();
        names.sort();
        names
    }

    // -- messages ---------------------------------------------------------------

    /// Append to the message log. Only the newest `max_messages` are kept.
    pub fn append_message(&mut self, content: &str, role: &str, metadata: Value, timestamp: f64) {
        self.messages.push(json!({
            "content": content,
            "role": role,
            "agent_id": self.agent_id,
            "timestamp": timestamp,
            "metadata": metadata,
        }));
        self.trim();
    }

    /// A copy of the message log.
    pub fn messages(&self) -> Vec<Value> {
        self.messages.clone()
    }

    fn trim(&mut self) {
        if self.messages.len() > self.max_messages {
            let drop = self.messages.len() - self.max_messages;
            self.messages.drain(0..drop);
        }
    }

    // -- merge ------------------------------------------------------------------

    /// Merge into a new state: facts by `LWWMap`, tags by `ORSet`, counters per name, messages by
    /// union on the deduplication key, sorted by time. The agent id is the sorted union joined
    /// by `+`, and `created_at` is the earlier time. Neither input changes.
    pub fn merge(&self, other: &Self) -> Self {
        let ids: BTreeSet<&str> = [self.agent_id.as_str(), other.agent_id.as_str()].into_iter().collect();
        let mut result = AgentState::new(&ids.into_iter().collect::<Vec<_>>().join("+"));
        result.max_messages = self.max_messages.max(other.max_messages);
        result.dedup_strategy = self.dedup_strategy.clone();
        result.facts = self.facts.merge(&other.facts);
        result.tags = self.tags.merge(&other.tags);
        let names: HashSet<&String> = self.counters.keys().chain(other.counters.keys()).collect();
        for name in names {
            let merged = match (self.counters.get(name), other.counters.get(name)) {
                (Some(a), Some(b)) => a.merge(b),
                (Some(a), None) => a.clone(),
                (None, Some(b)) => b.clone(),
                (None, None) => continue,
            };
            result.counters.insert(name.clone(), merged);
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut merged: Vec<Value> = Vec::new();
        for msg in self.messages.iter().chain(other.messages.iter()) {
            let content = msg["content"].as_str().unwrap_or("");
            let key = if self.dedup_strategy == "content_only" {
                content.to_string()
            } else {
                let agent = msg["agent_id"].as_str().unwrap_or("");
                let ts = match &msg["timestamp"] {
                    Value::Null => String::new(),
                    v => v.to_string(),
                };
                format!("{content}:{agent}:{ts}")
            };
            if seen.insert(key) {
                merged.push(msg.clone());
            }
        }
        let time = |m: &Value| m["timestamp"].as_f64().unwrap_or(0.0);
        merged.sort_by(|a, b| time(a).partial_cmp(&time(b)).unwrap_or(std::cmp::Ordering::Equal));
        result.messages = merged;
        result.trim();
        result.created_at = self.created_at.min(other.created_at);
        result
    }
}

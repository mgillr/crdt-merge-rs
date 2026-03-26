<div align="center">

# 🔀 crdt-merge

**Conflict-free merge, dedup & diff for any dataset — powered by CRDTs**

[![crates.io](https://img.shields.io/crates/v/crdt-merge.svg)](https://crates.io/crates/crdt-merge)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Tests: 88/88](https://img.shields.io/badge/tests-88%2F88-brightgreen.svg)](https://github.com/mgillr/crdt-merge-rs)

**Merge any two datasets in one function call. No conflicts. No coordination. No data loss.**

[Quick Start](#-quick-start) • [CLI](#-cli-usage) • [Library API](#-library-usage) • [All Languages](#-available-in-every-language)

</div>

---

## 🌐 Available in Every Language

| Language | Package | Install | Repo |
|---|---|---|---|
| **Python** 🐍 | `crdt-merge` | `pip install crdt-merge` | [crdt-merge](https://github.com/mgillr/crdt-merge) |
| **TypeScript** | `crdt-merge` | `npm install crdt-merge` | [crdt-merge-ts](https://github.com/mgillr/crdt-merge-ts) |
| **Rust** 🦀 | `crdt-merge` | `cargo add crdt-merge` | **You are here** |
| **Java** ☕ | `io.optitransfer:crdt-merge` | Maven / Gradle | [crdt-merge-java](https://github.com/mgillr/crdt-merge-java) |
| **CLI** 🖥️ | included in Rust | `cargo install crdt-merge` | **You are here** |

> **[🤗 Try it in the browser →](https://huggingface.co/spaces/Optitransfer/crdt-merge)**

---

## 🎯 The Problem

You have two versions of a dataset. Maybe two services updated the same records. Maybe two contributors edited the same file. Maybe you're merging data from multiple sources.

**Today:** Write custom merge scripts, lose data, or block on a coordinator.

**With crdt-merge:** One function call. Zero conflicts. Mathematically guaranteed.

```rust
use crdt_merge::merge;
let merged = merge(&dataset_a, &dataset_b, None); // done.
```

## ⚡ Quick Start

### As a Library

```toml
[dependencies]
crdt-merge = "0.1.0"
```

### As a CLI Tool

```bash
cargo install crdt-merge
```

## 🖥️ CLI Usage

```bash
# Merge two CSV files
crdt-merge merge a.csv b.csv --key id --output merged.csv

# Deduplicate
crdt-merge dedup input.csv --key name --threshold 0.85 --output deduped.csv

# Diff two files
crdt-merge diff a.csv b.csv --key id

# Deep-merge two JSON files
crdt-merge json-merge a.json b.json --output merged.json
```

## 📦 Library Usage

### Tabular Merge

```rust
use crdt_merge::merge;
use serde_json::json;

let a = vec![
    json!({"id": "1", "name": "Alice", "role": "engineer"}),
    json!({"id": "2", "name": "Bob", "role": "designer"}),
];

let b = vec![
    json!({"id": "2", "name": "Robert", "role": "designer"}),
    json!({"id": "3", "name": "Charlie", "role": "pm"}),
];

let merged = merge(&a, &b, None);
// id=1: Alice (only in A — preserved)
// id=2: Robert (B wins — latest)
// id=3: Charlie (only in B — preserved)
```

### Deduplication

```rust
use crdt_merge::dedup;
use serde_json::json;

let data = vec![
    json!({"name": "Alice"}),
    json!({"name": "Alicia"}),
    json!({"name": "Bob"}),
];

// Fuzzy dedup — catches near-duplicates
let result = dedup(&data, Some("name"), Some(0.7));
println!("Unique: {}", result.unique.len());
println!("Duplicates: {}", result.duplicates.len());
```

### Structural Diff

```rust
use crdt_merge::diff;

let diff_result = diff(&old_data, &new_data, None);
println!("{}", diff_result.summary);
// "+5 added, -2 removed, ~3 modified, =990 unchanged"
```

### Deep JSON Merge

```rust
use crdt_merge::merge_json;
use serde_json::json;

let config_a = json!({"model": {"name": "bert", "layers": 12}, "tags": ["nlp"]});
let config_b = json!({"model": {"name": "bert-large", "dropout": 0.1}, "tags": ["qa"]});

let merged = merge_json(&config_a, &config_b, None);
// {"model": {"name": "bert-large", "layers": 12, "dropout": 0.1}, "tags": ["nlp", "qa"]}
```

### Core CRDT Types

```rust
use crdt_merge::{GCounter, PNCounter, LWWRegister, ORSet};

// Distributed counter
let mut counter_a = GCounter::new();
counter_a.increment("server-1", 100);

let mut counter_b = GCounter::new();
counter_b.increment("server-2", 200);

let merged = counter_a.merge(&counter_b);
assert_eq!(merged.value(), 300);

// Last-writer-wins register
let reg_a = LWWRegister::new("Alice".into(), 1000);
let reg_b = LWWRegister::new("Alicia".into(), 2000);
assert_eq!(reg_a.merge(&reg_b).value(), "Alicia"); // later wins

// Observed-remove set
let mut set_a = ORSet::new();
set_a.add("item1".to_string());
let mut set_b = ORSet::new();
set_b.add("item2".to_string());
let merged = set_a.merge(&set_b);
assert!(merged.contains(&"item1".to_string()));
assert!(merged.contains(&"item2".to_string()));
```

## 🧠 Why CRDTs

**CRDT** = Conflict-free Replicated Data Type. A data structure with one mathematical superpower:

> **Any two copies can merge — in any order, at any time — and the result is always identical and always correct.**

Three mathematical guarantees (proven, not hoped):

| Property | What it means |
|---|---|
| **Commutative** | `merge(A, B) == merge(B, A)` — order doesn't matter |
| **Associative** | `merge(merge(A, B), C) == merge(A, merge(B, C))` — grouping doesn't matter |
| **Idempotent** | `merge(A, A) == A` — re-merging is safe |

This means: **zero coordination, zero locks, zero conflicts.**

### Built-in CRDT Types

| Type | Use Case | Example |
|---|---|---|
| `GCounter` | Grow-only counters | Download counts, page views |
| `PNCounter` | Increment + decrement | Stock levels, balances |
| `LWWRegister` | Single value (latest wins) | Name, email, status fields |
| `ORSet` | Add/remove set | Tags, memberships, dedup sets |

## Features

- **Core CRDTs**: GCounter, PNCounter, LWWRegister, ORSet — all with merge, serialize, deserialize
- **Tabular Merge**: Merge two datasets by key with LWW / KeepA / KeepB strategies
- **Dedup**: Exact + fuzzy deduplication with Jaccard similarity
- **Diff**: Structural diff showing added, removed, modified & unchanged records
- **JSON Merge**: Deep recursive merge of arbitrary JSON structures
- **CLI**: Merge, dedup, diff, and json-merge commands for CSV/JSON files

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

Copyright 2026 Ryan Gillespie / Optitransfer. See [NOTICE](NOTICE) for attribution requirements.

For commercial licensing inquiries: leer@optitransfer.ch

---

<div align="center">

Built with math, not hope. 🧬

**[⭐ Star on GitHub](https://github.com/mgillr/crdt-merge-rs)** • **[🤗 Try on HuggingFace](https://huggingface.co/spaces/Optitransfer/crdt-merge)** • **[📦 crates.io](https://crates.io/crates/crdt-merge)**

</div>

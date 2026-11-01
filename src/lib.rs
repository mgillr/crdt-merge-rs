// Copyright 2026 Ryan Gillespie
// SPDX-License-Identifier: Apache-2.0
//
// Commercial licensing: data@optitransfer.ch, rgillespie83@icloud.com

//! # crdt-merge
//!
//! Conflict-free merge, dedup & diff for any dataset. Powered by CRDTs.
//!
//! This library provides:
//! - Core CRDT types: GCounter, PNCounter, LWWRegister, ORSet
//! - Tabular merge engine for JSON record arrays
//! - Fuzzy & exact deduplication
//! - Structural diff
//! - Deep JSON merge

pub mod core;
pub mod dedup;
pub mod diff;
pub mod json_merge;
pub mod merge;

// Re-exports for convenience
pub use crate::core::{GCounter, LWWRegister, ORSet, PNCounter};
pub use crate::dedup::{dedup, DedupResult, DuplicatePair};
pub use crate::diff::{diff, DiffResult, FieldChange, ModifiedRow};
pub use crate::json_merge::merge_json;
pub use crate::merge::{merge, MergeOptions, Strategy};

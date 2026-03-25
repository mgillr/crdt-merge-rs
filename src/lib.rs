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
pub mod merge;
pub mod dedup;
pub mod diff;
pub mod json_merge;

// Re-exports for convenience
pub use crate::core::{GCounter, PNCounter, LWWRegister, ORSet};
pub use crate::merge::{merge, MergeOptions, Strategy};
pub use crate::dedup::{dedup, DedupResult, DuplicatePair};
pub use crate::diff::{diff, DiffResult, ModifiedRow, FieldChange};
pub use crate::json_merge::merge_json;

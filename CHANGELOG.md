# Changelog

## 0.3.0 - 2026-11-01

This release makes the Rust edition read and write the same wire form as the reference
implementation, so a state written by any edition loads in the others and re-emits byte for
byte. The protocol vectors in `tests/vectors/` check this. They are copied from the reference at
commit `b92d0ef`.

### Breaking changes

Wire form
- `ORSet` is written as `{"type": "or_set", "elements": ..., "element_types": ..., "tombstones": ...}`.
  It has a `type` key, `tombstones` is a sorted list that is written only when it is not empty
  and may be missing on read, and `element_types` is written only for elements that are not
  strings. Tags are sorted. Before, there was no `type` key and `tombstones` was required.
- `GCounter`, `PNCounter` and `LWWRegister` carry a `type` key (`g_counter`, `pn_counter`,
  `lww_register`), and it is required when reading. State serialised by an earlier version
  without it no longer loads.
- `PNCounter` is written with the fields `pos` and `neg`, not `positive` and `negative`.
- `LWWRegister` is written as `{type, value, timestamp, node_id}`. The timestamp is a float and
  `node_id` is optional on read.

API
- `ORSet` implements `Serialize` and `Deserialize` only for element types that implement the new
  `WireElement` trait: `String`, `i32`, `i64`, `u32`, `u64`, `bool` and `Element`. Other element
  types lose serialisation. `ORSet<String>` rejects an element that carries a type name.
- `ORSet::remove` and `ORSet::merge` keep the key of an element that has no live tag left.
  `value`, `contains`, `len` and equality are unchanged.
- `LWWRegister::timestamp()` returns `Timestamp` (a wrapper around `f64`) instead of `u64`.
  Comparisons with integer literals still work, but `let t: u64 = r.timestamp()` does not.
  `LWWRegister::new` and `set` take `impl Into<Timestamp>`.
- `LWWRegister::merge` settles equal timestamps by the greater `node_id`. When the node ids are
  also equal, the right-hand register still wins.

### Added
- `LWWMap`, `AgentState` and `Fact`.
- `Element`, an element of any wire type, so `ORSet<Element>` reads every set written by any edition.
- `WireElement` and `Timestamp`.
- `ORSet::remove_tag` and `LWWRegister::with_node` and `node_id`.
- The `canonical` module with `canonicalize` and `emit_canonical`: sorted keys, two-space indent,
  trailing newline, sorted tag lists.
- Continuous integration runs `cargo test` on pull requests and on pushes to main. A second workflow, added with the formatting cleanup, also runs `cargo fmt --check` and `cargo clippy -D warnings`.

### Known gaps
- A whole-number float prints as `5.0` in Python and Rust but `5` in JavaScript. The vectors
  avoid such numbers.
- When timestamp and node id are both equal, the reference keeps the left value and this edition
  keeps the right value, so that merge is not commutative in either. `LWWRegister::set` ignores
  an older timestamp here but overwrites in the reference; an existing test pins the Rust behaviour.
- An unknown `element_types` name is read as a string by `Element`.

use crdt_merge::{GCounter, PNCounter, LWWRegister, ORSet};

// ===========================================================================
// GCounter tests
// ===========================================================================

#[test]
fn gcounter_new_is_zero() {
    let c = GCounter::new();
    assert_eq!(c.value(), 0);
}

#[test]
fn gcounter_increment_single_node() {
    let mut c = GCounter::new();
    c.increment("node1");
    c.increment("node1");
    c.increment("node1");
    assert_eq!(c.value(), 3);
    assert_eq!(c.get("node1"), 3);
}

#[test]
fn gcounter_increment_multiple_nodes() {
    let mut c = GCounter::new();
    c.increment("a");
    c.increment("b");
    c.increment("c");
    assert_eq!(c.value(), 3);
}

#[test]
fn gcounter_increment_by() {
    let mut c = GCounter::new();
    c.increment_by("a", 10);
    c.increment_by("b", 5);
    assert_eq!(c.value(), 15);
}

#[test]
fn gcounter_merge_takes_max() {
    let mut a = GCounter::new();
    a.increment_by("x", 5);
    a.increment_by("y", 3);

    let mut b = GCounter::new();
    b.increment_by("x", 3);
    b.increment_by("y", 7);
    b.increment_by("z", 2);

    let merged = a.merge(&b);
    assert_eq!(merged.get("x"), 5); // max(5, 3)
    assert_eq!(merged.get("y"), 7); // max(3, 7)
    assert_eq!(merged.get("z"), 2); // only in b
    assert_eq!(merged.value(), 14);
}

#[test]
fn gcounter_merge_commutative() {
    let mut a = GCounter::new();
    a.increment("x");
    a.increment("x");

    let mut b = GCounter::new();
    b.increment("y");

    let ab = a.merge(&b);
    let ba = b.merge(&a);
    assert_eq!(ab.value(), ba.value());
    assert_eq!(ab.get("x"), ba.get("x"));
    assert_eq!(ab.get("y"), ba.get("y"));
}

#[test]
fn gcounter_merge_associative() {
    let mut a = GCounter::new();
    a.increment("a");
    let mut b = GCounter::new();
    b.increment("b");
    let mut c = GCounter::new();
    c.increment("c");

    let ab_c = a.merge(&b).merge(&c);
    let a_bc = a.merge(&b.merge(&c));
    assert_eq!(ab_c.value(), a_bc.value());
}

#[test]
fn gcounter_merge_idempotent() {
    let mut a = GCounter::new();
    a.increment("x");
    a.increment("y");

    let merged_once = a.merge(&a);
    let merged_twice = merged_once.merge(&a);
    assert_eq!(a.value(), merged_once.value());
    assert_eq!(a.value(), merged_twice.value());
}

// ===========================================================================
// PNCounter tests
// ===========================================================================

#[test]
fn pncounter_new_is_zero() {
    let c = PNCounter::new();
    assert_eq!(c.value(), 0);
}

#[test]
fn pncounter_increment_only() {
    let mut c = PNCounter::new();
    c.increment("a");
    c.increment("a");
    assert_eq!(c.value(), 2);
}

#[test]
fn pncounter_decrement() {
    let mut c = PNCounter::new();
    c.increment("a");
    c.increment("a");
    c.increment("a");
    c.decrement("a");
    assert_eq!(c.value(), 2);
}

#[test]
fn pncounter_negative_value() {
    let mut c = PNCounter::new();
    c.decrement("a");
    c.decrement("a");
    assert_eq!(c.value(), -2);
}

#[test]
fn pncounter_merge() {
    let mut a = PNCounter::new();
    a.increment_by("x", 10);
    a.decrement_by("x", 3);

    let mut b = PNCounter::new();
    b.increment_by("x", 5);
    b.decrement_by("y", 2);

    let merged = a.merge(&b);
    // positive: max(10, 5) = 10
    // negative: max(3, 0) + max(0, 2) = 3 + 2 = 5
    assert_eq!(merged.value(), 5); // 10 - 5
}

#[test]
fn pncounter_merge_commutative() {
    let mut a = PNCounter::new();
    a.increment("x");
    a.decrement("y");

    let mut b = PNCounter::new();
    b.increment("y");
    b.decrement("x");

    assert_eq!(a.merge(&b).value(), b.merge(&a).value());
}

#[test]
fn pncounter_merge_idempotent() {
    let mut a = PNCounter::new();
    a.increment("x");
    a.decrement("y");

    let merged = a.merge(&a);
    assert_eq!(a.value(), merged.value());
}

// ===========================================================================
// LWWRegister tests
// ===========================================================================

#[test]
fn lww_new() {
    let r = LWWRegister::new("hello", 1);
    assert_eq!(*r.value(), "hello");
    assert_eq!(r.timestamp(), 1);
}

#[test]
fn lww_set_newer_timestamp() {
    let mut r = LWWRegister::new("hello", 1);
    r.set("world", 2);
    assert_eq!(*r.value(), "world");
    assert_eq!(r.timestamp(), 2);
}

#[test]
fn lww_set_older_timestamp_ignored() {
    let mut r = LWWRegister::new("hello", 5);
    r.set("world", 3);
    assert_eq!(*r.value(), "hello");
    assert_eq!(r.timestamp(), 5);
}

#[test]
fn lww_set_equal_timestamp_overwrites() {
    let mut r = LWWRegister::new("hello", 1);
    r.set("world", 1);
    assert_eq!(*r.value(), "world");
}

#[test]
fn lww_merge_higher_wins() {
    let a = LWWRegister::new("old", 1);
    let b = LWWRegister::new("new", 2);
    let merged = a.merge(&b);
    assert_eq!(*merged.value(), "new");
    assert_eq!(merged.timestamp(), 2);
}

#[test]
fn lww_merge_equal_ts_right_bias() {
    let a = LWWRegister::new("left", 5);
    let b = LWWRegister::new("right", 5);
    let merged = a.merge(&b);
    assert_eq!(*merged.value(), "right");
}

#[test]
fn lww_merge_commutative_different_ts() {
    let a = LWWRegister::new("a", 1);
    let b = LWWRegister::new("b", 2);
    // Higher ts always wins regardless of order
    assert_eq!(*a.merge(&b).value(), "b");
    assert_eq!(*b.merge(&a).value(), "b");
}

#[test]
fn lww_merge_idempotent() {
    let a = LWWRegister::new("hello", 5);
    let merged = a.merge(&a);
    assert_eq!(*merged.value(), *a.value());
    assert_eq!(merged.timestamp(), a.timestamp());
}

// ===========================================================================
// ORSet tests
// ===========================================================================

#[test]
fn orset_new_is_empty() {
    let s: ORSet<String> = ORSet::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
}

#[test]
fn orset_add_contains() {
    let mut s = ORSet::new();
    s.add("apple".to_string());
    assert!(s.contains(&"apple".to_string()));
    assert_eq!(s.len(), 1);
}

#[test]
fn orset_add_multiple() {
    let mut s = ORSet::new();
    s.add("a".to_string());
    s.add("b".to_string());
    s.add("c".to_string());
    assert_eq!(s.len(), 3);
}

#[test]
fn orset_remove() {
    let mut s = ORSet::new();
    s.add("x".to_string());
    s.add("y".to_string());
    s.remove(&"x".to_string());
    assert!(!s.contains(&"x".to_string()));
    assert!(s.contains(&"y".to_string()));
    assert_eq!(s.len(), 1);
}

#[test]
fn orset_add_after_remove() {
    let mut s = ORSet::new();
    s.add("x".to_string());
    s.remove(&"x".to_string());
    assert!(!s.contains(&"x".to_string()));
    s.add("x".to_string());
    assert!(s.contains(&"x".to_string()));
}

#[test]
fn orset_merge_union() {
    let mut a = ORSet::new();
    a.add("x".to_string());

    let mut b = ORSet::new();
    b.add("y".to_string());

    let merged = a.merge(&b);
    assert!(merged.contains(&"x".to_string()));
    assert!(merged.contains(&"y".to_string()));
    assert_eq!(merged.len(), 2);
}

#[test]
fn orset_merge_add_wins() {
    // a adds "x", b adds then removes "x"
    let mut a = ORSet::new();
    a.add("x".to_string());

    let mut b = ORSet::new();
    b.add("x".to_string());
    b.remove(&"x".to_string());

    // a's add should win because it has a unique tag not in b's tombstones
    let merged = a.merge(&b);
    assert!(merged.contains(&"x".to_string()));
}

#[test]
fn orset_merge_commutative() {
    let mut a = ORSet::new();
    a.add("x".to_string());
    a.add("y".to_string());

    let mut b = ORSet::new();
    b.add("y".to_string());
    b.add("z".to_string());

    let ab = a.merge(&b);
    let ba = b.merge(&a);
    assert_eq!(ab.value(), ba.value());
}

#[test]
fn orset_merge_idempotent() {
    let mut a = ORSet::new();
    a.add("x".to_string());
    a.add("y".to_string());

    let merged = a.merge(&a);
    assert_eq!(a.value(), merged.value());
}

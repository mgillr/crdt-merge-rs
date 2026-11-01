use crdt_merge::canonical::emit_canonical;
use crdt_merge::AgentState;
use serde_json::json;

#[test]
fn a_new_state_is_empty_and_keeps_its_id() {
    let s = AgentState::new("a1");
    assert_eq!(s.agent_id(), "a1");
    assert!(s.list_facts().is_empty());
    assert!(s.tags().is_empty());
    assert!(s.messages().is_empty());
    assert_eq!(s.counter_value("none"), 0);
}

#[test]
fn add_fact_records_value_confidence_source_and_time() {
    let mut s = AgentState::new("a1");
    s.add_fact("color", json!("blue"), 0.75, 10.5);
    let f = s.get_fact("color").unwrap();
    assert_eq!(f.value, json!("blue"));
    assert_eq!(f.confidence, 0.75);
    assert_eq!(f.source_agent, "a1");
    assert_eq!(f.timestamp, 10.5);
    assert!(s.get_fact("missing").is_none());
}

#[test]
fn add_fact_on_an_existing_key_overwrites_it_locally_as_the_reference_does() {
    let mut s = AgentState::new("a1");
    s.add_fact("k", json!("first"), 0.5, 20.5);
    s.add_fact("k", json!("second"), 0.5, 10.5);
    assert_eq!(s.get_fact("k").unwrap().value, json!("second"));
}

#[test]
fn tags_add_remove_and_report() {
    let mut s = AgentState::new("a1");
    s.add_tag("urgent");
    assert!(s.has_tag("urgent"));
    s.remove_tag("urgent");
    assert!(!s.has_tag("urgent"));
    assert!(s.tags().is_empty());
}

#[test]
fn counters_go_up_and_down() {
    let mut s = AgentState::new("a1");
    s.increment("visits", 3);
    s.decrement("visits", 1);
    assert_eq!(s.counter_value("visits"), 2);
    assert_eq!(s.counter_names(), vec!["visits".to_string()]);
}

#[test]
fn append_message_records_content_role_agent_time_and_metadata() {
    let mut s = AgentState::new("a1");
    s.append_message("hello", "agent", json!({"k": 1}), 20.5);
    assert_eq!(
        s.messages(),
        vec![
            json!({"content": "hello", "role": "agent", "agent_id": "a1", "timestamp": 20.5, "metadata": {"k": 1}})
        ]
    );
}

#[test]
fn the_message_log_keeps_only_the_newest_max_messages() {
    let mut s = AgentState::new("a1").with_max_messages(2);
    s.append_message("1", "agent", json!({}), 1.5);
    s.append_message("2", "agent", json!({}), 2.5);
    s.append_message("3", "agent", json!({}), 3.5);
    let contents: Vec<_> = s.messages().iter().map(|m| m["content"].clone()).collect();
    assert_eq!(contents, vec![json!("2"), json!("3")]);
}

#[test]
fn merge_takes_the_sorted_union_of_agent_ids_and_the_earlier_creation_time() {
    let a = AgentState::new("b").with_created_at(100.5);
    let b = AgentState::new("a").with_created_at(50.5);
    let m = a.merge(&b);
    assert_eq!(m.agent_id(), "a+b");
    assert_eq!(serde_json::to_value(&m).unwrap()["created_at"], json!(50.5));
}

#[test]
fn merge_leaves_both_inputs_unchanged() {
    let mut a = AgentState::new("a");
    a.add_fact("k", json!(1), 0.5, 1.5);
    let mut b = AgentState::new("b");
    b.add_tag("t");
    let before = (
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap(),
    );
    let _ = a.merge(&b);
    assert_eq!(
        (
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        ),
        before
    );
}

#[test]
fn merge_a_later_fact_wins_in_either_order() {
    let mut a = AgentState::new("a");
    a.add_fact("k", json!("x"), 0.5, 10.5);
    let mut b = AgentState::new("b");
    b.add_fact("k", json!("y"), 0.5, 11.5);
    assert_eq!(a.merge(&b).get_fact("k").unwrap().value, json!("y"));
    assert_eq!(b.merge(&a).get_fact("k").unwrap().value, json!("y"));
}

#[test]
fn merge_counters_take_the_larger_count_per_node_so_a_repeated_merge_adds_nothing() {
    let mut a = AgentState::new("a");
    a.increment("n", 3);
    let mut b: AgentState = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    b.increment("n", 2);
    let once = a.merge(&b);
    assert_eq!(once.counter_value("n"), 5);
    assert_eq!(once.merge(&b).counter_value("n"), 5);
}

#[test]
fn merge_an_observed_tag_removal_wins_over_the_tag_it_saw() {
    let mut a = AgentState::new("a");
    a.add_tag("t");
    let stale: AgentState = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    a.remove_tag("t");
    assert!(!a.merge(&stale).has_tag("t"));
    assert!(!stale.merge(&a).has_tag("t"));
}

#[test]
fn merge_the_same_message_on_both_sides_appears_once_sorted_by_time() {
    let mut a = AgentState::new("a");
    a.append_message("hello", "agent", json!({}), 20.5);
    let mut b = AgentState::new("b");
    b.append_message("reply", "agent", json!({}), 22.5);
    let dup: AgentState = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    let m = b.merge(&a).merge(&dup);
    let contents: Vec<_> = m.messages().iter().map(|x| x["content"].clone()).collect();
    assert_eq!(contents, vec![json!("hello"), json!("reply")]);
}

#[test]
fn merge_with_content_only_deduplication_ignores_agent_and_time() {
    let mut a = AgentState::new("a").with_dedup_strategy("content_only");
    a.append_message("same", "agent", json!({}), 1.5);
    let mut b = AgentState::new("b");
    b.append_message("same", "agent", json!({}), 2.5);
    assert_eq!(a.merge(&b).messages().len(), 1);
}

#[test]
fn the_wire_form_round_trips_and_has_the_agent_state_type() {
    let mut s = AgentState::new("a1").with_created_at(5.5);
    s.add_fact("k", json!({"n": [1, 2]}), 0.5, 10.5);
    s.add_tag("t");
    s.increment("c", 2);
    s.append_message("hi", "agent", json!({}), 3.5);
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["type"], json!("agent_state"));
    let back: AgentState = serde_json::from_value(v.clone()).unwrap();
    let again = serde_json::to_value(&back).unwrap();
    assert_eq!(
        emit_canonical(&crdt_merge::canonical::canonicalize(&again)),
        emit_canonical(&crdt_merge::canonical::canonicalize(&v))
    );
}

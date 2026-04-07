pub(crate) fn assert_event_order(calls: &[String], first: &str, second: &str) {
    let first_index = calls
        .iter()
        .position(|call| call == first)
        .unwrap_or_else(|| panic!("missing call: {first}"));
    let second_index = calls
        .iter()
        .position(|call| call == second)
        .unwrap_or_else(|| panic!("missing call: {second}"));

    assert!(
        first_index < second_index,
        "expected {first} before {second}, got {calls:?}"
    );
}

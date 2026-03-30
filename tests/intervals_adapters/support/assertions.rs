pub(crate) fn assert_valid_traceparent(traceparent: Option<&str>) {
    let traceparent = traceparent.expect("expected traceparent header to be present");
    let parts: Vec<_> = traceparent.split('-').collect();

    assert_eq!(
        parts.len(),
        4,
        "expected 4 traceparent parts, got {traceparent}"
    );
    assert_eq!(
        parts[0].len(),
        2,
        "expected 2-char version in {traceparent}"
    );
    assert_eq!(
        parts[1].len(),
        32,
        "expected 32-char trace id in {traceparent}"
    );
    assert_eq!(
        parts[2].len(),
        16,
        "expected 16-char parent id in {traceparent}"
    );
    assert_eq!(parts[3].len(), 2, "expected 2-char flags in {traceparent}");
    assert_ne!(parts[1], "00000000000000000000000000000000");
    assert_ne!(parts[2], "0000000000000000");
    assert!(parts
        .iter()
        .all(|part| part.chars().all(|ch| ch.is_ascii_hexdigit())));
}

use super::sanitize;

#[test]
fn sanitize_replaces_percent() {
    let out = sanitize("Any%");
    assert!(!out.contains('%'));
    assert_eq!(out, "Any％");
}

#[test]
fn sanitize_replaces_reserved_chars() {
    assert_eq!(sanitize("a:b/c"), "a-b-c");
}

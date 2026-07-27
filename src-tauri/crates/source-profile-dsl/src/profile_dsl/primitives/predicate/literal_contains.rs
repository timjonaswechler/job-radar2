/// Canonical literal containment used by Detection Strategy options without
/// admitting an additional Predicate registry identity.
pub fn literal_contains(value: &str, expected: &str) -> bool {
    value.contains(expected)
}

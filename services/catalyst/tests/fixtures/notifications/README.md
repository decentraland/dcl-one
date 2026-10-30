Copied from catalyrst/crates/catalyrst-conformance/fixtures/notifications/*.json (2026-09-25).
Body-shape fixtures of the notifications service (no request side, no volatile_paths):
tests/social_notifications_fixtures.rs replays each against the in-process router as
`serve` mounts it (nested under /notifications) and compares the JSON shape (key set per
object, first element of an array; scalars loosely) plus the values that are not volatile.

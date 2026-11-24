//! The review flow as creators, curators and the validator meet it, through
//! the router as `serve` mounts it: with `builder-auto-curation-review-flow`
//! on, a review's timeline records its starts, verdicts, appeals and
//! comments; the validator's verdict rejects or reopens the review it was
//! asked about; creators retry, appeal and comment; with the flag off the
//! four routes answer as absent and nothing changes. Each test has a
//! database of its own and drives the node's clock.

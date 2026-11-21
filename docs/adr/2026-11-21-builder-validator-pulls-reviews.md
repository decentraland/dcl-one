# ADR: The wearable validator pulls its reviews from the builder

- Status: Accepted
- Date: 2026-11-21

## Context

The wearable validator renders each item of a standard collection and
reviews it, then reports a verdict. It runs as a job of its own that starts
when there is work and stops when there is none, with no listening endpoint.
It needs, for each review, the items' entity metadata and files, and it must
be the only party whose verdict the builder accepts.

A review is requested when a standard collection is first published,
unapproved on chain, and when its creator submits changes for review on a
published standard collection. About one review a day is requested, at most
28 in one day; a published standard collection holds a median of one item.

Unless the review flow is on, the verdict is advisory: it doesn't approve or
reject a curation. Curators decide, and the validator posts its own findings
to their channel.

## Decision

- **A request is a row.** The builder records a validation request
  (`builder.validation_requests`, the item ids) in the transaction that
  publishes or submits, so a request can't be lost and a refused publication
  records none. A collection has at most one open request; a newer one
  supersedes it. A collection gets at most five requests a UTC day and none
  for more than 50 items; the curators are told when a collection gets no
  automated review.
- **The validator pulls.** It claims the oldest claimable request
  (`POST /v1/validator/claims`, `FOR UPDATE SKIP LOCKED`), which answers
  the validation manifest built from the items as saved at that moment, a
  claim id and a 30-minute lease. It extends the lease
  (`POST /v1/validator/claims/{validationId}/lease`) up to two hours after
  the claim, and posts its verdict with the claim id
  (`POST /v1/collections/{id}/validation-result`). Only the current claim's
  verdict settles the request. A lapsed lease makes the request claimable
  again; after its third claim lapses the request is abandoned. A request
  unsettled for seven days expires. Both tell the curators. The leased job
  `builder-validation-requests` abandons, expires and deletes settled
  requests after 30 days.
- **One secret signs every call.** The validator signs each call with
  `WEARABLE_VALIDATOR_CALLBACK_SECRET`: an HMAC-SHA256 over the timestamp,
  the method, the path it requested and the raw body, within five minutes of
  the builder's clock, compared in constant time. A signature seen within its
  window is refused (`builder.validator_seen_signatures`). Each source gets
  60 calls a minute across the three routes, and each route caps its body.
  Nothing is read from the database before the signature verifies.
- **Files are already public.** The validator reads item files from the
  builder's unsigned storage route. A request is made only for a published
  standard collection, whose files the builder's file rule makes public, and
  a claim names only the hashes its items name when claimed. The validator
  gets no reader of private files.
- **The verdict is advisory unless the review flow is on.** It settles its
  request and is logged; it writes no curation, event or model text. With
  the review flow on, it also acts within its claim
  ([ADR](2026-11-21-builder-review-flow.md)).
- **Off unless configured.** Nothing is recorded, and the validator's routes
  refuse as an unsigned call, unless the secret is set and the `builder`
  flags document, with this node's overrides, has `builder-auto-curation`
  on. With creator writes off, the three routes answer 503 and the job
  writes nothing.

## Consequences

- The builder calls out to nobody for a review but Slack. The validator sets
  its own pace and concurrency; a burst of publications is a burst of rows.
- A validator that is down or never deployed blocks nothing: its requests
  expire after seven days, each with a notice that the collection needs a
  curator review without it.
- The manifest reflects the items as saved when claimed, so an edit between
  request and claim is reviewed as saved. An edit after the claim makes the
  replaced file private and the verdict's content hash stale; the builder
  logs the stale items.
- Rotating the secret costs at most one lease: the validator's calls are
  refused until both sides hold the new secret, and its request is claimed
  again once the lease lapses.
- The review flow (a verdict acting on a curation, the creator's review
  timeline, retries and appeals) builds on the same request table
  ([ADR](2026-11-21-builder-review-flow.md)).

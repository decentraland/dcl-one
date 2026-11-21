# ADR: The builder's review flow lets the validator decide within its claim

- Status: Accepted
- Date: 2026-11-21

## Context

A standard collection's review is shared by its creator, the curators and
the wearable validator. The `/create` dApp shows creators and curators a
review timeline: each automated review's start and verdict, the creator's
appeals, submitted changes and comments. It lets a creator ask for another
automated review, appeal a rejection to a curator and write to the curators.
It calls the four routes this needs (`POST /v1/collections/{id}/validations`,
`POST /v1/collections/{id}/curation/appeal`,
`GET /v1/collections/{id}/events` and
`POST /v1/collections/{id}/comments`) only while the `builder` flags document
has `builder-auto-curation-review-flow` on.

The validator's verdict on a request reaches the builder over a signed call
bound to the claim it answers
([ADR](2026-11-21-builder-validator-pulls-reviews.md)). Whoever holds the
validator's secret can post verdicts, so what a verdict may change has to be
bounded by the request it answers.

## Decision

- **One switch.** The review flow is on only while the automated review is
  (the validator's secret is set and `builder-auto-curation` is on) and the
  `builder` flags document, with this node's overrides, holds
  `builder-auto-curation-review-flow` as the boolean `true`. Off, the four
  routes answer `404 Not found` after the signature, before any other read,
  and change nothing; a verdict only settles its request.
- **The timeline is the record.** With the flow on, recording a validation
  request also records its start on the collection's timeline
  (`builder.collection_events`), and a first publication records the
  publication. A verdict records its event once per validation. A request
  abandoned after three lapsed claims, or expired after seven days, records
  that the automated review gave up. A third-party collection's publication
  records once that a curator reviews it by hand.
- **The validator is a curation actor with one move each way.** The
  transition table that every curation write asks has the validator as an
  actor: its rejection rejects the pending review or opens a rejected one;
  its pass reopens only its own rejection; it can't approve and can't touch a
  decided review. A verdict acts only from its request's current claim, on
  the collection that request names, in the transaction that settles the
  request, and only if no curator decided since the request started. The
  review records `reviewed_by = 'validator'`, which no wallet address can
  equal, and each such write goes to the operator audit as the validator.
- **Creators act on their own collection.** A retry and an appeal need a role
  on the collection (its owner, an on-chain manager or its provider's
  manager); a comment also admits the committee. A retry is refused while a
  review runs, after five requests in the UTC day, after a curator's
  decision, and when nothing changed since the last verdict.
- **Creator text is bounded.** A comment and an appeal note are at most 2,000
  characters, trimmed. A collection takes at most 200 comments and five
  appeals in a UTC day, and a wallet writes at most 200 comments and 20
  appeals in a UTC day across collections; beyond, the route answers `429`
  with when the next day starts. Text is stored as text and escaped on one
  line in the curators' Slack notices.
- **The timeline reads like the collection.** Its owner, managers and the
  committee read it, newest first, at most 200 events a page; creators read
  it without the validator's ids and failures. Anyone else gets the answer
  an absent collection gets.

## Consequences

- The most the holder of the validator's secret can do is reject the pending
  review, or open a rejected one, of a collection that has an open
  validation request no curator decided since, and reopen its own
  rejections. It can't approve, can't act on a collection it wasn't asked
  about, can't overrule a curator and can't create requests.
- The dApp's review screens work against the node unchanged.
- The committee listing doesn't yet tell collections in an automated review
  apart from the ones waiting for a curator.

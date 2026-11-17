# ADR: Shop builds without Social and Comms

- Status: Accepted
- Date: 2026-11-16

## Context

After an edit to Content, the node's crates compile in one serial chain:
`content → explore → comms → social → shop → catalyst`. Shop is the largest unit on it and
waited for Social and Comms, although it used little of either: the notification writer and
its email sender, the profile-image renderer, the per-key rate limiter, and the Admin Toolkit
name-owner trait it implements.

## Decision

Shop depends on neither `catalyst-social` nor `catalyst-comms`.

- **One notification writer, in `shared::notifications`.** `NewNotification`,
  `insert_notifications`, `insert_keyed_notification`, the email delivery-row SQL, `SendFailure`,
  `format_mana` and the icon CDN helpers live there. Every producer, Social and Shop alike,
  writes `social.notifications` and its delivery rows through it, inside its own transaction.
  Social still owns the tables, reads them, delivers the email and keeps the templates, and
  re-exports these items from `social::notifications`.
- **One list of templated types.** `shared::with_emailable_kinds!` names every notification
  type with an email template. The writer's `EMAILABLE_TYPES` and Social's template sources are
  both generated from it, so a type cannot gain a delivery row without a template, or the
  reverse.
- **The rate limiter is `shared::limits`.** Comms re-exports it as `comms::limits`.
- **`serve` connects the rest** (`src/serve_adapters.rs`). Shop owns two small traits:
  `subscriptions::AvailabilityMailer` (one `send_html`, with the item-available email built on
  it) and `market::ProfileFaces`. `serve` implements them over Social's SendGrid sender and
  `/profile-images`. `MarketplaceState::name_owner` is an inherent method, and `serve` hands it
  to Comms as its `NameOwnerReader`.

No crate, process, pool, table, route or wire format is added or changed.

## Consequences

Shop compiles alongside Comms and Social, so an edit to Social or Comms no longer recompiles
Shop. Debug rebuilds after touching one file (18 cores, no compiler cache, incremental off), three
interleaved runs each:

| Edited crate | Before (median) | After (median) |
|---|---|---|
| `content` | 62 s | 49 s |
| `comms` | 48 s | 33 s |
| `social` | 40 s | 22 s |

With incremental compilation on, the rebuild after a `content` edit went from 15.1 s to
12.7 s, and after an `explore` edit from 13.0 s to 10.6 s (medians of three interleaved runs).

A new notification producer uses `shared::notifications`, and a new email template adds its
type to `with_emailable_kinds!`.

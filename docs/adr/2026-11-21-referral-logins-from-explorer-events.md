# ADR: Referral logins also come from verified explorer events

- Status: Accepted
- Date: 2026-11-21

## Context

A referral is granted after the invitee logs in on a configured number of distinct UTC days. The node recorded those logins only from the signed `POST /social-api/v1/client-login`. The Unity explorer and the mobile client do not call that route; they report `logged_in` and `logged_in_cached` to Segment, which the node already receives on `POST /events-notifier/forward` ([ADR](2026-11-07-segment-webhooks-replace-the-telemetry-sink.md)). social-service-ea granted referrals from those same events (`logged-in-handler.ts`), so without them the invitees of unchanged clients never complete and their referrers stop progressing.

## Decision

A `logged_in` or `logged_in_cached` event whose auth chain verifies counts as a login of the chain owner.

- Verified means the `[SIGNER, ECDSA_EPHEMERAL]` login chain checks out cryptographically for the event's `dcl_eth_address`: the owner's signature, the `Decentraland Login` delegation and its expiry. Any other event is refused before any write.
- The webhook calls the same login recording as `client-login`: login days, the 120-per-day receipt quota, the ban check, the grant, the accepted notification and the reward tier. There is no second implementation.
- The login ID is a UUID v5 of the Segment `messageId`, so a redelivery is a duplicate. The occurrence is Segment's `receivedAt`.
- Recording runs before the event row. A transient failure answers 503 and Segment redelivers. A refused login (stale or over quota) is logged and the event is stored.
- `POST /v1/client-login` is unchanged.

## Consequences

Referrals from unchanged clients progress as they did under social-service-ea. The referral badge and referral notifications follow, because both read the grant this path writes.

The chain proves only that its owner delegated to a key. Nothing in the event is signed, so anyone holding an unexpired chain can send login events in the owner's name. Such events can add login days to an invitee whose referral is still pending. The receipt quota, the one-day-per-UTC-date rule, the ban check and the delegation's expiry limit this. It is the same trust the CP7 checkpoint already places in these events.

Legacy queued logins and Redis login days are not imported. `tooling/migration/sources/social-service-ea/MAPPING.md` lists the operator steps at cutover.

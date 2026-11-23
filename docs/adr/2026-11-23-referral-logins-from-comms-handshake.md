# ADR: Referral logins from the comms handshake

- Status: Accepted
- Date: 2026-11-23
- Status changed: 2026-11-23
- Decided by: owner, 2026-11-23.
- Amends: [Native social events](2026-11-04-native-social-events.md), for clients that do not call `client-login`.
- Decision: A completed `/ws` challenge handshake counts as a login of its wallet for referral progress.

## Context

A referral is granted after the invitee logs in on a configured number of distinct UTC days. The [native social events decision](2026-11-04-native-social-events.md) records logins only from `POST /social-api/v1/client-login`. That route takes ADR-44 signed metadata (`loginId`, `occurredAt`), so a login cannot be reported without the signer's ephemeral key. The Unity explorer and the mobile client do not call it. Without another source, their invitees never complete a referral.

These clients do report `logged_in` and `logged_in_cached` to Segment, and the node receives those events. They are not used for credit. Such an event carries the login delegation (`[SIGNER, ECDSA_EPHEMERAL]`), and nothing in the event is signed. The same delegation travels with every signed fetch the client makes, including requests to third-party servers. A valid chain therefore proves that its owner once delegated a key. It does not prove that the owner sent this event or logged in on this day.

Both clients open the archipelago connector socket (`/ws`) when a realm advertises an `archipelago:` adapter, as the main realm does. That handshake verifies a signature over a challenge the server has just issued (`comms::archipelago::auth`):

- the challenge is random and used once;
- the chain's last link must sign exactly that challenge;
- the chain is verified with its expiry at the time of the handshake.

## Decision

When a wallet completes the `/ws` handshake, comms credits one login of that wallet through `interfaces::LoginRecorder`. Social implements it with the same recording as `client-login`: login days, receipts and their quota, the ban check, the grant, the accepted notification and the reward tier.

- The login ID is a UUID v5 of the handshake's challenge, so it is unique per handshake. The occurrence is the server's time. One login day per UTC date still applies.
- The credit runs on a spawned task after the welcome. It never delays or fails the handshake.
  - Concurrency and backlog are bounded; past the backlog, a credit is dropped.
  - Each attempt has a timeout.
  - A transient failure is retried with the same login ID. A refused login is logged.
- `serve` builds one `Referral` and hands it to both comms and social. Comms depends only on the interface.
- Only the primary `serve` runs `/ws`, so only the primary credits.
- Reconnects count as logins here. This changes the earlier rule that room admissions are not logins, but only for referral days: several handshakes on one date are still one day.
- `POST /v1/client-login` is unchanged.

## Consequences

Referrals progress for invitees whose client joins a realm through `/ws`. The referral badge and referral notifications follow, because both read the grant this path writes.

Some sessions never complete the `/ws` handshake, so they are not credited:

- a session that only visits worlds, where clients get their room from the gatekeeper's signed-fetch `/get-scene-adapter`. That request is timestamp-bound, not bound to a challenge the server issued;
- a mobile client whose archipelago connection is turned off by its feature flag.

Those invitees need `client-login` or a later login on a realm that uses `/ws`.

Legacy queued logins and Redis login days are not imported. `tooling/migration/sources/social-service-ea/MAPPING.md` gives the operator steps at cutover and their order.

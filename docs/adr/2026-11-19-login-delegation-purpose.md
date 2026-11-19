# ADR: Login auth chains require the `Decentraland Login` delegation

- Status: Accepted
- Date: 2026-11-19

## Context

An ephemeral delegation's payload is three lines: a purpose, `Ephemeral address: <address>` and `Expiration: <date>`. Every in-tree client that signs a login writes `Decentraland Login` as the first line: the auth site, the web shell, the Explorer guest, Creator Hub, the scene CLI and the parity tooling. Other purposes exist, such as the authoritative storage delegation, which travels in its own header and is verified by its own parser.

## Decision

`dcl-auth` verifies a chain presented as a login with `verify_login_chain`, `verify_login_chain_async` or `verify_login_chain_at_verification_time`. These refuse any ephemeral link, ECDSA or EIP-1654, whose first line (carriage returns removed) is not exactly `Decentraland Login`. The refusal is `AuthError::InvalidEphemeralPayload`, a settled verdict that no other payload or path retries.

Login surfaces are every signed fetch (`dcl_auth::signed_fetch::verify_signature` and the comms signed-fetch verifier), the Pulse handshake, the archipelago challenge, the worlds storage routes, the auth API's requests and identities, the Segment webhook identity check, the scene CLI's preview comms and the sites Creator Hub draft routes, whose TypeScript verifier (`verifyAuthChainRequest`) applies the same first-line rule and reads the address and expiration from lines 2 and 3. A chain with no ephemeral link, such as a wallet signing directly, carries no purpose and is unaffected.

`verify_auth_chain`, `verify_auth_chain_async`, `verify_peer_auth_chain` and `verify_synced_auth_chain` accept any purpose. They serve entity deployments, synced and mirrored entities, and the `/crypto/validate-signature` route, so historical entities keep verifying.

## Consequences

- A delegation signed for another purpose cannot authenticate a request, a socket or a session.
- A client that signs a login with another first line is refused until it signs `Decentraland Login`.
- The payload may still carry more than three lines; a line-count rule is a separate choice.

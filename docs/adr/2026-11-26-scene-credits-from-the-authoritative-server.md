# ADR: Scenes pay Credits only through their authoritative headless server

- Status: Accepted
- Date: 2026-11-26

## Context

`~system/SceneCredits.grantCredits` (protocol `scene_credits.proto`) lets a scene pay a player Credits out of a program that Decentraland set up for a studio and pinned to a place: a world and a base parcel, paid from by the scene that one of the studio's named deployer wallets deployed there. Credits are money. Whoever can call credits-server's payment route as a scene spends that program's budget.

A scene's code is public and runs on every client. Nothing in it, or in its Scene Storage, can prove which scene is asking. Authoritative scenes run in this engine's headless server, orchestrated by sdk-multiplayer-server, which already mints a per-scene delegation for World storage.

## Decision

- **Who pays.** Only an orchestrated headless server pays. Every other host (desktop, web, single-scene headless) answers `GCE_NOT_AVAILABLE` and never calls credits-server.
- **The claim.** The orchestrator mints an `Audience: credits` claim for each scene assignment, signed by its root key. The claim names:
  - the world, scene entity id and base parcel;
  - its own ephemeral address;
  - an expiry at most 15 minutes away.
- **The assignment.** The claim travels in `add-scene` with an assignment id. The engine asks for a renewal 5 minutes before expiry, naming that assignment. A renewal never opens a slot or moves one to another scene.
- **The signed request.** The engine signs each request itself with the claim's ephemeral key: a two-link signed-fetch chain over `POST /scene-rewards/programs/:programId/grants`. The SHA-256 of the body goes into the signed metadata, and the claim travels in `x-authoritative-scope`.
- **What the scene sees.** Scene code never sees the claim or the key, and the engine never signs a scene's own fetch with them.
- **A separate key.** Credits gets its own ephemeral key, never World storage's. A signed-fetch signature does not cover the host, so with a shared key a request captured at one service could be replayed at the other wherever the paths overlap. The envelope decoding, the delegated signer and the renewal system are shared.
- **Presence.** The engine reports it and credits-server decides. The body carries `recipientConnected`, which is true only when the player is in this scene's crdt context, on this scene's room transport, and that room is connected. A new payment needs it; a retry of an outcome already paid returns its receipt without it.
- **What the scene names.** Only the program, the recipient and its own outcome id. Amounts, budgets, eligibility and limits belong to the program in credits-server.
- **Confirming a payment.** An answer counts as paid only with its whole receipt. Anything ambiguous is `GCE_UNAVAILABLE`: a lost connection, a gateway page, a partial body or a request dropped with its scene. The scene then retries with the same program, recipient and outcome id. If the first call committed, credits-server returns its receipt to the scene that paid it, or to a later deployment at the program's place once it checks that deployment as the program's own; otherwise the retry is judged as a new payment.
- **Pending requests.** They are entities despawned with their scene (`DespawnWith`), at most 4 per scene. More are refused with `GCE_RATE_LIMITED`, not queued.

## Alternatives rejected

- **A key in the scene's code or Scene Storage, used by the scene's own signed fetch.** Anyone who can read the key could pay any wallet from anywhere, and credits-server could not tell which scene asked.
- **Reusing World storage's ephemeral key.** A request could be replayed across the two services, as described above.
- **Refusing in the engine when the recipient is not connected.** A retry after a lost answer would become a refusal that the scene cannot tell from "not paid".
- **Queuing requests over the per-scene cap.** A busy scene would pile payments up behind a slow server.
- **Naming the module `SceneRewards`.** "Rewards" already names the rewards server, which grants wearables and emotes. This module only pays Credits.

## Consequences

- **Turning payments off** needs no engine release. To stop new payments at once, credits-server switches issuance off: it refuses every new payment and still answers receipts already paid. Turning off minting (`CREDITS_DELEGATIONS_ENABLED`, read when the orchestrator starts) stops new claims, but each claim already issued stays usable until it expires.
- **Rotating the root key.** A claim signed by a root that credits-server no longer trusts is refused until the orchestrator mints a new one.
- **Redeploys.** A redeploy changes the scene entity id, and the claim names the new one. credits-server checks that it is the scene active at the program's place and that one of the program's deployers deployed it, so a redeploy by them over the approved footprint keeps paying with no change to the program. Once a deploy by any other wallet replaces it, requests observed after that are refused; one observed just before can still be paid within the evidence age, as the check promises only that the scene was active when it was observed.
- **Explorer clients** (this engine's desktop and web builds, the Unity explorer and the mobile explorer) ship a module that always answers `GCE_NOT_AVAILABLE`, so a scene that imports `~system/SceneCredits` still loads.

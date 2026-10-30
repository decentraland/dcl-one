# dcl-one

`README.md` covers what this is and how to build, run and test it.

## Agent work and commits

- Keep accepted architecture in `docs/adr/` and reusable instructions in this tree.
- Keep plans, briefs, handoffs, task reports and review evidence in ignored `.agent-work/`; follow `services/catalyst/docs/comms/README.md` for specialist reporting. `services/catalyst/docs/comms/plans/` and `.superpowers/` stay local too.
- Before committing, capture accepted public architectural choices in ADRs and check the staged paths for working documents and sensitive data.
- Never commit working documents unless the user asks.
- Tracked documentation must stand alone without local plans or private ledger exports.
- Make focused edits; preserve other agents' work.

## Agentic roles documentation

- README `Agentic roles` lists every agent role: name, goal, main functions, relative link to its tracked definition or workflow.
- Adding, renaming, changing or removing a role updates that table in the same change, verified against the definitions.
- Describe local-only roles only from tracked workflows; never link ignored files.

## Scope

Code that serves none of these deliverables is removed, not kept for reference:

1. landing page;
2. What's On and discovery (places, events, map);
3. `/play`: Explorer (`rendering/bevy-explorer`), HUD (`apps/ui3/src/play`), desktop packaging (`apps/bevy-explorer`);
4. `/shop`: Marketplace and Store merged;
5. `/create`: Creator Hub, web and `apps/creator-hub`;
6. `apps/dcl-scene-cli`: the `@dcl/sdk-commands` rewrite;
7. `/docs`: the docs.decentraland.org rewrite;
8. page-coherence scripts (`apps/sites/scripts`);
9. one backend schema (`schemas/`);
10. the `catalyst` binary: every backend area behind one base domain;
11. the protocol (`library/protocol`): the `.proto` contract and the `@dcl/protocol` package.

- Docs state current behavior, verified capabilities and next work; no transition histories or internal generation labels.
- Compare compatibility only against pinned `github.com/decentraland` repositories or dated Decentraland server captures.
- A recorded difference is not approval; identify unresolved gaps and their evidence in the review report.
- Check the current readiness evidence before claiming deployment or compatibility; missing evidence means the claim remains unverified.
- Write PR descriptions per `.github/PULL_REQUEST_TEMPLATE.md`.
- For schema, SQL, ingestion, mirrors or caches, apply `DATABASE.md`; state workload and invariants before choosing tables or indexes.
- Record measured performance changes in the task report; follow-ups are candidates until measured.
- Preserve wire, authorization and crash/resume contracts.
- Never reset an existing database to fit a changed schema.

## Code structure

- Read the [layout ADR](docs/adr/2026-10-19-organizing-library-services-folders.md) before adding, moving or renaming a folder or crate; record layout changes as a new ADR.
- `library/` depends only on library code.
- `library/protocol/proto` is the only `.proto` source ([ADR](docs/adr/2026-10-25-protocol-in-dcl-one.md)); `tooling/proto-source-check.py` allowlists the copies still to be removed.
- Reach other files through `env!("DCL_ONE_ROOT")`, never `../` chains.
- Domain crates follow `docs/module-layers.toml`; no upward dependency.
- Keep the fresh-schema bootstrap single.
- Put helpers other crates' tests use behind a `test-support` feature.
- ops, presence, serve and main stay in the root crate.

## Ownership

- `explore`: discovery and explorer-facing APIs (places, events, map, realms, flags, client config), including durable world-scene place identities maintained inside deployment transactions; `places::RemovedPlaces` captures catalogue removals, which comms consumes for moderation and ingress cleanup.
- `worlds`: world catalogues, deployments, permissions, world/player storage, encrypted env values, SDK7 scene hosting with per-scene isolation, limits and cancellation.
- Guest code reaches storage through a scoped host API, never the shared pool.
- `comms` (layer 8): Pulse, the archipelago connector (`/ws`), the gatekeeper (admission, moderation, voice, streaming, LiveKit webhook, room events) and presence; all but Pulse run in `catalyst serve`.
  - Worlds supplies access and scene identity via `comms::interfaces`; social supplies membership, privacy and notification persistence via `SocialDirectory` and keeps MLS.
  - Comms owns streaming lifecycle and cleanup intent; social owns notification delivery records.
  - Comms enforces access through local calls and its own SQL: owner/player/host scopes, one room resolver, one LiveKit/WebSocket transport; LiveKit media stays external.
  - Pulse keeps assignments and unfinished takeovers in process memory, persists nothing, admits only from its room-authority response and imports nothing from the gatekeeper.
  - Only the gatekeeper holds the LiveKit key, mints/verifies the relay admission proof and applies live room policy; it owns cleanup receipts and acknowledges retained takeovers.
  - Pulse runs in its own process unless a load measurement shows the loop is unaffected in-process.
  - The connector is part of the gatekeeper (local calls); Pulse and `serve` share the NATS subjects in `comms::protocol::bus`.
  - Only the primary `serve` (`FLEET_JOBS_ENABLED`) serves `/ws` and the island subscriber.
  - Social replicas derive online status from Pulse's wallet topology over NATS; every `serve` recovers room occupancy through the gatekeeper's LiveKit adapter; presence owns observations, source age and public/admission projections.
- `storage`: the shared hash-addressed blob store.
- Domain ownership is not a separate deployable; keep the shared SQL and process contracts.

- Prefer the smallest readable implementation: no forwarding layers, internal HTTP hops or per-service pools; direct SQL and a passed transaction for atomic writes.
- A new crate needs an independent consumer or a concrete build requirement.
- Preserve external API paths and room names when code moves.
- Shop absorbs every Marketplace feature; its UI lives in `apps/ui3/src/shop`.
- Credits are the main payment option; logged-in web3 wallets also see MANA.
- LAND and every secondary sale are MANA-only, with guidance for web2 users.
- Enforce eligibility in backend and settlement, including legacy routes; hiding a button is insufficient.

## Self-contained, public tree

- Never add a dependency on a sibling checkout.
- `rendering/bevy-explorer/source` is its own workspace; integration lives in `apps/bevy-explorer`.
- Keep component licenses and pins (`upstream.lock.json`, `abgen-release.lock`, `livekit-release.lock`).
- Regenerate `.github/CODEOWNERS` from `ownership.json` with `tooling/owners.py`.
- No machine names, private addresses, home paths, personal emails or the operator's domain; deployments supply them via gitignored config.
- Never delete the gitignored `credentials` and `data` directories.
- `/docs` data is `apps/sites/content/docs` (`pnpm gen:docs-content`); media is Git LFS.

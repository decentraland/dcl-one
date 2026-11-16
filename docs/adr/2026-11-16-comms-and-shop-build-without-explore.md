# ADR: Comms and Shop build without Explore

- Status: Accepted
- Date: 2026-11-16

## Context

With Shop no longer waiting for Social and Comms
([ADR](2026-11-16-shop-builds-without-social-and-comms.md)), an edit to Content still
compiled `content → explore → comms → social → catalyst` in series, and Shop, the largest
unit, still started only once Explore's metadata was ready. Both Comms and Shop used little
of Explore:

- Comms: the place-removal feed reader (`places::removed`, pure SQL over Explore's outbox,
  called only by Comms), `sanitize::strip_all_markup`, and an `impl places::live::Participants`.
- Shop: the feature-flag document type (`api::feature_flags`) and one Registry CORS constant.

## Decision

Neither Comms nor Shop depends on `catalyst-explore`.

- **Comms owns the removal-feed reader** (`comms::gatekeeper::removed_places`). It consumes
  `explore.place_removal_pending` and `explore.place_removal_state` by shared SQL; Explore's
  trigger stays the only writer of new rows. The edge and its columns are recorded in
  `AGENTS.md` and `DATABASE.md` "Place removal feed".
- **`shared::markup`** holds the rich-text tag scanner and `strip_all_markup`; Explore
  re-exports them.
- **`serve` connects live participants**: `CommsState::live_participants` is an inherent
  method, and `src/serve_adapters.rs` hands it to Explore's `Participants` trait.
- **`shared::feature_flags`** holds the flag document (`Snapshot`, validation, local
  overrides on `explore_api.feature_flags_local`, the document fetch). Explore keeps the
  routes, the mirror and the refresh, and re-exports the vocabulary. The Registry CORS rule is
  `shared::cors::registry_cors`.

No crate, process, pool, table, route or wire format is added or changed.

## Consequences

Comms and Shop start as soon as Content's metadata is ready, and an edit to Explore no
longer recompiles Comms or Shop. Debug rebuilds after touching one file (18 cores, no
compiler cache), medians of three interleaved runs, against the Shop decoupling. An
`explore` edit also drops from 109 to 60 CPU-seconds with incremental off:

| Edit | Incremental | Before (median) | After (median) |
|---|---|---|---|
| `content` | off | 52.2 s | 45.2 s |
| `content` | on | 12.7 s | 11.0 s |
| `explore` | off | 45.5 s | 32.6 s |
| `explore` | on | 10.6 s | 7.9 s |

Shop is the longest unit on the chain again. A change to the removal-feed columns updates
Comms' reader and the documented edge; a new Explore trait that higher crates need is
implemented in `serve`, not by linking Explore.

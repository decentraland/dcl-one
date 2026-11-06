# Content server: sync, history, status and queries

Area: the content-server routes that peers sync from and that clients use to read history,
status and item media: `/deployments`, `/pointer-changes`, `/failed-deployments`, `/snapshots`,
`/status`, `/challenge`, `/queries/items/*`, `/queries/erc721/*`.

Pinned revisions:

- Upstream content server: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21). It serves no route in this area.
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04), Rust workspace `services/catalyst`.

Everything below is from reading the code, except where a line cites a capture under
[`captures/content-sync/`](captures/content-sync/), which states what the production peer answered on
2026-11-04 ([`captures/README.md`](captures/README.md)). Nothing was built or run, and no line is
"verified". Where an upstream behaviour depends on a package that is not in the clone
(`@dcl/http-server`, `@dcl/schemas`, `@dcl/snapshots-fetcher`, `qs`), the entry says so.

Files read:

- Upstream catalyst:
  - `src/controllers/routes.ts`
  - `src/controllers/middlewares.ts`
  - `src/controllers/errors.ts`
  - `src/controllers/utils.ts`
  - `src/controllers/handlers/{get-deployments,pointer-changes,failed-deployments,get-snapshots,status,get-challenge,get-entity-thumbnail,get-entity-image,get-erc721-entity}-handler.ts`
  - `src/logic/deployments/{component,types}.ts`
  - `src/logic/query-params/{component,types}.ts`
  - `src/logic/snapshots/{component,types}.ts`
  - `src/logic/time-range.ts`
  - `src/logic/status-checks.ts`
  - `src/logic/peer-cluster/{component,types}.ts`
  - `src/logic/entities/{component,utils}.ts`
  - `src/logic/active-entities/component.ts` (`withPointers`, `findEntities`)
  - `src/adapters/deployments-repository/{component,types}.ts`
  - `src/adapters/snapshots-repository/component.ts`
  - `src/adapters/failed-deployments/{component,types}.ts`
  - `src/adapters/content-file-writer/component.ts`
  - `src/deployment-types.ts`
  - `src/types.ts` (`parseEntityType`, `DeploymentField`)
  - `src/Environment.ts` (`CURRENT_VERSION`, `CURRENT_COMMIT_HASH`)
  - `src/migrations/scripts/1589393477829_initial.ts` (column types)
- Upstream lamb2: `src/controllers/routes.ts` (no `/collections/contents/*` route at this revision).
- dcl-one (`services/catalyst/`):
  - `content/src/lib.rs` (`content_routes`, `build_router`, `SyncStatus`)
  - `content/src/deployments.rs`
  - `content/src/pointer_changes.rs`
  - `content/src/failed_deployments.rs`
  - `content/src/snapshots.rs`
  - `content/src/snapshot_gen.rs`
  - `content/src/status.rs`
  - `content/src/time_range.rs`
  - `content/src/entity_thumbnail.rs`
  - `content/src/formatters.rs`
  - `content/src/queries.rs`
  - `content/src/wire_types.rs`
  - `content/src/errors.rs`
  - `content/src/validator/erc721.rs`
  - `content/src/sync/pointer_changes.rs` (`keyset_next_url`)
  - `content/src/sync_deployer.rs` (failed-deployment writes)
  - `shared/src/query_params.rs`
  - `storage/src/denylist.rs`
  - `../../schemas/00_content.sql` (`content.snapshots`, `content.failed_deployments`)
  - `tests/fixtures/content/deployments-400-*.json`

Common notes:

- **Mount points.** In production, upstream serves the content server under `/content`. dcl-one
  mounts the same routes at the root and under `/content` (`services/catalyst/content/src/lib.rs:710-711`).
- **Error envelope.** Both trees use `{"error": "<message>"}`:
  - upstream `decentraland/catalyst:src/controllers/middlewares.ts:41-82`;
  - dcl-one `services/catalyst/content/src/errors.rs` (`json_error`).
  - Both map an unexpected error to 500 `{"error":"Internal Server Error"}`.
  - dcl-one's `AppError::ServiceUnavailable` renders the bare message, with no `Retry-After` header.
- **Unknown paths.** dcl-one's fallback is 404 with the `text/plain` body `Not found`
  (`services/catalyst/content/src/lib.rs:623-625`). The upstream 404 body comes from
  `@dcl/http-server`, which is not in the clone.
- **Trailing slash and path case.** Upstream's router (the `@dcl/http-server` koa-router
  derivative) routes `/content/deployments/` and `/content/Deployments` as `/content/deployments`
  (captured, CS-23). dcl-one trims a trailing slash for the whole app
  (`services/catalyst/src/serve.rs`, `NormalizePath`) and spells the static segments of a content
  route under `/content/` in the route's case before routing
  (`services/catalyst/content/src/routing.rs` `upstream_routing`). The root mounts stay
  case-sensitive; upstream production serves none.
- **Rate limits.** Upstream rate-limits only `GET`/`HEAD /contents/:hashId` and `POST /entities`
  (`decentraland/catalyst:src/controllers/routes.ts:101-114`), so no endpoint in this area has a
  rate limit upstream. dcl-one's app has none either, but this tree's gateway limits every path by
  client IP: `/content/*` with `limit_req zone=content` (100 r/s, burst 1000, `nodelay`) and the root
  mounts with the server-wide `zone=read` (30 r/s, burst 60), answering nginx's HTML 429
  (`infra/gateway/gateway.py:166,257-260,292`). A peer or client that reads `/content/deployments`,
  `/content/pointer-changes` or `/content/snapshots` faster than that gets a 429 that upstream never
  sends; there is no exemption for sync peers (CR-4 in `content-reads.md`).
- **Bootstrapping 503.** Upstream's `preventExecutionIfBoostrapping` guards only `POST /entities`
  (`decentraland/catalyst:src/controllers/routes.ts:62`), so no endpoint in this area returns the
  bootstrapping 503.
- **Read-only mode.** Read-only mode does not affect these routes.

## Endpoint inventory

| # | method | upstream path | prod prefix | dcl-one route(s) | status |
|---|---|---|---|---|---|
| 1 | GET | `/deployments` | `/content` | `/deployments`, `/content/deployments` → `services/catalyst/content/src/deployments.rs:37` (route `services/catalyst/content/src/lib.rs:652`) | DIVERGENT |
| 2 | GET | `/pointer-changes` | `/content` | `services/catalyst/content/src/pointer_changes.rs:23` (route `lib.rs:657-660`) | DIVERGENT |
| 3 | GET | `/failed-deployments` | `/content` | `services/catalyst/content/src/failed_deployments.rs:14` (route `lib.rs:653-656`) | DIVERGENT |
| 4 | GET | `/snapshots` | `/content` | `services/catalyst/content/src/snapshots.rs:77` (route `lib.rs:661`) | DIVERGENT |
| 5 | GET | `/status` | `/content` | `services/catalyst/content/src/status.rs:12` (route `lib.rs:664`) | DIVERGENT |
| 6 | GET | `/challenge` | `/content` | `services/catalyst/content/src/status.rs` `get_challenge` (route `lib.rs:629`) | MATCH |
| 7 | GET | `/queries/items/:pointer/thumbnail` | `/content` | `services/catalyst/content/src/entity_thumbnail.rs:91` (route `lib.rs:665-669`) | DIVERGENT |
| 8 | HEAD | `/queries/items/:pointer/thumbnail` | `/content` | same handler | DIVERGENT |
| 9 | GET | `/queries/items/:pointer/image` | `/content` | `services/catalyst/content/src/entity_thumbnail.rs:121` (route `lib.rs:670-673`) | DIVERGENT |
| 10 | HEAD | `/queries/items/:pointer/image` | `/content` | same handler | DIVERGENT |
| 11 | GET | `/queries/erc721/:chainId/:contract/:option/:emission?` | `/content` | `services/catalyst/content/src/validator/erc721.rs:295`, as two routes with and without `{emission}` (`lib.rs:674-681`) | DIVERGENT |

Counts: 11 endpoints. MATCH 1, GAP 0, DIVERGENT 10, MISSING 0. Every route exists in dcl-one.
Each one differs in at least one field, header or error branch, as listed below.

---

## 1. GET /deployments

### Request

Upstream handler: `decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:21-140`.
Query parsing uses `qs.parse(..., {parseArrays:true, parameterLimit:2000, arrayLimit:100, depth:5})`
(`decentraland/catalyst:src/logic/query-params/component.ts:14-19`).

**Query parameters**

| param | upstream | dcl-one |
|---|---|---|
| `entityType` (repeatable; `entityType[0]=` and `entityType[]=` also accepted) | `qsGetArray`: a missing key or a single empty value gives `[]`; an empty element inside a multi-value list is kept; non-string elements are dropped (`query-params/component.ts:21-31`). So `?entityType=` adds no filter, and `?entityType=scene&entityType=` is a 400 (`parseEntityType('')` is undefined). `parseEntityType` trims and lowercases, then accepts the exact type or the type plus `s`, except for `outfits` (`types.ts:212-221`). Types: `scene`, `profile`, `wearable`, `store`, `emote`, `outfits`. | `qs_get_filter_array` applies the same rule (`shared/src/query_params.rs`), so `?entityType=` adds no filter. `parse_entity_type` builds the same name set (`outfitss` rejected). MATCH. |
| `entityId` (repeatable) | Case-sensitive `entity_id = ANY(...)`. A single empty value (`?entityId=`) is absent, so no filter; an empty element in a list (`entityId=a&entityId=`) is kept. | The same (`qs_get_filter_array`). MATCH. |
| `pointer` (repeatable) | Lowercased, then `entity_pointers && $pointers` (`deployments-repository/component.ts:118-121`). A single empty value (`?pointer=`) is absent, so no filter; an empty element in a list is kept. | Lowercased (`deployments.rs:139-142`); empty values as upstream (`qs_get_filter_array`). MATCH. |
| `deployedBy` | **Not read by the handler.** `DeploymentFilters.deployedBy` exists in `deployment-types.ts:7`, but the handler never fills it. | Read, lowercased, and filtered with `LOWER(deployer_address) = ANY(...)` (`deployments.rs:144-147`, `queries.rs:540-543`). Also echoed in `filters.deployedBy` and in `next`. |
| `onlyCurrentlyPointed` | `qsGetBoolean`: `undefined` when missing or empty; otherwise `=== 'true'`. When true, adds `deleter_deployment IS NULL`. | `qs_get_js_bool`, the same rule (`shared/src/query_params.rs`). MATCH (CS-08). |
| `from`, `to` | `parseInt(v, 10)`, so `10abc` → 10 and `1.5` → 1. A falsy value (0) adds no bound (`deployments-repository/component.ts:156-188`). | `qs_get_js_timestamp`, `parseInt`'s reading (`10abc` → 10, `1.5` → 1). `0` adds a `>= 0` bound, which returns the same rows. A value outside PostgreSQL's timestamp range is clamped to it; upstream passes it on and answers PostgreSQL's "timestamp out of range" as a 500. MATCH (CS-08), except that 500. |
| `offset` | `parseInt`. More than 5000 → 400. A negative value is treated as 0 (`logic/deployments/component.ts:237-239`). | Same (`deployments.rs` `parse_deployment_query_options`, `queries.rs`). |
| `limit` | `parseInt`. A value of 1..500 is used as given; anything else (missing, 0, negative, more than 500) becomes **500** (`logic/deployments/component.ts:235-242`). | Same (`queries.rs:21-28`). |
| `lastId` | Lowercased. Only used as a tiebreak on the bound in the direction of travel (`createOrClause`, `deployments-repository/component.ts:145-241`). | Lowercased. The tuple cursor is `(ts, lower(id)) < (ts of the lastId row, or T, lower(lastId))` (`queries.rs:374-453`). It returns the same rows, and it also handles sub-millisecond ties. |
| `fields` | Comma split, **no trim**. Only `content`, `pointers`, `metadata` and `auditInfo` are kept, and unknown values are dropped silently. Empty or whitespace means the default `pointers,content,metadata`. | Same, untrimmed (`deployments.rs` `parse_deployment_query_options`). MATCH (CS-08). |
| `sortingField` | camelCase → snake_case; must be `local_timestamp` or `entity_timestamp`. An empty value is ignored. Any other value → 400 `Found an unrecognized sort field param`. | Same mapping; an empty value is ignored (`deployments.rs` `parse_sort`). MATCH (CS-06). |
| `sortingOrder` | `ASC` or `DESC`, case-sensitive. An empty value is ignored (`|| undefined`). | Case-sensitive; an empty value is ignored, a repeated one is a 400 (`deployments.rs` `parse_sort`). MATCH (CS-06). |

**Filter caps and limits**

- The cap is 1000 values per filter for `entityId`, `pointer` and `entityType`. Both trees return
  the same message.
- `deployedBy` has no cap in dcl-one.
- `qs` has a parameter limit of 2000; dcl-one keeps the first 2000 pairs (`shared/src/query_params.rs:9,25`).

**Validation order**

- Upstream: entityType → offset → too many → sortingField → sortingOrder.
- dcl-one: the same (`deployments.rs` `parse_deployment_query_options`; test
  `offset_is_checked_before_the_caps_and_the_sort`) (CS-21).

**Auth and rate limits**

- No auth. No rate limit in the app; the gateway limit above applies.
- Upstream runs the query through `sequentialExecutor` (`get-deployments-handler.ts:125-128`).
- dcl-one memoizes the body for 5 s, keyed by the sorted query string, clears the memo on commit,
  and applies a server-side timeout (`deployments.rs:37-85`).

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | Example below. | `content-type: application/json` (http-server default) in both; the dcl-one memo adds no header. | `get-deployments-handler.ts:136-139` | `deployments.rs` `get_deployments` | DIVERGENT: `deployedBy` filter. Field shape: see notes. |
| Unknown entity type | 400 | `{"error":"Found an unrecognized entity type"}` | | `get-deployments-handler.ts:55-60` | `deployments.rs:130-135` | MATCH, `?entityType=` included (no filter, 200) |
| `offset` > 5000 | 400 | `{"error":"Offset can't be higher than 5000. Please use the 'next' property for pagination."}` | | `:62-67` | `deployments.rs` `parse_deployment_query_options` | MATCH |
| More than 1000 values in a filter | 400 | `{"error":"Too many filter values; the maximum allowed per filter is 1000"}` | | `:69-78` | `:142-149` | MATCH |
| Bad `sortingField` | 400 | `{"error":"Found an unrecognized sort field param"}` | | `:92-98` | `deployments.rs` `parse_sort` | MATCH; an empty value is ignored in both. A repeated value is upstream's 500 (`fromCamelCaseToSnakeCase` on an array); dcl-one reads the first |
| Bad `sortingOrder` | 400 | `{"error":"Found an unrecognized sort order param"}` | | `:99-105` | `deployments.rs` `parse_sort` | MATCH; an empty value is ignored in both |
| Repeated `sortingOrder` | 400 upstream (the value is an array, so `asEnumValue` returns `unknown`) | as above | | `utils.ts:36-45` | `deployments.rs` `parse_sort` | MATCH |
| Query exceeds the time budget | 503 | `{"error":"deployments query exceeded server-side time budget; narrow the time range or filters"}`, no `Retry-After` | | none | `deployments.rs:75-85` | dcl-one only |
| Database or unexpected error | 500 | `{"error":"Internal Server Error"}` | | `middlewares.ts:72-81` | `errors.rs` `AppError::Internal` | MATCH |

Success body example. With the default `fields`, `auditInfo` is absent; it is shown here with
`fields=pointers,content,metadata,auditInfo`:

```json
{
  "deployments": [
    {
      "entityVersion": "v3",
      "entityType": "scene",
      "entityId": "bafkreiexample",
      "entityTimestamp": 1700000000000,
      "deployedBy": "0x0000000000000000000000000000000000000001",
      "pointers": ["0,0"],
      "content": [{"key": "scene.json", "hash": "bafkreiexamplefile"}],
      "metadata": {"main": "bin/index.js"},
      "auditInfo": {"version": "v3", "authChain": [{"type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001", "signature": ""}], "localTimestamp": 1700000000500},
      "localTimestamp": 1700000000500
    }
  ],
  "filters": {"pointers": [], "entityTypes": ["scene"], "entityIds": [], "from": 1, "to": 2},
  "pagination": {"offset": 0, "limit": 500, "moreData": true, "next": "?entityType=scene&from=1&to=1700000000500&sortingField=local_timestamp&sortingOrder=DESC&lastId=bafkreiexample"}
}
```

**Deployment objects** (upstream `deployment2ControllerEntity`, `get-deployments-handler.ts:142-159`;
dcl-one `wire_types.rs:20-37`, `queries.rs:571-596`):

- Key order in both trees: `entityVersion, entityType, entityId, entityTimestamp, deployedBy,
  pointers?, content?, metadata?, auditInfo?, localTimestamp`.
- `content` is always an array when requested. Order:
  - upstream: the `content_files` read order;
  - dcl-one: `ORDER BY key COLLATE "en_US.utf8"` (`queries.rs:136`).
- `metadata`:
  - upstream omits the key when the stored `{v}` is falsy (`row.entity_metadata ? .v : undefined`,
    then `metadata && ...`);
  - dcl-one the same: a NULL column or a falsy `v` (`null`, `false`, `0`, `""`) leaves the key out
    (`queries.rs` `js_truthy`) (CS-22).
- `auditInfo`:
  - upstream: `{version, authChain, localTimestamp}`. `overwrittenBy` is never selected, so the key
    is always absent (`deployments-repository/component.ts:84-98`, `logic/deployments/component.ts:256-262`);
  - dcl-one: the same three keys (`wire_types.rs:12-18`).
- `authChain` key order inside each link:
  - upstream: stored-text order, because `auth_chain` is a `json` column that node-pg parses in
    order (`migrations/scripts/1589393477829_initial.ts`);
  - dcl-one: `serde_json::Value` without `preserve_order`, so keys are sorted (`payload, signature, type`).
- Timestamps:
  - upstream emits the raw `date_part('epoch', ts) * 1000` double;
  - dcl-one truncates it with `as i64` (`queries.rs:580,590`).
  - For millisecond-precision rows the two agree. A captured comparison is needed for rows whose
    double is not an integer (CS-19).

**Filters echo**:

- upstream: `{pointers, entityTypes, entityIds, onlyCurrentlyPointed?, from?, to?}`
  (`get-deployments-handler.ts:107-114`, `logic/deployments/component.ts:326-328`);
- dcl-one: the same keys plus `deployedBy` when non-empty (`wire_types.rs:51-65`).

**Pagination**:

- `{offset, limit, moreData, next?, lastId?}` in both trees.
- `lastId` echoes the lowercased request value; both omit it when it is missing.
- Both trees compute `moreData` before dropping denylisted entities from the truncated page, and build `next`
  from the last kept deployment, so a page that is entirely denylisted answers `moreData: true` with no
  `next` in both (`decentraland/catalyst:src/logic/deployments/component.ts:302-318`,
  `get-deployments-handler.ts:130-133`; `services/catalyst/content/src/deployments.rs:87-107`).

**`next` link format**:

- Upstream (`get-deployments-handler.ts:161-196`, `query-params/component.ts:40-74`):
  - `qs.stringify(..., {arrayFormat:'repeat'})` with falsy values dropped and array keys
    singularised.
  - Key order follows insertion: `pointer*, entityType*, entityId*, onlyCurrentlyPointed (if true),
    from, to, fields (if not the default array instance), sortingField, sortingOrder, lastId, limit (raw request value, if truthy)`.
  - The timestamp written to `from` (ASC) or `to` (DESC) is the last row's `localTimestamp` or
    `entityTimestamp`, depending on the sort field.
- dcl-one (`deployments.rs` `calculate_next_relative_path`, `shared/src/query_params.rs`
  `to_ordered_query_string`):
  - The same pairs in the same order, falsy values dropped, plus `deployedBy` after `entityId`.
  - `fields` is sent whenever the client listed fields that survive the filter, the default set included.
  - Percent-encoding uses the same unreserved set as `qs` (RFC 3986).
- The string matches, except for `deployedBy` (CS-03) and a `from`/`to` that upstream writes as a
  fractional double (CS-19) (test `next_keeps_upstreams_key_order_and_fields_rule`) (CS-07).

### Data dependency

- The `content.deployments` and `content.content_files` rows that the peer sync writes
  (`content/src/sync/*`), plus the denylist: the operator's `content.denylist` and the remote lists of
  `DENYLIST_URLS` (Decentraland's ABR `GET /denylist` under `NODE_POSTURE=mirror`, as upstream's default),
  fetched at boot and every 120 s (`content/src/denylist.rs`).
- dcl-one has all of these.
- Row and `localTimestamp` values differ per node by design (each node stamps its own
  `local_timestamp`).

---

## 2. GET /pointer-changes

### Request

Upstream handler: `decentraland/catalyst:src/controllers/handlers/pointer-changes-handler.ts:12-95`.
Logic: `decentraland/catalyst:src/logic/deployments/component.ts:381-420`.

- **Parameters.** `entityType`, `from`, `to`, `offset` (max 5000), `limit` (1..500, default and
  fallback 500), `lastId` (lowercased), `sortingField`, `sortingOrder` and `includeAuthChain`.
  Parsing matches §1: an empty `sortingField` or `sortingOrder` is ignored in both (CS-06).
- **No cap.** There is no 1000-value cap on `entityType` in either tree.
- **`includeAuthChain`.** Upstream parses it (default `false`) and only echoes it in `filters`.
  `authChain` is **always** included in each delta. dcl-one does the same
  (`pointer_changes.rs:58`, `queries.rs:656-668`).
- **Validation order.** Both trees check entityType → offset → sortingField → sortingOrder
  (`pointer_changes.rs` `parse_pointer_changes_options`) (CS-21).
- **No auth.** dcl-one memoizes for 5 s (`pointer_changes.rs:23-41`).

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | Example below. | `content-type: application/json` | `pointer-changes-handler.ts:90-94` | `pointer_changes.rs` `fetch_pointer_changes` | MATCH on the shape and `next` (CS-09); locally tombstoned scenes are left out (dcl-one only). |
| Unknown entity type | 400 | `{"error":"Found an unrecognized entity type"}` | | `:41-43` | `:46-52` | MATCH, `?entityType=` included |
| `offset` > 5000 | 400 | `{"error":"Offset can't be higher than 5000. Please use the 'next' property for pagination."}` | | `:45-47` | `:81-85` | MATCH |
| Bad `sortingField` | 400 | `{"error":"Found an unrecognized sort field param"}` | | `:51-57` | `deployments.rs` `parse_sort` | MATCH |
| Bad `sortingOrder` | 400 | `{"error":"Found an unrecognized sort order param"}` | | `:58-64` | `deployments.rs` `parse_sort` | MATCH |
| Unexpected error | 500 | `{"error":"Internal Server Error"}` | | `middlewares.ts:72-81` | `errors.rs` | MATCH |

```json
{
  "deltas": [
    {
      "deploymentId": 12345,
      "entityType": "scene",
      "entityId": "bafkreiexample",
      "pointers": ["0,0"],
      "entityTimestamp": 1700000000000,
      "deployerAddress": "0x0000000000000000000000000000000000000001",
      "version": "v3",
      "authChain": [{"type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001", "signature": ""}],
      "localTimestamp": 1700000000500
    }
  ],
  "filters": {"entityTypes": [], "from": 1, "includeAuthChain": false},
  "pagination": {"offset": 0, "limit": 500, "moreData": true, "next": "?from=1&to=1700000000500&limit=500&lastId=bafkreiexample"}
}
```

**Delta shape**:

- Upstream returns the repository `HistoricalDeployment` as it is
  (`deployments-repository/component.ts:204-218`):
  - `metadata` is selected as NULL, so it is `undefined` and absent;
  - `overwrittenBy` is not selected, so it is absent.
- dcl-one (`wire_types.rs:74-86`): **same keys, same order**.
- `deploymentId` is each node's own serial id, so its value differs between any two nodes.

**Filters echo**:

- upstream: `{entityTypes: [...], from?, to?, includeAuthChain}`;
- dcl-one: the same (`wire_types.rs:88-97`).

**Pagination**:

- Upstream: `{offset, limit, moreData, next?}`. **There is no `lastId`**
  (`logic/deployments/component.ts:414-418`).
- dcl-one sets `last_id: None` (`pointer_changes.rs:127-133`). MATCH.

**`next` link format**:

- Upstream (`pointer-changes-handler.ts:97-114`):
  - Pairs: `entityType*, from (if set), to=<last.localTimestamp>, includeAuthChain=true (only if
    true), limit=<curated limit>, lastId=<last.entityId>`.
  - It always writes `to` and never carries `sortingField` or `sortingOrder`, so a client that
    asked for ASC is sent back DESC.
- dcl-one (`pointer_changes.rs` `next_relative_path`): the same pairs and order, `includeAuthChain=true`
  included, `from=0` dropped as upstream drops a falsy value, encoded as `qs` does (test
  `next_carries_include_auth_chain`) (CS-09).
- The ASC behaviour matches upstream. dcl-one's own sync client builds its own keyset URL and does
  not follow `next` (`content/src/sync/pointer_changes.rs:200-221`).

**Row selection**:

- Both trees order by `ORDER BY <ts> <order>, LOWER(entity_id) <order>` with limit+1.
- Both compute `moreData` before the denylist filter and drop denylisted ids after truncating.
- dcl-one also drops deployments of entities this node tombstoned (local unpublish), and the
  entities those overwrote (`queries.rs:713-728`). Upstream has no tombstones.

### Data dependency

- `content.deployments`, the denylist (`content.denylist` and the `DENYLIST_URLS` lists, as for
  `/deployments`), and `content.local_entities` (dcl-one only).
- An upstream peer that syncs from dcl-one reads `entityId`, `entityType`, `pointers`,
  `entityTimestamp`, `localTimestamp` and `authChain` from each delta. dcl-one serves all of them.

---

## 3. GET /failed-deployments

### Request

- `offset` and `limit` are optional (`parseInt`).
- With neither one, the full list comes back.
- With either one, the response is a slice:
  - `offset` < 0 → 0;
  - `limit` is clamped to `[0, 1000]`;
  - a missing `limit` → 1000.
- Upstream: `decentraland/catalyst:src/controllers/handlers/failed-deployments-handler.ts:9-38`.
- dcl-one: `services/catalyst/content/src/failed_deployments.rs:14-35`.
- The paging rules match.

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | Bare array; example below. | `content-type: application/json` | `failed-deployments-handler.ts:24-37` | `failed_deployments.rs`, `queries.rs` `failed_deployments` | MATCH on `snapshotHash` and `reason` (CS-10); the order follows the table, see below. |
| Unexpected error | 500 | `{"error":"Internal Server Error"}` | | `middlewares.ts` | `errors.rs` | MATCH |

```json
[
  {
    "entityId": "bafkreiexample",
    "entityType": "scene",
    "failureTimestamp": 1700000000000,
    "reason": "Deployment error",
    "authChain": [{"type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001", "signature": ""}],
    "errorDescription": "...",
    "snapshotHash": "bafkreisnapshot",
    "retryCount": 0,
    "nextRetryAt": 1700000900000
  }
]
```

Field by field:

- **Source.**
  - Upstream answers from an in-memory map: rows loaded from the DB at start (no ORDER BY), then
    failures in the order they happened (`adapters/failed-deployments/component.ts:176-187`).
  - dcl-one reads the table in physical order (`ORDER BY ctid`, `queries.rs` `failed_deployments`): the
    order upstream reads it in at start. A failure reported again while upstream runs keeps its place
    in the map; here the update moves its row (CS-10).
  - With `offset` or `limit`, dcl-one orders the page by `failure_time, entity_id` instead: a retry's
    upsert moves a row's `ctid`, so paging in physical order would skip and repeat rows.
- **`snapshotHash`.**
  - Upstream: absent for failures that did not come from a snapshot. Those are cached but not
    persisted (`component.ts:241-254`).
  - dcl-one: the key is left out when the stored value is `""` (column `NOT NULL`, `sync_deployer.rs`
    writes `""` for those failures) (CS-10).
- **`reason`.**
  - Upstream: only `"Deployment error"` (`FailureReason`, `adapters/failed-deployments/types.ts:5-7`).
  - dcl-one: every stored reason is answered as `"Deployment error"` (`queries.rs` `failed_deployments`);
    `errorDescription` says which kind (CS-10).
- **`failureTimestamp` and `nextRetryAt`.** Upstream emits the raw doubles; dcl-one emits
  truncated integers.
- **Per-replica state.** With several upstream replicas, each one answers its own in-memory set.
  dcl-one answers from the shared table.

### Data dependency

- `content.failed_deployments`, which dcl-one's sync deployer writes.

---

## 4. GET /snapshots (and the snapshot file served by /contents)

### Request

- No parameters and no auth.
- dcl-one memoizes the list in the 5 s history memo (`snapshots.rs:62-75`).

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Snapshots exist | 200 | Array of `SnapshotMetadata`; example below. | `content-type: application/json` | `get-snapshots-handler.ts:18-21` | `snapshots.rs:77-85` | MATCH |
| Upstream: no completed scheduled generation since process start. dcl-one: `content.snapshots` empty. | 503 | `{"error":"New Snapshots not yet created"}` | Upstream sends no `Retry-After`; dcl-one neither. | `get-snapshots-handler.ts:11-16` | `snapshots.rs:80-83` | MATCH (body). The triggers differ, see below. |
| Unexpected error | 500 | `{"error":"Internal Server Error"}` | | | | MATCH |

```json
[
  {
    "hash": "bafybeiexampleyear",
    "timeRange": {"initTimestamp": 1577836800000, "endTimestamp": 1606867200000},
    "replacedSnapshotHashes": [],
    "numberOfEntities": 123456,
    "generationTimestamp": 1700000000000
  }
]
```

Key order:

- upstream: `hash, timeRange, replacedSnapshotHashes, numberOfEntities, generationTimestamp`
  (`snapshots-repository/component.ts:61-72`, `logic/snapshots/component.ts:126-133`);
- dcl-one: `hash, timeRange, numberOfEntities, replacedSnapshotHashes, generationTimestamp`
  (`wire_types.rs:144-153`).
- JSON consumers ignore key order.

**Windows.** Both trees split `[1577836800000, now]` into windows of year
(`12*28` days) / month (28 days) / week / day with the same algorithm. The trailing remainder has
no snapshot in either tree:

- upstream `decentraland/catalyst:src/logic/time-range.ts:75-106`;
- dcl-one `services/catalyst/content/src/time_range.rs:18-80`.

**Array order**:

- Upstream: the order of the division (year windows first, then month, week, day), oldest first
  (`logic/snapshots/component.ts:71-163`).
- dcl-one: `ORDER BY init_timestamp, end_timestamp`, the same division order (`queries.rs`
  `all_snapshots`). MATCH.

**`replacedSnapshotHashes`**:

- Upstream:
  - when a window is regenerated, this is the hashes of the saved rows inside it, if they covered
    it or held inactive entities, and `[]` otherwise;
  - the new hash itself is not removed from the list;
  - reused rows carry their stored list.
- dcl-one:
  - the hashes of every row deleted inside the window, minus the new hash (`snapshot_gen.rs:670-683`);
  - always present (`queries.rs:966`).
- Upstream also lists the window's previous hash when the window was regenerated because it was
  outdated, and dcl-one does the same. MATCH in effect.

**`generationTimestamp`**:

- Upstream: `Date.now()` taken before the stream opens (`logic/snapshots/component.ts:39`).
- dcl-one:
  - the start of the oldest open transaction, capped at `now()` (`snapshot_gen.rs`, `read_horizon_ms`);
  - a deleter repair that superseded rows of a window sets `outdated` on its row and leaves this value
    as it was; the next pass regenerates the window.

**`hash`**:

- Upstream lists only stored hashes.
- dcl-one leaves out a row whose `hash` is NULL or empty (`queries.rs` `all_snapshots`); the column is
  nullable (`schemas/00_content.sql:119`) but the current generator always inserts a hash. MATCH.

**When regeneration happens**:

- Upstream regenerates a window when:
  - it is not covered;
  - several snapshots cover it;
  - a file is missing;
  - it is more than one month old and has fewer active entities than `numberOfEntities`; or
  - it is outdated.
- dcl-one regenerates when it is outdated, when its row is more than one month (four weeks) old
  and the window now counts fewer active entities than `numberOfEntities` (upstream's rule, with
  the old hash in `replacedSnapshotHashes`), or when the bytes fail the hash check
  (`snapshot_gen.rs` `pass`, `has_inactive_entities`). The count leaves out tombstoned local
  scenes, as the file does. `POST /admin/api/content/snapshots/regenerate` rebuilds every window
  at once. (CS-14)
- Until a window is regenerated it lists the overwritten entities. A peer that reads them still
  converges, because newer entities win.

**When the 503 is returned**:

- Upstream returns 503 until the first scheduled run of the process finishes (6 h job,
  `components.ts:477`).
- dcl-one returns 503 only while the table is empty.

**Snapshot file served by `GET /contents/{hash}`**:

- Upstream (`logic/snapshots/component.ts:42-46`, `snapshots-repository/component.ts:9-33`):
  - the header line `### Decentraland json snapshot\n`, then one `JSON.stringify` line per active
    deployment;
  - rows where `entity_timestamp BETWEEN init AND end` (inclusive), `ORDER BY entity_timestamp`,
    with no tiebreak;
  - keys `entityId, entityType, pointers, authChain, entityTimestamp`;
  - the hash is `hashV1` of the bytes, stored compressed (`content-file-writer/component.ts:72-83`).
- dcl-one (`snapshot_gen.rs:1-17,502-510`):
  - the same header, keys and window;
  - order `(entity_timestamp, entity_id)`;
  - `authChain` link keys sorted (`payload, signature, type`) where upstream keeps stored order
    (`type, payload, signature`);
  - `entityTimestamp` as an integer where upstream writes the raw double;
  - locally tombstoned entities are left out;
  - the hash is CIDv1 of the bytes.
- Result: for the same data the bytes and the CID differ from an upstream node's. A peer
  re-processes the file instead of skipping it as already processed. The content is equivalent.

### Data dependency

- `content.snapshots` and the blob store, both filled by dcl-one's generator (`snapshot_gen.rs`).
- Generation waits until sync reaches `Syncing`.

---

## 5. GET /status

### Request

- No parameters and no auth.

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Normal | 200 | Example below. | `content-type: application/json` | `status-handler.ts:6-25` | `status.rs` `get_status` | DIVERGENT: `version` and the extra keys (CS-15) |
| Upstream: a status-capable component rejected. No component implements `getComponentStatus` at this revision (`logic/status-checks.ts:16-40`), so this branch never fires. | 503 | same body | | `status-handler.ts:14` | none | n/a |

```json
{
  "version": "Unknown",
  "commitHash": "Unknown",
  "ethNetwork": "mainnet",
  "synchronizationStatus": {"lastSyncWithDAO": 1700000000000, "synchronizationState": "Syncing"}
}
```

| field | upstream | dcl-one |
|---|---|---|
| `version` | `process.env.CURRENT_VERSION ?? 'Unknown'` (`Environment.ts:270`) | `CURRENT_VERSION` when set and not blank, else this build's `CARGO_PKG_VERSION` (`content/src/lib.rs` `Settings::from_config`, read through `config/src/social.rs` `social_rpc_current_version`) (CS-15) |
| `commitHash` | `process.env.COMMIT_HASH ?? 'Unknown'` (`Environment.ts:269`) | The configured or build-time hash, otherwise `"Unknown"` (`status.rs`) |
| `ethNetwork` | `ETH_NETWORK` config | `config.node.eth_network` |
| `synchronizationStatus.lastSyncWithDAO` | Time of the last successful DAO server-list refresh; 0 before the first one (`logic/peer-cluster/component.ts:43,79,123-125`) | The last sync heartbeat (this node's equivalent: it follows `SYNC_SOURCE` peers, not a DAO list); 0 before the first one (`status.rs`) |
| `synchronizationStatus.synchronizationState` | `"Bootstrapping"` or `"Syncing"` (`sync-orchestrator/types.ts:7-10`) | `"Bootstrapping"`, `"Syncing"` (which also covers partially synced) or **`"Failed"`** (`content/src/lib.rs:373-379`); the status stays 200 for every state |
| extra keys | none | `syncFrontier`, `lastHeartbeat` and `up` when the sync worker reports them (`wire_types.rs:106-120`) |

Rollout: `/content/status` answers 200 whatever the sync state, as upstream does at this revision, so a
monitor that alerted on a non-200 status loses the failed-sync signal; read `synchronizationState`
(`"Failed"`) instead. Nothing in the repository probes it; the gateway checks `/_gateway/healthz`.

### Data dependency

- The process configuration and the sync worker state.
- dcl-one has no DAO list refresh to time; `lastSyncWithDAO` reports the sync heartbeat instead.

---

## 6. GET /challenge

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Always | 200 | `{"challengeText": "<string>"}` | `get-challenge-handler.ts:4-12` | `status.rs` `get_challenge` | MATCH |

| | upstream | dcl-one |
|---|---|---|
| Value | One `uuidv4()` per process, the same on every call (`logic/peer-cluster/component.ts:51,127-129`) | One UUID v4 per process in `ContentState`; the console's `challenge/refresh` replaces it in that process only. With several `serve` replicas behind one URL, `GET /challenge` can answer a different UUID per replica, as upstream would with several processes |
| Format | A bare UUID, e.g. `"8c9e...-..."` | the same |
| Purpose | "a peer that fetches `/challenge` from every DAO server and finds a matching value has located its own process" (`logic/peer-cluster/types.ts:8-13`) | — |

Each `catalyst serve` process has its own value, as each upstream process does.

- **Data dependency:** none.

---

## 7–10. GET/HEAD /queries/items/:pointer/thumbnail and /image

### Request

- The path parameter `pointer` is percent-decoded by the router.
- Lookup is case-insensitive in both trees:
  - upstream `withPointers` → `getDeploymentsForActiveEntities` lowercases
    (`logic/deployments/component.ts:368`);
  - dcl-one lowercases (`queries.rs:187`).
- Headers read: `If-None-Match` and `Range`.
- No auth. No rate limit in the app; the gateway limit above applies.
- Upstream reads no query parameter.
- dcl-one's thumbnail route reads `?width=` (`entity_thumbnail.rs:38-45,98-104`). The image route
  does not.
- Both trees serve the bytes in the body. **Neither one redirects** (no 302). dcl-one may return
  `X-Accel-Redirect` with an empty body for nginx to fill (`entity_thumbnail.rs:211-222`). Through
  nginx the client gets the body; directly, it gets 200 with `Content-Length: 0`.

### Responses

Upstream: `decentraland/catalyst:src/controllers/handlers/get-entity-thumbnail-handler.ts:7-55`
(image: `get-entity-image-handler.ts:7-55`). dcl-one: `services/catalyst/content/src/entity_thumbnail.rs:91-233`.

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| No active entity at the pointer, or the entity is denylisted | 404 | `{"error":"Entity not found."}` | | `:15-20` | `:47-62` | MATCH |
| `metadata.thumbnail` (or `.image`) is missing, the file is not in `content`, or the hash is denylisted | 404 | `{"error":"Entity has no thumbnail."}` / `{"error":"Entity has no image."}` | | `:22-26` | `:106-110`, `:128-132` | MATCH |
| `If-None-Match` is `*` or matches `"<hash>"` (weak tags allowed) | 304 | empty | `ETag: "<hash>"`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Access-Control-Expose-Headers: ETag` | `:28-29`, `utils.ts:88-113` | `:114-116`, `formatters.rs:9-33` | MATCH |
| The blob is not in storage | 404 | upstream `{"error":"Entity has no thumbnail."}` / `"Entity has no image."` | | `:33-35` | `entity_thumbnail.rs` `serve_content_blob`: the same messages | MATCH (CS-18) |
| Unsatisfiable `Range` | 416 | empty | `Content-Range: bytes */<total>`, `Access-Control-Expose-Headers: Content-Range` | `:37-42`, `utils.ts:187-195` | `:182-192` | MATCH |
| Satisfiable `Range` | 206 | the byte range (empty for HEAD) | the content headers plus `Content-Range: bytes s-e/total` and `Content-Length` | `:44-54`, `utils.ts:197-211` | `:193-209` | MATCH |
| Plain GET or HEAD | 200 | the bytes (empty for HEAD) | `Content-Type` (sniffed; `application/octet-stream` fallback), `ETag: "<hash>"`, `Access-Control-Expose-Headers: ETag, Content-Range, Accept-Ranges, Content-Length`, `Accept-Ranges: bytes`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Content-Length`. Upstream also sends `Content-Encoding` when the blob is stored compressed. | `utils.ts:115-154` | `:210-231`, `formatters.rs:85-102` | MATCH. Sniffing: upstream uses `file-type` on the stream; dcl-one sniffs the first 32 bytes (`get_content::detect_content_type`). These agree for PNG, JPEG and WebP. dcl-one never sends `Content-Encoding`. |
| `?width=` on the thumbnail route, not 80/160/320/640/960 | — | upstream ignores it and serves 200 | | — | `entity_thumbnail.rs` `requested_width`: ignored, the stored file is served | MATCH (CS-18) |
| `?width=` valid | — | upstream serves the original | | — | `:111-113,255-301`: a resized PNG, WebP or JPEG with `ETag: "thumbnail-v1-<w>-<hash>"` and `Cache-Control: public, max-age=300, stale-while-revalidate=86400` | dcl-one only |
| Pointer or file not local, read-through on | — | upstream 404 | | — | dcl-one fills it from the sync sources first (`:47-52,150-163`) | dcl-one only (a superset) |

### Data dependency

- Active entities (`content.active_pointers` and `content.deployments`), the content blob store,
  and the denylist.
- dcl-one has all of these from sync.

---

## 11. GET /queries/erc721/:chainId/:contract/:option/:emission?

### Request

- `chainId` must match `^\d+$`.
- The URN protocol comes from `getURNProtocol(chainId)` in `@dcl/schemas`, which is not in the
  clone.
- `contract` starting with `0x` gives `collections-v2`; anything else gives `collections-v1`.
- The URN is `urn:decentraland:<protocol>:collections-<v>:<contract>:<option>`, used as given
  (lookup is case-insensitive).
- `emission` is optional.
- No auth.

### Responses

Upstream: `decentraland/catalyst:src/controllers/handlers/get-erc721-entity-handler.ts:8-44`.
Formatter: `src/logic/entities/component.ts:54-94,159-180`.
dcl-one: `services/catalyst/content/src/validator/erc721.rs:48-131,198-222,295-328`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `chainId` is not all digits | 400 | `{"error":"Invalid chainId '<chainId>'"}` | `:19-21` | `erc721.rs` `get_erc721_entity` (digits only) | MATCH (CS-17) |
| Unknown chain | 400 | same | `:23-27` | `:198-210,301`. Table: 1 ethereum, 3 ropsten, 4 rinkeby, 5 goerli, 137 matic, 80001 mumbai, 80002 amoy, 11155111 sepolia. | MATCH if `@dcl/schemas` maps the same set (not checked: the package is not in the clone) |
| No active entity, entity denylisted, or a falsy metadata | 404 | `{"error":"Entity does not exist"}` | `:32-34` | `erc721.rs` `get_erc721_entity` | MATCH |
| `metadata.rarity` is falsy (absent, `null`, `""`) | 400 | `{"error":"Wearable is not standard."}` | `:36-38` (`!rarity`) | `erc721.rs` `get_erc721_entity` (`js_truthy`) | MATCH (CS-17) |
| Success | 200 | Example below. | `:40-43` | `erc721.rs` `format_erc721_entity` | MATCH on the fields below |
| `metadata.i18n` is missing, or the item data lacks `tags` or `representations` | upstream 500 (`i18ns.filter`, `itemData.tags.reduce` or the `representations` loop on undefined) | | `component.ts:159-180` | 200 without `name`, with no `Tag` trait | DIVERGENT: upstream's crash on a malformed item is not reproduced; a deployed wearable or emote has all three |

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000001:0",
  "name": "Example Hat",
  "description": "DCL Wearable 42/1000",
  "language": "en-US",
  "image": "https://peer.example.org/content/contents/bafkreiimage",
  "thumbnail": "https://peer.example.org/content/contents/bafkreithumb",
  "attributes": [
    {"trait_type": "Rarity", "value": "epic"},
    {"trait_type": "Category", "value": "hat"},
    {"trait_type": "Tag", "value": "cool"}
  ]
}
```

Field by field:

- **`description` with an unknown rarity.**
  - Upstream: `DCL Wearable <em>/undefined`.
  - dcl-one: the same (`erc721.rs` `format_erc721_entity`) (CS-17).
- **Item data source.**
  - Upstream reads `emoteDataADR74 ?? data` (`component.ts:178-180`).
  - dcl-one the same (`erc721.rs` `item_data`) (CS-17).
- **`Category` trait.**
  - Upstream always emits it, even when the category is undefined; `{"trait_type":"Category"}`
    then has no `value` key.
  - dcl-one does the same: the trait is always second, without `value` when the category is absent
    (`erc721.rs` `format_erc721_entity`). MATCH.
- **`Body Shape` traits.**
  - Upstream compares each body shape with `BodyShape[BodyShape.MALE]` (`component.ts:164-176`).
    If `BodyShape` is a string enum (which has no reverse mapping), that expression is `undefined`
    and upstream emits **no** Body Shape traits. Captured: a wearable with a `BaseMale` and a
    `BaseFemale` representation answers `Rarity`, `Category` and `Tag` traits and no `Body Shape`
    trait (`captures/content-sync/CS-16-erc721-wearable.json`).
  - dcl-one emits none either (`erc721.rs` `format_erc721_entity`). MATCH.
- **`image` and `thumbnail`.**
  - Upstream: `new URL('contents/<hash>', CONTENT_SERVER_ADDRESS)`. Without a trailing slash, the
    last segment of the address is replaced.
  - dcl-one: `<content_public_url without trailing '/'>/contents/<hash>`, where the default
    `content_public_url` is `<own_url>/content/` (`content/src/lib.rs:217-218`).
  - They agree when the upstream address ends in `/`.
  - Both omit the key when the hash is missing.
- **`name`.**
  - Upstream: the `en` entry, otherwise the first entry; the key is absent if the text is undefined.
  - dcl-one: the same choice, and no key when there is no text (`erc721.rs` `Erc721Metadata`) (CS-17).

### Data dependency

- Active wearable and emote entities in `content.deployments`.
- dcl-one does not use read-through on this route, while the thumbnail routes do.

---

## Sync-peer and explorer expectations

### Upstream peers syncing from dcl-one

Their synchronizer reads, in this order:

1. `GET /snapshots`.
2. Each listed hash through `GET /contents/{hash}`. Each line is parsed after the header line.
3. `GET /pointer-changes` from the latest window end, following `pagination.next` while `moreData`
   is true.

For these steps dcl-one serves every field that upstream serves, in a compatible shape, and every
content hash it lists is one `GET /contents/{hash}` serves (CS-24). The ways a peer would see a
difference:

- **Re-processing.** The snapshot bytes and CIDs differ from an upstream node's for the same data
  (sorted `authChain` keys, integer timestamps, a tiebreak on `entity_id`). A peer re-processes
  dcl-one's files instead of skipping them as already processed. (CS-13)
- **Epoch `generationTimestamp`.** A window can be listed with `generationTimestamp: 0`. (CS-12)
- **Array order.** `/snapshots` is newest first instead of oldest first. (CS-11)
- **`next` drops a flag.** `next` drops `includeAuthChain=true`. `authChain` is still always sent,
  so this does not affect data. (CS-09)
- **Tombstoned scenes.** dcl-one leaves out scenes it tombstoned locally from `/pointer-changes`
  and from the snapshots. A peer that synced such a scene earlier keeps it.

These are out of scope for this file, but a peer only syncs from dcl-one if dcl-one is in the DAO
catalyst list and its `/about` is healthy.

### Explorer

- In this area the explorer and the marketplace read:
  - `/queries/items/:pointer/{thumbnail,image}`, for the bytes;
  - `/queries/erc721/*`, for marketplace and token metadata;
  - `/status`, through realm and health probes.
- The most visible differences are the extra and missing ERC-721 traits (CS-16, CS-17).

---

## Backlog (CS-)

1. **CS-01 · `/deployments` · denylisted entities are dropped from the page.** Status: MATCH, no work.
   - Upstream: filter `denylist.isDenylisted(entityId)` on the truncated page, with `moreData` computed
     before the filter (`decentraland/catalyst:src/logic/deployments/component.ts:316-318`).
   - dcl-one: the same filter after the query, before `next` is built
     (`services/catalyst/content/src/deployments.rs:93-98`).
   - Severity: none (closed).

2. **CS-02 · `/deployments` · empty filter values filter on `''`.** Status: MATCH, no work.
   - Upstream: `qsGetArray` treats a missing key or a single empty value as absent and keeps empty strings
     inside a multi-value list (`decentraland/catalyst:src/logic/query-params/component.ts:21-31`).
   - dcl-one: `qs_get_filter_array` (`services/catalyst/shared/src/query_params.rs`), used by every content
     route that reads a filter list (`/deployments`, `/pointer-changes`, `/entities/:type`,
     `/available-content`); `qs_get_array` keeps its old rule for the shop and worlds callers. Pinned in
     `content/src/tests.rs` (`history_reads_over_seeded_rows`).
   - Severity: none (closed).

3. **CS-03 · `/deployments` · the `deployedBy` filter is dcl-one only.** Status: Kept by decision.
   - Gap: upstream ignores `deployedBy`; dcl-one filters on it and echoes it in `filters` and `next`.
   - Decision: kept as a documented superset, because the creator hub lists a wallet's deployments with it
     (`apps/sites/src/data/lib/catalyst/creator-hub/discover-deployed.test.ts`). A request without the
     parameter answers as upstream; a client that passes it to upstream gets unfiltered rows.
   - Where: `services/catalyst/content/src/deployments.rs` `parse_deployment_query_options`,
     `services/catalyst/content/src/queries.rs`, `services/catalyst/content/src/wire_types.rs`.
   - Severity: P2 (by design).

4. **CS-04 · `/deployments` · extra response headers.** Status: MATCH, no work.
   - Upstream: no `cache-control` and no `x-cache`
     (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:136-139`).
   - dcl-one: the 5 s memo stays internal; the response carries only `content-type`
     (`services/catalyst/content/src/deployments.rs`).
   - Severity: none (closed).

5. **CS-05 · `/deployments` and `/pointer-changes` · `?entityType=` gives a 400.** Status: MATCH, no work.
   - Upstream: a single empty value is no filter (200); an empty element inside a list is a 400.
   - dcl-one: the same through `qs_get_filter_array` (CS-02).
   - Severity: none (closed).

6. **CS-06 · `/deployments` and `/pointer-changes` · an empty `sortingField` or `sortingOrder`.** Status: MATCH, no work.
   - Upstream: `''` is missing (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:44-50`);
     a repeated `sortingOrder` is an array, which reads as unknown (400).
   - dcl-one: the same (`deployments.rs` `parse_sort`, shared by both routes; tests
     `values_read_as_upstreams_query_params`, `values_and_checks_follow_upstream`).
   - Severity: none (closed).

7. **CS-07 · `/deployments` · `pagination.next` key order and `fields` rule.** Status: MATCH, no work.
   - Upstream: the order `pointer, entityType, entityId, onlyCurrentlyPointed, from, to, fields, sortingField,
     sortingOrder, lastId, limit`, falsy values dropped, `fields` whenever the client listed some
     (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:161-196`).
   - dcl-one: the same (`deployments.rs` `calculate_next_relative_path`, `shared/src/query_params.rs`
     `to_ordered_query_string`; test `next_keeps_upstreams_key_order_and_fields_rule`); `deployedBy` (CS-03)
     follows `entityId`.
   - Severity: none (closed).

8. **CS-08 · `/deployments` and `/pointer-changes` · number and boolean parsing.** Status: MATCH, no work.
   - Upstream: `qsGetNumber` (`parseInt`: `10abc` → 10, `1.5` → 1) and `qsGetBoolean` (empty is absent),
     `fields` items untrimmed (`decentraland/catalyst:src/logic/query-params/component.ts:32-39`).
   - dcl-one: `qs_get_js_number`, `qs_get_js_bool` (`services/catalyst/shared/src/query_params.rs`) on
     `/deployments`, `/pointer-changes` and `/failed-deployments`; `fields` untrimmed (tests
     `js_number_and_bool_follow_qs_get_number_and_boolean`, `values_read_as_upstreams_query_params`).
   - Severity: none (closed).

9. **CS-09 · `/pointer-changes` · `next` carries `includeAuthChain=true`.** Status: MATCH, no work.
   - Upstream: `includeAuthChain=true` after `to` when the flag is true
     (`decentraland/catalyst:src/controllers/handlers/pointer-changes-handler.ts:97-114`).
   - dcl-one: the same (`pointer_changes.rs` `next_relative_path`; test `next_carries_include_auth_chain`).
   - Severity: none (closed).

10. **CS-10 · `/failed-deployments` · `snapshotHash`, `reason` and order.**
    - Upstream: `snapshotHash` absent outside a snapshot, `reason` always `"Deployment error"`
      (`decentraland/catalyst:src/adapters/failed-deployments/types.ts:5-25`); the in-memory map's order:
      the table read at start (no ORDER BY), then report order.
    - Done: `snapshotHash` left out when empty, `reason` always `"Deployment error"`, rows in table order
      (`ORDER BY ctid`, the order upstream loads them in) (`services/catalyst/content/src/queries.rs`
      `failed_deployments`, `wire_types.rs`; test `history_reads_over_seeded_rows`).
    - Left: a failure reported again keeps its place in upstream's running map; here the upsert moves its
      row. Matching that needs a report sequence column (a schema step); not worth it for an operator list.
    - Severity: P2 (order of re-reported failures only).

11. **CS-11 · `/snapshots` · array order.** Status: MATCH, no work.
    - Upstream: division order, oldest window first (`decentraland/catalyst:src/logic/snapshots/component.ts:67-164`).
    - dcl-one: `ORDER BY init_timestamp, end_timestamp` (`services/catalyst/content/src/queries.rs` `all_snapshots`),
      pinned in `content/src/tests.rs` (`snapshot_routes_over_the_database`).
    - Severity: none (closed).

12. **CS-12 · `/snapshots` · `generationTimestamp` 0 and empty `hash`.** Status: MATCH, no work.
    - Upstream: lists only stored snapshots, each with the real generation time
      (`decentraland/catalyst:src/logic/snapshots/component.ts:39,126-133`).
    - dcl-one: rows without a hash are left out; the deleter repair marks a window with
      `content.snapshots.outdated` (step `0026_content_snapshots_outdated`) and keeps its generation time
      (`services/catalyst/content/src/snapshot_gen.rs`, `content/src/sync/repository.rs`), pinned in
      `tests/snapshot_generation.rs`.
    - Severity: none (closed).

13. **CS-13 · snapshot file bytes.** Status: Kept by decision.
    - Decision: the bytes are kept (option a). Changing them changes every CID this node has
      published, so every peer would re-download every window once.
    - Gap: `authChain` keys are sorted, `entityTimestamp` is an integer, and ties are broken by
      `entity_id`. For the same data the CID differs from an upstream node's, so peers re-process
      the file.
    - Expected: the upstream line is `JSON.stringify({entityId, entityType, pointers, authChain, entityTimestamp})`,
      with `authChain` in stored key order
      (`decentraland/catalyst:src/adapters/snapshots-repository/component.ts:9-33`,
      `decentraland/catalyst:src/logic/snapshots/component.ts:42-46`).
    - Not changed here: upstream orders by `entity_timestamp` only, so its tie order is the plan's and two
      upstream nodes need not agree either; equal CIDs are not reachable for windows with ties. Changing
      the bytes changes every CID this tree has published, so every peer re-downloads every window once,
      and the golden `tests/fixtures/snapshots/old-generator.json` (the old generator's output) must be
      re-pinned. The option not taken: read `auth_chain::text` into an order-keeping value and write
      `entityTimestamp` with `js_millis`, keeping the `entity_id` tie-break, and re-pin the golden.
    - Where: `services/catalyst/content/src/snapshot_gen.rs` `SnapshotLine`, `WINDOW_SQL`.
    - Severity: P2 (by decision). Sync is still correct; the cost is bandwidth and bootstrap work.

14. **CS-14 · snapshots · "inactive entities after a month" regeneration.** Status: MATCH, no work.
    - Upstream: regenerate a window whose single saved row was generated more than `MS_PER_MONTH`
      (four weeks) ago and whose active count is below `numberOfEntities`, with the old hash in
      `replacedSnapshotHashes` (`decentraland/catalyst:src/logic/snapshots/component.ts:81-87,124-125`,
      `decentraland/catalyst:src/adapters/snapshots-repository/component.ts:163-177`).
    - dcl-one: the same check in each pass (`services/catalyst/content/src/snapshot_gen.rs` `pass`,
      `has_inactive_entities`), measured against the pass's end. The count uses the file's own
      filter, so tombstoned local scenes are left out. The other indexes do not serve the count (a
      large window reads the whole heap); the partial index
      `deployments_active_entity_timestamp_idx` (`schemas/00_content.sql`; built on existing
      databases by the object pass of `catalyst schema migrate`) makes it an index-only scan
      (`METRICS.md`, "Snapshot inactive-entities count"). Pinned in `tests/snapshot_generation.rs`
      (`a_month_old_window_listing_an_overwritten_entity_is_regenerated`).
    - Severity: none (closed).

15. **CS-15 · `/status` · field values.** Status: Kept by decision (the extra keys).
    - Decision: the extra keys `syncFrontier`, `lastHeartbeat` and `up` are kept, because the
      conformance tooling reads them (`testing/src/conformance/volatility.toml`).
    - Done: `version` is `CURRENT_VERSION` when set (upstream's variable), else this build's version
      (`content/src/lib.rs` `Settings::from_config`, `config/src/social.rs`; test
      `current_version_overrides_the_build_version`). The variable is the one the social RPC `/status`
      reads too.
    - Left, by design: unset, `version` is this build's version where upstream says `"Unknown"`; the
      extra keys (dcl-one sync health, above); `lastSyncWithDAO` is the last sync heartbeat, as this
      node has no DAO list. A client reading the upstream keys sees the same types.
    - Matching: always 200 (a `Failed` sync state included), and the `commitHash` fallback `"Unknown"`
      (`services/catalyst/content/src/status.rs`).
    - Expected: `decentraland/catalyst:src/controllers/handlers/status-handler.ts:6-25`,
      `decentraland/catalyst:src/logic/peer-cluster/component.ts:123-125`,
      `decentraland/catalyst:src/Environment.ts:269-270`.
    - Severity: P2 (by design).

16. **CS-16 · `/queries/erc721` · Category and Body Shape traits.** Status: MATCH, no work.
    - Upstream: `Rarity`, `Category` (always, `{"trait_type":"Category"}` without a category), one `Tag`
      per tag, and no `Body Shape`. Capture: `GET /content/queries/erc721/137/<contract>/1/5` for an epic
      `top_head` wearable with `BaseMale` and `BaseFemale` representations
      (`captures/content-sync/CS-16-erc721-wearable.json`). The undefined-`Category` case was not captured.
    - dcl-one: the same traits; it emits no Body Shape trait
      (`services/catalyst/content/src/validator/erc721.rs` `format_erc721_entity`; test
      `validator::tests` `the_erc721_route_formats_the_active_wearable`).
    - Severity: none (closed).

17. **CS-17 · `/queries/erc721` · edge values.** Status: MATCH, no work.
    - Upstream: an unknown rarity gives `/undefined`; a falsy `rarity` (`null`, `""`) is 400 `Wearable is not
      standard.`; no `name` key without an i18n text; `emoteDataADR74 ?? data`; `chainId` must match `^\d+$`
      (`decentraland/catalyst:src/logic/entities/component.ts:59-63,159-180`,
      `decentraland/catalyst:src/controllers/handlers/get-erc721-entity-handler.ts:19-38`).
    - dcl-one: the same (`services/catalyst/content/src/validator/erc721.rs`; tests `edge_values_follow_upstream`,
      `validator::tests` `the_erc721_route_formats_the_active_wearable`). Upstream's 500 on an item without
      `i18n`, `tags` or `representations` is not reproduced (section 11).
    - Severity: none (closed).

18. **CS-18 · `/queries/items/*` · message for a missing blob, and `?width=`.** Status: MATCH, no work.
    - Upstream: a missing blob is `Entity has no thumbnail.` or `Entity has no image.`; `?width=` is ignored
      (`decentraland/catalyst:src/controllers/handlers/get-entity-thumbnail-handler.ts:33-35`,
      `get-entity-image-handler.ts:33-35`).
    - dcl-one: the same messages; a `width` that is not a served size is ignored and the stored file served
      (`services/catalyst/content/src/entity_thumbnail.rs` `requested_width`, `serve_content_blob`; tests
      `a_missing_blob_answers_the_routes_message`, `an_items_thumbnail_and_image_are_served_by_pointer`).
    - Severity: none (closed).

19. **CS-19 · history timestamps · double vs integer.**
    - Gap: upstream emits `date_part(...) * 1000` doubles as they are; dcl-one truncates them to
      integers.
    - Expected: capture `/deployments` and `/pointer-changes` from a production peer and compare
      `entityTimestamp` and `localTimestamp` for rows stamped with sub-millisecond precision.
    - Where: `services/catalyst/content/src/queries.rs:580-590,758-768`.
    - Severity: P2.

20. **CS-20 · `/challenge` · stable value and format.** Status: MATCH, no work.
    - Upstream: one bare `uuidv4()` per process (`decentraland/catalyst:src/logic/peer-cluster/component.ts:51,127-129`).
    - dcl-one: one bare UUID v4 per process in `ContentState` (`services/catalyst/content/src/lib.rs`,
      `status.rs`); the console's `challenge/refresh` replaces it. Pinned in `content/src/tests.rs`
      (`challenge_is_one_uuid_per_process`).
    - Severity: none (closed).

21. **CS-21 · `/deployments` and `/pointer-changes` · validation order.** Status: MATCH, no work.
    - Upstream: entity types, `offset`, the filter caps (`/deployments`), then the sort field and order
      (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:55-105`).
    - dcl-one: the same (`deployments.rs`, `pointer_changes.rs`; test `offset_is_checked_before_the_caps_and_the_sort`).
    - Severity: none (closed).

22. **CS-22 · `/deployments` · `metadata: null`.** Status: MATCH, no work.
    - Upstream: a falsy `metadata` leaves the key out
      (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:151-153`).
    - dcl-one: the same (`services/catalyst/content/src/queries.rs` `js_truthy`; test `js_truthy_reads_as_javascript`).
    - Severity: none (closed).

23. **CS-23 · all routes · trailing slash and path case.** Status: MATCH, no work.
    - Capture: `GET /content/deployments/?limit=1` and `GET /content/Deployments?limit=1` both answer
      `200` with the deployments page (`captures/content-sync/CS-23-trailing-slash-deployments.json`,
      `CS-23-path-case-deployments.json`).
    - dcl-one: the trailing slash is trimmed for the whole app (`services/catalyst/src/serve.rs`,
      `NormalizePath`); `/content/Deployments` is routed as `/content/deployments` by
      `services/catalyst/content/src/routing.rs` `upstream_routing`, as CR-16 (test
      `content_paths_match_case_insensitively`). Only paths under `/content/` are rewritten.
    - Severity: none (closed).

24. **CS-24 · `/deployments`, `/pointer-changes`, snapshots · every listed content hash is served.** Status: MATCH, no work.
    - Upstream: a synced entity is stored with every content file it lists
      (`decentraland/catalyst:src/logic/batch-deployer/component.ts:87`, `downloadEntityAndContentFiles`
      from `@dcl/snapshots-fetcher`, which is not in the clone), so a peer syncing from it can fetch
      each hash its history and snapshots name through `GET /contents/{hash}`.
    - dcl-one: sync and the read-through download every content file of an entity, a profile's
      `face.png`, `face128.png`, `face256.png` and `body.png` included
      (`services/catalyst/content/src/deploy_remote_entity.rs` `extract_content_hashes`; test
      `profiles_fetch_every_avatar_render`). A hash this node lists but does not hold (a database
      synced before the renders were stored, or a lost blob) is fetched from the sync sources on its
      first `GET /contents/{hash}` when `READ_THROUGH_ENABLED` is on, as `NODE_POSTURE=mirror` sets it
      (`services/catalyst/content/src/read_through.rs` `fill_content`; test
      `a_profile_render_is_stored_and_a_missing_one_is_filled_on_read`). Until that read,
      `/available-content` answers `false` for it. No background job re-fetches the content files of
      deployments already stored.
    - Severity: none (closed).

## Endpoints dcl-one serves in this area that upstream does not

- `GET /snapshot` and `GET /snapshot/{type}` (`services/catalyst/content/src/snapshots.rs:97-124`):
  - the legacy catalyst 3.x shapes `{hash, lastIncludedDeploymentTimestamp, entities}` and
    `{hash, lastIncludedDeploymentTimestamp}`;
  - 400 `Unrecognized type: <type>`;
  - 503 `Snapshot not yet created`.
- `GET /scenes` (`services/catalyst/content/src/lib.rs:618-621`): `{"scenes":[],"total":0}`, the
  shape of the worlds-content-server listing.
- `GET`/`HEAD /lambdas/collections/contents/{pointer}/thumbnail` and `/image`
  (`services/catalyst/content/src/lib.rs:701-709`):
  - the same handlers as `/queries/items/*`, served with bytes, not a redirect;
  - lamb2 at the pinned revision has no `/collections/contents/*` route.
- `?width=` derivatives on the thumbnail routes for the served sizes 80, 160, 320, 640 and 960; any
  other value is ignored as upstream ignores the parameter (`services/catalyst/content/src/entity_thumbnail.rs`).
- The 503 on `/deployments` when the query runs past its time budget
  (`services/catalyst/content/src/deployments.rs:75-85`).

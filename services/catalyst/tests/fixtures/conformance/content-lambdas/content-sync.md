# Content server: sync, history, status and queries

Area: the content-server routes that peers sync from and that clients use to read history,
status and item media: `/deployments`, `/pointer-changes`, `/failed-deployments`, `/snapshots`,
`/status`, `/challenge`, `/queries/items/*`, `/queries/erc721/*`.

Pinned revisions:

- Upstream content server: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21). It serves no route in this area.
- dcl-one: `dcl-one-conformance` @ `f2f04942` (origin/main, 2026-11-04), Rust workspace `services/catalyst`.

Everything below is from reading the code. Nothing was built, run or captured, and no line is
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
  mounts the same routes at the root and under `/content` (`services/catalyst/content/src/lib.rs:705-706`).
- **Error envelope.** Both trees use `{"error": "<message>"}`:
  - upstream `decentraland/catalyst:src/controllers/middlewares.ts:41-82`;
  - dcl-one `services/catalyst/content/src/errors.rs` (`json_error`).
  - Both map an unexpected error to 500 `{"error":"Internal Server Error"}`.
  - dcl-one's `AppError::ServiceUnavailable` renders the bare message, with no `Retry-After` header.
- **Unknown paths.** dcl-one's fallback is 404 with the `text/plain` body `Not found`
  (`services/catalyst/content/src/lib.rs:618-620`). The upstream 404 body comes from
  `@dcl/http-server`, which is not in the clone.
- **Trailing slash and path case.** dcl-one (axum) matches paths exactly, so `/deployments/` and
  `/Deployments` both return the fallback 404. Upstream's router is the `@dcl/http-server`
  koa-router derivative, and its `strict`/`sensitive` defaults were not read; see backlog CS-23.
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
| 1 | GET | `/deployments` | `/content` | `/deployments`, `/content/deployments` → `services/catalyst/content/src/deployments.rs:37` (route `services/catalyst/content/src/lib.rs:647`) | GAP |
| 2 | GET | `/pointer-changes` | `/content` | `services/catalyst/content/src/pointer_changes.rs:23` (route `lib.rs:652-655`) | DIVERGENT |
| 3 | GET | `/failed-deployments` | `/content` | `services/catalyst/content/src/failed_deployments.rs:14` (route `lib.rs:648-651`) | DIVERGENT |
| 4 | GET | `/snapshots` | `/content` | `services/catalyst/content/src/snapshots.rs:77` (route `lib.rs:656`) | DIVERGENT |
| 5 | GET | `/status` | `/content` | `services/catalyst/content/src/status.rs:12` (route `lib.rs:659`) | DIVERGENT |
| 6 | GET | `/challenge` | `/content` | `services/catalyst/content/src/status.rs:59` (route `lib.rs:624`) | DIVERGENT |
| 7 | GET | `/queries/items/:pointer/thumbnail` | `/content` | `services/catalyst/content/src/entity_thumbnail.rs:91` (route `lib.rs:660-664`) | DIVERGENT |
| 8 | HEAD | `/queries/items/:pointer/thumbnail` | `/content` | same handler | DIVERGENT |
| 9 | GET | `/queries/items/:pointer/image` | `/content` | `services/catalyst/content/src/entity_thumbnail.rs:121` (route `lib.rs:665-668`) | DIVERGENT |
| 10 | HEAD | `/queries/items/:pointer/image` | `/content` | same handler | DIVERGENT |
| 11 | GET | `/queries/erc721/:chainId/:contract/:option/:emission?` | `/content` | `services/catalyst/content/src/validator/erc721.rs:295`, as two routes with and without `{emission}` (`lib.rs:669-676`) | DIVERGENT |

Counts: 11 endpoints. MATCH 0, GAP 1, DIVERGENT 10, MISSING 0. Every route exists in dcl-one.
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
| `entityType` (repeatable; `entityType[0]=` and `entityType[]=` also accepted) | `qsGetArray`: a missing key or a single empty value gives `[]`; an empty element inside a multi-value list is kept; non-string elements are dropped (`query-params/component.ts:21-31`). So `?entityType=` adds no filter, and `?entityType=scene&entityType=` is a 400 (`parseEntityType('')` is undefined). `parseEntityType` trims and lowercases, then accepts the exact type or the type plus `s`, except for `outfits` (`types.ts:212-221`). Types: `scene`, `profile`, `wearable`, `store`, `emote`, `outfits`. | `qs_get_array` keeps every value, a single empty one included (`shared/src/query_params.rs:40-42`), so `?entityType=` gives `[""]` and a **400**. `parse_entity_type` also strips one trailing `s`, so it accepts `outfitss` (`shared/src/query_params.rs:56-62`). |
| `entityId` (repeatable) | Case-sensitive `entity_id = ANY(...)`. A single empty value (`?entityId=`) is absent, so no filter; an empty element in a list (`entityId=a&entityId=`) is kept. | Case-sensitive. **A single empty value is kept**: `?entityId=` filters on `''` and returns nothing. A list matches upstream. |
| `pointer` (repeatable) | Lowercased, then `entity_pointers && $pointers` (`deployments-repository/component.ts:118-121`). A single empty value (`?pointer=`) is absent, so no filter; an empty element in a list is kept. | Lowercased (`deployments.rs:132-135`). **A single empty value is kept**: `?pointer=` filters on `''` and returns nothing. A list matches upstream. |
| `deployedBy` | **Not read by the handler.** `DeploymentFilters.deployedBy` exists in `deployment-types.ts:7`, but the handler never fills it. | Read, lowercased, and filtered with `LOWER(deployer_address) = ANY(...)` (`deployments.rs:137-140`, `queries.rs:540-543`). Also echoed in `filters.deployedBy` and in `next`. |
| `onlyCurrentlyPointed` | `qsGetBoolean`: `undefined` when missing or empty; otherwise `=== 'true'`. When true, adds `deleter_deployment IS NULL`. | `qs_get_bool`: an empty value gives `Some(false)`, which is echoed as `"onlyCurrentlyPointed": false` where upstream omits the key. |
| `from`, `to` | `parseInt(v, 10)`, so `10abc` → 10 and `1.5` → 1. A falsy value (0) adds no bound (`deployments-repository/component.ts:156-188`). | `str::parse::<i64>`, so `10abc` and `1.5` are ignored. `0` adds a `>= 0` bound, which returns the same rows. |
| `offset` | `parseInt`. More than 5000 → 400. A negative value is treated as 0 (`logic/deployments/component.ts:237-239`). | Same, at `deployments.rs:188-192` and `queries.rs:30-35`. |
| `limit` | `parseInt`. A value of 1..500 is used as given; anything else (missing, 0, negative, more than 500) becomes **500** (`logic/deployments/component.ts:235-242`). | Same (`queries.rs:21-28`). |
| `lastId` | Lowercased. Only used as a tiebreak on the bound in the direction of travel (`createOrClause`, `deployments-repository/component.ts:145-241`). | Lowercased. The tuple cursor is `(ts, lower(id)) < (ts of the lastId row, or T, lower(lastId))` (`queries.rs:374-453`). It returns the same rows, and it also handles sub-millisecond ties. |
| `fields` | Comma split, **no trim**. Only `content`, `pointers`, `metadata` and `auditInfo` are kept, and unknown values are dropped silently. Empty or whitespace means the default `pointers,content,metadata`. | Same, but each item is trimmed, so `pointers, content` keeps `content` where upstream drops it (`deployments.rs:158-165`). |
| `sortingField` | camelCase → snake_case; must be `local_timestamp` or `entity_timestamp`. An empty value is ignored. Any other value → 400 `Found an unrecognized sort field param`. | Same mapping. **An empty value gives a 400**, because `camel_to_snake("")` is not a valid field (`deployments.rs:167-176`). |
| `sortingOrder` | `ASC` or `DESC`, case-sensitive. An empty value is ignored (`|| undefined`). | Case-sensitive. **An empty value gives a 400** (`deployments.rs:178-186`). |

**Filter caps and limits**

- The cap is 1000 values per filter for `entityId`, `pointer` and `entityType`. Both trees return
  the same message.
- `deployedBy` has no cap in dcl-one.
- `qs` has a parameter limit of 2000; dcl-one keeps the first 2000 pairs (`shared/src/query_params.rs:9,25`).

**Validation order**

- Upstream: entityType → offset → too many → sortingField → sortingOrder.
- dcl-one: entityType → too many → sortingField → sortingOrder → offset.
- The two only differ when a request has more than one error.

**Auth and rate limits**

- No auth. No rate limit in the app; the gateway limit above applies.
- Upstream runs the query through `sequentialExecutor` (`get-deployments-handler.ts:125-128`).
- dcl-one memoizes the body for 5 s, keyed by the sorted query string, clears the memo on commit,
  and applies a server-side timeout (`deployments.rs:37-85`).

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | Example below. | Upstream: `content-type: application/json` (http-server default). dcl-one adds `cache-control: max-age=5` and `x-cache: HIT\|MISS`. | `get-deployments-handler.ts:136-139` | `deployments.rs:62-68` | DIVERGENT: extra headers; denylisted entities are not removed; `deployedBy` filter; empty-value filters. Field shape: see notes. |
| Unknown entity type | 400 | `{"error":"Found an unrecognized entity type"}` | | `get-deployments-handler.ts:55-60` | `deployments.rs:123-128` | MATCH. `?entityType=` gives a 400 in dcl-one but a 200 upstream. |
| `offset` > 5000 | 400 | `{"error":"Offset can't be higher than 5000. Please use the 'next' property for pagination."}` | | `:62-67` | `:188-192` | MATCH |
| More than 1000 values in a filter | 400 | `{"error":"Too many filter values; the maximum allowed per filter is 1000"}` | | `:69-78` | `:142-149` | MATCH |
| Bad `sortingField` | 400 | `{"error":"Found an unrecognized sort field param"}` | | `:92-98` | `:167-176` | MATCH. An empty value differs. |
| Bad `sortingOrder` | 400 | `{"error":"Found an unrecognized sort order param"}` | | `:99-105` | `:178-186` | MATCH. An empty value differs. |
| Repeated `sortingOrder` | 400 upstream (the value is an array, so `asEnumValue` returns `unknown`) | as above | | `utils.ts:36-45` | dcl-one reads the first value and returns 200 | DIVERGENT, P2 |
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
  - dcl-one omits it when the column is NULL but emits `"metadata": null` when the stored value is
    `{"v": null}` (`queries.rs:108-110`).
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
- `moreData` is computed before the denylist filter upstream. dcl-one has no denylist filter here.

**`next` link format**:

- Upstream (`get-deployments-handler.ts:161-196`, `query-params/component.ts:40-74`):
  - `qs.stringify(..., {arrayFormat:'repeat'})` with falsy values dropped and array keys
    singularised.
  - Key order follows insertion: `pointer*, entityType*, entityId*, onlyCurrentlyPointed (if true),
    from, to, fields (if not the default array instance), sortingField, sortingOrder, lastId, limit (raw request value, if truthy)`.
  - The timestamp written to `from` (ASC) or `to` (DESC) is the last row's `localTimestamp` or
    `entityTimestamp`, depending on the sort field.
- dcl-one (`deployments.rs:230-303`, `shared/src/query_params.rs:74-92`):
  - The same pairs, plus `deployedBy`.
  - **Keys are sorted alphabetically.**
  - `fields` is left out when the set equals the default, even when the client passed it explicitly.
  - Percent-encoding uses the same unreserved set as `qs` (RFC 3986).
- Following `next` gives the same rows in both trees. The string differs (CS-07).

### Data dependency

- The `content.deployments` and `content.content_files` rows that the peer sync writes
  (`content/src/sync/*`), plus the operator denylist.
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
  Parsing matches §1 and has the same empty-value differences:
  - `?entityType=` → dcl-one 400 (CS-05);
  - an empty `sortingField` or `sortingOrder` → dcl-one 400 (CS-06).
- **No cap.** There is no 1000-value cap on `entityType` in either tree.
- **`includeAuthChain`.** Upstream parses it (default `false`) and only echoes it in `filters`.
  `authChain` is **always** included in each delta. dcl-one does the same
  (`pointer_changes.rs:58`, `queries.rs:656-668`).
- **Validation order.** Both trees check entityType → offset → sortingField → sortingOrder.
  dcl-one checks offset last (`pointer_changes.rs:81-85`).
- **No auth.** dcl-one memoizes for 5 s (`pointer_changes.rs:23-41`).

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | Example below. | `content-type: application/json` | `pointer-changes-handler.ts:90-94` | `pointer_changes.rs:36-40` | DIVERGENT: `next` drops `includeAuthChain=true`; `pagination.lastId` is echoed; locally tombstoned scenes are left out. |
| Unknown entity type | 400 | `{"error":"Found an unrecognized entity type"}` | | `:41-43` | `:46-52` | MATCH (except the empty-value case) |
| `offset` > 5000 | 400 | `{"error":"Offset can't be higher than 5000. Please use the 'next' property for pagination."}` | | `:45-47` | `:81-85` | MATCH |
| Bad `sortingField` | 400 | `{"error":"Found an unrecognized sort field param"}` | | `:51-57` | `:60-69` | MATCH (except the empty-value case) |
| Bad `sortingOrder` | 400 | `{"error":"Found an unrecognized sort order param"}` | | `:58-64` | `:71-79` | MATCH (except the empty-value case) |
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
- dcl-one (`pointer_changes.rs:110-125`):
  - The same pairs and order, **without `includeAuthChain=true`**.
  - The values are not percent-encoded. Entity ids and type names are URL-safe, so the result is
    the same.
- The ASC behaviour matches upstream. dcl-one's own sync client builds its own keyset URL and does
  not follow `next` (`content/src/sync/pointer_changes.rs:200-221`).

**Row selection**:

- Both trees order by `ORDER BY <ts> <order>, LOWER(entity_id) <order>` with limit+1.
- Both compute `moreData` before the denylist filter and drop denylisted ids after truncating.
- dcl-one also drops deployments of entities this node tombstoned (local unpublish), and the
  entities those overwrote (`queries.rs:713-728`). Upstream has no tombstones.

### Data dependency

- `content.deployments`, the denylist, and `content.local_entities` (dcl-one only).
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
| Success | 200 | Bare array; example below. | `content-type: application/json` | `failed-deployments-handler.ts:24-37` | `failed_deployments.rs:33-34`, `queries.rs:798-854` | DIVERGENT: order, `snapshotHash`, `reason` values. |
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
  - dcl-one reads the table with `ORDER BY failure_time, entity_id` (`queries.rs:828`).
- **`snapshotHash`.**
  - Upstream: absent for failures that did not come from a snapshot. Those are cached but not
    persisted (`component.ts:241-254`).
  - dcl-one: always present, `""` for those failures (`sync_deployer.rs:1168`, column `NOT NULL`
    in `schemas/00_content.sql:106`).
- **`reason`.**
  - Upstream: only `"Deployment error"` (`FailureReason`, `adapters/failed-deployments/types.ts:5-7`).
  - dcl-one: `"No entity"` is passed through; every other stored reason is mapped to
    `"Deployment error"` (`queries.rs:842-846`).
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
| Snapshots exist | 200 | Array of `SnapshotMetadata`; example below. | `content-type: application/json` | `get-snapshots-handler.ts:18-21` | `snapshots.rs:77-85` | DIVERGENT: array order, possible empty `hash`, `generationTimestamp` value |
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
- dcl-one: `ORDER BY end_timestamp DESC`, newest first (`queries.rs:931-970`). The generator's own
  `snapshot_gen::all_snapshots` is ordered oldest first (`snapshot_gen.rs:790-813`), but the route
  does not use it.

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
  - the start of the oldest open transaction, capped at `now()` (`snapshot_gen.rs:597-610`);
  - **or 0** (epoch) when a deleter repair committed while the window was being read
    (`snapshot_gen.rs:660-665`). A peer reads that value as "generated at 1970".

**`hash`**:

- Upstream lists only stored hashes.
- dcl-one lists a NULL `hash` column as `""` (`queries.rs:960`). The column is nullable
  (`schemas/00_content.sql:119`). The current generator always inserts a hash; NULL rows can only
  be legacy rows. A peer that receives `""` fetches `/contents/` and fails.

**When regeneration happens**:

- Upstream regenerates a window when:
  - it is not covered;
  - several snapshots cover it;
  - a file is missing;
  - it is more than one month old and has fewer active entities than `numberOfEntities`; or
  - it is outdated.
- dcl-one regenerates when it is outdated or when the bytes fail the hash check. **It has no
  "inactive entities after a month" rule**: windows that list overwritten entities are rebuilt only
  by `POST /admin/api/content/snapshots/regenerate` (`snapshot_gen.rs:19-31`).
- A peer that reads the extra entities still converges, because newer entities win. The cost is
  bootstrap work.

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
| Normal | 200 | Example below. | `content-type: application/json` | `status-handler.ts:6-25` | `status.rs:12-49` | DIVERGENT: field values and extra fields |
| Upstream: a status-capable component rejected. No component implements `getComponentStatus` at this revision (`logic/status-checks.ts:16-40`), so this branch never fires. | 503 | same body | | `status-handler.ts:14` | none | n/a |
| dcl-one: sync state `Failed` | 503 | same body with `"synchronizationState":"Failed"` | | none | `status.rs:42-46` | dcl-one only |

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
| `version` | `process.env.CURRENT_VERSION ?? 'Unknown'` (`Environment.ts:270`) | `CARGO_PKG_VERSION` (`content/src/lib.rs:211`) |
| `commitHash` | `process.env.COMMIT_HASH ?? 'Unknown'` (`Environment.ts:269`) | The configured or build-time hash, otherwise **`"unknown"`** in lowercase (`status.rs:26-30`) |
| `ethNetwork` | `ETH_NETWORK` config | `config.node.eth_network` |
| `synchronizationStatus.lastSyncWithDAO` | Time of the last successful DAO server-list refresh; 0 before the first one (`logic/peer-cluster/component.ts:43,79,123-125`) | **The current time** on every call (`status.rs:33`) |
| `synchronizationStatus.synchronizationState` | `"Bootstrapping"` or `"Syncing"` (`sync-orchestrator/types.ts:7-10`) | `"Bootstrapping"`, `"Syncing"` (which also covers partially synced) or **`"Failed"`** (`content/src/lib.rs:368-374`) |
| extra keys | none | `syncFrontier`, `lastHeartbeat` and `up` when the sync worker reports them (`wire_types.rs:106-120`) |

### Data dependency

- The process configuration and the sync worker state.
- dcl-one has no DAO list refresh to time, so `lastSyncWithDAO` has no source.

---

## 6. GET /challenge

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Always | 200 | `{"challengeText": "<string>"}` | `get-challenge-handler.ts:4-12` | `status.rs:59-63` | DIVERGENT |

| | upstream | dcl-one |
|---|---|---|
| Value | One `uuidv4()` per process, **the same on every call** (`logic/peer-cluster/component.ts:51,127-129`) | **A new value per call** (`status.rs:53-55`) |
| Format | A bare UUID, e.g. `"8c9e...-..."` | `"dcl-crypto-<uuid>"` |
| Purpose | "a peer that fetches `/challenge` from every DAO server and finds a matching value has located its own process" (`logic/peer-cluster/types.ts:8-13`) | — |

A per-call value can never match a second fetch. A tool that identifies a node by comparing two
reads of `/challenge` treats dcl-one as two different processes.

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
| The blob is not in storage | 404 | upstream `{"error":"Entity has no thumbnail."}` / `"Entity has no image."` | | `:33-35` | `:171-173`: `{"error":"Content not found."}` | DIVERGENT, P2 |
| Unsatisfiable `Range` | 416 | empty | `Content-Range: bytes */<total>`, `Access-Control-Expose-Headers: Content-Range` | `:37-42`, `utils.ts:187-195` | `:182-192` | MATCH |
| Satisfiable `Range` | 206 | the byte range (empty for HEAD) | the content headers plus `Content-Range: bytes s-e/total` and `Content-Length` | `:44-54`, `utils.ts:197-211` | `:193-209` | MATCH |
| Plain GET or HEAD | 200 | the bytes (empty for HEAD) | `Content-Type` (sniffed; `application/octet-stream` fallback), `ETag: "<hash>"`, `Access-Control-Expose-Headers: ETag, Content-Range, Accept-Ranges, Content-Length`, `Accept-Ranges: bytes`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Content-Length`. Upstream also sends `Content-Encoding` when the blob is stored compressed. | `utils.ts:115-154` | `:210-231`, `formatters.rs:85-102` | MATCH. Sniffing: upstream uses `file-type` on the stream; dcl-one sniffs the first 32 bytes (`get_content::detect_content_type`). These agree for PNG, JPEG and WebP. dcl-one never sends `Content-Encoding`. |
| `?width=` on the thumbnail route, not 80/160/320/640/960 | — | upstream ignores it and serves 200 | | — | `:98-104`: 400 `text/plain` `width must be 80, 160, 320, 640 or 960`. A non-numeric value is rejected by axum `Query` with its plain-text 400. | DIVERGENT, P2 |
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
| `chainId` is not all digits | 400 | `{"error":"Invalid chainId '<chainId>'"}` | `:19-21` | `:299-300` (`u64::parse`) | MATCH, except `+1`: Rust accepts it, upstream rejects it (P2) |
| Unknown chain | 400 | same | `:23-27` | `:198-210,301`. Table: 1 ethereum, 3 ropsten, 4 rinkeby, 5 goerli, 137 matic, 80001 mumbai, 80002 amoy, 11155111 sepolia. | MATCH if `@dcl/schemas` maps the same set (not checked: the package is not in the clone) |
| No active entity, entity denylisted, or no metadata | 404 | `{"error":"Entity does not exist"}` | `:32-34` | `:308-316` | MATCH |
| `metadata.rarity` is falsy | 400 | `{"error":"Wearable is not standard."}` | `:36-38` (`!rarity`) | `:317-319` (`is_none()`) | DIVERGENT, P2: `"rarity": null` or `""` → upstream 400; dcl-one 200 with `"value": ""` |
| Success | 200 | Example below. | `:40-43` | `:322-327` | DIVERGENT: see the field list |
| `metadata.i18n` is missing | upstream 500 (`i18ns.filter` on undefined) | | `component.ts:159-162` | `"name": null`, 200 | DIVERGENT, P2 |

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
  - dcl-one: `DCL Wearable <em>/0` (`erc721.rs:77-83`).
- **Item data source.**
  - Upstream reads `emoteDataADR74 ?? data` (`component.ts:178-180`).
  - dcl-one reads `data` first, then `emoteDataADR74` (`erc721.rs:145-165`).
  - These differ only for an entity that has both.
- **`Category` trait.**
  - Upstream always emits it, even when the category is undefined; `{"trait_type":"Category"}`
    then has no `value` key.
  - dcl-one leaves it out (`erc721.rs:98-104`).
- **`Body Shape` traits.**
  - Upstream compares each body shape with `BodyShape[BodyShape.MALE]` (`component.ts:164-176`).
    If `BodyShape` is a string enum (which has no reverse mapping), that expression is `undefined`
    and upstream emits **no** Body Shape traits. Whether it is a string enum is not checked here,
    because `@dcl/schemas` is not in the clone.
  - dcl-one emits `BaseFemale` and then `BaseMale`, sorted with a `BTreeSet`, for full URNs and
    short names (`erc721.rs:167-196`). Upstream's `Set` order is insertion order.
- **`image` and `thumbnail`.**
  - Upstream: `new URL('contents/<hash>', CONTENT_SERVER_ADDRESS)`. Without a trailing slash, the
    last segment of the address is replaced.
  - dcl-one: `<content_public_url without trailing '/'>/contents/<hash>`, where the default
    `content_public_url` is `<own_url>/content/` (`content/src/lib.rs:214-215`).
  - They agree when the upstream address ends in `/`.
  - Both omit the key when the hash is missing.
- **`name`.**
  - Upstream: the `en` entry, otherwise the first entry; the key is absent if the text is undefined.
  - dcl-one: the same choice, but `"name": null` instead of an absent key (`erc721.rs:23-33`).

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

For these steps dcl-one serves every field that upstream serves, in a compatible shape. The ways a
peer would see a difference:

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

1. **CS-01 · `/deployments` · denylisted entities are served.**
   - Gap: dcl-one returns deployments of denylisted entity ids. Upstream drops them after the
     `moreData` check.
   - Expected: filter `denylist.contains(entityId)` on the truncated page, keeping
     `moreData` computed before the filter (`decentraland/catalyst:src/logic/deployments/component.ts:316-318`).
   - Where: `services/catalyst/content/src/deployments.rs:87-91` (`fetch_deployments_response`).
   - Severity: P0 (moderated content stays visible through history).

2. **CS-02 · `/deployments` · empty filter values filter on `''`.**
   - Gap: `?pointer=` or `?entityId=` returns no rows. Upstream treats a single empty value as absent
     (`decentraland/catalyst:src/logic/query-params/component.ts:21-31`).
   - Expected: `qs_get_array` returns `[]` when the key is missing or holds exactly one empty value (the bare
     key `pointer` included), and keeps empty strings inside a multi-value list, as `qsGetArray` does. CR-9
     relies on the same rule.
   - Where: `services/catalyst/shared/src/query_params.rs:40-42`. This is shared, so check the other
     callers.
   - Severity: P1.

3. **CS-03 · `/deployments` · the `deployedBy` filter is dcl-one only.**
   - Gap: upstream ignores `deployedBy`; dcl-one filters on it and echoes it in `filters` and `next`.
   - Expected: either ignore it, or keep it as a documented superset. A client that passes it
     against upstream gets unfiltered rows.
   - Where: `services/catalyst/content/src/deployments.rs:137-140,260-262`,
     `services/catalyst/content/src/queries.rs:540-543`, `services/catalyst/content/src/wire_types.rs:63-64`.
   - Severity: P2.

4. **CS-04 · `/deployments` · extra response headers.**
   - Gap: dcl-one adds `cache-control: max-age=5` and `x-cache`. Upstream sends neither.
   - Expected: no `cache-control`, unless the CDN behaviour is meant to differ
     (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:136-139`).
   - Where: `services/catalyst/content/src/deployments.rs:62-68`.
   - Severity: P1 (CDN and proxy caching differ).

5. **CS-05 · `/deployments` and `/pointer-changes` · `?entityType=` gives a 400.**
   - Gap: an empty value is treated as an unknown type. Upstream returns 200 with no type filter.
   - Expected: treat a single empty value as absent before `parse_entity_type`, as in CS-02; an empty element
     inside a list stays a 400.
   - Where: `services/catalyst/content/src/deployments.rs:122-128`, `services/catalyst/content/src/pointer_changes.rs:46-52`.
   - Severity: P2.

6. **CS-06 · `/deployments` and `/pointer-changes` · an empty `sortingField` or `sortingOrder` gives a 400.**
   - Gap: upstream treats `''` as missing (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:44-50`).
   - Expected: treat an empty string as `None`.
   - Where: `services/catalyst/content/src/deployments.rs:167-186`, `services/catalyst/content/src/pointer_changes.rs:60-79`.
   - Severity: P2.

7. **CS-07 · `/deployments` · `pagination.next` key order and `fields` rule.**
   - Gap: dcl-one sorts keys alphabetically, and leaves `fields` out when the set equals the
     default even if the client sent it.
   - Expected: the upstream order `pointer, entityType, entityId, onlyCurrentlyPointed, from, to,
     fields, sortingField, sortingOrder, lastId, limit`
     (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:185-196`).
   - Where: `services/catalyst/content/src/deployments.rs:230-303`. Use an ordered list, not a `HashMap`.
   - Severity: P2.

8. **CS-08 · `/deployments` and `/pointer-changes` · number and boolean parsing.**
   - Gap: `parseInt` semantics (`10abc` → 10, `1.5` → 1) are not reproduced, and an empty
     `onlyCurrentlyPointed` gives `false` instead of absent. `fields` items are trimmed; upstream
     does not trim them.
   - Expected: upstream `qsGetNumber` and `qsGetBoolean` (`decentraland/catalyst:src/logic/query-params/component.ts:32-39`).
   - Where: `services/catalyst/shared/src/query_params.rs:48-54`, `services/catalyst/content/src/deployments.rs:158-165`.
   - Severity: P2.

9. **CS-09 · `/pointer-changes` · `next` drops `includeAuthChain=true`.**
   - Expected: append `includeAuthChain=true` after `to` when the flag is true
     (`decentraland/catalyst:src/controllers/handlers/pointer-changes-handler.ts:97-114`).
   - Where: `services/catalyst/content/src/pointer_changes.rs:114-124`.
   - Severity: P2.

10. **CS-10 · `/failed-deployments` · `snapshotHash`, `reason` and order.**
    - Gap:
      - `snapshotHash: ""` where upstream leaves the key out;
      - the `"No entity"` reason is passed through;
      - rows are sorted by time where upstream uses insertion order.
    - Expected: leave out `snapshotHash` when it is empty, and always use `"Deployment error"`
      (`decentraland/catalyst:src/adapters/failed-deployments/types.ts:5-25`).
    - Where: `services/catalyst/content/src/queries.rs:798-854`, `services/catalyst/content/src/wire_types.rs:162-174`.
    - Severity: P2.

11. **CS-11 · `/snapshots` · array order.**
    - Gap: dcl-one is newest first. Upstream is in division order (oldest window first).
    - Expected: `ORDER BY init_timestamp, end_timestamp`, as `snapshot_gen::all_snapshots` already does
      (`decentraland/catalyst:src/logic/snapshots/component.ts:67-164`).
    - Where: `services/catalyst/content/src/queries.rs:931-970`, or route `services/catalyst/content/src/snapshots.rs:66`
      to `snapshot_gen::all_snapshots`.
    - Severity: P1.

12. **CS-12 · `/snapshots` · `generationTimestamp` 0 and empty `hash`.**
    - Gap: a window can be listed with `generationTimestamp: 0` (the deleter-repair mark), and a
      NULL-hash row is listed as `hash: ""`.
    - Expected: upstream lists only stored snapshots, each with the real generation time
      (`decentraland/catalyst:src/logic/snapshots/component.ts:39,126-133`).
    - Where: `services/catalyst/content/src/queries.rs:931-970` (skip rows with an empty hash),
      `services/catalyst/content/src/snapshot_gen.rs:660-665` (keep the internal "outdated" mark
      without publishing epoch, e.g. in a separate column).
    - Severity: P1.

13. **CS-13 · snapshot file bytes.**
    - Gap: `authChain` keys are sorted, `entityTimestamp` is an integer, and ties are broken by
      `entity_id`. For the same data the CID differs from an upstream node's, so peers re-process
      the file.
    - Expected: the upstream line is `JSON.stringify({entityId, entityType, pointers, authChain, entityTimestamp})`,
      with `authChain` in stored key order
      (`decentraland/catalyst:src/adapters/snapshots-repository/component.ts:9-33`,
      `decentraland/catalyst:src/logic/snapshots/component.ts:42-46`).
    - Where: `services/catalyst/content/src/snapshot_gen.rs:502-510,529`. This needs `preserve_order`
      or raw `json` text for `authChain`. The fixtures in `tests/fixtures/snapshots/` must be re-pinned.
    - Severity: P2. Sync is still correct; the cost is bandwidth and bootstrap work.

14. **CS-14 · snapshots · no "inactive entities after a month" regeneration.**
    - Gap: windows that list overwritten entities are rebuilt only on an admin request.
    - Expected: regenerate a window older than one month whose active count is below
      `numberOfEntities`, and set `replacedSnapshotHashes` to the old hash
      (`decentraland/catalyst:src/logic/snapshots/component.ts:81-87,124-125`).
    - Where: `services/catalyst/content/src/snapshot_gen.rs:375-400`.
    - Severity: P2.

15. **CS-15 · `/status` · field values.**
    - Gap:
      - `lastSyncWithDAO` is the current time;
      - `commitHash` falls back to `"unknown"`;
      - `version` is the crate version;
      - there are extra keys `syncFrontier`, `lastHeartbeat` and `up`;
      - it returns 503 when sync has `Failed`.
    - Expected: `lastSyncWithDAO` is the time of the last DAO-list refresh, the fallback is
      `"Unknown"`, the extra keys are absent, and the status is always 200 at this revision
      (`decentraland/catalyst:src/controllers/handlers/status-handler.ts:6-25`,
      `decentraland/catalyst:src/logic/peer-cluster/component.ts:123-125`,
      `decentraland/catalyst:src/Environment.ts:269-270`).
    - Where: `services/catalyst/content/src/status.rs:12-49`, `services/catalyst/content/src/wire_types.rs:106-120`.
    - Severity: P1 for `lastSyncWithDAO` and the 503 (health monitors read them); P2 for the rest.

16. **CS-16 · `/queries/erc721` · Category and Body Shape traits.**
    - Gap:
      - dcl-one leaves out `Category` when it is undefined (upstream emits `{"trait_type":"Category"}`);
      - dcl-one emits sorted Body Shape traits, where upstream likely emits none (string-enum
        reverse lookup, `decentraland/catalyst:src/logic/entities/component.ts:164-176`).
    - Expected: capture one wearable from a production peer to settle the Body Shape question,
      then match it.
    - Where: `services/catalyst/content/src/validator/erc721.rs:93-121,167-196`.
    - Severity: P1.

17. **CS-17 · `/queries/erc721` · edge values.**
    - Gap:
      - an unknown rarity gives `/0` (upstream `/undefined`);
      - `"rarity": null` or `""` is accepted (upstream returns 400 `Wearable is not standard.`);
      - `name: null` is emitted where upstream omits the key (or returns 500);
      - `data` is read before `emoteDataADR74`;
      - `+1` is accepted as a chainId.
    - Expected: `decentraland/catalyst:src/logic/entities/component.ts:59-63,159-180` and
      `decentraland/catalyst:src/controllers/handlers/get-erc721-entity-handler.ts:19-38`.
    - Where: `services/catalyst/content/src/validator/erc721.rs:23-33,77-83,145-165,299-319`.
    - Severity: P2.

18. **CS-18 · `/queries/items/*` · message for a missing blob, and `?width=`.**
    - Gap:
      - a missing blob gives `{"error":"Content not found."}` (upstream: `Entity has no thumbnail.`
        or `Entity has no image.`);
      - an invalid `?width=` gives a plain-text 400 where upstream ignores the parameter.
    - Expected: `decentraland/catalyst:src/controllers/handlers/get-entity-thumbnail-handler.ts:33-35`
      and `decentraland/catalyst:src/controllers/handlers/get-entity-image-handler.ts:33-35`.
    - Where: `services/catalyst/content/src/entity_thumbnail.rs:98-104,171-173`.
    - Severity: P2.

19. **CS-19 · history timestamps · double vs integer.**
    - Gap: upstream emits `date_part(...) * 1000` doubles as they are; dcl-one truncates them to
      integers.
    - Expected: capture `/deployments` and `/pointer-changes` from a production peer and compare
      `entityTimestamp` and `localTimestamp` for rows stamped with sub-millisecond precision.
    - Where: `services/catalyst/content/src/queries.rs:580-590,758-768`.
    - Severity: P2.

20. **CS-20 · `/challenge` · stable value and format.**
    - Gap: a new `dcl-crypto-<uuid>` value per call. Upstream returns one bare UUID per process.
    - Expected: generate it once per process (stored in `ContentState`) and drop the prefix
      (`decentraland/catalyst:src/logic/peer-cluster/component.ts:51,127-129`).
    - Where: `services/catalyst/content/src/status.rs:53-63`.
    - Severity: P1.

21. **CS-21 · `/deployments` and `/pointer-changes` · validation order.**
    - Gap: dcl-one checks `offset` after the sort checks (and after "too many" on `/deployments`).
    - Expected: the upstream order (`decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:55-105`).
    - Where: `services/catalyst/content/src/deployments.rs:121-192`, `services/catalyst/content/src/pointer_changes.rs:43-85`.
    - Severity: P2.

22. **CS-22 · `/deployments` · `metadata: null`.**
    - Gap: a stored `{"v": null}` is emitted as `"metadata": null`. Upstream leaves the key out.
    - Expected: `decentraland/catalyst:src/controllers/handlers/get-deployments-handler.ts:151-153`.
    - Where: `services/catalyst/content/src/queries.rs:108-110`, or skip `Value::Null` in projection.
    - Severity: P2.

23. **CS-23 · all routes · trailing slash and path case.**
    - Gap: dcl-one returns a plain-text 404 for `/deployments/` and `/Deployments`. The upstream
      behaviour depends on the `@dcl/http-server` router defaults, which were not read.
    - Expected: capture both against a production peer before changing anything.
    - Where: `services/catalyst/content/src/lib.rs:690-706`.
    - Severity: P2.

## Endpoints dcl-one serves in this area that upstream does not

- `GET /snapshot` and `GET /snapshot/{type}` (`services/catalyst/content/src/snapshots.rs:97-124`):
  - the legacy catalyst 3.x shapes `{hash, lastIncludedDeploymentTimestamp, entities}` and
    `{hash, lastIncludedDeploymentTimestamp}`;
  - 400 `Unrecognized type: <type>`;
  - 503 `Snapshot not yet created`.
- `GET /scenes` (`services/catalyst/content/src/lib.rs:613-616`): `{"scenes":[],"total":0}`, the
  shape of the worlds-content-server listing.
- `GET`/`HEAD /lambdas/collections/contents/{pointer}/thumbnail` and `/image`
  (`services/catalyst/content/src/lib.rs:696-704`):
  - the same handlers as `/queries/items/*`, served with bytes, not a redirect;
  - lamb2 at the pinned revision has no `/collections/contents/*` route.
- `?width=` derivatives on the thumbnail routes (`services/catalyst/content/src/entity_thumbnail.rs:235-301`).
- The 503 on `/deployments` when the query runs past its time budget
  (`services/catalyst/content/src/deployments.rs:75-85`).

# Content server: write path (`POST /entities`)

Area: content server write path. ID prefix `CW-`.

Pinned revisions:

- Upstream content server: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04). Paths below are relative to the repo root.
- `@dcl/content-validator` (`^8.1.0` in `package.json`) and `@dcl/crypto`, `@dcl/http-server` and `@dcl/rate-limiter-component` are npm dependencies. Their sources are not in the pinned tree. Validator strings were spot-checked against a local `decentraland/content-validator` clone @ `b1893d37` (2025-11-04). That clone is older than `^8.1.0`: it has no `createStagingValidator`. Every comparison here is static reading ("read"). Nothing was run.

Files read (upstream):
`src/controllers/routes.ts`, `middlewares.ts`, `errors.ts`, `multipart.ts`, `deployment-quota.ts`,
`deployment-memory-admission.ts`, `source-upload-admission.ts`, `client-source.ts`, `request-arrival.ts`,
`handlers/create-entity-handler.ts`, `src/logic/deployment-service/{component,server-validator,rate-limiter,errors}.ts`,
`src/logic/partial-deployments/{component,errors}.ts`, `src/logic/deployment-quota/{component,errors}.ts`,
`src/logic/entities/component.ts`, `src/logic/crypto/component.ts`, `src/adapters/{source-upload-limits,upload-budget,content-locks}/errors.ts`,
`src/adapters/content-validator/component.ts`, `src/Environment.ts`, `src/components.ts` (server, CORS, rate limiter), `src/service.ts`,
`docs/PARTIAL_UPLOADS.md`.

Files read (dcl-one):
`services/catalyst/content/src/{lib.rs,create_entity.rs,partial_deployments.rs,upload_admission.rs,writes.rs,write_deployer.rs,errors.rs,denylist.rs,unpublish_scene.rs,forward.rs,forward/status.rs,active_entities.rs,write_path_tests.rs}`,
`services/catalyst/config/src/content.rs`, `schemas/00_content.sql` (`content.pending_deployments`, `pending_deployment_files`, `partial_upload_rates`),
`services/catalyst/content/src/validator/{mod.rs,rules.rs}`, `services/catalyst/shared/src/{client_ip.rs,cors.rs}`,
`library/dcl-auth/src/{auth_chain.rs,error.rs}`, `infra/gateway/gateway.py`, `services/catalyst/src/serve.rs`,
`services/catalyst/tests/fixtures/content/create-entity-400-not-multipart.json`.

## 1. Endpoint inventory

| method | upstream path | production URL prefix | dcl-one route(s) | status |
|---|---|---|---|---|
| POST | `/entities` (regular deployment) | `/content` | `services/catalyst/content/src/lib.rs:682` (`create_entity::create_entity`), served at the root and under `/content` (`lib.rs:710-711`) | DIVERGENT |
| POST | `/entities` with `partial=true` (form field, and the optional `?partial=true` query) | `/content` | the same route (`lib.rs:682-686`); a form with `partial=true` goes to `partial_deployments::handle` (`create_entity.rs:436-441,524-579`, `partial_deployments.rs:571`) | DIVERGENT |
| OPTIONS | `/entities` (CORS preflight) | `/content` | `services/catalyst/shared/src/cors.rs:20-52` (global middleware) | DIVERGENT |
| GET/HEAD/PUT/DELETE | `/entities` (method not allowed) | `/content` | axum `MethodRouter` 405 on `lib.rs:682` | MATCH |

Counts: 4 rows: 1 MATCH, 0 GAP, 3 DIVERGENT, 0 MISSING.

READ_ONLY mode is a mode of the first row (upstream does not register the route). It is covered in section 2.

---

## 2. `POST /entities`: regular deployment

### Request

- **Body:** `multipart/form-data`.
  - Fields: `entityId` (required); `authChain` as a JSON array, or as `authChain[i][type|payload|signature]`, at most 10 links; optional `partial`.
  - Files: any field names. In the regular path the names are ignored and each file is hashed (`create-entity-handler.ts:133`, `deployment-service/component.ts:419-431`).
- **Auth:** the auth chain in the form, signing `entityId`. The signature expiry is checked at `entity.timestamp` (`create-entity-handler.ts:144,159-170`). It is not signed fetch.
- **Upstream middleware chain, in order** (`routes.ts:58-83`):
  1. `stampRequestArrival`.
  2. Burst limit: `POST_ENTITIES_RATE_LIMIT_MAX`=200 per `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`=60, keyed by client IP (`Environment.ts:86-87`, `routes.ts:47-51`).
  3. Bootstrapping guard (`middlewares.ts:19-39`).
  4. Daily-quota pre-check: an exhausted marker only (`deployment-quota.ts:41-59`).
  5. Memory admission: declared Content-Length must fit `MAX_IN_MEMORY_DEPLOYMENT_BYTES`=2 GiB (`deployment-memory-admission.ts:16-35`).
  6. Source upload admission: `MAX_CONCURRENT_UPLOADS_PER_SOURCE`=4 and per-source bytes = `MAX_UPLOAD_TOTAL_SIZE` (`source-upload-admission.ts:16-47`).
  7. Multipart parser with these limits:
     - file 100 MiB (`MAX_UPLOAD_FILE_SIZE`);
     - 3000 files;
     - 32 fields;
     - 32 KiB per field;
     - total 2 GiB;
     - timeout 5 min;
     - disk spool and aggregate in-flight budget 4 GiB (`Environment.ts:35-70`, `multipart.ts:119-500`).
  8. Daily-quota consume: counted after the parse, before the handler (`deployment-quota.ts:111-129`).
  9. Handler.
- **Client source** (burst limit and daily quota):
  - Upstream: `TRUSTED_CLIENT_IP_HEADER`, read from the right with one trusted proxy, else the socket address. Both are canonicalized (`client-source.ts:17-36`).
  - dcl-one: `x-real-ip`, else the last `x-forwarded-for` entry, else none. With no source the quota is skipped (`services/catalyst/shared/src/client_ip.rs:7-23`, `upload_admission.rs:190-197`). IPv6 is grouped by /64 in both (upstream `CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`=64; dcl-one `upload_admission.rs:85-98`).
- **dcl-one chain, in order** (`create_entity.rs:419-491,575-731`):
  1. `writes::refuse`: read-only, then the `CONTENT_WRITES_ENABLED` switch (`writes.rs:83-88`).
  2. Deployer attached.
  2a. Burst limit: `POST_ENTITIES_RATE_LIMIT_MAX`=200 per `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`=60, a fixed window per client address (IPv6 by /64), every request counted, per process (`create_entity.rs:432-436`, `upload_admission.rs:140-185`).
  3. Bootstrapping (`create_entity.rs:437-439`).
  4. Daily-quota pre-check: a spent source gets the 429, except a batch declared with `?partial=true` (`create_entity.rs:440-447`).
  5. Upload budget on declared length, minimum 16 MiB, capacity max(`MAX_IN_MEMORY_DEPLOYMENT_BYTES`, 200 MiB) (`create_entity.rs:449-457`, `upload_admission.rs:53-75`).
  6. Source upload admission: 4 bodies and one maximum-size upload's bytes per client address (`create_entity.rs:458-466`, CW-10).
  7. Not a `multipart/form-data` body (400), or a declared length past the form limits (413), uncounted (`create_entity.rs:467-475`).
  8. Form read within `MULTIPART_UPLOAD_TIMEOUT_MS` (408 past it), held in memory, with upstream's limits and texts: 3000 files, 100 MiB per file, field count/size from settings (default 32 / 32 KiB), 200 MiB of values in all, no repeated name (`create_entity.rs:255-345,575-616`).
  9. The query-flag 400, then the daily quota consume; a partial batch is counted as section 3 says (`create_entity.rs:620-631`).
  10. `entityId`.
  11. Auth chain.
  12. Replay: an entity already deployed answers its stored timestamp (`create_entity.rs:661-674`).
  13. Unsynced-copy type gate.
  14. `WriteDeployer::deploy_entity`.
- **Gateway in front of dcl-one:** `limit_req zone=deploy` 5 r/s, burst 200, `limit_req_status 429`, `limit_conn deploy_uploads 10` per address (`limit_conn_status 429`), `client_max_body_size 248m` (`CONTENT_DEPLOY_CAP`, just above the app's route limit with framing), `client_body_timeout 300s` (`infra/gateway/gateway.py`). It is a coarse abuse guard set above the app's burst limit and upstream's 4 concurrent uploads per source, so a client within upstream's limits never meets nginx's HTML 429 or 413.

### Responses

Upstream error envelopes:
- HTTP errors thrown from middleware or the handler are `{"error": string}` (`middlewares.ts:41-82`).
- Deployment rejections are `{"errors": string[]}` (`create-entity-handler.ts:142,169,357-360`).

Success example (both):

```json
{ "creationTimestamp": 1761990000000 }
```

#### 2a. Mode and admission

| trigger | status | body shape | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| `READ_ONLY=true` | 404 (route not registered; `/entities` matches no route). The default not-found of `@dcl/http-server` (body not in tree). | default | none | `routes.ts:43-45`, `service.ts:63-65` | Read-only toggle (`POST /admin/api/content/read-only`): 404, this router's own not-found (`text/plain` `Not found`), checked before every other gate (`writes.rs:59-63,83-88`, `lib.rs:624-631`) | MATCH on status; the body is each router's own not-found |
| (no upstream equivalent) writes switch off | n/a | n/a | n/a | n/a | 503 `{"error":"content writes are disabled on this node (CONTENT_WRITES_ENABLED=false): ..."}`, `Retry-After: 3600` (`writes.rs:26,40-54`); restore-stamp variants (`writes.rs:46-54`) | DIVERGENT: a node with writes off does not look like a read-only catalyst (404) |
| burst limit exceeded | 429 | `{"error":"Too many requests"}` | `Retry-After` (and the component's `RateLimit-*` headers; set not in tree) | `routes.ts:47-51`, `components.ts:545-556` | 429 `{"error":"Too many requests"}`, `Retry-After` = seconds left in the 60 s window; 200 per window per client address, before the bootstrapping gate; per process (`create_entity.rs:399-403`). The gateway's deploy zone (5 r/s, burst 200) sits above it. | MATCH on limit, body and `Retry-After`. The `RateLimit-*` headers and the component's exact window algorithm are not in the pinned tree and not reproduced. |
| sync state BOOTSTRAPPING | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | Same body and header (`writes.rs:65-74`, `create_entity.rs:404-406`) | MATCH |
| unsynced copy, entity type not ready | n/a (upstream gates on BOOTSTRAPPING only) | | | | 503 bootstrapping body after the form is read; 400 `{"errors":["The entity file was not part of the uploaded files or has no type."]}` when the type cannot be read (`create_entity.rs:453-467`) | dcl-one only |
| daily quota exhausted (pre-body marker, or at consume) | 429 | `{"error":"Too many requests"}` | `Retry-After: <s to window end>` | `deployment-quota.ts:28-34,41-59,120-127`; window 86400 s, max 300 (`logic/deployment-quota/component.ts:6,21-25`) | 429 `{"error":"Too many requests"}`, `Retry-After`; checked before the body, counted once the form is read (`create_entity.rs:440-447,626-631`, `upload_admission.rs:262-330`) | MATCH. The refund of the deployer's own 503 and the skip without a stamped address are dcl-one's: see CW-14. |
| declared Content-Length does not fit the memory budget | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `deployment-memory-admission.ts:21-33`, `upload-budget/errors.ts:3`, `multipart.ts:92` | Same body and header, after the quota pre-check (`create_entity.rs:449-457`, `upload_admission.rs:33-34`, `writes.rs:38`) | MATCH |
| source has 4 uploads or its byte share in flight | 429 | `{"error":"Too many uploads in progress from this client, please retry shortly."}` | `Retry-After: 5` | `source-upload-admission.ts:9,22-46`, `source-upload-limits/errors.ts:3` | same body and header, `MAX_CONCURRENT_UPLOADS_PER_SOURCE` and `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` (`create_entity.rs:458-466`, `upload_admission.rs:89-160`) | MATCH |
| aggregate spool budget full (declared or while growing) | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:141-151,274-284` | Covered by the single in-memory budget above | MATCH (one budget instead of two) |

#### 2b. Multipart parsing (upstream `multipart.ts`, mapped by `middlewares.ts:41-82`)

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Content-Type not `multipart/form-data` | 400 | `{"error":"Invalid request: expected a multipart/form-data body"}` | | `multipart.ts:196-202` | Same body, after the read-only, writes, bootstrapping and budget gates and a spent quota's 429, uncounted (`create_entity.rs:424-446`, `tests/fixtures/content/create-entity-400-not-multipart.json`) | MATCH |
| declared Content-Length > total + framing | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes plus multipart framing (<n> bytes in all)."}` | | `multipart.ts:133-136,161-166` | Same text over dcl-one's 200 MiB total: `... 209715200 bytes plus multipart framing (<n> bytes in all).`, before the body is read; a body that grows past `<n>` gets the same 413 (`create_entity.rs:75-95,427-446`, `lib.rs:711-713`). The gateway's cap (248 MiB) sits above `<n>` at the default field count, so the app's 413 is the one a client meets. | MATCH except the total (200 MiB, as CW-7 keeps it) |
| received bytes > `MAX_UPLOAD_TOTAL_SIZE` | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes."}` | | `multipart.ts:259-270` | Same text with 209715200 (`create_entity.rs:235-245`) | MATCH except the total |
| more than 3000 files | 413 | `{"error":"Too many files in the request. The maximum allowed is 3000."}` | | `multipart.ts:296-298` | same (`create_entity.rs:261-265`) | MATCH |
| file > 100 MiB | 413 | `{"error":"File '<filename>' is too large. The maximum allowed size per file is 104857600 bytes."}` | | `multipart.ts:378-386` | same, `undefined` for a part without a file name as busboy prints it (`create_entity.rs:269-278`) | MATCH |
| more than 32 fields | 413 | `{"error":"Too many form fields in the request. The maximum allowed is 32."}` | | `multipart.ts:301-307` | same (`create_entity.rs:284-288`) | MATCH |
| field > 32 KiB | 413 | `{"error":"Field '<name>' is too large. The maximum allowed size per field is 32768 bytes."}` | | `multipart.ts:314-321` | same (`create_entity.rs:292-296`) | MATCH |
| repeated field name | 400 | `{"error":"Duplicate form field '<name>'"}` | | `multipart.ts:223-232` | same, for fields and file parts (`create_entity.rs:266-268,299-301`) | MATCH |
| body still arriving after 5 min | 408 | `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}` | | `multipart.ts:110-117,437-450` | Same body after `MULTIPART_UPLOAD_TIMEOUT_MS` (300 s), the bytes counted off the wire (`create_entity.rs:493-543,604-611`); the whole-request timeout skips this route (`lib.rs:777-794`). Gateway `client_body_timeout 300s` still gives an nginx 408 for an idle body. | MATCH |
| read form not decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS` (5 min) | n/a (no deadline after the read) | | | | 503 `{"errors":["Deployment processing exceeded the <ms>ms deadline."]}`, `Retry-After: 30`, worlds' text; the deploy is dropped, which releases its upload budget, source share and pointer claim, and the request stays counted against the quota (`create_entity.rs` `processing_timed_out`) | dcl-one only: bounds what the blob collector's age floor assumes (`gc::orphan_age_floor`) |
| malformed multipart (parser error) | 400 | `{"error":"Invalid multipart/form-data request"}` | | `multipart.ts:455-470` | same; the parser's text is logged (`create_entity.rs:100-108`). A field value that is not UTF-8 is decoded with replacement characters, as busboy does. | MATCH |
| spool write/folder failure | 503 | `{"error":"The server cannot store uploads right now, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:94,169-176` | n/a (no spool, forms held in memory) | n/a |
| spool flush did not finish in time | 503 | `{"error":"The upload could not be stored in time, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:476-485` | n/a | n/a |

#### 2c. Handler: form fields and auth chain (`create-entity-handler.ts`)

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `?partial=true` without the form field `partial=true` | 400 | `{"error":"The 'partial=true' query parameter requires the 'partial=true' form field"}` | `deployment-quota.ts:116-119` | Same body, once the form is read and before the `entityId` check (`create_entity.rs:620-625`) | MATCH |
| no `entityId` | 400 | `{"error":"Missing required field: entityId"}` | `:73-76` | same (`create_entity.rs:110,656-659`) | MATCH |
| no `authChain` field and no `authChain[i]` fields | 400 | `{"error":"No auth chain can be derivated"}` | `:89-91` | same; a legacy `ethAddress`+`signature` pair is no chain (`create_entity.rs:227-238`) | MATCH |
| `authChain` not JSON or not an array | 400 | `{"error":"Invalid auth chain"}` | `:434-446` | same (`create_entity.rs:72-77`) | MATCH |
| chain longer than 10 (JSON or indexed) | 400 | `{"error":"Auth chain is too long; the maximum allowed is 10 elements"}` | `:450-452,472-474` | same (`create_entity.rs:78-80,98-100,126-128`; `library/dcl-auth/src/auth_chain.rs:6`) | MATCH |
| indexed link missing payload/signature/type | 400 | `{"error":"Missing auth chain element at index <i>"}` | `:477-484` | same (`create_entity.rs:116-124`) | MATCH |
| chain fails `AuthChain.validate` (e.g. `[]`, a link without `type`/`payload`, an unknown `type`) | 400 | `{"error":"Invalid auth chain"}` | `:92-94` | same, the schema of `@dcl/schemas`' `AuthChain` ported (`create_entity.rs:189-226`) | MATCH |
| entity already deployed (any `entityId` present in `deployments`) | 200 | `{"creationTimestamp": <stored localTimestamp>}`, answered before the signature, TTL and rate-limit checks | `:98-109`, `deployment-service/component.ts:453-454,461-468` | Same, right after the auth-chain parse (`create_entity.rs:565-579`); a deploy racing it answers the stored timestamp from the write (`write_deployer.rs:1203-1213`). Deploys of one entity run one at a time in a process (`EntityLocks` in `write_deployer.rs`): a copy that arrives while the original is in flight waits for it and answers the timestamp it stored, so a retry racing its original gets 200, not the per-pointer `Entity rate limited`. A failed lookup goes on to the deploy. | MATCH |

#### 2d. Deployment validation (`deployment-service/component.ts`, `server-validator.ts`, `@dcl/content-validator`)

Upstream order:
1. Read the entity.
2. Signature.
3. Pointers / conflict.
4. Server checks: newer, already deployed, rate limit, TTL back, TTL forward.
5. Protocol validator.
6. Current access (scene older than the TTL).
7. Store.

dcl-one order (`write_deployer.rs:932-1117`):
1. Hash.
2. Entity file.
3. Parse (the `id` is the request's).
4. Denylist (dcl-one only).
5. Signature.
6. Pointers.
7. In-flight claim on the pointers (this process).
8. Server checks: newer, rate limit, TTL back, TTL forward.
9. Validator (structure, hashing, metadata, ADR-45, signature again, size, ..., access).
10. Reserve rate limit.
11. Persist (the binding newer-entity check inside the SQL; an already deployed entity is a success).

When more than one check fails, the order decides which message is returned.

All rows below are 400 `{"errors":[...]}` upstream unless the row says otherwise.

| trigger | upstream message | upstream ref | dcl-one | verdict |
|---|---|---|---|---|
| no uploaded file hashes to `entityId` | `Failed to find the entity file.` | `component.ts:402-405,427-430` | same (`write_deployer.rs:945-949`) | MATCH |
| entity file not JSON / bad type / bad pointers / bad timestamp / bad content | `There was a problem parsing the entity` (`InvalidEntityError` text is logged only) | `component.ts:406-415`, `entities/component.ts:99-156` | same; the serde text is logged (`write_deployer.rs:782-786`) | MATCH |
| entity file `id` field differs from `entityId` | accepted (id is taken from the request) | `entities/component.ts:126-128` | accepted, the id taken from the request (`write_deployer.rs:959-961`) | MATCH |
| signature invalid at `entity.timestamp` | `The signature is invalid. <@dcl/crypto message>` | `create-entity-handler.ts:159-170`, `crypto/component.ts:25-32` | `The signature is invalid. <dcl-auth AuthError Display>`, before the server checks (`write_deployer.rs:976-980`, `library/dcl-auth/src/error.rs:4-38`) | MATCH on order; DIVERGENT: tail wording (not pinned upstream) |
| contract-wallet (EIP-1654) link cannot be checked | 400 with the `@dcl/crypto` message | same | 503 `{"errors":["The signature is invalid. EIP-1654 ..."]}`, `Retry-After: 30` (`validator/rules.rs:308-311`, `create_entity.rs:262-278`) | DIVERGENT (intentional) |
| `pointers: []` | `The entity does not have any pointer.` | `component.ts:473-477` | same, before the validator (`write_deployer.rs:981-983`) | MATCH |
| same pointers in flight in this process | `The following pointers are currently being deployed: '<p1,p2>'. Please try again in a few seconds.` | `component.ts:33,480-490` | same text, per entity type, held from the checks to the write (`write_deployer.rs:549-575,984-991`); a partial publication waits it out as upstream's finalize (CW-1); a concurrent copy of the same entity waits for it and answers its stored timestamp (`EntityLocks`) | MATCH |
| newer entity on any pointer | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).` | `server-validator.ts:24-28,77-78` | same, checked before the rate limit and again by the write (`write_deployer.rs:895-901,993-999`) | MATCH |
| already deployed (reached only on a race) | `This entity was already deployed. You can't redeploy it` | `server-validator.ts:80-81` | treated as success (`write_deployer.rs:1168-1174`) | DIVERGENT (race only) |
| pointer rate-limited | `Entity rate limited (entityId=<id> pointers=<p,..>).` | `server-validator.ts:83-87` | same text (`write_deployer.rs:793-812`) | MATCH on text. Windows: env `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`_MAX_<TYPE>` (`component.ts:109-121`, `Environment.ts:537-542,775-790`), unset the per-type defaults profile 3 s/500, scene 20 s/1000, wearable 20 s/1000, store 3 s/300, emote 20 s/1000, outfits 3 s/2000 (`rate-limiter.ts:199-247`); `DEPLOYMENTS_DEFAULT_RATE_LIMIT_*` (60 s / 300) only for a type outside that table. dcl-one reads the same env names with the same defaults (`write_deployer.rs`, CW-15). Profile unchanged-content 5 min in both. |
| `now - timestamp > 20 min` | `The request is not recent enough, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:89-94`, `Environment.ts:564-565` | same, `REQUEST_TTL_BACKWARDS` configurable (`write_deployer.rs:1021-1028`) | MATCH |
| `timestamp - now > 15 min` | `The request is too far in the future, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:8,96-101` | same (`write_deployer.rs:1029-1035`) | MATCH |
| entity id, content hash or pointer on the operator denylist | accepted (the denylist filters reads only) | `component.ts:305-312` | `The entity <id> is denylisted on this node.` / `These content files are denylisted on this node: ..` / `These pointers are denylisted on this node: ..` (`denylist.rs:87-117`, `write_deployer.rs:963-972`) | DIVERGENT: dcl-one-only refusal, kept on purpose (CW-13) |
| two uploaded files map to one declared hash with different bytes | n/a | | `two different uploaded files map to the same content hash <h>` (`write_deployer.rs:833-837`) | dcl-one only |
| content-validator rejections (structure, IPFS hashing, metadata schema, ADR-45, size per pointer, content cross-check, profile/scene/outfits/items rules, access) | `@dcl/content-validator` strings | `component.ts:362-369` | Port in `services/catalyst/content/src/validator/rules.rs`. Spot-checked strings equal to the local clone: `There are repeated pointers in your request.`; `The entity needs to be pointed by one or more pointers.`; `This hash '<h>' is not valid. It should be IPFS v2 format.`; `The deployment is too big. The maximum allowed size per pointer is ..`; `This hash is referenced in the entity but was not uploaded or previously available: <h>`; `This hash was uploaded but is not referenced in the entity: <h>`; `The metadata for this entity type (<t>) is not valid.`; `Type <t> is not supported yet` (`rules.rs:250,257,279,327,367,762-780`, `metadata_schema.rs:45`) | MATCH for the strings checked; the rest is not compared (upstream source not in the pinned tree) |
| access checker fails (RPC down / timeout) | scenes: 400 `Cannot validate deployment`; names, items, collections and third parties: the failure becomes the not-owned or no-access refusal (`access/on-chain/scenes.ts`, `access/on-chain/client.ts`, `collection-asset.ts`, `third-party-asset.ts` in the local clone) | `@dcl/content-validator` | 503 with `Retry-After: 30`: scenes `{"errors":["Cannot validate deployment"]}`, the others `Error checking name ownership.`, `Error checking item ownership.`, `Error checking wearable ownership.`, `Error checking collection access.` or `Error checking third-party access.`; the cause is logged, never answered (`undecided` in `validator/checker.rs`, `validator/mod.rs:23-27,358-370`, `rules.rs:958`) | DIVERGENT: status (intentional); scene text MATCH |
| content store unreadable | n/a (throws, then the generic error) | | 503 `This node could not read its own content store, so the deployment could not be validated. Please retry.`; the cause is logged (`validator/rules.rs:56-63`) | DIVERGENT: status and text (dcl-one classifies it as undecidable, CW-17) |
| scene finalized older than the TTL, current access lost | `The deployer no longer has access to the entity pointers (access is required both when a partial upload starts and when it is finalized).` | `component.ts:384-395` | The publication of a complete upload runs the full validator, access included, so lost access is refused with the validator's access message (`partial_deployments.rs:1509-1530`). Upstream's fallback text, used when its validator returns no message, is never produced. | MATCH except the fallback text (P2) |
| commit after the partial-upload deadline | `This upload expired before it could be published. Create a new entity with a fresh timestamp.` | `deployment-service/errors.ts:4`, `component.ts:595-597` | Checked before publication: 400 `This upload expired. Create a new entity with a fresh timestamp.`, upstream's `assertLiveUntil` text at the same point (`partial-deployments/component.ts:249-256,348-349`); the commit itself has no deadline (`partial_deployments.rs:298-300,1532-1534`) | DIVERGENT only for a commit that outlives the deadline (CW-1) |
| internal error while storing | 400 `{"errors":["There was an error deploying the entity"]}` or `["An internal server error occurred. This will raise an automatic alarm."]` | `component.ts:530-541,598-602` | 503 `{"errors":["There was an error deploying the entity"]}`, `Retry-After: 30`, for a database, store or hashing-task failure; the cause is logged (`write_deployer.rs:71-79`) | DIVERGENT: status only (503 so the client retries, kept as a classification) |
| content lock busy (GC) | 503 `{"error":"The content lock for entity <id> is still busy, please retry shortly."}`, `Retry-After: 10` | `create-entity-handler.ts:409-418`, `errors.ts:39`, `content-locks/errors.ts:1-7` | n/a (no shared content lock) | n/a |
| spool read failure (transient errno) | 503 `{"error":"The server cannot store uploads right now, please retry shortly."}`, `Retry-After: 30` | `create-entity-handler.ts:321-335` | n/a | n/a |
| unexpected exception | 500 `{"error":"Internal Server Error"}` | `middlewares.ts:72-81` | 500 `{"error":"Internal Server Error"}` (`errors.rs:52-58`) | MATCH |

#### 2e. Success

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| new deployment committed | 200 | `{"creationTimestamp": number}` = the stored `localTimestamp` (ms), the same value `/deployments` and `/audit` show | `Content-Type: application/json` | `create-entity-handler.ts:337-349`, `component.ts:206-210,604` | `{"creationTimestamp": <ms>}`: the deploy call's `now()`, which is the stored `local_timestamp` (one statement, one transaction), truncated to ms as the history routes report it (`write_deployer.rs:63-69,937-952,1094`) | MATCH |

### Visibility after a successful deploy

- **Upstream:** writes `active_pointers` in the deploy transaction. After the commit it updates the in-process active-entities cache and invalidates the collection-prefix caches (`component.ts:222-290`). A `POST /entities/active` right after the 200 returns the new entity, on this process and on any process reading the database.
- **dcl-one:**
  - Commits deployment, content files, active pointers and overwrites in one statement (`write_deployer.rs:923-1075`).
  - `POST /entities/active` and the collection listing read `state.pool` directly, with no in-process entity cache (`active_entities.rs:56-85`). The entity is visible on the next request on every instance. Verdict: MATCH.
  - `deployments_cache` (5 s TTL, `lib.rs:120`) is cleared only on the instance that took the deploy (`create_entity.rs:489`). Other instances can serve `/deployments`, `/pointer-changes` and `/snapshots` up to 5 s stale. That belongs to the read area.
- Upstream sends `Cache-Control: public, max-age=<ENTITIES_CACHE_CONTROL_MAX_AGE>` (default 10) only on `GET /entities/:type`; `POST /entities/active` carries no cache header (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:36-37`). dcl-one sends `public, max-age=10` on both (`services/catalyst/content/src/active_entities.rs:119`; CR-8 in `content-reads.md`). Shared caches do not store POST responses, so in both trees only the GET read can be served up to 10 s stale.

### Data dependency

- **Content DB:**
  - `content.deployments`, `content_files`, `active_pointers`;
  - `content.local_entities` (dcl-one provenance);
  - `content.forward_outbox` (dcl-one forwarding).
- **Blob store:** for content existence (`isContentStoredAlready`) and stored sizes (size rule).
- **Ownership:**
  - Upstream: on-chain (L1/L2 RPC: Checker contract, collection and third-party registry contracts) when `L1_HTTP_PROVIDER_URL` and `L2_HTTP_PROVIDER_URL` are set, else the subgraphs (land manager, blocks L1/L2, collections L1/L2, ENS owner, third-party registry) (`content-validator/component.ts:238-270`).
  - dcl-one: on-chain only, `ETH_RPC_URL` and `RPC_ENDPOINT_POLYGON` (`validator/mod.rs:14-20`), plus the `shop.usage_grants` lease overlay (dcl-one-only grants). The `land_authz` index is used only by the test path without the validator (`write_deployer.rs:557-601`).
  - dcl-one has these sources when the RPC URLs are configured. With `CONTENT_WRITES_ENABLED` on, a missing or plaintext RPC refuses to start (`validator/mod.rs:108-130`).
- **Sync state:** the bootstrapping gate needs the sync status. dcl-one: `SyncStatus` (`lib.rs:395-415`).

---

## 3. `POST /entities` with `partial=true` (multi-request upload, ADR-325)

### Request

- **Same route and middleware.** `?partial=true` skips the pre-body daily-quota check and the memory admission (`deployment-quota.ts:24-26,47`, `deployment-memory-admission.ts:22`).
  - dcl-one skips the quota consume on admission for a declared batch and counts it once the form is read (`create_entity.rs:338-350,524-579`). It keeps the in-memory upload budget for every batch (`create_entity.rs:326-337`): a batch is buffered in memory like a regular deploy, where upstream spools it to disk.
  - Before any file part is buffered, dcl-one turns away a batch from a source that spent its quota (unless it resumes the claimed signer's own live upload) and one from an account over its byte rate, with upstream's 429 messages (`create_entity.rs:197-201,410-432`, `partial_deployments.rs:186-237`). Upstream decides both after authentication; the bodies and statuses are the same.
- **Form:** `partial=true` field. File field names must be content hashes. The first batch must carry the entity file, at most 10 MiB (`partial-deployments/component.ts:63`). Only IPFS v2 scene entities are accepted. dcl-one: the same rules (`partial_deployments.rs:90,690-797`).
- **Limits** (`Environment.ts:667-699`), with the same env names and defaults in dcl-one (`services/catalyst/config/src/content.rs:184-217`, `partial_deployments.rs:104-140`):
  - `PENDING_DEPLOYMENT_TTL` 1 h;
  - `PENDING_DEPLOYMENTS_CLEANUP_INTERVAL` 5 min;
  - `MAX_PENDING_DEPLOYMENTS_PER_DEPLOYER` 10;
  - `MAX_PENDING_UPLOADS_PER_POINTER` 4;
  - `MAX_PENDING_BYTES_PER_DEPLOYER` 1 GiB;
  - `MAX_PENDING_BYTES` 50 GiB;
  - `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` 512 MiB.
  - Upstream refuses to start when `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` cannot fit the entity file plus one `MAX_UPLOAD_FILE_SIZE` file (`partial-deployments/component.ts:227`); dcl-one has no such startup check.
- **Per-file and body caps:** the regular multipart caps apply to every batch in both trees: 100 MiB per file in both, 2 GiB per body upstream and 200 MiB in dcl-one (CW-7).

### Responses

Bodies are `{"errors":[...]}` unless the row says otherwise. Every row is read; nothing was run.

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| batch stored, content still missing | 202 | `{"missing": ["<hash>", ...]}` | | `create-entity-handler.ts:233-240` | same (`partial_deployments.rs:590`) | MATCH |
| batch completes the upload | 200 | `{"creationTimestamp": number}` | | `create-entity-handler.ts:223-231` | same, the stored `local_timestamp` (`partial_deployments.rs:580-587,1534-1541`) | MATCH |
| replay of a published entity | 200 | `{"creationTimestamp": number}`, before authentication | | `create-entity-handler.ts:100-109` | same (`partial_deployments.rs:610-612`) | MATCH |
| source quota spent, not the signer's own live upload | 429 | `The daily deployment quota for this client is exhausted. Only uploads in progress can continue.` | `Retry-After` | `create-entity-handler.ts:111-126` | same text and header, before the files are buffered or after the replay check (`partial_deployments.rs:186-213,618-628`) | MATCH |
| signature invalid | 400 | `The signature is invalid. <msg>` | | `:159-170,174-197` | same prefix (`partial_deployments.rs:634-649`, `write_deployer.rs:494-516`); the tail is `dcl-auth`'s text. An EIP-1654 link this node cannot check answers 503 (CW-17). | DIVERGENT: tail wording, as on the regular path |
| entity id not IPFS v2 | 400 | `The entity id '<id>' is not a valid IPFS v2 hash. Partial deployments require IPFS v2 entities.` | | `partial-deployments/component.ts:694-697` | same (`partial_deployments.rs:700-704`) | MATCH |
| file hash ≠ field name | 400 | `The uploaded file '<key>' does not match its content hash (computed <h>).` | | `:707-711` | same (`partial_deployments.rs:722-728`) | MATCH |
| entity file > 10 MiB | 400 | `The entity file '<id>' is too large (<n> bytes, max 10485760).` | | `:721-725` | same (`partial_deployments.rs:733-738`) | MATCH |
| no entity file on first batch / other signer | 400 | `The first partial request for an entity, and any request by another signer, must include the entity file '<id>'.` | | `:431-433` | same (`partial_deployments.rs:295-299,895-924`) | MATCH |
| entity unparsable / not a scene / no pointer | 400 | `There was a problem parsing the entity: <e>` / `Partial deployments are only supported for scenes, but the entity type is '<t>'.` / `The entity does not have any pointer.` | | `:739-747` | same texts; `<e>` is serde's message, not the JS error's (`partial_deployments.rs:748-760`) | MATCH (the `<e>` tail differs) |
| staging validation fails | 400 | validator errors, or `The staging validation was not successful.` | | `:771` | the ported validator's staging subset, access skipped for the creator's own resume, as upstream (`partial_deployments.rs:790-797`, `write_deployer.rs:517-542`) | MATCH where the ported strings match (section 2d) |
| newer entity / rate limit / TTL | 400 / 429 / 400 | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).`; `Entity rate limited (...)` (429, `Retry-After`); `The request is not recent enough, please submit it again with a new timestamp (entityId=<id>).`; `The request is too far in the future, please submit it again with a new timestamp (entityId=<id>).` | | `:775-802` | same texts and statuses, TTL measured from the upload's first batch (`partial_deployments.rs:815-857,872-878`). The `Retry-After` comes from the type's window (CW-15). | MATCH |
| upload started by another account / expired / gone | 400 | `This upload was started by another account.` / `This upload expired. Create a new entity with a fresh timestamp.` / `Upload no longer exists; resend its manifest.` | | `:250,490,547` | same (`partial_deployments.rs:291-293,1236-1244,1366-1373`) | MATCH |
| per-account / per-pointer upload count | 429 | `Too many partial uploads in progress for this account (max 10). Complete an upload or wait for expired uploads to be cleaned up.`; `Too many partial uploads in progress for parcels <p..> (max 4 per parcel). Complete an upload or wait for one to expire.`; `Too many partial uploads in progress for this account on parcels <p..> (max <n> per parcel). Complete an upload or wait for one to expire.` | `Retry-After` | `:122-140,498` | same texts; `Retry-After` counts from the last cleanup run (`partial_deployments.rs:430-458,1255-1262`) | MATCH |
| byte budgets | 400 / 429 | `This batch is <n> bytes, over the partial upload byte rate limit of <m> bytes per minute. Send it in smaller batches.` (400); `Partial upload byte rate exceeded for this account: ...` (429); `This upload needs <n> bytes of partial upload storage, over the <limit> budget of <m> bytes.` (400); `Partial upload storage budget exceeded for this account: ...` (429); `Partial upload storage on this server is full. Retry later.` (429); `Deployment failed: The deployment is too big.` (400) | `Retry-After` on 429 | `:558-606,583,891` | same texts and statuses (`partial_deployments.rs:1390-1500,1058`) | MATCH |
| publication failed server-side | 503 | `{"errors":[...]}` | `Retry-After: 5` | `create-entity-handler.ts:266-278`, `partial-deployments/component.ts:60,398` | 503 `{"errors":["There was an error deploying the entity"]}`, `Retry-After: 5`, the cause logged; a staged file the store cannot write answers the same text (`partial_deployments.rs:1198-1213`) | MATCH |
| another batch holds the entity's lock | 503 | `{"error":"The content lock for entity <id> is still busy, please retry shortly."}` | `Retry-After: 10` | `:279-284`, `content-locks/errors.ts:1-7`, `controllers/errors.ts:39-42` | same body and header, after waiting up to 30 s for the lock (`partial_deployments.rs:100,106,327-339,686`) | MATCH |
| memory budget full | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `:279-284` (entity file only) | same body and header, but on the declared length of every batch before its body is read (`create_entity.rs:449-457`) | DIVERGENT: dcl-one sheds whole batches that upstream would spool to disk (P2) |
| bootstrapping | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | same body and header, once the form is read (`create_entity.rs:650-652`) | MATCH |

**Quota counting.** Both trees count a partial upload against the daily quota once, when its first batch creates it, and count a batch refused with a 400 before it joins an upload; 429s, 503s and replays are not counted (`deployment-quota.ts:111-129`; `create_entity.rs:519-573`). Read, matching.

**Data dependency.**
- Upstream: a pending-deployments table (`pendingDeploymentsRepository`), staged blobs in the served content store, per-account accounting, and a cleanup job.
- dcl-one: `content.pending_deployments`, `content.pending_deployment_files` and `content.partial_upload_rates` (`schemas/00_content.sql`); staged blobs in the one content store, so `/contents/{hash}` serves them before publication as upstream does; the leased `content-partial-uploads-cleanup` job, attached only where content writes are on (`partial_deployments.rs:1753`, `services/catalyst/src/serve.rs:587-596`). The budget and entity locks are PostgreSQL advisory locks, so they hold across replicas.

---

## 4. `OPTIONS /entities` (CORS preflight)

| trigger | status | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| preflight | 204 (koa-cors default) | `Access-Control-Allow-Origin: *`; methods `GET, HEAD, POST, OPTIONS`; request headers reflected; `Access-Control-Max-Age: 600`; no credentials; responses `Access-Control-Expose-Headers: *` | `components.ts:489-506` | 204; `Access-Control-Allow-Origin: <Origin>` with `Access-Control-Allow-Credentials: true`; methods `GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS`; request headers reflected; `Access-Control-Max-Age: 86400`; `Vary: Origin`; expose `*` (`shared/src/cors.rs:15-79`) | DIVERGENT. This is shared CORS behaviour; `Retry-After` stays readable to browser deployers in both (expose `*`, no credentialed request). |

## 5. Other methods on `/entities`

| trigger | status | upstream | dcl-one | verdict |
|---|---|---|---|---|
| GET/HEAD/PUT/DELETE `/entities` | 405 with `Allow: POST` (`router.allowedMethods()`, `service.ts:63-65`); 404 in READ_ONLY | 405 with `Allow: POST`; 404 (`Not found`) while the read-only toggle is on (`create_entity.rs:373-383`, `lib.rs:711-713`) | MATCH (read; upstream body not in tree) |

---

## 6. Backlog

1. **CW-1: `POST /entities` partial uploads: what differs from upstream.**
   - The protocol matches (section 3): 202 `{"missing":[...]}`, 200 on completion and replay, the query-flag 400, the staging checks, the quota and budget messages with `Retry-After`, the env names and defaults, and the quota counting (`services/catalyst/content/src/partial_deployments.rs`, `create_entity.rs:733-792`).
   - Fixed: another batch holding the entity's lock answers upstream's 503 `{"error":"The content lock for entity <id> is still busy, please retry shortly."}` with `Retry-After: 10` (`partial_deployments.rs:106,327-339,686`). `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` below the entity file plus one maximal file (110 MiB) refuses to start (`services/catalyst/config/src/content.rs:215-224,290-294`). A publication meeting another deploy of this process on its pointers waits 300 ms up to 3 times, then answers 429 with `Retry-After: 5`, as upstream's finalize (`partial_deployments.rs:101-105`, `partial-deployments/component.ts:55-58,374-409`).
   - Not a gap: a complete upload past its deadline answers `This upload expired. Create a new entity with a fresh timestamp.`, which is upstream's `assertLiveUntil` at the start of its finalize (`partial-deployments/component.ts:249-256,348-349`). Upstream's `This upload expired before it could be published. ...` (`deployment-service/errors.ts:4`) comes only from a commit that outlives the deadline (`mustCommitBy`); dcl-one does not bound the commit itself.
   - Remaining gap: a batch is buffered in memory and every batch takes the in-memory upload budget on its declared length; upstream spools a declared batch to disk and skips the memory admission (`deployment-memory-admission.ts:22`). Under load dcl-one answers 503 `Server is handling too many uploads` to batches upstream would take. Closing it needs batches streamed to the store instead of read into memory (an architecture change).
   - Place: `create_entity.rs:449-457` (skip the memory admission for a declared batch once batches can stream).
   - Severity: P2. Deploys that split their files succeed against dcl-one.

2. **CW-2: replay idempotency.** Status: MATCH, no work. An entity already deployed answers 200 with its stored timestamp right after the auth-chain parse, before the signature, TTL and rate-limit checks (`create_entity.rs:565-579`); a deploy racing it answers the stored timestamp from the write (`write_deployer.rs:1203-1213`).

3. **CW-3: `creationTimestamp` value.** Status: MATCH, no work. `creationTimestamp` is the deploy call's `now()`, which is the stored `local_timestamp`, truncated to ms as `/deployments` and `/audit` report it (`write_deployer.rs:63-69,937-952,1094`).

4. **CW-4: read-only mode.** Status: MATCH, no work. The read-only toggle answers 404 with this router's not-found, before every other gate, for `POST /entities`, every other method on `/entities` and `DELETE /scenes/{coord}` (`writes.rs:59-63,83-88`, `create_entity.rs:373-383`, `lib.rs:711-713`), as upstream READ_ONLY, which does not register the route (`routes.ts:43-45`). Writes-off (`CONTENT_WRITES_ENABLED=false`, or held by a restore stamp) keeps its 503 with `Retry-After: 3600`: a dcl-one-only state with no upstream counterpart, which names why the node refuses (section 7); dcl-one's forwarder retries both answers (`forward.rs:19-21`).

5. **CW-5: bootstrapping `Retry-After`.** Status: MATCH, no work. Every bootstrapping 503 carries `Retry-After: 60` (`writes.rs:65-74`).

6. **CW-6: per-client burst limit.** Status: MATCH, no work. 200 requests per 60 s per client address (`POST_ENTITIES_RATE_LIMIT_MAX`, `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`), every request counted, answered 429 `{"error":"Too many requests"}` with `Retry-After` (`create_entity.rs:399-403`, `upload_admission.rs:140-185`, `services/catalyst/config/src/content.rs:169-182`). Per process, as upstream's limiter: N upstreams admit N times it. The gateway's deploy zone is now 5 r/s with burst 200, above it (`infra/gateway/gateway.py:146-153,263`). Not reproduced: the `RateLimit-*` headers of `@dcl/rate-limiter-component` (not in the pinned tree).

7. **CW-7: multipart limit statuses, values and messages.** Status: MATCH, no work. Every form limit answers 413 `{"error":...}` with upstream's text and limits (3000 files, 100 MiB per file, 32 fields of 32 KiB), the total kept at 200 MiB (`create_entity.rs:62-108,217-308`, `lib.rs:711-713`).

8. **CW-8: non-multipart body.** Status: MATCH, no work. A body that is not a form answers 400 `{"error":"Invalid request: expected a multipart/form-data body"}` after the gates, uncounted (`create_entity.rs:424-446`); the fixture `create-entity-400-not-multipart.json` replays it on a node that takes deploys.

9. **CW-9: internal error text in responses.** Status: MATCH, no work. Store, database and hashing failures answer `There was an error deploying the entity` (503 with `Retry-After`, CW-17's classification), a parse failure `There was a problem parsing the entity`, a parser failure `Invalid multipart/form-data request`, the unreadable store its sentence without the cause, a partial upload's unreadable store `There was an error deploying the entity`, an access source that cannot answer a fixed sentence (row "access checker fails"); each cause is logged (`write_deployer.rs:71-79,781-786`, `create_entity.rs:100-108`, `validator/rules.rs:56-63`, `store_unreadable` and `store_files` in `partial_deployments.rs`, `undecided` in `validator/checker.rs`).

10. **CW-10: per-source upload admission.** Status: MATCH, no work. Before the body, after the upload budget, a client address (IPv6 by /64) with `MAX_CONCURRENT_UPLOADS_PER_SOURCE` (4) bodies or `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` bytes (unset, one maximum-size upload, 200 MiB here; a set value below one 350 MiB world deploy is refused by the config, since worlds reads it too) in flight answers 429 `{"error":"Too many uploads in progress from this client, please retry shortly."}` with `Retry-After: 5`; a body counts its declared length, else the cap (`create_entity.rs:458-466`, `upload_admission.rs:89-160`, `services/catalyst/config/src/content.rs:226-244`). Per process, as upstream's. The client is chosen as for the quota (CW-14): the socket address, or a trusted proxy's `X-Real-IP`. One process serves both upstream services and they share these variable names, so the worlds multipart uploads read the same two settings. Unset, the worlds byte share is one 350 MiB world deploy. Trade-off of the default: a source with one large or chunked body in flight (a body without `Content-Length` counts the 200 MiB cap) gets 429 for any other upload until it ends; an operator raises the variable to let one source send several large bodies at once. The gateway also caps each address at 10 deploy uploads in flight (`limit_conn deploy_uploads`), above upstream's 4, as a coarse guard.

11. **CW-11: auth-chain error wording and checks.** Status: MATCH, no work. A form without a chain answers `{"error":"No auth chain can be derivated"}`; the legacy `ethAddress`+`signature` pair is no chain. A chain is checked as `@dcl/schemas`' `AuthChain.validate` (at least one link; a known `type`, a string `payload`, a string or null `signature`, required except on a SIGNER, whose signature is absent, null or empty; schema read from the 26.5.0 package), else `{"error":"Invalid auth chain"}`. A form without `entityId` answers `{"error":"Missing required field: entityId"}`; the query-flag 400 comes before it, as upstream's `withDeploymentQuota` (`create_entity.rs:110,189-238,620-660`).

12. **CW-12: deployment rejection wording.** Status: MATCH, no work. The write deployer answers upstream's texts (section 2d): `Failed to find the entity file.`; `There was a problem parsing the entity`; `The entity does not have any pointer.` before the validator; `The following pointers are currently being deployed: '<p,..>'. Please try again in a few seconds.` for pointers another deploy of this process holds; the ` (entityId=<id> pointers=<p,..>)` suffix on the newer-entity and both TTL messages (`write_deployer.rs:885-911,945-1035`). A complete upload's publication runs the same deployer, so its texts are these too.

13. **CW-13: check order and dcl-one-only refusals.** Status: MATCH, no work. The deployer checks in upstream's order: entity file, parse, signature at the entity's timestamp, pointers, the in-flight claim, newer entity, rate limit, TTL back, TTL forward, then the validator, which checks the signature again as upstream's validator does (`write_deployer.rs:945-1035`). The entity file's `id` field is ignored, the id is the request's (`write_deployer.rs:959-961`). Kept on purpose: the operator denylist refuses a listed entity id, content hash or pointer (`denylist.rs:87-117`), after the parse; upstream filters only reads. It is a dcl-one moderation tool (section 7), not a client contract.

14. **CW-14: daily quota counting.** Status: Kept by decision.
    - Fixed: the spent-quota 429 is answered before the upload budget, and the quota counts a request once its form is read, so forms the parser or its limits refuse are not charged (`create_entity.rs:440-447,626-631`).
    - Decision: the difference below is kept, and the client address follows upstream's socket fallback.
      - dcl-one gives back a deploy its deployer answered 503 for this node's own state (a source down, the store unreadable); upstream keeps it counted. Kept because of CW-17: the client was told to retry, so the retry should not spend its quota. The option not taken: count it as upstream.
      - The client is the socket address, as upstream's fallback (`client-source.ts:25`), unless the socket peer is in `HTTP_TRUSTED_PROXY_CIDRS`; then its single `X-Real-IP` names the client (`deploy_client` in `create_entity.rs`). Upstream reads the forwarding headers of any peer; dcl-one ignores them from other peers, so a client reaching the API directly can't rotate its budget or spend another address's. Only an in-process request without a socket is still keyed by its header, or skipped without one. Behind the gateway without that setting every deployer shares the gateway's bucket; the node logs one warning when an untrusted private or loopback peer forwards `X-Real-IP` (`looks_like_an_untrusted_proxy` in `create_entity.rs`).
    - Place: `create_entity.rs:632-643`, `upload_admission.rs:277-285`.
    - Severity: P2 (by decision).

15. **CW-15: per-pointer rate-limit configuration.** Status: MATCH.
    - Fixed: `REQUEST_TTL_BACKWARDS` (default 20 min), `DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL`/`_MAX` and `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`DEPLOYMENT_RATE_LIMIT_MAX_<TYPE>` are read under upstream's names, durations as the `ms` package writes them (`services/catalyst/config/src/content.rs`, `DeployLimits::from_config` in `write_deployer.rs`). Each of the six entity types takes its own `DEPLOYMENT_RATE_LIMIT_*_<TYPE>` or its built-in window, as upstream's `getCacheConfigPerEntityMap` (`rate-limiter.ts:199-247`); the `DEPLOYMENTS_DEFAULT_*` values apply only to a type outside that table (`rate-limiter.ts:163-164`), so setting them changes none of the six.
    - The built-in windows are upstream's per-type defaults: profile 3 s/500, scene 20 s/1000, wearable 20 s/1000, store 3 s/300, emote 20 s/1000, outfits 3 s/2000 (`rate-limiter.ts:199-247`). The production values are deployment configuration, not in the pinned tree.
    - Place: `REFERENCE_RATE_LIMITS` and `DeployLimits::from_config` in `write_deployer.rs`.

16. **CW-16: upload timeout body.** Status: MATCH, no work. A repeated form name answers upstream's 400 `{"error":"Duplicate form field '<name>'"}`. A form still arriving `MULTIPART_UPLOAD_TIMEOUT_MS` (300 s, the setting worlds reads too) after admission answers 408 `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}`, the bytes counted off the wire (`create_entity.rs:493-543,604-611`). The whole-request `REQUEST_TIMEOUT_SECS` does not apply to `POST /entities` (`build_router` in `lib.rs`); once read, the form must be decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS` (300 s), else 503 with `Retry-After: 30` (section 2b). The gateway's `client_body_timeout 300s` still answers an idle body with nginx's 408.

17. **CW-17: undecidable deploys answer 503.** Status: Kept by decision.
    - Gap: an EIP-1654 signature that cannot be checked, or an access-checker failure, answers 503 `{"errors"}` with `Retry-After: 30`. Upstream answers 400 `{"errors"}` with the validator text.
    - Decision: the 503 is kept. dcl-one tells the client the node could not decide (a chain RPC down or unset) instead of rejecting a deploy that may be valid; a 400 would make clients drop valid deploys during an RPC outage. The option not taken: answer 400 as upstream. Clients that do not retry 503 on POST report a transient failure where upstream reports a rejection.
    - Place: `validator/mod.rs:358-370`, `create_entity.rs:359-376`.
    - Severity: P2 (by decision).

18. **CW-18: CORS preflight on `/entities`.**
    - Gap: dcl-one reflects the Origin with credentials, allows a wider method list, and caches the preflight for 86400 s.
    - Expected: `Access-Control-Allow-Origin: *`, no credentials, methods `GET, HEAD, POST, OPTIONS`, max-age 600 (`components.ts:489-506`; captured on `/content/entities/active` in `captures/content-reads/CR-11-preflight.json`).
    - Same change as CR-11 (`content-reads.md`), in the shared `services/catalyst/shared/src/cors.rs:15-79`; it is fixed there, not here.
    - Severity: P2.

## 7. Endpoints dcl-one serves in this area that upstream does not

- `DELETE /scenes/{coord}` and `DELETE /content/scenes/{coord}` (`content/src/lib.rs:687`, `unpublish_scene.rs`):
  - Signed-fetch unpublish of a locally published scene by its LAND holder.
  - 200 `{entityId, unpublished: true, parcels}`; 400 / 401 / 403 / 404 / 409 / 503; behind the same `CONTENT_WRITES_ENABLED` and read-only gates.
- `GET /forwarding`, `GET /forwarding/{entityId}`, `POST /forwarding/{entityId}/retry`, and the `/content/` variants (`lib.rs:688-690`, `forward/status.rs:139-200`):
  - Admin-token routes over the upstream-forwarding outbox (`CONTENT_FORWARD_UPSTREAM_URL`).
- `POST /entities` at the root path, besides `/content/entities` (`lib.rs:710-711`). Upstream production serves it under `/content` only.
- The write switch's 503 (`CONTENT_WRITES_ENABLED`) and its restore-stamp variants (`writes.rs:40-54`). There is no upstream counterpart.

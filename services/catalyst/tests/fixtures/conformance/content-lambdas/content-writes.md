# Content server: write path (`POST /entities`)

Area: content server write path. ID prefix `CW-`.

Pinned revisions:

- Upstream content server: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- dcl-one: `dcl-one-conformance` @ `f2f04942` (origin/main, 2026-11-04). Paths below are relative to the repo root.
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
`services/catalyst/content/src/{lib.rs,create_entity.rs,upload_admission.rs,writes.rs,write_deployer.rs,errors.rs,denylist.rs,unpublish_scene.rs,forward.rs,forward/status.rs,active_entities.rs,write_path_tests.rs}`,
`services/catalyst/content/src/validator/{mod.rs,rules.rs}`, `services/catalyst/shared/src/{client_ip.rs,cors.rs}`,
`library/dcl-auth/src/{auth_chain.rs,error.rs}`, `infra/gateway/gateway.py`, `services/catalyst/src/serve.rs`,
`services/catalyst/tests/fixtures/content/create-entity-400-not-multipart.json`.

## 1. Endpoint inventory

| method | upstream path | production URL prefix | dcl-one route(s) | status |
|---|---|---|---|---|
| POST | `/entities` (regular deployment) | `/content` | `services/catalyst/content/src/lib.rs:677` (`create_entity::create_entity`), served at the root and under `/content` (`lib.rs:705-706`) | DIVERGENT |
| POST | `/entities` with `partial=true` (form field, and the optional `?partial=true` query) | `/content` | none. The form field is ignored and the request is handled as a regular deployment. | MISSING |
| OPTIONS | `/entities` (CORS preflight) | `/content` | `services/catalyst/shared/src/cors.rs:20-52` (global middleware) | DIVERGENT |
| GET/HEAD/PUT/DELETE | `/entities` (method not allowed) | `/content` | axum `MethodRouter` 405 on `lib.rs:677` | MATCH |

Counts: 4 rows: 1 MATCH, 0 GAP, 2 DIVERGENT, 1 MISSING.

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
- **Client source:**
  - Upstream: `TRUSTED_CLIENT_IP_HEADER`, read from the right with one trusted proxy, else the socket address. Both are canonicalized (`client-source.ts:17-36`).
  - dcl-one: `x-real-ip`, else the last `x-forwarded-for` entry, else none. With no source the quota is skipped (`services/catalyst/shared/src/client_ip.rs:7-23`, `upload_admission.rs:190-197`). IPv6 is grouped by /64 in both (upstream `CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`=64; dcl-one `upload_admission.rs:85-98`).
- **dcl-one chain, in order** (`create_entity.rs:278-421`):
  1. axum `Multipart` extractor: Content-Type check only.
  2. `writes::refuse`: the `CONTENT_WRITES_ENABLED` switch, then read-only (`writes.rs:67-75`).
  3. Deployer attached.
  4. Bootstrapping (`create_entity.rs:290-297`).
  5. Upload budget on declared length, minimum 16 MiB, capacity max(`MAX_IN_MEMORY_DEPLOYMENT_BYTES`, 200 MiB) (`create_entity.rs:299-310`, `upload_admission.rs:53-75`).
  6. Daily quota consume, counted at admission (`create_entity.rs:311-315`).
  7. Form read, held in memory: 1000 files, 50 MiB per file, field count/size from settings (default 32 / 32 KiB), body cap 200 MiB (`create_entity.rs:44-49,159-222`, `lib.rs:680`).
  8. `entityId`.
  9. Unsynced-copy type gate.
  10. Auth chain.
  11. `WriteDeployer::deploy_entity`.
- **Gateway in front of dcl-one:** `limit_req zone=deploy` 2 r/s, burst 4, `limit_req_status 429`, `client_max_body_size 200m`, `client_body_timeout 300s` (`infra/gateway/gateway.py:147-149,259-260`).

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
| `READ_ONLY=true` | 404 (route not registered; `/entities` matches no route). The default not-found of `@dcl/http-server` (body not in tree). | default | none | `routes.ts:43-45`, `service.ts:63-65` | Read-only toggle (`POST /admin/api/content/read-only`): 403 `text/plain` `Content Server is in read-only mode; deployments are disabled` (`writes.rs:29,56-58`) | DIVERGENT: 403 text instead of 404 |
| (no upstream equivalent) writes switch off | n/a | n/a | n/a | n/a | 503 `{"error":"content writes are disabled on this node (CONTENT_WRITES_ENABLED=false): ..."}`, `Retry-After: 3600` (`writes.rs:26,40-54`); restore-stamp variants (`writes.rs:46-54`) | DIVERGENT: a node with writes off does not look like a read-only catalyst (404) |
| burst limit exceeded | 429 | `{"error":"Too many requests"}` | `Retry-After` (and the component's `RateLimit-*` headers; set not in tree) | `routes.ts:47-51`, `components.ts:545-556` | No app-level limit. Gateway nginx 429 HTML body, no `Retry-After`, at 2 r/s burst 4 (`gateway.py:149,259-260`). | DIVERGENT: limit (200/min vs 2/s), body (HTML vs JSON), header |
| sync state BOOTSTRAPPING | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | Same body via `AppError::ServiceUnavailable` (`create_entity.rs:290-297`, `writes.rs:31-32`, `errors.rs:51,70-72`), no `Retry-After` | DIVERGENT: `Retry-After` missing |
| unsynced copy, entity type not ready | n/a (upstream gates on BOOTSTRAPPING only) | | | | 503 bootstrapping body after the form is read; 400 `{"errors":["The entity file was not part of the uploaded files or has no type."]}` when the type cannot be read (`create_entity.rs:357-371`) | dcl-one only |
| daily quota exhausted (pre-body marker, or at consume) | 429 | `{"error":"Too many requests"}` | `Retry-After: <s to window end>` | `deployment-quota.ts:28-34,41-59,120-127`; window 86400 s, max 300 (`logic/deployment-quota/component.ts:6,21-25`) | 429 `{"error":"Too many requests"}`, `Retry-After` (`create_entity.rs:260-266,311-315`, `upload_admission.rs:178-248`) | MATCH on the wire. DIVERGENT on counting: see CW-14. |
| declared Content-Length does not fit the memory budget | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `deployment-memory-admission.ts:21-33`, `upload-budget/errors.ts:3`, `multipart.ts:92` | Same body and header (`create_entity.rs:299-310`, `upload_admission.rs:33-34`, `writes.rs:38`) | MATCH (checked before the quota in dcl-one, after it upstream) |
| source has 4 uploads or its byte share in flight | 429 | `{"error":"Too many uploads in progress from this client, please retry shortly."}` | `Retry-After: 5` | `source-upload-admission.ts:9,22-46`, `source-upload-limits/errors.ts:3` | none | MISSING |
| aggregate spool budget full (declared or while growing) | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:141-151,274-284` | Covered by the single in-memory budget above | MATCH (one budget instead of two) |

#### 2b. Multipart parsing (upstream `multipart.ts`, mapped by `middlewares.ts:41-82`)

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Content-Type not `multipart/form-data` | 400 | `{"error":"Invalid request: expected a multipart/form-data body"}` | | `multipart.ts:196-202` | axum `Multipart` rejection: 400 `text/plain` `` Invalid `boundary` for `multipart/form-data` request ``, before every gate (`tests/fixtures/content/create-entity-400-not-multipart.json`, `write_path_tests.rs:225-236`) | DIVERGENT: body and content-type. The fixture says "Upstream 500s", which is not what the pinned revision does. |
| declared Content-Length > 2 GiB + framing | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes plus multipart framing (<n> bytes in all)."}` | | `multipart.ts:133-136,161-166` | Body over 200 MiB: the axum length limit surfaces as a read error and is mapped to 400 `{"error":"Failed to read file data: ..."}` or `"Failed to read multipart field: ..."` (`create_entity.rs:171,185`, `lib.rs:680`). Gateway: nginx 413 HTML over 200m. | DIVERGENT: status 400 vs 413, limit 200 MiB vs 2 GiB, message |
| received bytes > `MAX_UPLOAD_TOTAL_SIZE` | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes."}` | | `multipart.ts:259-270` | as above | DIVERGENT |
| more than 3000 files | 413 | `{"error":"Too many files in the request. The maximum allowed is 3000."}` | | `multipart.ts:296-298` | 400 `{"error":"deployment exceeds maximum of 1000 files"}` (`create_entity.rs:44,176-180`) | DIVERGENT: status, limit, message |
| file > 100 MiB | 413 | `{"error":"File '<filename>' is too large. The maximum allowed size per file is 104857600 bytes."}` | | `multipart.ts:378-386` | 400 `{"error":"file <field name> exceeds 52428800 bytes"}` (`create_entity.rs:45,187-191`) | DIVERGENT: status, limit, message (field name vs filename) |
| more than 32 fields | 413 | `{"error":"Too many form fields in the request. The maximum allowed is 32."}` | | `multipart.ts:301-307` | 400, same text (`create_entity.rs:197-201`) | DIVERGENT: status only |
| field > 32 KiB | 413 | `{"error":"Field '<name>' is too large. The maximum allowed size per field is 32768 bytes."}` | | `multipart.ts:314-321` | 400, same text (`create_entity.rs:209-213`) | DIVERGENT: status only |
| repeated field name | 400 | `{"error":"Duplicate form field '<name>'"}` | | `multipart.ts:223-232` | Accepted. The last value wins and repeats count toward the field limit (`create_entity.rs:196-218`). | DIVERGENT |
| body still arriving after 5 min | 408 | `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}` | | `multipart.ts:110-117,437-450` | Optional `TimeoutLayer` 408 with an empty body when `request_timeout_secs` > 0 (`lib.rs:232-235,715-720`). Gateway `client_body_timeout 300s` gives an nginx 408. | DIVERGENT: body |
| malformed multipart (parser error) | 400 | `{"error":"Invalid multipart/form-data request"}` | | `multipart.ts:455-470` | 400 `{"error":"Failed to read multipart field: <multer error>"}` / `"Failed to read file data: ..."` / `"Failed to read field value: ..."` (`create_entity.rs:168-217`) | DIVERGENT: message (leaks parser detail) |
| spool write/folder failure | 503 | `{"error":"The server cannot store uploads right now, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:94,169-176` | n/a (no spool, forms held in memory) | n/a |
| spool flush did not finish in time | 503 | `{"error":"The upload could not be stored in time, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:476-485` | n/a | n/a |

#### 2c. Handler: form fields and auth chain (`create-entity-handler.ts`)

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `?partial=true` without the form field `partial=true` | 400 | `{"error":"The 'partial=true' query parameter requires the 'partial=true' form field"}` | `deployment-quota.ts:116-119` | Query ignored, deployed as regular | MISSING |
| no `entityId` | 400 | `{"error":"Missing required field: entityId"}` | `:73-76` | `{"error":"Missing entityId field"}` (`create_entity.rs:353-356`) | DIVERGENT: wording |
| no `authChain` field and no `authChain[i]` fields | 400 | `{"error":"No auth chain can be derivated"}` | `:89-91` | `{"error":"No auth chain can be derived"}`. A legacy `ethAddress`+`signature` pair is accepted as a SIGNER + ECDSA_SIGNED_ENTITY chain (`create_entity.rs:140-146`). | DIVERGENT: wording; dcl-one accepts a form upstream refuses |
| `authChain` not JSON or not an array | 400 | `{"error":"Invalid auth chain"}` | `:434-446` | same (`create_entity.rs:61-66`) | MATCH |
| chain longer than 10 (JSON or indexed) | 400 | `{"error":"Auth chain is too long; the maximum allowed is 10 elements"}` | `:450-452,472-474` | same (`create_entity.rs:67-69,87-89,115-117`; `library/dcl-auth/src/auth_chain.rs:6`) | MATCH |
| indexed link missing payload/signature/type | 400 | `{"error":"Missing auth chain element at index <i>"}` | `:477-484` | same (`create_entity.rs:105-113`) | MATCH |
| chain fails `AuthChain.validate` (e.g. `[]`, a link without `type`/`payload`, an unknown `type`) | 400 | `{"error":"Invalid auth chain"}` | `:92-94` | A link without `type`: `{"error":"invalid auth chain format: each link must have a \"type\" field"}`. A link without `payload`: `"... \"payload\" field"` (`create_entity.rs:122-139`). `[]` passes and fails later inside `{errors}`. | DIVERGENT |
| entity already deployed (any `entityId` present in `deployments`) | 200 | `{"creationTimestamp": <stored localTimestamp>}`, answered before the signature, TTL and rate-limit checks | `:98-109`, `deployment-service/component.ts:453-454,461-468` | No early replay. TTL (20 min), the per-pointer rate limit (3-20 s), validation and the newer-entity check run first. Then `persist` answers `creationTimestamp: now_ms` (`write_deployer.rs:654-683,1034-1046`). | DIVERGENT: a retry within the rate-limit window gets 400 `Entity rate limited`; one after 20 min gets 400 `not recent enough`; one overwritten since gets 400 newer-entity; the timestamp is the current time, not the original |

#### 2d. Deployment validation (`deployment-service/component.ts`, `server-validator.ts`, `@dcl/content-validator`)

Upstream order:
1. Read the entity.
2. Signature.
3. Pointers / conflict.
4. Server checks: newer, already deployed, rate limit, TTL back, TTL forward.
5. Protocol validator.
6. Current access (scene older than the TTL).
7. Store.

dcl-one order (`write_deployer.rs:607-760`):
1. Hash.
2. Entity file.
3. Parse.
4. Id match.
5. Denylist.
6. TTL back.
7. TTL forward.
8. Rate limit.
9. Validator (structure, hashing, metadata, ADR-45, signature, size, ..., access).
10. Reserve rate limit.
11. Persist (newer-entity check inside the SQL).

When more than one check fails, the order decides which message is returned.

All rows below are 400 `{"errors":[...]}` upstream unless the row says otherwise.

| trigger | upstream message | upstream ref | dcl-one | verdict |
|---|---|---|---|---|
| no uploaded file hashes to `entityId` | `Failed to find the entity file.` | `component.ts:402-405,427-430` | `The entity file was not part of the uploaded files.` (`write_deployer.rs:621-628`) | DIVERGENT: wording |
| entity file not JSON / bad type / bad pointers / bad timestamp / bad content | `There was a problem parsing the entity` (`InvalidEntityError` text is logged only) | `component.ts:406-415`, `entities/component.ts:99-156` | `There was a problem parsing the entity: <serde error>` (`write_deployer.rs:630-631`) | DIVERGENT: suffix leaks the serde message |
| entity file `id` field differs from `entityId` | accepted (id is taken from the request) | `entities/component.ts:126-128` | `Entity id does not match the uploaded entity file.` (`write_deployer.rs:636-642`) | DIVERGENT: dcl-one refuses |
| signature invalid at `entity.timestamp` | `The signature is invalid. <@dcl/crypto message>` | `create-entity-handler.ts:159-170`, `crypto/component.ts:25-32` | `The signature is invalid. <dcl-auth AuthError Display>` from the validator, after the structure, metadata and ADR-45 rules (`validator/rules.rs:249-264`, `library/dcl-auth/src/error.rs:4-38`) | DIVERGENT: tail wording (not pinned upstream) and order |
| contract-wallet (EIP-1654) link cannot be checked | 400 with the `@dcl/crypto` message | same | 503 `{"errors":["The signature is invalid. EIP-1654 ..."]}`, `Retry-After: 30` (`validator/rules.rs:258-261`, `create_entity.rs:236-252`) | DIVERGENT (intentional) |
| `pointers: []` | `The entity does not have any pointer.` | `component.ts:473-477` | `The entity needs to be pointed by one or more pointers.` (validator, `validator/rules.rs:200`) | DIVERGENT: wording |
| same pointers in flight in this process | `The following pointers are currently being deployed: '<p1,p2>'. Please try again in a few seconds.` | `component.ts:33,480-490` | No in-process conflict check. The concurrent deploy hits the rate-limit reservation: `Entity rate limited (entityId=.. pointers=..).` (`write_deployer.rs:741-748`) | DIVERGENT |
| newer entity on any pointer | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).` | `server-validator.ts:24-28,77-78` | `There is a newer entity pointed by one or more of the pointers you provided.` (`write_deployer.rs:778-779,1034-1037`) | DIVERGENT: suffix missing |
| already deployed (reached only on a race) | `This entity was already deployed. You can't redeploy it` | `server-validator.ts:80-81` | treated as success (`write_deployer.rs:1039-1045`) | DIVERGENT (race only) |
| pointer rate-limited | `Entity rate limited (entityId=<id> pointers=<p,..>).` | `server-validator.ts:83-87` | same text (`write_deployer.rs:664-683`) | MATCH on text. Windows: upstream code default 60 s / max 300 keys per type, env `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`_MAX_<TYPE>` (`component.ts:109-121`, `Environment.ts:537-542,775-790`); dcl-one hard-codes profile 3 s/500, scene 20 s/1000, wearable 20 s/1000, store 3 s/300, emote 20 s/1000, outfits 3 s/2000 (`write_deployer.rs:276-283`). Profile unchanged-content 5 min in both. |
| `now - timestamp > 20 min` | `The request is not recent enough, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:89-94`, `Environment.ts:564-565` | `The request is not recent enough, please submit it again with a new timestamp.` (`write_deployer.rs:653-659`) | DIVERGENT: suffix missing; TTL not configurable |
| `timestamp - now > 15 min` | `The request is too far in the future, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:8,96-101` | `The request timestamp is too far in the future.` (`write_deployer.rs:661-663`) | DIVERGENT: wording |
| entity id, content hash or pointer on the operator denylist | accepted (the denylist filters reads only) | `component.ts:305-312` | `The entity <id> is denylisted on this node.` / `These content files are denylisted on this node: ..` / `These pointers are denylisted on this node: ..` (`denylist.rs:87-117`, `write_deployer.rs:644-650`) | DIVERGENT: dcl-one-only refusal |
| two uploaded files map to one declared hash with different bytes | n/a | | `two different uploaded files map to the same content hash <h>` (`write_deployer.rs:704-708`) | dcl-one only |
| content-validator rejections (structure, IPFS hashing, metadata schema, ADR-45, size per pointer, content cross-check, profile/scene/outfits/items rules, access) | `@dcl/content-validator` strings | `component.ts:362-369` | Port in `services/catalyst/content/src/validator/rules.rs`. Spot-checked strings equal to the local clone: `There are repeated pointers in your request.`; `The entity needs to be pointed by one or more pointers.`; `This hash '<h>' is not valid. It should be IPFS v2 format.`; `The deployment is too big. The maximum allowed size per pointer is ..`; `This hash is referenced in the entity but was not uploaded or previously available: <h>`; `This hash was uploaded but is not referenced in the entity: <h>`; `The metadata for this entity type (<t>) is not valid.`; `Type <t> is not supported yet` (`rules.rs:200,207,229,277,317,712-730`, `metadata_schema.rs:45`) | MATCH for the strings checked; the rest is not compared (upstream source not in the pinned tree) |
| access checker fails (RPC down / timeout) | rejected inside `{errors}` (400) with the validator's `Error checking ...` text | `@dcl/content-validator` | 503 `{"errors":["Error checking ..."]}`, `Retry-After: 30` (`validator/mod.rs:23-27,332-344`, `rules.rs:908`) | DIVERGENT (intentional) |
| content store unreadable | n/a (throws, then the generic error) | | 503 `This node could not read its own content store, so the deployment could not be validated. Please retry. Cause: <cause>` (`validator/rules.rs:56-61`) | DIVERGENT: leaks the cause |
| scene finalized older than the TTL, current access lost | `The deployer no longer has access to the entity pointers (access is required both when a partial upload starts and when it is finalized).` | `component.ts:384-395` | n/a (reachable only from partial uploads) | MISSING (with partial uploads) |
| commit after the partial-upload deadline | `This upload expired before it could be published. Create a new entity with a fresh timestamp.` | `deployment-service/errors.ts:4`, `component.ts:595-597` | n/a | MISSING (with partial uploads) |
| internal error while storing | 400 `{"errors":["There was an error deploying the entity"]}` or `["An internal server error occurred. This will raise an automatic alarm."]` | `component.ts:530-541,598-602` | 503 `{"errors":["deployment write failed: <sqlx error>"]}` / `["failed to store deployment file <h>: <io error>"]` / `["Upload hashing capacity is unavailable."]`, `Retry-After: 30` (`write_deployer.rs:84,138,1026-1029`) | DIVERGENT: status, and internal error text reaches the client |
| content lock busy (GC) | 503 `{"error":"The content lock for entity <id> is still busy, please retry shortly."}`, `Retry-After: 10` | `create-entity-handler.ts:409-418`, `errors.ts:39`, `content-locks/errors.ts:1-7` | n/a (no shared content lock) | n/a |
| spool read failure (transient errno) | 503 `{"error":"The server cannot store uploads right now, please retry shortly."}`, `Retry-After: 30` | `create-entity-handler.ts:321-335` | n/a | n/a |
| unexpected exception | 500 `{"error":"Internal Server Error"}` | `middlewares.ts:72-81` | 500 `{"error":"Internal Server Error"}` (`errors.rs:52-58`) | MATCH |

#### 2e. Success

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| new deployment committed | 200 | `{"creationTimestamp": number}` = the stored `localTimestamp` (ms), the same value `/deployments` and `/audit` show | `Content-Type: application/json` | `create-entity-handler.ts:337-349`, `component.ts:206-210,604` | `{"creationTimestamp": now_ms}` read from the Rust clock after the commit. The stored `local_timestamp` is PostgreSQL `now()` inside the statement (`create_entity.rs:391-408`, `write_deployer.rs:831,1031,1056-1059`). | DIVERGENT: the value differs from the stored `localTimestamp` by the statement duration |

### Visibility after a successful deploy

- **Upstream:** writes `active_pointers` in the deploy transaction. After the commit it updates the in-process active-entities cache and invalidates the collection-prefix caches (`component.ts:222-290`). A `POST /entities/active` right after the 200 returns the new entity, on this process and on any process reading the database.
- **dcl-one:**
  - Commits deployment, content files, active pointers and overwrites in one statement (`write_deployer.rs:794-946`).
  - `POST /entities/active` and the collection listing read `state.pool` directly, with no in-process entity cache (`active_entities.rs:63-92`). The entity is visible on the next request on every instance. Verdict: MATCH.
  - `deployments_cache` (5 s TTL, `lib.rs:119`) is cleared only on the instance that took the deploy (`create_entity.rs:393`). Other instances can serve `/deployments`, `/pointer-changes` and `/snapshots` up to 5 s stale. That belongs to the read area.
- Upstream sends `Cache-Control: public, max-age=<ENTITIES_CACHE_CONTROL_MAX_AGE>` (default 10) only on `GET /entities/:type`; `POST /entities/active` carries no cache header (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:36-37`). dcl-one sends `public, max-age=10` on both (`services/catalyst/content/src/active_entities.rs:125`; CR-8 in `content-reads.md`). Shared caches do not store POST responses, so in both trees only the GET read can be served up to 10 s stale.

### Data dependency

- **Content DB:**
  - `content.deployments`, `content_files`, `active_pointers`;
  - `content.local_entities` (dcl-one provenance);
  - `content.forward_outbox` (dcl-one forwarding).
- **Blob store:** for content existence (`isContentStoredAlready`) and stored sizes (size rule).
- **Ownership:**
  - Upstream: on-chain (L1/L2 RPC: Checker contract, collection and third-party registry contracts) when `L1_HTTP_PROVIDER_URL` and `L2_HTTP_PROVIDER_URL` are set, else the subgraphs (land manager, blocks L1/L2, collections L1/L2, ENS owner, third-party registry) (`content-validator/component.ts:238-270`).
  - dcl-one: on-chain only, `ETH_RPC_URL` and `RPC_ENDPOINT_POLYGON` (`validator/mod.rs:14-20`), plus the `shop.usage_grants` lease overlay (dcl-one-only grants). The `land_authz` index is used only by the test path without the validator (`write_deployer.rs:443-487`).
  - dcl-one has these sources when the RPC URLs are configured. With `CONTENT_WRITES_ENABLED` on, a missing or plaintext RPC refuses to start (`validator/mod.rs:108-130`).
- **Sync state:** the bootstrapping gate needs the sync status. dcl-one: `SyncStatus` (`lib.rs:390-410`).

---

## 3. `POST /entities` with `partial=true` (multi-request upload, ADR-325)

### Request

- **Same route and middleware.** `?partial=true` skips the pre-body daily-quota check and the memory admission (`deployment-quota.ts:24-26,47`, `deployment-memory-admission.ts:22`).
- **Form:** `partial=true` field. File field names must be content hashes. The first batch must carry the entity file, at most 10 MiB (`partial-deployments/component.ts:63`). Only IPFS v2 scene entities are accepted.
- **Limits:**
  - `PENDING_DEPLOYMENT_TTL` 1 h;
  - `MAX_PENDING_DEPLOYMENTS_PER_DEPLOYER` 10;
  - `MAX_PENDING_UPLOADS_PER_POINTER` 4;
  - `MAX_PENDING_BYTES_PER_DEPLOYER` 1 GiB;
  - `MAX_PENDING_BYTES` 50 GiB;
  - `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` 512 MiB (`Environment.ts:667-699`).

### Responses

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| batch stored, content still missing | 202 | `{"missing": ["<hash>", ...]}` | | `create-entity-handler.ts:233-240` | none | MISSING |
| batch completes the upload | 200 | `{"creationTimestamp": number}` | | `create-entity-handler.ts:223-231` | none | MISSING |
| replay of a published entity | 200 | `{"creationTimestamp": number}` | | `create-entity-handler.ts:100-109` | none | MISSING |
| source quota spent, not the signer's own live upload | 429 | `{"errors":["The daily deployment quota for this client is exhausted. Only uploads in progress can continue."]}` | `Retry-After` | `create-entity-handler.ts:111-126` | none | MISSING |
| signature invalid | 400 | `{"errors":["The signature is invalid. <msg>"]}` | | `:159-170,174-197` | none | MISSING |
| entity id not IPFS v2 | 400 | `{"errors":["The entity id '<id>' is not a valid IPFS v2 hash. Partial deployments require IPFS v2 entities."]}` | | `partial-deployments/component.ts:694-697` | none | MISSING |
| file hash ≠ field name | 400 | `The uploaded file '<key>' does not match its content hash (computed <h>).` | | `:707-711` | none | MISSING |
| entity file > 10 MiB | 400 | `The entity file '<id>' is too large (<n> bytes, max 10485760).` | | `:721-725` | none | MISSING |
| no entity file on first batch / other signer | 400 | `The first partial request for an entity, and any request by another signer, must include the entity file '<id>'.` | | `:431-433` | none | MISSING |
| entity unparsable / not a scene / no pointer | 400 | `There was a problem parsing the entity: <e>` / `Partial deployments are only supported for scenes, but the entity type is '<t>'.` / `The entity does not have any pointer.` | | `:739-747` | none | MISSING |
| staging validation fails | 400 | validator errors, or `The staging validation was not successful.` | | `:771` | none | MISSING |
| newer entity / rate limit / TTL | 400 / 429 / 400 | `newerEntityMessage`; `Entity rate limited (...)` (429, `Retry-After`); `The request is not recent enough, please submit it again with a new timestamp (entityId=<id>).`; `The request is too far in the future, please submit it again with a new timestamp (entityId=<id>).` | | `:775-802` | none | MISSING |
| upload started by another account / expired / gone | 400 | `This upload was started by another account.` / `This upload expired. Create a new entity with a fresh timestamp.` / `Upload no longer exists; resend its manifest.` | | `:250,490,547` | none | MISSING |
| per-account / per-pointer upload count | 429 | `Too many partial uploads in progress for this account (max 10). Complete an upload or wait for expired uploads to be cleaned up.`; `Too many partial uploads in progress for parcels <p..> (max 4 per parcel). ...`; `Too many partial uploads in progress for this account on parcels <p..> (max <n> per parcel). ...` | `Retry-After` | `:122-140,498` | none | MISSING |
| byte budgets | 400 / 429 | `This batch is <n> bytes, over the partial upload byte rate limit of <m> bytes per minute. Send it in smaller batches.` (400); `Partial upload byte rate exceeded for this account: ...` (429); `This upload needs <n> bytes of partial upload storage, over the <limit> budget of <m> bytes.` (400); `Partial upload storage budget exceeded for this account: ...` (429); `Partial upload storage on this server is full. Retry later.` (429); `Deployment failed: The deployment is too big.` (400) | `Retry-After` on 429 | `:558-606,583,891` | none | MISSING |
| publication failed server-side | 503 | `{"errors":[...]}` | `Retry-After` | `create-entity-handler.ts:266-278` | none | MISSING |
| lock busy / memory budget | 503 | `{"error":"<message>"}` | `Retry-After: 10` / `30` | `:279-284` | none | MISSING |

**What dcl-one answers today.** A partial first batch that leaves out stored-elsewhere files is validated as a complete deployment. It answers 400 `{"errors":["This hash is referenced in the entity but was not uploaded or previously available: <h>", ...]}` instead of 202 `{"missing":[...]}`. A batch without the entity file gets 400 `The entity file was not part of the uploaded files.`.

**Data dependency.**
- Upstream: a pending-deployments table (`pendingDeploymentsRepository`), staged blobs, and per-account accounting.
- dcl-one has none of these.

---

## 4. `OPTIONS /entities` (CORS preflight)

| trigger | status | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| preflight | 204 (koa-cors default) | `Access-Control-Allow-Origin: *`; methods `GET, HEAD, POST, OPTIONS`; request headers reflected; `Access-Control-Max-Age: 600`; no credentials; responses `Access-Control-Expose-Headers: *` | `components.ts:489-506` | 204; `Access-Control-Allow-Origin: <Origin>` with `Access-Control-Allow-Credentials: true`; methods `GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS`; request headers reflected; `Access-Control-Max-Age: 86400`; `Vary: Origin`; expose `*` (`shared/src/cors.rs:15-79`) | DIVERGENT. This is shared CORS behaviour; `Retry-After` stays readable to browser deployers in both (expose `*`, no credentialed request). |

## 5. Other methods on `/entities`

| trigger | status | upstream | dcl-one | verdict |
|---|---|---|---|---|
| GET/HEAD/PUT/DELETE `/entities` | 405 with `Allow: POST` (`router.allowedMethods()`, `service.ts:63-65`); 404 in READ_ONLY | axum `MethodRouter` 405 with `Allow: POST` (`lib.rs:677-681`); the read-only toggle keeps the route (405) | MATCH outside READ_ONLY (read; upstream body not in tree) |

---

## 6. Backlog

1. **CW-1: `POST /entities` partial uploads.**
   - Gap: `partial=true` (form) and `?partial=true` (query) are not implemented. dcl-one never answers 202 `{"missing":[...]}`.
   - Expected: the ADR-325 protocol in `docs/PARTIAL_UPLOADS.md` and `create-entity-handler.ts:96-131,174-295`.
     - 202 `{"missing": [hash...]}` for a stored batch.
     - 200 `{"creationTimestamp"}` on completion or replay.
     - 400 when the query flag comes without the form field: `The 'partial=true' query parameter requires the 'partial=true' form field` (`deployment-quota.ts:116-119`).
     - The 400/429/503 strings in section 3.
     - The quota rules: an upload counts once when created; a source that spent the quota may resume its own uploads only.
     - Staging: IPFS v2 scenes only; entity file at most 10 MiB; files keyed by hash; TTL 1 h; per-account, per-pointer and byte quotas with `Retry-After`.
   - Place: `services/catalyst/content/src/create_entity.rs` (field `partial`, query flag), a new staging path in `write_deployer.rs`, a pending-uploads table under `schema/` with a migration, `upload_admission.rs` (skip the memory admission for declared partial batches).
   - Severity: P0. SDK and Creator Hub deploys of large scenes send 32 MiB partial batches and fail against dcl-one.

2. **CW-2: replay idempotency.**
   - Gap: dcl-one runs TTL, rate limit, validation and the newer-entity check before noticing that the entity is already deployed. A retried deploy can get 400 `Entity rate limited (...)` within 3-20 s, 400 `not recent enough` after 20 min, or 400 newer-entity once overwritten. When it succeeds it answers `now_ms`.
   - Expected: look up `entityId` in `content.deployments` right after the auth-chain parse. If found, answer 200 `{"creationTimestamp": <stored local_timestamp ms>}` with no signature, TTL or rate-limit check (`create-entity-handler.ts:98-109`, `component.ts:453-454`).
   - Place: `create_entity.rs:deploy_form` before `deploy_entity`, or the start of `write_deployer.rs:607`. Also `persist` (`write_deployer.rs:1039-1045`) should return the stored `local_timestamp`.
   - Severity: P1.

3. **CW-3: `creationTimestamp` value.**
   - Gap: the value comes from the Rust clock after the commit. The stored `local_timestamp` is PostgreSQL `now()`.
   - Expected: `creationTimestamp` equals the stored `localTimestamp`, as `/deployments` and `/audit` report it (`component.ts:206-210,604`).
   - Place: make the deploy function return `local_timestamp` (`write_deployer.rs:831` `now()`, `PersistOutcome` at `write_deployer.rs:782-788`, `1031-1059`).
   - Severity: P1.

4. **CW-4: read-only mode.**
   - Gap: the read-only toggle answers 403 `text/plain`. Writes-off answers 503 with `Retry-After: 3600`.
   - Expected: upstream READ_ONLY does not register `POST /entities`, so the request gets the router's 404 (`routes.ts:43-45`). Clients and forwarders read 404/405 as "read-only catalyst"; dcl-one's own forwarder already does (`forward.rs:19-21`).
   - Place: `writes.rs:56-75`. Answer 404 with the content not-found shape for read-only, and decide whether writes-off should also look like READ_ONLY.
   - Severity: P1.

5. **CW-5: bootstrapping `Retry-After`.**
   - Gap: the 503 has no `Retry-After`.
   - Expected: `Retry-After: 60` (`middlewares.ts:17,30-34`).
   - Place: `create_entity.rs:290-297,366-370`. Return a response with the header instead of `AppError::ServiceUnavailable`.
   - Severity: P1.

6. **CW-6: per-client burst limit.**
   - Gap: no app-level limit. The gateway allows 2 r/s with burst 4 and answers an nginx HTML 429 without `Retry-After`.
   - Expected: 200 requests per 60 s per client IP, 429 `{"error":"Too many requests"}` with `Retry-After` (`routes.ts:47-51`, `components.ts:545-556`, `Environment.ts:86-87`).
   - Place: `create_entity.rs` (an in-process limiter beside `DailyQuota` in `upload_admission.rs`) and `infra/gateway/gateway.py:149,259` (loosen or remove `zone=deploy` so it does not cut normal bursts).
   - Severity: P1.

7. **CW-7: multipart limit statuses, values and messages.**
   - Gap: dcl-one answers 400 with other limits and messages.
   - Expected, all 413 `{"error":...}` with these limits (`multipart.ts:133-136,259-321,378-386`, `Environment.ts:35-49`):
     - per file: `File '<filename>' is too large. The maximum allowed size per file is 104857600 bytes.`;
     - files: `Too many files in the request. The maximum allowed is 3000.`;
     - fields: `Too many form fields in the request. The maximum allowed is 32.`;
     - field size: `Field '<name>' is too large. The maximum allowed size per field is 32768 bytes.`;
     - total: `The request body is too large. The maximum allowed total upload size is <n> bytes.`, with the Content-Length pre-check variant `... plus multipart framing (<m> bytes in all).`
   - The 200 MiB body cap can stay, since the infrastructure cuts above about 200 MB upstream too (`docs/PARTIAL_UPLOADS.md` Limits). Over the cap the answer should still be 413 JSON.
   - Place: `create_entity.rs:44-49,159-222`, `lib.rs:680`.
   - Severity: P1.

8. **CW-8: non-multipart body.**
   - Gap: dcl-one answers axum's 400 `text/plain` `` Invalid `boundary` for `multipart/form-data` request ``, before every gate.
   - Expected: 400 `{"error":"Invalid request: expected a multipart/form-data body"}` (`multipart.ts:196-202`), after the burst, bootstrapping and quota gates.
   - Place: `create_entity.rs:278-282` (take `Result<Multipart, MultipartRejection>` and map it) and `services/catalyst/tests/fixtures/content/create-entity-400-not-multipart.json`. The fixture's note that upstream answers 500 does not hold at the pinned revision.
   - Severity: P1.

9. **CW-9: internal error text in responses.**
   - Gap: 503 bodies carry `deployment write failed: <sqlx error>`, `failed to store deployment file <h>: <io error>`, `There was a problem parsing the entity: <serde error>`, `Failed to read multipart field: <multer error>` and `... Cause: <cause>`.
   - Expected: no internal detail.
     - Upstream answers 400 `{"errors":["There was an error deploying the entity"]}` for a store failure (`component.ts:598-602`).
     - Upstream answers `There was a problem parsing the entity` for any parse failure (`component.ts:406-415`).
     - Upstream answers `Invalid multipart/form-data request` for a parser failure (`multipart.ts:469`).
   - Keep the 503/`Retry-After` classification if wanted, with fixed messages.
   - Place: `write_deployer.rs:84,138,631,1029`, `create_entity.rs:171,185,207,217`, `validator/rules.rs:56-61`.
   - Severity: P1.

10. **CW-10: per-source upload admission.**
    - Gap: no limit on concurrent uploads per source.
    - Expected: at most `MAX_CONCURRENT_UPLOADS_PER_SOURCE`=4 bodies and `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` bytes in flight per client, checked before the body. Otherwise 429 `{"error":"Too many uploads in progress from this client, please retry shortly."}` with `Retry-After: 5` (`source-upload-admission.ts:9-47`).
    - Place: `upload_admission.rs` and `create_entity.rs:299-315`.
    - Severity: P2.

11. **CW-11: auth-chain error wording and checks.**
    - Gaps:
      - The no-chain message differs: upstream `No auth chain can be derivated` (`create-entity-handler.ts:90`).
      - dcl-one accepts the legacy `ethAddress`+`signature` pair, which upstream refuses with that 400 (`create_entity.rs:140-146`).
      - Structurally invalid chains get `invalid auth chain format: ...` instead of `{"error":"Invalid auth chain"}` (`AuthChain.validate`, `create-entity-handler.ts:92-94`). `[]` is not refused up front.
    - Place: `create_entity.rs:119-147`.
    - Severity: P2.

12. **CW-12: deployment rejection wording.** Expected strings come from the refs in section 2d. Place: `write_deployer.rs` and `create_entity.rs`. Severity: P2.

    | dcl-one text | upstream text | dcl-one place |
    |---|---|---|
    | `Missing entityId field` | `Missing required field: entityId` | `create_entity.rs:355` |
    | `The entity file was not part of the uploaded files.` | `Failed to find the entity file.` | `write_deployer.rs:627` |
    | `There was a problem parsing the entity: <serde error>` | `There was a problem parsing the entity` | `write_deployer.rs:631` |
    | `The entity needs to be pointed by one or more pointers.` (empty pointers) | `The entity does not have any pointer.`, checked before the validator | validator |
    | newer-entity message without suffix | add ` (entityId=<id> pointers=<p,..>)` before the final period | `write_deployer.rs:778` |
    | `not recent enough` message without suffix | add ` (entityId=<id> pointers=<p,..>)` | `write_deployer.rs:656` |
    | `The request timestamp is too far in the future.` | `The request is too far in the future, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `write_deployer.rs:662` |
    | `Entity rate limited (...)` for a concurrent deploy of the same pointers | `The following pointers are currently being deployed: '<p..>'. Please try again in a few seconds.` | — |

13. **CW-13: check order and dcl-one-only refusals.**
    - Gap, order: upstream checks the signature before the server checks, and the newer-entity check before the rate limit and TTL (`create-entity-handler.ts:144`, `server-validator.ts:76-101`). dcl-one checks TTL and rate limit first and the signature inside the validator.
    - Gap, extra refusals: dcl-one refuses denylisted ids, hashes and pointers (`denylist.rs:87-117`); upstream accepts them. dcl-one refuses an entity file whose `id` differs (`write_deployer.rs:636-642`).
    - Expected: the upstream order, and either the same acceptance or the extra refusals documented as intentional.
    - Place: `write_deployer.rs:607-760`.
    - Severity: P2.

14. **CW-14: daily quota counting.**
    - Gap:
      - dcl-one counts at admission, so requests refused by the multipart parser or form limits are charged; upstream counts after a successful parse (`deployment-quota.ts:111-129`).
      - dcl-one refunds `Unavailable` deploys; upstream does not.
      - dcl-one checks the upload budget before the quota; upstream checks the quota first (`routes.ts:63-64`).
      - dcl-one skips the quota without a gateway-stamped address; upstream falls back to the socket address (`client-source.ts:25`).
    - Place: `create_entity.rs:299-328`, `upload_admission.rs:178-227`.
    - Severity: P2.

15. **CW-15: per-pointer rate-limit configuration.**
    - Gap: TTL and max are hard-coded per type (`write_deployer.rs:276-283`).
    - Expected: upstream code default is 60 s / 300 keys for every type, overridable by `DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL`/`_MAX` and `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`DEPLOYMENT_RATE_LIMIT_MAX_<TYPE>` (`component.ts:109-121`, `Environment.ts:537-542,775-790`). `REQUEST_TTL_BACKWARDS` (20 min) is configurable upstream (`Environment.ts:564-565`).
    - Place: `write_deployer.rs:270-283,598-599`, `services/catalyst/config`.
    - Severity: P2.

16. **CW-16: duplicate form fields and upload timeout body.**
    - Gap: a repeated field name is accepted (last value wins). A slow body gets an empty or nginx 408.
    - Expected: 400 `{"error":"Duplicate form field '<name>'"}` (`multipart.ts:223-232`). 408 `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}` (`multipart.ts:110-117`).
    - Place: `create_entity.rs:159-222`.
    - Severity: P2.

17. **CW-17: undecidable deploys answer 503.**
    - Gap: an EIP-1654 signature that cannot be checked, or an access-checker failure, answers 503 `{"errors"}` with `Retry-After: 30`. Upstream answers 400 `{"errors"}` with the validator text.
    - Expected: keep the 503 if this divergence is intended, but document it as such. Clients that do not retry 503 on POST will report a transient failure where upstream reports a rejection.
    - Place: `validator/mod.rs:332-344`, `create_entity.rs:236-252`.
    - Severity: P2.

18. **CW-18: CORS preflight on `/entities`.**
    - Gap: dcl-one reflects the Origin with credentials, allows a wider method list, and caches the preflight for 86400 s.
    - Expected: `Access-Control-Allow-Origin: *`, no credentials, methods `GET, HEAD, POST, OPTIONS`, max-age 600 (`components.ts:489-506`).
    - Place: `services/catalyst/shared/src/cors.rs:15-79`, shared with every route.
    - Severity: P2.

## 7. Endpoints dcl-one serves in this area that upstream does not

- `DELETE /scenes/{coord}` and `DELETE /content/scenes/{coord}` (`content/src/lib.rs:682`, `unpublish_scene.rs`):
  - Signed-fetch unpublish of a locally published scene by its LAND holder.
  - 200 `{entityId, unpublished: true, parcels}`; 400 / 401 / 403 / 404 / 409 / 503; behind the same `CONTENT_WRITES_ENABLED` and read-only gates.
- `GET /forwarding`, `GET /forwarding/{entityId}`, `POST /forwarding/{entityId}/retry`, and the `/content/` variants (`lib.rs:683-685`, `forward/status.rs:139-200`):
  - Admin-token routes over the upstream-forwarding outbox (`CONTENT_FORWARD_UPSTREAM_URL`).
- `POST /entities` at the root path, besides `/content/entities` (`lib.rs:705-706`). Upstream production serves it under `/content` only.
- The write switch's 503 (`CONTENT_WRITES_ENABLED`) and its restore-stamp variants (`writes.rs:40-54`). There is no upstream counterpart.

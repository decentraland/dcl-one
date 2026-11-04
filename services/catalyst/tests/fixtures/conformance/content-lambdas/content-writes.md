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
- **Client source:**
  - Upstream: `TRUSTED_CLIENT_IP_HEADER`, read from the right with one trusted proxy, else the socket address. Both are canonicalized (`client-source.ts:17-36`).
  - dcl-one: `x-real-ip`, else the last `x-forwarded-for` entry, else none. With no source the quota is skipped (`services/catalyst/shared/src/client_ip.rs:7-23`, `upload_admission.rs:190-197`). IPv6 is grouped by /64 in both (upstream `CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`=64; dcl-one `upload_admission.rs:85-98`).
- **dcl-one chain, in order** (`create_entity.rs:304-517`):
  1. axum `Multipart` extractor: Content-Type check only.
  2. `writes::refuse`: the `CONTENT_WRITES_ENABLED` switch, then read-only (`writes.rs:67-75`).
  3. Deployer attached.
  4. Bootstrapping (`create_entity.rs:317-324`).
  5. Upload budget on declared length, minimum 16 MiB, capacity max(`MAX_IN_MEMORY_DEPLOYMENT_BYTES`, 200 MiB) (`create_entity.rs:326-337`, `upload_admission.rs:53-75`).
  6. Daily quota consume, counted at admission, except for a batch declared with `?partial=true`, which section 3 counts once its form is read (`create_entity.rs:338-350`).
  7. Form read, held in memory: 1000 files, 50 MiB per file, field count/size from settings (default 32 / 32 KiB), body cap 200 MiB (`create_entity.rs:55-60,178-248`, `lib.rs:685`).
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
| sync state BOOTSTRAPPING | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | Same body via `AppError::ServiceUnavailable` (`create_entity.rs:317-324`, `writes.rs:31-32`, `errors.rs:51,70-72`), no `Retry-After` | DIVERGENT: `Retry-After` missing |
| unsynced copy, entity type not ready | n/a (upstream gates on BOOTSTRAPPING only) | | | | 503 bootstrapping body after the form is read; 400 `{"errors":["The entity file was not part of the uploaded files or has no type."]}` when the type cannot be read (`create_entity.rs:453-467`) | dcl-one only |
| daily quota exhausted (pre-body marker, or at consume) | 429 | `{"error":"Too many requests"}` | `Retry-After: <s to window end>` | `deployment-quota.ts:28-34,41-59,120-127`; window 86400 s, max 300 (`logic/deployment-quota/component.ts:6,21-25`) | 429 `{"error":"Too many requests"}`, `Retry-After` (`create_entity.rs:286-292,338-350`, `upload_admission.rs:178-265`) | MATCH on the wire. DIVERGENT on counting: see CW-14. |
| declared Content-Length does not fit the memory budget | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `deployment-memory-admission.ts:21-33`, `upload-budget/errors.ts:3`, `multipart.ts:92` | Same body and header (`create_entity.rs:326-337`, `upload_admission.rs:33-34`, `writes.rs:38`) | MATCH (checked before the quota in dcl-one, after it upstream) |
| source has 4 uploads or its byte share in flight | 429 | `{"error":"Too many uploads in progress from this client, please retry shortly."}` | `Retry-After: 5` | `source-upload-admission.ts:9,22-46`, `source-upload-limits/errors.ts:3` | none | MISSING |
| aggregate spool budget full (declared or while growing) | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:141-151,274-284` | Covered by the single in-memory budget above | MATCH (one budget instead of two) |

#### 2b. Multipart parsing (upstream `multipart.ts`, mapped by `middlewares.ts:41-82`)

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Content-Type not `multipart/form-data` | 400 | `{"error":"Invalid request: expected a multipart/form-data body"}` | | `multipart.ts:196-202` | axum `Multipart` rejection: 400 `text/plain` `` Invalid `boundary` for `multipart/form-data` request ``, before every gate (`tests/fixtures/content/create-entity-400-not-multipart.json`, `write_path_tests.rs:225-236`) | DIVERGENT: body and content-type. The fixture says "Upstream 500s", which is not what the pinned revision does. |
| declared Content-Length > 2 GiB + framing | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes plus multipart framing (<n> bytes in all)."}` | | `multipart.ts:133-136,161-166` | Body over 200 MiB: the axum length limit surfaces as a read error and is mapped to 400 `{"error":"Failed to read file data: ..."}` or `"Failed to read multipart field: ..."` (`create_entity.rs:192,211`, `lib.rs:685`). Gateway: nginx 413 HTML over 200m. | DIVERGENT: status 400 vs 413, limit 200 MiB vs 2 GiB, message |
| received bytes > `MAX_UPLOAD_TOTAL_SIZE` | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes."}` | | `multipart.ts:259-270` | as above | DIVERGENT |
| more than 3000 files | 413 | `{"error":"Too many files in the request. The maximum allowed is 3000."}` | | `multipart.ts:296-298` | 400 `{"error":"deployment exceeds maximum of 1000 files"}` (`create_entity.rs:55,202-206`) | DIVERGENT: status, limit, message |
| file > 100 MiB | 413 | `{"error":"File '<filename>' is too large. The maximum allowed size per file is 104857600 bytes."}` | | `multipart.ts:378-386` | 400 `{"error":"file <field name> exceeds 52428800 bytes"}` (`create_entity.rs:56,213-217`) | DIVERGENT: status, limit, message (field name vs filename) |
| more than 32 fields | 413 | `{"error":"Too many form fields in the request. The maximum allowed is 32."}` | | `multipart.ts:301-307` | 400, same text (`create_entity.rs:223-227`) | DIVERGENT: status only |
| field > 32 KiB | 413 | `{"error":"Field '<name>' is too large. The maximum allowed size per field is 32768 bytes."}` | | `multipart.ts:314-321` | 400, same text (`create_entity.rs:235-239`) | DIVERGENT: status only |
| repeated field name | 400 | `{"error":"Duplicate form field '<name>'"}` | | `multipart.ts:223-232` | Accepted. The last value wins and repeats count toward the field limit (`create_entity.rs:222-244`). | DIVERGENT |
| body still arriving after 5 min | 408 | `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}` | | `multipart.ts:110-117,437-450` | Optional `TimeoutLayer` 408 with an empty body when `request_timeout_secs` > 0 (`lib.rs:235-238,720-725`). Gateway `client_body_timeout 300s` gives an nginx 408. | DIVERGENT: body |
| malformed multipart (parser error) | 400 | `{"error":"Invalid multipart/form-data request"}` | | `multipart.ts:455-470` | 400 `{"error":"Failed to read multipart field: <multer error>"}` / `"Failed to read file data: ..."` / `"Failed to read field value: ..."` (`create_entity.rs:189-243`) | DIVERGENT: message (leaks parser detail) |
| spool write/folder failure | 503 | `{"error":"The server cannot store uploads right now, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:94,169-176` | n/a (no spool, forms held in memory) | n/a |
| spool flush did not finish in time | 503 | `{"error":"The upload could not be stored in time, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:476-485` | n/a | n/a |

#### 2c. Handler: form fields and auth chain (`create-entity-handler.ts`)

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `?partial=true` without the form field `partial=true` | 400 | `{"error":"The 'partial=true' query parameter requires the 'partial=true' form field"}` | `deployment-quota.ts:116-119` | Same body, after the form is read and after the `entityId` check, so a form without `entityId` gets `Missing entityId field` first (`create_entity.rs:442-451`) | MATCH (order differs only for a form missing `entityId`) |
| no `entityId` | 400 | `{"error":"Missing required field: entityId"}` | `:73-76` | `{"error":"Missing entityId field"}` (`create_entity.rs:442-445`) | DIVERGENT: wording |
| no `authChain` field and no `authChain[i]` fields | 400 | `{"error":"No auth chain can be derivated"}` | `:89-91` | `{"error":"No auth chain can be derived"}`. A legacy `ethAddress`+`signature` pair is accepted as a SIGNER + ECDSA_SIGNED_ENTITY chain (`create_entity.rs:151-157`). | DIVERGENT: wording; dcl-one accepts a form upstream refuses |
| `authChain` not JSON or not an array | 400 | `{"error":"Invalid auth chain"}` | `:434-446` | same (`create_entity.rs:72-77`) | MATCH |
| chain longer than 10 (JSON or indexed) | 400 | `{"error":"Auth chain is too long; the maximum allowed is 10 elements"}` | `:450-452,472-474` | same (`create_entity.rs:78-80,98-100,126-128`; `library/dcl-auth/src/auth_chain.rs:6`) | MATCH |
| indexed link missing payload/signature/type | 400 | `{"error":"Missing auth chain element at index <i>"}` | `:477-484` | same (`create_entity.rs:116-124`) | MATCH |
| chain fails `AuthChain.validate` (e.g. `[]`, a link without `type`/`payload`, an unknown `type`) | 400 | `{"error":"Invalid auth chain"}` | `:92-94` | A link without `type`: `{"error":"invalid auth chain format: each link must have a \"type\" field"}`. A link without `payload`: `"... \"payload\" field"` (`create_entity.rs:133-150`). `[]` passes and fails later inside `{errors}`. | DIVERGENT |
| entity already deployed (any `entityId` present in `deployments`) | 200 | `{"creationTimestamp": <stored localTimestamp>}`, answered before the signature, TTL and rate-limit checks | `:98-109`, `deployment-service/component.ts:453-454,461-468` | No early replay. TTL (20 min), the per-pointer rate limit (3-20 s), validation and the newer-entity check run first. Then `persist` answers `creationTimestamp: now_ms` (`write_deployer.rs:783-812,1163-1175`). | DIVERGENT: a retry within the rate-limit window gets 400 `Entity rate limited`; one after 20 min gets 400 `not recent enough`; one overwritten since gets 400 newer-entity; the timestamp is the current time, not the original |

#### 2d. Deployment validation (`deployment-service/component.ts`, `server-validator.ts`, `@dcl/content-validator`)

Upstream order:
1. Read the entity.
2. Signature.
3. Pointers / conflict.
4. Server checks: newer, already deployed, rate limit, TTL back, TTL forward.
5. Protocol validator.
6. Current access (scene older than the TTL).
7. Store.

dcl-one order (`write_deployer.rs:721-889`):
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
| no uploaded file hashes to `entityId` | `Failed to find the entity file.` | `component.ts:402-405,427-430` | `The entity file was not part of the uploaded files.` (`write_deployer.rs:750-757`) | DIVERGENT: wording |
| entity file not JSON / bad type / bad pointers / bad timestamp / bad content | `There was a problem parsing the entity` (`InvalidEntityError` text is logged only) | `component.ts:406-415`, `entities/component.ts:99-156` | `There was a problem parsing the entity: <serde error>` (`write_deployer.rs:759-760`) | DIVERGENT: suffix leaks the serde message |
| entity file `id` field differs from `entityId` | accepted (id is taken from the request) | `entities/component.ts:126-128` | `Entity id does not match the uploaded entity file.` (`write_deployer.rs:765-771`) | DIVERGENT: dcl-one refuses |
| signature invalid at `entity.timestamp` | `The signature is invalid. <@dcl/crypto message>` | `create-entity-handler.ts:159-170`, `crypto/component.ts:25-32` | `The signature is invalid. <dcl-auth AuthError Display>` from the validator, after the structure, metadata and ADR-45 rules (`validator/rules.rs:299-314`, `library/dcl-auth/src/error.rs:4-38`) | DIVERGENT: tail wording (not pinned upstream) and order |
| contract-wallet (EIP-1654) link cannot be checked | 400 with the `@dcl/crypto` message | same | 503 `{"errors":["The signature is invalid. EIP-1654 ..."]}`, `Retry-After: 30` (`validator/rules.rs:308-311`, `create_entity.rs:262-278`) | DIVERGENT (intentional) |
| `pointers: []` | `The entity does not have any pointer.` | `component.ts:473-477` | `The entity needs to be pointed by one or more pointers.` (validator, `validator/rules.rs:250`) | DIVERGENT: wording |
| same pointers in flight in this process | `The following pointers are currently being deployed: '<p1,p2>'. Please try again in a few seconds.` | `component.ts:33,480-490` | No in-process conflict check. The concurrent deploy hits the rate-limit reservation: `Entity rate limited (entityId=.. pointers=..).` (`write_deployer.rs:870-877`) | DIVERGENT |
| newer entity on any pointer | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).` | `server-validator.ts:24-28,77-78` | `There is a newer entity pointed by one or more of the pointers you provided.` (`write_deployer.rs:907-908,1163-1166`) | DIVERGENT: suffix missing |
| already deployed (reached only on a race) | `This entity was already deployed. You can't redeploy it` | `server-validator.ts:80-81` | treated as success (`write_deployer.rs:1168-1174`) | DIVERGENT (race only) |
| pointer rate-limited | `Entity rate limited (entityId=<id> pointers=<p,..>).` | `server-validator.ts:83-87` | same text (`write_deployer.rs:793-812`) | MATCH on text. Windows: upstream code default 60 s / max 300 keys per type, env `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`_MAX_<TYPE>` (`component.ts:109-121`, `Environment.ts:537-542,775-790`); dcl-one hard-codes profile 3 s/500, scene 20 s/1000, wearable 20 s/1000, store 3 s/300, emote 20 s/1000, outfits 3 s/2000 (`write_deployer.rs:292-299`). Profile unchanged-content 5 min in both. |
| `now - timestamp > 20 min` | `The request is not recent enough, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:89-94`, `Environment.ts:564-565` | `The request is not recent enough, please submit it again with a new timestamp.` (`write_deployer.rs:782-788`) | DIVERGENT: suffix missing; TTL not configurable |
| `timestamp - now > 15 min` | `The request is too far in the future, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:8,96-101` | `The request timestamp is too far in the future.` (`write_deployer.rs:790-792`) | DIVERGENT: wording |
| entity id, content hash or pointer on the operator denylist | accepted (the denylist filters reads only) | `component.ts:305-312` | `The entity <id> is denylisted on this node.` / `These content files are denylisted on this node: ..` / `These pointers are denylisted on this node: ..` (`denylist.rs:87-117`, `write_deployer.rs:773-779`) | DIVERGENT: dcl-one-only refusal |
| two uploaded files map to one declared hash with different bytes | n/a | | `two different uploaded files map to the same content hash <h>` (`write_deployer.rs:833-837`) | dcl-one only |
| content-validator rejections (structure, IPFS hashing, metadata schema, ADR-45, size per pointer, content cross-check, profile/scene/outfits/items rules, access) | `@dcl/content-validator` strings | `component.ts:362-369` | Port in `services/catalyst/content/src/validator/rules.rs`. Spot-checked strings equal to the local clone: `There are repeated pointers in your request.`; `The entity needs to be pointed by one or more pointers.`; `This hash '<h>' is not valid. It should be IPFS v2 format.`; `The deployment is too big. The maximum allowed size per pointer is ..`; `This hash is referenced in the entity but was not uploaded or previously available: <h>`; `This hash was uploaded but is not referenced in the entity: <h>`; `The metadata for this entity type (<t>) is not valid.`; `Type <t> is not supported yet` (`rules.rs:250,257,279,327,367,762-780`, `metadata_schema.rs:45`) | MATCH for the strings checked; the rest is not compared (upstream source not in the pinned tree) |
| access checker fails (RPC down / timeout) | rejected inside `{errors}` (400) with the validator's `Error checking ...` text | `@dcl/content-validator` | 503 `{"errors":["Error checking ..."]}`, `Retry-After: 30` (`validator/mod.rs:23-27,358-370`, `rules.rs:958`) | DIVERGENT (intentional) |
| content store unreadable | n/a (throws, then the generic error) | | 503 `This node could not read its own content store, so the deployment could not be validated. Please retry. Cause: <cause>` (`validator/rules.rs:56-61`) | DIVERGENT: leaks the cause |
| scene finalized older than the TTL, current access lost | `The deployer no longer has access to the entity pointers (access is required both when a partial upload starts and when it is finalized).` | `component.ts:384-395` | The publication of a complete upload runs the full validator, access included, so lost access is refused with the validator's access message (`partial_deployments.rs:1509-1530`). Upstream's fallback text, used when its validator returns no message, is never produced. | MATCH except the fallback text (P2) |
| commit after the partial-upload deadline | `This upload expired before it could be published. Create a new entity with a fresh timestamp.` | `deployment-service/errors.ts:4`, `component.ts:595-597` | Checked before publication: 400 `This upload expired. Create a new entity with a fresh timestamp.` (`partial_deployments.rs:1519-1521`, `partial_deployments.rs:291-293`) | DIVERGENT: wording (P2) |
| internal error while storing | 400 `{"errors":["There was an error deploying the entity"]}` or `["An internal server error occurred. This will raise an automatic alarm."]` | `component.ts:530-541,598-602` | 503 `{"errors":["deployment write failed: <sqlx error>"]}` / `["failed to store deployment file <h>: <io error>"]` / `["Upload hashing capacity is unavailable."]`, `Retry-After: 30` (`write_deployer.rs:85,139,1155-1158`) | DIVERGENT: status, and internal error text reaches the client |
| content lock busy (GC) | 503 `{"error":"The content lock for entity <id> is still busy, please retry shortly."}`, `Retry-After: 10` | `create-entity-handler.ts:409-418`, `errors.ts:39`, `content-locks/errors.ts:1-7` | n/a (no shared content lock) | n/a |
| spool read failure (transient errno) | 503 `{"error":"The server cannot store uploads right now, please retry shortly."}`, `Retry-After: 30` | `create-entity-handler.ts:321-335` | n/a | n/a |
| unexpected exception | 500 `{"error":"Internal Server Error"}` | `middlewares.ts:72-81` | 500 `{"error":"Internal Server Error"}` (`errors.rs:52-58`) | MATCH |

#### 2e. Success

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| new deployment committed | 200 | `{"creationTimestamp": number}` = the stored `localTimestamp` (ms), the same value `/deployments` and `/audit` show | `Content-Type: application/json` | `create-entity-handler.ts:337-349`, `component.ts:206-210,604` | `{"creationTimestamp": now_ms}` read from the Rust clock after the commit. The stored `local_timestamp` is PostgreSQL `now()` inside the statement (`create_entity.rs:487-504`, `write_deployer.rs:960,1160,1185-1188`). | DIVERGENT: the value differs from the stored `localTimestamp` by the statement duration |

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
- **Limits** (`Environment.ts:667-699`), with the same env names and defaults in dcl-one (`services/catalyst/config/src/content.rs:169-202`, `partial_deployments.rs:104-140`):
  - `PENDING_DEPLOYMENT_TTL` 1 h;
  - `PENDING_DEPLOYMENTS_CLEANUP_INTERVAL` 5 min;
  - `MAX_PENDING_DEPLOYMENTS_PER_DEPLOYER` 10;
  - `MAX_PENDING_UPLOADS_PER_POINTER` 4;
  - `MAX_PENDING_BYTES_PER_DEPLOYER` 1 GiB;
  - `MAX_PENDING_BYTES` 50 GiB;
  - `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` 512 MiB.
  - Upstream refuses to start when `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` cannot fit the entity file plus one `MAX_UPLOAD_FILE_SIZE` file (`partial-deployments/component.ts:227`); dcl-one has no such startup check.
- **Per-file and body caps:** the regular multipart caps apply to every batch in both trees: upstream 100 MiB per file and 2 GiB per body, dcl-one 50 MiB per file and 200 MiB per body (CW-7).

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
| newer entity / rate limit / TTL | 400 / 429 / 400 | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).`; `Entity rate limited (...)` (429, `Retry-After`); `The request is not recent enough, please submit it again with a new timestamp (entityId=<id>).`; `The request is too far in the future, please submit it again with a new timestamp (entityId=<id>).` | | `:775-802` | same texts and statuses, TTL measured from the upload's first batch (`partial_deployments.rs:815-857,872-878`). The `Retry-After` comes from dcl-one's hard-coded per-type windows (CW-15). | MATCH |
| upload started by another account / expired / gone | 400 | `This upload was started by another account.` / `This upload expired. Create a new entity with a fresh timestamp.` / `Upload no longer exists; resend its manifest.` | | `:250,490,547` | same (`partial_deployments.rs:291-293,1236-1244,1366-1373`) | MATCH |
| per-account / per-pointer upload count | 429 | `Too many partial uploads in progress for this account (max 10). Complete an upload or wait for expired uploads to be cleaned up.`; `Too many partial uploads in progress for parcels <p..> (max 4 per parcel). Complete an upload or wait for one to expire.`; `Too many partial uploads in progress for this account on parcels <p..> (max <n> per parcel). Complete an upload or wait for one to expire.` | `Retry-After` | `:122-140,498` | same texts; `Retry-After` counts from the last cleanup run (`partial_deployments.rs:430-458,1255-1262`) | MATCH |
| byte budgets | 400 / 429 | `This batch is <n> bytes, over the partial upload byte rate limit of <m> bytes per minute. Send it in smaller batches.` (400); `Partial upload byte rate exceeded for this account: ...` (429); `This upload needs <n> bytes of partial upload storage, over the <limit> budget of <m> bytes.` (400); `Partial upload storage budget exceeded for this account: ...` (429); `Partial upload storage on this server is full. Retry later.` (429); `Deployment failed: The deployment is too big.` (400) | `Retry-After` on 429 | `:558-606,583,891` | same texts and statuses (`partial_deployments.rs:1390-1500,1058`) | MATCH |
| publication failed server-side | 503 | `{"errors":[...]}` | `Retry-After: 5` | `create-entity-handler.ts:266-278`, `partial-deployments/component.ts:60,398` | 503 `{"errors":[...]}`, `Retry-After: 5`; the errors carry the deployer's internal text, e.g. `deployment write failed: <sqlx error>` (`partial_deployments.rs:1562-1565`) | DIVERGENT: internal text (CW-9) |
| another batch holds the entity's lock | 503 | `{"error":"The content lock for entity <id> is still busy, please retry shortly."}` | `Retry-After: 10` | `:279-284`, `content-locks/errors.ts:1-7`, `controllers/errors.ts:39-42` | after waiting up to 30 s: 503 `{"errors":["Another batch of this upload is in progress. Retry shortly."]}`, `Retry-After: 5` (`partial_deployments.rs:100,661-670`) | DIVERGENT: envelope, text and `Retry-After` (P2) |
| memory budget full | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `:279-284` (entity file only) | same body and header, but on the declared length of every batch before its body is read (`create_entity.rs:326-337`) | DIVERGENT: dcl-one sheds whole batches that upstream would spool to disk (P2) |
| bootstrapping | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | same body once the form is read, no `Retry-After` (`create_entity.rs:531-535`) | DIVERGENT: `Retry-After` missing (CW-5) |

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
| GET/HEAD/PUT/DELETE `/entities` | 405 with `Allow: POST` (`router.allowedMethods()`, `service.ts:63-65`); 404 in READ_ONLY | axum `MethodRouter` 405 with `Allow: POST` (`lib.rs:682-686`); the read-only toggle keeps the route (405) | MATCH outside READ_ONLY (read; upstream body not in tree) |

---

## 6. Backlog

1. **CW-1: `POST /entities` partial uploads: what differs from upstream.**
   - The protocol matches (section 3): 202 `{"missing":[...]}`, 200 on completion and replay, the query-flag 400, the staging checks, the quota and budget messages with `Retry-After`, the env names and defaults, and the quota counting (`services/catalyst/content/src/partial_deployments.rs`, `create_entity.rs:524-579`).
   - Remaining gaps:
     - A batch is buffered in memory and every batch takes the in-memory upload budget on its declared length; upstream spools a declared batch to disk and skips the memory admission (`deployment-memory-admission.ts:22`). Under load dcl-one answers 503 `Server is handling too many uploads` to batches upstream would take.
     - Another batch holding the entity's lock: 503 `{"errors":["Another batch of this upload is in progress. Retry shortly."]}` with `Retry-After: 5`; upstream answers `{"error":"The content lock for entity <id> is still busy, please retry shortly."}` with `Retry-After: 10` (`content-locks/errors.ts:1-7`, `controllers/errors.ts:39-42`).
     - A complete upload past its deadline answers `This upload expired. Create a new entity with a fresh timestamp.`, where upstream's commit answers `This upload expired before it could be published. Create a new entity with a fresh timestamp.` (`deployment-service/errors.ts:4`).
     - No startup check that `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` fits the entity file plus one maximal file (`partial-deployments/component.ts:227`).
     - The per-file cap is 50 MiB, not 100 MiB, so a single file between the two cannot be sent in any batch (CW-7). The bootstrapping 503 lacks `Retry-After` (CW-5); a publication failure carries internal text (CW-9).
   - Place: `create_entity.rs:326-337` (skip the memory admission for a declared batch once batches can stream), `partial_deployments.rs:661-670`, `partial_deployments.rs:1519-1521`, `partial_deployments.rs:145-170` (`Limits::from_config`).
   - Severity: P2. Deploys that split their files succeed against dcl-one.

2. **CW-2: replay idempotency.**
   - Gap: dcl-one runs TTL, rate limit, validation and the newer-entity check before noticing that the entity is already deployed. A retried deploy can get 400 `Entity rate limited (...)` within 3-20 s, 400 `not recent enough` after 20 min, or 400 newer-entity once overwritten. When it succeeds it answers `now_ms`.
   - Expected: look up `entityId` in `content.deployments` right after the auth-chain parse. If found, answer 200 `{"creationTimestamp": <stored local_timestamp ms>}` with no signature, TTL or rate-limit check (`create-entity-handler.ts:98-109`, `component.ts:453-454`).
   - Place: `create_entity.rs:deploy_form` before `deploy_entity`, or the start of `write_deployer.rs:721`; `WriteDeployer::deployed_timestamp` (`write_deployer.rs:458-467`) already answers this lookup for the partial path (`partial_deployments.rs:610-612`). Also `persist` (`write_deployer.rs:1168-1174`) should return the stored `local_timestamp`.
   - Severity: P1.

3. **CW-3: `creationTimestamp` value.**
   - Gap: the value comes from the Rust clock after the commit. The stored `local_timestamp` is PostgreSQL `now()`.
   - Expected: `creationTimestamp` equals the stored `localTimestamp`, as `/deployments` and `/audit` report it (`component.ts:206-210,604`).
   - Place: make the deploy function return `local_timestamp` (`write_deployer.rs:960` `now()`, `PersistOutcome` at `write_deployer.rs:911-917`, `1031-1059`).
   - Severity: P1.

4. **CW-4: read-only mode.**
   - Gap: the read-only toggle answers 403 `text/plain`. Writes-off answers 503 with `Retry-After: 3600`.
   - Expected: upstream READ_ONLY does not register `POST /entities`, so the request gets the router's 404 (`routes.ts:43-45`). Clients and forwarders read 404/405 as "read-only catalyst"; dcl-one's own forwarder already does (`forward.rs:19-21`).
   - Place: `writes.rs:56-75`. Answer 404 with the content not-found shape for read-only, and decide whether writes-off should also look like READ_ONLY.
   - Severity: P1.

5. **CW-5: bootstrapping `Retry-After`.**
   - Gap: the 503 has no `Retry-After`.
   - Expected: `Retry-After: 60` (`middlewares.ts:17,30-34`).
   - Place: `create_entity.rs:317-324,462-466`. Return a response with the header instead of `AppError::ServiceUnavailable`.
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
   - The same caps bound every partial-upload batch, so a file over 50 MiB cannot be deployed in either mode.
   - Place: `create_entity.rs:55-60,178-248`, `lib.rs:685`.
   - Severity: P1.

8. **CW-8: non-multipart body.**
   - Gap: dcl-one answers axum's 400 `text/plain` `` Invalid `boundary` for `multipart/form-data` request ``, before every gate.
   - Expected: 400 `{"error":"Invalid request: expected a multipart/form-data body"}` (`multipart.ts:196-202`), after the burst, bootstrapping and quota gates.
   - Place: `create_entity.rs:304-309` (take `Result<Multipart, MultipartRejection>` and map it) and `services/catalyst/tests/fixtures/content/create-entity-400-not-multipart.json`. The fixture's note that upstream answers 500 does not hold at the pinned revision.
   - Severity: P1.

9. **CW-9: internal error text in responses.**
   - Gap: 503 bodies carry `deployment write failed: <sqlx error>`, `failed to store deployment file <h>: <io error>`, `There was a problem parsing the entity: <serde error>`, `Failed to read multipart field: <multer error>` and `... Cause: <cause>`.
   - Expected: no internal detail.
     - Upstream answers 400 `{"errors":["There was an error deploying the entity"]}` for a store failure (`component.ts:598-602`).
     - Upstream answers `There was a problem parsing the entity` for any parse failure (`component.ts:406-415`).
     - Upstream answers `Invalid multipart/form-data request` for a parser failure (`multipart.ts:469`).
   - Keep the 503/`Retry-After` classification if wanted, with fixed messages.
   - Place: `write_deployer.rs:85,139,760,1158`, `create_entity.rs:192,211,233,243`, `validator/rules.rs:56-61`.
   - Severity: P1.

10. **CW-10: per-source upload admission.**
    - Gap: no limit on concurrent uploads per source.
    - Expected: at most `MAX_CONCURRENT_UPLOADS_PER_SOURCE`=4 bodies and `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` bytes in flight per client, checked before the body. Otherwise 429 `{"error":"Too many uploads in progress from this client, please retry shortly."}` with `Retry-After: 5` (`source-upload-admission.ts:9-47`).
    - Place: `upload_admission.rs` and `create_entity.rs:326-350`.
    - Severity: P2.

11. **CW-11: auth-chain error wording and checks.**
    - Gaps:
      - The no-chain message differs: upstream `No auth chain can be derivated` (`create-entity-handler.ts:90`).
      - dcl-one accepts the legacy `ethAddress`+`signature` pair, which upstream refuses with that 400 (`create_entity.rs:151-157`).
      - Structurally invalid chains get `invalid auth chain format: ...` instead of `{"error":"Invalid auth chain"}` (`AuthChain.validate`, `create-entity-handler.ts:92-94`). `[]` is not refused up front.
    - Place: `create_entity.rs:130-158`.
    - Severity: P2.

12. **CW-12: deployment rejection wording.** Expected strings come from the refs in section 2d. The partial path's staging checks already use upstream's texts (section 3), but a complete upload's publication runs the write deployer, so these texts reach it too. Place: `write_deployer.rs` and `create_entity.rs`. Severity: P2.

    | dcl-one text | upstream text | dcl-one place |
    |---|---|---|
    | `Missing entityId field` | `Missing required field: entityId` | `create_entity.rs:444` |
    | `The entity file was not part of the uploaded files.` | `Failed to find the entity file.` | `write_deployer.rs:756` |
    | `There was a problem parsing the entity: <serde error>` | `There was a problem parsing the entity` | `write_deployer.rs:760` |
    | `The entity needs to be pointed by one or more pointers.` (empty pointers) | `The entity does not have any pointer.`, checked before the validator | validator |
    | newer-entity message without suffix | add ` (entityId=<id> pointers=<p,..>)` before the final period | `write_deployer.rs:907` |
    | `not recent enough` message without suffix | add ` (entityId=<id> pointers=<p,..>)` | `write_deployer.rs:785` |
    | `The request timestamp is too far in the future.` | `The request is too far in the future, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `write_deployer.rs:791` |
    | `Entity rate limited (...)` for a concurrent deploy of the same pointers | `The following pointers are currently being deployed: '<p..>'. Please try again in a few seconds.` | — |

13. **CW-13: check order and dcl-one-only refusals.**
    - Gap, order: upstream checks the signature before the server checks, and the newer-entity check before the rate limit and TTL (`create-entity-handler.ts:144`, `server-validator.ts:76-101`). dcl-one checks TTL and rate limit first and the signature inside the validator.
    - Gap, extra refusals: dcl-one refuses denylisted ids, hashes and pointers (`denylist.rs:87-117`); upstream accepts them. dcl-one refuses an entity file whose `id` differs (`write_deployer.rs:765-771`).
    - Expected: the upstream order, and either the same acceptance or the extra refusals documented as intentional.
    - Place: `write_deployer.rs:721-889`.
    - Severity: P2.

14. **CW-14: daily quota counting.**
    - Gap:
      - dcl-one counts at admission, so requests refused by the multipart parser or form limits are charged; upstream counts after a successful parse (`deployment-quota.ts:111-129`).
      - dcl-one refunds `Unavailable` deploys; upstream does not.
      - dcl-one checks the upload budget before the quota; upstream checks the quota first (`routes.ts:63-64`).
      - dcl-one skips the quota without a gateway-stamped address; upstream falls back to the socket address (`client-source.ts:25`).
    - Place: `create_entity.rs:326-375`, `upload_admission.rs:178-227`.
    - Severity: P2.

15. **CW-15: per-pointer rate-limit configuration.**
    - Gap: TTL and max are hard-coded per type (`write_deployer.rs:292-299`).
    - Expected: upstream code default is 60 s / 300 keys for every type, overridable by `DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL`/`_MAX` and `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`DEPLOYMENT_RATE_LIMIT_MAX_<TYPE>` (`component.ts:109-121`, `Environment.ts:537-542,775-790`). `REQUEST_TTL_BACKWARDS` (20 min) is configurable upstream (`Environment.ts:564-565`).
    - Place: `write_deployer.rs:286-299,712-713`, `services/catalyst/config`.
    - Severity: P2.

16. **CW-16: duplicate form fields and upload timeout body.**
    - Gap: a repeated field name is accepted (last value wins). A slow body gets an empty or nginx 408.
    - Expected: 400 `{"error":"Duplicate form field '<name>'"}` (`multipart.ts:223-232`). 408 `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}` (`multipart.ts:110-117`).
    - Place: `create_entity.rs:178-248`.
    - Severity: P2.

17. **CW-17: undecidable deploys answer 503.**
    - Gap: an EIP-1654 signature that cannot be checked, or an access-checker failure, answers 503 `{"errors"}` with `Retry-After: 30`. Upstream answers 400 `{"errors"}` with the validator text.
    - Expected: keep the 503 if this divergence is intended, but document it as such. Clients that do not retry 503 on POST will report a transient failure where upstream reports a rejection.
    - Place: `validator/mod.rs:358-370`, `create_entity.rs:262-278`.
    - Severity: P2.

18. **CW-18: CORS preflight on `/entities`.**
    - Gap: dcl-one reflects the Origin with credentials, allows a wider method list, and caches the preflight for 86400 s.
    - Expected: `Access-Control-Allow-Origin: *`, no credentials, methods `GET, HEAD, POST, OPTIONS`, max-age 600 (`components.ts:489-506`).
    - Place: `services/catalyst/shared/src/cors.rs:15-79`, shared with every route.
    - Severity: P2.

## 7. Endpoints dcl-one serves in this area that upstream does not

- `DELETE /scenes/{coord}` and `DELETE /content/scenes/{coord}` (`content/src/lib.rs:687`, `unpublish_scene.rs`):
  - Signed-fetch unpublish of a locally published scene by its LAND holder.
  - 200 `{entityId, unpublished: true, parcels}`; 400 / 401 / 403 / 404 / 409 / 503; behind the same `CONTENT_WRITES_ENABLED` and read-only gates.
- `GET /forwarding`, `GET /forwarding/{entityId}`, `POST /forwarding/{entityId}/retry`, and the `/content/` variants (`lib.rs:688-690`, `forward/status.rs:139-200`):
  - Admin-token routes over the upstream-forwarding outbox (`CONTENT_FORWARD_UPSTREAM_URL`).
- `POST /entities` at the root path, besides `/content/entities` (`lib.rs:710-711`). Upstream production serves it under `/content` only.
- The write switch's 503 (`CONTENT_WRITES_ENABLED`) and its restore-stamp variants (`writes.rs:40-54`). There is no upstream counterpart.

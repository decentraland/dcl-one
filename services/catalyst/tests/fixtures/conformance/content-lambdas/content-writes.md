# Content server: write path (`POST /entities`)

Area: content server write path. ID prefix `CW-`.

Pinned revisions:

- Upstream content server: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- dcl-one: `fix/deploy-limits-legacy` @ `472bf2c3` (2026-11-21); the dcl-one line references below are anchored there. Paths below are relative to the repo root.
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
| POST | `/entities` (regular deployment) | `/content` | `services/catalyst/content/src/lib.rs:870-875` (`create_entity::create_entity`), served at the root and under `/content` (`lib.rs:902-903`) | DIVERGENT |
| POST | `/entities` with `partial=true` (form field, and the optional `?partial=true` query) | `/content` | the same route (`lib.rs:870-875`); a form with `partial=true` goes to `partial_deployments::handle` (`create_entity.rs:856,1076-1079`, `partial_deployments.rs:590`) | DIVERGENT |
| OPTIONS | `/entities` (CORS preflight) | `/content` | `services/catalyst/shared/src/cors.rs:20-52` (global middleware) | DIVERGENT |
| GET/HEAD/PUT/DELETE | `/entities` (method not allowed) | `/content` | axum `MethodRouter` 405 on `lib.rs:870-875` (`other_method`) | MATCH |

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
  - dcl-one: `x-real-ip`, else the last `x-forwarded-for` entry, else none. With no source the quota is skipped (`services/catalyst/shared/src/client_ip.rs:32-51`, `upload_admission.rs:194-196,310-317`). IPv6 is grouped by /64 in both (upstream `CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`=64; dcl-one `services/catalyst/shared/src/client_ip.rs:169-180`).
- **dcl-one chain, in order** (`create_entity.rs:826-922,1016-1110`):
  1. `writes::refuse`: read-only, then the `CONTENT_WRITES_ENABLED` switch (`writes.rs:74-79`).
  2. Deployer attached.
  2a. Burst limit: `POST_ENTITIES_RATE_LIMIT_MAX`=200 per `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`=60, a fixed window per client address (IPv6 by /64), every request counted, per process (`create_entity.rs:845-848`, `upload_admission.rs:277-334`).
  3. Bootstrapping (`create_entity.rs:849-851`).
  4. Daily-quota pre-check: a spent source gets the 429, except a batch declared with `?partial=true` (`create_entity.rs:852-859`).
  5. Two budgets, as upstream: the memory budget (`MAX_IN_MEMORY_DEPLOYMENT_BYTES`, 2 GiB) never below one request's most, admits a request when a fixed 16 MiB for its fields and parsing fits, holding nothing while the body arrives, as upstream's `fits` check, and once the form is read reserves that share, its entity file and the largest file its validation reads (an item's thumbnail, a profile's face256), each up to `MAX_UPLOAD_FILE_SIZE`, so a form within the limits always fits an idle node; the spool budget (`MAX_IN_FLIGHT_UPLOAD_BYTES`, read by worlds too, 4 GiB and never below one maximal form with each file and the request's directory charged 16 KiB) admits a request when its declared payload (none without `Content-Length`) fits beside what is reserved, as upstream's `acquire(0, declaredPayload)`, holds only its directory's 16 KiB and grows with the bytes spooled, so a slow or short body holds only what it sent (`create_entity.rs:870-884`, `upload_admission.rs:85-101`). Upstream holds a 16 MiB minimum per request, which about 256 idle bodies (64 sources at 4 each) fill; here the default 4 GiB holds 262,144 idle bodies (65,536 sources at 4 each), each also bounded by its source's slots and the upload timeout.
  6. Source upload admission: 4 bodies and one maximum-size upload's bytes per client address (`create_entity.rs:885-888`, CW-10).
  7. Not a `multipart/form-data` body (400), or a declared length past the form limits (413), uncounted (`create_entity.rs:889-897`).
  8. Form read within `MULTIPART_UPLOAD_TIMEOUT_MS` (408 past it), spooled to disk, with upstream's variables, defaults and texts: `MAX_UPLOAD_FILE_COUNT` 3000 files, `MAX_UPLOAD_FILE_SIZE` 100 MiB per file, `MAX_UPLOAD_FIELD_COUNT`/`MAX_UPLOAD_FIELD_SIZE` 32 fields of 32 KiB, `MAX_UPLOAD_TOTAL_SIZE` 2 GiB of values in all, no repeated name (`create_entity.rs` `read_form`). Each file part is streamed and hashed into its own file under `UPLOAD_SPOOL_FOLDER` (default `<STORAGE_ROOT_FOLDER>/tmp/content-uploads`), the limits checked as the bytes arrive, and its CIDv0 and CIDv1 computed as it is written. The entity file is then read onto the heap; the validator reads another part only to inspect its bytes (a thumbnail), with ordinary I/O, a failed read answering the spool's 503; every part is stored by hard link from its spool file. The request's directory is removed when it ends (success, refusal, error, disconnect); a crashed process's directories go at the next start, once older than the upload and processing deadlines.
  9. The query-flag 400, then the daily quota consume; a partial batch is counted as section 3 says (`create_entity.rs:1080-1091`).
  10. `entityId`.
  11. Auth chain.
  12. Replay: an entity already deployed answers its stored timestamp once the form carries its entity file, which names nothing denylisted, and a chain that signs the id at the entity's timestamp; otherwise the error a fresh deploy gets (`create_entity.rs`, `WriteDeployer::verify_replay`).
  13. Unsynced-copy type gate.
  14. `WriteDeployer::deploy_entity`.
- **Gateway in front of dcl-one:** `limit_req zone=deploy` 5 r/s, burst 200, `limit_req_status 429`, `limit_conn deploy_uploads 10` per address (`limit_conn_status 429`), `client_max_body_size 2096m` (`CONTENT_DEPLOY_CAP`, just above the app's route limit with framing at the default `MAX_UPLOAD_*`; a node that changes them changes it too), `client_body_timeout 300s`, `proxy_request_buffering off` so the body streams to the app's spool instead of nginx's temp space (`infra/gateway/gateway.py`). It is a coarse abuse guard set above the app's burst limit and upstream's 4 concurrent uploads per source, so a client within upstream's limits never meets nginx's HTML 429 or 413.

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
| `READ_ONLY=true` | 404 (route not registered; `/entities` matches no route). The default not-found of `@dcl/http-server` (body not in tree). | default | none | `routes.ts:43-45`, `service.ts:63-65` | `READ_ONLY=true` or the read-only toggle (`POST /admin/api/content/read-only`): 404, this router's own not-found (`text/plain` `Not found`), checked before every other gate (`writes.rs`, `lib.rs` `is_read_only`) | MATCH on status; the body is each router's own not-found |
| (no upstream equivalent) writes switch off | n/a | n/a | n/a | n/a | 503 `{"error":"content writes are disabled on this node (CONTENT_WRITES_ENABLED=false)"}`, `Retry-After: 3600` (`writes.rs:25,41-48`) | DIVERGENT: a node with writes off does not look like a read-only catalyst (404) |
| burst limit exceeded | 429 | `{"error":"Too many requests"}` | `Retry-After` (and the component's `RateLimit-*` headers; set not in tree) | `routes.ts:47-51`, `components.ts:545-556` | 429 `{"error":"Too many requests"}`, `Retry-After` = seconds left in the 60 s window; 200 per window per client address, before the bootstrapping gate; per process (`create_entity.rs:845-848`). The gateway's deploy zone (5 r/s, burst 200) sits above it. | MATCH on limit, body and `Retry-After`. The `RateLimit-*` headers and the component's exact window algorithm are not in the pinned tree and not reproduced. |
| sync state BOOTSTRAPPING | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | Same body and header (`writes.rs:56-65`, `create_entity.rs:849-851`) | MATCH |
| unsynced copy, entity type not ready | n/a (upstream gates on BOOTSTRAPPING only) | | | | 503 bootstrapping body after the form is read; 400 `{"errors":["The entity file was not part of the uploaded files or has no type."]}` when the type cannot be read (`create_entity.rs:1163-1178`) | dcl-one only |
| daily quota exhausted (pre-body marker, or at consume) | 429 | `{"error":"Too many requests"}` | `Retry-After: <s to window end>` | `deployment-quota.ts:28-34,41-59,120-127`; window 86400 s, max 300 (`logic/deployment-quota/component.ts:6,21-25`) | 429 `{"error":"Too many requests"}`, `Retry-After`; checked before the body, counted once the form is read (`create_entity.rs:852-859,1086-1091`, `upload_admission.rs:296-351`) | MATCH. The refund of the deployer's own 503 and the skip without a stamped address are dcl-one's: see CW-14. |
| a request does not fit the memory or spool budget | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `deployment-memory-admission.ts:21-33`, `upload-budget/errors.ts:3`, `multipart.ts:92` | Same body and header, after the quota pre-check: memory when its fixed share does not fit (and once the form is read on that share, its entity file and the largest file its validation reads), spool when its declared payload does not fit beside what is reserved, holding only its directory's 16 KiB until bytes arrive (`create_entity.rs:870-884`, `upload_admission.rs:85-101`, `writes.rs:39`) | MATCH |
| source has 4 uploads or its byte share in flight | 429 | `{"error":"Too many uploads in progress from this client, please retry shortly."}` | `Retry-After: 5` | `source-upload-admission.ts:9,22-46`, `source-upload-limits/errors.ts:3` | same body and header, `MAX_CONCURRENT_UPLOADS_PER_SOURCE` and `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE`: the declared body must fit beside the bytes the source's uploads have received, and the share then grows with the bytes received (`create_entity.rs` `source_busy`, `upload_admission.rs` `SourceLease::grow`) | MATCH |
| aggregate spool budget full (declared or while growing) | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:141-151,274-284` | same body and header, when the declared payload does not fit before the body, and as the spooled bytes grow (`create_entity.rs` `read_form`) | MATCH |

#### 2b. Multipart parsing (upstream `multipart.ts`, mapped by `middlewares.ts:41-82`)

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Content-Type not `multipart/form-data` | 400 | `{"error":"Invalid request: expected a multipart/form-data body"}` | | `multipart.ts:196-202` | Same body, after the read-only, writes, bootstrapping and budget gates and a spent quota's 429, uncounted (`create_entity.rs:889-892`, `tests/fixtures/content/create-entity-400-not-multipart.json`) | MATCH |
| declared Content-Length > total + framing | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes plus multipart framing (<n> bytes in all)."}` | | `multipart.ts:133-136,161-166` | Same text and total (`MAX_UPLOAD_TOTAL_SIZE`), before the body is read; a body that grows past `<n>` gets the same 413 (`create_entity.rs` `max_deployment_body_bytes`, `lib.rs` `build_router`). The gateway's cap (2096 MiB) sits above `<n>` at the defaults, so the app's 413 is the one a client meets. | MATCH |
| received bytes > `MAX_UPLOAD_TOTAL_SIZE` | 413 | `{"error":"The request body is too large. The maximum allowed total upload size is 2147483648 bytes."}` | | `multipart.ts:259-270` | Same text and total (`create_entity.rs` `read_form`) | MATCH |
| more than 3000 files | 413 | `{"error":"Too many files in the request. The maximum allowed is 3000."}` | | `multipart.ts:296-298` | same (`create_entity.rs:426-430`) | MATCH |
| file > 100 MiB | 413 | `{"error":"File '<filename>' is too large. The maximum allowed size per file is 104857600 bytes."}` | | `multipart.ts:378-386` | same, `undefined` for a part without a file name as busboy prints it (`create_entity.rs:434-436,444-448`) | MATCH |
| more than 32 fields | 413 | `{"error":"Too many form fields in the request. The maximum allowed is 32."}` | | `multipart.ts:301-307` | same (`create_entity.rs:472-476`) | MATCH |
| field > 32 KiB | 413 | `{"error":"Field '<name>' is too large. The maximum allowed size per field is 32768 bytes."}` | | `multipart.ts:314-321` | same (`create_entity.rs:480-484`) | MATCH |
| repeated field name | 400 | `{"error":"Duplicate form field '<name>'"}` | | `multipart.ts:223-232` | same, for fields and file parts (`create_entity.rs:404,431-433,487-489`) | MATCH |
| body still arriving after 5 min | 408 | `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}` | | `multipart.ts:110-117,437-450` | Same body after `MULTIPART_UPLOAD_TIMEOUT_MS` (300 s), the bytes counted off the wire (`create_entity.rs:936-970,1060-1067`); the whole-request timeout skips this route (`lib.rs:912-930`). Gateway `client_body_timeout 300s` still gives an nginx 408 for an idle body. | MATCH |
| read form not decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS` (5 min) | n/a (no deadline after the read) | | | | 503 `{"errors":["Deployment processing exceeded the <ms>ms deadline."]}`, `Retry-After: 30`, worlds' text; the deploy is dropped, which releases its upload budget, source share and pointer claim, and the request stays counted against the quota (`create_entity.rs` `processing_timed_out`) | dcl-one only: bounds what the blob collector's age floor assumes (`gc::orphan_age_floor`) |
| malformed multipart (parser error) | 400 | `{"error":"Invalid multipart/form-data request"}` | | `multipart.ts:455-470` | same; the parser's text is logged (`create_entity.rs:214-222`). A field value that is not UTF-8 is decoded with replacement characters, as busboy does. | MATCH |
| spool write/folder failure | 503 | `{"error":"The server cannot store uploads right now, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:94,169-176` | same body and header for a spool folder or part that cannot be created or written (a full or failing disk); the cause is logged (`create_entity.rs` `spool_unavailable`) | MATCH |
| spool flush did not finish in time | 503 | `{"error":"The upload could not be stored in time, please retry shortly."}` | `Retry-After: 30` | `multipart.ts:476-485` | n/a | n/a |

#### 2c. Handler: form fields and auth chain (`create-entity-handler.ts`)

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `?partial=true` without the form field `partial=true` | 400 | `{"error":"The 'partial=true' query parameter requires the 'partial=true' form field"}` | `deployment-quota.ts:116-119` | Same body, once the form is read and before the `entityId` check (`create_entity.rs:1080-1085`) | MATCH |
| no `entityId` | 400 | `{"error":"Missing required field: entityId"}` | `:73-76` | same (`create_entity.rs:209,1123-1126`) | MATCH |
| no `authChain` field and no `authChain[i]` fields | 400 | `{"error":"No auth chain can be derivated"}` | `:89-91` | same; a legacy `ethAddress`+`signature` pair is no chain (`create_entity.rs:242-244,338-341`) | MATCH |
| `authChain` not JSON or not an array | 400 | `{"error":"Invalid auth chain"}` | `:434-446` | same (`create_entity.rs:230-235`) | MATCH |
| chain longer than 10 (JSON or indexed) | 400 | `{"error":"Auth chain is too long; the maximum allowed is 10 elements"}` | `:450-452,472-474` | same (`create_entity.rs:236-238,245-247,292-294`; `library/dcl-auth/src/auth_chain.rs:6`) | MATCH |
| indexed link missing payload/signature/type | 400 | `{"error":"Missing auth chain element at index <i>"}` | `:477-484` | same (`create_entity.rs:282-290`) | MATCH |
| chain fails `AuthChain.validate` (e.g. `[]`, a link without `type`/`payload`, an unknown `type`) | 400 | `{"error":"Invalid auth chain"}` | `:92-94` | same, the schema of `@dcl/schemas`' `AuthChain` ported (`create_entity.rs:296-333`) | MATCH |
| entity already deployed (any `entityId` present in `deployments`) | 200 | `{"creationTimestamp": <stored localTimestamp>}`, answered before the signature, TTL and rate-limit checks | `:98-109`, `deployment-service/component.ts:453-454,461-468` | 200 only once the form carries the entity file, which parses and names nothing denylisted, and a chain that signs the id at the entity's timestamp; otherwise the error a fresh deploy gets. The TTL, rate-limit and access checks are not run again (`create_entity.rs:1138-1162`, `WriteDeployer::verify_replay`). A deploy racing it answers the stored timestamp from the write, after its own checks (`write_deployer.rs:1826-1836`). Deploys of one entity run one at a time in a process (`EntityLocks` in `write_deployer.rs`): a copy that arrives while the original is in flight waits for it and, once it passes `verify_replay` (run after the lock is released), answers the timestamp it stored, so a retry racing its original gets 200, not the per-pointer `Entity rate limited`. A failed lookup goes on to the deploy. | DIFFERENT (deliberate, CW-2) |

#### 2d. Deployment validation (`deployment-service/component.ts`, `server-validator.ts`, `@dcl/content-validator`)

Upstream order:
1. Read the entity.
2. Signature.
3. Pointers / conflict.
4. Server checks: newer, already deployed, rate limit, TTL back, TTL forward.
5. Protocol validator.
6. Current access (scene older than the TTL).
7. Store.

dcl-one order (`write_deployer.rs:1282-1498`):
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
| no uploaded file hashes to `entityId` | `Failed to find the entity file.` | `component.ts:402-405,427-430` | same (`write_deployer.rs:216-222`) | MATCH |
| entity file not JSON / bad type / bad pointers / bad timestamp / bad content | `There was a problem parsing the entity` (`InvalidEntityError` text is logged only) | `component.ts:406-415`, `entities/component.ts:99-156` | same; the serde text is logged (`write_deployer.rs:224-228`) | MATCH |
| entity file `id` field differs from `entityId` | accepted (id is taken from the request) | `entities/component.ts:126-128` | accepted, the id taken from the request (`write_deployer.rs:232-234`) | MATCH |
| signature invalid at `entity.timestamp` | `The signature is invalid. <@dcl/crypto message>` | `create-entity-handler.ts:159-170`, `crypto/component.ts:25-32` | `The signature is invalid. <dcl-auth AuthError Display>`, before the server checks (`write_deployer.rs:1300-1304`, `library/dcl-auth/src/error.rs:4-38`) | MATCH on order; DIVERGENT: tail wording (not pinned upstream) |
| contract-wallet (EIP-1654) link cannot be checked | 400 with the `@dcl/crypto` message | same | 503 `{"errors":["The signature is invalid. EIP-1654 ..."]}`, `Retry-After: 30` (`validator/rules.rs:345-349`, `create_entity.rs:680-712`) | DIVERGENT (intentional) |
| `pointers: []` | `The entity does not have any pointer.` | `component.ts:473-477` | same, before the validator (`write_deployer.rs:1305-1307`) | MATCH |
| same pointers in flight in this process | `The following pointers are currently being deployed: '<p1,p2>'. Please try again in a few seconds.` | `component.ts:33,480-490` | same text, per entity type; checked before the server checks without taking a claim, and claimed only once validation passed, until the write (`write_deployer.rs` `busy_pointers`, `claim_pointers`); a partial publication waits it out as upstream's finalize (CW-1); a concurrent copy of the same entity waits for it and answers its stored timestamp (`EntityLocks`) | MATCH on text; DIVERGENT (intentional) on order: upstream claims before its server checks and validator, so a deploy that fails validation holds the pointers until it ends |
| scene with more parcels than the map, or another type with more than 100 pointers | (no such check) | | `A <type> may name at most <n> pointers.`, before the in-flight and server checks (`write_deployer.rs` `MAX_SCENE_POINTERS`, `MAX_OTHER_POINTERS`) | DIVERGENT (intentional): bounds work before the validator, which refuses every such entity; only the message differs |
| newer entity on any pointer | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).` | `server-validator.ts:24-28,77-78` | same, checked before the rate limit and again by the write (`write_deployer.rs:1335-1346,1820-1825`) | MATCH |
| already deployed (reached only on a race) | `This entity was already deployed. You can't redeploy it` | `server-validator.ts:80-81` | treated as success (`write_deployer.rs:1826-1836`) | DIVERGENT (race only) |
| pointer rate-limited | `Entity rate limited (entityId=<id> pointers=<p,..>).` | `server-validator.ts:83-87` | same text (`write_deployer.rs:1347-1372`) | MATCH on text. Windows: env `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`_MAX_<TYPE>` (`component.ts:109-121`, `Environment.ts:537-542,775-790`), unset the per-type defaults profile 3 s/500, scene 20 s/1000, wearable 20 s/1000, store 3 s/300, emote 20 s/1000, outfits 3 s/2000 (`rate-limiter.ts:199-247`); `DEPLOYMENTS_DEFAULT_RATE_LIMIT_*` (60 s / 300) only for a type outside that table. dcl-one reads the same env names with the same defaults (`write_deployer.rs`, CW-15). Profile unchanged-content 5 min in both. |
| `now - timestamp > 20 min` | `The request is not recent enough, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:89-94`, `Environment.ts:564-565` | same, `REQUEST_TTL_BACKWARDS` configurable (`write_deployer.rs:1374-1381`) | MATCH |
| `timestamp - now > 15 min` | `The request is too far in the future, please submit it again with a new timestamp (entityId=<id> pointers=<p,..>).` | `server-validator.ts:8,96-101` | same (`write_deployer.rs:1382-1388`) | MATCH |
| entity id, content hash or pointer on the operator denylist | accepted (the denylist filters reads only) | `component.ts:305-312` | `The entity <id> is denylisted on this node.` / `These content files are denylisted on this node: ..` / `These pointers are denylisted on this node: ..` (`denylist.rs:134-182`, `write_deployer.rs:1243-1257,1297`) | DIVERGENT: dcl-one-only refusal, kept on purpose (CW-13) |
| two uploaded files map to one declared hash with different bytes | n/a | | `two different uploaded files map to the same content hash <h>` (`write_deployer.rs:1413-1420`) | dcl-one only |
| content-validator rejections (structure, IPFS hashing, metadata schema, ADR-45, size per pointer, content cross-check, profile/scene/outfits/items rules, access) | `@dcl/content-validator` strings | `component.ts:362-369` | Port in `services/catalyst/content/src/validator/rules.rs`. Spot-checked strings equal to the local clone: `There are repeated pointers in your request.`; `The entity needs to be pointed by one or more pointers.`; `This hash '<h>' is not valid. It should be IPFS v2 format.`; `The deployment is too big. The maximum allowed size per pointer is ..`; `This hash is referenced in the entity but was not uploaded or previously available: <h>`; `This hash was uploaded but is not referenced in the entity: <h>`; `The metadata for this entity type (<t>) is not valid.`; `Type <t> is not supported yet` (`rules.rs:309,316,365,406,793,805,1063`, `metadata_schema.rs:48`) | MATCH for the strings checked; the rest is not compared (upstream source not in the pinned tree) |
| access checker fails (RPC down / timeout) | scenes: 400 `Cannot validate deployment`; names, items, collections and third parties: the failure becomes the not-owned or no-access refusal (`access/on-chain/scenes.ts`, `access/on-chain/client.ts`, `collection-asset.ts`, `third-party-asset.ts` in the local clone) | `@dcl/content-validator` | 503 with `Retry-After: 30`: scenes `{"errors":["Cannot validate deployment"]}`, the others `Error checking name ownership.`, `Error checking item ownership.`, `Error checking wearable ownership.`, `Error checking collection access.` or `Error checking third-party access.`; the cause is logged, never answered (`undecided` in `validator/checker.rs`, `validator/mod.rs:22-27,388-403`, `validator/checker.rs:14`) | DIVERGENT: status (intentional); scene text MATCH |
| more than 200 distinct items, third-party NFT contracts or claimed names in one access check | no bound: the on-chain client asks about every one (`@dcl/content-validator` 8.1.0, the version the pinned lockfile resolves, `validations/access/on-chain/client.js`) | | 400 `Error checking item ownership: too many items to check: <n> distinct items` (and the contract and name equivalents) (`validator/onchain/mod.rs` `MAX_ITEMS_ASKED`, `ValidatorError::TooManyItems`) | DIVERGENT (kept): every item is an `eth_call` to the operator's RPC, and any signer can name any number, since `@dcl/schemas` caps neither a profile's avatars nor an outfit's wearables; a profile wears at most 18 wearable categories and 10 emotes per avatar, and an outfits entity ten such outfits, so no entity the clients build reaches 200 |
| content store unreadable | n/a (throws, then the generic error) | | 503 `This node could not read its own content store, so the deployment could not be validated. Please retry.`; the cause is logged (`validator/rules.rs:57-64`) | DIVERGENT: status and text (dcl-one classifies it as undecidable, CW-17) |
| scene finalized older than the TTL, current access lost | `The deployer no longer has access to the entity pointers (access is required both when a partial upload starts and when it is finalized).` | `component.ts:384-395` | The publication of a complete upload runs the full validator, access included, so lost access is refused with the validator's access message (`partial_deployments.rs:1550-1581`). Upstream's fallback text, used when its validator returns no message, is never produced. | MATCH except the fallback text (P2) |
| commit after the partial-upload deadline | `This upload expired before it could be published. Create a new entity with a fresh timestamp.` | `deployment-service/errors.ts:4`, `component.ts:595-597` | Checked before publication: 400 `This upload expired. Create a new entity with a fresh timestamp.`, upstream's `assertLiveUntil` text at the same point (`partial-deployments/component.ts:249-256,348-349`); the commit itself has no deadline (`partial_deployments.rs:304-306,1564-1566`) | DIVERGENT only for a commit that outlives the deadline (CW-1) |
| internal error while storing | 400 `{"errors":["There was an error deploying the entity"]}` or `["An internal server error occurred. This will raise an automatic alarm."]` | `component.ts:530-541,598-602` | 503 `{"errors":["There was an error deploying the entity"]}`, `Retry-After: 30`, for a database, store or hashing-task failure; the cause is logged (`write_deployer.rs` `deploy_error`) | DIVERGENT: status only (503 so the client retries, CW-17) |
| a NUL in the entity's text (`\u0000` in metadata, a pointer or a file name) | the database write throws (no answer of its own) | `component.ts:530-541` | 400 `{"errors":["The entity contains a NUL character (\\u0000), which cannot be stored."]}`: the database refuses it (`22P05`, `22021`) and only the request carries it (`write_deployer.rs` `is_unstorable_text`) | dcl-one only: a request fault answered 400 (CW-17) |
| content lock busy (GC) | 503 `{"error":"The content lock for entity <id> is still busy, please retry shortly."}`, `Retry-After: 10` | `create-entity-handler.ts:409-418`, `errors.ts:39`, `content-locks/errors.ts:1-7` | n/a (no shared content lock) | n/a |
| spool read failure (transient errno) | 503 `{"error":"The server cannot store uploads right now, please retry shortly."}`, `Retry-After: 30` | `create-entity-handler.ts:321-335` | n/a | n/a |
| unexpected exception | 500 `{"error":"Internal Server Error"}` | `middlewares.ts:72-81` | 500 `{"error":"Internal Server Error"}` (`errors.rs:56-62`) | MATCH |

#### 2e. Success

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| new deployment committed | 200 | `{"creationTimestamp": number}` = the stored `localTimestamp` (ms), the same value `/deployments` and `/audit` show | `Content-Type: application/json` | `create-entity-handler.ts:337-349`, `component.ts:206-210,604` | `{"creationTimestamp": <ms>}`: the deploy call's `now()`, which is the stored `local_timestamp` (one statement, one transaction), truncated to ms as the history routes report it (`write_deployer.rs:74-80,1514-1522,1663-1668`) | MATCH |

### Visibility after a successful deploy

- **Upstream:** writes `active_pointers` in the deploy transaction. After the commit it updates the in-process active-entities cache and invalidates the collection-prefix caches (`component.ts:222-290`). A `POST /entities/active` right after the 200 returns the new entity, on this process and on any process reading the database.
- **dcl-one:**
  - Commits deployment, content files, active pointers and overwrites in one statement (`write_deployer.rs:1524-1617`).
  - `POST /entities/active` and the collection listing read `state.pool` directly, with no in-process entity cache (`active_entities.rs:55-87`). The entity is visible on the next request on every instance. Verdict: MATCH.
  - `deployments_cache` (5 s TTL, `lib.rs:128`) is cleared only on the instance that took the deploy (`create_entity.rs:1200,1281`). Other instances can serve `/deployments`, `/pointer-changes` and `/snapshots` up to 5 s stale. That belongs to the read area.
- Upstream sends `Cache-Control: public, max-age=<ENTITIES_CACHE_CONTROL_MAX_AGE>` (default 10) only on `GET /entities/:type`; `POST /entities/active` carries no cache header (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:36-37`). dcl-one sends `public, max-age=10` on both (`services/catalyst/content/src/active_entities.rs:119`; CR-8 in `content-reads.md`). Shared caches do not store POST responses, so in both trees only the GET read can be served up to 10 s stale.

### Data dependency

- **Content DB:**
  - `content.deployments`, `content_files`, `active_pointers`;
  - `content.local_entities` (dcl-one provenance);
  - `content.forward_outbox` (dcl-one forwarding).
- **Blob store:** for content existence (`isContentStoredAlready`) and stored sizes (size rule).
- **Ownership:**
  - Upstream: on-chain (L1/L2 RPC: Checker contract, collection and third-party registry contracts) when `L1_HTTP_PROVIDER_URL` and `L2_HTTP_PROVIDER_URL` are set, else the subgraphs (land manager, blocks L1/L2, collections L1/L2, ENS owner, third-party registry) (`content-validator/component.ts:238-270`).
  - dcl-one: on-chain only, `ETH_RPC_URL` and `RPC_ENDPOINT_POLYGON` (`validator/mod.rs:14-20`), plus the `shop.usage_grants` lease overlay (dcl-one-only grants). The `land_authz` index is used only by the test path without the validator (`write_deployer.rs:916-968`).
  - dcl-one has these sources when the RPC URLs are configured. With `CONTENT_WRITES_ENABLED` on, a missing or plaintext RPC refuses to start (`validator/mod.rs:113-159`).
- **Sync state:** the bootstrapping gate needs the sync status. dcl-one: `SyncStatus` (`lib.rs:434-442`).

---

## 3. `POST /entities` with `partial=true` (multi-request upload, ADR-325)

### Request

- **Same route and middleware.** `?partial=true` skips the pre-body daily-quota check and the memory admission (`deployment-quota.ts:24-26,47`, `deployment-memory-admission.ts:22`).
  - dcl-one skips the quota consume on admission for a declared batch and counts it once the form is read (`create_entity.rs:852-859,1076-1079,1215-1284`). A batch is spooled to disk like a regular deploy and takes the memory budget's fixed share and its entity file only.
  - Before any file part is buffered, dcl-one turns away a batch from a source that spent its quota (unless it resumes the claimed signer's own live upload or replays a deployed entity) and one from an account over its byte rate, with upstream's 429 messages (`create_entity.rs:1043-1058`, `partial_deployments.rs:186-248`). Upstream decides both after authentication; the bodies and statuses are the same.
- **Form:** `partial=true` field. File field names must be content hashes. The first batch must carry the entity file, at most 10 MiB (`partial-deployments/component.ts:63`). Only IPFS v2 scene entities are accepted. dcl-one: the same rules (`partial_deployments.rs:92,751-822,934-952`).
- **Limits** (`Environment.ts:667-699`), with the same env names and defaults in dcl-one (`services/catalyst/config/src/content.rs:282-323`, `partial_deployments.rs:112-178`):
  - `PENDING_DEPLOYMENT_TTL` 1 h;
  - `PENDING_DEPLOYMENTS_CLEANUP_INTERVAL` 5 min;
  - `MAX_PENDING_DEPLOYMENTS_PER_DEPLOYER` 10;
  - `MAX_PENDING_UPLOADS_PER_POINTER` 4;
  - `MAX_PENDING_BYTES_PER_DEPLOYER` 1 GiB;
  - `MAX_PENDING_BYTES` 50 GiB;
  - `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` 512 MiB.
  - Upstream refuses to start when `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` cannot fit the entity file plus one `MAX_UPLOAD_FILE_SIZE` file (`partial-deployments/component.ts:227`); dcl-one has no such startup check.
- **Per-file and body caps:** the regular multipart caps apply to every batch in both trees: `MAX_UPLOAD_FILE_SIZE` (100 MiB) per file and `MAX_UPLOAD_TOTAL_SIZE` (2 GiB) per body (CW-7).

### Responses

Bodies are `{"errors":[...]}` unless the row says otherwise. Every row is read; nothing was run.

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| batch stored, content still missing | 202 | `{"missing": ["<hash>", ...]}` | | `create-entity-handler.ts:233-240` | same (`partial_deployments.rs:607-610`) | MATCH |
| batch completes the upload | 200 | `{"creationTimestamp": number}` | | `create-entity-handler.ts:223-231` | same, the stored `local_timestamp` (`partial_deployments.rs:599-606,1597-1604`) | MATCH |
| replay of a published entity | 200 | `{"creationTimestamp": number}`, before authentication | | `create-entity-handler.ts:100-109` | 200 once the stored entity's id, content hashes and pointers are not denylisted and the chain signs the id at the stored entity's timestamp, else the regular path's denylist 400 or the signature 400 (`partial_deployments.rs:629-663`). A replay not answered 200, or whose chain has an EIP-1654 link, counts against the daily quota; from a source that spent it, a chain with an EIP-1654 link gets 429 `The daily deployment quota for this client is exhausted. A replay signed by a contract wallet is checked only within the quota.` before its contract is asked | DIFFERENT (deliberate, CW-2) |
| source quota spent, not the signer's own live upload | 429 | `The daily deployment quota for this client is exhausted. Only uploads in progress can continue.` | `Retry-After` | `create-entity-handler.ts:111-126` | same text and header, before the files are buffered or after the replay check (`partial_deployments.rs:186-248,669-679`) | MATCH |
| signature invalid | 400 | `The signature is invalid. <msg>` | | `:159-170,174-197` | same prefix (`partial_deployments.rs:681-703`, `write_deployer.rs:851-869`); the tail is `dcl-auth`'s text. An EIP-1654 link this node cannot check answers 503 (CW-17). | DIVERGENT: tail wording, as on the regular path |
| entity id not IPFS v2 | 400 | `The entity id '<id>' is not a valid IPFS v2 hash. Partial deployments require IPFS v2 entities.` | | `partial-deployments/component.ts:694-697` | same (`partial_deployments.rs:751-755`) | MATCH |
| file hash ≠ field name | 400 | `The uploaded file '<key>' does not match its content hash (computed <h>).` | | `:707-711` | same (`partial_deployments.rs:783-787`) | MATCH |
| entity file > 10 MiB | 400 | `The entity file '<id>' is too large (<n> bytes, max 10485760).` | | `:721-725` | same (`partial_deployments.rs:793-798`) | MATCH |
| no entity file on first batch / other signer | 400 | `The first partial request for an entity, and any request by another signer, must include the entity file '<id>'.` | | `:431-433` | same (`partial_deployments.rs:308-312,934-952`) | MATCH |
| entity unparsable / not a scene / no pointer | 400 | `There was a problem parsing the entity: <e>` / `Partial deployments are only supported for scenes, but the entity type is '<t>'.` / `The entity does not have any pointer.` | | `:739-747` | same texts; `<e>` is serde's message, not the JS error's (`partial_deployments.rs:806-822`) | MATCH (the `<e>` tail differs) |
| staging validation fails | 400 | validator errors, or `The staging validation was not successful.` | | `:771` | the ported validator's staging subset, access skipped for the creator's own resume, as upstream (`partial_deployments.rs:836-847`, `write_deployer.rs:871-901`) | MATCH where the ported strings match (section 2d) |
| newer entity / rate limit / TTL | 400 / 429 / 400 | `There is a newer entity pointed by one or more of the pointers you provided (entityId=<id> pointers=<p,..>).`; `Entity rate limited (...)` (429, `Retry-After`); `The request is not recent enough, please submit it again with a new timestamp (entityId=<id>).`; `The request is too far in the future, please submit it again with a new timestamp (entityId=<id>).` | | `:775-802` | same texts and statuses, TTL measured from the upload's first batch (`partial_deployments.rs:862-906`). The `Retry-After` comes from the type's window (CW-15). | MATCH |
| upload started by another account / expired / gone | 400 | `This upload was started by another account.` / `This upload expired. Create a new entity with a fresh timestamp.` / `Upload no longer exists; resend its manifest.` | | `:250,490,547` | same (`partial_deployments.rs:304-306,1277-1290,1414-1418`) | MATCH |
| per-account / per-pointer upload count | 429 | `Too many partial uploads in progress for this account (max 10). Complete an upload or wait for expired uploads to be cleaned up.`; `Too many partial uploads in progress for parcels <p..> (max 4 per parcel). Complete an upload or wait for one to expire.`; `Too many partial uploads in progress for this account on parcels <p..> (max <n> per parcel). Complete an upload or wait for one to expire.` | `Retry-After` | `:122-140,498` | same texts; `Retry-After` counts from the last cleanup run (`partial_deployments.rs:428-487,1293-1310`) | MATCH |
| byte budgets | 400 / 429 | `This batch is <n> bytes, over the partial upload byte rate limit of <m> bytes per minute. Send it in smaller batches.` (400); `Partial upload byte rate exceeded for this account: ...` (429); `This upload needs <n> bytes of partial upload storage, over the <limit> budget of <m> bytes.` (400); `Partial upload storage budget exceeded for this account: ...` (429); `Partial upload storage on this server is full. Retry later.` (429); `Deployment failed: The deployment is too big.` (400) | `Retry-After` on 429 | `:558-606,583,891` | same texts and statuses (`partial_deployments.rs:1438-1545,1095-1098`) | MATCH |
| publication failed server-side | 503 | `{"errors":[...]}` | `Retry-After: 5` | `create-entity-handler.ts:266-278`, `partial-deployments/component.ts:60,398` | 503 `{"errors":["There was an error deploying the entity"]}`, `Retry-After: 5`, the cause logged; a staged file the store cannot write answers the same text (`partial_deployments.rs:1233-1258`) | MATCH |
| another batch holds the entity's lock | 503 | `{"error":"The content lock for entity <id> is still busy, please retry shortly."}` | `Retry-After: 10` | `:279-284`, `content-locks/errors.ts:1-7`, `controllers/errors.ts:39-42` | same body and header, after waiting up to 30 s for the lock (`partial_deployments.rs:102,108,333-345,714-720`) | MATCH |
| memory budget full | 503 | `{"error":"Server is handling too many uploads, please retry shortly."}` | `Retry-After: 30` | `:279-284` (entity file only) | same body and header, on the fixed per-request share, the entity file and the largest file its validation reads, which upstream reads uncharged (`create_entity.rs` `uploads_of`) | MATCH |
| bootstrapping | 503 | `{"error":"Deployments are not allowed while the Catalyst is bootstrapping"}` | `Retry-After: 60` | `middlewares.ts:17,26-35` | same body and header, once the form is read (`create_entity.rs:849-851,1230-1232`) | MATCH |

**Quota counting.** Both trees count a partial upload against the daily quota once, when its first batch creates it, and count a batch refused with a 400 before it joins an upload; 429s, 503s and replays are not counted (`deployment-quota.ts:111-129`; `create_entity.rs:1215-1284`). Read, matching, except that dcl-one also counts a replay of a deployed entity that is not answered 200 or that asked a contract wallet (CW-2).

**Data dependency.**
- Upstream: a pending-deployments table (`pendingDeploymentsRepository`), staged blobs in the served content store, per-account accounting, and a cleanup job.
- dcl-one: `content.pending_deployments`, `content.pending_deployment_files` and `content.partial_upload_rates` (`schemas/00_content.sql`); staged blobs in the one content store, so `/contents/{hash}` serves them before publication as upstream does; the leased `content-partial-uploads-cleanup` job, attached only where content writes are on (`partial_deployments.rs:1817-1828`, `services/catalyst/src/serve.rs:603-612`). The budget and entity locks are PostgreSQL advisory locks, so they hold across replicas.

---

## 4. `OPTIONS /entities` (CORS preflight)

| trigger | status | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| preflight | 204 (koa-cors default) | `Access-Control-Allow-Origin: *`; methods `GET, HEAD, POST, OPTIONS`; request headers reflected; `Access-Control-Max-Age: 600`; no credentials; responses `Access-Control-Expose-Headers: *` | `components.ts:489-506` | 204; `Access-Control-Allow-Origin: <Origin>` with `Access-Control-Allow-Credentials: true`; methods `GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS`; request headers reflected; `Access-Control-Max-Age: 86400`; `Vary: Origin`; expose `*` (`shared/src/cors.rs:15-79`) | DIVERGENT. This is shared CORS behaviour; `Retry-After` stays readable to browser deployers in both (expose `*`, no credentialed request). |

## 5. Other methods on `/entities`

| trigger | status | upstream | dcl-one | verdict |
|---|---|---|---|---|
| GET/HEAD/PUT/DELETE `/entities` | 405 with `Allow: POST` (`router.allowedMethods()`, `service.ts:63-65`); 404 in READ_ONLY | 405 with `Allow: POST`; 404 (`Not found`) while the read-only toggle is on (`create_entity.rs:760-771`, `lib.rs:708-710`) | MATCH (read; upstream body not in tree) |

---

## 6. Backlog

1. **CW-1: `POST /entities` partial uploads: what differs from upstream.**
   - The protocol matches (section 3): 202 `{"missing":[...]}`, 200 on completion and replay, the query-flag 400, the staging checks, the quota and budget messages with `Retry-After`, the env names and defaults, and the quota counting except for replays (CW-2) (`services/catalyst/content/src/partial_deployments.rs`, `create_entity.rs:1215-1284`).
   - Fixed: another batch holding the entity's lock answers upstream's 503 `{"error":"The content lock for entity <id> is still busy, please retry shortly."}` with `Retry-After: 10` (`partial_deployments.rs:108,333-345,714-720`). `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` below the entity file plus one maximal file (110 MiB) refuses to start (`services/catalyst/config/src/content.rs:314-323,406-408,462-472`). A publication meeting another deploy of this process on its pointers waits 300 ms up to 3 times, then answers 429 with `Retry-After: 5`, as upstream's finalize (`partial_deployments.rs:103-106`, `partial-deployments/component.ts:55-58,374-409`).
   - Not a gap: a complete upload past its deadline answers `This upload expired. Create a new entity with a fresh timestamp.`, which is upstream's `assertLiveUntil` at the start of its finalize (`partial-deployments/component.ts:249-256,348-349`). Upstream's `This upload expired before it could be published. ...` (`deployment-service/errors.ts:4`) comes only from a commit that outlives the deadline (`mustCommitBy`); dcl-one does not bound the commit itself.
   - Fixed: a batch is spooled to disk and holds the memory budget's fixed share and its entity file only, so a batch upstream would spool is no longer shed for its size.
   - Severity: P2. Deploys that split their files succeed against dcl-one.

2. **CW-2: replay idempotency.** Status: DIFFERENT, deliberate. Upstream answers a replay 200 right after the auth-chain shape check, without the entity file or a signature check (`create-entity-handler.ts:100-109` at `6c56f8a3`; `deployment-service/component.ts:455-466` likewise in `deployEntity`). dcl-one answers 200 only once the request proves what a fresh deploy proves first: a regular deploy carries the entity file, which parses and names nothing denylisted, and a chain that signs the id at the entity's timestamp (`WriteDeployer::verify_replay`, which also answers a copy of a deploy in flight in this process); a partial batch, which may carry no entity file, a stored entity whose id, content hashes and pointers are not denylisted and a chain that signs the id at the stored entity's timestamp. Anything else gets the error a fresh deploy gets (400 `Failed to find the entity file.`, the denylist refusal, `The signature is invalid. ...`). The TTL, rate-limit and access checks are not run again, so an entity older than `REQUEST_TTL_BACKWARDS` still replays. A contract-wallet (EIP-1654) chain is asked of its contract again, so a replay of one answers 503 while that check cannot be made. A replay that is not answered 200 counts against the daily quota, on the partial path too, where upstream counts no replay; so does a partial replay that asked a contract wallet, as every regular replay is counted; a source that spent its quota gets 429 for a partial replay with an EIP-1654 link, before its contract is asked. A deploy racing it answers the stored timestamp from the write, after its own checks.

3. **CW-3: `creationTimestamp` value.** Status: MATCH, no work. `creationTimestamp` is the deploy call's `now()`, which is the stored `local_timestamp`, truncated to ms as `/deployments` and `/audit` report it (`write_deployer.rs:74-80,1514-1522,1663-1668`).

4. **CW-4: read-only mode.** Status: MATCH. `READ_ONLY=true` (upstream's variable, read as `true`/`false`; `services/catalyst/config/src/node.rs`) and the console's read-only toggle answer 404 with this router's not-found, before every other gate, for `POST /entities`, every other method on `/entities` and `DELETE /scenes/{coord}` (`writes.rs`, `create_entity.rs` `other_method`, `lib.rs` `is_read_only`), as upstream READ_ONLY, which does not register the route (`routes.ts:43-45`). `READ_ONLY=true` turns the write path off as `CONTENT_WRITES_ENABLED=false` does (`WritesConfig::content_writes`: no partial-upload reclaim, no deploy-RPC startup requirement, the write status reports content off); with both set and disagreeing, the refusal wins, and the console toggle cannot lift `READ_ONLY`. Writes-off (`CONTENT_WRITES_ENABLED=false` alone) keeps its 503 with `Retry-After: 3600`: a dcl-one-only state with no upstream counterpart, which names why the node refuses (section 7); dcl-one's forwarder retries both answers (`forward.rs:20-22`).

5. **CW-5: bootstrapping `Retry-After`.** Status: MATCH, no work. Every bootstrapping 503 carries `Retry-After: 60` (`writes.rs:56-65`).

6. **CW-6: per-client burst limit.** Status: MATCH, no work. 200 requests per 60 s per client address (`POST_ENTITIES_RATE_LIMIT_MAX`, `POST_ENTITIES_RATE_LIMIT_WINDOW_SECONDS`), every request counted, answered 429 `{"error":"Too many requests"}` with `Retry-After` (`create_entity.rs:845-848`, `upload_admission.rs:277-334`, `services/catalyst/config/src/content.rs:231-244`). Per process, as upstream's limiter: N upstreams admit N times it. The gateway's deploy zone is now 5 r/s with burst 200, above it (`infra/gateway/gateway.py:146-153,263`). Not reproduced: the `RateLimit-*` headers of `@dcl/rate-limiter-component` (not in the pinned tree).

7. **CW-7: multipart limit statuses, values and messages.** Status: MATCH, no work. Every form limit answers 413 `{"error":...}` with upstream's text, variables and defaults (`MAX_UPLOAD_FILE_COUNT` 3000, `MAX_UPLOAD_FILE_SIZE` 100 MiB, `MAX_UPLOAD_FIELD_COUNT` 32, `MAX_UPLOAD_FIELD_SIZE` 32 KiB, `MAX_UPLOAD_TOTAL_SIZE` 2 GiB), and the route's body limit is the total plus the framing of every part, as upstream's `maxMultipartBodySize` (`create_entity.rs` `read_form`, `max_deployment_body_bytes`). A `MAX_PARTIAL_UPLOAD_BYTES_PER_MINUTE` below the entity file and one `MAX_UPLOAD_FILE_SIZE` file refuses to start; a set `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` below `MAX_UPLOAD_TOTAL_SIZE` is logged at startup, since uploads past it are always refused (`services/catalyst/config/src/content.rs` `check_upload_limits`, `upload_limit_warnings`). The files are spooled to disk as upstream spools them (step 8), so the 2 GiB total bounds a deploy's disk, not its memory.

8. **CW-8: non-multipart body.** Status: MATCH, no work. A body that is not a form answers 400 `{"error":"Invalid request: expected a multipart/form-data body"}` after the gates, uncounted (`create_entity.rs:889-892`); the fixture `create-entity-400-not-multipart.json` replays it on a node that takes deploys.

9. **CW-9: internal error text in responses.** Status: MATCH, no work. Store, database and hashing failures answer `There was an error deploying the entity` (503 with `Retry-After`, CW-17's classification), a parse failure `There was a problem parsing the entity`, a parser failure `Invalid multipart/form-data request`, the unreadable store its sentence without the cause, a partial upload's unreadable store `There was an error deploying the entity`, an access source that cannot answer a fixed sentence (row "access checker fails"); each cause is logged (`write_deployer.rs:82-84,99-103,224-228`, `create_entity.rs:214-222`, `validator/rules.rs:57-64`, `store_unreadable` and `store_files` in `partial_deployments.rs`, `undecided` in `validator/checker.rs`).

10. **CW-10: per-source upload admission.** Status: MATCH, no work. Before the body, after the memory and spool budgets, as each client's share of the spool, a client address (IPv6 by /64) with `MAX_CONCURRENT_UPLOADS_PER_SOURCE` (4) bodies or `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` bytes (unset, one maximum-size upload, `MAX_UPLOAD_TOTAL_SIZE`; a set value below one 350 MiB world deploy is refused by the config, since worlds reads it too, and one below `MAX_UPLOAD_TOTAL_SIZE` is logged at startup) in flight answers 429 `{"error":"Too many uploads in progress from this client, please retry shortly."}` with `Retry-After: 5`; a body is admitted when its declared length, else the cap, fits beside the bytes the source's uploads have received, and then holds the bytes it receives (429 once they outgrow the share) (`create_entity.rs:458-462,885-888`, `upload_admission.rs:152-244`, `services/catalyst/config/src/content.rs:325-354`). Per process, as upstream's. The client is chosen as for the quota (CW-14): the socket address, or a trusted proxy's `X-Real-IP`. One process serves both upstream services and they share these variable names, so the worlds multipart uploads read the same two settings. Unset, the worlds byte share is one 350 MiB world deploy. Trade-off of the default: a source whose uploads have received most of its share gets 429 for another upload that would not fit (a body without `Content-Length` needs the whole `MAX_UPLOAD_TOTAL_SIZE` free); an operator raises the variable to let one source send several large bodies at once. The gateway also caps each address at 10 deploy uploads in flight (`limit_conn deploy_uploads`), above upstream's 4, as a coarse guard.

11. **CW-11: auth-chain error wording and checks.** Status: MATCH, no work. A form without a chain answers `{"error":"No auth chain can be derivated"}`; the legacy `ethAddress`+`signature` pair is no chain. A chain is checked as `@dcl/schemas`' `AuthChain.validate` (at least one link; a known `type`, a string `payload`, a string or null `signature`, required except on a SIGNER, whose signature is absent, null or empty; schema read from the 26.5.0 package), else `{"error":"Invalid auth chain"}`. A form without `entityId` answers `{"error":"Missing required field: entityId"}`; the query-flag 400 comes before it, as upstream's `withDeploymentQuota` (`create_entity.rs:209,224-346,1080-1085,1123-1126`).

12. **CW-12: deployment rejection wording.** Status: MATCH, no work. The write deployer answers upstream's texts (section 2d): `Failed to find the entity file.`; `There was a problem parsing the entity`; `The entity does not have any pointer.` before the validator; `The following pointers are currently being deployed: '<p,..>'. Please try again in a few seconds.` for pointers another deploy of this process holds; the ` (entityId=<id> pointers=<p,..>)` suffix on the newer-entity and both TTL messages (`write_deployer.rs:216-236,1149-1163,1295-1388`). A complete upload's publication runs the same deployer, so its texts are these too.

13. **CW-13: check order and dcl-one-only refusals.** Status: MATCH, no work. The deployer checks in upstream's order: entity file, parse, signature at the entity's timestamp, pointers, the in-flight check, newer entity, rate limit, TTL back, TTL forward, then the validator, and takes the in-flight claim only after the validator passes, which checks the signature again as upstream's validator does (`write_deployer.rs:1295-1466`). The entity file's `id` field is ignored, the id is the request's (`write_deployer.rs:232-234`). Kept on purpose: the operator denylist refuses a listed entity id, content hash or pointer (`denylist.rs:134-182`), after the parse; upstream filters only reads. It is a dcl-one moderation tool (section 7), not a client contract.

14. **CW-14: daily quota counting.** Status: Kept by decision.
    - Fixed: the spent-quota 429 is answered before the upload budget, and the quota counts a request once its form is read, so forms the parser or its limits refuse are not charged (`create_entity.rs:852-859,1086-1091`).
    - Decision: the difference below is kept, and the client address follows upstream's socket fallback.
      - dcl-one gives back a deploy its deployer answered 503 for this node's own state (a source down, the store unreadable); upstream keeps it counted. Kept because of CW-17: the client was told to retry, so the retry should not spend its quota. The option not taken: count it as upstream.
      - Only a deploy whose access check fits one Checker batch (at most 100 on-chain sub-calls: parcels, claimed names and worn items) and asks only Decentraland's own contracts is given back (`create_entity.rs` `is_refundable`). A larger one, a wearable or emote (its collection), or a profile or outfit wearing an on-chain item can drive the check past its deadline or into a non-revert failure, so its 503 stays counted.
      - The client is the socket address, as upstream's fallback (`client-source.ts:25`), unless the socket peer is in `HTTP_TRUSTED_PROXY_CIDRS`; then its single `X-Real-IP` names the client (`deploy_client` in `create_entity.rs`). Upstream reads the forwarding headers of any peer; dcl-one ignores them from other peers, so a client reaching the API directly can't rotate its budget or spend another address's. Only an in-process request without a socket is still keyed by its header, or skipped without one. Behind the gateway without that setting every deployer shares the gateway's bucket; the node logs one warning when an untrusted private or loopback peer forwards `X-Real-IP` (`looks_like_an_untrusted_proxy` in `create_entity.rs`).
    - Place: `create_entity.rs:852-859,1086-1109`, `upload_admission.rs:259-275`.
    - Severity: P2 (by decision).

15. **CW-15: per-pointer rate-limit configuration.** Status: MATCH.
    - Fixed: `REQUEST_TTL_BACKWARDS` (default 20 min), `DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL`/`_MAX` and `DEPLOYMENT_RATE_LIMIT_TTL_<TYPE>`/`DEPLOYMENT_RATE_LIMIT_MAX_<TYPE>` are read under upstream's names, durations as the `ms` package writes them (`services/catalyst/config/src/content.rs`, `DeployLimits::from_config` in `write_deployer.rs`). Each of the six entity types takes its own `DEPLOYMENT_RATE_LIMIT_*_<TYPE>` or its built-in window, as upstream's `getCacheConfigPerEntityMap` (`rate-limiter.ts:199-247`); the `DEPLOYMENTS_DEFAULT_*` values apply only to a type outside that table (`rate-limiter.ts:163-164`), so setting them changes none of the six.
    - The built-in windows are upstream's per-type defaults: profile 3 s/500, scene 20 s/1000, wearable 20 s/1000, store 3 s/300, emote 20 s/1000, outfits 3 s/2000 (`rate-limiter.ts:199-247`). The production values are deployment configuration, not in the pinned tree.
    - Place: `REFERENCE_RATE_LIMITS` and `DeployLimits::from_config` in `write_deployer.rs`.

16. **CW-16: upload timeout body.** Status: MATCH, no work. A repeated form name answers upstream's 400 `{"error":"Duplicate form field '<name>'"}`. A form still arriving `MULTIPART_UPLOAD_TIMEOUT_MS` (300 s, the setting worlds reads too) after admission answers 408 `{"error":"The upload did not finish within 300 s: received <r> of <d> bytes (<k> KiB/s). Retry on a faster connection or send smaller batches."}`, the bytes counted off the wire (`create_entity.rs:936-970,1060-1067`). The whole-request `REQUEST_TIMEOUT_SECS` does not apply to `POST /entities` (`build_router` in `lib.rs`); once read, the form must be decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS` (300 s), else 503 with `Retry-After: 30` (section 2b). The gateway's `client_body_timeout 300s` still answers an idle body with nginx's 408.

17. **CW-17: undecidable deploys answer 503.** Status: Kept by decision.
    - Gap: an EIP-1654 signature that cannot be checked, or an access-checker failure, answers 503 `{"errors"}` with `Retry-After: 30`. Upstream answers 400 `{"errors"}` with the validator text.
    - Decision: the 503 is kept. dcl-one tells the client the node could not decide (a chain RPC down or unset) instead of rejecting a deploy that may be valid; a 400 would make clients drop valid deploys during an RPC outage. The option not taken: answer 400 as upstream. Clients that do not retry 503 on POST report a transient failure where upstream reports a rejection.
    - Mapping (`deployment_failure_response` in `create_entity.rs`): 503 with `Retry-After` only for this node's side: the database or content store failing, a store collection racing the deploy, an access-check RPC unreachable, failing or past the 30 s access deadline, a source unset (no `ETH_RPC_URL` for a contract-wallet link), a read form not decided within `DEPLOYMENT_PROCESSING_TIMEOUT_MS`, the upload budget full, the node bootstrapping or its writes off. Every failure that follows from the request is a 400 `{errors}`, as upstream answers it: the entity, its files, signature (a contract wallet's contract answering no or reverting included), rights, pointers, TTLs, rate limits, text the database cannot store (a NUL) and more items than one access check asks about.
    - Place: `validator/mod.rs:388-403`, `create_entity.rs:680-712`.
    - Severity: P2 (by decision).

18. **CW-18: CORS preflight on `/entities`.**
    - Gap: dcl-one reflects the Origin with credentials, allows a wider method list, and caches the preflight for 86400 s.
    - Expected: `Access-Control-Allow-Origin: *`, no credentials, methods `GET, HEAD, POST, OPTIONS`, max-age 600 (`components.ts:489-506`; captured on `/content/entities/active` in `captures/content-reads/CR-11-preflight.json`).
    - Same change as CR-11 (`content-reads.md`), in the shared `services/catalyst/shared/src/cors.rs:15-79`; it is fixed there, not here.
    - Severity: P2.

## 7. Endpoints dcl-one serves in this area that upstream does not

- `DELETE /scenes/{coord}` and `DELETE /content/scenes/{coord}` (`content/src/lib.rs:876`, `unpublish_scene.rs`):
  - Signed-fetch unpublish of a locally published scene by its LAND holder.
  - 200 `{entityId, unpublished: true, parcels}`; 400 / 401 / 403 / 404 / 409 / 503; behind the same `CONTENT_WRITES_ENABLED` and read-only gates.
- `GET /forwarding`, `GET /forwarding/{entityId}`, `POST /forwarding/{entityId}/retry`, and the `/content/` variants (`lib.rs:877-879`, `forward/status.rs:139-200`):
  - Admin-token routes over the upstream-forwarding outbox (`CONTENT_FORWARD_UPSTREAM_URL`).
- `POST /entities` at the root path, besides `/content/entities` (`lib.rs:902-903`). Upstream production serves it under `/content` only.
- The write switch's 503 (`CONTENT_WRITES_ENABLED`, `writes.rs:41-48`). There is no upstream counterpart.

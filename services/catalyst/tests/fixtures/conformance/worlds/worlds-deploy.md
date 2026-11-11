# Worlds content server: deployment writes

Area: the worlds routes that publish and unpublish scenes: `POST /entities` (a single-request
deploy and the `partial=true` batched upload), `DELETE /entities/:world_name` (undeploy the whole
world) and `DELETE /world/:world_name/scenes/:coordinate` (undeploy one scene). It covers the
middlewares in front of `POST /entities` (`stampRequestArrival`, `createSourceUploadAdmission`,
the multipart parser with `DEPLOYMENT_FIELD_LIMITS`, the in-flight upload budget, the upload
timeout, `uploadErrorHandler`) and the deploy validation (auth chain, signer, world name ownership,
deployment grants over parcels, size limits, multi-scene placement).

Pinned revisions:

- Upstream: `decentraland/worlds-content-server` @ `10af5255bb04a1a28e0c26bd190317c26a6015a9` (2026-10-21).
  Packages read from their published tarballs at the versions `yarn.lock` pins:
  `@dcl/http-commons` 2.0.0 (`errorHandler`), `@dcl/crypto-middleware` 6.3.0 (`wellKnownComponents`,
  `rejectIfSigner`, `verify`), `@dcl/http-server` 2.3.0 (string and JSON bodies).
  Also read from their tarballs: `@dcl/crypto` 3.7.0 (`Authenticator.validateSignature`), `busboy`
  1.6.0 (limits, content-type and end-of-form texts) and `@dcl/schemas` 27.1.0 (`AuthChain`,
  `IPFSv2`, the AJV options).
- dcl-one: the revision that contains this file, Rust workspace `services/catalyst`.

The rows were reached by reading the code and tests of both trees; nothing was captured from a
live upstream. Each backlog item names the dcl-one tests that pin it; the gateway's JSON 413/429
pages are covered by `infra/gateway/tests/test_config.py` (unit tests on the rendered config).

Files read:

- Upstream:
  - `src/controllers/routes.ts`
  - `src/controllers/request-arrival.ts`, `src/controllers/source-upload-admission.ts`,
    `src/controllers/upload-error-handler.ts`
  - `src/controllers/multipart/{middleware,limits,errors,types}.ts`
  - `src/controllers/handlers/{deploy-entity-handler,undeploy-entity-handler,scenes-handler}.ts`
  - `src/logic/{extract-auth-chain,form-data,utils,deployment-processing,name-ownership-validation}.ts`
  - `src/logic/validations/{common,scene,parcels,validator,utils}.ts`
  - `src/logic/scene-deployments/component.ts`, `src/logic/partial-upload-admission/{component,errors}.ts`
  - `src/logic/upload-limits/{constants,load}.ts`, `src/logic/client-source/component.ts`,
    `src/logic/coordinates/component.ts`, `src/logic/worlds/component.ts` (`undeployWorld`, `undeployWorldScenes`),
    `src/logic/permissions/component.ts` (`hasPermissionForParcels`, `hasWorldWidePermission`)
  - `src/adapters/{entity-deployer,dcl-name-checker,name-deny-list-checker,name-ownership}.ts`,
    `src/adapters/in-flight-upload-budget/component.ts`, `src/adapters/source-upload-limits/*`,
    `src/adapters/content-locks/component.ts`, `src/adapters/limits-manager/{component,errors}.ts`,
    `src/adapters/pending-scenes-repository/errors.ts`,
    `src/adapters/worlds-manager.ts` (`deployScene`, `undeployWorld`, `undeployScene`)
  - `src/types.ts` (`SceneReplacementConflictError`, `DeploymentUniqueViolationError`), `src/components.ts`
  - Tests: `test/integration/{deploy,partial-deploy,partial-upload-accounting,multipart-error-handling,multipart-limiter,source-upload-limits,deploy-parcel-permission,strict-scene-signer-gate,undeploy-entity-handler,scenes-handler,content-locks}.spec.ts`,
    `test/contracts/partial-deployment.ts`, `test/unit/validations/common.spec.ts`
- dcl-one (`services/catalyst/`):
  - `worlds/src/core/mod.rs` (routes), `worlds/src/lib.rs` (`app_router`), `worlds/src/writes.rs`
  - `worlds/src/core/{deploy,deploy_form,deploy_validate,partial,upload_limits,upload_spool,scenes,store,quota,auth,http}.rs`
  - `worlds/src/core/tests.rs`, `worlds/src/core/tests/partial_uploads.rs` (test names only)
  - `comms/src/worlds_envelope.rs`, `comms/src/signed_fetch.rs`, `shared/src/signed_fetch.rs`,
    `comms/src/world_identity.rs` (`WorldName`, `Parcel`)
  - `src/serve.rs` (mounts), `docs/worlds-compatibility.md`
  - `../../infra/gateway/gateway.py` (deploy locations)

Common notes:

- **Mount points.** Upstream serves the routes at its host root. dcl-one mounts `POST /entities` and
  `DELETE /entities/{world_name}` only under `/worlds-content-server`: at the root those paths belong to
  the content server (`worlds/src/core/mod.rs` `root_router`, `content_colliding_routes`). The
  gateway rewrites the `worlds-content-server.<domain>` host into that prefix, so a client pointed at
  the worlds host reaches them at upstream's paths. `DELETE /world/{world_name}/scenes/{scene_coord}`
  answers at the root and under the prefix (`mod.rs` `timed_routes`).
- **Writes switch.** Every route in this area is a write behind `WORLDS_WRITES_ENABLED`
  (`worlds/src/writes.rs` `is_gated`), default `false` (`config/src/worlds.rs` `worlds_writes_enabled`). Off, each answers 503
  `{"ok":false,"error":"Service Unavailable","message":…}` with `Retry-After: 3600`
  (`writes.rs` `refused_response`). The rows below assume the switch is on (WD-03).
- **Error envelope.** Upstream answers a thrown `InvalidRequestError` with
  `400 {"error":"Bad request","message":…}`, `NotAuthorizedError` with `401 {"error":"Not Authorized",…}`
  and anything else with `500 {"error":"Internal Server Error"}` (`@dcl/http-commons` 2.0.0
  `dist/controllers/handlers/error-handler.js`). The upload errors are mapped by
  `uploadErrorHandler` and the deploy handler (`src/controllers/upload-error-handler.ts:19-70`).
  dcl-one answers deploy refusals with `DeployRejection {error, message}`, `error` being
  `Bad request` for 400 and the status's reason phrase otherwise
  (`worlds/src/core/deploy.rs` `rejection`), and the DELETE routes with `ApiError` through
  `comms/src/worlds_envelope.rs` (`error_label`).
- **Signed fetch (DELETE routes).** Upstream's strict `signedFetchMiddleware` (no legacy payload
  fallback, `rejectIfSigner('decentraland-kernel-scene')`, `src/controllers/routes.ts:72-98`)
  answers a refusal with the middleware's status (400 shape, 401 signature/expiry, 503 catalyst)
  and the body `{"error":<verify message>,"message":"This endpoint requires a signed fetch request. See ADR-44."}`.
  dcl-one's `require_verified` with an empty key list (`comms/src/signed_fetch.rs`) has the
  same statuses, the same scene gate and the same body (WD-02).
- **Gateway.** `POST /worlds-content-server/entities` passes nginx's `limit_req zone=deploy`
  (5 r/s, burst 4), `limit_conn` 10 per address and `client_max_body_size 360m`, and nginx buffers the
  body before the app sees it (`infra/gateway/gateway.py`). Upstream applies no request rate limit
  here. The gateway's own 413 and 429 answer the worlds JSON (WD-20).

## Endpoint inventory

| # | method | upstream path | dcl-one route | status |
|---|---|---|---|---|
| 1 | POST | `/entities` (single request) | `/worlds-content-server/entities` → `services/catalyst/worlds/src/core/deploy.rs` (`deploy_entity`), route `core/mod.rs` `content_colliding_routes` | DIVERGENT (intended only) |
| 2 | POST | `/entities` with form field `partial=true` (and optional `?partial=true`) | same handler → `core/partial.rs` (`stage`) | DIVERGENT (intended only) |
| 3 | DELETE | `/entities/:world_name` | `/worlds-content-server/entities/{world_name}` → `core/scenes.rs` (`undeploy_world`), route `mod.rs` `content_colliding_routes` | DIVERGENT (intended only) |
| 4 | DELETE | `/world/:world_name/scenes/:coordinate` | root and prefix → `core/scenes.rs` (`delete_scene`), route `mod.rs` `timed_routes` | DIVERGENT (intended only) |

Counts: 4 endpoints. MATCH 0, GAP 0, DIVERGENT 4 (on intended differences and the writes switch, WD-03),
MISSING 0. Every route exists in dcl-one. What still
differs is listed in `docs/worlds-compatibility.md` ("Storage and deployments", "Invalid world
names").

---

## 1. POST /entities (single request)

### Request

Upstream chain (`src/controllers/routes.ts:181-197`):

1. `stampRequestArrival()` stamps `requestArrivedAt` before the body is read (`request-arrival.ts:11-20`).
2. `createSourceUploadAdmission` charges the client source (`cf-connecting-ip`, `TRUSTED_CLIENT_IP_HEADER`)
   the declared `Content-Length` (capped at 350 MiB; undeclared charged 350 MiB); a source over
   `MAX_CONCURRENT_UPLOADS_PER_SOURCE` (4) or `MAX_IN_FLIGHT_UPLOAD_BYTES_PER_SOURCE` (350 MiB) gets
   429; a request with no source is not limited per source (`source-upload-admission.ts:15-50`,
   `adapters/source-upload-limits/component.ts:23-41`).
3. `multipartParserWrapper(deployEntity, {limits: DEPLOYMENT_FIELD_LIMITS, …})`
   (`multipart/middleware.ts:86-206`):
   - invalid `Content-Length` → 400; declared over 360 MiB (350 MiB payload + 10 MiB framing) → 413;
   - the shared in-flight budget (4 GiB; each request charged at least 16 MiB, each file part 16 KiB;
     40 orphaned directories) → 503;
   - busboy limits: 32 fields of at most 32 KiB, 10,000 files, 10,032 parts, files of at most 350 MiB,
     payload at most 350 MiB, wire at most 360 MiB (`multipart/limits.ts:5-25`);
   - a repeated form name → 400 `Duplicate form field '<name>'` (`middleware.ts:234-243`);
   - the upload timeout (`MULTIPART_UPLOAD_TIMEOUT_MS`, 5 min) → 408.
4. `deployEntity` (`handlers/deploy-entity-handler.ts:381-453`): `entityId` field (`requireString`),
   the auth chain from `authChain[i][type|payload|signature]` (`logic/extract-auth-chain.ts`), then
   `validateAuthChain`, `validateSigner`, `validateSignature` before anything is read
   (`deploy-entity-handler.ts:80-102`); the entity file (≤ 5 MiB, JSON) and the pre-storage validation
   (`logic/validations/validator.ts:61-86`), all before the content lock; then under the lock the storage
   lookups, `validateFiles` and `createValidateSize` (`validator.ts:98-113`), the fast-fail newer-scene
   check, the storage writes and `deployScene` (`adapters/entity-deployer.ts:54-120`,
   `adapters/worlds-manager.ts:494-764`).

Validation order (upstream, first failure answers): Entity schema → type `scene` → `MAX_FILE_COUNT`
→ auth-chain schema → signer → TTL (300 s back, 900 s forward) → entity id = hash of the entity
file → signature → `Scene` schema and `worldConfiguration.name` → deprecated keys → parcel bounds →
pointers = `scene.parcels` → `MAX_PARCELS` → deny list → permission (owner of the name, else a
deployment grant over the parcels plus the full footprint of every overlapping scene) → minimap
images → skybox textures → thumbnail → files (extra, CIDv1, hash, missing) → size.

dcl-one chain (`deploy.rs` `deploy_entity`, `deploy_entity_inner`, `validate`):
the arrival stamp, Content-Length precheck, per-source admission keyed on `X-Real-IP`
(`upload_limits.rs`), a concurrency slot (`MAX_CONCURRENT_UPLOADS`, 40), busboy's content-type
checks, the form read under upstream's `DEPLOYMENT_FIELD_LIMITS` and the byte and file budgets with
parts spooled to disk (`deploy_form.rs`), the query/form `partial` agreement, then the signer
(`deploy.rs` `authenticate`) before the entity file is read, then `validate` in the same order
as upstream, with these differences: the AJV schema checks are hand-written and report the first
failure (intended, `docs/worlds-compatibility.md`), the world name family is checked after the scene
schema (WD-13), and the world disabled flag and the live owner are read after the permission. Files
are keyed by their form name and held to the CIDv1 of their bytes, as upstream (WD-06). The
newer-scene fast-fail runs before any blob is stored (WD-15); then `store::deploy_scene_with` runs
one transaction (`store.rs`).

### Responses

| trigger | status | upstream body | dcl-one body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | `{"creationTimestamp":<publication time>,"message":"Your scene was deployed to World \"<name>\" at parcels: <p1, p2>!\nAccess world: https://play.decentraland.org/?realm=<enc(base/world/name)>&position=<enc(p1)>"}` | same; `creationTimestamp` is the stored row's `created_at` | `scene-deployments/component.ts:219`, `worlds-manager.ts:571,764`, `logic/utils.ts:48-56` | `deploy.rs` `deploy_entity_inner`, `publish` | MATCH (WD-01) |
| Invalid `Content-Length` | 400 | `{"error":"Bad request","message":"Invalid Content-Length header."}` | same | `middleware.ts:157-161` | `deploy.rs` `deploy_entity` | MATCH |
| Declared > 360 MiB | 413 | `{"error":"Payload Too Large","message":"The multipart request is too large."}` | same from the app and from the gateway's `client_max_body_size` | `middleware.ts:162-167` | `deploy.rs`, `infra/gateway/gateway.py` | MATCH (the gateway answers the same, WD-20) |
| Source over its share | 429 + `Retry-After: 5` | `{"error":"Too Many Requests","message":"Too many uploads in progress from this client, please retry shortly."}` | same; source from `X-Real-IP` | `upload-error-handler.ts:62-68`, `source-upload-limits/errors.ts` | `upload_limits.rs` `admit_source`, `source_limit_response` | MATCH, intended header (worlds-compatibility.md) |
| Budget full | 503 + `Retry-After: 5` | `{"error":"Service Unavailable","message":"Server is buffering too many uploads, please retry shortly."}` | same for bytes and for the slot cap; `…too many upload files…` for files, as upstream's file charge | `multipart/errors.ts:25-29,73-77` | `upload_limits.rs`, `deploy.rs` | MATCH (WD-11); the slot and file caps are intended (worlds-compatibility.md) |
| Temp storage failure | 503 + `Retry-After: 5` | `The server could not store the multipart upload, please retry shortly.` | same | `multipart/errors.ts:67-71` | `deploy_form.rs` `SPOOL_FAILED_MESSAGE` | MATCH (WD-11) |
| More than 32 fields | 413 | `The multipart request has too many fields. The maximum allowed is 32.` | same at the 33rd field | `middleware.ts:313-320`, test `multipart-error-handling.spec.ts:107-135` | `deploy_form.rs` | MATCH (WD-04) |
| Field over 32 KiB | 413 | `Field '<name>' is too large. The maximum allowed size per field is 32768 bytes.` | same | `middleware.ts:331-339`, test `:137-163` | `deploy_form.rs` | MATCH (WD-04) |
| Too many files / parts | 413 | `The multipart request has too many files.` (10,001st file part, the entity included) / `…too many parts.` (busboy answers when the count reaches 10,032) | same | `middleware.ts:307-312`, busboy 1.6.0 `lib/types/multipart.js` | `deploy_form.rs` | MATCH (WD-04) |
| File over 350 MiB / payload over 350 MiB | 413 | `An uploaded file exceeds the maximum allowed size.` / `The multipart request is too large.` | same | `middleware.ts:268-279,382-384` | `deploy_form.rs` `read_deploy_form`, `account_deploy_bytes` | MATCH |
| Part name over 200 bytes | 400 | none: busboy 1.6.0 applies `fieldNameSize` to urlencoded names only (truncating them); a part's headers past 16 KiB answer `Malformed part header` | `Form field name longer than 200 bytes` | busboy 1.6.0 `lib/types/urlencoded.js`, `lib/types/multipart.js` (`MAX_HEADER_SIZE`) | `deploy_form.rs` `read_deploy_form` | intended (worlds-compatibility.md "Publish forms") |
| Repeated form name | 400 | `Duplicate form field '<name>'` (a field once read, a file on its headers) | same | `middleware.ts:234-243` | `deploy_form.rs` | MATCH (WD-05) |
| Not multipart / truncated body | 400 | `{"error":"Bad request","message":"Unsupported content type: application/json"}` / `"Unexpected end of form"` (also `Missing Content-Type`, `Malformed content type`, `Multipart: Boundary not found`) | same; a urlencoded body is refused (upstream parses it) and another unreadable body answers multer's text | test `multipart-error-handling.spec.ts:15-105`, busboy 1.6.0 `lib/index.js` | `deploy_form.rs` `content_type_error`, `read_failed` | MATCH (WD-09), intended rest (worlds-compatibility.md) |
| Body not received in time | 408 | `{"error":"Request Timeout","message":"The upload did not finish within <s> s: received <n> of <m> (about <r> KiB/s). Retry on a faster connection or send smaller batches."}` | same, from the wire bytes counted | `multipart/errors.ts:32-50`, test `multipart-limiter.spec.ts:392-426` | `upload_limits.rs` `describe_upload_timeout` | MATCH (WD-10) |
| Processing deadline | 408 | `The server could not finish processing the deployment within 300 s after receiving it. Retry the deployment; if it keeps timing out, report it.` | same | `deployment-processing.ts:21-29`, `upload-error-handler.ts:25-27` | `upload_limits.rs` `processing_timeout_message` | MATCH (WD-10) |
| `entityId` missing | 400 | `A string was expected` | same; an empty value goes on to validation, as upstream | `logic/utils.ts:37-40`, test `multipart-error-handling.spec.ts:57-80,165-190` | `deploy.rs` `deploy_entity_inner` | MATCH (WD-07) |
| No auth chain | 400 | `Missing auth chain` | same; a JSON `authChain` field is not read, as upstream | `extract-auth-chain.ts:19`, test `source-upload-limits.spec.ts:23-25` | `deploy_validate.rs` `extract_auth_chain_from_fields` | MATCH (WD-07) |
| Auth chain link part missing | 400 | `A string was expected` | same | `extract-auth-chain.ts:21-27` | `deploy_validate.rs` | MATCH (WD-07) |
| More than 10 links | 413 (field cap) | `The multipart request has too many fields. The maximum allowed is 32.` | same | test `partial-deploy.spec.ts:640-661` | `deploy_form.rs` | MATCH (WD-04, WD-07) |
| Auth chain malformed / bad signer / bad signature | 400 | `Deployment failed: <AJV errors>` / `Deployment failed: Invalid signer: <payload>` / `Deployment failed: <@dcl/crypto message>` | same for an unknown link type, the signer and `@dcl/crypto`'s malformed-chain, signer-mismatch, missing-signature, expired-key and final-authority texts; an unrecoverable signature and the verifier's structure rules answer its own text | `deploy-entity-handler.ts:96-101`, `validations/common.ts:60-81`, `@dcl/crypto` 3.7.0 `Authenticator.js` | `deploy.rs` `authenticate`, `crypto_message` | MATCH (WD-08) for those; intended rest (worlds-compatibility.md) |
| EIP-1654 chain | 400 upstream (`validateSignature` with a null provider) | `Deployment failed: ERROR. Link type: <t>. Missing provider.` | that text without `ETH_RPC_URL`; accepted with it, 503 `The auth chain could not be verified right now: …` when it fails | `common.ts:78` | `deploy.rs` `authenticate` | intended superset (worlds-compatibility.md, WD-08) |
| Entity file missing | 400 | `Entity file "<id>" is missing from the request.` | same; a part named `<id>` whose bytes hash elsewhere answers `Invalid entity hash: expected <h> but got <id>` | `deploy-entity-handler.ts:226`, `common.ts:11-23` | `deploy.rs` | MATCH (WD-06) |
| Entity file > 5 MiB / not JSON | 400 | `The entity file is too large. The maximum allowed size is 5242880 bytes.` / `The entity file is not valid JSON.` | same | `deploy-entity-handler.ts:44-50,133-142` | `deploy.rs` `read_entity_file`, `parse_entity`; `deploy_validate.rs` `entity_file_too_large_error` | MATCH |
| Entity schema | 400 | `Deployment failed: <every AJV error, comma-joined>` | `Deployment failed: <first error>` | `common.ts:28-34` | `deploy_validate.rs` `entity_schema_error` | intended (worlds-compatibility.md: first failure only) |
| Type not scene | 400 | `Deployment failed: Entity type <t> is not supported.` | same | `common.ts:166-172` | `deploy.rs` `validate` | MATCH |
| Too many content entries | 400 | `Deployment failed: The deployment has too many files. The maximum allowed is <n> but the deployment has <m>.` | same | `scene.ts:201-213` | `deploy.rs` `validate` | MATCH |
| TTL | 400 | `Deployment failed: Deployment was created <s> secs ago. Max allowed: 300 secs.` / `… <s> secs in the future. Max allowed: 900 secs.` | same | `common.ts:36-58`, tests `deploy.spec.ts:1731-1866` | `deploy.rs` `entity_timestamp_error` | MATCH |
| No timestamp | 400 | AJV `must have required property 'timestamp'` (test matches `/timestamp/`) | `Deployment failed: must have required property 'timestamp'` | test `deploy.spec.ts:1665-1716` | `deploy_validate.rs` `entity_schema_error` | MATCH (pinned regex) |
| No `worldConfiguration.name` | 400 | `Deployment failed: scene.json needs to specify a worldConfiguration section with a valid name inside.` | same | `scene.ts:12-24`, test `deploy.spec.ts:1363-1408` | `deploy.rs` `validate` | MATCH |
| Name not `.dcl.eth` (or `.eth` with ENS off) | 400 | the permission refusal below | `Deployment failed: Only .dcl.eth world names are supported for publishing (got "<name>")` (`.dcl.eth and .eth` with `ALLOW_ENS_DOMAINS`) | `scene.ts:108-163` | `deploy.rs` `validate` | intended (worlds-compatibility.md, WD-13) |
| Deprecated `dclName` / `minimapVisible` / `skybox` | 400 | `Deployment failed: \`dclName\` in scene.json was renamed to \`name\`. …` etc. | same texts, same order | `scene.ts:62-84` | `deploy.rs` `deprecated_config_error` | MATCH |
| Parcel out of bounds | 400 | `Deployment failed: Coordinate X value 9999 is out of bounds. Must be between -150 and 150.` | same | `scene.ts:185-198`, `coordinates/component.ts:33-58`, test `deploy.spec.ts:1589-1648` | `deploy_validate.rs` `validate_parcel_in_bounds`, `deploy.rs` `validate` | MATCH |
| Base not in parcels / not canonical / pointers ≠ parcels | 400 | `Deployment failed: The scene base parcel [b] must be included in the scene parcels [..].` / `… Scene pointers and parcels must be unique canonical parcel coordinates.` / `… The scene pointers [..] must match the scene parcels [..].` | same | `scene.ts:30-60`, test `deploy-parcel-permission.spec.ts:252-321` | `deploy.rs` `scene_pointers_error` | MATCH |
| Over `MAX_PARCELS` | 400 | `Deployment failed: Max allowed scene dimensions is <n> parcels.` | same | `scene.ts:167-178` | `deploy.rs` `validate` | MATCH |
| Name on the DAO deny list | 400 | `Deployment failed: Deployment failed: World "<name>" can not be deployed because the name is in the name deny list managed by Decentraland DAO.` | same (prefix twice in both) | `scene.ts:86-101` | `deploy.rs` `validate` | MATCH |
| No permission | 400 | `Deployment failed: Deployment failed: Your wallet has no permission to publish this scene because it does not have permission to deploy under "<name>". Check scene.json to select a name that either you own or you were given permission to deploy.` | same | `scene.ts:108-163`, tests `deploy.spec.ts:1251-1303`, `deploy-parcel-permission.spec.ts:104-250` | `deploy.rs` `validate` | MATCH |
| Name authority cannot answer | 500 `{"error":"Internal Server Error"}` (the fetch throws) | | 403 `Not authorized: could not verify deployment permissions (deploy denied)` | `dcl-name-checker.ts:9-20` | `deploy.rs` `validate` | intended (worlds-compatibility.md, WD-14) |
| World disabled by an operator | — | | 403 with the disabled message | none | `deploy.rs` `validate` (`store::is_world_disabled`) | dcl-one only |
| Grantee publish, owner unresolved | 500 (`Cannot deploy scene … owner address could not be resolved.` thrown as `Error`) | | 400 with that message; 503 when the authority faults | `entity-deployer.ts:162-169` | `deploy.rs` `validate` | intended (worlds-compatibility.md, WD-14) |
| Minimap / skybox / thumbnail | 400 | `Deployment failed: The file <f> is not present in the entity.` / `The texture file <f> …` / `Scene thumbnail '<t>' must be a relative path …` / `… must be a file included in the deployment.` | same | `scene.ts:320-402` | `deploy.rs` `validate`, `deploy_validate.rs` (`missing_minimap_images`, `missing_skybox_textures`, `validate_navmap_thumbnail`) | MATCH |
| Extra / missing files | 400 | `Deployment failed: Extra file detected <name>, …` + `The file <h> (<f>) is neither present in the storage or in the provided entity` | same; the extra file is named by its form name | `common.ts:89-164` | `deploy.rs` `Uploads::file_errors`, `validate` | MATCH (WD-06) |
| Hash ≠ form name / name not CIDv1 | 400 | `The hashed file doesn't match the provided content: <name>` / `Only CIDv1 are allowed for content files: <name>` | same, with the missing files on a publish and right after the window on a batch | `common.ts:106-124` | `deploy.rs` `Uploads::file_errors` | MATCH (WD-06) |
| Too big | 400 | `Deployment failed: The deployment is too big. The maximum total size allowed is <max> bytes for scenes. You can upload up to <max> bytes but you tried to upload <size>.` | same | `scene.ts:215-271` | `quota.rs` `too_big`, `deploy.rs` `validate` | MATCH |
| Owner of a DCL NAME world not found while sizing | 400 `Deployment failed: Could not determine owner for world <name>` | | 503 `Deployment failed: the storage allowance could not be determined right now` | `limits-manager/component.ts:66-71`, `scene.ts:265-267` | `deploy.rs` `validate` | intended (worlds-compatibility.md, WD-14) |
| A newer scene on the parcels | 400 | `Deployment failed: a newer scene is already deployed on one or more of these parcels.` before any blob is stored | same, before any blob is stored; the publish transaction checks it again | `entity-deployer.ts:65-79`, test `partial-deploy.spec.ts:1600-1655` | `deploy.rs` `deploy_entity_inner`, `store.rs` | MATCH (WD-15) |
| Scoped replacement raced | 409 | `{"error":"Conflict","message":"Scene replacement authorization changed while deploying to world \"<name as deployed>\". Please retry."}` | same | `types.ts:518-523`, `upload-error-handler.ts:35-37` | `store.rs` `deploy_scene_with` | MATCH (WD-16) |
| Same entity deployed twice | 200 | idempotent: the live publication's `creationTimestamp` | 200; the row is kept, so `creationTimestamp` is the first publication's `created_at` | `scene-deployments/component.ts:199-218` | `store.rs` `deploy_scene_with`; test `partial_uploads.rs` `publishing_the_same_entity_twice_keeps_its_publication_time` | MATCH |
| Client disconnect | 499 `{"error":"Client Closed Request",…}` (not seen by a client) | | the request future is dropped | `upload-error-handler.ts:20-24` | — | not observable |
| Unexpected error | 500 | `{"error":"Internal Server Error"}` | same | `error-handler.js` | `deploy.rs` `rejection` | MATCH (WD-14) |

Side effects upstream publishes after the commit (best effort): SNS `WorldDeploymentEvent` with the
`isMultiplayer` attribute, `WORLD_SETTINGS_CHANGED` when the metadata refresh changed the settings,
`unblockIfUnderQuota` (`entity-deployer.ts:190-267`; tests `deploy.spec.ts:265-277,318-360,1175-1246`).
dcl-one kicks the forwarder, re-checks the quota block and updates the in-tree scene identities
(`deploy.rs` `publish`, `store.rs`); it publishes no event (intended, worlds-compatibility.md
"No deployment events", WD-19).

### Data dependency

Name ownership (DCL NAME registrar or ENS), the deployment grants, the storage allowance (wallet
holdings over JSON-RPC), the DAO deny list and the blob store. dcl-one reads these through
`crate::policy::Policy` (`can_deploy`, `live_name_owner`, `world_limits`) and
`CoreState::scene_cap`; the allowance rules are in `docs/worlds-compatibility.md` ("Wallet storage
allowance").

---

## 2. POST /entities with `partial=true`

### Request

Same chain as section 1 up to the handler. Upstream (`deploy-entity-handler.ts:80-235,273-339`):

- `?partial=true` without the form field → 400 `The 'partial=true' query parameter requires the 'partial=true' form field`.
- The signature is checked with `AuthChain.validate` and `Authenticator.validateSignature`, answering
  `Invalid auth chain.` / `Invalid auth chain: <message>` (no `Deployment failed:` prefix).
- Before the lock: the live pending upload, then the replay (a completion receipt of this signer
  within `COMPLETED_UPLOAD_TTL`, else the live publication for any signer → 200 with its
  `creationTimestamp`), then the staging validation (`createStagingValidateFns`, permission skipped for
  the upload's own signer).
- Under the shared content lock and a per-entity advisory lock: the replay again, the stored manifest
  for a resume batch of the upload's own signer, `stage()` (`scene-deployments/component.ts:255-416`):
  newer-scene check for a new upload, size against the scene cap, `admitBatch` (per-account,
  per-world, per-minute and server quotas), storage, progress; 202 `{missing}` or the full validation
  and publication.
- The first batch's lifetime and freshness anchor is the request's arrival (`stampRequestArrival`).

dcl-one (`partial.rs`): the same steps. Batches of one entity are serialized by an
in-process lock taken before validation (intended: no `CONTENT_LOCK_CONNECTIONS`); the anchor is the
request's arrival at the handler, which the gateway reaches only once it holds the whole body (WD-12).

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Batch incomplete | 202 | `{"missing":[<hash>…]}` | `deploy-entity-handler.ts:327`, tests `partial-deploy.spec.ts:471-567`, contract `test/contracts/partial-deployment.ts:23-30` | `partial.rs` `stage_batch` | MATCH |
| Batch completes / single partial with all content | 200 | `{"creationTimestamp":<publication>,"message":…}` | `:318-326`, contract `:96-104` | `partial.rs` `stage_batch`, `success` | MATCH |
| Lost completion replayed by its finalizer | 200 | the receipt's `creationTimestamp` | contract `:32-49` | `partial.rs` `replay` | MATCH |
| Batch of an entity already published | 200 | the live publication's `creationTimestamp` | `:170-175`, contract `:51-75`, test `partial-deploy.spec.ts:1020-1043` | `partial.rs` `replay` | MATCH (WD-01: a single-request publish answers the same row time) |
| `?partial=true` without the field | 400 | `The 'partial=true' query parameter requires the 'partial=true' form field` | `:85-87`, test `:1296-1321` | `deploy.rs` `deploy_entity` | MATCH |
| First request without the entity file / another signer resumes without it | 400 | `The first partial request for an entity must include the entity file "<id>".` | `:296-300`, tests `:805-841,1342-1364` | `partial.rs` `stage_batch` | MATCH |
| Another account sends a batch with the entity file | 400 | `This upload was started by another account.` | `partial-upload-admission/errors.ts:6-11`, test `:1224-1260` | `partial.rs` `upsert` | MATCH |
| Upload expired / reclaimed mid-batch | 400 | `This upload expired. Create a new entity with a fresh timestamp.` | `pending-scenes-repository/errors.ts`, tests `:738-762,929-964` | `partial.rs` `expired`, `store.rs` `PARTIAL_UPLOAD_EXPIRED` | MATCH |
| Upload gone before reserve | 400 | `Upload no longer exists; resend its manifest.` | `errors.ts:13-19` | `partial.rs` `NOT_FOUND_MESSAGE`, `reserve` | MATCH |
| Same file twice in a batch | 400 | `Duplicate form field '<hash>'` | contract `:77-94`, test `:1262-1294` | `deploy_form.rs` (any repeated name) | MATCH (WD-05) |
| Partial batch without permission | 400 | `Deployment failed: Deployment failed: Your wallet has no permission …` | test `:874-890` | `deploy.rs` `validate` | MATCH |
| Tampered file | 400 | `Deployment failed: The hashed file doesn't match the provided content: <hash>` | test `:892-908` | `deploy.rs` `Uploads::file_errors` | MATCH (WD-06) |
| Auth chain refusal | 400 | `Invalid auth chain.` / `Invalid auth chain: <message>` | `:90-94` | `deploy.rs` `authenticate` | MATCH (WD-08; `<message>` as in section 1) |
| Scene over the cap while staging | 400 | `Deployment failed: The deployment is too big.` | `scene-deployments/component.ts:321` | `partial.rs` `stage_batch` | MATCH |
| Newer scene holds the parcels (new upload / stale finalize) | 400 | `Deployment failed: a newer scene is already deployed on one or more of these parcels.` | `:272-281`, tests `:1448-1503` | `partial.rs` `stage_batch`, `store.rs` `deploy_scene_with` | MATCH |
| Account / world / per-account-in-world upload caps | 429 + `Retry-After` | `{"error":"Too Many Requests","message":"Too many partial uploads in progress for this account: <n> of the <max> allowed. …"}` and the world variants | `partial-upload-admission/component.ts:85-118`, tests `:1657-1735` | `partial.rs` `upsert` | MATCH |
| Byte rate, account, world, server budgets | 400 (alone too large) / 429 + `Retry-After` | upstream texts (`formatBytes`) | `component.ts:146-221`, `partial-upload-accounting.spec.ts:326-1470` | `partial.rs` `reserve` | MATCH; the per-source budget is dcl-one only (intended) |
| Personal world not yet published here | — | `Partial uploads to a personal world start once it is published here; …` | none | `partial.rs` `stage_batch` | intended (worlds-compatibility.md) |
| A stored file collected before completion | 400 upstream would fail the publish | | 202 with it in `missing` again | — | `partial.rs` `vanished` | intended (worlds-compatibility.md) |
| Field cap, field size | 413 | as section 1 | tests `:640-684` | as section 1 | MATCH (WD-04) |

---

## 3. DELETE /entities/:world_name

### Request

Upstream (`routes.ts:199`, `handlers/undeploy-entity-handler.ts:6-38`): strict signed fetch; the owner
(`namePermissionChecker.checkPermission`) or a world-wide deployment grantee
(`permissions.hasWorldWidePermission`); `worlds.undeployWorld` soft-deletes every scene, clears the
spawn and the stats, publishes `WorldUndeploymentEvent` and re-checks the owner's block
(`logic/worlds/component.ts:179-197`, `worlds-manager.ts:864-890`).

dcl-one (`scenes.rs` `undeploy_world`): world name parsed first (400 `Invalid world name: …`), the strict signed
fetch, `Policy::is_owner`, else `has_world_wide_permission`, `store::undeploy_world` (rows retired,
spawn recomputed, `store.rs` `undeploy_world`, `recompute_spawn`), quota re-check off the request.

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Undeployed | 200 | the string `{}` (`content-type: text/plain; charset=utf-8`) in both | `undeploy-entity-handler.ts:34-37`, `@dcl/http-server` `logic.js:298-312`, test `undeploy-entity-handler.spec.ts:57-97` | `scenes.rs` `undeploy_world` | MATCH (WD-17) |
| Neither owner nor world-wide grantee | 400 | `{"error":"Bad request","message":"Invalid request. You must have world-wide deployment permission to undeploy the entire world."}` | `:16-29`, test `:20-37` | `scenes.rs` `undeploy_world` | MATCH |
| Scene signer / unsigned / bad signature | 400 / 401 | `{"error":<verify message>,"message":"This endpoint requires a signed fetch request. See ADR-44."}` in both | test `:39-55` | `scenes.rs` `undeploy_world`, `comms/src/signed_fetch.rs` `map_auth_error_api` | MATCH (WD-02) |
| Malformed world name | 400 | dcl-one `Invalid world name: …` before the signature | none | `scenes.rs` | intended (worlds-compatibility.md "Invalid world names", WD-18) |
| Authority cannot answer | 500 upstream; dcl-one the `AuthorityNotEstablished` status (503 for a store fault) | | `dcl-name-checker.ts` | `scenes.rs` | intended (worlds-compatibility.md, WD-14) |

---

## 4. DELETE /world/:world_name/scenes/:coordinate

### Request

Upstream (`routes.ts:210`, `scenes-handler.ts:87-133`): strict signed fetch; the coordinate must match
`^-?\d+,-?\d+$` (no bounds check); the owner removes every scene on the parcel; anyone else needs a
deployment grant over the whole footprint of every scene on it (else 401), and only those scene
identities are removed; `undeployWorldScenes` recomputes the spawn and publishes
`WorldScenesUndeploymentEvent` (`logic/worlds/component.ts:210-244`, `worlds-manager.ts:1052-1120`).

dcl-one (`scenes.rs` `delete_scene`): world name parsed first, the strict signed fetch, `Parcel::parse_strict`
(the same pattern, values within `i32`, `comms/src/world_identity.rs`), `Policy::is_owner`, else
`has_permission_covering` over the parcel plus the footprints of the overlapping scenes, then
`store::undeploy_scene` scoped to those identities (`store.rs` `undeploy_scene`). Runs under the 30 s
`request_timeout` layer (`mod.rs` `request_timeout`).

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Undeployed, or nothing on the parcel | 200 | `{"message":"Scene at parcel <coordinate as sent> undeployed successfully"}` | `:127-132`, tests `scenes-handler.spec.ts:555-940` | `scenes.rs` `delete_scene` | MATCH |
| Non-canonical alias (`020,024`) | 200 | removes the scene stored at `20,24`; the message echoes `020,024` | test `:622-642` | `Parcel::parse_strict` then `canonical()` (`store.rs` `undeploy_scene`) | MATCH |
| Grant over part of a multi-parcel scene / no grant | 401 | `{"error":"Not Authorized","message":"Unauthorized. You do not have permission to undeploy scenes in this world."}` | `:103-125`, tests `:686-800` | `scenes.rs` `delete_scene` | MATCH |
| Bad coordinate | 400 | `{"error":"Bad request","message":"Invalid coordinate format: abc,def. Expected format: x,y (e.g., 0,0 or -1,2)"}` | `:9-16`, test `:803-830` | `scenes.rs` `delete_scene` | MATCH; a coordinate outside `i32` is 400 here and 200 upstream (intended, WD-18) |
| Unsigned / scene signer | 400 | `{"error":"Invalid Auth Chain","message":"This endpoint requires a signed fetch request. See ADR-44."}` in both | test `:833-856` | `map_auth_error_api` | MATCH (WD-02) |
| Spawn on the removed scene / elsewhere / last scene | 200 | spawn moved to the remaining bounding rectangle's centre / kept / cleared | tests `:943-1075` | `store.rs` `recompute_spawn` | MATCH |
| Slow request | 408 | dcl-one `request timed out` after 30 s | none | `mod.rs` `request_timeout` | intended (worlds-compatibility.md "Invalid world names", WD-18) |

---

## Backlog (WD-)

1. **WD-01 · `POST /entities` · `creationTimestamp` of a single-request deploy.** Status: MATCH, no work.
   - Upstream: `{ creationTimestamp: Date.now(), ...result }`, where `result.creationTimestamp` is the
     time `deployScene` wrote into `world_scenes.created_at` under the world lock
     (`decentraland/worlds-content-server:src/logic/scene-deployments/component.ts:219`,
     `src/adapters/worlds-manager.ts:558-572,731,764`), pinned by
     `test/unit/scene-deployments.spec.ts:1029-1110`. A later partial batch of the same entity
     answers that same value (`test/integration/partial-deploy.spec.ts:1020-1043`,
     `test/contracts/partial-deployment.ts:51-75`).
   - dcl-one: `deploy_entity_inner` answers the `created_at_ms` that `publish` returns
     (`services/catalyst/worlds/src/core/deploy.rs`), the row's time a later batch's
     `replay` also answers (`partial.rs` `replay`); a second single-request publish of the same
     entity keeps the row and answers the same time. Pinned by
     `worlds/src/core/tests/partial_uploads.rs`
     `a_batch_of_an_entity_published_in_one_request_answers_its_publication_time` and
     `publishing_the_same_entity_twice_keeps_its_publication_time`.
   - Severity: none (closed).

2. **WD-02 · `DELETE /entities/:world_name`, `DELETE /world/:world_name/scenes/:coordinate` · signed-fetch refusal body.** Status: MATCH, no work.
   - Upstream: `{"error":<verify message>,"message":"This endpoint requires a signed fetch request. See ADR-44."}`
     with the middleware's status (`src/controllers/routes.ts:72-98`; `@dcl/crypto-middleware` 6.3.0
     `dist/index.js` `errorToResponse`); tests pin `message`
     (`test/integration/undeploy-entity-handler.spec.ts:39-55`, `scenes-handler.spec.ts:833-856`).
   - dcl-one: `map_auth_error_api` (`services/catalyst/comms/src/signed_fetch.rs`) answers every
     worlds signed route with the crypto-middleware 6.3.0 text in `error` and the ADR-44 line in
     `message` (worlds-comms.md WC-01). Pinned by `worlds/src/core/tests.rs`
     `signed_routes_refuse_with_the_crypto_middleware_body` (unsigned and wrong-payload requests on
     both routes).
   - Severity: none (closed).

3. **WD-03 · all routes of this area · the worlds writes switch is off by default.** Status: intended (worlds-compatibility.md).
   - Upstream: no switch.
   - dcl-one: `WORLDS_WRITES_ENABLED` defaults to `false`; while off every route here answers
     503 `{"ok":false,"error":"Service Unavailable","message":…}` with `Retry-After: 3600`
     (`services/catalyst/worlds/src/writes.rs` `is_gated`, `refused_response`; `config/src/worlds.rs`
     `worlds_writes_enabled`). Listed under "Writes
     switch", one line for every worlds write route (WW-27): DATABASE.md requires a demonstrated
     restore before local authority is accepted.
   - Severity: none (closed).

4. **WD-04 · `POST /entities` · multipart field, part and file caps.** Status: MATCH, no work.
   - Upstream: `DEPLOYMENT_FIELD_LIMITS` over busboy 1.6.0: 32 fields, 32 KiB per field, 10,032
     parts, 10,000 files; 413 `The multipart request has too many fields. The maximum allowed is 32.`,
     `Field '<name>' is too large. The maximum allowed size per field is 32768 bytes.`,
     `The multipart request has too many files.`, `The multipart request has too many parts.`,
     `An uploaded file exceeds the maximum allowed size.` and `The multipart request is too large.`
     (`src/controllers/multipart/limits.ts:5-25`, `middleware.ts:268-339,382-384`; tests
     `multipart-error-handling.spec.ts:107-163`, `partial-deploy.spec.ts:621-684`). The settings
     route goes through the same parser with the default limits.
   - dcl-one: `read_deploy_form` (`services/catalyst/worlds/src/core/deploy_form.rs`) caps 32
     fields of at most 32 KiB, 10,000 file parts (the entity included), and answers the parts cap
     when the count reaches 10,032, as busboy emits `partsLimit`; an 11-link chain meets the field
     cap. The texts are `upload_limits.rs` `too_many_fields_message`, `field_too_large_message`,
     `TOO_MANY_FILES_MESSAGE`, `TOO_MANY_PARTS_MESSAGE`,
     `FILE_TOO_LARGE_MESSAGE` and `PAYLOAD_TOO_LARGE_MESSAGE`, with the route's 32 and 32768.
     Pinned by `upload_limits.rs` `multipart_limit_texts_are_upstreams` and `deploy_form.rs`
     `the_33rd_field_is_refused_with_the_cap`, `a_field_over_32_kib_is_refused_by_name`,
     `read_deploy_form_takes_at_most_busboys_file_count` (status and body).
   - Severity: none (closed).

5. **WD-05 · `POST /entities` · repeated form names.** Status: MATCH, no work.
   - Upstream: any repeated part name answers 400 `Duplicate form field '<name>'`
     (`src/controllers/multipart/middleware.ts:234-243`).
   - dcl-one: `read_deploy_form` refuses any repeated part name with that 400, a file on its headers
     and a field once read (`deploy_form.rs` `read_deploy_form`, `duplicate`); the partial path uses the same rule. Pinned by
     `deploy_form.rs` `a_repeated_form_name_is_refused` and
     `worlds/src/core/tests/partial_uploads.rs` `malformed_batches_are_refused`.
   - Severity: none (closed).

6. **WD-06 · `POST /entities` · files keyed by their form name.** Status: MATCH, no work.
   - Upstream: `files` is keyed by the part name; each name must be referenced, be a CIDv1 and equal
     the hash of the bytes; the entity file must hash to `entityId`
     (`src/logic/validations/common.ts:11-23,89-124`; test `partial-deploy.spec.ts:892-908`).
   - dcl-one: `SpooledFile` keeps its part name and `Uploads` is keyed by it (`deploy.rs` `Uploads::new`);
     `validate` (`deploy.rs`) runs `validateEntityId` and `validateUploadedFiles`
     (`Uploads::entity_hash_error`, `Uploads::file_errors`) in upstream's places: on a publish the entity id
     after the window and the uploads with the missing files, on a batch the entity id before the
     window and the uploads after it. Blobs are stored under the CIDv1 of their bytes, which the
     checks make equal to the name. Pinned by `worlds/src/core/tests/partial_uploads.rs`
     `a_part_not_named_by_its_hash_is_refused` and `a_part_not_named_by_a_cidv1_is_refused`, and
     `worlds/src/core/tests.rs`
     `a_publish_answers_the_first_failed_check_in_the_old_words`.
   - Severity: none (closed).

7. **WD-07 · `POST /entities` · form and auth-chain extraction messages.** Status: MATCH, no work.
   - Upstream: `requireString` answers `A string was expected` for a missing `entityId` or link part;
     no indexed field answers `Missing auth chain`; only the indexed form is read
     (`src/logic/extract-auth-chain.ts:6-30`, `src/logic/utils.ts:37-40`; tests
     `multipart-error-handling.spec.ts:57-80,165-190`, `source-upload-limits.spec.ts:23-25`).
   - dcl-one: the same texts; only the indexed fields are read, so a JSON `authChain` field is
     ignored, and an empty `entityId` goes on to validation (`deploy_validate.rs`
     `extract_auth_chain_from_fields`, `deploy.rs` `deploy_entity_inner`). Pinned by
     `deploy_validate.rs` `extract_auth_chain_refuses_in_upstreams_words` and
     `worlds/src/core/tests.rs` `deploy_form_and_auth_chain_refusals_answer_upstreams_texts`.
   - Severity: none (closed).

8. **WD-08 · `POST /entities` · auth-chain verification messages and EIP-1654.** Status: intended (worlds-compatibility.md).
   - Upstream: single request `Deployment failed: <AJV errors>` / `Invalid signer: <p>` / the
     `@dcl/crypto` message; partial `Invalid auth chain.` / `Invalid auth chain: <message>`; contract
     wallets are refused (null provider) (`src/controllers/handlers/deploy-entity-handler.ts:80-102`,
     `src/logic/validations/common.ts:60-81`).
   - dcl-one: `deploy.rs` `authenticate` and `crypto_message`: a batch answers
     `Invalid auth chain.` / `Invalid auth chain: <message>`, a single request
     `Deployment failed: <message>`; `<message>` is `@dcl/crypto` 3.7.0's for a malformed chain, a
     lone SIGNER, a signer mismatch, a missing signature, an expired key, the final authority and
     (without `ETH_RPC_URL`) a contract wallet's `Missing provider`; an unknown link type answers
     AJV's `must be equal to one of the allowed values`. Listed under "Publish auth chains": an
     unrecoverable signature and the shared verifier's stricter structure answer the verifier's
     text, the other `AuthChain` schema texts (a SIGNER with a signature) are not rebuilt, and
     EIP-1654 chains are accepted with `ETH_RPC_URL`. `worlds/src/core/tests.rs`
     `deploy_form_and_auth_chain_refusals_answer_upstreams_texts` pins the single request's
     `Invalid signer`, final-authority and unknown-link-type texts, and
     `worlds/src/core/tests/partial_uploads.rs`
     `every_batch_is_authenticated_and_only_the_starter_resumes_without_the_manifest` a batch's
     `Invalid auth chain: <message>` with the final-authority text. The malformed-chain, lone-SIGNER,
     signer-mismatch, missing-signature, expired-key and `Missing provider` texts and a batch's
     `Invalid auth chain.` are not pinned by a test.
   - Severity: none (closed).

9. **WD-09 · `POST /entities` · non-multipart and truncated bodies.** Status: intended (worlds-compatibility.md).
   - Upstream: JSON 400 `Unsupported content type: application/json`, `Unexpected end of form`
     (`test/integration/multipart-error-handling.spec.ts:15-105`).
   - dcl-one: the handler takes the request and checks the content type in busboy's words
     (`Missing Content-Type`, `Malformed content type`, `Unsupported content type: <type>`,
     `Multipart: Boundary not found`) after the source and slot admission (`deploy_form.rs`
     `content_type_error`); multer's incomplete-stream errors answer `Unexpected end of form` and
     the body limit the 413 payload text (`deploy_form.rs` `read_failed`). Listed under
     "Publish forms": a urlencoded body is refused (upstream parses it) and other parser errors
     answer multer's text. Pinned by
     `deploy_form.rs` `the_content_type_is_refused_in_busboys_words` and `tests.rs`
     `deploy_form_and_auth_chain_refusals_answer_upstreams_texts`.
   - Severity: none (closed).

10. **WD-10 · `POST /entities` · upload and processing timeout messages.** Status: MATCH, no work.
    - Upstream: `describeUploadTimeout` (seconds, bytes received of declared, KiB/s) and
      `The server could not finish processing the deployment within <s> s after receiving it. Retry the deployment; if it keeps timing out, report it.`
      (`src/controllers/multipart/errors.ts:32-50`, `src/logic/deployment-processing.ts:21-29`; test
      `multipart-limiter.spec.ts:392-426`).
    - dcl-one: the body stream counts wire bytes, and `describe_upload_timeout` /
      `processing_timeout_message` (`upload_limits.rs`) format upstream's texts with
      `format_bytes`. Pinned by `upload_limits.rs` `timeout_texts_are_upstreams`.
    - Severity: none (closed).

11. **WD-11 · `POST /entities` · in-flight budget answers.** Status: intended (worlds-compatibility.md).
    - Upstream: one byte budget bounds bytes, requests (16 MiB minimum each) and files (16 KiB each);
      every shed says `Server is buffering too many uploads, please retry shortly.` or
      `…too many upload files…`; a failed temp write `The server could not store the multipart upload, please retry shortly.`
      (`src/adapters/in-flight-upload-budget/component.ts:63-140`, `multipart/errors.ts:25-77`).
    - dcl-one: separate caps (`MAX_CONCURRENT_UPLOADS` 40, lowered to three quarters of the pool;
      40,000 files) listed under "Publish forms"; the slot cap answers upstream's byte-budget text
      (`deploy.rs` `deploy_entity`), the file budget upstream's file text and a spool failure upstream's storage
      text (`deploy_form.rs` `SPOOL_FAILED_MESSAGE`). The settings route keeps its own slot text.
      Pinned by `worlds/src/core/tests.rs` `a_publish_past_the_upload_slots_gets_the_byte_budget_503`.
    - Severity: none (closed).

12. **WD-12 · `POST /entities?partial=true` · the first batch's arrival.** Status: intended (worlds-compatibility.md).
    - Upstream: `stampRequestArrival` before parsing; the upload's `created_at` and freshness anchor
      are the arrival (`src/controllers/request-arrival.ts`, `scene-deployments/component.ts:251,265`;
      test `partial-deploy.spec.ts:569-604`).
    - dcl-one: the arrival is stamped at handler entry (`deploy.rs` `deploy_entity`) and is the first batch's
      anchor (`partial.rs` `stage_batch`). The gateway buffers the body before proxying it, so the arrival the
      app sees is the body's end at the gateway; listed under "Publish forms". Pinned by
      `worlds/src/core/tests/partial_uploads.rs` `a_first_batch_is_dated_when_it_arrives`.
    - Severity: none (closed).

13. **WD-13 · `POST /entities` · world name family and personal worlds.** Status: intended (worlds-compatibility.md).
    - Upstream: any name; one that nobody owns fails the permission check with its message
      (`src/logic/validations/scene.ts:108-163`, `src/adapters/name-ownership.ts:46-60,98`).
    - dcl-one: `Only .dcl.eth world names are supported for publishing (got "<n>")` (or `.dcl.eth and .eth`
      with `ALLOW_ENS_DOMAINS`) before the deprecated-config check; a `<address>.dcl.eth` personal world
      is deployable by its address (`services/catalyst/worlds/src/core/deploy.rs` `validate`). Listed
      under "Publish world names".
    - Severity: none (closed).

14. **WD-14 · all routes · authority and internal faults.** Status: intended (worlds-compatibility.md).
    - Upstream: a failing name lookup or unresolved owner throws, answering 500
      `{"error":"Internal Server Error"}`; a DCL NAME world without an owner while sizing answers
      400 `Deployment failed: Could not determine owner for world <n>`
      (`src/adapters/entity-deployer.ts:162-169`, `limits-manager/component.ts:66-71`).
    - dcl-one: a deploy 500 is `{"error":"Internal Server Error"}` (`deploy.rs` `rejection`,
      pinned by `deploy.rs` `refusals_answer_the_old_envelope_and_body_limits_413`). Authority
      faults fail closed: 403 `Not authorized: could not verify deployment permissions (deploy denied)`,
      400 `Cannot deploy scene … owner address could not be resolved.`, 503 for owner/allowance
      faults (`deploy.rs` `validate`); listed under "Authority faults on writes fail closed".
    - Severity: none (closed).

15. **WD-15 · `POST /entities` · a newer-scene refusal before any blob is stored.** Status: MATCH, no work.
    - Upstream: the fast-fail runs before any storage write (`src/adapters/entity-deployer.ts:65-79`;
      test `partial-deploy.spec.ts:1600-1655` checks nothing was stored).
    - dcl-one: `deploy_entity_inner` calls `refuse_if_newer_scene` (`store::newer_scene_deployed`) before the
      store loop (`deploy.rs`); the publish transaction checks it again. Pinned by
      `worlds/src/core/tests.rs` `publishing_an_older_entity_answers_400_and_keeps_the_newer_scene`
      (the refused publish's file is not stored).
    - Severity: none (closed).

16. **WD-16 · `POST /entities` · the 409 names the world as deployed.** Status: MATCH, no work.
    - Upstream: `SceneReplacementConflictError(worldName)` with the name as deployed (`src/types.ts:518-523`).
    - dcl-one: `world.display()` (`services/catalyst/worlds/src/core/store.rs` `deploy_scene_with`). Pinned by
      `worlds/src/core/tests.rs` `scoped_replacement_conflicts_on_unauthorized_overlap` (a
      mixed-case world).
    - Severity: none (closed).

17. **WD-17 · `DELETE /entities/:world_name` · 200 content type.** Status: MATCH, no work.
    - Upstream: `body: '{}'`, a string, so `text/plain; charset=utf-8`
      (`src/controllers/handlers/undeploy-entity-handler.ts:34-37`; `@dcl/http-server` 2.3.0 `dist/logic.js:298-312`).
    - dcl-one: `undeploy_world` answers `([(CONTENT_TYPE, "text/plain; charset=utf-8")], "{}")`
      (`services/catalyst/worlds/src/core/scenes.rs` `undeploy_world`). Pinned by `worlds/src/core/tests.rs`
      `every_route_answers_its_contract` (status, content type and body).
    - Severity: none (closed).

18. **WD-18 · DELETE routes · checks upstream does not have.** Status: intended (worlds-compatibility.md).
    - Upstream: the signature first, no world name check, the coordinate pattern only, no request timeout
      (`src/controllers/handlers/scenes-handler.ts:9-16,87-97`).
    - dcl-one: `parse_world` answers 400 `Invalid world name: …` before the signature
      (`services/catalyst/worlds/src/core/permissions.rs` `parse_world`); a coordinate outside `i32` is 400
      (`comms/src/world_identity.rs` `Parcel::parse_strict`); the scene route answers 408 `request timed out` after 30 s
      (`mod.rs` `request_timeout`). Listed under "Invalid world names", with the `i32` coordinate and the 30 s
      timeout.
    - Severity: none (closed).

19. **WD-19 · all routes · no deployment or undeployment events.** Status: intended (worlds-compatibility.md).
    - Upstream: SNS `WorldDeploymentEvent` (with `isMultiplayer`), `WORLD_SETTINGS_CHANGED` after a deploy
      that changed the settings, `WorldUndeploymentEvent`, `WorldScenesUndeploymentEvent`
      (`src/adapters/entity-deployer.ts:196-248`, `src/logic/worlds/component.ts:179-244`).
    - dcl-one: in-tree consumers only (scene identities, the forward outbox, the quota re-check)
      (`services/catalyst/worlds/src/core/deploy.rs` `publish`, `store.rs`). "No deployment events"
      names what replaces each event.
    - Severity: none (closed).

20. **WD-20 · `POST /entities` · gateway limits answer before the app.** Status: intended (worlds-compatibility.md).
    - Upstream: no request rate limit on this route; its own JSON 413/429.
    - dcl-one: nginx `limit_req zone=deploy` (5 r/s, burst 4), 10 connections per address and
      `client_max_body_size 360m`, the body buffered before the app (listed under "Publish forms").
      The worlds deploy location sends nginx's own refusals to named locations that answer
      `Content-Type: application/json` (as the app's axum `Json`) with the gateway's security and CORS
      headers: 413 `{"error":"Payload Too Large","message":"The multipart request is too large."}`;
      429 `{"error":"Too Many Requests","message":"Too many deploy requests from this client, please retry shortly."}`
      with `Retry-After: 1` over the request rate; 429
      `{"error":"Too Many Requests","message":"Too many uploads in progress from this client, please retry shortly."}`
      with `Retry-After: 5` over the connection cap. The app's own errors pass through
      (`infra/gateway/gateway.py`). A body that stalls for 300 s (`client_body_timeout`, between two
      reads) gets no response: nginx closes the connection and logs 408, so the app's 408 upload-timeout
      JSON is not reached behind the gateway (open gap). Pinned by `infra/gateway/tests/test_config.py`
      `test_worlds_deploy_limits_answer_the_services_json`.
    - Severity: none (closed).

21. **WD-21 · `POST /entities` · validation texts of the scene checks.** Status: MATCH, no work.
    - Upstream: `src/logic/validations/{common,scene}.ts` (TTL, file count, type, `worldConfiguration`,
      deprecated keys, bounds, pointers, parcels cap, deny list, permission, minimap, skybox, thumbnail,
      extra/missing files, size).
    - dcl-one: the same texts and order (`services/catalyst/worlds/src/core/deploy.rs` `validate`,
      `deploy_validate.rs`, `quota.rs` `too_big`); AJV schema texts first-failure only (intended).
    - Severity: none (closed).

22. **WD-22 · `POST /entities` · success message.** Status: MATCH, no work.
    - Upstream: `buildSceneDeploymentMessage` (`src/logic/utils.ts:48-56`; test `deploy.spec.ts:203-213`).
    - dcl-one: `deployment_message` with `encodeURIComponent` (`deploy.rs`).
    - Severity: none (closed).

23. **WD-23 · `POST /entities` · Content-Length, payload and file size limits, entity file.** Status: MATCH, no work.
    - Upstream: `src/controllers/multipart/middleware.ts:157-167,268-279,382-384`,
      `deploy-entity-handler.ts:27,44-50,133-142`.
    - dcl-one: `deploy.rs` `declared_length_exceeds_limit`, `deploy_form.rs`
      `account_deploy_bytes`, `deploy_validate.rs` (`The entity file is too large…`).
    - Severity: none (closed).

24. **WD-24 · `POST /entities` · per-source admission.** Status: MATCH, no work.
    - Upstream: `src/controllers/source-upload-admission.ts:15-50` (`cf-connecting-ip`; unattributed not limited).
    - dcl-one: `upload_limits.rs` per-source admission (`X-Real-IP`, IPv6 by /64, intended); 429,
      `Retry-After: 5`, upstream's message.
    - Severity: none (closed).

25. **WD-25 · `POST /entities?partial=true` · the partial contract.** Status: MATCH, no work.
    - Upstream: `test/contracts/partial-deployment.ts`, `partial-deploy.spec.ts` (202 `{missing}`, completion,
      replay, expiry, ownership, query/field agreement, manifest required, quotas with `Retry-After`).
    - dcl-one: `services/catalyst/worlds/src/core/partial.rs`, tests in `worlds/src/core/tests/partial_uploads.rs`;
      the intended differences are in `docs/worlds-compatibility.md` ("Partial uploads").
    - Severity: none (closed).

26. **WD-26 · `DELETE /world/:world_name/scenes/:coordinate` · authorization scope and spawn.** Status: MATCH, no work.
    - Upstream: owner unrestricted; grantee over the full footprint, scoped to the identities it saw;
      spawn kept, recentred or cleared (`src/controllers/handlers/scenes-handler.ts:99-127`,
      `worlds-manager.ts:1052-1120`; tests `scenes-handler.spec.ts:555-1075`).
    - dcl-one: `scenes.rs` `delete_scene`, `store.rs` `undeploy_scene`, `recompute_spawn`.
    - Severity: none (closed).

27. **WD-27 · `DELETE /entities/:world_name` · authorization.** Status: MATCH, no work.
    - Upstream: owner or world-wide deployment grantee, else 400 with the fixed text
      (`undeploy-entity-handler.ts:16-29`).
    - dcl-one: `scenes.rs` `undeploy_world`.
    - Severity: none (closed).

## Endpoints dcl-one serves in this area that upstream does not

None at a new path. Behaviour dcl-one adds on these routes:

- `POST /entities` accepts EIP-1654 chains when `ETH_RPC_URL` is set (WD-08), personal worlds `<address>.dcl.eth` (WD-13), answers 403 for an operator-disabled world and
  503 when stored content changed under the publish (`store.rs` `missing_stored_file`), and charges a per-source
  staging budget on partial uploads (`MAX_PENDING_BYTES_PER_SOURCE`, documented).
- `/admin/gc` (root and prefix) and `/gc` (prefix) reclaim expired partial uploads with the collector
  (`mod.rs` `router`); they belong to the operator area.

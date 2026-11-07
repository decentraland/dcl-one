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
  Not read: `@dcl/schemas` (the AJV texts of `Entity`, `AuthChain`, `Scene`), `@dcl/crypto`
  (`Authenticator.validateSignature` texts), `busboy` (parser error texts beyond those the tests pin).
- dcl-one: `fix/worlds-content-parity` @ `2da8b8aea` (origin/main), Rust workspace `services/catalyst`.

Everything below is from reading the code and tests of both trees. Nothing was built, run or
captured, and no line is "verified".

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
  the content server (`worlds/src/core/mod.rs:583-596`, `content_colliding_routes` at `:616-628`). The
  gateway rewrites the `worlds-content-server.<domain>` host into that prefix, so a client pointed at
  the worlds host reaches them at upstream's paths. `DELETE /world/{world_name}/scenes/{scene_coord}`
  answers at the root and under the prefix (`mod.rs:674-677`).
- **Writes switch.** Every route in this area is a write behind `WORLDS_WRITES_ENABLED`
  (`worlds/src/writes.rs:33-56`), default `false` (`config/src/worlds.rs:86`). Off, each answers 503
  `{"ok":false,"error":"Service Unavailable","message":…}` with `Retry-After: 3600`
  (`writes.rs:62-76`). The rows below assume the switch is on (WD-03).
- **Error envelope.** Upstream answers a thrown `InvalidRequestError` with
  `400 {"error":"Bad request","message":…}`, `NotAuthorizedError` with `401 {"error":"Not Authorized",…}`
  and anything else with `500 {"error":"Internal Server Error"}` (`@dcl/http-commons` 2.0.0
  `dist/controllers/handlers/error-handler.js`). The upload errors are mapped by
  `uploadErrorHandler` and the deploy handler (`src/controllers/upload-error-handler.ts:19-70`).
  dcl-one answers deploy refusals with `DeployRejection {error, message}`, `error` being
  `Bad request` for 400 and the status's reason phrase otherwise
  (`worlds/src/core/deploy.rs:89-103`), and the DELETE routes with `ApiError` through
  `comms/src/worlds_envelope.rs:133-140`.
- **Signed fetch (DELETE routes).** Upstream's strict `signedFetchMiddleware` (no legacy payload
  fallback, `rejectIfSigner('decentraland-kernel-scene')`, `src/controllers/routes.ts:72-98`)
  answers a refusal with the middleware's status (400 shape, 401 signature/expiry, 503 catalyst)
  and the body `{"error":<verify message>,"message":"This endpoint requires a signed fetch request. See ADR-44."}`.
  dcl-one's `require_verified` with an empty key list (`comms/src/signed_fetch.rs:131-185`) has the
  same statuses and the same scene gate, but a different body (WD-02).
- **Gateway.** `POST /worlds-content-server/entities` passes nginx's `limit_req zone=deploy`
  (5 r/s, burst 4), `limit_conn` 10 per address and `client_max_body_size 360m`, and nginx buffers the
  body before the app sees it (`infra/gateway/gateway.py:13,22,169-191,332-335`). Upstream applies no
  request rate limit here (WD-20).

## Endpoint inventory

| # | method | upstream path | dcl-one route | status |
|---|---|---|---|---|
| 1 | POST | `/entities` (single request) | `/worlds-content-server/entities` → `services/catalyst/worlds/src/core/deploy.rs:143` (`deploy_entity`), route `core/mod.rs:618-621` | DIVERGENT |
| 2 | POST | `/entities` with form field `partial=true` (and optional `?partial=true`) | same handler → `core/partial.rs:448` (`stage`) | DIVERGENT |
| 3 | DELETE | `/entities/:world_name` | `/worlds-content-server/entities/{world_name}` → `core/scenes.rs:105` (`undeploy_world`), route `mod.rs:627` | DIVERGENT |
| 4 | DELETE | `/world/:world_name/scenes/:coordinate` | root and prefix → `core/scenes.rs:46` (`delete_scene`), route `mod.rs:674-677` | DIVERGENT |

Counts: 4 endpoints. MATCH 0, GAP 0, DIVERGENT 4, MISSING 0. Every route exists in dcl-one. The happy
paths answer upstream's status and body shape; each route differs on error texts, one field or a
header, as listed below.

---

## 1. POST /entities (single request)

### Request

Upstream chain (`src/controllers/routes.ts:169-185`):

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

dcl-one chain (`deploy.rs:143-264`, `deploy_entity_inner` at `:832-935`, `validate` at `:492-724`):
Content-Length precheck, per-source admission keyed on `X-Real-IP` (`upload_limits.rs:350-379`), a
concurrency slot (`MAX_CONCURRENT_UPLOADS`, 40), the form read under the byte and file budgets with
parts spooled to disk (`deploy_form.rs`), the query/form `partial` agreement, then the signer
(`authenticate`, `deploy.rs:437-464`) before the entity file is read, then `validate` in the same order
as upstream, with these differences: the AJV schema checks are hand-written and report the first
failure (intended, `docs/worlds-compatibility.md`), the world name family is checked after the scene
schema, the world disabled flag and the live owner are read after the permission, and the files are
keyed by the CIDv1 of their bytes instead of their form name (WD-06). The blobs are stored, then
`store::deploy_scene_with` runs one transaction (`store.rs:598-895`).

### Responses

| trigger | status | upstream body | dcl-one body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| Success | 200 | `{"creationTimestamp":<publication time>,"message":"Your scene was deployed to World \"<name>\" at parcels: <p1, p2>!\nAccess world: https://play.decentraland.org/?realm=<enc(base/world/name)>&position=<enc(p1)>"}` | same keys and message; `creationTimestamp` is the request's time, not the row's | `scene-deployments/component.ts:219`, `worlds-manager.ts:571,764`, `logic/utils.ts:48-56` | `deploy.rs:921-935`, `:304-326` | DIVERGENT (WD-01) |
| Invalid `Content-Length` | 400 | `{"error":"Bad request","message":"Invalid Content-Length header."}` | same | `middleware.ts:157-161` | `deploy.rs:150-153` | MATCH |
| Declared > 360 MiB | 413 | `{"error":"Payload Too Large","message":"The multipart request is too large."}` | same from the app; the gateway answers nginx's HTML 413 first | `middleware.ts:162-167` | `deploy.rs:157-161` | MATCH (app), WD-20 (gateway) |
| Source over its share | 429 + `Retry-After: 5` | `{"error":"Too Many Requests","message":"Too many uploads in progress from this client, please retry shortly."}` | same; source from `X-Real-IP` | `upload-error-handler.ts:62-68`, `source-upload-limits/errors.ts` | `upload_limits.rs:380-392` | MATCH, intended header (worlds-compatibility.md) |
| Budget full | 503 + `Retry-After: 5` | `{"error":"Service Unavailable","message":"Server is buffering too many uploads, please retry shortly."}` | same for bytes; `…too many upload files…` for files; `Server is handling too many concurrent uploads, please retry shortly.` for the slot cap | `multipart/errors.ts:25-29,73-77` | `upload_limits.rs:20-24`, `deploy.rs:173-181` | DIVERGENT (WD-11) |
| Temp storage failure | 503 + `Retry-After: 5` | `The server could not store the multipart upload, please retry shortly.` | `Server could not buffer the upload, please retry shortly.` | `multipart/errors.ts:67-71` | `deploy_form.rs:21-27` | DIVERGENT (WD-11) |
| More than 32 fields | 413 | `The multipart request has too many fields. The maximum allowed is 32.` | at 101 fields: `The multipart request has too many fields.` | `middleware.ts:313-320`, test `multipart-error-handling.spec.ts:107-135` | `deploy_form.rs:163-168`, `upload_limits.rs:35` | DIVERGENT (WD-04) |
| Field over 32 KiB | 413 | `Field '<name>' is too large. The maximum allowed size per field is 32768 bytes.` | over 1 MiB: `The multipart request is too large.` | `middleware.ts:331-339`, test `:137-163` | `deploy_form.rs:170-181` | DIVERGENT (WD-04) |
| Too many files / parts | 413 | `The multipart request has too many files.` (10,001st) / `…too many parts.` (10,033rd) | same texts at the 10,002nd file / 10,101st part | `middleware.ts:307-312` | `deploy_form.rs:99-110` | DIVERGENT (WD-04) |
| File over 350 MiB / payload over 350 MiB | 413 | `An uploaded file exceeds the maximum allowed size.` / `The multipart request is too large.` | same | `middleware.ts:268-279,382-384` | `deploy_form.rs:144-149`, `:33-67` | MATCH |
| Repeated form name | 400 | `Duplicate form field '<name>'` | fields: last value wins; files: repeated bytes kept once; no error | `middleware.ts:234-243` | `deploy_form.rs:197`, `deploy.rs:415-432` | DIVERGENT (WD-05) |
| Not multipart / truncated body | 400 | `{"error":"Bad request","message":"Unsupported content type: application/json"}` / `"Unexpected end of form"` | axum's `Multipart` rejection (plain text) / `Failed to read multipart field: <multer error>` | test `multipart-error-handling.spec.ts:15-105` | `deploy.rs:147` (extractor), `deploy_form.rs:88-95` | DIVERGENT (WD-09) |
| Body not received in time | 408 | `{"error":"Request Timeout","message":"The upload did not finish within <s> s: received <n> of <m> (about <r> KiB/s). Retry on a faster connection or send smaller batches."}` | `{"error":"Request Timeout","message":"The multipart upload timed out."}` | `multipart/errors.ts:32-50`, test `multipart-limiter.spec.ts:392-426` | `deploy.rs:213-222`, `upload_limits.rs:27` | DIVERGENT (WD-10) |
| Processing deadline | 408 | `The server could not finish processing the deployment within 300 s after receiving it. Retry the deployment; if it keeps timing out, report it.` | `Deployment processing exceeded the 300000ms deadline.` | `deployment-processing.ts:21-29`, `upload-error-handler.ts:25-27` | `deploy.rs:243-260` | DIVERGENT (WD-10) |
| `entityId` missing | 400 | `A string was expected` | `Missing entityId field` (also for an empty value) | `logic/utils.ts:37-40`, test `multipart-error-handling.spec.ts:57-80,165-190` | `deploy.rs:842-845` | DIVERGENT (WD-07) |
| No auth chain | 400 | `Missing auth chain` | `No auth chain can be derived` | `extract-auth-chain.ts:19`, test `source-upload-limits.spec.ts:23-25` | `deploy_validate.rs:115-125` | DIVERGENT (WD-07) |
| Auth chain link part missing | 400 | `A string was expected` | `Missing auth chain element at index <i>` | `extract-auth-chain.ts:21-27` | `deploy_validate.rs:86-93` | DIVERGENT (WD-07) |
| More than 10 links | 413 (field cap) | `The multipart request has too many fields. The maximum allowed is 32.` | 400 `Auth chain is too long; the maximum allowed is 10 elements` | test `partial-deploy.spec.ts:640-661` | `deploy_validate.rs:81-83,126-128` | DIVERGENT (WD-04, WD-07) |
| Auth chain malformed / bad signer / bad signature | 400 | `Deployment failed: <AJV errors>` / `Deployment failed: Invalid signer: <payload>` / `Deployment failed: <@dcl/crypto message>` | `Deployment failed: The auth chain is malformed: <serde>` / `Deployment failed: Invalid signer: <payload>` / `Deployment failed: The auth chain is invalid: <error>` | `deploy-entity-handler.ts:96-101`, `validations/common.ts:60-81` | `deploy.rs:437-464` | DIVERGENT (WD-08) |
| EIP-1654 chain | 400 upstream (`validateSignature` with a null provider) | `Deployment failed: …` | accepted when the validator is configured; 503 `The auth chain could not be verified right now: …` when it fails | `common.ts:78` | `deploy.rs:452-461` | dcl-one superset (WD-08) |
| Entity file missing | 400 | `Entity file "<id>" is missing from the request.` | same; also when the uploaded entity bytes do not hash to `<id>` | `deploy-entity-handler.ts:226` | `deploy.rs:873-878` | MATCH (WD-06 for the hash case) |
| Entity file > 5 MiB / not JSON | 400 | `The entity file is too large. The maximum allowed size is 5242880 bytes.` / `The entity file is not valid JSON.` | same | `deploy-entity-handler.ts:44-50,133-142` | `deploy.rs:939-959`, `deploy_validate.rs:132-136` | MATCH |
| Entity schema | 400 | `Deployment failed: <every AJV error, comma-joined>` | `Deployment failed: <first error>` | `common.ts:28-34` | `deploy_validate.rs` `entity_schema_error` | intended (worlds-compatibility.md: first failure only) |
| Type not scene | 400 | `Deployment failed: Entity type <t> is not supported.` | same | `common.ts:166-172` | `deploy.rs:505-511` | MATCH |
| Too many content entries | 400 | `Deployment failed: The deployment has too many files. The maximum allowed is <n> but the deployment has <m>.` | same | `scene.ts:201-213` | `deploy.rs:512-519` | MATCH |
| TTL | 400 | `Deployment failed: Deployment was created <s> secs ago. Max allowed: 300 secs.` / `… <s> secs in the future. Max allowed: 900 secs.` | same | `common.ts:36-58`, tests `deploy.spec.ts:1731-1866` | `deploy.rs:281-302` | MATCH |
| No timestamp | 400 | AJV `must have required property 'timestamp'` (test matches `/timestamp/`) | `Deployment failed: must have required property 'timestamp'` | test `deploy.spec.ts:1665-1716` | `deploy_validate.rs` `entity_schema_error` | MATCH (pinned regex) |
| No `worldConfiguration.name` | 400 | `Deployment failed: scene.json needs to specify a worldConfiguration section with a valid name inside.` | same | `scene.ts:12-24`, test `deploy.spec.ts:1363-1408` | `deploy.rs:524-531` | MATCH |
| Name not `.dcl.eth` (or `.eth` with ENS off) | 400 | the permission refusal below | `Deployment failed: Only .dcl.eth world names are supported for publishing (got "<name>")` (`.dcl.eth and .eth` with `ALLOW_ENS_DOMAINS`) | `scene.ts:108-163` | `deploy.rs:532-548` | DIVERGENT (WD-13) |
| Deprecated `dclName` / `minimapVisible` / `skybox` | 400 | `Deployment failed: \`dclName\` in scene.json was renamed to \`name\`. …` etc. | same texts, same order | `scene.ts:62-84` | `deploy.rs:394-412` | MATCH |
| Parcel out of bounds | 400 | `Deployment failed: Coordinate X value 9999 is out of bounds. Must be between -150 and 150.` | same | `scene.ts:185-198`, `coordinates/component.ts:33-58`, test `deploy.spec.ts:1589-1648` | `deploy_validate.rs:56-72`, `deploy.rs:552-560` | MATCH |
| Base not in parcels / not canonical / pointers ≠ parcels | 400 | `Deployment failed: The scene base parcel [b] must be included in the scene parcels [..].` / `… Scene pointers and parcels must be unique canonical parcel coordinates.` / `… The scene pointers [..] must match the scene parcels [..].` | same | `scene.ts:30-60`, test `deploy-parcel-permission.spec.ts:252-321` | `deploy.rs:349-391` | MATCH |
| Over `MAX_PARCELS` | 400 | `Deployment failed: Max allowed scene dimensions is <n> parcels.` | same | `scene.ts:167-178` | `deploy.rs:568-573` | MATCH |
| Name on the DAO deny list | 400 | `Deployment failed: Deployment failed: World "<name>" can not be deployed because the name is in the name deny list managed by Decentraland DAO.` | same (prefix twice in both) | `scene.ts:86-101` | `deploy.rs:575-584` | MATCH |
| No permission | 400 | `Deployment failed: Deployment failed: Your wallet has no permission to publish this scene because it does not have permission to deploy under "<name>". Check scene.json to select a name that either you own or you were given permission to deploy.` | same | `scene.ts:108-163`, tests `deploy.spec.ts:1251-1303`, `deploy-parcel-permission.spec.ts:104-250` | `deploy.rs:585-607` | MATCH |
| Name authority cannot answer | 500 `{"error":"Internal Server Error"}` (the fetch throws) | | 403 `Not authorized: could not verify deployment permissions (deploy denied)` | `dcl-name-checker.ts:9-20` | `deploy.rs:600-605` | DIVERGENT (WD-14) |
| World disabled by an operator | — | | 403 with the disabled message | none | `deploy.rs:609-616` | dcl-one only |
| Grantee publish, owner unresolved | 500 (`Cannot deploy scene … owner address could not be resolved.` thrown as `Error`) | | 400 with that message; 503 when the authority faults | `entity-deployer.ts:162-169` | `deploy.rs:618-638` | DIVERGENT (WD-14) |
| Minimap / skybox / thumbnail | 400 | `Deployment failed: The file <f> is not present in the entity.` / `The texture file <f> …` / `Scene thumbnail '<t>' must be a relative path …` / `… must be a file included in the deployment.` | same | `scene.ts:320-402` | `deploy.rs:640-652`, `deploy_validate.rs` | MATCH |
| Extra / missing files | 400 | `Deployment failed: Extra file detected <name>, …` + `The file <h> (<f>) is neither present in the storage or in the provided entity` | same texts; the extra file is named by its CIDv1, not its form name | `common.ts:89-164` | `deploy.rs:654-702` | DIVERGENT (WD-06) |
| Hash ≠ form name / name not CIDv1 | 400 | `The hashed file doesn't match the provided content: <name>` / `Only CIDv1 are allowed for content files: <name>` | never produced; the file is keyed by its own hash | `common.ts:106-124` | `deploy.rs:415-432` | DIVERGENT (WD-06) |
| Too big | 400 | `Deployment failed: The deployment is too big. The maximum total size allowed is <max> bytes for scenes. You can upload up to <max> bytes but you tried to upload <size>.` | same | `scene.ts:215-271` | `quota.rs:198-202`, `deploy.rs:704-715` | MATCH |
| Owner of a DCL NAME world not found while sizing | 400 `Deployment failed: Could not determine owner for world <name>` | | 503 `Deployment failed: the storage allowance could not be determined right now` | `limits-manager/component.ts:66-71`, `scene.ts:265-267` | `deploy.rs:704-712` | DIVERGENT (WD-14) |
| A newer scene on the parcels | 400 | `Deployment failed: a newer scene is already deployed on one or more of these parcels.` before any blob is stored | same text, from the publish transaction after the blobs are stored | `entity-deployer.ts:65-79`, test `partial-deploy.spec.ts:1600-1655` | `store.rs:678-694` | MATCH on the wire (WD-15) |
| Scoped replacement raced | 409 | `{"error":"Conflict","message":"Scene replacement authorization changed while deploying to world \"<name as deployed>\". Please retry."}` | same, the world named by its lowercase key | `types.ts:518-523`, `upload-error-handler.ts:35-37` | `store.rs:763-770`, `deploy.rs:765-768` | DIVERGENT (WD-16) |
| Same entity deployed twice | 200 | idempotent: the live publication's `creationTimestamp` | 200, the row upserted; `creationTimestamp` is the request time (WD-01) | `scene-deployments/component.ts:199-218` | `store.rs:787-801` | MATCH except WD-01 |
| Client disconnect | 499 `{"error":"Client Closed Request",…}` (not seen by a client) | | the request future is dropped | `upload-error-handler.ts:20-24` | — | not observable |
| Unexpected error | 500 | `{"error":"Internal Server Error"}` | `{"error":"Internal Server Error","message":"Failed to persist deployment"}` and similar | `error-handler.js` | `deploy.rs:89-103,135-137` | DIVERGENT (body carries `message`; folded into WD-14) |

Side effects upstream publishes after the commit (best effort): SNS `WorldDeploymentEvent` with the
`isMultiplayer` attribute, `WORLD_SETTINGS_CHANGED` when the metadata refresh changed the settings,
`unblockIfUnderQuota` (`entity-deployer.ts:190-267`; tests `deploy.spec.ts:265-277,318-360,1175-1246`).
dcl-one kicks the forwarder, re-checks the quota block and updates the in-tree scene identities
(`deploy.rs:815-826`, `store.rs:815-846`); it publishes no event (WD-19).

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

dcl-one (`partial.rs:448-760`): the same steps. Batches of one entity are serialized by an
in-process lock taken before validation (intended: no `CONTENT_LOCK_CONNECTIONS`); the anchor is the
time the form finished arriving (WD-12).

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Batch incomplete | 202 | `{"missing":[<hash>…]}` | `deploy-entity-handler.ts:327`, tests `partial-deploy.spec.ts:471-567`, contract `test/contracts/partial-deployment.ts:23-30` | `partial.rs:668-677` | MATCH |
| Batch completes / single partial with all content | 200 | `{"creationTimestamp":<publication>,"message":…}` | `:318-326`, contract `:96-104` | `partial.rs:741-758` | MATCH |
| Lost completion replayed by its finalizer | 200 | the receipt's `creationTimestamp` | contract `:32-49` | `partial.rs:366-393` | MATCH |
| Batch of an entity already published | 200 | the live publication's `creationTimestamp` | `:170-175`, contract `:51-75`, test `partial-deploy.spec.ts:1020-1043` | `partial.rs:388-392` | DIVERGENT when the publication was a single-request deploy (WD-01) |
| `?partial=true` without the field | 400 | `The 'partial=true' query parameter requires the 'partial=true' form field` | `:85-87`, test `:1296-1321` | `deploy.rs:236-240` | MATCH |
| First request without the entity file / another signer resumes without it | 400 | `The first partial request for an entity must include the entity file "<id>".` | `:296-300`, tests `:805-841,1342-1364` | `partial.rs:489-504` | MATCH |
| Another account sends a batch with the entity file | 400 | `This upload was started by another account.` | `partial-upload-admission/errors.ts:6-11`, test `:1224-1260` | `partial.rs:837-840` | MATCH |
| Upload expired / reclaimed mid-batch | 400 | `This upload expired. Create a new entity with a fresh timestamp.` | `pending-scenes-repository/errors.ts`, tests `:738-762,929-964` | `partial.rs:270-272`, `store.rs:593-594` | MATCH |
| Upload gone before reserve | 400 | `Upload no longer exists; resend its manifest.` | `errors.ts:13-19` | `partial.rs:53,961` | MATCH |
| Same file twice in a batch | 400 | `Duplicate form field '<hash>'` | contract `:77-94`, test `:1262-1294` | `partial.rs:439-474` | intended (worlds-compatibility.md: keyed by hash) |
| Partial batch without permission | 400 | `Deployment failed: Deployment failed: Your wallet has no permission …` | test `:874-890` | `deploy.rs:585-599` | MATCH |
| Tampered file | 400 | `…The hashed file doesn't match the provided content: <hash>` | test `:892-908` | `Extra file detected <actual hash>` | DIVERGENT (WD-06) |
| Auth chain refusal | 400 | `Invalid auth chain.` / `Invalid auth chain: <message>` | `:90-94` | `Deployment failed: The auth chain is …` | DIVERGENT (WD-08) |
| Scene over the cap while staging | 400 | `Deployment failed: The deployment is too big.` | `scene-deployments/component.ts:321` | `partial.rs:651-653` | MATCH |
| Newer scene holds the parcels (new upload / stale finalize) | 400 | `Deployment failed: a newer scene is already deployed on one or more of these parcels.` | `:272-281`, tests `:1448-1503` | `partial.rs:541-549`, `store.rs:692` | MATCH |
| Account / world / per-account-in-world upload caps | 429 + `Retry-After` | `{"error":"Too Many Requests","message":"Too many partial uploads in progress for this account: <n> of the <max> allowed. …"}` and the world variants | `partial-upload-admission/component.ts:85-118`, tests `:1657-1735` | `partial.rs:858-909` | MATCH |
| Byte rate, account, world, server budgets | 400 (alone too large) / 429 + `Retry-After` | upstream texts (`formatBytes`) | `component.ts:146-221`, `partial-upload-accounting.spec.ts:326-1470` | `partial.rs:960-1160` | MATCH; the per-source budget is dcl-one only (intended) |
| Personal world not yet published here | — | `Partial uploads to a personal world start once it is published here; …` | none | `partial.rs:526-541` | intended (worlds-compatibility.md) |
| A stored file collected before completion | 400 upstream would fail the publish | | 202 with it in `missing` again | — | `partial.rs:719-731` | intended (worlds-compatibility.md) |
| Field cap, field size | 413 | as section 1 | tests `:640-684` | as section 1 | DIVERGENT (WD-04) |

---

## 3. DELETE /entities/:world_name

### Request

Upstream (`routes.ts:187`, `handlers/undeploy-entity-handler.ts:6-38`): strict signed fetch; the owner
(`namePermissionChecker.checkPermission`) or a world-wide deployment grantee
(`permissions.hasWorldWidePermission`); `worlds.undeployWorld` soft-deletes every scene, clears the
spawn and the stats, publishes `WorldUndeploymentEvent` and re-checks the owner's block
(`logic/worlds/component.ts:179-197`, `worlds-manager.ts:864-890`).

dcl-one (`scenes.rs:105-143`): world name parsed first (400 `Invalid world name: …`), the strict signed
fetch, `Policy::is_owner`, else `has_world_wide_permission`, `store::undeploy_world` (rows retired,
spawn recomputed, `store.rs:950-972,1089-1098`), quota re-check off the request.

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Undeployed | 200 | upstream: the string `{}` (`content-type: text/plain; charset=utf-8`); dcl-one: JSON `{}` (`application/json`) | `undeploy-entity-handler.ts:34-37`, `@dcl/http-server` `logic.js:298-312`, test `undeploy-entity-handler.spec.ts:57-97` | `scenes.rs:142` | DIVERGENT (WD-17) |
| Neither owner nor world-wide grantee | 400 | `{"error":"Bad request","message":"Invalid request. You must have world-wide deployment permission to undeploy the entire world."}` | `:16-29`, test `:20-37` | `scenes.rs:121-137` | MATCH |
| Scene signer / unsigned / bad signature | 400 / 401 | upstream `{"error":<verify message>,"message":"This endpoint requires a signed fetch request. See ADR-44."}`; dcl-one `{"error":"Bad request"|"Not Authorized","message":"Invalid Auth Chain: …"|"Unauthenticated: …"}` | test `:39-55` | `scenes.rs:113-120`, `shared/src/signed_fetch.rs:76-87` | DIVERGENT (WD-02) |
| Malformed world name | 400 | dcl-one `Invalid world name: …` before the signature | none | `scenes.rs:112` | DIVERGENT (WD-18) |
| Authority cannot answer | 500 upstream; dcl-one the `AuthorityNotEstablished` status (503 for a store fault) | | `dcl-name-checker.ts` | `scenes.rs:131` | DIVERGENT (WD-14) |

---

## 4. DELETE /world/:world_name/scenes/:coordinate

### Request

Upstream (`routes.ts:198`, `scenes-handler.ts:87-133`): strict signed fetch; the coordinate must match
`^-?\d+,-?\d+$` (no bounds check); the owner removes every scene on the parcel; anyone else needs a
deployment grant over the whole footprint of every scene on it (else 401), and only those scene
identities are removed; `undeployWorldScenes` recomputes the spawn and publishes
`WorldScenesUndeploymentEvent` (`logic/worlds/component.ts:210-244`, `worlds-manager.ts:1052-1120`).

dcl-one (`scenes.rs:46-102`): world name parsed first, the strict signed fetch, `Parcel::parse_strict`
(the same pattern, values within `i32`, `comms/src/world_identity.rs:333-360`), `Policy::is_owner`, else
`has_permission_covering` over the parcel plus the footprints of the overlapping scenes, then
`store::undeploy_scene` scoped to those identities (`store.rs:1056-1087`). Runs under the 30 s
`request_timeout` layer (`mod.rs:706-719`).

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Undeployed, or nothing on the parcel | 200 | `{"message":"Scene at parcel <coordinate as sent> undeployed successfully"}` | `:127-132`, tests `scenes-handler.spec.ts:555-940` | `scenes.rs:97-101` | MATCH |
| Non-canonical alias (`020,024`) | 200 | removes the scene stored at `20,24`; the message echoes `020,024` | test `:622-642` | `Parcel::parse_strict` then `canonical()` (`store.rs:1063`) | MATCH |
| Grant over part of a multi-parcel scene / no grant | 401 | `{"error":"Not Authorized","message":"Unauthorized. You do not have permission to undeploy scenes in this world."}` | `:103-125`, tests `:686-800` | `scenes.rs:68-95` | MATCH |
| Bad coordinate | 400 | `{"error":"Bad request","message":"Invalid coordinate format: abc,def. Expected format: x,y (e.g., 0,0 or -1,2)"}` | `:9-16`, test `:803-830` | `scenes.rs:63-67` | MATCH; a coordinate outside `i32` is 400 here and 200 upstream (WD-18) |
| Unsigned / scene signer | 400 | upstream `{"error":"Invalid Auth Chain","message":"This endpoint requires a signed fetch request. See ADR-44."}` | test `:833-856` | `{"error":"Bad request","message":"Invalid Auth Chain"}` | DIVERGENT (WD-02) |
| Spawn on the removed scene / elsewhere / last scene | 200 | spawn moved to the remaining bounding rectangle's centre / kept / cleared | tests `:943-1075` | `store.rs:974-1020` | MATCH |
| Slow request | 408 | dcl-one `request timed out` after 30 s | none | `mod.rs:710-719` | DIVERGENT (WD-18) |

---

## Backlog (WD-)

1. **WD-01 · `POST /entities` · a single-request deploy answers the request time as `creationTimestamp`.**
   - Upstream: `{ creationTimestamp: Date.now(), ...result }`, where `result.creationTimestamp` is the
     time `deployScene` wrote into `world_scenes.created_at` under the world lock
     (`decentraland/worlds-content-server:src/logic/scene-deployments/component.ts:219`,
     `src/adapters/worlds-manager.ts:558-572,731,764`). A later partial batch of the same entity
     answers that same value (`test/integration/partial-deploy.spec.ts:1020-1043`,
     `test/contracts/partial-deployment.ts:51-75`).
   - dcl-one: `creation_timestamp: now_ms`, taken before validation
     (`services/catalyst/worlds/src/core/deploy.rs:852,921-935`); `publish` returns the row's
     `created_at_ms` (`deploy.rs:732-830`, `store.rs:787-801,887-895`) and the value is dropped. The
     replay of a later batch answers the row's time (`partial.rs:388-420`), so the two answers differ.
   - Proposed fix: in `deploy_entity_inner`, keep `Ok(created)` from `publish` and answer it;
     add the regular-then-partial case to `worlds/src/core/tests/partial_uploads.rs`.
   - Severity: P1 (the partial-deployment contract's "already published" case fails after a regular deploy).

2. **WD-02 · `DELETE /entities/:world_name`, `DELETE /world/:world_name/scenes/:coordinate` · signed-fetch refusal body.**
   - Upstream: `{"error":<verify message>,"message":"This endpoint requires a signed fetch request. See ADR-44."}`
     with the middleware's status (`src/controllers/routes.ts:72-98`; `@dcl/crypto-middleware` 6.3.0
     `dist/index.js` `errorToResponse`); tests pin `message`
     (`test/integration/undeploy-entity-handler.spec.ts:39-55`, `scenes-handler.spec.ts:833-856`).
   - dcl-one: `map_auth_error_api` puts the verify message in `message` and the status label in `error`
     (`services/catalyst/comms/src/signed_fetch.rs:187-190`, `shared/src/signed_fetch.rs:76-87`,
     `comms/src/worlds_envelope.rs:133-140`). The storage routes already answer upstream's shape
     (`worlds/src/storage/errors.rs:14`).
   - Proposed fix: have `map_auth_error_api` answer `{error: <message>, message: SIGNED_FETCH_MESSAGE}`
     (move the constant to `comms::worlds_envelope`), for every worlds signed-fetch route.
   - Severity: P1 (builder and CLI show `message`). Depends on: the permissions/settings areas, which share the mapper.

3. **WD-03 · all routes of this area · the worlds writes switch is off by default.**
   - Upstream: no switch.
   - dcl-one: `WORLDS_WRITES_ENABLED` defaults to `false`; while off every route here answers
     503 `{"ok":false,"error":"Service Unavailable","message":…}` with `Retry-After: 3600`
     (`services/catalyst/worlds/src/writes.rs:33-76`, `config/src/worlds.rs:86`). It is not listed in
     `docs/worlds-compatibility.md`.
   - Proposed fix: keep (DATABASE.md requires a demonstrated restore before local authority is
     accepted); add the row to `docs/worlds-compatibility.md` so an operator replacing upstream knows to
     set it.
   - Severity: P1 (a node with the default answers 503 to every publish).

4. **WD-04 · `POST /entities` · multipart field, part and file caps.**
   - Upstream: `DEPLOYMENT_FIELD_LIMITS`: 32 fields, 32 KiB per field, 10,032 parts, 10,000 files, with
     `The multipart request has too many fields. The maximum allowed is 32.` and
     `Field '<name>' is too large. The maximum allowed size per field is 32768 bytes.`
     (`src/controllers/multipart/limits.ts:5-25`, `middleware.ts:307-339`; tests
     `multipart-error-handling.spec.ts:107-163`, `partial-deploy.spec.ts:621-684`).
   - dcl-one: 100 fields, 1 MiB per field, 10,100 parts, a 10,002nd file; messages without the limit
     or field name (`services/catalyst/worlds/src/core/upload_limits.rs:28-37`,
     `deploy_form.rs:99-110,163-181`). An 11-link chain answers 400 `Auth chain is too long…`
     (`deploy_validate.rs:81-83,126-128`) instead of the 413 field cap.
   - Proposed fix: deploy-route constants `MAX_DEPLOYMENT_FIELDS = 32`,
     `MAX_DEPLOYMENT_FIELD_SIZE_BYTES = 32 * 1024`, parts `10_000 + 32`, files `>= 10_000` before the push,
     and upstream's two messages in `deploy_form.rs`; the settings route keeps 100 fields / 1 MiB.
   - Severity: P2.

5. **WD-05 · `POST /entities` (single request) · repeated form names are accepted.**
   - Upstream: any repeated part name answers 400 `Duplicate form field '<name>'`
     (`src/controllers/multipart/middleware.ts:234-243`).
   - dcl-one: fields are a `BTreeMap` (last value wins, `deploy_form.rs:197`); a repeated file is kept
     once (`deploy.rs:415-432`). The partial path refuses a repeated hash (`partial.rs:439-474`, intended).
   - Proposed fix: track part names in `read_deploy_form` and answer upstream's 400 on a repeat.
   - Severity: P2.

6. **WD-06 · `POST /entities` · files keyed by their computed hash, not their form name.**
   - Upstream: `files` is keyed by the part name; each name must be referenced, be a CIDv1 and equal
     the hash of the bytes; the entity file must hash to `entityId`
     (`src/logic/validations/common.ts:11-23,89-124`; test `partial-deploy.spec.ts:892-908`).
   - dcl-one: the part name is dropped and the CIDv1 of the bytes is the key
     (`services/catalyst/worlds/src/core/upload_spool.rs:28-34,126-133`, `deploy_form.rs:161`). A tampered
     file answers `Extra file detected <actual hash>`, a wrong entity file `Entity file "<id>" is missing
     from the request.`, and the CIDv1 check never runs.
   - Proposed fix: keep the part name on `SpooledFile`, key `Uploads` by it, and add upstream's three
     checks (`Invalid entity hash: expected <h> but got <id>`, `Only CIDv1 are allowed for content files: <n>`,
     `The hashed file doesn't match the provided content: <n>`) to `deploy.rs` `validate`, in upstream's
     order (entity id before the signature; the files after the size inputs).
   - Severity: P2.

7. **WD-07 · `POST /entities` · form and auth-chain extraction messages.**
   - Upstream: `requireString` answers `A string was expected` for a missing `entityId` or link part;
     no indexed field answers `Missing auth chain`; only the indexed form is read
     (`src/logic/extract-auth-chain.ts:6-30`, `src/logic/utils.ts:37-40`; tests
     `multipart-error-handling.spec.ts:57-80,165-190`, `source-upload-limits.spec.ts:23-25`).
   - dcl-one: `Missing entityId field` (an empty value too), `No auth chain can be derived`,
     `Missing auth chain element at index <i>`, and a JSON `authChain` field is accepted
     (`services/catalyst/worlds/src/core/deploy.rs:842-845`, `deploy_validate.rs:86-133`).
   - Proposed fix: upstream's three texts in `extract_auth_chain_from_fields` and `deploy_entity_inner`;
     accept `entityId=""` as upstream does (it then fails validation); keep or drop the JSON form
     (a superset; document it if kept).
   - Severity: P2.

8. **WD-08 · `POST /entities` · auth-chain verification messages and EIP-1654.**
   - Upstream: single request `Deployment failed: <AJV errors>` / `Invalid signer: <p>` / the
     `@dcl/crypto` message; partial `Invalid auth chain.` / `Invalid auth chain: <message>`; contract
     wallets are refused (null provider) (`src/controllers/handlers/deploy-entity-handler.ts:80-102`,
     `src/logic/validations/common.ts:60-81`).
   - dcl-one: `Deployment failed: The auth chain is malformed: <serde>` / `… is invalid: <error>` on both
     paths; EIP-1654 accepted with a validator, 503 when it cannot answer
     (`services/catalyst/worlds/src/core/deploy.rs:437-464`).
   - Proposed fix: in `authenticate`, take a `partial` flag and answer upstream's texts; map
     `crate::auth::AuthError` variants to `@dcl/crypto`'s messages (needs that package's texts, not
     vendored). Keep EIP-1654 and document it.
   - Severity: P2.

9. **WD-09 · `POST /entities` · non-multipart and truncated bodies.**
   - Upstream: JSON 400 `Unsupported content type: application/json`, `Unexpected end of form`
     (`test/integration/multipart-error-handling.spec.ts:15-105`).
   - dcl-one: the `Multipart` extractor rejects a bad content type before the handler (axum's plain-text
     400); a truncated body answers `Failed to read multipart field: <multer error>`
     (`services/catalyst/worlds/src/core/deploy.rs:143-148`, `deploy_form.rs:88-95`).
   - Proposed fix: take `Request` and build `Multipart` in the handler after the Content-Length and
     source checks, mapping a missing or non-multipart type to `Unsupported content type: <type>` and an
     incomplete stream to `Unexpected end of form`.
   - Severity: P2.

10. **WD-10 · `POST /entities` · upload and processing timeout messages.**
    - Upstream: `describeUploadTimeout` (seconds, bytes received of declared, KiB/s) and
      `The server could not finish processing the deployment within <s> s after receiving it. Retry the deployment; if it keeps timing out, report it.`
      (`src/controllers/multipart/errors.ts:32-50`, `src/logic/deployment-processing.ts:21-29`; test
      `multipart-limiter.spec.ts:392-426`).
    - dcl-one: `The multipart upload timed out.` and `Deployment processing exceeded the <ms>ms deadline.`
      (`services/catalyst/worlds/src/core/upload_limits.rs:27`, `deploy.rs:213-222,243-260`).
    - Proposed fix: count wire bytes in `read_deploy_form` and format both messages as upstream.
    - Severity: P2.

11. **WD-11 · `POST /entities` · in-flight budget answers.**
    - Upstream: one byte budget bounds bytes, requests (16 MiB minimum each) and files (16 KiB each);
      every shed says `Server is buffering too many uploads, please retry shortly.` or
      `…too many upload files…`; a failed temp write `The server could not store the multipart upload, please retry shortly.`
      (`src/adapters/in-flight-upload-budget/component.ts:63-140`, `multipart/errors.ts:25-77`).
    - dcl-one: separate caps (`MAX_CONCURRENT_UPLOADS` 40, lowered to three quarters of the pool;
      40,000 files), the slot cap answering `Server is handling too many concurrent uploads, please retry shortly.`,
      the spool failure `Server could not buffer the upload, please retry shortly.`
      (`services/catalyst/worlds/src/core/upload_limits.rs:14-24`, `deploy.rs:173-181`, `deploy_form.rs:21-27`).
    - Proposed fix: keep the caps (documented pool bound); answer upstream's two texts.
    - Severity: P2.

12. **WD-12 · `POST /entities?partial=true` · the first batch is admitted when its body has arrived.**
    - Upstream: `stampRequestArrival` before parsing; the upload's `created_at` and freshness anchor
      are the arrival (`src/controllers/request-arrival.ts`, `scene-deployments/component.ts:251,265`;
      test `partial-deploy.spec.ts:569-604`).
    - dcl-one: `now_ms` after the form is read (`services/catalyst/worlds/src/core/deploy.rs:852`,
      `partial.rs:508`); the gateway also buffers the body before the app.
    - Proposed fix: take the time at `deploy_entity` entry and pass it down; the gateway buffering
      remains (document).
    - Severity: P2.

13. **WD-13 · `POST /entities` · world name family and personal worlds.**
    - Upstream: any name; one that nobody owns fails the permission check with its message
      (`src/logic/validations/scene.ts:108-163`, `src/adapters/name-ownership.ts:46-60,98`).
    - dcl-one: `Only .dcl.eth world names are supported for publishing (got "<n>")` (or `.dcl.eth and .eth`
      with `ALLOW_ENS_DOMAINS`) before the deprecated-config check; a `<address>.dcl.eth` personal world
      is deployable by its address (`services/catalyst/worlds/src/core/deploy.rs:532-548,585-599`).
    - Proposed fix: keep (realm policy); list both in `docs/worlds-compatibility.md`.
    - Severity: P2.

14. **WD-14 · all routes · authority and internal faults.**
    - Upstream: a failing name lookup or unresolved owner throws, answering 500
      `{"error":"Internal Server Error"}`; a DCL NAME world without an owner while sizing answers
      400 `Deployment failed: Could not determine owner for world <n>`
      (`src/adapters/entity-deployer.ts:162-169`, `limits-manager/component.ts:66-71`).
    - dcl-one: 403 `Not authorized: could not verify deployment permissions (deploy denied)`,
      400 `Cannot deploy scene … owner address could not be resolved.`, 503 for owner/allowance faults,
      and 500 bodies that carry a `message` (`services/catalyst/worlds/src/core/deploy.rs:600-638,704-712,135-137`).
    - Proposed fix: keep fail-closed; document the 403/503 in `docs/worlds-compatibility.md`; drop
      `message` from deploy 500s (`deploy.rs` `rejection` for 500) to match the envelope rule.
    - Severity: P2.

15. **WD-15 · `POST /entities` · a newer-scene refusal stores the blobs first.**
    - Upstream: the fast-fail runs before any storage write (`src/adapters/entity-deployer.ts:65-79`;
      test `partial-deploy.spec.ts:1600-1655` checks nothing was stored).
    - dcl-one: the same 400, from inside the publish transaction after `store_file`
      (`services/catalyst/worlds/src/core/deploy.rs:900-905`, `store.rs:678-694`); the blobs wait for the collector.
    - Proposed fix: call `store::newer_scene_deployed` before the store loop, as `partial.rs:541-549` does;
      keep the transactional check.
    - Severity: P2 (no wire difference).

16. **WD-16 · `POST /entities` · the 409 names the world in lowercase.**
    - Upstream: `SceneReplacementConflictError(worldName)` with the name as deployed (`src/types.ts:518-523`).
    - dcl-one: `world.key()` (`services/catalyst/worlds/src/core/store.rs:763-770`).
    - Proposed fix: `world.display()`.
    - Severity: P2.

17. **WD-17 · `DELETE /entities/:world_name` · 200 content type.**
    - Upstream: `body: '{}'`, a string, so `text/plain; charset=utf-8`
      (`src/controllers/handlers/undeploy-entity-handler.ts:34-37`; `@dcl/http-server` 2.3.0 `dist/logic.js:298-312`).
    - dcl-one: `Json(json!({}))`, `application/json` (`services/catalyst/worlds/src/core/scenes.rs:142`).
    - Proposed fix: answer `([(CONTENT_TYPE, "text/plain; charset=utf-8")], "{}")`.
    - Severity: P2.

18. **WD-18 · DELETE routes · checks upstream does not have.**
    - Upstream: the signature first, no world name check, the coordinate pattern only, no request timeout
      (`src/controllers/handlers/scenes-handler.ts:9-16,87-97`).
    - dcl-one: `parse_world` answers 400 `Invalid world name: …` before the signature
      (`services/catalyst/worlds/src/core/scenes.rs:53,112`); a coordinate outside `i32` is 400
      (`comms/src/world_identity.rs:333-360`); the scene route answers 408 `request timed out` after 30 s
      (`mod.rs:706-719`). `docs/worlds-compatibility.md` lists the world-name check for the permission,
      settings and manifest routes only.
    - Proposed fix: extend the compatibility row to these two routes; keep the rest.
    - Severity: P2.

19. **WD-19 · all routes · no deployment or undeployment events.**
    - Upstream: SNS `WorldDeploymentEvent` (with `isMultiplayer`), `WORLD_SETTINGS_CHANGED` after a deploy
      that changed the settings, `WorldUndeploymentEvent`, `WorldScenesUndeploymentEvent`
      (`src/adapters/entity-deployer.ts:196-248`, `src/logic/worlds/component.ts:179-244`).
    - dcl-one: in-tree consumers only (scene identities, the forward outbox, the quota re-check)
      (`services/catalyst/worlds/src/core/deploy.rs:815-826`, `store.rs:815-846,950-972`). The compatibility
      page covers notices, not these events.
    - Proposed fix: keep; add a row to `docs/worlds-compatibility.md` naming what replaces each event.
    - Severity: P2. Depends on: the places and asset-bundle areas (the events' upstream consumers).

20. **WD-20 · `POST /entities` · gateway limits answer before the app.**
    - Upstream: no request rate limit on this route; its own JSON 413/429.
    - dcl-one: nginx `limit_req zone=deploy` (5 r/s, burst 4), 10 connections per address and
      `client_max_body_size 360m`, answered with nginx's HTML 413/429, the body buffered before the app
      (`infra/gateway/gateway.py:13,22,169-191,332-335`).
    - Proposed fix: JSON error pages for those locations in `gateway.py`
      (`{"error":"Too Many Requests",…}` with `Retry-After`; `{"error":"Payload Too Large","message":"The multipart request is too large."}`).
    - Severity: P2. Depends on: the gateway (infra).

21. **WD-21 · `POST /entities` · validation texts of the scene checks.** Status: MATCH, no work.
    - Upstream: `src/logic/validations/{common,scene}.ts` (TTL, file count, type, `worldConfiguration`,
      deprecated keys, bounds, pointers, parcels cap, deny list, permission, minimap, skybox, thumbnail,
      extra/missing files, size).
    - dcl-one: the same texts and order (`services/catalyst/worlds/src/core/deploy.rs:281-412,492-724`,
      `deploy_validate.rs`, `quota.rs:198-202`); AJV schema texts first-failure only (intended).
    - Severity: none (closed).

22. **WD-22 · `POST /entities` · success message.** Status: MATCH, no work.
    - Upstream: `buildSceneDeploymentMessage` (`src/logic/utils.ts:48-56`; test `deploy.spec.ts:203-213`).
    - dcl-one: `deployment_message` with `encodeURIComponent` (`deploy.rs:304-326`).
    - Severity: none (closed).

23. **WD-23 · `POST /entities` · Content-Length, payload and file size limits, entity file.** Status: MATCH, no work.
    - Upstream: `src/controllers/multipart/middleware.ts:157-167,268-279,382-384`,
      `deploy-entity-handler.ts:27,44-50,133-142`.
    - dcl-one: `deploy.rs:51-58,145-161,939-959`, `deploy_form.rs:144-149`, `deploy_validate.rs:13,132-136`.
    - Severity: none (closed).

24. **WD-24 · `POST /entities` · per-source admission.** Status: MATCH, no work.
    - Upstream: `src/controllers/source-upload-admission.ts:15-50` (`cf-connecting-ip`; unattributed not limited).
    - dcl-one: `upload_limits.rs:270-392` (`X-Real-IP`, IPv6 by /64, intended); 429, `Retry-After: 5`,
      upstream's message.
    - Severity: none (closed).

25. **WD-25 · `POST /entities?partial=true` · the partial contract.** Status: MATCH, no work.
    - Upstream: `test/contracts/partial-deployment.ts`, `partial-deploy.spec.ts` (202 `{missing}`, completion,
      replay, expiry, ownership, query/field agreement, manifest required, quotas with `Retry-After`).
    - dcl-one: `services/catalyst/worlds/src/core/partial.rs`, tests in `worlds/src/core/tests/partial_uploads.rs`;
      the intended differences are in `docs/worlds-compatibility.md` ("Partial uploads"). WD-01, WD-06,
      WD-08 and WD-12 stay open.
    - Severity: none (closed).

26. **WD-26 · `DELETE /world/:world_name/scenes/:coordinate` · authorization scope and spawn.** Status: MATCH, no work.
    - Upstream: owner unrestricted; grantee over the full footprint, scoped to the identities it saw;
      spawn kept, recentred or cleared (`src/controllers/handlers/scenes-handler.ts:99-127`,
      `worlds-manager.ts:1052-1120`; tests `scenes-handler.spec.ts:555-1075`).
    - dcl-one: `scenes.rs:68-95`, `store.rs:974-1020,1056-1087`.
    - Severity: none (closed).

27. **WD-27 · `DELETE /entities/:world_name` · authorization.** Status: MATCH, no work.
    - Upstream: owner or world-wide deployment grantee, else 400 with the fixed text
      (`undeploy-entity-handler.ts:16-29`).
    - dcl-one: `scenes.rs:121-137`.
    - Severity: none (closed).

## Endpoints dcl-one serves in this area that upstream does not

None at a new path. Behaviour dcl-one adds on these routes:

- `POST /entities` accepts a JSON `authChain` field beside the indexed fields (WD-07), EIP-1654 chains
  (WD-08), personal worlds `<address>.dcl.eth` (WD-13), answers 403 for an operator-disabled world and
  503 when stored content changed under the publish (`store.rs:627-629`), and charges a per-source
  staging budget on partial uploads (`MAX_PENDING_BYTES_PER_SOURCE`, documented).
- `/admin/gc` (root and prefix) and `/gc` (prefix) reclaim expired partial uploads with the collector
  (`mod.rs:588-596`); they belong to the operator area.

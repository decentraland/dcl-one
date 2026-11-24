# Content server metrics

`catalyst serve` exports the content server's series on its process scrape
([shared service metrics ADR](../../../../docs/adr/2026-11-24-shared-service-metrics.md)). They keep
the names, types, label names, label values and histogram buckets of the content server's
`src/metrics.ts` at `decentraland/catalyst@6c56f8a3eaff631cb7be0dc01ef92a24333bbc93`,
except where a family serve already declares measures the same thing ("replaced by" below).

The families are declared in `content/src/series.rs` (`catalyst_content::series::FAMILIES`)
and `storage/src/gc.rs` (in `catalyst_storage::FAMILIES`), and both lists are composed into
the serve registry in `src/serve_lifecycle.rs`, which applies the declared buckets. A
histogram declared without buckets uses the Prometheus client's default buckets (0.005 to
10 s). A family is absent from the scrape until its first observation.

Every label value comes from a closed set. An entity type a client or a peer names is one
of `scene`, `profile`, `wearable`, `store`, `emote`, `outfits`, else `unknown`.

Paths below are relative to `services/catalyst`.

## Deployments

| Legacy series | Type and labels | Status | Observed at | Notes |
|---|---|---|---|---|
| `dcl_deployments_endpoint_counter` | counter `kind` = `success`, `validation_error`, `error` | emitted | `content/src/create_entity.rs` `deploy_regular`, `deploy_form` | A regular `POST /entities` deploy: `success` for a stored or replayed entity, `validation_error` for a 400 refusal, `error` for a 503 (store, database, access check, processing timeout). Requests turned away before the form is read (limits, budgets, bootstrapping) are not counted. |
| `total_deployments_count` | counter `entity_type`, `deployment_context` = `LOCAL`, `SYNCED`, `FIX_ATTEMPT` | emitted | `content/src/write_deployer.rs` `deploy_entity_unlocked`; `content/src/sync_deployer.rs` `flush_batch` | Entities newly stored: a local deploy (partial publications included) once its row is inserted, a synced or retried entity once the batch flush that inserted it commits. Replays are not counted. |
| `dcl_deployment_time` | histogram `entity_type`, `failed` | covered in `sync.md` | | Observed by the sync code. |
| `dcl_pending_deployment_gauge` | gauge `entity_type` | covered in `sync.md` | | Observed by the sync code. |
| `dcl_ignored_sync_deployments` | counter | covered in `sync.md` | | Part of snapshot and pointer-change syncing. |
| `dcl_pending_download_gauge` | gauge `entity_type` | covered in `sync.md` | | Part of snapshot and pointer-change syncing. |

## Partial uploads

| Legacy series | Type and labels | Status | Observed at | Notes |
|---|---|---|---|---|
| `dcl_partial_deployments_staging_total` | counter `kind` = `accepted`, `finalized`, `replayed`, `validation_error`, `throttled`, `busy`, `error` | emitted | `content/src/partial_deployments.rs` `handle`, `admit_before_files`; `content/src/create_entity.rs` `deploy_form` | Every answered batch. `busy` is a held entity lock or a full upload budget; `error` a 503 or 500, a spooled part that could not be read back included. A batch turned away before its files on the daily quota or the byte rate counts `throttled`. |
| `dcl_partial_upload_throttled_total` | counter `reason` = `uploads_per_account`, `uploads_per_pointer`, `uploads_per_pointer_account`, `bytes_per_account`, `bytes_per_server`, `bytes_per_minute`, `entity_rate_limit`, `pointer_conflict` | emitted | `content/src/partial_deployments.rs` `Refusal::record` | Each 429 of a quota or limit. The daily deploy quota's 429 has no reason and counts only in the staging counter. |
| `dcl_partial_uploads_started_total` | counter | emitted | `content/src/partial_deployments.rs` `commit` | A first batch admitted. |
| `dcl_partial_uploads_completed_total` | counter | emitted | `content/src/partial_deployments.rs` `finalize` | An upload published by its final batch. |
| `dcl_partial_upload_duration_seconds` | histogram; 5 to 3600 s | emitted | `content/src/partial_deployments.rs` `finalize` | From the first batch's arrival to publication. |
| `dcl_partial_upload_batches_per_upload` | histogram; 1 to 250 | emitted | `content/src/partial_deployments.rs` `finalize` | Stored batches (`pending_deployments.batches`) at publication. |
| `dcl_partial_upload_stage_duration_seconds` | histogram `stage` = `hash`, `validate`, `lock_wait`, `inventory`, `accounting`, `store`, `progress`, `finalize_verify`, `finalize_deploy`; 0.005 to 120 s | emitted | `content/src/partial_deployments.rs` `prepare`, `stage`, `commit` | Each stage of a batch, whether it succeeded or not. |
| `dcl_partial_upload_metadata_checks_total` | counter | emitted | `content/src/partial_deployments.rs` `commit` | Content hashes looked up in the store: the first batch's inventory and the completing batch's check. |
| `dcl_partial_upload_batches_total` | counter `outcome` = `incomplete`, `finalizing` | emitted | `content/src/partial_deployments.rs` `commit` | A stored batch, by whether content is still missing. |
| `dcl_partial_upload_capacity_bytes` | gauge | emitted | `content/src/lib.rs` `ContentState::with_sync` | `MAX_PENDING_BYTES`. |
| `dcl_partial_upload_reserved_bytes` | gauge | emitted | `content/src/partial_deployments.rs` `reserve`, `record_totals` | Bytes reserved across all uploads, after an accepted reservation and at the end of each cleanup run. |
| `dcl_partial_upload_cleanup_backlog_bytes` | gauge | emitted | `content/src/partial_deployments.rs` `record_totals` | Bytes of expired uploads, at the end of each cleanup run. |
| `dcl_partial_uploads_pending` | gauge `state` = `live`, `expired` | emitted | `content/src/partial_deployments.rs` `record_totals` | Uploads in `content.pending_deployments` at the end of each cleanup run. |
| `dcl_pending_deployments_expired_total` | counter | emitted | `content/src/partial_deployments.rs` `cleanup_expired` | Expired uploads a cleanup run reclaimed. |
| `dcl_partial_upload_cleanup_runs_total` | counter `outcome` = `success`, `deferred`, `error` | emitted | `content/src/partial_deployments.rs` `cleanup_expired` | `deferred`: the exclusive store lock stayed busy. A run with a failed upload is `error`. |
| `dcl_partial_upload_cleanup_failures_total` | counter `reason` = `storage`, `database` | emitted | `content/src/partial_deployments.rs` `reclaim_expired` | Uploads a run could not reclaim: a blob that could not be unlinked, or a database failure that ended the run (every upload of that batch). |
| `dcl_partial_upload_cleanup_duration_seconds` | histogram; 0.1 to 300 s | replaced by `dcl_job_duration_seconds{worker="content-partial-uploads-cleanup"}` | `jobs/src/instrumentation.rs` | The cleanup job's pass duration, with the job buckets (0.01 to 1800 s). |
| `dcl_partial_upload_cleanup_last_success_timestamp_seconds` | gauge | emitted | `content/src/partial_deployments.rs` `cleanup_expired` | Unix seconds of the last run that reclaimed every expired upload it listed, in this process. The job is leased, so one replica sets it. A deferred run counts as a success in `dcl_job_last_success_timestamp_seconds` but not here. |

## Upload budgets and spool

| Legacy series | Type and labels | Status | Observed at | Notes |
|---|---|---|---|---|
| `dcl_upload_budget_capacity_bytes` | gauge `budget` = `memory`, `disk` | emitted | `content/src/upload_admission.rs` `UploadBudget::new` | `MAX_IN_MEMORY_DEPLOYMENT_BYTES` and `MAX_IN_FLIGHT_UPLOAD_BYTES`, as raised to fit one maximum request. |
| `dcl_upload_budget_reserved_bytes` | gauge `budget` = `memory`, `disk` | emitted | `content/src/upload_admission.rs` `UploadBudget`, `Reservation` | Set on every admission, growth and release. |
| `dcl_upload_budget_active` | gauge `budget` = `memory`, `disk` | emitted | `content/src/upload_admission.rs` `UploadBudget`, `Reservation` | Requests holding a share. |
| `dcl_upload_budget_rejections_total` | counter `budget`, `reason`: `memory`/`disk` with `bytes`, `source` with `source_bytes`, `source_concurrency` | emitted | `content/src/upload_admission.rs` `try_admit`, `Reservation::grow`, `SourceUploads::try_acquire`, `SourceLease::grow` | |
| `dcl_upload_spool_failures_total` | counter `reason` | replaced by `upload_spool_failures_total{route="content-entities"}` and `multipart_upload_cleanup_failures{route="content-entities"}` | `content/src/create_entity.rs` `spool_unavailable`; `storage/src/upload_spool.rs` `UploadSpool` drop | The first counts every spool failure that answers 503 (directory, part write or flush, read-back), without the reason; the second counts directories that could not be removed. |
| `dcl_upload_spool_reclaimed_folders_total` | counter | emitted | `src/serve.rs` `serve` | Spool directories of exited processes removed at startup. |
| `dcl_multipart_upload_timeouts_total` | counter | emitted | `content/src/create_entity.rs` `deploy_form` | Bodies past `MULTIPART_UPLOAD_TIMEOUT_MS` (408). |
| `dcl_multipart_upload_aborts_total` | counter `reason` = `client_closed` | emitted | `content/src/create_entity.rs` `counting_body`; `content/src/series.rs` `ReceivingBody` | The client closed or broke the connection while its body was received: the body stream failed, or the request was dropped mid-read. Counted once per request. |

## Rate limiter, locks, chain reads

| Legacy series | Type and labels | Status | Observed at | Notes |
|---|---|---|---|---|
| `dcl_content_rate_limited_deployments_total` | counter `entity_type`, `reason` = `ttl`, `max_size`, `unchanged_ttl` | emitted | `content/src/write_deployer.rs` `deploy_entity_unlocked`; `content/src/partial_deployments.rs` `prepare` | Each refusal by the per-pointer deploy limit, with the window that refused it. |
| `dcl_content_rate_limiter_cache_keys` | gauge `entity_type`, `cache_type` = `deployment`, `unchanged` | emitted | `content/src/write_deployer.rs` `DeployRateLimiter::reserve`, `Reservation::release` | Pointers a window holds after a reservation or its release. Only profiles have an unchanged-content window. |
| `dcl_content_rate_limiter_cache_max_size` | gauge `entity_type` | emitted | `content/src/write_deployer.rs` `DeployRateLimiter::buckets` | `DEPLOYMENT_RATE_LIMIT_MAX_<TYPE>` or its default. |
| `dcl_content_lock_writer_timeouts_total` | counter | emitted | `storage/src/gc.rs` `lock_store_exclusive_within` | Every exclusive store-lock wait that gave up because writers held the shared side: the history prune, the partial-upload cleanup and the other store collectors of the process. |
| `dcl_block_fetch_retries_total` | counter `network` = `mainnet`, `sepolia`, `polygon`, `amoy` | emitted | `content/src/validator/onchain/mod.rs` `read_block` | A block read asked again after a transient error. |

## Garbage collection

`catalyst serve` collects content through the history prune (`CONTENT_HISTORY_PRUNE_ENABLED`),
which drops superseded deployments and unlinks the blobs nothing else names. Its passes are
the collection runs below. The whole-store collector (`catalyst storage gc`) is an operator
command outside `serve` and exports nothing.

| Legacy series | Type and labels | Status | Observed at | Notes |
|---|---|---|---|---|
| `dcl_content_garbage_collection_items_total` | counter | replaced by `content_history_prune_blobs_total` | `content/src/history_prune.rs` `pass` | Blobs the prune unlinked. |
| `dcl_content_garbage_collection_time` | histogram | replaced by `dcl_job_duration_seconds{worker="content-history-prune"}` | `jobs/src/instrumentation.rs` | The prune job's pass duration, with the job buckets (0.01 to 1800 s). |
| `dcl_content_garbage_collection_runs_total` | counter `outcome` = `success`, `deferred`, `error` | emitted | `content/src/history_prune.rs` `run` | `deferred`: the exclusive store lock stayed busy. A pass the flag refuses is not a run. |
| `dcl_content_garbage_collection_last_success_timestamp_seconds` | gauge | replaced by `dcl_job_last_success_timestamp_seconds{worker="content-history-prune"}` | `jobs/src/instrumentation.rs` | A deferred pass fails the job, as it fails the legacy run. The job is leased, so one replica sets it. |

## Server state and snapshots

| Legacy series | Type and labels | Status | Observed at | Notes |
|---|---|---|---|---|
| `dcl_content_server_build_info` | gauge `version`, `commitHash`, `ethNetwork`; value 1 | emitted | `content/src/lib.rs` `ContentState::with_sync` | `commitHash` is `Unknown` when the process has none. |
| `dcl_content_server_sync_state` | gauge | covered in `sync.md` | | Observed by the sync code. |
| `dcl_content_server_failed_deployments` | gauge | covered in `sync.md` | | Observed by the sync code. |
| `dcl_content_server_snapshot_generation_time` | histogram `result` = `success`, `error`, `interval_size` = `day`, `week`, `month`, `year`, `unknown`, `reason` = `cover_time_range`, `snapshots_not_stored`, `inactive_entities`, `is_outdated`, `unknown`; default buckets | emitted | `content/src/snapshot_gen.rs` `SnapshotGenerator::pass` | Each window written. `cover_time_range`: no row for the window; `snapshots_not_stored`: its stored file could not be verified; `unknown`: a requested rebuild. |
| `dcl_content_server_snapshot_entities` | gauge `type` | not applicable | | Declared but never set by the content server, so no dashboard reads a value from it. |
| `dcl_deployed_entities_bloom_filter_checks_total` | counter `hit` = `true`, `false` | emitted | `content/src/batch_deployer.rs` `schedule_entity_deployment`, `retain_undeployed` | Already-deployed probes of synced entities: `true` when the entity is deployed, `false` for a filter miss and for a false positive the database refuted. |

## Not applicable

| Legacy series | Type and labels | Status | Notes |
|---|---|---|---|
| `dcl_files_migrated` | counter | not applicable | Counts a one-off content folder migration this server does not have. |
| `dcl_entities_cache_accesses_total` | counter `entity_type`, `result` | not applicable | Active entities are read from the database on every request; there is no in-process entity cache. |
| `dcl_entities_cache_storage_max_size` | gauge | not applicable | Same: no in-process entity cache. |
| `dcl_entities_cache_storage_size` | gauge `entity_type` | not applicable | Same: no in-process entity cache. |
| `dcl_db_query_duration_seconds` | histogram `query`, `status` | not applicable | Queries run through `sqlx` directly, with no named-query wrapper to label them. |
| `dcl_db_tx_acquired_clients_total` | counter | not applicable | Transactions take connections from the `sqlx` pool directly, with no transaction wrapper to count them. |
| `dcl_db_tx_released_clients_total` | counter | not applicable | Same as above. |

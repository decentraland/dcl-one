# Sync metrics

`catalyst serve` exports the series that the content network's dashboards and alerts read for peer
sync. They keep the legacy name, type, label names, label values and buckets of these pinned sources:

- `decentraland/snapshots-fetcher@53e9c07a`, `src/metrics.ts`;
- `decentraland/deployments-to-sqs@8afab047`, `src/metrics.ts`;
- `decentraland/catalyst@6c56f8a3`, `src/metrics.ts` (its sync series only).

The series are defined in `content/src/sync/metrics.rs`. Fixed label sets render at 0 from start
(`register_series`, called from `src/serve_lifecycle.rs`). The histogram buckets reach the exporter through
`comms::metrics::prometheus_builder`, which applies `sync::metrics::HISTOGRAM_BUCKETS`.

## Labels

- `remote_server` is the origin of the peer URL that was asked, for example `https://peer.example`
  for `https://peer.example/content`. It is empty for snapshot entities, because a snapshot has no
  single origin. A download that follows a redirect is still counted under the peer that was asked,
  not under the redirect target. The target is chosen by the peer and is not bounded.
- `entity_type` and `entityType` are one of `scene`, `wearable`, `profile`, `store`, `emote` or
  `outfits`. Any other value from a peer is reported as `other`.

## snapshots-fetcher

| Legacy series | Type and labels | dcl-one | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `dcl_content_download_bytes_total` | counter `remote_server` | emitted | `deploy_remote_entity::fetch_verified`, `sync::snapshots::download_snapshot_file` | Decoded bytes of a body that arrived whole. |
| `dcl_content_download_duration_seconds` | histogram `remote_server`, buckets 0.01–60 | emitted | same | One observation per request, whether it succeeds or fails. |
| `dcl_content_download_errors_total` | counter `remote_server` | emitted | same | Request error, non-2xx status (404 included), unreadable or truncated body. A read-through miss that a source answers 404 counts here too. |
| `dcl_content_download_hash_errors_total` | counter `remote_server` | emitted | same | Whole body that does not hash to its name. |
| `dcl_corrupt_entity_files_evicted_total` | counter | emitted | `deploy_remote_entity::evict_corrupt_entity_file_locked` | Stored entity file removed after it failed its hash check. |
| `dcl_entities_deployments_processed_total` | counter `remote_server`, `source` | emitted | `batch_deployer::DeploymentReport::record_acknowledged`, `batch_deployer::skipped` | Deployed, recorded in `content.failed_deployments`, or skipped. |
| `dcl_entities_deployments_streamed_total` | counter `remote_server`, `source` | emitted | `sync::snapshots::schedule_records`, `sync::pointer_changes::deploy_entities_from_pointer_changes` | Entities handed to the deployer, after the type and genesis filters. |
| `dcl_catalysts_pointer_changes_response_time_seconds` | histogram `remote_server`, buckets 0.01–60 | emitted | `sync::pointer_changes::fetch_page` | Each page request, answered or not. Covers bootstrap passes and the steady-state stream. |
| `dcl_deployments_stream_reconnection_count` | counter `remote_server` | emitted | `sync::orchestrator::start_steady_state_sync` | Each start of a steady-state stream run. |
| `dcl_deployments_stream_failure_count` | counter `remote_server` | emitted | same | Each steady-state stream run that ends in an error. |
| `dcl_content_download_job_succeed_retries` | histogram, buckets 1–30 | emitted | `deploy_remote_entity::download_file`, `download_snapshot_file` | Request number (1-based) that stored the file. |
| `dcl_available_servers_histogram` | histogram, buckets 1–15 | emitted | same | Candidate peers, observed once per job for a file not already stored. |
| `dcl_bootstrapping_servers` | gauge `from` (`snapshots`, `pointer-changes`) | emitted | `sync::orchestrator::report_phases` | Peers in each bootstrap phase. Set to 0 when the orchestrator stops. |
| `dcl_syncing_servers` | gauge | emitted | same | Peers in steady state. |
| `dcl_processed_snapshots_total` | counter `state` (`stream_end`, `saved`, `incomplete`) | emitted | `sync::snapshots::deploy_entities_from_snapshot`, `sync::orchestrator::bootstrap_from_snapshots` | `saved` counts snapshots marked processed after they were streamed. It does not count snapshots retired without streaming because they were replaced. |

## Content server (sync series)

| Legacy series | Type and labels | dcl-one | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `dcl_ignored_sync_deployments` | counter | emitted | `batch_deployer::skipped` | Covers a type outside `SYNC_ENTITY_TYPES`, a profile older than `SYNC_PROFILE_MAX_AGE_DAYS`, and an entity that is already deployed. |
| `dcl_pending_download_gauge` | gauge `entity_type` | emitted | `deploy_remote_entity::deploy_entity_streaming` | Entities whose entity and content files are downloading, during sync, retries and read-through. |
| `dcl_pending_deployment_gauge` | gauge `entity_type` | emitted | `batch_deployer::BatchDeployer::schedule_undeployed` | Entities that hold a slot in the sync queue, from scheduling until their outcome is recorded. |
| `dcl_deployment_time` | histogram `entity_type`, `failed`, prom-client default buckets | emitted | `deploy_remote_entity::deploy_entity_streaming` | Validation and hand-off to the batch writer. The database write happens in a later flush. |
| `dcl_content_server_sync_state` | gauge | emitted | `SyncStatus::set_state` | `Syncing` is 1. `Bootstrapping`, `PartiallySynced` (profiles still bootstrapping) and `Failed` are 0. A node serving unsynced (`SERVE_UNSYNCED`) reports 1. A process that does not run sync reports the state it mirrors from the leader. |
| `dcl_content_server_failed_deployments` | gauge | emitted | `sync::retry_failed::execute_retry_cycle` | Rows in `content.failed_deployments`, counted once per retry cycle (`retry_delay_ms`, 15 minutes by default). Absent until the first cycle (which waits for bootstrap when this process runs sync) and while the retry worker is off. |

## deployments-to-sqs

dcl-one has no SNS or SQS stage. In this table, a deployment is a sync deployment: an entity that a
peer announced, downloaded with its content files, and then validated and handed to the batch writer.

| Legacy series | Type and labels | dcl-one | Observed at | Notes |
| --- | --- | --- | --- | --- |
| `test_ping_counter` | counter `pathname` | not applicable | — | Counted calls to that service's own `/ping` route. |
| `sns_publish_success` | counter `type` | not applicable | — | No SNS publisher. |
| `sns_publish_failure` | counter `type` | not applicable | — | No SNS publisher. |
| `entity_download_success` | counter `entityType` | emitted | `deploy_remote_entity::deploy_entity_streaming` | Entity file and every required content file stored. Covers sync, retries and read-through. |
| `entity_download_failure` | counter `entityType`, `retryable` | emitted | same | `retryable` is always `true`: every failure is recorded for the retry worker, or asked again on the next read-through miss. Discards of overtaken entities are not counted. |
| `schedule_entity_deployment_attempt` | counter `entityType` | emitted | with `dcl_entities_deployments_streamed_total` | Each entity handed to the sync deployer. |
| `entity_already_stored` | counter `entityType` | emitted | `batch_deployer::skipped` | Skipped because it is already deployed: the bloom filter matched and the database confirmed it. |
| `entity_deployment_success` | counter `entityType` | emitted | `deploy_remote_entity::deploy_entity_streaming` | Validated and handed to the batch writer, after a successful download. |
| `entity_deployment_failure` | counter `entityType`, `retryable` | emitted | same | Validation or hand-off failed after a successful download. Download failures are not counted here. `retryable` is always `true`. |
| `download_queue_size` | gauge | not applicable | — | Each entity starts as soon as it is scheduled. A full queue blocks the scheduler instead of holding jobs, so no job waits unstarted. |
| `download_queue_pending` | gauge | emitted | `batch_deployer::BatchDeployer::schedule_undeployed` | Sum of `dcl_pending_deployment_gauge`. A value at the queue depth (1000) means the queue is saturated. |
| `entity_deployment_queue_failure` | counter `entityType` | not applicable | — | The sync queue has no timeout, so a job is never rejected after it is scheduled. |
| `entity_skipped_old` | counter `entityType` | emitted | `batch_deployer::skipped` | Profiles older than `SYNC_PROFILE_MAX_AGE_DAYS`. The age limit applies only to profiles. |
| `entity_skipped_invalid_id` | counter `entityType` | not applicable | — | Sync has no step that skips an entity id. An id the store refuses fails its download and is recorded as a failed deployment. |

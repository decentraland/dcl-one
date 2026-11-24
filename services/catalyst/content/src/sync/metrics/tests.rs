use super::*;

fn render(f: impl FnOnce()) -> String {
    let (guard, handle) = local_recorder();
    f();
    drop(guard);
    handle.render()
}

fn assert_samples(rendered: &str, expected: &[(&str, f64)]) {
    for (series, value) in expected {
        assert_eq!(
            sample(rendered, series),
            Some(*value),
            "{series}:\n{rendered}"
        );
    }
}

fn assert_types(rendered: &str, expected: &[(&str, &str)]) {
    for (name, kind) in expected {
        assert!(
            rendered.contains(&format!("# TYPE {name} {kind}\n")),
            "{name} {kind}:\n{rendered}"
        );
    }
}

#[test]
fn labels_are_the_peer_origin_and_a_known_entity_type() {
    assert_eq!(
        remote_server("https://peer.example/content"),
        "https://peer.example"
    );
    assert_eq!(
        remote_server("http://127.0.0.1:5140/content/pointer-changes?from=1"),
        "http://127.0.0.1:5140"
    );
    assert_eq!(entity_type_label("wearable"), "wearable");
    assert_eq!(entity_type_label("Scene"), "other");
    assert_eq!(entity_type_label("land"), "other");
}

#[test]
fn every_series_keeps_its_legacy_name_type_and_labels() {
    let rendered = render(|| {
        let peer = "https://peer.example/content";
        download_finished("https://peer.example", Duration::from_millis(5), Some(100));
        download_finished("https://peer.example", Duration::from_millis(5), None);
        download_hash_error("https://peer.example");
        corrupt_entity_file_evicted();
        pointer_changes_page(peer, Duration::from_millis(5));
        stream_connected(peer);
        stream_failed(peer);
        download_job_started(2);
        download_job_succeeded(1);
        server_phases(1, 2, 3);
        snapshots_processed("saved", 2);
        sync_state(&SyncState::Syncing);
        failed_deployments(7);

        let snapshots = SyncSource::snapshots();
        snapshots.streamed("scene");
        snapshots.processed();
        let changes = SyncSource::pointer_changes(peer);
        changes.streamed("not-a-type");
        changes.processed();

        skipped("profile", Skip::OldProfile);
        skipped("scene", Skip::AlreadyDeployed);
        skipped("outfits", Skip::IgnoredType);
        entity_downloaded("scene", true);
        entity_downloaded("scene", false);
        entity_deployed("scene", Duration::from_millis(5), true);
        entity_deployed("scene", Duration::from_millis(5), false);
        drop((InFlight::deployment("emote"), InFlight::download("emote")));
    });
    let peer = "remote_server=\"https://peer.example\"";
    assert_samples(
        &rendered,
        &[
            (
                &format!("dcl_content_download_bytes_total{{{peer}}}"),
                100.0,
            ),
            (&format!("dcl_content_download_errors_total{{{peer}}}"), 1.0),
            (
                &format!("dcl_content_download_hash_errors_total{{{peer}}}"),
                1.0,
            ),
            (
                &format!("dcl_content_download_duration_seconds_count{{{peer}}}"),
                2.0,
            ),
            ("dcl_corrupt_entity_files_evicted_total", 1.0),
            (
                &format!("dcl_catalysts_pointer_changes_response_time_seconds_count{{{peer}}}"),
                1.0,
            ),
            (
                &format!("dcl_deployments_stream_reconnection_count{{{peer}}}"),
                1.0,
            ),
            (
                &format!("dcl_deployments_stream_failure_count{{{peer}}}"),
                1.0,
            ),
            ("dcl_available_servers_histogram_count", 1.0),
            ("dcl_content_download_job_succeed_retries_count", 1.0),
            ("dcl_bootstrapping_servers{from=\"snapshots\"}", 1.0),
            ("dcl_bootstrapping_servers{from=\"pointer-changes\"}", 2.0),
            ("dcl_syncing_servers", 3.0),
            ("dcl_processed_snapshots_total{state=\"saved\"}", 2.0),
            ("dcl_content_server_sync_state", 1.0),
            ("dcl_content_server_failed_deployments", 7.0),
            (
                "dcl_entities_deployments_streamed_total{remote_server=\"\",source=\"snapshots\"}",
                1.0,
            ),
            (
                "dcl_entities_deployments_processed_total{remote_server=\"\",source=\"snapshots\"}",
                1.0,
            ),
            (
                &format!(
                    "dcl_entities_deployments_streamed_total{{{peer},source=\"pointer-changes\"}}"
                ),
                1.0,
            ),
            (
                &format!(
                    "dcl_entities_deployments_processed_total{{{peer},source=\"pointer-changes\"}}"
                ),
                1.0,
            ),
            (
                "schedule_entity_deployment_attempt{entityType=\"scene\"}",
                1.0,
            ),
            (
                "schedule_entity_deployment_attempt{entityType=\"other\"}",
                1.0,
            ),
            ("dcl_ignored_sync_deployments", 3.0),
            ("entity_skipped_old{entityType=\"profile\"}", 1.0),
            ("entity_already_stored{entityType=\"scene\"}", 1.0),
            ("entity_download_success{entityType=\"scene\"}", 1.0),
            (
                "entity_download_failure{entityType=\"scene\",retryable=\"true\"}",
                1.0,
            ),
            ("entity_deployment_success{entityType=\"scene\"}", 1.0),
            (
                "entity_deployment_failure{entityType=\"scene\",retryable=\"true\"}",
                1.0,
            ),
            (
                "dcl_deployment_time_count{entity_type=\"scene\",failed=\"false\"}",
                1.0,
            ),
            (
                "dcl_deployment_time_count{entity_type=\"scene\",failed=\"true\"}",
                1.0,
            ),
            ("dcl_pending_deployment_gauge{entity_type=\"emote\"}", 0.0),
            ("dcl_pending_download_gauge{entity_type=\"emote\"}", 0.0),
            ("download_queue_pending", 0.0),
        ],
    );
    assert_types(
        &rendered,
        &[
            ("dcl_content_download_bytes_total", "counter"),
            ("dcl_content_download_errors_total", "counter"),
            ("dcl_content_download_hash_errors_total", "counter"),
            ("dcl_entities_deployments_processed_total", "counter"),
            ("dcl_entities_deployments_streamed_total", "counter"),
            ("dcl_deployments_stream_reconnection_count", "counter"),
            ("dcl_deployments_stream_failure_count", "counter"),
            ("dcl_content_server_sync_state", "gauge"),
            ("dcl_content_server_failed_deployments", "gauge"),
            ("dcl_pending_deployment_gauge", "gauge"),
            ("dcl_pending_download_gauge", "gauge"),
            ("schedule_entity_deployment_attempt", "counter"),
            ("entity_already_stored", "counter"),
            ("entity_skipped_old", "counter"),
            ("entity_download_success", "counter"),
            ("entity_download_failure", "counter"),
            ("entity_deployment_success", "counter"),
            ("entity_deployment_failure", "counter"),
        ],
    );
}

#[test]
fn in_flight_gauges_count_what_is_held() {
    let (_guard, handle) = local_recorder();
    let held = [InFlight::deployment("scene"), InFlight::deployment("scene")];
    let download = InFlight::download("profile");
    assert_samples(
        &handle.render(),
        &[
            ("dcl_pending_deployment_gauge{entity_type=\"scene\"}", 2.0),
            ("download_queue_pending", 2.0),
            ("dcl_pending_download_gauge{entity_type=\"profile\"}", 1.0),
        ],
    );
    drop((held, download));
    assert_samples(
        &handle.render(),
        &[
            ("dcl_pending_deployment_gauge{entity_type=\"scene\"}", 0.0),
            ("download_queue_pending", 0.0),
            ("dcl_pending_download_gauge{entity_type=\"profile\"}", 0.0),
        ],
    );
}

/// Each sync state maps onto legacy's 0 (bootstrapping) or 1 (syncing).
#[test]
fn only_a_synced_node_reports_state_one() {
    for (state, value) in [
        (SyncState::Bootstrapping, 0.0),
        (
            SyncState::PartiallySynced {
                ready_types: vec!["scene".into()],
            },
            0.0,
        ),
        (SyncState::Failed, 0.0),
        (SyncState::Syncing, 1.0),
    ] {
        let rendered = render(|| crate::SyncStatus::default().set_state(state.clone()));
        assert_samples(&rendered, &[("dcl_content_server_sync_state", value)]);
    }
}

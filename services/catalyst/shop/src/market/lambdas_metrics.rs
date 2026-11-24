//! The lamb2 series the lambdas keep, declared for serve's registry.
use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily};

/// lamb2's `dcl_lamb2_server_build_info{commitHash}`: 1 for the running build.
const BUILD_INFO: GaugeFamily<1> = GaugeFamily::new(
    "dcl_lamb2_server_build_info",
    "Lamb2 server static build info.",
    None,
    &["commitHash"],
);

/// Reads of an owner's names from the per-owner subgraph memo by the lambdas
/// `/users/{address}/names` route, by `result`: `hit` answered from the memo,
/// `miss` fetched from (or joined a fetch of) the marketplace subgraph. It
/// replaces lamb2's `elements_cache_reads_total` for that read.
const OWNER_MEMO_READS: CounterFamily<1> = CounterFamily::new(
    "dcl_lambdas_owner_memo_reads_total",
    "Lambdas reads of an owner's names from the per-owner subgraph memo, by result.",
    None,
    &["result"],
);

pub const FAMILIES: &[Descriptor] = &[BUILD_INFO.descriptor, OWNER_MEMO_READS.descriptor];

/// Sets the build info at startup: `commit_hash` is `COMMIT_HASH`, else the
/// build's revision, else empty, as lamb2 reported `COMMIT_HASH` or empty.
pub fn record_build_info(commit_hash: &str) {
    BUILD_INFO
        .handle_owned([commit_hash.to_owned().into()])
        .set(1.0);
}

pub(super) fn record_owner_memo_read(hit: bool) {
    OWNER_MEMO_READS
        .handle([if hit { "hit" } else { "miss" }])
        .increment(1);
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "current_thread")]
    async fn build_info_and_owner_memo_reads_render() {
        let recorder = dcl_metrics::Registry::new(&[super::FAMILIES])
            .unwrap()
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = dcl_metrics::set_default_local_recorder(&recorder);
        super::record_build_info("abc123");
        let (index, _) = crate::chain_index::graph::test_support::index_with_names().await;
        let owner = "0x00000000000000000000000000000000000000aa";
        for _ in 0..2 {
            let hit = crate::chain_index::graph::all_owned_names_memoized(&index, owner);
            super::record_owner_memo_read(hit);
            crate::chain_index::graph::owned_names_or_stale(&index, owner)
                .await
                .unwrap();
        }
        let rendered = handle.render();
        for line in [
            "dcl_lamb2_server_build_info{commitHash=\"abc123\"} 1",
            "dcl_lambdas_owner_memo_reads_total{result=\"miss\"} 1",
            "dcl_lambdas_owner_memo_reads_total{result=\"hit\"} 1",
        ] {
            assert!(rendered.contains(line), "missing {line}:\n{rendered}");
        }
    }
}

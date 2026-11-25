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
/// replaces lamb2's `elements_cache_reads_total` for that read, and counts
/// only where the subgraph is the NAME source
/// ([`super::select_name_authority`]): never on a mainnet node with the `nft`
/// index on.
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
    async fn build_info_renders() {
        let recorder = dcl_metrics::Registry::new(&[super::FAMILIES])
            .unwrap()
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = dcl_metrics::set_default_local_recorder(&recorder);
        super::record_build_info("abc123");
        let rendered = handle.render();
        let line = "dcl_lamb2_server_build_info{commitHash=\"abc123\"} 1";
        assert!(rendered.contains(line), "missing {line}:\n{rendered}");
    }
}

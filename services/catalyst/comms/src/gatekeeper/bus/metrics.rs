//! The link's own series, over the `metrics` facade (a no-op recorder when none is installed):
//! `comms_nats_connected`, 1 while the link is ready (a broker round trip confirmed on its
//! current connection), 0 otherwise.

use crate::metrics::declarations::NATS_CONNECTED;

pub(crate) fn nats_connected(up: bool) {
    NATS_CONNECTED.handle([]).set(if up { 1.0 } else { 0.0 });
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_metrics::PrometheusBuilder;

    #[test]
    fn the_connected_gauge_follows_the_link() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, || {
            nats_connected(false);
            nats_connected(true);
        });
        assert!(handle.render().contains("comms_nats_connected 1"));
    }
}

//! Measurements for periodic passes while this process owns the job.
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dcl_metrics::{
    CounterFamily, Descriptor, Gauge, GaugeFamily, Histogram, HistogramFamily, SharedString, Unit,
};

const LEASES: GaugeFamily<0> = GaugeFamily::new(
    "dcl_job_leases_held",
    "Job leases currently held by this process.",
    None,
    &[],
);
const RUNS: CounterFamily<2> = CounterFamily::new(
    "dcl_job_runs_total",
    "Periodic job passes by outcome.",
    None,
    &["worker", "outcome"],
);
const DURATION: HistogramFamily<1> = HistogramFamily::new(
    "dcl_job_duration_seconds",
    "Duration of completed periodic job passes.",
    Some(Unit::Seconds),
    &["worker"],
    &[0.01, 0.1, 1., 5., 30., 60., 300., 1800.],
);
const ACTIVE: GaugeFamily<1> = GaugeFamily::new(
    "dcl_job_in_progress",
    "Periodic job passes currently executing.",
    None,
    &["worker"],
);
const SUCCESS: GaugeFamily<1> = GaugeFamily::new(
    "dcl_job_last_success_timestamp_seconds",
    "Unix timestamp of the last successful periodic pass.",
    Some(Unit::Seconds),
    &["worker"],
);

pub const FAMILIES: &[Descriptor] = &[
    LEASES.descriptor,
    RUNS.descriptor,
    DURATION.descriptor,
    ACTIVE.descriptor,
    SUCCESS.descriptor,
];

/// Stored with the acquisition so release, session loss and shutdown all update the gauge.
pub(super) struct HeldLease(Gauge);

impl HeldLease {
    pub(super) fn new() -> Self {
        let gauge = LEASES.handle([]);
        gauge.increment(1.);
        Self(gauge)
    }
}

impl Drop for HeldLease {
    fn drop(&mut self) {
        self.0.decrement(1.);
    }
}

pub(super) struct PeriodicJob {
    name: SharedString,
    active: Gauge,
    duration: Histogram,
    success: Gauge,
}

impl PeriodicJob {
    pub(super) fn new(name: &str) -> Self {
        let name: SharedString = name.to_owned().into();
        Self {
            active: ACTIVE.handle_owned([name.clone()]),
            duration: DURATION.handle_owned([name.clone()]),
            success: SUCCESS.handle_owned([name.clone()]),
            name,
        }
    }

    pub(super) fn begin(&self) -> Pass<'_> {
        self.active.increment(1.0);
        Pass(self)
    }
}

pub(super) struct Pass<'a>(&'a PeriodicJob);

impl Pass<'_> {
    pub(super) fn finish(self, outcome: &'static str, elapsed: Duration) {
        self.0.duration.record(elapsed.as_secs_f64());
        RUNS.handle_owned([self.0.name.clone(), outcome.into()])
            .increment(1);
        if outcome == "success"
            && let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH)
        {
            self.0.success.set(now.as_secs_f64());
        }
    }
}

impl Drop for Pass<'_> {
    fn drop(&mut self) {
        self.0.active.decrement(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_pass_releases_gauge_and_only_finished_passes_increment_totals() {
        let registry = dcl_metrics::Registry::new(&[FAMILIES]).unwrap();
        let recorder = registry
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, || {
            let lease = HeldLease::new();
            assert!(handle.render().contains("dcl_job_leases_held 1"));
            drop(lease);
            let metrics = PeriodicJob::new("mirror");
            let cancelled = metrics.begin();
            assert!(
                handle
                    .render()
                    .contains("dcl_job_in_progress{worker=\"mirror\"} 1")
            );
            drop(cancelled);
            metrics.begin().finish("success", Duration::from_secs(2));
            metrics.begin().finish("error", Duration::from_secs(1));
        });
        let body = handle.render();
        assert!(body.contains("dcl_job_leases_held 0"));
        assert!(body.contains("dcl_job_in_progress{worker=\"mirror\"} 0"));
        assert!(body.contains("dcl_job_runs_total{worker=\"mirror\",outcome=\"success\"} 1"));
        assert!(body.contains("dcl_job_runs_total{worker=\"mirror\",outcome=\"error\"} 1"));
        assert!(body.contains("dcl_job_duration_seconds_sum{worker=\"mirror\"} 3"));
        assert!(body.contains("dcl_job_last_success_timestamp_seconds{worker=\"mirror\"}"));
    }
}

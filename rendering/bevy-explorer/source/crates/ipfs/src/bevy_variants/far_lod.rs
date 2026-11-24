//! What the far LOD requests have answered this session ([`IpfsIo::bevy_far_lod`]): the scenes
//! the server has no mesh for, and a breaker that pauses far LODs while the LOD service fails.
//!
//! [`IpfsIo::bevy_far_lod`]: crate::IpfsIo::bevy_far_lod

use std::collections::{HashSet, VecDeque};

use web_time::{Duration, Instant};

/// Far LOD requests in a row that fail with an error or a timeout (not a missing mesh) after
/// which the LOD service is taken to be down and far LODs pause.
pub(crate) const FAR_LOD_BREAKER_FAILURES: u32 = 5;

/// How long far LODs pause once the breaker trips. When it ends one request goes out as a probe;
/// while that one fails, the next probe waits as long again.
pub(crate) const FAR_LOD_COOL_DOWN: Duration = Duration::from_secs(60);

/// Most scenes the answered names and the missing meshes each hold, the same bound
/// `failed_remotes` keeps urls to.
pub(crate) const FAR_LOD_MEMORY_LIMIT: usize = crate::failed_remotes::LIMIT;

/// A change of the breaker, which the caller logs once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BreakerChange {
    Tripped,
    Recovered,
}

#[derive(Default)]
pub(crate) struct FarLodHealth {
    /// scenes (lower case) whose far LOD url `bevy_far_lod` handed out and whose download has
    /// not answered yet. A name stays here until its answer comes, so that answer counts; these
    /// are scenes the realm lists, named once each
    pending: HashSet<String>,
    /// scenes whose download answered, oldest first, at most [`FAR_LOD_MEMORY_LIMIT`]: past it
    /// the oldest is forgotten, so naming never refuses. With `pending`, the names a download
    /// report counts for: a url a scene made up changes nothing
    answered_order: VecDeque<String>,
    answered_names: HashSet<String>,
    /// scenes (lower case) whose mesh the server answered a definitive status for, at most
    /// [`FAR_LOD_MEMORY_LIMIT`]: past it a further one is not remembered and may be asked for
    /// again. It is asked before the url is named
    missing: HashSet<String>,
    /// failed requests in a row
    failures: u32,
    /// while tripped: when the next request, the probe, may go out
    paused_until: Option<Instant>,
}

/// The breaker's state, for `/scene_lod_state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarLodStatus {
    /// scenes the server has no far LOD for, remembered for the session
    pub missing: usize,
    /// far LOD requests failed in a row
    pub failures: u32,
    /// while far LODs are paused: how long until the next probe
    pub paused_for: Option<Duration>,
}

impl FarLodHealth {
    /// Records that the url for `scene` is handed out: its download's answer will count.
    pub(crate) fn name(&mut self, scene: &str) {
        if !self.is_named(scene) {
            self.pending.insert(scene.to_owned());
        }
    }

    /// Whether `scene`'s far LOD url was handed out, and its answer not since forgotten.
    pub(crate) fn is_named(&self, scene: &str) -> bool {
        self.pending.contains(scene) || self.answered_names.contains(scene)
    }

    /// `scene`'s download answered: its name moves from `pending` to the bounded answered names.
    fn settle(&mut self, scene: &str) {
        if !self.pending.remove(scene) {
            return;
        }
        if self.answered_order.len() >= FAR_LOD_MEMORY_LIMIT {
            if let Some(oldest) = self.answered_order.pop_front() {
                self.answered_names.remove(&oldest);
            }
        }
        self.answered_order.push_back(scene.to_owned());
        self.answered_names.insert(scene.to_owned());
    }

    /// The variants base changed: what the old service answered no longer holds. The names stay,
    /// so a far LOD named under the old base still reports.
    pub(crate) fn new_base(&mut self) {
        self.missing.clear();
        self.failures = 0;
        self.paused_until = None;
    }

    /// Whether the server said earlier this session that it has no mesh for `scene`.
    pub(crate) fn is_missing(&self, scene: &str) -> bool {
        self.missing.contains(scene)
    }

    /// Whether a far LOD request may go out at `now`. While the breaker is tripped it may not,
    /// until the cool-down ends; then the first request goes as the probe and the next waits for
    /// another cool-down, so a probe that never reports (served from the cache, or dropped)
    /// cannot hold far LODs paused.
    pub(crate) fn may_request(&mut self, now: Instant) -> bool {
        match self.paused_until {
            Some(until) if now < until => false,
            Some(_) => {
                self.paused_until = Some(now + FAR_LOD_COOL_DOWN);
                true
            }
            None => true,
        }
    }

    /// The server answered: with the mesh, or (`missing`) a definitive status that it has none
    /// for `scene`. Either way the service is up.
    pub(crate) fn answered(&mut self, scene: &str, missing: bool) -> Option<BreakerChange> {
        self.settle(scene);
        if missing && self.missing.len() < FAR_LOD_MEMORY_LIMIT {
            self.missing.insert(scene.to_owned());
        }
        self.failures = 0;
        self.paused_until.take().map(|_| BreakerChange::Recovered)
    }

    /// `scene`'s request failed with an error, a timeout or a status that may change.
    pub(crate) fn failed(&mut self, scene: &str, now: Instant) -> Option<BreakerChange> {
        self.settle(scene);
        self.failures = self.failures.saturating_add(1);
        if self.paused_until.is_some() || self.failures < FAR_LOD_BREAKER_FAILURES {
            return None;
        }
        self.paused_until = Some(now + FAR_LOD_COOL_DOWN);
        Some(BreakerChange::Tripped)
    }

    pub(crate) fn status(&self, now: Instant) -> FarLodStatus {
        FarLodStatus {
            missing: self.missing.len(),
            failures: self.failures,
            paused_for: self
                .paused_until
                .map(|until| until.checked_duration_since(now).unwrap_or_default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trip(health: &mut FarLodHealth, now: Instant) {
        for _ in 1..FAR_LOD_BREAKER_FAILURES {
            assert_eq!(health.failed("x", now), None);
        }
        assert_eq!(health.failed("x", now), Some(BreakerChange::Tripped));
    }

    #[test]
    fn a_missing_mesh_is_not_asked_for_again() {
        let now = Instant::now();
        let mut health = FarLodHealth::default();
        assert!(!health.is_missing("a"));
        assert!(health.may_request(now));
        assert_eq!(health.answered("a", true), None);
        let later = now + Duration::from_secs(3600);
        assert!(health.is_missing("a"));
        assert!(!health.is_missing("b"));
        assert!(health.may_request(later));
        assert_eq!(health.status(later).missing, 1);
    }

    #[test]
    fn naming_never_refuses_and_a_pending_name_is_kept() {
        let now = Instant::now();
        let mut health = FarLodHealth::default();
        assert!(!health.is_named("a"));
        health.name("a");
        assert!(health.is_named("a"));
        // more answered names than the bound: the oldest are forgotten, naming goes on
        for n in 0..FAR_LOD_MEMORY_LIMIT + 10 {
            let scene = format!("s{n}");
            health.name(&scene);
            health.answered(&scene, false);
        }
        assert_eq!(health.answered_order.len(), FAR_LOD_MEMORY_LIMIT);
        assert!(!health.is_named("s0"));
        assert!(health.is_named(&format!("s{}", FAR_LOD_MEMORY_LIMIT + 9)));
        // a name whose download has not answered is never forgotten
        assert!(health.is_named("a"));
        health.name("one-more");
        assert!(health.is_named("one-more"));
        // missing meshes stay bounded on their own
        for n in 0..FAR_LOD_MEMORY_LIMIT + 10 {
            let scene = format!("m{n}");
            health.name(&scene);
            health.answered(&scene, true);
        }
        assert_eq!(health.status(now).missing, FAR_LOD_MEMORY_LIMIT);
        assert!(health.is_missing("m0"));
    }

    #[test]
    fn failures_in_a_row_pause_far_lods_for_the_cool_down() {
        let now = Instant::now();
        let mut health = FarLodHealth::default();
        trip(&mut health, now);
        assert!(!health.may_request(now));
        let almost = now + FAR_LOD_COOL_DOWN - Duration::from_millis(1);
        assert!(!health.may_request(almost));
        assert_eq!(
            health.status(almost).paused_for,
            Some(Duration::from_millis(1))
        );
        // further failures while paused change nothing
        assert_eq!(health.failed("x", almost), None);
    }

    #[test]
    fn an_answer_between_failures_keeps_the_breaker_closed() {
        let now = Instant::now();
        let mut health = FarLodHealth::default();
        for _ in 1..FAR_LOD_BREAKER_FAILURES {
            health.failed("x", now);
        }
        // a missing mesh is an answer: the service is up
        health.answered("a", true);
        for _ in 1..FAR_LOD_BREAKER_FAILURES {
            assert_eq!(health.failed("x", now), None);
        }
        assert!(health.may_request(now));
    }

    #[test]
    fn one_probe_per_cool_down_and_its_answer_resumes() {
        let now = Instant::now();
        let mut health = FarLodHealth::default();
        trip(&mut health, now);
        let end = now + FAR_LOD_COOL_DOWN;
        // the probe, then nothing until another cool-down passes
        assert!(health.may_request(end));
        assert!(!health.may_request(end));
        // the probe fails: still paused, the next probe a cool-down later
        assert_eq!(health.failed("x", end), None);
        assert!(!health.may_request(end + FAR_LOD_COOL_DOWN / 2));
        let second = end + FAR_LOD_COOL_DOWN;
        assert!(health.may_request(second));
        // it is answered: far LODs resume
        assert_eq!(health.answered("b", false), Some(BreakerChange::Recovered));
        assert!(health.may_request(second));
        assert!(health.may_request(second));
        assert_eq!(
            health.status(second),
            FarLodStatus {
                missing: 0,
                failures: 0,
                paused_for: None
            }
        );
    }
}

//! Places for uploads in flight: a node-wide count, and of it a count per
//! wallet and per source address, taken before an upload's body is read and
//! given back when its [`Admission`] drops. A refused place answers 429 with
//! `Retry-After` at once ([`busy`]); nothing waits for a place.
//!
//! An admitted upload reads its body through [`Admission::read`]: all of it
//! by its [`Timing::deadline`], never [`Timing::idle`] without a byte, else
//! 408. It is behind while it has received less than its route's largest
//! body takes at the pace that body needs to arrive by the deadline
//! ([`Timing::most`] over [`Timing::deadline`]), counted from its admission.
//! While every place is held and some upload is behind, a newcomer takes an
//! extra place and waits for its own first bytes, one newcomer for each
//! upload behind; when they arrive ([`Admission::arrived`]) the slowest
//! upload behind ends with 408 and `Retry-After`, or, if none is behind any
//! more and no place came free, the newcomer does. A newcomer that sends
//! nothing ends at its idle bound and cuts nobody.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use tokio::time::{Instant, timeout_at};

use crate::http::ApiError;

/// The `Retry-After` of a refused place, in seconds.
pub const RETRY_AFTER_SECS: u32 = 30;

/// The longest an upload's body may go without a byte before the upload
/// ends.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// How an upload's body must arrive.
#[derive(Clone, Copy)]
pub struct Timing {
    /// All of it by then, counted from its admission.
    pub deadline: Duration,
    /// Never this long without a byte.
    pub idle: Duration,
    /// The route's largest body.
    pub most: u64,
}

impl Timing {
    fn pace(&self) -> f64 {
        self.most as f64 / self.deadline.as_secs_f64().max(f64::MIN_POSITIVE)
    }
}

/// The places of one kind of upload.
pub struct Places {
    what: &'static str,
    total: usize,
    per_signer: usize,
    per_source: usize,
    in_flight: Arc<Mutex<InFlight>>,
}

#[derive(Default)]
struct InFlight {
    total: usize,
    by_key: HashMap<String, usize>,
    reading: Vec<Arc<Progress>>,
}

impl InFlight {
    fn count(&self, which: impl Fn(&Progress) -> bool) -> usize {
        self.reading.iter().filter(|p| which(p)).count()
    }

    fn beyond(&self) -> usize {
        self.count(|p| p.cut.load(Ordering::SeqCst) || p.waiting.load(Ordering::SeqCst))
    }
}

/// What an upload reading its body has received since its admission.
struct Progress {
    started: Instant,
    pace: f64,
    received: AtomicU64,
    cut: AtomicBool,
    waiting: AtomicBool,
    cut_now: tokio::sync::Notify,
}

impl Progress {
    fn rate(&self, now: Instant) -> f64 {
        let secs = now.saturating_duration_since(self.started).as_secs_f64();
        self.received.load(Ordering::SeqCst) as f64 / secs.max(f64::MIN_POSITIVE)
    }

    fn behind(&self, now: Instant) -> bool {
        !self.cut.load(Ordering::SeqCst)
            && !self.waiting.load(Ordering::SeqCst)
            && self.rate(now) < self.pace
    }
}

/// One admitted upload's place, given back when dropped.
pub struct Admission {
    in_flight: Arc<Mutex<InFlight>>,
    keys: Vec<String>,
    total: usize,
    what: &'static str,
    progress: Arc<Progress>,
    deadline: Instant,
    idle: Duration,
    last: Instant,
}

impl Drop for Admission {
    fn drop(&mut self) {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        in_flight.total -= 1;
        for key in &self.keys {
            if let Some(count) = in_flight.by_key.get_mut(key) {
                *count -= 1;
                if *count == 0 {
                    in_flight.by_key.remove(key);
                }
            }
        }
        in_flight
            .reading
            .retain(|p| !Arc::ptr_eq(p, &self.progress));
    }
}

/// A refused place: 429 at once, with a `Retry-After`.
pub fn busy(error: &str) -> Response {
    retry_later(ApiError::http(429, error))
}

/// `refusal` with the `Retry-After` of a refused place.
pub fn retry_later(refusal: ApiError) -> Response {
    let mut response = refusal.into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from(RETRY_AFTER_SECS));
    response
}

fn full(what: &str) -> String {
    format!("Too many {what} in progress, try again later")
}

impl Places {
    /// `total` places for uploads of `what` (its refusals name it), at most
    /// `per_signer` of one wallet and `per_source` of one source address.
    pub fn new(what: &'static str, total: usize, per_signer: usize, per_source: usize) -> Self {
        Self {
            what,
            total,
            per_signer,
            per_source,
            in_flight: Arc::default(),
        }
    }

    /// A place for `signer`'s upload from `source` (`limits::source`: `None`
    /// when the request names no single client, which counts toward no
    /// source) whose body arrives as `timing` says, or the refusal
    /// ([`busy`]). While every place is held, an extra place that waits for
    /// the upload's first bytes, if an upload is behind for each newcomer
    /// already waiting and one more.
    pub fn admit(
        &self,
        signer: &str,
        source: Option<&str>,
        timing: Timing,
    ) -> Result<Admission, Response> {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        let wallet = format!("wallet:{signer}");
        let count = |key: &str| in_flight.by_key.get(key).copied().unwrap_or(0);
        if count(&wallet) >= self.per_signer {
            return Err(busy(&format!(
                "Too many {} of this wallet in progress",
                self.what
            )));
        }
        let mut keys = vec![wallet];
        if let Some(source) = source {
            let source = format!("source:{source}");
            if count(&source) >= self.per_source {
                return Err(busy(&format!(
                    "Too many {} from this address in progress",
                    self.what
                )));
            }
            keys.push(source);
        }
        let now = Instant::now();
        let waiting = in_flight.total >= self.total + in_flight.beyond();
        if waiting
            && in_flight.count(|p| p.behind(now))
                <= in_flight.count(|p| p.waiting.load(Ordering::SeqCst))
        {
            return Err(busy(&full(self.what)));
        }
        for key in &keys {
            *in_flight.by_key.entry(key.clone()).or_default() += 1;
        }
        in_flight.total += 1;
        let progress = Arc::new(Progress {
            started: now,
            pace: timing.pace(),
            received: AtomicU64::new(0),
            cut: AtomicBool::new(false),
            waiting: AtomicBool::new(waiting),
            cut_now: tokio::sync::Notify::new(),
        });
        in_flight.reading.push(progress.clone());
        Ok(Admission {
            in_flight: self.in_flight.clone(),
            keys,
            total: self.total,
            what: self.what,
            progress,
            deadline: now + timing.deadline,
            idle: timing.idle,
            last: now,
        })
    }

    /// Uploads holding a place.
    pub fn in_flight(&self) -> usize {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .total
    }
}

fn timed_out() -> ApiError {
    ApiError::http(408, "The upload timed out")
}

fn yielded() -> ApiError {
    ApiError::http(
        408,
        "The upload gave its place to another while the node was busy",
    )
}

impl Admission {
    /// `read` of the body: 408 once the deadline or the idle bound since the
    /// last bytes passes, or when the upload gave its place to a newcomer.
    pub async fn read<T>(&self, read: impl Future<Output = T>) -> Result<T, ApiError> {
        let until = self.deadline.min(self.last + self.idle);
        tokio::select! {
            read = timeout_at(until, read) => read.map_err(|_| timed_out()),
            () = self.progress.cut_now.notified() => Err(yielded()),
        }
    }

    /// `bytes` of the body arrived. A newcomer's first bytes take the place
    /// of the slowest upload behind; with none behind and no place come
    /// free, the newcomer gives way itself (429).
    pub fn arrived(&mut self, bytes: usize) -> Result<(), ApiError> {
        self.last = Instant::now();
        self.progress
            .received
            .fetch_add(bytes as u64, Ordering::SeqCst);
        if bytes == 0 || !self.progress.waiting.load(Ordering::SeqCst) {
            return Ok(());
        }
        let in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        self.progress.waiting.store(false, Ordering::SeqCst);
        if in_flight.total <= self.total + in_flight.beyond() {
            return Ok(());
        }
        let now = Instant::now();
        let slowest = in_flight
            .reading
            .iter()
            .filter(|p| !Arc::ptr_eq(p, &self.progress) && p.behind(now))
            .min_by(|a, b| a.rate(now).total_cmp(&b.rate(now)));
        match slowest {
            Some(slowest) => {
                slowest.cut.store(true, Ordering::SeqCst);
                slowest.cut_now.notify_one();
                Ok(())
            }
            None => {
                self.progress.cut.store(true, Ordering::SeqCst);
                Err(ApiError::http(429, full(self.what)))
            }
        }
    }

    /// The whole body arrived: no newcomer takes its place while it stores.
    pub fn body_arrived(&self) {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reading
            .retain(|p| !Arc::ptr_eq(p, &self.progress));
    }

    /// It gave its place to a newcomer, or, a newcomer itself, found none to
    /// take.
    pub fn gave_way(&self) -> bool {
        self.progress.cut.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMING: Timing = Timing {
        deadline: Duration::from_secs(100),
        idle: IDLE_TIMEOUT,
        most: 1_000,
    };

    fn four() -> Places {
        Places::new("uploads", 4, 1, 2)
    }

    fn hold(places: &Places, n: usize) -> Admission {
        places
            .admit(&format!("0xholder{n}"), None, TIMING)
            .map_err(|refused| refused.status())
            .expect("a place")
    }

    /// Uploads that each send nothing and restart just before their idle
    /// bound never keep out a creator that sends: its first bytes take the
    /// place of one of them.
    #[tokio::test(start_paused = true)]
    async fn uploads_restarting_inside_their_idle_window_never_keep_out_a_creator_that_sends() {
        let places = four();
        let mut held: Vec<Admission> = Vec::new();
        for round in 0..10 {
            held.clear();
            held.extend((0..4).map(|n| hold(&places, 4 * round + n)));
            tokio::time::advance(Duration::from_secs(29)).await;
            let mut creator = places
                .admit("0xcreator", None, TIMING)
                .map_err(|refused| refused.status())
                .expect("the creator waits for its bytes");
            assert!(creator.arrived(1).is_ok());
            assert_eq!(held.iter().filter(|h| h.gave_way()).count(), 1);
        }
    }

    /// Keeping a creator out takes every place sending at the pace its
    /// route needs: while all keep it, the creator is refused at once; the
    /// first that falls behind gives way once the creator's bytes arrive.
    #[tokio::test(start_paused = true)]
    async fn keeping_a_creator_out_takes_every_place_at_its_pace() {
        let places = four();
        let mut held: Vec<Admission> = (0..4).map(|n| hold(&places, n)).collect();
        for _ in 0..5 {
            tokio::time::advance(Duration::from_secs(10)).await;
            for (n, upload) in held.iter_mut().enumerate() {
                if n != 2 {
                    upload.arrived(100).unwrap();
                }
            }
            if held[2].progress.received.load(Ordering::SeqCst) == 0 {
                held[2].arrived(100).unwrap();
                let refused = places.admit("0xcreator", None, TIMING).err();
                assert_eq!(refused.map(|r| r.status().as_u16()), Some(429));
            }
        }
        let mut creator = places
            .admit("0xcreator", None, TIMING)
            .map_err(|refused| refused.status())
            .expect("the creator waits for its bytes");
        assert!(!held[2].gave_way(), "cut before the creator sent");
        creator.arrived(1).unwrap();
        let cut: Vec<usize> = (0..4).filter(|n| held[*n].gave_way()).collect();
        assert_eq!(cut, [2]);
    }

    /// A newcomer whose bytes arrive once no upload is behind and no place
    /// came free gives way itself, and the upload that caught up keeps its
    /// place.
    #[tokio::test(start_paused = true)]
    async fn a_newcomer_finding_no_upload_behind_when_its_bytes_arrive_gives_way_itself() {
        let places = Places::new("uploads", 1, 1, 2);
        let mut holder = hold(&places, 0);
        tokio::time::advance(Duration::from_secs(10)).await;
        let mut newcomer = places
            .admit("0xnewcomer", None, TIMING)
            .map_err(|refused| refused.status())
            .expect("the newcomer waits for its bytes");
        holder.arrived(1_000).unwrap();
        assert_eq!(newcomer.arrived(1).map_err(|e| e.status()), Err(429));
        assert!(newcomer.gave_way() && !holder.gave_way());
        drop(newcomer);
        assert_eq!(places.in_flight(), 1);
    }

    /// A newcomer whose bytes arrive after a place came free takes it and
    /// cuts nobody.
    #[tokio::test(start_paused = true)]
    async fn a_newcomer_whose_bytes_arrive_after_a_place_came_free_cuts_nobody() {
        let places = Places::new("uploads", 2, 1, 2);
        let first = hold(&places, 0);
        let second = hold(&places, 1);
        tokio::time::advance(Duration::from_secs(10)).await;
        let mut newcomer = places
            .admit("0xnewcomer", None, TIMING)
            .map_err(|refused| refused.status())
            .expect("the newcomer waits for its bytes");
        drop(first);
        newcomer.arrived(1).unwrap();
        assert!(!second.gave_way() && !newcomer.gave_way());
        assert_eq!(places.in_flight(), 2);
    }
}

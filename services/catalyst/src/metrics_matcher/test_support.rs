//! Finite engine checks called only by the dedicated integration-test process.
use super::*;
use crate::comms::stats::{self, content, http};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Hooks(std::sync::Mutex<Vec<http::Observation>>);
#[async_trait::async_trait]
impl http::Hooks for Hooks {
    async fn probe(&self, _: bool) -> http::Lifecycle {
        http::Lifecycle {
            initialized: true,
            started: true,
            live: true,
            checks: vec![],
        }
    }
    async fn metrics(&self) -> Result<String, content::Error> {
        Ok("actual_fixture 1\n".into())
    }
    async fn diagnostics(&self) -> Result<String, content::Error> {
        Ok("{}".into())
    }
    fn observe(&self, observation: http::Observation) {
        self.0.lock().unwrap().push(observation);
    }
}
struct EmptyContent;
#[async_trait::async_trait]
impl content::Source for EmptyContent {
    async fn fetch(
        &self,
        _: &[String],
    ) -> Result<Vec<crate::content::OrderedJson>, content::Error> {
        Ok(vec![])
    }
}

fn prepared(pattern: &str) -> PreparedStats {
    // SAFETY: run's caller keeps this complete test inside the initialized child
    // subtree. No prepared/native object is returned to the integration harness.
    unsafe {
        prepare(Settings {
            metrics_path: pattern.into(),
            metrics_bearer_token: Some("fixture".into()),
            ..Default::default()
        })
    }
    .unwrap()
}

fn state(prepared: PreparedStats) -> (http::State, Arc<Hooks>) {
    let hooks = Arc::new(Hooks::default());
    let (settings, matcher) = prepared.into_parts();
    (
        http::State::new(
            Arc::new(stats::Reader::default()),
            Arc::new(EmptyContent),
            hooks.clone(),
            settings,
            matcher,
        ),
        hooks,
    )
}

fn distribution(mut samples: Vec<u128>) -> Value {
    samples.sort_unstable();
    let n = samples.len();
    json!({"n":n,"p50_ns":samples[n/2],"p95_ns":samples[n*95/100],"p99_ns":samples[n*99/100],"max_ns":samples[n-1]})
}

const FINITE_PATTERN: &str = "/scrape/:name((?=[a-z0-9]{16,64}$)[a-z]+[0-9]+)";

fn finite_input(index: usize) -> (String, bool) {
    let matched = index.is_multiple_of(2);
    let suffix = if matched { "" } else { "-" };
    (
        format!("/scrape/tenant{}{:04}{suffix}", "a".repeat(32), index),
        matched,
    )
}

#[derive(Default)]
struct NativeCounters {
    entered: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    completed: AtomicUsize,
    finalized: AtomicBool,
    entered_first: tokio::sync::Notify,
}

pub(super) struct NativeProbe {
    counters: Arc<NativeCounters>,
    release: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    admission: Arc<Semaphore>,
    require_retained_teardown: bool,
}

impl NativeProbe {
    pub(super) fn enter(&self) -> NativeOperation<'_> {
        let active = self.counters.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.counters.peak.fetch_max(active, Ordering::SeqCst);
        self.counters.entered.fetch_add(1, Ordering::SeqCst);
        // Take the receiver before waiting so subsequent operations cannot block.
        let release = self.release.lock().unwrap().take();
        if let Some(release) = release {
            self.counters.entered_first.notify_one();
            release.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        NativeOperation(&self.counters)
    }
}

impl Drop for NativeProbe {
    fn drop(&mut self) {
        if self.require_retained_teardown {
            assert_eq!(
                self.admission.available_permits(),
                0,
                "final native teardown must precede submitted admission release"
            );
        }
        self.counters.finalized.store(true, Ordering::SeqCst);
    }
}

pub(super) struct NativeOperation<'a>(&'a NativeCounters);
impl Drop for NativeOperation<'_> {
    fn drop(&mut self) {
        self.0.completed.fetch_add(1, Ordering::SeqCst);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

fn probed_adapter(
    retained_teardown: bool,
) -> (
    Arc<Adapter>,
    Arc<NativeCounters>,
    std::sync::mpsc::Sender<()>,
) {
    let admission = Arc::new(Semaphore::new(1));
    let counters = Arc::new(NativeCounters::default());
    let (release, wait) = std::sync::mpsc::channel();
    let mut compiled = Compiled::new(FINITE_PATTERN).unwrap();
    compiled.probe = Some(NativeProbe {
        counters: counters.clone(),
        release: std::sync::Mutex::new(Some(wait)),
        admission: admission.clone(),
        require_retained_teardown: retained_teardown,
    });
    (
        Arc::new(Adapter {
            compiled: Arc::new(compiled),
            admission,
        }),
        counters,
        release,
    )
}

async fn unrelated_http(client: &reqwest::Client, address: std::net::SocketAddr) {
    assert_eq!(
        client
            .get(format!("http://{address}/unrelated"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "shared"
    );
}

async fn native_contention() -> Value {
    tokio::time::timeout(Duration::from_secs(30), async {
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut rows = Vec::new();
        for cancellation in ["blocking_pool_queued", "native_locked_before_evaluation"] {
            let start = Instant::now();
            let (adapter, counters, release) = probed_adapter(false);
            let (pool_release, pool_wait) = std::sync::mpsc::channel();
            let (pool_started, pool_start) = tokio::sync::oneshot::channel();
            let busy = tokio::task::spawn_blocking(move || {
                pool_started.send(()).unwrap();
                pool_wait.recv_timeout(Duration::from_secs(10)).unwrap();
            });
            pool_start.await.unwrap();
            let pool_start = Instant::now();
            let first_adapter = adapter.clone();
            let mut first = Some(tokio::spawn(async move { first_adapter.matches(&finite_input(0).0).await }));
            while adapter.admission.available_permits() != 0 { tokio::task::yield_now().await; }
            assert_eq!(counters.entered.load(Ordering::SeqCst), 0);
            if cancellation == "blocking_pool_queued" {
                let first = first.take().unwrap();
                first.abort();
                assert!(first.await.unwrap_err().is_cancelled());
                // No other matcher can reacquire and hide a premature release.
                assert_eq!(adapter.admission.available_permits(), 0);
            }
            let (state, _) = state(PreparedStats {
                settings: Settings { metrics_path: FINITE_PATTERN.into(), metrics_bearer_token: Some("fixture".into()), ..Default::default() },
                matcher: adapter.clone(),
            });
            let app = http::mount_services(
                axum::Router::new().fallback(|| async { "shared" }), state.clone(),
                http::standalone(state), axum::Router::new(),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                axum::serve(listener, app).with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
            });
            let mut requests = Vec::new();
            for index in 0..32 {
                let client = client.clone();
                requests.push(tokio::spawn(async move {
                    let start = Instant::now();
                    let (path, matched) = finite_input(index);
                    let response = client.get(format!("http://{address}/archipelago-ea-stats{path}"))
                        .bearer_auth("fixture").send().await.unwrap();
                    assert_eq!(response.status().as_u16(), if matched { 200 } else { 404 });
                    let body = response.text().await.unwrap();
                    if matched { assert_eq!(body, "actual_fixture 1\n"); }
                    start.elapsed().as_nanos()
                }));
            }
            unrelated_http(&client, address).await;
            assert!(requests.iter().all(|request| !request.is_finished()));
            assert_eq!(adapter.admission.available_permits(), 0);
            assert_eq!(counters.entered.load(Ordering::SeqCst), 0);
            let pool_blocked_ns = pool_start.elapsed().as_nanos();
            pool_release.send(()).unwrap();
            busy.await.unwrap();
            counters.entered_first.notified().await;
            let held = Instant::now();
            if let Some(first) = first {
                first.abort();
                assert!(first.await.unwrap_err().is_cancelled());
            }
            assert_eq!(adapter.admission.available_permits(), 0);
            assert_eq!(counters.entered.load(Ordering::SeqCst), 1);
            assert_eq!(counters.active.load(Ordering::SeqCst), 1);
            unrelated_http(&client, address).await;
            assert!(requests.iter().all(|request| !request.is_finished()));
            let native_probe_held_ns = held.elapsed().as_nanos();
            release.send(()).unwrap();
            let mut times = Vec::new();
            for request in requests { times.push(request.await.unwrap()); }
            assert_eq!(counters.completed.load(Ordering::SeqCst), 33);
            assert_eq!(counters.peak.load(Ordering::SeqCst), 1);
            stop.send(()).unwrap();
            server.await.unwrap();
            drop(adapter);
            assert!(counters.finalized.load(Ordering::SeqCst));
            rows.push(json!({"cancellation":cancellation,"native_completed":33,"peak_native_locked":1,
                "controlled_pool_blocked_ns":pool_blocked_ns,"controlled_native_locked_ns":native_probe_held_ns,
                "http":distribution(times),"elapsed_ns":start.elapsed().as_nanos(),"unrelated_http_completed":2,
                "resident_bytes":resident_bytes()}));
        }
        let (adapter, counters, release) = probed_adapter(true);
        let admission = adapter.admission.clone();
        let caller = adapter.clone();
        let first = tokio::spawn(async move { caller.matches(&finite_input(0).0).await });
        counters.entered_first.notified().await;
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        // Only the submitted real adapter closure now owns the compiled isolate.
        drop(adapter);
        assert!(!counters.finalized.load(Ordering::SeqCst));
        assert_eq!(admission.available_permits(), 0);
        release.send(()).unwrap();
        // This runtime has one blocking thread: the marker follows full teardown.
        tokio::task::spawn_blocking(|| ()).await.unwrap();
        assert!(counters.finalized.load(Ordering::SeqCst));
        assert_eq!(counters.completed.load(Ordering::SeqCst), 1);
        assert_eq!(admission.available_permits(), 1);
        json!({"workload":"native-adapter-contention","cancellations":rows,
            "detached_last_owner_native_teardown":true})
    }).await.unwrap()
}

async fn finite_workload() -> Value {
    let compiled = Compiled::new(FINITE_PATTERN).unwrap();
    let mut matched = Vec::new();
    let mut near_miss = Vec::new();
    for index in 0..32 {
        let (path, expected) = finite_input(index);
        let start = Instant::now();
        let actual = compiled.matches(&path).unwrap();
        let elapsed = start.elapsed().as_nanos();
        assert_eq!(actual, expected);
        if expected {
            matched.push(elapsed);
        } else {
            near_miss.push(elapsed);
        }
    }
    drop(compiled);
    json!({"workload":"finite-lookahead-near-miss","pattern":FINITE_PATTERN,"distinct_inputs":32,
        "native_completed":32,"matched":distribution(matched),"near_miss":distribution(near_miss),
        "resident_bytes":resident_bytes()})
}

/// Run finite adapter, HTTP and performance checks, returning handle-free data.
///
/// # Safety
/// Invoke only inside a fresh child of the sole initializing thread in the
/// dedicated engine-test process. All runtime/matcher state and panic payloads
/// must be destroyed in that child; the platform must remain alive.
pub unsafe fn run() -> Value {
    let cases: Vec<Value> = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/src/metrics_matcher/cases.json"
    )))
    .unwrap();
    let expected: Vec<Value> = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/src/metrics_matcher/expected.json"
    )))
    .unwrap();
    let before = resident_bytes();
    let cold = Instant::now();
    let first = prepared("/metrics");
    let compile_ns = cold.elapsed().as_nanos();
    let after_compile = resident_bytes();
    drop(first);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let mut rows = Vec::new();
    runtime.block_on(async {
        for (case, expected) in cases.iter().zip(&expected) {
            let id = case["id"].as_str().unwrap();
            assert_eq!(case["id"], expected["id"]);
            let settings = Settings {
                metrics_path: case["pattern"].as_str().unwrap().into(),
                ..Default::default()
            };
            // SAFETY: this runtime and every blocking worker descend from the
            // initializing child, and each prepared owner is dropped here.
            let compiled = unsafe { prepare(settings) };
            if expected["ok"] == false {
                let error = match compiled {
                    Err(error) => error,
                    Ok(_) => panic!("{id}: expected compiler failure"),
                };
                assert!(
                    error.0.ends_with(expected["error"].as_str().unwrap()),
                    "{id}: {error}"
                );
                rows.push(json!({"id":id,"error":error.0}));
            } else {
                let compiled = compiled.unwrap_or_else(|error| panic!("{id}: {error}"));
                let mut matches = Vec::new();
                for (input, expected) in case["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(expected["matches"].as_array().unwrap())
                {
                    let actual = compiled
                        .matcher
                        .matches(input.as_str().unwrap())
                        .await
                        .unwrap();
                    assert_eq!(actual, expected.as_bool().unwrap(), "{id}: {input}");
                    matches.push(actual);
                }
                rows.push(json!({"id":id,"matches":matches}));
            }
        }
        let optional = prepared("/scrape/:name?");
        assert!(optional.matcher.matches("/scrape").await.unwrap());
        assert!(optional.matcher.matches("/scrape/server").await.unwrap());
        assert!(
            !optional
                .matcher
                .matches("/scrape/server/other")
                .await
                .unwrap()
        );
    });
    drop(runtime);
    let configured = std::thread::available_parallelism().unwrap().get();
    let mut measurements = Vec::new();
    for workers in [1, 2, configured] {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .enable_all()
            .build()
            .unwrap();
        measurements.push(runtime.block_on(http_and_costs(workers)));
        // Runtime::drop waits for submitted blocking work; no shutdown timeout.
        drop(runtime);
    }
    let mut native = Vec::new();
    for workers in [1, 2, configured] {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let finite = runtime.block_on(finite_workload());
        let contention = runtime.block_on(native_contention());
        drop(runtime);
        native.push(json!({"workers":workers,"finite":finite,"contention":contention}));
    }
    json!({"engine":v8::V8::get_version(),"compile_ns":compile_ns,"resident_before":before,
        "resident_after_compile":after_compile,"resident_after":resident_bytes(),"cases":rows,"measurements":measurements,"native_workloads":native})
}

async fn http_and_costs(workers: usize) -> Value {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut http_rows = Vec::new();
    for (pattern, path) in [
        ("/metrics", "/metrics"),
        ("/scrape/:name?", "/scrape"),
        ("/scrape/:name+", "/scrape/a/b"),
        ("/scrape{/:name}?", "/scrape/a"),
        ("/x/:v((?=ab)ab)", "/x/ab"),
        (r"/x/:v((?<a>a)\k<a>)", "/x/aa"),
        (r"/x/\:literal", "/x/:literal"),
        ("/x/%C3%A9", "/x/%C3%A9"),
        ("/peers", "/peers"),
        ("/health/:name", "/health/live"),
    ] {
        let (state, hooks) = state(prepared(pattern));
        let app = http::mount_services(
            axum::Router::new().fallback(|| async { "shared" }),
            state.clone(),
            http::standalone(state),
            axum::Router::new(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        let health = path == "/health/live";
        for (method, auth, expected) in [
            ("GET", None, if health { 200 } else { 401 }),
            ("GET", Some("Bearer wrong"), if health { 200 } else { 401 }),
            ("GET", Some("Bearer fixture"), 200),
            ("HEAD", None, if health { 200 } else { 401 }),
            ("HEAD", Some("Bearer wrong"), if health { 200 } else { 401 }),
            ("HEAD", Some("Bearer fixture"), 200),
            ("POST", Some("Bearer fixture"), 405),
            ("PROPFIND", None, 501),
            ("OPTIONS", None, 204),
        ] {
            let mut request = client
                .request(
                    reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
                    format!("http://{address}/archipelago-ea-stats{path}"),
                )
                .header("origin", "https://example.invalid");
            if let Some(auth) = auth {
                request = request.header("authorization", auth);
            }
            let response = request.send().await.unwrap();
            assert_eq!(
                response.status().as_u16(),
                expected,
                "{workers} {pattern} {method}"
            );
            let headers: std::collections::BTreeMap<String, String> = response
                .headers()
                .iter()
                .filter(|(name, _)| !matches!(name.as_str(), "date" | "connection" | "keep-alive"))
                .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
                .collect();
            let body = response.text().await.unwrap();
            if method == "HEAD" || method == "OPTIONS" {
                assert!(body.is_empty());
            }
            let observations = std::mem::take(&mut *hooks.0.lock().unwrap());
            if method == "OPTIONS" || (health && matches!(method, "GET" | "HEAD")) {
                assert!(observations.is_empty());
            } else {
                assert_eq!(observations.len(), 1);
                assert_eq!(
                    observations[0].handler,
                    if matches!(method, "GET" | "HEAD") {
                        pattern
                    } else {
                        ""
                    }
                );
            }
            let observation = observations
                .first()
                .map(|o| json!({"handler":o.handler,"code":o.code,"method":o.method}));
            http_rows.push(json!({"pattern":pattern,"pathname":path,"method":method,"auth":auth,"status":expected,"body":body,"headers":headers,"observation":observation}));
        }
        if pattern == "/peers" {
            let response = client
                .get(format!("http://{address}/peers"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.text().await.unwrap(), "shared");
        }
        stop.send(()).unwrap();
        server.await.unwrap();
    }
    let compiled = Compiled::new("/scrape/:name?").unwrap();
    let mut execution = Vec::new();
    for _ in 0..128 {
        let start = Instant::now();
        assert!(compiled.matches("/scrape/a").unwrap());
        execution.push(start.elapsed().as_nanos());
    }
    drop(compiled);
    let gate = Arc::new(Semaphore::new(1));
    let mut admission = Vec::new();
    let mut scheduling = Vec::new();
    for _ in 0..128 {
        let start = Instant::now();
        let permit = gate.clone().acquire_owned().await.unwrap();
        admission.push(start.elapsed().as_nanos());
        drop(permit);
        let start = Instant::now();
        scheduling.push(
            tokio::task::spawn_blocking(move || start.elapsed().as_nanos())
                .await
                .unwrap(),
        );
    }
    let (state, _) = state(prepared("/scrape/:name?"));
    let app = http::mount_services(
        axum::Router::new().fallback(|| async { "shared" }),
        state.clone(),
        http::standalone(state),
        axum::Router::new(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    let mut loads = Vec::new();
    for concurrency in [1, 2, 32] {
        let done = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let d = done.clone();
        let p = progress.clone();
        let c = client.clone();
        let other = tokio::spawn(async move {
            while !d.load(Ordering::SeqCst) {
                assert_eq!(
                    c.get(format!("http://{address}/unrelated"))
                        .send()
                        .await
                        .unwrap()
                        .text()
                        .await
                        .unwrap(),
                    "shared"
                );
                p.fetch_add(1, Ordering::SeqCst);
            }
        });
        let mut jobs = Vec::new();
        let start = Instant::now();
        for _ in 0..concurrency {
            let c = client.clone();
            jobs.push(tokio::spawn(async move {
                let mut times = Vec::new();
                for _ in 0..8 {
                    let start = Instant::now();
                    let r = c
                        .get(format!("http://{address}/archipelago-ea-stats/scrape/a"))
                        .bearer_auth("fixture")
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(r.text().await.unwrap(), "actual_fixture 1\n");
                    times.push(start.elapsed().as_nanos());
                }
                times
            }));
        }
        let mut times = Vec::new();
        for job in jobs {
            times.extend(job.await.unwrap());
        }
        done.store(true, Ordering::SeqCst);
        other.await.unwrap();
        assert!(progress.load(Ordering::SeqCst) > 0);
        loads.push(json!({"concurrency":concurrency,"http":distribution(times),"elapsed_ns":start.elapsed().as_nanos(),"unrelated_http_completed":progress.load(Ordering::SeqCst),"resident_bytes":resident_bytes()}));
    }
    stop.send(()).unwrap();
    server.await.unwrap();
    json!({"workers":workers,"http_cases":http_rows,"execute":distribution(execution),"uncontended_admission":distribution(admission),"empty_blocking_schedule":distribution(scheduling),"loads":loads})
}

#[cfg(windows)]
fn resident_bytes() -> u64 {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        size: u32,
        faults: u32,
        peak: usize,
        working: usize,
        page_peak: usize,
        page: usize,
        nonpage_peak: usize,
        nonpage: usize,
        pagefile: usize,
        pagefile_peak: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, m: *mut Counters, size: u32) -> i32;
    }
    let mut m = Counters::default();
    m.size = std::mem::size_of::<Counters>() as u32;
    // SAFETY: current-process query with an initialized writable native structure.
    assert_ne!(
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.size) },
        0
    );
    m.working as u64
}
#[cfg(not(windows))]
fn resident_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("VmRSS:")
                    .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
        * 1024
}

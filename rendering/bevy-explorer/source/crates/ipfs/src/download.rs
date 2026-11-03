use super::*;
use priority_gate::{DownloadAttempts, DownloadReaders, GateClosed, Registration};
use std::sync::{atomic::AtomicU64, Mutex};
use tokio::sync::oneshot;

pub(super) struct SharedDownload {
    readers: DownloadReaders,
    result: watch::Receiver<Option<Result<SharedBytes, AssetReaderError>>>,
}

pub(super) struct SharedReader {
    _registration: Registration,
    download: Arc<SharedDownload>,
}

impl SharedReader {
    pub(super) fn result(&self) -> watch::Receiver<Option<Result<SharedBytes, AssetReaderError>>> {
        self.download.result.clone()
    }
}

pub(super) type InFlightDownloads = Mutex<HashMap<String, Weak<SharedDownload>>>;

impl IpfsIo {
    pub(super) fn share_download(
        self: &Arc<Self>,
        key: CacheKey,
        token: u16,
        path: PathBuf,
        remote: String,
        scene: Option<String>,
    ) -> (
        SharedReader,
        Option<impl ConditionalSendFuture<Output = ()> + 'static>,
    ) {
        let mut entries = self.in_flight.lock().unwrap();
        if let Some(download) = entries.get(&remote).and_then(Weak::upgrade) {
            if let Some(reader) = download.readers.try_register(scene.clone()) {
                debug!("[{token:?}]: remote url: `{remote}` joining in-flight download");
                return (
                    SharedReader {
                        _registration: reader,
                        download,
                    },
                    None,
                );
            }
        }
        entries.retain(|_, entry| entry.strong_count() > 0);
        let (cancel, cancelled) = oneshot::channel();
        let (reader, attempts) = DownloadReaders::first(&self.request_slots, scene, Some(cancel));
        let (sender, result) = watch::channel(None);
        let download = Arc::new(SharedDownload {
            readers: reader.readers.clone(),
            result,
        });
        let identity = Arc::downgrade(&download);
        entries.insert(remote.clone(), identity.clone());
        drop(entries);

        let io = self.clone();
        let task = async move {
            // The last registration cancels the transport, queue wait or backoff on both executors.
            let result = tokio::select! {
                biased;
                _ = cancelled => None,
                result = io.request_metrics.measure(io.clone().download_inner(
                    token, path, remote.clone(), Some(key), attempts,
                )) => Some(result),
            };
            // Another reader may already have started a new generation for the same URL.
            let mut entries = io.in_flight.lock().unwrap();
            if entries
                .get(&remote)
                .is_some_and(|entry| entry.ptr_eq(&identity))
            {
                entries.remove(&remote);
            }
            drop(entries);
            if let Some(result) = result {
                sender.send_replace(Some(result));
            }
        };
        (
            SharedReader {
                _registration: reader,
                download,
            },
            Some(task),
        )
    }

    pub(super) async fn download(
        self: Arc<Self>,
        token: u16,
        path: PathBuf,
        remote: String,
        cache_key: Option<CacheKey>,
        scene: Option<String>,
    ) -> Result<SharedBytes, AssetReaderError> {
        let (_reader, attempts) = DownloadReaders::first(&self.request_slots, scene, None);
        self.request_metrics
            .measure(
                self.clone()
                    .download_inner(token, path, remote, cache_key, attempts),
            )
            .await
    }

    /// Fetch unsigned content through its owner's gate queue, or the unowned queue.
    pub async fn fetch_request(
        &self,
        request: reqwest::Request,
        client: reqwest::Client,
        headers_timeout: Duration,
        idle_timeout: Duration,
        owner: Option<String>,
    ) -> Result<platform::FetchedResponse, platform::FetchError<anyhow::Error>> {
        let (_reader, attempts) = DownloadReaders::first(&self.request_slots, owner, None);
        self.request_metrics
            .measure(self.fetch_request_inner(
                attempts,
                request,
                &client,
                headers_timeout,
                idle_timeout,
                async_std::task::sleep,
            ))
            .await
    }

    /// A retry competes with every other waiting download, even for an immediate timeout retry.
    async fn fetch_request_inner<F: std::future::Future<Output = ()>>(
        &self,
        mut attempts: DownloadAttempts,
        request: reqwest::Request,
        client: &reqwest::Client,
        headers_timeout: Duration,
        idle_timeout: Duration,
        retry_wait: impl Fn(Duration) -> F,
    ) -> Result<platform::FetchedResponse, platform::FetchError<anyhow::Error>> {
        let is_get = request.method() == reqwest::Method::GET;
        let mut original = Some(request);
        for attempt in 1..=4 {
            // Keep a replayable GET for the next attempt. Consume every other request once.
            let request = original
                .as_ref()
                .filter(|_| is_get && attempt < 4)
                .and_then(reqwest::Request::try_clone)
                .or_else(|| original.take())
                .expect("request is replayable or this is its only attempt");
            let permit = attempts
                .acquire()
                .await
                .map_err(|e| platform::FetchError::Send(anyhow!(e)))?;
            let result = platform::http::fetch_with_permit(
                permit,
                || {
                    add_saturating(&self.request_metrics.attempts, 1);
                    if attempt > 1 {
                        add_saturating(&self.request_metrics.retries, 1);
                    }
                    async { client.execute(request).await.map_err(anyhow::Error::from) }
                },
                headers_timeout,
                idle_timeout,
                |bytes| add_saturating(&self.request_metrics.bytes, bytes as u64),
            )
            .await;
            let retry = match &result {
                Err(platform::FetchError::Status(status, headers)) => {
                    add_saturating(&self.request_metrics.status_failures, 1);
                    server_retry_delay(*status, headers)
                        .filter(|delay| *delay <= platform::http::MAX_RETRY_AFTER)
                }
                Err(platform::FetchError::Headers | platform::FetchError::Stalled) => {
                    Some(Duration::ZERO)
                }
                Err(platform::FetchError::Send(e))
                    if e.downcast_ref::<reqwest::Error>()
                        .is_some_and(reqwest::Error::is_timeout) =>
                {
                    Some(Duration::ZERO)
                }
                _ => None,
            };
            if let (Some(request), Some(delay)) = (&original, retry) {
                let url = request.url();
                match &result {
                    Err(platform::FetchError::Status(status, _)) => warn!(
                        "content request to `{url}` returned {status}, retrying in {delay:?} (attempt {attempt}/4)"
                    ),
                    Err(platform::FetchError::Stalled) => warn!(
                        "body from `{url}` stalled, retrying in {delay:?} (attempt {attempt}/4)"
                    ),
                    _ => warn!("request to `{url}` timed out, retrying in {delay:?} (attempt {attempt}/4)"),
                }
                if !delay.is_zero() {
                    retry_wait(delay).await;
                }
                continue;
            }
            return result;
        }
        unreachable!()
    }

    async fn download_inner(
        self: Arc<Self>,
        token: u16,
        path: PathBuf,
        remote: String,
        cache_key: Option<CacheKey>,
        attempts: DownloadAttempts,
    ) -> Result<SharedBytes, AssetReaderError> {
        let start = web_time::Instant::now();
        let ipfs_io_read_state = IpfsIoReadState {
            #[cfg(feature = "ipfs_debug")]
            sender: &self.debug_overlay_sender,
            path: Cow::Owned(path),
            #[cfg(feature = "ipfs_debug")]
            start,
        };
        // in wasm we add a custom header to allow the service worker to cache ipfs requests across content servers
        #[cfg(target_arch = "wasm32")]
        let sw_caches = cache_key.is_some();

        debug!("[{token:?}]: remote url: `{remote}` awaiting a request slot");
        // a request slot, granted nearest scene first
        let _deferred_in_flight = DeferredDropper::new(&IPFS_IN_FLIGHT);
        let mut no_cache = false;
        // whether the service worker answered from its cache
        #[cfg(target_arch = "wasm32")]
        let sw_cached;
        let client = &self.client;
        let request = client.get(&remote);

        #[cfg(target_arch = "wasm32")]
        let request = if sw_caches {
            request.header("X-IPFS", "true")
        } else {
            request
        };

        let request = ipfs_io_read_state.send_failure(request.build().map_err(|e| {
            AssetReaderError::Io(Arc::new(std::io::Error::other(format!("[{token:?}]: {e}"))))
        }))?;

        // Headers-phase timeout + body inactivity timeout; the total transfer
        // time is unbounded, so a slow-but-progressing download is never
        // killed — only a stall (no chunk for 10s) or a dead connection trips.
        let fetched = match self
            .fetch_request_inner(
                attempts,
                request,
                client,
                Duration::from_secs(30),
                Duration::from_secs(10),
                async_std::task::sleep,
            )
            .await
        {
            Ok(fetched) => fetched,
            Err(e) => {
                if matches!(&e, platform::FetchError::Send(e) if e.is::<GateClosed>()) {
                    return Err(AssetReaderError::Io(Arc::new(std::io::Error::new(
                        ErrorKind::Interrupted,
                        GateClosed,
                    ))));
                }
                let detail = match e {
                    platform::FetchError::Headers => "timed out awaiting headers".to_owned(),
                    platform::FetchError::Stalled => "stalled (no data for 10s)".to_owned(),
                    platform::FetchError::Send(e) => format!("server responded `{e}`"),
                    platform::FetchError::Status(s, _) => {
                        format!("server responded with status {s}")
                    }
                    platform::FetchError::Body(e) => format!("body stream error: {e}"),
                };
                self.context
                    .write()
                    .await
                    .failed_remotes
                    .insert(remote.clone(), Instant::now());
                return ipfs_io_read_state.send_failure(Err(AssetReaderError::Io(Arc::new(
                    std::io::Error::other(format!(
                        "[{token:?}] failed to retrieve `{remote}`: {detail}"
                    )),
                ))));
            }
        };

        if let Some(cache_control) = fetched.headers.get("cache-control") {
            if cache_control
                .to_str()
                .unwrap_or_default()
                .contains("no-store")
            {
                no_cache = true;
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            sw_cached = fetched.headers.contains_key(web_cache::CACHED_HEADER);
        }

        let data = SharedBytes(Arc::new(fetched.body));

        // A cache keyed on the hash alone answers for every server, so it only takes bytes
        // that are what the hash names: any server (and on web any scene, through the service
        // worker) can otherwise file its own bytes under another's hash.
        let cacheable = |key: &CacheKey| match key {
            CacheKey::Url(_) => true,
            CacheKey::Content(hash) => {
                let matches = content_hash::matches_content_hash(hash, &data);
                if !matches {
                    // not expected from a decentraland deployment: either the server sent
                    // other bytes, or the hash was made in a way `content_hash` doesn't know
                    warn!(
                        "`{remote}` ({} bytes) does not hash to `{hash}`: not caching it under that hash",
                        data.len()
                    );
                }
                matches
            }
        };

        // the worker only answers from its cache; fill it: content under its hash, for every
        // server, once checked; a url under its own key
        #[cfg(target_arch = "wasm32")]
        if sw_caches && !sw_cached {
            let kept = match &cache_key {
                Some(key @ CacheKey::Content(_)) => {
                    web_cache::share(&remote, key.name(), &data, || cacheable(key)).await
                }
                Some(CacheKey::Url(_)) if !no_cache => web_cache::keep(&remote, &data).await,
                _ => Ok(()),
            };
            if let Err(e) = kept {
                warn!("failed to cache `{remote}`: {e:?}");
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(key) = cache_key
            .as_ref()
            .filter(|_| !no_cache && self.cache_path().is_some())
        {
            if cacheable(key) {
                if let Err(error) = self.cache_bytes(key.name(), &data).await {
                    warn!("failed to cache `{remote}`: {error}");
                }
            }
        }

        debug!("[{token:?}]: completed remote url: `{remote}`");
        ipfs_io_read_state.send_success(data.len());
        Ok(data)
    }

    /// Atomically publish bytes under a native cache key. Each writer owns its staging file.
    pub async fn cache_bytes(&self, key: &str, data: &[u8]) -> anyhow::Result<()> {
        let Some(root) = self.cache_path() else {
            return Ok(());
        };
        static NEXT_CACHE_PART: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_CACHE_PART.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let (Some(path), Some(destination)) = (
            root.join_relative(format!("{key}.part-{pid}-{sequence}")),
            root.join_relative(key),
        ) else {
            anyhow::bail!("refusing to cache under `{key}`");
        };
        let file = async_fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?;
        let mut staging = CacheTemp {
            path,
            published: false,
        };
        // On cancellation the file handle must close before the guard removes the staging file.
        let mut file = file;
        file.write_all(data).await?;
        file.sync_all().await?;
        drop(file);
        async_fs::rename(&staging.path, destination).await?;
        staging.published = true;
        Ok(())
    }
}

struct CacheTemp {
    path: PathBuf,
    published: bool,
}

impl Drop for CacheTemp {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Default)]
pub(super) struct RequestMetrics {
    pub(super) attempts: AtomicU64,
    pub(super) bytes: AtomicU64,
    pub(super) duration_ms: AtomicU64,
    pub(super) retries: AtomicU64,
    pub(super) status_failures: AtomicU64,
    pub(super) cancelled: AtomicU64,
}

fn add_saturating(counter: &AtomicU64, value: u64) {
    counter.update(Ordering::Relaxed, Ordering::Relaxed, |old| {
        old.saturating_add(value)
    });
}

struct RequestMeasurement<'a> {
    metrics: &'a RequestMetrics,
    started: Instant,
    completed: bool,
}
impl Drop for RequestMeasurement<'_> {
    fn drop(&mut self) {
        add_saturating(
            &self.metrics.duration_ms,
            self.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        );
        if !self.completed {
            add_saturating(&self.metrics.cancelled, 1);
        }
    }
}

impl RequestMetrics {
    pub(super) fn counters(&self) -> [(DiagnosticPath, &AtomicU64); 6] {
        [
            (
                const { DiagnosticPath::const_new("IPFS_REQUEST_ATTEMPTS") },
                &self.attempts,
            ),
            (
                const { DiagnosticPath::const_new("IPFS_REQUEST_BYTES") },
                &self.bytes,
            ),
            (
                const { DiagnosticPath::const_new("IPFS_REQUEST_DURATION_MS") },
                &self.duration_ms,
            ),
            (
                const { DiagnosticPath::const_new("IPFS_REQUEST_RETRIES") },
                &self.retries,
            ),
            (
                const { DiagnosticPath::const_new("IPFS_REQUEST_STATUS_FAILURES") },
                &self.status_failures,
            ),
            (
                const { DiagnosticPath::const_new("IPFS_REQUEST_CANCELLED") },
                &self.cancelled,
            ),
        ]
    }

    pub(super) async fn measure<T>(&self, operation: impl std::future::Future<Output = T>) -> T {
        let mut measurement = RequestMeasurement {
            metrics: self,
            started: Instant::now(),
            completed: false,
        };
        let result = operation.await;
        measurement.completed = true;
        result
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "download_tests.rs"]
mod tests;

//! Native Bevy asset reads selected by public realm, content endpoint and immutable CID.
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use bevy::{
    asset::io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader},
    prelude::{warn, Resource},
};
use common::util::JoinRelativeExt;

use super::{
    content_hash::matches_content_hash,
    ipfs_path::{CacheKey, IpfsPath},
    PassThroughReader, RealmConfig,
};

const MAX_ITEM: u64 = 16 * 1024 * 1024;
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const MAX_BLOBS: usize = 4096;
const MAX_EVENTS: usize = 16384;
const MAX_JOURNAL: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Capture,
    Replay,
}

pub struct RecordingOptions {
    about_urls: Vec<String>,
}

impl RecordingOptions {
    pub fn for_domain(domain: &str) -> Self {
        let peers = [
            format!("https://peer.{domain}"),
            format!("https://peer-ap1.{domain}"),
        ];
        let mut about_urls: Vec<_> = peers.iter().map(|peer| format!("{peer}/about")).collect();
        about_urls.push(format!("https://realm-provider-ea.{domain}/main/about"));
        Self { about_urls }
    }

    fn public_realm(&self, realm: Option<&RealmConfig>) -> bool {
        let Some(realm) = realm else { return false };
        let trusted_about = |about: &str| {
            self.about_urls
                .iter()
                .any(|url| url == about.trim_end_matches('/'))
        };
        if !realm.connected
            || realm.secret.is_some()
            || !realm.advertised_by.as_deref().is_some_and(trusted_about)
            || !trusted_about(&realm.about_url)
        {
            return false;
        }
        let Some(content) = realm
            .about
            .content_url()
            .and_then(|url| url::Url::parse(url).ok())
        else {
            return false;
        };
        if !content.username().is_empty()
            || content.password().is_some()
            || content.query().is_some()
            || content.fragment().is_some()
            || content.path().trim_end_matches('/') != "/content"
        {
            return false;
        }
        // A trusted realm can serve its own content or advertise a different public Catalyst.
        if url::Url::parse(&realm.about_url).is_ok_and(|about| about.origin() == content.origin()) {
            return true;
        }
        content.scheme() == "https"
            && match content.host() {
                Some(url::Host::Domain(host)) => {
                    let host = host.trim_end_matches('.');
                    host.contains('.')
                        && !host.ends_with(".localhost")
                        && !host.ends_with(".local")
                        && !host.ends_with(".internal")
                }
                Some(url::Host::Ipv4(ip)) => !common::util::is_forbidden_ip(&ip.into()),
                Some(url::Host::Ipv6(ip)) => !common::util::is_forbidden_ip(&ip.into()),
                None => false,
            }
    }

    fn public_hash(
        &self,
        realm: Option<&RealmConfig>,
        path: &IpfsPath,
        context: &super::IpfsContext,
    ) -> Option<String> {
        let realm = realm?;
        if !self.public_realm(Some(realm))
            || context.about.as_ref()?.content_url()?.trim_end_matches('/')
                != realm.about.content_url()?.trim_end_matches('/')
            || path.base_url().is_some()
            || context
                .modifiers
                .get(path.scene_hash()?)
                .is_some_and(|modifier| modifier.base_url.is_some())
        {
            return None;
        }
        match path.cache_key(context)? {
            CacheKey::Content(hash) if valid_hash(&hash) => Some(hash),
            _ => None,
        }
    }
}

pub(super) struct RecordingReader {
    inner: PassThroughReader,
    recording: Arc<Recording>,
    diagnostic_realm: std::sync::Mutex<tokio::sync::watch::Receiver<Option<RealmConfig>>>,
}

impl RecordingReader {
    pub(super) fn new(inner: PassThroughReader, recording: Arc<Recording>) -> Self {
        let mut realm = inner.inner.realm_config_receiver.clone();
        realm.mark_changed();
        Self {
            inner,
            recording,
            diagnostic_realm: std::sync::Mutex::new(realm),
        }
    }
}

impl AssetReader for RecordingReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let io = &self.inner.inner;
        let Some(asset) = IpfsPath::new_from_path(path)
            .ok()
            .flatten()
            .filter(|p| !io.is_preview && p.scene_hash().is_some() && p.base_url().is_none())
        else {
            return io.read(path).await;
        };
        // A cold hash read can arrive before /about. Resolve the realm before deciding
        // whether replay may contact the ordinary reader.
        let _ = io.connected().await;
        {
            let mut realm = self.diagnostic_realm.lock().unwrap();
            if realm.has_changed().unwrap_or(false)
                && !self
                    .recording
                    .options
                    .public_realm(realm.borrow_and_update().as_ref())
            {
                warn!("asset recording is inactive in this realm; reads use the ordinary cache and network");
            }
        }
        let mut realm = io.realm_config_receiver.clone();
        let select = |context: &super::IpfsContext, realm: &Option<RealmConfig>| {
            self.recording
                .options
                .public_hash(realm.as_ref(), &asset, context)
        };
        let hash = select(&*io.context.read().await, &realm.borrow_and_update());
        let Some(hash) = hash else {
            return io.read(path).await;
        };
        match &self.recording.store {
            Store::Replay(entries) => {
                let len = *entries
                    .get(&hash)
                    .ok_or_else(|| AssetReaderError::NotFound(path.to_owned()))?;
                let directory = self.recording.directory.clone();
                let bytes =
                    async_std::task::spawn_blocking(move || read_verified(&directory, &hash, len))
                        .await?;
                Ok(Box::new(VecReader::new(bytes)) as Box<dyn Reader>)
            }
            Store::Capture(writer) => {
                let mut reader = io.read(path).await?;
                let current = select(&*io.context.read().await, &realm.borrow());
                if realm.has_changed().unwrap_or(true) || current.as_ref() != Some(&hash) {
                    return Ok(reader);
                }
                let mut writer = writer.clone().lock_owned().await;
                if writer.recorded.contains(&hash) {
                    return Ok(reader);
                }
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).await?;
                drop(reader);
                if bytes.len() as u64 <= MAX_ITEM {
                    bytes = async_std::task::spawn_blocking(move || {
                        if let Err(error) = writer.record(&hash, &bytes) {
                            warn!("asset read was not recorded: {error}");
                        }
                        bytes
                    })
                    .await;
                } else {
                    warn!(
                        "asset {hash} exceeds the {MAX_ITEM}-byte recording limit and was skipped"
                    );
                }
                Ok(Box::new(VecReader::new(bytes)) as Box<dyn Reader>)
            }
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        AssetReader::read_meta(&self.inner, path).await
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        AssetReader::read_directory(&self.inner, path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        AssetReader::is_directory(&self.inner, path).await
    }
}

#[derive(Resource)]
pub struct Recording {
    options: RecordingOptions,
    directory: PathBuf,
    store: Store,
}

enum Store {
    Replay(HashMap<String, u64>),
    Capture(Arc<tokio::sync::Mutex<Writer>>),
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn valid_hash(hash: &str) -> bool {
    (hash.len() == 46 && hash.starts_with("Qm") && hash.bytes().all(|c| c.is_ascii_alphanumeric()))
        || (hash.len() == 59
            && hash.starts_with("baf")
            && hash
                .bytes()
                .all(|c| c.is_ascii_lowercase() || (b'2'..=b'7').contains(&c)))
}

fn blob_path(directory: &Path, hash: &str) -> io::Result<PathBuf> {
    directory
        .join_relative(hash)
        .filter(|_| valid_hash(hash))
        .ok_or_else(|| invalid("invalid recording key"))
}

fn read_verified(directory: &Path, hash: &str, len: u64) -> io::Result<Vec<u8>> {
    let path = blob_path(directory, hash)?;
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.len() != len || len > MAX_ITEM {
        return Err(invalid("invalid recording blob"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(len + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != len || !matches_content_hash(hash, &bytes) {
        return Err(invalid("recording content mismatch"));
    }
    Ok(bytes)
}

/// Reject cache aliases and nesting, resolving existing symlinks/junctions even when the
/// final directory has not been created yet.
fn validate_separation(directory: &Path, cache: &Path) -> std::io::Result<()> {
    fn resolved(path: &Path) -> std::io::Result<PathBuf> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let mut current = absolute.as_path();
        let mut missing = Vec::new();
        loop {
            match std::fs::canonicalize(current) {
                Ok(mut root) => {
                    for component in missing.iter().rev() {
                        root.push(component);
                    }
                    return Ok(root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let name = current.file_name().ok_or(error)?;
                    missing.push(name.to_os_string());
                    current = current
                        .parent()
                        .ok_or_else(|| invalid("invalid recording path"))?;
                }
                Err(error) => return Err(error),
            }
        }
    }
    let directory = resolved(directory)?;
    let cache = resolved(cache)?;
    #[cfg(windows)]
    let contains = |parent: &Path, child: &Path| {
        child.ancestors().any(|ancestor| {
            ancestor
                .as_os_str()
                .eq_ignore_ascii_case(parent.as_os_str())
        })
    };
    #[cfg(not(windows))]
    let contains = |parent: &Path, child: &Path| child.starts_with(parent);
    if contains(&cache, &directory) || contains(&directory, &cache) {
        return Err(invalid("recording and cache directories overlap"));
    }
    Ok(())
}

struct Writer {
    directory: PathBuf,
    _lock: File,
    journal: File,
    journal_bytes: u64,
    events: usize,
    blobs: HashMap<String, u64>,
    recorded: HashSet<String>,
    bytes: u64,
}

impl Writer {
    fn record(&mut self, hash: &str, bytes: &[u8]) -> io::Result<()> {
        if self.recorded.contains(hash) {
            return Ok(());
        }
        let len = bytes.len() as u64;
        let line = format!("{hash} {len} read\n");
        if self.events >= MAX_EVENTS || self.journal_bytes + line.len() as u64 > MAX_JOURNAL {
            return Err(invalid("recording journal is full"));
        }
        if len > MAX_ITEM || !matches_content_hash(hash, bytes) {
            return Err(invalid("recording content mismatch"));
        }
        if self.blobs.contains_key(hash) {
            read_verified(&self.directory, hash, len)?;
        } else {
            if self.blobs.len() >= MAX_BLOBS || self.bytes + len > MAX_TOTAL {
                return Err(invalid("recording exceeds limits"));
            }
            let path = blob_path(&self.directory, hash)?;
            install(&self.directory, ".blob.tmp", &path, bytes)?;
            self.blobs.insert(hash.to_owned(), len);
            self.bytes += len;
        }
        // A failed/partial append may still extend the file. Restore the committed
        // boundary and write cursor before retrying under the exclusive writer lock.
        if self.journal.metadata()?.len() != self.journal_bytes {
            self.journal.set_len(self.journal_bytes)?;
            self.journal.seek(SeekFrom::Start(self.journal_bytes))?;
        }
        self.journal.write_all(line.as_bytes())?;
        self.journal.sync_data()?;
        self.journal_bytes += line.len() as u64;
        self.events += 1;
        self.recorded.insert(hash.to_owned());
        Ok(())
    }
}

// Only the lock holder touches these two staging names. Publishing the blob
// precedes its journal entry, so interruption can leave an unreferenced blob.
fn install(directory: &Path, stage: &str, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let staging = directory.join(stage);
    let mut file = File::create_new(&staging)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&staging, path)?;
        #[cfg(unix)]
        File::open(directory)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result
}

impl Recording {
    pub fn open(
        directory: &Path,
        cache: &Path,
        mode: Mode,
        options: RecordingOptions,
    ) -> io::Result<Self> {
        validate_separation(directory, cache)?;
        if mode == Mode::Capture {
            fs::create_dir_all(directory)?;
        }
        let directory = fs::canonicalize(directory)?;
        let lock = if mode == Mode::Capture {
            let path = directory.join(".writer.lock");
            if fs::symlink_metadata(&path).is_ok_and(|meta| !meta.is_file() || meta.len() != 0) {
                return Err(invalid("invalid recording lock"));
            }
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)?;
            file.try_lock()
                .map_err(|_| invalid("recording already has a writer"))?;
            Some(file)
        } else {
            None
        };

        let mut blobs = HashMap::new();
        let mut bytes = 0u64;
        let mut journal = Vec::new();
        let mut staging = Vec::new();
        let journal_path = directory.join("reads.log");
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            let meta = fs::symlink_metadata(&path)?;
            if !meta.is_file() {
                return Err(invalid(&format!(
                    "invalid recording entry {:?}",
                    path.file_name().unwrap_or_default()
                )));
            }
            match path.file_name().and_then(|name| name.to_str()) {
                Some(".writer.lock") if meta.len() == 0 => {}
                Some(".DS_Store" | "Thumbs.db" | "desktop.ini") => {}
                Some(".blob.tmp") if meta.len() <= MAX_ITEM => staging.push(path),
                Some(".reads.tmp") if meta.len() <= MAX_JOURNAL => staging.push(path),
                Some("reads.log") if meta.len() <= MAX_JOURNAL => {
                    File::open(path)?
                        .take(MAX_JOURNAL + 1)
                        .read_to_end(&mut journal)?;
                    if journal.len() as u64 > MAX_JOURNAL {
                        return Err(invalid("recording journal too large"));
                    }
                }
                Some(hash) if valid_hash(hash) && meta.len() <= MAX_ITEM => {
                    bytes += meta.len();
                    blobs.insert(hash.to_owned(), meta.len());
                    if blobs.len() > MAX_BLOBS || bytes > MAX_TOTAL {
                        return Err(invalid("recording exceeds limits"));
                    }
                }
                _ => {
                    return Err(invalid(&format!(
                        "unexpected recording entry {:?}",
                        path.file_name().unwrap_or_default()
                    )))
                }
            }
        }
        let complete = journal
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |i| i + 1);
        journal.truncate(complete);
        let log =
            std::str::from_utf8(&journal).map_err(|_| invalid("invalid recording journal"))?;
        let mut entries = HashMap::new();
        let mut events = 0;
        let mut unique = Vec::new();
        for line in log.lines() {
            let mut fields = line.split(' ');
            let hash = fields.next().unwrap_or_default();
            let len = fields
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| invalid("invalid recording length"))?;
            if !matches!(fields.next(), Some("network" | "read"))
                || fields.next().is_some()
                || blobs.get(hash) != Some(&len)
                || events >= MAX_EVENTS
            {
                return Err(invalid("invalid recording journal entry"));
            }
            if entries.insert(hash.to_owned(), len).is_none() && mode == Mode::Capture {
                unique.extend_from_slice(line.as_bytes());
                unique.push(b'\n');
            }
            events += 1;
        }
        let store = if let Some(lock) = lock {
            for path in staging {
                fs::remove_file(path)?;
            }
            // Detach an imported/hard-linked journal before appending, and discard
            // its torn tail. This single bounded copy is made only when opening a writer.
            install(&directory, ".reads.tmp", &journal_path, &unique)?;
            let mut handle = OpenOptions::new().write(true).open(journal_path)?;
            handle.seek(SeekFrom::End(0))?;
            let writer = Writer {
                directory: directory.clone(),
                _lock: lock,
                journal: handle,
                journal_bytes: unique.len() as u64,
                events: entries.len(),
                blobs,
                bytes,
                recorded: entries.into_keys().collect(),
            };
            Store::Capture(Arc::new(tokio::sync::Mutex::new(writer)))
        } else {
            Store::Replay(entries)
        };
        Ok(Self {
            options,
            directory,
            store,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipfs_path::{IpfsKey, IpfsType},
        EndpointConfig, IpfsContext, IpfsIo, IpfsModifier, ServerAbout,
    };
    use bevy::asset::io::file::FileAssetReader;

    const EMPTY: &str = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku";
    const OTHER: &str = "QmdfTbBqBPQ7VNxZEYEj14VmRuZBkqFbiwReogJgS1zR1n";

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            loop {
                let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let dir = std::env::temp_dir()
                    .join(format!("dcl-asset-recording-{}-{n}", std::process::id()));
                match fs::create_dir(&dir) {
                    Ok(()) => return Self(dir),
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("{e}"),
                }
            }
        }
        fn open(&self, mode: Mode) -> io::Result<Recording> {
            Recording::open(
                &self.0.join("capture"),
                &self.0.join("cache"),
                mode,
                RecordingOptions::for_domain("example.test"),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn entity(hash: &str) -> IpfsPath {
        IpfsPath::new(IpfsType::Entity {
            hash: hash.into(),
            ext: "json".into(),
        })
    }
    fn public_realm() -> RealmConfig {
        RealmConfig {
            about_url: "https://realm-provider-ea.example.test/main/about".into(),
            connected: true,
            advertised_by: Some("https://realm-provider-ea.example.test/main/about".into()),
            secret: None,
            address: String::new(),
            about: ServerAbout {
                content: Some(EndpointConfig {
                    healthy: true,
                    public_url: "https://catalyst.operator.example/content/".into(),
                }),
                ..Default::default()
            },
        }
    }

    #[test]
    fn only_connected_public_content_without_overrides_qualifies() {
        let options = RecordingOptions::for_domain("example.test");
        let realm = public_realm();
        let mut ctx = IpfsContext {
            about: Some(realm.about.clone()),
            ..Default::default()
        };
        let path = entity(EMPTY);
        assert_eq!(
            options.public_hash(Some(&realm), &path, &ctx).as_deref(),
            Some(EMPTY)
        );
        assert!(options.public_hash(None, &path, &ctx).is_none());
        // Both layouts occur in Genesis: a regional host under the base domain, and
        // a Catalyst operator on a separate domain. /about can include a final slash.
        let mut regional = public_realm();
        regional.about_url = "https://peer.example.test/about/".into();
        regional.advertised_by = Some(regional.about_url.clone());
        regional.about.content.as_mut().unwrap().public_url =
            "https://peer-ec1.example.test/content/".into();
        ctx.about = Some(regional.about.clone());
        ctx.about.as_mut().unwrap().trim_public_urls();
        assert_eq!(
            options.public_hash(Some(&regional), &path, &ctx).as_deref(),
            Some(EMPTY)
        );
        ctx.about = Some(realm.about.clone());
        for changed in [
            RealmConfig {
                advertised_by: None,
                ..public_realm()
            },
            RealmConfig {
                connected: false,
                ..public_realm()
            },
            RealmConfig {
                advertised_by: Some("https://user:secret@peer.example.test/about".into()),
                ..public_realm()
            },
            RealmConfig {
                secret: Some("secret".into()),
                ..public_realm()
            },
            RealmConfig {
                about_url: "https://worlds.example.test/about".into(),
                ..public_realm()
            },
            RealmConfig {
                about_url: format!("{}?token=value", realm.about_url),
                ..public_realm()
            },
        ] {
            assert!(options.public_hash(Some(&changed), &path, &ctx).is_none());
        }
        assert!(options
            .public_hash(
                Some(&realm),
                &IpfsPath::new_from_url("https://peer-ap1.example.test/content", "png"),
                &ctx
            )
            .is_none());
        let custom = entity(EMPTY).with_keyvalue(
            IpfsKey::BaseUrl,
            "https://peer-ap1.example.test/content/contents/".into(),
        );
        assert!(options.public_hash(Some(&realm), &custom, &ctx).is_none());
        ctx.modifiers.insert(
            EMPTY.into(),
            IpfsModifier {
                base_url: Some("https://peer-ap1.example.test/content/contents/".into()),
            },
        );
        assert!(options.public_hash(Some(&realm), &path, &ctx).is_none());
        ctx.modifiers.clear();
        for base in [
            "https://peer-ap1.example.test/content?token=value",
            "https://user@peer-ap1.example.test/content",
            "https://peer.example.test/private",
            "https://127.0.0.1/content",
            "https://[::1]/content",
            "https://service.localhost/content",
            "http://operator.example/content",
            "https://operator.example/content#secret",
        ] {
            let mut changed = public_realm();
            changed.about.content.as_mut().unwrap().public_url = base.into();
            ctx.about = Some(changed.about.clone());
            assert!(
                options.public_hash(Some(&changed), &path, &ctx).is_none(),
                "{base}"
            );
        }
        ctx.about = Some(regional.about);
        assert!(options.public_hash(Some(&realm), &path, &ctx).is_none());
    }

    #[tokio::test]
    async fn records_warm_and_network_reads_and_replays_only_committed_blobs() {
        use std::net::TcpListener;
        bevy::tasks::IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let fixture = Fixture::new();
        let cache = fixture.0.join("cache");
        let capture = fixture.0.join("capture");
        fs::create_dir(&cache).unwrap();
        fs::write(cache.join(EMPTY), b"").unwrap();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.local_addr().unwrap());
        let fixture_realm = || {
            let mut realm = public_realm();
            realm.about_url = format!("{base}/about");
            realm.advertised_by = Some(realm.about_url.clone());
            realm.about.content.as_mut().unwrap().public_url = format!("{base}/content");
            realm
        };
        let make = |mode| {
            let io = Arc::new(IpfsIo::new(
                false,
                Box::new(FileAssetReader::new(&cache)),
                Some(cache.clone()),
                Default::default(),
                2,
                #[cfg(feature = "ipfs_debug")]
                tokio::sync::mpsc::unbounded_channel().0,
            ));
            let realm = fixture_realm();
            io.context.try_write().unwrap().about = Some(realm.about.clone());
            io.realm_config_sender.send(Some(realm)).unwrap();
            let options = RecordingOptions {
                about_urls: vec![format!("{base}/about")],
            };
            RecordingReader::new(
                PassThroughReader { inner: io },
                Arc::new(Recording::open(&capture, &cache, mode, options).unwrap()),
            )
        };
        let recorder = make(Mode::Capture);
        let warm = PathBuf::from(&entity(EMPTY));
        let cold = PathBuf::from(&entity(OTHER));
        let origin = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            use std::io::BufRead;
            let mut line = String::new();
            io::BufReader::new(&stream).read_line(&mut line).unwrap();
            assert!(line.starts_with(&format!("GET /content/contents/{OTHER} ")));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
        });
        let (first, second) = tokio::join!(recorder.read(&warm), recorder.read(&cold));
        for mut reader in [first.unwrap(), second.unwrap()] {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await.unwrap();
            assert!(bytes.is_empty());
        }
        drop(recorder);
        origin.join().unwrap();
        assert_eq!(
            fs::read_to_string(capture.join("reads.log"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        // Valid bytes at a key with no completed journal event must never become a replay hit.
        let orphan = "bafkreicojquuwmy7piqjti3zx3buxh47ya64i2vumxmzr5gwqpnfgsd6nu";
        let bytes: Vec<_> = (0..1000).map(|i| (i % 251) as u8).collect();
        fs::write(capture.join(orphan), &bytes).unwrap();
        fs::write(cache.join(orphan), &bytes).unwrap();
        fs::write(cache.join(EMPTY), b"ordinary reader").unwrap();
        let replay = make(Mode::Replay);
        let mut output = Vec::new();
        replay
            .read(&warm)
            .await
            .unwrap()
            .read_to_end(&mut output)
            .await
            .unwrap();
        assert!(output.is_empty());
        assert!(replay.read(&PathBuf::from(&entity(orphan))).await.is_err());
        // A secret realm with the same CID uses the ordinary reader.
        let mut private = fixture_realm();
        private.secret = Some("secret".into());
        replay
            .inner
            .inner
            .realm_config_sender
            .send(Some(private))
            .unwrap();
        replay
            .read(&warm)
            .await
            .unwrap()
            .read_to_end(&mut output)
            .await
            .unwrap();
        assert_eq!(output, b"ordinary reader");
    }

    #[test]
    fn resumes_legacy_journal_repairs_torn_tails_and_excludes_second_writer() {
        let fixture = Fixture::new();
        let dir = fixture.0.join("capture");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(EMPTY), b"").unwrap();
        let original = format!("{EMPTY} 0 network\ntorn");
        let linked = fixture.0.join("imported.log");
        fs::write(&linked, &original).unwrap();
        fs::hard_link(&linked, dir.join("reads.log")).unwrap();
        fs::write(dir.join(".blob.tmp"), b"incomplete").unwrap();
        let rec = fixture.open(Mode::Capture).unwrap();
        assert!(fixture.open(Mode::Capture).is_err());
        let Store::Capture(writer) = &rec.store else {
            panic!()
        };
        let mut opened_log = File::open(dir.join("reads.log")).unwrap();
        let mut writer = writer.blocking_lock();
        writer.record(EMPTY, b"").unwrap();
        // Failed append before newline, followed by another read in the same run.
        writer.journal.write_all(b"partial").unwrap();
        writer.record(OTHER, b"").unwrap();
        writer.record(OTHER, b"").unwrap();
        drop(writer);
        drop(rec);
        let log = fs::read_to_string(dir.join("reads.log")).unwrap();
        assert_eq!(log, format!("{EMPTY} 0 network\n{OTHER} 0 read\n"));
        let mut appended = String::new();
        opened_log.read_to_string(&mut appended).unwrap();
        assert_eq!(appended, log, "each event appends to the same open journal");
        assert_eq!(fs::read_to_string(linked).unwrap(), original);
        assert!(!dir.join(".blob.tmp").exists());
        fixture.open(Mode::Replay).unwrap();
    }

    #[test]
    fn rejects_tampering_limits_and_overlapping_directories() {
        let fixture = Fixture::new();
        let dir = fixture.0.join("capture");
        assert!(validate_separation(&dir, &dir).is_err());
        assert!(validate_separation(&dir.join("nested"), &dir).is_err());
        assert!(validate_separation(&dir, &dir.join("nested")).is_err());
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(EMPTY), b"").unwrap();
        let log = dir.join("reads.log");
        fs::write(&log, format!("{EMPTY} 0 read\n").repeat(MAX_EVENTS)).unwrap();
        let rec = fixture.open(Mode::Capture).unwrap();
        let Store::Capture(writer) = &rec.store else {
            panic!()
        };
        writer.blocking_lock().record(OTHER, b"").unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 2);
        assert!(dir.join(OTHER).exists());
        drop(rec);
        for name in [".DS_Store", "Thumbs.db", "desktop.ini"] {
            fs::write(dir.join(name), b"metadata").unwrap();
        }
        fixture.open(Mode::Replay).unwrap();
        fs::write(dir.join("unexpected.txt"), b"").unwrap();
        assert!(fixture
            .open(Mode::Replay)
            .err()
            .unwrap()
            .to_string()
            .contains("unexpected.txt"));
        fs::remove_file(dir.join("unexpected.txt")).unwrap();
        fs::write(&log, b"broken\n").unwrap();
        assert!(fixture.open(Mode::Replay).is_err());
        fs::write(&log, format!("{EMPTY} 0 read\n")).unwrap();
        fs::write(dir.join(EMPTY), b"invalid").unwrap();
        assert!(fixture.open(Mode::Replay).is_err());
        assert!(read_verified(&dir, EMPTY, 7).is_err());
        assert!(blob_path(&dir, "../other").is_err());
        File::create(dir.join(OTHER))
            .unwrap()
            .set_len(MAX_ITEM + 1)
            .unwrap();
        assert!(fixture.open(Mode::Capture).is_err());
    }
}

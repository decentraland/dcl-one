use super::*;
use crate::wait_for_staging_cleanup;

const HASH: &str = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776";
const OTHER: &str = "bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa";

async fn fresh(tag: &str) -> (PathBuf, Storage) {
    let tmp = std::env::temp_dir().join(format!("catalyst-storage-{tag}-{}", std::process::id()));
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    let storage = Storage::from_config(&tmp).await.unwrap();
    (tmp, storage)
}

fn shard_dir_of(storage: &Storage, hash: &str) -> PathBuf {
    resolve_file_path(storage.root(), hash)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn strings(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[cfg(unix)]
fn running_as_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[tokio::test]
async fn store_retrieve_delete_roundtrip() {
    let (tmp, storage) = fresh("rt").await;
    let data = Bytes::from_static(b"hello decentraland");

    storage.store(HASH, data.clone()).await.unwrap();
    assert!(storage.exist(HASH).await.unwrap());
    assert_eq!(storage.retrieve(HASH).await.unwrap().unwrap(), data);
    assert_eq!(
        storage.file_info(HASH).await.unwrap().unwrap().size,
        data.len() as u64
    );

    storage.delete(HASH).await.unwrap();
    assert!(!storage.exist(HASH).await.unwrap());
    assert!(storage.retrieve(HASH).await.unwrap().is_none());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn root_layout_matches_the_nginx_alias() {
    let (tmp, storage) = fresh("layout").await;
    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    assert!(
        tmp.join("contents")
            .join(crate::hex_prefix(HASH))
            .join(HASH)
            .is_file()
    );
    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn delete_missing_is_silent_and_delete_strict_on_missing_is_ok() {
    let (tmp, storage) = fresh("delmiss").await;
    storage.delete(HASH).await.unwrap();
    storage.delete_strict(HASH).await.unwrap();
    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn invalid_id_is_rejected_by_point_lookups_and_a_miss_for_open() {
    let (tmp, storage) = fresh("bad").await;

    match storage.exist("../etc/passwd").await {
        Err(StorageError::InvalidId(_)) => {}
        other => panic!("expected InvalidId, got {:?}", other),
    }
    match storage.store("Qm\0evil", Bytes::from_static(b"")).await {
        Err(StorageError::InvalidId(_)) => {}
        other => panic!("expected InvalidId, got {:?}", other),
    }
    assert!(storage.open("../etc/passwd").await.unwrap().is_none());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn exist_multiple_invalid_id_is_a_per_id_miss() {
    let (tmp, storage) = fresh("batch").await;
    let over_long = format!("ba{}", "a".repeat(200));
    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();

    let results = storage
        .exist_multiple(&strings(&[&over_long, HASH, "../etc/passwd", OTHER]))
        .await
        .unwrap();
    assert_eq!(results.len(), 4);
    assert!(!results[&over_long]);
    assert!(results[HASH]);
    assert!(!results["../etc/passwd"]);
    assert!(!results[OTHER]);

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn retrieve_ignores_gzip_companion() {
    let (tmp, storage) = fresh("gzip").await;
    let raw_data = Bytes::from_static(b"raw content");
    storage.store(HASH, raw_data.clone()).await.unwrap();

    let raw_path = resolve_file_path(storage.root(), HASH).unwrap();
    let gzip_path = PathBuf::from(format!("{}.gzip", raw_path.display()));
    tokio::fs::write(&gzip_path, b"attacker-planted gzip content")
        .await
        .unwrap();

    assert_eq!(
        storage.retrieve(HASH).await.unwrap().unwrap(),
        raw_data,
        "retrieve() must NOT prefer a `.gzip` sibling (unverified)"
    );
    assert_eq!(
        storage.file_info(HASH).await.unwrap().unwrap().size,
        raw_data.len() as u64
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[cfg(unix)]
#[tokio::test]
async fn read_probes_report_faults_not_misses() {
    if running_as_root() {
        eprintln!("skipping: permissions do not bind when running as root");
        return;
    }
    let (tmp, storage) = fresh("fault").await;
    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();

    let shard_dir = shard_dir_of(&storage, HASH);
    set_mode(&shard_dir, 0o000);

    assert!(matches!(
        storage.exist(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.retrieve(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.file_info(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.read_range(HASH, 0, 0).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(storage.open(HASH).await, Err(StorageError::Io(_))));

    set_mode(&shard_dir, 0o755);
    assert!(storage.exist(HASH).await.unwrap());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn non_regular_file_at_content_path_is_a_fault() {
    let (tmp, storage) = fresh("squat").await;
    let path = resolve_file_path(storage.root(), HASH).unwrap();
    tokio::fs::create_dir_all(&path).await.unwrap();

    assert!(matches!(
        storage.exist(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.retrieve(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.file_info(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(storage.open(HASH).await, Err(StorageError::Io(_))));

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[cfg(unix)]
#[tokio::test]
async fn delete_strict_propagates_unlink_faults() {
    if running_as_root() {
        eprintln!("skipping: permissions do not bind when running as root");
        return;
    }
    let (tmp, storage) = fresh("delstrict").await;
    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();

    let shard_dir = shard_dir_of(&storage, HASH);
    set_mode(&shard_dir, 0o555);

    assert!(matches!(
        storage.delete_strict(HASH).await,
        Err(StorageError::Io(_))
    ));
    storage.delete(HASH).await.unwrap();
    assert!(
        storage.exist(HASH).await.unwrap(),
        "file must still be there"
    );

    set_mode(&shard_dir, 0o755);
    storage.delete_strict(HASH).await.unwrap();
    assert!(!storage.exist(HASH).await.unwrap());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn reads_of_a_never_created_shard_are_a_plain_miss_and_create_nothing() {
    let (tmp, storage) = fresh("virgin").await;
    let shard_dir = shard_dir_of(&storage, HASH);

    assert!(!storage.exist(HASH).await.unwrap());
    assert!(storage.retrieve(HASH).await.unwrap().is_none());
    assert!(storage.file_info(HASH).await.unwrap().is_none());
    assert!(storage.read_range(HASH, 0, 31).await.unwrap().is_none());
    assert!(storage.open(HASH).await.unwrap().is_none());
    storage.delete(HASH).await.unwrap();
    storage.delete_strict(HASH).await.unwrap();

    assert!(
        !shard_dir.exists(),
        "a read must not create {}",
        shard_dir.display()
    );
    let mut entries = tokio::fs::read_dir(storage.root()).await.unwrap();
    assert!(
        entries.next_entry().await.unwrap().is_none(),
        "reads must leave the storage root empty"
    );

    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    assert!(shard_dir.is_dir());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn read_range_serves_only_the_window() {
    let (tmp, storage) = fresh("range").await;
    storage
        .store(HASH, Bytes::from_static(b"0123456789"))
        .await
        .unwrap();

    assert_eq!(
        storage.read_range(HASH, 0, 3).await.unwrap().unwrap(),
        Bytes::from_static(b"0123")
    );
    assert_eq!(
        storage.read_range(HASH, 4, 6).await.unwrap().unwrap(),
        Bytes::from_static(b"456"),
        "the window starts where it was asked to, not at the beginning of the file"
    );
    assert_eq!(
        storage.read_range(HASH, 8, 4096).await.unwrap().unwrap(),
        Bytes::from_static(b"89"),
        "an end past the last byte is clamped to it"
    );
    assert_eq!(
        storage.read_range(HASH, 9, 9).await.unwrap().unwrap(),
        Bytes::from_static(b"9")
    );
    assert!(
        storage.read_range(HASH, 10, 20).await.unwrap().is_none(),
        "a window that starts past the end holds no bytes"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn open_replays_the_sniffed_head_and_streams_the_rest() {
    let (tmp, storage) = fresh("open").await;
    let mut blob = vec![0u8; 200 * 1024];
    for (i, b) in blob.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let blob = Bytes::from(blob);
    storage.store(HASH, blob.clone()).await.unwrap();

    let mut opened = storage.open(HASH).await.unwrap().unwrap();
    assert_eq!(
        opened.size,
        blob.len() as u64,
        "the size comes from the descriptor being returned"
    );
    let head = opened.read_range(0, 31).await.unwrap();
    assert_eq!(&head[..], &blob[..32]);
    let body = axum::body::to_bytes(opened.into_body(head), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        body, blob,
        "the head is replayed once and the rest follows it"
    );

    let mut opened = storage.open(HASH).await.unwrap().unwrap();
    assert_eq!(&opened.read_range(10, 19).await.unwrap()[..], &blob[10..20]);
    assert!(
        opened
            .read_range(blob.len() as u64, u64::MAX)
            .await
            .unwrap()
            .is_empty()
    );

    let body = axum::body::to_bytes(
        storage
            .open(HASH)
            .await
            .unwrap()
            .unwrap()
            .into_body(Bytes::new()),
        usize::MAX,
    )
    .await
    .unwrap();
    assert_eq!(body, blob, "with nothing consumed the whole file streams");

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn destroyed_shard_is_a_fault_not_a_miss_and_the_report_is_stable() {
    let (tmp, storage) = fresh("destroyed").await;
    let shard_dir = shard_dir_of(&storage, HASH);

    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    tokio::fs::remove_dir_all(&shard_dir).await.unwrap();

    for attempt in 1..=3 {
        assert!(
            matches!(storage.exist(HASH).await, Err(StorageError::Io(_))),
            "read {attempt} of a destroyed shard must still report the damage"
        );
    }
    assert!(matches!(
        storage.retrieve(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.file_info(HASH).await,
        Err(StorageError::Io(_))
    ));
    assert!(matches!(
        storage.read_range(HASH, 0, 3).await,
        Err(StorageError::Io(_))
    ));
    assert!(
        matches!(storage.open(HASH).await, Err(StorageError::Io(_))),
        "a destroyed shard is a fault, not a 404"
    );
    assert!(
        !shard_dir.exists(),
        "the faulting read must not have healed the shard"
    );

    storage.store(HASH, Bytes::from_static(b"y")).await.unwrap();
    assert!(storage.exist(HASH).await.unwrap());
    assert!(storage.exist(HASH).await.unwrap());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// `from_config()` created the root, so nothing else has to prove it existed. Without the rule a
/// wiped volume answers "absent" for the entire key space -- one `rm -rf` read back as an empty node
/// by every consumer that syncs from it.
#[tokio::test]
async fn a_destroyed_root_is_a_fault_not_an_empty_node() {
    let (tmp, storage) = fresh("noroot").await;

    assert!(
        !storage.exist(OTHER).await.unwrap(),
        "with the root intact it is an ordinary miss"
    );

    tokio::fs::remove_dir_all(storage.root()).await.unwrap();
    assert!(
        matches!(storage.exist(OTHER).await, Err(StorageError::Io(_))),
        "a read must not report absence out of a root that is not there"
    );
    assert!(matches!(
        storage.retrieve(OTHER).await,
        Err(StorageError::Io(_))
    ));
    assert!(
        matches!(storage.open(OTHER).await, Err(StorageError::Io(_))),
        "including the streaming one"
    );
    assert!(
        !storage.root().exists(),
        "the faulting read must not have healed the root"
    );

    storage
        .store(OTHER, Bytes::from_static(b"x"))
        .await
        .unwrap();
    assert!(storage.exist(OTHER).await.unwrap());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// Why a root gets exactly ONE instance: the record of observed shards is per-instance, so two
/// instances over one root answer the same damage differently until each has reported it.
#[tokio::test]
async fn instances_over_one_root_agree_only_when_shared() {
    let (tmp, shared) = fresh("twoinst").await;
    let shared = std::sync::Arc::new(shared);
    let reader = shared.clone();
    shared.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    tokio::fs::remove_dir_all(shard_dir_of(&shared, HASH))
        .await
        .unwrap();
    assert!(
        matches!(reader.exist(HASH).await, Err(StorageError::Io(_))),
        "a clone of the shared instance reports the same damage the writer would"
    );

    let writer = Storage::from_config(&tmp).await.unwrap();
    let stranger = Storage::from_config(&tmp).await.unwrap();
    writer.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    tokio::fs::remove_dir_all(shard_dir_of(&writer, HASH))
        .await
        .unwrap();
    assert!(matches!(writer.exist(HASH).await, Err(StorageError::Io(_))));
    assert!(
        !stranger.exist(HASH).await.unwrap(),
        "documents the divergence: an instance that observed nothing reports a plain miss"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_fifo_at_a_content_path_is_rejected_not_awaited() {
    let (tmp, storage) = fresh("fifo").await;
    let path = resolve_file_path(storage.root(), HASH).unwrap();
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();

    let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(
        unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) },
        0,
        "failed to create the test FIFO"
    );

    let verdict = tokio::time::timeout(std::time::Duration::from_secs(5), storage.open(HASH))
        .await
        .expect("open blocked on a writer-less FIFO");
    assert!(
        matches!(verdict, Err(StorageError::Io(_))),
        "a FIFO is not content: it must be a fault"
    );
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(5), storage.exist(HASH))
            .await
            .expect("exist blocked on a writer-less FIFO"),
        Err(StorageError::Io(_))
    ));

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// The answer comes from the tree, so which reader asks first cannot decide what the others are told
/// -- the alternative hands the fault to one of them and a silent 404 to the other fifteen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_destroyed_shard_is_reported_to_every_concurrent_reader() {
    let (tmp, storage) = fresh("race").await;
    let storage = std::sync::Arc::new(storage);
    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    tokio::fs::remove_dir_all(shard_dir_of(&storage, HASH))
        .await
        .unwrap();

    let readers: Vec<_> = (0..16)
        .map(|_| {
            let storage = storage.clone();
            tokio::spawn(async move { storage.exist(HASH).await })
        })
        .collect();
    for reader in readers {
        assert!(matches!(reader.await.unwrap(), Err(StorageError::Io(_))));
    }

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// The store is dropped right after it hands the staging create to its detached task: the step
/// where a guard held by the caller leaked the file the task went on to create.
#[tokio::test]
async fn cancelled_store_leaves_no_staging_file() {
    let (tmp, storage) = fresh("cancel").await;
    let tasks = tokio::runtime::Handle::current().metrics();
    assert!(
        catalyst_shared::test_wait::until(async || tasks.num_alive_tasks() == 0).await,
        "the startup sweep never finished"
    );
    let payload = Bytes::from(vec![3u8; 64 * 1024 * 1024]);
    let mut store = Box::pin(storage.store(HASH, payload));
    assert!(
        catalyst_shared::test_wait::until(async || {
            assert!(
                futures::poll!(&mut store).is_pending(),
                "the store finished before its staging create ran"
            );
            tasks.num_alive_tasks() > 0
        })
        .await,
        "the store never spawned its staging create"
    );
    drop(store);

    let leaked = wait_for_staging_cleanup(&shard_dir_of(&storage, HASH)).await;
    assert!(
        leaked.is_empty(),
        "cancellation must not leak staging files, found {leaked:?}"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

async fn collect_ids(storage: &Storage, prefix: Option<&str>) -> Vec<String> {
    let mut walk = storage.all_file_ids(prefix);
    let mut ids = Vec::new();
    while let Some(id) = walk.next().await.unwrap() {
        ids.push(id);
    }
    ids
}

#[tokio::test]
async fn all_file_ids_lists_stored_files() {
    let (tmp, storage) = fresh("list").await;
    let b = "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";
    storage.store(HASH, Bytes::from_static(b"a")).await.unwrap();
    storage.store(b, Bytes::from_static(b"b")).await.unwrap();

    let ids = collect_ids(&storage, None).await;
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&HASH.to_string()));
    assert!(ids.contains(&b.to_string()));
    assert_eq!(
        collect_ids(&storage, Some("bafkreihdw")).await,
        vec![HASH.to_string()],
        "the prefix filter applies to ids"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// Deleting the second id after the first has been pulled is the only way a test can tell the two
/// shapes apart: a walk that materialized the corpus before returning would have captured the id
/// while it existed and would still offer it.
#[tokio::test]
async fn all_file_ids_streams_rather_than_buffering() {
    let (tmp, storage) = fresh("stream").await;
    let b = "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";
    assert_ne!(
        crate::hex_prefix(HASH),
        crate::hex_prefix(b),
        "the two ids must live in different shards for the walk to reach them separately"
    );
    storage.store(HASH, Bytes::from_static(b"a")).await.unwrap();
    storage.store(b, Bytes::from_static(b"b")).await.unwrap();

    let mut walk = storage.all_file_ids(None);
    let first = walk.next().await.unwrap().expect("one id is on disk");
    let second = if first == HASH { b } else { HASH };

    storage.delete_strict(second).await.unwrap();
    assert_eq!(
        walk.next().await.unwrap(),
        None,
        "an id deleted before the walk reached it is not yielded from a buffer"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn enumeration_observes_the_shards_it_lists() {
    let (tmp, writer) = fresh("enumobs").await;
    writer.store(HASH, Bytes::from_static(b"x")).await.unwrap();

    let reader = Storage::from_config(&tmp).await.unwrap();
    assert_eq!(collect_ids(&reader, None).await, vec![HASH.to_string()]);

    tokio::fs::remove_dir_all(shard_dir_of(&reader, HASH))
        .await
        .unwrap();
    assert!(
        matches!(reader.exist(HASH).await, Err(StorageError::Io(_))),
        "a shard the walk listed and then lost is damage, not an ordinary miss"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn all_file_ids_skips_misplaced_and_non_content_entries() {
    let (tmp, storage) = fresh("misplaced").await;
    storage.store(HASH, Bytes::from_static(b"a")).await.unwrap();

    let shard = shard_dir_of(&storage, HASH);
    tokio::fs::write(shard.join(OTHER), b"b").await.unwrap();
    tokio::fs::write(shard.join(format!("{HASH}.4242.0.tmp")), b"c")
        .await
        .unwrap();
    tokio::fs::create_dir(shard.join("junkdir")).await.unwrap();

    assert_eq!(
        collect_ids(&storage, None).await,
        vec![HASH.to_string()],
        "only the id that round-trips is yielded"
    );
    assert!(
        !storage.exist(OTHER).await.unwrap(),
        "the misplaced file is indeed unreachable by id"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn file_path_follows_the_read_rules() {
    let (tmp, storage) = fresh("fpath").await;
    assert!(storage.file_path(HASH).await.unwrap().is_none());
    assert!(
        !shard_dir_of(&storage, HASH).exists(),
        "a miss creates nothing"
    );

    storage.store(HASH, Bytes::from_static(b"x")).await.unwrap();
    assert_eq!(
        storage.file_path(HASH).await.unwrap().unwrap(),
        resolve_file_path(storage.root(), HASH).unwrap()
    );

    tokio::fs::remove_dir_all(shard_dir_of(&storage, HASH))
        .await
        .unwrap();
    assert!(matches!(
        storage.file_path(HASH).await,
        Err(StorageError::Io(_))
    ));

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// The exact shape of the 2026-05 snapshot incident: content sitting under a key it does not hash
/// to. `exist` is happy -- there is a file there -- and only re-reading the bytes tells them apart.
#[tokio::test]
async fn stored_content_hash_reports_the_bytes_not_the_key() {
    let (tmp, storage) = fresh("verify").await;

    let data = Bytes::from(
        (0..200u32 * 262_144)
            .map(|i| (i % 251) as u8)
            .collect::<Vec<u8>>(),
    );
    let truth = crate::auth::hash::hash_bytes_v1(&data);
    let wrong = HASH;

    storage.store(wrong, data.clone()).await.unwrap();

    assert!(storage.exist(wrong).await.unwrap(), "the file is present");
    assert_eq!(
        storage.stored_content_hash(wrong).await.unwrap().unwrap(),
        truth,
        "verification must report what the bytes hash to, not the key they were filed under"
    );

    assert!(storage.rekey(wrong, &truth).await.unwrap());
    assert!(
        !storage.exist(wrong).await.unwrap(),
        "the wrong key must stop serving; a 404 beats bytes under a CID they contradict"
    );
    assert_eq!(storage.retrieve(&truth).await.unwrap().unwrap(), data);
    assert_eq!(
        storage.stored_content_hash(&truth).await.unwrap().unwrap(),
        truth
    );

    assert!(
        !storage.rekey(wrong, &truth).await.unwrap(),
        "re-keying what is no longer there reports false rather than inventing a move"
    );

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

#[tokio::test]
async fn stored_content_hash_of_absent_id_is_none() {
    let (tmp, storage) = fresh("verify-abs").await;
    assert!(storage.stored_content_hash(OTHER).await.unwrap().is_none());
    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

/// The moderation guard: a listed hash is a miss on every read that hands
/// out bytes or their metadata, on both halves of the projection, while the
/// presence probes keep seeing the file; the entity-aware read refuses on
/// the entity too; a listed id that is no CID is a miss, not an `InvalidId`.
#[tokio::test]
async fn a_withheld_hash_is_absent_to_every_byte_read_and_present_to_the_probes() {
    let (tmp, storage) = fresh("withheld").await;
    let data = Bytes::from_static(b"withheld bytes");
    storage.store(HASH, data.clone()).await.unwrap();
    storage.store(OTHER, data.clone()).await.unwrap();

    storage.denylist().operator_insert(HASH);
    assert!(storage.withholds(HASH) && !storage.withholds(OTHER));
    assert!(storage.retrieve(HASH).await.unwrap().is_none());
    assert!(
        storage
            .retrieve_of("bafyentity", HASH)
            .await
            .unwrap()
            .is_none()
    );
    assert!(storage.open(HASH).await.unwrap().is_none());
    assert!(storage.read_range(HASH, 0, 3).await.unwrap().is_none());
    assert!(storage.file_info(HASH).await.unwrap().is_none());
    assert!(storage.file_path(HASH).await.unwrap().is_none());
    assert!(
        storage.exist(HASH).await.unwrap(),
        "presence is bookkeeping, not bytes"
    );
    assert!(storage.exist_multiple(&[HASH, OTHER]).await.unwrap()[HASH]);
    assert!(
        storage.stored_content_hash(HASH).await.unwrap().is_some(),
        "verification reads the file"
    );
    assert_eq!(storage.retrieve(OTHER).await.unwrap().unwrap(), data);

    assert!(storage.denylist().operator_remove(HASH));
    assert_eq!(
        storage.retrieve(HASH).await.unwrap().unwrap(),
        data,
        "lifted: served again"
    );

    storage.denylist().add(OTHER);
    assert!(
        storage.retrieve(OTHER).await.unwrap().is_none(),
        "the camera reel's half withholds too"
    );
    assert!(storage.open(OTHER).await.unwrap().is_none());
    storage.denylist().remove(OTHER);
    assert_eq!(storage.retrieve(OTHER).await.unwrap().unwrap(), data);

    storage.denylist().operator_insert("bafyentity");
    assert!(
        storage
            .retrieve_of("bafyentity", HASH)
            .await
            .unwrap()
            .is_none(),
        "the entity withholds its files"
    );
    assert_eq!(
        storage
            .retrieve_of("bafyother", HASH)
            .await
            .unwrap()
            .unwrap(),
        data
    );

    storage.denylist().operator_insert("not a cid");
    assert!(storage.retrieve("not a cid").await.unwrap().is_none());

    let _ = tokio::fs::remove_dir_all(&tmp).await;
}

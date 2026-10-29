use super::test_support::*;
use super::*;
use crate::crdt_gen::TestDir;

/// The check before the state is saved (the pulls no longer wait on
/// the node between a write and the save, so it is driven directly).
#[test]
fn a_pulled_file_removed_before_the_state_is_saved_is_not_claimed() {
    let dir = TestDir::new("sync-recheck");
    // The pulls wrote a.txt to e.txt and deleted d.txt; then a.txt and
    // e.txt were removed and b.txt edited before the state was saved.
    dir.write("b.txt", "mine, edited right after");
    dir.write("c.txt", "theirs c");
    let h = |s: &str| Some(hash_bytes_v1(s.as_bytes()));
    let pulled: Vec<(String, Option<String>)> = vec![
        ("a.txt".into(), h("theirs a")),
        ("b.txt".into(), h("theirs b")),
        ("c.txt".into(), h("theirs c")),
        ("d.txt".into(), None),
        ("e.txt".into(), h("theirs e")),
    ];
    let applied: Vec<(&String, &Option<String>)> = pulled.iter().map(|(p, h)| (p, h)).collect();
    let base = map(&[
        ("a.txt", "old a"),
        ("b.txt", "old b"),
        ("c.txt", "old c"),
        ("d.txt", "old d"),
    ]);
    let mut agreed: BTreeMap<String, String> = pulled
        .iter()
        .filter_map(|(p, h)| Some((p.clone(), h.clone()?)))
        .collect();
    settle(&dir.0, &applied, &base, &mut agreed, &Listings::default()).unwrap();
    assert_eq!(
        agreed,
        BTreeMap::from([
            ("a.txt".into(), "old a".into()),
            ("b.txt".into(), "old b".into()),
            ("c.txt".into(), h("theirs c").unwrap()),
        ]),
        "the base claims only what the folder holds: the next sync compares the rest again"
    );
}

/// A pull reads each directory's listing a few times over, not once
/// per file in it: 300 files updated in one directory, 100 directories
/// made beside it.
#[tokio::test]
async fn a_pull_reads_each_listing_a_few_times() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-listings");
    for i in 0..300 {
        dir.write(&format!("assets/f{i:03}.txt"), "v1");
    }
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    for i in 0..300 {
        let path = format!("assets/f{i:03}.txt");
        node_write(&fake, &project, &path, Some(format!("v2 {i}").as_bytes()));
    }
    for i in 0..100 {
        node_write(&fake, &project, &format!("d{i:03}/x.txt"), Some(b"x"));
    }
    NAMES_READ.set(0);
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!((report.plan.pull.len(), report.kept.len()), (400, 0));
    let read = NAMES_READ.get();
    assert!(
        read < 4 * 400,
        "{read} names read from listings for a folder of 400 files"
    );
}

/// The same through `sync`: the pulls write a.txt, b.txt, c.txt and
/// e.txt and delete d.txt; then, before the state is saved, a.txt and
/// e.txt are removed, b.txt edited and d.txt made again.
#[tokio::test]
async fn a_pull_undone_before_the_state_is_saved_is_not_claimed() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-settle");
    for f in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        dir.write(f, "base");
    }
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    for f in ["a.txt", "b.txt", "c.txt", "e.txt"] {
        node_write(&fake, &project, f, Some(format!("theirs {f}").as_bytes()));
    }
    node_write(&fake, &project, "d.txt", None);
    let root = dir.0.clone();
    AFTER_PULLS.set(Some(Box::new(move || {
        std::fs::remove_file(root.join("a.txt")).unwrap();
        std::fs::remove_file(root.join("e.txt")).unwrap();
        std::fs::write(root.join("b.txt"), "mine, edited right after").unwrap();
        std::fs::write(root.join("d.txt"), "mine, made again").unwrap();
    })));
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(report.plan.pull.len(), 5);
    assert_eq!(report.kept, vec![], "every pull was written");
    let base = hash_bytes_v1(b"base");
    assert_eq!(
        load_state(&dir.0).unwrap().files,
        BTreeMap::from([
            ("a.txt".into(), base.clone()),
            ("b.txt".into(), base.clone()),
            ("c.txt".into(), hash_bytes_v1(b"theirs c.txt")),
            ("d.txt".into(), base),
        ]),
        "the base claims only what the folder still holds"
    );
}

#[tokio::test]
async fn a_node_file_named_dcl_one_never_comes_down() {
    let (node, fake) = fake_node().await;
    let first = TestDir::new("sync-dcl-one-name");
    first.write("a.txt", "one");
    let signer = key();
    let project = sync(&opts(&first, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, ".dcl-one", Some(b"a file"));
    node_write(&fake, &project, ".DCL-ONE", Some(b"a file"));
    let fresh = TestDir::new("sync-dcl-one-name-fresh");
    let command = SyncOptions {
        project: Some(project.clone()),
        ..opts(&fresh, &node)
    };
    let report = sync(&command, signer.clone()).await.unwrap();
    assert!(!report.plan.pull.contains_key(".dcl-one"));
    assert!(!report.plan.pull.contains_key(".DCL-ONE"), "in any case");
    assert_eq!(load_state(&fresh.0).unwrap().project, project);
    assert_eq!(read(&fresh, "a.txt").as_deref(), Some("one"));
    sync(&command, signer).await.unwrap();
    let files = node_files(&fake, &project);
    assert!(
        files.contains_key(".dcl-one") && files.contains_key(".DCL-ONE"),
        "they stay on the node untouched"
    );
}

/// Whether the temp directory is case-insensitive (APFS, NTFS, a
/// casefold tmpfs as TMPDIR): the tests of case-only names are
/// meaningful only there, and skipped elsewhere.
fn case_insensitive_tmp(tag: &str) -> bool {
    let probe = TestDir::new(&format!("{tag}-probe"));
    probe.write("Probe", "x");
    let folds = probe.0.join("probe").exists();
    if !folds {
        eprintln!("skipped: the temp directory is case-sensitive");
    }
    folds
}

/// Whether the node, and the folder, hold `bytes` under some path.
fn held_somewhere(fake: &Shared, project: &str, dir: &TestDir, bytes: &[u8]) -> (bool, bool) {
    let h = hash_bytes_v1(bytes);
    let node = node_files(fake, project).values().any(|v| *v == h);
    let disk = scan(&dir.0).unwrap().0.values().any(|v| *v == h);
    (node, disk)
}

/// The node renames scene.ts to Scene.ts.
#[tokio::test]
async fn a_case_only_rename_on_the_node_keeps_the_file() {
    if !case_insensitive_tmp("sync-casefold") {
        return;
    }
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-casefold");
    dir.write("scene.ts", "CONTENT");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "scene.ts", None);
    node_write(&fake, &project, "Scene.ts", Some(b"CONTENT"));
    sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    let names: Vec<String> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == "Scene.ts"),
        "the folder holds the renamed file: {names:?}"
    );
    let again = sync(&opts(&dir, &node), signer).await.unwrap();
    assert!(again.plan.push.is_empty() && again.plan.pull.is_empty());
    assert_eq!(
        node_files(&fake, &project).get("Scene.ts"),
        Some(&hash_bytes_v1(b"CONTENT")),
        "the node keeps the renamed file"
    );
}

/// `run` as the command runs it, signing with `key` from a file.
async fn run_signed(tag: &str, key: String, command: SyncOptions) -> Result<()> {
    let keyfile = TestDir::new(&format!("{tag}-key"));
    keyfile.write("key", key);
    run(SyncOptions {
        sign_key: Some(keyfile.0.join("key")),
        ..command
    })
    .await
}

/// The node renames the directory Src/ to src/ (case only) while the
/// folder's Src/ also holds a file not synced yet.
#[tokio::test]
async fn a_case_only_directory_rename_on_the_node_keeps_the_file() {
    if !case_insensitive_tmp("sync-casefold-dir") {
        return;
    }
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-casefold-dir");
    dir.write("Src/a.ts", "AAA");
    dir.write("keep.txt", "keep");
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    dir.write("Src/c.ts", "CCC");
    node_write(&fake, &project, "Src/a.ts", None);
    node_write(&fake, &project, "src/a.ts", Some(b"AAA"));
    let first = sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    assert_eq!(
        first.kept,
        vec![kept("src/a.ts", Hold::Spelling("src".into()))],
        "nothing is written into Src/ for src/"
    );
    assert!(
        !load_state(&dir.0).unwrap().files.contains_key("src/a.ts"),
        "the base never claims src/a.ts"
    );
    let second = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        second.plan.push,
        BTreeMap::new(),
        "nothing is deleted on the node"
    );
    assert_eq!(
        node_files(&fake, &project).get("src/a.ts"),
        Some(&hash_bytes_v1(b"AAA"))
    );
    let (n, d) = held_somewhere(&fake, &project, &dir, b"AAA");
    assert!(
        n || d,
        "a.ts survives on one side at least: node {n}, disk {d}"
    );
    let err = run_signed(
        "sync-casefold-dir",
        wallet.private_key_hex(),
        opts(&dir, &node),
    )
    .await
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("src/a.ts (the folder holds src under another spelling)"),
        "{err}"
    );
}

/// The node holds scene.ts and Scene.ts with equal bytes (a
/// case-sensitive client made both); a fresh case-insensitive folder
/// syncs twice.
#[tokio::test]
async fn two_node_spellings_of_one_name_are_never_deleted() {
    if !case_insensitive_tmp("sync-casefold-two") {
        return;
    }
    let (node, fake) = fake_node().await;
    let first = TestDir::new("sync-casefold-two-first");
    first.write("keep.txt", "keep");
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let project = sync(&opts(&first, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "scene.ts", Some(b"SAME"));
    node_write(&fake, &project, "Scene.ts", Some(b"SAME"));
    let fresh = TestDir::new("sync-casefold-two-fresh");
    let command = SyncOptions {
        project: Some(project.clone()),
        ..opts(&fresh, &node)
    };
    let report = sync(&command, signer.clone()).await.unwrap();
    assert_eq!(
        report.kept,
        vec![kept("scene.ts", Hold::Spelling("scene.ts".into()))],
        "one spelling comes down, the other is refused and named"
    );
    let names: Vec<String> = std::fs::read_dir(&fresh.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".ts"))
        .collect();
    assert_eq!(names, ["Scene.ts"]);
    let state = load_state(&fresh.0).unwrap().files;
    assert!(
        state.contains_key("Scene.ts") && !state.contains_key("scene.ts"),
        "the base claims only the name the folder lists"
    );
    let again = sync(&command, signer).await.unwrap();
    assert_eq!(again.plan.push, BTreeMap::new());
    let files = node_files(&fake, &project);
    assert!(
        files.contains_key("scene.ts") && files.contains_key("Scene.ts"),
        "the node keeps both: {:?}",
        files.keys().collect::<Vec<_>>()
    );
    let err = run_signed("sync-casefold-two", wallet.private_key_hex(), command)
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("scene.ts (the folder holds scene.ts under another spelling)"),
        "{err}"
    );
}

/// The state an older build saved after the sync above: it claims both
/// scene.ts and Scene.ts, which the folder holds as one.
#[tokio::test]
async fn an_old_state_claiming_two_spellings_deletes_neither() {
    if !case_insensitive_tmp("sync-casefold-old-two") {
        return;
    }
    let (node, fake) = fake_node().await;
    let first = TestDir::new("sync-casefold-old-two-first");
    first.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&first, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "scene.ts", Some(b"SAME"));
    node_write(&fake, &project, "Scene.ts", Some(b"SAME"));
    let fresh = TestDir::new("sync-casefold-old-two-fresh");
    let command = SyncOptions {
        project: Some(project.clone()),
        ..opts(&fresh, &node)
    };
    sync(&command, signer.clone()).await.unwrap();
    let mut state = load_state(&fresh.0).unwrap();
    state
        .files
        .insert("scene.ts".into(), hash_bytes_v1(b"SAME"));
    save_state(&fresh.0, &state).unwrap();

    let report = sync(&command, signer).await.unwrap();
    assert_eq!(
        report.plan.push,
        BTreeMap::new(),
        "nothing is deleted on the node"
    );
    let files = node_files(&fake, &project);
    assert!(
        files.contains_key("scene.ts") && files.contains_key("Scene.ts"),
        "the node keeps both: {:?}",
        files.keys().collect::<Vec<_>>()
    );
    assert_eq!(read(&fresh, "Scene.ts").as_deref(), Some("SAME"));
    assert!(
        !load_state(&fresh.0).unwrap().files.contains_key("scene.ts"),
        "the base drops the spelling the folder does not list"
    );
}

/// The state an older build saved after the node renamed Src/ to src/
/// beside a new Src/c.ts: it claims Src/a.ts, which the folder holds,
/// and src/a.ts.
#[tokio::test]
async fn an_old_state_claiming_two_spellings_of_a_directory_loses_no_file() {
    if !case_insensitive_tmp("sync-casefold-old-dir") {
        return;
    }
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-casefold-old-dir");
    dir.write("Src/a.ts", "AAA");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    dir.write("Src/c.ts", "CCC");
    node_write(&fake, &project, "Src/a.ts", None);
    node_write(&fake, &project, "src/a.ts", Some(b"AAA"));
    sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    dir.write("Src/a.ts", "AAA");
    let mut state = load_state(&dir.0).unwrap();
    for path in ["Src/a.ts", "src/a.ts"] {
        state.files.insert(path.into(), hash_bytes_v1(b"AAA"));
    }
    save_state(&dir.0, &state).unwrap();

    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        report.plan.push,
        BTreeMap::new(),
        "nothing is deleted on the node"
    );
    assert_eq!(
        node_files(&fake, &project).get("src/a.ts"),
        Some(&hash_bytes_v1(b"AAA"))
    );
    let (n, d) = held_somewhere(&fake, &project, &dir, b"AAA");
    assert!(
        n || d,
        "a.ts survives on one side at least: node {n}, disk {d}"
    );
}

/// The folder renames scene.ts to Scene.ts.
#[tokio::test]
async fn a_case_only_rename_here_goes_up() {
    if !case_insensitive_tmp("sync-casefold-here") {
        return;
    }
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-casefold-here");
    dir.write("scene.ts", "CONTENT");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    std::fs::remove_file(dir.0.join("scene.ts")).unwrap();
    dir.write("Scene.ts", "CONTENT");
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        report.plan.push,
        BTreeMap::from([
            ("Scene.ts".into(), Some(hash_bytes_v1(b"CONTENT"))),
            ("scene.ts".into(), None),
        ])
    );
    assert_eq!(
        node_files(&fake, &project).keys().collect::<Vec<_>>(),
        ["Scene.ts", "keep.txt"]
    );
    assert_eq!(
        load_state(&dir.0).unwrap().files.keys().collect::<Vec<_>>(),
        ["Scene.ts", "keep.txt"]
    );
}

/// A node file `.DCL-ONE` and a fresh case-insensitive folder.
#[tokio::test]
async fn a_node_file_named_dcl_one_in_another_case_never_comes_down() {
    if !case_insensitive_tmp("sync-casefold-dcl-one") {
        return;
    }
    let (node, fake) = fake_node().await;
    let first = TestDir::new("sync-casefold-dcl-one");
    first.write("a.txt", "one");
    let signer = key();
    let project = sync(&opts(&first, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, ".DCL-ONE", Some(b"a file"));
    let fresh = TestDir::new("sync-casefold-dcl-one-fresh");
    let command = SyncOptions {
        project: Some(project.clone()),
        ..opts(&fresh, &node)
    };
    let report = sync(&command, signer.clone()).await.unwrap();
    assert!(!report.plan.pull.contains_key(".DCL-ONE"));
    assert!(
        sync(&command, signer).await.is_ok(),
        "the second sync of the fresh folder works"
    );
    assert!(node_files(&fake, &project).contains_key(".DCL-ONE"));
}

/// The node renames z.ts to a.ts, and the content's download fails.
#[tokio::test]
async fn a_failed_download_changes_nothing_here() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-failed-download");
    dir.write("z.ts", "ZZZ");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "z.ts", None);
    node_write(&fake, &project, "a.ts", Some(b"ZZZ"));
    let zzz = fake
        .lock()
        .unwrap()
        .blobs
        .remove(&hash_bytes_v1(b"ZZZ"))
        .unwrap();
    assert!(sync(&opts(&dir, &node), signer.clone()).await.is_err());
    assert_eq!(
        read(&dir, "z.ts").as_deref(),
        Some("ZZZ"),
        "the folder keeps its copy"
    );
    assert_eq!(read(&dir, "a.ts"), None);
    assert_eq!(
        load_state(&dir.0).unwrap().files.get("z.ts"),
        Some(&hash_bytes_v1(b"ZZZ")),
        "and its base"
    );

    fake.lock()
        .unwrap()
        .blobs
        .insert(hash_bytes_v1(b"ZZZ"), zzz);
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(report.kept, vec![]);
    assert_eq!(
        (read(&dir, "a.ts").as_deref(), read(&dir, "z.ts")),
        (Some("ZZZ"), None),
        "once the node answers, the rename comes down"
    );
}

/// The contents of the temporaries waiting under `.dcl-one/`, as hashes.
fn staged(dir: &TestDir) -> Vec<String> {
    let Ok(names) = std::fs::read_dir(dir.0.join(".dcl-one")) else {
        return Vec::new();
    };
    names
        .map(|e| e.unwrap().path())
        .filter(|p| is_sync_temp(&p.file_name().unwrap().to_string_lossy()))
        .map(|p| hash_bytes_v1(&std::fs::read(p).unwrap()))
        .collect()
}

/// Two contents come down, one of them to two paths, while the second's
/// download waits; then two more, the second's download failing.
#[tokio::test]
async fn pulled_contents_wait_on_disk_and_go_with_the_sync() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-staged");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "a.bin", Some(b"AAA"));
    node_write(&fake, &project, "a2.bin", Some(b"AAA"));
    node_write(&fake, &project, "b.bin", Some(b"BBB"));
    let (arrived, answer) = hold(&fake, Wait::Content(hash_bytes_v1(b"BBB")));
    let meanwhile = async {
        let seen = (staged(&dir), read(&dir, "a.bin"));
        answer.send(()).unwrap();
        seen
    };
    let command = opts(&dir, &node);
    let (report, (waiting, placed)) =
        while_held(sync(&command, signer.clone()), arrived, meanwhile).await;
    report.unwrap();
    assert!(
        waiting.contains(&hash_bytes_v1(b"AAA")),
        "a.bin's content waits on disk, not in memory"
    );
    assert_eq!(placed, None, "nothing is placed before every content is in");
    assert_eq!(read(&dir, "a.bin").as_deref(), Some("AAA"));
    assert_eq!(read(&dir, "a2.bin").as_deref(), Some("AAA"));
    assert_eq!(read(&dir, "b.bin").as_deref(), Some("BBB"));
    assert_eq!(staged(&dir), Vec::<String>::new(), "no temporary is left");

    node_write(&fake, &project, "c.bin", Some(b"CCC"));
    node_write(&fake, &project, "d.bin", Some(b"DDD"));
    fake.lock().unwrap().blobs.remove(&hash_bytes_v1(b"DDD"));
    assert!(sync(&command, signer).await.is_err());
    assert_eq!(read(&dir, "c.bin"), None);
    assert_eq!(staged(&dir), Vec::<String>::new(), "nor after a failure");
    assert_eq!(
        scan(&dir.0).unwrap().0.keys().collect::<Vec<_>>(),
        ["a.bin", "a2.bin", "b.bin", "keep.txt"]
    );
}

/// A temporary a killed sync left goes with the next sync; a sync
/// started while another downloads is refused and leaves the live
/// sync's temporaries alone.
#[tokio::test]
async fn a_dead_syncs_temporaries_go_and_a_live_ones_stay() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-orphans");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    let orphan = dir.0.join(".dcl-one").join(sync_temp_name());
    std::fs::write(&orphan, "half a download").unwrap();
    node_write(&fake, &project, "a.bin", Some(b"AAA"));
    node_write(&fake, &project, "b.bin", Some(b"BBB"));
    let (arrived, answer) = hold(&fake, Wait::Content(hash_bytes_v1(b"BBB")));
    let command = opts(&dir, &node);
    let meanwhile = async {
        let mut live = staged(&dir);
        let beside = sync(&command, signer.clone()).await;
        let mut after = staged(&dir);
        answer.send(()).unwrap();
        live.sort();
        after.sort();
        (live, beside.err().map(|e| e.to_string()), after)
    };
    let (report, (live, refused, after)) =
        while_held(sync(&command, signer.clone()), arrived, meanwhile).await;
    report.unwrap();
    assert!(!orphan.exists(), "the dead sync's temporary is gone");
    assert!(live.contains(&hash_bytes_v1(b"AAA")), "{live:?}");
    assert!(
        refused
            .as_deref()
            .is_some_and(|e| e.contains("another sync of this folder is running")),
        "{refused:?}"
    );
    assert_eq!(after, live, "the live sync's temporaries stay");
    assert_eq!(read(&dir, "a.bin").as_deref(), Some("AAA"));
    assert_eq!(read(&dir, "b.bin").as_deref(), Some("BBB"));
    assert_eq!(staged(&dir), Vec::<String>::new());
    sync(&command, signer).await.unwrap();
}

/// A child another thread forks during a sync, parked before its exec,
/// does not keep the folder locked once the sync is done.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_child_forked_during_a_sync_leaves_the_folder_unlocked() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-forked-child");
    dir.write("keep.txt", "keep");
    let signer = key();
    let command = opts(&dir, &node);
    sync(&command, signer.clone()).await.unwrap();
    let (arrived, answer) = hold(&fake, Wait::Files);
    let meanwhile = async {
        let parked = crate::exec_file::Parked::fork();
        answer.send(()).unwrap();
        parked
    };
    let (report, parked) = while_held(sync(&command, signer.clone()), arrived, meanwhile).await;
    report.unwrap();
    let next = sync(&command, signer).await;
    drop(parked);
    next.unwrap();
}

/// A sync refused for its arguments leaves a fresh folder as it was.
#[tokio::test]
async fn a_sync_refused_for_its_arguments_leaves_nothing() {
    let dir = TestDir::new("sync-refused");
    dir.write("scene.ts", "x");
    let mut no_node = opts(&dir, "http://127.0.0.1:9");
    no_node.node = None;
    let refusals = [
        opts(&dir, "ftp://example.org"),
        no_node,
        SyncOptions {
            project: Some("not-a-uuid".into()),
            ..opts(&dir, "http://127.0.0.1:9")
        },
    ];
    for command in refusals {
        let refused = sync(&command, key()).await.unwrap_err().to_string();
        assert!(
            [
                "not an http(s) URL",
                "no node to sync with",
                "is not a project id"
            ]
            .iter()
            .any(|why| refused.contains(why)),
            "{refused}"
        );
        assert!(
            !dir.0.join(".dcl-one").exists(),
            "{refused}: .dcl-one/ made"
        );
    }
}

/// The folder turns lib/ into a file while the node deletes lib/b.ts.
#[tokio::test]
async fn a_held_path_the_node_deleted_too_is_held_quietly() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-held-gone");
    dir.write("lib/b.ts", "B");
    dir.write("keep.txt", "keep");
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    std::fs::remove_dir_all(dir.0.join("lib")).unwrap();
    dir.write("lib", "a file");
    node_write(&fake, &project, "lib/b.ts", None);
    let first = sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    assert_eq!(first.kept, vec![]);
    assert_eq!(
        first.plan.push,
        BTreeMap::from([("lib".into(), Some(hash_bytes_v1(b"a file")))])
    );
    assert!(
        load_state(&dir.0).unwrap().files.contains_key("lib/b.ts"),
        "the base keeps what the file hides"
    );
    let again = sync(&opts(&dir, &node), signer).await.unwrap();
    assert!(!node_files(&fake, &project).contains_key("lib/b.ts"));
    assert_eq!(
        again.kept,
        vec![],
        "both sides lack lib/b.ts; nothing is left to hold"
    );
    run_signed(
        "sync-held-gone",
        wallet.private_key_hex(),
        opts(&dir, &node),
    )
    .await
    .unwrap();
    assert_eq!(read(&dir, "lib").as_deref(), Some("a file"));
}

/// Names that are not UTF-8, as unzipping a Windows-1252 archive leaves
/// them: a file and a directory.
#[cfg(unix)]
#[tokio::test]
async fn a_name_that_is_not_utf8_is_left_out_and_named() {
    use std::os::unix::ffi::OsStrExt;
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-not-utf8");
    dir.write("a.txt", "a");
    let file = dir.0.join(std::ffi::OsStr::from_bytes(b"caf\xe9.ts"));
    if std::fs::write(&file, "CAFE").is_err() {
        eprintln!("skipped: the temp directory takes only UTF-8 names");
        return;
    }
    let sub = dir.0.join(std::ffi::OsStr::from_bytes(b"d\xe9j\xe0"));
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("x.ts"), "X").unwrap();
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(report.unsynced, ["caf\u{FFFD}.ts", "d\u{FFFD}j\u{FFFD}"]);
    assert_eq!(
        node_files(&fake, &report.project)
            .keys()
            .collect::<Vec<_>>(),
        ["a.txt"],
        "nothing goes up under a lossy name"
    );
    node_write(&fake, &report.project, "b.txt", Some(b"b"));
    let err = run_signed("sync-not-utf8", wallet.private_key_hex(), opts(&dir, &node))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("caf\u{FFFD}.ts"), "{err}");
    assert_eq!(read(&dir, "b.txt").as_deref(), Some("b"), "the rest syncs");
    assert_eq!(std::fs::read(&file).unwrap(), b"CAFE", "they stay here");
    assert_eq!(std::fs::read(sub.join("x.ts")).unwrap(), b"X");
}

/// lib/ is moved aside for a file while the node deletes both its
/// files; then the real lib/ comes back, c.ts edited meanwhile.
#[tokio::test]
async fn a_hidden_copy_that_comes_back_meets_the_nodes_deletion() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-held-back");
    dir.write("lib/b.ts", "B");
    dir.write("lib/c.ts", "C");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    std::fs::rename(dir.0.join("lib"), dir.0.join(".lib-aside")).unwrap();
    dir.write("lib", "a file for now");
    node_write(&fake, &project, "lib/b.ts", None);
    node_write(&fake, &project, "lib/c.ts", None);
    sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    std::fs::remove_file(dir.0.join("lib")).unwrap();
    std::fs::rename(dir.0.join(".lib-aside"), dir.0.join("lib")).unwrap();
    dir.write("lib/c.ts", "C, edited while hidden");

    let back = sync(&opts(&dir, &node), signer).await.unwrap();
    let files = node_files(&fake, &project);
    assert!(
        !files.contains_key("lib/b.ts") && !files.contains_key("lib/c.ts"),
        "the node's deletions are not undone: {:?}",
        files.keys().collect::<Vec<_>>()
    );
    assert_eq!(read(&dir, "lib/b.ts"), None, "the deletion comes down");
    assert_eq!(
        back.plan.conflicts,
        vec![Conflict {
            path: "lib/c.ts".into(),
            local: Some(hash_bytes_v1(b"C, edited while hidden")),
            remote: None,
        }],
        "an edit against the node's deletion is a conflict"
    );
    assert_eq!(
        read(&dir, "lib/c.ts").as_deref(),
        Some("C, edited while hidden")
    );
}

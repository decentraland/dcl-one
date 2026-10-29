use super::test_support::*;
use super::*;
use crate::crdt_gen::TestDir;

#[test]
fn plans_follow_the_three_way_table() {
    let base = map(&[
        ("same", "a"),
        ("theirs", "a"),
        ("ours", "a"),
        ("both", "a"),
        ("gone", "a"),
        ("lost", "a"),
    ]);
    let local = map(&[
        ("same", "a"),
        ("theirs", "a"),
        ("ours", "b"),
        ("both", "b"),
        ("lost", "a"),
        ("new", "n"),
    ]);
    let remote = map(&[
        ("same", "a"),
        ("theirs", "c"),
        ("ours", "a"),
        ("both", "c"),
        ("gone", "a"),
        ("node_modules/x", "z"),
    ]);
    let p = plan(&local, &remote, &base, None);
    assert_eq!(
        p.push,
        BTreeMap::from([
            ("ours".into(), Some("b".into())),
            ("gone".into(), None),
            ("new".into(), Some("n".into()))
        ])
    );
    assert_eq!(
        p.pull,
        BTreeMap::from([("theirs".into(), Some("c".into())), ("lost".into(), None)])
    );
    assert_eq!(
        p.conflicts,
        vec![Conflict {
            path: "both".into(),
            local: Some("b".into()),
            remote: Some("c".into())
        }]
    );
    assert_eq!(
        p.base,
        map(&[
            ("same", "a"),
            ("theirs", "c"),
            ("ours", "b"),
            ("both", "a"),
            ("new", "n")
        ]),
        "a conflict keeps its old base"
    );
    assert_eq!(
        p.remote_after(&remote)
            .get("node_modules/x")
            .map(String::as_str),
        Some("z"),
        "a path the walk never lists stays on the node"
    );
    let mine = plan(&local, &remote, &base, Some(Side::Local));
    assert!(mine.conflicts.is_empty());
    assert_eq!(mine.push.get("both"), Some(&Some("b".into())));
    let theirs = plan(&local, &remote, &base, Some(Side::Remote));
    assert_eq!(theirs.pull.get("both"), Some(&Some("c".into())));
    let fresh = plan(
        &map(&[("x", "1")]),
        &map(&[("x", "2")]),
        &BTreeMap::new(),
        None,
    );
    assert_eq!(
        fresh.conflicts.len(),
        1,
        "two first copies that differ conflict"
    );
}

#[test]
fn the_walk_keeps_secrets_and_tooling_out() {
    for ok in [
        "scene.json",
        "src/index.ts",
        ".gitignore",
        "assets/.hidden.glb",
    ] {
        assert!(tracked(ok), "{ok}");
    }
    for out in [
        ".env",
        "src/.env.local",
        "node_modules/x/y.js",
        ".dcl-one/sync.json",
        "bin/index.js",
        "a//b",
        "../x",
        "src/.dcl-one-sync-0123456789abcdef.tmp",
        "src/.scene.ts.dcl-one-sync",
    ] {
        assert!(!tracked(out), "{out}");
    }
    assert!(is_sync_temp(&sync_temp_name()));
    assert!(tracked("src/.dcl-one-sync-notes.tmp"));
    for name in [".dcl-one-sync", "..dcl-one-sync", "...dcl-one-sync"] {
        assert!(tracked(name), "{name}: no old sync wrote it");
    }
    for name in [".dcl-one", ".DCL-ONE", ".Dcl-One"] {
        assert!(!tracked(name), "{name}: the root's .dcl-one is the sync's");
    }
    assert!(tracked("assets/.dcl-one"));
    let dir = TestDir::new("sync-walk-dcl-one");
    dir.write(".DCL-ONE", "a file");
    dir.write("a.txt", "a");
    assert_eq!(
        scan(&dir.0).unwrap().0.keys().collect::<Vec<_>>(),
        ["a.txt"],
        "kept local, not refused"
    );
    assert!(is_project_id(&new_project_id()));
    assert!(!is_project_id("not-a-uuid"));
}

#[tokio::test]
async fn a_folder_goes_up_then_nothing_moves() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-up");
    dir.write(
        "scene.json",
        r#"{"display":{"title":"Up"},"scene":{"parcels":["0,0","1,0"]}}"#,
    );
    dir.write("src/index.ts", "export {}");
    dir.write("assets/copy.ts", "export {}");
    dir.write(".env", "SECRET=1");
    dir.write("node_modules/x/index.js", "x");
    let signer = key();
    let first = sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    assert!(first.created);
    assert_eq!(first.version, 1);
    assert_eq!(first.uploaded, 2, "equal bytes travel once");
    let files = node_files(&fake, &first.project);
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        ["assets/copy.ts", "scene.json", "src/index.ts"]
    );
    let state = load_state(&dir.0).unwrap();
    assert_eq!(
        (state.project.as_str(), state.version),
        (first.project.as_str(), 1)
    );
    assert_eq!(state.files, files);

    let again = sync(
        &SyncOptions {
            node: None,
            ..opts(&dir, &node)
        },
        signer,
    )
    .await
    .unwrap();
    assert_eq!(
        again.project, first.project,
        "the node and project are remembered"
    );
    assert!(again.plan.push.is_empty() && again.plan.pull.is_empty());
    let fake = fake.lock().unwrap();
    assert_eq!(
        (fake.uploads, fake.puts),
        (2, 2),
        "the node named the two contents it lacked; an unchanged folder sends nothing"
    );
}

#[tokio::test]
async fn only_the_contents_the_node_lacks_travel() {
    let (node, fake) = fake_node().await;
    let signer = key();
    let first = TestDir::new("sync-dedup-a");
    first.write("model.glb", "the same big model");
    sync(&opts(&first, &node), signer.clone()).await.unwrap();
    let second = TestDir::new("sync-dedup-b");
    second.write("assets/model.glb", "the same big model");
    second.write("src/index.ts", "export {}");
    let report = sync(&opts(&second, &node), signer).await.unwrap();
    assert_eq!(report.plan.push.len(), 2);
    assert_eq!(
        report.uploaded, 1,
        "another project's upload is not sent again"
    );
    assert_eq!(fake.lock().unwrap().uploads, 2);
    assert_eq!(
        node_files(&fake, &report.project)["assets/model.glb"],
        hash_bytes_v1(b"the same big model")
    );
}

/// Every local file, path -> contents.
fn contents(dir: &TestDir) -> BTreeMap<String, String> {
    scan(&dir.0)
        .unwrap()
        .0
        .into_keys()
        .map(|p| {
            let c = read(dir, &p).unwrap();
            (p, c)
        })
        .collect()
}

#[tokio::test]
async fn a_project_gone_from_the_node_leaves_the_folder_alone() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-gone");
    dir.write("scene.json", "{}");
    dir.write("src/index.ts", "export {}");
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    let before = contents(&dir);
    let state = load_state(&dir.0).unwrap();
    fake.lock().unwrap().projects.remove(&project);

    let keyfile = TestDir::new("sync-gone-key");
    keyfile.write("key", wallet.private_key_hex());
    let err = run(SyncOptions {
        sign_key: Some(keyfile.0.join("key")),
        ..opts(&dir, &node)
    })
    .await
    .unwrap_err();
    assert!(err.to_string().contains("no longer on"), "{err}");
    assert_eq!(contents(&dir), before, "no local file is deleted");
    assert_eq!(load_state(&dir.0).unwrap(), state, "the state stays");
    assert!(
        !fake.lock().unwrap().projects.contains_key(&project),
        "nor is the project made again"
    );

    std::fs::remove_file(dir.0.join(".dcl-one/sync.json")).unwrap();
    let fresh = sync(&opts(&dir, &node), signer).await.unwrap();
    assert!(fresh.created);
    assert_ne!(fresh.project, project);
    assert_eq!(fresh.plan.push.len(), 2, "the folder goes up whole");
    assert_eq!(contents(&dir), before);
}

#[tokio::test]
async fn a_project_made_again_under_its_id_is_not_the_old_one() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-remade");
    dir.write("a.txt", "one");
    dir.write("b.txt", "two");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    let before = contents(&dir);
    {
        // The node's restart of a deleted project: empty, a new created_at.
        let mut fake = fake.lock().unwrap();
        let p = fake.projects.get_mut(&project).unwrap();
        p.1 += 1;
        p.2.clear();
        p.3 = "2027-01-01T00:00:00.000Z".into();
    }
    let err = sync(&opts(&dir, &node), signer.clone()).await.unwrap_err();
    assert!(err.to_string().contains("made again"), "{err}");
    assert_eq!(contents(&dir), before, "no local file is deleted");

    std::fs::remove_file(dir.0.join(".dcl-one/sync.json")).unwrap();
    let merged = sync(
        &SyncOptions {
            project: Some(project.clone()),
            ..opts(&dir, &node)
        },
        signer,
    )
    .await
    .unwrap();
    assert!(!merged.created);
    assert_eq!(
        merged.plan.push.len(),
        2,
        "a first sync into the new project"
    );
    assert_eq!(node_files(&fake, &project).len(), 2);
    assert_eq!(
        load_state(&dir.0).unwrap().created_at.as_deref(),
        Some("2027-01-01T00:00:00.000Z")
    );
}

#[tokio::test]
async fn the_node_comes_down_into_the_folder() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-down");
    dir.write("a.txt", "one");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "a.txt", Some(b"two"));
    node_write(&fake, &project, "assets/new.txt", Some(b"new"));
    let report = sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    assert_eq!(report.plan.pull.len(), 2);
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("two"));
    assert_eq!(read(&dir, "assets/new.txt").as_deref(), Some("new"));
    assert_eq!(
        load_state(&dir.0).unwrap().files,
        node_files(&fake, &project)
    );

    let fresh = TestDir::new("sync-down-fresh");
    let cloned = sync(
        &SyncOptions {
            project: Some(project.clone()),
            ..opts(&fresh, &node)
        },
        signer,
    )
    .await
    .unwrap();
    assert!(!cloned.created);
    assert_eq!(
        read(&fresh, "assets/new.txt").as_deref(),
        Some("new"),
        "an empty folder takes the whole project"
    );
}

#[tokio::test]
async fn deletions_travel_both_ways() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-delete");
    dir.write("keep.txt", "keep");
    dir.write("local-gone.txt", "x");
    dir.write("deep/node-gone.txt", "y");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    std::fs::remove_file(dir.0.join("local-gone.txt")).unwrap();
    node_write(&fake, &project, "deep/node-gone.txt", None);
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        report.plan.push,
        BTreeMap::from([("local-gone.txt".into(), None)])
    );
    assert_eq!(
        report.plan.pull,
        BTreeMap::from([("deep/node-gone.txt".into(), None)])
    );
    assert_eq!(
        node_files(&fake, &project).keys().collect::<Vec<_>>(),
        ["keep.txt"]
    );
    assert!(
        !dir.0.join("deep").exists(),
        "an emptied directory goes with its last file"
    );
    assert_eq!(read(&dir, "keep.txt").as_deref(), Some("keep"));
}

#[tokio::test]
async fn a_path_changed_on_both_sides_is_reported_and_left_alone() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-conflict");
    dir.write("a.txt", "base");
    dir.write("b.txt", "base");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    let base_hash = hash_bytes_v1(b"base");
    dir.write("a.txt", "mine");
    dir.write("b.txt", "mine too");
    node_write(&fake, &project, "a.txt", Some(b"theirs"));

    let report = sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    assert_eq!(
        report.plan.conflicts,
        vec![Conflict {
            path: "a.txt".into(),
            local: Some(hash_bytes_v1(b"mine")),
            remote: Some(hash_bytes_v1(b"theirs")),
        }]
    );
    assert_eq!(
        read(&dir, "a.txt").as_deref(),
        Some("mine"),
        "the folder's copy stays"
    );
    let files = node_files(&fake, &project);
    assert_eq!(
        files["a.txt"],
        hash_bytes_v1(b"theirs"),
        "the node's copy stays"
    );
    assert_eq!(
        files["b.txt"],
        hash_bytes_v1(b"mine too"),
        "the rest still syncs"
    );
    assert_eq!(
        load_state(&dir.0).unwrap().files["a.txt"],
        base_hash,
        "the base waits for a resolution"
    );
    let again = sync(&opts(&dir, &node), signer.clone()).await.unwrap();
    assert_eq!(again.plan.conflicts.len(), 1, "reported until resolved");

    let resolved = sync(
        &SyncOptions {
            prefer: Some(Side::Remote),
            ..opts(&dir, &node)
        },
        signer,
    )
    .await
    .unwrap();
    assert!(resolved.plan.conflicts.is_empty());
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("theirs"));
}

#[tokio::test]
async fn the_command_fails_on_a_conflict() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-conflict-exit");
    let keyfile = TestDir::new("sync-conflict-exit-key");
    let wallet = crate::random_test_wallet();
    keyfile.write("key", wallet.private_key_hex());
    dir.write("a.txt", "base");
    let command = SyncOptions {
        sign_key: Some(keyfile.0.join("key")),
        ..opts(&dir, &node)
    };
    run(command.clone()).await.unwrap();
    let project = load_state(&dir.0).unwrap().project;
    dir.write("a.txt", "mine");
    node_write(&fake, &project, "a.txt", Some(b"theirs"));
    let err = run(command).await.unwrap_err();
    assert!(err.to_string().contains("a.txt"), "{err}");
}

#[tokio::test]
async fn a_concurrent_write_is_merged_after_the_409() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-race");
    dir.write("a.txt", "one");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    dir.write("mine.txt", "mine");
    fake.lock().unwrap().race = Some(("theirs.txt".into(), b"theirs".to_vec()));
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert!(report.plan.conflicts.is_empty());
    let files = node_files(&fake, &project);
    assert!(files.contains_key("mine.txt") && files.contains_key("theirs.txt"));
    assert_eq!(read(&dir, "theirs.txt").as_deref(), Some("theirs"));
    assert_eq!(
        report.version, 3,
        "the first sync wrote 1, the racing writer 2, the retry 3"
    );
}

#[tokio::test]
async fn another_wallets_project_is_refused() {
    let (node, _fake) = fake_node().await;
    let dir = TestDir::new("sync-foreign");
    dir.write("a.txt", "one");
    let project = sync(&opts(&dir, &node), key()).await.unwrap().project;
    let other = TestDir::new("sync-foreign-other");
    let err = sync(
        &SyncOptions {
            project: Some(project),
            ..opts(&other, &node)
        },
        key(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("401"), "{err}");
}

#[tokio::test]
async fn a_dry_run_changes_nothing() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-dry");
    dir.write("a.txt", "one");
    let report = sync(
        &SyncOptions {
            dry_run: true,
            ..opts(&dir, &node)
        },
        key(),
    )
    .await
    .unwrap();
    assert_eq!(report.plan.push.len(), 1);
    assert!(fake.lock().unwrap().projects.is_empty());
    assert!(load_state(&dir.0).is_none());
}

#[cfg(unix)]
fn outside(dir: &TestDir, rel: &str) -> Option<String> {
    std::fs::read_to_string(dir.0.join(rel)).ok()
}

#[cfg(unix)]
#[tokio::test]
async fn a_pull_never_writes_through_a_symlink() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-link-write");
    let elsewhere = TestDir::new("sync-link-write-elsewhere");
    elsewhere.write("scene.ts", "SENTINEL");
    elsewhere.write("tmp-sentinel", "SENTINEL");
    dir.write("scene.json", "{}");
    dir.write("assets/keep.txt", "keep");
    std::os::unix::fs::symlink(&elsewhere.0, dir.0.join("src")).unwrap();
    // The fixed temporary name the write used to take, a link out.
    std::os::unix::fs::symlink(
        elsewhere.0.join("tmp-sentinel"),
        dir.0.join("assets/.model.glb.dcl-one-sync"),
    )
    .unwrap();
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "src/scene.ts", Some(b"REMOTE"));
    node_write(&fake, &project, "assets/model.glb", Some(b"REMOTE"));

    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        outside(&elsewhere, "scene.ts").as_deref(),
        Some("SENTINEL"),
        "nothing outside the folder is written"
    );
    assert_eq!(
        outside(&elsewhere, "tmp-sentinel").as_deref(),
        Some("SENTINEL")
    );
    assert_eq!(read(&dir, "assets/model.glb").as_deref(), Some("REMOTE"));
    assert!(
        std::fs::symlink_metadata(dir.0.join("assets/model.glb"))
            .unwrap()
            .is_file()
    );
    assert_eq!(
        report.kept,
        vec![kept("src/scene.ts", Hold::Symlink("src".into()))]
    );
    let state = load_state(&dir.0).unwrap();
    assert!(
        !state.files.contains_key("src/scene.ts"),
        "the base does not claim the unwritten file"
    );

    let keyfile = TestDir::new("sync-link-write-key");
    keyfile.write("key", wallet.private_key_hex());
    let err = run(SyncOptions {
        sign_key: Some(keyfile.0.join("key")),
        ..opts(&dir, &node)
    })
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("src/scene.ts (src is a symlink)"),
        "{err}"
    );
    assert_eq!(outside(&elsewhere, "scene.ts").as_deref(), Some("SENTINEL"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_pulled_deletion_never_follows_a_symlink() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-link-delete");
    let elsewhere = TestDir::new("sync-link-delete-elsewhere");
    dir.write("src/scene.ts", "SENTINEL");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    node_write(&fake, &project, "src/scene.ts", None);

    // While the node answers, src/ moves out of the folder, a link left
    // in its place: the file behind it is byte-for-byte the scanned one.
    let moved = elsewhere.0.join("src");
    let (arrived, answer) = hold(&fake, Wait::Files);
    let meanwhile = async {
        std::fs::rename(dir.0.join("src"), &moved).unwrap();
        std::os::unix::fs::symlink(&moved, dir.0.join("src")).unwrap();
        answer.send(()).unwrap();
    };
    let command = opts(&dir, &node);
    let (report, ()) = while_held(sync(&command, signer), arrived, meanwhile).await;
    let report = report.unwrap();
    assert_eq!(
        outside(&elsewhere, "src/scene.ts").as_deref(),
        Some("SENTINEL"),
        "nothing outside the folder is deleted"
    );
    assert_eq!(
        report.kept,
        vec![kept("src/scene.ts", Hold::Symlink("src".into()))]
    );
    assert_eq!(
        load_state(&dir.0).unwrap().files.get("src/scene.ts"),
        Some(&hash_bytes_v1(b"SENTINEL")),
        "the base does not claim the deletion"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_sync_state_never_goes_through_a_symlink() {
    let (node, fake) = fake_node().await;
    let elsewhere = TestDir::new("sync-link-state-elsewhere");
    elsewhere.write("sync.json", "SENTINEL");
    elsewhere.write("tmp-sentinel", "SENTINEL");

    let linked = TestDir::new("sync-link-state");
    linked.write("a.txt", "one");
    std::os::unix::fs::symlink(&elsewhere.0, linked.0.join(".dcl-one")).unwrap();
    let result = sync(&opts(&linked, &node), key()).await;
    assert_eq!(
        outside(&elsewhere, "sync.json").as_deref(),
        Some("SENTINEL"),
        "nothing is written through .dcl-one"
    );
    assert!(!elsewhere.0.join(".gitignore").exists());
    let err = result.unwrap_err();
    assert!(err.to_string().contains(".dcl-one is a symlink"), "{err}");
    assert!(
        fake.lock().unwrap().projects.is_empty(),
        "the refusal comes before anything changes"
    );

    let state_link = TestDir::new("sync-link-state-file");
    state_link.write("a.txt", "one");
    std::fs::create_dir(state_link.0.join(".dcl-one")).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.0.join("sync.json"),
        state_link.0.join(".dcl-one/sync.json"),
    )
    .unwrap();
    let err = sync(&opts(&state_link, &node), key()).await.unwrap_err();
    assert!(
        err.to_string().contains(".dcl-one/sync.json is a symlink"),
        "{err}"
    );
    assert_eq!(
        outside(&elsewhere, "sync.json").as_deref(),
        Some("SENTINEL")
    );

    // The fixed temporary name the state used to take, a link out.
    let tmp_link = TestDir::new("sync-link-state-tmp");
    tmp_link.write("a.txt", "one");
    std::fs::create_dir(tmp_link.0.join(".dcl-one")).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.0.join("tmp-sentinel"),
        tmp_link.0.join(".dcl-one/sync.json.tmp"),
    )
    .unwrap();
    let report = sync(&opts(&tmp_link, &node), key()).await.unwrap();
    assert_eq!(
        outside(&elsewhere, "tmp-sentinel").as_deref(),
        Some("SENTINEL")
    );
    assert_eq!(load_state(&tmp_link.0).unwrap().project, report.project);
    assert!(
        std::fs::symlink_metadata(tmp_link.0.join(".dcl-one/sync.json"))
            .unwrap()
            .is_file()
    );
}

#[tokio::test]
async fn a_file_changed_while_its_download_waits_is_kept() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-edit-during-download");
    for f in ["a.txt", "b.txt", "c.txt"] {
        dir.write(f, "base");
    }
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    for f in ["a.txt", "b.txt", "c.txt"] {
        node_write(&fake, &project, f, Some(format!("theirs {f}").as_bytes()));
    }

    // a.txt's download waits while a.txt is edited and b.txt deleted.
    let (arrived, answer) = hold(&fake, Wait::Content(hash_bytes_v1(b"theirs a.txt")));
    let meanwhile = async {
        dir.write("a.txt", "mine, edited meanwhile");
        std::fs::remove_file(dir.0.join("b.txt")).unwrap();
        answer.send(()).unwrap();
    };
    let command = opts(&dir, &node);
    let (report, ()) = while_held(sync(&command, signer.clone()), arrived, meanwhile).await;
    let report = report.unwrap();
    assert_eq!(
        read(&dir, "a.txt").as_deref(),
        Some("mine, edited meanwhile"),
        "the edit survives"
    );
    assert_eq!(read(&dir, "b.txt"), None, "the deletion survives");
    assert_eq!(
        read(&dir, "c.txt").as_deref(),
        Some("theirs c.txt"),
        "the rest still comes down"
    );
    assert_eq!(
        report.kept,
        vec![kept("a.txt", Hold::Changed), kept("b.txt", Hold::Changed)]
    );
    let base = hash_bytes_v1(b"base");
    let state = load_state(&dir.0).unwrap().files;
    assert_eq!(
        [&state["a.txt"], &state["b.txt"], &state["c.txt"]],
        [&base, &base, &hash_bytes_v1(b"theirs c.txt")],
        "the base claims only what the folder took"
    );

    let again = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        again.plan.conflicts,
        vec![
            Conflict {
                path: "a.txt".into(),
                local: Some(hash_bytes_v1(b"mine, edited meanwhile")),
                remote: Some(hash_bytes_v1(b"theirs a.txt")),
            },
            Conflict {
                path: "b.txt".into(),
                local: None,
                remote: Some(hash_bytes_v1(b"theirs b.txt")),
            },
        ],
        "the next sync reports both as conflicts"
    );
    assert_eq!(
        read(&dir, "a.txt").as_deref(),
        Some("mine, edited meanwhile")
    );
}

#[tokio::test]
async fn a_file_edited_while_the_file_set_waits_is_not_deleted() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-edit-during-files");
    let keyfile = TestDir::new("sync-edit-during-files-key");
    let wallet = crate::random_test_wallet();
    keyfile.write("key", wallet.private_key_hex());
    dir.write("keep.txt", "keep");
    dir.write("gone.txt", "base");
    let command = SyncOptions {
        sign_key: Some(keyfile.0.join("key")),
        ..opts(&dir, &node)
    };
    run(command.clone()).await.unwrap();
    let project = load_state(&dir.0).unwrap().project;
    node_write(&fake, &project, "gone.txt", None);

    let (arrived, answer) = hold(&fake, Wait::Files);
    let meanwhile = async {
        dir.write("gone.txt", "mine, edited meanwhile");
        answer.send(()).unwrap();
    };
    let (result, ()) = while_held(run(command.clone()), arrived, meanwhile).await;
    assert_eq!(
        read(&dir, "gone.txt").as_deref(),
        Some("mine, edited meanwhile"),
        "the edit is not deleted"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("gone.txt (changed here while syncing)"),
        "{err}"
    );
    assert_eq!(
        load_state(&dir.0).unwrap().files.get("gone.txt"),
        Some(&hash_bytes_v1(b"base")),
        "the base does not claim the deletion"
    );

    let err = run(command).await.unwrap_err();
    assert!(err.to_string().contains("changed on both sides"), "{err}");
    assert_eq!(
        read(&dir, "gone.txt").as_deref(),
        Some("mine, edited meanwhile")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_a_symlink_now_hides_is_not_deleted_on_the_node() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-link-hides");
    let elsewhere = TestDir::new("sync-link-hides-elsewhere");
    elsewhere.write("a.ts", "OUTSIDE");
    dir.write("src/a.ts", "export const a = 1");
    dir.write("lib/b.ts", "export const b = 1");
    dir.write("keep.txt", "keep");
    let wallet = crate::random_test_wallet();
    let signer = Signer::Key(wallet.private_key_hex());
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    let before = node_files(&fake, &project);
    let base = load_state(&dir.0).unwrap().files;

    // src/ becomes a link out of the folder, lib/ a file.
    std::fs::remove_dir_all(dir.0.join("src")).unwrap();
    std::os::unix::fs::symlink(&elsewhere.0, dir.0.join("src")).unwrap();
    std::fs::remove_dir_all(dir.0.join("lib")).unwrap();
    dir.write("lib", "a file where a directory was");
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    let files = node_files(&fake, &project);
    assert_eq!(
        files.get("src/a.ts"),
        before.get("src/a.ts"),
        "the node keeps the file a symlink hides"
    );
    assert_eq!(files.get("lib/b.ts"), before.get("lib/b.ts"));
    assert!(
        fake.lock()
            .unwrap()
            .put_sets
            .iter()
            .all(|set| set.contains_key("src/a.ts") && set.contains_key("lib/b.ts")),
        "no PUT drops them"
    );
    assert_eq!(
        report.plan.push,
        BTreeMap::from([(
            "lib".into(),
            Some(hash_bytes_v1(b"a file where a directory was"))
        )]),
        "only the new file goes up"
    );
    assert_eq!(
        report.kept,
        vec![
            kept("lib/b.ts", Hold::NotADirectory("lib".into())),
            kept("src/a.ts", Hold::Symlink("src".into())),
        ]
    );
    let state = load_state(&dir.0).unwrap().files;
    assert_eq!(
        (state.get("src/a.ts"), state.get("lib/b.ts")),
        (base.get("src/a.ts"), base.get("lib/b.ts")),
        "their base stays"
    );
    assert_eq!(
        std::fs::read_dir(&elsewhere.0).unwrap().count(),
        1,
        "nothing is added outside the folder"
    );
    assert_eq!(outside(&elsewhere, "a.ts").as_deref(), Some("OUTSIDE"));

    let keyfile = TestDir::new("sync-link-hides-key");
    keyfile.write("key", wallet.private_key_hex());
    let err = run(SyncOptions {
        sign_key: Some(keyfile.0.join("key")),
        ..opts(&dir, &node)
    })
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("src/a.ts (src is a symlink)"),
        "{err}"
    );
    assert_eq!(
        node_files(&fake, &project).get("src/a.ts"),
        before.get("src/a.ts")
    );
}

#[tokio::test]
async fn a_stale_temporary_is_never_pushed() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-stale-temp");
    dir.write("a.txt", "one");
    dir.write("src/.dcl-one-sync-0123456789abcdef.tmp", "half written");
    dir.write("src/.scene.ts.dcl-one-sync", "half written, older");
    let report = sync(&opts(&dir, &node), key()).await.unwrap();
    let fake = fake.lock().unwrap();
    assert_eq!(
        fake.projects[&report.project].2.keys().collect::<Vec<_>>(),
        ["a.txt"],
        "no temporary goes up"
    );
    assert!(fake.put_sets.iter().all(|set| set.len() == 1));
    for bytes in [&b"half written"[..], b"half written, older"] {
        assert!(!fake.blobs.contains_key(&hash_bytes_v1(bytes)));
    }
    assert_eq!(
        read(&dir, "src/.dcl-one-sync-0123456789abcdef.tmp").as_deref(),
        Some("half written"),
        "and the folder's copy is left alone"
    );
}

#[tokio::test]
async fn a_directory_the_node_made_a_file_comes_down_in_one_sync() {
    let (node, fake) = fake_node().await;
    let dir = TestDir::new("sync-reshape");
    dir.write("a/x.txt", "x");
    dir.write("keep.txt", "keep");
    let signer = key();
    let project = sync(&opts(&dir, &node), signer.clone())
        .await
        .unwrap()
        .project;
    // Another client turns the directory a/ into the file a.
    node_write(&fake, &project, "a/x.txt", None);
    node_write(&fake, &project, "a", Some(b"now a file"));
    let report = sync(&opts(&dir, &node), signer).await.unwrap();
    assert_eq!(
        read(&dir, "a").as_deref(),
        Some("now a file"),
        "the deletion clears the way for the write"
    );
    assert_eq!(report.kept, vec![]);
    assert_eq!(
        load_state(&dir.0).unwrap().files,
        node_files(&fake, &project)
    );
}

use super::*;

const HAT: &str = "urn:decentraland:matic:collections-v2:0xabc:1";
const CAPE: &str = "urn:decentraland:matic:collections-v2:0xabc:2";

fn equipped(urns: &[&str], shape: &'static str) -> Option<EquippedSet> {
    Some(EquippedSet {
        urns: urns.iter().map(|u| normalize_urn(u)).collect(),
        shape,
    })
}

fn ready(collection: &str) -> Phase {
    Phase::Ready {
        collection: collection.to_owned(),
    }
}

/// An entry's state with every scene that should run reported running.
fn state(model: &Model, urn: &str) -> Option<PortableState> {
    model
        .state_of(&normalize_urn(urn), |_| (PortableState::Running, None))
        .map(|(state, _)| state)
}

fn logged_in() -> Model {
    let mut model = Model::default();
    model.observe_profile(equipped(&[], MALE));
    model
}

fn equip(model: &mut Model, urns: &[&str]) {
    let shape = model.shape();
    model.observe_profile(equipped(urns, shape));
}

#[test]
fn an_equipped_smart_wearable_runs_from_its_wearables_content() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    assert_eq!(state(&model, HAT), Some(PortableState::Loading));
    assert!(model.effective().is_empty());

    model.set_phase(&urn, ready("bafyhat"));
    assert_eq!(state(&model, HAT), Some(PortableState::Running));
    assert_eq!(
        model.effective().into_iter().collect::<Vec<_>>(),
        vec![("bafyhat-sw-male".to_owned(), urn.clone())]
    );

    let (scene_id, source) = model.portable(&urn).unwrap();
    assert_eq!(scene_id, "bafyhat-sw-male");
    assert_eq!(source.pid, urn);
    assert!(source.is_smart_wearable() && !source.is_startup());
    let PortableKind::SmartWearable { location, .. } = source.kind else {
        panic!("not a smart wearable");
    };
    assert_eq!(
        location,
        SceneIpfsLocation::ContentFile {
            hash: "bafyhat".to_owned(),
            file: "male.smart_wearable".to_owned(),
        }
    );
}

#[test]
fn a_wearable_without_a_scene_is_not_listed() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    model.set_phase(&normalize_urn(HAT), Phase::NotSmart);
    assert_eq!(state(&model, HAT), None);
    assert!(model.effective().is_empty());
}

#[test]
fn the_scene_state_comes_from_the_running_scene() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready("bafyhat"));
    let blocked = model.state_of(&urn, |id| {
        assert_eq!(id, "bafyhat-sw-male");
        (PortableState::Blocked, None)
    });
    assert_eq!(blocked, Some((PortableState::Blocked, None)));
    // blocking leaves the scene in place
    assert_eq!(model.effective().len(), 1);
}

#[test]
fn unequipping_removes_the_scene_and_forgets_the_stop() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready("bafyhat"));
    assert!(model.stop(&urn));
    assert!(model.effective().is_empty());

    equip(&mut model, &[]);
    assert!(model.entries.is_empty());
    assert!(!model.stopped.contains(&urn));

    // re-equipping starts over
    equip(&mut model, &[HAT]);
    model.set_phase(&urn, ready("bafyhat"));
    assert_eq!(state(&model, HAT), Some(PortableState::Running));
}

#[test]
fn a_stopped_wearable_stays_off_until_restarted() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready("bafyhat"));
    assert!(model.stop(&urn));
    assert!(!model.stop(&urn), "a second stop has nothing to stop");
    assert_eq!(state(&model, HAT), Some(PortableState::Stopped));
    // a profile re-sent with the same wearables does not revive it
    equip(&mut model, &[HAT, CAPE]);
    assert_eq!(state(&model, HAT), Some(PortableState::Stopped));
    assert!(model.effective().is_empty());

    assert!(model.restart(&urn));
    assert_eq!(state(&model, HAT), Some(PortableState::Running));
    assert!(!model.restart("urn:unknown"));
}

#[test]
fn a_failure_is_retried_on_restart() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, Phase::Failed("wearable Missing".to_owned()));
    assert!(!model.stop(&urn), "nothing to stop");
    assert_eq!(
        model.state_of(&urn, |_| (PortableState::Running, None)),
        Some((PortableState::Failed, Some("wearable Missing".to_owned())))
    );
    assert!(model.restart(&urn));
    assert_eq!(model.entries[&urn].phase, Phase::Loading);
}

#[test]
fn a_body_shape_change_re_resolves_and_keeps_the_stop() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready("bafyhat"));
    assert!(model.stop(&urn));

    model.observe_profile(equipped(&[HAT], FEMALE));
    let entry = &model.entries[&urn];
    assert_eq!(entry.phase, Phase::Loading);
    assert_eq!(entry.shape, FEMALE);
    assert!(model.stopped.contains(&urn));
    model.set_phase(&urn, ready("bafyhat"));
    assert!(model.restart(&urn));
    assert_eq!(
        model.effective().into_keys().collect::<Vec<_>>(),
        vec!["bafyhat-sw-female".to_owned()]
    );
}

#[test]
fn logout_forgets_the_session_but_keeps_console_runs() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let hat = normalize_urn(HAT);
    model.set_phase(&hat, ready("bafyhat"));
    assert!(model.stop(&hat));
    let cape = normalize_urn(CAPE);
    model.run(cape.clone());
    model.set_phase(&cape, ready("bafycape"));

    model.observe_profile(None);
    assert!(!model.entries.contains_key(&hat));
    assert!(model.stopped.is_empty());
    assert_eq!(
        model.entries[&cape].source,
        SmartWearableSource::Console,
        "a console run survives logout"
    );
    assert_eq!(state(&model, CAPE), Some(PortableState::Running));
}

#[test]
fn a_console_run_of_an_equipped_wearable_survives_unequipping() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready("bafyhat"));
    model.run(urn.clone());
    assert_eq!(model.entries[&urn].source, SmartWearableSource::Equipped);
    equip(&mut model, &[]);
    assert_eq!(model.entries[&urn].source, SmartWearableSource::Console);
    assert_eq!(state(&model, HAT), Some(PortableState::Running));
}

#[test]
fn urns_are_normalized() {
    assert_eq!(
        normalize_urn("urn:decentraland:matic:collections-v2:0xABC:1:42"),
        "urn:decentraland:matic:collections-v2:0xabc:1"
    );
    // not catalyst urns: kept, lowercased
    assert_eq!(
        normalize_urn("urn:decentraland:off-chain:e2e:smart-wearable"),
        "urn:decentraland:off-chain:e2e:smart-wearable"
    );
    assert_eq!(
        normalize_urn("A0F68DC0-1a46-41b8-a7dc-e17fc3885dce"),
        "a0f68dc0-1a46-41b8-a7dc-e17fc3885dce"
    );
}

#[test]
fn preview_wearables_parse_the_catalyst_shape() {
    let body = br#"{ "ok": true, "data": [ {
        "id": "urn:decentraland:off-chain:e2e:smart-wearable",
        "name": "Smart Wearable Fixture",
        "data": { "representations": [ { "contents": [
            { "key": "male/scene.json", "url": "http://127.0.0.1:1/content/contents/bafyscene", "hash": "bafyscene" },
            { "key": "male/bin/index.js", "url": "http://127.0.0.1:1/content/contents/bafymain", "hash": "bafymain" }
        ] }, { "contents": [
            { "key": "female/scene.json", "url": "http://127.0.0.1:1/content/contents/bafyscene", "hash": "bafyscene" }
        ] } ] }
    }, {
        "id": "urn:no-base-url",
        "data": { "representations": [ { "contents": [
            { "key": "scene.json", "url": "http://elsewhere/x", "hash": "bafyother" }
        ] } ] }
    } ] }"#;
    let previews = parse_preview_wearables(body).unwrap();
    assert_eq!(previews.len(), 1);
    let preview = &previews[0];
    assert_eq!(preview.urn, "urn:decentraland:off-chain:e2e:smart-wearable");
    assert_eq!(preview.name, "Smart Wearable Fixture");
    assert_eq!(preview.base_url, "http://127.0.0.1:1/content/contents/");
    assert_eq!(
        preview.content.hash("female/scene.json").as_deref(),
        Some("bafyscene")
    );
    assert_eq!(
        preview.content.hash("male/bin/index.js").as_deref(),
        Some("bafymain")
    );
}

#[test]
fn preview_wearables_parse_top_level_contents() {
    let body = br#"{ "ok": true, "data": [ {
        "id": "a0f68dc0-1a46-41b8-a7dc-e17fc3885dce",
        "name": "Fixture",
        "data": { "category": "hat", "representations": [ { "mainFile": "model.glb", "contents": ["model.glb"] } ] },
        "baseUrl": "http://localhost:8000/content/contents",
        "contents": [
            { "key": "scene.json", "url": "http://localhost:8000/content/contents/b64-s", "hash": "b64-s" },
            { "key": "bin/index.js", "url": "http://localhost:8000/content/contents/b64-i", "hash": "b64-i" }
        ]
    }, { "id": "urn:no-name", "contents": [
        { "key": "scene.json", "url": "http://host/c/h1", "hash": "h1" }
    ] } ] }"#;
    let previews = parse_preview_wearables(body).unwrap();
    assert_eq!(previews.len(), 2);
    assert_eq!(previews[0].urn, "a0f68dc0-1a46-41b8-a7dc-e17fc3885dce");
    assert_eq!(
        previews[0].base_url,
        "http://localhost:8000/content/contents/"
    );
    assert_eq!(
        previews[0].content.hash("bin/index.js").as_deref(),
        Some("b64-i")
    );
    assert_eq!(previews[1].name, "urn:no-name");
    assert_eq!(previews[1].base_url, "http://host/c/");

    assert!(parse_preview_wearables(b"not json").is_err());
}

#[test]
fn previews_replace_the_previous_realms() {
    let mut model = Model::default();
    let preview = |urn: &str| PreviewWearable {
        urn: urn.to_owned(),
        name: urn.to_owned(),
        content: ContentMap::from_content([("scene.json".to_owned(), "h".to_owned())]),
        base_url: "http://x/".to_owned(),
        data: None,
    };
    model.set_previews(vec![preview("a"), preview("b")]);
    assert_eq!(model.preview_count(), 2);
    assert_eq!(model.entries["a"].source, SmartWearableSource::Preview);
    model.set_previews(vec![preview("b")]);
    assert_eq!(
        model.entries.keys().cloned().collect::<Vec<_>>(),
        vec!["b".to_owned()]
    );
}

#[test]
fn the_listing_has_a_header_and_one_line_per_entry() {
    let mut model = logged_in();
    equip(&mut model, &[HAT, CAPE]);
    let hat = normalize_urn(HAT);
    let cape = normalize_urn(CAPE);
    model.set_phase(&hat, ready("bafyhat"));
    model.set_phase(&cape, Phase::Failed("wearable Missing".to_owned()));
    let entries = diagnostics::entries(&model, |_| (PortableState::Blocked, None));
    assert_eq!(
        diagnostics::format_listing(false, &entries),
        format!("current scene allows: false\n{hat} blocked\n{cape} failed wearable Missing")
    );
    model.set_phase(&cape, Phase::NotSmart);
    assert!(model.stop(&hat));
    let entries = diagnostics::entries(&model, |_| (PortableState::Running, None));
    assert_eq!(
        diagnostics::format_listing(true, &entries),
        format!("current scene allows: true\n{hat} stopped\n{cape} notSmart")
    );
    assert_eq!(diagnostics::format_listing(true, &[]), "no smart wearables");
}

/// `dcl-scene-cli start` and upstream's `sdk-commands start` list a preview wearable with every
/// file under `male/` in each representation, whichever body shapes it names: both bodies run it.
#[test]
fn a_creator_preview_runs_for_either_body_shape() {
    let body = br#"{ "ok": true, "data": [ {
        "id": "urn:6f1d3c2e-0000-4000-8000-000000000001",
        "name": "One representation",
        "baseUrl": "http://127.0.0.1:8000/content/contents/",
        "data": { "category": "hat", "representations": [ {
            "bodyShapes": [
                "urn:decentraland:off-chain:base-avatars:BaseMale",
                "urn:decentraland:off-chain:base-avatars:BaseFemale"
            ],
            "mainFile": "male/model.glb",
            "contents": [
                { "key": "male/scene.json", "url": "http://127.0.0.1:8000/content/contents/b64-s", "hash": "b64-s" },
                { "key": "male/bin/index.js", "url": "http://127.0.0.1:8000/content/contents/b64-i", "hash": "b64-i" }
            ]
        } ] }
    }, {
        "id": "urn:6f1d3c2e-0000-4000-8000-000000000002",
        "name": "A representation per body",
        "baseUrl": "http://127.0.0.1:8000/content/contents/",
        "data": { "category": "hat", "representations": [ {
            "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
            "contents": [ { "key": "male/scene.json", "url": "u", "hash": "b64-s2" } ]
        }, {
            "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
            "contents": [ { "key": "male/scene.json", "url": "u", "hash": "b64-s2" } ]
        } ] }
    } ] }"#;
    let previews = parse_preview_wearables(body).unwrap();
    assert_eq!(previews.len(), 2);
    for preview in previews {
        let aliases = with_scene_aliases(preview.content.clone());
        for shape in [MALE, FEMALE] {
            assert!(
                aliases.hash(&scene_file(shape)).is_some(),
                "{} runs for {shape}",
                preview.name
            );
            assert!(preview_collection(&preview.content, shape).is_some());
        }
    }
}

fn preview(urn: &str, main: &str) -> PreviewWearable {
    PreviewWearable {
        urn: urn.to_owned(),
        name: urn.to_owned(),
        content: ContentMap::from_content([
            ("male/scene.json".to_owned(), "b64-scene".to_owned()),
            ("male/bin/index.js".to_owned(), main.to_owned()),
        ]),
        base_url: "http://127.0.0.1:8000/content/contents/".to_owned(),
        data: None,
    }
}

/// What `resolve_entries` decides for a loading preview, without the IPFS registration.
fn resolve_previews(model: &mut Model) {
    let shape = model.shape();
    let loading: Vec<String> = model
        .entries
        .iter()
        .filter(|(_, e)| e.phase == Phase::Loading)
        .map(|(urn, _)| urn.clone())
        .collect();
    for urn in loading {
        let Some(ForcedSource::Preview(p)) = model.forced.get(&urn) else {
            continue;
        };
        let collection = preview_collection(&p.content, shape).unwrap();
        model.set_phase(&urn, ready(&collection));
    }
}

/// Saving a previewed smart wearable's project restarts its scene on the new files, even one the
/// creator had stopped; another previewed wearable keeps running untouched.
#[test]
fn saving_a_previewed_smart_wearable_restarts_its_scene() {
    let mut model = logged_in();
    model.set_previews(vec![
        preview("urn:hat", "b64-main-1"),
        preview("urn:cape", "b64-cape"),
    ]);
    resolve_previews(&mut model);
    let before = model.effective();
    let hat_before = model.entries["urn:hat"].scene_id().unwrap();
    let cape = model.entries["urn:cape"].scene_id().unwrap();
    assert_eq!(before.len(), 2);
    assert!(model.stop("urn:hat"));

    model.reload_preview(preview("urn:hat", "b64-main-2"));
    resolve_previews(&mut model);

    assert_eq!(state(&model, "urn:hat"), Some(PortableState::Running));
    let hat_after = model.entries["urn:hat"].scene_id().unwrap();
    assert_ne!(hat_after, hat_before, "a new scene id is a restart");
    assert_eq!(model.entries["urn:cape"].scene_id().unwrap(), cape);
    assert_eq!(
        model.effective().keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([hat_after, cape])
    );
    assert_eq!(model.preview_count(), 2);
}

/// A preview reload names the saved project's scene id, the id the realm keys
/// `/preview-wearables/{id}` on; a model reload names it too.
#[test]
fn a_preview_reload_asks_the_realm_for_the_saved_projects_wearable() {
    let about = "http://127.0.0.1:8000/about";
    let reload = common::structs::PreviewCommand::ReloadScene {
        hash: "b64-scene-id".to_owned(),
    };
    assert_eq!(
        preview_reload_url(about, &reload).as_deref(),
        Some("http://127.0.0.1:8000/preview-wearables/b64-scene-id")
    );
    let model = common::structs::PreviewCommand::ReloadModel {
        hash: "b64-model".to_owned(),
        src: "model.glb".to_owned(),
        scene_id: "b64-scene-id".to_owned(),
    };
    assert_eq!(
        preview_reload_url(about, &model).as_deref(),
        Some("http://127.0.0.1:8000/preview-wearables/b64-scene-id")
    );
    assert_eq!(preview_reload_url("", &reload), None);
}

const PREVIEW_HAT: &[u8] = br#"{ "ok": true, "data": [ {
    "id": "urn:6f1d3c2e-0000-4000-8000-000000000003",
    "name": "Preview Hat",
    "baseUrl": "http://127.0.0.1:8000/content/contents/",
    "data": { "category": "hat", "replaces": [], "hides": ["hair"], "tags": [], "representations": [ {
        "bodyShapes": [
            "urn:decentraland:off-chain:base-avatars:BaseMale",
            "urn:decentraland:off-chain:base-avatars:BaseFemale"
        ],
        "mainFile": "male/model.glb",
        "overrideHides": [],
        "overrideReplaces": [],
        "contents": [
            { "key": "male/model.glb", "url": "http://127.0.0.1:8000/content/contents/b64-m", "hash": "b64-m" },
            { "key": "male/scene.json", "url": "http://127.0.0.1:8000/content/contents/b64-s", "hash": "b64-s" }
        ]
    } ] }
} ] }"#;

/// The preview listing is the wearable's whole definition: its category and hides, and for each
/// body shape a model file that is in its content.
#[test]
fn a_preview_wearable_is_defined_for_each_body_from_the_listing() {
    let preview = parse_preview_wearables(PREVIEW_HAT).unwrap().remove(0);
    let data = preview.data.as_ref().expect("a wearable definition");
    assert_eq!(data.category, WearableCategory::HAT);
    assert_eq!(data.hides, vec![WearableCategory::HAIR]);
    let bodies: Vec<(Vec<String>, String)> = data
        .representations
        .iter()
        .map(|r| (r.body_shapes.clone(), r.main_file.clone()))
        .collect();
    assert_eq!(
        bodies,
        vec![
            (
                vec!["urn:decentraland:off-chain:base-avatars:BaseMale".to_owned()],
                "male/model.glb".to_owned()
            ),
            (
                vec!["urn:decentraland:off-chain:base-avatars:BaseFemale".to_owned()],
                "female/model.glb".to_owned()
            ),
        ]
    );
    for representation in data.representations.iter() {
        assert!(
            preview.content.hash(&representation.main_file).is_some(),
            "{} is in the content",
            representation.main_file
        );
    }
}

/// The local avatar wears every preview wearable ahead of the profile's, so each takes its
/// category's slot; a preview that goes, or is replaced by a save, leaves the avatar.
#[test]
fn the_local_avatar_wears_the_previews_ahead_of_its_profile() {
    let first = preview_wearable_urn(&preview("urn:hat", "b64-main-1").content);
    let saved = preview_wearable_urn(&preview("urn:hat", "b64-main-2").content);
    let profile = vec![
        "urn:decentraland:matic:collections-v2:0xabc:1".to_owned(),
        "urn:decentraland:off-chain:base-avatars:eyebrows_00".to_owned(),
    ];

    let worn = with_previews(&profile, std::slice::from_ref(&first));
    assert_eq!(worn[0], first);
    assert_eq!(worn[1..], profile[..]);
    assert_eq!(with_previews(&worn, std::slice::from_ref(&first)), worn);

    let after_save = with_previews(&worn, std::slice::from_ref(&saved));
    assert_eq!(after_save[0], saved);
    assert_eq!(after_save[1..], profile[..]);

    assert_eq!(with_previews(&after_save, &[]), profile);
}

/// A preview wearable is a collectible of its own, never looked up on a catalyst, and a save that
/// changes its files makes it a new one, so the avatar loads the new model.
#[test]
fn a_save_that_changes_a_preview_wearable_makes_a_new_collectible() {
    let first = preview_wearable_urn(&preview("urn:hat", "b64-main-1").content);
    let unchanged = preview_wearable_urn(&preview("urn:hat", "b64-main-1").content);
    let saved = preview_wearable_urn(&preview("urn:hat", "b64-main-2").content);
    assert_eq!(first, unchanged);
    assert_ne!(first, saved);
    for urn in [&first, &saved] {
        let parsed = WearableUrn::new(urn).expect("a collectible urn");
        assert_eq!(parsed.as_str(), urn.as_str());
    }
}

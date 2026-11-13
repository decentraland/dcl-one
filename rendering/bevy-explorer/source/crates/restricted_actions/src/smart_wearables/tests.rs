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
fn preview_wearables_parse_the_scene_cli_shape() {
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

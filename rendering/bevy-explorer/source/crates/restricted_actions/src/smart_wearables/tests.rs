use super::*;

const HAT: &str = "urn:decentraland:matic:collections-v2:0xabc:1";
const CAPE: &str = "urn:decentraland:matic:collections-v2:0xabc:2";

fn equipped(urns: &[&str], prefix: &str) -> Option<EquippedSet> {
    Some(EquippedSet {
        urns: urns.iter().map(|u| normalize_urn(u)).collect(),
        body_shape_prefix: prefix.to_owned(),
    })
}

fn ready(urn: &str, declared: &[&str]) -> Phase {
    let declared: Vec<String> = declared.iter().map(|d| d.to_string()).collect();
    Phase::Ready(ReadyScene {
        scene_id: format!("{urn}-sw-male"),
        name: urn.to_owned(),
        gated: declared
            .iter()
            .filter_map(|d| smart_wearable_permission(d))
            .collect(),
        declared,
    })
}

fn state(model: &Model, urn: &str, allows: bool) -> SmartWearableState {
    model.state_of(&normalize_urn(urn), allows).0
}

/// A model logged in with nothing equipped, so later equips are not login equips.
fn logged_in() -> Model {
    let mut model = Model::default();
    model.observe_profile(equipped(&[], MALE_PREFIX));
    model
}

fn equip(model: &mut Model, urns: &[&str]) {
    let prefix = model.body_shape_prefix().to_owned();
    model.observe_profile(equipped(urns, &prefix));
}

#[test]
fn an_equipped_smart_wearable_without_gated_permissions_runs() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    assert_eq!(state(&model, HAT, true), SmartWearableState::Loading);
    model.set_phase(&urn, ready(&urn, &["ALLOW_TO_TRIGGER_AVATAR_EMOTE"]));
    assert!(model.next_auth_request(true).is_none());
    assert_eq!(state(&model, HAT, true), SmartWearableState::Running);
    assert_eq!(
        model.effective(true).into_iter().collect::<Vec<_>>(),
        vec![(format!("{urn}-sw-male"), urn)]
    );
}

#[test]
fn unequipping_removes_the_scene_and_forgets_the_kill() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready(&urn, &[]));
    assert!(model.kill(&urn));
    assert!(model.effective(true).is_empty());

    equip(&mut model, &[]);
    assert!(model.entries.is_empty());
    assert!(model.effective(true).is_empty());
    assert!(!model.killed.contains(&urn));

    // re-equipping starts over
    equip(&mut model, &[HAT]);
    model.set_phase(&urn, ready(&urn, &[]));
    assert_eq!(state(&model, HAT, true), SmartWearableState::Running);
}

#[test]
fn a_killed_wearable_stays_off_until_restarted() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready(&urn, &[]));
    assert!(model.kill(&urn));
    assert!(!model.kill(&urn), "a second kill has nothing to stop");
    assert_eq!(state(&model, HAT, true), SmartWearableState::Killed);
    // a profile re-sent with the same wearables does not revive it
    equip(&mut model, &[HAT, CAPE]);
    assert_eq!(state(&model, HAT, true), SmartWearableState::Killed);
    assert!(model.effective(true).is_empty());

    assert!(model.restart(&urn));
    assert_eq!(state(&model, HAT, true), SmartWearableState::Running);
    assert!(!model.restart("urn:unknown"));
}

#[test]
fn a_disabled_parcel_blocks_and_leaving_it_resumes() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready(&urn, &[]));
    assert_eq!(model.effective(true).len(), 1);

    assert!(model.effective(false).is_empty());
    assert_eq!(state(&model, HAT, false), SmartWearableState::Blocked);
    let info = model.info(PortableExperiencesToggle::Disabled);
    assert!(!info.current_scene_allows);
    assert_eq!(info.entries[0].state, SmartWearableState::Blocked);

    assert_eq!(model.effective(true).len(), 1);
    let info = model.info(PortableExperiencesToggle::HideUi);
    assert!(info.current_scene_allows && info.current_scene_hides_ui);
    assert_eq!(info.entries[0].state, SmartWearableState::Running);
}

#[test]
fn a_body_shape_change_re_resolves_and_keeps_the_kill() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready(&urn, &[]));
    assert!(model.kill(&urn));

    model.observe_profile(equipped(&[HAT], FEMALE_PREFIX));
    let entry = &model.entries[&urn];
    assert_eq!(entry.phase, Phase::Loading);
    assert_eq!(entry.body_shape_prefix, FEMALE_PREFIX);
    assert!(model.is_loading(&urn, FEMALE_PREFIX));
    assert!(!model.is_loading(&urn, MALE_PREFIX));
    assert!(model.killed.contains(&urn));
}

#[test]
fn login_asks_only_for_web3() {
    let mut model = Model::default();
    // the first profile carries both
    model.observe_profile(equipped(&[HAT, CAPE], MALE_PREFIX));
    let (hat, cape) = (normalize_urn(HAT), normalize_urn(CAPE));
    assert!(model.login_pending());
    model.set_phase(&hat, ready(&hat, &["USE_FETCH", "USE_WEBSOCKET"]));
    assert!(model.login_pending(), "the cape is still loading");
    model.set_phase(&cape, ready(&cape, &["USE_WEB3_API"]));
    assert!(!model.login_pending());

    let request = model.next_auth_request(true).expect("web3 is asked");
    assert_eq!(request.urn, cape);
    assert_eq!(request.reason, SmartWearableAuthReason::Login);
    assert_eq!(request.permissions, vec!["USE_WEB3_API".to_owned()]);
    assert_eq!(state(&model, HAT, true), SmartWearableState::Running);
    assert_eq!(
        state(&model, CAPE, true),
        SmartWearableState::AwaitingAuthorization
    );
    // one question at a time, repeated to a late subscriber
    assert!(model.next_auth_request(true).is_none());
    assert_eq!(model.pending_request().map(|r| r.id), Some(request.id));

    assert!(!model.answer(request.id + 1, true));
    assert!(model.answer(request.id, true));
    assert_eq!(state(&model, CAPE, true), SmartWearableState::Running);
    assert!(model.pending_request().is_none());
}

#[test]
fn equipping_later_asks_for_any_gated_permission() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready(&urn, &["USE_FETCH", "ALLOW_MEDIA_HOSTNAMES"]));

    // no HUD listening: it waits, it is not allowed
    assert!(model.next_auth_request(false).is_none());
    assert_eq!(
        state(&model, HAT, true),
        SmartWearableState::AwaitingAuthorization
    );
    assert!(model.effective(true).is_empty());

    let request = model.next_auth_request(true).expect("fetch is asked");
    assert_eq!(request.reason, SmartWearableAuthReason::Equip);
    assert_eq!(
        request.permissions,
        vec!["USE_FETCH".to_owned(), "ALLOW_MEDIA_HOSTNAMES".to_owned()]
    );
    assert!(model.answer(request.id, false));
    assert_eq!(state(&model, HAT, true), SmartWearableState::Denied);
    assert!(model.effective(true).is_empty());
    assert!(model.next_auth_request(true).is_none());

    // restart asks again
    assert!(model.restart(&urn));
    let again = model.next_auth_request(true).expect("asked again");
    assert_ne!(again.id, request.id);
    assert!(model.answer(again.id, true));
    assert_eq!(state(&model, HAT, true), SmartWearableState::Running);
}

#[test]
fn logout_forgets_the_session_but_keeps_console_runs() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let hat = normalize_urn(HAT);
    model.set_phase(&hat, ready(&hat, &["USE_FETCH"]));
    let request = model.next_auth_request(true).unwrap();
    model.answer(request.id, true);
    model.run(normalize_urn(CAPE));
    let cape = normalize_urn(CAPE);
    model.set_phase(&cape, ready(&cape, &[]));
    assert!(model.kill(&cape));

    model.observe_profile(None);
    assert!(!model.entries.contains_key(&hat));
    assert!(model.authorized.is_empty() && model.killed.is_empty());
    assert_eq!(
        model.entries[&cape].source,
        SmartWearableSource::Console,
        "a console run survives logout"
    );
    assert_eq!(state(&model, CAPE, true), SmartWearableState::Running);

    // the next profile is a new login
    model.observe_profile(equipped(&[HAT], MALE_PREFIX));
    assert!(model.login_set.contains(&hat));
}

#[test]
fn a_console_run_of_an_equipped_wearable_survives_unequipping() {
    let mut model = logged_in();
    equip(&mut model, &[HAT]);
    let urn = normalize_urn(HAT);
    model.set_phase(&urn, ready(&urn, &[]));
    model.run(urn.clone());
    assert_eq!(model.entries[&urn].source, SmartWearableSource::Equipped);
    equip(&mut model, &[]);
    assert_eq!(model.entries[&urn].source, SmartWearableSource::Console);
    assert_eq!(state(&model, HAT, true), SmartWearableState::Running);
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

const SCENE_JSON: &str = r#"{
    "main": "bin/index.js",
    "runtimeVersion": "7",
    "isPortableExperience": true,
    "requiredPermissions": ["USE_FETCH", "ALLOW_TO_TRIGGER_AVATAR_EMOTE"],
    "scene": { "base": "0,0", "parcels": ["0,0"] }
}"#;

fn packed_content() -> ContentMap {
    ContentMap::from_content(
        [
            ("male/scene.json", "hm"),
            ("male/bin/index.js", "im"),
            ("male/bin/scene.js", "sm"),
            ("female/scene.json", "hf"),
            ("female/bin/index.js", "if"),
            ("model.glb", "glb"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned())),
    )
}

#[test]
fn the_scene_json_of_the_body_shape_wins_over_a_bare_one() {
    let files = vec![
        "female/scene.json".to_owned(),
        "male/scene.json".to_owned(),
        "scene.json".to_owned(),
    ];
    assert_eq!(
        scene_json_key(&files, FEMALE_PREFIX).as_deref(),
        Some("female/scene.json")
    );
    assert_eq!(
        scene_json_key(&files[2..], MALE_PREFIX).as_deref(),
        Some("scene.json")
    );
    assert_eq!(scene_json_key(&files[..1], MALE_PREFIX), None);
}

#[test]
fn a_scene_json_resolves_to_the_body_shapes_scene() {
    let resolved = build_resolved(
        "bafywearable",
        MALE_PREFIX,
        "Hat".to_owned(),
        SCENE_JSON.as_bytes(),
        &packed_content(),
        "http://cat/contents/".to_owned(),
    )
    .unwrap();
    assert_eq!(resolved.scene_id, "bafywearable-sw-male");
    assert_eq!(resolved.ready.scene_id, resolved.scene_id);
    // every file of the shape, unprefixed, plus the shared ones; nothing of the other shape
    assert_eq!(
        resolved.stripped.hash("bin/index.js").as_deref(),
        Some("im")
    );
    assert_eq!(
        resolved.stripped.hash("bin/scene.js").as_deref(),
        Some("sm")
    );
    assert_eq!(resolved.stripped.hash("model.glb").as_deref(), Some("glb"));
    assert!(resolved.stripped.hash("female/bin/index.js").is_none());
    assert_eq!(
        resolved.ready.declared,
        vec![
            "USE_FETCH".to_owned(),
            "ALLOW_TO_TRIGGER_AVATAR_EMOTE".to_owned()
        ]
    );
    assert_eq!(
        resolved.ready.gated,
        [PermissionType::Fetch].into_iter().collect()
    );
    assert_eq!(resolved.meta_value["main"], "bin/index.js");

    let female = build_resolved(
        "bafywearable",
        FEMALE_PREFIX,
        "Hat".to_owned(),
        SCENE_JSON.as_bytes(),
        &packed_content(),
        String::new(),
    )
    .unwrap();
    assert_eq!(female.scene_id, "bafywearable-sw-female");
    assert_eq!(female.stripped.hash("bin/index.js").as_deref(), Some("if"));
}

#[test]
fn an_unprefixed_wearable_resolves_as_is() {
    let content = ContentMap::from_content(
        [
            ("scene.json", "s"),
            ("bin/index.js", "i"),
            ("bin/sdk-runtime.js", "r"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned())),
    );
    let resolved = build_resolved(
        "p",
        FEMALE_PREFIX,
        String::new(),
        SCENE_JSON.as_bytes(),
        &content,
        String::new(),
    )
    .unwrap();
    assert_eq!(resolved.stripped.0.len(), 3);
}

#[test]
fn a_scene_json_must_be_sdk7_with_its_main() {
    let sdk6 = SCENE_JSON.replace(r#""runtimeVersion": "7","#, "");
    let err = build_resolved(
        "h",
        MALE_PREFIX,
        String::new(),
        sdk6.as_bytes(),
        &packed_content(),
        String::new(),
    )
    .unwrap_err();
    assert!(err.contains("runtimeVersion"), "{err}");

    let no_main = SCENE_JSON.replace("bin/index.js", "bin/missing.js");
    let err = build_resolved(
        "h",
        MALE_PREFIX,
        String::new(),
        no_main.as_bytes(),
        &packed_content(),
        String::new(),
    )
    .unwrap_err();
    assert!(err.contains("bin/missing.js"), "{err}");

    assert!(build_resolved(
        "h",
        MALE_PREFIX,
        String::new(),
        b"{",
        &packed_content(),
        String::new()
    )
    .is_err());
}

#[test]
fn the_listing_has_a_header_and_one_line_per_entry() {
    let mut model = logged_in();
    equip(&mut model, &[HAT, CAPE]);
    let hat = normalize_urn(HAT);
    let cape = normalize_urn(CAPE);
    model.set_phase(&hat, ready(&hat, &[]));
    model.set_phase(&cape, Phase::Failed("wearable Missing".to_owned()));
    let listing = diagnostics::format_listing(&model.info(PortableExperiencesToggle::Enabled));
    assert_eq!(
        listing,
        format!("current scene allows: true\n{hat} running\n{cape} failed wearable Missing")
    );
    assert_eq!(
        diagnostics::format_listing(&Model::default().info(PortableExperiencesToggle::Enabled)),
        "no smart wearables"
    );
}

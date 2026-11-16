use super::*;
use crate::protocol::pulse::decentraland::{
    common::Position,
    kernel::comms::v3::{IslandData, IslandStatusMessage, ServiceDiscoveryMessage, ServiceStatus},
};
use prost::Message;

fn sample() -> Snapshot {
    Snapshot {
        peers: vec![
            Peer {
                address: "B".into(),
                position: [-0.1, 2., 32.],
                time: 123,
            },
            Peer {
                address: "a".into(),
                position: [16., 0., 0.],
                time: 99,
            },
            Peer {
                address: "A".into(),
                position: [-0.01, 0., 32.],
                time: -1,
            },
        ],
        ..Default::default()
    }
}
#[test]
fn renders_order_case_filters_time_and_floor_without_enrichment() {
    let s = sample();
    assert_eq!(
        render::peers(&s, &["a".into(), "B".into(), "a".into()]),
        r#"{"ok":true,"peers":[{"id":"B","address":"B","lastPing":123,"parcel":[-1,2],"position":[-0.1,2,32]},{"id":"a","address":"a","lastPing":99,"parcel":[1,0],"position":[16,0,0]}]}"#
    );
    assert_eq!(render::peers(&s, &["".into()]), r#"{"ok":true,"peers":[]}"#);
    assert_eq!(
        render::peer(&s, "b"),
        (404, r#"{"ok":false,"peer":null}"#.into())
    );
    assert_eq!(
        render::parcels(&s),
        r#"{"parcels":[{"peersCount":2,"parcel":{"x":-1,"y":2}},{"peersCount":1,"parcel":{"x":1,"y":0}}]}"#
    );
    assert_eq!(
        render::tiles(&s),
        vec![("-1,2".into(), 2), ("1,0".into(), 1)]
    );
}
#[test]
fn islands_keep_geometry_member_repeats_and_first_exact_id() {
    let mut s = sample();
    s.islands = IslandStatusMessage {
        data: vec![
            IslandData {
                id: "skip".into(),
                ..Default::default()
            },
            IslandData {
                id: "C1".into(),
                peers: vec!["a".into(), "unknown".into(), "A".into(), "a".into()],
                max_peers: 42,
                center: Some(Position {
                    x: 1.,
                    y: 2.,
                    z: 3.,
                }),
                radius: 1.0000000000000002,
            },
            IslandData {
                id: "C1".into(),
                center: Some(Position::default()),
                ..Default::default()
            },
        ],
    }
    .encode_to_vec();
    let all: serde_json::Value = serde_json::from_str(&render::islands(&s)).unwrap();
    assert_eq!(all["islands"].as_array().unwrap().len(), 2);
    let one = render::island(&s, "C1").unwrap();
    assert!(
        one.starts_with(
            r#"{"id":"C1","maxPeers":42,"center":[1,2,3],"radius":1.0000000000000002,"peers":[$"#
                .trim_end_matches('$')
        )
    );
    let one: serde_json::Value = serde_json::from_str(&one).unwrap();
    assert_eq!(
        one["peers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "A", "a"]
    );
    assert!(render::island(&s, "c1").is_none());
}
#[test]
fn discovery_uses_absolute_strict_age_and_advertised_count() {
    let mut s = sample();
    assert_eq!(
        render::core_status(&s, 100),
        r#"{"healthy":false,"userCount":0}"#
    );
    s.discovery = ServiceDiscoveryMessage {
        server_name: "core".into(),
        status: Some(ServiceStatus {
            current_time: 100000,
            user_count: 777,
            commit_hash: None,
        }),
    }
    .encode_to_vec();
    for (now, healthy) in [
        (10001, true),
        (10000, false),
        (189999, true),
        (190000, false),
    ] {
        assert_eq!(
            render::core_status(&s, now),
            format!(r#"{{"healthy":{healthy},"userCount":777}}"#)
        );
    }
}
#[test]
fn javascript_numbers_keep_boundaries_and_nonfinite_parcel_keys() {
    for (n, want) in [
        (0., "0"),
        (-0., "0"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (1e-6, "0.000001"),
        (1e-7, "1e-7"),
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
    ] {
        assert_eq!(render::number(n), want);
    }
    let s = Snapshot {
        peers: vec![Peer {
            address: "N".into(),
            position: [f64::NAN, f64::INFINITY, -0.],
            time: 1,
        }],
        ..Default::default()
    };
    assert_eq!(
        render::peers(&s, &[]),
        r#"{"ok":true,"peers":[{"id":"N","address":"N","lastPing":1,"parcel":[null,0],"position":[null,null,0]}]}"#
    );
    assert_eq!(render::tiles(&s), vec![("NaN,0".into(), 1)]);
}
#[test]
fn numeric_serialization_uses_javascript_shortest_tie_breaking() {
    assert_eq!(
        render::number(2.9802322387695312e-8),
        "2.9802322387695312e-8"
    );
    assert_eq!(
        render::json(&serde_json::json!({"number":1000000000000000100f64,"text":"x\\\"-1.0 λ"})),
        r#"{"number":1000000000000000100,"text":"x\\\"-1.0 λ"}"#
    );
}

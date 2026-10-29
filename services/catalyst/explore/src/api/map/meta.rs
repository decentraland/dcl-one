// from: atlas-server@b20adc5dbeb7555e80bacb0b2a84ecf599247c33:src/modules/district/data/{districts,contributions}.json (districts.json, contributions.json, byte for byte)
//! The atlas API's JSON routes: the OpenSea-shaped LAND and Estate metadata
//! (`/v2/parcels/{x}/{y}`, `/v2/estates/{id}`, `/v2/contracts/{address}/
//! tokens/{id}`), the district documents (`/v2/districts[/{id}]`,
//! `/v2/addresses/{address}/contributions`) and the probes (`/ping`,
//! `/ready`). The old handlers read the squid `parcel`/`estate`/`data`
//! tables; v2 has no squid database, so the token id, the name and the
//! estate membership come from the mirrored tile grid (upstream fills a
//! tile's `name` and `tokenId` from the same LAND data), the token id is
//! derived from the coordinates when the grid carries none, and the
//! description is empty (the grid has no description column). The district
//! and contribution documents are upstream's vendored data at the revision
//! production still serves (42 districts, 1,695 contributing addresses; the
//! old crate and atlas-server `8b2c3620`, #142, ship them as `[]`/`{}` while
//! `api.decentraland.org` kept answering the earlier data, which the
//! conformance fixtures `explore/api/district*` captured).

use std::collections::HashMap;
use std::sync::OnceLock;

use alloy_primitives::U256;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::super::Shared;
use super::{Grid, http_cache};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "map/"))]
pub struct NftAttribute {
    pub trait_type: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub value: i64,
    pub display_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "map/"))]
pub struct NftMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: String,
    pub external_url: Option<String>,
    pub background_color: String,
    pub attributes: Vec<NftAttribute>,
}

const PROXIMITY_JSON: &str = include_str!("proximity.json");
const DISTRICTS_JSON: &str = include_str!("districts.json");
const CONTRIBUTIONS_JSON: &str = include_str!("contributions.json");

#[derive(Debug, Clone, Deserialize)]
struct CoordProximity {
    district: Option<i64>,
    plaza: Option<i64>,
    road: Option<i64>,
}

fn proximity_table() -> &'static HashMap<String, CoordProximity> {
    static T: OnceLock<HashMap<String, CoordProximity>> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(PROXIMITY_JSON).expect("proximity.json must parse"))
}

fn min_proximity(coords: &[(i32, i32)]) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
    let t = proximity_table();
    let (mut district, mut plaza, mut road): (Option<i64>, Option<i64>, Option<i64>) =
        (None, None, None);
    let mut any = false;
    let keep_min = |slot: &mut Option<i64>, v: Option<i64>| {
        if let Some(v) = v
            && slot.is_none_or(|cur| v < cur)
        {
            *slot = Some(v);
        }
    };
    for (x, y) in coords {
        if let Some(cp) = t.get(&format!("{x},{y}")) {
            any = true;
            keep_min(&mut district, cp.district);
            keep_min(&mut plaza, cp.plaza);
            keep_min(&mut road, cp.road);
        }
    }
    any.then_some((district, plaza, road))
}

fn distance_attr(name: &str, value: i64) -> NftAttribute {
    NftAttribute {
        trait_type: format!("Distance to {name}"),
        value,
        display_type: "number".into(),
    }
}

/// The minimum distance to a district, plaza and road over `coords`, in
/// that order, each only when known for at least one coordinate.
pub fn append_proximity(attributes: &mut Vec<NftAttribute>, coords: &[(i32, i32)]) {
    let Some((district, plaza, road)) = min_proximity(coords) else {
        return;
    };
    if let Some(d) = district {
        attributes.push(distance_attr("District", d));
    }
    if let Some(p) = plaza {
        attributes.push(distance_attr("Plaza", p));
    }
    if let Some(r) = road {
        attributes.push(distance_attr("Road", r));
    }
}

pub fn districts() -> &'static Vec<Value> {
    static D: OnceLock<Vec<Value>> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(DISTRICTS_JSON).expect("districts.json must parse"))
}

pub fn district(id: &str) -> Option<&'static Value> {
    districts()
        .iter()
        .find(|d| d.get("id").and_then(Value::as_str) == Some(id))
}

fn contributions() -> &'static HashMap<String, Value> {
    static C: OnceLock<HashMap<String, Value>> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(CONTRIBUTIONS_JSON).expect("contributions.json must parse")
    })
}

pub fn contributions_for(address: &str) -> Value {
    contributions()
        .get(&address.to_lowercase())
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]))
}

/// LAND's `_encodeTokenId`: the signed x in the high 128 bits, the signed
/// y in the low 128 (`land_authz::decode_land_token_id` is the inverse),
/// as the decimal string the old squid rows carried.
pub fn encode_land_token_id(x: i32, y: i32) -> String {
    let hi = U256::from((x as i128) as u128).wrapping_shl(128);
    let lo = U256::from((y as i128) as u128);
    (hi | lo).to_string()
}

/// A decimal LAND token id back to its coordinates (the low 32 bits of
/// each half carry the sign, as the SQL twin decodes them).
pub fn decode_land_token_id(decimal: &str) -> Option<(i32, i32)> {
    let v = U256::from_str_radix(decimal.trim(), 10).ok()?;
    let hi: u128 = v.wrapping_shr(128).to::<u128>();
    let lo: u128 = (v & U256::from(u128::MAX)).to::<u128>();
    Some((hi as u32 as i32, lo as u32 as i32))
}

fn finalize(mut resp: Response, last: i64) -> Response {
    http_cache::apply(
        &mut resp,
        last,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
    );
    resp
}

fn not_ready() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "Not ready").into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "ok": false, "error": "Not Found" })),
    )
        .into_response()
}

fn ok_nft(nft: NftMetadata) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::to_value(nft).unwrap_or(Value::Null)),
    )
        .into_response()
}

pub(crate) fn parcel_nft(state: &Shared, grid: &Grid, x: i32, y: i32) -> Option<NftMetadata> {
    let tile = grid.tiles.get(&format!("{x},{y}"))?;
    let token_id = tile
        .token_id
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| encode_land_token_id(x, y));
    let mut attributes = vec![
        NftAttribute {
            trait_type: "X".into(),
            value: x as i64,
            display_type: "number".into(),
        },
        NftAttribute {
            trait_type: "Y".into(),
            value: y as i64,
            display_type: "number".into(),
        },
    ];
    append_proximity(&mut attributes, &[(x, y)]);
    let s = &state.settings;
    Some(NftMetadata {
        name: tile
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("Parcel {x},{y}")),
        description: String::new(),
        image: format!(
            "{}/parcels/{x}/{y}/map.png?size=24&width=1024&height=1024",
            s.map_image_base_url
        ),
        external_url: s.map_external_base_url.as_ref().map(|b| {
            format!(
                "{b}/contracts/{}/tokens/{token_id}",
                s.land_contract_address
            )
        }),
        background_color: "000000".into(),
        attributes,
        id: token_id,
    })
}

/// Owned parcels first, then every parcel carrying the estate id (the
/// `map.png` estate selection's rule); the estate's name is the name its
/// tiles carry.
pub(crate) fn estate_nft(state: &Shared, grid: &Grid, id: &str) -> Option<NftMetadata> {
    let coords: Vec<(i32, i32)> = grid
        .estates_owned
        .get(id)
        .filter(|v| !v.is_empty())
        .or_else(|| grid.estates_all.get(id))
        .cloned()
        .unwrap_or_default();
    if coords.is_empty() {
        return None;
    }
    let name = coords
        .iter()
        .filter_map(|(x, y)| grid.tiles.get(&format!("{x},{y}")))
        .filter_map(|t| t.name.clone())
        .find(|n| !n.is_empty())
        .unwrap_or_default();
    let mut attributes = vec![NftAttribute {
        trait_type: "Size".into(),
        value: coords.len() as i64,
        display_type: "number".into(),
    }];
    append_proximity(&mut attributes, &coords);
    let s = &state.settings;
    Some(NftMetadata {
        id: id.to_string(),
        name,
        description: String::new(),
        image: format!(
            "{}/estates/{id}/map.png?size=24&width=1024&height=1024",
            s.map_image_base_url
        ),
        external_url: s
            .map_external_base_url
            .as_ref()
            .map(|b| format!("{b}/contracts/{}/tokens/{id}", s.estate_contract_address)),
        background_color: "000000".into(),
        attributes,
    })
}

fn with_grid(state: &Shared, headers: &HeaderMap, f: impl FnOnce(&Grid) -> Response) -> Response {
    let last = state.map.last_updated_at();
    if let Some(r) = http_cache::not_modified(
        headers,
        last,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
    ) {
        return r;
    }
    let resp = match state.map.snapshot() {
        Some(grid) => f(&grid),
        None => not_ready(),
    };
    finalize(resp, last)
}

pub async fn get_parcel(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path((x, y)): Path<(String, String)>,
) -> Response {
    with_grid(&state, &headers, |grid| {
        let (Ok(xi), Ok(yi)) = (x.trim().parse::<i32>(), y.trim().parse::<i32>()) else {
            return (StatusCode::FORBIDDEN, "Invalid x or y").into_response();
        };
        match parcel_nft(&state, grid, xi, yi) {
            Some(nft) => ok_nft(nft),
            None => not_found(),
        }
    })
}

pub async fn get_estate(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    with_grid(&state, &headers, |grid| {
        if id.parse::<i64>().is_err() {
            return (StatusCode::FORBIDDEN, "Invalid id").into_response();
        }
        match estate_nft(&state, grid, &id) {
            Some(nft) => ok_nft(nft),
            None => not_found(),
        }
    })
}

/// A LAND token answers its parcel (immutable: the coordinates are the
/// id), an Estate token its estate; any other contract is 404.
pub async fn get_token(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path((address, id)): Path<(String, String)>,
) -> Response {
    let last = state.map.last_updated_at();
    if let Some(r) = http_cache::not_modified(
        &headers,
        last,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
    ) {
        return r;
    }
    let Some(grid) = state.map.snapshot() else {
        return finalize(not_ready(), last);
    };
    let addr = address.to_lowercase();
    if addr == state.settings.land_contract_address {
        let Some((x, y)) = decode_land_token_id(&id) else {
            return finalize(not_found(), last);
        };
        return match parcel_nft(&state, &grid, x, y) {
            Some(nft) => {
                let mut resp = ok_nft(nft);
                http_cache::apply_cache_control(
                    &mut resp,
                    last,
                    http_cache::LAND_IMMUTABLE_CACHE_CONTROL,
                );
                resp
            }
            None => finalize(not_found(), last),
        };
    }
    if addr == state.settings.estate_contract_address && id.parse::<i64>().is_ok() {
        return finalize(
            estate_nft(&state, &grid, &id)
                .map(ok_nft)
                .unwrap_or_else(not_found),
            last,
        );
    }
    finalize(not_found(), last)
}

pub async fn get_districts(State(state): State<Shared>, headers: HeaderMap) -> Response {
    let last = state.map.last_updated_at();
    if let Some(r) = http_cache::not_modified(
        &headers,
        last,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
    ) {
        return r;
    }
    finalize(
        Json(json!({ "ok": true, "data": districts() })).into_response(),
        last,
    )
}

pub async fn get_district(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let last = state.map.last_updated_at();
    if let Some(r) = http_cache::not_modified(
        &headers,
        last,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
    ) {
        return r;
    }
    let resp = match district(&id) {
        Some(d) => Json(json!({ "ok": true, "data": d })).into_response(),
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    };
    finalize(resp, last)
}

pub async fn get_contributions(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(address): Path<String>,
) -> Response {
    let last = state.map.last_updated_at();
    if let Some(r) = http_cache::not_modified(
        &headers,
        last,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
    ) {
        return r;
    }
    finalize(
        Json(json!({ "ok": true, "data": contributions_for(&address) })).into_response(),
        last,
    )
}

pub async fn ping() -> &'static str {
    "ok"
}

pub async fn ready(State(state): State<Shared>) -> Response {
    if state.map.is_ready() {
        (StatusCode::OK, "ok").into_response()
    } else {
        not_ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn land_token_ids_round_trip_with_the_contract_encoding() {
        assert_eq!(encode_land_token_id(0, 0), "0");
        assert_eq!(encode_land_token_id(0, 1), "1");
        assert_eq!(
            encode_land_token_id(1, 0),
            "340282366920938463463374607431768211456"
        );
        // -1,-1: both halves all ones.
        assert_eq!(
            encode_land_token_id(-1, -1),
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        );
        for (x, y) in [
            (0, 0),
            (1, 0),
            (0, 1),
            (-1, -1),
            (-150, 150),
            (102, -33),
            (-64, -128),
        ] {
            let id = encode_land_token_id(x, y);
            assert_eq!(decode_land_token_id(&id), Some((x, y)), "{x},{y} -> {id}");
            let hex = format!("0x{:064x}", U256::from_str_radix(&id, 10).unwrap());
            assert_eq!(
                crate::land_authz::decode_land_token_id(&hex),
                Some((x, y)),
                "the land_authz decoder agrees"
            );
        }
        assert_eq!(decode_land_token_id("nope"), None);
    }

    #[test]
    fn proximity_attributes_appended_for_known_coord() {
        let mut attrs: Vec<NftAttribute> = vec![
            NftAttribute {
                trait_type: "X".into(),
                value: 102,
                display_type: "number".into(),
            },
            NftAttribute {
                trait_type: "Y".into(),
                value: -33,
                display_type: "number".into(),
            },
        ];
        append_proximity(&mut attrs, &[(102, -33)]);
        assert!(attrs.len() > 2, "expected proximity traits appended");
        for a in &attrs[2..] {
            assert!(
                a.trait_type.starts_with("Distance to "),
                "unexpected trait_type: {}",
                a.trait_type
            );
            assert_eq!(a.display_type, "number");
        }
        assert!(
            attrs
                .iter()
                .any(|a| a.trait_type == "Distance to Road" && a.value == 4)
        );
        assert_eq!(attrs[2].trait_type, "Distance to Road");
    }

    #[test]
    fn proximity_noop_for_unknown_coord() {
        let mut attrs: Vec<NftAttribute> = vec![];
        append_proximity(&mut attrs, &[(99999, 99999)]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn proximity_takes_the_minimum_over_an_estate() {
        let mut attrs = vec![];
        append_proximity(&mut attrs, &[(102, -33), (18, -139)]);
        let road = attrs
            .iter()
            .find(|a| a.trait_type == "Distance to Road")
            .unwrap();
        assert_eq!(road.value, 0);
        let plaza = attrs
            .iter()
            .find(|a| a.trait_type == "Distance to Plaza")
            .unwrap();
        assert_eq!(plaza.value, 5);
        assert_eq!(
            attrs
                .iter()
                .map(|a| a.trait_type.as_str())
                .collect::<Vec<_>>(),
            ["Distance to Plaza", "Distance to Road"]
        );
    }

    /// The embedded district data is upstream's, not the empty files the old
    /// crate shipped: the SciArt Lab document of the conformance capture
    /// (`tests/fixtures/conformance/explore/api/district-by-id.json`).
    #[test]
    fn the_district_documents_are_upstreams_vendored_data() {
        let all = districts();
        assert_eq!(all.len(), 42);
        let plaza = district("55327350-d9f0-4cae-b0f3-8745a0431099").expect("Genesis Plaza");
        assert_eq!(
            (
                plaza["totalParcels"].clone(),
                plaza["parcels"].as_array().unwrap().len()
            ),
            (Value::Null, 3588),
            "served as upstream vendored it, null and all"
        );
        let sciart = district("ea2c3a3e-bbd6-49d9-814c-3b64b37b057a").expect("SciArt Lab");
        assert_eq!(sciart["name"], "SciArt Lab");
        assert_eq!(sciart["totalParcels"], 208);
        assert_eq!(sciart["parcels"][0], "12,61");
        assert!(district("no-such-district").is_none());
        let rows = contributions_for("0x004398361D0FAD60D7FCF5533D94398EE9FC8DDC");
        assert_eq!(
            rows,
            json!([{"address": "0x004398361d0fad60d7fcf5533d94398ee9fc8ddc", "districtId": "219ac351-e6ce-4e17-8b84-eb008afddf69", "totalParcels": 6}])
        );
        assert_eq!(
            contributions_for("0x8d6f63e382d73cf53858864f673f39e9ff915a1e"),
            json!([]),
            "the contributions capture's address has no rows"
        );
    }

    /// The wire shape the old parity test pinned (`fixtures/nft_parcel.json`).
    #[test]
    fn nft_metadata_envelope_parity() {
        let nft = NftMetadata {
            id: "0".into(),
            name: "Parcel 0,0".into(),
            description: String::new(),
            image: "https://example.test/v2/parcels/0/0/map.png?size=24&width=1024&height=1024"
                .into(),
            external_url: None,
            background_color: "000000".into(),
            attributes: vec![
                NftAttribute {
                    trait_type: "X".into(),
                    value: 0,
                    display_type: "number".into(),
                },
                NftAttribute {
                    trait_type: "Y".into(),
                    value: 0,
                    display_type: "number".into(),
                },
            ],
        };
        let wire = serde_json::to_string(&nft).unwrap();
        let keys: Vec<&str> = wire
            .split('"')
            .skip(1)
            .step_by(2)
            .filter(|k| {
                [
                    "id",
                    "name",
                    "description",
                    "image",
                    "external_url",
                    "background_color",
                    "attributes",
                ]
                .contains(k)
            })
            .collect();
        assert_eq!(
            keys,
            [
                "id",
                "name",
                "description",
                "image",
                "external_url",
                "background_color",
                "attributes"
            ],
            "NFT wire key order drifted"
        );
        let got: Value = serde_json::from_str(&wire).unwrap();
        assert!(
            got["external_url"].is_null(),
            "external_url must serialize null"
        );
        assert_eq!(got["background_color"], "000000");
        let attr = serde_json::to_string(&nft.attributes[0]).unwrap();
        assert!(
            attr.starts_with("{\"trait_type\":\"X\",\"value\":0,\"display_type\":\"number\"}"),
            "{attr}"
        );
    }
}

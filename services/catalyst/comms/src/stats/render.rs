//! Public JSON retains field order and JavaScript number/parcel semantics.
use crate::protocol::bus::statistics::{Peer, Snapshot};
use crate::protocol::pulse::decentraland::kernel::comms::v3::{
    IslandStatusMessage, ServiceDiscoveryMessage,
};
use prost::Message;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Number-to-string also supplies Map keys, where nonfinite values are strings.
pub fn number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "Infinity"
        } else {
            "-Infinity"
        }
        .into();
    }
    if value == 0. {
        return "0".into();
    }
    // serde_json's shortest-roundtrip formatter chooses even decimal ties.
    // Adapt its digits to ECMAScript's fixed/scientific notation thresholds.
    let source = serde_json::to_string(&value.abs()).expect("finite number");
    let (mantissa, exponent) = source
        .split_once(['e', 'E'])
        .map_or((source.as_str(), 0), |(m, e)| {
            (m, e.parse::<i32>().unwrap())
        });
    let mut point = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
    let mut digits = mantissa.replace('.', "");
    let leading = digits.bytes().take_while(|b| *b == b'0').count();
    digits.drain(..leading);
    point -= leading as i32;
    while digits.ends_with('0') {
        digits.pop();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if point > 0 && point <= 21 {
        if point as usize >= digits.len() {
            format!(
                "{sign}{digits}{}",
                "0".repeat(point as usize - digits.len())
            )
        } else {
            format!(
                "{sign}{}.{}",
                &digits[..point as usize],
                &digits[point as usize..]
            )
        }
    } else if point <= 0 && point > -6 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else {
        let mantissa = if digits.len() == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let exponent = point - 1;
        format!(
            "{sign}{mantissa}e{}{exponent}",
            if exponent >= 0 { "+" } else { "" }
        )
    }
}

/// Serialize structs in declaration order, then normalize only JSON numeric tokens.
/// Quoted metadata and escaped strings are never interpreted as numbers.
pub fn json<T: Serialize>(value: &T) -> String {
    let source = serde_json::to_string(value).expect("statistics JSON serialization");
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let mut start = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else if bytes[i] == b'-' || bytes[i].is_ascii_digit() {
            out.push_str(&source[start..i]);
            let from = i;
            i += 1;
            while i < bytes.len()
                && matches!(bytes[i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
            {
                i += 1;
            }
            let n = source[from..i].parse::<f64>().expect("JSON number");
            if n.is_finite() {
                out.push_str(&number(n));
            } else {
                out.push_str("null");
            }
            start = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&source[start..]);
    out
}

#[derive(Serialize)]
pub struct PublicPeer<'a> {
    pub id: &'a str,
    pub address: &'a str,
    #[serde(rename = "lastPing")]
    pub last_ping: i64,
    pub parcel: [f64; 2],
    pub position: [f64; 3],
}
impl<'a> From<&'a Peer> for PublicPeer<'a> {
    fn from(p: &'a Peer) -> Self {
        Self {
            id: &p.address,
            address: &p.address,
            last_ping: p.time,
            parcel: parcel(p),
            position: p.position,
        }
    }
}
pub fn parcel(p: &Peer) -> [f64; 2] {
    [(p.position[0] / 16.).floor(), (p.position[2] / 16.).floor()]
}
#[derive(Serialize)]
struct Peers<'a> {
    ok: bool,
    peers: Vec<PublicPeer<'a>>,
}
#[derive(Serialize)]
struct OnePeer<'a> {
    ok: bool,
    peer: Option<PublicPeer<'a>>,
}
pub fn peers(snapshot: &Snapshot, ids: &[String]) -> String {
    let ids: HashSet<_> = ids.iter().map(String::as_str).collect();
    json(&Peers {
        ok: true,
        peers: snapshot
            .peers
            .iter()
            .filter(|p| ids.is_empty() || ids.contains(p.address.as_str()))
            .map(Into::into)
            .collect(),
    })
}
pub fn peer(snapshot: &Snapshot, id: &str) -> (u16, String) {
    let peer = snapshot
        .peers
        .iter()
        .find(|p| p.address == id)
        .map(Into::into);
    (
        if peer.is_some() { 200 } else { 404 },
        json(&OnePeer {
            ok: peer.is_some(),
            peer,
        }),
    )
}
#[derive(Serialize)]
struct Parcel {
    x: f64,
    y: f64,
}
#[derive(Serialize)]
struct ParcelCount {
    #[serde(rename = "peersCount")]
    peers_count: usize,
    parcel: Parcel,
}
#[derive(Serialize)]
struct Parcels {
    parcels: Vec<ParcelCount>,
}
pub fn parcels(snapshot: &Snapshot) -> String {
    let mut index = HashMap::new();
    let mut parcels: Vec<ParcelCount> = Vec::new();
    for peer in &snapshot.peers {
        let [x, y] = parcel(peer);
        let key = format!("{}:{}", number(x), number(y));
        let i = *index.entry(key).or_insert_with(|| {
            parcels.push(ParcelCount {
                peers_count: 0,
                parcel: Parcel { x, y },
            });
            parcels.len() - 1
        });
        parcels[i].peers_count += 1;
    }
    json(&Parcels { parcels })
}
pub fn tiles(snapshot: &Snapshot) -> Vec<(String, usize)> {
    let mut index = HashMap::new();
    let mut tiles: Vec<(String, usize)> = Vec::new();
    for peer in &snapshot.peers {
        let [x, y] = parcel(peer);
        let key = format!("{},{}", number(x), number(y));
        let i = *index.entry(key.clone()).or_insert_with(|| {
            tiles.push((key, 0));
            tiles.len() - 1
        });
        tiles[i].1 += 1;
    }
    tiles
}
#[derive(Serialize)]
pub struct Island<'a> {
    pub id: String,
    #[serde(rename = "maxPeers")]
    pub max_peers: u32,
    pub center: [f64; 3],
    pub radius: f64,
    pub peers: Vec<PublicPeer<'a>>,
}
pub fn public_islands(snapshot: &Snapshot) -> Vec<Island<'_>> {
    let peers: HashMap<_, _> = snapshot
        .peers
        .iter()
        .map(|p| (p.address.as_str(), p))
        .collect();
    IslandStatusMessage::decode(snapshot.islands.as_slice())
        .unwrap_or_default()
        .data
        .into_iter()
        .filter_map(|island| {
            let center = island.center?;
            Some(Island {
                id: island.id,
                max_peers: island.max_peers,
                center: [center.x as f64, center.y as f64, center.z as f64],
                radius: island.radius,
                peers: island
                    .peers
                    .iter()
                    .filter_map(|id| peers.get(id.as_str()).map(|p| PublicPeer::from(*p)))
                    .collect(),
            })
        })
        .collect()
}
pub fn islands(snapshot: &Snapshot) -> String {
    #[derive(Serialize)]
    struct Islands<'a> {
        ok: bool,
        islands: Vec<Island<'a>>,
    }
    json(&Islands {
        ok: true,
        islands: public_islands(snapshot),
    })
}
pub fn island(snapshot: &Snapshot, id: &str) -> Option<String> {
    public_islands(snapshot)
        .into_iter()
        .find(|island| island.id == id)
        .map(|island| json(&island))
}
pub fn core_status(snapshot: &Snapshot, now: i64) -> String {
    json(&core(snapshot, now))
}
#[derive(Serialize)]
pub struct Core {
    pub healthy: bool,
    #[serde(rename = "userCount")]
    pub user_count: u32,
}
pub fn core(snapshot: &Snapshot, now: i64) -> Core {
    let status = ServiceDiscoveryMessage::decode(snapshot.discovery.as_slice())
        .ok()
        .and_then(|d| d.status);
    Core {
        healthy: status
            .as_ref()
            .is_some_and(|s| (now as f64 - s.current_time as f64).abs() < 90000.),
        user_count: status.map_or(0, |s| s.user_count),
    }
}

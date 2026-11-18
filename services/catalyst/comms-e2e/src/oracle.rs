//! Sources of truth outside the bots: server metrics and the statistics routes.
//!
//! The live stack serves the statistics routes at the API root (`/peers`, `/islands`); the
//! connector also answers them under `/comms` and `/stats`, which this module does not use.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use url::Url;

const TIMEOUT: Duration = Duration::from_secs(10);

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().timeout(TIMEOUT).build()?)
}

async fn get_text(url: &Url) -> Result<String> {
    let response = client()?
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("GET {url} answered {status}");
    }
    response
        .text()
        .await
        .with_context(|| format!("reading {url}"))
}

/// One line of a Prometheus text exposition.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub value: f64,
}

/// A scrape of a `/metrics` route.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    pub samples: Vec<Sample>,
}

impl Metrics {
    /// Reads the Prometheus text format; comment lines are skipped and a line that does not parse
    /// is an error.
    pub fn parse(text: &str) -> Result<Metrics> {
        let mut samples = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            samples.push(
                parse_sample(line)
                    .with_context(|| format!("metrics line {}: {line:?}", number + 1))?,
            );
        }
        Ok(Metrics { samples })
    }

    /// The sample called `name` whose labels include every pair of `labels`. With no pairs it is
    /// the sample that has no labels at all.
    pub fn value(&self, name: &str, labels: &[(&str, &str)]) -> Option<f64> {
        self.samples
            .iter()
            .find(|sample| {
                sample.name == name
                    && if labels.is_empty() {
                        sample.labels.is_empty()
                    } else {
                        labels.iter().all(|(key, value)| {
                            sample.labels.get(*key).is_some_and(|held| held == value)
                        })
                    }
            })
            .map(|sample| sample.value)
    }

    /// The total of every sample called `name`, whatever its labels; 0 when there is none.
    pub fn sum(&self, name: &str) -> f64 {
        self.samples
            .iter()
            .filter(|sample| sample.name == name)
            .map(|sample| sample.value)
            .sum()
    }

    /// The total of the samples called `name` whose labels include every pair of `labels`.
    pub fn sum_where(&self, name: &str, labels: &[(&str, &str)]) -> f64 {
        self.samples
            .iter()
            .filter(|sample| {
                sample.name == name
                    && labels.iter().all(|(key, value)| {
                        sample.labels.get(*key).is_some_and(|held| held == value)
                    })
            })
            .map(|sample| sample.value)
            .sum()
    }
}

fn parse_sample(line: &str) -> Result<Sample> {
    let (name, rest) = match line.find(['{', ' ']) {
        Some(position) => line.split_at(position),
        None => bail!("no value"),
    };
    let (labels, rest) = if let Some(inner) = rest.strip_prefix('{') {
        parse_labels(inner)?
    } else {
        (BTreeMap::new(), rest)
    };
    // a value, then an optional timestamp
    let value = rest
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("no value"))?;
    let value = match value {
        "+Inf" | "Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        "NaN" => f64::NAN,
        other => other
            .parse::<f64>()
            .map_err(|_| anyhow!("{other:?} is not a number"))?,
    };
    Ok(Sample {
        name: name.to_owned(),
        labels,
        value,
    })
}

/// Reads `key="value",...}` and returns the labels and what follows the closing brace.
fn parse_labels(text: &str) -> Result<(BTreeMap<String, String>, &str)> {
    let mut labels = BTreeMap::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix('}') {
            return Ok((labels, after));
        }
        let (key, after) = rest
            .split_once('=')
            .ok_or_else(|| anyhow!("a label without `=`"))?;
        let after = after
            .trim_start()
            .strip_prefix('"')
            .ok_or_else(|| anyhow!("a label value without a quote"))?;
        let mut value = String::new();
        let mut chars = after.char_indices();
        let end = loop {
            match chars.next() {
                Some((_, '\\')) => match chars.next() {
                    Some((_, 'n')) => value.push('\n'),
                    Some((_, other)) => value.push(other),
                    None => bail!("a label value ends inside an escape"),
                },
                Some((index, '"')) => break index,
                Some((_, other)) => value.push(other),
                None => bail!("a label value is not closed"),
            }
        };
        labels.insert(key.trim().to_owned(), value);
        rest = after[end + 1..].trim_start();
        rest = rest.strip_prefix(',').unwrap_or(rest);
    }
}

/// Scrapes `url` (a `/metrics` route).
pub async fn metrics(url: &Url) -> Result<Metrics> {
    Metrics::parse(&get_text(url).await?).with_context(|| format!("the metrics of {url}"))
}

/// One peer as the statistics route lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub address: String,
    /// The island the `/islands` route lists the peer in, if any.
    pub island: Option<String>,
    pub last_ping_ms: i64,
    pub parcel: [f64; 2],
    pub position: [f64; 3],
}

/// One island and its peers.
#[derive(Debug, Clone, PartialEq)]
pub struct Island {
    pub id: String,
    pub max_peers: u64,
    pub peers: Vec<Peer>,
}

#[derive(Deserialize)]
struct RawPeer {
    address: String,
    #[serde(rename = "lastPing", default)]
    last_ping: i64,
    #[serde(default)]
    parcel: [Option<f64>; 2],
    #[serde(default)]
    position: [Option<f64>; 3],
}

impl RawPeer {
    fn into_peer(self, island: Option<&str>) -> Peer {
        let or_nan = |value: Option<f64>| value.unwrap_or(f64::NAN);
        Peer {
            address: self.address,
            island: island.map(str::to_owned),
            last_ping_ms: self.last_ping,
            parcel: self.parcel.map(or_nan),
            position: self.position.map(or_nan),
        }
    }
}

#[derive(Deserialize)]
struct RawIsland {
    id: String,
    #[serde(rename = "maxPeers", default)]
    max_peers: u64,
    #[serde(default)]
    peers: Vec<RawPeer>,
}

/// The islands the statistics route lists.
pub fn parse_islands(text: &str) -> Result<Vec<Island>> {
    #[derive(Deserialize)]
    struct Reply {
        islands: Vec<RawIsland>,
    }
    let reply: Reply = serde_json::from_str(text).context("the islands reply")?;
    Ok(reply
        .islands
        .into_iter()
        .map(|island| {
            let id = island.id;
            let peers = island
                .peers
                .into_iter()
                .map(|peer| peer.into_peer(Some(&id)))
                .collect();
            Island {
                id,
                max_peers: island.max_peers,
                peers,
            }
        })
        .collect())
}

/// The peers the statistics route lists, each with the island `/islands` puts it in.
pub fn parse_peers(text: &str, islands: &[Island]) -> Result<Vec<Peer>> {
    #[derive(Deserialize)]
    struct Reply {
        peers: Vec<RawPeer>,
    }
    let reply: Reply = serde_json::from_str(text).context("the peers reply")?;
    Ok(reply
        .peers
        .into_iter()
        .map(|peer| {
            let island = islands
                .iter()
                .find(|island| island.peers.iter().any(|held| held.address == peer.address))
                .map(|island| island.id.as_str());
            peer.into_peer(island)
        })
        .collect())
}

fn route(api: &Url, path: &str) -> Result<Url> {
    api.join(path).with_context(|| format!("{path} on {api}"))
}

/// `GET /islands` of the API at `api`.
pub async fn islands(api: &Url) -> Result<Vec<Island>> {
    parse_islands(&get_text(&route(api, "/islands")?).await?)
}

/// `GET /peers` of the API at `api`, with each peer's island from `GET /islands`. The two reads
/// are not one snapshot: a peer that moved between them may show no island.
pub async fn peers(api: &Url) -> Result<Vec<Peer>> {
    let islands = islands(api).await?;
    parse_peers(&get_text(&route(api, "/peers")?).await?, &islands)
}

/// The entries of `peers` whose address is `address` (case-insensitive): more than one is a ghost.
pub fn of_wallet<'a>(peers: &'a [Peer], address: &str) -> Vec<&'a Peer> {
    peers
        .iter()
        .filter(|peer| peer.address.eq_ignore_ascii_case(address))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPOSITION: &str = r#"
# HELP ws_errors errors
# TYPE ws_errors counter
ws_errors 3
requests_total{route="/ws",code="200"} 7
requests_total{route="/peers",code="200"} 5
requests_total{route="/ws",code="429"} 1
odd{note="a \"quoted\" , value",kind="x"} 2.5 1700000000
pulse_nats_connected 1
latency_bucket{le="+Inf"} 12
"#;

    #[test]
    fn the_prometheus_text_is_read_with_its_labels() {
        let metrics = Metrics::parse(EXPOSITION).unwrap();
        assert_eq!(metrics.value("ws_errors", &[]), Some(3.0));
        assert_eq!(
            metrics.value("requests_total", &[("route", "/ws"), ("code", "429")]),
            Some(1.0)
        );
        assert_eq!(
            metrics.value("requests_total", &[("route", "/ws")]),
            Some(7.0)
        );
        // a labelled-only metric has no unlabelled sample
        assert_eq!(metrics.value("requests_total", &[]), None);
        assert_eq!(metrics.value("requests_total", &[("route", "/none")]), None);
        assert_eq!(metrics.value("odd", &[("kind", "x")]), Some(2.5));
        assert_eq!(
            metrics.value("odd", &[("note", "a \"quoted\" , value")]),
            Some(2.5)
        );
        assert_eq!(
            metrics.value("latency_bucket", &[("le", "+Inf")]),
            Some(12.0)
        );
    }

    #[test]
    fn sums_add_every_label_set_of_one_name() {
        let metrics = Metrics::parse(EXPOSITION).unwrap();
        assert_eq!(metrics.sum("requests_total"), 13.0);
        assert_eq!(
            metrics.sum_where("requests_total", &[("route", "/ws")]),
            8.0
        );
        assert_eq!(metrics.sum("missing"), 0.0);
        // a name that only starts like another is not it
        assert_eq!(metrics.sum("request"), 0.0);
    }

    #[test]
    fn a_line_that_is_not_a_sample_is_an_error_naming_it() {
        let error = Metrics::parse("fine 1\nbroken\n").unwrap_err();
        assert!(format!("{error:#}").contains("broken"), "{error:#}");
        assert!(Metrics::parse("x{a=\"1\" 1").is_err());
        assert!(Metrics::parse("x notanumber").is_err());
    }

    const ISLANDS: &str = r#"{"ok":true,"islands":[
        {"id":"I1","maxPeers":100,"center":[0,0,0],"radius":150,"peers":[
            {"id":"0xa","address":"0xa","lastPing":5,"parcel":[0,0],"position":[1,0,2]},
            {"id":"0xb","address":"0xb","lastPing":6,"parcel":[1,0],"position":[17,0,2]}]},
        {"id":"I2","maxPeers":100,"center":[0,0,0],"radius":150,"peers":[
            {"id":"0xc","address":"0xc","lastPing":7,"parcel":[null,0],"position":[null,null,0]}]}]}"#;
    const PEERS: &str = r#"{"ok":true,"peers":[
        {"id":"0xb","address":"0xb","lastPing":6,"parcel":[1,0],"position":[17,0,2]},
        {"id":"0xd","address":"0xd","lastPing":9,"parcel":[3,3],"position":[48,0,48]}]}"#;

    #[test]
    fn islands_and_peers_are_read_and_joined() {
        let islands = parse_islands(ISLANDS).unwrap();
        assert_eq!(islands.len(), 2);
        assert_eq!(islands[0].id, "I1");
        assert_eq!(islands[0].peers.len(), 2);
        assert_eq!(islands[0].peers[1].position, [17.0, 0.0, 2.0]);
        assert!(islands[1].peers[0].parcel[0].is_nan());

        let peers = parse_peers(PEERS, &islands).unwrap();
        assert_eq!(peers[0].island.as_deref(), Some("I1"));
        // a peer no island lists has none
        assert_eq!(peers[1].island, None);
        assert_eq!(of_wallet(&peers, "0xB").len(), 1);
        assert!(of_wallet(&peers, "0xzz").is_empty());
    }

    #[test]
    fn a_reply_that_is_not_the_documented_shape_is_an_error() {
        assert!(parse_islands("{\"ok\":true}").is_err());
        assert!(parse_peers("[]", &[]).is_err());
    }
}

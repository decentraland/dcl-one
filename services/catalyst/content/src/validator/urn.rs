//! Decentraland asset URNs as `@dcl/urn-resolver`'s `parseUrn` resolves them:
//! the input is read as a WHATWG URL (`url`), a `urn:` path is matched
//! against the resolver's routes (case-sensitive literals, `[^:]+` groups,
//! each group percent-decoded) and a legacy `dcl://` address is rewritten to
//! its URN first. A URN that matches no route, or names an unknown network or
//! a network without the contract the route needs, does not resolve.
//! `parseUrn` throws where the input is not a URL, or where a path matches a
//! route but one of its groups holds a malformed percent escape.

use url::Url;

/// The type `parseUrn` gives a resolved URN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrnType {
    OffChain,
    Entity,
    Land,
    Erc721,
    CollectionV1,
    CollectionV1Asset,
    CollectionV1Item,
    CollectionV2,
    CollectionV2Asset,
    CollectionV2Item,
    ThirdPartyName,
    ThirdPartyCollection,
    ThirdParty,
    ThirdPartyItem,
}

impl UrnType {
    /// A collections-v1 or v2 asset: an item without its token id.
    pub fn is_collection_asset(self) -> bool {
        matches!(self, Self::CollectionV1Asset | Self::CollectionV2Asset)
    }
}

/// The networks the resolver accepts.
const NETWORKS: &[&str] = &[
    "ethereum", "goerli", "sepolia", "matic", "mumbai", "amoy", "arbitrum",
];
/// The networks with a third-party registry (`TPR`) contract.
const TPR_NETWORKS: &[&str] = &["matic", "mumbai", "amoy"];
/// The networks with a LAND contract (`LANDProxy`).
const LAND_NETWORKS: &[&str] = &["ethereum", "sepolia", "goerli"];

/// Where `parseUrn` throws instead of resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throws;

/// A URN `parseUrn` resolved: its type and its decoded path parts (the URN
/// without `urn:`, split on `:`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub kind: UrnType,
    parts: Vec<String>,
}

impl Parsed {
    /// The decoded `{registry}` of an off-chain URN
    /// (`decentraland:off-chain:{registry}:{name}`).
    pub fn off_chain_registry(&self) -> Option<&str> {
        match (self.kind, self.parts.as_slice()) {
            (UrnType::OffChain, [_, _, registry, _]) => Some(registry),
            _ => None,
        }
    }
}

/// `parseUrn(input)`: `Ok(None)` where it resolves nothing, `Err` where it
/// throws.
pub fn parse_urn(input: &str) -> Result<Option<Parsed>, Throws> {
    Ok(resolved(input)?.map(|(parsed, _)| parsed))
}

/// The resolved type of `input`, `None` when `parseUrn` resolves nothing or
/// throws.
pub fn parse(input: &str) -> Option<UrnType> {
    parse_urn(input).ok().flatten().map(|parsed| parsed.kind)
}

/// `input`'s segments as the resolver reads them, each as `(raw, decoded)`:
/// `urn`, then each URL path segment (no surrounding whitespace, tab or
/// newline, query or fragment), a legacy `dcl://` address as its URN; `None`
/// when it does not resolve.
pub fn segments(input: &str) -> Option<Vec<(String, String)>> {
    let (_, urn) = resolved(input).ok().flatten()?;
    let path = urn.strip_prefix("urn:")?;
    let mut segments = vec![("urn".to_string(), "urn".to_string())];
    for part in path.split(':') {
        segments.push((part.to_string(), decode(part)?));
    }
    Some(segments)
}

/// Whether a collections-v1 collection segment routes by contract, as
/// `resolve` decides it: on the raw spelling.
pub fn is_v1_contract(raw: &str) -> bool {
    is_hex(raw)
}

fn resolved(input: &str) -> Result<Option<(Parsed, String)>, Throws> {
    let url = Url::parse(input).map_err(|_| Throws)?;
    match url.scheme() {
        "urn" => Ok(resolve(url.path())?.map(|parsed| (parsed, format!("urn:{}", url.path())))),
        "dcl" => {
            let Some(urn) = legacy_dcl_urn(&url) else {
                return Ok(None);
            };
            Ok(parse_urn_scheme(&urn)?.map(|parsed| (parsed, urn)))
        }
        _ => Ok(None),
    }
}

fn parse_urn_scheme(input: &str) -> Result<Option<Parsed>, Throws> {
    let url = Url::parse(input).map_err(|_| Throws)?;
    if url.scheme() == "urn" {
        resolve(url.path())
    } else {
        Ok(None)
    }
}

/// `dcl://{collection}/{name}`: a base avatar or emote, or a collections-v1
/// asset on Ethereum named by its collection.
fn legacy_dcl_urn(url: &Url) -> Option<String> {
    let path = url.path();
    let (host, segments): (String, Vec<&str>) = match path.strip_prefix("//") {
        Some(rest) => {
            let mut parts = rest.split('/');
            let host = parts.next().unwrap_or_default().to_string();
            (host, parts.collect())
        }
        None => {
            let host = match (url.host_str(), url.port()) {
                (Some(host), Some(port)) => format!("{host}:{port}"),
                (Some(host), None) => host.to_string(),
                (None, _) => String::new(),
            };
            (
                host,
                path.strip_prefix('/').unwrap_or(path).split('/').collect(),
            )
        }
    };
    let [name] = segments[..] else {
        return None;
    };
    Some(match host.as_str() {
        "base-avatars" | "base-emotes" => format!("urn:decentraland:off-chain:{host}:{name}"),
        _ => format!("urn:decentraland:ethereum:collections-v1:{host}:{name}"),
    })
}

/// `decodeURIComponent`: `None` on a malformed escape or invalid UTF-8.
fn decode(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'%'
            && !bytes
                .get(i + 1..i + 3)
                .is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit))
        {
            return None;
        }
    }
    urlencoding::decode(segment).ok().map(|s| s.into_owned())
}

fn is_hex(s: &str) -> bool {
    s.strip_prefix("0x")
        .is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn resolve(path: &str) -> Result<Option<Parsed>, Throws> {
    let parts: Vec<&str> = path.split(':').collect();
    // Every route segment is a literal or a non-empty `[^:]+` group.
    if parts.iter().any(|p| p.is_empty()) {
        return Ok(None);
    }
    let Some(decoded) = parts.iter().map(|p| decode(p)).collect::<Option<Vec<_>>>() else {
        // A literal never holds a `%`, so the malformed escape is in a group
        // of every route the path matches, and decoding it throws.
        return if matches_a_route(&parts) {
            Err(Throws)
        } else {
            Ok(None)
        };
    };
    Ok(route(&parts, &decoded).map(|kind| Parsed {
        kind,
        parts: decoded,
    }))
}

/// Whether `parts` matches one of the resolver's route patterns, before its
/// handler checks the network, contract or position.
fn matches_a_route(parts: &[&str]) -> bool {
    let item_id = |id: &str| is_hex(id) || is_digits(id);
    match parts {
        ["decentraland", "off-chain", _, _] | ["decentraland", "entity", _] => true,
        ["decentraland", _, "collections-v1", rest @ ..] => match rest {
            [_] | [_, _] => true,
            [_, _, token] => is_digits(token),
            _ => false,
        },
        ["decentraland", _, "collections-v2", contract, rest @ ..] if is_hex(contract) => {
            match rest {
                [] => true,
                [id] => item_id(id),
                [id, token] => item_id(id) && is_digits(token),
                _ => false,
            }
        }
        ["decentraland", _, "LAND", _] => true,
        ["decentraland", _, "collections-thirdparty", rest @ ..] => match rest {
            [_] | [_, _] | [_, _, _] => true,
            [_, _, _, _, contract, token] => is_hex(contract) && is_digits(token),
            _ => false,
        },
        ["decentraland", _, "erc721", contract, _] => is_hex(contract),
        _ => false,
    }
}

/// The type `parseUrn` gives a path whose groups all decode.
fn route(parts: &[&str], decoded: &[String]) -> Option<UrnType> {
    let in_set = |set: &[&str], network: &str| set.contains(&network.to_lowercase().as_str());
    match parts[..] {
        ["decentraland", "off-chain", _, _] => Some(UrnType::OffChain),
        ["decentraland", "entity", _] => Some(UrnType::Entity),
        ["decentraland", _, "collections-v1", ref rest @ ..] => collection_v1(
            &decoded[1],
            in_set(NETWORKS, &decoded[1]),
            rest,
            &decoded[3..],
        ),
        ["decentraland", _, "collections-v2", ref rest @ ..] if in_set(NETWORKS, &decoded[1]) => {
            collection_v2(rest)
        }
        ["decentraland", _, "LAND", _] if in_set(LAND_NETWORKS, &decoded[1]) => {
            land_position_resolves(&decoded[3]).then_some(UrnType::Land)
        }
        ["decentraland", _, "collections-thirdparty", ref rest @ ..]
            if in_set(TPR_NETWORKS, &decoded[1]) =>
        {
            match rest {
                [_] => Some(UrnType::ThirdPartyName),
                [_, _] => Some(UrnType::ThirdPartyCollection),
                [_, _, _] => Some(UrnType::ThirdParty),
                [_, _, _, _, contract, token] if is_hex(contract) && is_digits(token) => {
                    Some(UrnType::ThirdPartyItem)
                }
                _ => None,
            }
        }
        ["decentraland", _, "erc721", contract, _]
            if in_set(NETWORKS, &decoded[1]) && is_hex(contract) =>
        {
            Some(UrnType::Erc721)
        }
        _ => None,
    }
}

/// A collections-v1 route: by contract on any network, else by collection
/// name on `ethereum` only (spelled exactly so).
fn collection_v1(
    network: &str,
    network_known: bool,
    rest: &[&str],
    decoded: &[String],
) -> Option<UrnType> {
    let by_contract = network_known && rest.first().is_some_and(|c| is_hex(c));
    let by_name = network == "ethereum";
    match rest {
        [_] if by_contract => Some(UrnType::CollectionV1),
        [_] if by_name => super::onchain::v1_collection_contract(&decoded[0])
            .is_some()
            .then_some(UrnType::CollectionV1),
        [_, _] if by_contract || by_name => Some(UrnType::CollectionV1Asset),
        [_, _, token] if is_digits(token) && (by_contract || by_name) => {
            Some(UrnType::CollectionV1Item)
        }
        _ => None,
    }
}

fn collection_v2(rest: &[&str]) -> Option<UrnType> {
    let item_id = |id: &str| is_hex(id) || is_digits(id);
    match rest {
        [contract] if is_hex(contract) => Some(UrnType::CollectionV2),
        [contract, id] if is_hex(contract) && item_id(id) => Some(UrnType::CollectionV2Asset),
        [contract, id, token] if is_hex(contract) && item_id(id) && is_digits(token) => {
            Some(UrnType::CollectionV2Item)
        }
        _ => None,
    }
}

/// `parseInt(s, 10)`: optional whitespace and sign, then the leading digits.
fn parse_int_prefix(s: &str) -> Option<i128> {
    let s = s.trim_start();
    let (negative, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let end = digits.bytes().take_while(u8::is_ascii_digit).count();
    let value: i128 = digits[..end].get(..end.min(30))?.parse().ok()?;
    Some(if negative { -value } else { value })
}

/// A LAND position: `x,y` within the contract's bounds, or a token id that
/// decodes to one.
fn land_position_resolves(position: &str) -> bool {
    const BOUND: i128 = 1_000_000;
    let in_bounds =
        |x: i128, y: i128| (-BOUND + 1..BOUND).contains(&x) && (-BOUND + 1..BOUND).contains(&y);
    let mut coords = position.trim().split(',').map(parse_int_prefix);
    if let (Some(Some(x)), Some(Some(y))) = (coords.next(), coords.next()) {
        return in_bounds(x, y);
    }
    let trimmed = position.trim();
    let token = match trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(hex) => alloy_primitives::U256::from_str_radix(hex, 16).ok(),
        None if is_digits(trimmed) => alloy_primitives::U256::from_str_radix(trimmed, 10).ok(),
        None => None,
    };
    let Some(token) = token else {
        return false;
    };
    let half = |v: alloy_primitives::U256| -> i128 {
        let low =
            v & ((alloy_primitives::U256::from(1u8) << 128) - alloy_primitives::U256::from(1u8));
        let raw = u128::try_from(low).unwrap_or_default();
        raw as i128
    };
    in_bounds(half(token >> 128), half(token))
}

#[cfg(test)]
mod tests {
    use super::UrnType::*;
    use super::*;

    const C: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";

    #[test]
    fn the_resolver_vectors_resolve_to_their_types() {
        let cases = [
            (
                "urn:decentraland:off-chain:base-avatars:f_sweater",
                OffChain,
            ),
            (
                "urn:decentraland:off-chain:static-portable-experiences:quest-1",
                OffChain,
            ),
            (
                "urn:decentraland:sepolia:LAND:-10,-13?atBlock=151231111",
                Land,
            ),
            (
                "urn:decentraland:sepolia:LAND:0x1?atBlock=151231111#4",
                Land,
            ),
            ("urn:decentraland:ETHEREUM:LAND:0x1", Land),
            ("urn:decentraland:ethereum:LAND:0,0", Land),
            ("urn:decentraland:ethereum:LAND:0%2C0", Land),
            ("urn:decentraland:ethereum:LAND:13,-137", Land),
            (
                "urn:decentraland:ethereum:LAND:0xdffffffffffffffffffffffffffffff77",
                Land,
            ),
            (
                "urn:decentraland:ethereum:LAND:4763953136893138488487244504044754960247",
                Land,
            ),
            (
                "urn:decentraland:ethereum:collections-v1:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d",
                CollectionV1,
            ),
            (
                "urn:decentraland:ethereum:collections-v1:community_contest",
                CollectionV1,
            ),
            (
                "urn:decentraland:ethereum:collections-v1:0x32b7495895264ac9d0b12d32afd435453458b1c6:cw_bell_attendant_hat",
                CollectionV1Asset,
            ),
            (
                "urn:decentraland:ethereum:collections-v1:community_contest:cw_bell_attendant_hat",
                CollectionV1Asset,
            ),
            (
                "urn:decentraland:ethereum:collections-v1:InExIsTeNtCoLlEcTiOn19283719:maddona-modern-life",
                CollectionV1Asset,
            ),
            (
                "urn:decentraland:ethereum:collections-v1:community_contest:cw_bell_attendant_hat:789",
                CollectionV1Item,
            ),
            (
                "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d",
                CollectionV2,
            ),
            (
                "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:0",
                CollectionV2Asset,
            ),
            (
                "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:0x1",
                CollectionV2Asset,
            ),
            (
                "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:0x000011111111111abcdef9087654321",
                CollectionV2Asset,
            ),
            (
                "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:123456789",
                CollectionV2Asset,
            ),
            (
                "urn:decentraland:amoy:collections-thirdparty:aThirdParty",
                ThirdPartyName,
            ),
            (
                "urn:decentraland:amoy:collections-thirdparty:aThirdParty:winterCollection",
                ThirdPartyCollection,
            ),
            (
                "urn:decentraland:amoy:collections-thirdparty:aThirdParty:summerCollection:hat",
                ThirdParty,
            ),
            (
                "urn:decentraland:amoy:collections-thirdparty:aThirdParty:summerCollection:hat:sepolia:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:123456789",
                ThirdPartyItem,
            ),
            (
                "urn:decentraland:ethereum:erc721:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:111111111111111111",
                Erc721,
            ),
            (
                "urn:decentraland:matic:erc721:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:111111111111111111",
                Erc721,
            ),
            ("dcl://base-avatars/eyes_03", OffChain),
            ("dcl://base-avatars/f_sweater", OffChain),
            (
                "dcl://halloween_2019/bride_of_frankie_earring",
                CollectionV1Asset,
            ),
        ];
        for (urn, expected) in cases {
            assert_eq!(parse(urn), Some(expected), "{urn}");
        }
    }

    #[test]
    fn percent_encoded_groups_resolve_decoded() {
        let urn = format!("urn:decentraland:%6datic:collections-v2:{C}:1:42");
        assert_eq!(parse(&urn), Some(CollectionV2Item));
        assert_eq!(
            segments(&urn).map(|s| s.into_iter().map(|(_, d)| d).collect::<Vec<_>>()),
            Some(
                [
                    "urn",
                    "decentraland",
                    "matic",
                    "collections-v2",
                    C,
                    "1",
                    "42"
                ]
                .map(String::from)
                .to_vec()
            )
        );
        let by_name = "urn:decentraland:ethereum:collections-v1:halloween%5F2019:hat";
        assert_eq!(parse(by_name), Some(CollectionV1Asset));
        assert_eq!(
            segments(by_name)
                .as_deref()
                .map(|s| (s[4].0.as_str(), s[4].1.as_str())),
            Some(("halloween%5F2019", "halloween_2019"))
        );
        assert_eq!(
            segments(&format!("urn:decentraland:matic:collections-v2:{C}:%zz:2")),
            None
        );
    }

    #[test]
    fn a_malformed_escape_throws_only_inside_a_matched_route() {
        for urn in [
            "urn:decentraland:off-chain:base-emotes:%ZZ".to_string(),
            "urn:decentraland:off-chain:%zz:x".into(),
            format!("urn:decentraland:%ZZ:collections-v2:{C}"),
            "urn:decentraland:bogusnet:LAND:%ZZ".into(),
            "urn:decentraland:matic:collections-v1:%ZZ".into(),
            "urn:decentraland:matic:collections-thirdparty:%E0%A4".into(),
            "dcl://base-avatars/%ZZ".into(),
            "not-a-real-urn".into(),
        ] {
            assert_eq!(parse_urn(&urn), Err(Throws), "{urn}");
            assert_eq!(parse(&urn), None, "{urn}");
        }
        for urn in [
            "urn:decentraland:off-chain:base-emotes:%ZZ:extra".to_string(),
            "urn:decentraland:matic:collections-v2:%ZZ".into(),
            format!("urn:decentraland:matic:collections-v2:{C}:%zz:2"),
            "urn:foo:%ZZ".into(),
        ] {
            assert_eq!(parse_urn(&urn), Ok(None), "{urn}");
        }
    }

    #[test]
    fn the_off_chain_registry_is_read_decoded() {
        let registry = |urn: &str| {
            parse_urn(urn)
                .unwrap()
                .unwrap()
                .off_chain_registry()
                .map(str::to_string)
        };
        assert_eq!(
            registry("urn:decentraland:off-chain:base%2Demotes:wave").as_deref(),
            Some("base-emotes")
        );
        assert_eq!(
            registry("dcl://base-avatars/eyes_00").as_deref(),
            Some("base-avatars")
        );
        assert_eq!(
            registry(&format!("urn:decentraland:matic:collections-v2:{C}")),
            None
        );
    }

    #[test]
    fn the_resolver_vectors_without_a_type_do_not_resolve() {
        for urn in [
            "urn:test",
            "urn:foo:bar",
            "",
            "hat",
            "https://decentraland.org/x",
            "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:test_name",
            "urn:decentraland:ethereum:collections-v2:0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d:test_name:abc",
            "dcl://base-avatars/",
            "dcl://base-avatars",
            "dcl://base-avatars/a/b/c",
        ] {
            assert_eq!(parse(urn), None, "{urn:?}");
        }
    }

    #[test]
    fn wearable_and_emote_urns_resolve_as_the_resolver_reads_them() {
        let items = [
            (
                format!("urn:decentraland:matic:collections-v2:{C}:1:42"),
                CollectionV2Item,
            ),
            (
                format!("URN:decentraland:matic:collections-v2:{C}:1:42"),
                CollectionV2Item,
            ),
            (
                format!("urn:decentraland:MATIC:collections-v2:{C}:0x1:7"),
                CollectionV2Item,
            ),
            (
                format!("urn:decentraland:matic:collections-v2:{C}:1:42?x=1"),
                CollectionV2Item,
            ),
            (
                format!("urn:decentraland:matic:collections-v1:{C}:hat:3"),
                CollectionV1Item,
            ),
            (
                format!("urn:decentraland:matic:collections-v2:{C}:1"),
                CollectionV2Asset,
            ),
            (
                format!("urn:decentraland:matic:collections-v2:{C}:1?x:2"),
                CollectionV2Asset,
            ),
            (
                format!("urn:decentraland:matic:collections-v1:{C}:hat"),
                CollectionV1Asset,
            ),
            (
                format!("urn:decentraland:matic:collections-v2:{C}"),
                CollectionV2,
            ),
            (
                format!("urn:decentraland:matic:collections-thirdparty:tp:col:hat:matic:{C}:5"),
                ThirdPartyItem,
            ),
            (
                "urn:decentraland:off-chain:base-emotes:wave".into(),
                OffChain,
            ),
            (
                "urn:decentraland:off-chain:base-scene-emotes:handsair".into(),
                OffChain,
            ),
            ("dcl://base-emotes/wave".into(), OffChain),
        ];
        for (urn, expected) in items {
            assert_eq!(parse(&urn), Some(expected), "{urn}");
        }
        for urn in [
            "urn:decentraland:matic:collections-v1:halloween_2019:hat:3".to_string(),
            "urn:Decentraland:matic:collections-v2:0x1:1:2".into(),
            "urn:decentraland:matic:Collections-V2:0x1:1:2".into(),
            "urn:decentraland:mainnet:collections-v2:0x1:1:2".into(),
            "urn:decentraland:polygon:collections-v2:0x1:1:2".into(),
            format!("urn:decentraland:matic:collections-v2:0X{}:1:2", &C[2..]),
            format!("urn:decentraland:matic:collections-v2:{C}:1:2:3"),
            format!("urn:decentraland:matic:collections-v2:{C}:1:"),
            format!("urn:decentraland:matic:collections-v2:{C}:%zz:2"),
            "urn:decentraland:ethereum:collections-thirdparty:tp:col:hat".to_string(),
            format!("urn:decentraland:matic:collections-thirdparty:tp:col:hat:matic:{C}"),
            "dcl://halloween_2019/hat/12".into(),
        ] {
            assert_eq!(parse(&urn), None, "{urn}");
        }
        assert!(
            parse(&format!("urn:decentraland:matic:collections-v2:{C}:1"))
                .unwrap()
                .is_collection_asset()
        );
        assert!(
            !parse(&format!("urn:decentraland:matic:collections-v2:{C}"))
                .unwrap()
                .is_collection_asset()
        );
    }
}

//! Decentraland asset URNs as `@dcl/urn-resolver`'s `parseUrn` resolves them:
//! the input is read as a WHATWG URL (`url`), a `urn:` path is matched
//! against the resolver's routes (case-sensitive literals, `[^:]+` groups,
//! each group percent-decoded) and a legacy `dcl://` address is rewritten to
//! its URN first. A URN that matches no route, names an unknown network or a
//! network without the contract the route needs, or holds a malformed
//! percent escape, does not resolve.

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

/// The resolved type of `input`, `None` when `parseUrn` resolves nothing.
pub fn parse(input: &str) -> Option<UrnType> {
    resolved(input).map(|(kind, _)| kind)
}

/// `input` as the resolver reads it: `urn:` and the URL path (no
/// surrounding whitespace, tab or newline, query or fragment), a legacy
/// `dcl://` address as its URN; `None` when it does not resolve.
pub fn canonical(input: &str) -> Option<String> {
    resolved(input).map(|(_, urn)| urn)
}

fn resolved(input: &str) -> Option<(UrnType, String)> {
    let url = Url::parse(input).ok()?;
    match url.scheme() {
        "urn" => resolve(url.path()).map(|kind| (kind, format!("urn:{}", url.path()))),
        "dcl" => {
            let urn = legacy_dcl_urn(&url)?;
            Some((parse_urn_scheme(&urn)?, urn))
        }
        _ => None,
    }
}

fn parse_urn_scheme(input: &str) -> Option<UrnType> {
    let url = Url::parse(input).ok()?;
    (url.scheme() == "urn")
        .then(|| resolve(url.path()))
        .flatten()
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

fn resolve(path: &str) -> Option<UrnType> {
    let parts: Vec<&str> = path.split(':').collect();
    // Every route segment is a literal or a non-empty `[^:]+` group.
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let decoded = parts
        .iter()
        .map(|p| decode(p))
        .collect::<Option<Vec<_>>>()?;
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

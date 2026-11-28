//! Third-party item URNs, parsed as builder-server's `tpItemURNRegex`
//! accepts them: `urn:decentraland:<network>:collections-thirdparty:
//! <provider>:<collection>:<item>`, the network one of the six it names and
//! each named part one or more characters that are neither `:`, `|` nor
//! JavaScript whitespace. Parts keep their case; whether another item of the
//! provider holds an item part is decided lowercased, as content pointers
//! are, and an item is looked up by its URN exactly.

/// A third-party item URN, by its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThirdPartyItemUrn {
    /// `urn:decentraland:<network>:collections-thirdparty:<provider>`.
    pub third_party_id: String,
    pub collection: String,
    pub item: String,
}

const NETWORKS: [&str; 6] = ["mainnet", "goerli", "sepolia", "matic", "mumbai", "amoy"];

/// JavaScript's `\s`.
fn js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

fn slot(part: &str) -> bool {
    !part.is_empty() && !part.chars().any(|c| c == '|' || js_whitespace(c))
}

impl ThirdPartyItemUrn {
    /// The parts of `urn`, or `None` when it isn't a third-party item URN.
    pub fn parse(urn: &str) -> Option<Self> {
        let parts: Vec<&str> = urn.split(':').collect();
        let [
            "urn",
            "decentraland",
            network,
            "collections-thirdparty",
            provider,
            collection,
            item,
        ] = parts.as_slice()
        else {
            return None;
        };
        if !NETWORKS.contains(network) || ![provider, collection, item].iter().all(|p| slot(p)) {
            return None;
        }
        Some(Self {
            third_party_id: format!("urn:decentraland:{network}:collections-thirdparty:{provider}"),
            collection: collection.to_string(),
            item: item.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_third_party_item_urn_parses_into_its_parts_and_nothing_else_does() {
        let urn = ThirdPartyItemUrn::parse(
            "urn:decentraland:matic:collections-thirdparty:provider:winter:hat-1",
        )
        .unwrap();
        assert_eq!(
            urn,
            ThirdPartyItemUrn {
                third_party_id: "urn:decentraland:matic:collections-thirdparty:provider".into(),
                collection: "winter".into(),
                item: "hat-1".into(),
            }
        );
        assert_eq!(
            ThirdPartyItemUrn::parse("urn:decentraland:amoy:collections-thirdparty:P:W:HAT")
                .unwrap()
                .item,
            "HAT"
        );
        for refused in [
            "",
            "c011ec71-0000-4000-8000-000000000001",
            "urn:decentraland:matic:collections-thirdparty:provider:winter",
            "urn:decentraland:matic:collections-thirdparty:provider:winter:hat:extra",
            "urn:decentraland:polygon:collections-thirdparty:provider:winter:hat",
            "URN:decentraland:matic:collections-thirdparty:provider:winter:hat",
            "urn:decentraland:Matic:collections-thirdparty:provider:winter:hat",
            "urn:decentraland:matic:collections-v2:provider:winter:hat",
            "urn:decentraland:matic:collections-thirdparty::winter:hat",
            "urn:decentraland:matic:collections-thirdparty:provider:winter:",
            "urn:decentraland:matic:collections-thirdparty:provider:win ter:hat",
            "urn:decentraland:matic:collections-thirdparty:provider:winter:h|at",
            "urn:decentraland:matic:collections-thirdparty:provider:winter:hat\u{FEFF}",
            "urn:decentraland:matic:collections-thirdparty:provider:winter:hat\n",
            " urn:decentraland:matic:collections-thirdparty:provider:winter:hat",
        ] {
            assert_eq!(ThirdPartyItemUrn::parse(refused), None, "{refused:?}");
        }
        assert!(
            ThirdPartyItemUrn::parse(
                "urn:decentraland:matic:collections-thirdparty:provider:winter:hat\u{0085}"
            )
            .is_some(),
            "U+0085 is not JavaScript whitespace"
        );
    }
}

use std::collections::HashMap;

use super::*;

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn lookup(map: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
    |key| map.get(key).cloned()
}

fn expanded(posture: Posture, pairs: &[(&str, &str)]) -> HashMap<&'static str, String> {
    let map = env(pairs);
    expand(posture, &lookup(&map)).into_iter().collect()
}

/// The config a process would parse: the explicit pairs, then the
/// posture's expansion for the keys they leave unset.
fn parsed(pairs: &[(&str, &str)]) -> Config {
    let map = env(pairs);
    let mut all = map.clone();
    if let Some((_, vars)) = resolve(&lookup(&map)).unwrap() {
        for (k, v) in vars {
            all.insert(k.to_string(), v);
        }
    }
    Config::parse_env_for_tests(all).unwrap()
}

#[test]
fn the_mirror_sources_follow_the_catalogue_chain_pair() {
    let main = expanded(Posture::Mirror, &[]);
    assert_eq!(
        main["ETH_COLLECTIONS_SUBGRAPH_URL"],
        "https://subgraph.decentraland.org/collections-ethereum-mainnet"
    );
    assert_eq!(
        main["MATIC_COLLECTIONS_SUBGRAPH_URL"],
        "https://subgraph.decentraland.org/collections-matic-mainnet"
    );
    let cfg = expanded(
        Posture::Mirror,
        &[
            ("MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM", "11155111"),
            ("MARKETPLACE_INDEX_CHAIN_ID_MATIC", "80002"),
        ],
    );
    assert_eq!(
        cfg["ETH_COLLECTIONS_SUBGRAPH_URL"],
        "https://subgraph.decentraland.org/collections-ethereum-sepolia"
    );
    assert_eq!(
        cfg["MATIC_COLLECTIONS_SUBGRAPH_URL"],
        "https://subgraph.decentraland.org/collections-matic-amoy"
    );
}

#[test]
fn every_key_is_one_the_binary_reads_and_appears_once() {
    let known = Config::env_names_for_tests();
    let mut seen = std::collections::HashSet::new();
    for key in keys() {
        assert!(known.contains(key), "{key} is not a serve variable");
        assert!(seen.insert(key), "{key} appears twice");
    }
}

#[test]
fn unknown_or_absent_posture() {
    let map = env(&[]);
    assert!(resolve(&lookup(&map)).unwrap().is_none());
    let map = env(&[(ENV, "archipelago")]);
    let err = resolve(&lookup(&map)).unwrap_err().to_string();
    assert!(err.contains("not one of mirror, island, journey"), "{err}");
}

#[test]
fn an_explicit_variable_or_its_file_wins_even_empty() {
    let got = expanded(
        Posture::Mirror,
        &[
            ("SYNC_ENABLED", "false"),
            ("OPENSEA_FALLBACK_URL", ""),
            ("TRADE_RPC_URLS_FILE", "/run/credentials/x"),
        ],
    );
    assert!(!got.contains_key("SYNC_ENABLED"));
    assert!(!got.contains_key("OPENSEA_FALLBACK_URL"));
    assert!(!got.contains_key("TRADE_RPC_URLS"));
    assert_eq!(got["EXPLORER_MIRRORS_ENABLED"], "true");
}

#[test]
fn a_mirror_reads_the_subgraphs_lamb2_defaults_to() {
    let mirror = expanded(Posture::Mirror, &[]);
    assert_eq!(
        mirror["MARKETPLACE_SUBGRAPH_URL"].as_str(),
        "https://subgraph.decentraland.org/marketplace"
    );
    // LAND rights read the land-authz index alone: no posture names the
    // land-manager subgraph (a value left in an environment is no Config
    // variable, so the process never reads it).
    for posture in [Posture::Mirror, Posture::Island, Posture::Journey] {
        assert!(
            !expanded(posture, &[]).contains_key("LAND_SUBGRAPH_URL"),
            "{posture:?}"
        );
    }
}

/// lamb2's defaults for a mirror's profile snapshots and /about parcel
/// view; island and journey leave both to the node's own routes.
#[test]
fn a_mirror_names_lamb2_snapshot_and_parcel_view_hosts() {
    let mirror = expanded(Posture::Mirror, &[]);
    assert_eq!(
        mirror["PROFILE_CDN_BASE_URL"],
        "https://profile-images.decentraland.org"
    );
    assert_eq!(
        mirror["MAP_PARCEL_VIEW_URL"],
        "https://api.decentraland.org/v1/minimap.png"
    );
    for posture in [Posture::Island, Posture::Journey] {
        let got = expanded(posture, &[]);
        assert!(!got.contains_key("PROFILE_CDN_BASE_URL"), "{posture:?}");
        assert!(!got.contains_key("MAP_PARCEL_VIEW_URL"), "{posture:?}");
    }
    let c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert_eq!(
        c.market_upstreams.profile_cdn_base_url.as_deref(),
        Some("https://profile-images.decentraland.org")
    );
    assert_eq!(
        c.map_urls().parcel_view,
        "https://api.decentraland.org/v1/minimap.png"
    );
    let island = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "island")]);
    assert!(island.market_upstreams.profile_cdn_base_url.is_none());
}

/// A mirror may not blank the readers that filter its profiles; an island
/// has none and serves the items as deployed.
#[test]
fn a_mirror_refuses_blank_profile_ownership_readers() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let check_of = |pairs: &[(&str, &str)]| {
        let c = parsed(pairs);
        let map = env(pairs);
        check(&c, &lookup(&map))
    };
    check_of(&[db, (ENV, "mirror")]).unwrap();
    // On mainnet the names read the nft index the mirror walks: the
    // marketplace subgraph is no profile ownership source there.
    check_of(&[db, (ENV, "mirror"), ("MARKETPLACE_SUBGRAPH_URL", "")]).unwrap();
    // Off mainnet it is the names' only source.
    let sepolia = ("ETH_NETWORK", "sepolia");
    check_of(&[db, (ENV, "mirror"), sepolia]).unwrap();
    let err = check_of(&[
        db,
        (ENV, "mirror"),
        sepolia,
        ("MARKETPLACE_SUBGRAPH_URL", ""),
    ])
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("MARKETPLACE_SUBGRAPH_URL is empty: unset it"),
        "{err}"
    );
    // The collections subgraphs are no profile ownership source, but they
    // are the catalogue the nft walk takes its contracts from: a node that
    // fills its own catalogue may blank neither, one whose catalogue another
    // node fills may.
    for (key, blank) in [
        ("ETH_COLLECTIONS_SUBGRAPH_URL", " "),
        ("MATIC_COLLECTIONS_SUBGRAPH_URL", ""),
    ] {
        let err = check_of(&[db, (ENV, "mirror"), (key, blank)])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&format!("{key} is empty")) && err.contains("never publishes"),
            "{err}"
        );
    }
    check_of(&[
        db,
        (ENV, "mirror"),
        ("ETH_COLLECTIONS_SUBGRAPH_URL", " "),
        ("MATIC_COLLECTIONS_SUBGRAPH_URL", ""),
        ("MARKETPLACE_MIRRORS_ENABLED", "false"),
    ])
    .unwrap();
    let err = check_of(&[
        db,
        (ENV, "mirror"),
        ("MARKETPLACE_NFT_INDEX_ENABLED", "false"),
    ])
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("MARKETPLACE_NFT_INDEX_ENABLED is off"),
        "{err}"
    );
    let err = check_of(&[
        db,
        (ENV, "mirror"),
        ("TRADE_RPC_URLS", "1=https://eth.example"),
    ])
    .unwrap_err()
    .to_string();
    assert!(err.contains("names no 137= endpoint"), "{err}");
    let err = check_of(&[db, (ENV, "mirror"), ("TRADE_RPC_URLS", "")])
        .unwrap_err()
        .to_string();
    assert!(err.contains("names no 137= endpoint"), "{err}");
    check_of(&[
        db,
        (ENV, "mirror"),
        ("TRADE_RPC_URLS", "137=https://polygon.example"),
    ])
    .unwrap();
    // A file is read after the check, and the list it held is checked
    // once the secrets are resolved (`refuse_remote_secrets`, the last step
    // of `Config::resolve_secret_files`).
    let with_file = [
        db,
        (ENV, "mirror"),
        ("TRADE_RPC_URLS_FILE", "/run/credentials/trade"),
    ];
    check(&parsed_with_files(&with_file), &lookup(&env(&with_file))).unwrap();
    for (held, refused) in [
        ("1=https://eth.example", true),
        ("1=https://eth.example,137=", true),
        ("1=https://eth.example,137=https://polygon.example", false),
    ] {
        let mut c = parsed_with_files(&with_file);
        // What `resolve_secret_files` reads from the file, then the defaults.
        c.marketplace.trade_rpc_urls = Some(held.into());
        c.default_rpc_urls();
        c.derive_trade_rpc_urls();
        let result = refuse_remote_secrets(&mut c);
        assert_eq!(result.is_err(), refused, "{held}: {result:?}");
        if let Err(e) = result {
            assert!(e.to_string().contains("names no 137= endpoint"), "{e}");
        }
    }
    // ETH_NETWORK and NODE_POSTURE are read trimmed everywhere: padded
    // values still derive the node's own 1= and 137= list, which passes; a
    // list that never derived is refused, as an empty one is.
    for padded in [("ETH_NETWORK", "mainnet "), (ENV, "mirror ")] {
        let mut c = parsed(&[db, (ENV, "mirror"), padded]);
        c.default_rpc_urls();
        c.derive_trade_rpc_urls();
        assert!(
            c.marketplace
                .trade_rpc_urls
                .as_deref()
                .is_some_and(|l| l.contains("137=")),
            "{padded:?}: {:?}",
            c.marketplace.trade_rpc_urls
        );
        refuse_remote_secrets(&mut c).unwrap();
    }
    let mut c = parsed(&[db, (ENV, "mirror"), ("ETH_NETWORK", "mainnet ")]);
    c.default_rpc_urls();
    c.derive_trade_rpc_urls();
    c.marketplace.trade_rpc_urls = None;
    let err = refuse_remote_secrets(&mut c).unwrap_err().to_string();
    assert!(err.contains("names no 137= endpoint"), "{err}");
    // A testnet mirror serves the items as deployed until its chains are walked.
    check_of(&[
        db,
        (ENV, "mirror"),
        ("ETH_NETWORK", "sepolia"),
        ("MARKETPLACE_NFT_INDEX_ENABLED", "false"),
    ])
    .unwrap();
    check_of(&[db, (ENV, "island")]).unwrap();
}

#[test]
fn a_mirror_reads_third_party_holdings_from_the_nft_worker() {
    assert_eq!(
        expanded(Posture::Mirror, &[])["NFT_WORKER_BASE_URL"],
        "https://nfts.decentraland.org"
    );
    for posture in [Posture::Island, Posture::Journey] {
        assert_eq!(expanded(posture, &[])["NFT_WORKER_BASE_URL"], "");
    }
    let c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert_eq!(
        c.market_upstreams.nft_worker_base_url.as_deref(),
        Some("https://nfts.decentraland.org")
    );
}

/// LI-9: a mirror's third-party routes are public, so it may not fall back
/// to the chain path; island and journey keep it without a worker.
#[test]
fn a_mirror_refuses_a_blank_nft_worker() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let check_of = |pairs: &[(&str, &str)]| {
        let c = parsed(pairs);
        let map = env(pairs);
        check(&c, &lookup(&map))
    };
    check_of(&[db, (ENV, "mirror")]).unwrap();
    for blank in ["", " "] {
        let err = check_of(&[db, (ENV, "mirror"), ("NFT_WORKER_BASE_URL", blank)])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("NFT_WORKER_BASE_URL is empty: unset it"),
            "{err}"
        );
    }
    check_of(&[db, (ENV, "island")]).unwrap();
    check_of(&[db, (ENV, "journey")]).unwrap();
}

/// A mirror walks the chain every 30 s and keeps tracking open MANA
/// executions every 5 s; island and journey keep the binary's pace.
#[test]
fn a_mirror_walks_the_chain_every_30_seconds() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let mirror = parsed(&[db, (ENV, "mirror")]);
    assert_eq!(
        (
            mirror.marketplace.marketplace_index_interval_secs,
            mirror
                .marketplace
                .marketplace_executions_track_interval_secs
        ),
        (30, 5)
    );
    for posture in [Posture::Island, Posture::Journey] {
        assert!(!expanded(posture, &[]).contains_key("MARKETPLACE_INDEX_INTERVAL_SECS"));
    }
    let own = parsed(&[
        db,
        (ENV, "mirror"),
        ("MARKETPLACE_INDEX_INTERVAL_SECS", "5"),
    ]);
    assert_eq!(own.marketplace.marketplace_index_interval_secs, 5);
}

#[test]
fn a_mirror_indexes_the_third_party_registry_without_a_subgraph() {
    const SWITCH: &str = "MARKETPLACE_THIRD_PARTY_REGISTRY_INDEX_ENABLED";
    const URL: &str = "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL";
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let c = parsed(&[db, (ENV, "mirror")]);
    assert!(c.marketplace.marketplace_third_party_registry_index_enabled);
    assert_eq!(c.validator.third_party_registry_l2_subgraph_url, None);
    for posture in [Posture::Island, Posture::Journey] {
        assert_eq!(expanded(posture, &[])[SWITCH], "false");
    }
    for blank in ["", " "] {
        let pairs = [db, (ENV, "mirror"), (URL, blank)];
        check(&parsed(&pairs), &lookup(&env(&pairs))).unwrap();
    }
    let pairs = [
        db,
        (ENV, "mirror"),
        (
            "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL_FILE",
            "/run/credentials/tpr",
        ),
    ];
    let mut c = parsed_with_files(&pairs);
    assert_eq!(c.validator.third_party_registry_l2_subgraph_url, None);
    // `resolve_secret_files`'s steps before its last one: a mirror's
    // effective TRADE_RPC_URLS must name chain 137 (`refuse_unwalkable_polygon`).
    c.default_rpc_urls();
    c.derive_trade_rpc_urls();
    refuse_remote_secrets(&mut c).unwrap();
}

#[test]
fn a_sepolia_mirror_keeps_its_polygon_registry_subgraph_during_index_catch_up() {
    const URL: &str = "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL";
    let pairs = [
        ("DATABASE_URL", "postgres:///catalyst"),
        (ENV, "mirror"),
        ("ETH_NETWORK", "sepolia"),
    ];
    let config = parsed(&pairs);
    assert!(
        config
            .marketplace
            .marketplace_third_party_registry_index_enabled
    );
    assert_eq!(
        config
            .validator
            .third_party_registry_l2_subgraph_url
            .as_deref(),
        Some("https://subgraph.decentraland.org/tpr-matic-mainnet")
    );
    let custom = parsed(&[
        pairs[0],
        pairs[1],
        pairs[2],
        (URL, "https://registry.example"),
    ]);
    assert_eq!(
        custom
            .validator
            .third_party_registry_l2_subgraph_url
            .as_deref(),
        Some("https://registry.example")
    );
}

/// Escrow leases are a dcl-one extension: the explorer backpack lists them
/// and a local deploy counts them as item ownership only when the operator
/// opts in, under every posture and without one.
#[test]
fn escrow_leases_count_only_on_an_operator_opt_in() {
    for posture in Posture::ALL {
        for key in [
            "EXPLORER_ESCROW_GRANTS_ENABLED",
            "CONTENT_ESCROW_LEASE_OWNERSHIP_ENABLED",
        ] {
            assert_eq!(expanded(posture, &[])[key], "false", "{key}");
        }
    }
    let db = ("DATABASE_URL", "postgres:///catalyst");
    for posture in [None, Some("mirror"), Some("island"), Some("journey")] {
        let mut pairs = vec![db];
        pairs.extend(posture.map(|p| (ENV, p)));
        let c = parsed(&pairs);
        assert!(
            !c.market_upstreams.explorer_escrow_grants_enabled,
            "{posture:?}"
        );
        assert!(
            !c.validator.content_escrow_lease_ownership_enabled,
            "{posture:?}"
        );
        pairs.extend([
            ("EXPLORER_ESCROW_GRANTS_ENABLED", "true"),
            ("CONTENT_ESCROW_LEASE_OWNERSHIP_ENABLED", "true"),
        ]);
        let c = parsed(&pairs);
        assert!(
            c.market_upstreams.explorer_escrow_grants_enabled,
            "{posture:?}"
        );
        assert!(
            c.validator.content_escrow_lease_ownership_enabled,
            "{posture:?}"
        );
    }
}

#[test]
fn the_catalog_defaults_to_upstream_filters() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let opt_in = ("LAMBDAS_CATALOG_EXTENSIONS_ENABLED", "true");
    assert!(
        !parsed(&[db])
            .market_upstreams
            .lambdas_catalog_extensions_enabled
    );
    for posture in ["mirror", "island", "journey"] {
        assert!(
            !parsed(&[db, (ENV, posture)])
                .market_upstreams
                .lambdas_catalog_extensions_enabled
        );
        assert!(
            parsed(&[db, (ENV, posture), opt_in])
                .market_upstreams
                .lambdas_catalog_extensions_enabled
        );
    }
}

/// CS-25: a mirror syncs from the DAO catalyst list, `SYNC_SOURCE` its fallback.
#[test]
fn a_mirror_syncs_from_the_dao_list() {
    assert_eq!(expanded(Posture::Mirror, &[])["SYNC_FROM_DAO"], "true");
    for posture in [Posture::Island, Posture::Journey] {
        assert_eq!(expanded(posture, &[])["SYNC_FROM_DAO"], "false");
    }
    let c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert!(c.content.sync_from_dao);
    assert!(!c.content.sync_source.is_empty());
}

#[test]
fn read_through_copies_a_miss_on_a_mirror_only() {
    assert_eq!(
        expanded(Posture::Mirror, &[])["READ_THROUGH_ENABLED"],
        "true"
    );
    for posture in [Posture::Island, Posture::Journey] {
        assert_eq!(expanded(posture, &[])["READ_THROUGH_ENABLED"], "false");
        let pairs = [
            ("DATABASE_URL", "postgres:///catalyst"),
            (ENV, posture.name()),
            ("READ_THROUGH_ENABLED", "true"),
        ];
        let err = check(&parsed(&pairs), &lookup(&env(&pairs)))
            .unwrap_err()
            .to_string();
        assert!(err.contains("READ_THROUGH_ENABLED is on"), "{err}");
    }
}

/// A mirror hides what Decentraland's moderation list names, as the upstream
/// catalyst does by default; island and journey ask no one, and a list
/// naming an outside host is refused there.
#[test]
fn a_mirror_reads_decentralands_moderation_denylist() {
    const ABR: &str = "https://asset-bundle-registry.decentraland.org/denylist";
    assert_eq!(expanded(Posture::Mirror, &[])["DENYLIST_URLS"], ABR);
    let mirror = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert_eq!(mirror.content.denylist_urls, ABR);
    let unset = parsed(&[("DATABASE_URL", "postgres:///catalyst")]);
    assert_eq!(unset.content.denylist_urls, "", "no posture, no list");
    for posture in [Posture::Island, Posture::Journey] {
        assert_eq!(expanded(posture, &[])["DENYLIST_URLS"], "");
        let pairs = [
            ("DATABASE_URL", "postgres:///catalyst"),
            (ENV, posture.name()),
        ];
        assert_eq!(parsed(&pairs).content.denylist_urls, "");
        let pairs = [
            ("DATABASE_URL", "postgres:///catalyst"),
            (ENV, posture.name()),
            ("DENYLIST_URLS", ABR),
        ];
        let err = check(&parsed(&pairs), &lookup(&env(&pairs)))
            .unwrap_err()
            .to_string();
        assert!(err.contains("DENYLIST_URLS names"), "{err}");
    }
    // A value that is not an http(s) URL fails the startup check, posture or
    // not, before serve spawns anything.
    for posture in [None, Some("mirror")] {
        let mut pairs = vec![
            ("DATABASE_URL", "postgres:///catalyst"),
            ("DENYLIST_URLS", "ftp://registry.example/denylist"),
        ];
        pairs.extend(posture.map(|p| (ENV, p)));
        let err = check(&parsed(&pairs), &lookup(&env(&pairs)))
            .unwrap_err()
            .to_string();
        assert!(err.contains("only http(s)"), "{posture:?}: {err}");
    }
}

#[test]
fn island_sets_every_upstream_empty_and_every_follower_switch_off() {
    let got = expanded(Posture::Island, &[]);
    for (key, value) in &got {
        let switch = value == "true" || value == "false";
        if switch {
            // Local switches that dial no host.
            let allowed = [
                "SERVE_UNSYNCED",
                "PHASED_SYNC",
                "LAMBDAS_CATALOG_EXTENSIONS_ENABLED",
            ];
            assert!(
                value == "false" || allowed.contains(key),
                "island turns {key} on"
            );
        } else if key.ends_with("_URL") || key.starts_with("RPC_") || key.ends_with("_AB_CDN") {
            assert_eq!(value, "", "island keeps an upstream in {key}");
        }
    }
    for key in [
        "SYNC_SOURCE",
        "TRADES_SYNC_UPSTREAM_URL",
        "RPC_UPSTREAM_MAINNET",
        "ABGEN_UPSTREAM_AB_CDN",
        "BVIMPOSTERS_UPSTREAM_URL",
        "EXPLORER_REGISTRY_SOURCE_URL",
        "OPENSEA_FALLBACK_URL",
        "COINGECKO_URL",
        "ETH_RPC_URL",
        "TRADE_RPC_URLS",
        "PRICE_FALLBACK_SOURCES",
        "OPENSEA_IPFS_GATEWAYS",
        "OPENSEA_CRYPTOKITTIES_API",
        "LEASE_AUTHORIZATIONS_URL",
        "BUILDER_FOUNDATION_URL",
        "WORLDS_CONTENT_MIRROR_SOURCE",
        "COMMUNITIES_MIRROR_THUMBNAIL_URL",
        "MAP_SATELLITE_BASE_URL",
    ] {
        assert_eq!(got[key], "", "{key}");
    }
    assert!(
        !got.contains_key("MAP_PARCEL_VIEW_URL"),
        "island leaves the parcel view to the node's own"
    );
}

/// Only a mirror fetches the disposable email list or names SendGrid; an island's empty
/// SendGrid URL leaves email unconfigured.
#[test]
fn only_the_mirror_fetches_the_disposable_list_and_names_sendgrid() {
    let bare = Config::parse_env_for_tests([("DATABASE_URL", "postgres:///catalyst")]).unwrap();
    assert_eq!(bare.social.disposable_email_domains_url, "");
    for posture in ["island", "journey"] {
        let c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, posture)]);
        assert_eq!(c.social.disposable_email_domains_url, "", "{posture}");
        assert_eq!(c.social.sendgrid_api_url, "", "{posture}");
    }
    let mirror = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert!(
        mirror
            .social
            .disposable_email_domains_url
            .starts_with("https://raw.githubusercontent.com/disposable-email-domains/")
    );
    assert_eq!(mirror.social.sendgrid_api_url, "https://api.sendgrid.com");
}

/// Every URL the mirror row names is one the binary leaves empty by
/// default: a bare `serve` and an island name no Decentraland host.
#[test]
fn only_the_mirror_row_names_decentraland() {
    let bare = Config::parse_env_for_tests([("DATABASE_URL", "postgres:///catalyst")]).unwrap();
    let island = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "island")]);
    for c in [&bare, &island] {
        assert!(c.explore.places_upstream_url.is_empty());
        assert!(c.explore.world_manifest_upstream_url.is_empty());
        assert!(c.social.communities_upstream_url.is_empty());
        assert!(c.social.lease_authorizations_url.is_empty());
        assert!(c.create.builder_foundation_url.is_empty());
        assert!(c.price.price_fallback_sources.is_empty());
        assert!(!c.social.media_convert_remote_sources);
    }
    let mirror = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert!(mirror.social.media_convert_remote_sources);
    assert!(
        mirror
            .price
            .price_fallback_sources
            .contains("coinbase=https://")
    );
    let journey = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "journey")]);
    // Only the scratch journey stack runs on the LiveKit dev pair.
    assert!(journey.social.livekit_allow_dev_creds);
    assert!(!island.social.livekit_allow_dev_creds && !mirror.social.livekit_allow_dev_creds);
}

/// The tile grid's old copy source is gone (the map grid is built from the
/// node's own indexes): no posture sets its two variables, the binary reads
/// neither, and an environment that still sets them passes every posture's
/// check, which looks only at keys the binary reads.
#[test]
fn the_retired_map_source_variables_are_ignored_at_startup() {
    let leftover = [
        ("EXPLORER_MAP_SOURCE_URL", "https://api.decentraland.org"),
        ("EXPLORER_MAP_MIN_REFRESH_SECS", "3600"),
    ];
    let read = Config::env_names_for_tests();
    for (key, _) in leftover {
        assert!(!read.contains(key), "{key} is read by the binary");
        assert!(keys().all(|k| k != key), "{key} is in the posture table");
    }
    for posture in ["mirror", "island", "journey"] {
        let pairs = [("DATABASE_URL", "postgres:///catalyst"), (ENV, posture)];
        let config = parsed(&pairs);
        let mut environment = env(&pairs);
        environment.extend(env(&leftover));
        check(&config, &lookup(&environment)).unwrap_or_else(|e| panic!("{posture}: {e}"));
        assert!(
            expanded(Posture::parse(posture).unwrap(), &leftover)
                .keys()
                .all(|k| !k.starts_with("EXPLORER_MAP_")),
            "{posture}"
        );
    }
}

#[test]
fn island_config_names_no_upstream_and_advertises_its_own_map() {
    let mut c = parsed(&[
        ("DATABASE_URL", "postgres:///catalyst"),
        (ENV, "island"),
        ("PUBLIC_URL", "https://node.example.org/"),
    ]);
    assert!(!c.node.sync_enabled && c.node.serve_unsynced);
    assert!(!c.explore.explorer_mirrors_enabled && !c.marketplace.marketplace_mirrors_enabled);
    assert!(Config::non_empty(&c.market_upstreams.trades_sync_upstream_url).is_none());
    assert!(Config::non_empty(&c.ab_cdn.abgen_upstream_ab_cdn).is_none());
    assert!(c.explore.opensea_fallback_url.is_empty());
    assert!(c.price.coingecko_url.is_empty());
    c.default_rpc_urls();
    assert!(c.content.eth_rpc_url().unwrap().is_none());
    assert_eq!(c.economy.rpc_endpoint_polygon.as_deref(), Some(""));
    let map = c.map_urls();
    assert_eq!(map.satellite_base, None, "no satellite view on an island");
    assert_eq!(
        map.parcel_view,
        "https://node.example.org/api/v1/minimap.png"
    );
    assert_eq!(
        map.estate_view,
        "https://node.example.org/api/v1/estatemap.png"
    );
    assert_eq!(map.land_image_base, "https://node.example.org/api");
    assert_eq!(c.social.profile_images_cache_max_bytes, 5 << 30);
    assert!(check(&c, &|k| (k == ENV).then(|| "island".to_string())).is_ok());
}

#[test]
fn mirror_follows_decentraland_and_derives_the_trade_rpcs() {
    let mut c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    assert_eq!(
        c.marketplace.trade_rpc_urls, None,
        "derived after the secret files"
    );
    c.default_rpc_urls();
    c.derive_trade_rpc_urls();
    assert!(c.node.sync_enabled && !c.node.serve_unsynced);
    assert!(c.content.sync_source.contains("peer.decentraland.org"));
    assert!(c.explore.explorer_mirrors_enabled && c.social.communities_mirror_upstream);
    assert!(c.marketplace.marketplace_nft_index_enabled && c.land_authz.land_authz_index_enabled);
    assert!(c.shop.credits_usage_index_enabled);
    assert_eq!(c.shop.shop_rentals_index_enabled, Some(true));
    assert_eq!(
        c.shop.credits_manager_contract.as_deref(),
        Some(CREDITS_MANAGER_MAINNET)
    );
    assert!(c.price.price_poll_enabled);
    assert_eq!(
        c.marketplace.trade_rpc_urls.as_deref(),
        Some(
            "1=https://rpc.decentraland.org/mainnet?project=dcl-one,\
             137=https://rpc.decentraland.org/polygon?project=dcl-one"
        )
    );
    assert!(c.content.content_history_prune_enabled);
    assert!(!c.writes.content_writes_enabled && !c.worlds.worlds_writes_enabled);
    assert_eq!(
        c.map_urls().satellite_base.as_deref(),
        Some("https://genesis.city/map/latest")
    );
    assert_eq!(
        c.map_urls().land_image_base,
        "https://api.decentraland.org",
        "the base of the marketplace subgraph's LAND images"
    );

    // The site's own endpoints feed the trade list; a satellite mirror
    // still advertises genesis.city (its pyramid is another scheme).
    let mut c = parsed(&[
        ("DATABASE_URL", "postgres:///catalyst"),
        (ENV, "mirror"),
        ("ETH_RPC_URL", "https://eth.example.org/key"),
        ("RPC_ENDPOINT_POLYGON", "https://polygon.example.org/key"),
        (
            "SATELLITE_MIRROR_UPSTREAM_URL",
            "https://renders.example.org/sat",
        ),
        ("CONTENT_WRITES_ENABLED", "true"),
    ]);
    c.derive_trade_rpc_urls();
    assert_eq!(
        c.marketplace.trade_rpc_urls.as_deref(),
        Some("1=https://eth.example.org/key,137=https://polygon.example.org/key")
    );
    assert_eq!(
        c.map_urls().satellite_base.as_deref(),
        Some("https://genesis.city/map/latest")
    );
    assert!(
        c.content.content_history_prune_enabled,
        "content writes leave the prune default alone"
    );

    // The preset leaves the chain endpoints to the binary's default; off
    // mainnet it knows no contract.
    let got = expanded(Posture::Mirror, &[("ETH_NETWORK", "sepolia")]);
    for key in [
        "ETH_RPC_URL",
        "RPC_ENDPOINT_POLYGON",
        "TRADE_RPC_URLS",
        "CREDITS_MANAGER_CONTRACT",
    ] {
        assert!(!got.contains_key(key), "{key}");
    }
}

/// The headcount archive is a database write: no posture turns it on
/// past DATA_WRITES_ENABLED (a mirror or island restore found its rows).
#[test]
fn the_presence_collector_follows_the_data_write_switch() {
    for posture in Posture::ALL {
        assert!(
            !expanded(posture, &[]).contains_key("PRESENCE_COLLECTOR_ENABLED"),
            "{}",
            posture.name()
        );
    }
    let c = parsed(&[
        ("DATABASE_URL", "postgres:///catalyst"),
        ("NODE_POSTURE", "island"),
    ]);
    assert_eq!(c.presence.presence_collector_enabled, None);
    assert!(!c.writes.data_writes_enabled);
}

#[test]
fn journey_opens_all_six_write_domains() {
    let c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "journey")]);
    assert!(c.writes.content_writes_enabled && c.create.create_writes_enabled);
    assert!(c.writes.data_writes_enabled && c.writes.social_writes_enabled);
    assert!(c.worlds.worlds_writes_enabled && c.ops.ops_writes_enabled);
    assert!(!c.node.sync_enabled && c.node.serve_unsynced);
    assert!(Config::non_empty(&c.market_upstreams.trades_sync_upstream_url).is_none());
    assert!(check(&c, &|k| (k == ENV).then(|| "journey".to_string())).is_ok());
}

#[test]
fn startup_refuses_the_contradictions() {
    let refused = |pairs: &[(&str, &str)]| {
        let c = parsed(pairs);
        let map = env(pairs);
        check(&c, &lookup(&map)).unwrap_err().to_string()
    };
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let err = refused(&[db, ("SYNC_ENABLED", "true"), ("SYNC_SOURCE", " ")]);
    assert!(err.contains("no peer to sync from"), "{err}");
    let err = refused(&[db, (ENV, "mirror"), ("SYNC_SOURCE", "")]);
    assert!(err.contains("no peer to sync from"), "{err}");
    let err = refused(&[db, (ENV, "island"), ("SYNC_ENABLED", "true")]);
    assert!(err.contains("syncs from no peer"), "{err}");
    let err = refused(&[
        db,
        (ENV, "journey"),
        ("WORLDS_FORWARD_UPSTREAM_URL", "https://worlds.example.org"),
    ]);
    assert!(err.contains("WORLDS_FORWARD_UPSTREAM_URL"), "{err}");

    // A write switch alone opens its plane; a forward URL is a mirror's choice.
    let ok = |pairs: &[(&str, &str)]| {
        let c = parsed(pairs);
        let map = env(pairs);
        check(&c, &lookup(&map))
    };
    ok(&[db, (ENV, "island"), ("SOCIAL_WRITES_ENABLED", "true")]).unwrap();
    ok(&[
        db,
        (ENV, "mirror"),
        ("CONTENT_WRITES_ENABLED", "true"),
        ("OPS_WRITES_ENABLED", "true"),
    ])
    .unwrap();
    ok(&[
        db,
        (ENV, "mirror"),
        ("CONTENT_FORWARD_UPSTREAM_URL", "https://peer.example.org"),
    ])
    .unwrap();
}

#[test]
fn a_posture_flag_without_the_variable_is_refused() {
    let c = Config::parse_for_tests(&[
        "x",
        "--database-url",
        "postgres:///catalyst",
        "--node-posture",
        "island",
    ])
    .unwrap();
    let err = check(&c, &|_| None).unwrap_err().to_string();
    assert!(
        err.contains("read from the NODE_POSTURE environment variable alone"),
        "{err}"
    );
}

#[test]
fn env_prints_the_named_keys_as_the_posture_sets_them() {
    let keys = |k: &[&str]| k.iter().map(|k| k.to_string()).collect::<Vec<_>>();
    assert_eq!(
        env_file(Posture::Mirror, &keys(&["COINGECKO_URL"])).unwrap(),
        "COINGECKO_URL=https://api.coingecko.com/api/v3\n"
    );
    assert_eq!(
        env_file(Posture::Island, &keys(&["COINGECKO_URL", "SYNC_ENABLED"])).unwrap(),
        "SYNC_ENABLED=false\nCOINGECKO_URL=\n"
    );
    assert_eq!(
        env_file(Posture::Island, &keys(&["MAP_PARCEL_VIEW_URL"])).unwrap(),
        "",
        "a key left to the binary prints nothing"
    );
    let err = env_file(Posture::Mirror, &keys(&["NOT_A_KEY"])).unwrap_err();
    assert!(err.to_string().contains("NOT_A_KEY is not a key"), "{err}");
    let all = env_file(Posture::Journey, &[]).unwrap();
    assert!(all.contains("CONTENT_WRITES_ENABLED=true\n"), "{all}");
    assert!(
        all.contains("IGNORE_BLOCKCHAIN_ACCESS_CHECKS=true\n"),
        "the journey stack reads no chain: {all}"
    );
    for posture in [Posture::Mirror, Posture::Island] {
        assert_eq!(
            expanded(posture, &[])["IGNORE_BLOCKCHAIN_ACCESS_CHECKS"],
            "false"
        );
    }
}

#[test]
fn show_prints_every_key_of_the_table() {
    for posture in Posture::ALL {
        let text = show(posture);
        for key in keys() {
            assert!(text.contains(key), "{} lacks {key}", posture.name());
        }
    }
    let island = show(Posture::Island);
    assert!(island.contains("\nSYNC_ENABLED=false\n"));
    assert!(
        island.contains(
            "# MAP_PARCEL_VIEW_URL unset: this node's own <PUBLIC_URL>/api/v1/minimap.png"
        )
    );
    assert!(show(Posture::Mirror).contains(
        "# TRADE_RPC_URLS unset: this node's own 1=<ETH_RPC_URL>,137=<RPC_ENDPOINT_POLYGON> list"
    ));
}

/// The config a process parses when `pairs` include `_FILE` variables:
/// they steer the expansion, then clap reads the rest.
fn parsed_with_files(pairs: &[(&str, &str)]) -> Config {
    let map = env(pairs);
    let mut all: HashMap<String, String> = map
        .iter()
        .filter(|(k, _)| !k.ends_with("_FILE"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some((_, vars)) = resolve(&lookup(&map)).unwrap() {
        for (k, v) in vars {
            all.insert(k.to_string(), v);
        }
    }
    Config::parse_env_for_tests(all).unwrap()
}

/// A polygon endpoint held in RPC_ENDPOINT_POLYGON_FILE is out of the
/// expansion's sight; the list derives once the file is read.
#[test]
fn a_polygon_endpoint_from_its_file_still_makes_the_trade_list() {
    let mut c = parsed_with_files(&[
        ("DATABASE_URL", "postgres:///catalyst"),
        (ENV, "mirror"),
        ("RPC_ENDPOINT_POLYGON_FILE", "/run/credentials/polygon"),
    ]);
    assert_eq!(c.economy.rpc_endpoint_polygon, None);
    // What `resolve_secret_files` reads from the file, then the defaults.
    c.economy.rpc_endpoint_polygon = Some("https://polygon.example.org/key".into());
    c.default_rpc_urls();
    c.derive_trade_rpc_urls();
    assert_eq!(
        c.marketplace.trade_rpc_urls.as_deref(),
        Some(
            "1=https://rpc.decentraland.org/mainnet?project=dcl-one,\
             137=https://polygon.example.org/key"
        )
    );

    // An explicit list, even empty, wins; island derives nothing.
    let mut c = parsed(&[
        ("DATABASE_URL", "postgres:///catalyst"),
        (ENV, "mirror"),
        ("TRADE_RPC_URLS", ""),
    ]);
    c.derive_trade_rpc_urls();
    assert_eq!(c.marketplace.trade_rpc_urls.as_deref(), Some(""));
    let mut c = parsed(&[
        ("DATABASE_URL", "postgres:///catalyst"),
        (ENV, "island"),
        ("ETH_RPC_URL", "https://eth.example.org/key"),
    ]);
    c.derive_trade_rpc_urls();
    assert_eq!(c.marketplace.trade_rpc_urls.as_deref(), Some(""));
}

#[test]
fn an_empty_file_variable_counts_as_unset() {
    let got = expanded(
        Posture::Island,
        &[("RPC_ENDPOINT_POLYGON_FILE", ""), ("ETH_RPC_URL_FILE", " ")],
    );
    assert_eq!(got["RPC_ENDPOINT_POLYGON"], "");
    assert_eq!(got["ETH_RPC_URL"], "");
    let got = expanded(
        Posture::Island,
        &[("RPC_ENDPOINT_POLYGON_FILE", "/run/credentials/polygon")],
    );
    assert!(!got.contains_key("RPC_ENDPOINT_POLYGON"));
}

fn refusal(pairs: &[(&str, &str)]) -> Option<String> {
    let c = parsed(pairs);
    let map = env(pairs);
    let mut all = map.clone();
    if let Some((_, vars)) = resolve(&lookup(&map)).unwrap() {
        for (k, v) in vars {
            all.entry(k.to_string()).or_insert(v);
        }
    }
    check(&c, &lookup(&all)).err().map(|e| e.to_string())
}

#[test]
fn island_and_journey_refuse_every_follower_switch() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let mut followers: Vec<&str> = ROWS
        .iter()
        .filter(|r| follows(r) && r.key != "SYNC_ENABLED")
        .map(|r| r.key)
        .collect();
    followers.push("MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED");
    assert!(followers.contains(&"MARKETPLACE_COLLECTIONS_INDEX_ENABLED"));
    assert!(followers.contains(&"PRICE_POLL_ENABLED") && followers.len() >= 14);
    for posture in ["island", "journey"] {
        for key in &followers {
            let err = refusal(&[db, (ENV, posture), (key, "true")])
                .unwrap_or_else(|| panic!("{posture} admits {key}=true"));
            assert!(
                err.contains(key) && err.contains("reads from no Decentraland"),
                "{err}"
            );
        }
        assert_eq!(refusal(&[db, (ENV, posture)]), None);
    }
    assert_eq!(
        refusal(&[db, (ENV, "mirror"), ("PRICE_POLL_ENABLED", "true")]),
        None
    );
}

/// The processes outside [`Config`] hold their own URLs to the same
/// rule: Pulse's room authority and broker.
#[test]
fn check_values_holds_outside_processes_to_loopback_off_mirror() {
    for posture in [Posture::Island, Posture::Journey] {
        let err = check_values(
            posture,
            [(
                "PULSE_ROOM_AUTHORITY_URL",
                "https://authority.example.org/v1".to_string(),
            )],
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("PULSE_ROOM_AUTHORITY_URL names authority.example.org"),
            "{err}"
        );
        for (key, value) in [
            ("PULSE_NATS_URL", "nats.example.org:4222"),
            ("NATS_URL", "nats://192.0.2.7:4222"),
            ("PULSE_ROOM_AUTHORITY_URL", "https:authority.example.org/v1"),
        ] {
            let err = check_values(posture, [(key, value.to_string())]).unwrap_err();
            assert!(err.to_string().contains(key), "{err}");
        }
        check_values(
            posture,
            [
                ("PULSE_NATS_URL", "nats://127.0.0.1:4222".to_string()),
                ("NATS_URL", "127.0.0.1:4222".to_string()),
                (
                    "PULSE_ROOM_AUTHORITY_URL",
                    "http://127.0.0.1:5210/internal/pulse/room-authority/v1".to_string(),
                ),
                ("PUBLIC_URL", "https://node.example.org".to_string()),
                (
                    "PULSE_ROOM_AUTHORITY_URL",
                    "https://node.example.org/internal/pulse/room-authority/v1".to_string(),
                ),
                ("PULSE_NATS_URL", "nats://broker.invalid:4222".to_string()),
            ],
        )
        .unwrap();
    }
    check_values(
        Posture::Mirror,
        [("NATS_URL", "nats://192.0.2.7:4222".to_string())],
    )
    .unwrap();
    let env = env(&[(ENV, "journey")]);
    assert_eq!(from_env(&lookup(&env)).unwrap(), Some(Posture::Journey));
    assert_eq!(from_env(&|_| Some(" ".to_string())).unwrap(), None);
    assert!(from_env(&|_| Some("isle".to_string())).is_err());
}

/// A worlds federation peer file names other nodes to poll; off mirror
/// the node follows no one.
#[test]
fn island_and_journey_refuse_a_worlds_federation_peer_file() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let peers = ("WORLDS_FED_PEERS_FILE", "federation-peers.toml");
    for posture in ["island", "journey"] {
        let err = refusal(&[db, (ENV, posture), peers])
            .unwrap_or_else(|| panic!("{posture} admits WORLDS_FED_PEERS_FILE"));
        assert!(
            err.contains("WORLDS_FED_PEERS_FILE") && err.contains("reads from no Decentraland"),
            "{err}"
        );
    }
    assert_eq!(refusal(&[db, (ENV, "mirror"), peers]), None);
    assert_eq!(refusal(&[db, peers]), None);
}

/// Every variable the configuration reads is held to loopback and the
/// node's own address off mirror, unless [`ANY_HOST`] names it: a new
/// URL-valued field is covered the day it is added.
#[test]
fn island_and_journey_refuse_any_variable_naming_an_outside_host() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let public = ("PUBLIC_URL", "https://node.example.org");
    let names = Config::env_names();
    assert!(names.len() > 400, "{}", names.len());
    let island = parsed(&[db, (ENV, "island"), public]);
    for key in names {
        let exempt = ANY_HOST.iter().any(|(k, _)| k == key);
        for value in [
            "https://third.example.org/x",
            "1=https://rpc.example.org/key,137=http://127.0.0.1:1/",
            "fixed-adapter:signed-login:https://adapter.example.org/login",
            "https:third.example.org/x",
            "https:/third.example.org/x",
            r"https:\\third.example.org\x",
            "1=https:third.example.org/key",
        ] {
            let map = env(&[(ENV, "island"), (key.as_str(), value)]);
            let got = refuse_remote_hosts(Posture::Island, &island, &lookup(&map));
            match (exempt, got) {
                (true, got) => assert!(got.is_ok(), "{key} is exempt"),
                (false, Err(e)) => assert!(e.to_string().contains(key.as_str()), "{e}"),
                (false, Ok(())) => panic!("{key}={value} passes on an island"),
            }
        }
    }
    for key in ["LIVEKIT_HOST", "NATS_URL", "LIVEKIT_API_HOST"] {
        let map = env(&[(key, "livekit.example.org:7880")]);
        assert!(refuse_remote_hosts(Posture::Island, &island, &lookup(&map)).is_err());
    }
    // Loopback, the node's own host and a never-resolving name pass.
    for (key, value) in [
        ("NATS_URL", "nats://127.0.0.1:4222"),
        ("NATS_URL", "127.0.0.1:4222"),
        ("LIVEKIT_HOST", "ws://[::1]:7880"),
        ("LIVEKIT_HOST", "livekit.localhost"),
        ("CONTENT_URL", "https://NODE.example.org./content"),
        (
            "MATIC_COLLECTIONS_SUBGRAPH_URL",
            "https://fixtures.invalid/subgraph",
        ),
        (
            "COMMS_FIXED_ADAPTER",
            "signed-login:http://localhost:5210/login",
        ),
        ("STORAGE_X_ACCEL_BASE", "/x-accel/"),
        ("COMMS_ADAPTER", "offline:offline"),
        ("REALM_NAME", "isla-ñandú:main"),
    ] {
        let map = env(&[(key, value)]);
        assert!(
            refuse_remote_hosts(Posture::Island, &island, &lookup(&map)).is_ok(),
            "{key}={value}"
        );
    }
    let err = refusal(&[db, (ENV, "journey"), ("NATS_URL", "nats://192.0.2.7:4222")]).unwrap();
    assert!(err.contains("NATS_URL names 192.0.2.7"), "{err}");
    assert_eq!(
        refusal(&[db, (ENV, "mirror"), ("NATS_URL", "nats://192.0.2.7:4222")]),
        None
    );
}

#[test]
fn a_secret_file_naming_an_outside_host_is_refused_off_mirror() {
    let mut c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "island")]);
    c.economy.rpc_endpoint_polygon = Some("https://polygon.example.org/key".into());
    let err = refuse_remote_secrets(&mut c).unwrap_err().to_string();
    assert!(
        err.contains("RPC_ENDPOINT_POLYGON names polygon.example.org"),
        "{err}"
    );
    c.economy.rpc_endpoint_polygon = Some("http://127.0.0.1:8545".into());
    refuse_remote_secrets(&mut c).unwrap();
    let mut c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "mirror")]);
    c.economy.rpc_endpoint_polygon = Some("https://polygon.example.org/key".into());
    c.default_rpc_urls();
    c.derive_trade_rpc_urls();
    refuse_remote_secrets(&mut c).unwrap();
}

/// No default of the binary names an outside host: a URL the node
/// dials is a mirror row of the table, empty elsewhere. The exceptions
/// are the links of [`ANY_HOST`] (never dialed) and the API bases of
/// [`THIRD_PARTY_SECRETS`] (dialed only with a secret island and
/// journey refuse).
#[test]
fn no_default_names_an_outside_host() {
    let defaults = Config::defaults_for_tests();
    assert!(defaults.len() > 400, "{}", defaults.len());
    let mut outside = Vec::new();
    for (key, default) in &defaults {
        let link = ANY_HOST.iter().any(|(k, why)| k == key && *why == LINK);
        let gated = THIRD_PARTY_SECRETS
            .iter()
            .any(|(_, base, _)| *base == Some(key.as_str()));
        if link || gated {
            continue;
        }
        if !default.contains("://") {
            continue;
        }
        if named_hosts(key, default).iter().any(|h| !is_local(h, &[])) {
            outside.push(format!("{key}={default}"));
        }
    }
    assert!(
        outside.is_empty(),
        "defaults naming an outside host: {outside:#?}"
    );
    let sync = defaults.iter().find(|(k, _)| k == "SYNC_SOURCE").unwrap();
    assert!(sync.1.starts_with("http://127.0.0.1:"), "{sync:?}");
}

#[test]
fn island_refuses_the_secrets_that_call_a_third_party() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    for posture in ["island", "journey"] {
        for (secret, api) in [
            ("SENDGRID_API_KEY", "SendGrid"),
            ("MAGIC_SECRET_KEY", "Magic"),
            ("SLACK_BOT_TOKEN", "Slack"),
            ("TURNSTILE_SECRET_KEY", "Turnstile"),
            ("STRIPE_SECRET_KEY", "Stripe"),
            ("CREDITS_CAPTCHA_SECRET", "hCaptcha"),
        ] {
            let err = refusal(&[db, (ENV, posture), (secret, "s3cr3t")]).unwrap();
            assert!(err.contains(secret) && err.contains(api), "{err}");
            assert!(!err.contains("s3cr3t"), "{err}");
        }
        let map = env(&[
            (ENV, posture),
            ("SENDGRID_API_KEY_FILE", "/run/credentials/sg"),
        ]);
        let p = Posture::parse(posture).unwrap();
        assert!(refuse_third_party_secrets(p, &lookup(&map)).is_err());
        // A captcha verifier named on loopback is the journey's own fake.
        assert_eq!(
            refusal(&[
                db,
                (ENV, posture),
                ("CREDITS_CAPTCHA_SECRET", "s3cr3t"),
                (
                    "CREDITS_CAPTCHA_VERIFY_URL",
                    "http://127.0.0.1:9/siteverify"
                ),
            ]),
            None
        );
    }
    assert_eq!(
        refusal(&[db, (ENV, "mirror"), ("SENDGRID_API_KEY", "s3cr3t")]),
        None
    );
}

#[test]
fn optional_moderation_webhooks_obey_posture_before_and_after_secret_resolution() {
    for posture in ["island", "journey"] {
        let error = refusal(&[
            ("DATABASE_URL", "postgres:///catalyst"),
            (ENV, posture),
            (
                "SLACK_WEBHOOK",
                "https://hooks.slack.com/services/private-token",
            ),
        ])
        .unwrap();
        assert!(error.contains("SLACK_WEBHOOK"));
        assert!(!error.contains("private-token"));
        let mut config = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, posture)]);
        config.explore.events_slack_webhook =
            Some("https://hooks.slack.com/services/private-token".into());
        assert!(
            refuse_remote_secrets(&mut config).is_err(),
            "credential-file values obey the same posture"
        );
        config.explore.events_slack_webhook = Some("http://127.0.0.1:9999/slack-fixture".into());
        refuse_remote_secrets(&mut config).unwrap();
    }
}

/// Every node reads the public RPCs of its network unless the operator names
/// others; island and journey, which dial no outside host, read none.
#[test]
fn the_chain_rpcs_default_by_network_outside_island_and_journey() {
    let rpcs = |pairs: &[(&str, &str)]| {
        let mut all = vec![("DATABASE_URL", "postgres:///catalyst")];
        all.extend_from_slice(pairs);
        let mut c = parsed(&all);
        c.default_rpc_urls();
        (c.content.eth_rpc_url, c.economy.rpc_endpoint_polygon)
    };
    let pair = |l1: &str, l2: &str| {
        let url = |chain: &str| {
            Some(format!(
                "https://rpc.decentraland.org/{chain}?project=dcl-one"
            ))
        };
        (url(l1), url(l2))
    };
    assert_eq!(rpcs(&[]), pair("mainnet", "polygon"));
    assert_eq!(rpcs(&[(ENV, "mirror")]), pair("mainnet", "polygon"));
    // Guard: the tag is never derived from PUBLIC_URL. It would tie the node's
    // address, public or internal, to its RPC activity.
    for public_url in [
        "https://node.example.org",
        "http://192.0.2.7:5000",
        "http://catalyst.test",
    ] {
        assert_eq!(
            rpcs(&[(ENV, "mirror"), ("PUBLIC_URL", public_url)]),
            pair("mainnet", "polygon"),
            "{public_url}"
        );
    }
    assert_eq!(rpcs(&[("ETH_NETWORK", "sepolia")]), pair("sepolia", "amoy"));
    assert_eq!(
        rpcs(&[(ENV, "mirror"), ("ETH_NETWORK", "sepolia")]),
        pair("sepolia", "amoy")
    );
    // Empty is unset.
    assert_eq!(
        rpcs(&[("ETH_RPC_URL", ""), ("RPC_ENDPOINT_POLYGON", " ")]),
        pair("mainnet", "polygon")
    );
    // An operator's endpoint wins.
    assert_eq!(
        rpcs(&[
            ("ETH_RPC_URL", "https://eth.example.org/key"),
            ("ETH_NETWORK", "sepolia"),
        ]),
        (
            Some("https://eth.example.org/key".to_string()),
            pair("sepolia", "amoy").1
        )
    );
    for posture in ["island", "journey"] {
        assert_eq!(
            rpcs(&[(ENV, posture)]),
            (Some(String::new()), Some(String::new())),
            "{posture}"
        );
        assert_eq!(
            rpcs(&[(ENV, posture), ("ETH_RPC_URL", "http://127.0.0.1:8545")]).0,
            Some("http://127.0.0.1:8545".to_string()),
            "{posture}"
        );
    }
    // The journey stack's defaults pass its own refusals.
    let mut c = parsed(&[("DATABASE_URL", "postgres:///catalyst"), (ENV, "journey")]);
    c.default_rpc_urls();
    refuse_remote_secrets(&mut c).unwrap();
}

/// lamb2's /about CPU and memory check is opt-in: off under every posture
/// and with none, on only when `ABOUT_RESOURCE_CHECK` says so.
#[test]
fn about_resource_check_is_off_unless_set() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    assert!(!parsed(&[db]).node.about_resource_check);
    for posture in ["mirror", "island", "journey"] {
        assert!(!parsed(&[db, (ENV, posture)]).node.about_resource_check);
        let on = parsed(&[db, (ENV, posture), ("ABOUT_RESOURCE_CHECK", "true")]);
        assert!(on.node.about_resource_check, "{posture}");
    }
}

#[test]
fn a_collections_subgraph_without_the_nft_walk_is_warned_about() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let eth = ("ETH_COLLECTIONS_SUBGRAPH_URL", "https://reader.example/l1");
    let matic = (
        "MATIC_COLLECTIONS_SUBGRAPH_URL",
        "https://reader.example/l2",
    );
    let index = ("MARKETPLACE_NFT_INDEX_ENABLED", "true");
    let sepolia = ("ETH_NETWORK", "sepolia");
    let run = |pairs: &[(&str, &str)]| unchecked_profile_items(&parsed(pairs));

    for pairs in [
        &[db, eth][..],
        &[db, matic][..],
        &[db, eth, matic, index, sepolia][..],
    ] {
        let warning = run(pairs).unwrap_or_else(|| panic!("{pairs:?}"));
        assert!(warning.contains("served as deployed"), "{warning}");
        assert!(!warning.contains("reader.example"), "{warning}");
    }
    for pairs in [
        &[db][..],
        &[db, index][..],
        &[db, eth, matic, index][..],
        &[db, ("ETH_COLLECTIONS_SUBGRAPH_URL", " ")][..],
    ] {
        assert_eq!(run(pairs), None, "{pairs:?}");
    }
}

#[test]
fn world_writes_need_the_names_reader_unless_chain_checks_are_skipped() {
    let db = ("DATABASE_URL", "postgres:///catalyst");
    let writes = ("WORLDS_WRITES_ENABLED", "true");
    let reader = (
        "MARKETPLACE_SUBGRAPH_URL",
        "https://reader.example/subgraph",
    );
    let walks = ("MARKETPLACE_NFT_INDEX_ENABLED", "true");
    let skip = ("IGNORE_BLOCKCHAIN_ACCESS_CHECKS", "true");
    let sepolia = ("ETH_NETWORK", "sepolia");
    let run = |pairs: &[(&str, &str)]| worlds_names_reader(&parsed(pairs));

    // A node that walks the nft index reads the holders there; the indexer
    // process publishes, `serve` reads the same database.
    for pairs in [
        &[db, writes, walks][..],
        &[db, writes, walks, ("MARKETPLACE_SUBGRAPH_URL", " ")][..],
        &[db, writes, walks, ("ETH_NETWORK", " mainnet ")][..],
    ] {
        assert_eq!(run(pairs).unwrap(), None, "{pairs:?}");
    }

    // One that does not (the index off, or off mainnet) needs the subgraph.
    for pairs in [&[db, writes][..], &[db, writes, walks, sepolia][..]] {
        let err = run(pairs).unwrap_err().to_string();
        assert!(err.contains("MARKETPLACE_SUBGRAPH_URL"), "{pairs:?}: {err}");
    }
    assert!(
        run(&[db, writes, ("MARKETPLACE_SUBGRAPH_URL", " ")]).is_err(),
        "a blank reader is no reader"
    );
    assert!(
        run(&[db, (ENV, "island"), writes]).is_err(),
        "refused under a posture too"
    );

    assert!(
        run(&[db, writes, skip])
            .unwrap()
            .unwrap()
            .contains("MARKETPLACE_SUBGRAPH_URL"),
        "no chain access: starts and warns"
    );

    for pairs in [
        &[db, writes, reader][..],
        &[db, writes, sepolia, reader][..],
        &[db][..],
        &[db, skip][..],
    ] {
        assert_eq!(run(pairs).unwrap(), None, "{pairs:?}");
    }
}

/// The indexer shares serve's environment but takes no world writes: the
/// shared refusals leave the names reader to serve.
#[test]
fn the_shared_check_leaves_the_names_reader_to_serve() {
    let pairs = [
        ("DATABASE_URL", "postgres:///catalyst"),
        ("WORLDS_WRITES_ENABLED", "true"),
    ];
    let map = env(&pairs);
    check(&parsed(&pairs), &lookup(&map)).unwrap();
}

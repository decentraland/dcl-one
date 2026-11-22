//! `serve` refuses world writes without the DCL NAME reader before it opens
//! the database; `index` keeps only the shared posture refusals.

use std::collections::HashMap;

use catalyst::config::Config;

fn run(pairs: &[(&str, &str)]) -> anyhow::Result<()> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let config = Config::parse_env_for_tests(map.clone()).unwrap();
    crate::serve_preflight(&config, &|key: &str| map.get(key).cloned())
}

const DB: (&str, &str) = ("DATABASE_URL", "postgres:///catalyst");
const WRITES: (&str, &str) = ("WORLDS_WRITES_ENABLED", "true");

#[test]
fn serve_refuses_world_writes_without_the_names_reader() {
    let err = run(&[DB, WRITES]).unwrap_err().to_string();
    assert!(err.contains("MARKETPLACE_SUBGRAPH_URL"), "{err}");
}

#[test]
fn serve_starts_with_the_reader_or_without_chain_access() {
    let reader = (
        "MARKETPLACE_SUBGRAPH_URL",
        "https://reader.example/subgraph",
    );
    let skip = ("IGNORE_BLOCKCHAIN_ACCESS_CHECKS", "true");
    for pairs in [
        &[DB][..],
        &[DB, WRITES, reader][..],
        &[DB, WRITES, skip][..],
    ] {
        run(pairs).unwrap_or_else(|e| panic!("{pairs:?}: {e}"));
    }
}

#[test]
fn serve_keeps_the_shared_posture_refusals() {
    let err = run(&[DB, ("NODE_POSTURE", "island"), ("SYNC_ENABLED", "true")])
        .unwrap_err()
        .to_string();
    assert!(err.contains("SYNC_ENABLED=true"), "{err}");
}

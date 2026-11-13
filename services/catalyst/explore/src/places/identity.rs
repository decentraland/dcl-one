//! Reproducible identities for newly created places. Existing identities always win.

use uuid::Uuid;

/// Both catalogue implementations use the URL namespace and this versioned JSON tuple.
/// Entity IDs are immutable, case-sensitive identifiers; do not normalize their spelling.
pub fn new_place_id(world: Option<&str>, entity_id: &str) -> Option<Uuid> {
    if entity_id.is_empty() || entity_id.trim() != entity_id {
        return None;
    }
    let scope = match world {
        Some(world) if !world.is_empty() && world.trim() == world => {
            format!("world:{}", world.to_lowercase())
        }
        Some(_) => return None,
        None => "genesis".to_string(),
    };
    let name = serde_json::to_vec(&["decentraland-place", "v1", &scope, entity_id]).ok()?;
    Some(Uuid::new_v5(&Uuid::NAMESPACE_URL, &name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_ids_match_places() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/explore/src/places/place_identity_vectors.json"
        )))
        .unwrap();
        for vector in vectors.as_array().unwrap() {
            assert_eq!(
                new_place_id(vector["world"].as_str(), vector["entity"].as_str().unwrap())
                    .unwrap()
                    .to_string(),
                vector["id"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn missing_or_padded_identity_inputs_are_rejected() {
        for (world, entity) in [
            (None, ""),
            (None, " entity"),
            (Some(""), "entity"),
            (Some(" world.dcl.eth"), "entity"),
        ] {
            assert_eq!(new_place_id(world, entity), None);
        }
        assert_ne!(new_place_id(None, "QmCase"), new_place_id(None, "qmcase"));
    }
}

use std::collections::HashSet;

use metrics::counter;
use serde_json::Value;

const SYNCED_SCENE_PARCELS_DIFFER: &str = "content_sync_scene_parcels_differ_total";

pub(super) fn parcels_differ(pointers: &[String], metadata: Option<&Value>) -> bool {
    let Some(parcels) = metadata
        .and_then(|m| m.pointer("/scene/parcels"))
        .and_then(Value::as_array)
        .and_then(|list| {
            list.iter()
                .map(Value::as_str)
                .collect::<Option<HashSet<&str>>>()
        })
    else {
        return true;
    };
    parcels != pointers.iter().map(String::as_str).collect()
}

pub(super) fn report_drift(
    entity_type: &str,
    entity_id: &str,
    pointers: &[String],
    metadata: Option<&Value>,
) -> bool {
    if entity_type != "scene" || !parcels_differ(pointers, metadata) {
        return false;
    }
    counter!(SYNCED_SCENE_PARCELS_DIFFER).increment(1);
    tracing::warn!(
        entity_id,
        pointers = pointers.len(),
        "synced scene: pointers differ from metadata.scene.parcels"
    );
    true
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ptrs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn scene(parcels: Value) -> Value {
        json!({ "main": "bin/index.js", "scene": { "base": "0,0", "parcels": parcels } })
    }

    #[test]
    fn parcels_differ_compares_sets() {
        let both = ptrs(&["0,0", "0,1", "-1,0"]);
        let differ = |parcels: Value| parcels_differ(&both, Some(&scene(parcels)));
        assert!(!differ(json!(["-1,0", "0,0", "0,1"])));
        assert!(!differ(json!(["0,1", "-1,0", "0,0", "0,1"])));
        assert!(differ(json!(["0,0", "0,1"])));
        assert!(differ(json!(["0,0", "0,1", "-1,0", "1,1"])));
        assert!(differ(json!(["0,0", "0,1", "-1,1"])));
        assert!(differ(json!("0,0")));
        assert!(differ(json!(["0,0", "0,1", 7])));
        assert!(parcels_differ(&both, Some(&json!({ "scene": {} }))));
        assert!(parcels_differ(&both, None));
    }

    #[test]
    fn only_scenes_whose_pointers_are_not_their_parcels_are_reported() {
        let pointers = ptrs(&["3,3"]);
        let matching = scene(json!(["3,3"]));
        let differing = scene(json!(["3,3", "3,4"]));
        assert!(!report_drift("scene", "a", &pointers, Some(&matching)));
        assert!(report_drift("scene", "b", &pointers, Some(&differing)));
        assert!(report_drift("scene", "c", &pointers, None));
        assert!(!report_drift("profile", "d", &pointers, Some(&differing)));
        assert!(!report_drift("profile", "e", &pointers, None));
    }
}

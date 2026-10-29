//! The entity file as a deploy parses it: the six deployable types, the
//! `content` mapping and the lenient shape (`id`/`version` optional) that the
//! write path accepts and fills in from the upload.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Scene,
    Profile,
    Wearable,
    Store,
    Emote,
    Outfits,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Scene => "scene",
            EntityType::Profile => "profile",
            EntityType::Wearable => "wearable",
            EntityType::Store => "store",
            EntityType::Emote => "emote",
            EntityType::Outfits => "outfits",
        }
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentMapping {
    pub file: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    #[serde(default)]
    pub id: String,

    #[serde(rename = "type")]
    pub entity_type: EntityType,

    pub pointers: Vec<String>,

    pub timestamp: i64,

    // The Explorer deploys a profile without snapshots as an entity with no `content`.
    #[serde(default)]
    pub content: Vec<ContentMapping>,

    #[serde(default)]
    pub version: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entity_without_content_has_no_files() {
        let entity: Entity = serde_json::from_str(
            r#"{"version":"v3","type":"profile","pointers":["0xab"],"timestamp":1,"metadata":{}}"#,
        )
        .unwrap();
        assert!(entity.content.is_empty());
        assert_eq!(entity.entity_type, EntityType::Profile);
    }

    #[test]
    fn entity_type_round_trips_lowercase_on_the_wire() {
        let json = serde_json::to_string(&EntityType::Outfits).unwrap();
        assert_eq!(json, "\"outfits\"");
        let parsed: EntityType = serde_json::from_str("\"scene\"").unwrap();
        assert_eq!(parsed, EntityType::Scene);
        assert!(serde_json::from_str::<EntityType>("\"Scene\"").is_err());
        assert_eq!(EntityType::Wearable.to_string(), "wearable");
    }

    #[test]
    fn entity_id_and_version_default_when_absent() {
        let entity: Entity = serde_json::from_value(serde_json::json!({
            "type": "profile",
            "pointers": ["0xabc"],
            "timestamp": 1,
            "content": [{"file": "body.png", "hash": "bafy"}]
        }))
        .unwrap();
        assert_eq!(entity.id, "");
        assert_eq!(entity.version, "");
        assert_eq!(entity.content[0].hash, "bafy");
        assert!(entity.metadata.is_none());
    }
}

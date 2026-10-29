//! The ERC-721 metadata document of a standard wearable
//! (`GET /queries/erc721/{chainId}/{contract}/{option}[/{emission}]` on the
//! content server, root and `/content`): the active entity at the item's URN
//! formatted as the token metadata marketplaces read.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::types::{ContentMapping, Entity};
use crate::ContentState;
use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Erc721Trait {
    pub trait_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Erc721Metadata {
    pub id: String,
    pub name: Option<String>,
    pub description: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    pub attributes: Vec<Erc721Trait>,
}

fn rarity_emission(rarity: &str) -> Option<u64> {
    match rarity {
        "common" => Some(100_000),
        "uncommon" => Some(10_000),
        "rare" => Some(5_000),
        "epic" => Some(1_000),
        "legendary" => Some(100),
        "mythic" => Some(10),
        "unique" => Some(1),
        _ => None,
    }
}

pub fn format_erc721_entity(
    urn: &str,
    entity: &Entity,
    base_url: &str,
    emission: Option<&str>,
) -> Erc721Metadata {
    let metadata = entity.metadata.as_ref();

    let name = metadata
        .and_then(|m| m.get("i18n"))
        .and_then(|i18n| i18n.as_array())
        .and_then(|arr| {
            let english = arr.iter().find(|entry| {
                entry
                    .get("code")
                    .and_then(|c| c.as_str())
                    .map(|c| c.to_lowercase() == "en")
                    .unwrap_or(false)
            });
            let chosen = english.or_else(|| arr.first());
            chosen.and_then(|entry| entry.get("text").and_then(|t| t.as_str()))
        })
        .map(|s| s.to_string());

    let rarity = metadata
        .and_then(|m| m.get("rarity"))
        .and_then(|r| r.as_str())
        .unwrap_or("");

    let description = match emission {
        Some(em) => {
            let total = rarity_emission(rarity).unwrap_or(0);
            format!("DCL Wearable {em}/{total}")
        }
        None => String::new(),
    };

    let image_hash = find_content_hash(entity, "image");
    let thumbnail_hash = find_content_hash(entity, "thumbnail");

    let base = base_url.trim_end_matches('/');
    let image = image_hash.map(|h| format!("{base}/contents/{h}"));
    let thumbnail = thumbnail_hash.map(|h| format!("{base}/contents/{h}"));

    let mut attributes = Vec::new();
    attributes.push(Erc721Trait {
        trait_type: "Rarity".to_string(),
        value: rarity.to_string(),
    });

    let category = extract_item_category(metadata);
    if let Some(cat) = category {
        attributes.push(Erc721Trait {
            trait_type: "Category".to_string(),
            value: cat,
        });
    }

    let tags = extract_item_tags(metadata);
    for tag in tags {
        attributes.push(Erc721Trait {
            trait_type: "Tag".to_string(),
            value: tag,
        });
    }

    let body_shapes = extract_body_shapes(metadata);
    for shape in body_shapes {
        attributes.push(Erc721Trait {
            trait_type: "Body Shape".to_string(),
            value: shape,
        });
    }

    Erc721Metadata {
        id: urn.to_string(),
        name,
        description,
        language: "en-US".to_string(),
        image,
        thumbnail,
        attributes,
    }
}

fn find_content_hash(entity: &Entity, field_name: &str) -> Option<String> {
    let metadata = entity.metadata.as_ref()?;

    let file_path = metadata.get(field_name).and_then(|v| v.as_str())?;

    entity
        .content
        .iter()
        .find(|c| c.file == file_path)
        .map(|c| c.hash.clone())
}

fn extract_item_category(metadata: Option<&serde_json::Value>) -> Option<String> {
    metadata?
        .get("data")
        .or_else(|| metadata?.get("emoteDataADR74"))
        .and_then(|d| d.get("category"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

fn extract_item_tags(metadata: Option<&serde_json::Value>) -> Vec<String> {
    metadata
        .and_then(|m| m.get("data").or_else(|| m.get("emoteDataADR74")))
        .and_then(|d| d.get("tags"))
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_body_shapes(metadata: Option<&serde_json::Value>) -> Vec<String> {
    let data = metadata.and_then(|m| m.get("data").or_else(|| m.get("emoteDataADR74")));

    let representations = data
        .and_then(|d| d.get("representations"))
        .and_then(|r| r.as_array());

    let mut shapes = std::collections::BTreeSet::new();
    if let Some(reps) = representations {
        for rep in reps {
            if let Some(body_shapes) = rep.get("bodyShapes").and_then(|b| b.as_array()) {
                for shape in body_shapes {
                    if let Some(s) = shape.as_str() {
                        match s {
                            "urn:decentraland:off-chain:base-avatars:BaseMale" | "BaseMale" => {
                                shapes.insert("BaseMale".to_string());
                            }
                            "urn:decentraland:off-chain:base-avatars:BaseFemale" | "BaseFemale" => {
                                shapes.insert("BaseFemale".to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    shapes.into_iter().collect()
}

fn get_urn_protocol(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("ethereum"),
        3 => Some("ropsten"),
        4 => Some("rinkeby"),
        5 => Some("goerli"),
        137 => Some("matic"),
        80001 => Some("mumbai"),
        80002 => Some("amoy"),
        11155111 => Some("sepolia"),
        _ => None,
    }
}

fn build_urn(protocol: &str, contract: &str, option: &str) -> String {
    let version = if contract.starts_with("0x") {
        "v2"
    } else {
        "v1"
    };
    format!(
        "urn:decentraland:{}:collections-{}:{}:{}",
        protocol, version, contract, option
    )
}

fn checked_f64_to_i64(v: f64) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    if v < i64::MIN as f64 || v > i64::MAX as f64 {
        return None;
    }
    Some(v as i64)
}

/// The served entity document back into an [`Entity`]; `content` entries
/// carry `file` on the wire and `key` in older stored forms.
fn value_to_entity(value: &serde_json::Value) -> Option<Entity> {
    let id = value.get("id")?.as_str()?.to_string();
    let entity_type = super::entity_parser::parse_entity_type(value.get("type")?.as_str()?)?;
    let pointers: Vec<String> = value
        .get("pointers")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let timestamp = checked_f64_to_i64(value.get("timestamp")?.as_f64()?)?;
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v3")
        .to_string();
    let metadata = value.get("metadata").cloned();

    let content: Vec<ContentMapping> = value
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let file = entry
                        .get("file")
                        .or_else(|| entry.get("key"))?
                        .as_str()?
                        .to_string();
                    let hash = entry.get("hash")?.as_str()?.to_string();
                    Some(ContentMapping { file, hash })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(Entity {
        id,
        entity_type,
        pointers,
        timestamp,
        content,
        version,
        metadata,
    })
}

#[derive(Debug, Deserialize)]
pub struct Erc721Params {
    #[serde(rename = "chainId")]
    pub chain_id: String,
    pub contract: String,
    pub option: String,
    #[serde(default)]
    pub emission: Option<String>,
}

/// `GET /queries/erc721/{chainId}/{contract}/{option}[/{emission}]`: 400 for
/// an unknown chain or a wearable without `rarity`, 404 for no active entity
/// at the URN (or a denylisted one), the document otherwise.
pub async fn get_erc721_entity(
    State(state): State<Arc<ContentState>>,
    Path(params): Path<Erc721Params>,
) -> Result<Json<Erc721Metadata>, AppError> {
    let invalid_chain = || AppError::invalid(format!("Invalid chainId '{}'", params.chain_id));
    let chain_id: u64 = params.chain_id.parse().map_err(|_| invalid_chain())?;
    let protocol = get_urn_protocol(chain_id).ok_or_else(invalid_chain)?;
    let pointer = build_urn(protocol, &params.contract, &params.option);

    let docs =
        crate::queries::active_entity_docs_by_pointers(&state.pool, std::slice::from_ref(&pointer))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    let not_found = || AppError::not_found("Entity does not exist");
    let doc = docs.into_iter().next().ok_or_else(not_found)?;
    if doc.id.is_empty() || state.denylist.contains(&doc.id) {
        return Err(not_found());
    }
    let entity_value = doc
        .to_value()
        .ok_or_else(|| AppError::Internal("unparseable entity document".to_string()))?;
    let metadata = entity_value.get("metadata").ok_or_else(not_found)?;
    if metadata.get("rarity").is_none() {
        return Err(AppError::invalid("Wearable is not standard."));
    }
    let entity = value_to_entity(&entity_value)
        .ok_or_else(|| AppError::Internal("Failed to parse entity from DB".to_string()))?;

    Ok(Json(format_erc721_entity(
        &pointer,
        &entity,
        &state.settings.content_public_url,
        params.emission.as_deref(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityType;

    #[test]
    fn rarity_emissions() {
        assert_eq!(rarity_emission("common"), Some(100_000));
        assert_eq!(rarity_emission("unique"), Some(1));
        assert_eq!(rarity_emission("invalid"), None);
    }

    #[test]
    fn format_minimal_entity() {
        let entity = Entity {
            id: "bafkrei".to_string(),
            entity_type: EntityType::Wearable,
            pointers: vec!["urn:decentraland:matic:collections-v2:0xabc:0".to_string()],
            timestamp: 1700000000000,
            content: vec![],
            version: "v3".to_string(),
            metadata: Some(serde_json::json!({
                "i18n": [{"code": "en", "text": "Cool Hat"}],
                "rarity": "epic",
                "data": {
                    "category": "hat",
                    "tags": ["cool", "hat"],
                    "representations": []
                }
            })),
        };

        let result = format_erc721_entity(
            "urn:decentraland:matic:collections-v2:0xabc:0",
            &entity,
            "https://peer.decentraland.org/content",
            Some("42"),
        );

        assert_eq!(result.name, Some("Cool Hat".to_string()));
        assert_eq!(result.description, "DCL Wearable 42/1000");
        assert_eq!(result.language, "en-US");
        assert!(
            result
                .attributes
                .iter()
                .any(|a| a.trait_type == "Rarity" && a.value == "epic")
        );
        assert!(
            result
                .attributes
                .iter()
                .any(|a| a.trait_type == "Category" && a.value == "hat")
        );
    }
}

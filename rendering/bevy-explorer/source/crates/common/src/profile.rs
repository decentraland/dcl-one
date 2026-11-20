use std::collections::HashMap;

use dcl_component::proto_components::common::Color3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AvatarSnapshots {
    pub face256: String,
    pub body: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AvatarEmote {
    #[serde(deserialize_with = "integral_slot")]
    pub slot: u32,
    pub urn: String,
}

/// A slot in any integral JSON number form (`1`, `1.0`, `1e0`): the catalyst
/// accepts and serves them all.
fn integral_slot<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let slot = f64::deserialize(deserializer)?;
    if slot.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&slot) {
        Ok(slot as u32)
    } else {
        Err(serde::de::Error::custom(format!(
            "invalid emote slot {slot}, expected a non-negative integer"
        )))
    }
}

/// Emotes without a valid slot are dropped, so one entry does not fail the
/// whole profile, nor a batched profile fetch with it.
fn valid_emotes<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<AvatarEmote>>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Valid(AvatarEmote),
        Invalid(serde::de::IgnoredAny),
    }
    Ok(
        Option::<Vec<Entry>>::deserialize(deserializer)?.map(|entries| {
            entries
                .into_iter()
                .filter_map(|entry| match entry {
                    Entry::Valid(emote) => Some(emote),
                    Entry::Invalid(_) => None,
                })
                .collect()
        }),
    )
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Default)]
pub struct AvatarColor {
    pub color: Color3,
}

impl PartialEq for AvatarColor {
    fn eq(&self, other: &Self) -> bool {
        self.color == other.color
    }
}

impl Eq for AvatarColor {}

impl AvatarColor {
    pub fn new(color: Color3) -> Self {
        Self { color }
    }
}

// Optional fields are omitted when absent, not serialized as `null`: other explorers
// fail to parse present-but-null fields in deployed profiles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvatarWireFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eyes: Option<AvatarColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hair: Option<AvatarColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<AvatarColor>,
    pub wearables: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_render: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "valid_emotes",
        skip_serializing_if = "Option::is_none"
    )]
    pub emotes: Option<Vec<AvatarEmote>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshots: Option<AvatarSnapshots>,
    // Load-bearing: this flatten field forces map-mode (named) encoding. Without it,
    // rmp's compact `to_vec` (see rmp_encode) writes this struct as a positional array,
    // and the `skip_serializing_if` fields above silently shorten it and shift the
    // remaining slots, corrupting the native scene<->renderer IPC round-trip. As a
    // bonus it preserves unknown avatar keys, mirroring SerializedProfile::extra_fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub struct LambdaProfiles {
    pub avatars: Vec<SerializedProfile>,
}

/// The compact profile tier: what a list row shows of a person, in the shape of the
/// catalyst's `POST /lambdas/profiles/metadata` (upstream's `ProfileMetadataDTO`). No field
/// skips serializing: the scene IPC encodes this struct positionally.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactProfile {
    /// The owner's address
    pub pointer: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub has_claimed_name: bool,
    #[serde(default)]
    pub name_color: Option<Color3>,
    /// The face snapshot's url; empty when the profile has none
    #[serde(default)]
    pub thumbnail_url: String,
}

impl From<&SerializedProfile> for CompactProfile {
    fn from(profile: &SerializedProfile) -> Self {
        Self {
            pointer: profile.eth_address.to_lowercase(),
            name: profile.name.clone(),
            has_claimed_name: profile.has_claimed_name,
            name_color: profile.name_color,
            thumbnail_url: profile
                .avatar
                .snapshots
                .as_ref()
                .map(|snapshots| snapshots.face256.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: i64,
    pub eth_address: String,
    #[serde(default)]
    pub has_claimed_name: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_connected_web3: Option<bool>,
    pub avatar: AvatarWireFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_color: Option<Color3>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, serde_json::Value>,
}

impl Default for SerializedProfile {
    fn default() -> Self {
        let avatar = serde_json::from_str("
            {
                \"bodyShape\":\"urn:decentraland:off-chain:base-avatars:BaseFemale\",
                \"wearables\":[
                    \"urn:decentraland:off-chain:base-avatars:f_sweater\",
                    \"urn:decentraland:off-chain:base-avatars:f_jeans\",
                    \"urn:decentraland:off-chain:base-avatars:bun_shoes\",
                    \"urn:decentraland:off-chain:base-avatars:standard_hair\",
                    \"urn:decentraland:off-chain:base-avatars:f_eyes_01\",
                    \"urn:decentraland:off-chain:base-avatars:f_eyebrows_00\",
                    \"urn:decentraland:off-chain:base-avatars:f_mouth_00\"
                ],
                \"emotes\":[
                    {\"slot\": 0, \"urn\": \"handsair\" },
                    {\"slot\": 1, \"urn\": \"wave\" },
                    {\"slot\": 2, \"urn\": \"fistpump\" },
                    {\"slot\": 3, \"urn\": \"dance\" },
                    {\"slot\": 4, \"urn\": \"raisehand\" },
                    {\"slot\": 5, \"urn\": \"clap\" },
                    {\"slot\": 6, \"urn\": \"money\" },
                    {\"slot\": 7, \"urn\": \"kiss\" },
                    {\"slot\": 8, \"urn\": \"headexplode\" },
                    {\"slot\": 9, \"urn\": \"shrug\" }
                ],
               \"snapshots\": {
                    \"face256\":\"\",
                    \"body\":\"\"
                },
                \"eyes\":{
                    \"color\":{\"r\":0.3,\"g\":0.2235294133424759,\"b\":0.99,\"a\":1}
                },
                \"hair\":{
                    \"color\":{\"r\":0.5960784554481506,\"g\":0.37254902720451355,\"b\":0.21568627655506134,\"a\":1}
                },
                \"skin\":{
                    \"color\":{\"r\":0.4901960790157318,\"g\":0.364705890417099,\"b\":0.27843138575553894,\"a\":1}
                }
            }
        ").unwrap();

        //                     {\"slot\": 1, \"urn\": \"urn:decentraland:matic:collections-v2:0x9087f96750c4e7607454c67c4f0bcf357ae62a46:2\" }

        Self {
            user_id: Default::default(),
            name: "Bevy_User".to_string(),
            version: 0,
            eth_address: "0x0000000000000000000000000000000000000000".to_owned(),
            has_claimed_name: Default::default(),
            has_connected_web3: Default::default(),
            avatar,
            name_color: None,
            extra_fields: HashMap::from_iter([
                (
                    "description".to_owned(),
                    serde_json::to_value(String::default()).unwrap(),
                ),
                (
                    "tutorialStep".to_owned(),
                    serde_json::to_value(0u32).unwrap(),
                ),
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emote_slots_read_in_every_integral_json_number_form() {
        let slot = |slot: &str| {
            serde_json::from_str::<AvatarEmote>(&format!(r#"{{"slot": {slot}, "urn": "wave"}}"#))
                .map(|emote| emote.slot)
        };
        for (text, expected) in [("1", 1), ("1.0", 1), ("1e0", 1), ("-0.0", 0)] {
            assert_eq!(slot(text).unwrap(), expected, "{text}");
        }
        for text in ["1.5", "-1", "\"1\""] {
            assert!(slot(text).is_err(), "{text}");
        }

        // The scene IPC still round-trips a profile's integer slots.
        let mut profile = SerializedProfile::default();
        profile.avatar.emotes = Some(vec![AvatarEmote {
            slot: 3,
            urn: "wave".to_owned(),
        }]);
        let bytes = crate::rpc::rmp_encode(&profile).unwrap();
        let decoded: SerializedProfile = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.avatar.emotes, profile.avatar.emotes);
    }

    #[test]
    fn emotes_with_an_invalid_slot_are_dropped_from_a_profile_batch() {
        let batch: LambdaProfiles = serde_json::from_str(
            r#"{"avatars": [{
                "ethAddress": "0x1",
                "avatar": {"wearables": [], "emotes": [
                    {"slot": 1.5, "urn": "spin"},
                    {"slot": 2.0, "urn": "wave"}
                ]}
            }]}"#,
        )
        .unwrap();
        assert_eq!(
            batch.avatars[0].avatar.emotes,
            Some(vec![AvatarEmote {
                slot: 2,
                urn: "wave".to_owned(),
            }])
        );

        let no_emotes: AvatarWireFormat = serde_json::from_str(r#"{"wearables": []}"#).unwrap();
        assert_eq!(no_emotes.emotes, None);
    }

    #[test]
    fn compact_profile_of_a_full_profile() {
        let color = Color3 {
            r: 1.0,
            g: 0.5,
            b: 0.0,
        };
        let mut profile = SerializedProfile {
            name: "Ryumi".to_owned(),
            eth_address: "0xAbCdEf0000000000000000000000000000000001".to_owned(),
            has_claimed_name: true,
            name_color: Some(color),
            ..Default::default()
        };
        profile.avatar.snapshots = Some(AvatarSnapshots {
            face256: "https://cdn.example/entities/bafy/face.png".to_owned(),
            body: "https://cdn.example/entities/bafy/body.png".to_owned(),
        });
        assert_eq!(
            CompactProfile::from(&profile),
            CompactProfile {
                pointer: "0xabcdef0000000000000000000000000000000001".to_owned(),
                name: "Ryumi".to_owned(),
                has_claimed_name: true,
                name_color: Some(color),
                thumbnail_url: "https://cdn.example/entities/bafy/face.png".to_owned(),
            }
        );

        profile.avatar.snapshots = None;
        assert_eq!(CompactProfile::from(&profile).thumbnail_url, "");
    }
}

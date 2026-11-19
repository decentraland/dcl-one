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
    pub slot: u32,
    pub urn: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
        let base_avatar = |name: &str| format!("urn:decentraland:off-chain:base-avatars:{name}");
        let color = |r, g, b| Some(AvatarColor::new(Color3 { r, g, b }));
        let avatar = AvatarWireFormat {
            name: None,
            body_shape: Some(base_avatar("BaseFemale")),
            eyes: color(0.3, 0.22352941, 0.99),
            hair: color(0.59607846, 0.37254903, 0.21568628),
            skin: color(0.49019608, 0.3647059, 0.2784314),
            wearables: [
                "f_sweater",
                "f_jeans",
                "bun_shoes",
                "standard_hair",
                "f_eyes_01",
                "f_eyebrows_00",
                "f_mouth_00",
            ]
            .into_iter()
            .map(base_avatar)
            .collect(),
            force_render: None,
            emotes: Some(
                [
                    "handsair",
                    "wave",
                    "fistpump",
                    "dance",
                    "raisehand",
                    "clap",
                    "money",
                    "kiss",
                    "headexplode",
                    "shrug",
                ]
                .into_iter()
                .zip(0..)
                .map(|(urn, slot)| AvatarEmote {
                    slot,
                    urn: urn.to_owned(),
                })
                .collect(),
            ),
            snapshots: Some(AvatarSnapshots {
                face256: String::new(),
                body: String::new(),
            }),
            extra: HashMap::new(),
        };

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
                    serde_json::Value::String(String::new()),
                ),
                ("tutorialStep".to_owned(), serde_json::Value::from(0u32)),
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

//! The room names and the vocabulary of the ONE room resolver: the room
//! canonical names, the LiveKit webhook's room classifier, and the request,
//! answer and error types of a resolution
//! (realm/world/scene request to the active deployment, its stable storage
//! namespace and its room). The resolver reads worlds' and content's rows,
//! so it lives in worlds (`worlds::rooms`, which re-exports these items) and
//! comms asks it through [`crate::interfaces::SceneIdentity`]. The client's
//! adapter request (comms), the authoritative host and the storage routes
//! resolve through that one reading, so they select the same room and the
//! same namespace (DATABASE.md: "Resolve room identity from the same active
//! deployment for both clients and authoritative hosts").
//!
//! Worlds use one canonical lobby (`world-<lower(world)>`) and one canonical
//! scene name (`w1:<base36 world byte length>:<world key><encoded scene>`).
//! Genesis/preview retain `scene:<signed realm>:<signed scene>`. Complete
//! world-scene names decode without the active catalogue, so redeployment
//! cannot hide a connected room from world revocation. Decoding establishes
//! identity only; admission, relay and metadata still require an active tuple.
//!
//! The private and community voice-chat rooms and the Pulse island rooms
//! (formerly in `shared::livekit`) are named here too, so every LiveKit room
//! name has this one owner.

use crate::shared::identity::CommunityId;
use crate::world_identity::{
    Parcel, ParcelSet, RealmId, SceneId, SceneNamespace, WorldName, is_world_realm_name,
};

pub const WORLD_ROOM_PREFIX: &str = "world-";
pub const GATEKEEPER_SCENE_ROOM_PREFIX: &str = "scene:";
pub const SCENE_ROOM_PREFIX: &str = "w1:";

// Part of the w1 wire format: never reorder, renumber or derive this table
// from the toolchain's Unicode version. These valid scene scalars contract
// from three UTF-8 bytes to two on lowercase. Space and slash are forbidden
// in source SceneIds, so the escapes are injective without restricting keys.
// Kelvin (three bytes to one) has the separate one-byte '/' escape.
const SCENE_CONTRACTIONS_V1: [char; 23] = [
    '\u{1e9e}', '\u{2126}', '\u{212b}', '\u{2c62}', '\u{2c64}', '\u{2c6d}', '\u{2c6e}', '\u{2c6f}',
    '\u{2c70}', '\u{2c7e}', '\u{2c7f}', '\u{a78d}', '\u{a7aa}', '\u{a7ab}', '\u{a7ac}', '\u{a7ad}',
    '\u{a7ae}', '\u{a7b0}', '\u{a7b1}', '\u{a7b2}', '\u{a7c5}', '\u{a7cb}', '\u{a7dc}',
];

fn encode_scene_v1(scene: &SceneId) -> String {
    let mut encoded = String::with_capacity(scene.as_str().len());
    for ch in scene.as_str().chars() {
        if ch == '\u{212a}' {
            encoded.push('/');
        } else if let Some(index) = SCENE_CONTRACTIONS_V1.iter().position(|&entry| entry == ch) {
            encoded.push(' ');
            encoded.push((b'A' + index as u8) as char);
        } else {
            encoded.push(ch);
        }
    }
    encoded
}

fn decode_scene_v1(encoded: &str) -> Option<SceneId> {
    let mut original = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();
    while let Some(ch) = chars.next() {
        original.push(match ch {
            '/' => '\u{212a}',
            ' ' => {
                let code = chars.next()?;
                let index = (code as u32).checked_sub('A' as u32)? as usize;
                *SCENE_CONTRACTIONS_V1.get(index)?
            }
            _ => ch,
        });
    }
    let scene = SceneId::parse(&original).ok()?;
    (encode_scene_v1(&scene) == encoded).then_some(scene)
}

fn base36(mut value: usize) -> String {
    let mut bytes = Vec::new();
    loop {
        bytes.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[value % 36]);
        value /= 36;
        if value == 0 {
            break;
        }
    }
    bytes.reverse();
    String::from_utf8(bytes).expect("base36 is ASCII")
}

/// The worlds service's lobby room: `world-<lower(name)>`.
pub fn worlds_service_world_room(world: &WorldName) -> String {
    format!("{WORLD_ROOM_PREFIX}{}", world.key())
}

/// An injective room identity. Length is the UTF-8 byte length of the world
/// key, not its character count; both keys may themselves contain colons.
/// Preserve entity ID bytes: case-bearing hashes name distinct deployments.
pub fn worlds_service_scene_room(world: &WorldName, scene: &SceneId) -> String {
    format!(
        "{SCENE_ROOM_PREFIX}{}:{}{}",
        base36(world.key().len()),
        world.key(),
        encode_scene_v1(scene)
    )
}

/// Decode identity without granting deployment or participant authority.
/// Canonical base36 lengths and exact re-encoding reject aliases, trimming,
/// overflow and a length ending inside a UTF-8 code point. Route-specific
/// proof/webhook bounds remain at their existing boundaries.
pub fn decode_world_scene_room(room: &str) -> Option<(WorldName, SceneId)> {
    let (length, keys) = room.strip_prefix(SCENE_ROOM_PREFIX)?.split_once(':')?;
    if length.is_empty()
        || !length
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
    {
        return None;
    }
    let length = usize::from_str_radix(length, 36).ok()?;
    let world = WorldName::parse(keys.get(..length)?).ok()?;
    let scene = decode_scene_v1(keys.get(length..)?)?;
    (worlds_service_scene_room(&world, &scene) == room).then_some((world, scene))
}

/// The gatekeeper's Genesis/preview scene room: `scene:<realm>:<scene id>`,
/// both as signed.
pub fn gatekeeper_scene_room(realm: &str, scene: &str) -> String {
    format!("{GATEKEEPER_SCENE_ROOM_PREFIX}{realm}:{scene}")
}

// ── voice and island rooms ──

pub const PRIVATE_VOICE_CHAT_ROOM_PREFIX: &str = "voice-chat-private-";
pub const COMMUNITY_VOICE_CHAT_ROOM_PREFIX: &str = "voice-chat-community-";

pub fn private_voice_chat_room_name(call_id: &uuid::Uuid) -> String {
    format!("{PRIVATE_VOICE_CHAT_ROOM_PREFIX}{call_id}")
}

pub fn is_private_voice_chat_room(room_name: &str) -> bool {
    room_name.starts_with(PRIVATE_VOICE_CHAT_ROOM_PREFIX)
}

/// The call id a private room name carries, if it is one.
pub fn private_call_id_from_room_name(room_name: &str) -> Option<uuid::Uuid> {
    uuid::Uuid::parse_str(room_name.strip_prefix(PRIVATE_VOICE_CHAT_ROOM_PREFIX)?).ok()
}

pub fn is_community_voice_chat_room(room_name: &str) -> bool {
    room_name.starts_with(COMMUNITY_VOICE_CHAT_ROOM_PREFIX)
}

pub fn community_voice_chat_room_name(community: &CommunityId) -> String {
    format!("{COMMUNITY_VOICE_CHAT_ROOM_PREFIX}{}", community.as_uuid())
}

/// The community id carried by its room name.
pub fn community_id_from_room_name(room_name: &str) -> Option<CommunityId> {
    CommunityId::parse(room_name.strip_prefix(COMMUNITY_VOICE_CHAT_ROOM_PREFIX)?).ok()
}

/// The room a Pulse cluster is published as; no other room shape here starts
/// with it.
pub const ISLAND_ROOM_PREFIX: &str = "island-";

pub fn island_room_name(cluster_id: &str) -> String {
    format!("{ISLAND_ROOM_PREFIX}{cluster_id}")
}

/// A canonical room's purpose. World-scene identity decodes locally; resolving
/// its active deployment is a separate operation for callers needing authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedRoom<'a> {
    Local(ResolvedRoom),
    WorldScene(&'a str),
    WorldLobby(&'a str),
}

pub fn parse_room(room: &str) -> Option<ParsedRoom<'_>> {
    if room.starts_with(SCENE_ROOM_PREFIX) {
        return decode_world_scene_room(room).map(|_| ParsedRoom::WorldScene(room));
    }
    let (realm, scene_id) = if let Some(raw) = room.strip_prefix(WORLD_ROOM_PREFIX) {
        let world = WorldName::parse(raw).ok()?;
        if worlds_service_world_room(&world) != room {
            return None;
        }
        return Some(ParsedRoom::WorldLobby(room));
    } else {
        let (realm, scene) = room
            .strip_prefix(GATEKEEPER_SCENE_ROOM_PREFIX)?
            .split_once(':')?;
        if realm.contains(':') || scene.contains(':') || is_world_realm_name(realm) {
            return None;
        }
        let name = WorldName::parse(realm).ok()?;
        let id = SceneId::parse(scene).ok()?;
        if name.display() != realm || id.as_str() != scene {
            return None;
        }
        (RealmId::Shared(realm.to_owned()), Some(id))
    };
    Some(ParsedRoom::Local(ResolvedRoom {
        realm,
        scene_id,
        deployment: None,
        room: room.to_owned(),
    }))
}

/// One parsed identity for relay policy, webhook presence and hosting events.
/// Unknown/ambiguous names carry no authority. Store faults remain errors so
/// a webhook can retry without marking the event applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomRefusal {
    Malformed,
    Unresolved,
    Ambiguous,
    Noncanonical,
    Legacy,
}

/// A refused relevant name carries its exact bytes, never an authoritative identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomClassification {
    Canonical(ResolvedRoom),
    RelevantRefused { room: String, reason: RoomRefusal },
    Unrelated,
}

pub fn refused_room(room: &str, reason: RoomRefusal) -> RoomClassification {
    RoomClassification::RelevantRefused {
        room: room.to_owned(),
        reason,
    }
}

pub async fn classify_room_name(
    scenes: &dyn crate::interfaces::SceneIdentity,
    room: &str,
) -> Result<RoomClassification, ResolveError> {
    scenes.classify_room(room).await
}

/// Deliberate event-projection filter: refusals have no identity or event.
/// Complete inventory consumers must use classify_room_name instead.
pub async fn resolve_room_name(
    scenes: &dyn crate::interfaces::SceneIdentity,
    room: &str,
) -> Result<Option<ResolvedRoom>, ResolveError> {
    Ok(match classify_room_name(scenes, room).await? {
        RoomClassification::Canonical(resolved) => Some(resolved),
        RoomClassification::RelevantRefused { .. } | RoomClassification::Unrelated => None,
    })
}

/// One active deployment as the resolver read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveDeployment {
    pub scene_id: SceneId,
    /// Display-only enrichment from this exact deployment; never authority.
    pub title: Option<String>,
    pub parcels: ParcelSet,
    /// The effective base parcel (`metadata.scene.base` when it belongs to
    /// the footprint, else the first canonical parcel).
    pub base: Parcel,
    /// The storage scope: the world scene's identity, or (`main`, `base`)
    /// for a Genesis scene; `None` for a world scene without an identity.
    pub namespace: Option<SceneNamespace>,
    /// A world scene published here (`mirror_source IS NULL`) or a Genesis
    /// deployment; `false` for a mirrored world scene, which grants no host
    /// authority.
    pub local: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("unknown or ambiguous canonical room {0}")]
    UnresolvedRoom(String),
    #[error("Scene \"{scene}\" not found in world \"{world}\".")]
    SceneNotInWorld { world: String, scene: String },
    #[error("Failed to resolve scene ID for world {0}")]
    WorldHasNoScene(String),
    #[error("no active scene at {realm} {parcel}")]
    NoSceneAtParcel { realm: String, parcel: String },
    #[error("the active deployment of {scene} has no usable parcel")]
    NoParcels { scene: String },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// What a caller asks the resolver for.
#[derive(Debug, Clone)]
pub enum RoomRequest<'a> {
    /// Decode this complete room and resolve its exact active deployment.
    NamedWorldScene { room: &'a str },
    /// The world lobby (worlds service `/worlds/{name}/comms`): no scene.
    WorldLobby { world: &'a WorldName },
    /// A named scene of a world (worlds service `/worlds/{name}/scenes/{id}/comms`):
    /// the scene must be deployed in that world.
    WorldScene {
        world: &'a WorldName,
        scene: &'a SceneId,
    },
    /// The gatekeeper's adapter request: a realm and a scene id. In a world
    /// realm a world-shaped scene id (`foo.dcl.eth`) resolves to the world's
    /// active scene and any other id is taken as given (looked up
    /// best-effort); in a shared realm the id is taken as given.
    Adapter {
        realm: &'a RealmId,
        scene: &'a SceneId,
    },
    /// The active scene standing on a parcel of a realm (the storage scope,
    /// `/scene-participants?pointer=`).
    Parcel { realm: &'a RealmId, parcel: Parcel },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoom {
    pub realm: RealmId,
    /// `None` only for the world lobby.
    pub scene_id: Option<SceneId>,
    /// The active deployment behind `scene_id`, when this node holds one.
    pub deployment: Option<ActiveDeployment>,
    pub room: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(name: &str) -> WorldName {
        WorldName::parse(name).unwrap()
    }

    fn scene(id: &str) -> SceneId {
        SceneId::parse(id).unwrap()
    }

    #[test]
    fn canonical_versioned_world_scene_preserves_original_scene_bytes() {
        assert_eq!(
            worlds_service_scene_room(&world("MiXeD.dcl.eth"), &scene("Cid-With:Case")),
            "w1:d:mixed.dcl.ethCid-With:Case"
        );
        assert_eq!(
            worlds_service_scene_room(&world("É:world"), &scene("Scene:é")),
            "w1:8:é:worldScene:é"
        );
    }

    #[test]
    fn canonical_world_scene_encoding_is_injective_across_delimiters_and_scene_case() {
        let a = worlds_service_scene_room(&world("custom-world"), &scene("Cid-with-hyphen"));
        let b = worlds_service_scene_room(&world("custom-world-cid"), &scene("with-hyphen"));
        assert_ne!(a, b, "world/scene delimiter collisions cannot share media");
        assert_ne!(
            worlds_service_scene_room(&world("custom-world"), &scene("CaseHash")),
            worlds_service_scene_room(&world("custom-world"), &scene("casehash")),
            "case-bearing entity IDs remain distinct deployments"
        );
    }

    #[test]
    fn canonical_wire_codec_never_grows_a_formerly_supported_room_or_json_string() {
        let world = world("w");
        let mut checked = 0;
        for ch in (0..=0x10ffff).filter_map(char::from_u32) {
            let original = ch.to_string().repeat(8);
            let Ok(id) = SceneId::parse(&original) else {
                continue;
            };
            if id.as_str() != original {
                continue;
            }
            let legacy = format!("scene-{}-{}", world.key(), original.to_lowercase());
            let canonical = worlds_service_scene_room(&world, &id);
            assert!(
                canonical.len() <= legacy.len(),
                "UTF-8 growth U+{:X}",
                ch as u32
            );
            assert!(
                serde_json::to_string(&canonical).unwrap().len()
                    <= serde_json::to_string(&legacy).unwrap().len(),
                "JSON growth U+{:X}",
                ch as u32
            );
            assert_eq!(
                decode_world_scene_room(&canonical),
                Some((world.clone(), id))
            );
            checked += 1;
        }
        assert!(
            checked > 1_000_000,
            "the pinned Unicode repertoire was exhausted"
        );
    }

    #[test]
    fn canonical_codec_keeps_exact_budgets_and_base36_digit_transitions() {
        for limit in [256, 512] {
            for world_bytes in [1, 34, 35, 36, 37, 185, 248, 504] {
                if world_bytes + 8 > limit {
                    continue;
                }
                let world = world(&"w".repeat(world_bytes));
                let lower_scene_bytes = limit - 7 - world_bytes;
                for original in [
                    "A".repeat(lower_scene_bytes),
                    "\u{212a}".repeat(lower_scene_bytes),
                ] {
                    let id = scene(&original);
                    let legacy = format!("scene-{}-{}", world.key(), original.to_lowercase());
                    assert_eq!(legacy.len(), limit);
                    let canonical = worlds_service_scene_room(&world, &id);
                    assert!(canonical.len() <= limit);
                    assert_eq!(
                        decode_world_scene_room(&canonical),
                        Some((world.clone(), id))
                    );
                }
            }
        }
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(1295), "zz");
        assert_eq!(base36(1296), "100");
    }

    #[test]
    fn canonical_scene_escape_table_is_immutable_and_handles_contextual_casing() {
        let world = world("w");
        let original = "\u{212a}\u{1e9e}\u{2126}\u{212b}\u{2c62}\u{2c64}\u{2c6d}\u{2c6e}\u{2c6f}\u{2c70}\u{2c7e}\u{2c7f}\u{a78d}\u{a7aa}\u{a7ab}\u{a7ac}\u{a7ad}\u{a7ae}\u{a7b0}\u{a7b1}\u{a7b2}\u{a7c5}\u{a7cb}\u{a7dc}";
        let room = worlds_service_scene_room(&world, &scene(original));
        assert_eq!(
            room,
            "w1:1:w/ A B C D E F G H I J K L M N O P Q R S T U V W"
        );
        assert_eq!(
            decode_world_scene_room(&room),
            Some((world.clone(), scene(original)))
        );
        for original in [
            "A\u{3a3}",
            "A\u{3a3}A",
            "\u{130}I\u{307}",
            "x\u{212a}\u{1e9e}Y",
            "\"\\\u{212a}\u{1e9e}",
        ] {
            let id = scene(original);
            let canonical = worlds_service_scene_room(&world, &id);
            let legacy = format!("scene-w-{}", original.to_lowercase());
            assert!(canonical.len() <= legacy.len());
            assert!(
                serde_json::to_string(&canonical).unwrap().len()
                    <= serde_json::to_string(&legacy).unwrap().len()
            );
            assert_eq!(
                decode_world_scene_room(&canonical),
                Some((world.clone(), id))
            );
        }
        for malformed in [
            "w1:1:w ",
            "w1:1:w X",
            "w1:1:w @",
            "w1:1:w\u{212a}",
            "w1:1:w\u{1e9e}",
            "w1:1:w\tA",
        ] {
            assert!(
                decode_world_scene_room(malformed).is_none(),
                "{malformed:?}"
            );
        }
    }

    #[test]
    fn canonical_versioned_parser_rejects_noncanonical_lengths_and_malformed_keys() {
        for room in [
            "w1:d:mixed.dcl.ethSceneA",
            "w1:8:é:worldScene:é",
            "w1:5:a:b:cS:cene",
        ] {
            assert_eq!(
                parse_room(room),
                Some(ParsedRoom::WorldScene(room)),
                "{room}"
            );
        }
        for room in [
            "w1:0d:mixed.dcl.ethSceneA",
            "w1:+d:mixed.dcl.ethSceneA",
            "w1:-1:mixed.dcl.ethSceneA",
            "w1:D:mixed.dcl.ethSceneA",
            "w1:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzz:aSceneA",
            "w1:1:éSceneA",
            "w1:0:SceneA",
            "w1:d:MiXeD.dcl.ethSceneA",
            "w1:d:mixed.dcl.eth",
            "w1:d:mixed.dcl.eth XSceneA",
            "w1:d:mixed.dcl.ethSceneA ",
            "w1:d:mixed.dcl.ethScene\nA",
            "scene-v2:13:mixed.dcl.eth:SceneA",
            "scene-v1:13:mixed.dcl.eth:SceneA",
            "scene-mixed.dcl.eth-scenea",
        ] {
            assert_eq!(parse_room(room), None, "{room:?}");
            // Broad routing sends relevant malformed/retired names to the
            // strict authority reader; it does not grant canonical identity.
            assert_eq!(
                room_scope(room),
                Some(RoomScope::WorldScene(room)),
                "{room:?}"
            );
        }
    }

    #[test]
    fn the_worlds_service_canonicalizes_world_and_preserves_scene() {
        assert_eq!(
            worlds_service_world_room(&world("Foo.eth")),
            "world-foo.eth"
        );
        assert_eq!(
            worlds_service_scene_room(&world("Foo.eth"), &scene("ABC")),
            "w1:7:foo.ethABC"
        );
    }

    #[test]
    fn the_webhook_classifier_reads_world_and_scene_rooms() {
        let Some(ParsedRoom::WorldLobby(room)) = parse_room("world-foo.dcl.eth") else {
            panic!("lobby")
        };
        assert_eq!(room, "world-foo.dcl.eth");
        assert_eq!(
            parse_room("w1:b:foo.dcl.ethbafybeihash"),
            Some(ParsedRoom::WorldScene("w1:b:foo.dcl.ethbafybeihash"))
        );
        assert_eq!(parse_room("lobby"), None);
        let Some(ParsedRoom::Local(s)) = parse_room("scene:main:abc") else {
            panic!("Genesis")
        };
        assert_eq!(s.realm, RealmId::Shared("main".into()));
        assert_eq!(s.scene_id.unwrap().as_str(), "abc");
    }

    // The names rooms-change-1 moved here from `shared::livekit` (or whose
    // duplicates there it deleted), each pinned to the string that module
    // produced for the same input.

    const CALL: &str = "6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b";

    fn call() -> uuid::Uuid {
        uuid::Uuid::parse_str(CALL).unwrap()
    }

    /// shared's `room_names_match_upstream`, moved with the names it pins.
    #[test]
    fn room_names_match_upstream() {
        assert_eq!(gatekeeper_scene_room("main", "abc"), "scene:main:abc");
        let id = uuid::Uuid::new_v4();
        let private = private_voice_chat_room_name(&id);
        assert_eq!(private, format!("voice-chat-private-{id}"));
        assert!(is_private_voice_chat_room(&private));
        assert_eq!(private_call_id_from_room_name(&private), Some(id));
        let community = CommunityId::from_uuid(id);
        let room = community_voice_chat_room_name(&community);
        assert_eq!(room, format!("voice-chat-community-{id}"));
        assert!(is_community_voice_chat_room(&room));
        assert_eq!(community_id_from_room_name(&room), Some(community));
        assert_eq!(
            community_id_from_room_name("voice-chat-community-not-a-uuid"),
            None
        );
        assert!(is_world_realm_name("foo.dcl.eth") && !is_world_realm_name("main"));
    }

    #[test]
    fn the_world_room_prefix() {
        assert_eq!(WORLD_ROOM_PREFIX, "world-");
    }

    #[test]
    fn the_gatekeeper_scene_room_takes_the_id_as_signed() {
        assert_eq!(
            gatekeeper_scene_room("LocalPreview", " b64 x"),
            "scene:LocalPreview: b64 x"
        );
    }

    /// shared's second `is_world_realm_name` read the same: `*.eth`, untrimmed.
    #[test]
    fn a_world_realm_reads_as_dot_eth_untrimmed() {
        assert!(is_world_realm_name("foo.dcl.eth") && !is_world_realm_name("main"));
        assert!(!is_world_realm_name("bar.eth "));
    }

    #[test]
    fn the_private_voice_chat_prefix() {
        assert_eq!(PRIVATE_VOICE_CHAT_ROOM_PREFIX, "voice-chat-private-");
    }

    #[test]
    fn a_private_voice_chat_room_is_named_by_its_call() {
        assert_eq!(
            private_voice_chat_room_name(&call()),
            "voice-chat-private-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
        );
    }

    #[test]
    fn a_private_voice_chat_room_is_known_by_its_prefix() {
        assert!(is_private_voice_chat_room(
            "voice-chat-private-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
        ));
        assert!(!is_private_voice_chat_room(
            "voice-chat-community-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
        ));
    }

    #[test]
    fn a_private_voice_chat_room_carries_its_call_id() {
        assert_eq!(
            private_call_id_from_room_name(
                "voice-chat-private-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some(call())
        );
        assert_eq!(
            private_call_id_from_room_name("voice-chat-private-not-a-uuid"),
            None
        );
        assert_eq!(
            private_call_id_from_room_name("island-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"),
            None
        );
    }

    #[test]
    fn the_community_voice_chat_prefix() {
        assert_eq!(COMMUNITY_VOICE_CHAT_ROOM_PREFIX, "voice-chat-community-");
    }

    #[test]
    fn a_community_voice_chat_room_is_known_by_its_prefix() {
        assert!(is_community_voice_chat_room(
            "voice-chat-community-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
        ));
        assert!(!is_community_voice_chat_room(
            "voice-chat-private-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
        ));
    }

    /// The community builder produces what this reads back.
    #[test]
    fn a_community_voice_chat_room_carries_its_community_id() {
        let community = CommunityId::from_uuid(call());
        assert_eq!(
            community_voice_chat_room_name(&community),
            "voice-chat-community-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
        );
        assert_eq!(
            community_id_from_room_name(
                "voice-chat-community-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"
            ),
            Some(community)
        );
        assert_eq!(
            community_id_from_room_name("voice-chat-private-6f1c2a7e-3b4d-4e5f-8a9b-0c1d2e3f4a5b"),
            None
        );
    }

    #[test]
    fn the_island_prefix() {
        assert_eq!(ISLAND_ROOM_PREFIX, "island-");
    }

    #[test]
    fn an_island_room_is_named_by_its_cluster() {
        assert_eq!(island_room_name("C12"), "island-C12");
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum RoomScope<'a> {
    Island(&'a str),
    WorldScene(&'a str),
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
}

pub(crate) fn room_scope(room: &str) -> Option<RoomScope<'_>> {
    if room.starts_with(SCENE_ROOM_PREFIX)
        || room.starts_with("scene-")
        || room.starts_with(WORLD_ROOM_PREFIX)
        || room.starts_with(GATEKEEPER_SCENE_ROOM_PREFIX)
    {
        return Some(RoomScope::WorldScene(room));
    }
    room.strip_prefix(ISLAND_ROOM_PREFIX)
        .filter(|island| identifier(island))
        .map(|_| RoomScope::Island(room))
}

/// Complete canonical world ownership, independent of current deployment.
/// A normal redeployment removes the old catalogue tuple while participants
/// may still occupy its room. Retired spellings require an operator drain.
pub fn is_world_room(room: &str, world: &WorldName) -> bool {
    room == worlds_service_world_room(world)
        || decode_world_scene_room(room).is_some_and(|(owner, _)| owner == *world)
}

#[cfg(test)]
mod owner_tests {
    use super::*;
    #[test]
    fn strict_scope_reads_builder_outputs() {
        assert_eq!(
            room_scope(&island_room_name("cluster-1")),
            Some(RoomScope::Island("island-cluster-1"))
        );
        assert_eq!(
            room_scope(&gatekeeper_scene_room("main", "scene-1")),
            Some(RoomScope::WorldScene("scene:main:scene-1"))
        );
        assert_eq!(
            room_scope(&worlds_service_world_room(
                &WorldName::parse("Foo.eth").unwrap()
            )),
            Some(RoomScope::WorldScene("world-foo.eth"))
        );
        assert_eq!(
            room_scope(&worlds_service_scene_room(
                &WorldName::parse("Foo.eth").unwrap(),
                &SceneId::parse("scene-1").unwrap()
            )),
            Some(RoomScope::WorldScene("w1:7:foo.ethscene-1"))
        );
        let community = CommunityId::from_uuid(uuid::Uuid::nil());
        assert_eq!(
            community_id_from_room_name(&community_voice_chat_room_name(&community)),
            Some(community)
        );
    }
    #[test]
    fn world_room_match_keeps_case_and_prefix_boundaries() {
        let world = WorldName::parse("Foo.eth").unwrap();
        for room in ["world-foo.eth", "w1:7:foo.ethId"] {
            assert!(is_world_room(room, &world));
        }
        for room in [
            "world-foo.ethical",
            "world-other.eth",
            "scene:foo.eth:id",
            "scene-foo.eth",
            "world-Foo.eth",
            "world-FOO.ETH-Scene",
            "scene-foo.eth-id",
            "w1:d:foo.eth-otherId",
        ] {
            assert!(!is_world_room(room, &world));
        }
    }
}

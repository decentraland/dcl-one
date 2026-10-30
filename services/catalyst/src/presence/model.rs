//! The typed rows one pass collects. The old crate parsed them out of the
//! JSON its siblings served over HTTP; here the sources hand them over as
//! values ([`super::sources`]), so what remains of that parsing is the
//! normalization the archive depends on: lowercase deduplicated wallets, the
//! base-parcel pointer of a hot scene, and worlds with users only.

#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub address: String,
    pub parcel_x: Option<i32>,
    pub parcel_y: Option<i32>,
    pub position_x: Option<f64>,
    pub position_y: Option<f64>,
    pub position_z: Option<f64>,
    pub last_ping: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Island {
    pub island_id: String,
    pub peer_count: i32,
    pub max_peers: Option<i32>,
    pub center_x: Option<f64>,
    pub center_y: Option<f64>,
    pub center_z: Option<f64>,
    pub radius: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HotScene {
    pub scene_id: String,
    pub name: Option<String>,
    pub base_x: Option<i32>,
    pub base_y: Option<i32>,
    pub users_count: Option<i32>,
    pub parcel_count: i32,
    pub creator: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveWorld {
    pub world_name: String,
    pub users: i32,
}

/// The `scene_occupancy.pointer` of a hot scene: its base parcel as `x,y`.
pub fn hot_scene_pointer(s: &HotScene) -> Option<String> {
    match (s.base_x, s.base_y) {
        (Some(x), Some(y)) => Some(format!("{x},{y}")),
        _ => None,
    }
}

/// Lowercase, first occurrence wins, empty strings dropped: the wallet list
/// the archive stores and counts.
pub fn dedup_lower<'a>(it: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for a in it {
        if a.is_empty() {
            continue;
        }
        let lo = a.to_lowercase();
        if seen.insert(lo.clone()) {
            out.push(lo);
        }
    }
    out
}

/// The worlds worth polling: a named world with at least one user (the old
/// `parse_active_worlds` filter over `/live-data`'s `perWorld`).
pub fn active_worlds(per_world: impl IntoIterator<Item = (String, i64)>) -> Vec<ActiveWorld> {
    per_world
        .into_iter()
        .filter(|(name, users)| !name.is_empty() && *users > 0)
        .map(|(world_name, users)| ActiveWorld {
            world_name,
            users: clamp_i32(users),
        })
        .collect()
}

pub fn clamp_i32(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(base: Option<(i32, i32)>) -> HotScene {
        HotScene {
            scene_id: "s".into(),
            name: None,
            base_x: base.map(|b| b.0),
            base_y: base.map(|b| b.1),
            users_count: None,
            parcel_count: 0,
            creator: None,
            description: None,
        }
    }

    #[test]
    fn hot_scene_pointer_is_the_base_parcel() {
        assert_eq!(
            hot_scene_pointer(&scene(Some((96, -132)))).as_deref(),
            Some("96,-132")
        );
        assert_eq!(hot_scene_pointer(&scene(None)), None);
    }

    #[test]
    fn wallets_are_lowercased_deduplicated_and_never_empty() {
        assert_eq!(
            dedup_lower(["0xAA", "0xbb", "0xAA"].into_iter()),
            vec!["0xaa", "0xbb"]
        );
        assert_eq!(dedup_lower(["0xEE", "", "0xee"].into_iter()), vec!["0xee"]);
        assert!(dedup_lower(std::iter::empty()).is_empty());
    }

    #[test]
    fn active_worlds_filters_zero_users_and_unnamed_worlds() {
        let worlds = active_worlds(vec![
            ("alice.eth".to_string(), 5),
            ("empty.eth".to_string(), 0),
            ("bob.dcl.eth".to_string(), 7),
            (String::new(), 3),
            ("huge.eth".to_string(), i64::MAX),
        ]);
        assert_eq!(worlds.len(), 3);
        assert_eq!(
            (worlds[0].world_name.as_str(), worlds[0].users),
            ("alice.eth", 5)
        );
        assert_eq!(
            (worlds[1].world_name.as_str(), worlds[1].users),
            ("bob.dcl.eth", 7)
        );
        assert_eq!(worlds[2].users, i32::MAX);
    }
}

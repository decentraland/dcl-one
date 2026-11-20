//! Every scenario the utility knows, by name.

pub mod connections;
pub mod connections_support;
pub mod crowd;
pub mod crowd_discovery;
pub mod explorer;
pub mod explorer_uplink;
pub mod livekit;
pub mod pulse;
pub mod support;
pub mod visit;
pub mod webtransport;

use crate::engine::Scenario;

/// Every tag a scenario may carry, each with one spelling: `--tag` selects by exact name, so a
/// second spelling of a group would split it.
pub const TAGS: &[&str] = &[
    "assignment",
    "battle",
    "churn",
    "connect",
    "connections",
    "crowd",
    "emotes",
    "explorer",
    "faults",
    "gatekeeper",
    "limits",
    "listeners",
    "livekit",
    "moderation",
    "probe",
    "pulse",
    "races",
    "realms",
    "refusals",
    "restart",
    "resync",
    "session",
    "slow",
    "smoke",
    "swarm",
    "uplink",
    "visit",
    "webtransport",
];

/// All scenarios, in the order a full run takes them.
pub fn registry() -> Vec<&'static Scenario> {
    let mut all = pulse::all();
    all.extend(crowd::all());
    all.extend(webtransport::all());
    all.extend(connections::all());
    all.extend(livekit::all());
    all.extend(crowd_discovery::all());
    all.extend(visit::all());
    all.extend(explorer::all());
    all
}

/// The scenarios a selection names: one name, or every scenario carrying a tag.
pub fn select(name: Option<&str>, tag: Option<&str>, all: bool) -> Vec<&'static Scenario> {
    registry()
        .into_iter()
        .filter(|scenario| {
            all || name.is_some_and(|name| scenario.name == name)
                || tag.is_some_and(|tag| scenario.tags.contains(&tag))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_of_every_scenario_is_a_known_one() {
        let unknown: Vec<String> = registry()
            .iter()
            .flat_map(|scenario| {
                scenario
                    .tags
                    .iter()
                    .filter(|tag| !TAGS.contains(tag))
                    .map(move |tag| format!("{}: {tag}", scenario.name))
            })
            .collect();
        assert!(unknown.is_empty(), "tags outside TAGS: {unknown:?}");
    }

    #[test]
    fn the_readme_names_every_scenario_and_every_tag() {
        let readme = include_str!("../../README.md");
        let missing: Vec<&str> = registry()
            .iter()
            .map(|scenario| scenario.name)
            .chain(TAGS.iter().copied())
            .filter(|name| !readme.contains(&format!("`{name}`")))
            .collect();
        assert!(missing.is_empty(), "the README does not name {missing:?}");
    }

    #[test]
    fn the_registry_holds_every_scenario_once() {
        let names: Vec<&str> = registry().iter().map(|scenario| scenario.name).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "a name twice in {names:?}");
        // the README's groups: pulse 15 (crowd-visit among them), connections 9, explorer 17
        assert_eq!(names.len(), 41, "{names:?}");
    }

    #[test]
    fn every_known_tag_selects_a_scenario() {
        for tag in TAGS {
            assert!(!select(None, Some(tag), false).is_empty(), "{tag}");
        }
    }
}

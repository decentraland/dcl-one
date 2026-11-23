//! Every scenario the utility knows, by name.

pub mod connections;
pub mod connections_support;
pub mod crowd;
pub mod crowd_discovery;
pub mod explorer;
pub mod explorer_uplink;
pub mod livekit;
pub mod pulse;
pub mod pulse_clock;
pub mod pulse_enet;
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
    "timeouts",
    "uplink",
    "visit",
    "webtransport",
];

/// All scenarios, in the order a full run takes them.
pub fn registry() -> Vec<&'static Scenario> {
    let mut all = pulse::all();
    all.extend(pulse_clock::all());
    all.extend(pulse_enet::all());
    all.extend(crowd::all());
    all.extend(webtransport::all());
    all.extend(connections::all());
    all.extend(livekit::all());
    all.extend(crowd_discovery::all());
    all.extend(visit::all());
    all.extend(explorer::all());
    all
}

/// The scenarios a selection names: one name, every scenario carrying a tag, or all of them. A
/// scenario that runs only by name ([`crate::engine::Needs::by_name`]) is selected by its name
/// alone.
pub fn select(name: Option<&str>, tag: Option<&str>, all: bool) -> Vec<&'static Scenario> {
    registry()
        .into_iter()
        .filter(|scenario| {
            name == Some(scenario.name) || (!scenario.needs.by_name && covers(scenario, tag, all))
        })
        .collect()
}

/// The scenarios the tag or `all` covers that [`select`] leaves out: they run only by name and
/// the selection does not name them.
pub fn left_out(name: Option<&str>, tag: Option<&str>, all: bool) -> Vec<&'static Scenario> {
    registry()
        .into_iter()
        .filter(|scenario| {
            scenario.needs.by_name && name != Some(scenario.name) && covers(scenario, tag, all)
        })
        .collect()
}

/// The scenarios a listing shows: those carrying the tag, or all of them.
pub fn listed(tag: Option<&str>) -> Vec<&'static Scenario> {
    registry()
        .into_iter()
        .filter(|scenario| covers(scenario, tag, tag.is_none()))
        .collect()
}

fn covers(scenario: &Scenario, tag: Option<&str>, all: bool) -> bool {
    all || tag.is_some_and(|tag| scenario.tags.contains(&tag))
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
        // the README's groups: pulse 20 (pulse-handshake-clock and crowd-visit among them),
        // connections 9, explorer 17
        assert_eq!(names.len(), 46, "{names:?}");
    }

    #[test]
    fn all_and_a_tag_leave_out_a_scenario_that_runs_only_by_name() {
        let by_name = visit::CROWD_VISIT.name;
        let names = |selection: Vec<&'static Scenario>| -> Vec<&'static str> {
            selection
                .into_iter()
                .map(|scenario| scenario.name)
                .collect()
        };
        let without = registry()
            .iter()
            .filter(|scenario| !scenario.needs.by_name)
            .count();
        assert_eq!(select(None, None, true).len(), without);
        assert!(!names(select(None, None, true)).contains(&by_name));
        for tag in visit::CROWD_VISIT.tags {
            assert!(
                !names(select(None, Some(tag), false)).contains(&by_name),
                "{tag}"
            );
        }
        assert_eq!(names(select(Some(by_name), None, false)), vec![by_name]);

        assert_eq!(names(left_out(None, None, true)), vec![by_name]);
        assert_eq!(names(left_out(None, Some("crowd"), false)), vec![by_name]);
        assert!(left_out(None, Some("smoke"), false).is_empty());
        assert!(left_out(Some(by_name), None, false).is_empty());
        assert!(left_out(Some(by_name), None, true).is_empty());
        assert_eq!(listed(None).len(), registry().len());
        assert!(names(listed(Some("visit"))).contains(&by_name));
    }

    #[test]
    fn every_known_tag_lists_a_scenario() {
        for tag in TAGS {
            assert!(!listed(Some(tag)).is_empty(), "{tag}");
        }
    }
}

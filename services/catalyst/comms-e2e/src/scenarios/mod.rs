//! Every scenario the utility knows, by name.

pub mod connections;
pub mod connections_support;
pub mod crowd;
pub mod crowd_discovery;
pub mod explorer;
pub mod livekit;
pub mod pulse;
pub mod support;
pub mod webtransport;

use crate::engine::Scenario;

/// All scenarios, in the order a full run takes them.
pub fn registry() -> Vec<&'static Scenario> {
    let mut all = pulse::all();
    all.extend(crowd::all());
    all.extend(webtransport::all());
    all.extend(connections::all());
    all.extend(livekit::all());
    all.extend(crowd_discovery::all());
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

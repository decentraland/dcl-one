//! Every scenario the utility knows, by name.

use crate::engine::Scenario;

/// All scenarios, in the order a full run takes them.
pub fn registry() -> Vec<&'static Scenario> {
    Vec::new()
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

//! The route guard each domain crate runs as a unit test: CI runs no clippy
//! over `services/catalyst`, so this scan keeps production routes on
//! [`crate::routing`]. Items under `#[cfg(test)]`, files named `*test*` and
//! `tests/` folders are test code and exempt.

use std::path::Path;

const METHODS: [&str; 6] = ["any", "delete", "get", "patch", "post", "put"];

/// `file:line` of every production line under `src` that names one of axum's
/// method routers (`axum::routing::get`, or `routing::get` inside `use axum::{..}`).
pub fn direct_axum_routes(src: &Path) -> Vec<String> {
    let mut found = Vec::new();
    visit(src, &mut found);
    found
}

fn visit(dir: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                visit(&path, found);
            }
            continue;
        }
        if !name.ends_with(".rs") || name.contains("test") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for number in direct_lines(&text) {
            found.push(format!("{}:{number}", path.display()));
        }
    }
}

/// 1-based numbers of the production lines in `text` that name an axum method router.
fn direct_lines(text: &str) -> Vec<usize> {
    let mut found = Vec::new();
    // Brace depth, the depth a skipped test item closes at, and whether
    // a `#[cfg(test)]` item has not reached its `;` or `{` yet.
    let (mut depth, mut skip_until, mut pending) = (0i64, None, false);
    // Inside `use axum::{ .. }`, `routing::get` names axum's own.
    let mut axum_group = false;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let delta = line.matches('{').count() as i64 - line.matches('}').count() as i64;
        if let Some(end) = skip_until {
            depth += delta;
            if depth <= end {
                skip_until = None;
            }
            continue;
        }
        if pending {
            if delta > 0 {
                skip_until = Some(depth);
                pending = false;
            } else if !trimmed.starts_with("#[")
                && (trimmed.ends_with(';') || trimmed.ends_with('}'))
            {
                pending = false;
            }
            depth += delta;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            pending = true;
            continue;
        }
        if trimmed.starts_with("use axum::{") && !trimmed.ends_with(';') {
            axum_group = true;
        }
        let direct = line
            .split_once("axum::routing::")
            .is_some_and(|(_, after)| names_a_method(after));
        let grouped = (axum_group || trimmed.starts_with("use axum::{"))
            && trimmed
                .split_once("routing::")
                .is_some_and(|(_, after)| names_a_method(after));
        if direct || grouped {
            found.push(number + 1);
        }
        if axum_group && trimmed.ends_with("};") {
            axum_group = false;
        }
        depth += delta;
    }
    found
}

fn names_a_method(after: &str) -> bool {
    let names = match after.strip_prefix('{') {
        Some(group) => group.split('}').next().unwrap_or(""),
        None => after,
    };
    names
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .take(if after.starts_with('{') {
            usize::MAX
        } else {
            1
        })
        .any(|name| METHODS.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::direct_lines;

    #[test]
    fn production_lines_are_found_and_test_items_are_not() {
        let text = r#"use axum::routing::{MethodRouter, get};
use axum::routing::MethodRouter;
use axum::{
    Router,
    routing::post,
};
use axum::{Json, routing::any};
use crate::shared::routing::{get, put};

#[cfg(test)]
mod tests;

fn router() -> MethodRouter {
    axum::routing::delete(handler)
}

#[cfg(test)]
mod inline {
    use axum::routing::get;
    fn app() { let _ = axum::routing::post(h); }
}

#[cfg(test)]
fn helper() {
    axum::routing::put(h);
}

fn after() { axum::routing::patch(h); }
"#;
        assert_eq!(direct_lines(text), [1, 5, 7, 14, 28]);
    }
}

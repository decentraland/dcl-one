//! The route guard each domain crate runs as a unit test: CI runs no clippy
//! over `services/catalyst`, so this scan keeps production routes on
//! [`crate::routing`]. Items under a test-only `#[cfg(..)]`, test files and
//! `tests/` folders are exempt. It reads tokens, not types: chained
//! `MethodRouter` methods (`.post(..)`) stay a review rule.

use std::path::Path;

const METHODS: [&str; 7] = ["any", "delete", "get", "on", "patch", "post", "put"];

/// `#[cfg(..)]` forms that compile only into tests or test support.
const TEST_CFGS: [&str; 3] = [
    "#[cfg(test)]",
    "#[cfg(feature = \"test-support\")]",
    "#[cfg(any(test, feature = \"test-support\"))]",
];

/// `file:line` of every production line under `src` that names one of axum's
/// method routers or imports `axum::routing` as a module.
pub fn direct_axum_routes(src: &Path) -> Vec<String> {
    let mut found = Vec::new();
    visit(src, &mut found);
    found
}

fn is_test_file(name: &str) -> bool {
    name == "tests.rs"
        || name == "testkit.rs"
        || name.ends_with("_tests.rs")
        || name.starts_with("test_")
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
        if !name.ends_with(".rs") || is_test_file(&name) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for number in direct_lines(&text) {
            found.push(format!("{}:{number}", path.display()));
        }
    }
}

/// Whether an `r` after `before` starts a raw string (`r"`, `br"`), not an identifier.
fn raw_prefix(before: &[char]) -> bool {
    let ident = |c: &char| c.is_alphanumeric() || *c == '_';
    match before {
        [.., p, 'b'] => !ident(p),
        [.., p] => !ident(p),
        [] => true,
    }
}

/// `text` with comments and string and char literals blanked, newlines kept.
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r' && (next == Some('"') || next == Some('#')) && raw_prefix(&chars[..i]) {
            let hashes = chars[i + 1..].iter().take_while(|&&h| h == '#').count();
            if chars.get(i + 1 + hashes) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            let start = i;
            i += 2 + hashes;
            let closes = |at: usize| {
                chars[at] == '"'
                    && chars[at + 1..]
                        .iter()
                        .take(hashes)
                        .filter(|&&h| h == '#')
                        .count()
                        == hashes
            };
            while i < chars.len() && !closes(i) {
                i += 1;
            }
            i = (i + 1 + hashes).min(chars.len());
            out.extend(chars[start..i].iter().map(|&c| blank(c)));
        } else if c == '"' {
            out.push(' ');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                let step = if chars[i] == '\\' { 2 } else { 1 };
                for k in 0..step {
                    if let Some(&c) = chars.get(i + k) {
                        out.push(blank(c));
                    }
                }
                i += step;
            }
            out.push(' ');
            i += 1;
        } else if c == '\'' {
            // A char literal closes within a few chars; otherwise it is a lifetime.
            let end = if next == Some('\\') {
                chars[i + 2..]
                    .iter()
                    .position(|&c| c == '\'')
                    .map(|p| i + 2 + p)
            } else {
                (chars.get(i + 2) == Some(&'\'')).then_some(i + 2)
            };
            match end {
                Some(end) => {
                    out.extend(chars[i..=end].iter().map(|&c| blank(c)));
                    i = end + 1;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// 1-based numbers of the production lines in `text` that name an axum method
/// router or import `axum::routing` as a module.
fn direct_lines(text: &str) -> Vec<usize> {
    let code = code_only(text);
    let mut found = Vec::new();
    // Brace depth, the depth a skipped test item closes at, and whether
    // a test-only item has not reached its `;` or `{` yet.
    let (mut depth, mut skip_until, mut pending) = (0i64, None, false);
    // Inside a multi-line `use axum::{ .. }` or `use axum::routing::{ .. }`.
    // A routing group nested in `use axum::{ .. }` ends at its own `}`.
    let (mut axum_group, mut routing_group, mut nested) = (false, false, false);
    for (number, (line, original)) in code.lines().zip(text.lines()).enumerate() {
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
        // Attributes are matched before literals are blanked.
        if TEST_CFGS.contains(&original.trim()) || trimmed.starts_with("#[cfg(all(test") {
            pending = true;
            continue;
        }
        let opens_group = !trimmed.ends_with(';');
        if trimmed.starts_with("use axum::{") && opens_group {
            axum_group = true;
        }
        if trimmed.starts_with("use axum::routing::{") && opens_group {
            routing_group = true;
        }
        let opens_nested = axum_group && trimmed.ends_with("routing::{");
        let direct = line
            .split_once("axum::routing::")
            .is_some_and(|(_, after)| names_a_method(after));
        let grouped = (axum_group || trimmed.starts_with("use axum::{"))
            && trimmed
                .split_once("routing::")
                .is_some_and(|(_, after)| names_a_method(after));
        let in_routing_group = routing_group
            && !trimmed.starts_with("use ")
            && trimmed
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|name| METHODS.contains(&name) || name == "self");
        let module = trimmed == "use axum::routing;"
            || ((axum_group || trimmed.starts_with("use axum::{"))
                && trimmed
                    .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
                    .any(|name| name == "routing"));
        if direct || grouped || in_routing_group || module {
            found.push(number + 1);
        }
        if nested && trimmed.starts_with('}') {
            routing_group = false;
            nested = false;
        }
        if opens_nested {
            routing_group = true;
            nested = true;
        }
        if trimmed.ends_with("};") {
            axum_group = false;
            routing_group = false;
            nested = false;
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
        .any(|name| METHODS.contains(&name) || name == "self")
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

    #[test]
    fn literals_cfg_forms_groups_and_module_imports() {
        let text = r##"#[cfg(test)]
mod unbalanced {
    const OPEN: &str = "{";
    const BRACE: char = '{';
    // }
}
fn prod<'a>() { axum::routing::on(f, h); }
const DOC: &str = "axum::routing::get";
/* axum::routing::post */
const RAW: &str = r#"axum::routing::put {"#;
const BYTES: &[u8] = br#"{"#;
#[cfg(any(test, feature = "test-support"))]
fn support() { axum::routing::get(h); }
#[cfg(any(windows, test))]
fn platform() { axum::routing::get(h); }
use axum::routing::{
    MethodRouter,
    post,
};
use axum::routing;
use axum::{Json, routing};
use axum::routing::MethodRouter;
use axum::{
    Json,
    routing::{
        MethodRouter,
        get,
    },
    Router,
};
use axum::routing::{self, MethodRouter};
"##;
        assert_eq!(direct_lines(text), [7, 15, 18, 20, 21, 27, 31]);
    }

    #[test]
    fn test_files_are_named_explicitly() {
        for name in ["tests.rs", "http_tests.rs", "test_db.rs", "testkit.rs"] {
            assert!(super::is_test_file(name), "{name}");
        }
        for name in ["latest.rs", "attestation.rs", "contest.rs"] {
            assert!(!super::is_test_file(name), "{name}");
        }
    }
}

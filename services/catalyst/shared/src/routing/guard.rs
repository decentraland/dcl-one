//! The route and spawn guards each domain crate runs as unit tests: CI runs no
//! clippy over `services/catalyst`, so these scans keep production routes on
//! [`crate::routing`] and production spawns on [`crate::task`]. Items and
//! out-of-line modules under a test-only `#[cfg(..)]`, test files and `tests/`
//! folders are exempt. They
//! read tokens, not types: chained `MethodRouter` methods (`.post(..)`) stay a
//! review rule.

use std::path::{Path, PathBuf};

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
    visit(src, direct_lines, &mut found);
    found
}

/// `file:line` of every production line under `src` that names `tokio::spawn`.
pub fn direct_tokio_spawns(src: &Path) -> Vec<String> {
    let mut found = Vec::new();
    visit(src, spawn_lines, &mut found);
    found
}

fn is_test_file(name: &str) -> bool {
    name == "tests.rs"
        || name == "testkit.rs"
        || name.ends_with("_tests.rs")
        || name.starts_with("test_")
}

fn visit(src: &Path, scan: fn(&str) -> Vec<usize>, found: &mut Vec<String>) {
    let mut files = Vec::new();
    collect(src, &mut files);
    let texts: Vec<(PathBuf, String)> = files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            (path, text)
        })
        .collect();
    let test_modules: Vec<PathBuf> = texts
        .iter()
        .flat_map(|(path, text)| test_module_paths(path, text))
        .collect();
    for (path, text) in &texts {
        if test_modules.iter().any(|module| path.starts_with(module)) {
            continue;
        }
        for number in scan(text) {
            found.push(format!("{}:{number}", path.display()));
        }
    }
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                collect(&path, files);
            }
        } else if name.ends_with(".rs") && !is_test_file(&name) {
            files.push(path);
        }
    }
}

/// The files and folders of the out-of-line modules `path` declares under a
/// test-only `#[cfg(..)]` (`#[cfg(test)] mod name;`).
fn test_module_paths(path: &Path, text: &str) -> Vec<PathBuf> {
    let stem = path.file_stem().unwrap().to_string_lossy();
    let dir = path.parent().unwrap();
    let dir = if ["lib", "main", "mod"].contains(&stem.as_ref()) {
        dir.to_path_buf()
    } else {
        dir.join(stem.as_ref())
    };
    let mut modules = Vec::new();
    let mut test_only = false;
    for line in text.lines().map(str::trim) {
        if TEST_CFGS.contains(&line) || line.starts_with("#[cfg(all(test") {
            test_only = true;
            continue;
        }
        if test_only && line.starts_with("#[") {
            continue;
        }
        if test_only
            && let Some(name) = line
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub ")
                .strip_prefix("mod ")
                .and_then(|rest| rest.strip_suffix(';'))
        {
            modules.push(dir.join(format!("{name}.rs")));
            modules.push(dir.join(name));
        }
        test_only = false;
    }
    modules
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

/// 1-based numbers and code of the lines in `text` outside test-only items.
fn production_lines(text: &str) -> Vec<(usize, String)> {
    let code = code_only(text);
    let mut lines = Vec::new();
    // Brace depth, the depth a skipped test item closes at, and whether
    // a test-only item has not reached its `;` or `{` yet.
    let (mut depth, mut skip_until, mut pending) = (0i64, None, false);
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
        lines.push((number + 1, line.to_string()));
        depth += delta;
    }
    lines
}

/// 1-based numbers of the production lines in `text` that name `tokio::spawn`
/// or import it in a single-line `use tokio::{ .. }` group.
fn spawn_lines(text: &str) -> Vec<usize> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    production_lines(text)
        .into_iter()
        .filter(|(_, line)| {
            let line: String = line.split_whitespace().collect();
            let path = ["tokio::spawn", "tokio::task::spawn"].iter().any(|path| {
                line.split(path)
                    .skip(1)
                    .any(|after| !after.starts_with(ident))
            });
            let grouped = line.starts_with("usetokio::")
                && line.contains('{')
                && line.split(|c: char| !ident(c)).any(|name| name == "spawn");
            path || grouped
        })
        .map(|(number, _)| number)
        .collect()
}

/// 1-based numbers of the production lines in `text` that name an axum method
/// router or import `axum::routing` as a module.
fn direct_lines(text: &str) -> Vec<usize> {
    let mut found = Vec::new();
    // Inside a multi-line `use axum::{ .. }` or `use axum::routing::{ .. }`.
    // A routing group nested in `use axum::{ .. }` ends at its own `}`.
    let (mut axum_group, mut routing_group, mut nested) = (false, false, false);
    for (number, line) in production_lines(text) {
        let line = line.as_str();
        let trimmed = line.trim();
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
            found.push(number);
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
    fn spawn_lines_skip_tests_and_other_tokio_functions() {
        let text = r#"use tokio::task::{JoinHandle, spawn_blocking};
use tokio::task::{JoinHandle, spawn};
fn a() { tokio::spawn(f); }
fn b() { tokio::task::spawn_blocking(f); }
fn c() { tokio::task::spawn(f); }
const S: &str = "tokio::spawn(f)";
#[cfg(test)]
mod tests {
    fn d() { tokio::spawn(f); }
}
fn e() { crate::shared::task::spawn(f); }
"#;
        assert_eq!(super::spawn_lines(text), [2, 3, 5]);
    }

    #[test]
    fn out_of_line_test_modules_are_found() {
        let text = "#[cfg(test)]\nmod load;\nmod prod;\n#[cfg(test)]\n#[path = \"x.rs\"]\npub(crate) mod other;\n";
        let found = super::test_module_paths(std::path::Path::new("src/bus/broker.rs"), text);
        let names: Vec<String> = found.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(
            names,
            [
                "src/bus/broker/load.rs",
                "src/bus/broker/load",
                "src/bus/broker/other.rs",
                "src/bus/broker/other",
            ]
        );
        let lib = super::test_module_paths(
            std::path::Path::new("src/lib.rs"),
            "#[cfg(test)]\nmod kit;\n",
        );
        assert_eq!(
            lib,
            [
                std::path::Path::new("src/kit.rs"),
                std::path::Path::new("src/kit")
            ]
        );
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

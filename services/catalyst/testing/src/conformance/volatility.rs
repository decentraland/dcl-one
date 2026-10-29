//! Per-section ignore rules (`volatility.toml`, a minimal hand-rolled parser)
//! and the dotted-path glob they match with; the replay uses the empty
//! `Volatility::default()`, so only a fixture's `volatile_paths` and the global
//! IGNORED_FIELDS of diff.rs apply, as the old replay binary did. The audit at
//! the end is the rule every fixture's `volatile_paths` must satisfy: a path
//! names a leaf of the baseline body or an array's order (`path[]`), never a
//! whole body, object or array, and the leaves it covers stay at or under half
//! of the body's (`every_fixture_declares_leaf_volatility` in
//! `tests/support/conformance/replay.rs` walks the corpus with it).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::Value;

use super::fixture::is_order_path;

/// The leaf paths of a body, array indices collapsed (`data[3].id` is
/// `data.id`, the scrubber's view); an empty object or array is a leaf.
pub fn leaf_paths(body: &Value) -> Vec<String> {
    fn walk(v: &Value, path: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(map) if !map.is_empty() => {
                for (k, child) in map {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    walk(child, &p, out);
                }
            }
            Value::Array(arr) if !arr.is_empty() => {
                for child in arr {
                    walk(child, path, out);
                }
            }
            _ => out.push(path.to_string()),
        }
    }
    let mut out = Vec::new();
    walk(body, "", &mut out);
    out
}

/// The values a dotted path resolves to (arrays walked element by element).
fn resolve<'a>(body: &'a Value, parts: &[&str], out: &mut Vec<&'a Value>) {
    if parts.is_empty() {
        out.push(body);
        return;
    }
    match body {
        Value::Object(map) => {
            if let Some(child) = map.get(parts[0]) {
                resolve(child, &parts[1..], out);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                resolve(child, parts, out);
            }
        }
        _ => {}
    }
}

/// Why a fixture's `volatile_paths` are not leaf volatility over `body`; empty
/// when they are. The replay test `every_fixture_declares_leaf_volatility`
/// fails on any non-empty answer.
pub fn audit_volatile_paths(body: &Value, volatile_paths: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let leaves = leaf_paths(body);
    let leaf_set: BTreeSet<&str> = leaves.iter().map(String::as_str).collect();
    let mut covered = 0usize;
    for vp in volatile_paths {
        if vp.is_empty() || vp == "[]" && !body.is_array() {
            problems.push(format!("`{vp}` names the whole body"));
            continue;
        }
        if vp.starts_with("response.") || vp.starts_with("body_json") || vp.starts_with("request.")
        {
            problems.push(format!(
                "`{vp}` names the fixture envelope, not a leaf of the body"
            ));
            continue;
        }
        if is_order_path(vp) {
            let base = &vp[..vp.len() - 2];
            let parts: Vec<&str> = if base.is_empty() {
                Vec::new()
            } else {
                base.split('.').collect()
            };
            let mut found = Vec::new();
            resolve(body, &parts, &mut found);
            if found.iter().any(|v| !v.is_array()) {
                problems.push(format!(
                    "`{vp}` declares the order of something that is not an array"
                ));
            }
            continue;
        }
        let parts: Vec<&str> = vp.split('.').collect();
        let mut found = Vec::new();
        resolve(body, &parts, &mut found);
        if let Some(container) = found.iter().find(|v| {
            matches!(v, Value::Object(m) if !m.is_empty())
                || matches!(v, Value::Array(a) if !a.is_empty())
        }) {
            let kind = if container.is_array() {
                "array"
            } else {
                "object"
            };
            problems.push(format!(
                "`{vp}` covers a whole {kind} ({} leaves under it)",
                leaf_paths(container).len()
            ));
            continue;
        }
        if leaf_set.contains(vp.as_str()) {
            covered += leaves.iter().filter(|l| l.as_str() == vp.as_str()).count();
        }
    }
    if !leaves.is_empty() && covered * 2 > leaves.len() {
        problems.push(format!(
            "volatile paths cover {covered} of the body's {} leaves (over half)",
            leaves.len()
        ));
    }
    problems
}

#[derive(Debug, Default, Clone)]
pub struct SectionRules {
    pub ignore: Vec<String>,
    pub ignore_whole_response: bool,
}

#[derive(Debug, Default, Clone)]
pub struct Volatility {
    sections: HashMap<String, SectionRules>,
}

impl Volatility {
    #[allow(dead_code)]
    pub fn load_or_default(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => match parse_toml(&contents) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "  ~ volatility.toml parse error ({}), using built-in defaults",
                        e
                    );
                    Self::defaults()
                }
            },
            Err(_) => Self::defaults(),
        }
    }

    pub fn defaults() -> Self {
        let mut sections = HashMap::new();
        sections.insert(
            "about".to_string(),
            SectionRules {
                ignore: vec![
                    "configurations.realmName".to_string(),
                    "configurations.networkId".to_string(),
                    "content.publicUrl".to_string(),
                    "lambdas.publicUrl".to_string(),
                    "comms.publicUrl".to_string(),
                    "comms.adapter".to_string(),
                    "bff.publicUrl".to_string(),
                    "comms".to_string(),
                ],
                ignore_whole_response: false,
            },
        );
        sections.insert(
            "status".to_string(),
            SectionRules {
                ignore: vec![
                    "lastSyncWithDAO".to_string(),
                    "lastDeploymentAt".to_string(),
                    "synchronizationStatus.lastSyncWithOtherServers[].lastSyncTimestamp"
                        .to_string(),
                    "synchronizationStatus.lastSyncWithOtherServers[].lastDeploymentTimestamp"
                        .to_string(),
                    "synchronizationStatus.lastSyncWithOtherServers[].url".to_string(),
                    "synchronizationStatus.otherServers[]".to_string(),
                    "synchronizationStatus.lastHeartbeat".to_string(),
                    "synchronizationStatus.syncFrontier".to_string(),
                    "synchronizationStatus.up".to_string(),
                ],
                ignore_whole_response: false,
            },
        );
        sections.insert(
            "challenge".to_string(),
            SectionRules {
                ignore: vec!["challengeText".to_string()],
                ignore_whole_response: false,
            },
        );
        sections.insert(
            "audit".to_string(),
            SectionRules {
                ignore: vec!["localTimestamp".to_string()],
                ignore_whole_response: false,
            },
        );
        sections.insert(
            "snapshots".to_string(),
            SectionRules {
                ignore: vec![],
                ignore_whole_response: true,
            },
        );
        sections.insert(
            "failed-deployments".to_string(),
            SectionRules {
                ignore: vec![],
                ignore_whole_response: true,
            },
        );
        sections.insert(
            "deployments".to_string(),
            SectionRules {
                ignore: vec![
                    "deployments[].localTimestamp".to_string(),
                    "pagination.next".to_string(),
                    "pagination.self".to_string(),
                ],
                ignore_whole_response: false,
            },
        );
        sections.insert(
            "lambdas-status".to_string(),
            SectionRules {
                ignore: vec![
                    "version".to_string(),
                    "currentTime".to_string(),
                    "commitHash".to_string(),
                ],
                ignore_whole_response: false,
            },
        );
        Self { sections }
    }

    pub fn is_ignored(&self, section: &str, json_path: &str) -> bool {
        let Some(rules) = self.sections.get(section) else {
            return false;
        };
        rules.ignore.iter().any(|pat| glob_match(pat, json_path))
    }

    pub fn ignore_whole(&self, section: &str) -> bool {
        self.sections
            .get(section)
            .map(|r| r.ignore_whole_response)
            .unwrap_or(false)
    }
}

fn glob_match(pattern: &str, path: &str) -> bool {
    if matches_inner(pattern, path) {
        return true;
    }
    if let Some(stripped) = path.split_once('.').map(|(_, rest)| rest)
        && matches_inner(pattern, stripped)
    {
        return true;
    }
    false
}

fn matches_inner(pattern: &str, path: &str) -> bool {
    let p_segs = tokenize(pattern);
    let v_segs = tokenize(path);
    if p_segs.len() != v_segs.len() {
        return false;
    }
    for (ps, vs) in p_segs.iter().zip(v_segs.iter()) {
        if !segment_match(ps, vs) {
            return false;
        }
    }
    true
}

fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == '.' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn segment_match(pattern: &str, segment: &str) -> bool {
    let (p_name, p_idx) = split_bracket(pattern);
    let (v_name, v_idx) = split_bracket(segment);

    let name_ok = p_name == "*" || p_name == v_name;
    if !name_ok {
        return false;
    }
    match (p_idx, v_idx) {
        (None, None) => true,
        (Some(""), Some(_)) => true,
        (Some(p), Some(v)) => p == v,
        _ => false,
    }
}

fn split_bracket(s: &str) -> (&str, Option<&str>) {
    if let Some(open) = s.find('[')
        && let Some(close_rel) = s[open..].find(']')
    {
        let close = open + close_rel;
        let name = &s[..open];
        let idx = &s[open + 1..close];
        return (name, Some(idx));
    }
    (s, None)
}

fn parse_toml(input: &str) -> Result<Volatility, String> {
    let mut sections: HashMap<String, SectionRules> = HashMap::new();
    let mut current: Option<String> = None;

    let logical_lines = fold_arrays(input);

    for (lineno, line) in logical_lines {
        let line = strip_comment(&line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') && !line.contains('=') {
            let name = line[1..line.len() - 1].trim().to_string();
            sections.entry(name.clone()).or_default();
            current = Some(name);
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {}: expected key=value", lineno));
        };
        let key = key.trim();
        let value = value.trim();

        let section = current
            .as_ref()
            .ok_or_else(|| format!("line {}: key outside any [section]", lineno))?;
        let rules = sections.entry(section.clone()).or_default();

        match key {
            "ignore" => {
                rules.ignore =
                    parse_string_array(value).map_err(|e| format!("line {}: {}", lineno, e))?;
            }
            "ignore_whole_response" => {
                rules.ignore_whole_response = value == "true";
            }
            other => {
                return Err(format!("line {}: unknown key '{}'", lineno, other));
            }
        }
    }
    Ok(Volatility { sections })
}

fn fold_arrays(input: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut buf = String::new();
    let mut buf_start_lineno = 0usize;
    let mut depth = 0i32;

    for (idx, raw) in input.lines().enumerate() {
        let lineno = idx + 1;
        if depth == 0 {
            buf.clear();
            buf.push_str(raw);
            buf_start_lineno = lineno;
        } else {
            buf.push(' ');
            buf.push_str(raw);
        }

        let stripped = strip_comment(raw);
        let mut in_str = false;
        for ch in stripped.chars() {
            match ch {
                '"' => in_str = !in_str,
                '[' if !in_str => depth += 1,
                ']' if !in_str => depth -= 1,
                _ => {}
            }
        }

        if depth <= 0 {
            depth = 0;
            out.push((buf_start_lineno, std::mem::take(&mut buf)));
        }
    }
    if !buf.is_empty() {
        out.push((buf_start_lineno, buf));
    }
    out
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn parse_string_array(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(format!("expected array, got `{}`", value));
    }
    let inner = &value[1..value.len() - 1];
    let mut out = Vec::new();
    let mut in_str = false;
    let mut cur = String::new();
    for ch in inner.chars() {
        match ch {
            '"' => {
                if in_str {
                    out.push(std::mem::take(&mut cur));
                    in_str = false;
                } else {
                    in_str = true;
                }
            }
            _ if in_str => cur.push(ch),
            ',' | ' ' | '\t' | '\n' => {}
            other => return Err(format!("unexpected char '{}' in array", other)),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_array_wildcard() {
        assert!(glob_match(
            "deployments[].localTimestamp",
            "deployments[3].localTimestamp"
        ));
        assert!(!glob_match(
            "deployments[].localTimestamp",
            "deployments[3].entityId"
        ));
    }

    #[test]
    fn glob_star_segment() {
        assert!(glob_match("configurations.*", "configurations.realmName"));
        assert!(!glob_match("configurations.*", "configurations.a.b"));
    }

    #[test]
    fn glob_with_section_prefix() {
        assert!(glob_match(
            "configurations.realmName",
            "configurations.realmName"
        ));
        assert!(glob_match(
            "configurations.realmName",
            "/about.configurations.realmName"
        ));
    }

    #[test]
    fn glob_top_level_only_patterns() {
        assert!(glob_match(
            "localTimestamp",
            "/audit/scene/bafkreieo2bzxurwq2zyu5wbhr2c6x7mez2tjcksphjazvqmhhuc7lr2jem.localTimestamp"
        ));
        assert!(!glob_match(
            "localTimestamp",
            "/audit/scene/x.foo.localTimestamp"
        ));
        assert!(glob_match("comms", "/about.comms"));
        assert!(glob_match(
            "synchronizationStatus.up",
            "/status.synchronizationStatus.up"
        ));
    }

    #[test]
    fn defaults_have_snapshots_whole() {
        let v = Volatility::defaults();
        assert!(v.ignore_whole("snapshots"));
        assert!(v.ignore_whole("failed-deployments"));
        assert!(!v.ignore_whole("about"));
    }

    #[test]
    fn shipped_toml_parses() {
        let src = include_str!("volatility.toml");
        let v = parse_toml(src).unwrap();
        assert!(v.is_ignored("about", "/about.comms"));
        assert!(v.is_ignored("status", "/status.synchronizationStatus.up"));
        assert!(v.is_ignored("audit", "/audit/scene/abc.localTimestamp"));
        assert!(v.ignore_whole("snapshots"));
    }

    #[test]
    fn leaf_paths_collapse_indices_and_count_empty_containers() {
        let body = serde_json::json!({"ok": true, "data": [{"id": 1, "tags": []}, {"id": 2, "tags": ["a", "b"]}], "meta": {}});
        let mut paths = leaf_paths(&body);
        paths.sort();
        assert_eq!(
            paths,
            vec![
                "data.id",
                "data.id",
                "data.tags",
                "data.tags",
                "data.tags",
                "meta",
                "ok"
            ]
        );
        assert_eq!(leaf_paths(&serde_json::json!([])), vec![""]);
    }

    #[test]
    fn audit_accepts_leaves_and_order_only() {
        let body = serde_json::json!({"ok": true, "total": 2, "data": [{"id": "a", "updated_at": 1, "name": "x"}, {"id": "b", "updated_at": 2, "name": "y"}]});
        let ok = |paths: &[&str]| {
            audit_volatile_paths(
                &body,
                &paths.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            )
        };
        assert!(ok(&["total", "data.updated_at", "data[]"]).is_empty());
        assert!(
            ok(&["data"])[0].contains("whole array"),
            "{:?}",
            ok(&["data"])
        );
        assert!(ok(&["data.id", "data.updated_at", "data.name"])[0].contains("over half"));
        assert!(ok(&[""])[0].contains("whole body"));
        assert!(ok(&["response.body_json.total"])[0].contains("envelope"));
        assert!(ok(&["total[]"])[0].contains("not an array"));
        assert!(
            ok(&["absent.key"]).is_empty(),
            "a key only the candidate emits is allowed"
        );
        let root_array =
            serde_json::json!([{"baseUrl": "a", "owner": "b"}, {"baseUrl": "c", "owner": "d"}]);
        assert!(audit_volatile_paths(&root_array, &["[]".to_string()]).is_empty());
        assert!(
            audit_volatile_paths(&root_array, &["baseUrl".to_string(), "owner".to_string()])[0]
                .contains("over half")
        );
        let one_leaf = serde_json::json!({"lastUpdatedAt": 1});
        assert!(
            audit_volatile_paths(&one_leaf, &["lastUpdatedAt".to_string()])[0]
                .contains("over half")
        );
        assert!(
            audit_volatile_paths(
                &serde_json::json!({"data": [], "total": 0}),
                &["total".to_string()]
            )
            .is_empty(),
            "exactly half is allowed"
        );
    }

    #[test]
    fn parse_small_toml() {
        let src = r#"
            [about]
            ignore = ["configurations.realmName", "content.publicUrl"]

            [snapshots]
            ignore_whole_response = true
        "#;
        let v = parse_toml(src).unwrap();
        assert!(v.is_ignored("about", "configurations.realmName"));
        assert!(v.ignore_whole("snapshots"));
    }
}

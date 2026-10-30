//! Structural JSON comparison after the volatility rules: object keys in
//! sorted order, arrays element by element (a length mismatch is one
//! difference, then the common prefix is walked), numbers that both look like
//! millisecond timestamps within one second equal, the leaf names in
//! IGNORED_FIELDS skipped at every depth, and a baseline leaf redacted at
//! capture (`<<REDACTED>>`, `<<REDACTED:jwt>>`, ...) or recorded as the
//! volatile sentinel (`<<VOLATILE>>`) a mask that matches any candidate leaf
//! (the key and, in a list, the length still count).

use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

use super::fixture::VOLATILE_SENTINEL;
use super::volatility::Volatility;

#[derive(Debug, Clone)]
pub struct Difference {
    pub path: String,
    pub baseline_value: String,
    pub candidate_value: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: baseline={} candidate={}",
            self.path, self.baseline_value, self.candidate_value
        )
    }
}

pub const IGNORED_FIELDS: &[&str] = &[
    "currentTime",
    "commitHash",
    "version",
    "realmName",
    "publicUrl",
    "url",
    "challengeText",
    "lastSyncWithDAO",
    "synchronizationTime",
    "generationTimestamp",
    "id",
    "scope",
    "next",
    "self",
    "previous",
];

const TIMESTAMP_TOLERANCE_MS: f64 = 1000.0;
pub const REDACTION_MARK: &str = "<<REDACTED";

fn is_masked(v: &Value) -> bool {
    matches!(v, Value::String(s) if s.contains(REDACTION_MARK) || s == VOLATILE_SENTINEL)
}

pub fn compare_json(
    section: &str,
    name: &str,
    baseline_val: &Value,
    candidate_val: &Value,
    volatility: &Volatility,
) -> Vec<Difference> {
    let mut diffs = Vec::new();
    compare_recursive(
        section,
        name,
        baseline_val,
        candidate_val,
        &mut diffs,
        volatility,
    );
    diffs
}

fn is_globally_ignored(path: &str) -> bool {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    let leaf_clean = leaf.split('[').next().unwrap_or(leaf);
    IGNORED_FIELDS.contains(&leaf_clean)
}

fn is_ignored_for_section(section: &str, path: &str, volatility: &Volatility) -> bool {
    if is_globally_ignored(path) {
        return true;
    }
    volatility.is_ignored(section, path)
}

fn looks_like_timestamp(n: f64) -> bool {
    n > 1_000_000_000_000.0 && n < 3_000_000_000_000.0
}

fn compare_recursive(
    section: &str,
    path: &str,
    baseline_val: &Value,
    candidate_val: &Value,
    diffs: &mut Vec<Difference>,
    volatility: &Volatility,
) {
    if is_ignored_for_section(section, path, volatility) || is_masked(baseline_val) {
        return;
    }

    match (baseline_val, candidate_val) {
        (Value::Object(b_map), Value::Object(c_map)) => {
            let all_keys: HashSet<&String> = b_map.keys().chain(c_map.keys()).collect();
            let mut keys: Vec<&&String> = all_keys.iter().collect();
            keys.sort();
            for key in keys {
                let child_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };
                match (b_map.get(*key), c_map.get(*key)) {
                    (Some(bv), Some(cv)) => {
                        compare_recursive(section, &child_path, bv, cv, diffs, volatility);
                    }
                    (Some(bv), None) => {
                        if !is_ignored_for_section(section, &child_path, volatility) {
                            diffs.push(Difference {
                                path: child_path,
                                baseline_value: truncate_value(bv),
                                candidate_value: "missing".to_string(),
                            });
                        }
                    }
                    (None, Some(cv)) => {
                        if !is_ignored_for_section(section, &child_path, volatility) {
                            diffs.push(Difference {
                                path: child_path,
                                baseline_value: "missing".to_string(),
                                candidate_value: truncate_value(cv),
                            });
                        }
                    }
                    (None, None) => unreachable!(),
                }
            }
        }
        (Value::Array(b_arr), Value::Array(c_arr)) => {
            if b_arr.len() != c_arr.len() {
                diffs.push(Difference {
                    path: format!("{}.length", path),
                    baseline_value: b_arr.len().to_string(),
                    candidate_value: c_arr.len().to_string(),
                });
            }
            let len = b_arr.len().min(c_arr.len());
            for i in 0..len {
                let child_path = format!("{}[{}]", path, i);
                compare_recursive(
                    section,
                    &child_path,
                    &b_arr[i],
                    &c_arr[i],
                    diffs,
                    volatility,
                );
            }
        }
        (Value::Number(b_n), Value::Number(c_n)) => {
            let b_f = b_n.as_f64().unwrap_or(0.0);
            let c_f = c_n.as_f64().unwrap_or(0.0);
            if looks_like_timestamp(b_f)
                && looks_like_timestamp(c_f)
                && (b_f - c_f).abs() <= TIMESTAMP_TOLERANCE_MS
            {
                return;
            }
            if b_f != c_f {
                diffs.push(Difference {
                    path: path.to_string(),
                    baseline_value: baseline_val.to_string(),
                    candidate_value: candidate_val.to_string(),
                });
            }
        }
        _ => {
            if baseline_val != candidate_val {
                diffs.push(Difference {
                    path: path.to_string(),
                    baseline_value: truncate_value(baseline_val),
                    candidate_value: truncate_value(candidate_val),
                });
            }
        }
    }
}

fn truncate_value(val: &Value) -> String {
    let s = val.to_string();
    if s.len() > 120 {
        let mut cut = 117;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}...", &s[..cut])
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn additive_and_missing_keys_are_one_difference_each() {
        let diffs = compare_json(
            "s",
            "fixture",
            &json!({"a": 1, "b": 2}),
            &json!({"a": 1, "c": 3}),
            &Volatility::default(),
        );
        let lines: Vec<String> = diffs.iter().map(|d| d.to_string()).collect();
        assert_eq!(
            lines,
            vec![
                "fixture.b: baseline=2 candidate=missing",
                "fixture.c: baseline=missing candidate=3"
            ]
        );
    }

    #[test]
    fn array_length_then_common_prefix() {
        let diffs = compare_json(
            "s",
            "f",
            &json!([1, 2, 3]),
            &json!([1, 9]),
            &Volatility::default(),
        );
        let lines: Vec<String> = diffs.iter().map(|d| d.to_string()).collect();
        assert_eq!(
            lines,
            vec![
                "f.length: baseline=3 candidate=2",
                "f[1]: baseline=2 candidate=9"
            ]
        );
    }

    #[test]
    fn a_redacted_baseline_leaf_matches_anything_but_keeps_the_shape() {
        let baseline = json!({"email": "<<REDACTED>>", "blocked": ["<<REDACTED>>"], "adapter": "livekit:wss://x?access_token=<<REDACTED:jwt>>"});
        let same_shape = json!({"email": "a@b.c", "blocked": ["0xabc"], "adapter": "livekit:wss://y?access_token=eyJ"});
        assert!(compare_json("s", "f", &baseline, &same_shape, &Volatility::default()).is_empty());
        let longer = json!({"email": null, "blocked": ["0xabc", "0xdef"], "adapter": "x"});
        let lines: Vec<String> = compare_json("s", "f", &baseline, &longer, &Volatility::default())
            .iter()
            .map(|d| d.to_string())
            .collect();
        assert_eq!(lines, vec!["f.blocked.length: baseline=1 candidate=2"]);
        let missing = json!({"blocked": ["0xabc"], "adapter": "x"});
        assert_eq!(
            compare_json("s", "f", &baseline, &missing, &Volatility::default()).len(),
            1,
            "a missing key is still a difference"
        );
    }

    #[test]
    fn a_baseline_leaf_recorded_as_the_volatile_sentinel_is_a_mask() {
        let baseline = json!({"acceptingUsers": "<<VOLATILE>>", "comms": {"protocol": "<<VOLATILE>>", "healthy": true}});
        assert!(
            compare_json(
                "s",
                "f",
                &baseline,
                &json!({"acceptingUsers": true, "comms": {"protocol": "v3", "healthy": true}}),
                &Volatility::default()
            )
            .is_empty()
        );
        let lines: Vec<String> = compare_json(
            "s",
            "f",
            &baseline,
            &json!({"acceptingUsers": true, "comms": {"healthy": false}}),
            &Volatility::default(),
        )
        .iter()
        .map(|d| d.to_string())
        .collect();
        assert_eq!(
            lines,
            vec![
                "f.comms.healthy: baseline=true candidate=false",
                "f.comms.protocol: baseline=\"<<VOLATILE>>\" candidate=missing"
            ]
        );
    }

    #[test]
    fn globally_ignored_leaves_and_close_timestamps_are_equal() {
        let diffs = compare_json(
            "s",
            "f",
            &json!({"id": "a", "items": [{"url": "x", "at": 1790000000000_i64}]}),
            &json!({"id": "b", "items": [{"url": "y", "at": 1790000000900_i64}]}),
            &Volatility::default(),
        );
        assert!(diffs.is_empty(), "{diffs:?}");
        let diffs = compare_json(
            "s",
            "f",
            &json!({"at": 1790000000000_i64}),
            &json!({"at": 1790000002000_i64}),
            &Volatility::default(),
        );
        assert_eq!(diffs.len(), 1);
    }
}

//! Local fixture support; no sibling checkout or runtime oracle dependency.
use serde_json::Value;

pub fn manifest() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/pulse_baseline_manifest.json"
    )))
    .unwrap()
}

pub fn cases() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/pulse_baseline_traces.json"
    )))
    .unwrap()
}

pub fn load_case(case_id: &str) -> Value {
    cases()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_id"] == case_id)
        .unwrap_or_else(|| panic!("missing Pulse case {case_id}"))
        .clone()
}

pub fn assert_trace(case_id: &str, expected: &Value, actual: &Value) {
    assert_eq!(expected, actual, "Pulse case {case_id}");
}

pub fn bytes(case: &Value) -> Vec<u8> {
    hex::decode(case["inputs"][0]["raw_hex"].as_str().unwrap()).unwrap()
}

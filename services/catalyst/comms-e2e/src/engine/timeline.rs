//! One merged, timestamped record of what every actor of a scenario sent, saw and did. A failed
//! scenario prints it; nothing else explains a race.

use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// Milliseconds since the scenario started.
    pub at_ms: u64,
    pub actor: String,
    pub kind: String,
    pub detail: serde_json::Value,
}

#[derive(Clone)]
pub struct Timeline {
    started: Instant,
    entries: Arc<Mutex<Vec<Entry>>>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn record(&self, actor: &str, kind: &str, detail: impl Serialize) {
        let detail = serde_json::to_value(detail).unwrap_or(serde_json::Value::Null);
        self.entries.lock().push(Entry {
            at_ms: self.started.elapsed().as_millis() as u64,
            actor: actor.to_owned(),
            kind: kind.to_owned(),
            detail: redact(detail),
        });
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    pub fn snapshot(&self) -> Vec<Entry> {
        self.entries.lock().clone()
    }

    /// How many entries of `kind` `actor` has recorded.
    pub fn count(&self, actor: &str, kind: &str) -> usize {
        self.entries
            .lock()
            .iter()
            .filter(|entry| entry.actor == actor && entry.kind == kind)
            .count()
    }
}

/// Text with every access token cut out: a connection string carries one, and neither a log nor a
/// report may keep it.
pub fn redact_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find("access_token=") {
        let (before, after) = rest.split_at(position + "access_token=".len());
        out.push_str(before);
        out.push_str("<redacted>");
        let end = after
            .find(|c: char| c == '&' || c == '"' || c.is_whitespace())
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn redact(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(redact_text(&text)),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(redact).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, redact(value)))
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_keep_their_order_and_actor() {
        let timeline = Timeline::new();
        timeline.record("a", "sent", "one");
        timeline.record("b", "seen", serde_json::json!({ "n": 2 }));
        let entries = timeline.snapshot();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            (entries[0].actor.as_str(), entries[0].kind.as_str()),
            ("a", "sent")
        );
        assert_eq!(entries[1].detail["n"], 2);
        assert_eq!(timeline.count("a", "sent"), 1);
        assert_eq!(timeline.count("a", "seen"), 0);
    }

    #[test]
    fn a_connection_string_loses_its_token() {
        let timeline = Timeline::new();
        timeline.record(
            "bot",
            "assignment",
            serde_json::json!({ "to": "livekit:wss://host/rtc?access_token=abc.def.ghi&x=1" }),
        );
        let text = serde_json::to_string(&timeline.snapshot()).unwrap();
        assert!(!text.contains("abc.def.ghi"), "{text}");
        assert!(text.contains("access_token=<redacted>&x=1"), "{text}");
        assert_eq!(
            redact_text("a access_token=tok b"),
            "a access_token=<redacted> b"
        );
        assert_eq!(redact_text("nothing here"), "nothing here");
    }
}

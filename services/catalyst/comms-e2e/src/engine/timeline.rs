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

const REDACTED: &str = "<redacted>";

/// Text with every secret it can carry cut out: the `access_token=` of a connection string, the
/// value of an `Authorization` header, a `Bearer` token and any bare JWT. Neither a log, a timeline
/// nor a report may keep them. Addresses, hashes and names stay: failure messages need them.
pub fn redact_text(text: &str) -> String {
    let text = redact_after(text, "access_token=", |c| {
        c == '&' || c == '"' || c.is_whitespace()
    });
    let text = redact_authorization(&text);
    let text = redact_after_word(&text, "bearer ", |c| {
        c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';')
    });
    redact_jwts(&text)
}

/// Replaces what follows each `marker` (ASCII, matched as written) up to the first character
/// `ends` accepts.
fn redact_after(text: &str, marker: &str, ends: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find(marker) {
        let (before, after) = rest.split_at(position + marker.len());
        out.push_str(before);
        out.push_str(REDACTED);
        let end = after.find(&ends).unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// [`redact_after`] with the marker matched without regard to ASCII case.
fn redact_after_word(text: &str, marker: &str, ends: impl Fn(char) -> bool) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    while let Some(found) = lower[from..].find(marker) {
        let start = from + found + marker.len();
        out.push_str(&text[from..start]);
        out.push_str(REDACTED);
        let end = text[start..]
            .find(&ends)
            .map_or(text.len(), |end| start + end);
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// The value of `Authorization: ...`, `authorization=...` and `"authorization": "..."`, whatever
/// its scheme. Prose that merely says "authorization" has no `:` or `=` after it and is kept.
fn redact_authorization(text: &str) -> String {
    const WORD: &str = "authorization";
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    while let Some(found) = lower[from..].find(WORD) {
        let word_end = from + found + WORD.len();
        let separator_end = word_end
            + text[word_end..]
                .find(|c: char| !matches!(c, '"' | '\'' | ' ' | ':' | '='))
                .unwrap_or(text.len() - word_end);
        let separated = text[word_end..separator_end].contains([':', '=']);
        if !separated || separator_end == text.len() {
            out.push_str(&text[from..word_end]);
            from = word_end;
            continue;
        }
        out.push_str(&text[from..separator_end]);
        out.push_str(REDACTED);
        let quoted = text[word_end..separator_end].contains(['"', '\'']);
        let value = &text[separator_end..];
        let token_end = |value: &str| {
            value
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ','))
                .unwrap_or(value.len())
        };
        let length = if quoted {
            value.find(['"', '\'', '\r', '\n']).unwrap_or(value.len())
        } else {
            let first = token_end(value);
            let scheme = value[..first].to_ascii_lowercase();
            let two_parts = ["bearer", "basic", "digest", "token", "signature"]
                .contains(&scheme.as_str())
                && value[first..].starts_with(' ');
            if two_parts {
                first + 1 + token_end(&value[first + 1..])
            } else {
                first
            }
        };
        from = separator_end + length;
    }
    out.push_str(&text[from..]);
    out
}

fn is_base64url(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Three base64url parts joined by dots, the first starting with `eyJ` (a JSON object, as every
/// JWT header is). A dotted name that does not start so is not touched.
fn redact_jwts(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    let mut search = 0;
    while let Some(found) = text[search..].find("eyJ") {
        let start = search + found;
        let at_word_start = start == 0 || !is_base64url(bytes[start - 1]);
        let mut end = start;
        let mut parts = 0;
        if at_word_start {
            loop {
                let part_start = end;
                while end < bytes.len() && is_base64url(bytes[end]) {
                    end += 1;
                }
                if end == part_start {
                    break;
                }
                parts += 1;
                if parts < 3 && end < bytes.len() && bytes[end] == b'.' {
                    end += 1;
                } else {
                    break;
                }
            }
        }
        if at_word_start && parts == 3 {
            out.push_str(&text[from..start]);
            out.push_str(REDACTED);
            from = end;
            search = end;
        } else {
            search = start + 3;
        }
    }
    out.push_str(&text[from..]);
    out
}

/// A value with every string in it passed through [`redact_text`].
pub(super) fn redact(value: serde_json::Value) -> serde_json::Value {
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

    const JWT: &str = "eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJ4In0.c2lnbmF0dXJl";

    #[test]
    fn authorization_values_and_bearer_tokens_are_cut() {
        assert_eq!(
            redact_text("sent Authorization: Bearer abc123 to the server"),
            "sent Authorization: <redacted> to the server"
        );
        assert_eq!(
            redact_text("{\"authorization\": \"Basic dXNlcjpwdw==\", \"x\": 1}"),
            "{\"authorization\": \"<redacted>\", \"x\": 1}"
        );
        assert_eq!(
            redact_text("header authorization=Signed-abc\nnext line"),
            "header authorization=<redacted>\nnext line"
        );
        assert_eq!(
            redact_text("got bearer tok.en/+= and went on"),
            "got bearer <redacted> and went on"
        );
        // the word alone is prose
        assert_eq!(
            redact_text("authorization failed for the wallet"),
            "authorization failed for the wallet"
        );
    }

    #[test]
    fn a_bare_jwt_is_cut_and_its_neighbours_stay() {
        assert_eq!(
            redact_text(&format!("token {JWT}, then \"{JWT}\".")),
            "token <redacted>, then \"<redacted>\"."
        );
        // two parts, or a first part that is not a JSON header, are something else
        assert_eq!(redact_text("eyJabc.def and eyJ"), "eyJabc.def and eyJ");
        assert_eq!(redact_text("aeyJabc.def.ghi"), "aeyJabc.def.ghi");
    }

    #[test]
    fn what_scenarios_print_stays_as_it_is() {
        for text in [
            "0x1234567890abcdef1234567890abcdef12345678",
            "hash 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            "a.b.c",
            "livekit:wss://host.example/rtc?room=e2e-1&x=1",
            "peer 0xAbC joined realm e2e-run-1-connect-0-a at 127.0.0.1:5217",
        ] {
            assert_eq!(redact_text(text), text);
        }
    }
}

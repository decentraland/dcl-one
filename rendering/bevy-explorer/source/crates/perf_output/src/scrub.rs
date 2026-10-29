//! Credentials never reach the benchmark log. The log is a run's artifact: the harness keeps it
//! and the web page posts it to the serving origin, so whatever an event's text holds is read by
//! whoever reads the artifact. [`credentials`] replaces the value of every credential the engine's
//! strings are known to carry, in a message or a category, before the layer writes it:
//!
//! - an `access_token` query parameter, the room credential of a comms adapter string
//!   (`livekit:wss://host/room?access_token=<token>`);
//! - a string field named `access_token`, `accessToken`, `privateKey`, `private_key` or
//!   `signature`, as JSON prints it (`"privateKey":"…"`), as `Debug` prints it inside another
//!   string (`\"privateKey\":\"…\"`) or a struct (`signature: "…"`), or as an event field
//!   (`signature="…"`): an auth chain's signatures and an identity's ephemeral key;
//! - a base64 blob that decodes to text naming a private key, which is how an `AuthIdentity`
//!   travels in the `/login_identity` command's arguments.
//!
//! What stays is the shape of the text, so a line still says what happened. A value is replaced
//! by [`REDACTED`]; an empty one has nothing to hide and stays.

use std::borrow::Cow;

use base64::{
    alphabet,
    engine::{general_purpose::GeneralPurpose, general_purpose::NO_PAD, DecodePaddingMode},
    Engine as _,
};

/// What replaces a credential's value.
pub const REDACTED: &str = "REDACTED";

/// The names whose string value is a credential, in lower case: names are matched in any case.
const FIELDS: [&str; 5] = [
    "access_token",
    "accesstoken",
    "privatekey",
    "private_key",
    "signature",
];

/// The names that, unlike the others, also carry their value bare in a URL's query:
/// `access_token=<value>`, which ends at the next character [`ends_bare_value`] accepts.
const QUERY_FIELDS: [&str; 2] = ["access_token", "accesstoken"];

/// A base64 run shorter than this cannot hold the text of an identity.
const MIN_BLOB: usize = 24;

/// `text` with the value of every credential replaced; borrowed when it holds none.
pub fn credentials(text: &str) -> Cow<'_, str> {
    let mut current = Cow::Borrowed(text);
    if let Some(changed) = redact_fields(&current) {
        current = Cow::Owned(changed);
    }
    if let Some(changed) = redact_identities(&current) {
        current = Cow::Owned(changed);
    }
    current
}

/// A character that ends a value written bare: the next query parameter or fragment, whitespace,
/// and the quotes and brackets a log line wraps a string in.
fn ends_bare_value(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '&' | '#' | '"' | '\'' | '`' | '\\' | '<' | '>' | ')' | ']' | '}' | ',' | ';'
        )
}

fn redact_fields(text: &str) -> Option<String> {
    // ASCII lowering keeps every byte offset of `text`
    let lowered = text.to_ascii_lowercase();
    let mut out: Option<String> = None;
    // `text[..copied]` is already in `out`
    let mut copied = 0;
    let mut from = 0;
    while let Some((at, name)) = next_name(&lowered, from) {
        let name_end = at + name.len();
        from = name_end;
        let Some((start, end)) = value_span(text, name, at, name_end) else {
            continue;
        };
        if start == end {
            continue;
        }
        let out = out.get_or_insert_with(|| String::with_capacity(text.len()));
        out.push_str(&text[copied..start]);
        out.push_str(REDACTED);
        copied = end;
        from = end;
    }
    out.map(|mut out| {
        out.push_str(&text[copied..]);
        out
    })
}

/// The first of [`FIELDS`] at or after `from`.
fn next_name(lowered: &str, from: usize) -> Option<(usize, &'static str)> {
    FIELDS
        .iter()
        .filter_map(|name| lowered[from..].find(name).map(|at| (from + at, *name)))
        .min_by_key(|(at, _)| *at)
}

/// The byte range of the value that follows the name found at `at..name_end`, if one does: an
/// optional closing quote of the name, `:` or `=`, then a quoted string, or for the query names
/// after `=` a bare value.
fn value_span(text: &str, name: &str, at: usize, name_end: usize) -> Option<(usize, usize)> {
    let quote = quote_before(&text[..at]);
    let mut rest = &text[name_end..];
    if !quote.is_empty() {
        rest = rest.strip_prefix(quote)?;
    }
    let rest = rest.trim_start();
    let (separator, rest) = match rest.chars().next()? {
        c @ (':' | '=') => (c, rest[1..].trim_start()),
        _ => return None,
    };
    let value_start = text.len() - rest.len();

    if let Some(open) = opening_quote(rest) {
        let start = value_start + open.len();
        let end = if open == "\"" {
            start + unescaped_quote(&text[start..])
        } else {
            text[start..]
                .find(open)
                .map_or(text.len(), |close| start + close)
        };
        return Some((start, end));
    }
    if separator == '=' && QUERY_FIELDS.contains(&name) {
        let end = rest
            .find(ends_bare_value)
            .map_or(text.len(), |end| value_start + end);
        return Some((value_start, end));
    }
    None
}

/// The quote that closes a name written in quotes: `"`, or the escaped `\"`, `\\\"` of a string
/// that `Debug` printed once, twice. Empty for a name written bare.
fn quote_before(before: &str) -> &str {
    let Some(without_quote) = before.strip_suffix('"') else {
        return "";
    };
    let backslashes = without_quote.len() - without_quote.trim_end_matches('\\').len();
    &before[before.len() - 1 - backslashes..]
}

/// The quote that opens the string `rest` starts with, written as [`quote_before`] reads one.
fn opening_quote(rest: &str) -> Option<&str> {
    let backslashes = rest.len() - rest.trim_start_matches('\\').len();
    rest[backslashes..]
        .starts_with('"')
        .then(|| &rest[..backslashes + 1])
}

/// Where a JSON string that starts at the beginning of `text` ends: its first quote that no
/// backslash escapes; the end of `text` when it never closes.
fn unescaped_quote(text: &str) -> usize {
    let mut escaped = false;
    for (at, c) in text.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return at,
            _ => {}
        }
    }
    text.len()
}

/// Base64 blobs whose decoded text names a private key.
fn redact_identities(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut at = 0;
    while at < bytes.len() {
        if !is_base64(bytes[at]) {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && is_base64(bytes[at]) {
            at += 1;
        }
        let padding = bytes[at..]
            .iter()
            .take(2)
            .take_while(|b| **b == b'=')
            .count();
        at += padding;
        let blob = &text[start..at];
        if blob.len() >= MIN_BLOB && names_private_key(blob) {
            let out = out.get_or_insert_with(|| String::with_capacity(text.len()));
            out.push_str(&text[copied..start]);
            out.push_str(REDACTED);
            copied = at;
        }
    }
    out.map(|mut out| {
        out.push_str(&text[copied..]);
        out
    })
}

/// The standard and the URL-safe alphabets together.
fn is_base64(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'-' | b'_')
}

/// Whether `blob` is base64 of text that names a private key. Only whole groups of four are
/// decoded, so a blob cut short or run into other text still shows what its start says.
fn names_private_key(blob: &str) -> bool {
    let body = blob.trim_end_matches('=');
    let body = &body[..body.len() - body.len() % 4];
    let chosen = if body.contains(['-', '_']) {
        &alphabet::URL_SAFE
    } else {
        &alphabet::STANDARD
    };
    let engine = GeneralPurpose::new(
        chosen,
        NO_PAD.with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    let Ok(decoded) = engine.decode(body) else {
        return false;
    };
    let lowered = decoded.to_ascii_lowercase();
    [b"privatekey".as_slice(), b"private_key"]
        .iter()
        .any(|needle| {
            lowered
                .windows(needle.len())
                .any(|window| window == *needle)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrubbed(text: &str) -> String {
        credentials(text).into_owned()
    }

    #[test]
    fn the_value_of_an_access_token_is_replaced_and_the_rest_of_the_adapter_kept() {
        assert_eq!(
            scrubbed("livekit:wss://host.example/room?access_token=abc.def-ghi_jkl"),
            "livekit:wss://host.example/room?access_token=REDACTED"
        );
        assert_eq!(
            scrubbed("livekit:wss://h/r?access_token=abc.def&room=one&x=y"),
            "livekit:wss://h/r?access_token=REDACTED&room=one&x=y"
        );
        assert_eq!(
            scrubbed("livekit:wss://h/r?room=one&access_token=abc#frag"),
            "livekit:wss://h/r?room=one&access_token=REDACTED#frag"
        );
    }

    #[test]
    fn text_without_a_credential_is_returned_as_it_is() {
        for text in [
            "",
            "ws-room:wss://host.example/room",
            "access_token is mentioned, never given",
            "livekit:wss://h/r?room=one",
            "the signature does not match; signature: bad length",
            "x?access_token=&y",
            r#"{"signature":"","privateKey":""}"#,
            "bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy",
            "a/very/long/path/with/many/segments/that/is/not/base64/at/all/but/long",
        ] {
            assert!(
                matches!(credentials(text), Cow::Borrowed(t) if t == text),
                "{text}"
            );
        }
    }

    #[test]
    fn every_token_in_a_message_goes_whatever_wraps_it() {
        assert_eq!(
            scrubbed(
                "retry [livekit:wss://a/r?access_token=one] then \"wss://b/r?access_token=two\", \
                 then {'wss://c/r?access_token=three'} and (access_token=four) done"
            ),
            "retry [livekit:wss://a/r?access_token=REDACTED] then \"wss://b/r?access_token=REDACTED\", \
             then {'wss://c/r?access_token=REDACTED'} and (access_token=REDACTED) done"
        );
        // a Debug-printed string escapes its quotes
        assert_eq!(
            scrubbed(r#"["wss://h/r?access_token=one\" and more"]"#),
            r#"["wss://h/r?access_token=REDACTED\" and more"]"#
        );
        // two in a row, and one at the very end
        assert_eq!(
            scrubbed("access_token=a access_token=b"),
            "access_token=REDACTED access_token=REDACTED"
        );
        assert_eq!(scrubbed("x?access_token=tail"), "x?access_token=REDACTED");
    }

    #[test]
    fn names_are_matched_in_any_letter_case_and_the_text_keeps_its_own() {
        assert_eq!(
            scrubbed("a?ACCESS_Token=secret&Béz=1"),
            "a?ACCESS_Token=REDACTED&Béz=1"
        );
        // non-ASCII text before the name moves no offset
        assert_eq!(
            scrubbed("ÅÄÖ İ ?access_token=secret rest"),
            "ÅÄÖ İ ?access_token=REDACTED rest"
        );
        assert_eq!(
            scrubbed(r#"{"PrivateKey":"secret"}"#),
            r#"{"PrivateKey":"REDACTED"}"#
        );
    }

    #[test]
    fn json_fields_keep_their_names_and_shape() {
        assert_eq!(
            scrubbed(
                r#"{"ephemeralIdentity":{"address":"0xfake","privateKey":"0xsecret","publicKey":"0x04fake"},"expiration":"x"}"#
            ),
            r#"{"ephemeralIdentity":{"address":"0xfake","privateKey":"REDACTED","publicKey":"0x04fake"},"expiration":"x"}"#
        );
        // the chain a signed challenge makes: the empty signature stays, the others go
        assert_eq!(
            scrubbed(
                r#"[{"type":"SIGNER","payload":"0xfake","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"dcl-1","signature" : "0xsecret"}]"#
            ),
            r#"[{"type":"SIGNER","payload":"0xfake","signature":""},{"type":"ECDSA_SIGNED_ENTITY","payload":"dcl-1","signature" : "REDACTED"}]"#
        );
        // a value with an escaped quote ends at its own closing quote
        assert_eq!(
            scrubbed(r#"{"accessToken":"se\"cret","next":"kept"}"#),
            r#"{"accessToken":"REDACTED","next":"kept"}"#
        );
        // a string that never closes loses everything to its end
        assert_eq!(
            scrubbed(r#"{"private_key":"secret"#),
            r#"{"private_key":"REDACTED"#
        );
        // not a string: nothing to read the value from
        assert_eq!(scrubbed(r#"{"signature":null}"#), r#"{"signature":null}"#);
    }

    #[test]
    fn debug_printed_json_and_structs_and_event_fields_are_read_too() {
        // a JSON string that Debug printed once, and twice
        assert_eq!(
            scrubbed(r#"chain "[{\"signature\":\"0xsecret\",\"type\":\"X\"}]""#),
            r#"chain "[{\"signature\":\"REDACTED\",\"type\":\"X\"}]""#
        );
        assert_eq!(
            scrubbed(r#"{\\\"privateKey\\\":\\\"0xsecret\\\",\\\"n\\\":1}"#),
            r#"{\\\"privateKey\\\":\\\"REDACTED\\\",\\\"n\\\":1}"#
        );
        // a HashMap's Debug, a struct's Debug, and the layer's own ` name=value` fields
        assert_eq!(
            scrubbed(r#"{"room": "r", "access_token": "eyJsecret.eyJsecret.sig"}"#),
            r#"{"room": "r", "access_token": "REDACTED"}"#
        );
        assert_eq!(
            scrubbed(r#"AuthLink { ty: SIGNER, payload: "0xfake", signature: "0xsecret" }"#),
            r#"AuthLink { ty: SIGNER, payload: "0xfake", signature: "REDACTED" }"#
        );
        assert_eq!(
            scrubbed(r#"ready signature="0xsecret" tries=2"#),
            r#"ready signature="REDACTED" tries=2"#
        );
        // a quoted value after `=` for the query names reads as a string
        assert_eq!(
            scrubbed(r#"access_token="quoted secret" next"#),
            r#"access_token="REDACTED" next"#
        );
    }

    fn identity(json: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(json)
    }

    const IDENTITY: &str = r#"{"ephemeralIdentity":{"address":"0xfake","privateKey":"0xfeedfeedfeed","publicKey":"0x04fake"},"expiration":"2026-11-26T00:00:00.000Z","authChain":[]}"#;

    #[test]
    fn an_identity_in_a_commands_arguments_is_replaced_whole() {
        let blob = identity(IDENTITY);
        let args =
            format!("Command entered: `/login_identity`, with args: `[\"{blob}\", \"--guest\"]`");
        assert_eq!(
            scrubbed(&args),
            "Command entered: `/login_identity`, with args: `[\"REDACTED\", \"--guest\"]`"
        );
        // unpadded, URL-safe, bare, and cut short in the middle of a group
        for blob in [
            identity(IDENTITY).trim_end_matches('=').to_owned(),
            base64::engine::general_purpose::URL_SAFE.encode(IDENTITY),
            identity(IDENTITY)[..101].to_owned(),
        ] {
            assert_eq!(
                scrubbed(&format!("payload={blob}")),
                "payload=REDACTED",
                "{blob}"
            );
        }
    }

    #[test]
    fn base64_that_names_no_private_key_stays() {
        let blob = identity(
            r#"{"ephemeralIdentity":{"address":"0xfake","publicKey":"0x04fake"},"expiration":"x"}"#,
        );
        assert!(blob.len() > MIN_BLOB);
        assert!(matches!(credentials(&blob), Cow::Borrowed(_)));
        let text = format!(
            "an image {} follows",
            identity("not a key, just a long enough text")
        );
        assert!(matches!(credentials(&text), Cow::Borrowed(_)));
    }

    #[test]
    fn scrubbing_twice_changes_nothing_more() {
        for text in [
            "l:wss://h/r?access_token=abc&b=1".to_owned(),
            r#"{"privateKey":"0xsecret","signature":"0xsecret"}"#.to_owned(),
            format!("args [\"{}\"]", identity(IDENTITY)),
        ] {
            let once = scrubbed(&text);
            assert_eq!(scrubbed(&once), once, "{text}");
        }
    }

    #[test]
    fn a_secret_never_survives_in_the_output() {
        for text in [
            "x?access_token=SECRETSECRET",
            "x?access_token=SECRETSECRET&access_token=SECRETSECRET",
            "access_token=access_token=SECRETSECRET",
            r#"{"url": "x?access_token=SECRETSECRET"}"#,
            r#"{"privateKey":"SECRETSECRET","signature":"SECRETSECRET"}"#,
            r#"{"a":{"b":{"signature":"SECRETSECRET"}},"privateKey":"SECRETSECRET"}"#,
            r#"[{\"signature\":\"SECRETSECRET\"}]"#,
        ] {
            assert!(!scrubbed(text).contains("SECRETSECRET"), "{text}");
        }
    }
}

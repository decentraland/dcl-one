//! Rich-text tag scanning shared by every reader of user-authored text:
//! Explore's description sanitizer builds on these primitives, and
//! [`strip_all_markup`] is the plain-text rule for names and titles.

/// Passes a strip runs to reach a fixed point before it fails closed.
pub const MAX_SANITIZE_PASSES: usize = 5;

/// The fail-closed end: every bracket, escaped or not, is removed, until no
/// removal joins a new escape.
pub fn drop_brackets(mut text: String) -> String {
    // Without `\` no escape can form, and without brackets no tag: one pass.
    text.retain(|c| !matches!(c, '<' | '>' | '\\'));
    text
}

/// A tag name starts with a letter, or with `\`: the client decodes a
/// `\uXXXX` escape into any character before it reads the tag.
pub fn starts_tag_name(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'\\'
}

/// The rich-text colour shorthand `<#rgb>`, `<#rgba>`, `<#rrggbb>` or
/// `<#rrggbbaa>`; any other `<#` is text.
pub fn colour_tag_end(bytes: &[u8], lt: usize) -> Option<usize> {
    if bytes.get(lt + 1) != Some(&b'#') {
        return None;
    }
    let digits = bytes[lt + 2..]
        .iter()
        .take_while(|b| b.is_ascii_hexdigit())
        .count();
    let end = lt + 2 + digits;
    (matches!(digits, 3 | 4 | 6 | 8) && bytes.get(end) == Some(&b'>')).then_some(end + 1)
}

/// The rich-text renderer decodes `\uXXXX` and `\UXXXXXXXX` escapes before it
/// reads tags, so an escaped `<` or `>` is turned into the bracket itself
/// first and then sanitized like one.
pub fn decode_bracket_escapes(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("\\u") && !text.contains("\\U") {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let width = match tail.as_bytes().get(1) {
            Some(b'u') => 4,
            Some(b'U') => 8,
            _ => 0,
        };
        let decoded = (width > 0)
            .then(|| tail.get(2..2 + width))
            .flatten()
            .filter(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .and_then(char::from_u32)
            .filter(|c| matches!(c, '<' | '>'));
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[2 + width..];
            }
            None => {
                out.push('\\');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

/// Tag names the client's text renderer acts on. A stripped span with any
/// other name is shown by the client as text, which decides the fallback of
/// [`strip_all_markup`]; it never decides what is stripped.
pub const RICH_TEXT_TAG_NAMES: &[&str] = &[
    "a",
    "action",
    "align",
    "allcaps",
    "alpha",
    "b",
    "br",
    "class",
    "color",
    "cr",
    "cspace",
    "font",
    "font-weight",
    "gradient",
    "i",
    "indent",
    "line-height",
    "line-indent",
    "link",
    "lowercase",
    "margin",
    "margin-left",
    "margin-right",
    "mark",
    "material",
    "mspace",
    "nbsp",
    "nobr",
    "noparse",
    "page",
    "pos",
    "rotate",
    "s",
    "scale",
    "shy",
    "size",
    "smallcaps",
    "space",
    "sprite",
    "strikethrough",
    "style",
    "sub",
    "sup",
    "table",
    "td",
    "th",
    "tr",
    "u",
    "uppercase",
    "voffset",
    "width",
    "zwj",
    "zwsp",
];

/// Plain text for names and titles, which keep no markup at all: every
/// tag-shaped span (a `<` then a letter, `/` or the colour shorthand, up to
/// the next `>`) is dropped, escapes decoded in every pass, to a fixed point,
/// so no link, size or colour tag survives; other `<`/`>` text is kept, and
/// only the whitespace a dropped tag leaves is collapsed. Fails closed like
/// [`sanitize_markup_description`]. Text that was only rich-text tags comes
/// back empty, so callers use their own fallback; text the client would show
/// literally (`<<Best Party>>`) keeps its words.
pub fn strip_all_markup(text: &str) -> String {
    let mut current = text.to_owned();
    let mut stable = false;
    for _ in 0..MAX_SANITIZE_PASSES {
        let next = strip_tag_spans(&decode_bracket_escapes(&current));
        if next == current {
            stable = true;
            break;
        }
        current = next;
    }
    if !stable {
        current = drop_brackets(current);
    }
    if current == text || current.chars().any(char::is_alphanumeric) {
        return current;
    }
    if has_literal_span(&decode_bracket_escapes(text)) {
        let words = decode_bracket_escapes(text).replace(['<', '>'], " ");
        let words = drop_brackets(words);
        if words.chars().any(char::is_alphanumeric) {
            return words.split_whitespace().collect::<Vec<_>>().join(" ");
        }
    }
    String::new()
}

/// Whether `text` holds a tag-shaped span the client shows as text (its name
/// is no rich-text tag), the one case where tag-only text keeps its words.
fn has_literal_span(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.match_indices('<')
        .any(|(lt, _)| name_tag_end(bytes, lt).is_some_and(|end| !is_rich_text_tag(&text[lt..end])))
}

/// Drops tag spans, and the whitespace a span leaves at either end of the
/// text or doubled between words.
fn strip_tag_spans(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut dropped_at_end = false;
    while let Some(rel) = text[i..].find('<') {
        let lt = i + rel;
        out.push_str(&text[i..lt]);
        match name_tag_end(bytes, lt).or_else(|| colour_tag_end(bytes, lt)) {
            Some(end) => {
                i = end;
                if out.is_empty() || out.ends_with(char::is_whitespace) {
                    i += text[i..].len() - text[i..].trim_start().len();
                }
                dropped_at_end = i == text.len();
            }
            None => {
                out.push('<');
                i = lt + 1;
                dropped_at_end = false;
            }
        }
    }
    out.push_str(&text[i..]);
    if dropped_at_end {
        out.truncate(out.trim_end().len());
    }
    out
}

fn is_rich_text_tag(span: &str) -> bool {
    let inner = span.trim_start_matches('<').trim_start_matches('/');
    if inner.starts_with(['#', '\\']) {
        return true;
    }
    let name_len = inner.find([' ', '=', '/', '>']).unwrap_or(inner.len());
    RICH_TEXT_TAG_NAMES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(&inner[..name_len]))
}

/// A `<name…>` or `</name…>` tag ending at the first `>` before any other `<`.
fn name_tag_end(bytes: &[u8], lt: usize) -> Option<usize> {
    let mut j = lt + 1;
    if bytes.get(j) == Some(&b'/') {
        j += 1;
    }
    if !starts_tag_name(*bytes.get(j)?) {
        return None;
    }
    let rel = bytes[j..].iter().position(|b| matches!(b, b'<' | b'>'))?;
    (bytes[j + rel] == b'>').then_some(j + rel + 1)
}

/// [`strip_all_markup`] for an optional field: text that was only tags
/// becomes `None`, so the field reads as unset.
pub fn strip_all_markup_opt(text: Option<String>) -> Option<String> {
    text.map(|t| strip_all_markup(&t)).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_all_markup_is_idempotent() {
        for input in [
            "Party <link=\"file:///etc\">click</link><size=300>!",
            "<<<<<<<<b>b>b>b>b>b>b>b>",
            "<<Best Party>>",
            "\\u003Cb\\u003Ebold",
            "Club <VIP>  Night",
            "\\u<b>003Clink=\"file:///x\"\\u<b>003Eclick",
            "\\u0<b>03C\\u<i>003E",
            "<<<<<<\\u003Cb>b>b>b>b>b>",
        ] {
            let once = strip_all_markup(input);
            assert_eq!(strip_all_markup(&once), once, "{input:?}");
            assert!(
                !once.contains('<') || !once.contains('>'),
                "{input:?}: {once:?}"
            );
        }
    }

    #[test]
    fn strip_all_markup_drops_every_tag_and_keeps_plain_text() {
        for (input, expected) in [
            (
                "Party <link=\"file:///etc\">click</link><size=300>!",
                "Party click!",
            ),
            ("<link=\"https://a.com\">safe</link>", "safe"),
            ("<#ff0000>red", "red"),
            ("<<b>link=x>click", "click"),
            ("Plain title", "Plain title"),
            ("Plaza <b>", "Plaza"),
            ("<size=9> Tagged </size>", "Tagged"),
            (" Untagged ", " Untagged "),
            ("Club <VIP> Night", "Club Night"),
            ("<<Best Party>>", "Best Party"),
            ("<\\u0073ize=300>BIG", "BIG"),
            ("</\\u006Cink>x", "x"),
            ("<br/>", ""),
            ("<<b>foo>", ""),
            ("<size=300><b>", ""),
            ("<b> </b>", ""),
            ("A  B <b>bold</b>", "A  B bold"),
            ("tab\there <b>x</b>", "tab\there x"),
            ("< b>not a tag", "< b>not a tag"),
            (
                "\\u<b>003Clink=\"file:///x\"\\u<b>003Eclick\\u<b>003C/link\\u<b>003E",
                "click",
            ),
            ("\\u<b>003Csize=300\\u<b>003EBig", "Big"),
            ("I <3 you >_<", "I <3 you >_<"),
            ("issue <#12 and a > b", "issue <#12 and a > b"),
            (
                "\\u003Clink=\"file:///x\"\\u003Eclick\\u003C/link\\u003E",
                "click",
            ),
            ("<size=300\\U0000003EBig", "Big"),
            ("a \\u0041 b", "a \\u0041 b"),
            ("5 < 10, I <3 it", "5 < 10, I <3 it"),
            ("a > b", "a > b"),
        ] {
            assert_eq!(strip_all_markup(input), expected, "{input:?}");
        }
        let out = strip_all_markup("<<<<<<<<b>b>b>b>b>b>b>b>");
        assert!(!out.contains('<') && !out.contains('>'), "{out}");
        assert_eq!(strip_all_markup_opt(None), None);
        assert_eq!(strip_all_markup_opt(Some("<size=300><b>".into())), None);
        assert_eq!(strip_all_markup_opt(Some(String::new())), None);
    }
}

//! Whether a creator's file is an SVG image: a well-formed XML document
//! whose root is an `svg` element in the SVG namespace, with no DTD, no
//! entity but the predefined and character ones, at most [`MAX_DEPTH`]
//! elements deep, checked in one pass over at most [`MAX_BYTES`].

const NAMESPACE: &str = "http://www.w3.org/2000/svg";
pub(crate) const MAX_DEPTH: usize = 256;

/// The largest file read whole to tell whether it is SVG.
pub const MAX_BYTES: u64 = 1 << 20;

fn space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ':' | '_' | '-' | '.')
}

fn valid_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':')
        && name.chars().all(name_char)
}

/// Text or an attribute value: no `<`, and every `&` starts a predefined or
/// character reference.
fn valid_text(text: &str) -> bool {
    if text.contains('<') {
        return false;
    }
    text.split('&').skip(1).all(|after| {
        let Some((reference, _)) = after.split_once(';') else {
            return false;
        };
        match reference.strip_prefix('#') {
            Some(hex) if hex.starts_with('x') => {
                hex.len() > 1 && hex[1..].chars().all(|c| c.is_ascii_hexdigit())
            }
            Some(decimal) => !decimal.is_empty() && decimal.chars().all(|c| c.is_ascii_digit()),
            None => matches!(reference, "lt" | "gt" | "amp" | "quot" | "apos"),
        }
    })
}

/// A start tag: its element name, its attributes, whether it closes itself,
/// and what follows its `>`.
struct StartTag<'a> {
    name: &'a str,
    attributes: Vec<(&'a str, &'a str)>,
    closed: bool,
    after: &'a str,
}

/// The start tag after a `<`.
fn start_tag(rest: &str) -> Option<StartTag<'_>> {
    let end = rest.find(|c: char| space(c) || c == '/' || c == '>')?;
    let (name, mut rest) = rest.split_at(end);
    if !valid_name(name) {
        return None;
    }
    let mut attributes: Vec<(&str, &str)> = Vec::new();
    loop {
        let trimmed = rest.trim_start_matches(space);
        let spaced = trimmed.len() < rest.len();
        if let Some(after) = trimmed.strip_prefix("/>") {
            return Some(StartTag {
                name,
                attributes,
                closed: true,
                after,
            });
        }
        if let Some(after) = trimmed.strip_prefix('>') {
            return Some(StartTag {
                name,
                attributes,
                closed: false,
                after,
            });
        }
        if !spaced {
            return None;
        }
        let (attribute, after) = trimmed.split_at(trimmed.find(|c: char| space(c) || c == '=')?);
        let after = after.trim_start_matches(space).strip_prefix('=')?;
        let after = after.trim_start_matches(space);
        let quote = after.chars().next().filter(|q| *q == '"' || *q == '\'')?;
        let (value, after) = after[1..].split_once(quote)?;
        if !valid_name(attribute)
            || !valid_text(value)
            || attributes.iter().any(|(seen, _)| *seen == attribute)
        {
            return None;
        }
        attributes.push((attribute, value));
        rest = after;
    }
}

/// Whether `bytes` is an SVG image as this module defines it.
pub fn is_svg(bytes: &[u8]) -> bool {
    if bytes.len() as u64 > MAX_BYTES {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut open: Vec<&str> = Vec::new();
    let mut rooted = false;
    loop {
        if open.is_empty() {
            rest = rest.trim_start_matches(space);
            if rest.is_empty() {
                return rooted;
            }
        }
        let Some(lt) = rest.find('<') else {
            return false;
        };
        let (content, tag) = rest.split_at(lt);
        if (open.is_empty() && !content.is_empty())
            || content.contains("]]>")
            || !valid_text(content)
        {
            return false;
        }
        rest = if let Some(after) = tag.strip_prefix("<!--") {
            let Some((comment, after)) = after.split_once("-->") else {
                return false;
            };
            if comment.contains("--") {
                return false;
            }
            after
        } else if let Some(after) = tag.strip_prefix("<?") {
            let Some((_, after)) = after.split_once("?>") else {
                return false;
            };
            after
        } else if let Some(after) = tag.strip_prefix("<![CDATA[") {
            let Some((_, after)) = after.split_once("]]>").filter(|_| !open.is_empty()) else {
                return false;
            };
            after
        } else if tag.starts_with("<!") {
            return false;
        } else if let Some(after) = tag.strip_prefix("</") {
            let Some((name, after)) = after.split_once('>') else {
                return false;
            };
            if open.pop() != Some(name.trim_end_matches(space)) {
                return false;
            }
            after
        } else {
            let Some(StartTag {
                name,
                attributes,
                closed,
                after,
            }) = start_tag(&tag[1..])
            else {
                return false;
            };
            if open.is_empty() {
                let svg_root = name == "svg"
                    && attributes
                        .iter()
                        .any(|(attribute, value)| *attribute == "xmlns" && *value == NAMESPACE);
                if rooted || !svg_root {
                    return false;
                }
                rooted = true;
            }
            if !closed {
                open.push(name);
                if open.len() > MAX_DEPTH {
                    return false;
                }
            }
            after
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\">";

    #[test]
    fn a_document_is_svg_only_when_it_is_well_formed_with_an_svg_root() {
        for (document, svg) in [
            (format!("{SVG}</svg>"), true),
            (
                format!(
                    "\u{feff}<?xml version=\"1.0\"?>\n<!-- a -->{SVG}<g a='1' b=\"&amp;&#38;&#x26;\"/>text</svg>\n<!-- b -->"
                ),
                true,
            ),
            (format!("{SVG}<![CDATA[<x>]]></svg>"), true),
            (format!("{SVG}</svg><svg/>"), false),
            (format!("{SVG}<g></svg>"), false),
            (format!("{SVG}&custom;</svg>"), false),
            (format!("<!DOCTYPE svg>{SVG}</svg>"), false),
            (format!("{SVG}<g a=\"1\" a=\"2\"/></svg>"), false),
            (format!("{SVG}<g a=1/></svg>"), false),
            (format!("{SVG}<g a=\"<\"/></svg>"), false),
            (format!("{SVG}<!-- a -- b --></svg>"), false),
            (format!("text{SVG}</svg>"), false),
            ("<svg></svg>".to_string(), false),
            (
                "<html><svg xmlns=\"http://www.w3.org/2000/svg\"></svg></html>".to_string(),
                false,
            ),
            (
                format!(
                    "{SVG}{}{}</svg>",
                    "<g>".repeat(MAX_DEPTH),
                    "</g>".repeat(MAX_DEPTH)
                ),
                false,
            ),
            (
                format!(
                    "{SVG}{}{}</svg>",
                    "<g>".repeat(MAX_DEPTH - 1),
                    "</g>".repeat(MAX_DEPTH - 1)
                ),
                true,
            ),
        ] {
            assert_eq!(is_svg(document.as_bytes()), svg, "{document:.80}");
        }
        assert!(!is_svg(&[0xff, 0xfe]));
    }
}

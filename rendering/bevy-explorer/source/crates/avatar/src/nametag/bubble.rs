//! Chat bubbles: the messages a nametag opens up to show, and how their text is read.

use std::ops::Range;

use alloy_core::primitives::Address;
use bevy::prelude::*;
use common::structs::ChatBubbleVisibility;

use super::text::Span;

/// The message text's colour.
pub const TEXT_COLOR: Color = Color::srgb_u8(252, 252, 252);
/// The colour of links, coordinates and mentions in a message.
const LINK_COLOR: Color = Color::srgb_u8(0x00, 0xB2, 0xFF);

/// The most characters of a message a bubble shows; the rest is cut.
pub const MAX_MESSAGE_CHARS: usize = 100;
/// The most characters of a community's name a bubble shows.
pub const MAX_COMMUNITY_NAME_CHARS: usize = 28;
/// The most characters of a player's name a tag shows.
pub const MAX_NAME_CHARS: usize = 32;

/// `text` on one line and at most `max` characters long, ending in an ellipsis when cut. What
/// a peer sends is bounded here, before any of it is laid out.
pub fn capped(text: &str, max: usize) -> String {
    let mut chars = text.chars().map(|c| if c.is_control() { ' ' } else { c });
    let mut capped = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        capped.truncate(capped.trim_end().len());
        capped.push('…');
    }
    capped
}

/// The most a bold or italic tag holds between the whitespace after its letter and its `>`,
/// in UTF-16 code units: what the HUD's pattern counts.
const MAX_STYLE_TAG_REST: usize = 32;

/// Whitespace as the HUD's pattern reads it (`\s` in JavaScript).
fn js_whitespace(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// The length of the bold or italic tag opening `text`: `<b>`, `</b>`, `<i>` or `</i>`, each
/// also with whitespace and up to [`MAX_STYLE_TAG_REST`] more, none of it `<`, before its `>`.
/// The HUD's chat reads the same tags (`TOKEN_RE` in `chatParse.ts`).
fn style_tag_len(text: &str) -> Option<usize> {
    let body = text.strip_prefix('<')?;
    let body = body.strip_prefix('/').unwrap_or(body);
    let after = body.strip_prefix(['b', 'i'])?;
    let name_end = text.len() - after.len();
    let mut chars = after.char_indices();
    match chars.next()?.1 {
        '>' => Some(name_end + 1),
        c if js_whitespace(c) => {
            let mut rest = 0;
            for (index, c) in chars {
                match c {
                    '>' => return Some(name_end + index + 1),
                    '<' => return None,
                    _ => rest += c.len_utf16(),
                }
                if rest > MAX_STYLE_TAG_REST {
                    return None;
                }
            }
            None
        }
        _ => None,
    }
}

/// `message` with each bold and italic tag replaced by `with`.
fn replace_style_tags(message: &str, with: &str) -> String {
    let mut plain = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(start) = rest.find('<') {
        plain.push_str(&rest[..start]);
        rest = &rest[start..];
        let skipped = match style_tag_len(rest) {
            Some(len) => {
                plain.push_str(with);
                len
            }
            None => {
                plain.push('<');
                1
            }
        };
        rest = &rest[skipped..];
    }
    plain.push_str(rest);
    plain
}

/// `message` without its bold and italic tags: the HUD's chat styles the text between them, a
/// bubble has one face and shows the words alone.
pub(crate) fn without_style_tags(message: &str) -> String {
    replace_style_tags(message, "")
}

/// A chat message to show over its sender's nametag. Nothing is shown for a sender with no
/// nametag up, and the message is not kept for later.
#[derive(Event, Clone, Debug)]
pub struct ChatBubbleRequest {
    pub sender: Entity,
    pub kind: BubbleKind,
    pub message: String,
    /// Names the message for a later [`ChatBubbleRetext`].
    pub message_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BubbleKind {
    Nearby,
    /// In a community's chat; the community's name, when known.
    Community(Option<String>),
    /// A direct message; on the local user's own, who it is for.
    Dm {
        recipient: Option<Address>,
    },
}

impl BubbleKind {
    /// Whether the user's setting lets this kind of message show.
    pub fn allowed_by(&self, visibility: ChatBubbleVisibility) -> bool {
        match visibility {
            ChatBubbleVisibility::All => true,
            ChatBubbleVisibility::NearbyOnly => *self == BubbleKind::Nearby,
            ChatBubbleVisibility::None => false,
        }
    }
}

/// Replaces the text of a bubble still showing the message `message_id`: its translation, or
/// the original back.
#[derive(Event, Clone, Debug)]
pub struct ChatBubbleRetext {
    pub message_id: String,
    pub message: String,
}

fn boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| c.is_whitespace() || matches!(c, '!' | '?' | '.' | ','))
}

/// The byte ranges of the words starting at a line's start or after whitespace.
fn words(message: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut chars = message.char_indices().peekable();
    std::iter::from_fn(move || {
        while chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}
        let (start, _) = chars.next()?;
        let mut end = message.len();
        while let Some((index, c)) = chars.peek().copied() {
            if c.is_whitespace() {
                end = index;
                break;
            }
            chars.next();
        }
        Some(start..end)
    })
}

/// The length of an @mention opening `word`: `@`, up to 15 letters or digits, an optional
/// `#` and four more, then the word's end or punctuation. The HUD's chat reads mentions the
/// same way (`MENTION_PATTERN` in `mention.ts`).
fn mention_len(word: &str) -> Option<usize> {
    let rest = word.strip_prefix('@')?;
    let name = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    if !(1..=15).contains(&name) {
        return None;
    }
    let mut len = name;
    if let Some(tag) = rest[name..].strip_prefix('#') {
        let tag_len = tag
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(tag.len());
        if tag_len != 4 {
            return None;
        }
        len += 5;
    }
    boundary(rest[len..].chars().next()).then_some(len + 1)
}

/// The length of the parcel coordinates opening `word` (`-12,34`).
fn coordinates_len(word: &str) -> Option<usize> {
    let number = |text: &str| {
        let digits = text.strip_prefix('-').unwrap_or(text);
        let len = digits
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(digits.len());
        (1..=3)
            .contains(&len)
            .then_some(text.len() - digits.len() + len)
    };
    let x = number(word)?;
    let y = number(word[x..].strip_prefix(',')?)?;
    let len = x + 1 + y;
    boundary(word[len..].chars().next()).then_some(len)
}

fn is_url(word: &str) -> bool {
    let Some(rest) = word
        .strip_prefix("https://")
        .or_else(|| word.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default();
    let mut labels = host.split('.');
    let Some(top) = labels.next_back() else {
        return false;
    };
    let label = |label: &str| {
        !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    top.len() >= 2
        && top.chars().all(|c| c.is_ascii_alphabetic())
        && labels.clone().next().is_some()
        && labels.all(label)
}

/// The message's links: URLs, parcel coordinates and @mentions.
fn links(message: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    words(message).filter_map(move |word| {
        let text = &message[word.clone()];
        let len = if is_url(text) {
            text.len()
        } else {
            mention_len(text).or_else(|| coordinates_len(text))?
        };
        Some(word.start..word.start + len)
    })
}

/// The message as coloured runs: its links stand out.
pub fn message_spans(message: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut at = 0;
    for link in links(message) {
        if link.start > at {
            spans.push(Span::new(&message[at..link.start], TEXT_COLOR));
        }
        spans.push(Span::new(&message[link.clone()], LINK_COLOR));
        at = link.end;
    }
    if at < message.len() {
        spans.push(Span::new(&message[at..], TEXT_COLOR));
    }
    spans
}

/// What follows `@` in a mention of this user: the name's letters and digits, plus `#` and
/// the address' last four characters when the name isn't claimed.
pub fn mention_name(name: &str, address: &str, claimed: bool) -> String {
    let base = name
        .split('#')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    match address.len().checked_sub(4) {
        Some(last4) if !claimed && address.is_char_boundary(last4) => {
            format!("{base}#{}", &address[last4..])
        }
        _ => base,
    }
}

/// Does the message @-mention exactly this mention name (case-insensitive)? A bold or italic
/// tag ends a word as a space does, so a mention may stand right inside one (`<b>@Ada</b>`).
pub fn mentions(message: &str, mention: &str) -> bool {
    let message = &replace_style_tags(message, " ");
    !mention.is_empty()
        && words(message).any(|word| {
            let text = &message[word];
            mention_len(text).is_some_and(|len| text[1..len].eq_ignore_ascii_case(mention))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_cut_to_its_cap_on_one_line() {
        assert_eq!(capped("hello", 5), "hello");
        assert_eq!(capped("hello there", 5), "hello…");
        assert_eq!(capped("hello there", 6), "hello…");
        // characters, not bytes
        assert_eq!(capped("名前😀名前", 3), "名前😀…");
        assert_eq!(capped("one\ntwo\r\nthree", 100), "one two  three");
        let long = capped(&"😀\n".repeat(5000), MAX_MESSAGE_CHARS);
        assert_eq!(long.chars().count(), MAX_MESSAGE_CHARS);
        assert!(!long.contains('\n'));
    }

    #[test]
    fn bold_and_italic_tags_are_left_out() {
        // the tags `parseMessage` in library/explorer-core/features/chat/chatParse.ts reads
        assert_eq!(
            without_style_tags("<b>bold</b> and <i>italic</i>"),
            "bold and italic"
        );
        assert_eq!(without_style_tags("<b x=1>open</b >"), "open");
        assert_eq!(without_style_tags("<i><b>both"), "both");
        // a tag ends at its own `>`, within reach: what follows another `<` is not its text
        assert_eq!(without_style_tags("I <i really> mean it"), "I  mean it");
        assert_eq!(
            without_style_tags("x <b hello <i>world"),
            "x <b hello world"
        );
        // counted as the HUD counts: an emoji is two
        let emoji = format!("<b {}>", "😀".repeat(MAX_STYLE_TAG_REST / 2 + 1));
        assert_eq!(without_style_tags(&emoji), emoji);
        let emoji = format!("<b {}>", "😀".repeat(MAX_STYLE_TAG_REST / 2));
        assert_eq!(without_style_tags(&emoji), "");
        // and whitespace as it reads it
        assert_eq!(without_style_tags("<b\u{feff}x>"), "");
        assert_eq!(without_style_tags("<b\u{85}x>"), "<b\u{85}x>");
        let long = format!("<b {}>", "x".repeat(MAX_STYLE_TAG_REST + 1));
        assert_eq!(without_style_tags(&long), long);
        let fits = format!("<b {}>", "x".repeat(MAX_STYLE_TAG_REST));
        assert_eq!(without_style_tags(&fits), "");
        for kept in [
            "<B>caps</B>",
            "<br>",
            "<bold>",
            "< b>",
            "a < b",
            "<b",
            "<b x",
            "<>",
            "名<前",
        ] {
            assert_eq!(without_style_tags(kept), kept, "{kept}");
        }
    }

    #[test]
    fn mentions_are_whole_words_naming_the_user() {
        // the cases `mentionsName` in library/explorer-core/engine/mention.ts decides
        assert!(mentions("hi @Ada", "ada"));
        assert!(mentions("@ada, hello", "Ada"));
        assert!(mentions("@Ada!", "Ada"));
        assert!(mentions("hey @Ada#12ab?", "Ada#12ab"));
        assert!(!mentions("hey @Ada#12ab", "Ada"));
        assert!(!mentions("hey @Adam", "Ada"));
        assert!(!mentions("mail@Ada", "Ada"));
        assert!(!mentions("@Ada#12a", "Ada"));
        assert!(!mentions("@Ada;", "Ada"));
        assert!(!mentions("@abcdefghijklmnop", "abcdefghijklmnop"));
        assert!(!mentions("hello", ""));
        // right inside bold or italic tags
        assert!(mentions("<b>@Ada</b>", "Ada"));
        assert!(mentions("<b>hi @Ada</b>", "Ada"));
        assert!(mentions("say<i>@Ada#12ab</i>now", "Ada#12ab"));
        assert!(!mentions("<u>@Ada</u>", "Ada"));
    }

    #[test]
    fn a_mention_name_is_the_names_letters_and_digits() {
        let address = "0x00000000000000000000000000000000000012ab";
        assert_eq!(mention_name("Ada Lovelace!", address, true), "AdaLovelace");
        assert_eq!(mention_name("Ada", address, false), "Ada#12ab");
        assert_eq!(mention_name("Ada#12ab", address, false), "Ada#12ab");
        assert_eq!(mention_name("Ada", "0x1", false), "Ada");
    }

    fn colours(message: &str) -> Vec<(String, bool)> {
        message_spans(message)
            .into_iter()
            .map(|span| (span.text, span.color == LINK_COLOR))
            .collect()
    }

    #[test]
    fn links_stand_out_in_a_message() {
        assert_eq!(colours("plain text"), [("plain text".to_owned(), false)]);
        assert_eq!(
            colours("meet @Ada at -12,34!"),
            [
                ("meet ".to_owned(), false),
                ("@Ada".to_owned(), true),
                (" at ".to_owned(), false),
                ("-12,34".to_owned(), true),
                ("!".to_owned(), false),
            ]
        );
        assert_eq!(
            colours("see https://decentraland.org/play?x=1 now"),
            [
                ("see ".to_owned(), false),
                ("https://decentraland.org/play?x=1".to_owned(), true),
                (" now".to_owned(), false),
            ]
        );
        // not links: no scheme, a bare host, four digits, a tag of the wrong length
        for plain in ["decentraland.org", "https://localhost", "1234,5", "@Ada#1"] {
            assert_eq!(colours(plain), [(plain.to_owned(), false)], "{plain}");
        }
    }

    #[test]
    fn the_setting_picks_which_messages_show() {
        let dm = BubbleKind::Dm { recipient: None };
        let community = BubbleKind::Community(None);
        for (visibility, nearby, other) in [
            (ChatBubbleVisibility::All, true, true),
            (ChatBubbleVisibility::NearbyOnly, true, false),
            (ChatBubbleVisibility::None, false, false),
        ] {
            assert_eq!(BubbleKind::Nearby.allowed_by(visibility), nearby);
            assert_eq!(dm.allowed_by(visibility), other);
            assert_eq!(community.allowed_by(visibility), other);
        }
    }
}

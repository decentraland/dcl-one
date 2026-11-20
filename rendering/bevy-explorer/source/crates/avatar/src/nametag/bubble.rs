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
    let mut chars = text.chars().map(one_line);
    let mut capped = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        capped.truncate(capped.trim_end().len());
        capped.push('…');
    }
    capped
}

/// A character as it stands on one line: a control character is a space.
fn one_line(c: char) -> char {
    if c.is_control() {
        ' '
    } else {
        c
    }
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

/// A bold or italic tag of a message, or the text between two.
enum Piece<'a> {
    Text(&'a str),
    Tag {
        text: &'a str,
        /// 0 for bold, 1 for italic.
        style: usize,
        close: bool,
    },
}

/// `message` as its bold and italic tags and the text between them, in order.
fn pieces(message: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut text_start = 0;
    let mut at = 0;
    while let Some(found) = message[at..].find('<') {
        let start = at + found;
        let Some(len) = style_tag_len(&message[start..]) else {
            at = start + 1;
            continue;
        };
        if start > text_start {
            pieces.push(Piece::Text(&message[text_start..start]));
        }
        let text = &message[start..start + len];
        let close = text.starts_with("</");
        let letter = if close { 2 } else { 1 };
        pieces.push(Piece::Tag {
            text,
            style: usize::from(text.as_bytes()[letter] == b'i'),
            close,
        });
        at = start + len;
        text_start = at;
    }
    if text_start < message.len() {
        pieces.push(Piece::Text(&message[text_start..]));
    }
    pieces
}

/// For each piece, whether it is a tag that styles text: an opening tag whose style is off,
/// with text before that style's next closing tag, or the closing tag that ends such a one.
/// Any other tag (`<b></b>`, a closing tag with nothing open, a second `<b>` inside bold)
/// is shown as written. The HUD's chat reads the same (`styling` in `chatParse.ts`).
fn styling(pieces: &[Piece]) -> Vec<bool> {
    // from the end: whether text stands between each opening tag and its style's next
    // closing tag
    let mut styles = vec![false; pieces.len()];
    let mut ahead = [false; 2];
    for (index, piece) in pieces.iter().enumerate().rev() {
        match *piece {
            Piece::Text(_) => ahead = [true; 2],
            Piece::Tag {
                style, close: true, ..
            } => ahead[style] = false,
            Piece::Tag { style, .. } => styles[index] = ahead[style],
        }
    }
    let mut on = [false; 2];
    for (styles, piece) in styles.iter_mut().zip(pieces) {
        if let Piece::Tag { style, close, .. } = *piece {
            *styles = if close {
                on[style]
            } else {
                !on[style] && *styles
            };
            if *styles {
                on[style] = !close;
            }
        }
    }
    styles
}

/// A message as a bubble shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BubbleText {
    /// The words alone: the HUD's chat styles the text between bold and italic tags, a bubble
    /// has one face.
    pub text: String,
    /// The byte offsets in `text` where a tag stood, in order. A tag ends a word as a space
    /// does, so a link beside one is coloured as the HUD reads it.
    breaks: Vec<usize>,
}

/// `message` on one line, without the bold and italic tags that style its text, and at most
/// `max` characters long, ending in an ellipsis when cut. What a peer sends is bounded here,
/// before any of it is laid out.
pub(crate) fn bubble_text(message: &str, max: usize) -> BubbleText {
    let pieces = pieces(message);
    let mut text = String::with_capacity(message.len());
    let mut breaks = Vec::new();
    for (piece, styles) in pieces.iter().zip(styling(&pieces)) {
        match *piece {
            Piece::Text(piece) => text.extend(piece.chars().map(one_line)),
            Piece::Tag { .. } if styles => breaks.push(text.len()),
            Piece::Tag { text: tag, .. } => {
                breaks.push(text.len());
                text.extend(tag.chars().map(one_line));
                breaks.push(text.len());
            }
        }
    }
    if let Some((cut, _)) = text.char_indices().nth(max) {
        text.truncate(cut);
        text.truncate(text.trim_end().len());
        breaks.retain(|at| *at < text.len());
        text.push('…');
    }
    BubbleText { text, breaks }
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
    c.is_none_or(|c| js_whitespace(c) || matches!(c, '!' | '?' | '.' | ','))
}

/// The byte ranges of the words starting at a line's start, after whitespace or at one of
/// `breaks`, which are in order. Whitespace is the HUD's, as for the tags, so both part the
/// same words.
fn words<'a>(message: &'a str, breaks: &'a [usize]) -> impl Iterator<Item = Range<usize>> + 'a {
    let mut chars = message.char_indices().peekable();
    std::iter::from_fn(move || {
        while chars.next_if(|(_, c)| js_whitespace(*c)).is_some() {}
        let (start, _) = chars.next()?;
        let mut end = message.len();
        while let Some((index, c)) = chars.peek().copied() {
            if js_whitespace(c) || breaks.binary_search(&index).is_ok() {
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
fn links<'a>(message: &'a str, breaks: &'a [usize]) -> impl Iterator<Item = Range<usize>> + 'a {
    words(message, breaks).filter_map(move |word| {
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
pub(crate) fn message_spans(message: &BubbleText) -> Vec<Span> {
    let BubbleText {
        text: message,
        breaks,
    } = message;
    let mut spans = Vec::new();
    let mut at = 0;
    for link in links(message, breaks) {
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
/// tag ends a word as a space does, so a mention may stand right inside one (`<b>@Ada</b>`),
/// and text in a tag's brackets is none.
pub fn mentions(message: &str, mention: &str) -> bool {
    let message = &pieces(message)
        .iter()
        .map(|piece| match *piece {
            Piece::Text(text) => text,
            Piece::Tag { .. } => " ",
        })
        .collect::<String>();
    !mention.is_empty()
        && words(message, &[]).any(|word| {
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

    fn plain(message: &str) -> String {
        bubble_text(message, usize::MAX).text
    }

    #[test]
    fn a_bubbles_text_is_cut_like_any_other() {
        for message in ["hello there", "one\ntwo\r\nthree", "名前😀名前"] {
            for max in [3, 5, 6, 100] {
                assert_eq!(bubble_text(message, max).text, capped(message, max));
            }
        }
        // the cap counts what is shown, and the tags are not
        assert_eq!(bubble_text("<b>hello</b> there", 5).text, "hello…");
        assert_eq!(bubble_text("<b>hello</b>", 5).text, "hello");
        // a control character in a tag shown as written is a space too
        assert_eq!(plain("a</b\tx>"), "a</b x>");
    }

    #[test]
    fn bold_and_italic_tags_are_left_out() {
        // the tags `parseMessage` in library/explorer-core/features/chat/chatParse.ts reads
        assert_eq!(plain("<b>bold</b> and <i>italic</i>"), "bold and italic");
        assert_eq!(plain("<b x=1>open</b >"), "open");
        assert_eq!(plain("<i><b>both"), "both");
        // a tag ends at its own `>`, within reach: what follows another `<` is not its text
        assert_eq!(plain("I <i really> mean it"), "I  mean it");
        assert_eq!(plain("x <b hello <i>world"), "x <b hello world");
        // counted as the HUD counts: an emoji is two
        let emoji = format!("<b {}>x", "😀".repeat(MAX_STYLE_TAG_REST / 2 + 1));
        assert_eq!(plain(&emoji), emoji);
        let emoji = format!("<b {}>x", "😀".repeat(MAX_STYLE_TAG_REST / 2));
        assert_eq!(plain(&emoji), "x");
        // and whitespace as it reads it
        assert_eq!(plain("<b\u{feff}x>y"), "y");
        assert_eq!(plain("<b\u{85}x>y"), "<b x>y");
        let long = format!("<b {}>x", "x".repeat(MAX_STYLE_TAG_REST + 1));
        assert_eq!(plain(&long), long);
        let fits = format!("<b {}>x", "x".repeat(MAX_STYLE_TAG_REST));
        assert_eq!(plain(&fits), "x");
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
            assert_eq!(plain(kept), kept, "{kept}");
        }
    }

    #[test]
    fn a_tag_that_styles_no_text_is_shown_as_written() {
        // the cases `styling` in chatParse.ts decides
        for kept in [
            "<b></b>",
            "a</i>b",
            "a <b>",
            "<b><i></b></i>",
            "<b><i></i></b>",
            "</b >",
            "boe<i></i>do.dcl.eth",
        ] {
            assert_eq!(plain(kept), kept, "{kept}");
        }
        // a second opening inside the first, and the closing left over
        assert_eq!(plain("<b>a<b>b</b>c</b>"), "a<b>bc</b>");
        // a tag of the other style between is no text
        assert_eq!(plain("<b><i>a</i></b>"), "a");
        // many tags are read once each
        let flood = "<b><i>".repeat(20_000);
        assert_eq!(plain(&flood), flood);
        // whitespace is text: the pair is hidden around it
        assert_eq!(plain("<b> </b>"), " ");
        // a closing tag past the one that ended the style
        assert_eq!(plain("<b>x</b></b>"), "x</b>");
        // each style ends at its own closing tag, in whichever order they come
        assert_eq!(plain("<b><i>x</b></i>"), "x");
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
        // text in a tag's brackets is hidden, and no mention
        assert!(!mentions("<b @Ada x>hello", "Ada"));
        assert!(!mentions("hey <i @Ada>", "Ada"));
        // and the space a tag counts as completes no other tag
        assert!(!mentions("<b<i>>@Ada", "Ada"));
        assert!(!mentions("@Ada<b <i>>x", "Ada"));
        // whitespace as the HUD reads it
        assert!(mentions("hi\u{feff}@Ada", "Ada"));
        assert!(mentions("@Ada\u{feff}", "Ada"));
        assert!(!mentions("x\u{85}@Ada", "Ada"));
        assert!(!mentions("@Ada\u{85}", "Ada"));
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
        message_spans(&bubble_text(message, MAX_MESSAGE_CHARS))
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
    fn a_tag_ends_a_link_as_a_space_does() {
        // as `parseMessage` in chatParse.ts reads them: the tag is gone, its word break stays
        assert_eq!(
            colours("<b>@Ada</b>there"),
            [("@Ada".to_owned(), true), ("there".to_owned(), false)]
        );
        assert_eq!(
            colours("hi<b>@Ada</b>"),
            [("hi".to_owned(), false), ("@Ada".to_owned(), true)]
        );
        assert_eq!(
            colours("at<i>10,20</i>, then https://dcl.org/a<b>b"),
            [
                ("at".to_owned(), false),
                ("10,20".to_owned(), true),
                (", then ".to_owned(), false),
                ("https://dcl.org/a".to_owned(), true),
                ("b".to_owned(), false),
            ]
        );
        // a tag shown as written is a break on both sides too
        assert_eq!(
            colours("hi</b>@Ada"),
            [("hi</b>".to_owned(), false), ("@Ada".to_owned(), true)]
        );
        // whitespace as the HUD reads it
        assert_eq!(
            colours("hi\u{feff}@Ada"),
            [("hi\u{feff}".to_owned(), false), ("@Ada".to_owned(), true)]
        );
        // a break past the cut is dropped with the text
        let long = format!("{}<b>@Ada</b>", "x".repeat(MAX_MESSAGE_CHARS));
        assert_eq!(
            colours(&long),
            [(format!("{}…", "x".repeat(MAX_MESSAGE_CHARS)), false)]
        );
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

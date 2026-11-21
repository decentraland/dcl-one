//! The words of a collection's forum post, as builder-server writes them: a
//! title naming the collection and its creator, and a body that links the
//! review page and lists the first [`MAX_ITEMS`] items. Text a creator
//! chose (a collection's or an item's name, an item's description) is put
//! on one line with every Markdown control character escaped and every
//! mention, tag or emoji trigger broken, so it can't format, link or
//! notify.

use uuid::Uuid;

use super::slack::{Links, shorten};

/// The most items a post lists.
pub const MAX_ITEMS: usize = 20;

/// What a post says of one item.
#[derive(Clone, Debug, Default, sqlx::FromRow)]
pub struct PostItem {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub rarity: Option<String>,
    pub category: Option<String>,
    pub thumbnail_hash: Option<String>,
}

/// What a post says of its collection.
#[derive(Clone, Debug)]
pub struct PostCollection {
    pub id: Uuid,
    pub name: String,
    pub contract: Option<String>,
    pub items: Vec<PostItem>,
}

fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// Builder-server's `sanitizeForumText`.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        if js_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
            continue;
        }
        in_space = false;
        if "\\`*_{}[]()<>#+-.!|~&".contains(c) {
            out.push('\\');
        }
        out.push(c);
        if matches!(c, '@' | '#' | ':') {
            out.push('\u{200b}');
        }
    }
    out
}

/// Builder-server's `removeEmojis`, over the code points its UTF-16
/// pattern matches.
pub fn remove_emojis(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{2700}'..='\u{27bf}'
                    | '\u{e000}'..='\u{f8ff}'
                    | '\u{1f000}'..='\u{1f7ff}'
                    | '\u{2011}'..='\u{26ff}'
                    | '\u{1f910}'..='\u{1f9ff}'
            )
        })
        .collect()
}

fn category(text: &str) -> String {
    if !text.is_empty()
        && text.len() <= 32
        && text.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
    {
        text.to_string()
    } else {
        sanitize(text)
    }
}

/// The title of a standard collection's post, created by `creator`: the
/// creator's claimed name, else its shortened address.
pub fn title(collection: &str, creator: &str) -> String {
    format!("Collection '{collection}' created by {creator} is ready for review!")
}

/// The title retried after the forum refused `title` as already used.
pub fn retried_title(title: &str, contract: &str) -> String {
    format!("{title} {}", shorten(contract))
}

fn items_text(collection: &PostCollection, links: &Links, files: &str) -> String {
    collection
        .items
        .iter()
        .take(MAX_ITEMS)
        .map(|item| {
            let mut sections = Vec::new();
            if let Some(description) = item.description.as_deref().filter(|d| !d.is_empty()) {
                sections.push(format!("- Description: {}", sanitize(description)));
            }
            if let Some(rarity) = item.rarity.as_deref().filter(|r| !r.is_empty()) {
                sections.push(format!("- Rarity: {rarity}"));
            }
            if let Some(c) = item.category.as_deref().filter(|c| !c.is_empty()) {
                sections.push(format!("- Category: {}", category(c)));
            }
            format!(
                "**{}**\n      {}\n      ![]({}/v1/storage/contents/{})\n      [Link to editor]({})",
                sanitize(&item.name),
                sections.join("\n"),
                files.trim_end_matches('/'),
                item.thumbnail_hash.as_deref().unwrap_or_default(),
                links.item_editor(collection.id, item.id)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The body of a collection's post. A thumbnail links into the item files
/// under `files` (`BUILDER_SERVER_URL`).
pub fn body(collection: &PostCollection, links: &Links, files: &str) -> String {
    format!(
        "# {}\n\n  [View entire collection]({})\n\n  ## Wearables\n\n  {}",
        sanitize(&collection.name),
        links.editor(collection.id),
        items_text(collection, links, files)
    )
}

/// A third-party collection's post `old` with the items of `collection`
/// published on `date` (`M/D/YYYY`) appended.
pub fn update(
    old: &str,
    collection: &PostCollection,
    links: &Links,
    files: &str,
    date: &str,
) -> String {
    format!(
        "\n  {old}\n  \n\n\n## New wearables updated at {date}\n  {}\n  ",
        items_text(collection, links, files)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: Uuid = Uuid::from_u128(0x0401);
    const ITEM: Uuid = Uuid::from_u128(0x0402);

    fn links() -> Links {
        Links {
            creators_url: Some("https://creators.example.org/".into()),
            builder_url: None,
        }
    }

    #[test]
    fn the_body_is_builder_servers() {
        let collection = PostCollection {
            id: ID,
            name: "Hats".into(),
            contract: None,
            items: vec![
                PostItem {
                    id: ITEM,
                    name: "Red hat".into(),
                    description: Some("A hat".into()),
                    rarity: Some("common".into()),
                    category: Some("hat".into()),
                    thumbnail_hash: Some("bafkreithumb".into()),
                },
                PostItem {
                    id: ID,
                    name: "Bare".into(),
                    ..PostItem::default()
                },
            ],
        };
        let editor = "https://creators.example.org/collections/editor?collection=00000000-0000-0000-0000-000000000401";
        assert_eq!(
            body(&collection, &links(), "https://builder-api.example.org/"),
            format!(
                "# Hats\n\n  [View entire collection]({editor}&reviewing=true)\n\n  ## Wearables\n\n  \
                 **Red hat**\n      - Description: A hat\n- Rarity: common\n- Category: hat\n      \
                 ![](https://builder-api.example.org/v1/storage/contents/bafkreithumb)\n      \
                 [Link to editor]({editor}&item=00000000-0000-0000-0000-000000000402&reviewing=true)\n\n\
                 **Bare**\n      \n      ![](https://builder-api.example.org/v1/storage/contents/)\n      \
                 [Link to editor]({editor}&item=00000000-0000-0000-0000-000000000401&reviewing=true)"
            )
        );
    }

    #[test]
    fn a_post_lists_its_first_items_only() {
        let collection = PostCollection {
            id: ID,
            name: "Many".into(),
            contract: None,
            items: (0..MAX_ITEMS + 5)
                .map(|n| PostItem {
                    id: ITEM,
                    name: format!("item{n}"),
                    ..PostItem::default()
                })
                .collect(),
        };
        let text = body(&collection, &links(), "");
        assert_eq!(text.matches("[Link to editor]").count(), MAX_ITEMS);
        assert!(text.contains("**item19**") && !text.contains("**item20**"));
    }

    #[test]
    fn creator_text_cannot_format_link_mention_or_break_lines() {
        assert_eq!(
            sanitize("  @everyone [click](https://evil.example)\n# big  <b>x</b> :smile: &amp;"),
            " @\u{200b}everyone \\[click\\]\\(https:\u{200b}//evil\\.example\\) \\#\u{200b} big \\<b\\>x\\</b\\> :\u{200b}smile:\u{200b} \\&amp;"
        );
        assert_eq!(sanitize("a\u{2028}\u{3000}\u{feff}b"), "a b");
        let collection = PostCollection {
            id: ID,
            name: "x\n\n## Approved by the committee".into(),
            contract: None,
            items: vec![PostItem {
                id: ITEM,
                name: "y".into(),
                category: Some("hat](https://evil.example)".into()),
                ..PostItem::default()
            }],
        };
        let text = body(&collection, &links(), "");
        assert!(
            text.starts_with("# x \\#\u{200b}\\#\u{200b} Approved"),
            "{text}"
        );
        assert!(
            text.contains("- Category: hat\\]\\(https:\u{200b}//evil\\.example\\)"),
            "{text}"
        );
    }

    #[test]
    fn a_title_names_the_creator_and_loses_its_emojis() {
        assert_eq!(
            remove_emojis(&title("Hats 🎩✨", "0x1234...abcd")),
            "Collection 'Hats ' created by 0x1234...abcd is ready for review!"
        );
        assert_eq!(
            remove_emojis("a\u{2014}b\u{1f9d9}c\u{e000}d\u{2010}"),
            "abcd\u{2010}"
        );
        assert_eq!(
            retried_title("T", "0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e"),
            "T 0xc1ab...c70e"
        );
    }
}

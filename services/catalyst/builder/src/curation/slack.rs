//! The curators' Slack channel: builder-server's notices, for builder-server's
//! events, posted with Slack's `chat.postMessage` as its bot
//! (`SLACK_BOT_TOKEN`) to its channel (`SLACK_CURATION_CHANNEL`). A notice
//! names the collection by a link to its review page (`CREATORS_URL`, else
//! `BUILDER_URL`) and a person by a link to their page, labelled with the
//! shortened address.
//!
//! A notice is a side effect of a committed write: [`Notices::send`] runs it
//! after the transaction, in its own task, through the guarded client that
//! follows no redirect, under [`TIMEOUT`]; a failure is logged without the
//! token and never reaches the request. Untrusted text (a collection's name,
//! a comment, an appeal note) is put on one line, cut and made inert
//! ([`escape`]), so it can't mention, link or format. Without a token or a channel nothing is sent.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

/// Slack's Web API.
pub const API: &str = "https://slack.com/api";
/// How long one post may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// The most characters of a collection's name a notice quotes.
pub const MAX_NAME: usize = 300;

/// The bot and its channel.
#[derive(Clone)]
pub struct Slack {
    pub token: String,
    pub channel: String,
    pub api: String,
}

impl std::fmt::Debug for Slack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slack")
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

/// Where a notice's links point: the creators site, else the legacy builder.
#[derive(Clone, Debug, Default)]
pub struct Links {
    pub creators_url: Option<String>,
    pub builder_url: Option<String>,
}

/// What happened to a collection.
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// A standard collection's first publication, with its item count.
    Published { items: usize },
    /// A third-party collection's items submitted for curation, with their
    /// count.
    ThirdPartyPublished { items: usize },
    /// A creator asked for a review again.
    ReviewRequested { after_rejection: bool },
    /// The assignee changed; `None` is unassigned.
    Assigned { assignee: Option<String> },
    /// A curator decided.
    Verdict { approved: bool },
    /// The collection gets no automated review, for `reason`.
    ValidationNotRequested { reason: String },
    /// The validator passed the collection; `stale` items changed while it
    /// ran.
    AutomatedPassed { stale: usize },
    /// The validator can't check the items `unsupported`.
    Unsupported { unsupported: Vec<String> },
    /// The validator could not validate the collection in `validation`.
    ValidatorFailed { validation: String },
    /// The validator failed in `validation` and will be asked again.
    AutomatedFailed { validation: String },
    /// A creator asked a curator to look again, with `note`.
    AppealRequested { note: String },
    /// A creator commented `message`.
    CommentPosted { message: String },
}

/// The most characters of a comment a notice quotes.
pub const MAX_QUOTE: usize = 300;

fn quoted(text: &str) -> String {
    let line = one_line(text);
    if line.chars().count() > MAX_QUOTE {
        escape(
            &line
                .chars()
                .take(MAX_QUOTE)
                .chain(['…'])
                .collect::<String>(),
        )
    } else {
        escape(&line)
    }
}

/// Builder-server's `toOneLine`: each line trimmed, blank ones dropped,
/// joined by a space.
pub fn one_line(text: &str) -> String {
    text.split(['\r', '\n', '\u{2028}', '\u{2029}'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Untrusted text made inert in a `mrkdwn` message: builder-server's
/// `escapeSlackText` (`&`, `<`, `>` as entities, so no mention or link
/// markup), a zero-width space around each formatting mark (`*`, `_`, `~`,
/// `` ` ``) so no span forms, and after each `:`, `.` and `@` so no URL,
/// domain, address or emoji code is recognized.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '*' | '_' | '~' | '`' => {
                out.push(ZERO_WIDTH_SPACE);
                out.push(c);
                out.push(ZERO_WIDTH_SPACE);
            }
            ':' | '.' | '@' => {
                out.push(c);
                out.push(ZERO_WIDTH_SPACE);
            }
            c => out.push(c),
        }
    }
    out
}

const ZERO_WIDTH_SPACE: char = '\u{200B}';

fn base(url: &Option<String>) -> Option<&str> {
    url.as_deref()
        .map(|u| u.trim_end_matches('/'))
        .filter(|u| !u.is_empty())
}

impl Links {
    /// The review page of `collection`.
    pub fn editor(&self, collection: Uuid) -> String {
        match base(&self.creators_url) {
            Some(creators) => {
                format!("{creators}/collections/editor?collection={collection}&reviewing=true")
            }
            None => format!(
                "{}/item-editor?collection={collection}",
                base(&self.builder_url).unwrap_or_default()
            ),
        }
    }

    /// The review page of `item` in `collection`.
    pub fn item_editor(&self, collection: Uuid, item: Uuid) -> String {
        match base(&self.creators_url) {
            Some(creators) => format!(
                "{creators}/collections/editor?collection={collection}&item={item}&reviewing=true"
            ),
            None => format!(
                "{}/item-editor?collection={collection}&item={item}",
                base(&self.builder_url).unwrap_or_default()
            ),
        }
    }

    /// The origin of the creators site, which also serves the shop and the
    /// profiles; none when it isn't an http(s) URL.
    fn origin(&self) -> Option<String> {
        let url = url::Url::parse(self.creators_url.as_deref()?).ok()?;
        matches!(url.scheme(), "http" | "https").then(|| url.origin().ascii_serialization())
    }

    /// `address` as a link to its shop page (a creator) or its profile (a
    /// curator), labelled with the shortened address.
    fn person(&self, address: &str, curator: bool) -> String {
        let label = shorten(address);
        let Some(origin) = self.origin() else {
            return label;
        };
        let path = if curator {
            "/profile/"
        } else {
            "/shop/items/creator/"
        };
        format!("<{origin}{path}{}|{label}>", address.to_ascii_lowercase())
    }

    fn collection(&self, collection: Uuid, name: &str) -> String {
        let line = one_line(name);
        let quoted: String = if line.chars().count() > MAX_NAME {
            line.chars().take(MAX_NAME).chain(['…']).collect()
        } else {
            line
        };
        format!("<{}|{}>", self.editor(collection), escape(&quoted))
    }

    /// builder-server's `describeCollection`, which its review-flow notices
    /// name a collection by: the name quoted, the id, its creators page.
    fn described(&self, collection: Uuid, name: &str) -> String {
        let line = one_line(name);
        let quoted: String = if line.chars().count() > MAX_NAME {
            line.chars().take(MAX_NAME).chain(['…']).collect()
        } else {
            line
        };
        format!(
            "\"{}\" ({collection}) {}/collections/{collection}",
            escape(&quoted),
            self.creators_url.as_deref().unwrap_or_default()
        )
    }

    /// The text of `notice` about the collection `collection`, named `name`,
    /// sent by `actor`.
    pub fn text(&self, collection: Uuid, name: &str, actor: &str, notice: &Notice) -> String {
        let link = self.collection(collection, name);
        let described = self.described(collection, name);
        match notice {
            Notice::Published { items } => format!(
                "🎨 New collection ready for review: {link} by {} ({items} {}).",
                self.person(actor, false),
                if *items == 1 { "item" } else { "items" }
            ),
            Notice::ThirdPartyPublished { items } => format!(
                "📦 New Third Party items ready for review: {link} by {} ({items} {}).",
                self.person(actor, false),
                if *items == 1 { "item" } else { "items" }
            ),
            Notice::ReviewRequested {
                after_rejection: true,
            } => format!(
                "🔁 Review requested again for {link} by {} after a rejection.",
                self.person(actor, false)
            ),
            Notice::ReviewRequested {
                after_rejection: false,
            } => format!(
                "✏️ Changes submitted for review on {link} by {}.",
                self.person(actor, false)
            ),
            Notice::Assigned { assignee: None } => {
                format!("👤 {link} was unassigned by {}.", self.person(actor, true))
            }
            Notice::Assigned {
                assignee: Some(assignee),
            } if assignee.eq_ignore_ascii_case(actor) => {
                format!("👤 {link} was assigned to {}.", self.person(assignee, true))
            }
            Notice::Assigned {
                assignee: Some(assignee),
            } => format!(
                "👤 {link} was assigned to {} by {}.",
                self.person(assignee, true),
                self.person(actor, true)
            ),
            Notice::Verdict { approved } => format!(
                "{} {link} was {} by {}.",
                if *approved { "✅" } else { "❌" },
                if *approved { "approved" } else { "rejected" },
                self.person(actor, true)
            ),
            Notice::ValidationNotRequested { reason } => format!(
                "⚠️ No automated review for {link}: {reason}. It needs a curator review without it."
            ),
            Notice::AutomatedPassed { stale: 0 } => format!(
                "Automated review passed for collection {described}. It is ready for a curator approval."
            ),
            Notice::AutomatedPassed { stale } => format!(
                "Automated review passed for collection {described}, but {stale} item(s) changed while it ran, so it needs a new review."
            ),
            Notice::Unsupported { unsupported } => format!(
                "Collection {described} needs a curator review: the validator cannot check {} item(s) ({}).",
                unsupported.len(),
                unsupported.join(", ")
            ),
            Notice::ValidatorFailed { validation } => format!(
                "Collection {described} needs a curator review: the validator could not validate it (validation {validation})."
            ),
            Notice::AutomatedFailed { validation } => format!(
                "Automated review failed for collection {described} (validation {validation}). It will be re-sent automatically."
            ),
            Notice::AppealRequested { note } if note.is_empty() => {
                format!("Human review requested for collection {described}.")
            }
            Notice::AppealRequested { note } => format!(
                "Human review requested for collection {described}.\n> {}",
                escape(&one_line(note))
            ),
            Notice::CommentPosted { message } => format!(
                "💬 New comment on {link} by {}: {}",
                self.person(actor, false),
                quoted(message)
            ),
        }
    }
}

/// Builder-server's `shortenAddress`.
pub fn shorten(address: &str) -> String {
    let chars: Vec<char> = address.chars().collect();
    if chars.len() < 10 {
        return address.to_string();
    }
    let head: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}...{tail}")
}

/// The configured channel and the notices still being sent.
#[derive(Clone, Debug, Default)]
pub struct Notices {
    pub slack: Option<Slack>,
    pub links: Links,
    in_flight: Arc<AtomicUsize>,
}

impl Notices {
    pub fn new(slack: Option<Slack>, links: Links) -> Self {
        Self {
            slack,
            links,
            in_flight: Arc::default(),
        }
    }

    /// Posts `notice` in its own task through `client`; nothing without a
    /// channel.
    pub fn send(
        &self,
        client: &dcl_http::Outbound,
        collection: Uuid,
        name: &str,
        actor: &str,
        notice: Notice,
    ) {
        let Some(slack) = self.slack.clone() else {
            return;
        };
        let text = self.links.text(collection, name, actor, &notice);
        let client = client.clone();
        let held = InFlight::hold(&self.in_flight);
        tokio::spawn(async move {
            post(&client, &slack, &text).await;
            drop(held);
        });
    }

    /// Waits until every notice sent so far has finished.
    pub async fn settled(&self) {
        while self.in_flight.load(Ordering::SeqCst) > 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

/// One notice counted in flight until it is dropped, its task's panic
/// included.
struct InFlight(Arc<AtomicUsize>);

impl InFlight {
    fn hold(count: &Arc<AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count.clone())
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn post(client: &dcl_http::Outbound, slack: &Slack, text: &str) {
    let sent = client
        .post(format!(
            "{}/chat.postMessage",
            slack.api.trim_end_matches('/')
        ))
        .header("authorization", format!("Bearer {}", slack.token))
        .timeout(TIMEOUT)
        .json(&json!({
            "channel": slack.channel,
            "text": text,
            "unfurl_links": false,
            "unfurl_media": false,
        }))
        .send()
        .await;
    match sent {
        Ok(response) => {
            let status = response.status();
            let answer: Option<serde_json::Value> = response.json().await.ok();
            let ok = answer.as_ref().and_then(|a| a["ok"].as_bool()) == Some(true);
            if !status.is_success() || !ok {
                let error = answer
                    .as_ref()
                    .and_then(|a| a["error"].as_str())
                    .unwrap_or_default()
                    .chars()
                    .take(64)
                    .collect::<String>();
                tracing::warn!(%status, %error, channel = %slack.channel, "Slack refused a curation notice");
            }
        }
        Err(error) => {
            tracing::warn!(
                connect = error.is_connect(),
                timeout = error.is_timeout(),
                channel = %slack.channel,
                "a curation notice did not reach Slack"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links() -> Links {
        Links {
            creators_url: Some("https://creators.example.org/create/".into()),
            builder_url: None,
        }
    }

    const ID: Uuid = Uuid::from_u128(0x0302);
    const CURATOR: &str = "0x01eba3f0e992f53debaf51ed3738dc2f23e09aee";
    const CREATOR: &str = "0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e";

    #[test]
    fn the_texts_are_builder_servers() {
        let l = links();
        let link = "<https://creators.example.org/create/collections/editor?collection=00000000-0000-0000-0000-000000000302&reviewing=true|Hats>";
        let curator = "<https://creators.example.org/profile/0x01eba3f0e992f53debaf51ed3738dc2f23e09aee|0x01eb...9aee>";
        let creator = "<https://creators.example.org/shop/items/creator/0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e|0xc1ab...c70e>";
        for (notice, actor, text) in [
            (
                Notice::Published { items: 1 },
                CREATOR,
                format!("🎨 New collection ready for review: {link} by {creator} (1 item)."),
            ),
            (
                Notice::ReviewRequested {
                    after_rejection: true,
                },
                CREATOR,
                format!("🔁 Review requested again for {link} by {creator} after a rejection."),
            ),
            (
                Notice::ReviewRequested {
                    after_rejection: false,
                },
                CREATOR,
                format!("✏️ Changes submitted for review on {link} by {creator}."),
            ),
            (
                Notice::Assigned { assignee: None },
                CURATOR,
                format!("👤 {link} was unassigned by {curator}."),
            ),
            (
                Notice::Assigned {
                    assignee: Some(CURATOR.into()),
                },
                CURATOR,
                format!("👤 {link} was assigned to {curator}."),
            ),
            (
                Notice::Assigned {
                    assignee: Some(CURATOR.into()),
                },
                CREATOR,
                format!(
                    "👤 {link} was assigned to {curator} by <https://creators.example.org/profile/{CREATOR}|0xc1ab...c70e>."
                ),
            ),
            (
                Notice::Verdict { approved: false },
                CURATOR,
                format!("❌ {link} was rejected by {curator}."),
            ),
        ] {
            assert_eq!(l.text(ID, "Hats", actor, &notice), text);
        }
    }

    #[test]
    fn a_hostile_name_cannot_mention_link_format_or_forge_a_line() {
        let hostile = "<!channel> <https://evil.example|click> & *bold* _it_ ~gone~ `code` \
                       ```block``` https://evil.example/login www.evil.example jane.doe@example.com \
                       :rotating_light: <@U123> <#C123>\n✅ forged line\n> quoted";
        assert!(hostile.chars().count() < MAX_QUOTE);
        let inert = escape(&one_line(hostile));
        for form in [
            "<",
            ">",
            "*bold*",
            "_it_",
            "~gone~",
            "`code`",
            "```",
            "https://",
            "www.evil",
            "evil.example",
            "example.com",
            ":rotating_light:",
            "\n",
        ] {
            assert!(!inert.contains(form), "{form:?} in {inert}");
        }
        let l = links();
        for (notice, name) in [
            (Notice::Verdict { approved: true }, hostile),
            (
                Notice::CommentPosted {
                    message: hostile.into(),
                },
                "Hats",
            ),
            (
                Notice::AppealRequested {
                    note: hostile.into(),
                },
                hostile,
            ),
        ] {
            let text = l.text(ID, name, CREATOR, &notice);
            assert!(text.contains(&inert), "{text}");
            assert_eq!(
                text.matches('\n').count(),
                usize::from(matches!(notice, Notice::AppealRequested { .. })),
                "{text}"
            );
        }
        let link = l.text(ID, "Hats", CURATOR, &Notice::Verdict { approved: true });
        assert!(
            link.starts_with("✅ <https://creators.example.org/")
                && link.contains(
                    "reviewing=true|Hats> was approved by <https://creators.example.org/profile/"
                ),
            "the node's own links stay links: {link}"
        );
        let long = "x".repeat(MAX_NAME + 50);
        let text = l.text(ID, &long, CURATOR, &Notice::Verdict { approved: true });
        assert!(
            text.contains(&format!("|{}…>", "x".repeat(MAX_NAME))),
            "{text}"
        );
    }

    #[test]
    fn without_a_creators_site_the_legacy_editor_and_bare_labels_are_named() {
        let l = Links {
            creators_url: None,
            builder_url: Some("https://builder.example.org".into()),
        };
        assert_eq!(
            l.text(ID, "Hats", CURATOR, &Notice::Verdict { approved: true }),
            "✅ <https://builder.example.org/item-editor?collection=00000000-0000-0000-0000-000000000302|Hats> was approved by 0x01eb...9aee."
        );
    }
}

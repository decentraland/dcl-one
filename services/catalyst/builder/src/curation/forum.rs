//! The forum the curators review on: a Discourse site (`FORUM_URL`), posted
//! to with its API key (`FORUM_API_KEY`) as its user (`FORUM_API_USERNAME`),
//! a collection's topic in its category (`FORUM_CATEGORY`), the calls
//! builder-server makes. Every call goes through the guarded client that
//! follows no redirect, under [`TIMEOUT`], and reads at most [`MAX_ANSWER`]
//! bytes of the answer; the key travels only in its header and is never
//! logged. A link the node stores is built from the configured forum and
//! the forum's validated answer, so it is always under that forum
//! ([`Forum::holds`]).

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::http::ApiError;

/// How long one forum call may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// The most bytes of a forum answer the node reads.
pub const MAX_ANSWER: usize = 1024 * 1024;
/// The longest forum link a collection keeps.
pub const MAX_LINK: usize = 2048;
/// The error Discourse answers for a title another topic has.
const DUPLICATED_TITLE: &str = "Title has already been used";

/// The forum and the account the node posts as.
#[derive(Clone)]
pub struct Forum {
    /// The forum's origin, without a trailing slash.
    pub url: String,
    /// Where the forum's API is called: its origin.
    pub api: String,
    pub api_key: String,
    pub api_username: String,
    /// The category a collection's topic is created in.
    pub category: String,
}

impl std::fmt::Debug for Forum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forum")
            .field("url", &self.url)
            .field("api_username", &self.api_username)
            .field("category", &self.category)
            .finish_non_exhaustive()
    }
}

/// A topic the node made or found: its first post's id, its id and slug.
#[derive(Clone, Debug, PartialEq)]
pub struct Topic {
    pub post: i32,
    pub id: i64,
    pub slug: String,
}

/// What became of a topic the node asked the forum to create.
#[derive(Debug, PartialEq)]
pub enum Created {
    Topic(Topic),
    /// Another topic has the title.
    DuplicateTitle,
}

/// Why a forum call gave no answer the node can use.
#[derive(Debug, PartialEq)]
pub enum Failure {
    /// The forum answered and refused.
    Refused(StatusCode),
    /// No answer arrived, or not all of it: the forum may have acted.
    Unreachable,
    /// The answer isn't what the forum answers.
    Invalid,
}

impl Failure {
    /// The refusal a request gets: the forum's own words never reach it.
    pub fn api_error(&self, id: impl std::fmt::Display) -> ApiError {
        ApiError::http(500, "Error creating forum post").with_data(
            json!({ "id": id.to_string(), "errors": "The forum did not accept the post" }),
        )
    }
}

impl Forum {
    /// The forum from its four settings; `None` unless all are set.
    pub fn parse(
        url: Option<String>,
        api_key: Option<String>,
        api_username: Option<String>,
        category: Option<String>,
    ) -> anyhow::Result<Option<Self>> {
        let (Some(url), Some(api_key), Some(api_username), Some(category)) =
            (url, api_key, api_username, category)
        else {
            return Ok(None);
        };
        let origin = url.strip_suffix('/').unwrap_or(&url).to_string();
        let parsed = url::Url::parse(&origin).ok();
        let is_origin = parsed.as_ref().is_some_and(|u| {
            u.scheme() == "https"
                && u.username().is_empty()
                && u.password().is_none()
                && u.path() == "/"
                && u.query().is_none()
                && u.fragment().is_none()
                && u.origin().ascii_serialization() == origin
        });
        if !is_origin || !dcl_http::is_safe_http_url(&origin) {
            anyhow::bail!(
                "FORUM_URL must be a public https origin, such as https://forum.example.test: it carries a key"
            );
        }
        if category.is_empty()
            || category.len() > 9
            || !category.bytes().all(|b| b.is_ascii_digit())
        {
            anyhow::bail!("FORUM_CATEGORY must be a forum category id");
        }
        if !api_username.chars().all(|c| c.is_ascii_graphic()) {
            anyhow::bail!("FORUM_API_USERNAME must be a forum username");
        }
        if !api_key.chars().all(|c| c.is_ascii_graphic()) {
            anyhow::bail!("FORUM_API_KEY must be printable ASCII");
        }
        Ok(Some(Self {
            api: origin.clone(),
            url: origin,
            api_key,
            api_username,
            category,
        }))
    }

    /// The link of `topic`: under the forum, with its slug when the slug is
    /// a plain path segment.
    pub fn link(&self, topic: &Topic) -> String {
        let plain = !topic.slug.is_empty()
            && topic.slug.len() <= 255
            && topic
                .slug
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'%'));
        let slug = if plain { topic.slug.as_str() } else { "topic" };
        format!("{}/t/{slug}/{}", self.url, topic.id)
    }

    /// Whether `link` is a link the node keeps for this forum.
    pub fn holds(&self, link: &str) -> bool {
        under(&self.url, link)
    }
}

/// Whether `link` is under the forum at `origin`: the origin followed by
/// `/`, at most [`MAX_LINK`] characters, no space or control character.
pub fn under(origin: &str, link: &str) -> bool {
    link.chars().count() <= MAX_LINK
        && link
            .strip_prefix(origin)
            .is_some_and(|rest| rest.starts_with('/'))
        && !link.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn positive(value: &Value) -> Option<i64> {
    value.as_i64().filter(|n| *n > 0)
}

fn post_id(value: &Value) -> Option<i32> {
    positive(value).and_then(|n| i32::try_from(n).ok())
}

async fn answer(response: reqwest::Response) -> Result<(StatusCode, Option<Value>), Failure> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_ANSWER as u64)
    {
        return Err(Failure::Invalid);
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unreachable)? {
        if bytes.len() + chunk.len() > MAX_ANSWER {
            return Err(Failure::Invalid);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, serde_json::from_slice(&bytes).ok()))
}

fn errors(body: &Option<Value>) -> Option<&Vec<Value>> {
    body.as_ref()
        .and_then(|b| b.get("errors"))
        .and_then(Value::as_array)
}

impl Forum {
    fn request(
        &self,
        client: &dcl_http::Outbound,
        method: reqwest::Method,
        path: &str,
        as_user: bool,
    ) -> dcl_http::GuardedRequest {
        let request = client
            .request(method, format!("{}{path}", self.api))
            .header("Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .timeout(TIMEOUT)
            .max_redirects(0);
        if as_user {
            request.header("Api-Username", &self.api_username)
        } else {
            request
        }
    }

    async fn send(
        &self,
        request: dcl_http::GuardedRequest,
        call: &'static str,
    ) -> Result<(StatusCode, Option<Value>), Failure> {
        let response = request.send().await.map_err(|error| {
            tracing::warn!(
                call,
                timeout = error.is_timeout(),
                connect = error.is_connect(),
                "a forum call got no answer"
            );
            Failure::Unreachable
        })?;
        let answered = answer(response).await;
        if let Ok((status, _)) = &answered
            && !status.is_success()
            && !status.is_redirection()
        {
            tracing::warn!(call, %status, "the forum refused a call");
        }
        answered
    }

    /// Creates a topic in the forum's category.
    pub async fn create(
        &self,
        client: &dcl_http::Outbound,
        title: &str,
        raw: &str,
    ) -> Result<Created, Failure> {
        let request = self
            .request(client, reqwest::Method::POST, "/posts.json", false)
            .json(&json!({
                "title": title,
                "raw": raw,
                "category": self.category,
            }));
        let (status, body) = self.send(request, "create topic").await?;
        if errors(&body).is_some_and(|errors| {
            errors
                .iter()
                .any(|e| e.as_str().is_some_and(|e| e.contains(DUPLICATED_TITLE)))
        }) {
            return Ok(Created::DuplicateTitle);
        }
        if !status.is_success() || errors(&body).is_some() {
            return Err(Failure::Refused(status));
        }
        let body = body.ok_or(Failure::Invalid)?;
        Ok(Created::Topic(Topic {
            post: post_id(&body["id"]).ok_or(Failure::Invalid)?,
            id: positive(&body["topic_id"]).ok_or(Failure::Invalid)?,
            slug: body["topic_slug"]
                .as_str()
                .ok_or(Failure::Invalid)?
                .to_string(),
        }))
    }

    /// The topic of the post `post`; `None` when the forum names none.
    pub async fn topic_of(
        &self,
        client: &dcl_http::Outbound,
        post: i32,
    ) -> Result<Option<i64>, Failure> {
        let request = self.request(
            client,
            reqwest::Method::GET,
            &format!("/posts/{post}.json"),
            true,
        );
        let (status, body) = self.send(request, "read post").await?;
        if !status.is_success() {
            return Err(Failure::Refused(status));
        }
        let body = body.ok_or(Failure::Invalid)?;
        Ok(positive(&body["topic_id"]))
    }

    /// The text of the post `post`.
    pub async fn raw_of(&self, client: &dcl_http::Outbound, post: i32) -> Result<String, Failure> {
        let request = self.request(
            client,
            reqwest::Method::GET,
            &format!("/posts/{post}.json"),
            true,
        );
        let (status, body) = self.send(request, "read post").await?;
        if !status.is_success() {
            return Err(Failure::Refused(status));
        }
        body.as_ref()
            .and_then(|b| b["raw"].as_str())
            .map(str::to_string)
            .ok_or(Failure::Invalid)
    }

    /// Replaces the text of the post `post` with `raw`.
    pub async fn update(
        &self,
        client: &dcl_http::Outbound,
        post: i32,
        raw: &str,
    ) -> Result<(), Failure> {
        let request = self
            .request(
                client,
                reqwest::Method::PUT,
                &format!("/posts/{post}.json"),
                true,
            )
            .json(&json!({ "raw": raw }));
        let (status, body) = self.send(request, "update post").await?;
        if !status.is_success() || errors(&body).is_some() {
            return Err(Failure::Refused(status));
        }
        Ok(())
    }

    /// Replies `raw` in the topic `topic`.
    pub async fn reply(
        &self,
        client: &dcl_http::Outbound,
        topic: i64,
        raw: &str,
    ) -> Result<(), Failure> {
        let request = self
            .request(client, reqwest::Method::POST, "/posts.json", false)
            .json(&json!({ "topic_id": topic, "raw": raw }));
        let (status, body) = self.send(request, "reply").await?;
        if !status.is_success() || errors(&body).is_some() {
            return Err(Failure::Refused(status));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forum() -> Forum {
        Forum {
            url: "https://forum.example.test".into(),
            api: "https://forum.example.test".into(),
            api_key: "sk-example123".into(),
            api_username: "builder".into(),
            category: "12".into(),
        }
    }

    fn parsed(url: &str, category: &str) -> anyhow::Result<Option<Forum>> {
        Forum::parse(
            Some(url.into()),
            Some("sk-example123".into()),
            Some("builder".into()),
            Some(category.into()),
        )
    }

    #[test]
    fn the_forum_is_a_public_origin_with_a_category_id_and_hides_its_key() {
        let forum = parsed("https://forum.example.test/", "12")
            .unwrap()
            .unwrap();
        assert_eq!(
            (forum.url.as_str(), forum.api.as_str()),
            ("https://forum.example.test", "https://forum.example.test")
        );
        assert!(!format!("{forum:?}").contains("sk-example123"));
        for url in [
            "https://forum.example.test/forum",
            "https://forum.example.test/?a=1",
            "https://user:pw@forum.example.test",
            "ftp://forum.example.test",
            "http://forum.example.test",
            "https://127.0.0.1",
            "http://localhost",
            "forum.example.test",
        ] {
            assert!(parsed(url, "12").is_err(), "{url}");
        }
        for category in ["", "abc", "1 2", "1234567890"] {
            assert!(
                parsed("https://forum.example.test", category).is_err(),
                "{category:?}"
            );
        }
        assert!(
            Forum::parse(
                Some("https://forum.example.test".into()),
                None,
                Some("b".into()),
                Some("1".into())
            )
            .unwrap()
            .is_none()
        );
        let err = Forum::parse(
            Some("https://forum.example.test".into()),
            Some("sk-example123\r\nx: y".into()),
            Some("b".into()),
            Some("1".into()),
        )
        .unwrap_err()
        .to_string();
        assert!(!err.contains("sk-example123"), "{err}");
    }

    #[test]
    fn a_link_is_built_under_the_forum_whatever_the_slug() {
        let f = forum();
        let topic = |slug: &str| Topic {
            post: 7,
            id: 41,
            slug: slug.into(),
        };
        assert_eq!(
            f.link(&topic("red-hats")),
            "https://forum.example.test/t/red-hats/41"
        );
        for hostile in [
            "",
            "../../x",
            "a/b",
            "x?y",
            "x#y",
            "@evil.example",
            "a b",
            "\u{e9}",
            &"a".repeat(256),
        ] {
            assert_eq!(
                f.link(&topic(hostile)),
                "https://forum.example.test/t/topic/41",
                "{hostile:?}"
            );
        }
    }

    #[test]
    fn only_a_link_under_the_forum_is_held() {
        let f = forum();
        assert!(f.holds("https://forum.example.test/t/hat/1"));
        for link in [
            "https://forum.example.test",
            "https://forum.example.test.evil.example/t/1",
            "https://forum.example.testx/t/1",
            "http://forum.example.test/t/1",
            "https://evil.example/t/1",
            "https://forum.example.test/t/a b",
            "https://forum.example.test/t/\n",
            "javascript:alert(1)",
        ] {
            assert!(!f.holds(link), "{link}");
        }
        assert!(!f.holds(&format!(
            "https://forum.example.test/{}",
            "a".repeat(MAX_LINK)
        )));
    }

    async fn serving(body: String, status: axum::http::StatusCode) -> Forum {
        let app = axum::Router::new().fallback(move || {
            let body = body.clone();
            async move { (status, body) }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut forum = forum();
        forum.api = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        forum
    }

    #[tokio::test]
    async fn forum_answers_have_positive_ids_and_a_bounded_body() {
        let client = dcl_http::Outbound::for_tests();
        for (post, topic) in [(0, 1), (-1, 1), (i32::MAX as i64 + 1, 1), (1, 0), (1, -1)] {
            let f = serving(
                json!({"id":post,"topic_id":topic,"topic_slug":"hats"}).to_string(),
                StatusCode::OK,
            )
            .await;
            assert_eq!(
                f.create(&client, "hats", "body").await,
                Err(Failure::Invalid)
            );
        }
        let f = serving(" ".repeat(1024 * 1024 + 1), StatusCode::OK).await;
        assert_eq!(
            f.create(&client, "hats", "body").await,
            Err(Failure::Invalid)
        );
        let f = serving(
            json!({"id":1,"topic_id":2,"topic_slug":"hats","padding":" ".repeat(1024 * 1024)})
                .to_string(),
            StatusCode::OK,
        )
        .await;
        assert_eq!(
            f.create(&client, "hats", "body").await,
            Err(Failure::Invalid)
        );
        let f = serving(
            json!({"id":1,"topic_id":2,"topic_slug":"hats"}).to_string(),
            StatusCode::FORBIDDEN,
        )
        .await;
        assert_eq!(
            f.create(&client, "hats", "body").await,
            Err(Failure::Refused(StatusCode::FORBIDDEN))
        );
    }
}

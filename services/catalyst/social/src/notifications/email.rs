//! Outbound mail: the confirmation email of the set-email flow and the
//! `email` delivery channel of the outbox worker, both SendGrid dynamic
//! templates over the ONE outbound client (SendGrid and Turnstile are fixed
//! public hosts, so the guard costs nothing). Unconfigured means refused
//! loudly for the confirmation (the caller answers 500) and `skipped` for a
//! delivery row: never a silent success.

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::errors::ApiError;
use crate::http::Outbound;

pub const CODE_LEN: usize = 32;
const SENDGRID_SEND_URL: &str = "https://api.sendgrid.com/v3/mail/send";
const TURNSTILE_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/// 32 hex characters (a v4 uuid: 122 random bits); the old crate drew 32
/// alphanumerics from `rand`, which v2 does not otherwise depend on. The
/// confirm route checks length and equality only.
pub fn make_code() -> String {
    Uuid::new_v4().simple().to_string()
}

#[derive(Clone, Debug, Default)]
pub struct EmailConfig {
    pub sendgrid_api_key: Option<String>,
    pub from_email: Option<String>,
    pub validate_email_template_id: Option<String>,
    pub validate_credits_email_template_id: Option<String>,
    /// The dynamic template of a delivered notification (the outbox's
    /// `email` channel); unset, every delivery row ends `skipped`.
    pub notification_template_id: Option<String>,
    pub account_base_url: String,
    pub marketplace_base_url: String,
    pub turnstile_secret_key: Option<String>,
    pub domain_blacklist: Vec<String>,
}

impl EmailConfig {
    /// `EMAIL_DOMAIN_BLACKLIST`: comma-separated, lowercased, blanks dropped.
    pub fn parse_domain_blacklist(raw: &str) -> Vec<String> {
        raw.split(',')
            .map(|d| d.trim().to_lowercase())
            .filter(|d| !d.is_empty())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailSource {
    Account,
    Credits,
}

impl EmailSource {
    pub fn as_str(self) -> &'static str {
        match self {
            EmailSource::Account => "account",
            EmailSource::Credits => "credits",
        }
    }

    pub fn from_credits_workflow(is_credits: bool) -> Self {
        if is_credits {
            EmailSource::Credits
        } else {
            EmailSource::Account
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "credits" => EmailSource::Credits,
            _ => EmailSource::Account,
        }
    }
}

/// Why a delivery send did not happen; decides retry vs terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendFailure {
    /// Nothing to send with: no key, sender or template.
    NotConfigured,
    /// Transport or 5xx/429: retry later.
    Transient(String),
    /// The provider refused the message (4xx): retrying cannot help.
    Permanent(String),
}

#[derive(Clone)]
pub struct EmailSender {
    cfg: EmailConfig,
    http: Outbound,
}

impl EmailSender {
    pub fn new(cfg: EmailConfig, http: Outbound) -> Self {
        Self { cfg, http }
    }

    pub fn is_enabled(&self, source: EmailSource) -> bool {
        self.cfg.sendgrid_api_key.is_some()
            && self.cfg.from_email.is_some()
            && self.template_id(source).is_some()
    }

    /// Key, sender and the notification template are all set.
    pub fn notifications_configured(&self) -> bool {
        self.cfg.sendgrid_api_key.is_some()
            && self.cfg.from_email.is_some()
            && self.cfg.notification_template_id.is_some()
    }

    pub fn is_domain_blacklisted(&self, email: &str) -> bool {
        if self.cfg.domain_blacklist.is_empty() {
            return false;
        }
        match email.rsplit_once('@') {
            Some((_, domain)) => self
                .cfg
                .domain_blacklist
                .iter()
                .any(|d| d == &domain.to_lowercase()),
            None => false,
        }
    }

    pub fn turnstile_secret(&self) -> Option<&str> {
        self.cfg.turnstile_secret_key.as_deref()
    }

    fn template_id(&self, source: EmailSource) -> Option<&str> {
        match source {
            EmailSource::Account => self.cfg.validate_email_template_id.as_deref(),
            EmailSource::Credits => self.cfg.validate_credits_email_template_id.as_deref(),
        }
    }

    fn base_url(&self, source: EmailSource) -> &str {
        match source {
            EmailSource::Account => &self.cfg.account_base_url,
            EmailSource::Credits => &self.cfg.marketplace_base_url,
        }
    }

    fn confirm_path(source: EmailSource, code: &str) -> String {
        match source {
            EmailSource::Account => format!("/confirm-email-challenge/{code}"),
            EmailSource::Credits => format!("/credits-email-confirmed/{code}"),
        }
    }

    pub fn confirm_url(&self, source: EmailSource, address: &str, code: &str) -> String {
        format!(
            "{}{}?address={}&source={}",
            self.base_url(source).trim_end_matches('/'),
            Self::confirm_path(source, code),
            address,
            source.as_str()
        )
    }

    async fn sendgrid(&self, api_key: &str, body: &Json) -> Result<Option<String>, SendFailure> {
        let resp = self
            .http
            .post(SENDGRID_SEND_URL)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .json(body)
            .send()
            .await
            .map_err(|e| SendFailure::Transient(format!("sendgrid request failed: {e}")))?;
        let status = resp.status();
        if status.is_success() {
            let message_id = resp
                .headers()
                .get("x-message-id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            return Ok(message_id);
        }
        let detail = resp.text().await.unwrap_or_default();
        let detail: String = detail.chars().take(200).collect();
        tracing::error!(status = %status, detail = %detail, "SendGrid send failed");
        if status.as_u16() == 429 || status.is_server_error() {
            Err(SendFailure::Transient(format!("sendgrid {status}")))
        } else {
            Err(SendFailure::Permanent(format!(
                "sendgrid {status}: {detail}"
            )))
        }
    }

    pub async fn send_confirmation(
        &self,
        source: EmailSource,
        to: &str,
        address: &str,
        code: &str,
    ) -> Result<Option<String>, ApiError> {
        let confirm_url = self.confirm_url(source, address, code);
        let (Some(api_key), Some(from), Some(template_id)) = (
            self.cfg.sendgrid_api_key.as_deref(),
            self.cfg.from_email.as_deref(),
            self.template_id(source),
        ) else {
            tracing::error!(to = %to, source = source.as_str(), confirm_url = %confirm_url, "SendGrid not configured; cannot send confirmation email");
            return Err(ApiError::Internal(
                "email delivery is not configured".into(),
            ));
        };
        let body = json!({
            "personalizations": [{
                "to": [{ "email": to }],
                "dynamic_template_data": {
                    "confirmUrl": confirm_url,
                    "code": code,
                    "address": address,
                    "source": source.as_str(),
                }
            }],
            "from": { "email": from },
            "template_id": template_id,
        });
        match self.sendgrid(api_key, &body).await {
            Ok(message_id) => {
                tracing::info!(to = %to, source = source.as_str(), ?message_id, "confirmation email sent");
                Ok(message_id)
            }
            Err(SendFailure::Transient(e)) => Err(ApiError::Internal(e)),
            Err(SendFailure::Permanent(_)) => Err(ApiError::Internal(
                "failed to send confirmation email".into(),
            )),
            Err(SendFailure::NotConfigured) => Err(ApiError::Internal(
                "email delivery is not configured".into(),
            )),
        }
    }

    /// Key and sender are set: a code-rendered message can go out.
    pub fn html_configured(&self) -> bool {
        self.cfg.sendgrid_api_key.is_some() && self.cfg.from_email.is_some()
    }

    /// One code-rendered message (a subject, an HTML body and its text
    /// twin, the Shop's availability alert) through the same key and
    /// sender; `custom_args` ride on the personalization so a redelivery
    /// is recognisable. The provider's message id when it gives one.
    pub async fn send_html(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        text: &str,
        custom_args: &Json,
    ) -> Result<Option<String>, SendFailure> {
        let (Some(api_key), Some(from)) = (
            self.cfg.sendgrid_api_key.as_deref(),
            self.cfg.from_email.as_deref(),
        ) else {
            return Err(SendFailure::NotConfigured);
        };
        let body = json!({
            "personalizations": [{ "to": [{ "email": to }], "custom_args": custom_args }],
            "from": { "email": from },
            "subject": subject,
            "content": [
                { "type": "text/plain", "value": text },
                { "type": "text/html", "value": html },
            ],
        });
        self.sendgrid(api_key, &body).await
    }

    /// One notification through the notification template; the id rides in
    /// `custom_args` so a redelivery after a crash is recognisable.
    pub async fn send_notification(
        &self,
        to: &str,
        address: &str,
        notification_id: Uuid,
        kind: &str,
        metadata: &Json,
    ) -> Result<(), SendFailure> {
        let (Some(api_key), Some(from), Some(template_id)) = (
            self.cfg.sendgrid_api_key.as_deref(),
            self.cfg.from_email.as_deref(),
            self.cfg.notification_template_id.as_deref(),
        ) else {
            return Err(SendFailure::NotConfigured);
        };
        let body = json!({
            "personalizations": [{
                "to": [{ "email": to }],
                "custom_args": { "notification_id": notification_id.to_string() },
                "dynamic_template_data": {
                    "address": address,
                    "type": kind,
                    "notificationId": notification_id.to_string(),
                    "metadata": metadata,
                }
            }],
            "from": { "email": from },
            "template_id": template_id,
        });
        self.sendgrid(api_key, &body).await.map(|_| ())
    }
}

/// No secret configured passes (the check is off); a missing token fails;
/// otherwise Cloudflare's verdict.
pub async fn verify_turnstile(
    http: &Outbound,
    secret: Option<&str>,
    token: Option<&str>,
) -> Result<bool, ApiError> {
    let Some(secret) = secret else {
        return Ok(true);
    };
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return Ok(false);
    };
    let form = format!(
        "secret={}&response={}",
        urlencoding::encode(secret),
        urlencoding::encode(token)
    );
    let resp = http
        .post(TURNSTILE_VERIFY_URL)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("turnstile request failed: {e}")))?;
    let body: Json = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("turnstile decode failed: {e}")))?;
    Ok(body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sender() -> EmailSender {
        EmailSender::new(
            EmailConfig {
                account_base_url: "https://account.decentraland.org".into(),
                marketplace_base_url: "https://decentraland.org/marketplace".into(),
                ..Default::default()
            },
            Outbound::for_tests(),
        )
    }

    const ADDR: &str = "0x1234567890abcdef1234567890abcdef12345678";
    const CODE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef";

    #[test]
    fn account_confirm_url_uses_challenge_route() {
        let url = sender().confirm_url(EmailSource::Account, ADDR, CODE);
        assert_eq!(
            url,
            format!(
                "https://account.decentraland.org/confirm-email-challenge/{CODE}?address={ADDR}&source=account"
            )
        );
        assert!(
            !url.contains("/confirm-email/"),
            "must not use legacy route"
        );
    }

    #[test]
    fn credits_confirm_url_uses_credits_route() {
        let url = sender().confirm_url(EmailSource::Credits, ADDR, CODE);
        assert_eq!(
            url,
            format!(
                "https://decentraland.org/marketplace/credits-email-confirmed/{CODE}?address={ADDR}&source=credits"
            )
        );
    }

    #[test]
    fn base_url_trailing_slash_trimmed() {
        let s = EmailSender::new(
            EmailConfig {
                account_base_url: "https://account.decentraland.org/".into(),
                ..Default::default()
            },
            Outbound::for_tests(),
        );
        let url = s.confirm_url(EmailSource::Account, ADDR, CODE);
        assert!(url.starts_with("https://account.decentraland.org/confirm-email-challenge/"));
        assert!(!url.contains(".org//"));
    }

    #[tokio::test]
    async fn unconfigured_mailer_hard_fails() {
        let err = sender()
            .send_confirmation(EmailSource::Account, "user@example.com", ADDR, CODE)
            .await
            .expect_err("unconfigured mailer must error, never silently succeed");
        assert!(matches!(err, ApiError::Internal(_)));
        assert_eq!(
            sender()
                .send_notification(
                    "user@example.com",
                    ADDR,
                    Uuid::nil(),
                    "bid_received",
                    &json!({})
                )
                .await,
            Err(SendFailure::NotConfigured)
        );
        assert!(!sender().notifications_configured());
    }

    #[test]
    fn code_is_32_chars_and_unique() {
        let a = make_code();
        assert_eq!(a.len(), CODE_LEN);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(a, make_code());
    }

    #[test]
    fn domain_blacklist_parses_and_matches_case_insensitively() {
        let s = EmailSender::new(
            EmailConfig {
                domain_blacklist: EmailConfig::parse_domain_blacklist(" Spam.com, ,junk.io"),
                ..Default::default()
            },
            Outbound::for_tests(),
        );
        assert!(s.is_domain_blacklisted("x@SPAM.com"));
        assert!(s.is_domain_blacklisted("x@junk.io"));
        assert!(!s.is_domain_blacklisted("x@fine.org"));
        assert!(!s.is_domain_blacklisted("no-at-sign"));
        assert!(
            !sender().is_domain_blacklisted("x@spam.com"),
            "an empty list refuses nothing"
        );
    }

    #[tokio::test]
    async fn turnstile_without_a_secret_passes_and_without_a_token_fails() {
        let http = Outbound::for_tests();
        assert!(verify_turnstile(&http, None, None).await.unwrap());
        assert!(!verify_turnstile(&http, Some("s"), None).await.unwrap());
        assert!(!verify_turnstile(&http, Some("s"), Some("")).await.unwrap());
    }
}

// from: credits-server@d9c6a261:src/logic/purchase-email/component.ts (sendCreditsPurchase, resolveRecipient, emailFromSession, getUsername, PROFILE_TIMEOUT_MS), src/logic/purchase-email/template.ts (renderCreditsPurchaseEmail), src/controllers/handlers/stripe-webhook.ts (the receipt past the `credited` guard, resolveBuyerEmail), src/controllers/handlers/test-purchase-email.ts
//! The purchase receipt ("Your purchase was successful"), one per Stripe
//! pack purchase, through a transactional outbox instead of upstream's
//! fire-and-forget call:
//!
//! - [`enqueue_in_tx`]: the webhook's grant (`handlers/stripe.rs`,
//!   `credit_settled`) writes one `credits.purchase_receipts` row in its own
//!   transaction, keyed by the purchase. A grant that rolls back leaves no
//!   row, and a redelivered event, or the other of a purchase's settled
//!   events, adds none. The address the buyer typed into Checkout rides
//!   along when the event carries it (a session event).
//! - [`run_pass`] (the `credits-purchase-receipts` job): claim the due rows
//!   under a lease, send each after the grant's commit and outside any
//!   transaction, then [`record`] guarded on the claim. The recipient is the
//!   row's address, else the Checkout Session's read back from Stripe (a
//!   `payment_intent.succeeded` that settled first carries none), else the
//!   account's (`social.subscriptions.email`); none ends the row `skipped`.
//!   A purchase reversed before its send (`refunded`, `disputed`) is
//!   `skipped`: the email says its Credits are ready. A transient failure
//!   backs off as the waitlist does (30 s doubling to an hour, five attempts
//!   then `failed`); an unconfigured sender ends the row `skipped`, never a
//!   silent success. A terminal row drops its address.
//!
//! A claim sends once. A crash between the provider's accept and the record
//! sends it again once the lease ends, as the waitlist outbox does.

use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::json;
use sqlx::PgPool;

use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::notifications::SendFailure;
use crate::shared::now_utc;
use crate::subscriptions::AvailabilityMailer;
use crate::subscriptions::delivery::{
    LEASE, Outcome, PassSummary, RenderedEmail, escape_html, outcome_after,
};

use super::stripe::StripeClient;

pub const JOB_NAME: &str = "credits-purchase-receipts";
pub const BATCH: i64 = 50;
pub const SUBJECT: &str = "Your purchase was successful";
/// A slow profile read never holds a receipt back: without a name the
/// greeting still reads.
pub const PROFILE_TIMEOUT: Duration = Duration::from_secs(3);

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// The buyer's claimed name for the greeting.
#[async_trait::async_trait]
pub trait BuyerNames: Send + Sync {
    /// The profile's name when the address holds it as a NAME; `None`
    /// otherwise, or when the read fails.
    async fn claimed_name(&self, address: &str) -> Option<String>;
}

#[async_trait::async_trait]
impl BuyerNames for crate::market::MarketplaceState {
    async fn claimed_name(&self, address: &str) -> Option<String> {
        crate::market::profiles::claimed_name(self, address).await
    }
}

/// Where the email's links and images point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Links {
    /// GO SHOPPING.
    pub shop_url: String,
    /// The images' base, no trailing slash.
    pub assets_url: String,
    /// The footer's Unsubscribe: the account's notification preferences.
    pub preferences_url: String,
}

impl Links {
    /// `assets_url` defaults to `<shop>/email`, where the Shop app serves the
    /// images from its `public/` directory.
    pub fn new(shop_url: &str, assets_url: Option<&str>, preferences_url: &str) -> Self {
        let shop_url = shop_url.trim_end_matches('/').to_string();
        let assets_url = assets_url
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(|url| url.trim_end_matches('/').to_string())
            .unwrap_or_else(|| format!("{shop_url}/email"));
        Self {
            shop_url,
            assets_url,
            preferences_url: preferences_url.to_string(),
        }
    }
}

/// What the job and the test route send with.
pub struct Sender {
    pub mailer: Arc<dyn AvailabilityMailer>,
    /// `None` without the marketplace: every greeting drops the name.
    pub names: Option<Arc<dyn BuyerNames>>,
    pub links: Links,
}

impl Sender {
    /// The claimed name within [`PROFILE_TIMEOUT`], else `None`.
    pub async fn claimed_name(&self, address: &str) -> Option<String> {
        let names = self.names.as_ref()?;
        tokio::time::timeout(PROFILE_TIMEOUT, names.claimed_name(address))
            .await
            .ok()
            .flatten()
    }

    /// Renders and sends one receipt.
    pub async fn send(
        &self,
        to: &str,
        address: &str,
        credits: &str,
        custom_args: &serde_json::Value,
    ) -> Result<Option<String>, SendFailure> {
        let username = self.claimed_name(address).await;
        let email = render_purchase_receipt(username.as_deref(), credits, &self.links);
        self.mailer
            .send_html(to, &email.subject, &email.html, &email.text, custom_args)
            .await
    }
}

/// Writes the purchase's receipt row in the grant's transaction; a second
/// call for the same purchase is a no-op. True when this call wrote it.
pub(crate) async fn enqueue_in_tx(
    tx: &mut Tx<'_>,
    purchase_id: i64,
    buyer_email: Option<&str>,
) -> sqlx::Result<bool> {
    let buyer_email = buyer_email.map(str::trim).filter(|email| !email.is_empty());
    let res = sqlx::query(
        "INSERT INTO credits.purchase_receipts (purchase_id, buyer_email) VALUES ($1, $2) \
         ON CONFLICT (purchase_id) DO NOTHING",
    )
    .bind(purchase_id)
    .bind(buyer_email)
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected() == 1)
}

/// A claimed receipt with what its send needs, read from the purchase at
/// claim time.
#[derive(Debug, Clone)]
pub struct Claimed {
    pub purchase_id: i64,
    pub attempts: i32,
    pub address: String,
    pub credits: String,
    pub buyer_email: Option<String>,
    pub checkout_session: Option<String>,
    pub purchase_status: String,
}

/// Claims the due rows and, in the same statement, counts the attempt and
/// moves `next_attempt_at` to `now + LEASE`, so a crash before the outcome
/// lands makes the row due again later.
pub async fn claim(pool: &PgPool, now: NaiveDateTime, limit: i64) -> sqlx::Result<Vec<Claimed>> {
    let lease_until = now + chrono::Duration::from_std(LEASE).expect("LEASE fits");
    type Row = (
        i64,
        i32,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "WITH due AS ( \
             SELECT r.purchase_id FROM credits.purchase_receipts r \
             WHERE r.status = 'pending' AND r.next_attempt_at <= $1 \
             ORDER BY r.next_attempt_at, r.purchase_id LIMIT $2 FOR UPDATE SKIP LOCKED \
         ) \
         UPDATE credits.purchase_receipts r \
         SET attempts = r.attempts + 1, next_attempt_at = $3, updated_at = $1 \
         FROM due JOIN credits.purchases p ON p.id = due.purchase_id \
         WHERE r.purchase_id = due.purchase_id \
         RETURNING r.purchase_id, r.attempts, p.address, p.credits::text, r.buyer_email, \
                   p.stripe_checkout_session, p.status",
    )
    .bind(now)
    .bind(limit)
    .bind(lease_until)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                purchase_id,
                attempts,
                address,
                credits,
                buyer_email,
                checkout_session,
                purchase_status,
            )| Claimed {
                purchase_id,
                attempts,
                address,
                credits,
                buyer_email,
                checkout_session,
                purchase_status,
            },
        )
        .collect())
}

/// Records an outcome, guarded on the claim (still `pending`, same attempt):
/// a row another worker claimed after this lease ended is left to it. A
/// terminal row drops its address.
pub async fn record(
    pool: &PgPool,
    claimed: &Claimed,
    now: NaiveDateTime,
    outcome: &Outcome,
) -> sqlx::Result<()> {
    let query = match outcome {
        Outcome::Sent(message_id) => sqlx::query(
            "UPDATE credits.purchase_receipts SET status = 'sent', sent_at = $3, provider_message_id = $4, \
             last_error = NULL, buyer_email = NULL, updated_at = $3 \
             WHERE purchase_id = $1 AND status = 'pending' AND attempts = $2",
        )
        .bind(claimed.purchase_id)
        .bind(claimed.attempts)
        .bind(now)
        .bind(message_id),
        Outcome::Skipped(reason) | Outcome::Failed(reason) => {
            let status = if matches!(outcome, Outcome::Skipped(_)) {
                "skipped"
            } else {
                "failed"
            };
            sqlx::query(
                "UPDATE credits.purchase_receipts SET status = $4, last_error = $5, buyer_email = NULL, updated_at = $3 \
                 WHERE purchase_id = $1 AND status = 'pending' AND attempts = $2",
            )
            .bind(claimed.purchase_id)
            .bind(claimed.attempts)
            .bind(now)
            .bind(status)
            .bind(reason)
        }
        Outcome::Retry { after, error } => {
            let next = now + chrono::Duration::from_std(*after).expect("backoff fits");
            sqlx::query(
                "UPDATE credits.purchase_receipts SET next_attempt_at = $4, last_error = $5, updated_at = $3 \
                 WHERE purchase_id = $1 AND status = 'pending' AND attempts = $2",
            )
            .bind(claimed.purchase_id)
            .bind(claimed.attempts)
            .bind(now)
            .bind(next)
            .bind(error)
        }
    };
    query.execute(pool).await?;
    Ok(())
}

/// Where the receipt goes: the address on the row (the settling session
/// event's), else the Checkout Session's read back from Stripe, else the
/// account's email. A Stripe read that fails falls through to the account,
/// as upstream's does.
pub async fn recipient(
    pool: &PgPool,
    stripe: Option<&StripeClient>,
    claimed: &Claimed,
) -> sqlx::Result<Option<String>> {
    if let Some(email) = non_empty(claimed.buyer_email.as_deref()) {
        return Ok(Some(email));
    }
    if let (Some(stripe), Some(session)) = (stripe, claimed.checkout_session.as_deref()) {
        match stripe.retrieve_checkout_session(session).await {
            Ok(Some(found)) => {
                if let Some(email) = non_empty(found.buyer_email.as_deref()) {
                    return Ok(Some(email));
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(purchase_id = claimed.purchase_id, error = %e, "could not read the Checkout Session for the purchase receipt; trying the account email");
            }
        }
    }
    account_email(pool, &claimed.address).await
}

/// The account's notification email (`social.subscriptions`), if it set one.
pub async fn account_email(pool: &PgPool, address: &str) -> sqlx::Result<Option<String>> {
    let account: Option<Option<String>> =
        sqlx::query_scalar("SELECT email FROM social.subscriptions WHERE address = $1")
            .bind(address)
            .fetch_optional(pool)
            .await?;
    Ok(non_empty(account.flatten().as_deref()))
}

fn non_empty(email: Option<&str>) -> Option<String> {
    email
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_string)
}

/// One pass: claim, resolve the recipient, send outside any transaction,
/// record.
pub async fn run_pass(
    pool: &PgPool,
    sender: &Sender,
    stripe: Option<&StripeClient>,
) -> anyhow::Result<PassSummary> {
    let claimed = claim(pool, now_utc(), BATCH).await?;
    let mut summary = PassSummary {
        claimed: claimed.len(),
        ..Default::default()
    };
    let configured = sender.mailer.is_configured();
    for row in &claimed {
        let outcome = if row.purchase_status != "paid" {
            Outcome::Skipped(format!(
                "the purchase is {} before its receipt went out",
                row.purchase_status
            ))
        } else if !configured {
            outcome_after(row.attempts, Err(SendFailure::NotConfigured))
        } else {
            match recipient(pool, stripe, row).await? {
                None => Outcome::Skipped("no recipient".to_string()),
                Some(to) => {
                    let custom_args = json!({ "credits_purchase_id": row.purchase_id.to_string() });
                    let result = sender
                        .send(&to, &row.address, &row.credits, &custom_args)
                        .await;
                    outcome_after(row.attempts, result)
                }
            }
        };
        match &outcome {
            Outcome::Sent(_) => summary.sent += 1,
            Outcome::Skipped(_) => summary.skipped += 1,
            Outcome::Failed(_) => summary.failed += 1,
            Outcome::Retry { .. } => summary.retried += 1,
        }
        // The recipient is PII: the purchase id identifies the receipt.
        match &outcome {
            Outcome::Sent(_) => {
                tracing::info!(purchase_id = row.purchase_id, "purchase receipt sent")
            }
            Outcome::Skipped(reason) => {
                tracing::info!(purchase_id = row.purchase_id, reason = %reason, "purchase receipt skipped")
            }
            Outcome::Failed(e) => {
                tracing::warn!(purchase_id = row.purchase_id, error = %e, "purchase receipt failed")
            }
            Outcome::Retry { .. } => {}
        }
        record(pool, row, now_utc(), &outcome).await?;
    }
    Ok(summary)
}

/// The `credits-purchase-receipts` job, on the node that holds the credits
/// writes (`OPS_WRITES_ENABLED`), as the other credits writers.
pub fn spawn_job(state: &super::SharedState, interval: Duration, jobs: &Jobs) {
    let Some(sender) = state.receipts.get() else {
        return;
    };
    if !state.settings.writes_enabled {
        tracing::info!(
            "OPS_WRITES_ENABLED=false: purchase receipts are sent by the node that holds the credits writes"
        );
        return;
    }
    if !sender.mailer.is_configured() {
        tracing::info!(
            "SENDGRID_API_KEY / SENDGRID_FROM_EMAIL unset: purchase receipts are recorded as skipped"
        );
    }
    let state = state.clone();
    jobs.spawn_periodic_leased(
        JOB_NAME,
        PeriodicCfg::every(interval.max(Duration::from_secs(1)))
            .with_jitter(Duration::from_secs(5))
            .with_error_backoff(Duration::from_secs(300)),
        move |ctx| {
            let state = state.clone();
            async move {
                let Some(sender) = state.receipts.get() else {
                    return Ok(());
                };
                let summary = run_pass(&state.credits.pool, sender, state.stripe.as_ref()).await?;
                if summary.claimed > 0 {
                    tracing::info!(?summary, "purchase receipts pass");
                    ctx.progress(format!("{summary:?}"));
                }
                Ok(())
            }
        },
    );
}

/// Credits with en-US grouping, as the design's "100,000"; a fractional
/// part is kept without its trailing zeros.
pub fn format_credits(credits: &str) -> String {
    let credits = credits.trim();
    let (whole, fraction) = credits.split_once('.').unwrap_or((credits, ""));
    let digits = whole.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        grouped
    } else {
        format!("{grouped}.{fraction}")
    }
}

const CONTENT_WIDTH: u32 = 600;
const HERO_HEIGHT: u32 = 240;
/// Sampled from the hero gradient under the figure: what Outlook shows if
/// the image is blocked.
const HERO_FALLBACK: &str = "#632087";
const RUBY: &str = "#FF2D55";
const HEADING: &str = "#161518";
const BODY: &str = "#43404A";
const SNOW: &str = "#FCFCFC";
const CREDITS_LABEL: &str = "#ECEBED";
const FOOTER_TEXT: &str = "#696969";
const DIVIDER: &str = "#808080";
/// Inter is the design font; no webmail loads a web font, so the stack
/// falls back to the same grotesque metrics.
const FONT: &str =
    "'Inter',-apple-system,BlinkMacSystemFont,'Helvetica Neue',Helvetica,Arial,sans-serif";
/// Left offset of the figure inside the hero: the coin stack takes the rest.
const FIGURE_OFFSET: u32 = 306;
const GLYPH_GAP: u32 = 8;
const LABEL_GAP: u32 = 4;
const SOCIALS: [(&str, &str, u32); 7] = [
    ("discord", "https://dcl.gg/discord", 28),
    ("github", "https://github.com/decentraland", 23),
    ("x", "https://x.com/decentraland", 25),
    (
        "instagram",
        "https://instagram.com/decentraland_foundation/",
        28,
    ),
    (
        "youtube",
        "https://youtube.com/@decentraland_foundation",
        32,
    ),
    ("tiktok", "https://tiktok.com/@decentraland_fdn", 29),
    (
        "linkedin",
        "https://linkedin.com/company/decentralandorg",
        28,
    ),
];
const SOCIAL_ICON_HEIGHT: u32 = 28;
const SOCIAL_GAP: u32 = 36;

/// The receipt as email-client-safe HTML and its text twin: table layout
/// and inline styles, the hero a background image with the live figure on
/// top (Outlook gets a VML rect with a solid fallback), every interpolated
/// value HTML-escaped. The greeting names the buyer only by a claimed name.
pub fn render_purchase_receipt(
    username: Option<&str>,
    credits: &str,
    links: &Links,
) -> RenderedEmail {
    let amount = format_credits(credits);
    let assets = escape_html(&links.assets_url);
    let subject = SUBJECT.to_string();
    let preheader = format!("Your {amount} Credits are ready to spend.");
    let greeting_text = match username {
        Some(name) => format!("Hey there {name}, your"),
        None => "Your".to_string(),
    };
    let text = format!(
        "{greeting_text} purchase was successful!\n\n\
         {amount} CREDITS\n\n\
         Your Credits are ready! Use them for creator-made Wearables and Emotes, or a new NAME the \
         community will recognize. Credits never expire, so shop now or save them for your next \
         favorite look.\n\n\
         Go shopping: {shop}\n\n\
         This is an automated message, no reply is necessary.\n\
         Manage your email preferences: {preferences}",
        shop = links.shop_url,
        preferences = links.preferences_url,
    );
    let safe_shop_url = escape_html(&links.shop_url);
    let safe_preferences_url = escape_html(&links.preferences_url);
    let greeting = match username {
        Some(name) => format!(
            r#"Hey there <span style="color:{RUBY};">{}</span>, your purchase was successful! &#127881;"#,
            escape_html(name)
        ),
        None => "Hey there, your purchase was successful! &#127881;".to_string(),
    };
    let social_cells: String = SOCIALS
        .iter()
        .enumerate()
        .map(|(index, (name, href, width))| {
            let padding = if index == 0 { 0 } else { SOCIAL_GAP };
            format!(
                r#"<td style="padding:0 0 0 {padding}px;"><a href="{href}" target="_blank" style="text-decoration:none;"><img src="{assets}/social-{name}.png" width="{width}" height="{SOCIAL_ICON_HEIGHT}" alt="{name}" style="display:block;width:{width}px;height:{SOCIAL_ICON_HEIGHT}px;border:0;" /></a></td>"#
            )
        })
        .collect();
    let title = escape_html(&subject);
    let preheader = escape_html(&preheader);
    let amount_html = escape_html(&amount);
    let html = format!(
        r#"<!doctype html>
<html lang="en" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width,initial-scale=1" />
<meta name="x-apple-disable-message-reformatting" />
<meta name="color-scheme" content="light" />
<meta name="supported-color-schemes" content="light" />
<title>{title}</title>
<!--[if mso]><xml><o:OfficeDocumentSettings><o:PixelsPerInch>96</o:PixelsPerInch></o:OfficeDocumentSettings></xml><![endif]-->
<!--[if !mso]><!-->
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;700;800&display=swap" rel="stylesheet" />
<!--<![endif]-->
</head>
<body style="margin:0;padding:0;background-color:#ffffff;">
<div style="display:none;font-size:1px;color:#ffffff;line-height:1px;max-height:0;max-width:0;opacity:0;overflow:hidden;">{preheader}</div>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background-color:#ffffff;border-collapse:collapse;">
<tr><td style="padding:0;font-size:0;line-height:0;">
<img src="{assets}/top-bar.png" width="{CONTENT_WIDTH}" height="11" alt="" style="display:block;width:100%;height:11px;border:0;" />
</td></tr>
<tr><td align="center" style="padding:29px 16px 0 16px;">
<table role="presentation" width="{CONTENT_WIDTH}" cellpadding="0" cellspacing="0" border="0" style="width:{CONTENT_WIDTH}px;max-width:{CONTENT_WIDTH}px;border-collapse:collapse;">

<tr><td align="center" style="padding:0;">
<img src="{assets}/logo.png" width="48" height="48" alt="Decentraland" style="display:block;width:48px;height:48px;border:0;" />
</td></tr>

<tr><td align="center" style="padding:24px 0 0 0;font-family:{FONT};font-size:20px;line-height:24px;font-weight:700;color:{HEADING};">
{greeting}
</td></tr>

<tr><td style="padding:24px 0 0 0;">
<table role="presentation" width="{CONTENT_WIDTH}" cellpadding="0" cellspacing="0" border="0" style="width:{CONTENT_WIDTH}px;border-collapse:collapse;">
<tr><td background="{assets}/hero-credits.png" bgcolor="{HERO_FALLBACK}" width="{CONTENT_WIDTH}" height="{HERO_HEIGHT}" valign="middle" style="width:{CONTENT_WIDTH}px;height:{HERO_HEIGHT}px;background-color:{HERO_FALLBACK};background-image:url('{assets}/hero-credits.png');background-repeat:no-repeat;background-position:top left;background-size:{CONTENT_WIDTH}px {HERO_HEIGHT}px;border-radius:12px;">
<!--[if gte mso 9]>
<v:rect xmlns:v="urn:schemas-microsoft-com:vml" fill="true" stroke="false" style="width:{CONTENT_WIDTH}px;height:{HERO_HEIGHT}px;">
<v:fill type="frame" src="{assets}/hero-credits.png" color="{HERO_FALLBACK}" />
<v:textbox inset="0,0,0,0"><div>
<![endif]-->
<table role="presentation" width="{CONTENT_WIDTH}" cellpadding="0" cellspacing="0" border="0" style="width:{CONTENT_WIDTH}px;border-collapse:collapse;">
<tr>
<td width="{FIGURE_OFFSET}" style="width:{FIGURE_OFFSET}px;font-size:0;line-height:0;">&nbsp;</td>
<td align="left" valign="middle" style="padding:8px 0 0 0;">
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="border-collapse:collapse;">
<tr>
<td width="28" valign="middle" style="width:28px;">
<img src="{assets}/credit-glyph.png" width="28" height="30" alt="" style="display:block;width:28px;height:30px;border:0;" />
</td>
<td width="{GLYPH_GAP}" style="width:{GLYPH_GAP}px;font-size:0;line-height:0;">&nbsp;</td>
<td align="left" valign="middle" style="font-family:{FONT};font-size:40px;line-height:40px;font-weight:800;color:#ffffff;white-space:nowrap;">{amount_html}</td>
<td width="{LABEL_GAP}" style="width:{LABEL_GAP}px;font-size:0;line-height:0;">&nbsp;</td>
<td align="left" valign="middle" style="font-family:{FONT};font-size:20px;line-height:24px;font-weight:500;color:{CREDITS_LABEL};white-space:nowrap;">CREDITS</td>
</tr>
</table>
</td>
</tr>
</table>
<!--[if gte mso 9]></div></v:textbox></v:rect><![endif]-->
</td></tr>
</table>
</td></tr>

<tr><td align="center" style="padding:24px 0 0 0;font-family:{FONT};font-size:14px;line-height:24px;font-weight:700;color:{BODY};">
Your Credits are ready! Use them for creator-made Wearables and Emotes, or a new NAME the community will recognize. Credits never expire, so shop now or save them for your next favorite look.
</td></tr>

<tr><td align="center" style="padding:24px 0 0 0;">
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="border-collapse:collapse;">
<tr><td width="368" height="42" align="center" valign="middle" bgcolor="{RUBY}" style="width:368px;height:42px;background-color:{RUBY};border-radius:11px;">
<a href="{safe_shop_url}" target="_blank" style="display:block;width:368px;height:42px;line-height:42px;font-family:{FONT};font-size:14.69px;font-weight:700;color:{SNOW};text-decoration:none;text-transform:uppercase;text-align:center;">go shopping</a>
</td></tr>
</table>
</td></tr>

<tr><td style="padding:65px 0 0 0;">
<table role="presentation" width="{CONTENT_WIDTH}" cellpadding="0" cellspacing="0" border="0" style="width:{CONTENT_WIDTH}px;border-collapse:collapse;">
<tr><td align="center" style="padding:0;">
<table role="presentation" width="534" cellpadding="0" cellspacing="0" border="0" style="width:534px;border-collapse:collapse;">
<tr><td style="font-size:0;line-height:0;border-top:1px solid {DIVIDER};">&nbsp;</td></tr>
</table>
</td></tr>
<tr><td align="center" style="padding:38px 0 0 0;">
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="border-collapse:collapse;"><tr>{social_cells}</tr></table>
</td></tr>
<tr><td align="center" style="padding:35px 0 0 0;font-family:{FONT};font-size:14.5px;line-height:23px;color:{FOOTER_TEXT};">
This is an automated message, no reply is necessary.
</td></tr>
<tr><td align="center" style="padding:0;font-family:{FONT};font-size:14.5px;line-height:24px;">
<a href="{safe_preferences_url}" target="_blank" style="color:{RUBY};text-decoration:none;">Unsubscribe</a>
</td></tr>
</table>
</td></tr>

</table>
</td></tr>
<tr><td style="padding:0 0 40px 0;font-size:0;line-height:0;">&nbsp;</td></tr>
</table>
</body>
</html>"#
    );
    RenderedEmail {
        subject,
        html,
        text,
    }
}

//! Enrolment, the progress/credits reads of a wallet (the wallet in the path
//! must be the signer), the balance and the static seasons document (the
//! seasons domain was removed upstream; the shape is kept for the clients).
//! Money leaves as whole credits and integer cents: `usd.balanceCents` is
//! the SPENDABLE balance (the live pledges subtracted), `usd.held` the
//! pledges, so the Shop's credits page can show what is on hold and when it
//! comes back; the legacy MANA-credit fields (`credits`, `totalCredits`,
//! `totals`) are zero, as upstream answers a wallet without season credits.

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use serde::Serialize;

use super::signer_from;
use crate::credits::SharedState;
use crate::credits::dto::{
    CreditsData, CreditsProgramProgressResponse, CreditsTotals, CurrentSeasonInfo, SeasonData,
    SeasonsData, UsdCredits, UserCreditsResponse, UserData, Week,
};
use crate::credits::errors::ApiError;
use crate::credits::history::HeldOut;
use crate::types::UsdCents;

pub async fn enroll(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let signer = signer_from(&state, &headers, "post", "/users", &original).await?;
    state.credits.mark_started(signer.as_str()).await?;
    Ok(StatusCode::OK)
}

pub async fn progress(
    State(state): State<SharedState>,
    Path(wallet_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<CreditsProgramProgressResponse>, ApiError> {
    let wallet = wallet_id.to_lowercase();
    let path = format!("/users/{}/progress", wallet_id);
    let signer = signer_from(&state, &headers, "get", &path, &original).await?;

    if signer.as_str() != wallet {
        return Err(ApiError::forbidden("walletId does not match signer"));
    }

    let (has_started, credits_row) = state.credits.user_progress(&wallet).await?;

    let credits = match credits_row {
        Some(c) => {
            let available = UsdCents::new(c.available_cents).credits_floor();
            let earned = UsdCents::new(c.earned_cents).credits_floor();
            CreditsData {
                available,
                earned,
                paid: available - earned,
                expires_in: 0,
                is_blocked_for_claiming: c.is_blocked_for_claiming,
            }
        }
        None => CreditsData {
            available: 0,
            earned: 0,
            paid: 0,
            expires_in: 0,
            is_blocked_for_claiming: false,
        },
    };

    Ok(Json(CreditsProgramProgressResponse {
        user: UserData {
            has_started_program: has_started,
        },
        credits,
        goals: Vec::new(),
    }))
}

pub async fn user_credits(
    State(state): State<SharedState>,
    Path(wallet_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<UserCreditsResponse>, ApiError> {
    let wallet = wallet_id.to_lowercase();
    let path = format!("/users/{}/credits", wallet_id);
    let signer = signer_from(&state, &headers, "get", &path, &original).await?;

    if signer.as_str() != wallet {
        return Err(ApiError::forbidden("walletId does not match signer"));
    }

    let spendable = state.credits.spendable(&wallet).await?;
    let held = state.credits.held(&wallet).await?;
    Ok(Json(user_credits_response(
        spendable.spendable_cents(),
        held,
    )))
}

fn user_credits_response(spendable_cents: i64, held: Option<HeldOut>) -> UserCreditsResponse {
    let cents = UsdCents::new(spendable_cents);
    UserCreditsResponse {
        credits: Vec::new(),
        total_credits: 0,
        totals: CreditsTotals {
            expiring: 0,
            non_expiring: 0,
        },
        usd: UsdCredits {
            balance_cents: cents.get(),
            credits: cents.credits_floor(),
            held,
        },
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct BalanceOut {
    address: String,
    available: String,
}

pub async fn balance(
    State(state): State<SharedState>,
    Path(wallet_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<BalanceOut>, ApiError> {
    let wallet = wallet_id.to_lowercase();
    let path = format!("/wallet/{}/balance", wallet_id);
    let signer = signer_from(&state, &headers, "get", &path, &original).await?;

    if signer.as_str() != wallet {
        return Err(ApiError::forbidden("walletId does not match signer"));
    }

    let available = state.credits.balance(&wallet).await?;

    Ok(Json(BalanceOut {
        address: wallet,
        available,
    }))
}

/// Static placeholder data: the seasons domain was removed upstream; clients
/// and proxies hold the document for a while.
pub async fn seasons() -> ([(HeaderName, &'static str); 1], Json<SeasonsData>) {
    (
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(SeasonsData {
            last_season: SeasonData::default(),
            current_season: CurrentSeasonInfo {
                season: SeasonData::default(),
                week: Week::default(),
            },
            next_season: SeasonData::default(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_credits_wire_shape_matches_upstream_client_dto() {
        let v = serde_json::to_value(user_credits_response(125, None)).unwrap();
        assert_eq!(
            v,
            json!({
                "credits": [],
                "totalCredits": 0,
                "totals": { "expiring": 0, "nonExpiring": 0 },
                "usd": { "balanceCents": 125, "credits": 12 }
            })
        );
        let held = HeldOut {
            cents: 90,
            credits: 9,
            releases_at_seconds: Some(1_800_000_060),
            purchases: Vec::new(),
        };
        let v = serde_json::to_value(user_credits_response(1234, Some(held))).unwrap();
        assert_eq!(v["usd"]["balanceCents"], 1234);
        assert_eq!(v["usd"]["credits"], 123);
        assert_eq!(
            v["usd"]["held"],
            json!({ "cents": 90, "credits": 9, "releasesAtSeconds": 1_800_000_060i64, "purchases": [] })
        );
    }

    #[test]
    fn user_credits_zero_state_for_unknown_wallet() {
        let v = serde_json::to_value(user_credits_response(0, None)).unwrap();
        assert_eq!(v["totalCredits"], 0);
        assert_eq!(v["usd"]["credits"], 0);
        assert_eq!(v["usd"]["balanceCents"], 0);
        assert!(v["usd"].get("held").is_none());
    }

    #[test]
    fn wire_identity_balance() {
        let out = BalanceOut {
            address: "0xabc".into(),
            available: "12.5".into(),
        };
        assert_eq!(
            serde_json::to_value(&out).unwrap(),
            json!({ "address": "0xabc", "available": "12.5" })
        );
    }
}

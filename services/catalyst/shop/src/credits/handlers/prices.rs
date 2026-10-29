//! `POST /prices/quote`: the Credits price of up to 60 items and 60 raw wei
//! amounts, the item quotes memoised 60 s per `(collection, item)` (a miss
//! that fails at any step prices to `null`), the misses and the amounts
//! repriced in one batched statement.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use super::cart::{validate_collection, validate_item_id};
use crate::credits::SharedState;
use crate::credits::errors::ApiError;
use crate::credits::pricing::{QUOTE_ORDER_SCAN_MAX_PAGES, ensure_charge_covers_payment};
use crate::shared::cache::TtlMap;

const MAX_ENTRIES: usize = 60;

pub const QUOTE_CACHE_TTL: Duration = Duration::from_secs(60);

pub const QUOTE_CACHE_MAX_ENTRIES: usize = 10_000;

pub struct QuoteCache {
    max_entries: usize,
    inner: TtlMap<(String, String), Option<String>>,
}

impl QuoteCache {
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            max_entries,
            inner: TtlMap::bounded(ttl, max_entries),
        }
    }

    pub fn get(&self, collection: &str, item_id: &str) -> Option<Option<String>> {
        self.inner
            .get_fresh(&(collection.to_string(), item_id.to_string()))
    }

    /// A full memo is dropped whole rather than grown: the entries are 60 s
    /// quotes, and a scan is not worth keeping the newest ones.
    pub fn put(&self, collection: &str, item_id: &str, credits: Option<String>) {
        if self.inner.len() >= self.max_entries {
            self.inner.clear();
        }
        self.inner
            .insert((collection.to_string(), item_id.to_string()), credits);
    }
}

impl Default for QuoteCache {
    fn default() -> Self {
        Self::new(QUOTE_CACHE_TTL, QUOTE_CACHE_MAX_ENTRIES)
    }
}

#[derive(Debug, Deserialize)]
pub struct QuoteItemRef {
    #[serde(rename = "itemId")]
    item_id: String,
    collection: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct QuoteBody {
    #[serde(default)]
    items: Vec<QuoteItemRef>,
    #[serde(default)]
    amounts: Vec<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct ItemQuoteOut {
    #[serde(rename = "itemId")]
    item_id: String,
    collection: String,
    credits: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct PriceQuotesOut {
    items: Vec<ItemQuoteOut>,
    amounts: Vec<Option<String>>,
}

fn valid_wei(raw: &str) -> Option<&str> {
    let s = raw.trim();
    if s.is_empty() || s.len() > 30 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(s)
}

pub async fn quote(
    State(state): State<SharedState>,
    body: Json<QuoteBody>,
) -> Result<Json<PriceQuotesOut>, ApiError> {
    let Json(body) = body;
    if body.items.is_empty() && body.amounts.is_empty() {
        return Err(ApiError::bad_request("nothing to quote"));
    }
    if body.items.len() > MAX_ENTRIES || body.amounts.len() > MAX_ENTRIES {
        return Err(ApiError::bad_request(format!(
            "too many entries (max {MAX_ENTRIES} items and {MAX_ENTRIES} amounts)"
        )));
    }

    let mut refs = Vec::with_capacity(body.items.len());
    for r in &body.items {
        refs.push((
            validate_collection(&r.collection)?,
            validate_item_id(&r.item_id)?,
        ));
    }

    let mut quoted: Vec<Option<Option<String>>> = refs
        .iter()
        .map(|(collection, item_id)| state.quote_cache.get(collection, item_id))
        .collect();
    let misses: Vec<usize> = (0..refs.len()).filter(|&i| quoted[i].is_none()).collect();

    let valid_amounts: Vec<(usize, String)> = body
        .amounts
        .iter()
        .enumerate()
        .filter_map(|(i, raw)| valid_wei(raw).map(|wei| (i, wei.to_string())))
        .collect();
    let mut amounts: Vec<Option<String>> = vec![None; body.amounts.len()];

    if !misses.is_empty() || !valid_amounts.is_empty() {
        let mana_usd = state.pricing.fetch_mana_usd().await?;
        let amount_weis: Vec<String> = valid_amounts.iter().map(|(_, wei)| wei.clone()).collect();
        let (fresh, priced_amounts) =
            quote_misses(&state, &refs, &misses, &amount_weis, &mana_usd).await;
        for (&i, credits) in misses.iter().zip(fresh) {
            let (collection, item_id) = &refs[i];
            state.quote_cache.put(collection, item_id, credits.clone());
            quoted[i] = Some(credits);
        }
        for ((i, _), credit) in valid_amounts.iter().zip(priced_amounts) {
            amounts[*i] = credit;
        }
    }

    let items = refs
        .into_iter()
        .zip(quoted)
        .map(|((collection, item_id), credits)| ItemQuoteOut {
            item_id,
            collection,
            credits: credits.flatten(),
        })
        .collect();

    Ok(Json(PriceQuotesOut { items, amounts }))
}

/// Prices the cache misses with one catalogue batch and one order-book read,
/// and the credit conversion of the misses AND the raw `amount_weis` folded
/// into one batched query. A miss that fails at any step prices to `None`;
/// the second vector answers `amount_weis` element-wise.
async fn quote_misses(
    state: &SharedState,
    refs: &[(String, String)],
    misses: &[usize],
    amount_weis: &[String],
    mana_usd: &str,
) -> (Vec<Option<String>>, Vec<Option<String>>) {
    let none = || (vec![None; misses.len()], vec![None; amount_weis.len()]);
    let pairs: Vec<(String, String)> = misses.iter().map(|&i| refs[i].clone()).collect();
    let mode = state.settings.checkout_fulfillment_mode.as_str();
    let bases = match state
        .pricing
        .fetch_charge_bases_batch(&pairs, mode, QUOTE_ORDER_SCAN_MAX_PAGES)
        .await
    {
        Ok(bases) => bases,
        Err(_) => return none(),
    };

    let mut weis: Vec<String> = bases
        .iter()
        .filter_map(|b| b.as_ref().ok())
        .map(|b| b.basis_wei.clone())
        .collect();
    weis.extend_from_slice(amount_weis);
    let mut priced = match state
        .pricing
        .compute_credit_prices_batch(&state.credits.pool, &weis, mana_usd)
        .await
    {
        Ok(p) if p.len() == weis.len() => p.into_iter(),
        _ => return none(),
    };
    let mut items = Vec::with_capacity(bases.len());
    for basis in bases {
        let Ok(basis) = basis else {
            items.push(None);
            continue;
        };
        let Some(credits) = priced.next() else {
            items.push(None);
            continue;
        };
        if ensure_charge_covers_payment(&basis.basis_wei, &credits).is_err() {
            items.push(None);
            continue;
        }
        let offered =
            match super::cart::rule_line(state, crate::types::Session::SignedOut, &basis).await {
                Ok(rule) => rule.guard_quote().is_ok(),
                Err(_) => false,
            };
        items.push(offered.then_some(credits));
    }
    let amounts = priced.map(Some).collect();
    (items, amounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wire_identity_price_quotes() {
        let out = PriceQuotesOut {
            items: vec![
                ItemQuoteOut {
                    item_id: "12".into(),
                    collection: "0x59a90bad9570ecd08895f132daf7b79696337f61".into(),
                    credits: Some("2".into()),
                },
                ItemQuoteOut {
                    item_id: "3".into(),
                    collection: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                    credits: None,
                },
            ],
            amounts: vec![Some("1".into()), None],
        };
        assert_eq!(
            serde_json::to_value(&out).unwrap(),
            json!({
                "items": [
                    { "itemId": "12", "collection": "0x59a90bad9570ecd08895f132daf7b79696337f61", "credits": "2" },
                    { "itemId": "3", "collection": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "credits": null },
                ],
                "amounts": ["1", null],
            })
        );
    }

    #[test]
    fn quote_body_defaults_are_empty() {
        let b: QuoteBody = serde_json::from_value(json!({})).unwrap();
        assert!(b.items.is_empty());
        assert!(b.amounts.is_empty());
        let b: QuoteBody = serde_json::from_value(json!({
            "items": [{ "itemId": "1", "collection": "0xabc" }],
            "amounts": ["10000000000000000"],
        }))
        .unwrap();
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.amounts, vec!["10000000000000000".to_string()]);
    }

    #[tokio::test]
    async fn quote_cache_hit_within_ttl_serves_without_refetch() {
        let cache = QuoteCache::new(Duration::from_secs(60), 100);
        assert_eq!(cache.get("0xabc", "1"), None, "cold cache misses");
        cache.put("0xabc", "1", Some("2".into()));
        assert_eq!(cache.get("0xabc", "1"), Some(Some("2".into())));
        cache.put("0xabc", "2", None);
        assert_eq!(cache.get("0xabc", "2"), Some(None));
        assert_eq!(cache.get("0xdef", "1"), None);
    }

    #[tokio::test]
    async fn quote_cache_expires_after_ttl() {
        let cache = QuoteCache::new(Duration::ZERO, 100);
        cache.put("0xabc", "1", Some("2".into()));
        assert_eq!(cache.get("0xabc", "1"), None, "zero TTL expires instantly");
    }

    #[tokio::test]
    async fn quote_cache_stays_bounded() {
        let cache = QuoteCache::new(Duration::from_secs(60), 2);
        cache.put("0xabc", "1", Some("1".into()));
        cache.put("0xabc", "2", Some("2".into()));
        cache.put("0xabc", "3", Some("3".into()));
        assert_eq!(cache.get("0xabc", "3"), Some(Some("3".into())));
        assert_eq!(cache.get("0xabc", "1"), None);
        assert_eq!(cache.get("0xabc", "2"), None);
    }

    #[test]
    fn wei_validation() {
        assert_eq!(valid_wei(" 10000000000000000 "), Some("10000000000000000"));
        assert_eq!(valid_wei("0"), Some("0"));
        assert_eq!(valid_wei(""), None);
        assert_eq!(valid_wei("1.5"), None);
        assert_eq!(valid_wei("0x10"), None);
        assert_eq!(valid_wei(&"9".repeat(31)), None);
    }
}

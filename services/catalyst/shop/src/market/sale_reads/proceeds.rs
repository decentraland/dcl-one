use super::*;
use crate::proceeds::{ProceedsFilters, ProceedsRow, ProceedsSummary};

#[derive(serde::Deserialize)]
struct PaidSale {
    #[serde(flatten)]
    sale: Sale,
    fees: String,
    royalties: String,
    net: String,
}

pub(crate) async fn proceeds_page(
    state: &MarketplaceState,
    f: &ProceedsFilters,
) -> Result<(Vec<ProceedsRow>, i64), Response> {
    let (first, skip) = readers::deep_page(f.first, f.skip, 24)?;
    let _slot = seller_slot(state).await?;
    let (mut snap, nets) = snapshot(state, f.network).await?;
    let (total, incomplete, rows): (i64, bool, Value) = sales_query!(
        nets, wallet "$7";
        "settled_sales.sql",
        "proceeds.sql",
        "proceeds_page.sql"
    )
    .bind(&f.seller)
    .bind(f.from)
    .bind(f.to)
    .bind(first)
    .bind(skip)
    .bind(readers::count_cap(first, skip))
    .fetch_one(&mut *snap.transaction)
    .await
    .map_err(db_error)?;
    if incomplete {
        return Err(unavailable(
            Source::SalesIndex,
            "the requested proceeds lack a proven seller or payment split",
        ));
    }
    ensure_fresh(snap.expires_at)?;
    let rows: Vec<PaidSale> = serde_json::from_value(rows).map_err(db_error)?;
    Ok((
        rows.into_iter()
            .map(|r| {
                let s = r.sale;
                ProceedsRow {
                    id: s.id,
                    timestamp: s.timestamp,
                    network: s.network,
                    chain_id: s.chain_id,
                    sale_type: s.sale_type,
                    contract_address: s.contract_address,
                    item_id: s.item_id,
                    token_id: s.token_id,
                    buyer: s.buyer,
                    seller: s.seller,
                    gross_wei: s.price,
                    fees_wei: r.fees,
                    royalties_wei: r.royalties,
                    net_wei: r.net,
                    tx_hash: s.tx_hash,
                    currency: "MANA",
                    settlement: "mana",
                }
            })
            .collect(),
        total,
    ))
}

#[derive(sqlx::FromRow)]
struct Totals {
    sales: i64,
    mints: i64,
    resales: i64,
    gross_wei: String,
    fees_wei: String,
    royalties_wei: String,
    net_wei: String,
    royalty_resales: i64,
    royalties_earned_wei: String,
    first_sale_at: Option<i64>,
    last_sale_at: Option<i64>,
    incomplete: bool,
}

pub(crate) async fn proceeds_summary(
    state: &MarketplaceState,
    f: &ProceedsFilters,
) -> Result<ProceedsSummary, Response> {
    let _slot = seller_slot(state).await?;
    let (mut snap, nets) = snapshot(state, f.network).await?;
    require_metadata(&mut snap, nets, "items", Source::ItemsMirror).await?;
    let r: Totals = sales_query!(
        nets, wallet "$7";
        "settled_sales.sql",
        "proceeds.sql",
        "proceeds_summary.sql"
    )
    .bind(&f.seller)
    .bind(f.from)
    .bind(f.to)
    .fetch_one(&mut *snap.transaction)
    .await
    .map_err(db_error)?;
    if r.incomplete {
        return Err(unavailable(
            Source::SalesIndex,
            "the requested proceeds lack a proven seller or payment split",
        ));
    }
    ensure_fresh(snap.expires_at)?;
    Ok(ProceedsSummary {
        seller: f.seller.clone(),
        from: f.from,
        to: f.to,
        currency: "MANA",
        settlement: "mana",
        treasury_routing: false,
        sales: r.sales,
        mints: r.mints,
        resales: r.resales,
        gross_wei: r.gross_wei,
        fees_wei: r.fees_wei,
        royalties_wei: r.royalties_wei,
        net_wei: r.net_wei,
        royalty_resales: r.royalty_resales,
        royalties_earned_wei: r.royalties_earned_wei,
        first_sale_at: r.first_sale_at,
        last_sale_at: r.last_sale_at,
    })
}

//! The events moderators (`explore.moderators`, the old events service's
//! `ADMIN_ADDRESSES`): the operator writes. `profiles::is_moderator` reads a
//! wallet here per request, so a change applies on the next request.
//!
//! Two writers call this module: the console's `events/moderator-{add,remove}`
//! (`ops::admin::explore`, audited by the console) and `catalyst explore
//! moderators-{add,remove}` (the cutover seed, audited by the command). An
//! address is stored lowercase; the reads lowercase the signer.

use serde::Serialize;
use sqlx::{PgExecutor, PgPool};

/// One row of `explore.moderators`; `added_at` is a unix second.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Moderator {
    pub address: String,
    pub added_by: Option<String>,
    pub added_at: i64,
}

/// A comma-separated wallet list, as the old service's `ADMIN_ADDRESSES`
/// held it: lowercased and deduplicated, in first-seen order. Empty entries
/// are skipped, as upstream skipped them; any other entry that is not exactly
/// `0x` and 40 hex digits (upstream's `isEthereumAddress`, no trim) refuses
/// the whole list, naming the entries.
pub fn parse_list(raw: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let mut bad = Vec::new();
    for entry in raw.split(',').filter(|e| !e.is_empty()) {
        if is_address(entry) {
            let lower = entry.to_ascii_lowercase();
            if !out.contains(&lower) {
                out.push(lower);
            }
        } else {
            bad.push(format!("{entry:?}"));
        }
    }
    if bad.is_empty() {
        Ok(out)
    } else {
        Err(format!(
            "not wallet addresses (0x and 40 hex digits): {}",
            bad.join(", ")
        ))
    }
}

fn is_address(s: &str) -> bool {
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    matches!(hex, Some(h) if h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Every moderator, by address.
pub async fn list(pool: &PgPool) -> sqlx::Result<Vec<Moderator>> {
    sqlx::query_as(
        "SELECT address, added_by, added_at FROM explore.moderators ORDER BY address LIMIT 10000",
    )
    .fetch_all(pool)
    .await
}

/// Adds `addresses` (lowercased) with `added_by`; a listed wallet keeps its
/// row. Answers the addresses newly added.
pub async fn add<'e, E: PgExecutor<'e>>(
    executor: E,
    addresses: &[String],
    added_by: &str,
) -> sqlx::Result<Vec<String>> {
    let lower: Vec<String> = addresses.iter().map(|a| a.to_ascii_lowercase()).collect();
    sqlx::query_scalar(
        "INSERT INTO explore.moderators (address, added_by, added_at) \
         SELECT DISTINCT a, $2, extract(epoch FROM now())::bigint FROM unnest($1::text[]) AS a \
         ON CONFLICT (address) DO NOTHING RETURNING address",
    )
    .bind(&lower)
    .bind(added_by.to_ascii_lowercase())
    .fetch_all(executor)
    .await
}

/// Removes `addresses`; answers the ones that were listed.
pub async fn remove<'e, E: PgExecutor<'e>>(
    executor: E,
    addresses: &[String],
) -> sqlx::Result<Vec<String>> {
    let lower: Vec<String> = addresses.iter().map(|a| a.to_ascii_lowercase()).collect();
    sqlx::query_scalar(
        "DELETE FROM explore.moderators WHERE address = ANY($1::text[]) RETURNING address",
    )
    .bind(&lower)
    .fetch_all(executor)
    .await
}

/// What `catalyst explore moderators-{add,remove}` prints.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CliOutcome {
    pub requested: usize,
    pub changed: Vec<String>,
    pub unchanged: Vec<String>,
}

/// The operator command: adds (or removes) `raw`, a [`parse_list`] list, in
/// one transaction with one `explore`/`events.moderator.{add,remove}` audit
/// row per changed wallet, actor `cli:<operator>`. A malformed list writes
/// nothing and is an error.
pub async fn apply_cli(
    pool: &PgPool,
    raw: &str,
    operator: &str,
    present: bool,
) -> anyhow::Result<CliOutcome> {
    let addresses = parse_list(raw).map_err(anyhow::Error::msg)?;
    anyhow::ensure!(!addresses.is_empty(), "no addresses given");
    let operator = operator.to_ascii_lowercase();
    let mut tx = pool.begin().await?;
    let changed = if present {
        add(&mut *tx, &addresses, &operator).await?
    } else {
        remove(&mut *tx, &addresses).await?
    };
    let actor = format!("cli:{operator}");
    let action = if present {
        "events.moderator.add"
    } else {
        "events.moderator.remove"
    };
    let entries: Vec<_> = changed
        .iter()
        .map(|a| {
            crate::shared::audit::Entry::new(&actor, "explore", action)
                .target(Some(a))
                .detail(serde_json::json!({ "address": a, "source": "cli" }))
        })
        .collect();
    crate::shared::audit::record_all(&mut *tx, &entries).await?;
    tx.commit().await?;
    let unchanged = addresses
        .iter()
        .filter(|a| !changed.contains(a))
        .cloned()
        .collect();
    Ok(CliOutcome {
        requested: addresses.len(),
        changed,
        unchanged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0x00000000000000000000000000000000000000aA";
    const B: &str = "0x0000000000000000000000000000000000000001";

    #[test]
    fn a_list_is_lowercased_deduplicated_and_skips_empty_entries() {
        assert_eq!(
            parse_list(&format!(
                "{A},,{B},{},",
                A.to_uppercase().replace("0X", "0x")
            )),
            Ok(vec![A.to_ascii_lowercase(), B.to_string()])
        );
        assert_eq!(parse_list(""), Ok(vec![]));
    }

    #[test]
    fn a_list_with_a_non_address_is_refused_whole() {
        let err = parse_list(&format!("{B}, {A},admin,0x1234")).unwrap_err();
        assert!(err.contains(&format!("\" {A}\"")), "{err}");
        assert!(
            err.contains("\"admin\"") && err.contains("\"0x1234\""),
            "{err}"
        );
        assert!(!err.contains(&format!("\"{B}\"")), "{err}");
    }

    fn random_address() -> String {
        format!("0x{}", &uuid::Uuid::new_v4().simple().to_string()[..32]) + "00000001"
    }

    #[tokio::test]
    async fn added_wallets_pass_the_moderator_gate_until_removed() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let (a, b) = (random_address(), random_address());
        let mixed = a.to_ascii_uppercase().replacen("0X", "0x", 1);
        let operator = "0x00000000000000000000000000000000000000FF";
        let added = add(&pool, &[mixed.clone(), b.clone(), mixed], operator)
            .await
            .unwrap();
        assert_eq!(added.len(), 2, "{added:?}");
        assert!(
            super::super::profiles::is_moderator(&pool, &a)
                .await
                .unwrap()
        );
        assert!(
            super::super::profiles::is_moderator(
                &pool,
                &a.to_ascii_uppercase().replacen("0X", "0x", 1)
            )
            .await
            .unwrap(),
            "the gate lowercases the signer"
        );
        let rows = list(&pool).await.unwrap();
        let row = rows.iter().find(|m| m.address == a).unwrap();
        assert_eq!(
            row.added_by.as_deref(),
            Some(&*operator.to_ascii_lowercase())
        );
        assert!(
            row.added_at > 1_700_000_000,
            "a unix second: {}",
            row.added_at
        );

        assert!(
            add(&pool, std::slice::from_ref(&a), operator)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            remove(&pool, &[a.clone(), random_address()]).await.unwrap(),
            vec![a.clone()]
        );
        assert!(
            !super::super::profiles::is_moderator(&pool, &a)
                .await
                .unwrap()
        );
        assert!(
            super::super::profiles::is_moderator(&pool, &b)
                .await
                .unwrap()
        );
        remove(&pool, &[b]).await.unwrap();
    }

    #[tokio::test]
    async fn the_cli_seeds_a_legacy_list_once_and_audits_each_change() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let (a, b) = (random_address(), random_address());
        let operator = random_address();
        let actor = format!("cli:{operator}");
        let audit = |action: &'static str| {
            let (pool, actor) = (pool.clone(), actor.clone());
            async move {
                sqlx::query_scalar::<_, String>(
                    "SELECT target FROM ops.admin_audit WHERE actor = $1 AND domain = 'explore' \
                     AND action = $2 ORDER BY target",
                )
                .bind(&actor)
                .bind(action)
                .fetch_all(&pool)
                .await
                .unwrap()
            }
        };

        let err = apply_cli(&pool, &format!("{a},admin"), &operator, true)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("\"admin\""), "{err}");
        assert!(
            !super::super::profiles::is_moderator(&pool, &a)
                .await
                .unwrap()
        );

        let legacy = format!("{a},{},", b.to_ascii_uppercase().replacen("0X", "0x", 1));
        let out = apply_cli(&pool, &legacy, &operator, true).await.unwrap();
        assert_eq!((out.requested, out.changed.len()), (2, 2), "{out:?}");
        let mut both = vec![a.clone(), b.clone()];
        both.sort();
        assert_eq!(audit("events.moderator.add").await, both);
        assert!(
            super::super::profiles::is_moderator(&pool, &b)
                .await
                .unwrap()
        );

        let again = apply_cli(&pool, &legacy, &operator, true).await.unwrap();
        assert_eq!((again.changed.len(), again.unchanged.len()), (0, 2));
        assert_eq!(
            audit("events.moderator.add").await.len(),
            2,
            "a re-run audits nothing"
        );

        let out = apply_cli(&pool, &a, &operator, false).await.unwrap();
        assert_eq!(out.changed, vec![a.clone()]);
        assert_eq!(audit("events.moderator.remove").await, vec![a.clone()]);
        assert!(
            !super::super::profiles::is_moderator(&pool, &a)
                .await
                .unwrap()
        );

        remove(&pool, &[b]).await.unwrap();
        sqlx::query("DELETE FROM ops.admin_audit WHERE actor = $1")
            .bind(&actor)
            .execute(&pool)
            .await
            .unwrap();
    }
}

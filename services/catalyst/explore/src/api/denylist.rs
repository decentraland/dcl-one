//! `GET /denylist.json`: the wallets the explorer refuses to boot for
//! (`{"users": [{"wallet": ...}]}`). Mirrored from the config document into
//! `explore_api.denylist_users` (keyed lowercase, served as received) with the
//! `denylist_users_local` overlay, and held in memory for the route and the
//! `contains` check.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use anyhow::Context as _;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use parking_lot::RwLock;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;

use super::{Shared, mirror};
use crate::jobs::JobContext;

pub const MIRROR: &str = "denylist";
const BODY_CAP: usize = 4 << 20;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Denylist {
    pub users: Vec<UserEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserEntry {
    pub wallet: String,
}

impl<'de> Deserialize<'de> for Denylist {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default, deserialize_with = "de_users")]
            users: Vec<UserEntry>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(Denylist { users: raw.users })
    }
}

/// `users` as wallet strings or `{wallet}` objects, both shapes upstream has used.
fn de_users<'de, D>(deserializer: D) -> Result<Vec<UserEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    struct UsersVisitor;
    impl<'de> Visitor<'de> for UsersVisitor {
        type Value = Vec<UserEntry>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an array of wallet strings or {wallet} objects")
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Elem {
                Str(String),
                Obj { wallet: String },
            }
            let mut out = Vec::new();
            while let Some(elem) = seq.next_element::<Elem>()? {
                match elem {
                    Elem::Str(wallet) | Elem::Obj { wallet } => out.push(UserEntry { wallet }),
                }
            }
            Ok(out)
        }
    }
    deserializer.deserialize_seq(UsersVisitor)
}

pub fn normalize_wallet(w: &str) -> String {
    w.trim().to_lowercase()
}

struct Served {
    list: Arc<Denylist>,
    wallets: HashSet<String>,
}

/// The served list and its normalized set, rebuilt from the database after
/// every pass or overlay change.
pub struct Snapshot {
    inner: RwLock<Arc<Served>>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            inner: RwLock::new(Arc::new(Served {
                list: Arc::new(Denylist::default()),
                wallets: HashSet::new(),
            })),
        }
    }
}

impl Snapshot {
    pub fn list(&self) -> Arc<Denylist> {
        self.inner.read().list.clone()
    }

    pub fn contains(&self, wallet: &str) -> bool {
        self.inner
            .read()
            .wallets
            .contains(&normalize_wallet(wallet))
    }

    /// Mirror rows plus local additions, minus local removals; a wallet both
    /// mirrored and locally added keeps the upstream's casing.
    pub async fn reload(&self, pool: &PgPool) -> sqlx::Result<()> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT DISTINCT ON (wallet) wallet, wallet_raw FROM (
                 SELECT wallet, wallet_raw, 0 AS pri FROM explore_api.denylist_users
                 UNION ALL
                 SELECT wallet, wallet, 1 FROM explore_api.denylist_users_local WHERE present
             ) u
             WHERE wallet NOT IN (SELECT wallet FROM explore_api.denylist_users_local WHERE NOT present)
             ORDER BY wallet, pri",
        )
        .fetch_all(pool)
        .await?;
        let wallets = rows.iter().map(|(w, _)| w.clone()).collect();
        let users = rows
            .into_iter()
            .map(|(_, raw)| UserEntry { wallet: raw })
            .collect();
        *self.inner.write() = Arc::new(Served {
            list: Arc::new(Denylist { users }),
            wallets,
        });
        Ok(())
    }
}

pub async fn set_local(pool: &PgPool, wallet: &str, present: bool) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.denylist_users_local (wallet, present)
         VALUES ($1, $2)
         ON CONFLICT (wallet) DO UPDATE
         SET present = EXCLUDED.present, updated_at = timezone('UTC', now())",
    )
    .bind(normalize_wallet(wallet))
    .bind(present)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_local(pool: &PgPool, wallet: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM explore_api.denylist_users_local WHERE wallet = $1")
        .bind(normalize_wallet(wallet))
        .execute(pool)
        .await?;
    Ok(())
}

/// The mirror becomes exactly `users`, in one transaction.
pub async fn replace(pool: &PgPool, users: &[UserEntry]) -> sqlx::Result<()> {
    let mut keys: Vec<String> = Vec::with_capacity(users.len());
    let mut raws: Vec<String> = Vec::with_capacity(users.len());
    for u in users {
        let key = normalize_wallet(&u.wallet);
        if key.is_empty() || keys.contains(&key) {
            continue;
        }
        keys.push(key);
        raws.push(u.wallet.trim().to_string());
    }
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO explore_api.denylist_users (wallet, wallet_raw)
         SELECT k, r FROM unnest($1::text[], $2::text[]) AS t(k, r)
         ON CONFLICT (wallet) DO UPDATE SET wallet_raw = EXCLUDED.wallet_raw",
    )
    .bind(&keys)
    .bind(&raws)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM explore_api.denylist_users WHERE wallet <> ALL($1::text[])")
        .bind(&keys)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

pub async fn run_pass(state: &Shared, ctx: &JobContext) -> anyhow::Result<()> {
    mirror::run(&state.pool, MIRROR, ctx, || async {
        let url = state.settings.sources.denylist.as_str();
        let fetched = mirror::fetch(
            &state.http,
            &state.settings,
            reqwest::Method::GET,
            url,
            None,
            &[],
            BODY_CAP,
        )
        .await?;
        let doc: Denylist = serde_json::from_slice(&fetched.body)
            .with_context(|| format!("{}: body is not a denylist document", mirror::shown(url)))?;
        anyhow::ensure!(
            !doc.users.is_empty(),
            "{}: upstream answered an empty denylist; refusing to prune the mirror",
            mirror::shown(url)
        );
        replace(&state.pool, &doc.users)
            .await
            .context("replacing explore_api.denylist_users")?;
        state
            .denylist
            .reload(&state.pool)
            .await
            .context("reloading the denylist")?;
        Ok(mirror::PassOutcome {
            items: doc.users.len() as i64,
            cursor: fetched.etag,
        })
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/denylist.json", get(get_denylist))
}

async fn get_denylist(State(state): State<Shared>) -> Response {
    (StatusCode::OK, Json(state.denylist.list())).into_response()
}

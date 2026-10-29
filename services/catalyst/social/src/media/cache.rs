//! `social.translation_cache`: one row per (engine, target language, sha256
//! of the source text). Read as one `= ANY($hashes)` probe per request on the
//! primary key, written with one `UNNEST` upsert for the misses, pruned in
//! bounded batches when a TTL is configured.

use std::time::Duration;

use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::backend::TranslatedItem;
use crate::jobs::JobContext;

pub fn hash_text(text: &str) -> Vec<u8> {
    Sha256::digest(text.as_bytes()).to_vec()
}

pub struct CachedRow {
    pub text_sha256: Vec<u8>,
    pub translated_text: String,
    pub detected_language: String,
    pub detected_confidence: f32,
}

pub async fn fetch(
    pool: &PgPool,
    backend: &str,
    target: &str,
    hashes: &[Vec<u8>],
) -> Result<Vec<CachedRow>, sqlx::Error> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, (Vec<u8>, String, String, f32)>(
        "SELECT text_sha256, translated_text, detected_language, detected_confidence \
         FROM social.translation_cache \
         WHERE backend = $1 AND target_lang = $2 AND text_sha256 = ANY($3)",
    )
    .bind(backend)
    .bind(target)
    .bind(hashes)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(text_sha256, translated_text, detected_language, detected_confidence)| CachedRow {
                text_sha256,
                translated_text,
                detected_language,
                detected_confidence,
            },
        )
        .collect())
}

pub async fn store(
    pool: &PgPool,
    backend: &str,
    target: &str,
    rows: &[(Vec<u8>, &TranslatedItem)],
) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }
    let hashes: Vec<Vec<u8>> = rows.iter().map(|(h, _)| h.clone()).collect();
    let translated: Vec<String> = rows
        .iter()
        .map(|(_, i)| i.translated_text.clone())
        .collect();
    let languages: Vec<String> = rows
        .iter()
        .map(|(_, i)| i.detected_language.clone())
        .collect();
    let confidences: Vec<f32> = rows.iter().map(|(_, i)| i.detected_confidence).collect();
    sqlx::query(
        "INSERT INTO social.translation_cache \
           (backend, target_lang, text_sha256, translated_text, detected_language, detected_confidence) \
         SELECT $1, $2, h, t, l, c \
         FROM UNNEST($3::bytea[], $4::text[], $5::text[], $6::real[]) AS u(h, t, l, c) \
         ON CONFLICT (backend, target_lang, text_sha256) DO UPDATE SET \
           translated_text = EXCLUDED.translated_text, \
           detected_language = EXCLUDED.detected_language, \
           detected_confidence = EXCLUDED.detected_confidence",
    )
    .bind(backend)
    .bind(target)
    .bind(&hashes)
    .bind(&translated)
    .bind(&languages)
    .bind(&confidences)
    .execute(pool)
    .await?;
    Ok(())
}

/// Rows deleted per statement; each statement is its own short transaction.
pub const PRUNE_BATCH: i64 = 1000;
/// Statements per pass, so a huge backlog is drained over passes, never in one.
const PRUNE_MAX_BATCHES: usize = 200;

/// Deletes rows whose `created_at` is older than `now - ttl`, [`PRUNE_BATCH`]
/// at a time, until a batch comes back short, the batch cap is reached or the
/// job is cancelled. The cutoff is a bind: the cache table has no time index
/// (the prune is a rare sequential pass over a cache),
/// so this is a measured candidate, not an indexed due set.
pub async fn prune_older_than(
    pool: &PgPool,
    now: NaiveDateTime,
    ttl: Duration,
    ctx: &JobContext,
) -> anyhow::Result<u64> {
    let cutoff = now - chrono::Duration::from_std(ttl)?;
    let mut total = 0u64;
    for _ in 0..PRUNE_MAX_BATCHES {
        if ctx.is_cancelled() {
            break;
        }
        let deleted = sqlx::query(
            "WITH victims AS (\
               SELECT ctid FROM social.translation_cache WHERE created_at < $1 LIMIT $2\
             ) \
             DELETE FROM social.translation_cache t USING victims WHERE t.ctid = victims.ctid",
        )
        .bind(cutoff)
        .bind(PRUNE_BATCH)
        .execute(pool)
        .await?
        .rows_affected();
        total += deleted;
        if deleted < PRUNE_BATCH as u64 {
            break;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str) -> TranslatedItem {
        TranslatedItem {
            translated_text: text.to_string(),
            detected_language: "en".to_string(),
            detected_confidence: 0.9,
        }
    }

    async fn ctx() -> JobContext {
        let jobs = crate::jobs::Jobs::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        jobs.spawn("ctx", move |ctx| async move {
            let _ = tx.send(ctx.clone());
            ctx.cancel.cancelled().await;
            Ok(())
        });
        rx.await.unwrap()
    }

    #[test]
    fn hashes_are_sha256_of_the_utf8_text() {
        let h = hash_text("hola");
        assert_eq!(h.len(), 32);
        assert_eq!(
            hex::encode(&h),
            "b221d9dbb083a7f33428d7c2a3c3198ae925614d70210e28716ccaa7cd4ddb79"
        );
    }

    /// Store, fetch back, upsert over the same key, and a prune with the
    /// current time as a bind that removes the aged row and keeps the fresh
    /// one; rows are scoped by a uuid engine label and deleted at the end.
    #[tokio::test]
    async fn store_fetch_upsert_and_prune_on_the_scratch_database() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let backend = format!("test-{}", uuid::Uuid::new_v4().simple());
        let (a, b) = (hash_text("one"), hash_text("two"));
        store(
            &pool,
            &backend,
            "es",
            &[(a.clone(), &item("uno")), (b.clone(), &item("dos"))],
        )
        .await
        .unwrap();
        let mut rows = fetch(
            &pool,
            &backend,
            "es",
            &[a.clone(), b.clone(), hash_text("three")],
        )
        .await
        .unwrap();
        rows.sort_by(|x, y| x.translated_text.cmp(&y.translated_text));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].translated_text, "dos");
        assert_eq!(rows[1].translated_text, "uno");
        assert_eq!(rows[1].detected_language, "en");
        assert!(
            fetch(&pool, &backend, "fr", std::slice::from_ref(&a))
                .await
                .unwrap()
                .is_empty(),
            "the language is part of the key"
        );
        assert!(fetch(&pool, &backend, "es", &[]).await.unwrap().is_empty());

        store(&pool, &backend, "es", &[(a.clone(), &item("UNO"))])
            .await
            .unwrap();
        let rows = fetch(&pool, &backend, "es", std::slice::from_ref(&a))
            .await
            .unwrap();
        assert_eq!(
            rows[0].translated_text, "UNO",
            "the same key is overwritten, not duplicated"
        );

        sqlx::query(
            "UPDATE social.translation_cache SET created_at = created_at - interval '40 days' \
             WHERE backend = $1 AND text_sha256 = $2",
        )
        .bind(&backend)
        .bind(&a)
        .execute(&pool)
        .await
        .unwrap();
        let now = chrono::Utc::now().naive_utc();
        let deleted = prune_older_than(&pool, now, Duration::from_secs(30 * 86_400), &ctx().await)
            .await
            .unwrap();
        assert!(deleted >= 1, "the aged row is pruned ({deleted})");
        let left: Vec<CachedRow> = fetch(&pool, &backend, "es", &[a.clone(), b.clone()])
            .await
            .unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].text_sha256, b, "the fresh row survives");

        sqlx::query("DELETE FROM social.translation_cache WHERE backend = $1")
            .bind(&backend)
            .execute(&pool)
            .await
            .unwrap();
    }
}

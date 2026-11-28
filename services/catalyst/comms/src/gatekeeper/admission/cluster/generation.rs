//! Read-only generation enforcement on serve's existing pool and transactions.
use super::fence::{FenceError, PulseAdmissionStamp};
use crate::interfaces::PulseGenerationBinding;
use sqlx::{PgPool, Postgres, Row, Transaction};

pub const OPERATION_VALIDITY: std::time::Duration = std::time::Duration::from_secs(2);

fn binding(
    audience: &str,
    row: sqlx::postgres::PgRow,
) -> Result<PulseGenerationBinding, FenceError> {
    let epoch: i64 = row.try_get("epoch").map_err(|_| FenceError::Unavailable)?;
    let source: String = row
        .try_get("source_incarnation")
        .map_err(|_| FenceError::Unavailable)?;
    if epoch <= 0 {
        return Err(FenceError::Unavailable);
    }
    let value = PulseGenerationBinding {
        audience: audience.into(),
        epoch: crate::casts::non_negative(epoch),
        source_incarnation: source,
    };
    value
        .valid()
        .then_some(value)
        .ok_or(FenceError::Unavailable)
}
pub async fn current(pool: &PgPool, audience: &str) -> Result<PulseGenerationBinding, FenceError> {
    let row = sqlx::query("SELECT epoch,source_incarnation::text FROM social.pulse_authority_generation WHERE deployment_audience=$1")
        .bind(audience).fetch_optional(pool).await.map_err(|_| FenceError::Unavailable)?.ok_or(FenceError::Unavailable)?;
    binding(audience, row)
}
/// Always before wallet safety/realm locks. No external await is permitted in this transaction.
pub async fn lock(
    tx: &mut Transaction<'_, Postgres>,
    audience: &str,
    source: &str,
) -> Result<PulseGenerationBinding, FenceError> {
    let row = sqlx::query("SELECT epoch,source_incarnation::text FROM social.pulse_authority_generation WHERE deployment_audience=$1 FOR SHARE")
        .bind(audience).fetch_optional(&mut **tx).await.map_err(|_| FenceError::Unavailable)?.ok_or(FenceError::Unavailable)?;
    let value = binding(audience, row)?;
    if value.source_incarnation != source {
        return Err(FenceError::Conflict);
    }
    Ok(value)
}
/// Exact, short completion permission. Generic cached generation status grants nothing.
pub struct GenerationGrant {
    binding: PulseGenerationBinding,
    stamp: PulseAdmissionStamp,
    expires: tokio::time::Instant,
    used: std::sync::atomic::AtomicBool,
}
impl GenerationGrant {
    pub fn binding(&self) -> &PulseGenerationBinding {
        &self.binding
    }
    pub fn consume(&self, stamp: &PulseAdmissionStamp) -> bool {
        self.authorizes(stamp)
            && self
                .used
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
    }
    pub fn authorizes(&self, stamp: &PulseAdmissionStamp) -> bool {
        !self.used.load(std::sync::atomic::Ordering::SeqCst)
            && tokio::time::Instant::now() < self.expires
            && self.stamp == *stamp
            && self.binding.source_incarnation == stamp.source_incarnation
    }
}
pub async fn authorize(
    pool: &PgPool,
    audience: &str,
    stamp: &PulseAdmissionStamp,
) -> Result<GenerationGrant, FenceError> {
    let started = tokio::time::Instant::now();
    if !stamp.valid() {
        return Err(FenceError::Invalid);
    }
    let deadline = started + OPERATION_VALIDITY;
    tokio::time::timeout_at(deadline, async {
        let mut tx = pool.begin().await.map_err(|_| FenceError::Unavailable)?;
        sqlx::query("SET LOCAL lock_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        sqlx::query("SET LOCAL statement_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| FenceError::Unavailable)?;
        let binding = lock(&mut tx, audience, &stamp.source_incarnation).await?;
        tx.commit().await.map_err(|_| FenceError::Unavailable)?;
        let grant = GenerationGrant {
            binding,
            stamp: stamp.clone(),
            expires: deadline,
            used: std::sync::atomic::AtomicBool::new(false),
        };
        grant
            .authorizes(stamp)
            .then_some(grant)
            .ok_or(FenceError::Unavailable)
    })
    .await
    .map_err(|_| FenceError::Unavailable)?
}

#[cfg(test)]
mod tests {
    use super::super::fence::SelectedRoomOrigin;
    use super::*;
    #[test]
    fn exact_operation_permission_is_single_use_and_never_refreshes_its_deadline() {
        let source = uuid::Uuid::new_v4().to_string();
        let stamp = PulseAdmissionStamp {
            selected_origin: SelectedRoomOrigin::Native,
            selected_identity: 1,
            wallet: format!("0x{:040x}", 1),
            session: format!("0x{:040x}", 2),
            cluster_id: "room".into(),
            realm: "realm".into(),
            source_incarnation: source.clone(),
            assignment_version: 1,
            token_not_before: 1,
            credential_not_before: 1,
            safety_generation: 0,
            safety_token_floor: 0,
        };
        let mut grant = GenerationGrant {
            binding: PulseGenerationBinding {
                audience: "fixture".into(),
                epoch: 1,
                source_incarnation: source,
            },
            stamp: stamp.clone(),
            expires: tokio::time::Instant::now() + OPERATION_VALIDITY,
            used: std::sync::atomic::AtomicBool::new(false),
        };
        let mut other = stamp.clone();
        other.assignment_version += 1;
        assert!(!grant.consume(&other));
        assert!(grant.consume(&stamp));
        assert!(!grant.consume(&stamp));
        grant.used.store(false, std::sync::atomic::Ordering::SeqCst);
        grant.expires = tokio::time::Instant::now() - std::time::Duration::from_millis(1);
        assert!(!grant.consume(&stamp));
    }
}

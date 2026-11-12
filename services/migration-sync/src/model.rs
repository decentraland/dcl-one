//! The bounded, typed input contract for the Places favorites pilot.
//!
//! An envelope represents one committed source transaction. Changes retain
//! their source order, including multiple changes to the same favorite.

use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_CHANGES: usize = 1_000;
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub version: u32,
    pub source: String,
    pub stream: String,
    pub generation: Uuid,
    pub previous_lsn: String,
    pub commit_lsn: String,
    pub mode: Mode,
    pub changes: Vec<FavoriteChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<FavoriteCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like_command: Option<LikeCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poi_result: Option<crate::poi::PoiResult>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    SourceOnly,
    Dual,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum FavoriteChange {
    PoiMembership {
        entity_id: String,
        poi: bool,
    },
    LikeUpsert {
        user: String,
        entity_id: String,
        like: bool,
        user_activity: f64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    },
    LikeDelete {
        user: String,
        entity_id: String,
    },
    Upsert {
        user: String,
        entity_id: String,
        user_activity: f64,
        created_at: DateTime<Utc>,
    },
    Delete {
        user: String,
        entity_id: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    ApplyPoi,
    ExecuteCommand,
    Blocked(&'static str),
}

/// An accepted source command and its state observed in the same transaction.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FavoriteCommand {
    pub operation_id: Uuid,
    pub request_hash: String,
    pub user: String,
    pub entity_id: String,
    pub favorite: bool,
    pub user_activity: f64,
    pub expected: Option<FavoriteState>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FavoriteState {
    pub user_activity: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LikeState {
    pub like: bool,
    pub user_activity: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LikeCommand {
    pub operation_id: Uuid,
    pub request_hash: String,
    pub user: String,
    pub entity_id: String,
    pub like: Option<bool>,
    pub user_activity: f64,
    pub expected: Option<LikeState>,
}
impl LikeCommand {
    pub fn validate(&self) -> Result<()> {
        FavoriteCommand {
            operation_id: self.operation_id,
            request_hash: self.request_hash.clone(),
            user: self.user.clone(),
            entity_id: self.entity_id.clone(),
            favorite: self.like.is_some(),
            user_activity: self.user_activity,
            expected: self.expected.as_ref().map(|v| FavoriteState {
                user_activity: v.user_activity,
                created_at: v.created_at,
            }),
        }
        .validate()?;
        ensure!(
            self.like == self.expected.as_ref().map(|v| v.like),
            "source vote disagrees with command"
        );
        if let Some(state) = &self.expected {
            ensure!(
                state.updated_at.timestamp_subsec_nanos() % 1000 == 0,
                "source timestamp precision unsupported"
            );
        }
        Ok(())
    }
}

impl FavoriteCommand {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.operation_id.is_nil(), "operation ID must not be nil");
        ensure!(
            self.request_hash.len() == 64
                && self
                    .request_hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "request hash must be lowercase SHA256"
        );
        FavoriteChange::Delete {
            user: self.user.clone(),
            entity_id: self.entity_id.clone(),
        }
        .validate()?;
        ensure!(
            self.user_activity.is_finite(),
            "user activity must be finite"
        );
        ensure!(
            self.favorite == self.expected.is_some(),
            "source command result disagrees with requested favorite state"
        );
        if let Some(state) = &self.expected {
            ensure!(
                state.user_activity.is_finite(),
                "expected activity must be finite"
            );
            ensure!(
                state.created_at.timestamp_subsec_nanos() % 1000 == 0,
                "source timestamp must have PostgreSQL microsecond precision"
            );
        }
        Ok(())
    }
}

impl Envelope {
    /// Validate before retaining or classifying the envelope. Errors identify
    /// the violated contract without including wallet or payload values.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == SCHEMA_VERSION,
            "unsupported envelope version"
        );
        ensure!(self.source == "places", "unsupported source");
        validate_stream(&self.stream)?;
        ensure!(!self.generation.is_nil(), "generation must not be nil");
        let previous = parse_lsn(&self.previous_lsn)?;
        let committed = parse_lsn(&self.commit_lsn)?;
        ensure!(previous < committed, "commit LSN must follow previous LSN");
        ensure!(self.changes.len() <= MAX_CHANGES, "too many changes");
        for change in &self.changes {
            change.validate()?;
        }
        ensure!(
            usize::from(self.command.is_some())
                + usize::from(self.like_command.is_some())
                + usize::from(self.poi_result.is_some())
                <= 1,
            "multiple transaction execution mechanisms"
        );
        if let Some(result) = &self.poi_result {
            ensure!(
                self.mode == Mode::SourceOnly,
                "POI result cannot be native replay"
            );
            result.validate()?;
            ensure!(self.changes.iter().all(|change|matches!(change,FavoriteChange::PoiMembership {entity_id,..} if entity_id==&result.entity_id)),"POI result contains unrelated changes");
            if let Some(FavoriteChange::PoiMembership { poi, .. }) = self.changes.last() {
                ensure!(*poi == result.poi, "POI evidence disagrees with result");
            }
        }
        ensure!(
            self.command.is_none() || self.like_command.is_none(),
            "transaction has multiple command kinds"
        );
        if let Some(command) = &self.like_command {
            ensure!(
                self.mode == Mode::Dual,
                "source-only transactions cannot carry commands"
            );
            command.validate()?;
            for change in &self.changes {
                ensure!(
                    change.key() == (command.user.as_str(), command.entity_id.as_str()),
                    "like command changes escape scope"
                );
                ensure!(
                    matches!(
                        change,
                        FavoriteChange::LikeUpsert { .. } | FavoriteChange::LikeDelete { .. }
                    ),
                    "like command contains another operation"
                );
            }
            if let Some(change) = self.changes.last() {
                match (change, &command.expected) {
                    (
                        FavoriteChange::LikeUpsert {
                            like,
                            user_activity,
                            created_at,
                            updated_at,
                            ..
                        },
                        Some(v),
                    ) => ensure!(
                        *like == v.like
                            && *user_activity == v.user_activity
                            && *created_at == v.created_at
                            && *updated_at == v.updated_at,
                        "captured vote disagrees with receipt"
                    ),
                    (FavoriteChange::LikeDelete { .. }, None) => {}
                    _ => anyhow::bail!("captured vote disagrees with receipt"),
                }
            }
        }
        if let Some(command) = &self.command {
            ensure!(
                self.mode == Mode::Dual,
                "source-only transactions cannot carry commands"
            );
            command.validate()?;
            ensure!(
                self.changes.iter().all(|change| change.key()
                    == (command.user.as_str(), command.entity_id.as_str())
                    && matches!(
                        change,
                        FavoriteChange::Upsert { .. } | FavoriteChange::Delete { .. }
                    )),
                "dual transaction changes escape command scope"
            );
            if let Some(change) = self.changes.last() {
                match (change, &command.expected) {
                    (
                        FavoriteChange::Upsert {
                            user_activity,
                            created_at,
                            ..
                        },
                        Some(expected),
                    ) => {
                        ensure!(
                            *user_activity == expected.user_activity
                                && *created_at == expected.created_at,
                            "captured changes disagree with source command result"
                        );
                    }
                    (FavoriteChange::Delete { .. }, None) => {}
                    _ => anyhow::bail!("captured changes disagree with source command result"),
                }
            }
        }
        Ok(())
    }

    /// A capture marker alone cannot prove execution. A source command permits
    /// dispatch; advancement separately requires its atomic native receipt.
    pub fn classify(&self) -> Decision {
        match self.mode {
            Mode::SourceOnly if self.poi_result.is_some() => Decision::ApplyPoi,
            Mode::SourceOnly => Decision::Blocked("unreceipted_mutation"),
            Mode::Dual if self.command.is_some() || self.like_command.is_some() => {
                Decision::ExecuteCommand
            }
            Mode::Dual => Decision::Blocked("dual_receipts_required"),
        }
    }
}

impl FavoriteChange {
    pub fn key(&self) -> (&str, &str) {
        match self {
            Self::PoiMembership { entity_id, .. } => ("", entity_id),
            Self::Upsert {
                user, entity_id, ..
            }
            | Self::Delete { user, entity_id }
            | Self::LikeUpsert {
                user, entity_id, ..
            }
            | Self::LikeDelete { user, entity_id } => (user, entity_id),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if let Self::PoiMembership { entity_id, poi } = self {
            return crate::poi::PoiResult {
                producer: crate::poi::PRODUCER.into(),
                entity_id: entity_id.clone(),
                poi: *poi,
            }
            .validate();
        }

        if let Self::LikeUpsert {
            user,
            entity_id,
            user_activity,
            created_at,
            updated_at,
            ..
        } = self
        {
            ensure!(
                updated_at.timestamp_subsec_nanos() % 1000 == 0,
                "source timestamp precision unsupported"
            );
            return Self::Upsert {
                user: user.clone(),
                entity_id: entity_id.clone(),
                user_activity: *user_activity,
                created_at: *created_at,
            }
            .validate();
        }

        let (user, entity_id) = self.key();
        ensure!(
            user.len() == 42
                && user.starts_with("0x")
                && user[2..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "wallet must be canonical lowercase hex"
        );
        let entity = Uuid::parse_str(entity_id)
            .map_err(|_| anyhow::anyhow!("entity ID must be a canonical UUID"))?;
        ensure!(
            entity.hyphenated().to_string() == entity_id,
            "entity ID must be a canonical UUID"
        );
        if let Self::Upsert { user_activity, .. } = self {
            ensure!(user_activity.is_finite(), "user activity must be finite");
        }
        Ok(())
    }
}

pub fn validate_stream(stream: &str) -> Result<()> {
    ensure!(
        !stream.is_empty()
            && stream.len() <= 64
            && stream
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "stream must contain 1 to 64 ASCII letters, digits, hyphens or underscores"
    );
    Ok(())
}

/// Parse the canonical PostgreSQL LSN representation: two uppercase hex u32
/// components without zero padding. Comparisons use the combined u64 value.
pub fn parse_lsn(value: &str) -> Result<u64> {
    let (high, low) = value
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("LSN must be canonical HEX/HEX"))?;
    let component = |part: &str| -> Result<u32> {
        ensure!(
            !part.is_empty()
                && part.len() <= 8
                && (part == "0" || !part.starts_with('0'))
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte)),
            "LSN must be canonical HEX/HEX"
        );
        u32::from_str_radix(part, 16).map_err(|_| anyhow::anyhow!("LSN must be canonical HEX/HEX"))
    };
    Ok((u64::from(component(high)?) << 32) | u64::from(component(low)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> serde_json::Value {
        json!({
            "version": 1,
            "source": "places",
            "stream": "places-favorites",
            "generation": "10000000-0000-4000-8000-000000000001",
            "previous_lsn": "0/FF",
            "commit_lsn": "1/0",
            "mode": "source_only",
            "changes": [{
                "op": "upsert",
                "user": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "entity_id": "10000000-0000-4000-8000-000000000002",
                "user_activity": 1.5,
                "created_at": "2026-11-12T10:00:00Z"
            }]
        })
    }

    fn envelope() -> Envelope {
        serde_json::from_value(fixture()).expect("valid fixture")
    }

    #[test]
    fn unreceipted_transaction_requires_an_explicit_residual_mapping() {
        let envelope = envelope();
        envelope.validate().expect("valid source transaction");
        assert_eq!(
            envelope.classify(),
            Decision::Blocked("unreceipted_mutation")
        );
    }

    #[test]
    fn dual_marker_does_not_prove_native_execution() {
        let mut envelope = envelope();
        envelope.mode = Mode::Dual;
        envelope.validate().expect("valid capture marker");
        assert_eq!(
            envelope.classify(),
            Decision::Blocked("dual_receipts_required")
        );
    }

    #[test]
    fn dual_command_requires_matching_canonical_source_evidence() {
        let mut value = fixture();
        value["mode"] = json!("dual");
        value["command"] = json!({
            "operation_id":"10000000-0000-4000-8000-000000000003",
            "request_hash":"a".repeat(64),
            "user":value["changes"][0]["user"],
            "entity_id":value["changes"][0]["entity_id"],
            "favorite":true,"user_activity":1.5,
            "expected":{"user_activity":1.5,"created_at":"2026-11-12T10:00:00Z"}
        });
        let valid: Envelope = serde_json::from_value(value.clone()).unwrap();
        valid.validate().unwrap();
        assert_eq!(valid.classify(), Decision::ExecuteCommand);
        value["command"]["expected"]["user_activity"] = json!(2.0);
        assert!(
            serde_json::from_value::<Envelope>(value.clone())
                .unwrap()
                .validate()
                .is_err()
        );
        value["changes"] = json!([]);
        // No-op commands have no row event, but still require native receipt.
        assert!(
            serde_json::from_value::<Envelope>(value.clone())
                .unwrap()
                .validate()
                .is_ok()
        );
        value["mode"] = json!("source_only");
        assert!(
            serde_json::from_value::<Envelope>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }

    #[test]
    fn lsn_order_uses_both_u32_components() {
        assert_eq!(parse_lsn("0/0").unwrap(), 0);
        assert_eq!(parse_lsn("FFFFFFFF/FFFFFFFF").unwrap(), u64::MAX);
        assert!(parse_lsn("1/0").unwrap() > parse_lsn("0/FFFFFFFF").unwrap());
    }

    #[test]
    fn noncanonical_or_overflowing_lsns_are_rejected() {
        for invalid in [
            "",
            "0",
            "/0",
            "0/",
            "0/0/0",
            "00/0",
            "0/01",
            "a/0",
            "0/ff",
            "0x1/0",
            "100000000/0",
            "0/100000000",
            "-1/0",
            "+1/0",
            " 0/0",
            "0/0\n",
            "é/0",
        ] {
            assert!(parse_lsn(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn transaction_position_must_advance() {
        for commit in ["0/FE", "0/FF"] {
            let mut envelope = envelope();
            envelope.commit_lsn = commit.to_owned();
            assert!(envelope.validate().is_err());
        }
    }

    #[test]
    fn unsupported_sources_and_versions_are_rejected() {
        let mut envelope = envelope();
        envelope.version = 2;
        assert!(envelope.validate().is_err());
        envelope.version = SCHEMA_VERSION;
        for source in [
            "shop-server",
            "credits-server",
            "marketplace-favorites-server",
            "",
        ] {
            envelope.source = source.to_owned();
            assert!(envelope.validate().is_err());
        }
    }

    #[test]
    fn stream_identifiers_are_bounded_and_cannot_name_sql() {
        for invalid in ["", "a/b", "a.b", "a b", "é", "places;drop"] {
            assert!(validate_stream(invalid).is_err());
        }
        assert!(validate_stream(&"a".repeat(65)).is_err());
        assert!(validate_stream(&"a".repeat(64)).is_ok());
        assert!(validate_stream("Places_1-favorites").is_ok());
    }

    #[test]
    fn generation_cannot_be_unspecified() {
        let mut envelope = envelope();
        envelope.generation = Uuid::nil();
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn malformed_wallets_and_unsupported_entity_ids_are_rejected() {
        let valid = envelope().changes.remove(0);
        for wallet in [
            "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "0Xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "0xgaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "ééééééééééééééééééééé",
        ] {
            let mut change = valid.clone();
            if let FavoriteChange::Upsert { user, .. } = &mut change {
                *user = wallet.to_owned();
            }
            assert!(change.validate().is_err());
        }
        for entity in [
            "test.eth",
            "10000000000040008000000000000002",
            "A0000000-0000-4000-8000-000000000002",
            "urn:uuid:10000000-0000-4000-8000-000000000002",
        ] {
            let mut change = valid.clone();
            if let FavoriteChange::Upsert { entity_id, .. } = &mut change {
                *entity_id = entity.to_owned();
            }
            assert!(change.validate().is_err());
        }
    }

    #[test]
    fn nonfinite_activity_is_rejected_before_storage() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut envelope = envelope();
            if let FavoriteChange::Upsert { user_activity, .. } = &mut envelope.changes[0] {
                *user_activity = invalid;
            }
            assert!(envelope.validate().is_err());
        }
    }

    #[test]
    fn extra_fields_cannot_smuggle_table_names_or_execution_claims() {
        for field in ["table", "sql", "already_executed"] {
            let mut value = fixture();
            value[field] = json!("untrusted");
            assert!(serde_json::from_value::<Envelope>(value).is_err());
            let mut value = fixture();
            value["changes"][0][field] = json!("untrusted");
            assert!(serde_json::from_value::<Envelope>(value).is_err());
        }
    }

    #[test]
    fn invalid_modes_changes_and_timestamps_are_rejected() {
        for (path, value) in [
            ("mode", "bidirectional"),
            ("op", "truncate"),
            ("created_at", "not-a-date"),
        ] {
            let mut payload = fixture();
            if path == "mode" {
                payload[path] = json!(value);
            } else {
                payload["changes"][0][path] = json!(value);
            }
            assert!(serde_json::from_value::<Envelope>(payload).is_err());
        }
    }

    #[test]
    fn transaction_changes_remain_ordered_even_for_the_same_key() {
        let mut envelope = envelope();
        let (user, entity_id) = envelope.changes[0].key();
        envelope.changes.push(FavoriteChange::Delete {
            user: user.to_owned(),
            entity_id: entity_id.to_owned(),
        });
        envelope.validate().expect("upsert then delete is valid");
        let round_trip: Envelope =
            serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
        assert_eq!(round_trip, envelope);
        assert!(matches!(
            round_trip.changes[0],
            FavoriteChange::Upsert { .. }
        ));
        assert!(matches!(
            round_trip.changes[1],
            FavoriteChange::Delete { .. }
        ));
    }

    #[test]
    fn change_count_is_bounded_without_rejecting_empty_transactions() {
        let mut envelope = envelope();
        let change = envelope.changes[0].clone();
        envelope.changes.clear();
        envelope
            .validate()
            .expect("empty transaction can advance capture");
        envelope.changes = vec![change.clone(); MAX_CHANGES];
        envelope.validate().expect("limit is inclusive");
        envelope.changes.push(change);
        assert!(envelope.validate().is_err());
    }
}

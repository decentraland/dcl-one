//! `PUT /v1/collections/:id`: builder-server's `upsertCollectionSchema` and
//! upsert rules. The body is checked against the schema's types and bounds,
//! with every key named (a key the schema doesn't name is refused, where
//! builder-server drops it). A standard collection's salt and contract address
//! are computed here from its id and the `initialize` calldata the client
//! sends ([`super::contract`]), never taken from the body; a body never sets
//! the owner, managers, minters or published state. The write runs in one
//! transaction that holds the collection row and an advisory lock on the
//! name (standard) or URN (third party) it claims and, for a third-party
//! collection, on its provider: a save that creates the first collection of
//! a provider neither the registry nor a virtual third party has creates the
//! provider's virtual third party with the signer as its manager, and a
//! linked contract is listed on the provider's virtual third party, as
//! builder-server keeps it.

use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::access::{self, CollectionEdit};
use crate::chain_index::third_parties::LinkedContract;
use crate::chain_state;
use crate::http::ApiError;
use crate::items::ItemsComponent;
use crate::items::schema::{MAPPING_NETWORKS, URN_NETWORKS};
use crate::network::Network;
use crate::third_parties::{parse_raw_metadata, raw_metadata};

use super::{contract, project, row};

pub(crate) const TEXT_MAX: usize = 256;
pub(crate) const LIST_MAX: usize = 1000;
pub(crate) const DATA_MAX: usize = 1 << 20;
pub(crate) const NAME_MAX: usize = 42;

/// What a valid body asks for.
#[derive(Debug)]
pub struct CollectionSave {
    pub id: String,
    pub name: String,
    pub urn: Option<String>,
    pub claims_publication: bool,
    pub linked_contract_address: Option<String>,
    pub linked_contract_network: Option<String>,
    pub data: Option<String>,
}

/// A third-party collection URN's provider id and collection suffix.
pub fn third_party_urn(urn: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = urn.split(':').collect();
    let slot = |s: &str| !s.is_empty() && !s.chars().any(|c| c == '|' || c.is_whitespace());
    (parts.len() == 6
        && parts[..2] == ["urn", "decentraland"]
        && URN_NETWORKS.contains(&parts[2])
        && parts[3] == "collections-thirdparty"
        && slot(parts[4])
        && slot(parts[5]))
    .then(|| (&urn[..urn.len() - parts[5].len() - 1], parts[5]))
}

fn standard_urn(urn: &str) -> bool {
    let parts: Vec<&str> = urn.split(':').collect();
    parts.len() == 5
        && parts[..2] == ["urn", "decentraland"]
        && URN_NETWORKS.contains(&parts[2])
        && parts[3] == "collections-v2"
        && dcl_auth::is_eth_address(parts[4])
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && Uuid::parse_str(s).is_ok()
}

fn text(v: &Value) -> Option<&str> {
    v.as_str().filter(|s| s.chars().count() <= TEXT_MAX)
}

fn nullable(v: &Value, check: impl Fn(&str) -> bool) -> bool {
    v.is_null() || v.as_str().is_some_and(check)
}

fn field_ok(key: &str, v: &Value) -> bool {
    match key {
        "id" => v.as_str().is_some_and(is_uuid),
        "urn" => nullable(v, |s| {
            s.len() <= 512 && (third_party_urn(s).is_some() || standard_urn(s))
        }),
        "name" => v
            .as_str()
            .is_some_and(|s| s.chars().count() <= NAME_MAX && !s.trim().is_empty()),
        "eth_address" | "created_at" | "updated_at" => text(v).is_some(),
        "salt" | "contract_address" | "reviewed_at" => v.is_null() || text(v).is_some(),
        "is_published" | "is_approved" => v.is_boolean(),
        "minters" | "managers" => v
            .as_array()
            .is_some_and(|a| a.len() <= LIST_MAX && a.iter().all(|e| text(e).is_some())),
        "forum_link" => nullable(v, |s| s.chars().count() <= 512 && s.starts_with("https://")),
        "forum_id" => {
            v.is_null() || v.as_i64().is_some() || v.as_f64().is_some_and(|f| f.fract() == 0.0)
        }
        "linked_contract_address" => nullable(v, dcl_auth::is_eth_address),
        "linked_contract_network" => nullable(v, |s| MAPPING_NETWORKS.contains(&s)),
        _ => false,
    }
}

fn invalid_body() -> ApiError {
    ApiError::bad_request("Invalid request body")
}

fn opt_text(fields: &Map<String, Value>, key: &str) -> Option<String> {
    fields.get(key).and_then(Value::as_str).map(str::to_string)
}

/// The body checked against `upsertCollectionSchema` and its bounds.
pub fn parse(body: &Value) -> Result<CollectionSave, ApiError> {
    let top = body.as_object().ok_or_else(invalid_body)?;
    if top.keys().any(|k| k != "collection" && k != "data") {
        return Err(invalid_body());
    }
    let data = match top.get("data") {
        None => None,
        Some(Value::String(d)) if d.len() <= DATA_MAX => Some(d.clone()),
        Some(_) => return Err(invalid_body()),
    };
    let fields = top
        .get("collection")
        .and_then(Value::as_object)
        .ok_or_else(invalid_body)?;
    let required = ["id", "name", "eth_address", "salt", "contract_address"];
    if required.iter().any(|k| !fields.contains_key(*k))
        || fields.iter().any(|(k, v)| !field_ok(k, v))
    {
        return Err(invalid_body());
    }
    let flag = |key: &str| fields.get(key).and_then(Value::as_bool).unwrap_or(false);
    Ok(CollectionSave {
        id: opt_text(fields, "id").unwrap_or_default(),
        name: opt_text(fields, "name")
            .unwrap_or_default()
            .trim()
            .to_string(),
        urn: opt_text(fields, "urn"),
        claims_publication: flag("is_published") || flag("is_approved"),
        linked_contract_address: opt_text(fields, "linked_contract_address")
            .map(|a| a.to_ascii_lowercase()),
        linked_contract_network: opt_text(fields, "linked_contract_network"),
        data,
    })
}

/// What a standard save writes beside the body's fields.
pub struct Deployment {
    salt: String,
    contract_address: String,
}

impl Deployment {
    /// The address the save gives the collection.
    pub fn contract_address(&self) -> &str {
        &self.contract_address
    }
}

/// The checks a standard save makes before it asks who the signer is:
/// `data` present and no claim of publication.
pub fn standard_request(save: &CollectionSave, signer: &str) -> Result<(), ApiError> {
    if save.data.as_deref().is_none_or(str::is_empty) {
        return Err(ApiError::http(200, "Could not get data from request"));
    }
    if save.claims_publication {
        return Err(
            ApiError::conflict("Cannot change the is_published or is_approved property")
                .with_data(json!({ "id": save.id, "eth_address": signer })),
        );
    }
    Ok(())
}

fn invalid_data(save: &CollectionSave, signer: &str) -> ApiError {
    ApiError::conflict("Cannot upsert a collection without a valid data")
        .with_data(json!({ "id": save.id, "eth_address": signer }))
}

/// What a save whose `initialize` data names a token metadata base other
/// than the network's or this node's is told.
pub const FOREIGN_BASE_URI: &str =
    "The collection's token metadata base is neither this node's nor the network's";

/// The salt and contract address on `network` of a standard save whose
/// signer may save it: `data` must be `initialize` calldata naming the
/// signer and, as its token metadata base, the network's or the one the
/// node at `node` serves.
pub fn standard_deployment(
    network: &Network,
    node: &str,
    save: &CollectionSave,
    signer: &str,
) -> Result<Deployment, ApiError> {
    standard_request(save, signer)?;
    let data = save.data.as_deref().unwrap_or_default();
    let (bytes, base_uri) =
        contract::initialize_for(data, signer).ok_or_else(|| invalid_data(save, signer))?;
    if !network.serves_token_metadata(&base_uri, node) {
        return Err(ApiError::bad_request(FOREIGN_BASE_URI)
            .with_data(json!({ "id": save.id, "base_uri": base_uri })));
    }
    let salt = contract::salt(&save.id);
    Ok(Deployment {
        contract_address: contract::address(network, salt, &bytes),
        salt: format!("{salt:#x}"),
    })
}

/// builder-server's answers to a write its database refuses: a linked
/// contract another collection of the provider links, or a linked contract
/// cleared. Its handler throws them without a status, so they answer 200
/// with `ok: false`.
fn linked_contract_refusal(e: sqlx::Error) -> ApiError {
    match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("collections_linked_contract_key") => {
            ApiError::http(
                200,
                "duplicate key value violates unique constraint \"linkedContract_linkedNetwork_thirdPartyId_unique\"",
            )
        }
        sqlx::Error::Database(db)
            if db.code().as_deref() == Some("P0001")
                && db.message().starts_with("Columns linked_contract_address") =>
        {
            ApiError::http(
                200,
                "Columns linked_contract_address, linked_contract_network cannot be set to null if they were previously not null",
            )
        }
        _ => e.into(),
    }
}

fn same_contract(a: &LinkedContract, b: &LinkedContract) -> bool {
    a.network == b.network && a.address.eq_ignore_ascii_case(&b.address)
}

/// builder-server's `syncVirtualThirdPartyContracts`: the provider's virtual
/// third party, if any, lists `next` in place of `previous` and keeps the
/// contracts its other collections link; nothing is written when it already
/// reads that way.
async fn list_contract(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    provider: &str,
    previous: Option<&LinkedContract>,
    next: &LinkedContract,
) -> Result<(), ApiError> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT raw_metadata FROM builder.virtual_third_parties WHERE id = $1 FOR UPDATE",
    )
    .bind(provider)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(raw) = raw else {
        return Ok(());
    };
    let (name, description, contracts) = parse_raw_metadata(&raw);
    let mut kept: Vec<LinkedContract> = contracts
        .iter()
        .filter(|c| {
            previous
                .is_none_or(|previous| same_contract(previous, next) || !same_contract(c, previous))
        })
        .cloned()
        .collect();
    if !kept.iter().any(|c| same_contract(c, next)) {
        kept.push(next.clone());
    }
    if kept.len() == contracts.len()
        && kept
            .iter()
            .zip(&contracts)
            .all(|(a, b)| same_contract(a, b))
    {
        return Ok(());
    }
    sqlx::query(
        "UPDATE builder.virtual_third_parties SET raw_metadata = $2, updated_at = timezone('UTC', now()) WHERE id = $1",
    )
    .bind(provider)
    .bind(raw_metadata(&name, &description, &kept))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn urn_in_use(id: Uuid, urn: &str, action: &str) -> ApiError {
    ApiError::conflict(format!(
        "The URN provided already belongs to a collection. The collection can't be {action}."
    ))
    .with_data(json!({ "id": id, "urn": urn }))
}

impl ItemsComponent {
    /// The save `edit` was decided for, in one transaction: the collection
    /// row `FOR UPDATE`, then the claimed name or URN under an advisory lock,
    /// then the decision again and the kind and URN rules, then the upsert.
    /// Refuses (413) a new collection of `owner`'s while the owner holds
    /// `documents.collections` unpublished ones: every collection it holds
    /// is counted first, the unpublished ones only past the bound.
    async fn within_collections(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &str,
    ) -> Result<(), ApiError> {
        let bound = i64::try_from(self.documents.collections).unwrap_or(i64::MAX);
        crate::items::save::lock_owner(tx, owner).await?;
        let held: i64 =
            sqlx::query_scalar("SELECT count(*) FROM builder.collections WHERE eth_address = $1")
                .bind(owner)
                .fetch_one(&mut **tx)
                .await?;
        if held < bound {
            return Ok(());
        }
        let (chain_join, published) = chain_state::published_by_any_pass();
        let unpublished: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM builder.collections c {chain_join} \
             WHERE c.eth_address = $1 AND NOT {published}"
        )))
        .bind(owner)
        .fetch_one(&mut **tx)
        .await?;
        if unpublished >= bound {
            return Err(crate::items::save::drafts_full(
                json!({ "maxCollections": self.documents.collections }),
            ));
        }
        Ok(())
    }

    /// `deployment` is the standard save's computed address; `None` saves a
    /// third-party collection.
    pub async fn save_collection(
        &self,
        edit: &CollectionEdit,
        save: &CollectionSave,
        deployment: Option<Deployment>,
    ) -> Result<Value, ApiError> {
        let (id, signer) = (edit.id(), edit.signer());
        let provider = save.urn.as_deref().and_then(third_party_urn);
        let mut tx = self.pool.begin().await?;
        let view = chain_state::for_decision(&mut tx).await?;
        let stored: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT third_party_id, urn_suffix FROM builder.collections WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let claim = match provider {
            Some(_) => sqlx::query(
                "SELECT pg_advisory_xact_lock(hashtextextended('builder.collections urn:' || $1, 0))",
            )
            .bind(save.urn.as_deref().unwrap_or_default()),
            None => sqlx::query(
                "SELECT pg_advisory_xact_lock(hashtextextended('builder.collections name:' || lower($1), 0))",
            )
            .bind(&save.name),
        };
        claim.execute(&mut *tx).await?;
        if let Some((provider, _)) = provider {
            crate::third_parties::lock_provider(&mut tx, provider).await?;
        }
        let previous_contract: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT linked_contract_address, linked_contract_network FROM builder.collections WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let state = access::confirm_collection_edit(&mut tx, &view, edit).await?;
        match (&stored, provider) {
            (Some((None, _)), Some(_)) => {
                return Err(ApiError::conflict(
                    "The collection can't be converted into a third party collection.",
                )
                .with_data(json!({ "id": id })));
            }
            (Some((Some(_), _)), None) => {
                return Err(ApiError::unauthorized("Unauthorized to upsert collection")
                    .with_data(json!({ "id": id, "eth_address": signer })));
            }
            (Some((Some(stored), _)), Some((provider, _))) if stored != provider => {
                return Err(
                    ApiError::conflict("The third party id cannot be changed").with_data(
                        json!({ "id": id, "eth_address": signer, "third_party_id": provider }),
                    ),
                );
            }
            _ => {}
        }
        if provider.is_none() {
            let name_taken: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM builder.collections \
                 WHERE lower(name) = lower($1) AND third_party_id IS NULL AND id <> $2)",
            )
            .bind(&save.name)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if name_taken {
                return Err(ApiError::conflict("Name already in use")
                    .with_data(json!({ "id": save.id, "name": save.name })));
            }
            if state.published {
                return Err(ApiError::published(
                    crate::http::Subject::Collection,
                    &id.to_string(),
                    "updated",
                ));
            }
        }
        if let (Some(_), Some(urn)) = (provider, save.urn.as_deref()) {
            let existing = stored.as_ref().map(|(_, urn)| urn.as_deref());
            if existing.is_some_and(|stored_urn| stored_urn != Some(urn)) && state.published {
                return Err(ApiError::conflict(
                    "The third party collection already has published items. It can't be updated or inserted.",
                )
                .with_data(json!({ "id": id })));
            }
            let taken: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM builder.collections \
                 WHERE third_party_id IS NOT NULL AND urn_suffix = $1 AND id <> $2)",
            )
            .bind(urn)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if taken && existing.is_none_or(|stored_urn| stored_urn != Some(urn)) {
                let action = if existing.is_some() {
                    "updated"
                } else {
                    "updated or inserted"
                };
                return Err(urn_in_use(id, urn, action));
            }
        }
        if state.pending || state.locked {
            return Err(ApiError::locked(
                crate::http::Subject::Collection,
                &id.to_string(),
                "updated",
            ));
        }
        if stored.is_none() {
            self.within_collections(&mut tx, signer).await?;
        }
        let saved: Option<Uuid> = match (provider, &deployment) {
            (Some((provider, _)), _) => sqlx::query_scalar(
                "INSERT INTO builder.collections AS c (id, name, eth_address, salt, urn_suffix, third_party_id, \
                   linked_contract_address, linked_contract_network) VALUES ($1, $2, $3, '', $4, $5, $6, $7) \
                 ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, salt = '', urn_suffix = EXCLUDED.urn_suffix, \
                   linked_contract_address = EXCLUDED.linked_contract_address, \
                   linked_contract_network = EXCLUDED.linked_contract_network, updated_at = timezone('UTC', now()) \
                 WHERE NOT c.publication_pending AND c.third_party_id = EXCLUDED.third_party_id \
                 RETURNING id",
            )
            .bind(id)
            .bind(&save.name)
            .bind(signer)
            .bind(&save.urn)
            .bind(provider)
            .bind(&save.linked_contract_address)
            .bind(&save.linked_contract_network)
            .fetch_optional(&mut *tx)
            .await
            .map_err(linked_contract_refusal)?,
            (None, Some(deployment)) => sqlx::query_scalar(
                "INSERT INTO builder.collections AS c (id, name, eth_address, salt, contract_address, \
                   linked_contract_address, linked_contract_network) VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, salt = EXCLUDED.salt, \
                   contract_address = EXCLUDED.contract_address, \
                   linked_contract_address = EXCLUDED.linked_contract_address, \
                   linked_contract_network = EXCLUDED.linked_contract_network, updated_at = timezone('UTC', now()) \
                 WHERE NOT c.publication_pending AND c.third_party_id IS NULL AND c.eth_address = EXCLUDED.eth_address \
                 RETURNING id",
            )
            .bind(id)
            .bind(&save.name)
            .bind(signer)
            .bind(&deployment.salt)
            .bind(&deployment.contract_address)
            .bind(&save.linked_contract_address)
            .bind(&save.linked_contract_network)
            .fetch_optional(&mut *tx)
            .await
            .map_err(linked_contract_refusal)?,
            (None, None) => return Err(ApiError::internal("a standard save without its deployment")),
        };
        if saved.is_none() {
            access::confirm_collection_edit(&mut tx, &view, edit).await?;
            return Err(ApiError::internal(format!(
                "collection {id} changed under its row lock"
            )));
        }
        if let (Some((_, Some(old))), Some(new)) = (&stored, save.urn.as_deref())
            && old != new
        {
            access::content_knows_items(&mut tx, edit.own_content()).await?;
            let deployed: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM builder.items i \
                   JOIN content.active_pointers ap ON ap.pointer = lower($2 || substr(i.urn_suffix, length($3) + 1)) \
                   JOIN content.deployments d ON d.entity_id = ap.entity_id \
                   WHERE i.collection_id = $1 AND lower(left(i.urn_suffix, length($3) + 1)) = lower($3 || ':') \
                     AND d.deleter_deployment IS NULL)",
            )
            .bind(id)
            .bind(new)
            .bind(old)
            .fetch_one(&mut *tx)
            .await?;
            if deployed {
                return Err(ApiError::conflict(
                    "The collection's items would take URNs the catalyst already holds",
                )
                .with_data(json!({ "id": id, "urn": new })));
            }
            sqlx::query(
                "UPDATE builder.items SET urn_suffix = $2 || substr(urn_suffix, length($3) + 1) \
                 WHERE collection_id = $1 AND lower(left(urn_suffix, length($3) + 1)) = lower($3 || ':')",
            )
            .bind(id)
            .bind(new)
            .bind(old)
            .execute(&mut *tx)
            .await?;
        }
        if let Some((provider, _)) = provider {
            let next = save
                .linked_contract_address
                .clone()
                .zip(save.linked_contract_network.clone())
                .map(|(address, network)| LinkedContract { address, network });
            if state.new_provider {
                sqlx::query(
                    "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, ARRAY[$2], $3)",
                )
                .bind(provider)
                .bind(signer)
                .bind(raw_metadata(&save.name, "", next.as_slice()))
                .execute(&mut *tx)
                .await?;
            } else if let Some(next) = next {
                let previous = previous_contract
                    .and_then(|(address, network)| address.zip(network))
                    .map(|(address, network)| LinkedContract { address, network });
                list_contract(&mut tx, provider, previous.as_ref(), &next).await?;
            }
        }
        let body = row(&mut tx, &view, edit).await?.ok_or_else(|| {
            ApiError::internal(format!("collection {id} vanished after its save"))
        })?;
        tx.commit().await?;
        project(body.out(&self.network, None), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(collection: Value) -> Value {
        json!({ "collection": collection, "data": "0x" })
    }

    fn valid() -> Value {
        json!({
            "id": "c011ec71-0000-4000-8000-000000000002", "name": "Hats",
            "eth_address": "0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e", "salt": null, "contract_address": null,
            "is_published": false, "is_approved": false, "minters": [], "managers": [],
            "urn": null, "forum_link": null, "forum_id": null, "reviewed_at": null,
            "created_at": "2025-12-01T00:00:00.000Z", "updated_at": "2025-12-01T00:00:00.000Z",
            "linked_contract_address": null, "linked_contract_network": null
        })
    }

    #[test]
    fn the_schema_takes_every_field_it_names_and_nothing_else() {
        assert!(parse(&body(valid())).is_ok());
        let mut extra = valid();
        extra["forum_post"] = json!("x");
        let refusals = [
            body(extra),
            json!({ "collection": valid(), "data": "0x", "other": 1 }),
            json!({ "data": "0x" }),
            json!({ "collection": valid(), "data": 1 }),
            json!({ "collection": valid(), "data": "0".repeat(DATA_MAX + 1) }),
        ];
        for refused in refusals {
            assert_eq!(parse(&refused).unwrap_err().status(), 400, "{refused}");
        }
        for (key, bad) in [
            ("id", json!("not-a-uuid")),
            ("id", json!("urn:uuid:c011ec71-0000-4000-8000-000000000002")),
            ("name", json!("x".repeat(43))),
            ("name", json!("  ")),
            ("name", json!(42)),
            ("eth_address", json!("x".repeat(TEXT_MAX + 1))),
            ("salt", json!(1)),
            (
                "urn",
                json!("urn:decentraland:matic:collections-thirdparty:tp"),
            ),
            (
                "urn",
                json!("prefix urn:decentraland:matic:collections-thirdparty:tp:x"),
            ),
            (
                "urn",
                json!("urn:decentraland:polygon:collections-thirdparty:tp:x"),
            ),
            ("minters", json!(vec!["0x"; LIST_MAX + 1])),
            ("forum_link", json!("http://forum.example/t/1")),
            ("forum_id", json!(1.5)),
            ("linked_contract_address", json!("0x123")),
            ("linked_contract_network", json!("MAINNET")),
            ("is_published", json!("false")),
        ] {
            let mut collection = valid();
            collection[key] = bad.clone();
            assert!(parse(&body(collection)).is_err(), "{key}: {bad}");
        }
        for key in ["id", "name", "eth_address", "salt", "contract_address"] {
            let mut collection = valid();
            collection.as_object_mut().unwrap().remove(key);
            assert!(parse(&body(collection)).is_err(), "missing {key}");
        }
        let mut name = valid();
        name["name"] = json!("ä".repeat(42));
        assert!(parse(&body(name)).is_ok(), "the bound counts characters");
    }

    #[test]
    fn a_third_party_urn_names_its_provider_and_suffix() {
        assert_eq!(
            third_party_urn("urn:decentraland:amoy:collections-thirdparty:tp:hats"),
            Some(("urn:decentraland:amoy:collections-thirdparty:tp", "hats"))
        );
        for not in [
            "urn:decentraland:matic:collections-v2:0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e",
            "urn:decentraland:matic:collections-thirdparty:tp:ha ts",
            "urn:decentraland:matic:collections-thirdparty:tp:a|b",
            "urn:decentraland:matic:collections-thirdparty:tp:hats:extra",
        ] {
            assert_eq!(third_party_urn(not), None, "{not}");
        }
    }

    #[test]
    fn a_standard_save_needs_initialize_data_naming_the_signer() {
        let owner = "0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e";
        let mut save = parse(&body(valid())).unwrap();
        for data in [None, Some(String::new())] {
            save.data = data;
            let missing = standard_request(&save, owner).err().unwrap();
            assert_eq!(
                (missing.status(), missing.to_string().as_str()),
                (200, "Could not get data from request")
            );
        }
        save.data = Some("0x00".into());
        let refused = standard_deployment(&crate::network::POLYGON, "", &save, owner)
            .err()
            .unwrap();
        assert_eq!(
            (refused.status(), refused.to_string().as_str()),
            (409, "Cannot upsert a collection without a valid data")
        );
    }
}

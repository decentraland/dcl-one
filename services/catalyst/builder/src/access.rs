//! Who may do what to a builder collection or item: the one place that
//! decides, deny by default. A decision reads the verified signer, the
//! resource ids the request acts on, the builder's rows, the chain copy
//! through a [`DecisionView`] and the current committee
//! ([`crate::committee`]). It grants as soon as one role answers yes; it
//! refuses with builder-server's refusal only when every source it needed
//! answered no; when a source it needed can't tell, it answers 503 and grants
//! nothing.
//!
//! The roles, as builder-server defines them:
//!
//! - **Owner** of a standard collection: the chain copy's `creator` once the
//!   copy holds the collection, the stored `eth_address` before. A
//!   third-party collection has no owner.
//! - **On-chain manager**: a `managers` entry of the copy's row, so only of
//!   a collection on chain.
//! - **Committee member**: [`crate::committee`].
//! - **Third-party manager** of a collection's provider: a manager the
//!   third-party registry lists for the provider once it approved it; before
//!   approval, a manager the registry lists that the provider's virtual third
//!   party also lists, or the registry's managers when there is no virtual
//!   third party; with no registry record, the virtual third party's
//!   managers ([`provider_managers`]). A decision that involves a
//!   third-party collection reads the registry first, with no connection
//!   held, and accepts only a read that started after it began
//!   ([`crate::third_parties`]); the wallet stored as a third-party
//!   collection's creator decides nothing.
//! - **Item owner**: an item in a collection is its collection's: the
//!   collection's owner, on-chain manager or provider's manager decide; the
//!   item row's `eth_address` decides only for an item with no collection.
//!
//! A grant is a value only this module builds; the store functions that read
//! or change a collection or item take it. A write's value is confirmed again
//! inside the write's transaction, after its rows are locked
//! ([`confirm_collection_edit`], [`confirm_item_save`], [`confirm_item_change`]).
//!
//! ```no_run
//! async fn save(
//!     state: &catalyst_builder::SharedState,
//!     signer: &catalyst_shared::signed_fetch::Signer,
//!     save: &catalyst_builder::collections::save::CollectionSave,
//!     id: uuid::Uuid,
//! ) {
//!     let edit = catalyst_builder::access::edit_collection(state, signer, id, None).await.unwrap();
//!     let _ = state.items.save_collection(&edit, save, None).await;
//! }
//! ```
//!
//! A grant can't be built outside this module:
//!
//! ```compile_fail,E0451
//! let forged = catalyst_builder::access::CollectionEdit {
//!     id: uuid::Uuid::nil(),
//!     signer: String::new(),
//! };
//! ```
//!
//! and a store call can't skip the decision:
//!
//! ```compile_fail,E0308
//! async fn skip(
//!     state: &catalyst_builder::SharedState,
//!     save: &catalyst_builder::collections::save::CollectionSave,
//! ) {
//!     let _ = state.items.save_collection("0xabc", save, None).await;
//! }
//! ```

use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::chain_index::third_parties::Provider;
use crate::chain_state::{self, DecisionView};
use crate::curation::ITEM_CURATED;
use crate::http::{ApiError, Subject};
use crate::items::urn::ThirdPartyItemUrn;
use crate::kept::Since;
use crate::shared::signed_fetch::Signer;
use crate::third_parties::{self, Registry, has_manager};
use crate::{SharedState, State};

/// A signer's own wallet: what the per-wallet listings filter by, with the
/// providers it manages (each with whether it is programmatic) when the
/// listing reads them.
#[derive(Debug)]
pub struct Wallet {
    address: String,
    providers: Vec<(String, bool)>,
}

/// The signer is a current committee member.
#[derive(Debug)]
pub struct OnCommittee {
    member: String,
}

/// May read the collection; `full` also reads its unpublished items.
#[derive(Debug)]
pub struct CollectionRead {
    id: Uuid,
    full: bool,
    programmatic: bool,
}

/// May create or save the collection; `at` is when it was decided, which
/// the collection's lock is compared with.
#[derive(Debug)]
pub struct CollectionEdit {
    id: Uuid,
    signer: String,
    at: DateTime<Utc>,
    provider: Option<String>,
    registry: Option<Registry>,
    own_content: bool,
}

/// May lock the collection, at `at`.
#[derive(Debug)]
pub struct CollectionLock {
    id: Uuid,
    signer: String,
    at: DateTime<Utc>,
}

/// May delete the collection or accept its terms of service.
#[derive(Debug)]
pub struct CollectionManage {
    id: Uuid,
    signer: String,
    at: DateTime<Utc>,
    registry: Option<Registry>,
}

/// May publish the collection's standard deployment or sync its owner;
/// `committee` when the current committee admitted the signer.
#[derive(Debug)]
pub struct CollectionSync {
    id: Uuid,
    signer: String,
    committee: bool,
    provider: Option<String>,
    registry: Option<Registry>,
}

/// May submit the third-party collection's items for curation with a slot
/// cheque: a manager of its provider, by the registry the decision read.
#[derive(Debug)]
pub struct ThirdPartyPublish {
    id: Uuid,
    signer: String,
    provider: String,
    registry: Registry,
}

/// May read the item; `full` also reads what only its owner's side sees.
#[derive(Debug)]
pub struct ItemRead {
    id: Uuid,
    full: bool,
}

/// May save the item into `collection` (none: no collection).
#[derive(Debug)]
pub struct ItemSave {
    id: Uuid,
    signer: String,
    collection: Option<Uuid>,
    at: DateTime<Utc>,
    registry: Option<Registry>,
    own_content: bool,
}

/// May delete the item or upload its files.
#[derive(Debug)]
pub struct ItemChange {
    id: Uuid,
    signer: String,
    at: DateTime<Utc>,
    registry: Option<Registry>,
}

/// May run the node's publication of the collection.
#[derive(Debug)]
pub struct Publisher {
    id: Uuid,
    signer: String,
}

/// Anyone may read the public facts of this item: a minted item of a
/// standard collection.
#[derive(Debug)]
pub struct MintedItem {
    id: Uuid,
}

/// May read the item file `hash`; `public` when the published rule alone
/// opened it to an unsigned request.
#[derive(Debug)]
pub struct FileRead {
    hash: String,
    public: bool,
}

impl FileRead {
    pub fn hash(&self) -> &str {
        &self.hash
    }
    pub fn public(&self) -> bool {
        self.public
    }
}

impl MintedItem {
    pub fn id(&self) -> Uuid {
        self.id
    }
}

impl Wallet {
    /// The providers it manages, each with whether it is programmatic.
    pub fn providers(&self) -> &[(String, bool)] {
        &self.providers
    }

    /// The ids of the providers it manages.
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers.iter().map(|(id, _)| id.clone()).collect()
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}

impl OnCommittee {
    pub fn member(&self) -> &str {
        &self.member
    }
}

impl CollectionRead {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn full(&self) -> bool {
        self.full
    }

    /// Whether a third-party collection's provider is programmatic: the
    /// registry's record, else its virtual third party.
    pub fn programmatic(&self) -> bool {
        self.programmatic
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::CollectionRead {}
    impl Sealed for super::CollectionEdit {}
    impl Sealed for super::CollectionSync {}
    impl Sealed for super::CollectionManage {}
    impl Sealed for super::ThirdPartyPublish {}
    impl Sealed for super::Curator {}
}

/// A grant to record a review of one collection: a curation of it or of its
/// items ([`crate::curation`]); only this module's write grants implement it.
pub trait CurationGrant: sealed::Sealed {
    fn curated(&self) -> Uuid;
}

impl CurationGrant for CollectionManage {
    fn curated(&self) -> Uuid {
        self.id
    }
}

impl CurationGrant for ThirdPartyPublish {
    fn curated(&self) -> Uuid {
        self.id
    }
}

impl CurationGrant for Curator {
    fn curated(&self) -> Uuid {
        self.collection
    }
}

/// May read a collection's review timeline: a role on it, or the committee,
/// which also reads the validator's ids.
#[derive(Debug)]
pub struct ReviewRead {
    id: Uuid,
    committee: bool,
}

impl ReviewRead {
    pub fn id(&self) -> Uuid {
        self.id
    }
    /// The committee admitted the signer.
    pub fn committee(&self) -> bool {
        self.committee
    }
}

/// May read the curations of a published collection, or of an item of one,
/// and ask the transition table ([`crate::curation::transitions`]) for a
/// write: as a creator (a role on the collection) or as
/// the committee.
#[derive(Debug)]
pub struct Curator {
    collection: Uuid,
    item: Option<Uuid>,
    signer: String,
    committee: bool,
}

impl Curator {
    pub fn collection(&self) -> Uuid {
        self.collection
    }
    pub fn item(&self) -> Option<Uuid> {
        self.item
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
    /// The current committee admitted the signer.
    pub fn committee(&self) -> bool {
        self.committee
    }
}

/// Who `GET /v1/curations` answers for.
#[derive(Debug)]
pub enum CurationScope {
    /// Every collection's curation.
    Every(OnCommittee),
    /// The curations of the collections the wallet holds a role on.
    Own(Wallet),
}

/// A grant over one collection, for the reads that answer its row; only
/// this module's grants implement it.
pub(crate) trait CollectionGrant: sealed::Sealed {
    fn collection(&self) -> Uuid;
}

impl CollectionGrant for CollectionRead {
    fn collection(&self) -> Uuid {
        self.id
    }
}

impl CollectionGrant for CollectionEdit {
    fn collection(&self) -> Uuid {
        self.id
    }
}

impl CollectionGrant for CollectionSync {
    fn collection(&self) -> Uuid {
        self.id
    }
}

impl CollectionLock {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

impl CollectionManage {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
}

impl CollectionSync {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
    pub fn third_party(&self) -> bool {
        self.provider.is_some()
    }
    /// The full read this grant opens: the body and every item.
    pub fn full_read(&self) -> CollectionRead {
        CollectionRead {
            id: self.id,
            full: true,
            programmatic: false,
        }
    }
}

impl CollectionEdit {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
    pub fn own_content(&self) -> bool {
        self.own_content
    }
}

impl ItemRead {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn full(&self) -> bool {
        self.full
    }
}

impl ItemSave {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
    pub fn collection(&self) -> Option<Uuid> {
        self.collection
    }
}

impl ItemChange {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
}

impl Publisher {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
}

const COLLECTIONS: &str = "collections";
const ITEMS: &str = "items";
const UPSERT: &str = "inserted or updated";
const CURATION_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

/// `withModelExists`' 400 for an id that is not a UUID.
fn invalid_uuid(raw: &str) -> ApiError {
    ApiError::bad_request(format!("Invalid uuid {raw}")).with_data(json!({ "id": raw }))
}

/// `withModelExists`' 404.
fn not_found(id: Uuid, table: &str) -> ApiError {
    ApiError::not_found(format!("Couldn't find \"{id}\" on {table}"))
        .with_data(json!({ "id": id, "tableName": table }))
}

/// `withModelAuthorization`'s 401.
fn unauthorized_user(signer: &str, table: &str, id: Uuid) -> ApiError {
    ApiError::unauthorized(format!("Unauthorized user {signer} for {table} {id}"))
        .with_data(json!({ "ethAddress": signer, "tableName": table }))
}

fn parse(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| invalid_uuid(raw))
}

struct Collection {
    stored_owner: String,
    contract: Option<String>,
    provider: Option<String>,
    third_party: bool,
    provider_managers: Vec<String>,
    programmatic: bool,
    pending: bool,
    lock: Option<NaiveDateTime>,
    published: bool,
    creator: Option<String>,
    managers: Vec<String>,
    minters: Vec<String>,
}

impl Collection {
    fn owner(&self, signer: &str) -> bool {
        !self.third_party && self.creator.as_deref().unwrap_or(&self.stored_owner) == signer
    }

    fn manager(&self, signer: &str) -> bool {
        self.managers.iter().any(|manager| manager == signer)
    }

    fn minter(&self, signer: &str) -> bool {
        self.minters.iter().any(|minter| minter == signer)
    }

    fn provider_manager(&self, signer: &str) -> bool {
        self.third_party && has_manager(&self.provider_managers, signer)
    }

    fn role(&self, signer: &str) -> bool {
        self.owner(signer) || self.manager(signer) || self.provider_manager(signer)
    }

    /// The owner's lock holds at `at`: set less than a day before it.
    fn locked(&self, at: DateTime<Utc>) -> bool {
        self.lock
            .is_some_and(|lock| lock.and_utc() + LOCK_HOLDS > at)
    }
}

/// How long a collection's lock holds.
pub const LOCK_HOLDS: TimeDelta = TimeDelta::days(1);

/// A collection row's owner, provider, contract, publication flag and
/// lock, and its provider's virtual third party's managers.
type StoredCollection = (
    String,
    Option<String>,
    Option<String>,
    bool,
    Option<NaiveDateTime>,
    Option<Vec<String>>,
    Option<bool>,
);

/// Whether `signer` may change or remove the virtual third party `id`
/// listing `listed`: a wallet it lists that also manages the provider
/// ([`provider_managers`] over `registry`), so a change never widens an
/// unapproved record's managers for a wallet the registry leaves out.
pub(crate) fn changes_virtual_third_party(
    registry: &[Provider],
    id: &str,
    listed: &[String],
    signer: &str,
) -> bool {
    has_manager(listed, signer)
        && has_manager(&provider_managers(Some(registry), id, Some(listed)), signer)
}

/// The wallets that manage the provider `id`, as builder-server's
/// `ThirdPartyService.isManager` orders its sources: the registry's managers
/// once it approved the provider; before approval, its managers that the
/// virtual third party also lists, or all of them when there is no virtual
/// third party; with no registry record, the virtual third party's managers.
/// Nobody without the registry.
pub(crate) fn provider_managers(
    registry: Option<&[Provider]>,
    id: &str,
    virtual_managers: Option<&[String]>,
) -> Vec<String> {
    let Some(registry) = registry else {
        return Vec::new();
    };
    match (third_parties::provider(registry, id), virtual_managers) {
        (Some(record), Some(listed)) if !record.is_approved => record
            .managers
            .iter()
            .filter(|m| has_manager(listed, m))
            .cloned()
            .collect(),
        (Some(record), _) => record.managers.clone(),
        (None, Some(listed)) => listed.to_vec(),
        (None, None) => Vec::new(),
    }
}

/// The registry, read when a collection in `collections` or the collection
/// of an item in `items` is third-party, by a read that started at or after
/// `since`; `None` when none is. Holds no connection while it reads.
async fn registry_for(
    state: &SharedState,
    since: Since,
    collections: &[Uuid],
    items: &[Uuid],
) -> Result<Option<Registry>, ApiError> {
    let third_party: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.collections WHERE id = ANY($1) AND third_party_id IS NOT NULL) \
         OR EXISTS (SELECT 1 FROM builder.items i JOIN builder.collections c ON c.id = i.collection_id \
           WHERE i.id = ANY($2) AND c.third_party_id IS NOT NULL)",
    )
    .bind(collections)
    .bind(items)
    .fetch_one(state.items.pool())
    .await?;
    if !third_party {
        return Ok(None);
    }
    Ok(Some(registry(state, since).await?))
}

fn providers(registry: &Option<Registry>) -> Option<&[Provider]> {
    registry.as_ref().map(|r| r.as_slice())
}

async fn registry(state: &SharedState, since: Since) -> Result<Registry, ApiError> {
    state.third_parties.registry(state, since).await
}

async fn collection(
    conn: &mut PgConnection,
    view: &DecisionView,
    id: Uuid,
    registry: Option<&[Provider]>,
) -> Result<Option<Collection>, ApiError> {
    let row: Option<StoredCollection> = sqlx::query_as(
        "SELECT c.eth_address, c.third_party_id, c.contract_address, c.publication_pending, c.lock, v.managers, \
           v.is_programmatic FROM builder.collections c LEFT JOIN builder.virtual_third_parties v ON v.id = c.third_party_id \
         WHERE c.id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((
        stored_owner,
        provider,
        contract,
        pending,
        lock,
        virtual_managers,
        virtual_programmatic,
    )) = row
    else {
        return Ok(None);
    };
    let third_party = provider.is_some();
    let programmatic = provider
        .as_deref()
        .and_then(|id| registry.and_then(|r| third_parties::provider(r, id)))
        .map_or(virtual_programmatic.unwrap_or(false), |p| p.is_programmatic);
    let provider_managers = provider
        .as_deref()
        .map(|id| provider_managers(registry, id, virtual_managers.as_deref()))
        .unwrap_or_default();
    let published = chain_state::published(conn, view, id).await?;
    let facts = match &contract {
        Some(contract) if published && !third_party => {
            chain_state::collection(conn, view, contract).await?
        }
        _ => None,
    };
    Ok(Some(Collection {
        stored_owner,
        contract,
        provider,
        third_party,
        provider_managers,
        programmatic,
        pending,
        lock,
        published,
        creator: facts.as_ref().map(|f| f.creator.clone()),
        minters: facts
            .as_ref()
            .map(|f| f.minters.clone())
            .unwrap_or_default(),
        managers: facts.map(|f| f.managers).unwrap_or_default(),
    }))
}

struct Item {
    owner: String,
    collection: Option<Uuid>,
    minted: bool,
    published: bool,
}

async fn item(conn: &mut PgConnection, id: Uuid) -> Result<Option<Item>, ApiError> {
    let row: Option<(String, Option<Uuid>, bool, bool)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT eth_address, collection_id, blockchain_item_id IS NOT NULL, {ITEM_CURATED} \
         FROM builder.items i WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|(owner, collection, minted, published)| Item {
        owner,
        collection,
        minted,
        published,
    }))
}

async fn parent(
    conn: &mut PgConnection,
    view: &DecisionView,
    id: Uuid,
    registry: Option<&[Provider]>,
) -> Result<Collection, ApiError> {
    collection(conn, view, id, registry)
        .await?
        .ok_or_else(|| ApiError::internal(format!("item of a missing collection {id}")))
}

async fn decision(
    state: &SharedState,
) -> Result<(sqlx::pool::PoolConnection<sqlx::Postgres>, DecisionView), ApiError> {
    let mut conn = state.items.pool().acquire().await?;
    let view = chain_state::for_decision(&mut conn).await?;
    Ok((conn, view))
}

/// Whether the committee shows `signer` on it, false when it can't tell:
/// for a view only the committee widens.
async fn known_committee(state: &State, since: Since, signer: &str) -> bool {
    state.committee.known_member(state, since, signer).await
}

async fn on_committee(state: &State, since: Since, signer: &str) -> Result<bool, ApiError> {
    state.committee.is_member(state, since, signer).await
}

/// The signer's own wallet, for a listing of the rows it owns itself.
pub fn wallet(signer: &Signer) -> Wallet {
    Wallet {
        address: signer.as_str().to_string(),
        providers: Vec::new(),
    }
}

/// The signer's own wallet with the providers it manages: the ones
/// builder-server's `getThirdParties(manager)` lists that
/// [`provider_managers`] admits it to. Reads the registry.
pub async fn managing(state: &SharedState, signer: &Signer) -> Result<Wallet, ApiError> {
    let registry = registry(state, Since::read()).await?;
    let mut conn = state.items.pool().acquire().await?;
    let listed = third_parties::managed_by(&mut conn, &registry, signer.as_str()).await?;
    let providers = listed
        .into_iter()
        .filter(|l| {
            has_manager(
                &provider_managers(Some(&registry), &l.id, l.virtual_managers.as_deref()),
                signer.as_str(),
            )
        })
        .map(|l| (l.id, l.programmatic))
        .collect();
    Ok(Wallet {
        address: signer.as_str().to_string(),
        providers,
    })
}

/// `/v1/{address}/collections` and `/items`: the address must be the
/// signer's; anyone else is refused with the requested address. The wallet
/// carries the providers it manages ([`managing`]).
pub async fn own_address(
    state: &SharedState,
    signer: &Signer,
    address: &str,
) -> Result<Wallet, ApiError> {
    let address = address.to_ascii_lowercase();
    if address != signer.as_str() {
        return Err(
            ApiError::unauthorized("Unauthorized").with_data(json!({ "eth_address": address }))
        );
    }
    managing(state, signer).await
}

/// May read a provider's slots; carries the registry the decision read.
#[derive(Debug)]
pub struct ProviderManage {
    id: String,
    registry: Registry,
}

impl ProviderManage {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn registry(&self) -> &[Provider] {
        &self.registry
    }
}

/// `GET /v1/thirdParties/:id/slots`: a manager of the provider `id`
/// ([`provider_managers`]); anyone else, an unknown provider included, is
/// refused with builder-server's 401.
pub async fn manage_provider(
    state: &SharedState,
    signer: &Signer,
    id: &str,
) -> Result<ProviderManage, ApiError> {
    let registry = registry(state, Since::read()).await?;
    let listed: Option<Vec<String>> =
        sqlx::query_scalar("SELECT managers FROM builder.virtual_third_parties WHERE id = $1")
            .bind(id)
            .fetch_optional(state.items.pool())
            .await?;
    if !has_manager(
        &provider_managers(Some(&registry), id, listed.as_deref()),
        signer.as_str(),
    ) {
        return Err(ApiError::unauthorized(
            "Unauthorized access. Account is not manager of the collection",
        )
        .with_data(json!({ "eth_address": signer.as_str() })));
    }
    Ok(ProviderManage {
        id: id.to_string(),
        registry,
    })
}

/// May read the data the committee's registry transaction needs to approve
/// the collection.
#[derive(Debug)]
pub struct ApprovalRead {
    id: Uuid,
}

impl ApprovalRead {
    pub fn id(&self) -> Uuid {
        self.id
    }
}

/// `GET /v1/collections/:id/approvalData`: the collection must exist (400,
/// 404), then the signer must be a current committee member (401; 503
/// while the committee can't be read).
pub async fn read_approval(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<ApprovalRead, ApiError> {
    let id = parse(raw)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM builder.collections WHERE id = $1)")
            .bind(id)
            .fetch_one(state.items.pool())
            .await?;
    if !exists {
        return Err(not_found(id, COLLECTIONS));
    }
    committee(state, signer).await?;
    Ok(ApprovalRead { id })
}

/// The signer must be a current committee member.
pub async fn committee(state: &SharedState, signer: &Signer) -> Result<OnCommittee, ApiError> {
    if !on_committee(state, Since::read(), signer.as_str()).await? {
        return Err(ApiError::unauthorized("Unauthorized")
            .with_data(json!({ "eth_address": signer.as_str() })));
    }
    Ok(OnCommittee {
        member: signer.as_str().to_string(),
    })
}

/// A collection row's owner, provider and contract, with its provider's
/// virtual third party's managers and programmatic flag.
type StaleRow = (
    String,
    Option<String>,
    Option<String>,
    Option<Vec<String>>,
    Option<bool>,
);

/// While the copy has a complete pass but a stale one: whether `id` is a
/// collection the copy can't change that the signer reads in full (and, if
/// so, whether its provider is programmatic): a third-party one whose
/// provider it manages, or its own standard draft, whose contract isn't on
/// the copy. The owner's own listing answers the same rows from the same
/// pass.
async fn own_draft_while_stale(
    conn: &mut PgConnection,
    signer: &Signer,
    id: Uuid,
    registry: Option<&[Provider]>,
) -> Result<Option<bool>, ApiError> {
    let view = chain_state::for_read(conn).await?;
    let row: Option<StaleRow> =
        sqlx::query_as(
            "SELECT c.eth_address, c.third_party_id, c.contract_address, v.managers, v.is_programmatic \
             FROM builder.collections c LEFT JOIN builder.virtual_third_parties v ON v.id = c.third_party_id \
             WHERE c.id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let Some((owner, provider, contract, listed, virtual_programmatic)) = row else {
        return Ok(None);
    };
    if let Some(provider) = provider {
        let managers = provider_managers(registry, &provider, listed.as_deref());
        let programmatic = registry
            .and_then(|r| third_parties::provider(r, &provider))
            .map_or(virtual_programmatic.unwrap_or(false), |p| p.is_programmatic);
        return Ok(has_manager(&managers, signer.as_str()).then_some(programmatic));
    }
    if owner != signer.as_str() {
        return Ok(None);
    }
    Ok(match contract {
        Some(contract) => (!chain_state::holds(conn, &view, &contract).await?).then_some(false),
        None => Some(false),
    })
}

/// The signer is on the committee, None when it isn't or the committee
/// can't tell: for a view only the committee widens.
pub async fn known_committee_member(state: &SharedState, signer: &Signer) -> Option<OnCommittee> {
    known_committee(state, Since::read(), signer.as_str())
        .await
        .then(|| OnCommittee {
            member: signer.as_str().to_string(),
        })
}

/// `GET /v1/collections/:id`: any signer reads a published collection's
/// public detail, and its minters and the committee (when the committee
/// can tell) its full body; the owner, an on-chain manager, the
/// provider's manager and the committee read any collection in full.
pub async fn read_collection(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<CollectionRead, ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, Since::read(), &[id], &[]).await?;
    let mut conn = state.items.pool().acquire().await?;
    let view = match chain_state::for_decision(&mut conn).await {
        Ok(view) => view,
        Err(stale) => {
            return match own_draft_while_stale(&mut conn, signer, id, providers(&registry)).await? {
                Some(programmatic) => Ok(CollectionRead {
                    id,
                    full: true,
                    programmatic,
                }),
                None => Err(stale),
            };
        }
    };
    let c = collection(&mut conn, &view, id, providers(&registry))
        .await?
        .ok_or_else(|| not_found(id, COLLECTIONS))?;
    drop(conn);
    let s = signer.as_str();
    if c.role(s) {
        return Ok(CollectionRead {
            id,
            full: true,
            programmatic: c.programmatic,
        });
    }
    if c.published {
        let full = c.minter(s) || known_committee(state, Since::read(), s).await;
        return Ok(CollectionRead {
            id,
            full,
            programmatic: c.programmatic,
        });
    }
    if on_committee(state, Since::read(), s).await? {
        return Ok(CollectionRead {
            id,
            full: true,
            programmatic: c.programmatic,
        });
    }
    Err(ApiError::unauthorized("Unauthorized").with_data(json!({ "id": raw, "eth_address": s })))
}

/// `GET /v1/collections/:idOrContractAddress/items`: any signer lists a
/// published collection's published items; a role holder and the committee
/// list every item. For a published collection the committee only widens the
/// view ([`crate::committee::Committee::known_member`]): a member gets what any signer
/// gets while the committee can't tell.
pub async fn collection_items(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
    status: Option<&str>,
) -> Result<CollectionRead, ApiError> {
    let registry = match Uuid::parse_str(raw) {
        Ok(id) => registry_for(state, Since::read(), &[id], &[]).await?,
        Err(_) => None,
    };
    let (mut conn, view) = decision(state).await?;
    let id = match Uuid::parse_str(raw) {
        Ok(id) => id,
        Err(_) if dcl_auth::is_eth_address(raw) => {
            let contract = raw.to_ascii_lowercase();
            sqlx::query_scalar("SELECT id FROM builder.collections WHERE contract_address = $1")
                .bind(&contract)
                .fetch_optional(&mut *conn)
                .await?
                .ok_or_else(|| {
                    ApiError::not_found(format!(
                        "Collection not found for the provided contract address: {contract}"
                    ))
                    .with_data(json!({ "contractAddress": contract }))
                })?
        }
        Err(_) => {
            return Err(ApiError::bad_request(format!(
                "Invalid collection ID or contract address format: {raw}"
            ))
            .with_data(json!({ "idOrContractAddress": raw })));
        }
    };
    let c = collection(&mut conn, &view, id, providers(&registry))
        .await?
        .ok_or_else(|| not_found(id, COLLECTIONS))?;
    drop(conn);
    if let Some(status) = status.filter(|s| !CURATION_STATUSES.contains(s)) {
        return Err(ApiError::bad_request("Invalid Status provided")
            .with_data(json!({ "id": raw, "status": status })));
    }
    let s = signer.as_str();
    if c.role(s) {
        return Ok(CollectionRead {
            id,
            full: true,
            programmatic: c.programmatic,
        });
    }
    if c.published {
        let full = c.minter(s) || known_committee(state, Since::read(), s).await;
        return Ok(CollectionRead {
            id,
            full,
            programmatic: c.programmatic,
        });
    }
    if on_committee(state, Since::read(), s).await? {
        return Ok(CollectionRead {
            id,
            full: true,
            programmatic: c.programmatic,
        });
    }
    Err(ApiError::unauthorized("Unauthorized").with_data(json!({ "eth_address": s })))
}

/// `GET /v1/items/:id`: any signer reads a minted item of a published
/// standard collection, and a curated item of a published third-party
/// collection (one with an item curation), as the collection's listing
/// shows them ([`collection_items`]), or one the node's content serves
/// ([`served`]); the collection's owner, the provider's
/// manager (the item's owner for an item with no collection) and the
/// committee read any. `full` is false for a caller who reads only what
/// any signer may; for a published item the committee only widens the view,
/// without waiting, as in [`collection_items`].
pub async fn read_item(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<ItemRead, ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, Since::read(), &[], &[id]).await?;
    let (mut conn, view) = decision(state).await?;
    let i = item(&mut conn, id)
        .await?
        .ok_or_else(|| not_found(id, ITEMS))?;
    let parent = match i.collection {
        Some(parent_id) => Some(parent(&mut conn, &view, parent_id, providers(&registry)).await?),
        None => None,
    };
    let s = signer.as_str();
    let owns = match &parent {
        Some(c) => c.role(s),
        None => i.owner == s,
    };
    if owns {
        return Ok(ItemRead { id, full: true });
    }
    let public = match &parent {
        Some(c) if c.published && (i.minted || (c.third_party && i.published)) => Ok(true),
        Some(c) if c.published && c.third_party => {
            served(&mut conn, id, state.content_sync.is_own_content_authority()).await
        }
        _ => Ok(false),
    };
    drop(conn);
    if let (Ok(true), Some(c)) = (&public, &parent) {
        let full = c.owner(s)
            || c.manager(s)
            || c.minter(s)
            || known_committee(state, Since::read(), s).await;
        return Ok(ItemRead { id, full });
    }
    if on_committee(state, Since::read(), s).await? {
        return Ok(ItemRead { id, full: true });
    }
    public?;
    Err(ApiError::unauthorized("Unauthorized").with_data(json!({ "id": raw, "eth_address": s })))
}

/// Whether this node's content serves item `id` at its URN, told as the
/// save's URN check tells a taken URN ([`urn_taken`]): when the content
/// lacks it, 503 unless the node can tell a URN it lacks from one it hasn't
/// synced ([`content_knows_items`]).
async fn served(conn: &mut PgConnection, id: Uuid, own_content: bool) -> Result<bool, ApiError> {
    let urn: Option<String> =
        sqlx::query_scalar("SELECT urn_suffix FROM builder.items WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();
    let Some(urn) = urn else {
        return Ok(false);
    };
    if urn_deployed(conn, &urn).await? {
        return Ok(true);
    }
    content_knows_items(conn, own_content).await?;
    urn_deployed(conn, &urn).await
}

/// `PUT /v1/collections/:id`: anyone creates a standard collection under a
/// new id, and a third-party one under `provider` when neither the registry
/// nor a virtual third party has the provider, or when it manages the
/// provider; only the owner, or the provider's manager, saves an existing
/// one.
pub async fn edit_collection(
    state: &SharedState,
    signer: &Signer,
    id: Uuid,
    provider: Option<&str>,
) -> Result<CollectionEdit, ApiError> {
    let since = Since::write();
    let registry = match provider {
        Some(_) => Some(registry(state, since).await?),
        None => registry_for(state, since, &[id], &[]).await?,
    };
    let edit = CollectionEdit {
        id,
        signer: signer.as_str().to_string(),
        at: (state.now)(),
        provider: provider.map(str::to_string),
        registry,
        own_content: state.content_sync.is_own_content_authority(),
    };
    let (mut conn, view) = decision(state).await?;
    confirm_collection_edit(&mut conn, &view, &edit).await?;
    Ok(edit)
}

/// The state of a collection a save was confirmed for: whether it is
/// published, whether the node's own publication of it is in progress,
/// whether its owner's lock holds, and, for a third-party collection a save
/// creates, whether its provider is new to the node, so the save creates
/// the provider's virtual third party with the signer as its manager.
#[derive(Debug, Default)]
pub struct EditState {
    pub published: bool,
    pub pending: bool,
    pub locked: bool,
    pub new_provider: bool,
}

/// [`edit_collection`]'s decision again, inside the write's transaction,
/// with the state the save's own rules read.
pub(crate) async fn confirm_collection_edit(
    conn: &mut PgConnection,
    view: &DecisionView,
    edit: &CollectionEdit,
) -> Result<EditState, ApiError> {
    let s = edit.signer.as_str();
    let refused = || {
        ApiError::unauthorized("Unauthorized to upsert collection")
            .with_data(json!({ "id": edit.id.to_string(), "eth_address": s }))
    };
    let Some(c) = collection(conn, view, edit.id, providers(&edit.registry)).await? else {
        let Some(provider) = edit.provider.as_deref() else {
            return Ok(EditState::default());
        };
        let registry = providers(&edit.registry).ok_or_else(refused)?;
        let listed: Option<Vec<String>> =
            sqlx::query_scalar("SELECT managers FROM builder.virtual_third_parties WHERE id = $1")
                .bind(provider)
                .fetch_optional(&mut *conn)
                .await?;
        if third_parties::provider(registry, provider).is_none() && listed.is_none() {
            return Ok(EditState {
                new_provider: true,
                ..EditState::default()
            });
        }
        if !has_manager(
            &provider_managers(Some(registry), provider, listed.as_deref()),
            s,
        ) {
            return Err(refused());
        }
        return Ok(EditState::default());
    };
    if !c.owner(s) && !c.provider_manager(s) {
        return Err(refused());
    }
    Ok(EditState {
        published: c.published,
        pending: c.pending,
        locked: c.locked(edit.at),
        new_provider: false,
    })
}

/// `PUT /v1/items/:idOrURN`, decided with [`confirm_item_save`] outside a
/// transaction; the save decides again inside its own.
pub async fn save_item(
    state: &SharedState,
    signer: &Signer,
    id: Uuid,
    intent: &SaveIntent,
) -> Result<ItemSave, ApiError> {
    let registry = registry_for(
        state,
        Since::write(),
        &intent.collection.into_iter().collect::<Vec<_>>(),
        &[id],
    )
    .await?;
    let save = ItemSave {
        id,
        signer: signer.as_str().to_string(),
        collection: intent.collection,
        at: (state.now)(),
        registry,
        own_content: state.content_sync.is_own_content_authority(),
    };
    let (mut conn, view) = decision(state).await?;
    confirm_item_save(&mut conn, &view, &save, intent).await?;
    Ok(save)
}

/// The third-party URN a save's body names.
#[derive(Debug, Clone)]
pub enum UrnField {
    Absent,
    Null,
    ThirdParty(ThirdPartyItemUrn),
    Other,
}

/// What a save's body asks for, as the decision reads it.
#[derive(Debug, Clone)]
pub struct SaveIntent {
    /// The collection the body puts the item in (`None`: no collection).
    pub collection: Option<Uuid>,
    pub rarity: Option<String>,
    pub urn: UrnField,
    /// The body names the item's id (a save by URN may not).
    pub id_given: bool,
    /// The id the path names, which builder-server's refusals repeat.
    pub path_id: Option<String>,
}

/// How a confirmed save writes the item.
#[derive(Debug, Clone)]
pub enum SavePlan {
    /// A standard item: the owner and on-chain id it keeps, the contract
    /// address its content hash is computed for (a minted item of a
    /// collection with a contract) and whether its stored video is kept.
    Standard {
        owner: String,
        blockchain_item_id: Option<String>,
        hash_contract: Option<String>,
        keep_video: bool,
    },
    /// A third-party item, by its URN.
    ThirdParty { urn: String },
}

#[derive(sqlx::FromRow)]
struct StoredItem {
    owner: String,
    collection: Option<Uuid>,
    blockchain_item_id: Option<String>,
    urn: Option<String>,
    rarity: Option<String>,
    submitted: bool,
    smart: bool,
}

async fn stored_item(conn: &mut PgConnection, id: Uuid) -> Result<Option<StoredItem>, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT eth_address AS owner, collection_id AS collection, blockchain_item_id, \
         urn_suffix AS urn, rarity, {ITEM_CURATED} AS submitted, \
         type = 'wearable' AND EXISTS (SELECT 1 FROM builder.item_contents f \
           WHERE f.item_id = i.id AND f.file LIKE '%.js') AS smart \
         FROM builder.items i WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

async fn third_party_urn(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<(String, String)>, ApiError> {
    Ok(sqlx::query_as(
        "SELECT third_party_id, urn_suffix FROM builder.collections WHERE id = $1 \
         AND third_party_id IS NOT NULL AND urn_suffix IS NOT NULL",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

async fn urn_in_use(
    conn: &mut PgConnection,
    id: Uuid,
    third_party_id: &str,
    item: &str,
) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.items i JOIN builder.collections c ON c.id = i.collection_id \
         WHERE i.id <> $1 AND c.third_party_id = $2 AND lower(i.urn_suffix) = lower(c.urn_suffix || ':' || $3))",
    )
    .bind(id)
    .bind(third_party_id)
    .bind(item)
    .fetch_one(conn)
    .await?)
}

/// Whether this node's content holds an active entity at `urn`.
async fn urn_deployed(conn: &mut PgConnection, urn: &str) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM content.active_pointers ap \
           JOIN content.deployments d ON d.entity_id = ap.entity_id \
           WHERE ap.pointer = lower($1) AND d.deleter_deployment IS NULL)",
    )
    .bind(urn)
    .fetch_one(conn)
    .await?)
}

/// Whether the third-party URN `urn` a save claims (item part `item` of
/// `third_party_id`) is taken: another item of the provider holds it, or
/// this node's content holds an entity at it. A URN the content lacks is
/// free only where the node can tell: it is its own content authority, or
/// its wearable and emote history is covered (read before the content, so
/// the coverage holds for what is read after it); else 503.
async fn urn_taken(
    conn: &mut PgConnection,
    save: &ItemSave,
    third_party_id: &str,
    item: &str,
    urn: &str,
) -> Result<bool, ApiError> {
    if urn_in_use(conn, save.id, third_party_id, item).await? || urn_deployed(conn, urn).await? {
        return Ok(true);
    }
    content_knows_items(conn, save.own_content).await?;
    urn_deployed(conn, urn).await
}

/// 503 unless this node can tell an item URN its content lacks from one it
/// hasn't synced yet: it is its own content authority (`own_content`), or
/// its wearable and emote history is covered. Read before the content, so
/// the coverage holds for what is read after it.
pub(crate) async fn content_knows_items(
    conn: &mut PgConnection,
    own_content: bool,
) -> Result<(), ApiError> {
    if own_content {
        return Ok(());
    }
    for entity_type in crate::content::sync::item_coverage::ITEM_TYPES {
        if !matches!(
            crate::content::sync::item_coverage::item_history_coverage(&mut *conn, entity_type)
                .await?,
            crate::content::sync::item_coverage::Coverage::Covered { .. }
        ) {
            return Err(ApiError::service_unavailable(
                "The catalyst's items are not available. Try again later.",
            ));
        }
    }
    Ok(())
}

fn with_id(id: Option<&str>, mut data: serde_json::Value) -> serde_json::Value {
    if let (Some(id), Some(fields)) = (id, data.as_object_mut()) {
        fields.insert("id".into(), id.into());
    }
    data
}

fn missing_collection(id: Uuid) -> ApiError {
    ApiError::not_found("The collection doesn't exist.").with_data(json!({ "collectionId": id }))
}

/// [`save_item`]'s decision, builder-server's `upsertItem` rules in its
/// order: a missing collection (404); a move into, out of or between
/// third-party collections (401); then the third-party rules (the
/// provider's manager, 423 while the node publishes the collection, a URN
/// another item holds or the URN of a submitted item, 409) or the standard
/// ones (the owner of an item with no collection; each collection's owner,
/// or its on-chain manager once published; on a published collection no
/// new item, no move in or out and no rarity change, 409; 423 while the node
/// publishes it). An existing item of a published collection otherwise takes
/// its owner's and its managers' saves.
pub(crate) async fn confirm_item_save(
    conn: &mut PgConnection,
    view: &DecisionView,
    save: &ItemSave,
    intent: &SaveIntent,
) -> Result<SavePlan, ApiError> {
    let (id, s) = (save.id, save.signer.as_str());
    let item_id = intent.id_given.then(|| id.to_string());
    let stored = stored_item(conn, id).await?;
    let current = stored.as_ref().and_then(|i| i.collection);
    let moving = current.is_some() && save.collection.is_some() && current != save.collection;
    let joining = stored.is_some() && current.is_none() && save.collection.is_some();
    let home_id = current.or(save.collection);
    let target_id = (moving || joining).then_some(save.collection).flatten();
    let home = match home_id {
        Some(c) => Some(
            collection(conn, view, c, providers(&save.registry))
                .await?
                .ok_or_else(|| missing_collection(c))?,
        ),
        None => None,
    };
    let target = match target_id {
        Some(c) => Some(
            collection(conn, view, c, providers(&save.registry))
                .await?
                .ok_or_else(|| missing_collection(c))?,
        ),
        None => None,
    };
    let any_third_party = home.as_ref().is_some_and(|c| c.third_party)
        || target.as_ref().is_some_and(|c| c.third_party);
    if (moving && any_third_party) || (joining && target.as_ref().is_some_and(|c| c.third_party)) {
        return Err(
            ApiError::unauthorized("Item can't change between collections.")
                .with_data(with_id(item_id.as_deref(), json!({}))),
        );
    }
    if let (Some(c), Some(home_id)) = (&home, home_id)
        && c.third_party
    {
        if stored.is_some() && save.collection != current {
            return Err(
                ApiError::unauthorized("Item can't change between collections.")
                    .with_data(with_id(item_id.as_deref(), json!({}))),
            );
        }
        if matches!(intent.urn, UrnField::Null) {
            return Err(ApiError::bad_request("The item URN is invalid."));
        }
        if !c.provider_manager(s) {
            return Err(ApiError::unauthorized(
                "The user is unauthorized to upsert the collection.",
            )
            .with_data(with_id(item_id.as_deref(), json!({ "eth_address": s }))));
        }
        if c.pending {
            return Err(ApiError::locked(Subject::Item, "", UPSERT)
                .with_data(with_id(intent.path_id.as_deref(), json!({}))));
        }
        let UrnField::ThirdParty(urn) = &intent.urn else {
            return Err(ApiError::http(
                200,
                "The given item URN is not TP compliant",
            ));
        };
        let (third_party_id, collection_urn) =
            third_party_urn(conn, home_id).await?.ok_or_else(|| {
                ApiError::internal(format!("third-party collection {home_id} without a URN"))
            })?;
        let built = format!("{collection_urn}:{}", urn.item);
        let in_use = |action: &str| {
            ApiError::conflict(format!(
                "The URN provided already belong to another item. The item can't be {action}."
            ))
            .with_data(with_id(intent.path_id.as_deref(), json!({ "urn": built })))
        };
        match &stored {
            Some(item) if item.urn.as_deref() != Some(built.as_str()) => {
                if item.submitted {
                    return Err(ApiError::conflict(format!(
                        "The third party item is already published. It can't be {UPSERT}."
                    ))
                    .with_data(with_id(
                        intent.path_id.as_deref(),
                        json!({ "urn": item.urn }),
                    )));
                }
                if intent.id_given
                    && urn_taken(conn, save, &third_party_id, &urn.item, &built).await?
                {
                    return Err(in_use(UPSERT));
                }
            }
            Some(_) => {}
            None => {
                if urn_taken(conn, save, &third_party_id, &urn.item, &built).await? {
                    return Err(in_use("inserted"));
                }
            }
        }
        return Ok(SavePlan::ThirdParty { urn: built });
    }
    if current.is_none()
        && let Some(item) = &stored
        && item.owner != s
    {
        return Err(
            ApiError::unauthorized("The user is unauthorized to upsert the collection.")
                .with_data(with_id(item_id.as_deref(), json!({ "eth_address": s }))),
        );
    }
    let not_yours = || {
        ApiError::unauthorized("You're not authorized to to change this collection.").with_data(
            with_id(
                item_id.as_deref(),
                json!({ "eth_address": s, "collection_id": save.collection }),
            ),
        )
    };
    let published = |action: &str| {
        ApiError::published(Subject::Item, "", action)
            .with_data(with_id(item_id.as_deref(), json!({})))
    };
    let mut keep_video = false;
    if let Some(c) = &home {
        if !c.owner(s) && !(c.published && c.manager(s)) {
            return Err(not_yours());
        }
        if c.published {
            if moving || current.is_none() {
                return Err(published(UPSERT));
            }
            if save.collection.is_none() {
                return Err(published("deleted"));
            }
            let stored = stored.as_ref().expect("an item in a collection is stored");
            if let (Some(next), Some(before)) = (intent.rarity.as_deref(), stored.rarity.as_deref())
                && next != before
            {
                return Err(published("updated with a new rarity"));
            }
            keep_video = stored.smart;
        } else if c.pending || c.locked(save.at) {
            return Err(ApiError::locked(Subject::Item, "", UPSERT)
                .with_data(with_id(intent.path_id.as_deref(), json!({}))));
        }
    }
    if let Some(c) = &target {
        if !c.owner(s) && !(c.published && c.manager(s)) {
            return Err(not_yours());
        }
        if c.published {
            if moving {
                return Err(published("inserted"));
            }
        } else if c.pending || c.locked(save.at) {
            return Err(ApiError::locked(Subject::Item, "", UPSERT)
                .with_data(with_id(intent.path_id.as_deref(), json!({}))));
        }
    }
    let blockchain_item_id = stored.as_ref().and_then(|i| i.blockchain_item_id.clone());
    let hash_contract = blockchain_item_id
        .as_ref()
        .and(target.as_ref().or(home.as_ref()))
        .and_then(|c| c.contract.clone());
    Ok(SavePlan::Standard {
        owner: stored.map_or_else(|| s.to_string(), |i| i.owner),
        blockchain_item_id,
        hash_contract,
        keep_video,
    })
}

/// `DELETE /v1/items/:id` and `POST /v1/items/:id/files`: the owner of an
/// item with no collection; for an item in a collection, that collection's
/// owner, on-chain manager or provider's manager.
pub async fn change_item(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<ItemChange, ApiError> {
    let id = parse(raw)?;
    let change = ItemChange {
        id,
        signer: signer.as_str().to_string(),
        at: (state.now)(),
        registry: registry_for(state, Since::write(), &[], &[id]).await?,
    };
    let (mut conn, view) = decision(state).await?;
    confirm_item_change(&mut conn, &view, &change).await?;
    Ok(change)
}

/// The state of the item a change was confirmed for, which a delete's own
/// rules read.
#[derive(Debug, Default)]
pub struct ChangeState {
    pub third_party: bool,
    pub published: bool,
    pub pending: bool,
    pub locked: bool,
    pub submitted: bool,
    pub contract: Option<String>,
    pub urn: Option<String>,
}

/// [`change_item`]'s decision again, inside the write's transaction.
pub(crate) async fn confirm_item_change(
    conn: &mut PgConnection,
    view: &DecisionView,
    change: &ItemChange,
) -> Result<ChangeState, ApiError> {
    let (id, s) = (change.id, change.signer.as_str());
    let i = item(conn, id).await?.ok_or_else(|| not_found(id, ITEMS))?;
    let Some(parent_id) = i.collection else {
        if i.owner != s {
            return Err(unauthorized_user(s, ITEMS, id));
        }
        return Ok(ChangeState::default());
    };
    let c = parent(conn, view, parent_id, providers(&change.registry)).await?;
    if !c.role(s) {
        return Err(unauthorized_user(s, ITEMS, id));
    }
    let urn: Option<String> =
        sqlx::query_scalar("SELECT urn_suffix FROM builder.items WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
    Ok(ChangeState {
        third_party: c.third_party,
        published: c.published,
        pending: c.pending,
        locked: c.locked(change.at),
        submitted: i.published,
        contract: c.contract.clone(),
        urn,
    })
}

/// `POST /v1/collections/:id/lock`: only the owner of a standard collection.
pub async fn lock_collection(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<CollectionLock, ApiError> {
    let lock = CollectionLock {
        id: parse(raw)?,
        signer: signer.as_str().to_string(),
        at: (state.now)(),
    };
    let (mut conn, view) = decision(state).await?;
    confirm_collection_lock(&mut conn, &view, &lock).await?;
    Ok(lock)
}

/// [`lock_collection`]'s decision again, inside the write's transaction.
pub(crate) async fn confirm_collection_lock(
    conn: &mut PgConnection,
    view: &DecisionView,
    lock: &CollectionLock,
) -> Result<(), ApiError> {
    let c = collection(conn, view, lock.id, None)
        .await?
        .ok_or_else(|| not_found(lock.id, COLLECTIONS))?;
    if !c.owner(&lock.signer) {
        return Err(unauthorized_user(&lock.signer, COLLECTIONS, lock.id));
    }
    Ok(())
}

/// `DELETE /v1/collections/:id` and `POST /v1/collections/:id/tos`: the
/// collection's owner, its on-chain manager or its provider's manager.
pub async fn manage_collection(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<CollectionManage, ApiError> {
    let id = parse(raw)?;
    let manage = CollectionManage {
        id,
        signer: signer.as_str().to_string(),
        at: (state.now)(),
        registry: registry_for(state, Since::write(), &[id], &[]).await?,
    };
    let (mut conn, view) = decision(state).await?;
    confirm_collection_manage(&mut conn, &view, &manage).await?;
    Ok(manage)
}

/// The state of the collection a manage grant was confirmed for, which a
/// delete's own rules read.
#[derive(Debug, Default)]
pub struct ManageState {
    pub third_party: bool,
    pub published: bool,
    pub pending: bool,
    pub locked: bool,
    pub contract: Option<String>,
    /// A standard collection's owner; none for a third-party one.
    pub owner: Option<String>,
}

/// [`manage_collection`]'s decision again, inside the write's transaction.
pub(crate) async fn confirm_collection_manage(
    conn: &mut PgConnection,
    view: &DecisionView,
    manage: &CollectionManage,
) -> Result<ManageState, ApiError> {
    let c = collection(conn, view, manage.id, providers(&manage.registry))
        .await?
        .ok_or_else(|| not_found(manage.id, COLLECTIONS))?;
    if !c.role(&manage.signer) {
        return Err(unauthorized_user(&manage.signer, COLLECTIONS, manage.id));
    }
    let owner = (!c.third_party).then(|| c.creator.clone().unwrap_or(c.stored_owner.clone()));
    Ok(ManageState {
        third_party: c.third_party,
        published: c.published,
        pending: c.pending,
        locked: c.locked(manage.at),
        contract: c.contract,
        owner,
    })
}

/// May reply in the forum topic of a collection: a current committee
/// member.
#[derive(Debug)]
pub struct ForumReply {
    collection: Uuid,
    member: String,
}

impl ForumReply {
    pub fn collection(&self) -> Uuid {
        self.collection
    }
    pub fn member(&self) -> &str {
        &self.member
    }
}

/// `POST /v1/collections/:id/curation/post`: a current committee member
/// (401, or 503 while the committee can't be read), then the collection
/// must exist (404).
pub async fn reply_on_forum(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<ForumReply, ApiError> {
    let s = signer.as_str();
    if !on_committee(state, Since::write(), s).await? {
        return Err(
            ApiError::unauthorized("Unauthorized").with_data(json!({ "id": raw, "ethAddress": s }))
        );
    }
    let id = parse(raw)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM builder.collections WHERE id = $1)")
            .bind(id)
            .fetch_one(state.items.pool())
            .await?;
    if !exists {
        return Err(ApiError::not_found("Collection not found").with_data(json!({ "id": id })));
    }
    Ok(ForumReply {
        collection: id,
        member: s.to_string(),
    })
}

/// `POST /v1/collections/:id/publish` and `/owner/sync`: the collection's
/// owner, its on-chain manager or a current committee member; a
/// third-party collection's provider manager alone.
pub async fn sync_collection(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<CollectionSync, ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, Since::write(), &[id], &[]).await?;
    let (mut conn, view) = decision(state).await?;
    let c = collection(&mut conn, &view, id, providers(&registry))
        .await?
        .ok_or_else(|| not_found(id, COLLECTIONS))?;
    drop(conn);
    let s = signer.as_str();
    let committee = !c.role(s);
    if committee && (c.third_party || !on_committee(state, Since::write(), s).await?) {
        return Err(unauthorized_user(s, COLLECTIONS, id));
    }
    Ok(CollectionSync {
        id,
        signer: s.to_string(),
        committee,
        provider: c.provider,
        registry,
    })
}

/// The third-party side of [`sync_collection`]'s grant: the publish of a
/// third-party collection; a standard collection's grant comes back.
pub fn publish_third_party(sync: CollectionSync) -> Result<ThirdPartyPublish, CollectionSync> {
    match (sync.provider, sync.registry) {
        (Some(provider), Some(registry)) if !sync.committee => Ok(ThirdPartyPublish {
            id: sync.id,
            signer: sync.signer,
            provider,
            registry,
        }),
        (provider, registry) => Err(CollectionSync {
            provider,
            registry,
            ..sync
        }),
    }
}

impl ThirdPartyPublish {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn signer(&self) -> &str {
        &self.signer
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn registry(&self) -> &[Provider] {
        &self.registry
    }

    /// The full read of the collection it publishes.
    pub fn full_read(&self) -> CollectionRead {
        CollectionRead {
            id: self.id,
            full: true,
            programmatic: third_parties::provider(&self.registry, &self.provider)
                .is_some_and(|p| p.is_programmatic),
        }
    }
}

/// [`publish_third_party`]'s decision again, inside the publish's
/// transaction: the collection still of the provider, the signer still its
/// manager.
pub(crate) async fn confirm_third_party_publish(
    conn: &mut PgConnection,
    view: &DecisionView,
    grant: &ThirdPartyPublish,
) -> Result<(), ApiError> {
    let c = collection(conn, view, grant.id, Some(&grant.registry)).await?;
    if c.is_none_or(|c| {
        c.provider.as_deref() != Some(grant.provider.as_str()) || !c.provider_manager(&grant.signer)
    }) {
        return Err(ApiError::unauthorized("Unauthorized to upsert collection")
            .with_data(json!({ "id": grant.id, "eth_address": grant.signer })));
    }
    Ok(())
}

/// [`sync_collection`]'s decision again, inside the write's transaction: a
/// role the rows decide, else a committee read that started after the
/// decision.
pub(crate) async fn confirm_collection_sync(
    conn: &mut PgConnection,
    view: &DecisionView,
    state: &State,
    sync: &CollectionSync,
) -> Result<(), ApiError> {
    let c = collection(conn, view, sync.id, providers(&sync.registry))
        .await?
        .ok_or_else(|| not_found(sync.id, COLLECTIONS))?;
    if !c.role(&sync.signer)
        && !(sync.committee && on_committee(state, Since::write(), &sync.signer).await?)
    {
        return Err(unauthorized_user(&sync.signer, COLLECTIONS, sync.id));
    }
    Ok(())
}

/// `builder-server`'s curation 404: no collection, or an item without one.
fn curation_not_found(signer: &str) -> ApiError {
    ApiError::not_found("Not found").with_data(json!({ "ethAddress": signer }))
}

/// `GET /v1/curations`: the committee reads every collection's curation
/// (503 while the committee can't be read); anyone else the curations of the
/// collections it holds a role on.
pub async fn curation_scope(
    state: &SharedState,
    signer: &Signer,
) -> Result<CurationScope, ApiError> {
    if on_committee(state, Since::read(), signer.as_str()).await? {
        return Ok(CurationScope::Every(OnCommittee {
            member: signer.as_str().to_string(),
        }));
    }
    Ok(CurationScope::Own(managing(state, signer).await?))
}

/// The curation routes of the collection `raw`: the collection must exist
/// (404) and be published (409), then the signer must hold a role on it or
/// be on the committee (401).
/// A curation read decides on [`Since::read`], a write on [`Since::write`].
pub async fn curate_collection(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
    since: Since,
) -> Result<Curator, ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, since, &[id], &[]).await?;
    let (mut conn, view) = decision(state).await?;
    let c = collection(&mut conn, &view, id, providers(&registry))
        .await?
        .ok_or_else(|| curation_not_found(signer.as_str()))?;
    drop(conn);
    let role = c.role(signer.as_str());
    admit(state, signer, &c, id, None, role, since).await
}

/// The curation routes of the item `raw`: its collection must exist (404)
/// and be published (409), then the signer must hold a role on its
/// collection or be on the committee (401).
/// A curation read decides on [`Since::read`], a write on [`Since::write`].
pub async fn curate_item(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
    since: Since,
) -> Result<Curator, ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, since, &[], &[id]).await?;
    let (mut conn, view) = decision(state).await?;
    let not_found = || curation_not_found(signer.as_str());
    let i = item(&mut conn, id).await?.ok_or_else(not_found)?;
    let parent_id = i.collection.ok_or_else(not_found)?;
    let c = parent(&mut conn, &view, parent_id, providers(&registry)).await?;
    drop(conn);
    let role = c.role(signer.as_str());
    admit(state, signer, &c, parent_id, Some(id), role, since).await
}

/// `GET /v1/collections/:id/events`: the signer holds a role on the
/// collection or is on the committee (503 while the committee can't be read
/// for a signer without a role, whether or not the collection exists).
/// While the registry can't be read nobody holds a role on a third-party
/// collection. Anyone else gets the answer an absent collection gets.
pub async fn read_review(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<ReviewRead, ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, Since::read(), &[id], &[])
        .await
        .ok()
        .flatten();
    let (mut conn, view) = decision(state).await?;
    let c = collection(&mut conn, &view, id, providers(&registry)).await?;
    drop(conn);
    let s = signer.as_str();
    if let Some(c) = &c
        && c.role(s)
    {
        return Ok(ReviewRead {
            id,
            committee: known_committee(state, Since::read(), s).await,
        });
    }
    if on_committee(state, Since::read(), s).await? && c.is_some() {
        return Ok(ReviewRead {
            id,
            committee: true,
        });
    }
    Err(not_found(id, COLLECTIONS))
}

/// `POST /v1/collections/:id/comments`: the collection must exist (404),
/// then the signer must hold a role on it or be on the committee (401). A
/// role holder writes as the committee only when the committee shows it on it.
pub async fn review_member(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<Curator, ApiError> {
    let (id, c) = reviewed(state, raw).await?;
    let s = signer.as_str();
    let role = c.role(s);
    let committee = if role {
        known_committee(state, Since::write(), s).await
    } else {
        on_committee(state, Since::write(), s).await?
    };
    if !role && !committee {
        return Err(
            ApiError::unauthorized("Unauthorized").with_data(json!({ "id": id, "ethAddress": s }))
        );
    }
    Ok(Curator {
        collection: id,
        item: None,
        signer: s.to_string(),
        committee,
    })
}

/// `POST /v1/collections/:id/validations` and `.../curation/appeal`: the
/// collection must exist (404), then the signer must hold a role on it
/// (401); the committee gets nothing here.
pub async fn review_creator(
    state: &SharedState,
    signer: &Signer,
    raw: &str,
) -> Result<Curator, ApiError> {
    let (id, c) = reviewed(state, raw).await?;
    let s = signer.as_str();
    if !c.role(s) {
        return Err(unauthorized_user(s, COLLECTIONS, id));
    }
    Ok(Curator {
        collection: id,
        item: None,
        signer: s.to_string(),
        committee: false,
    })
}

async fn reviewed(state: &SharedState, raw: &str) -> Result<(Uuid, Collection), ApiError> {
    let id = parse(raw)?;
    let registry = registry_for(state, Since::write(), &[id], &[]).await?;
    let (mut conn, view) = decision(state).await?;
    let c = collection(&mut conn, &view, id, providers(&registry))
        .await?
        .ok_or_else(|| not_found(id, COLLECTIONS))?;
    Ok((id, c))
}

async fn admit(
    state: &SharedState,
    signer: &Signer,
    c: &Collection,
    collection: Uuid,
    item: Option<Uuid>,
    role: bool,
    since: Since,
) -> Result<Curator, ApiError> {
    let s = signer.as_str();
    if !c.published {
        return Err(ApiError::conflict("Unpublished collection")
            .with_data(json!({ "id": collection, "ethAddress": s })));
    }
    let committee = !role && on_committee(state, since, s).await?;
    if !role && !committee {
        return Err(ApiError::unauthorized("Unauthorized")
            .with_data(json!({ "id": item.unwrap_or(collection), "ethAddress": s })));
    }
    Ok(Curator {
        collection,
        item,
        signer: s.to_string(),
        committee,
    })
}

/// `curator`, admitted as the committee when the signer is on it: for a
/// write the transition table gives only the committee (503 while the
/// committee can't be read).
pub async fn as_committee(state: &SharedState, mut curator: Curator) -> Result<Curator, ApiError> {
    if !curator.committee {
        curator.committee = on_committee(state, Since::write(), &curator.signer).await?;
    }
    Ok(curator)
}

/// Whether `address` is on the current committee: who may be assigned a
/// review (503 while the committee can't be read).
pub async fn assignable(state: &State, address: &str) -> Result<bool, ApiError> {
    on_committee(state, Since::write(), &address.to_ascii_lowercase()).await
}

/// May set the status of item curations of one published collection
/// through the operator's item status routes: made only from an [`Admin`]
/// proof; it grants no other role.
#[derive(Debug)]
pub struct Operator {
    collection: Uuid,
    actor: String,
}

impl Operator {
    pub fn collection(&self) -> Uuid {
        self.collection
    }
    /// Who acts, for the audit row.
    pub fn actor(&self) -> &str {
        &self.actor
    }
}

/// The operator's item status routes over `collection`: it must exist (404)
/// and be published (409).
pub async fn operate(
    state: &SharedState,
    admin: &crate::auth::Admin,
    collection: Uuid,
) -> Result<Operator, ApiError> {
    let (mut conn, view) = decision(state).await?;
    operated(&mut conn, &view, collection).await?;
    Ok(Operator {
        collection,
        actor: admin.actor().to_string(),
    })
}

async fn operated(
    conn: &mut PgConnection,
    view: &DecisionView,
    collection: Uuid,
) -> Result<(), ApiError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM builder.collections WHERE id = $1)")
            .bind(collection)
            .fetch_one(&mut *conn)
            .await?;
    if !exists {
        return Err(ApiError::not_found("Not found"));
    }
    if !chain_state::published(conn, view, collection).await? {
        return Err(
            ApiError::conflict("Unpublished collection").with_data(json!({ "id": collection }))
        );
    }
    Ok(())
}

/// Whether the committee shows the signer on it, false when it
/// can't tell: for a notice, never for a grant.
pub async fn known_curator(state: &SharedState, curator: &Curator) -> bool {
    curator.committee || known_committee(state, Since::read(), &curator.signer).await
}

/// The facts of the curated collection a write reads, decided again inside
/// its transaction.
#[derive(Debug)]
pub struct CuratedState {
    pub name: String,
    /// The signer is the collection's creator: the chain's once the copy
    /// holds it, the stored owner before.
    pub creator: bool,
    pub third_party: bool,
    pub contract: Option<String>,
    pub published: bool,
}

/// The node's own standard publication routes: the stored owner of a
/// standard collection.
pub async fn publisher(
    state: &SharedState,
    signer: &Signer,
    id: Uuid,
) -> Result<Publisher, ApiError> {
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT eth_address, third_party_id IS NOT NULL FROM builder.collections WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(state.items.pool())
    .await?;
    let s = signer.as_str();
    let Some((owner, third_party)) = row else {
        return Err(not_found(id, COLLECTIONS));
    };
    if third_party || owner != s {
        return Err(unauthorized_user(s, COLLECTIONS, id));
    }
    Ok(Publisher {
        id,
        signer: s.to_string(),
    })
}

/// Whether a published item names the hash `{hash}` in a name whose writer
/// holds an upload record for it: an entry of its contents
/// (`item_contents.named_by`) or its video (`items.video_named_by`). It
/// starts from the hash's upload records, in upload order, so it never
/// visits a name written by a wallet without the bytes, and a published
/// file's first uploader answers first.
const PUBLIC_FILE: &str = "EXISTS (SELECT 1 FROM (SELECT u.uploaded_by FROM builder.uploaded_contents u \
       WHERE u.hash = {hash} ORDER BY u.created_at, u.uploaded_by OFFSET 0) w \
     CROSS JOIN LATERAL (SELECT 1 FROM builder.item_contents ic \
       JOIN builder.items i ON i.id = ic.item_id \
       JOIN builder.collections c ON c.id = i.collection_id \
       {chain_join} \
       WHERE ic.hash = {hash} AND ic.named_by = w.uploaded_by \
         AND CASE WHEN c.third_party_id IS NOT NULL THEN {curated} ELSE {published} END \
       UNION ALL SELECT 1 FROM builder.items i \
       JOIN builder.collections c ON c.id = i.collection_id \
       {chain_join} \
       WHERE i.video = {hash} AND i.video_named_by = w.uploaded_by \
         AND CASE WHEN c.third_party_id IS NOT NULL THEN {curated} ELSE {published} END \
       LIMIT 1) named)";

/// [`FILE_READ`]'s answer: public, the signer's own, and the providers
/// with their virtual third parties' managers.
type FileAnswer = (
    bool,
    bool,
    sqlx::types::Json<Vec<(String, Option<Vec<String>>)>>,
);

/// The most providers a file read checks the signer against.
const FILE_PROVIDERS: i64 = 20;

const FILE_READ: &str = "SELECT p.public, o.own, CASE WHEN p.public OR o.own OR $2::text IS NULL THEN '[]'::jsonb \
       ELSE COALESCE((SELECT jsonb_agg(jsonb_build_array(t.id, v.managers)) FROM ( \
         SELECT DISTINCT c.third_party_id AS id FROM builder.uploaded_contents u \
           JOIN builder.item_contents ic ON ic.hash = u.hash AND ic.named_by = u.uploaded_by \
           JOIN builder.items i ON i.id = ic.item_id \
           JOIN builder.collections c ON c.id = i.collection_id \
           WHERE u.hash = $1 AND c.third_party_id IS NOT NULL ORDER BY 1 LIMIT $3) t \
         LEFT JOIN builder.virtual_third_parties v ON v.id = t.id), '[]'::jsonb) END \
     FROM (SELECT {public} AS public OFFSET 0) p \
     CROSS JOIN LATERAL (SELECT CASE WHEN p.public OR $2::text IS NULL THEN false ELSE ( \
         EXISTS (SELECT 1 FROM builder.uploaded_contents WHERE hash = $1 AND uploaded_by = $2) \
         OR EXISTS (SELECT 1 FROM builder.uploaded_contents u \
           JOIN builder.item_contents ic ON ic.hash = u.hash AND ic.named_by = u.uploaded_by \
           JOIN builder.items i ON i.id = ic.item_id \
           LEFT JOIN builder.collections c ON c.id = i.collection_id \
           WHERE u.hash = $1 AND c.third_party_id IS NULL \
             AND COALESCE(c.eth_address, i.eth_address) = $2)) END AS own) o";

/// [`PUBLIC_FILE`] for the hash `hash` (a bound parameter or a column), with
/// the published rule of [`chain_state::published_by_any_pass`].
fn public_file_sql(hash: &str) -> String {
    let (chain_join, published) = chain_state::published_by_any_pass();
    PUBLIC_FILE
        .replace("{chain_join}", chain_join)
        .replace("{published}", published)
        .replace("{curated}", ITEM_CURATED)
        .replace("{hash}", hash)
}

/// The bytes and files of every upload record of `wallet`'s
/// (`uploaded_contents_uploaded_by_idx` alone), or of every wallet's.
pub(crate) async fn uploaded(
    pool: &sqlx::PgPool,
    wallet: Option<&str>,
) -> Result<crate::limits::Held<i64>, ApiError> {
    let sql = match wallet {
        Some(_) => {
            "SELECT COALESCE(sum(size), 0)::bigint, count(*) FROM builder.uploaded_contents WHERE uploaded_by = $1"
        }
        None => {
            "SELECT COALESCE(sum(size), 0)::bigint, count(*) FROM builder.uploaded_contents WHERE $1::text IS NULL"
        }
    };
    let (bytes, files): (i64, i64) = sqlx::query_as(sql).bind(wallet).fetch_one(pool).await?;
    Ok(crate::limits::Held { bytes, files })
}

/// The hashes a published item names in a name whose writer holds an upload
/// record for them ([`PUBLIC_FILE`] as a set): from each collection to its
/// items' names, so a count is linear in the names and the records, however
/// many wallets hold one hash.
const PUBLIC_HASHES: &str = "SELECT ic.hash FROM builder.collections c {chain_join} \
       JOIN builder.items i ON i.collection_id = c.id \
       JOIN builder.item_contents ic ON ic.item_id = i.id \
       JOIN builder.uploaded_contents u ON u.hash = ic.hash AND u.uploaded_by = ic.named_by \
       WHERE CASE WHEN c.third_party_id IS NOT NULL THEN {curated} ELSE {published} END \
     UNION SELECT i.video FROM builder.collections c {chain_join} \
       JOIN builder.items i ON i.collection_id = c.id \
       JOIN builder.uploaded_contents u ON u.hash = i.video AND u.uploaded_by = i.video_named_by \
       WHERE CASE WHEN c.third_party_id IS NOT NULL THEN {curated} ELSE {published} END";

/// Bytes and files of upload records per wallet listed (`None`: the rest)
/// and of all of them (the grouping flag set).
type Sums = Vec<(Option<String>, bool, i64, i64)>;

/// The node's upload records counted whole: the bytes and files of every
/// wallet's unpublished ones (whose hash no published item names,
/// [`PUBLIC_FILE`]), and, of each wallet whose records pass `near_wallet`
/// in bytes or files, its unpublished ones and all of its records. The
/// unpublished ones are counted only when a wallet is listed or every
/// record together reaches `half_node`; else all of them count, as they do
/// when that count can't finish within its statement timeout.
pub(crate) async fn tally_uploads(
    pool: &sqlx::PgPool,
    near_wallet: crate::limits::Held<u64>,
    half_node: crate::limits::Held<u64>,
) -> Result<
    (
        crate::limits::Held<i64>,
        std::collections::HashMap<String, (crate::limits::Held<i64>, crate::limits::Held<i64>)>,
    ),
    ApiError,
> {
    use crate::limits::Held;
    let held = |bytes, files| Held { bytes, files };
    let sums: Sums = sqlx::query_as(
        "SELECT uploaded_by, GROUPING(uploaded_by) = 1, COALESCE(sum(size), 0)::bigint, count(*) \
         FROM builder.uploaded_contents GROUP BY GROUPING SETS ((uploaded_by), ()) \
         HAVING GROUPING(uploaded_by) = 1 OR sum(size) > $1 OR count(*) >= $2",
    )
    .bind(i64::try_from(near_wallet.bytes).unwrap_or(i64::MAX))
    .bind(i64::try_from(near_wallet.files).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await?;
    let mut all = Held::default();
    let mut wallets = std::collections::HashMap::new();
    for (wallet, whole, bytes, files) in sums {
        match (whole, wallet) {
            (true, _) => all = held(bytes, files),
            (false, Some(wallet)) => {
                wallets.insert(wallet, (held(bytes, files), held(bytes, files)));
            }
            (false, None) => {}
        }
    }
    let reaches = |n: i64, bound: u64| u64::try_from(n).unwrap_or(0) >= bound;
    if wallets.is_empty()
        && !reaches(all.bytes, half_node.bytes)
        && !reaches(all.files, half_node.files)
    {
        return Ok((all, wallets));
    }
    let listed: Vec<String> = wallets.keys().cloned().collect();
    match unpublished_sums(pool, &listed).await {
        Ok(sums) => {
            let mut node = Held::default();
            for (unpublished, _) in wallets.values_mut() {
                *unpublished = Held::default();
            }
            for (wallet, whole, bytes, files) in sums {
                match (whole, wallet.and_then(|w| wallets.get_mut(&w))) {
                    (true, _) => node = held(bytes, files),
                    (false, Some((unpublished, _))) => *unpublished = held(bytes, files),
                    (false, None) => {}
                }
            }
            Ok((node, wallets))
        }
        Err(error) => {
            tracing::warn!(%error, "builder: the unpublished uploads could not be counted; all records count");
            Ok((all, wallets))
        }
    }
}

async fn unpublished_sums(pool: &sqlx::PgPool, listed: &[String]) -> Result<Sums, sqlx::Error> {
    let (chain_join, published) = chain_state::published_by_any_pass();
    let public = PUBLIC_HASHES
        .replace("{chain_join}", chain_join)
        .replace("{published}", published)
        .replace("{curated}", ITEM_CURATED);
    let mut tx = pool.begin().await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "SET LOCAL statement_timeout = '{}s'",
        crate::limits::UNPUBLISHED_COUNT_TIMEOUT.as_secs()
    )))
    .execute(&mut *tx)
    .await?;
    sqlx::query("SET LOCAL jit = off").execute(&mut *tx).await?;
    let sums = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH public AS MATERIALIZED ({public}) \
         SELECT w, GROUPING(w) = 1, COALESCE(sum(size), 0)::bigint, count(*) FROM ( \
           SELECT CASE WHEN own.uploaded_by = ANY($1) THEN own.uploaded_by END AS w, own.size \
           FROM builder.uploaded_contents own \
           WHERE NOT EXISTS (SELECT 1 FROM public p WHERE p.hash = own.hash)) unpublished \
         GROUP BY GROUPING SETS ((w), ())"
    )))
    .bind(listed)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(sums)
}

/// The bytes and files of `wallet`'s upload records whose hash no published
/// item names ([`PUBLIC_FILE`]).
#[cfg(test)]
pub(crate) async fn unpublished(
    pool: &sqlx::PgPool,
    wallet: &str,
) -> Result<crate::limits::Held<i64>, ApiError> {
    let sums = unpublished_sums(pool, &[wallet.to_string()]).await?;
    Ok(sums
        .into_iter()
        .find(|(w, whole, _, _)| !whole && w.as_deref() == Some(wallet))
        .map(|(_, _, bytes, files)| crate::limits::Held { bytes, files })
        .unwrap_or_default())
}

/// [`FILE_READ`] with the published rule of [`chain_state::published_by_any_pass`].
pub(crate) fn file_read_sql() -> String {
    FILE_READ.replace("{public}", &public_file_sql("$1"))
}

/// `GET /v1/storage/contents/:hash` and its `exists` probe: who may read an
/// item file. Anyone, signed or not, while a published item names the hash
/// in an entry, or as its video, whose writer holds an upload record for it (a standard item is
/// published once its collection is; a third-party item once it has an item
/// curation, [`ITEM_CURATED`]). A signer also reads a file it uploaded itself, and a file
/// that an item it owns (the owner of its standard collection, or of an
/// item with no collection), or a third-party item whose collection it manages,
/// names in such an entry. A name opens a file only for the wallet that
/// wrote it holding the bytes; the committee and on-chain managers read what
/// anyone reads. Whether the signer manages a provider is the registry's
/// answer ([`provider_managers`]), read only for a signed request of a file
/// no other branch opens that a third-party item names.
/// `None` refuses: a hash that is not a canonical CID or that the node
/// withholds (`content::Denylist`) is refused before any read, and a private
/// file is refused as an absent one. Reads no committee and no fresh chain fact
/// ([`chain_state::published_by_any_pass`]).
pub async fn read_file(
    state: &SharedState,
    signer: Option<&Signer>,
    hash: &str,
) -> Result<Option<FileRead>, ApiError> {
    if !dcl_auth::is_canonical_cid(hash) || state.denylist.contains(hash) {
        return Ok(None);
    }
    let since = Since::read();
    let (public, own, named): FileAnswer = sqlx::query_as(sqlx::AssertSqlSafe(file_read_sql()))
        .bind(hash)
        .bind(signer.map(Signer::as_str))
        .bind(FILE_PROVIDERS)
        .fetch_one(state.items.pool())
        .await?;
    let readable = own
        || match (signer, named.0.is_empty()) {
            (Some(signer), false) => {
                let registry = registry(state, since).await?;
                named.0.iter().any(|(provider, listed)| {
                    has_manager(
                        &provider_managers(Some(&registry), provider, listed.as_deref()),
                        signer.as_str(),
                    )
                })
            }
            _ => false,
        };
    Ok((public || readable).then(|| FileRead {
        hash: hash.to_string(),
        public: signer.is_none(),
    }))
}

/// `GET /v1/published-collections/:address/items/:id/utility` and `GET
/// /v1/items/:address/:id/contents`, unsigned: the minted item whose
/// standard collection's stored contract is exactly `contract` and whose
/// on-chain id is exactly `chain_id`; `None` when no item is. Such an item's
/// collection is published by the node's own record, so no chain fact is
/// read. A `contract` that is not a lowercase address, or a `chain_id` that
/// is not decimal digits, names no stored item and reads nothing.
pub async fn minted_item(
    state: &SharedState,
    contract: &str,
    chain_id: &str,
) -> Result<Option<MintedItem>, ApiError> {
    let stored_contract = contract.len() == 42
        && contract.starts_with("0x")
        && contract[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let stored_id = !chain_id.is_empty()
        && chain_id.len() <= 78
        && chain_id.bytes().all(|b| b.is_ascii_digit());
    if !stored_contract || !stored_id {
        return Ok(None);
    }
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT i.id FROM builder.collections c JOIN builder.items i ON i.collection_id = c.id \
         WHERE c.contract_address = $1 AND c.third_party_id IS NULL AND i.blockchain_item_id = $2 \
         ORDER BY i.created_at, i.id LIMIT 1",
    )
    .bind(contract)
    .bind(chain_id)
    .fetch_optional(state.items.pool())
    .await?;
    Ok(id.map(|id| MintedItem { id }))
}

/// A minted item's contents entries `(file, hash)` that [`read_file`] opens
/// to an unsigned request, in their order: one statement over the same
/// published rule ([`PUBLIC_FILE`]), then the canonical-hash and withheld
/// checks.
pub async fn public_entries(
    state: &SharedState,
    item: &MintedItem,
) -> Result<Vec<(String, String)>, ApiError> {
    let rows: Vec<(String, String, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT e.file, e.hash, {} FROM builder.item_contents e \
         WHERE e.item_id = $1 ORDER BY e.position, e.file",
        public_file_sql("e.hash")
    )))
    .bind(item.id)
    .fetch_all(state.items.pool())
    .await?;
    Ok(rows
        .into_iter()
        .filter(|(_, hash, public)| {
            *public && dcl_auth::is_canonical_cid(hash) && !state.denylist.contains(hash)
        })
        .map(|(file, hash, _)| (file, hash))
        .collect())
}

/// Grants for store tests that exercise a store function by itself; the
/// writes still confirm their decision inside their transaction.
#[cfg(test)]
pub(crate) mod granted {
    use super::*;

    pub fn wallet(address: &str) -> Wallet {
        Wallet {
            address: address.to_string(),
            providers: Vec::new(),
        }
    }

    pub fn managing(address: &str, providers: &[&str]) -> Wallet {
        Wallet {
            address: address.to_string(),
            providers: providers.iter().map(|p| (p.to_string(), false)).collect(),
        }
    }

    pub fn read(id: Uuid) -> CollectionRead {
        CollectionRead {
            id,
            full: true,
            programmatic: false,
        }
    }

    /// An empty registry: a provider's managers are its virtual third
    /// party's.
    fn no_registry() -> Option<Registry> {
        Some(Registry::default())
    }

    pub fn edit(signer: &str, id: Uuid, provider: Option<&str>) -> CollectionEdit {
        CollectionEdit {
            id,
            signer: signer.to_string(),
            at: Utc::now(),
            provider: provider.map(str::to_string),
            registry: no_registry(),
            own_content: true,
        }
    }

    pub fn item(id: Uuid) -> ItemRead {
        ItemRead { id, full: true }
    }

    pub fn save(signer: &str, id: Uuid, collection: Option<Uuid>) -> ItemSave {
        ItemSave {
            id,
            signer: signer.to_string(),
            collection,
            at: Utc::now(),
            registry: no_registry(),
            own_content: true,
        }
    }

    pub fn change(signer: &str, id: Uuid) -> ItemChange {
        ItemChange {
            id,
            signer: signer.to_string(),
            at: Utc::now(),
            registry: no_registry(),
        }
    }

    pub fn third_party_publish(signer: &str, id: Uuid, provider: &str) -> ThirdPartyPublish {
        ThirdPartyPublish {
            id,
            signer: signer.to_string(),
            provider: provider.to_string(),
            registry: Registry::default(),
        }
    }

    pub fn publisher(signer: &str, id: Uuid) -> Publisher {
        Publisher {
            id,
            signer: signer.to_string(),
        }
    }
}

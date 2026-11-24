//! The DCL NAME authority: which NAMEs an address holds and who holds a
//! NAME, as every crate asks it (worlds' ownership and deploys, comms
//! admission and moderation, communities, the shop's profiles, outfits and
//! lists). One source is selected per node at startup ([`NameAuthority`] is
//! injected): the shop's nft index on mainnet, the marketplace subgraph only
//! where the operator selects it (a testnet node, which the index does not
//! walk), or none.
//!
//! Every read names its [`Purpose`]. An authorization needs the index to have
//! folded everything it walked; a display read accepts a recent publication.
//! A label is asked bare (`alice`, never `alice.dcl.eth`): the caller parses a
//! raw name once, at its own boundary, and no source strips a suffix again.
//! An answer carries the [`Observation`] it was read under, so a memo files it
//! there and asks [`NameAuthority::still_current`] before serving it again: a
//! rewind withdraws the publication and an advance leaves it unfolded, and
//! neither may let a former holder's answer back.

use std::collections::HashMap;
use std::sync::Arc;

use crate::ChainIndex;
pub use crate::graph::OwnedName;

/// Why a NAME is read, which decides how current the source must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// Grants or refuses a permission: a world mutation or admission, a
    /// community's creation or ownership transfer, the target of a
    /// moderation write. The index must have folded every block it walked,
    /// or, between walking and publishing a pass, have walked no NAME
    /// registrar event past its published fold (which is then every NAME's
    /// state still).
    Authorize,
    /// Shown to someone (lists, enrichment): published recently.
    Display,
}

/// The publication of the nft index an answer was read under. Two answers
/// with equal observations were read from the same fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Observation {
    /// The block the Ethereum fold was published through.
    pub folded_block: i64,
    /// When that fold was published (unix microseconds, the database's
    /// precision: two publications at one block differ here).
    pub folded_at_us: i64,
}

/// Why the authority gave no answer. Neither is "nobody holds it".
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NamesError {
    /// The authority cannot answer now: the index is not current for the
    /// purpose (catching up, an unfolded advance, a withdrawn publication,
    /// quarantined), two holders claim one label, or no source is selected.
    #[error("{0}")]
    NotEstablished(String),
    /// The source failed (a database or subgraph error).
    #[error("{0}")]
    Failed(String),
}

/// An answer and the publication it was read under (`None` for a source
/// without publications, the subgraph, whose callers keep their own TTLs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer<T> {
    pub value: T,
    pub observation: Option<Observation>,
}

#[async_trait::async_trait]
pub trait NameAuthority: Send + Sync {
    /// The NAMEs `owner` holds, as minted, in the order the old subgraph
    /// listed them (by NFT id), each with its open order's price.
    async fn owned_names(
        &self,
        owner: &str,
        purpose: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError>;

    /// Who holds the bare `label` (compared lowercased), lowercased; `None`
    /// when nobody does. Two holders of one label once lowercased are
    /// [`NamesError::NotEstablished`], never a choice, for a source that can
    /// hold both (the index); see [`SubgraphNames`] for the subgraph.
    async fn name_owner(
        &self,
        label: &str,
        purpose: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError>;

    /// [`NameAuthority::name_owner`] for many bare labels in one read, keyed
    /// by the lowercased label; an ambiguous label is left out.
    async fn name_owners(
        &self,
        labels: &[String],
        purpose: Purpose,
    ) -> Result<Answer<HashMap<String, Option<String>>>, NamesError>;

    /// [`NameAuthority::owned_names`] read on `conn`, a database session the
    /// caller holds (a partial upload's): a source that reads the database
    /// reads it there rather than waiting on a pool the caller may be
    /// holding the last connection of. Others ignore it.
    async fn owned_names_on(
        &self,
        conn: &mut sqlx::PgConnection,
        owner: &str,
        purpose: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        let _ = conn;
        self.owned_names(owner, purpose).await
    }

    /// [`NameAuthority::name_owner`] read on `conn` (see
    /// [`NameAuthority::owned_names_on`]).
    async fn name_owner_on(
        &self,
        conn: &mut sqlx::PgConnection,
        label: &str,
        purpose: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        let _ = conn;
        self.name_owner(label, purpose).await
    }

    /// Whether an answer read under `observation` may still be served for
    /// `purpose` now: the same publication stands and is current enough.
    /// A memo asks it on every hit.
    async fn still_current(
        &self,
        observation: &Observation,
        purpose: Purpose,
    ) -> Result<bool, NamesError>;

    /// Whether this source reads publications (the index): a memo may then
    /// keep an answer only with its observation.
    fn observes(&self) -> bool;
}

/// The marketplace subgraph as the NAME source, where the operator selects
/// it (a testnet node). Its answers carry no observation; an empty URL
/// answers [`NamesError::NotEstablished`], never an empty holder list. It
/// answers a label's first holder by NFT id (`first: 1`), for every purpose:
/// it keys a NAME by the registrar's lowercased `searchText`, so it cannot
/// see the two holders the index refuses.
pub struct SubgraphNames(pub Arc<ChainIndex>);

impl SubgraphNames {
    fn configured(&self) -> Result<(), NamesError> {
        if self.0.settings.marketplace_subgraph_url.is_some() {
            Ok(())
        } else {
            Err(NamesError::NotEstablished(
                "no marketplace subgraph is configured for DCL NAMEs".into(),
            ))
        }
    }
}

#[async_trait::async_trait]
impl NameAuthority for SubgraphNames {
    async fn owned_names(
        &self,
        owner: &str,
        _purpose: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        self.configured()?;
        crate::graph::owned_names(&self.0, owner)
            .await
            .map(|value| Answer {
                value,
                observation: None,
            })
            .map_err(NamesError::Failed)
    }

    async fn name_owner(
        &self,
        label: &str,
        _purpose: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        self.configured()?;
        crate::graph::name_owner(&self.0, label)
            .await
            .map(|value| Answer {
                value,
                observation: None,
            })
            .map_err(NamesError::Failed)
    }

    async fn name_owners(
        &self,
        labels: &[String],
        _purpose: Purpose,
    ) -> Result<Answer<HashMap<String, Option<String>>>, NamesError> {
        self.configured()?;
        crate::graph::name_owners(&self.0, labels)
            .await
            .map(|value| Answer {
                value,
                observation: None,
            })
            .map_err(NamesError::Failed)
    }

    async fn still_current(
        &self,
        _observation: &Observation,
        _purpose: Purpose,
    ) -> Result<bool, NamesError> {
        Ok(true)
    }

    fn observes(&self) -> bool {
        false
    }
}

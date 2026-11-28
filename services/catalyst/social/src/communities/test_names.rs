//! An in-memory DCL NAME source for the community tests: who holds which
//! NAME, or a source that cannot establish it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::chain_index::names::{
    Answer, NameAuthority, NamesError, Observation, OwnedName, Purpose,
};

/// How every read fails while set: a [`NamesError`] variant.
type Failure = fn(String) -> NamesError;

#[derive(Default)]
pub(crate) struct Held {
    names: Mutex<HashMap<String, Vec<OwnedName>>>,
    failing: Mutex<Option<Failure>>,
}

impl Held {
    pub(crate) fn grant(&self, wallet: &str, name: &str) {
        self.names
            .lock()
            .unwrap()
            .entry(wallet.to_lowercase())
            .or_default()
            .push(OwnedName {
                name: name.to_string(),
                contract_address: "0x0000000000000000000000000000000000000001".to_string(),
                token_id: "1".to_string(),
                price: None,
            });
    }

    pub(crate) fn clear(&self) {
        self.names.lock().unwrap().clear();
    }

    /// Every read fails with `failure` (`NamesError::NotEstablished`, a
    /// transient fault, ...) until called with `None`.
    pub(crate) fn fail_with(&self, failure: Option<Failure>) {
        *self.failing.lock().unwrap() = failure;
    }

    fn available(&self) -> Result<(), NamesError> {
        match *self.failing.lock().unwrap() {
            Some(failure) => Err(failure("the test NAME source cannot answer".into())),
            None => Ok(()),
        }
    }

    fn holder(&self, label: &str) -> Option<String> {
        let label = label.to_lowercase();
        self.names
            .lock()
            .unwrap()
            .iter()
            .find(|(_, names)| names.iter().any(|n| n.name.to_lowercase() == label))
            .map(|(owner, _)| owner.clone())
    }
}

#[async_trait::async_trait]
impl NameAuthority for Held {
    async fn owned_names(
        &self,
        owner: &str,
        _purpose: Purpose,
    ) -> Result<Answer<Arc<Vec<OwnedName>>>, NamesError> {
        self.available()?;
        let names = self
            .names
            .lock()
            .unwrap()
            .get(&owner.to_lowercase())
            .cloned();
        Ok(Answer {
            value: Arc::new(names.unwrap_or_default()),
            observation: None,
        })
    }

    async fn name_owner(
        &self,
        label: &str,
        _purpose: Purpose,
    ) -> Result<Answer<Option<String>>, NamesError> {
        self.available()?;
        Ok(Answer {
            value: self.holder(label),
            observation: None,
        })
    }

    async fn name_owners(
        &self,
        labels: &[String],
        _purpose: Purpose,
    ) -> Result<Answer<HashMap<String, Option<String>>>, NamesError> {
        self.available()?;
        Ok(Answer {
            value: labels
                .iter()
                .map(|l| (l.to_lowercase(), self.holder(l)))
                .collect(),
            observation: None,
        })
    }

    async fn still_current(&self, _: &Observation, _: Purpose) -> Result<bool, NamesError> {
        Ok(true)
    }

    fn observes(&self) -> bool {
        false
    }
}

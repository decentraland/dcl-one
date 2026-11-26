//! An in-memory DCL NAME source for the community tests: who holds which
//! NAME, or a source that cannot establish it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::chain_index::names::{
    Answer, NameAuthority, NamesError, Observation, OwnedName, Purpose,
};

#[derive(Default)]
pub(crate) struct Held {
    names: Mutex<HashMap<String, Vec<OwnedName>>>,
    unavailable: Mutex<bool>,
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

    /// Every read answers "not established" until called with `false`.
    pub(crate) fn set_unavailable(&self, unavailable: bool) {
        *self.unavailable.lock().unwrap() = unavailable;
    }

    fn available(&self) -> Result<(), NamesError> {
        if *self.unavailable.lock().unwrap() {
            Err(NamesError::NotEstablished(
                "the test NAME source is not current".into(),
            ))
        } else {
            Ok(())
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

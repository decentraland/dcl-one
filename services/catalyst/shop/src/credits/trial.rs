//! A closed trial of on-chain credits (`CREDITS_TRIAL_MODE`): new credit authorizations go only
//! to the buyers `CREDITS_TRIAL_BUYERS` names. The buyer is the root signer of the request's
//! auth chain, lowercase, as `signer_from` verified it; never the ephemeral key or an address of
//! the body. An empty list admits no one: a trial is never open by omission.

use std::collections::BTreeSet;

use super::usage::normalize_address;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrialBuyers {
    buyers: BTreeSet<String>,
}

impl TrialBuyers {
    /// The comma- or whitespace-separated addresses of `CREDITS_TRIAL_BUYERS` (or its `_FILE`),
    /// deduplicated. One entry that is not an address refuses the start, naming its position
    /// and not its text; none at all is a trial that issues nothing.
    pub fn parse(raw: Option<&str>) -> anyhow::Result<Self> {
        let mut buyers = BTreeSet::new();
        let entries = raw
            .unwrap_or_default()
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|entry| !entry.is_empty());
        for (index, entry) in entries.enumerate() {
            let address = normalize_address(entry).ok_or_else(|| {
                anyhow::anyhow!(
                    "CREDITS_TRIAL_BUYERS entry {} is not a 0x-prefixed 20-byte address",
                    index + 1
                )
            })?;
            buyers.insert(address);
        }
        Ok(Self { buyers })
    }

    /// Whether `signer` (the verified root signer) may get a new credit authorization.
    pub fn admits(&self, signer: &str) -> bool {
        normalize_address(signer).is_some_and(|address| self.buyers.contains(&address))
    }

    pub fn len(&self) -> usize {
        self.buyers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buyers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0x00000000000000000000000000000000000000aa";
    const B: &str = "0x00000000000000000000000000000000000000BB";

    #[test]
    fn the_list_is_parsed_deduplicated_and_compared_as_addresses() {
        let trial = TrialBuyers::parse(Some(&format!(
            " {A},{B}\n{} ,",
            A.to_uppercase().replace("0X", "0x")
        )))
        .unwrap();
        assert_eq!(trial.len(), 2);
        assert!(trial.admits(A));
        assert!(trial.admits(&B.to_lowercase()));
        assert!(trial.admits(B));
        assert!(!trial.admits("0x00000000000000000000000000000000000000cc"));
        assert!(!trial.admits("not an address"));
    }

    #[test]
    fn an_empty_or_absent_list_admits_no_one() {
        for raw in [None, Some(""), Some(" , \n")] {
            let trial = TrialBuyers::parse(raw).unwrap();
            assert!(trial.is_empty());
            assert!(!trial.admits(A));
        }
    }

    #[test]
    fn one_invalid_entry_refuses_the_list_without_echoing_it() {
        let why = TrialBuyers::parse(Some(&format!("{A},0xnot-SECRET")))
            .unwrap_err()
            .to_string();
        assert!(why.contains("entry 2") && !why.contains("SECRET"), "{why}");
    }
}

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

/// Under a trial the revoker and the relay never sign with one key: two direct signers, each with
/// its own send lock, would race for one `pending` nonce. Compared by the address each key derives
/// (as text when one doesn't parse); never echoed.
pub fn refuse_a_shared_key(revoker: Option<&str>, relayer: Option<&str>) -> anyhow::Result<()> {
    let (Some(revoker), Some(relayer)) = (revoker, relayer) else {
        return Ok(());
    };
    let address = |key: &str| {
        crate::evm::signer::parse_signing_key(key)
            .map(|signer| format!("{:#x}", signer.address()))
            .unwrap_or_else(|_| key.trim().trim_start_matches("0x").to_ascii_lowercase())
    };
    if address(revoker) == address(relayer) {
        anyhow::bail!(
            "CREDITS_TRIAL_MODE needs the revoker and the relay to sign with different keys: CREDITS_REVOKER_KEY_FILE and RELAYER_PRIVATE_KEY_FILE hold the same one"
        );
    }
    Ok(())
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
    fn the_revoker_and_the_relay_never_share_a_key() {
        const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
        const OTHER: &str = "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
        let why = refuse_a_shared_key(Some(KEY), Some(&KEY[2..].to_uppercase()))
            .unwrap_err()
            .to_string();
        assert!(
            why.contains("different keys") && !why.contains(&KEY[2..]),
            "{why}"
        );
        assert!(refuse_a_shared_key(Some(KEY), Some(OTHER)).is_ok());
        assert!(refuse_a_shared_key(Some(KEY), None).is_ok());
        assert!(refuse_a_shared_key(None, Some(KEY)).is_ok());
    }

    #[test]
    fn one_invalid_entry_refuses_the_list_without_echoing_it() {
        let why = TrialBuyers::parse(Some(&format!("{A},0xnot-SECRET")))
            .unwrap_err()
            .to_string();
        assert!(why.contains("entry 2") && !why.contains("SECRET"), "{why}");
    }
}

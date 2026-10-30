//! The one 0x-address predicate: 42 characters, the lowercase `0x` prefix
//! (`0X` is not an address: no signer, checksum or subgraph spells it so, and
//! the old market copy that took it was the only one), 40 hex digits of any
//! case. [`normalize_eth_address`] is the trim-lowercase-validate form the
//! ban lists and address parameters use; folding the case is its job, so it
//! reads `0X...` as the address it lowercases to.
pub type EthAddress = String;

pub fn is_eth_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// The address trimmed and lowercased, or `None` when it is not one.
pub fn normalize_eth_address(value: &str) -> Option<String> {
    let lowered = value.trim().to_lowercase();
    is_eth_address(&lowered).then_some(lowered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_eth_address_accepts_lowercase_and_mixed_case() {
        assert!(is_eth_address("0x0000000000000000000000000000000000000000"));
        assert!(is_eth_address("0xabcdefABCDEF0123456789abcdefABCDEF012345"));
        assert!(is_eth_address("0x1234567890abcdef1234567890abcdef12345678"));
    }

    #[test]
    fn is_eth_address_rejects_bad_inputs() {
        assert!(!is_eth_address("0x0"));
        assert!(!is_eth_address("0x123"));
        assert!(!is_eth_address(
            "00000000000000000000000000000000000000000000"
        ));
        assert!(!is_eth_address("1234567890abcdef1234567890abcdef12345678"));
        assert!(!is_eth_address(
            "0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"
        ));
        assert!(!is_eth_address("0x000000000000000000000000000000000000000"));
        assert!(!is_eth_address(""));
    }

    /// `is_eth_address` refuses the spelling; `normalize_eth_address` folds
    /// case first (its contract), so it reads the same input as the address.
    #[test]
    fn an_uppercase_0x_prefix_is_not_an_address_until_normalized() {
        assert!(!is_eth_address(
            "0X1234567890abcdef1234567890abcdef12345678"
        ));
        assert_eq!(
            normalize_eth_address("0X1234567890abcdef1234567890abcdef12345678").as_deref(),
            Some("0x1234567890abcdef1234567890abcdef12345678")
        );
    }

    #[test]
    fn normalize_lowercases_valid_addresses_and_trims() {
        assert_eq!(
            normalize_eth_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").as_deref(),
            Some("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        );
        assert_eq!(
            normalize_eth_address("  0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\n").as_deref(),
            Some("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        );
    }

    #[test]
    fn normalize_rejects_malformed() {
        assert_eq!(normalize_eth_address("0xABCdef"), None);
        assert_eq!(normalize_eth_address("not-an-address"), None);
        assert_eq!(
            normalize_eth_address("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"),
            None
        );
    }
}

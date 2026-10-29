use super::errors::AppError;

pub const MAX_IDS_OR_POINTERS: usize = 1000;

/// The lamb2 sentence for POST /entities/active; the multi-clause text is part
/// of the contract.
pub const ACTIVE_ENTITIES_SELECTOR_MESSAGE: &str = "ids or pointers must be present, but not both. \
    They must be arrays and contain at least one element. \
    None of the elements can be empty.";

/// GET /entities/{type} answers with the short form.
pub const ENTITIES_SELECTOR_MESSAGE: &str = "ids or pointers must be present, but not both";

#[derive(Debug, PartialEq, Eq)]
pub enum SelectorError {
    Malformed,
    TooMany(usize),
}

impl SelectorError {
    pub fn into_app_error(self, malformed_message: &str) -> AppError {
        match self {
            SelectorError::Malformed => AppError::invalid(malformed_message),
            SelectorError::TooMany(max) => AppError::invalid(format!(
                "Too many ids or pointers; the maximum allowed is {max}"
            )),
        }
    }
}

/// Exactly one of `ids`/`pointers`, non-empty, every element non-empty and
/// NUL-free; returns whether ids were selected.
pub fn validate_ids_or_pointers(
    ids: Option<&[String]>,
    pointers: Option<&[String]>,
    max: usize,
) -> Result<bool, SelectorError> {
    let is_valid = |s: &String| !s.is_empty() && !s.contains('\0');
    let (len, use_ids) = match (ids, pointers) {
        (Some(ids), None) if !ids.is_empty() && ids.iter().all(is_valid) => (ids.len(), true),
        (None, Some(pointers)) if !pointers.is_empty() && pointers.iter().all(is_valid) => {
            (pointers.len(), false)
        }
        _ => return Err(SelectorError::Malformed),
    };

    if len > max {
        return Err(SelectorError::TooMany(max));
    }

    Ok(use_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn validate(
        ids: Option<&[String]>,
        pointers: Option<&[String]>,
    ) -> Result<bool, SelectorError> {
        validate_ids_or_pointers(ids, pointers, MAX_IDS_OR_POINTERS)
    }

    #[test]
    fn accepts_ids_only() {
        let ids = strings(&["a", "b"]);
        assert!(validate(Some(&ids), None).unwrap());
    }

    #[test]
    fn accepts_pointers_only() {
        let ptrs = strings(&["0,0"]);
        assert!(!validate(None, Some(&ptrs)).unwrap());
    }

    #[test]
    fn rejects_both_present() {
        let ids = strings(&["a"]);
        let ptrs = strings(&["0,0"]);
        assert_eq!(
            validate(Some(&ids), Some(&ptrs)),
            Err(SelectorError::Malformed)
        );
    }

    #[test]
    fn rejects_neither_present() {
        assert_eq!(validate(None, None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_empty_ids_array() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(validate(Some(&empty), None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_empty_string_element_in_ids() {
        let ids = strings(&["a", "", "c"]);
        assert_eq!(validate(Some(&ids), None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_empty_string_element_in_pointers() {
        let ptrs = strings(&[""]);
        assert_eq!(validate(None, Some(&ptrs)), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_nul_byte_element_in_ids() {
        let ids = strings(&["a", "\0", "c"]);
        assert_eq!(validate(Some(&ids), None), Err(SelectorError::Malformed));
    }

    #[test]
    fn rejects_nul_byte_element_in_pointers() {
        let ptrs = strings(&["0,0\0"]);
        assert_eq!(validate(None, Some(&ptrs)), Err(SelectorError::Malformed));
    }

    #[test]
    fn accepts_exactly_1000_ids() {
        let ids: Vec<String> = (0..MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        assert_eq!(ids.len(), 1000);
        assert!(validate(Some(&ids), None).unwrap());
    }

    #[test]
    fn rejects_over_1000_ids() {
        let ids: Vec<String> = (0..=MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        assert_eq!(ids.len(), 1001);
        let err = validate(Some(&ids), None).unwrap_err();
        assert_eq!(err, SelectorError::TooMany(1000));
        assert_eq!(
            err.into_app_error("x").to_string(),
            "Too many ids or pointers; the maximum allowed is 1000"
        );
    }

    #[test]
    fn rejects_over_1000_pointers() {
        let ptrs: Vec<String> = (0..=MAX_IDS_OR_POINTERS).map(|i| i.to_string()).collect();
        assert_eq!(
            validate(None, Some(&ptrs)),
            Err(SelectorError::TooMany(1000))
        );
    }

    #[test]
    fn malformed_selector_carries_the_endpoint_message() {
        let err = SelectorError::Malformed.into_app_error(ACTIVE_ENTITIES_SELECTOR_MESSAGE);
        assert_eq!(err.to_string(), ACTIVE_ENTITIES_SELECTOR_MESSAGE);
        assert!(ACTIVE_ENTITIES_SELECTOR_MESSAGE.starts_with(ENTITIES_SELECTOR_MESSAGE));
    }
}

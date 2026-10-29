//! Verdicts. [`ValidationResponse`] is the rules' step type (the old one,
//! unchanged); [`ValidationError`] is what [`super::validate`] hands the write
//! path: each error string with its kind. [`ValidatorError`] is a source's
//! failure inside a check; every one of them becomes `Unavailable`.

use std::fmt;

#[derive(Debug, Clone)]
pub enum ValidationResponse {
    Ok,
    Failed {
        errors: Vec<String>,
    },
    /// The verdict is unknown because this node's own state could not be read; callers must answer
    /// with a retryable server error, never a rejection, so node damage is never blamed on the caller.
    Unavailable {
        errors: Vec<String>,
    },
}

impl ValidationResponse {
    pub fn is_ok(&self) -> bool {
        matches!(self, ValidationResponse::Ok)
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self, ValidationResponse::Unavailable { .. })
    }

    pub fn failed(errors: impl IntoIterator<Item = String>) -> Self {
        let errors: Vec<String> = errors.into_iter().collect();
        debug_assert!(!errors.is_empty(), "failed() must have at least one error");
        ValidationResponse::Failed { errors }
    }

    pub fn fail(msg: impl Into<String>) -> Self {
        ValidationResponse::Failed {
            errors: vec![msg.into()],
        }
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        ValidationResponse::Unavailable {
            errors: vec![msg.into()],
        }
    }

    pub fn from_errors(errors: Vec<String>) -> Self {
        if errors.is_empty() {
            ValidationResponse::Ok
        } else {
            ValidationResponse::Failed { errors }
        }
    }

    pub fn errors(&self) -> Option<&[String]> {
        match self {
            ValidationResponse::Ok => None,
            ValidationResponse::Failed { errors } => Some(errors),
            ValidationResponse::Unavailable { errors } => Some(errors),
        }
    }

    pub fn into_result(self) -> Result<(), Vec<ValidationError>> {
        match self {
            ValidationResponse::Ok => Ok(()),
            ValidationResponse::Failed { errors } => {
                Err(errors.into_iter().map(ValidationError::Rejected).collect())
            }
            ValidationResponse::Unavailable { errors } => Err(errors
                .into_iter()
                .map(ValidationError::Unavailable)
                .collect()),
        }
    }
}

impl fmt::Display for ValidationResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationResponse::Ok => write!(f, "ok"),
            ValidationResponse::Failed { errors } => {
                write!(f, "validation failed: {}", errors.join("; "))
            }
            ValidationResponse::Unavailable { errors } => {
                write!(
                    f,
                    "validation could not be completed: {}",
                    errors.join("; ")
                )
            }
        }
    }
}

/// One error of a refused deployment, carrying the old validator's text.
/// `Rejected`: the deployment is at fault (answer 400 with the text).
/// `Unavailable`: this node could not decide (answer 503 with `Retry-After`).
/// One verdict never mixes the two kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    Rejected(String),
    Unavailable(String),
}

impl ValidationError {
    pub fn message(&self) -> &str {
        match self {
            ValidationError::Rejected(m) | ValidationError::Unavailable(m) => m,
        }
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self, ValidationError::Unavailable(_))
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// A check that could not be completed. The checks answer `Ok(false)` /
/// a failing list for a provable "no" and this for everything else, so the
/// caller turns it into [`ValidationResponse::Unavailable`], never a rejection.
#[derive(Debug, thiserror::Error)]
pub enum ValidatorError {
    #[error("failed to parse entity: {0}")]
    EntityParse(String),

    #[error("blockchain query failed: {0}")]
    BlockchainQuery(String),

    #[error("subgraph query failed: {0}")]
    SubgraphQuery(String),

    #[error("{0}")]
    Land(#[from] crate::land_authz::Unavailable),

    /// The source this check needs is not configured on this node.
    #[error("{0}")]
    Unconfigured(String),
}

#[derive(Debug, Clone)]
pub struct PermissionResult {
    pub result: bool,
    pub failing: Option<Vec<String>>,
}

impl PermissionResult {
    pub fn ok() -> Self {
        PermissionResult {
            result: true,
            failing: None,
        }
    }

    pub fn denied(failing: Vec<String>) -> Self {
        PermissionResult {
            result: false,
            failing: if failing.is_empty() {
                None
            } else {
                Some(failing)
            },
        }
    }

    /// Denied iff anything fails.
    pub fn from_failing(failing: Vec<String>) -> Self {
        if failing.is_empty() {
            PermissionResult::ok()
        } else {
            PermissionResult::denied(failing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_keeps_its_kind_and_text() {
        assert_eq!(ValidationResponse::Ok.into_result(), Ok(()));
        assert_eq!(
            ValidationResponse::failed(["a".to_string(), "b".to_string()]).into_result(),
            Err(vec![
                ValidationError::Rejected("a".into()),
                ValidationError::Rejected("b".into())
            ])
        );
        let unavailable = ValidationResponse::unavailable("store")
            .into_result()
            .unwrap_err();
        assert!(unavailable[0].is_unavailable());
        assert_eq!(unavailable[0].to_string(), "store");
    }

    #[test]
    fn permission_denies_only_with_failing_entries() {
        assert!(PermissionResult::from_failing(vec![]).result);
        let denied = PermissionResult::from_failing(vec!["x".into()]);
        assert!(!denied.result);
        assert_eq!(denied.failing.as_deref(), Some(&["x".to_string()][..]));
    }
}

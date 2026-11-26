//! Places for uploads in flight: a node-wide count, and of it a count per
//! wallet and per source address, taken before an upload's body is read and
//! given back when its [`Admission`] drops. A refused place answers 429 with
//! `Retry-After` at once ([`busy`]); nothing waits for a place.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

use crate::http::ApiError;

/// The `Retry-After` of a refused place, in seconds.
pub const RETRY_AFTER_SECS: u32 = 30;

/// The places of one kind of upload.
pub struct Places {
    what: &'static str,
    total: usize,
    per_signer: usize,
    per_source: usize,
    in_flight: Arc<Mutex<InFlight>>,
}

#[derive(Default)]
struct InFlight {
    total: usize,
    by_key: HashMap<String, usize>,
}

/// One admitted upload's place, given back when dropped.
pub struct Admission {
    in_flight: Arc<Mutex<InFlight>>,
    keys: Vec<String>,
}

impl Drop for Admission {
    fn drop(&mut self) {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        in_flight.total -= 1;
        for key in &self.keys {
            if let Some(count) = in_flight.by_key.get_mut(key) {
                *count -= 1;
                if *count == 0 {
                    in_flight.by_key.remove(key);
                }
            }
        }
    }
}

/// A refused place: 429 at once, with a `Retry-After`.
pub fn busy(error: &str) -> Response {
    retry_later(ApiError::too_many(error))
}

/// `refusal` with the `Retry-After` of a refused place.
pub fn retry_later(refusal: ApiError) -> Response {
    let mut response = refusal.into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from(RETRY_AFTER_SECS));
    response
}

impl Places {
    /// `total` places for uploads of `what` (its refusals name it), at most
    /// `per_signer` of one wallet and `per_source` of one source address.
    pub fn new(what: &'static str, total: usize, per_signer: usize, per_source: usize) -> Self {
        Self {
            what,
            total,
            per_signer,
            per_source,
            in_flight: Arc::default(),
        }
    }

    /// A place for `signer`'s upload from `source` (`limits::source`: `None`
    /// when the request names no single client, which counts toward no
    /// source), or the refusal ([`busy`]).
    pub fn admit(&self, signer: &str, source: Option<&str>) -> Result<Admission, Response> {
        self.admit_beyond(signer, source, 0)
            .map_err(|(_, busy)| busy)
    }

    /// [`Self::admit`] with `extra` places beyond the node's; a refusal says
    /// whether the node's places were what refused it.
    pub(crate) fn admit_beyond(
        &self,
        signer: &str,
        source: Option<&str>,
        extra: usize,
    ) -> Result<Admission, (bool, Response)> {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
        let wallet = format!("wallet:{signer}");
        let count = |key: &str| in_flight.by_key.get(key).copied().unwrap_or(0);
        if count(&wallet) >= self.per_signer {
            return Err((
                false,
                busy(&format!(
                    "Too many {} of this wallet in progress",
                    self.what
                )),
            ));
        }
        let mut keys = vec![wallet];
        if let Some(source) = source {
            let source = format!("source:{source}");
            if count(&source) >= self.per_source {
                return Err((
                    false,
                    busy(&format!(
                        "Too many {} from this address in progress",
                        self.what
                    )),
                ));
            }
            keys.push(source);
        }
        if in_flight.total >= self.total + extra {
            return Err((
                true,
                busy(&format!(
                    "Too many {} in progress, try again later",
                    self.what
                )),
            ));
        }
        for key in &keys {
            *in_flight.by_key.entry(key.clone()).or_default() += 1;
        }
        in_flight.total += 1;
        Ok(Admission {
            in_flight: self.in_flight.clone(),
            keys,
        })
    }

    /// Uploads holding a place.
    pub fn in_flight(&self) -> usize {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .total
    }
}

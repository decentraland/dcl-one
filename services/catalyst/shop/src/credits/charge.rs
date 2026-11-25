// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/adapters/db/db.ts (getAuthorizedChargeBySalt), src/controllers/handlers/get-authorized-charge.ts
//! What a signed credit debits its buyer, by salt
//! (`GET /credits/authorize/{salt}`). A signed `useCredits` payload proves
//! what is bought and the MANA cap, not what the purchase takes out of the
//! Credits balance: that is the authorization's charged total, debited when
//! the salt is consumed on chain. The salt is the one value in both, so a
//! client about to ask a person to approve a purchase (the auth site's
//! credits approval) reads the charge by it and states a price only when the
//! two agree.

use sqlx::Row;

use super::errors::ApiError;
use super::store::Credits;

/// What one credit charges: the authorization's charged total
/// (`usd_cents`), how many purchase lines settle together under it (a group
/// is settled as one, so a caller describing one item must not quote the
/// group's total), and where its lifecycle stands, in credits-server's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedCharge {
    pub usd_cents: i64,
    pub lines: i64,
    pub status: &'static str,
}

/// credits-server's three states, read from what the credit can still do on
/// this node: `pending` while it can still be spent (`authorized`, and
/// `retired`: a buyer's cancel keeps a signed credit spendable until it
/// expires, so its payload still debits `usd_cents` if submitted, as the
/// history reports it), `settled` once consumed on chain, `expired` once it
/// can no longer be spent (`released`, `expired`). credits-server answered
/// `expired` after a cancel because its cancel made the intent unspendable.
fn charge_status(status: &str) -> &'static str {
    match status {
        "authorized" | "retired" => "pending",
        "consumed" => "settled",
        _ => "expired",
    }
}

impl Credits {
    /// The charge of `address`'s authorization under `salt` (a lowercase
    /// bytes32); `None` when this address holds none, another wallet's
    /// included, so the lookup never reveals someone else's ledger. A
    /// single-line authorization keeps its line on the authorization row
    /// itself, so it counts as one line.
    pub async fn authorized_charge(
        &self,
        address: &str,
        salt: &str,
    ) -> Result<Option<AuthorizedCharge>, ApiError> {
        let row = sqlx::query(
            "SELECT a.usd_cents, a.status, \
                    GREATEST((SELECT count(*) FROM credits.authorization_lines l \
                              WHERE l.authorization_id = a.id), 1) AS lines \
             FROM credits.authorizations a \
             WHERE a.id = $1 AND a.address = $2",
        )
        .bind(salt)
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| AuthorizedCharge {
            usd_cents: r.get("usd_cents"),
            lines: r.get("lines"),
            status: charge_status(&r.get::<String, _>("status")),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lifecycle_reads_as_credits_server_names_it() {
        assert_eq!(charge_status("authorized"), "pending");
        assert_eq!(
            charge_status("retired"),
            "pending",
            "a cancelled credit still debits until it expires"
        );
        assert_eq!(charge_status("consumed"), "settled");
        for gone in ["released", "expired"] {
            assert_eq!(charge_status(gone), "expired", "{gone}");
        }
    }
}

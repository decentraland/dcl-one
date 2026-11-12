# signatures-server → signatures

Source: signatures-server `049445ee` (`src/migrations`, public schema), the
LAND/Estate rentals listings.

| Step | Upstream | Target |
| --- | --- | --- |
| `rental_assets` | `metadata`, keyed by the asset of its rentals | `signatures.rental_assets` |
| `rentals` | `rentals` joined 1:1 with `rentals_listings` | `signatures.rentals`, `signatures.rental_listings` |
| `rental_periods` | `periods` | `signatures.rental_periods` |

## Rules

- Ids are kept. `token_id` text is cast to numeric. Contract, rentals-contract,
  target, lessor and tenant addresses are lowercased, as the node stores them
  (CHECKs, the fold's `rental_contract_address = $venue`). The listing
  `signature` is copied byte for byte, since the fold compares
  `lower(signature)`.
- `rental_assets`: `nft_id` comes from `metadata.id` (lowercased), network and
  chain from the newest rental. `fetched_at` is the newest rental's create time,
  which is when upstream last read the NFT. A newer `fetched_at` in the target
  wins.
- `status`, `started_at`, `rented_days` and `tenant` are upstream's
  rentals-subgraph sync. They seed the node, and the node's `rentals` chain
  index (`shop.rental_event`) refolds them once it is published. Rows the sync
  created (epoch `expiration`, zero nonces) are imported as `executed`. A row
  whose target `updated_at` is newer (a node fold) is not overwritten,
  including its listing tenant/lessor. Periods load only when the parent
  signature and update timestamp match the extracted parent version.
- Skipped: a signature that the node holds under another id, and a second
  `open` listing for an asset (the node allows one open listing per asset).
  Periods of skipped listings are skipped too.
- `timestamp` columns hold UTC.

## Not migrated

- `rentals_offers`: the server never writes it.
- `updates`: the subgraph sync cursor; the node keeps its own chain cursor.
- `pgmigrations`.

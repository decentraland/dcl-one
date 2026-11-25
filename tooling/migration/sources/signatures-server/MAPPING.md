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
  wins. Imported metadata has no published metadata or LAND membership block.
  When the upstream row wins, both versions return to zero until the next
  indexed fill.
- `status`, `started_at`, `rented_days` and `tenant` are upstream's
  rentals-subgraph sync. They seed the node, and the node's `rentals` chain
  index (`shop.rental_event`) refolds them once it is published. Rows the sync
  created (epoch `expiration`, zero nonces) are imported as `executed`. A row
  whose target `updated_at` is newer (a node fold) is not overwritten,
  including its listing tenant/lessor, with the one exception below. Periods
  load only when the parent signature and update timestamp match the
  extracted parent version.
- `cancel_reason` is not upstream's: upstream records no cause, so an imported
  cancellation lands with NULL (an unknown cause, which a rebuild of the
  `rentals` index never reopens). An update over a node row keeps the node's
  reason while both say `cancelled` and clears it on any other status.
- A rent of the listing wins over a cancellation. An upstream `executed` or
  `claimed` replaces a target row that is `cancelled`, whatever the two
  `updated_at` values and whatever the reason, and clears the reason. This
  covers the node's `rental-listing-status` job, which runs whatever
  `DATA_WRITES_ENABLED` says, as the `rentals` fold does, and stamps
  `updated_at` with the time it cancels a listing (`expired`, `transferred`,
  `dissolved`): a rent that upstream recorded before that time still lands.
  The row then carries the upstream `updated_at`, which can be older than the
  one it replaces. A later upstream version (the `claimed` after an
  `executed`) and the listing's periods therefore compare against the
  upstream time, and an upstream version older than the one applied stays
  skipped.
- No other older upstream status replaces a newer target row. An older
  upstream `open` or `cancelled` leaves a node cancellation and its reason
  as they are, and an older upstream `open` or `cancelled` leaves a target
  row that the node's `rentals` fold moved to `executed` or `claimed` as it
  is.
- Skipped: a signature that the node holds under another id, and a second
  `open` listing for an asset (the node allows one open listing per asset;
  of two in one batch, the latest `updated_at` lands).
  Periods of skipped listings are skipped too. Listings leaving `open` are
  closed before any insert, so a batch carrying both a cancelled listing and
  its open replacement lands the replacement.
- `timestamp` columns hold UTC.

## Not migrated

- `rentals_offers`: the server never writes it.
- `updates`: the subgraph sync cursor; the node keeps its own chain cursor.
- `pgmigrations`.

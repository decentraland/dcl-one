# Creators pages

Collections, the item editor, curation and the Blender live preview, served under `/create`
([ADR](../../../../../docs/adr/2026-11-19-creators-collections-in-sites.md)). This file lists the product rules
the code cannot explain on its own. Read the section for the flow you are changing first: a behavior
listed here is a decision, not a bug. When a change makes or reverses one, rewrite its entry in the same pull request.

## Layout

| What | Where |
| --- | --- |
| Routes (one per page, a `pageLoader` for the telemetry session) | `src/routes/routes/create.collections.*.tsx`, `create.curation.tsx`, `create.live-preview.tsx` |
| Shell: site chrome, the Create tabs, full-screen pages | `CreatorsShell.tsx`, `components/SubNav` |
| Components, query hooks, client stores | `components/`, `hooks/`, `store/` |
| builder-api client, chain, validation, encoding, flows | `src/data/lib/creators/` |
| Environment values per host (`.zone`, `.today`, `.org`) | `src/data/lib/creators/config/env/` |
| builder-api base: the page's node (`/builder-api/v1`), or the environment's on a Decentraland host | `src/data/lib/creators/builder/transport.ts` |
| Copy (English) | `src/data/lib/creators/intl/en.json`, through `useTranslation().t` |
| Browser suite | `tooling/creators-e2e` |

Unit tests run with the Sites suite (`pnpm exec vitest run src/features/creators src/data/lib/creators`). The browser
suite drives these pages with every service faked; see its README.

## Cross-cutting

- **builder-api is used as it is.** Paginated lists need both `page` and `limit`, or `/collections` and
  `/collections/:id/items` answer a bare array; numeric columns such as `item_count` arrive as strings.
- **A draft's files are read signed.** Storage serves an unpublished item's files only to a signed reader, so every
  byte read goes through the session's signed fetch, never into the HTTP cache, and a draft's thumbnails show as
  object URLs of those reads (`hooks/useItemFileSrc.ts`, at most four at a time, revoked once off screen); the
  editor's preview takes the selected draft's model files from memory (`hooks/useDraftPreviewFiles.ts`). A published
  item's files keep their plain storage URLs.
- **Subgraph lag is patched in the query cache.** builder-api reads minters, managers, `total_supply` and ownership
  from the collections subgraph, minutes behind a mined transaction. After an on-chain write the page patches the
  cached collection (and remembers roles and "sales enabled" for the session) instead of refetching.
- **Transactions go through `data/lib/creators/auth`.** A browser wallet on the contract's chain sends them itself;
  otherwise they go through the relayer as a gasless meta-transaction signed by the session's account. A wallet on
  another account is refused before anything is signed. A dismissed wallet prompt returns to the form, and a wallet
  on the wrong account or network gets the shared wallet message with a retry; neither is reported as an error.
- **Raw errors never reach the user.** Failures show friendly copy and are logged with a `flow`, which the site
  reports to its client-error endpoint; a retry is offered only when it can succeed. Curators are the exception: the
  approval flow shows them the raw error with a Copy button for support.
- **Phones are viewers.** At 900px and below the collection management actions are hidden; at 768px and below the
  editor is preview-only. Curation actions are desktop-only.
- **Telemetry** uses `bd_<area>_<action>` events, typed in the Sites catalogue, one story per page.
- **Feature flags** are the `builder` and `dapps` flags the rest of Sites reads, through `/internal/feature-flags`:
  decentraland.org's unless the server sets `FEATURE_FLAGS_URL` and `FEATURE_FLAGS_ORIGIN`. A local dev server
  overrides one with `VITE_FEATURE_FLAG_OVERRIDES=blender-live-preview:true`; a flag that cannot be read is off.

## Collections list and status model

- **One status rule set**, mirroring builder-api's `Collection.getStatusStatement`, so a pill always matches the chip
  it is listed under. First match wins: linked → Linked; not on chain → Draft (a locked draft with a publish in
  flight reads Publishing); latest review request `rejected` → Rejected, over approval and disable alike; approved on
  chain → Under review if the latest request is `pending`, else Published; not approved on chain and (latest request
  `approved`, or no request and approved before) → Paused (curators see "Disabled"); anything else → Under review.
- **`reviewedAt` alone never proves an approval**: the rescue step of a first approval stamps it too.
  `hasBeenApproved` is "approved now, or `reviewedAt` differs from `createdAt`".
- **Rejected pushed changes on an approved collection read Rejected**, while buyers keep the approved version.
- **Linked collections** sit behind their own chip, always shown, and carry no status.
- **Role badge.** A collection listed because the signer is an on-chain manager or minter carries a Collaborator or
  Sender pill.

## Collection detail

- **Only the item editor is public.** The detail page is for the collection's roles (owner, collaborators, senders;
  a linked collection's third-party managers); anyone else, curators included, gets the not-found page. Any signed-in
  address can open a published collection read-only in the item editor; the back arrow leads to the detail page for
  a role, to Curation for a curator, and is hidden for anyone else.
- **Rename only while unpublished**: renaming regenerates the `initialize` calldata over every item.
- **Preview in World** opens the desktop explorer through a `decentraland://` link into the per-environment preview
  world; if the tab keeps focus for about 750ms, the download page opens.
- **Items are filtered and paged in the browser**: builder-api cannot filter items by type.
- **Item status column** shows only on collections approved on chain and only while a row says more than Published.
  Tags and a wearable's metrics are ignored when comparing an item with its Catalyst entity (the Shop reads tags from
  builder-api); only the approval's deploy plan compares them.

## Create collection and add items

- **The create request carries** a client-generated UUID, a zero-address placeholder URN and the ABI-encoded
  `ERC721CollectionV2.initialize` calldata, from which builder-api derives the CREATE2 salt and the contract address.
  `symbol` is `DCL-` plus the name without lowercase vowels and whitespace, uppercased.
- **Names:** non-empty, at most 32 characters, no `:` (names feed URNs). Uniqueness is global and server-side.
- **One multi-item modal.** Nothing reaches the server until the batch is finished; failures reopen the modal with
  only the items that failed.
- **Model findings are warnings at import.** Blocking happens at publish, behind `block-publish-on-validation-errors`.
- **Defaults:** rarity Epic; price and beneficiary "not for sale". `removesDefaultHiding: [hands]` is written for
  upper-body items and recomputed on save.
- **Thumbnails are 1024×1024.** Categories with a pose (`upper_body`, `skin`) render offscreen with three.js, falling
  back to the preview screenshot. A thumbnail from the zip or picked by the creator is never regenerated.
- **One hidden preview iframe per modal measures each file in turn.** It sits behind the dialog at opacity 0.01,
  never off-screen or at opacity 0, where Chrome throttles a cross-origin iframe (Unity took about ten times longer).
  A run gets 45s to see its model load on a fresh iframe (20s after) and 75s to read it; on overrun the iframe is
  remounted, and a second overrun fails that file and moves on.
- **Zip budgets:** at most 500 entries, about 259MB inflated in total, and per-type caps (wearable 3MB, skin 9MB,
  emote 3MB, thumbnail 1MB). One wrapping folder is stripped, never `male/` or `female/`.
- **Social emotes (an `Armature_Other` armature) are rejected at import**: they are not supported on the platform yet.
- **`replaces` is folded into `hides`** on load, and every representation's `overrideHides` / `overrideReplaces` mirror
  `hides` / `replaces`.

## Smart wearables

- **A zip is smart when it carries `scene.json`** at the root, inside one project folder, or under both `male/` and
  `female/`, so a downloaded item imports back. `wearable.json` is optional.
- **One code per item**: two `scene.main` copies that differ are rejected.
- **The preview video is required.** A single `.mp4` in the zip pre-fills it; more than one fails the file. The video
  (up to 250MB) is outside the 3MB model cap. It uploads to `POST /items/:id/videos`; once the item is approved, a new
  video only replaces `contents['video.mp4']`, so the item reads as changed and goes through review.
- **Linked collections take no smart zip.**

## Item editor

- **The URL holds the selection** (`?collection=&item=&reviewing=true&unity=false`). A missing or unknown `item`
  selects the first wearable, else the first item. `reviewing=true` applies only to committee members.
- **The preview iframe never reloads for a change**: `AvatarPreview` mounts it once and sends every later change,
  items included, as UPDATE messages, re-sent after each boot.
- **Renderer:** `?unity=false` → Babylon; `unity-wearable-preview` flag off → Babylon; data saver, a measured link under
  10 Mbps, under 4GB of memory or a high-DPR low-core device → Babylon; else Unity, phones included. The link is
  measured by timing the second half of an incompressible 512KiB download.
- **Review mode is read-only and complete**: the properties panel is always shown, disabled, so a curator reads every
  item's metadata, not only its model.
- **Name and description:** at most 32 and 64 characters, no `:` (both are fields of the on-chain metadata). Rarity is
  locked once published.
- **The campaign tag hint** comes from the CMS behind the `builder-campaign` flag; no campaign or a CMS failure shows nothing.
- **Wearables saved without geometry are repaired on open.** Their metrics are counted from the model's glTF JSON
  (colliders left out). They are written back on open only for an unpublished, unlocked collection with no unsaved
  edits (in a linked collection, an item never sent for review), because metrics are part of the Catalyst entity
  hash. On a published standard collection they go with the owner's next save and reach the Catalyst with the next
  approval; a linked item already sent for review keeps its stored metrics until its model is replaced.
- **Download folds identical body shapes into the zip root** next to `thumbnail.png`, `image.png` and `video.mp4`,
  unless a file would collide once imported back. An emote ships one model for both body shapes.
- **Replace model and add a body shape** save on confirm. Replacing one shape of a unisex item keeps the other; a
  wearable↔emote switch is refused. Spring-bone params survive a model change per body shape, by bone name.
- **Spring bones** are keyed by model hash under `item.data.springBones.models[hash]` and pushed to the renderer on every
  load and emote play.

## Item validation

- **The wearable-validator rule book is the only validator**, and its English messages are shown as they are, so the
  editor says what curation says. Every distinct model of an item is checked, findings tagged by body shape.
- **A check that cannot run is a warning**, never a block. The rule book runs in a module worker; a run over 60s is
  taken as hung and the worker is replaced.
- **Results for saved items are cached in IndexedDB**, keyed by content hashes plus type, category, hides, loop and body
  shapes, and cleared when the validator version changes. Failed runs are never stored.
- **Validation runs on desktop only** (above 900px on the collection page, above 768px in the editor).
- **The publish gate is `block-publish-on-validation-errors`**: on, errors disable publishing; off, the creator sees
  them and may continue. Warnings never block.
- **Item size is counted like the Catalyst does** (each distinct hash once, thumbnail and rarity image included, the
  video left out; 3MB for wearables and emotes, 9MB for skins) and an oversized item always blocks. A size that could
  not be measured is a warning. A server-side size rejection on save still shows the generic error.
- **Curators get a collection validation badge** while the collection is To review or Under review; it never blocks.

## Publish collection

- **Three steps**, the contact email on the first. The collection name and the items' name and rarity are editable in place.
- **At most 50 items per publish**, the size the curation tooling handles.
- **The email is recorded through `POST /collections/:id/tos` before any payment** (three retries, none on a 4xx): it is
  the legal record of the acceptance.
- **Fees come from `GET /rarities`.** Credits are USD cents / 10 rounded up. The MANA total is checked on chain against
  `RaritiesWithOracle.getRarityByName` with a 5% tolerance before the wallet is asked; a mismatch refreshes the quote.
- **MANA approvals are exact**, never unlimited. Credits pay through `CreditsManager.useCredits` with
  `maxUncreditedValue = 0`.
- **`Qm…` content hashes are re-hashed and saved again at publish**: the Catalyst cannot deploy them.
- **A locked, unpublished collection can be published again.** A reverted transaction never unlocks it, so the creator
  retries instead of waiting a day. The retry is read-only and safe: the address is CREATE2-derived from the frozen
  salt, so once one `createCollection` lands any other reverts, and a retry reverted that way counts as published.
- **The sync after publishing is recoverable.** The page waits for the receipt and calls `POST /collections/:id/publish`,
  retrying on 401 for up to an hour while the subgraph catches up; the detail page runs it again whenever the
  collection reads published with an item still lacking a token id.
- **Publishing opens the first review request** once the publication is synced.

## Sell, edit price, remove from sale

- **Enabling sales is the first step of the first sale**: it adds `OffChainMarketplaceV2` to the collection's minters.
- **Only the owner sells**: the marketplace executes a primary order only when the creator signed it. Selling opens
  once the collection `hasBeenApproved`.
- **The 1 MANA minimum blocks**: below it the buyer would pay the meta-transaction gas. The ceiling is 1e12.
- **Credits are the default currency** (1 credit = $0.10), signed as a USD-pegged MANA order. The USD estimate comes from
  the Chainlink aggregator the marketplace settles with, cached 5 minutes, hidden when stale.
- **A giveaway** is MANA, amount 0, zero-address beneficiary. The Shop hides zero-price listings.
- **No expiration means 2100-01-01**; a chosen date ends at the end of that local day.
- **Remove from sale resolves the order's contract through the local registry**, never trusting the address
  marketplace-api returns. A CollectionStore price with no order is cleared with `editItemsData`.
- **Edit price is cancel, then sign again** with the old beneficiary and expiration. Only the POST is retried (up to
  eight attempts) while marketplace-api still indexes the cancelled order, never the signature.

## Send items

- **"Mint" is never said**: the flow is Send items, and recipients are grouped into transfers that each get one bundle.
- **Who can send:** the collection `hasBeenApproved` and the signer is the owner or an on-chain minter.
- **At most 100 copies per transaction**: Polygon caps a transaction at 2^25 gas and `issueTokens` costs about 157k gas
  per copy to a new holder (measured on Amoy, 2026-10).
- **Pasting a list adds every address in it**, validated, deduplicated and cut at the room left.

## Collection roles

- **Minters are "Senders" and managers "Collaborators"** in every creator-facing text.
- **The Shop's minter contracts are hidden from the Senders list** and kept on save: removing one would take the
  collection off sale.
- **The owner may list themselves**, so the role survives a transfer of ownership. A save is one transaction with the diff.

## Transfer ownership

- **Two-step confirmation with the collection name typed**: the transfer is irreversible. The owner's own address, the
  zero address, the Shop's contracts and the collection's contract are refused.
- **Reads never write.** builder-api checks permissions against its stored owner, which the chain never updates; the page
  polls `POST /collections/:id/owner/sync` after a transfer, and any later write to the published collection syncs it too.
- **The receipt wait gives up after five minutes** and the sync poll after ten; the transfer is done either way.

## Curation

- **No feature flag**: the committee check (`GET /committee`, failing closed) gates the tab, `/create/curation` and
  `?reviewing=true`.
- **A pending request is To review whether or not a curator has it**; the assignee cell says who. The list opens on
  To review and "Unassigned + me".
- **Rejecting a never-reviewed collection creates the request first**, then marks it rejected.
- **After approving on chain the pending request is marked approved too**, so a curator approves once.
- **The approval is one stepper with only the steps a run needs:** prepare (token ids), rescue (on a first approval
  every item, afterwards only items whose hash differs on chain, in chunks of 50), upload (entities signed with the
  curator's identity), enable (waits for builder-api to read `is_approved`, up to 2 minutes). Every step is idempotent.
- **Catalyst entity metadata key order is part of the content hash** (`data/lib/creators/catalystEntity.ts`). Do not reorder keys.
- **A blank subgraph answer means nothing on chain yet**: the plan is built from the reviewed items.
- **Deploying missing entities skips edited, unsynced items**: their new hash is not on chain, and deploying them would
  ship unreviewed changes.
- **Enable only flips `setApproved(true)`**, leaving the review request alone.
- **Creators can request review again after a rejection.** It runs the same validation step as publishing.

## Forum post

- **The forum topic is the interim feedback channel** until review threads ship. builder-api builds it
  (`POST /collections/:id/post`, no body) and answers 409 with the existing link, which is reused.
- **Creation runs after the publish sync and is recoverable**: a role opening a synced, unapproved collection without a
  link creates it once per tab, 1.5 hours to 30 days after publishing, under a per-collection Web Lock. Three retries.
- **Standard collections only**: builder-api creates third-party topics itself.

## Linked collections

- **Editing is for the third party's managers**, never the collection roles. A published linked collection opens
  read-only in the item editor to any signer; item curations and slots stay manager-only.
- **A zip whose `wearable.json` has no `mapping` gets All NFTs**, shown before upload.
- **The contract check probes the ERC-721 and ERC-1155 interface ids** through ERC-165.
- **Review threshold:** every item under 50, else max(50, 1%) capped at 300; Approve waits until it is reached and goes
  on chain only when the Merkle root changed.
- **Not supported:** Ethereum collections, emotes and smart wearables in linked collections.

## Blender live preview

- **The bridge URL is trusted only on local origins** (`localhost`, `127.0.0.1`, `[::1]`), else `http://localhost:8080`.
  The page is behind `blender-live-preview` and reached from the add-on's link.
- **Long poll with a fallback** for a bridge that ignores `since`; identical bytes are not redrawn.
- **Every push gets a new definition id**: both renderers cache by id.
- **Signed-out creators can preview**; signing in is asked only when adding to a collection.

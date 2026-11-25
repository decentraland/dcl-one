# Builder: what a node needs to serve it

The builder routes (`/v1/...` at the root and `/builder-api/v1/...`,
`services/catalyst/builder`) answer the wearable and emote builder of
`/create`, the Marketplace's reads and the wearable validator. This page
lists what a node needs before it serves them. Each section names the
setting and what a request gets while it is missing.

## Database

Run `catalyst schema migrate` before the binary that serves the builder
starts. Then run the builder's release scripts, each after its step and a
restore point, with `psql -X -f`, in this order:

1. `schemas/releases/builder-scenes-retired.sql`
2. `schemas/releases/builder-collection-flags-retired.sql` (creator writes
   off while old and new binaries overlap)
3. `schemas/releases/builder-item-documents-and-uploaders.sql` (until it
   runs, item saves and uploads answer 503)
4. `schemas/releases/builder-item-curation-retired.sql`
5. `schemas/releases/builder-linked-forum-retired.sql`
6. `schemas/releases/builder-linked-publications-retired.sql`

`DATABASE.md`, the "Retired builder" sections, gives each script's rollout.

## Network

The builder works on the Polygon chain the node's `ETH_NETWORK` pairs, the
role builder-server's `CHAIN_NAME` plays: `mainnet` (the default) is Polygon
(chain 137, URN network `matic`), and `sepolia` is Amoy (chain 80002, URN
network `amoy`). The chain decides the collection manager, factory and
forwarder of a publication, the factory's proxy code hash a collection's
address is computed with, the base URI of a publication the node prepares,
the rarities oracle, the third-party registry that slot cheques are signed
under, and the network of the collection and item URNs the builder answers
and stores. Startup refuses any other `ETH_NETWORK`.

A collection save's contract address is computed from the `initialize` data
the client sends, its base URI included. `/create` names its environment's
base URI on a Decentraland host, and elsewhere the token metadata route of
the node serving the page, `/lambdas/collections/standard/erc721/`, which
the node's content server answers. It saves a collection again before each
publication, so the address matches what it deploys; while an earlier
publication holds the collection's lock, it refuses a publication whose
base URI would deploy elsewhere.

The node keeps a save only when that base URI is one of two: its chain's
default (`https://peer.decentraland.org/lambdas/collections/standard/erc721/`
on mainnet, `https://peer.decentraland.zone/lambdas/collections/standard/erc721/`
on sepolia), or the node's own address followed by
`/lambdas/collections/standard/erc721/`. Any other base answers 400, "The
collection's token metadata base is neither this node's nor the network's",
because the collection's tokens would read their metadata from a host
neither the node nor the network answers for. The node's own address is
`PUBLIC_URL` without its trailing slash, so a node that serves `/create`
off Decentraland's hosts must set `PUBLIC_URL` to the origin browsers reach
it at. Without `PUBLIC_URL` the node's address is `http://<bind address>`,
which matches no page origin but a local one.

Every chain source the builder reads must serve that chain:

- `BUILDER_POLYGON_RPC_URL`: the receipt check, the deployment check and the
  rarities read ask its `eth_chainId` and answer 503 when it names another
  chain.
- The Polygon collections subgraph and, on Amoy, the third-party registry
  subgraph: the node can't check which chain a subgraph indexes, so the
  rarities, the curation committee and Amoy's third-party registry follow
  whatever chain those subgraphs index.
- On Polygon the third-party registry is the node's own registry index,
  which is bound to chain 137.

The builder reads those two subgraphs from Decentraland's subgraph worker
for its chain unless a setting names another. On Polygon it reads the
collections subgraph only:

| Subgraph | Setting that overrides it | Mainnet (Polygon) | Sepolia (Amoy) |
| --- | --- | --- | --- |
| Collections (rarities, committee) | `MATIC_COLLECTIONS_SUBGRAPH_URL` | `https://subgraph.decentraland.org/collections-matic-mainnet` | `https://subgraph.decentraland.org/collections-matic-amoy` |
| Third-party registry (providers, slot cheque receipts) | `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL` | not read: the registry index answers | `https://subgraph.decentraland.org/tpr-matic-amoy` |

A setting set to a URL wins; a setting set empty reads no subgraph, which
is how the `island` and `journey` postures keep the builder off
Decentraland's hosts. `NODE_POSTURE=mirror` sets both settings to the
mainnet subgraphs, so a mirror on `sepolia` must set them to Amoy's. The
shop's collections mirror reads `MATIC_COLLECTIONS_SUBGRAPH_URL` only and
gets no default.

Never change `ETH_NETWORK` on a database that holds builder rows. Saved
collections keep the contract address computed for the old chain's factory,
and published items keep their URNs.

## Chain state and the committee

The builder derives a collection's published and approved state and its
roles from the shop's Polygon collections copy on the same node, and reads
the curation committee from the Polygon collections subgraph.

- `MARKETPLACE_MIRRORS_ENABLED` with `MATIC_COLLECTIONS_SUBGRAPH_URL`: the
  collections mirror. Until its first complete pass, listings and bodies
  answer 503; once a pass is more than 30 minutes old, saves and deletes
  answer 503 and a publish 401.
- The collections subgraph (see "Network"), read for the committee: the
  accounts it marks `isCommitteeMember`, in pages of 1,000. A write's decision takes a
  read that started after the decision began; a read-only decision also
  takes one that started up to 5 seconds before it. While the subgraph is
  set empty, unreadable, or answers a malformed account or more than 1,000
  members, every decision that needs the committee answers 503. The builder
  doesn't need the shop's committee index
  (`MARKETPLACE_COMMITTEE_INDEX_ENABLED`).
- `BUILDER_POLYGON_RPC_URL`: an `https` JSON-RPC of the builder's Polygon
  chain on a public host, with the `finalized` block tag. It verifies publication receipts, reads the
  rarities' prices and checks a collection's contract before a save that
  changes its address, a delete and a publish. Unset, the publication and
  rarities routes answer 503 and the contract check is skipped.
- The third-party registry. On Polygon it's the shop's registry index:
  `MARKETPLACE_THIRD_PARTY_REGISTRY_INDEX_ENABLED` on, with a Polygon
  JSON-RPC for its walk, caught up and publishing. While the index is off,
  catching up, quarantined, diverged, or its publication is older than 30
  minutes, every request about a third party answers 503. On Amoy it's the
  registry subgraph (see "Network"): set empty or unreadable, every request
  about a third party answers 503.
- The content sync's item history coverage: a new third-party item URN is
  taken only where the content history is covered or this node is its own
  content authority; elsewhere the URN check answers 503.

## Ingress

- `HTTP_TRUSTED_PROXY_CIDRS` lists the gateway's networks. Without it, a
  signed request on a path the gateway rewrote (`builder-api.<domain>`) is
  refused, and the builder can't tell clients apart. Behind a gateway on a
  private address it applies no per-source bound, and only the gateway's
  per-client limit holds; behind one on a public address every client is
  one source and shares one address's budgets and upload places.
- `CORS_ORIGIN` lists the origins that may read the answers (`;`-separated,
  `regex:` entries), `CORS_METHOD` the methods a preflight allows. Unset,
  `CORS_ORIGIN` is the Decentraland front-ends and `http://localhost:<port>`.
  The gateway adds no `Access-Control-Allow-Origin` of its own under
  `/builder-api/`.
- The node's clock is synchronized: a signature is accepted at most 60
  seconds old or ahead.

## Writes, files and flags

- `CREATE_WRITES_ENABLED=true` opens the writes; off, each answers 503, and
  the builder's jobs (the terms forward to the warehouse and the
  validation-request sweep) change no row and call no outside service.
- `STORAGE_ROOT_FOLDER/item-files` holds item files and videos, apart from
  the content tree, so no content or worlds route serves them. The next
  section bounds what each wallet, source address and the whole node may
  store and send there.
- The `builder` flags document, which Explore's flags mirror stores, turns
  the automated review on (`builder-auto-curation`) and the review flow
  (`builder-auto-curation-review-flow`); both also need
  `WEARABLE_VALIDATOR_CALLBACK_SECRET`.

## Uploads, drafts and request budgets

The builder bounds what one wallet, one source address and the whole node
store and send, so no caller fills the node's disk, database or memory. The
values are in the [bounds table](#bounds); this section says what each
bound counts and what a request past it gets.

- **Sources.** A source address is the client's address, an IPv6 address
  widened to its /64; an upload source widens an IPv6 address to the block
  the table names. Behind the gateway, set `HTTP_TRUSTED_PROXY_CIDRS` to
  the gateway's networks. Without it, a gateway on a private address leaves
  the node with no client address, so it applies no per-source bound; a
  gateway on a public address becomes every client's one source, so all
  clients together share one address's budgets and places.
- **Request budgets.** Every request spends from its source address's
  reads or writes a minute, and a verified signer from its wallet's, before
  the body is read; past either, 429 with `Retry-After`.
- **Unpublished uploads.** A wallet's unpublished uploads are its upload
  records whose file no published item names. Before an upload's body is
  read, its declared length, else the route's largest body, counts as
  stored; a declared length past the route's body answers 413 at once,
  before the upload takes a place: past the wallet's bound the upload answers 413 with `maxBytes`
  or `maxFiles` ("The wallet's unpublished files reached their limit"),
  past the node's 507 ("The node's unpublished files reached their limit").
  `BUILDER_UNPUBLISHED_MAX_BYTES` and `BUILDER_UNPUBLISHED_MAX_FILES` set
  the node's bound. The node counts every wallet's unpublished uploads in
  the background, one count at a time, and keeps each count for the time
  the table names, so a publication frees a wallet's uploads within it.
- **Bytes no record names.** An upload stores its bytes before its
  transaction, so one refused or interrupted after that leaves a file no
  upload record names. No bound here counts those files, and the node
  never removes them on its own. Run `catalyst storage gc` (a dry run) to
  see them and `catalyst storage gc --delete` to unlink them; it takes only
  files untouched for at least an hour that no area names. Run it on a
  schedule sized to the disk.
- **Days of uploads.** A wallet has a day of uploaded files, an upload
  source a day of files and of bytes. Each is checked before the body and
  charged with what arrived, whatever came of the upload. Once every
  wallet's unpublished uploads hold half the node's bound, in bytes or in
  files, each byte and file an upload source sends costs its day the price
  the table names. A spent day answers 429 with `Retry-After`.
- **File uploads.** A file upload takes a place among the node's file
  places, of which a wallet and an upload source hold a share; with none
  free it answers 429 with `Retry-After` at once. A body that hasn't arrived
  within its deadline answers 408 ("The upload timed out") and frees the
  place.
- **Video uploads.** A video upload takes a place among the node's video
  places, of which a wallet and an upload source hold a share. The time it
  holds the place, whether bytes arrive or not and however it ends, spends
  the wallet's and the source's day of upload time. While every place is
  held, a newcomer takes the place of the upload whose wallet or source
  spent the longest on finished uploads today, if longer than the
  newcomer's; that upload ends with 408 ("The upload gave its place to
  another while the node was busy") and `Retry-After`, and its spooled
  bytes are removed. An upload whose wallet and source finished none today
  keeps its place until it ends. Otherwise the newcomer gets 429 with
  `Retry-After`. `BUILDER_VIDEO_UPLOADS_MAX` sets the
  node's places; startup refuses a value outside the table's range.
- **Drafts.** A wallet's drafts are the items it holds that no publication
  names (a standard item until its collection is published, a third-party
  item until it has an item curation), the collections it holds
  unpublished, and the stored bytes of those items' documents. A save that
  would pass one answers 413 ("The wallet's unpublished drafts reached
  their limit") with `maxItems`, `maxCollections` or `maxBytes`. A wallet
  at its item bound still edits the drafts it holds.
- **Days of documents.** An item or collection save spends one more write
  token per the table's bytes of its body, and its size, at the least the
  table's minimum, from the wallet's and its upload source's day of
  documents; past either, 429 with `Retry-After`.

### Bounds

Each row names, by its path in the builder crate
(`services/catalyst/builder/src`), the constant that holds its value. The builder's test
`the_builder_doc_states_each_bound_the_code_enforces` fails when a row and
its constant differ, a row names no bound it checks, a budget the node
builds holds another value, or a numeric constant of the crate is neither
a row nor listed in the test as no bound of this table (a unit, an internal
step, or a bound of the review, validator or outside-service areas).

| Bound | Value | Code |
| --- | --- | --- |
| Reads of a source address a minute | 3,000 | `limits::SOURCE_READS` |
| Writes of a source address a minute | 2,400 | `limits::SOURCE_WRITES` |
| Reads of a wallet a minute | 2,400 | `limits::WALLET_READS` |
| Writes of a wallet a minute | 1,200 | `limits::WALLET_WRITES` |
| Body of an item save | 256 KiB | `ITEM_BODY_LIMIT` |
| Body of a collection save, a terms acceptance or a publication | 2 MiB | `LARGE_BODY_LIMIT` |
| Body of another route but the uploads | 64 KiB | `BODY_LIMIT` |
| A wallet's unpublished upload bytes | 5,000,000,000 bytes | `limits::WALLET_BYTES` |
| A wallet's unpublished upload files | 100,000 | `limits::WALLET_FILES` |
| The node's unpublished upload bytes, `BUILDER_UNPUBLISHED_MAX_BYTES` unset | 1,000,000,000,000 bytes | `limits::NODE_BYTES` |
| The node's unpublished upload files, `BUILDER_UNPUBLISHED_MAX_FILES` unset | 10,000,000 | `limits::NODE_FILES` |
| How long a count of the unpublished uploads is kept | 1 minute | `limits::UNPUBLISHED_KEPT` |
| Bytes an upload source sends a day | 20 GiB | `limits::SOURCE_UPLOAD_MIB` |
| Files an upload source sends a day | 100,000 | `limits::SOURCE_UPLOAD_FILES` |
| Files a wallet uploads a day | 50,000 | `limits::WALLET_UPLOAD_FILES` |
| Price of a source's bytes and files past half the node's bound | 10 | `limits::NEAR_BOUND_PRICE` |
| The IPv6 block of one upload source | /48 | `limits::upload_source` |
| Parts of a file upload | 256 | `items::upload::MAX_FILES` |
| Bytes of a file upload's files | 20 MiB | `items::upload::MAX_BYTES` |
| Body of a file upload | 21 MiB | `UPLOAD_BODY_LIMIT` |
| Time for a file upload's body to arrive | 5 minutes | `items::upload::DEADLINE` |
| Wait of a file or video upload behind the blob collector's unlink batch; past it, 503 | 5 seconds | `storage::gc::SHARED_LOCK_WAIT` |
| File uploads at once on the node | 16 | `FILE_UPLOADS` |
| File uploads at once of a wallet | 4 | `FILE_UPLOADS_PER_SIGNER` |
| File uploads at once of an upload source | 8 | `FILE_UPLOADS_PER_SOURCE` |
| `Retry-After` of a refused place | 30 seconds | `items::places::RETRY_AFTER_SECS` |
| Bytes of a video | 250,000,000 bytes | `items::video::DEFAULT_MAX_BYTES` |
| Time for a video's body to arrive | 20 minutes | `items::video::DEFAULT_DEADLINE` |
| Idle window: time without a byte before a video upload ends | 30 seconds | `items::video::DEFAULT_IDLE_TIMEOUT` |
| Video uploads at once on the node, `BUILDER_VIDEO_UPLOADS_MAX` unset | 32 | `items::video::MAX_UPLOADS` |
| The most `BUILDER_VIDEO_UPLOADS_MAX` sets, from 1 | 256 | `MAX_VIDEO_UPLOADS_SETTING` |
| Video uploads at once of a wallet | 1 | `items::video::MAX_UPLOADS_PER_SIGNER` |
| Video uploads at once of an upload source | 2 | `items::video::MAX_UPLOADS_PER_SOURCE` |
| Video upload time of a wallet a day | 4 hours | `items::video::WALLET_VIDEO_SECS_A_DAY` |
| Video upload time of an upload source a day | 12 hours | `items::video::SOURCE_VIDEO_SECS_A_DAY` |
| A wallet's draft items | 25,000 | `limits::WALLET_ITEMS` |
| A wallet's unpublished collections | 5,000 | `limits::WALLET_COLLECTIONS` |
| Stored bytes of a wallet's draft item documents | 256 MiB | `limits::WALLET_DOCUMENT_BYTES` |
| Documents a wallet saves a day | 1 GiB | `limits::WALLET_DOCUMENT_KIB` |
| Documents an upload source saves a day | 2 GiB | `limits::SOURCE_DOCUMENT_KIB` |
| The least a save costs its day | 4 KiB | `limits::DOCUMENT_MIN_KIB` |
| Body bytes one more write token pays for | 16 KiB | `limits::BYTES_PER_WRITE_TOKEN` |
| Review writes (retry, appeal, comment) of a wallet a minute | 30 | `limits::REVIEW_WRITES` |
| Review timeline reads of a wallet a minute | 120 | `limits::REVIEW_READS` |
| Newsletter subscriptions of a source address an hour | 10 | `limits::NEWSLETTER_PER_SOURCE` |
| Newsletter subscriptions of an email a day | 3 | `limits::NEWSLETTER_PER_EMAIL` |
| Newsletter subscriptions through the node an hour | 1,000 | `limits::NEWSLETTER_PER_NODE` |
| Read tokens `GET /v1/addresses` and the public collection search spend beyond their first | 50 | `limits::ADDRESSES_TOKENS` |
| Rows of a listing one read token pays for | 20 | `limits::ROWS_PER_TOKEN` |
| Item document bytes of a listing one read token pays for | 64 KiB | `limits::BYTES_PER_TOKEN` |
| Item document bytes one listing answers; past it, 413 ("The listing is too large") | 64 MiB | `limits::LISTING_BYTES` |
| Wallets or sources each budget tracks; past it, the least spent are forgotten | 50,000 | `limits::KEYS` |
| Time the count of the unpublished uploads may take before every record counts | 50 seconds | `limits::UNPUBLISHED_COUNT_TIMEOUT` |
| The IPv6 block of one source address | /64 | `limits::source` |
| The shortest admin token or validator secret startup takes | 32 bytes | `MIN_SECRET_BYTES` |
| Rows a drafts listing answers, the newest | 5,000 | `drafts::DRAFTS_CAP` |
| Rows a collection's or an address's item listing answers | 20,000 | `items::listing::ROWS_CAP` |
| Rows `GET /v1/items` answers | 1,000 | `items::listing::EVERY_ITEM_CAP` |
| Rows the committee's collection listing or the public search answers | 1,000 | `collections::listing::COMMITTEE_CAP` |
| Rows an address's collection listing answers | 5,000 | `collections::listing::ADDRESS_CAP` |
| How long before a read-only decision a third-party registry or committee read may have started and still answer it (a write's decision takes only a read that started after it began) | 5 seconds | `kept::READ_WINDOW` |
| Committee members a read of the collections subgraph accepts; past it, committee decisions answer 503 | 1,000 | `committee::MAX_MEMBERS` |
| Characters of a collection search's `q` | 256 | `collections::listing::Q_MAX` |
| Assignees a collection listing filters by | 20 | `collections::listing::MAX_ASSIGNEES` |
| Tags a collection listing filters by | 5 | `collections::listing::MAX_TAGS` |
| Characters of one of those tags | 64 | `collections::listing::TAG_MAX` |
| Rows a curation list answers | 20,000 | `curation::rows::MAX_ROWS` |
| Items one third-party publication names | 20,000 | `collections::publish_third_party::MAX_ITEMS` |
| Hashes a terms acceptance names | 40,000 | `terms::MAX_HASHES` |
| Characters of one of those hashes | 64 | `terms::MAX_HASH_LEN` |
| Tags of a new item's save (an item the node holds may grow to it, or keep more) | 20 | `items::save::MAX_TAGS` |
| Outcomes of a social emote's save | 10 | `items::save::MAX_OUTCOMES` |
| Files an item document names, and models of one representation | 256 | `items::schema::MAX_CONTENTS` |
| Tags or required permissions of an item document | 1,000 | `items::schema::LIST_MAX` |
| Characters of a tag, permission, file name or spring bone name | 1,000 | `items::schema::TEXT_MAX` |
| Representations of an item document | 10 | `items::schema::REPRESENTATIONS_MAX` |
| Categories one list hides or replaces | 100 | `items::schema::HIDDEN_MAX` |
| Spring bones of one model | 1,000 | `items::schema::BONES_MAX` |
| Mapping entries of an item | 100 | `items::schema::MAX_MAPPINGS` |
| Token ids an item's mappings name | 10,000 | `items::schema::MAX_MAPPING_IDS` |
| Characters of a collection's name | 42 | `collections::save::NAME_MAX` |
| Characters of a collection's other text fields | 256 | `collections::save::TEXT_MAX` |
| Entries of a collection's address lists | 1,000 | `collections::save::LIST_MAX` |
| Bytes of a collection save's `data` | 1 MiB | `collections::save::DATA_MAX` |
| Items one bulk item curation write names | 1,000 | `handlers::curation::MAX_BULK_ITEMS` |
| Body of a newsletter subscription | 4 KiB | `handlers::newsletter::MAX_BODY` |
| Characters of a subscription's `source` | 100 | `handlers::newsletter::MAX_SOURCE` |
| Body of a validator claim or lease | 4 KiB | `handlers::validator::SMALL_BODY` |
| Body of a validator verdict | 5 MiB | `handlers::validator::VERDICT_BODY` |
| Characters of a third-party id in a path | 512 | `handlers::third_parties::ID_MAX` |
| Body of a video upload | 251,048,576 bytes | `items::video::BODY_LIMIT` |
| Bytes a video upload's form may hold besides the video's | 64 KiB | `items::video::FRAMING_BYTES` |
| Age at which a video upload removes a spool file a crash left | 1 hour | `items::video::SPOOL_ORPHAN_AGE` |

## Outside services and secrets

Each secret may also be given as `<NAME>_FILE`; none is printed. Startup
refuses a service given some of its settings without the others
(`WAREHOUSE_CONTEXT_PREFIX`, `CREATORS_URL`, `BUILDER_URL` and
`BUILDER_SERVER_URL` stay optional), and a `WAREHOUSE_URL`,
`NEWSLETTER_SERVICE_URL` or `FORUM_URL` that is not a public `https://` URL,
because each carries a token or key. It also refuses the forum without
`BUILDER_SERVER_URL`, the origin of the thumbnails a topic shows, and a
`NEWSLETTER_PUBLICATION_ID` other than letters, digits, `_` and `-`. `SLACK_BOT_TOKEN` is also the bot social's Slack notices
use: set alone, it leaves the curators' notices off.
`SLACK_CURATION_CHANNEL` turns them on; set without the token, the node
refuses to start.

| Service | Settings | Without them |
| --- | --- | --- |
| Wearable validator | `WEARABLE_VALIDATOR_CALLBACK_SECRET` (at least 32 bytes) | No validation is requested; the validator routes refuse |
| Curators' Slack | `SLACK_CURATION_CHANNEL`, with `SLACK_BOT_TOKEN` (shared with social), `CREATORS_URL` (else `BUILDER_URL`) | No notice is posted |
| Curators' forum | `FORUM_URL`, `FORUM_API_KEY`, `FORUM_API_USERNAME`, `FORUM_CATEGORY`, `BUILDER_SERVER_URL` | Forum posts and replies answer 503 |
| Terms of service warehouse | `WAREHOUSE_URL`, `WAREHOUSE_TOKEN`, `WAREHOUSE_CONTEXT_PREFIX` | Acceptances are stored and forwarded later |
| Newsletter service | `NEWSLETTER_SERVICE_URL`, `NEWSLETTER_PUBLICATION_ID`, `NEWSLETTER_SERVICE_API_KEY` | Subscriptions are answered `{ok:true}` and go nowhere; the node never stores one |
| Operator routes (item status, `GET /v1/collections/curation`) | `CATALYRST_BUILDER_ADMIN_TOKEN` (at least 32 bytes), the curation console's bearer; `BUILDER_ADMIN_ADDRESSES`, the wallets whose signed requests they admit | The routes refuse every request |

## Paying a publication fee with credits

A creator may pay a standard collection's publication fee with the node's
credits instead of MANA. The creators app asks `POST
/credits/credits/authorize-publication` (canonical `POST
/shop-api/v1/credit-authorizations/publication`), signed, with the
collection manager's `createCollection` call it is about to send. The node
answers one credit sized for the fee and its co-signature of that call, and
the app sends both in one `useCredits` to the node's CreditsManager, through
the relay or from the wallet.

The node authorizes only what it can check. The call must be the one that
deploys an unpublished standard collection the signer owns, at the address
that collection's row holds; a collection that is published, deployed or being
published by the node is refused. A collection under its owner's lock is
not: a first transaction may have reverted, and its owner retries. The fee is the
sum of the items' rarity prices in USD, rounded up to a cent, and a request
naming another amount answers 409. The credit holds the fee's MANA at the
node's rate plus `CREDITS_ORACLE_BUFFER_BPS`, lasts `CREDITS_TTL_SECONDS`, and
counts against the wallet's balance and its bound of 10 live credits like a
purchase credit. The relay accepts a `useCredits` whose call is
`createCollection` only for the sender's live authorization of exactly that
call and credit.

A node needs these to serve it, beside what a purchase with credits needs
(`CREDITS_ONCHAIN_AUTHORIZE_ENABLED`, `CREDITS_SIGNER_KEY_FILE`,
`CREDITS_MANAGER_CONTRACT`, the CreditUsed index and `OPS_WRITES_ENABLED`):

| Requirement | Where | Without it |
| --- | --- | --- |
| The external-call signer's key | `CREDITS_EXTERNAL_CALL_SIGNER_KEY_FILE`, a file holding a 32-byte secp256k1 key; the plain variable is refused | 503 naming the setting |
| `EXTERNAL_CALL_SIGNER_ROLE` for that key's address | On the node's CreditsManager, granted by its admin: `grantRole(keccak256("EXTERNAL_CALL_SIGNER_ROLE"), <signer address>)` | 503 naming the missing role |
| `createCollection` allowed as a custom external call | On the node's CreditsManager, by its admin: `allowCustomExternalCall(<CollectionManager>, 0x35a629c2, true)` | 503 naming the missing allowance |
| The rarities and the collections copy | `BUILDER_POLYGON_RPC_URL` and the Polygon collections subgraph, as the builder's own routes need them | 503 |

The CollectionManager is `0x9d32aac179153a991e832550d9f96441ea27763a` on
Polygon and `0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849` on Amoy. The node
reads both on-chain facts before it reserves or signs anything and keeps the
answer for a minute, so a grant takes effect within a minute and a revoked one
stops new authorizations as fast.


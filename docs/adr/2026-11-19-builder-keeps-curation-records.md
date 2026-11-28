# ADR: The builder keeps builder-server's curation records and reads curation state from them

- Status: Accepted
- Date: 2026-11-19

## Context

`github.com/decentraland/builder-server` at `98897af` reviews collections
and third-party items through four records: a collection's curations (one
per review request, with its assignee, decision and rejection reasons), an
item's curations (one per submission of a third-party item, with the
content hash reviewed), a collection's review events and the slot cheques a
third-party publication signs. Its clients read curation state from these
rows: a third-party item reads as published once it has an item curation,
and a collection's review state is its newest curation.

The node takes curation over from builder-server, so it must hold these
records with the meaning builder-server gives them, and imported data must
answer as it does upstream.

## Decision

- **The rows are the state.** `builder.collection_curations`,
  `builder.item_curations`, `builder.collection_events` and
  `builder.slot_usage_cheques` keep builder-server's columns and values. An
  item's curation status is the status of its newest item curation, read
  where it is needed. No column on an item or a collection copies it, so no
  writer has to keep a copy in step.
- **Newest is well defined.** A collection or an item has at most one
  curation per instant, and every writer stamps a new item curation after
  the item's newest one, so the newest row is the last written. Events are
  read newest first by time, then id, and a validation has one verdict.
- **Published follows curation.** A third-party item is published once it
  has an item curation, whatever its status. Its files, its reads, the
  listings and the provider's slot count use that one rule, and a
  collection with a curated item is published.
- **Stricter only where no client writes.** Statuses, event types, actors
  and rejection reasons are checked against builder-server's values;
  addresses are lowercase; a forum link is an `http(s)` URL that the node
  stores and returns but never fetches.
- **Third-party items reach curation through publish.** `POST
  /v1/collections/{id}/publish` of a third-party collection, as
  builder-server answers it, is the one way a third-party item is
  submitted: a current manager of the provider names the items with a slot
  cheque it signed, the EIP-712 `ConsumeSlots(thirdPartyId, qty, salt)`
  under the Polygon Third Party Registry's domain. The node recovers the
  signer, requires the signer, the provider, the quantity and the items to
  match, and counts the provider's free slots (the registry's `maxItems`
  less its items ever curated) under the provider's lock. In one
  transaction it stores the cheque, a pending curation of each item and a
  new pending curation of the collection; a refusal writes nothing. The
  committee then reviews the items through the curation routes and
  approves them with the registry transaction that consumes the cheque;
  `GET /v1/collections/{id}/approvalData` gives the committee the newest
  cheque, the items' content hashes, the registry's root and whether the
  registry already holds the cheque's receipt. No other builder decides a
  submission.
- **Deletes.** An item's curations and a collection's curations and events
  go with them; a slot cheque, the record of slots consumed, keeps its
  collection.
- **One transition table.** Who may move a curation from one status to
  another is one table (`services/catalyst/builder/src/curation/transitions.rs`):
  - a creator (a wallet with a role on the collection) opens a review
    request when none is pending, and refreshes the content hash of an
    item's pending curation;
  - only the committee assigns a review, changes a collection curation's
    status, or decides an item's curation, and a decided curation reopens
    only by a new request;
  - an assignee must be on the committee.

  Each write runs in one transaction that locks the collection row, decides
  again who may write and reads the newest curation under the lock, so
  concurrent curators end in one state.
- **What a decision writes.** A curator's decision records the reviewer, the
  rejection reasons and message, and a `review.assigned`,
  `review.approved` or `review.rejected` event; a repeated decision records
  nothing new, and a rejection resent with other feedback corrects it. Every
  committee write records its `ops.admin_audit` row in the same transaction.
- **What an approval pins.** A collection's approval sets each smart
  wearable's kept video to the `video.mp4` its saved contents name, when the
  videos route took it; a later save of a published item keeps that video.
  An item's curation records the item's content hash when it is submitted,
  and again when its pending curation is refreshed. The committee's
  approval of an item names the content hash it approves
  (`curation.contentHash` on `PATCH /v1/items/{id}/curation`): without it the
  node answers 400, and when the item holds other content, saved after the
  committee read it, 409 with the current hash; either way it writes
  nothing. So an approval records only content the approver names, never an
  edit made between the committee's `approvalData` read and its approval.
  The item's contents read by its contract and on-chain id stay its latest
  saved contents.
- **Slack notices.** A publication, a review request, an assignee change
  and a new decision are announced in the curators' Slack channel
  (`SLACK_BOT_TOKEN`, `SLACK_CURATION_CHANNEL`), with links to the
  collection's review page and the people's pages on `CREATORS_URL`. A
  notice is sent only after its write commits, in a task of its own with a
  10-second bound, through the client that follows no redirect; it is best
  effort, so a Slack failure is logged without the token and changes no
  answer. Text that comes from a creator is put on one line, cut and escaped
  so it can't mention, link or format.
- **Forum topics.** `builder.collections.forum_link` and `forum_id` are the
  one source of a collection's topic link and first post id; a collection
  with a stored link has its topic. An owner or manager requests a topic for
  a published standard collection, and a provider's manager for a
  third-party collection whether or not any of its items was submitted; a
  current
  committee member can reply in its topic. The node writes the topic title
  and body from the saved collection and its first 20 items, and resolves a
  reply's topic from the saved post id. The configured forum origin owns
  every newly recorded link.

  Topic creation commits a claim under the collection lock before calling
  Discourse, and releases the database connection for that call. A claim
  holds for 60 seconds and a whole attempt ends within 45, so two requests
  never call Discourse for one collection at once; an uncertain attempt
  keeps its claim. When Discourse made a topic but the node did not store
  its link (a lost response, a node restart), a request after the claim
  lapses asks Discourse again: a standard collection gets a second topic,
  under the title that adds its shortened contract address, and a
  third-party collection is refused by Discourse because its title is
  taken, so it has no stored topic until an operator stores the link. Of two
  attempts that both reach Discourse, the first link stored stays and both
  answer it. The account's permissions must be verified before forum writes
  are enabled on a deployment. Without the four forum settings, admitted
  writes answer 503.

## Consequences

- The import copies the four tables and the forum columns. Of an item's
  curations created at the same instant, it keeps the one updated last; it
  clears a forum link that is not a web URL.
- A database bootstrapped before this decision keeps
  `builder.items.curation_status` and `is_approved` until
  `schemas/releases/builder-item-curation-retired.sql` drops them, after
  recording a curation for each third-party item the node submitted itself.
- A writer of curations or events holds the collection locked, which the
  code upholds; the schema can't.
- The additive forum migration copies retained linked-publication links
  into collections before writes resume. The explicit linked-forum release
  removes the extra column with writers stopped and a verified backup.
  An upgraded deployment verifies retained links against its configured
  forum origin before enabling writes.
- A database bootstrapped before third-party publish keeps the linked
  publication table until
  `schemas/releases/builder-linked-publications-retired.sql` copies its
  submitted cheques into `builder.slot_usage_cheques` and drops it.
- A publish whose forum call or Slack notice fails stands: the publish
  opens its collection's topic after it commits, and a manager's client
  that finds none asks for it again with `POST /v1/collections/{id}/post`,
  which never changes a stored topic.
- A notice lost to a Slack outage isn't sent again; the timeline and the
  audit rows keep the decision.
- The curation lists answer at most 20,000 rows, above the reference data's
  largest committee listing and largest collection.

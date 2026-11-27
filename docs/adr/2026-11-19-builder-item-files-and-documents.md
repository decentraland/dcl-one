# ADR: The builder keeps item documents in order, stores only named, verified files and serves them only to their readers

- Status: Accepted
- Date: 2026-11-19

## Context

The builder answers items as `github.com/decentraland/builder-server` at
`98897af` does. Three of its properties shape how the node stores them:

- An item's `data`, `metrics` and `contents` are JSON documents that
  builder-server keeps as saved, keys in order. The content hash it stores
  for a published standard item (ADR-32 over its files and entity metadata)
  and for a third-party item (keccak-256 of its metadata, files and
  mappings) serializes those documents in that order, and the clients compare
  that hash with the catalyst's.
- A client saves an item, naming each file by its content hash, and then
  uploads the files.
- Item files are private until their item is published, so a name in an
  item must not open a file to whoever wrote the name.

## Decision

- **Documents in order.** `builder.items.data` and `metrics` are `json`,
  written as `JSON.stringify` writes them, and `builder.item_contents` keeps
  each file's position. Bodies answer them as stored, and the content hashes
  the node computes on a save match builder-server's byte for byte.
- **What a published item takes.** A standard item is published once it is
  minted and the copy of the Polygon collections subgraph holds it (or the
  node published it itself); its body then takes the chain's URN, price,
  beneficiary, rarity, supply and content hash. A third-party item is
  published once it has an item curation, whatever its status: a
  submission for review, recorded by the node or imported. On a published collection, the
  owner and the on-chain managers save an existing item's metadata and
  contents; a new item, a move in or out and a rarity change are refused.
- **Uploads are named and verified.** Each uploaded part is sent under the
  content hash its item names. Before it reads the body, the node takes one
  of 16 places for file uploads in flight, at most four for each wallet and
  eight for each source address (a refused place answers 429 with
  `Retry-After` at once). The whole body must arrive within 5 minutes
  (about 0.6 Mbit/s for the largest, 21 MiB), and 30 seconds without a byte
  ends the upload. While all 16 places are held, an upload behind that pace
  gives its place to a newcomer as a video upload does (below). It reads
  the body within its bounds, hashes each
  file, refuses a file whose bytes don't match the hash, whose item doesn't
  name it or that the item names only as its `video.mp4` (a video comes
  through `/videos` and its container check), and only then stores it,
  holding no connection or row lock while it writes. It records the files
  in one transaction with the item locked, once the decision, the names and
  the stored bytes still hold (a file the blob collector removed in between
  answers 503, as a content deployment does). A
  record the videos route writes is marked as a video
  (`builder.uploaded_contents.video`); both publications take, for an item's
  `video.mp4`, no other record, and an approval pins only such a video, so
  bytes that reached the store another way never become an item's video.
- **Uploads have budgets.** A source address sends at most 20 GiB of item
  files and videos a day, charged with the bytes each upload actually sent
  once it ends; an upload waits only while the address's day is spent. A
  wallet's uploads hold at most 5,000,000,000 bytes and 100,000 files that no
  published item names, and every wallet's together
  `BUILDER_UNPUBLISHED_MAX_BYTES` and `BUILDER_UNPUBLISHED_MAX_FILES`; once
  the plain sum of the upload records passes a bound, the node counts the
  unpublished ones at most once a minute and moves that count by what the
  records gained or lost since, so a publication frees them within a minute.
  An address the node can't tell apart, an internal one behind a gateway it
  doesn't trust, counts toward no source. Publication refuses an item that
  names a hash the store holds no bytes for, as it refuses an unuploaded
  one.
- **A video streams to the store.** `POST /v1/items/{id}/videos` takes a
  smart wearable's preview video, one file part named `video.mp4`, of up to
  250,000,000 bytes. The node decides access, checks that the item names a
  video and takes one of 32 places for uploads in flight
  (`BUILDER_VIDEO_UPLOADS_MAX`, 1 to 256), at most one for each wallet and
  two for each source address, before it reads a byte of the body (the
  places work as the file uploads' do); it never waits for a place, and a
  refused one answers 429 with `Retry-After` at once, so filling every place
  takes as many wallets and half as many addresses. A wallet and a source
  address may hold places for 4 and 12 hours of a day.
  The body then streams into one spool file beside the blob store, hashed as
  it arrives, holding no database connection or row lock. The multipart
  parser may hold at most 64 KiB beyond the bytes it has handed on, the
  whole body must arrive within 20 minutes (about 1.7 Mbit/s for the
  largest video), 30 seconds without a byte ends the upload, and the node
  stops reading the moment a bound is passed. The 20 minutes count from the
  moment the upload takes its place; the 30 seconds and the pace below count
  from the body's first read, so the budget checks before it count against
  neither. An upload is behind while it has received less, since that first
  read, than the pace the largest video needs to arrive in 20 minutes
  (about 1.7 Mbit/s). While every
  place is held and an upload is behind, a newcomer takes an extra place,
  one for each upload behind, and reads its body. It earns a place once it
  has received what that pace brings in 30 seconds (6,250,000 bytes for a
  video, about 2.2 MB for files): then the slowest upload behind ends with
  408 and `Retry-After`, or, when none is behind any more and no place came
  free, the newcomer ends with 429. A newcomer whose whole body arrives
  first cuts nobody. One that hasn't earned its place 30 seconds after its
  first read ends with 429 and `Retry-After`, having cut nobody, so a place
  is taken only by an upload that keeps the pace. An
  upload is behind from its first moment, with no grace: a grace would let
  uploads that send nothing and restart inside it hold every place for
  free. A small video on a slow link finishes while places are free, and
  holding every place costs a sender that pace per place. The video must be an MP4 container (an `ftyp`
  box first, a `moov` box, boxes that end where the file does), whatever
  its part's name or content type says. Only then the node syncs the spool
  file, decides access again, checks that the item still names the hash at
  `video.mp4` and stores the file, holding no lock; then, in one
  transaction with the item locked, it decides and checks again and records
  the upload under the store's shared lock while the file is still stored
  (a file the blob collector removed in between answers 503, as a file
  upload does). That store and record run in a task of their own, so a
  client that leaves can't cut them short. A refused or abandoned upload leaves no spool file,
  and a spool file a crash left is removed by the first video upload after
  it is an hour old.
- **Naming grants nothing.** An item may name any hash. What proves a wallet
  holds a file is its own upload record (`builder.uploaded_contents`, one row
  per hash and uploader), never a name in an item.
- **Each name has a writer.** Every row of `builder.item_contents` keeps
  `named_by`, the wallet whose save wrote that name. A save keeps the writer
  of a file and hash it re-sends, and of a hash the item already named
  under another file name; only a hash the item didn't name takes the
  saving signer. A name counts only while its writer holds an upload record
  for its hash. An item's `video` is a name too: `builder.items.video_named_by`
  keeps the wallet whose save wrote it (kept while the value is, else the
  writer of the item's own entry for that hash, else the saving signer), so a
  published smart wearable's kept video stays readable after a re-save
  replaces its `video.mp4`, and a wallet that writes another wallet's hash
  there opens nothing.
- **Records follow names.** When a save or a delete leaves a hash no item
  names, in its contents or as its video, its upload records go in the same
  transaction. The blob collector
  counts upload records and never a name a save writes: neither a hash in
  an item's `contents` nor its `thumbnail` file name keeps a blob. The item
  hashes it does count (`local_content_hash`, `content_hash`,
  `catalyst_content_hash`) are written by the node and the import, never
  by a save or an upload. So it may take the bytes once no record and
  nothing else names them.
- **A publication takes only uploaded files.** The standard publication and
  the third-party publish refuse an item with a name that doesn't count: a
  hash whose writer holds no upload record for it. The publication snapshot
  computes it for the standard one (`missing_content` in `SNAPSHOT_QUERY`,
  `services/catalyst/builder/src/publication.rs`) and the publish's item
  read for the third-party one; the tests
  `a_standard_publication_takes_only_files_each_names_writer_uploaded` and
  `every_refusal_answers_before_anything_is_written` hold it.
- **Imported records.** Builder-server keeps no uploader, so the import
  attributes every name to the item's owner and records that owner as an
  uploader of every hash the item names, with the blob's size from the blob
  copy's inventory. A wallet whose
  imported item names a hash therefore holds a record for it, whoever sent
  the bytes. The node treats an imported record as one written on upload:
  it protects the bytes from the collector, it satisfies a publication's
  check, and it opens the file to that wallet as the file routes below
  decide.
- **Files are private until published.** The node keeps item files in the
  item-files tree of the blob store, `<STORAGE_ROOT_FOLDER>/item-files`,
  which no content or worlds route reads: `/contents/{hash}`,
  `/available-content` and a deployment's check for files already stored
  never find an item file. The collector walks both trees. An upload checks
  for the stored file and records it under the store's shared lock, as a
  local deploy does, so the collector never removes a file an upload has
  just recorded. The node serves item files at `GET` and `HEAD
  /v1/storage/contents/{hash}` and the `exists` probe, and never redirects
  to a bucket. A request without a
  signature reads a file only while a published item names it in a name
  that counts. A standard item is published once its collection is; a
  third-party item once it has an item curation. A signed request also reads a
  file its signer holds an upload record for, and a file that an item the
  signer owns, or a third-party item whose collection the signer manages,
  names in a name that counts. On-chain roles, the committee and other
  wallets read what an unsigned request reads.
  `access::read_file` (`services/catalyst/builder/src/access.rs`) decides;
  the table `tests/item_files.rs` holds it.
- **A public list of names opens nothing more.** `GET
  /v1/items/{address}/{id}/contents`, which answers a minted item's
  `contents` to anyone, leaves out every entry an unsigned request couldn't
  read at the file routes, and a smart wearable's kept video when it is such
  a file; one predicate (`PUBLIC_FILE` in `access.rs`) decides both. So every
  hash it names opens for the client that follows it, and
  no answer names a file the rule above keeps private.
- **A refusal reveals nothing.** A hash that isn't a canonical CID, a
  withheld hash, a private file and a hash with no file all answer the same
  404 with `Cache-Control: no-store`, and the probe answers 404 `{ok:true}`
  for each. A signature that doesn't verify answers 401 whatever the hash.
- **Bytes never become active content.** The node reads the file's type
  from its first bytes, never from its upload; a type that isn't a media
  type is sent as an attachment, and every answer carries
  `X-Content-Type-Options: nosniff` and `Content-Security-Policy: sandbox`.
  An unsigned answer is cached publicly for a year and marked immutable,
  since a hash names its bytes for good, as the node's `/contents/{hash}`
  answers are and as builder-server's storage route answers; a signed
  answer is `private, no-store`. A hash the content denylist withholds, or
  one no published item names any more, answers 404 at the node from then
  on, while a copy a browser or a CDN kept of an earlier unsigned answer
  stays until its year runs out: the node purges no cache, for item files
  as for content.

## Consequences

- No file is stored that its item doesn't name, so what an owner stores
  beyond what its items name is bounded by what is in flight.
- A hash learned from elsewhere opens nothing: naming it in an item, even a
  published one, makes it readable only when the wallet that wrote the name
  uploaded the bytes.
- A save or a delete that leaves no item naming a hash removes its upload
  records, so the node refuses the file at once and the collector may take
  the bytes; a copy cached from an unsigned answer while a published item
  named it stays where it was cached. A record that no item names, such as an imported record whose
  item is gone or a blob of a retired scene project, stays readable by its
  uploader alone.
- The decision reads no committee and no fresh chain fact. The chain copy
  never removes a collection, so a stale copy, or one without a complete
  pass, can only leave a collection the node holds no record of unpublished
  a while longer.
- A client that shows a draft's file signs that request, so a browser
  fetches the bytes with the signature headers and shows them through an
  object URL.
- A database the release hasn't run on keeps `jsonb` documents and one
  upload record per hash until the release
  `schemas/releases/builder-item-documents-and-uploaders.sql` runs
  (`DATABASE.md`, "Builder items workload", gives the rollout order). Until
  then item saves and uploads answer 503 naming the release and the item
  import refuses to load, so no document loses its order; reads keep
  working.
- A client must save an item before uploading its files and must send each
  file under its hash, and its video as the file `video.mp4`, as the
  `/create` dApp does.
- The gateway passes a video's body to the node as it arrives, so a caller
  the node refuses never makes the gateway store a body first. What an
  admitted upload can cost is bounded: 250 MB of spool and of store for
  each upload, 32 at a time by default (8 GB of spool, and 250 MB for each
  place `BUILDER_VIDEO_UPLOADS_MAX` sets), one for each wallet and two for
  each source address, so filling every place takes 32 wallets on 16
  addresses. A newcomer waiting on an extra place holds what the pace
  brings in 30 seconds before an upload gives way, and the spool of the
  upload that gives way is removed. A stalled sender gives its place back
  within 30 seconds, and one sending below the pace as soon as a newcomer
  earns its place. A replaced video's bytes become collectable once no item
  names them.

## Alternatives considered

- **`jsonb` documents.** `jsonb` reorders keys, so the node's content hashes
  would differ from the catalyst's for third-party and published items.
- **Refusing a hash another wallet uploaded.** A creator who has the same
  bytes as another wallet, a common texture for example, couldn't name them
  before uploading, which is the order the clients save in.
- **Storing any uploaded file.** Storage would grow with files no item names,
  and nothing would tie a file to the item it was uploaded for.

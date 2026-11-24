# ADR: One volume root, namespaces named for their data, and access through the storage crate

- Status: Proposed
- Date: 2026-11-24

## Context

The node keeps files on local disk in several places, and each grew its own
name, setting and access code:

- **The blob store.** `STORAGE_ROOT_FOLDER` holds one hash-addressed tree,
  `contents/`, shared by every area that stores a blob (content, worlds,
  explore, social, comms, shop and the camera reel). A `tmp/` beside it
  holds `content-uploads` and `worlds-uploads`. The `storage` crate that
  owns it began as the content server's store and still carries that name
  in its layout.
- **Path-keyed stores outside it.** The asset-bundle cache
  (`AB_CDN_CACHE_DIR`), the imposter store (`BVIMPOSTERS_STORE_ROOT`), the
  profile-image render scratch (`PROFILE_IMAGES_CACHE_DIR`) and badge art
  (`BADGES_ASSETS_DIR`) are siblings of the blob root, each with its own
  temporary-file convention. Pulse keeps a replay journal at a path relative
  to its working directory.
- **Readers by path.** The gateway's nginx serves `contents/<shard>/<cid>`
  from the same mount, the snapshot writer and one worlds check resolve blob
  paths themselves, and the import tooling repeats the layout in Python.

Three problems follow:

- Directory names mix what the data is (`contents`, `profile-images`) with
  which service wrote it (`content-uploads`, `worlds-uploads`,
  `bvimposters`). A
  service name says nothing about how the files are served, backed up or
  collected, and it goes stale when a responsibility moves between areas.
- Settings that name a directory end in `_FOLDER`, `_DIR`, `_ROOT`,
  `_WORKDIR` or `_PATH`, and the roots are independent, so a deployment that
  mounts only the blob root silently loses the others.
- Nothing states which directories an operator must back up and which the
  node rebuilds.

## Decision

- **One root.** Everything the node persists on disk lives under
  `STORAGE_ROOT_FOLDER`. A deployment mounts one volume there. A setting
  that names another directory is an optional override whose default is a
  path under the root.
- **Four classes at the top level, named for how the files are kept.** The
  class decides backup, collection and who may read by path:

  | Class | Holds | Backed up | Removed by |
  | --- | --- | --- | --- |
  | `blobs/` | Hash-addressed files a database row names | Yes | The collector, when no row names them |
  | `files/` | Path-keyed files the node cannot rebuild | Yes | Their owner's own writes |
  | `cache/` | Files the node can fetch or render again | No | Eviction, or the operator at any time |
  | `tmp/` | Spools and scratch of work in flight | No | A sweep of anything older than its window |

- **Namespaces named for the data.** Under a class, a directory is named
  for what it holds, in lowercase words joined by hyphens, never for the
  crate, process or upstream service that writes it. Two areas that store
  the same kind of data use the same namespace.

  | Namespace | Holds | Today |
  | --- | --- | --- |
  | `blobs/contents` | Every area's public, content-addressed files | `contents` |
  | `blobs/item-files` | Wearable and emote files, private until published | None yet |
  | `files/badge-art` | Badge images and models | `BADGES_ASSETS_DIR` |
  | `cache/asset-bundles` | Converted asset bundles | `AB_CDN_CACHE_DIR` |
  | `cache/imposters` | Scene imposter archives | `BVIMPOSTERS_STORE_ROOT` |
  | `tmp/entity-uploads` | Entity deployments being received | `tmp/content-uploads` |
  | `tmp/scene-uploads` | World scene deployments being received | `tmp/worlds-uploads` |
  | `tmp/item-videos` | Item videos being received | None yet |
  | `tmp/avatar-renders` | Profile-image render work | `PROFILE_IMAGES_CACHE_DIR` |
  | `files/replay-journal` | Pulse's handshake replay journal | `PULSE_REPLAY_STATE_PATH` |

- **A new blob tree needs a reason the shared one cannot meet.**
  `blobs/contents` stays one flat tree, with ownership recorded only in the
  database columns the collector reads, so identical bytes are stored once.
  A separate tree exists only where files must not be reachable by the
  routes and the gateway alias that serve `contents`, as item files must
  not be.
- **Access through the storage crate.** The `storage` crate declares every
  namespace in one list, with its class, and hands out the handle that
  reads and writes it: a blob tree, an upload spool or a keyed file cache.
  No other crate joins a path under the root, and none reads another
  namespace's files by path. A test holds the list to the directories the
  code opens, as it holds the list of blob writers.
- **Readers outside the process are declared.** A namespace another
  container reads by path is marked so in that list. Today that is
  `blobs/contents` alone, which the gateway serves. Its
  `<shard>/<cid>` shape is a contract and changes only with the gateway and
  the import tooling in the same change.
- **Rules for a namespace's writers and readers.** Each holds for every
  namespace of its kind, present and new:
  - *A hash-addressed file is named by the node.* The node hashes the bytes
    it received and stores them under that hash; a name a client sent is
    only compared with it. Two writers that store the same name therefore
    store the same bytes, and neither can replace the other's.
  - *A stored file never changes.* Nothing writes into, truncates or
    replaces a file under its final name. A changed model or image is a new
    file under a new name, and the old one stays until no row names it.
  - *A row gives a file its meaning.* A writer stores the bytes, then
    commits the row that names them, holding the store's shared lock so
    the collector can't remove them in between. Every column that names a
    blob is in the collector's list, and only the collector unlinks a blob.
  - *Knowing a hash grants nothing.* A file goes into a tree every route
    serves only if anyone may read it. A file with readers goes into a tree
    of its own, is served only after an access decision on the row that
    names it, and its presence is never reported to a caller who may not
    read it, including by an upload that skips bytes the node already
    holds.
  - *Publishing copies through the destination's writer.* A file moves
    from one namespace to another only as an upload to the destination,
    which hashes, validates and bounds it as any other. No namespace links
    to, renames from or reads another's files.
  - *A path-keyed file has one owner and a built key.* One area writes a
    path-keyed namespace. The storage crate builds each path from validated
    parts; no request supplies a path segment. A write lands under a
    temporary name and is renamed into place.
  - *Every writer is bounded.* A write is charged to a bound on its caller
    before its body is read. A `cache/` namespace has a size bound and
    evicts; a `tmp/` entry belongs to one request or job and is swept once
    older than its window.
  - *Names are stable and unknown files stay.* A namespace is never reused
    for other data, and no code deletes a directory or file it doesn't
    recognize.
- **Setting names.** A setting that names a directory ends in `_DIR` and
  one that names a file in `_PATH`; `_FILE` stays reserved for a secret
  read from a file. `STORAGE_ROOT_FOLDER` keeps its name, which deployments
  already set.
- **Existing nodes move by a recorded step.** A node that holds data under
  a current name keeps reading it there until an operator command moves
  each directory to its namespace with a rename on the same filesystem. No
  startup moves files on its own, and none deletes a directory it does not
  recognize.

## Consequences

- The snapshot writer and the worlds partial-upload check move to the
  storage crate's handles. The keyed file cache
  moves from `explore` into `storage`.
- The gateway's alias, the import tooling's layout and `DATABASE.md`'s
  backup sections change with `blobs/contents`.
- `AB_CDN_CACHE_DIR`, `BVIMPOSTERS_STORE_ROOT`, `PROFILE_IMAGES_CACHE_DIR`,
  `BADGES_ASSETS_DIR`, `UPLOAD_SPOOL_FOLDER` and `PULSE_REPLAY_STATE_PATH`
  become overrides with defaults under the root, renamed to the suffix rule.
  Pulse runs as its own process, so its journal is under the root only
  where Pulse mounts the same volume.
- Deployments that mount several volumes mount one, or keep the overrides.
- Adding a directory to the node means adding a namespace to the list, with
  its class, in the same change.

## Alternatives

- **Flat namespaces with the class recorded only in code.** Shorter paths,
  but an operator can't tell from the volume what to back up, and a cleanup
  by path can't spare what the node cannot rebuild.
- **A directory for each area.** It names the writer, not the data: it says
  nothing about how the files are served or collected, it goes stale when a
  responsibility moves, and under `contents` it stores identical bytes once
  for each area and breaks the shape the gateway serves.
- **Independent roots, one setting each.** A deployment that mounts only
  some of them leaves the others on the container's own disk without an
  error.
- **Each area keeps its own file code.** Staging names, sweeps, the
  collector's lock and the write switches then have to be repeated, and a
  new store can skip them.

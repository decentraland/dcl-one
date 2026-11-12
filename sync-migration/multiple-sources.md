# Multiple source streams: decisions and integration contract

Status: implemented for bounded local rehearsal. These decisions extend the interaction pilot; they do not authorize broader public reads or production activation.

## Why share coordination but keep streams independent?

Places, Events and social settings come from different source databases. They do not share a commit cursor or one atomic snapshot. Each source therefore has one explicitly registered owner, immutable generation and scoped keys, its own capture adapter and its own observed/captured/applied positions. A stalled Events stream must not silently advance social settings or Places.

The shared worker supplies durable inboxes, source binding, scoped admission, transaction ordering, retry safety, progress and diagnostics. Domain modules supply strictly typed records, validation and target mapping. SQL identifiers are selected from a closed adapter enum, not constructed from caller-provided table names. No generic table/SQL replication API is exposed.

Separate capture processes use separate source credentials. They can write into one target database. Target application commits each source transaction atomically, but this is not a cross-source transaction protocol. Workflows requiring coordinated social/event/world permissions remain gated until their dependencies and enforcement are proven.

## New source-only contracts

- Events uses source identity `events`, adapter `events_schedules_outbox_v1` and a UUID schedule allowlist. It changes schedule facts only; event links, event delivery and event recurrence are outside this mapping.
- Social settings uses source identity `social-settings`, adapter `social_settings_outbox_v1` and a canonical wallet allowlist. It preserves the three source settings and absence of rows. Target observation timestamps are local metadata and are excluded from canonical equivalence.
- Places retains its own source identity and source ordering. Catalogue expansion must join its interaction snapshot and outbox, not create a competing writer for the same catalogue/interaction dependencies.

New domain keys and tombstones use dedicated migration metadata. Existing Places catalogue scope remains separate. Registration/enqueue/application must compare the envelope source with the registered stream source; a correctly shaped settings change cannot be injected into an Events stream. Native replay remains Places-only.

## Source and target changes

Source HTTP handlers and clients remain unchanged for these source-only mappings. Each source database needs an explicitly installed, local transactional capture adapter with reviewed grants. The adapters record normal SQL writes, including deletions and key changes, and reject unsupported truncation. Normal target runtimes and providers remain off.

Bootstrapping a domain requires an empty scoped target and a new registered source owner. No existing stream is reset or silently expanded. Unknown schemas, identities, adapter changes, missing history or out-of-scope keys stop progress. A widened target constraint permits new adapter identities but does not change old rows.

Domain-specific reasons, side-effect restrictions and test evidence belong in the service documents linked from the directory index. In particular, social settings synchronization does not reproduce gatekeeper metadata enforcement, and a finite schedule allowlist does not cover all future randomly generated schedule IDs.

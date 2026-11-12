# Alternatives considered

Every activation option requires migration and validation before activation. Shop and credits remain excluded. The current selection is a one-way variant of Option 4, using compatible duplicate operations first and selective DB sync only for demonstrated residual changes.

| Option | Proposal | Benefit | Main blockers and decision |
| --- | --- | --- | --- |
| **1. Gradual activation through duplicate requests** | Route included traffic through the gateway, execute approved commands in both backends and gradually expose target responses. | Exercises both implementations with real operations. | Internal writers, jobs and direct SQL bypass requests; commands can generate different IDs or repeat effects. Gateway duplication alone does not maintain all state. Use proven duplicate commands as the primary lane, complemented only by demonstrated residual sync. |
| **2. Backup snapshots** | Pre-migrate transformed data/assets, stop old writers at a final boundary, catch up/reconcile and transfer included traffic, callbacks and jobs together. | Avoids prolonged two-system workflow ownership and temporary adapters. | Requires replacement completeness, a measured interruption window, complete final deletions/revocations and tested recovery. An old backup alone loses later writes; a raw dump cannot replace schema transformations. Remains a cutover alternative, not the selected coexistence strategy. |
| **3. Automatic database synchronization** | Capture committed data changes and transform them into the target schema; execute each command once. | Covers included HTTP, job, internal and direct SQL writes without synchronization code at every application write site. | Requires semantic mappings, snapshots, deletes, replay and separate effect handling. General bidirectional sync adds conflicts and lossy reverse mappings. The one-way form is reserved for demonstrated residual outcomes; it is not the default for compatible commands. |
| **4. Duplicate requests plus selective DB sync** | Mix Options 1 and 3: reproduce safe commands natively and synchronize uncovered production changes. | Exercises compatible commands while covering independent writers. | Requires operation provenance and one ordered protocol so native execution and copied changes do not both apply the same mutation. Selected architecture. Prove compatible native execution first; add selective DB sync only for outcomes that cannot be reproduced reliably. |

## Snapshot-only PoC

A disposable dcl-one instance could use data from a specific moment with no continuous synchronization. This is useful for UI/backend demonstrations and compatibility testing with clearly dated data. It cannot promise current production state or safely accept shared authoritative writes. It is not the selected live soft-launch strategy; external effects and jobs still need isolation.

The UI-only approach backed entirely by existing services was removed from consideration because it does not exercise the dcl-one backend deployment required by this project.

## Why not bidirectional writes now?

Some forward mappings merge identities or histories: friendship pairs, badge progress and world-storage namespaces are examples. Reversing them needs retained source provenance or restrictions on representable writes. Origin markers prevent loops but do not settle concurrent edits, delete-versus-update or delayed permission revocations. A universal latest-timestamp-wins rule is not acceptable for these invariants. One production authority avoids those conflicts during this phase.

## Capture mechanisms

- **Logical CDC:** remains a possible transport after deployed database version, privileges, replica identity, failover and retention are verified. It still needs transformations and idempotent application; a replication product does not supply domain semantics.
- **Local transactional triggers/outbox:** implemented for the bounded pilot. They record locally, including internal writes, then a separate worker reads asynchronously. Source locking and storage costs require measurement.
- **Cross-database write triggers:** rejected for this phase because remote availability becomes part of production transaction behavior. They do not remove schema or conflict work.
- **Periodic timestamp-based imports:** useful batch tooling, not sufficient proof of continuous correctness. Late commits and deletions can be missed, and separately extracted batches do not share a consistent snapshot.

## Why not have the gateway invent every operation ID?

A gateway-generated ID can deduplicate downstream retries of one accepted request, but a new client retry generally receives a new ID. It therefore cannot guarantee that the retry is the same logical operation. This may be acceptable for a separately tested set/unset command, but it is not a generic contract for resource creation, payments or effects. No gateway-generated-ID mode was selected or implemented; the retry-identity requirement must be resolved explicitly rather than choosing DB sync merely to avoid client work.

Longer coexistence makes these constraints more important: schema drift, retained history, operational ownership and restore behavior must be tested for the intended duration. See [operation backlog](operation-classification.md).

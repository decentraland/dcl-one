# Durable streaming notification recipients

Streaming key reset, revoke, expiry, timeout and place-removal notices must
reach every affected wallet while preserving independent safety cleanup.
Owner or permission services can be unavailable; world identities can retire
before catalogue derivation observes removal; bounded jobs can stop midway
through a recipient list.

## Decision

Use the existing cleanup and removal outboxes for normalized recipient arrays,
a resolution bit and a durable delivery cursor. Who is told follows the rule
main adopted in #1932 (`streaming_notifications::recipients`): the owner, the
LAND operators and managers or the World deployment/streaming grantees of the
place, and the scene admins of the key's scene (of every scene of the world for
a removal). A World scene admin is told only while the address that granted it
still holds the world-wide authority, so an admin granted by a parcel-scoped
grantee or another admin is never told, from a live lookup or a saved list.
This decision adds durability on top of that rule: recipients are resolved
once, saved, and delivered by cursor.

At removal-event creation the trigger freezes the exact world scene places
(`admin_keys`) and, for a Genesis place, the valid active admins of its base
parcel, through a plain MVCC read of Social grants without Social row locks or
writes. This keeps a pre-maintenance re-grant from erasing an earlier Genesis
recipient. World admins are not frozen as recipients because their granter must
be checked; instead each is frozen with its original granter (`admin_grants`),
and safety cleanup adds the pairs of the grants it ends. A re-grant overwrites
the row's granter and lifts its `removed_at`, and a retired scene no longer
lists its admins, so resolution reads the frozen pairs as well as the live and
ended grants, and checks every granter's world-wide authority. Key notices do
the same: preparation first saves the admins with their granters in the
notice's context, before the place context, binding or any external lookup.
When the scene place is not known yet it saves them for every retained or
active binding it weighs for the credential's World scene, and the retry keeps
those of the place it settles on. A deferred resolution reads them live only
when that first save failed. Notification-only enrichment runs after that
commit. Resolve every permission surface before marking the snapshot complete
(an unreadable surface retries, never shrinks the set), and checkpoint each
Social acceptance. Retries keep the event identity, original context and
complete recipient set. Social's existing recipient/kind/event deduplication
covers replay after acceptance but before the cursor write. A delivery failure
does not undo revocation. A reset whose delivery started is finished as a
reset, never converted to a revocation, so nobody is told twice.

Key cleanup bounds all notification preparation to five seconds and saves
partial recipients, exact keys and unresolved binding obligations. Runtime
binding reads use MVCC without publication locks; deferred resolution also
reads inactive credentials. Preparation captures the original place context
before binding recovery, and deferred resolution checkpoints newly obtained
context before further fallible lookups. Existing context snapshots always win
over live catalogue data, including after removal or reappearance.

Credentials save the exact bound scene admin namespace at issuance. Explore
keeps exact current identity keys in both catalogue legs before final scene
retirement when no backing scenes remain, and freezes them with active keys
into each removal event. The world row lock serializes publication; identity
locks precede catalogue writes. Actual scene insertion clears only catalogue
evidence, because an imported identity can precede its backing scene.

Explore also retains each displaced exact `(world_name, entity_id, identity)`
binding in `world_scene_binding`, in the transaction that replaces, retires or
deletes that identity. This preserves the namespace even if a legacy key's
binding lookup fails, revocation commits, and the scene retires before the
first deferred retry. Comms reads this evidence and the current binding; a
missing key resolves only when they identify exactly one namespace. A saved
credential/context key takes precedence. Reusing an entity under different
identities remains ambiguous, never inferred from parcel overlap. A successful
lookup with zero or several exact candidates records
`admin_binding_pending=false` and `admin_binding_unresolvable=true`. That notice
completes for independently established owner and permission holders plus any
previously captured recipients, and tells no scene admin of that World scene. SQL failures and timeouts remain pending and retryable;
they do not establish absence. The capture writes Explore alone and takes no
Social row locks.

## Consequences

Outbox storage grows with the number of affected wallets and is deleted after
completion. Historical feed seeding, and a place first seen already disabled
(an import adding a row neither catalogue leg held), suppress notices but
preserve safety cleanup. Existing scalar recipients are expanded rather than treated as a
complete set. Exact retired namespaces and overwritten grant history from
before this evidence existed cannot always be recovered. Complete snapshots
remain immutable across permission edits, scene re-adds and delivery retries.
Binding facts are retained without expiry because unresolved credentials and
cleanup intents can outlive scene publication. Their count grows with distinct
displaced bindings. The primary key supports exact lookup and idempotent
capture. The schema object pass installs the new table, function and trigger
on existing databases without rewriting credentials. Information destroyed
before capture was installed cannot be reconstructed.

Before exposing writers, a Social-writing serve process scans active native
credentials with missing admin keys in bounded UUID batches, decodes their
canonical rooms with the existing codec, and saves an exact live world identity
binding under world/identity locks only when it is the one identity across
current and retained bindings for that scene, as deferred recovery requires. Read-only Social replicas skip this write.
The upgrade requires stopping old writers and finishing offline imports before
the coordinated startup, so identities cannot retire before this backfill.
Noncanonical imported rooms and scenes already retired before the upgrade
remain outside exact recovery. A definitive missing or ambiguous canonical
binding uses the owner/permission-holder fallback above; failed authority
lookups remain retryable.

## Credential issuance: no limit today, revisit later

Stream key issuance (POST/PUT `/scene-stream-access` and GET
`/cast/generate-stream-link`) has no per-place limit, as on the pinned
upstream: every new key is minted and every reset is announced to all of the
place's managers. A limit of six new keys per place per hour, answered with
HTTP 429, was proposed in this change and removed before merge by the
requester's decision, to keep today's behaviour and the upstream contract.

Next work: now that each reset notifies every manager (and emails them), a
place whose key is reset repeatedly can flood its owner, operators, grantees
and admins with notices. Before introducing any limit:

- measure real issuance and reset rates per place in production;
- decide whether to bound issuance, to coalesce notices, or both;
- get explicit compatibility approval for any new 429 response on those
  routes, recorded in the conformance file, since clients do not expect it.

# Explicit video position requests

## Status

Accepted.

## Context

A VideoPlayer retains its last requested position when a scene changes playing,
volume or loop. Those changes must preserve playback position. Assigning the same
position again requests another seek, but the SDK suppresses identical serialized
component writes. After a pause, a restart and a resume can otherwise produce the
same component data.

## Decision

VideoPlayer carries an optional uint32 `position_revision` in field 10. The SDK's
VideoPlayer component increments it modulo 2^32 for each explicit, defined
position assignment or component creation/replacement containing a position.
Configuration changes retain the revision. The revision is ordinary component
state, serialized through the existing CRDT path with its existing timestamps and
conflict rules.

The Explorer seeks when a defined requested position changes or its revision
changes. Replaying an identical revision does not seek again. For components
without a revision, the Explorer preserves the legacy rule: a changed position or
an identical explicit component replacement requests a seek; configuration
changes retaining a position do not.

The SDK owns authoring intent, the protocol carries it, and native and web media
backends consume the same engine decision. The Admin Toolkit needs no changes.
The state and playback cursor are inspectable through the existing component
snapshot and media diagnostics.

## Consequences

Scenes must be rebuilt with the updated SDK to emit repeated mutable position
requests. Already published bundles retain their compiled SDK behavior; an engine
update cannot recover commands those bundles discard. Existing renderers,
including Unity, may ignore this additive field and retain their existing seek
behavior. This decision does not establish repeated-seek compatibility for those
renderers.

Mutation tracking is local to VideoPlayer and adds no retained per-entity history
outside the component. Multiple assignments before one flush coalesce into the
last requested position. Clearing a position does not request a seek. Revision
wrap is defined; an entire 2^32 assignments between observed updates can alias an
earlier revision.

Regression coverage checks repeated Restart, Restart after Pause, configuration
updates, explicit replacements, clearing/deletion, revision wrap and repeated
component-factory access. Runtime acceptance uses the unchanged Toolkit in a
deployed scene rebuilt with the updated SDK on native and web.

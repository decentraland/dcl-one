# Code style

`rustfmt` and `clippy -D warnings` settle formatting and lints. This page is about the shape of
the code. The one idea behind most of it: lean into the ECS. Bevy already gives us storage,
change tracking, ownership and cleanup. Code that reinvents those in a resource, a `Local` or a
side map is not allowed and will fail a review.

## State lives on the entity it describes

- A fact about an entity is a component on that entity. Not a `HashMap<Entity, T>` in a
  resource, not a parallel `Vec`, not a `Local` in the one system that happens to use it.
  Queries then do the filtering, `Changed<T>` and `Added<T>` do the dirty tracking, and
  despawn does the cleanup for free. Some legacy maps exist; do not add to them.
- Resources are for genuine singletons: configuration, the scene loop, a cache, a connection.
- A boolean field that systems branch on is usually a marker component. `With<Playing>` and
  `Without<Playing>` split the work across two queries and make the state visible in the
  inspector; `if thing.playing` hides it.
- One-shot facts are events. State is components. Bevy `States` are reserved for the handful
  of platform toggles (microphone, transmitter, parcel grass).
- Scene data arrives as proto `Pb*` types and becomes an engine component through `TryFrom`.
  Keep the two apart: the proto type is the wire shape, the component is what systems read.
  Never redeclare a type whose shape already exists in the protocol or SDK crates; import it.

## Cleanup is structural

- Scene entities are children of their scene root. Despawning the root takes everything with
  it, and `DeletedSceneEntities` tells the loop what died this pass. Anything you spawn for a
  scene entity hangs off it, so it dies with it.
- Where parenting isn't the right relationship, tag the spawned entity with
  `DespawnWith(owner)` from `common::util` and it is despawned once the owner is gone. The
  render world has no such sync for entities spawned there, so it uses the same shape with its
  own reaper (boimp's `ImposterSubview`). Never despawn-everything-then-respawn each frame:
  anything that read the parent's child list between the two steps sees stale ids and panics
  on apply.
- Bevy UI entities are the exception to "hang it off the scene entity". There is no one-to-one
  mapping: a scene entity with a `UiTransform` gets a separate bevy-ui node, linked both ways
  (`UiLink` on the scene entity, `LinkedScene` on the node), and the two trees keep their own
  hierarchies. The ui tree lives under the scene's `SceneUiRoot`, which is `DespawnWith` the
  scene root, and its parenting is rebuilt from the scene's `UiTransform` parents each update,
  not from the scene entity tree. Anything added to scene UI follows that shape: link, don't
  parent, and let the root's `DespawnWith` do the teardown.
- Never keep a channel end in a `Local`. A `Local` outlives the scene that handed it over and
  keeps that scene's state alive, which is exactly the parked-future leak that moved stream
  senders onto the scene root as components. The same applies to anything a scene owns: put
  it on the scene entity, where its lifetime is already correct.

## Explicit updates over clever ordering

- If bevy's own system has already consumed an event this frame, set the affected state
  directly at the point of change. Don't run a second copy of the bevy system, don't reorder
  schedules to sneak in ahead of it. If the field isn't writable, a small fork change that
  matches upstream visibility is fine.
- Before writing a priority pick, a merge, or a sort over entities, look for the system that
  already does it (`ActivePlayerComponent` is the usual one) and compose with its output. A
  hand-rolled copy is more code and a worse version of the original.

## Async work

- IO goes on `IoTaskPool`; the result comes back through a channel or a `Task<T>` polled from
  a system, and lands in a component. Systems never block.
- The worker that knows the outcome reports it, per item and structured: `Vec<Result<T, E>>`
  in request order with a small named error enum. The caller relays; it does not re-derive
  failures by diffing what it asked for against what came back.
- Cancellation tokens are threaded through outer layers unchanged. The `select!` sits at the
  one await that can actually hang (the network read, the lock), not around the whole task.
- On web there is no `Send` and tasks run on the JS event loop. See the platforms page.

## Errors and invariants

- Scene data is untrusted and malformed scene data must never take the client down. Systems
  handling it warn and skip; they do not `unwrap` or `panic`. The `dcl_assert!` macro is for
  engine invariants and compiles out of release builds unless the `dcl-assert` feature is on.
- `anyhow` at boundaries (ops, loaders, fetches). A named error enum where a caller branches
  on the kind.
- When one variant of a `match` is filtered by a flag, the guard goes inside that arm, not in
  front of the `match`. The reader should see which arm is affected without re-deriving it.

## Logging

- `tracing` macros. `debug!` for per-frame detail, `info!` for lifecycle, `warn!` for a
  recoverable problem a scene caused, `error!` for something that is our bug.
- Filter directives match the target by string prefix, so `av=debug` also enables `avatar`.
  Use the shortest prefix that covers a crate family rather than listing crates.

## The HUD

`react-web/` has its own rules in `react-web/AGENTS.md` and `react-web/review.md`: the design
system tokens and primitives, the test tiers, and the pre-merge checklist. Those take
precedence over this page inside that directory.

# Presence contexts

`GlobalCrdtState` is a component: each context owns its CRDT store,
transport-facing channel, scene-facing broadcast, address-to-entity lookup and
player-ID allocator. Preserve these ownership boundaries when changing presence.

- The client has one shared context from `GlobalCrdtPlugin`. Client transports
  and scenes resolve to that context, preserving the shared nearby-player view.
- An orchestrated server has one context per scene room. Reuse it across adapter
  re-mints and dispose of its player entities when the scene is removed.
- Bind a transport to its context at connection time through
  `AdapterManager::connect(adapter, context)`. Resolve a scene's context through
  `CrdtContexts::for_scene_hash`.
- A `ForeignPlayer` belongs to one context. A wallet present in two rooms has
  separate entities and player IDs; leaving one room leaves the other intact.
- Player queries and connection/disconnection subscriptions use the calling
  scene's context. Room-scoped scenes use membership; client scenes retain
  positional semantics.
- Validate inbound scene messages against the bound context's room identity.
  Preserve outbound room selection through the scene's adapter.
- Keep IPC broadcasts scoped to their context. Scene initialization must pair
  its snapshot and subsequent updates without losing or crossing context data.
  Remove receivers and sidecar channels when their context has no consumers.

Verify these contracts at the target revision with client and orchestrated-server
tests. Route security findings, historical incidents and acceptance evidence
through the authorized private review and Notion workflow.

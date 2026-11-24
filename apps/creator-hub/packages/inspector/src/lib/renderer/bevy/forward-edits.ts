import type { ComponentDefinition, Entity } from '@dcl/ecs';
import { CrdtMessageType } from '@dcl/ecs';
import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer';

import type { EngineWindow } from './console';
import { consoleCommand } from './console';
import type { BevySceneContext } from './BevySceneContext';
import type { EngineSceneState } from './engine-scene-state';
import {
  SCENE_LIGHTING_COMPONENTS,
  emptyEngineSceneState,
  findPlayerAttachments,
  parseEngineSceneState,
} from './engine-scene-state';

/**
 * Forward edit bridge: inspector ECS changes → engine console commands.
 *
 * The bevy engine renders a scene it loaded from the realm; that scene is static
 * once loaded, with **no live CRDT channel back in**. The only way to mutate the
 * running scene is the engine's console (`set_component` / `delete_component` /
 * `delete_entity`) — exactly how bevy-editor edits (see its `writeComponent`).
 *
 * So we translate the inspector's authoritative ECS changes into those commands:
 * subscribe to the context's change stream and, for each supported component,
 * push a `set_component`. This is the forward half of editing; the reverse
 * channel (viewport pick / gizmo → inspector) is a separate slice.
 *
 * Scope (first slice): **engine-managed components addressed by short name**,
 * starting with Transform — the one moving/rotating/scaling produces, enough to
 * prove edits show live in Bevy. Custom/schema components (`core-schema::Name`,
 * `asset-packs::…`) need `set_component_raw` by numeric id and are deferred; we
 * skip anything not in the engine-name map rather than mis-send it as
 * `set_component` (which the engine would reject).
 */

/**
 * Map from an `@dcl/ecs` `componentName` to the engine's console component name.
 * `@dcl/ecs` reports engine-managed components as `core::Transform`; the engine's
 * `set_component` addresses them by the bare `Transform` (its ComponentNameRegistry
 * registers each proto component by that PascalCase name). Only components listed
 * here are forwarded; anything else (asset-packs::, core-schema:: like Name, and
 * other custom/schema components) is skipped rather than mis-sent as `set_component`
 * (the engine would reject an unknown name).
 *
 * These are the CORE VISUAL components — enough for a dropped/added model to appear
 * live in the engine (writing GltfContainer to a fresh entity id creates the entity
 * in the CRDT store and the engine's gltf loader renders it) without a full reload.
 * `Name` is intentionally omitted: it's `core-schema::Name` (a schema component the
 * registry doesn't expose by name), and it's only the tree label — the model renders
 * without it.
 */
const ENGINE_COMPONENT_NAMES: Record<string, string> = {
  'core::Transform': 'Transform',
  'core::GltfContainer': 'GltfContainer',
  'core::MeshRenderer': 'MeshRenderer',
  'core::MeshCollider': 'MeshCollider',
  'core::Material': 'Material',
  'core::VisibilityComponent': 'VisibilityComponent',
  'core::Billboard': 'Billboard',
  'core::TextShape': 'TextShape',
  'core::Animator': 'Animator',
  'core::VideoPlayer': 'VideoPlayer',
  'core::ParticleSystem': 'ParticleSystem',
  'core::LightSource': 'LightSource',
  'core::NftShape': 'NftShape',
  'core::GltfNodeModifiers': 'GltfNodeModifiers',
  'core::AvatarAttach': 'AvatarAttach',
  'core::AvatarModifierArea': 'AvatarModifierArea',
  'core::CameraModeArea': 'CameraModeArea',
  'core::VirtualCamera': 'VirtualCamera',
  'core::AudioSource': 'AudioSource',
  'core::AudioStream': 'AudioStream',
};

// PBParticleSystem.PlaybackState: PS_PLAYING=0, PS_PAUSED=1 ("simulation frozen; no
// new particles emitted"), PS_STOPPED=2. Used to freeze emission on scene pause.
const PARTICLE_PLAYBACK_PAUSED = 1;

// Pointer-collision layer bit (ColliderLayer.CL_POINTER). The editor forces it on so
// every visible mesh is clickable in the viewport (matching Babylon).
const CL_POINTER = 1;

/**
 * Convert an `@dcl/ecs` component value to the JSON shape the engine's
 * `set_component` accepts (#1466). The engine parses the value as serde-proto,
 * where a protobuf `oneof` is an externally-tagged enum — the variant name is the
 * single key, e.g. `{ mesh: { box: {...} } }`. `@dcl/ecs` (ts-proto) instead
 * tags oneofs with a `$case` discriminator: `{ mesh: { $case: 'box', box: {...} } }`.
 * serde rejects that extra key, so a MeshRenderer/Material/MeshCollider edit never
 * applied live (only after a reload, which loads from the composite, not
 * set_component). Recursively collapse each `{$case, [case]}` to `{ [case]: ... }`;
 * a value with no oneof passes through unchanged.
 */
function toEngineJson(value: unknown, seen: WeakSet<object> = new WeakSet()): unknown {
  if (value && typeof value === 'object') {
    // Cycle guard: schema-decoded component values are trees today, but a back-
    // reference would otherwise recurse forever. Track the current path (add on
    // descent, remove on unwind) so a repeated sibling in a DAG isn't mistaken for a
    // cycle; drop a true back-reference rather than loop.
    if (seen.has(value)) return undefined;
    seen.add(value);
    let result: unknown;
    if (Array.isArray(value)) {
      result = value.map(v => toEngineJson(v, seen));
    } else {
      const obj = value as Record<string, unknown>;
      if (typeof obj.$case === 'string') {
        result = { [obj.$case]: toEngineJson(obj[obj.$case], seen) };
      } else {
        const out: Record<string, unknown> = {};
        for (const [key, v] of Object.entries(obj)) out[key] = toEngineJson(v, seen);
        result = out;
      }
    }
    seen.delete(value);
    return result;
  }
  return value;
}

export interface ForwardEditBridgeOptions {
  /**
   * The renderer's scene context — the source of ECS changes to forward, plus the
   * `Name` component (to instantiate a new entity via `/new_entity`; see below).
   */
  context: Pick<
    BevySceneContext,
    'onChange' | 'Name' | 'Transform' | 'getForwardableComponent' | 'editorComponents' | 'engine'
  >;
  /** The live engine window to send console commands to. */
  engineWindow: EngineWindow;
  /**
   * Test seam: send a console command. Defaults to the real `consoleCommand`
   * against `engineWindow`. Tests inject a recorder.
   */
  send?: (cmd: string, args: string[]) => Promise<string>;
  /** Test seam: how a failed forward command is reported. */
  onError?: (context: string, error: unknown) => void;
  /**
   * Gate: forward a change only when this returns true. Defaults to a settle
   * window (see {@link SETTLE_QUIET_MS}). Tests pass `() => true` to forward
   * immediately.
   */
  shouldForward?: () => boolean;
  /**
   * Whether the scene is currently frozen (paused). Read when forwarding arms so
   * animations that loaded playing are paused to match the editor's default-frozen
   * state (#1382). Defaults to frozen (the editor boots static).
   */
  isFrozen?: () => boolean;
  /**
   * Whether the scene is frozen at its initial state: since it loaded or was
   * stopped, before any Play. While stopped the viewport keeps the editor's
   * lighting (see applySceneLighting) and shows editor Placeholders; a Pause is
   * frozen but not stopped, so it keeps what Play showed. Defaults to `isFrozen`.
   */
  isStopped?: () => boolean;
  /**
   * Whether entities attached to the local player or its camera should be hidden
   * (see `findPlayerAttachments`: a Transform parent chain reaching
   * `engine.CameraEntity` or `engine.PlayerEntity`, or an AvatarAttach on the
   * local player, and their descendants). While you edit with the free camera,
   * the engine's camera entity IS the editor's fly-camera, so anything the scene
   * parents to it would hang in front of the editor's view, and the player's
   * avatar is hidden, so what rides on it would float at the spawn point. Hiding
   * also switches off a LightSource on them: the engine lights only entities that
   * are visible. Defaults to never.
   */
  hideCameraAttachments?: () => boolean;
}

/**
 * The engine loads the whole scene from the realm on boot, so the inspector's
 * initial CRDT sync is a large burst of PUT_COMPONENTs the engine ALREADY has —
 * replaying them as `set_component` is redundant, floods the console, and (with
 * no scene pinned yet) every one is rejected "player is not in any scene",
 * wedging boot. bevy-editor avoids this by only forwarding explicit *user* edits,
 * never the initial snapshot. We have no per-change "user vs load" signal, so we
 * arm forwarding only after this delay from bridge start — the initial load
 * arrives immediately on connect; genuine user edits come much later.
 */
const ARM_DELAY_MS = 3000;

/**
 * Wire the inspector's ECS changes to engine console writes. Returns a
 * disconnect fn.
 *
 * Engine writes are fire-and-forget async; a failed command is logged (via
 * `onError`) but never throws into the ECS change loop — a dropped edit must not
 * wedge the inspector.
 */
export interface ForwardEditBridge {
  /** Tear down the change subscription + arm timer. */
  disconnect(): void;
  /** Apply the scene run/freeze toggle to what the engine renders: pause
   * (frozen=true) or resume (false) time-based playback by forwarding each
   * Animator/video/audio/particle component with its paused override or the
   * authored value (#1382). Play also swaps editor Placeholders for what the scene
   * has, so it shows the scene as players will see it, and a Pause keeps that. */
  setSceneFrozen(frozen: boolean): void;
  /** Run the day clock again when the scene is running. Call once the editor agent
   * is ready: its boot pins the clock at noon, also under a scene that is playing
   * (an engine reboot while playing, or a Play pressed while the agent logged in). */
  syncSceneClock(): void;
  /** Re-evaluate which camera-attached entities are hidden (see
   * `hideCameraAttachments`). Call when its inputs change: the camera mode or the
   * run state. */
  refreshCameraAttachments(): void;
  /** Re-read the engine's copy of the scene (entities its code created, its root
   * lighting) while it is stopped. Call when the engine's entity set changes, e.g.
   * once the scene's `main()` has run after load. */
  refreshEngineScene(): void;
  /** A scene reload (Stop, or a reload before Play) was just requested. Edits made
   * from here until `reconcileAfterReload` are not in the bundle it reloads. */
  noteReloadStarted(): void;
  /** Re-establish editor overrides after a scene reload (Stop/#1376), which
   * re-creates the engine entities: marks existing entities instantiated (no
   * re-`new_entity` → no id collision) and replays placeholder/visibility/pickable/
   * animation overrides so they survive the reload. */
  reconcileAfterReload(): void;
}

export function createForwardEditBridge(options: ForwardEditBridgeOptions): ForwardEditBridge {
  const { context, engineWindow } = options;
  // Default frozen — the editor boots static (see setSceneFrozen / #1382).
  const isFrozen = options.isFrozen ?? (() => true);
  const isStopped = options.isStopped ?? isFrozen;
  const hideCameraAttachments = options.hideCameraAttachments ?? (() => false);
  // Guard every send behind a disposed flag: once the bridge is disconnected (a
  // Bevy iframe reboot swaps `engineWindow`, or the renderer unmounts), queued
  // tasks that are still draining must NOT keep posting to the now-stale window.
  let disposed = false;
  const baseSend =
    options.send ?? ((cmd: string, args: string[]) => consoleCommand(engineWindow, cmd, args));
  const send = (cmd: string, args: string[]): Promise<string> =>
    disposed ? Promise.resolve('') : baseSend(cmd, args);
  const onError =
    options.onError ??
    ((ctx: string, error: unknown) => {
      console.warn(`[bevy] forward edit failed (${ctx}):`, error);
    });

  // The load burst is suppressed (shouldForward === false), so the editor's
  // component-derived overrides never reach the engine on load. Once forwarding
  // arms, replay them for every entity whose engine render must differ from what
  // it loaded:
  //  - editor `Hide` set  → force invisible (else it'd load visible)
  //  - authored VisibilityComponent{visible:false} → force VISIBLE in the editor
  //    (#1377; else it'd load invisible and be unselectable)
  //  - editor `Placeholder` → render its GLTF (#1372; a model-less item like a
  //    Trigger Area is otherwise invisible in the editor)
  // The forward* helpers compute the right value for each; we just touch every
  // candidate entity once.
  const reconcileEditorOverridesOnArm = () => {
    const Hide = context.editorComponents.Hide;
    const seen = new Set<Entity>();
    for (const [entity, hide] of context.engine.getEntitiesWith(Hide)) {
      if ((hide as { value?: boolean } | undefined)?.value && !seen.has(entity)) {
        seen.add(entity);
        forwardEditorVisibility(entity);
      }
    }
    const visComp = context.getForwardableComponent('core::VisibilityComponent');
    if (visComp) {
      for (const [entity, v] of context.engine.getEntitiesWith(visComp)) {
        if ((v as { visible?: boolean } | undefined)?.visible === false && !seen.has(entity)) {
          seen.add(entity);
          forwardEditorVisibility(entity);
        }
      }
    }
    // A reloaded or rebooted engine has no placeholder GLTFs, so a scene that is
    // running or paused needs nothing removed.
    placeholdersShown = false;
    if (isStopped()) {
      for (const [entity] of context.engine.getEntitiesWith(context.editorComponents.Placeholder)) {
        forwardPlaceholder(entity, false);
      }
    }
    // The reload also dropped the forwarded visibility of camera attachments.
    refreshCameraAttachments(true);
    // #1373: re-forward each loaded GltfContainer so forwardSet re-applies the
    // editor's pointer-pickable mask — a model saved without a pointer collider is
    // otherwise unclickable after a reload (the load burst that carried its
    // authored mask was suppressed).
    const gltf = context.getForwardableComponent(GLTF_CONTAINER);
    if (gltf) {
      for (const [entity, current] of context.engine.getEntitiesWith(gltf)) {
        // The placeholder forwarded above stands in for this model while frozen.
        if (shownPlaceholderSrc(entity) != null) continue;
        enqueue(entity, async () => {
          await ensureInstantiated(entity);
          await forwardSet(entity, 'GltfContainer', current);
        });
      }
    }
    // Re-forward each MeshRenderer primitive + its editor pointer collider so it
    // stays clickable after a reload (the load burst that would carry it was
    // suppressed) — the MeshRenderer counterpart of the GltfContainer replay above.
    const meshRenderer = context.getForwardableComponent('core::MeshRenderer');
    if (meshRenderer) {
      for (const [entity, current] of context.engine.getEntitiesWith(meshRenderer)) {
        enqueue(entity, async () => {
          await ensureInstantiated(entity);
          await forwardSet(entity, 'MeshRenderer', current);
        });
        forwardMeshPickCollider(entity, current);
      }
    }
    // #1382: the editor boots frozen, but a scene loads with its Animator clips
    // playing — pause them to match the frozen state (setPlaybackFrozen forwards
    // playing:false; the toolbar toggle later resumes/re-pauses).
    if (isFrozen()) setPlaybackFrozen(true);
    // The scene's code ran again from the saved composite: from here on, a parent
    // or AvatarAttach that differs from these is an edit made in the inspector.
    captureLoadedAttachments();
    // What the scene's code made in main() (camera attachments, root lighting) is
    // only in the engine. The first load and a Stop both land here, so both keep
    // the editor's lighting and hide the attachments the same way.
    void scanEngineScene();
    // A Play pressed before forwarding armed never reached the clock.
    syncSceneClock();
  };

  // Arm after a delay so the initial CRDT load burst is not forwarded (see
  // ARM_DELAY_MS). Overridable for tests.
  let armed = false;
  const armTimer = options.shouldForward
    ? null
    : setTimeout(() => {
        armed = true;
        // Everything the load burst brought in exists in the engine already (it
        // loaded the same composite), so the arm replays must not `/new_entity` it —
        // that collides ("id N already live"), same as after a reload.
        for (const [entity] of context.engine.getEntitiesWith(context.Name)) {
          instantiated.add(entity);
        }
        reconcileEditorOverridesOnArm();
      }, ARM_DELAY_MS);
  const shouldForward = options.shouldForward ?? (() => armed);

  const fire = (label: string, cmd: string, args: string[]) => {
    void send(cmd, args).catch(error => onError(label, error));
  };

  // Entities we've instantiated in the engine scene (via `/new_entity`). A NEW
  // entity (e.g. a dropped asset) doesn't exist in the engine's scene, so writing
  // its engine components (Transform/GltfContainer) is rejected as "missing
  // entity" — the model never appears. We detect a new entity by its `Name` PUT
  // (asset-add always writes Name; editing an existing entity never does),
  // instantiate it, then (re)send its engine components. A per-entity serial chain
  // keeps operations ordered (instantiate → set_component → …).
  const instantiated = new Set<Entity>();
  const queues = new Map<Entity, Promise<unknown>>();
  const enqueue = (entity: Entity, task: () => Promise<void>) => {
    const prev = queues.get(entity) ?? Promise.resolve();
    const next: Promise<unknown> = prev.then(task).catch(() => {});
    // Prune the entry once this chain settles IF it's still the tail — otherwise
    // `queues` grows one (resolved-Promise) entry per entity ever touched and is
    // never reclaimed over a long editing session.
    void next.then(() => {
      if (queues.get(entity) === next) queues.delete(entity);
    });
    queues.set(entity, next);
  };

  const NAME_COMPONENT = context.Name.componentName; // 'core-schema::Name'

  // What visibility the ENGINE should render for an entity in the editor, which
  // is NOT the entity's authored VisibilityComponent. Two editor rules override
  // the scene's runtime visibility, both mirroring the Babylon editor:
  //
  //  1. Editor `Hide` (the tree eye-icon) — an inspector-only component with no
  //     engine counterpart. When Hide is true the entity is invisible in the
  //     viewport (Babylon: setEnabled(false)).
  //  2. Authored `VisibilityComponent{visible:false}` (#1377) — a runtime concern
  //     the editor IGNORES so the entity stays visible/selectable while editing
  //     (Babylon has no VisibilityComponent handler at all, so it never hides).
  //     Preview/live still hide it: the CRDT keeps the authored value; we only
  //     override what the ENGINE renders.
  //
  // A third rule hides camera attachments while editing with the free camera (see
  // refreshCameraAttachments). So the effective editor visibility is: visible
  // unless Hide is set or it is a hidden camera attachment. Forward that as the
  // engine's VisibilityComponent. Existing scene entities
  // already live in the engine, so we never `/new_entity` here — ensureInstantiated
  // only matters for a NEW entity edited before its model instantiates (a no-op
  // for entities the engine already loaded).
  // Decided inside the queued task, after any runtime-visibility capture queued
  // ahead of it has landed (see captureRuntimeVisibility).
  const forwardEditorVisibility = (entity: Entity) => {
    enqueue(entity, async () => {
      const hideValue = context.editorComponents.Hide.getOrNull(entity) as
        | { value?: boolean }
        | null
        | undefined;
      const hidden = !!hideValue?.value || hiddenCameraAttachments.has(entity);
      const runtime = runtimeVisibility.get(entity);
      if (!hidden) runtimeVisibility.delete(entity);
      await ensureInstantiated(entity);
      await forwardSet(entity, 'VisibilityComponent', { visible: !hidden && (runtime ?? true) });
    });
  };

  // Editor `Placeholder` (#1372) — an inspector-only component `{ src }` holding a
  // GLTF that stands in for an otherwise-invisible item (e.g. a Trigger Area) so
  // it can be seen/selected in the editor but NOT in preview. The engine has no
  // Placeholder component, so translate it to a `GltfContainer` pointed at the
  // placeholder src. Editor-only: the CRDT keeps just the Placeholder component,
  // so preview (which ignores editor components) never shows it — and neither
  // does Play or a Pause, which keeps what Play showed: the placeholder shows only
  // while the scene is stopped.
  //
  // An entity can also carry an AUTHORED GltfContainer: the invisible smart items
  // pair a collider-only model (nothing to see, so nothing to click) with a
  // Placeholder. The engine holds one GltfContainer per entity, so while stopped
  // the placeholder replaces the authored model, and Play forwards the authored
  // model back, colliders included. A Pause changes nothing: the invisible wall
  // stays invisible and keeps blocking the player.
  const GLTF_CONTAINER = 'core::GltfContainer';
  // Whether the engine may hold placeholder GLTFs that the next Play must swap out.
  let placeholdersShown = false;
  const shownPlaceholderSrc = (entity: Entity, stopped = isStopped()): string | null => {
    if (!stopped) return null;
    const placeholder = context.editorComponents.Placeholder.getOrNull(entity) as {
      src?: string;
    } | null;
    return placeholder?.src || null;
  };
  const forwardPlaceholder = (entity: Entity, deleted: boolean, stopped = isStopped()) => {
    const src = deleted ? null : shownPlaceholderSrc(entity, stopped);
    if (src) placeholdersShown = true;
    enqueue(entity, async () => {
      await ensureInstantiated(entity);
      const authored = (
        context.getForwardableComponent(GLTF_CONTAINER) as {
          getOrNull?: (e: Entity) => unknown;
        } | null
      )?.getOrNull?.(entity);
      if (src) {
        await forwardSet(entity, 'GltfContainer', { src });
      } else if (authored != null) {
        await forwardSet(entity, 'GltfContainer', authored);
      } else {
        fire(`delete_component ${entity} GltfContainer`, 'delete_component', [
          String(entity),
          'GltfContainer',
        ]);
      }
    });
  };

  // Camera attachments: entities that follow the local player or its camera (see
  // findPlayerAttachments). While hidden (hideCameraAttachments), each one is
  // forwarded invisible through forwardEditorVisibility, which also switches off
  // its LightSource. Hiding, rather than dropping the attachment, never leaves a
  // light or model behind at the entity's own Transform. Every entity in the chain
  // gets its own value because the engine only inherits a parent's visibility
  // when that parent opts in with propagate_to_children.
  //
  // The editor's ECS only holds the authored composite; entities the scene's code
  // creates (a flashlight parented to the camera in main()) exist only in the
  // engine, so their parents and attachments come from its snapshot
  // (engineScene, refreshed by scanEngineScene). The scene's code can also attach
  // an authored entity (main() parents a torch from the composite to the camera),
  // so where both know an entity the engine's data is used, unless the entity's
  // parent or AvatarAttach was edited in the inspector since the scene loaded:
  // the frozen snapshot does not show that edit yet (loadedAttachments).
  let hiddenCameraAttachments = new Set<Entity>();
  // Each entity's Transform parent as of the last scan, so a Transform write that
  // keeps its parent (a move) skips the rescan. Empty while not hiding.
  let knownParents = new Map<Entity, Entity | undefined>();
  let engineScene: EngineSceneState = emptyEngineSceneState();
  const AVATAR_ATTACH = 'core::AvatarAttach';
  const isEditorEntity = (entity: Entity) =>
    context.Name.getOrNull(entity) != null || context.Transform.getOrNull(entity) != null;
  const editorLocalAvatarAttached = (): Set<Entity> => {
    const attached = new Set<Entity>();
    const avatarAttach = context.getForwardableComponent(AVATAR_ATTACH);
    if (!avatarAttach) return attached;
    for (const [entity, value] of context.engine.getEntitiesWith(avatarAttach)) {
      if ((value as { avatarId?: string } | null)?.avatarId == null) attached.add(entity);
    }
    return attached;
  };
  // The editor's parents and local AvatarAttach entities as of the last scene load.
  let loadedAttachments = {
    parents: new Map<Entity, Entity | undefined>(),
    localAvatarAttached: new Set<Entity>(),
  };
  const captureLoadedAttachments = () => {
    const parents = new Map<Entity, Entity | undefined>();
    for (const [entity, transform] of context.engine.getEntitiesWith(context.Transform)) {
      parents.set(entity, transform.parent);
    }
    loadedAttachments = { parents, localAvatarAttached: editorLocalAvatarAttached() };
  };
  const findCameraAttachments = (): Set<Entity> => {
    const parents = new Map<Entity, Entity | undefined>();
    knownParents = parents;
    if (!hideCameraAttachments()) return new Set();
    for (const [entity, parent] of engineScene.parents) parents.set(entity, parent);
    for (const [entity, transform] of context.engine.getEntitiesWith(context.Transform)) {
      const edited =
        !loadedAttachments.parents.has(entity) ||
        loadedAttachments.parents.get(entity) !== transform.parent;
      if (edited || !engineScene.parents.has(entity)) parents.set(entity, transform.parent);
    }
    const editorAttached = editorLocalAvatarAttached();
    const localAvatarAttached = new Set<Entity>(editorAttached);
    for (const entity of engineScene.localAvatarAttached) {
      const edited =
        loadedAttachments.localAvatarAttached.has(entity) !== editorAttached.has(entity);
      if (!isEditorEntity(entity) || !edited) localAvatarAttached.add(entity);
    }
    return findPlayerAttachments(
      parents,
      localAvatarAttached,
      new Set([context.engine.CameraEntity, context.engine.PlayerEntity]),
    );
  };
  // Forward visibility for every entity whose camera-attachment state changed, or
  // for every current attachment when `all` is set (after a reload).
  function refreshCameraAttachments(all = false): void {
    const previous = hiddenCameraAttachments;
    hiddenCameraAttachments = findCameraAttachments();
    for (const entity of hiddenCameraAttachments) {
      if (all || !previous.has(entity)) {
        if (!previous.has(entity)) captureRuntimeVisibility(entity);
        forwardEditorVisibility(entity);
      }
    }
    for (const entity of previous) {
      if (!hiddenCameraAttachments.has(entity)) forwardEditorVisibility(entity);
    }
  }
  const parentChanged = (entity: Entity, op: CrdtMessageType, value: unknown): boolean => {
    if (op === CrdtMessageType.DELETE_COMPONENT || !knownParents.has(entity)) return true;
    const transform = (value ?? context.Transform.getOrNull(entity)) as {
      parent?: Entity;
    } | null;
    return knownParents.get(entity) !== transform?.parent;
  };

  // Attachments are hidden only while the scene is stopped, and its main() may have
  // given them a visibility: the snapshot that found them holds that value, so Play
  // restores it instead of forcing the entity visible. An entity only the engine
  // knows was made by the scene's code, so its snapshot value is the scene's. For
  // an authored entity the snapshot value counts only when it differs from the
  // composite's: main() changed it (a flashlight that starts off). When they match,
  // the editor's rule stands (#1377: visible while editing).
  const runtimeVisibility = new Map<Entity, boolean>();
  const authoredVisibility = (entity: Entity): boolean => {
    const component = context.getForwardableComponent('core::VisibilityComponent') as {
      getOrNull?: (e: Entity) => unknown;
    } | null;
    const value = component?.getOrNull?.(entity) as { visible?: boolean } | null | undefined;
    return value?.visible !== false;
  };
  const captureRuntimeVisibility = (entity: Entity) => {
    const visible = engineScene.visibility.get(entity) ?? true;
    if (isEditorEntity(entity)) {
      if (visible !== authoredVisibility(entity)) runtimeVisibility.set(entity, visible);
      return;
    }
    if (engineScene.visibility.has(entity)) runtimeVisibility.set(entity, visible);
  };

  const forgetEntity = (entity: Entity) => {
    instantiated.delete(entity);
    hiddenCameraAttachments.delete(entity);
    runtimeVisibility.delete(entity);
  };

  // Scene lighting (SCENE_LIGHTING_COMPONENTS on the root entity): while the scene
  // is stopped the viewport keeps the editor's lighting, so a night scene stays
  // editable; Play shows the scene's own, and a Pause keeps it. Stopped: remove
  // each from the engine's copy and pin the editor clock at noon. Play: put back
  // exactly what the scene has (a configured value, a disabled LightSource as
  // disabled, and nothing for a component it never set) and run the clock again,
  // which hands the time of day back to the scene's SkyboxTime or scene.json fixed
  // time. The engine's `/time` with speed 0 overrides those, so the pin needs
  // nothing else. Pause holds the clock at the time the sky shows (see
  // holdSceneClock) and leaves the scene's lighting in place.
  //
  // The scene's values come from the engine snapshot (scene code writes most of
  // them in main()). A root lighting component edited in the inspector since load
  // uses the editor's value, or stays unset if it was removed: such an edit is
  // saved at once, but while stopped it only reaches the viewport on Play.
  const ROOT_ENTITY = 0 as Entity;
  const EDITOR_CLOCK = ['12', '0'];
  // Each edited component with the serial of its last edit. A reload clears the
  // marks of edits made before it started: they were saved by then, so the
  // reloaded scene starts from them and its code may set its own on top. An edit
  // made while the reload runs is not in the bundle it reloads, so it keeps its mark.
  const editedRootLighting = new Map<string, number>();
  let lightingEditSerial = 0;
  let reloadStartSerial: number | null = null;
  const isRootLighting = (entity: Entity, engineName: string) =>
    entity === ROOT_ENTITY && SCENE_LIGHTING_COMPONENTS.includes(engineName);
  const sceneLightingValue = (engineName: string): unknown => {
    if (editedRootLighting.has(engineName)) {
      const component = context.getForwardableComponent(`core::${engineName}`) as {
        getOrNull?: (e: Entity) => unknown;
      } | null;
      return component?.getOrNull?.(ROOT_ENTITY) ?? null;
    }
    return engineScene.rootLighting.get(engineName) ?? null;
  };
  let editorLightingShown = false;
  const applySceneLighting = (frozen: boolean) => {
    editorLightingShown = frozen;
    enqueue(ROOT_ENTITY, async () => {
      for (const engineName of SCENE_LIGHTING_COMPONENTS) {
        const value = sceneLightingValue(engineName);
        if (frozen) {
          if (value == null && !engineScene.rootLighting.has(engineName)) continue;
          try {
            await send('delete_component', [String(ROOT_ENTITY), engineName]);
          } catch (error) {
            onError(`delete_component ${ROOT_ENTITY} ${engineName}`, error);
          }
        } else if (value != null) {
          await forwardSet(ROOT_ENTITY, engineName, value);
        }
      }
      try {
        await send('time', frozen ? EDITOR_CLOCK : ['--resume']);
      } catch (error) {
        onError('time', error);
      }
    });
  };

  // Play from a Pause: the scene's lighting is still in place (the running scene
  // may have changed it since the last scan, so re-forwarding the scanned values
  // would undo that); only the clock runs again.
  const resumeSceneClock = () => {
    enqueue(ROOT_ENTITY, async () => {
      try {
        await send('time', ['--resume']);
      } catch (error) {
        onError('time', error);
      }
    });
  };
  // The agent's boot pins the clock at noon (pinSkyboxTime). When that lands under
  // a scene that is already running, nothing else would start the clock again.
  const syncSceneClock = () => {
    if (isStopped() || isFrozen()) return;
    editorLightingShown = false;
    resumeSceneClock();
  };
  // Pause: stop the day cycle where the sky is. `/time` with no arguments replies
  // with the current time in seconds (`elapsed: <s>`), and speed 0 holds it there.
  // That also overrides a scene's own fixed time, with the same value, until Play
  // resumes the clock and hands the sky back to the scene.
  const holdSceneClock = () => {
    enqueue(ROOT_ENTITY, async () => {
      try {
        const reply = await send('time', []);
        const seconds = Number(/elapsed: (-?[\d.]+)/.exec(reply)?.[1]);
        if (!Number.isFinite(seconds)) return;
        await send('time', [String(seconds / 3600), '0']);
      } catch (error) {
        onError('time', error);
      }
    });
  };

  // Re-read the engine's copy of the scene while it is stopped: the entities its
  // code made that follow the player or camera, and its root lighting. A frozen
  // scene's snapshot is its last tick, never the editor overrides queued since,
  // so it always holds the scene's own values. Only the latest scan applies, and
  // only while still stopped (a Play can land while the read is in flight).
  let scanSerial = 0;
  const scanEngineScene = async (): Promise<void> => {
    if (!isStopped()) return;
    const serial = ++scanSerial;
    let scanned: EngineSceneState | null = null;
    try {
      scanned = parseEngineSceneState(await send('crdt_snapshot', []));
    } catch {
      // No scene pinned yet: keep what was known, a later scan fills it in.
    }
    if (disposed || serial !== scanSerial || !isStopped()) return;
    if (scanned) engineScene = scanned;
    refreshCameraAttachments();
    applySceneLighting(true);
  };

  const MESH_COLLIDER = 'core::MeshCollider';
  // Make a MeshRenderer PRIMITIVE pointer-pickable in the editor. Unlike a GLTF
  // (whose visibleMeshesCollisionMask carries pointer collision, #1373), a bare
  // MeshRenderer has no collider, so a viewport click can't hit it — it's only
  // selectable from the tree. Forward an editor-only MeshCollider of the SAME shape
  // with CL_POINTER so it's clickable, matching Babylon (every visible mesh is
  // pickable). Skip when the entity has its OWN MeshCollider — forwardSet already
  // ORs CL_POINTER into that. Editor-only: the CRDT keeps just the MeshRenderer, so
  // preview/live collision is unchanged. The MeshCollider mesh oneof mirrors the
  // renderer's (extra fields like `uvs` are ignored by the collider schema).
  const forwardMeshPickCollider = (entity: Entity, renderer: unknown) => {
    const authored = (
      context.getForwardableComponent(MESH_COLLIDER) as {
        getOrNull?: (e: Entity) => unknown;
      } | null
    )?.getOrNull?.(entity);
    if (authored != null) return;
    const mesh = (renderer as { mesh?: unknown } | null | undefined)?.mesh;
    if (mesh == null) return;
    enqueue(entity, async () => {
      await ensureInstantiated(entity);
      await forwardSet(entity, 'MeshCollider', { collisionMask: CL_POINTER, mesh });
    });
  };

  /** Base64 of the entity's Name serialized with its SDK schema (for `/new_entity`). */
  const encodeName = (v: unknown): string => {
    const buf = new ReadWriteByteBuffer();
    context.Name.schema.serialize(v as never, buf);
    let binary = '';
    for (const byte of buf.toBinary()) binary += String.fromCharCode(byte);
    return btoa(binary);
  };

  /** Instantiate a new entity in the engine scene at its exact id, anchored by its
   * Name (a custom component — `/new_entity` requires one; engine components flow
   * one-way and can't create an entity). No-op if already instantiated. */
  const ensureInstantiated = async (entity: Entity): Promise<void> => {
    if (instantiated.has(entity)) return;
    const name = context.Name.getOrNull(entity);
    if (name == null) return; // no Name yet — can't anchor; a later Name PUT will
    instantiated.add(entity);
    const data = encodeName(name);
    try {
      await send('new_entity', [
        String(context.Name.componentId),
        data,
        '0',
        '--ids',
        String(entity),
      ]);
    } catch (error) {
      instantiated.delete(entity); // let a retry re-attempt
      onError(`new_entity ${entity}`, error);
    }
  };

  const off = context.onChange(
    (
      entity: Entity,
      op: CrdtMessageType,
      component?: ComponentDefinition<unknown>,
      value?: unknown,
    ) => {
      // Suppress the initial load burst; forward only once armed.
      if (!shouldForward()) return;
      if (op === CrdtMessageType.DELETE_ENTITY) {
        forgetEntity(entity);
        fire(`delete_entity ${entity}`, 'delete_entity', [String(entity)]);
        return;
      }
      if (!component) return;

      // A Name PUT marks a (new) entity — instantiate it in the engine scene, then
      // (re)send the engine components it already carries, since they typically
      // arrive in the same batch BEFORE the Name (and were rejected as "missing
      // entity"). Editing an existing entity never writes Name, so this only fires
      // for genuinely new entities.
      if (op === CrdtMessageType.PUT_COMPONENT && component.componentName === NAME_COMPONENT) {
        enqueue(entity, async () => {
          await ensureInstantiated(entity);
          for (const [ecsName, engineName] of Object.entries(ENGINE_COMPONENT_NAMES)) {
            // VisibilityComponent is decided by the editor, not the authored value
            // (see forwardEditorVisibility) — forward it separately, below.
            if (engineName === 'VisibilityComponent') continue;
            // A shown placeholder stands in for the authored model (forwarded below).
            if (engineName === 'GltfContainer' && shownPlaceholderSrc(entity) != null) continue;
            // Stopped, the viewport keeps the editor's lighting (applySceneLighting).
            if (isStopped() && isRootLighting(entity, engineName)) continue;
            const c = context.getForwardableComponent(ecsName) as {
              getOrNull?: (e: Entity) => unknown;
            } | null;
            const v = c?.getOrNull?.(entity);
            if (v != null) await forwardSet(entity, engineName, v);
          }
        });
        // Always assert the editor's effective visibility for a new entity (visible
        // unless Hidden) — overriding any authored VisibilityComponent{false}.
        forwardEditorVisibility(entity);
        // A new entity that carries a Placeholder — render its placeholder GLTF (a
        // dropped Trigger Area, an invisible wall etc.) while stopped.
        if (isStopped() && context.editorComponents.Placeholder.getOrNull(entity) != null) {
          forwardPlaceholder(entity, false);
        }
        return;
      }

      const isEntityTeardown =
        op === CrdtMessageType.DELETE_COMPONENT && component.componentName === NAME_COMPONENT;
      if (isEntityTeardown) {
        forgetEntity(entity);
        fire(`delete_entity ${entity}`, 'delete_entity', [String(entity)]);
        return;
      }

      // Editor `Hide` OR authored `VisibilityComponent` → the editor's effective
      // visibility (see forwardEditorVisibility). Both are re-derived there, so a
      // change to either just re-asserts the computed value.
      if (
        component.componentName === context.editorComponents.Hide.componentName ||
        component.componentName === 'core::VisibilityComponent'
      ) {
        forwardEditorVisibility(entity);
        return;
      }

      // Editor `Placeholder` → engine GltfContainer (see forwardPlaceholder): shows
      // the placeholder GLTF in the editor for model-less items (Trigger Area etc).
      if (component.componentName === context.editorComponents.Placeholder.componentName) {
        forwardPlaceholder(entity, op === CrdtMessageType.DELETE_COMPONENT);
        return;
      }

      const engineName = ENGINE_COMPONENT_NAMES[component.componentName];
      // Deferred (custom/schema) components: skip rather than mis-address them.
      if (!engineName) return;

      // While stopped a placeholder stands in for the authored model; Play forwards
      // the authored value as it is then (see forwardPlaceholder).
      if (engineName === 'GltfContainer' && shownPlaceholderSrc(entity) != null) return;

      // Root lighting: saved as always, but while stopped the viewport keeps the
      // editor's lighting and Play applies the edit (see applySceneLighting).
      if (isRootLighting(entity, engineName)) {
        editedRootLighting.set(engineName, ++lightingEditSerial);
        if (isStopped()) return;
      }

      // A reparent, or an AvatarAttach added or removed, can move a subtree onto or
      // off the player or its camera.
      const hiding = hiddenCameraAttachments.size > 0 || hideCameraAttachments();
      if (
        hiding &&
        ((component.componentName === context.Transform.componentName &&
          parentChanged(entity, op, value)) ||
          component.componentName === AVATAR_ATTACH)
      ) {
        refreshCameraAttachments();
      }

      if (op === CrdtMessageType.DELETE_COMPONENT) {
        fire(`delete_component ${entity} ${engineName}`, 'delete_component', [
          String(entity),
          engineName,
        ]);
        return;
      }

      // PUT_COMPONENT (and its NETWORK variant): write the current value as JSON.
      // Prefer the onChange value; fall back to a live read if absent.
      const readable = component as { getOrNull?: (e: Entity) => unknown };
      const current = value ?? readable.getOrNull?.(entity);
      if (current == null) return;
      // A time-based component PUT that arrives WHILE THE SCENE IS FROZEN must be
      // forced to its paused override, not forwarded as-is. After a Stop/reload the
      // scene's GLTFs re-load and re-PUT their authored Animator (playing:true) —
      // which would resume playback a beat after the freeze (#1421: "animations run
      // for a couple frames after stop"); likewise a VideoPlayer (#1469) or
      // ParticleSystem (#1467) added/loaded while paused must not start playing.
      // Route it through forwardFrozenOverride so a frozen scene stays paused.
      if (isFrozen() && FROZEN_OVERRIDES.has(engineName)) {
        forwardFrozenOverride(entity, engineName, current, true);
        return;
      }
      // Serialize the forward so a component arriving before its entity's Name PUT
      // still lands (the Name PUT's re-send will cover it if this one raced the
      // instantiation). Instantiate-if-needed keeps a lone engine edit on a new
      // entity working even without a Name (best effort; usually Name drives it).
      enqueue(entity, async () => {
        await ensureInstantiated(entity);
        await forwardSet(entity, engineName, current);
      });
      // A MeshRenderer primitive has no collider — add an editor-only pointer one so
      // it's selectable in the viewport, not just the tree. Re-derived on every
      // MeshRenderer PUT so a shape change updates the collider too.
      if (engineName === 'MeshRenderer') forwardMeshPickCollider(entity, current);
    },
  );

  /** Send one engine component, refreshing the content map first for GltfContainer
   * (its `src` points at a project file the engine must be able to resolve). */
  async function forwardSet(entity: Entity, engineName: string, current: unknown): Promise<void> {
    let payload = current;
    if (engineName === 'GltfContainer') {
      // #1373: in the EDITOR, force all visible meshes to be pointer-pickable so a
      // model without an authored pointer collider can still be clicked to select
      // it (matching Babylon, where every visible mesh is pickable). CL_POINTER is
      // bit 1; OR it into visibleMeshesCollisionMask. Editor-only — the CRDT keeps
      // the authored mask, so preview/live collision is unchanged.
      const g = (current ?? {}) as { visibleMeshesCollisionMask?: number; src?: string };
      payload = {
        ...g,
        visibleMeshesCollisionMask: (g.visibleMeshesCollisionMask ?? 0) | CL_POINTER,
      };
      await refreshContentForGltf(entity, g.src);
    } else if (engineName === 'MeshCollider') {
      // Same idea for an authored MeshCollider: OR CL_POINTER into its mask so the
      // primitive is clickable in the editor even if it was authored without pointer
      // collision. Editor-only — the CRDT keeps the authored mask.
      const m = (current ?? {}) as { collisionMask?: number };
      payload = { ...m, collisionMask: (m.collisionMask ?? 0) | CL_POINTER };
    }
    const setArgs = [String(entity), engineName, JSON.stringify(toEngineJson(payload))];
    try {
      await send('set_component', setArgs);
    } catch (error) {
      onError(`set_component ${entity} ${engineName}`, error);
    }
  }

  /**
   * Refresh the engine's content map before a GltfContainer set — and, if the src
   * file isn't in it yet, retry until it appears (#1459). `/scene_content` re-fetches
   * the dev server's content map (which globs the whole project), so a freshly
   * dropped asset's file lands there once its write propagates. On Windows that
   * write lags the CRDT edit, so the first refresh misses it, the engine can't
   * resolve the src, and the model never renders until a preview forces a full
   * reload. Waiting for the src to appear closes that race; on a fast FS (Mac) it's
   * already present on the first pass, so this returns immediately. Bounded — a src
   * that's genuinely absent (or unparseable reply) falls through and sets anyway.
   */
  async function refreshContentForGltf(entity: Entity, src?: string): Promise<void> {
    const target = src?.toLowerCase();
    const MAX_ATTEMPTS = 8;
    const RETRY_DELAY_MS = 120;
    for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
      let reply: string;
      try {
        reply = await send('scene_content', []);
      } catch (error) {
        onError(`scene_content (before ${entity} GltfContainer)`, error);
        return;
      }
      // No src to resolve, or the reply isn't the expected file-list JSON → the
      // refresh alone is all we can do; proceed to set_component.
      if (!target) return;
      let files: unknown;
      try {
        files = JSON.parse(reply);
      } catch {
        return;
      }
      if (!Array.isArray(files)) return;
      // `/scene_content` returns content-map keys already lowercased.
      if (files.some(f => typeof f === 'string' && f.toLowerCase() === target)) return;
      if (attempt < MAX_ATTEMPTS - 1) {
        await new Promise(resolve => setTimeout(resolve, RETRY_DELAY_MS));
        if (disposed) return;
      }
    }
    // Timed out waiting for the file — set anyway; a later refresh/preview picks it up.
  }

  // Freeze/resume time-based playback with the scene freeze. Freezing stops the SDK7
  // tick but the engine keeps advancing each of these on its own, so while FROZEN we
  // forward a paused override; on UNFREEZE we forward the AUTHORED value so it resumes
  // as the scene intends. The CRDT always keeps the authored value — this only changes
  // what the engine plays. Each entry maps the engine component to its paused override
  // (returns null to skip a missing value):
  //  - Animator (#1382/#1421): AnimationPlayers advance clips from states[].playing —
  //    force every state playing:false (a null Animator still pauses as {states:[]}).
  //  - VideoPlayer (#1469): a loaded video keeps playing — force playing:false.
  //  - ParticleSystem (#1467): emission/simulation keeps running — force PS_PAUSED.
  //  - AudioSource / AudioStream: sound keeps playing — force playing:false.
  const pausePlaying = (raw: unknown) =>
    raw == null ? null : { ...(raw as Record<string, unknown>), playing: false };
  const FROZEN_OVERRIDES = new Map<string, (raw: unknown) => unknown>([
    [
      'Animator',
      raw => {
        const a = raw as { states?: Array<Record<string, unknown>> } | null | undefined;
        return { states: (a?.states ?? []).map(s => ({ ...s, playing: false })) };
      },
    ],
    ['VideoPlayer', pausePlaying],
    ['AudioSource', pausePlaying],
    ['AudioStream', pausePlaying],
    [
      'ParticleSystem',
      raw =>
        raw == null
          ? null
          : { ...(raw as Record<string, unknown>), playbackState: PARTICLE_PLAYBACK_PAUSED },
    ],
  ]);

  // Forward one time-based component honoring the freeze state: paused override while
  // FROZEN, authored value while running. A null value (missing, or an override that
  // opted out) is skipped. (The editor's mute is NOT applied here: it lives on the
  // engine page's master mute, see engine-audio.ts.)
  const forwardFrozenOverride = (
    entity: Entity,
    engineName: string,
    raw: unknown,
    frozen: boolean,
  ) => {
    const override = FROZEN_OVERRIDES.get(engineName);
    const value = frozen && override ? override(raw) : raw;
    if (value == null) return;
    enqueue(entity, async () => {
      await ensureInstantiated(entity);
      await forwardSet(entity, engineName, value);
    });
  };

  // Freeze/resume all time-based playback (GLTF animation clips #1382, video #1469,
  // particle systems #1467) with the scene run state. Called on arm (boot frozen)
  // and by the run/freeze toggle.
  const setPlaybackFrozen = (frozen: boolean) => {
    for (const engineName of FROZEN_OVERRIDES.keys()) {
      const component = context.getForwardableComponent(`core::${engineName}`);
      if (!component) continue;
      for (const [entity, value] of context.engine.getEntitiesWith(component)) {
        forwardFrozenOverride(entity, engineName, value, frozen);
      }
    }
  };

  const setSceneFrozen = (frozen: boolean) => {
    setPlaybackFrozen(frozen);
    // Play swaps the placeholders a stopped scene showed for what the scene has. A
    // Pause, and a Play that follows one, leave the models as they are.
    if (frozen ? isStopped() : placeholdersShown) {
      for (const [entity] of context.engine.getEntitiesWith(context.editorComponents.Placeholder)) {
        forwardPlaceholder(entity, false, frozen);
      }
      if (!frozen) placeholdersShown = false;
    }
    if (!shouldForward()) return;
    if (!frozen) {
      if (editorLightingShown) applySceneLighting(false);
      else resumeSceneClock();
    } else if (isStopped()) {
      void scanEngineScene();
    } else {
      holdSceneClock();
    }
  };

  // Re-establish the editor overrides after a scene `reload` (Stop/#1376). The
  // reload RE-CREATES the scene's engine entities at their original ids, so:
  //  - every Name-bearing entity already exists in the engine again — mark them
  //    instantiated so the re-forward doesn't `/new_entity` them (which collides:
  //    "id 512 already live") and drop the placeholder/visibility override (the
  //    bug where a Trigger Area's placeholder vanished after Stop and never came
  //    back), and
  //  - the fresh entities lost the forwarded overrides (placeholder GLTF, editor
  //    visibility, pointer-pickable mask, animation pause) — replay them.
  const reconcileAfterReload = () => {
    runtimeVisibility.clear();
    // The reload re-ran the scene's code: what it made before is gone, and what
    // it makes now is found by the scan at the end of the reconcile.
    engineScene = emptyEngineSceneState();
    // Root lighting edits made before the reload started were saved, so the
    // reloaded scene starts from them and its code may set its own on top: the
    // next scan holds the scene's values. Later edits keep their marks.
    const clearUpTo = reloadStartSerial ?? lightingEditSerial;
    reloadStartSerial = null;
    for (const [name, serial] of editedRootLighting) {
      if (serial <= clearUpTo) editedRootLighting.delete(name);
    }
    for (const entity of hiddenCameraAttachments) {
      if (!isEditorEntity(entity)) hiddenCameraAttachments.delete(entity);
    }
    for (const [entity] of context.engine.getEntitiesWith(context.Name)) {
      instantiated.add(entity);
    }
    reconcileEditorOverridesOnArm();
  };

  return {
    disconnect: () => {
      disposed = true;
      if (armTimer !== null) clearTimeout(armTimer);
      off();
      // Drop per-entity queues + the instantiated set: the guarded `send` already
      // stops in-flight tasks from posting, and clearing releases their closures.
      queues.clear();
      instantiated.clear();
    },
    setSceneFrozen,
    syncSceneClock,
    refreshCameraAttachments: () => {
      // Before arming, the arm reconcile applies it once the scene is loaded.
      if (shouldForward()) refreshCameraAttachments();
    },
    refreshEngineScene: () => {
      if (shouldForward()) void scanEngineScene();
    },
    noteReloadStarted: () => {
      reloadStartSerial = lightingEditSerial;
    },
    reconcileAfterReload,
  };
}

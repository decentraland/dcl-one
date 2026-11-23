import type { Entity } from '@dcl/ecs';

/**
 * What the forward bridge reads from the engine's own copy of the inspected scene
 * (`/crdt_snapshot`). The inspector's ECS only holds the authored composite, so
 * anything the scene's code creates in `main()` (a flashlight parented to the
 * camera, a night sky on the root entity) is visible here and nowhere else. A
 * frozen scene's snapshot is its last tick, so it shows the scene's own values,
 * never the editor overrides queued on top of them.
 */
export interface EngineSceneState {
  /** Transform parent of every entity that has a Transform. */
  parents: Map<Entity, Entity>;
  /** Entities whose AvatarAttach follows the local player (no `avatarId`). */
  localAvatarAttached: Set<Entity>;
  /** Each entity's own VisibilityComponent value, when it has one. */
  visibility: Map<Entity, boolean>;
  /** The root entity's global lighting components, by engine name (see
   * {@link SCENE_LIGHTING_COMPONENTS}). An absent key means the scene left it unset. */
  rootLighting: Map<string, unknown>;
}

/**
 * Root-entity components that replace the editor's lighting, by engine console
 * name: the root `LightSource` drives the directional light (a disabled one
 * turns it off), `GlobalLight` and `Sun` its direction, colour and ambient,
 * `Skybox` the sky colours (which are also the ambient light), `EnvironmentFog`
 * the fog, and `SkyboxTime` the time of day.
 */
export const SCENE_LIGHTING_COMPONENTS: readonly string[] = [
  'LightSource',
  'GlobalLight',
  'Sun',
  'Skybox',
  'EnvironmentFog',
  'SkyboxTime',
];

const ROOT = 0 as Entity;

export function emptyEngineSceneState(): EngineSceneState {
  return {
    parents: new Map(),
    localAvatarAttached: new Set(),
    visibility: new Map(),
    rootLighting: new Map(),
  };
}

/**
 * Parse a `/crdt_snapshot` reply (`{ "<entity>": { "<Component>": <json> } }`).
 * Returns null when the reply is not that shape (no scene pinned yet, an engine
 * error string), so the caller keeps what it knew.
 */
export function parseEngineSceneState(reply: string): EngineSceneState | null {
  let snapshot: unknown;
  try {
    snapshot = JSON.parse(reply);
  } catch {
    return null;
  }
  if (!snapshot || typeof snapshot !== 'object' || Array.isArray(snapshot)) return null;

  const state = emptyEngineSceneState();
  for (const [key, value] of Object.entries(snapshot as Record<string, unknown>)) {
    const entity = Number(key);
    if (!Number.isInteger(entity) || entity < 0 || !value || typeof value !== 'object') continue;
    const components = value as Record<string, unknown>;

    const transform = components.Transform as { parent?: unknown } | undefined;
    if (transform && typeof transform.parent === 'number' && transform.parent !== entity) {
      state.parents.set(entity as Entity, transform.parent as Entity);
    }

    const attach = components.AvatarAttach as { avatarId?: unknown } | undefined;
    if (attach && typeof attach === 'object' && attach.avatarId == null) {
      state.localAvatarAttached.add(entity as Entity);
    }

    const visibility = components.VisibilityComponent as { visible?: unknown } | undefined;
    if (visibility && typeof visibility === 'object') {
      // An unset `visible` is the proto default: visible.
      state.visibility.set(entity as Entity, visibility.visible !== false);
    }

    if (entity === ROOT) {
      for (const name of SCENE_LIGHTING_COMPONENTS) {
        if (components[name] != null) state.rootLighting.set(name, components[name]);
      }
    }
  }
  return state;
}

/**
 * The entities that follow the local player or its camera: those whose Transform
 * parent chain reaches one of `anchors` (the scene's PlayerEntity and
 * CameraEntity), and those that carry, or descend from an entity that carries, an
 * AvatarAttach on the local player. Their descendants follow with them, so they
 * are included. A cyclic parent chain ends the walk.
 */
export function findPlayerAttachments(
  parents: ReadonlyMap<Entity, Entity | undefined>,
  localAvatarAttached: ReadonlySet<Entity>,
  anchors: ReadonlySet<Entity>,
): Set<Entity> {
  const attached = new Set<Entity>();
  const candidates = new Set<Entity>([...parents.keys(), ...localAvatarAttached]);
  for (const entity of candidates) {
    if (anchors.has(entity)) continue;
    const visited = new Set<Entity>();
    let current: Entity | undefined = entity;
    while (current !== undefined && !visited.has(current)) {
      if (localAvatarAttached.has(current)) {
        attached.add(entity);
        break;
      }
      visited.add(current);
      const parent = parents.get(current);
      if (parent !== undefined && anchors.has(parent)) {
        attached.add(entity);
        break;
      }
      current = parent;
    }
  }
  return attached;
}

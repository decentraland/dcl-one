import type { Entity } from '@dcl/ecs';

import { EDITOR_BUS_CHANNEL } from '@dcl/inspector-bevy-protocol';
import type {
  AgentToPage,
  AvatarHitbox,
  BusEnvelope,
  PageToScene,
} from '@dcl/inspector-bevy-protocol';
import type { BevySceneContext } from './BevySceneContext';

/**
 * Make the scene's NPCs clickable in the Bevy viewport. The engine gives a scene's avatars no
 * collider, so a click passes through them; this posts each NPC's pose to the editor agent,
 * which puts an invisible pointer box over it and reports a hit as a pick of the NPC.
 *
 * Only while editing: a running scene gets an empty set, so its NPCs behave as players will see
 * them. The boxes live in the agent's scene, so the published scene never has them.
 */

const AVATAR_SHAPE = 'core::AvatarShape';
// Below this are reserved entities: an AvatarShape on the Player changes the player's own avatar.
const FIRST_SCENE_ENTITY = 512;

interface Channel {
  postMessage(msg: unknown): void;
  onmessage: ((ev: { data: unknown }) => void) | null;
  close(): void;
}

export interface AvatarHitboxesBridgeOptions {
  context: Pick<
    BevySceneContext,
    | 'onChange'
    | 'editorComponents'
    | 'engine'
    | 'getForwardableComponent'
    | 'getEntityWorldPositions'
    | 'getEntityWorldRotation'
    | 'Transform'
  >;
  isRunning(): boolean;
  /** Test seam: the channel to post on. Defaults to a real BroadcastChannel. */
  channel?: Channel;
}

export interface AvatarHitboxesBridge {
  /** Re-post after the run state changes. */
  refresh(): void;
  disconnect(): void;
}

export function createAvatarHitboxesBridge(
  options: AvatarHitboxesBridgeOptions,
): AvatarHitboxesBridge {
  const { context, isRunning } = options;
  const channel = options.channel ?? (new BroadcastChannel(EDITOR_BUS_CHANNEL) as Channel);
  const Hide = context.editorComponents.Hide;
  const avatarShape = context.getForwardableComponent(AVATAR_SHAPE);

  let lastPosted: string | undefined;

  const collect = (): AvatarHitbox[] => {
    if (!avatarShape || isRunning()) return [];
    const npcs: Entity[] = [];
    for (const [entity] of context.engine.getEntitiesWith(avatarShape)) {
      if (entity < FIRST_SCENE_ENTITY || Hide.getOrNull(entity)?.value) continue;
      npcs.push(entity);
    }
    const positions = context.getEntityWorldPositions(npcs);
    const hitboxes: AvatarHitbox[] = [];
    for (const entity of npcs) {
      const position = positions.get(entity);
      const rotation = context.getEntityWorldRotation(entity);
      const scale = context.Transform.getOrNull(entity)?.scale;
      if (!position || !rotation || !scale) continue;
      hitboxes.push({
        entity,
        position: { x: position.x, y: position.y, z: position.z },
        rotation: { x: rotation.x, y: rotation.y, z: rotation.z, w: rotation.w },
        scale: { x: scale.x, y: scale.y, z: scale.z },
      });
    }
    return hitboxes;
  };

  const post = (force = false) => {
    const msg: PageToScene = { kind: 'set-avatar-hitboxes', hitboxes: collect() };
    const serialized = JSON.stringify(msg);
    // A forced post answers an agent (re)boot, which lost the boxes.
    if (!force && serialized === lastPosted) return;
    lastPosted = serialized;
    const envelope: BusEnvelope = { to: 'scene', msg };
    channel.postMessage(envelope);
  };

  const watched = new Set([
    avatarShape?.componentId,
    context.Transform.componentId,
    Hide.componentId,
  ]);
  const off = context.onChange((_entity, _op, component) => {
    // An entity delete carries no component.
    if (!component || watched.has(component.componentId)) post();
  });

  channel.onmessage = ({ data }: { data: unknown }) => {
    if (!data || typeof data !== 'object') return;
    const env = data as Partial<BusEnvelope>;
    if (env.to !== 'page' || !env.msg || typeof env.msg !== 'object') return;
    if ((env.msg as AgentToPage).kind === 'editor-ready') post(true);
  };

  post();

  return {
    refresh: () => post(),
    disconnect: () => {
      channel.onmessage = null;
      off();
      channel.close();
    },
  };
}
